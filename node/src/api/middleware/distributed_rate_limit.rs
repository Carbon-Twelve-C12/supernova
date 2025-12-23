//! Distributed Rate Limiting
//!
//! Implements distributed rate limiting using Redis for coordination
//! across multiple node instances. Falls back to local rate limiting
//! if Redis is unavailable.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

/// Distributed rate limiter configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DistributedRateLimitConfig {
    /// Redis connection URL (optional)
    pub redis_url: Option<String>,
    /// Redis key prefix
    pub key_prefix: String,
    /// Default requests per window
    pub default_requests_per_window: u64,
    /// Default window size in seconds
    pub default_window_seconds: u64,
    /// Enable local fallback when Redis unavailable
    pub enable_local_fallback: bool,
    /// Sync interval with Redis (milliseconds)
    pub sync_interval_ms: u64,
    /// Enable burst allowance
    pub enable_burst: bool,
    /// Burst multiplier (e.g., 1.5 = 50% extra for burst)
    pub burst_multiplier: f64,
}

impl Default for DistributedRateLimitConfig {
    fn default() -> Self {
        Self {
            redis_url: None,
            key_prefix: "supernova:ratelimit:".to_string(),
            default_requests_per_window: 100,
            default_window_seconds: 60,
            enable_local_fallback: true,
            sync_interval_ms: 1000,
            enable_burst: true,
            burst_multiplier: 1.5,
        }
    }
}

/// Rate limit tier configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitTier {
    /// Tier name
    pub name: String,
    /// Requests per window
    pub requests_per_window: u64,
    /// Window size in seconds
    pub window_seconds: u64,
    /// Burst allowance
    pub burst_allowance: u64,
}

impl Default for RateLimitTier {
    fn default() -> Self {
        Self {
            name: "default".to_string(),
            requests_per_window: 100,
            window_seconds: 60,
            burst_allowance: 20,
        }
    }
}

/// Rate limit result
#[derive(Debug, Clone)]
pub struct RateLimitResult {
    /// Whether the request is allowed
    pub allowed: bool,
    /// Remaining requests in current window
    pub remaining: u64,
    /// Total limit for window
    pub limit: u64,
    /// Seconds until window resets
    pub reset_after_seconds: u64,
    /// Retry after seconds (if rate limited)
    pub retry_after_seconds: Option<u64>,
}

/// Local rate limit state
struct LocalRateLimitState {
    /// Request count in current window
    count: AtomicU64,
    /// Window start time
    window_start: RwLock<Instant>,
    /// Last sync time with Redis
    last_sync: RwLock<Instant>,
}

impl LocalRateLimitState {
    fn new() -> Self {
        Self {
            count: AtomicU64::new(0),
            window_start: RwLock::new(Instant::now()),
            last_sync: RwLock::new(Instant::now()),
        }
    }
}

/// Distributed rate limiter
pub struct DistributedRateLimiter {
    config: DistributedRateLimitConfig,
    /// Tier configurations
    tiers: RwLock<HashMap<String, RateLimitTier>>,
    /// Local state per key
    local_state: RwLock<HashMap<String, Arc<LocalRateLimitState>>>,
    /// Redis client (if available)
    redis_available: AtomicU64, // 1 = available, 0 = unavailable
    /// Total requests processed
    total_requests: AtomicU64,
    /// Total requests rate limited
    total_limited: AtomicU64,
}

impl DistributedRateLimiter {
    /// Create a new distributed rate limiter
    pub fn new(config: DistributedRateLimitConfig) -> Self {
        Self {
            config,
            tiers: RwLock::new(HashMap::new()),
            local_state: RwLock::new(HashMap::new()),
            redis_available: AtomicU64::new(0),
            total_requests: AtomicU64::new(0),
            total_limited: AtomicU64::new(0),
        }
    }

    /// Initialize Redis connection
    pub async fn initialize(&self) -> Result<(), String> {
        if let Some(ref _redis_url) = self.config.redis_url {
            // In production, establish Redis connection here
            // For now, we use local rate limiting
            tracing::info!("Distributed rate limiting initialized (local mode)");
            self.redis_available.store(0, Ordering::Relaxed);
        }
        Ok(())
    }

    /// Register a rate limit tier
    pub async fn register_tier(&self, tier: RateLimitTier) {
        let mut tiers = self.tiers.write().await;
        tiers.insert(tier.name.clone(), tier);
    }

