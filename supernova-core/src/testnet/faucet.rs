use std::collections::{HashMap, VecDeque};
use std::time::{Duration, Instant};
use thiserror::Error;
use tracing::{info, warn};

/// Errors that can occur during faucet operations
#[derive(Error, Debug)]
pub enum FaucetError {
    #[error("Per-address cooldown active; try again in {remaining_time} seconds")]
    CooldownPeriod { remaining_time: u64 },
    #[error("Per-IP rate limit exceeded ({limit} claims / {window}s); retry after {remaining_time} seconds")]
    IpRateLimitExceeded {
        limit: u32,
        window: u64,
        remaining_time: u64,
    },
    #[error("Daily distribution limit exceeded")]
    DailyLimitExceeded,
    #[error("Insufficient funds in faucet")]
    InsufficientFunds,
    #[error("Invalid recipient address: {0}")]
    InvalidAddress(String),
    #[error("Faucet is disabled")]
    FaucetDisabled,
    #[error("Invalid proof-of-work challenge solution")]
    PowChallengeInvalid,
    #[error("Internal error: {0}")]
    Internal(String),
}

/// Per-IP rate-limit configuration.
#[derive(Debug, Clone)]
pub struct IpRateLimitConfig {
    /// Maximum claims allowed per IP inside the rolling window.
    pub limit: u32,
    /// Rolling-window length in seconds.
    pub window_secs: u64,
}

impl Default for IpRateLimitConfig {
    /// Plan A6 default: 3 claims per IP per 24 hours.
    fn default() -> Self {
        Self {
            limit: 3,
            window_secs: 24 * 60 * 60,
        }
    }
}

/// Faucet for distributing test coins
pub struct Faucet {
    /// Amount of coins to distribute per request
    distribution_amount: u64,
    /// Per-address cooldown in seconds between claims (plan: 86 400s).
    cooldown_period: u64,
    /// Last distribution time per address
    last_distribution: HashMap<String, Instant>,
    /// Per-IP rolling-window claim timestamps.
    ip_claims: HashMap<String, VecDeque<Instant>>,
    /// Per-IP limit configuration.
    ip_limit: IpRateLimitConfig,
    /// Total coins distributed
    total_distributed: u64,
    /// Distribution count
    distribution_count: u64,
    /// Rejections counted for observability (cooldown + IP limit + other).
    rejection_count: u64,
}

impl Faucet {
    /// Create a new test faucet with default per-IP limits (3 claims / 24h).
    pub fn new(distribution_amount: u64, cooldown_period: u64) -> Self {
        Self::with_ip_limit(distribution_amount, cooldown_period, IpRateLimitConfig::default())
    }

    /// Create a faucet with an explicit per-IP limit (for tests and ops overrides).
    pub fn with_ip_limit(
        distribution_amount: u64,
        cooldown_period: u64,
        ip_limit: IpRateLimitConfig,
    ) -> Self {
        info!(
            "Test faucet initialized: {} millinova per claim, {}s per-address cooldown, \
             {} claims / {}s per IP",
            distribution_amount, cooldown_period, ip_limit.limit, ip_limit.window_secs
        );

        Self {
            distribution_amount,
            cooldown_period,
            last_distribution: HashMap::new(),
            ip_claims: HashMap::new(),
            ip_limit,
            total_distributed: 0,
            distribution_count: 0,
            rejection_count: 0,
        }
    }

    /// Distribute coins to a recipient (no IP context — legacy entry point).
    ///
    /// For testnet HTTP faucets, prefer [`Faucet::distribute_coins_with_client`] so
    /// the per-IP rolling-window limit is enforced.
    pub fn distribute_coins(&mut self, recipient: &str) -> Result<u64, FaucetError> {
        self.distribute_coins_with_client(recipient, None)
    }