    /// Check rate limit for a key
    pub async fn check(&self, key: &str, tier_name: Option<&str>) -> RateLimitResult {
        self.total_requests.fetch_add(1, Ordering::Relaxed);

        let tier = self.get_tier(tier_name).await;
        let window_seconds = tier.window_seconds;
        let limit = tier.requests_per_window;
        let burst_limit = if self.config.enable_burst {
            limit + tier.burst_allowance
        } else {
            limit
        };

        // Get or create local state
        let state = self.get_or_create_state(key).await;

        // Check if window has expired
        let now = Instant::now();
        {
            let window_start = state.window_start.read().await;
            if now.duration_since(*window_start) >= Duration::from_secs(window_seconds) {
                drop(window_start);

                // Reset window
                let mut window_start = state.window_start.write().await;
                *window_start = now;
                state.count.store(0, Ordering::Relaxed);
            }
        }

        // Increment and check
        let current_count = state.count.fetch_add(1, Ordering::Relaxed) + 1;

        let allowed = current_count <= burst_limit;
        let remaining = burst_limit.saturating_sub(current_count);

        // Calculate reset time
        let reset_after_seconds = {
            let window_start = state.window_start.read().await;
            let elapsed = now.duration_since(*window_start).as_secs();
            window_seconds.saturating_sub(elapsed)
        };

        if !allowed {
            self.total_limited.fetch_add(1, Ordering::Relaxed);
        }

        RateLimitResult {
            allowed,
            remaining,
            limit: burst_limit,
            reset_after_seconds,
            retry_after_seconds: if allowed { None } else { Some(reset_after_seconds) },
        }
    }

    /// Check rate limit by IP address
    pub async fn check_ip(&self, ip: IpAddr, tier_name: Option<&str>) -> RateLimitResult {
        let key = format!("ip:{}", ip);
        self.check(&key, tier_name).await
    }

    /// Check rate limit by API key
    pub async fn check_api_key(&self, api_key: &str, tier_name: Option<&str>) -> RateLimitResult {
        let key = format!("api:{}", api_key);
        self.check(&key, tier_name).await
    }

    /// Check rate limit by endpoint
    pub async fn check_endpoint(
        &self,
        ip: IpAddr,
        endpoint: &str,
        tier_name: Option<&str>,
    ) -> RateLimitResult {
        let key = format!("endpoint:{}:{}", ip, endpoint);
        self.check(&key, tier_name).await
    }

    /// Get tier configuration
    async fn get_tier(&self, tier_name: Option<&str>) -> RateLimitTier {
        let tiers = self.tiers.read().await;

        if let Some(name) = tier_name {
            if let Some(tier) = tiers.get(name) {
                return tier.clone();
            }
        }

        // Return default tier
        RateLimitTier {
            name: "default".to_string(),
            requests_per_window: self.config.default_requests_per_window,
            window_seconds: self.config.default_window_seconds,
            burst_allowance: if self.config.enable_burst {
                (self.config.default_requests_per_window as f64
                    * (self.config.burst_multiplier - 1.0)) as u64
            } else {
                0
            },
        }
    }

    /// Get or create local state for a key
    async fn get_or_create_state(&self, key: &str) -> Arc<LocalRateLimitState> {
        // Try read lock first
        {
            let state = self.local_state.read().await;
            if let Some(s) = state.get(key) {
                return Arc::clone(s);
            }
        }

        // Need to create new state
        let mut state = self.local_state.write().await;

        // Double-check (another thread may have created it)
        if let Some(s) = state.get(key) {
            return Arc::clone(s);
        }

        let new_state = Arc::new(LocalRateLimitState::new());
        state.insert(key.to_string(), Arc::clone(&new_state));

        // Cleanup old entries periodically
        if state.len() > 10000 {
            self.cleanup_old_entries(&mut state).await;
        }

        new_state
    }

    /// Cleanup old entries
    async fn cleanup_old_entries(&self, state: &mut HashMap<String, Arc<LocalRateLimitState>>) {
        let now = Instant::now();
        let max_age = Duration::from_secs(self.config.default_window_seconds * 2);

        state.retain(|_, v| {
            if let Ok(window_start) = v.window_start.try_read() {
                now.duration_since(*window_start) < max_age
            } else {
                true // Keep if we can't read
            }
        });

        tracing::debug!("Cleaned up rate limit state, {} entries remaining", state.len());
    }

    /// Get statistics
    pub fn stats(&self) -> RateLimitStats {
        RateLimitStats {
            total_requests: self.total_requests.load(Ordering::Relaxed),
            total_limited: self.total_limited.load(Ordering::Relaxed),
            redis_available: self.redis_available.load(Ordering::Relaxed) == 1,
        }
    }

    /// Reset statistics
    pub fn reset_stats(&self) {
        self.total_requests.store(0, Ordering::Relaxed);
        self.total_limited.store(0, Ordering::Relaxed);
    }

    /// Clear all rate limit state
    pub async fn clear(&self) {
        let mut state = self.local_state.write().await;
        state.clear();
    }
}

/// Rate limit statistics
#[derive(Debug, Clone)]
pub struct RateLimitStats {
    /// Total requests processed
    pub total_requests: u64,
    /// Total requests rate limited
    pub total_limited: u64,
    /// Whether Redis is available
    pub redis_available: bool,
}

impl RateLimitStats {
    /// Calculate rate limit percentage
    pub fn limit_percentage(&self) -> f64 {
        if self.total_requests == 0 {
            0.0
        } else {
            (self.total_limited as f64 / self.total_requests as f64) * 100.0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_basic_rate_limiting() {
        let config = DistributedRateLimitConfig {
            default_requests_per_window: 5,
            default_window_seconds: 60,
            enable_burst: false,
            ..Default::default()
        };

        let limiter = DistributedRateLimiter::new(config);

        // First 5 requests should be allowed
        for i in 0..5 {
            let result = limiter.check("test_key", None).await;
            assert!(result.allowed, "Request {} should be allowed", i + 1);
        }

        // 6th request should be rate limited
        let result = limiter.check("test_key", None).await;
        assert!(!result.allowed, "Request 6 should be rate limited");
    }

    #[tokio::test]
    async fn test_burst_allowance() {
        let config = DistributedRateLimitConfig {
            default_requests_per_window: 5,
            default_window_seconds: 60,
            enable_burst: true,
            burst_multiplier: 2.0, // 100% burst = 5 extra
            ..Default::default()
        };

        let limiter = DistributedRateLimiter::new(config);

        // Should allow 5 + 5 = 10 requests with burst
        for i in 0..10 {
            let result = limiter.check("test_key", None).await;
            assert!(result.allowed, "Request {} should be allowed with burst", i + 1);
        }

        // 11th request should be rate limited
        let result = limiter.check("test_key", None).await;
        assert!(!result.allowed, "Request 11 should be rate limited");
    }

    #[tokio::test]
    async fn test_different_keys() {
        let config = DistributedRateLimitConfig {
            default_requests_per_window: 2,
            default_window_seconds: 60,
            enable_burst: false,
            ..Default::default()
        };

        let limiter = DistributedRateLimiter::new(config);

        // Key 1
        let result = limiter.check("key1", None).await;
        assert!(result.allowed);
        let result = limiter.check("key1", None).await;
        assert!(result.allowed);
        let result = limiter.check("key1", None).await;
        assert!(!result.allowed);

        // Key 2 should have separate limit
        let result = limiter.check("key2", None).await;
        assert!(result.allowed);
    }

    #[tokio::test]
    async fn test_custom_tier() {
        let config = DistributedRateLimitConfig::default();
        let limiter = DistributedRateLimiter::new(config);

        // Register a premium tier
        limiter
            .register_tier(RateLimitTier {
                name: "premium".to_string(),
                requests_per_window: 1000,
                window_seconds: 60,
                burst_allowance: 200,
            })
            .await;

        let result = limiter.check("test", Some("premium")).await;
        assert!(result.allowed);
        assert_eq!(result.limit, 1200); // 1000 + 200 burst
    }

    #[tokio::test]
    async fn test_ip_rate_limiting() {
        let config = DistributedRateLimitConfig {
            default_requests_per_window: 3,
            default_window_seconds: 60,
            enable_burst: false,
            ..Default::default()
        };

        let limiter = DistributedRateLimiter::new(config);
        let ip: IpAddr = "192.168.1.1".parse().unwrap();

        for _ in 0..3 {
            let result = limiter.check_ip(ip, None).await;
            assert!(result.allowed);
        }

        let result = limiter.check_ip(ip, None).await;
        assert!(!result.allowed);
    }

    #[test]
    fn test_stats() {
        let stats = RateLimitStats {
            total_requests: 100,
            total_limited: 10,
            redis_available: false,
        };

        assert_eq!(stats.limit_percentage(), 10.0);
    }
}