    /// Distribute coins with optional client-IP context.
    ///
    /// Enforces both:
    /// - Per-address cooldown (`cooldown_period`).
    /// - Per-IP rolling-window limit (`ip_limit`) when `client_ip` is provided.
    ///
    /// When `client_ip` is `None` the per-IP check is skipped. This preserves
    /// behavior for non-HTTP entry points (e.g. CLI-local testnet scripts) while
    /// keeping the API backwards compatible for existing callers.
    pub fn distribute_coins_with_client(
        &mut self,
        recipient: &str,
        client_ip: Option<&str>,
    ) -> Result<u64, FaucetError> {
        self.validate_address(recipient)?;

        let now = Instant::now();

        if let Some(last_time) = self.last_distribution.get(recipient) {
            let elapsed = last_time.elapsed();
            let cooldown = Duration::from_secs(self.cooldown_period);
            if elapsed < cooldown {
                self.rejection_count += 1;
                let remaining_time = cooldown.as_secs() - elapsed.as_secs();
                return Err(FaucetError::CooldownPeriod { remaining_time });
            }
        }

        if let Some(ip) = client_ip {
            self.check_ip_limit(ip, now)?;
        }

        self.last_distribution.insert(recipient.to_string(), now);
        if let Some(ip) = client_ip {
            self.ip_claims
                .entry(ip.to_string())
                .or_default()
                .push_back(now);
        }
        self.total_distributed += self.distribution_amount;
        self.distribution_count += 1;

        info!(
            "Faucet distributed {} millinova to {} (client_ip={:?})",
            self.distribution_amount, recipient, client_ip
        );

        Ok(self.distribution_amount)
    }

    fn check_ip_limit(&mut self, ip: &str, now: Instant) -> Result<(), FaucetError> {
        let window = Duration::from_secs(self.ip_limit.window_secs);
        let entry = self.ip_claims.entry(ip.to_string()).or_default();

        while let Some(front) = entry.front() {
            if now.duration_since(*front) > window {
                entry.pop_front();
            } else {
                break;
            }
        }

        if (entry.len() as u32) >= self.ip_limit.limit {
            let oldest = *entry.front().expect("non-empty after limit check");
            let elapsed = now.duration_since(oldest);
            let remaining_time = window
                .as_secs()
                .saturating_sub(elapsed.as_secs());
            self.rejection_count += 1;
            return Err(FaucetError::IpRateLimitExceeded {
                limit: self.ip_limit.limit,
                window: self.ip_limit.window_secs,
                remaining_time,
            });
        }

        Ok(())
    }

    /// Get faucet statistics
    pub fn get_statistics(&self) -> FaucetStatistics {
        FaucetStatistics {
            distribution_amount: self.distribution_amount,
            cooldown_period: self.cooldown_period,
            total_distributed: self.total_distributed,
            distribution_count: self.distribution_count,
            unique_recipients: self.last_distribution.len(),
            tracked_ips: self.ip_claims.len(),
            rejection_count: self.rejection_count,
            ip_limit: self.ip_limit.clone(),
        }
    }

    /// Validate a recipient address
    fn validate_address(&self, address: &str) -> Result<(), FaucetError> {
        if address.is_empty() {
            return Err(FaucetError::InvalidAddress(
                "Empty address is not valid".to_string(),
            ));
        }

        if !address.starts_with("test1") && !address.starts_with("tb1") {
            warn!("Address {} does not use testnet prefix", address);
        }

        Ok(())
    }

    pub fn set_distribution_amount(&mut self, amount: u64) {
        self.distribution_amount = amount;
        info!("Faucet distribution amount updated to {}", amount);
    }

    pub fn set_cooldown_period(&mut self, period: u64) {
        self.cooldown_period = period;
        info!("Faucet cooldown period updated to {} seconds", period);
    }

    /// Override the per-IP limit (operator-only, typically for runtime tuning).
    pub fn set_ip_limit(&mut self, ip_limit: IpRateLimitConfig) {
        info!(
            "Faucet per-IP limit updated: {} claims / {}s",
            ip_limit.limit, ip_limit.window_secs
        );
        self.ip_limit = ip_limit;
    }

    pub fn clear_cooldown(&mut self, address: &str) {
        if self.last_distribution.remove(address).is_some() {
            info!("Cleared cooldown for address {}", address);
        }
    }

    /// Clear the tracked-claim history for a given IP.
    pub fn clear_ip_history(&mut self, ip: &str) {
        if self.ip_claims.remove(ip).is_some() {
            info!("Cleared IP history for {}", ip);
        }
    }

    pub fn reset_all_cooldowns(&mut self) {
        let count = self.last_distribution.len();
        self.last_distribution.clear();
        self.ip_claims.clear();
        info!("Reset cooldowns + IP history for {} addresses", count);
    }

    /// Count of rejected claims — exported so the HTTP layer can emit metrics.
    pub fn rejection_count(&self) -> u64 {
        self.rejection_count
    }
}

/// PoW challenge helper for optional bot deterrence.
///
/// A caller issues a random challenge byte string and a difficulty (leading-zero
/// bits). The client must find a nonce such that `SHA3-256(challenge || nonce)`
/// has at least `difficulty` leading zero bits. This reuses the same style as
/// `node::network::eclipse_prevention` but lives here so non-node crates can use
/// it (e.g. test harnesses).
pub mod pow {
    use sha3::{Digest, Sha3_256};

    /// Count leading zero bits of `hash`.
    fn leading_zero_bits(hash: &[u8]) -> u32 {
        let mut count = 0u32;
        for &b in hash {
            if b == 0 {
                count += 8;
            } else {
                count += b.leading_zeros();
                break;
            }
        }
        count
    }

    /// Verify that `SHA3-256(challenge || nonce_le_bytes)` satisfies `difficulty`.
    pub fn verify_solution(challenge: &[u8], nonce: u64, difficulty: u32) -> bool {
        let mut hasher = Sha3_256::new();
        hasher.update(challenge);
        hasher.update(nonce.to_le_bytes());
        let hash = hasher.finalize();
        leading_zero_bits(&hash) >= difficulty
    }

    /// Brute-force a solving nonce — test helper only.
    #[cfg(test)]
    pub fn solve(challenge: &[u8], difficulty: u32) -> u64 {
        (0u64..u64::MAX)
            .find(|&n| verify_solution(challenge, n, difficulty))
            .expect("PoW solving bounded by u64::MAX")
    }
}

/// Statistics about faucet usage
#[derive(Debug, Clone)]
pub struct FaucetStatistics {
    pub distribution_amount: u64,
    pub cooldown_period: u64,
    pub total_distributed: u64,
    pub distribution_count: u64,
    pub unique_recipients: usize,
    pub tracked_ips: usize,
    pub rejection_count: u64,
    pub ip_limit: IpRateLimitConfig,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread::sleep;

    fn short_ip_limit() -> IpRateLimitConfig {
        IpRateLimitConfig { limit: 2, window_secs: 60 }
    }

    #[test]
    fn distribute_succeeds_then_rejects_within_cooldown() {
        let mut faucet = Faucet::new(1000, 10);
        assert!(faucet.distribute_coins("test1abc").is_ok());
        let err = faucet.distribute_coins("test1abc").unwrap_err();
        assert!(matches!(err, FaucetError::CooldownPeriod { .. }));
    }

    #[test]
    fn cooldown_clears_after_period() {
        let mut faucet = Faucet::new(1000, 1);
        assert!(faucet.distribute_coins("test1abc").is_ok());
        sleep(Duration::from_secs(2));
        assert!(faucet.distribute_coins("test1abc").is_ok());
    }

    #[test]
    fn per_ip_limit_rejects_once_window_full() {
        let mut faucet = Faucet::with_ip_limit(1000, 0, short_ip_limit());

        // Two different addresses, same IP: first two succeed, third rejected by IP limit.
        assert!(faucet.distribute_coins_with_client("test1a", Some("1.2.3.4")).is_ok());
        assert!(faucet.distribute_coins_with_client("test1b", Some("1.2.3.4")).is_ok());
        let err = faucet
            .distribute_coins_with_client("test1c", Some("1.2.3.4"))
            .unwrap_err();
        assert!(
            matches!(err, FaucetError::IpRateLimitExceeded { limit: 2, .. }),
            "unexpected error: {err:?}"
        );
    }

    #[test]
    fn per_ip_limit_scoped_to_ip() {
        let mut faucet = Faucet::with_ip_limit(1000, 0, short_ip_limit());

        assert!(faucet.distribute_coins_with_client("test1a", Some("1.1.1.1")).is_ok());
        assert!(faucet.distribute_coins_with_client("test1b", Some("1.1.1.1")).is_ok());
        // Different IP gets its own budget.
        assert!(faucet.distribute_coins_with_client("test1c", Some("2.2.2.2")).is_ok());
    }

    #[test]
    fn address_cooldown_still_applies_with_ip_context() {
        let mut faucet = Faucet::with_ip_limit(
            1000,
            60, // 60s cooldown
            IpRateLimitConfig { limit: 10, window_secs: 60 },
        );

        assert!(faucet
            .distribute_coins_with_client("test1abc", Some("9.9.9.9"))
            .is_ok());
        let err = faucet
            .distribute_coins_with_client("test1abc", Some("9.9.9.9"))
            .unwrap_err();
        assert!(matches!(err, FaucetError::CooldownPeriod { .. }));
    }

    #[test]
    fn second_claim_within_24h_is_rejected_per_plan_a6() {
        // Production defaults: plan specifies 1 claim / 24h per address.
        let mut faucet = Faucet::new(1000, 24 * 60 * 60);
        assert!(faucet
            .distribute_coins_with_client("test1rcp", Some("203.0.113.5"))
            .is_ok());
        let err = faucet
            .distribute_coins_with_client("test1rcp", Some("203.0.113.5"))
            .unwrap_err();
        match err {
            FaucetError::CooldownPeriod { remaining_time } => {
                assert!(remaining_time > 0 && remaining_time <= 24 * 60 * 60);
            }
            other => panic!("expected CooldownPeriod, got {other:?}"),
        }
    }

    #[test]
    fn third_ip_claim_within_24h_is_rejected_per_plan_a6() {
        // Plan A6: per-IP 3 claims / 24h across distinct addresses.
        let mut faucet = Faucet::with_ip_limit(
            1000,
            0, // disable address cooldown so only the IP rule applies
            IpRateLimitConfig::default(),
        );
        let ip = Some("198.51.100.7");
        assert!(faucet.distribute_coins_with_client("test1a", ip).is_ok());
        assert!(faucet.distribute_coins_with_client("test1b", ip).is_ok());
        assert!(faucet.distribute_coins_with_client("test1c", ip).is_ok());
        let err = faucet
            .distribute_coins_with_client("test1d", ip)
            .unwrap_err();
        assert!(matches!(
            err,
            FaucetError::IpRateLimitExceeded { limit: 3, window: 86400, .. }
        ));
    }

    #[test]
    fn statistics_reflect_rejections_and_tracked_state() {
        let mut faucet = Faucet::with_ip_limit(1000, 60, short_ip_limit());
        let _ = faucet.distribute_coins_with_client("test1a", Some("1.1.1.1"));
        let _ = faucet.distribute_coins_with_client("test1a", Some("1.1.1.1")); // rejected by cooldown
        let stats = faucet.get_statistics();
        assert_eq!(stats.distribution_count, 1);
        assert_eq!(stats.rejection_count, 1);
        assert_eq!(stats.unique_recipients, 1);
        assert_eq!(stats.tracked_ips, 1);
    }

    #[test]
    fn pow_solve_and_verify_roundtrip() {
        let challenge = b"test-challenge-abc";
        let difficulty = 8;
        let nonce = pow::solve(challenge, difficulty);
        assert!(pow::verify_solution(challenge, nonce, difficulty));
        assert!(!pow::verify_solution(challenge, nonce.wrapping_add(1), 24));
    }

    #[test]
    fn pow_rejects_wrong_challenge() {
        let nonce = pow::solve(b"challenge-a", 8);
        assert!(!pow::verify_solution(b"challenge-b", nonce, 8));
    }
}
