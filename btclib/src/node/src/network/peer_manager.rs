use std::collections::{HashMap, HashSet, VecDeque};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::time::{Duration, Instant};
use rand::{Rng, thread_rng};
use rand::distributions::Alphanumeric;
use sha2::{Sha256, Digest};

use libp2p::PeerId;
use serde::{Deserialize, Serialize};
use tracing::{debug, info, warn, error};

/// Represents a subnet of IP addresses for diversity tracking
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct IpSubnet {
    /// Base IP address
    pub base: IpAddr,
    /// Network mask bits (e.g., 24 for /24)
    pub mask_bits: u8,
}

impl IpSubnet {
    /// Create a new subnet from an IP address with default mask
    pub fn from_ip(ip: IpAddr) -> Self {
        match ip {
            IpAddr::V4(_) => Self {
                base: ip,
                mask_bits: 24, // /24 for IPv4 by default
            },
            IpAddr::V6(_) => Self {
                base: ip,
                mask_bits: 48, // /48 for IPv6 by default
            },
        }
    }

    /// Check if an IP address belongs to this subnet
    pub fn contains(&self, ip: &IpAddr) -> bool {
        match (self.base, ip) {
            (IpAddr::V4(base), IpAddr::V4(check)) => {
                let mask = !0u32 << (32 - self.mask_bits);
                (u32::from(base) & mask) == (u32::from(*check) & mask)
            }
            (IpAddr::V6(base), IpAddr::V6(check)) => {
                let mask_bits = self.mask_bits as u128;
                let mask = !0u128 << (128 - mask_bits);
                (u128::from(base) & mask) == (u128::from(*check) & mask)
            }
            _ => false, // Different IP versions
        }
    }
}

/// Score components for peer evaluation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerScore {
    /// Base score - higher is better
    pub base_score: f64,
    /// Connection stability score
    pub stability_score: f64,
    /// Behavior score based on protocol adherence
    pub behavior_score: f64,
    /// Latency score - higher means lower latency
    pub latency_score: f64,
    /// Diversity contribution score
    pub diversity_score: f64,
}

impl Default for PeerScore {
    fn default() -> Self {
        Self {
            base_score: 0.0,
            stability_score: 0.0,
            behavior_score: 0.0,
            latency_score: 0.0,
            diversity_score: 0.0,
        }
    }
}

impl PeerScore {
    /// Calculate total score
    pub fn total(&self) -> f64 {
        self.base_score +
            self.stability_score +
            self.behavior_score +
            self.latency_score +
            self.diversity_score
    }
}

/// Information about a peer connection attempt
#[derive(Debug, Clone)]
pub struct ConnectionAttempt {
    /// When the attempt was made
    pub timestamp: Instant,
    /// Whether the connection was successful
    pub successful: bool,
    /// Duration of the connection (None if still connected or connection failed)
    pub duration: Option<Duration>,
    /// Reason for disconnection if applicable
    pub disconnect_reason: Option<String>,
}

/// Rate limiting information for a specific IP address or subnet
#[derive(Debug, Clone)]
pub struct RateLimitInfo {
    /// IP address or subnet
    pub address: IpAddr,
    /// Recent connection attempts
    pub recent_attempts: Vec<Instant>,
    /// Time when this address was banned (if applicable)
    pub banned_until: Option<Instant>,
    /// Reason for banning
    pub ban_reason: Option<String>,
}

impl RateLimitInfo {
    /// Create a new rate limit tracker for an IP
    pub fn new(address: IpAddr) -> Self {
        Self {
            address,
            recent_attempts: Vec::new(),
            banned_until: None,
            ban_reason: None,
        }
    }

    /// Record a connection attempt
    pub fn record_attempt(&mut self) {
        self.recent_attempts.push(Instant::now());
        // Prune old attempts (older than 1 minute)
        let one_minute_ago = Instant::now() - Duration::from_secs(60);
        self.recent_attempts.retain(|time| *time >= one_minute_ago);
    }

    /// Check if rate limited
    pub fn is_rate_limited(&self, max_attempts: usize) -> bool {
        // If banned, it's rate limited
        if let Some(banned_until) = self.banned_until {
            if banned_until > Instant::now() {
                return true;
            }
        }

        // Check if too many recent attempts
        self.recent_attempts.len() > max_attempts
    }

    /// Ban this IP for a specific duration
    pub fn ban(&mut self, duration: Duration, reason: &str) {
        self.banned_until = Some(Instant::now() + duration);
        self.ban_reason = Some(reason.to_string());
    }
}

/// Challenge status for identity verification
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChallengeStatus {
    /// No challenge issued yet
    None,
    /// Challenge issued and waiting for response
    Pending {
        /// The challenge string
        challenge: String,
        /// When the challenge was issued
        issued_at: Instant,
        /// Number of attempts made
        attempts: u8,
    },
    /// Challenge successfully completed
    Verified {
        /// When verification occurred
        verified_at: Instant,
    },
    /// Challenge failed
    Failed {
        /// Reason for failure
        reason: String,
    },
}

/// Information about a connected peer
#[derive(Debug, Clone)]
pub struct PeerInfo {
    /// Peer ID
    pub peer_id: PeerId,
    /// IP address
    pub ip: IpAddr,
    /// Port number
    pub port: u16,
    /// When the peer was first seen
    pub first_seen: Instant,
    /// When the peer was last seen
    pub last_seen: Instant,
    /// Score information
    pub score: PeerScore,
    /// History of connection attempts with this peer
    pub connection_history: Vec<ConnectionAttempt>,
    /// Optional autonomous system number (AS)
    pub asn: Option<u32>,
    /// Optional geographic region
    pub region: Option<String>,
    /// Number of successful message exchanges
    pub successful_exchanges: u64,
    /// Number of failed message exchanges
    pub failed_exchanges: u64,
    /// Protocol versions supported
    pub protocols: Vec<String>,
    /// Identity verification challenge status
    pub challenge_status: ChallengeStatus,
    /// Behavior patterns tracking
    pub behavior_patterns: PeerBehavior,
}

/// Tracks peer behavior patterns for reputation scoring
#[derive(Debug, Clone, Default)]
pub struct PeerBehavior {
    /// Number of valid blocks announced
    pub valid_blocks_announced: u32,
    /// Number of invalid blocks announced
    pub invalid_blocks_announced: u32,
    /// Number of valid transactions relayed
    pub valid_txns_relayed: u32,
    /// Number of invalid transactions relayed
    pub invalid_txns_relayed: u32,
    /// Response time history (in milliseconds)
    pub response_times: VecDeque<u64>,
    /// Number of protocol violations
    pub protocol_violations: u32,
    /// Number of unexpected disconnections
    pub unexpected_disconnects: u32,
    /// Last seen software version
    pub client_version: Option<String>,
    /// Unusual message patterns detected
    pub unusual_patterns_detected: Vec<String>,
}

impl PeerBehavior {
    /// Create a new peer behavior tracker
    pub fn new() -> Self {
        Self {
            response_times: VecDeque::with_capacity(100),
            ..Default::default()
        }
    }

    /// Record message response time
    pub fn record_response_time(&mut self, time_ms: u64) {
        if self.response_times.len() >= 100 {
            self.response_times.pop_front();
        }
        self.response_times.push_back(time_ms);
    }

    /// Calculate average response time
    pub fn average_response_time(&self) -> Option<f64> {
        if self.response_times.is_empty() {
            return None;
        }

        let sum: u64 = self.response_times.iter().sum();
        Some(sum as f64 / self.response_times.len() as f64)
    }

    /// Record unusual behavior pattern
    pub fn record_unusual_pattern(&mut self, pattern: &str) {
        self.unusual_patterns_detected.push(pattern.to_string());
    }

    /// Calculate behavior reliability score (0-1)
    pub fn reliability_score(&self) -> f64 {
        let mut score = 1.0;

        // Penalize for invalid blocks/transactions
        let total_blocks = self.valid_blocks_announced + self.invalid_blocks_announced;
        if total_blocks > 0 {
            let invalid_ratio = self.invalid_blocks_announced as f64 / total_blocks as f64;
            score -= invalid_ratio * 0.3; // Up to 0.3 point penalty
        }

        let total_txns = self.valid_txns_relayed + self.invalid_txns_relayed;
        if total_txns > 0 {
            let invalid_ratio = self.invalid_txns_relayed as f64 / total_txns as f64;
            score -= invalid_ratio * 0.3; // Up to 0.3 point penalty
        }

        // Penalize for protocol violations
        score -= (self.protocol_violations as f64 * 0.05).min(0.2);

        // Penalize for unexpected disconnects
        score -= (self.unexpected_disconnects as f64 * 0.02).min(0.1);

        // Penalize for unusual patterns
        score -= (self.unusual_patterns_detected.len() as f64 * 0.05).min(0.1);

        score.max(0.0)
    }
}

impl PeerInfo {
    /// Create a new peer info
    pub fn new(peer_id: PeerId, ip: IpAddr, port: u16) -> Self {
        let now = Instant::now();
        Self {
            peer_id,
            ip,
            port,
            first_seen: now,
            last_seen: now,
            score: PeerScore::default(),
            connection_history: Vec::new(),
            asn: None,
            region: None,
            successful_exchanges: 0,
            failed_exchanges: 0,
            protocols: Vec::new(),
            challenge_status: ChallengeStatus::None,
            behavior_patterns: PeerBehavior::new(),
        }
    }

    /// Record a successful connection
    pub fn record_connection(&mut self) {
        self.last_seen = Instant::now();
        self.connection_history.push(ConnectionAttempt {
            timestamp: Instant::now(),
            successful: true,
            duration: None,
            disconnect_reason: None,
        });
    }

    /// Record a disconnection
    pub fn record_disconnection(&mut self, reason: Option<String>) {
        if let Some(last_attempt) = self.connection_history.last_mut() {
            if last_attempt.successful && last_attempt.duration.is_none() {
                last_attempt.duration = Some(Instant::now() - last_attempt.timestamp);
                last_attempt.disconnect_reason = reason;
            }
        }
    }

    /// Update peer score based on behavior
    pub fn update_score(&mut self) {
        // Base score increases slightly with age to favor long-term peers
        let age_days = self.first_seen.elapsed().as_secs_f64() / (24.0 * 3600.0);
        self.score.base_score = 1.0 + (age_days.min(30.0) * 0.1);

        // Stability score based on connection history
        if !self.connection_history.is_empty() {
            let successful = self.connection_history.iter()
                .filter(|attempt| attempt.successful)
                .count();
            let stability = successful as f64 / self.connection_history.len() as f64;
            self.score.stability_score = stability * 5.0;
        }

        // Behavior score based on successful vs. failed exchanges and behavior patterns
        if self.successful_exchanges + self.failed_exchanges > 0 {
            let total = self.successful_exchanges + self.failed_exchanges;
            let exchange_ratio = self.successful_exchanges as f64 / total as f64;

            // Combine with behavior reliability score
            let reliability = self.behavior_patterns.reliability_score();
            self.score.behavior_score = (exchange_ratio * 0.7 + reliability * 0.3) * 5.0;
        }

        // Adjust score based on verification status
        match &self.challenge_status {
            ChallengeStatus::Verified { .. } => {
                // Bonus for verified peers
                self.score.base_score += 1.0;
            },
            ChallengeStatus::Failed { .. } => {
                // Penalty for failed verification
                self.score.base_score = self.score.base_score.max(0.5);
            },
            _ => {}
        }
    }

    /// Get the latency if available
    pub fn get_latency(&self) -> Option<f64> {
        self.behavior_patterns.average_response_time()
    }
}

/// Manages peer diversity to prevent Sybil attacks
pub struct PeerDiversityManager {
    /// Distribution of peers by subnet
    pub subnet_distribution: HashMap<IpSubnet, HashSet<PeerId>>,
    /// Distribution by ASN if available
    pub asn_distribution: HashMap<u32, HashSet<PeerId>>,
    /// Geographic distribution if available
    pub geographic_distribution: HashMap<String, HashSet<PeerId>>,
    /// Maximum peers per subnet
    pub max_peers_per_subnet: usize,
    /// Maximum peers per ASN
    pub max_peers_per_asn: usize,
    /// Maximum peers per geographic region
    pub max_peers_per_region: usize,
}

impl PeerDiversityManager {
    /// Create a new peer diversity manager
    pub fn new() -> Self {
        Self {
            subnet_distribution: HashMap::new(),
            asn_distribution: HashMap::new(),
            geographic_distribution: HashMap::new(),
            max_peers_per_subnet: 3,
            max_peers_per_asn: 8,
            max_peers_per_region: 15,
        }
    }

    /// Add a peer to diversity tracking
    pub fn add_peer(&mut self, peer_id: PeerId, info: &PeerInfo) {
        // Add to subnet distribution
        let subnet = IpSubnet::from_ip(info.ip);
        self.subnet_distribution
            .entry(subnet)
            .or_insert_with(HashSet::new)
            .insert(peer_id);

        // Add to ASN distribution if available
        if let Some(asn) = info.asn {
            self.asn_distribution
                .entry(asn)
                .or_insert_with(HashSet::new)
                .insert(peer_id);
        }

        // Add to geographic distribution if available
        if let Some(region) = &info.region {
            self.geographic_distribution
                .entry(region.clone())
                .or_insert_with(HashSet::new)
                .insert(peer_id);
        }
    }

    /// Remove a peer from diversity tracking
    pub fn remove_peer(&mut self, peer_id: &PeerId, info: &PeerInfo) {
        // Remove from subnet distribution
        let subnet = IpSubnet::from_ip(info.ip);
        if let Some(peers) = self.subnet_distribution.get_mut(&subnet) {
            peers.remove(peer_id);
            if peers.is_empty() {
                self.subnet_distribution.remove(&subnet);
            }
        }

        // Remove from ASN distribution if available
        if let Some(asn) = info.asn {
            if let Some(peers) = self.asn_distribution.get_mut(&asn) {
                peers.remove(peer_id);
                if peers.is_empty() {
                    self.asn_distribution.remove(&asn);
                }
            }
        }

        // Remove from geographic distribution if available
        if let Some(region) = &info.region {
            if let Some(peers) = self.geographic_distribution.get_mut(region) {
                peers.remove(peer_id);
                if peers.is_empty() {
                    self.geographic_distribution.remove(region);
                }
            }
        }
    }

    /// Check if adding a peer would violate diversity limits
    pub fn would_violate_limits(&self, info: &PeerInfo) -> bool {
        // Check subnet limit
        let subnet = IpSubnet::from_ip(info.ip);
        if let Some(peers) = self.subnet_distribution.get(&subnet) {
            if peers.len() >= self.max_peers_per_subnet {
                return true;
            }
        }

        // Check ASN limit if available
        if let Some(asn) = info.asn {
            if let Some(peers) = self.asn_distribution.get(&asn) {
                if peers.len() >= self.max_peers_per_asn {
                    return true;
                }
            }
        }

        // Check geographic limit if available
        if let Some(region) = &info.region {
            if let Some(peers) = self.geographic_distribution.get(region) {
                if peers.len() >= self.max_peers_per_region {
                    return true;
                }
            }
        }

        false
    }

    /// Calculate diversity score for a peer
    pub fn calculate_diversity_score(&self, info: &PeerInfo) -> f64 {
        let mut score = 0.0;

        // Subnet diversity score - higher for less represented subnets
        let subnet = IpSubnet::from_ip(info.ip);
        let subnet_count = self.subnet_distribution
            .get(&subnet)
            .map_or(0, |peers| peers.len());

        // Add 1.0 - (count / max) to favor underrepresented subnets
        score += 1.0 - (subnet_count as f64 / self.max_peers_per_subnet as f64).min(1.0);

        // ASN diversity score if available
        if let Some(asn) = info.asn {
            let asn_count = self.asn_distribution
                .get(&asn)
                .map_or(0, |peers| peers.len());

            score += 1.0 - (asn_count as f64 / self.max_peers_per_asn as f64).min(1.0);
        }

        // Geographic diversity score if available
        if let Some(region) = &info.region {
            let region_count = self.geographic_distribution
                .get(region)
                .map_or(0, |peers| peers.len());

            score += 1.0 - (region_count as f64 / self.max_peers_per_region as f64).min(1.0);
        }

        // Normalize to 0-5 range
        let max_possible = 3.0; // One point for each diversity dimension
        (score / max_possible) * 5.0
    }
}

/// Manages peer connections and implements Sybil attack prevention
pub struct PeerManager {
    /// All known peers
    peers: HashMap<PeerId, PeerInfo>,
    /// Currently connected peers
    connected_peers: HashSet<PeerId>,
    /// Rate limit information by IP address
    rate_limits: HashMap<IpAddr, RateLimitInfo>,
    /// Rate limit information by IP subnet
    subnet_rate_limits: HashMap<IpSubnet, RateLimitInfo>,
    /// Peer diversity manager
    diversity_manager: PeerDiversityManager,
    /// Maximum connection attempts per minute per IP
    max_connection_attempts_per_min: usize,
    /// Connection challenge enabled
    enable_connection_challenges: bool,
    /// Challenge timeout in seconds
    challenge_timeout_seconds: u64,
    /// Challenge difficulty (0-255)
    challenge_difficulty: u8,
    /// Maximum challenge attempts
    max_challenge_attempts: u8,
    /// Forced peer rotation interval
    forced_rotation_interval: Duration,
    /// Last peer rotation time
    last_rotation_time: Instant,
}

impl PeerManager {
    /// Create a new peer manager
    pub fn new() -> Self {
        Self {
            peers: HashMap::new(),
            connected_peers: HashSet::new(),
            rate_limits: HashMap::new(),
            subnet_rate_limits: HashMap::new(),
            diversity_manager: PeerDiversityManager::new(),
            max_connection_attempts_per_min: 5,
            enable_connection_challenges: true,
            challenge_timeout_seconds: 30,
            challenge_difficulty: 16, // Moderate difficulty
            max_challenge_attempts: 3,
            forced_rotation_interval: Duration::from_secs(3600), // 1 hour
            last_rotation_time: Instant::now(),
        }
    }

    /// Generate a new challenge for peer verification
    pub fn generate_challenge(&self) -> String {
        let random_string: String = thread_rng()
            .sample_iter(&Alphanumeric)
            .take(32)
            .map(char::from)
            .collect();

        random_string
    }

    /// Verify a challenge response
    pub fn verify_challenge_response(&self, challenge: &str, response: &str, difficulty: u8) -> bool {
        // Check if response has the required number of leading zeros
        if let Ok(bytes) = hex::decode(response) {
            // Check if the first byte has required leading zeros
            if bytes[0] < (1 << (8 - difficulty.min(8))) {
                // Verify hash matches challenge
                let mut hasher = Sha256::new();
                hasher.update(challenge);
                let nonce_start = response.len() - 16;
                if nonce_start > 0 {
                    let nonce = &response[nonce_start..];
                    hasher.update(nonce);
                    let hash = hasher.finalize();
                    let hash_hex = hex::encode(&hash);
                    return hash_hex == response[0..nonce_start];
                }
            }
        }
        false
    }

    /// Issue a challenge to a peer
    pub fn issue_challenge(&mut self, peer_id: &PeerId) -> Option<String> {
        if let Some(peer) = self.peers.get_mut(peer_id) {
            match &peer.challenge_status {
                ChallengeStatus::None | ChallengeStatus::Failed { .. } => {
                    let challenge = self.generate_challenge();
                    peer.challenge_status = ChallengeStatus::Pending {
                        challenge: challenge.clone(),
                        issued_at: Instant::now(),
                        attempts: 0,
                    };
                    return Some(challenge);
                }
                ChallengeStatus::Pending { attempts, issued_at, .. } => {
                    // If challenge timed out, issue a new one
                    if issued_at.elapsed() > Duration::from_secs(self.challenge_timeout_seconds) {
                        if *attempts >= self.max_challenge_attempts {
                            peer.challenge_status = ChallengeStatus::Failed {
                                reason: "Too many failed attempts".to_string(),
                            };
                            return None;
                        }

                        let challenge = self.generate_challenge();
                        peer.challenge_status = ChallengeStatus::Pending {
                            challenge: challenge.clone(),
                            issued_at: Instant::now(),
                            attempts: attempts + 1,
                        };
                        return Some(challenge);
                    }
                    return None;
                }
                ChallengeStatus::Verified { .. } => return None,
            }
        }
        None
    }

    /// Process a challenge response
    pub fn process_challenge_response(&mut self, peer_id: &PeerId, response: &str) -> bool {
        if let Some(peer) = self.peers.get_mut(peer_id) {
            if let ChallengeStatus::Pending { challenge, attempts, .. } = &peer.challenge_status {
                let success = self.verify_challenge_response(challenge, response, self.challenge_difficulty);

                if success {
                    peer.challenge_status = ChallengeStatus::Verified {
                        verified_at: Instant::now(),
                    };
                    // Update score after verification
                    peer.update_score();
                    return true;
                } else if *attempts >= self.max_challenge_attempts {
                    peer.challenge_status = ChallengeStatus::Failed {
                        reason: "Failed to solve challenge correctly".to_string(),
                    };
                    // Update score after failure
                    peer.update_score();
                }
                return false;
            }
        }
        false
    }

    /// Try to add a new peer connection
    pub fn try_add_connection(&mut self, peer_id: PeerId, ip: IpAddr, port: u16) -> Result<bool, String> {
        // Check if IP is rate limited
        self.check_and_update_rate_limit(&ip)?;

        // Also check subnet rate limiting
        let subnet = IpSubnet::from_ip(ip);
        self.check_and_update_subnet_rate_limit(&subnet)?;

        // Check if we already know this peer
        let peer_info = match self.peers.get_mut(&peer_id) {
            Some(info) => {
                // Update existing peer information
                info.last_seen = Instant::now();
                info.record_connection();
                info.clone()
            }
            None => {
                // Create new peer information
                let new_info = PeerInfo::new(peer_id, ip, port);
                self.peers.insert(peer_id, new_info.clone());
                new_info
            }
        };

        // Check diversity limits
        if self.diversity_manager.would_violate_limits(&peer_info) {
            return Err("Connection would violate diversity limits".to_string());
        }

        // If challenges are enabled and peer isn't verified, don't fully accept yet
        if self.enable_connection_challenges {
            match &peer_info.challenge_status {
                ChallengeStatus::None => {
                    if let Some(challenge) = self.issue_challenge(&peer_id) {
                        // Return a special result indicating a challenge is required
                        return Ok(false);
                    }
                }
                ChallengeStatus::Pending { .. } => {
                    // We're waiting for a response
                    return Ok(false);
                }
                ChallengeStatus::Failed { reason } => {
                    return Err(format!("Challenge verification failed: {}", reason));
                }
                ChallengeStatus::Verified { .. } => {
                    // Already verified, proceed with connection
                }
            }
        }

        // Update diversity tracking
        self.diversity_manager.add_peer(peer_id, &peer_info);

        // Mark as connected
        self.connected_peers.insert(peer_id);

        // Update peer's diversity score
        if let Some(info) = self.peers.get_mut(&peer_id) {
            info.score.diversity_score = self.diversity_manager.calculate_diversity_score(info);
            info.update_score();
        }

        Ok(true)
    }

    /// Handle peer disconnection
    pub fn handle_disconnect(&mut self, peer_id: &PeerId, reason: Option<String>) {
        if let Some(info) = self.peers.get_mut(peer_id) {
            info.record_disconnection(reason);
            info.update_score();
            self.diversity_manager.remove_peer(peer_id, info);
        }
        self.connected_peers.remove(peer_id);
    }

    /// Check if an IP is rate limited and update tracking
    fn check_and_update_rate_limit(&mut self, ip: &IpAddr) -> Result<(), String> {
        let rate_limit = self.rate_limits
            .entry(*ip)
            .or_insert_with(|| RateLimitInfo::new(*ip));

        // Check if banned
        if let Some(banned_until) = rate_limit.banned_until {
            if banned_until > Instant::now() {
                return Err(format!("IP is banned until {:?}: {}",
                            banned_until,
                            rate_limit.ban_reason.as_deref().unwrap_or("No reason provided")));
            }
        }

        // Record attempt
        rate_limit.record_attempt();

        // Check rate limit
        if rate_limit.is_rate_limited(self.max_connection_attempts_per_min) {
            // Ban temporarily for excessive connection attempts
            rate_limit.ban(
                Duration::from_secs(300), // 5 minutes
                "Too many connection attempts"
            );
            return Err("Rate limited: too many connection attempts".to_string());
        }

        Ok(())
    }

    /// Check if subnet is rate limited and update tracking
    fn check_and_update_subnet_rate_limit(&mut self, subnet: &IpSubnet) -> Result<(), String> {
        let rate_limit = self.subnet_rate_limits
            .entry(subnet.clone())
            .or_insert_with(|| RateLimitInfo::new(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)))); // Dummy IP for subnet

        // Check if banned
        if let Some(banned_until) = rate_limit.banned_until {
            if banned_until > Instant::now() {
                return Err(format!("Subnet is banned until {:?}: {}",
                        banned_until,
                        rate_limit.ban_reason.as_deref().unwrap_or("No reason provided")));
            }
        }

        // Record attempt
        rate_limit.record_attempt();

        // Calculate subnet limit (higher than individual IP limit)
        let subnet_limit = self.max_connection_attempts_per_min * 3;

        // Check if too many recent attempts from this subnet
        if rate_limit.recent_attempts.len() > subnet_limit {
            // Ban subnet temporarily
            rate_limit.ban(Duration::from_secs(300), "Subnet rate limit exceeded");
            return Err(format!("Too many connection attempts from subnet, banned for 5 minutes"));
        }

        Ok(())
    }

    /// Get top-scored peers for outbound connections
    pub fn get_top_peers(&self, count: usize) -> Vec<PeerId> {
        let mut scored_peers: Vec<_> = self.peers.iter()
            .map(|(id, info)| (*id, info.score.total()))
            .collect();

        // Sort by score (descending)
        scored_peers.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        // Return top N peer IDs
        scored_peers.iter()
            .take(count)
            .map(|(id, _)| *id)
            .collect()
    }

    /// Create a forced peer rotation plan to enhance diversity
    pub fn create_rotation_plan(&self) -> Option<(Vec<PeerId>, Vec<PeerId>)> {
        // Skip if we don't have enough peers
        if self.connected_peers.len() < 5 {
            return None;
        }

        let mut to_disconnect = Vec::new();
        let mut to_connect = Vec::new();

        // Find overrepresented subnets
        let subnet_counts: HashMap<_, _> = self.diversity_manager.subnet_distribution.iter()
            .map(|(subnet, peers)| (subnet, peers.len()))
            .collect();

        // Find candidates to disconnect from overrepresented areas
        for (subnet, peers) in &self.diversity_manager.subnet_distribution {
            if peers.len() > self.diversity_manager.max_peers_per_subnet {
                // Too many peers from this subnet, select some for disconnection
                let mut subnet_peers: Vec<_> = peers.iter().collect();
                subnet_peers.sort_by(|a, b| {
                    let score_a = self.peers.get(a).map_or(0.0, |p| p.score.total());
                    let score_b = self.peers.get(b).map_or(0.0, |p| p.score.total());
                    score_b.partial_cmp(&score_a).unwrap_or(std::cmp::Ordering::Equal)
                });

                // Take the lowest scored peers to disconnect
                let excess = peers.len() - self.diversity_manager.max_peers_per_subnet;
                for peer_id in subnet_peers.iter().take(excess) {
                    to_disconnect.push(**peer_id);
                }
            }
        }

        // Find candidates to connect from underrepresented areas
        let mut potential_connects: Vec<_> = self.peers.iter()
            .filter(|(id, _)| !self.connected_peers.contains(id))
            .filter(|(_, info)| {
                let subnet = IpSubnet::from_ip(info.ip);
                let subnet_count = subnet_counts.get(&subnet).copied().unwrap_or(0);
                subnet_count < self.diversity_manager.max_peers_per_subnet
            })
            .map(|(id, info)| (*id, info.score.total()))
            .collect();

        // Sort by score (descending)
        potential_connects.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        // Take top peers to connect
        for (id, _) in potential_connects.iter().take(to_disconnect.len()) {
            to_connect.push(*id);
        }

        if to_disconnect.is_empty() || to_connect.is_empty() {
            None
        } else {
            Some((to_disconnect, to_connect))
        }
    }

    /// Update peer score based on a successful message exchange
    pub fn record_successful_exchange(&mut self, peer_id: &PeerId) {
        if let Some(info) = self.peers.get_mut(peer_id) {
            info.successful_exchanges += 1;
            info.update_score();
        }
    }

    /// Update peer score based on a failed message exchange
    pub fn record_failed_exchange(&mut self, peer_id: &PeerId) {
        if let Some(info) = self.peers.get_mut(peer_id) {
            info.failed_exchanges += 1;
            info.update_score();
        }
    }

    /// Set protocols supported by a peer
    pub fn set_peer_protocols(&mut self, peer_id: &PeerId, protocols: Vec<String>) {
        if let Some(info) = self.peers.get_mut(peer_id) {
            info.protocols = protocols;
        }
    }

    /// Get statistics about current peer distribution
    pub fn get_diversity_stats(&self) -> PeerDiversityStats {
        let subnet_count = self.diversity_manager.subnet_distribution.len();
        let asn_count = self.diversity_manager.asn_distribution.len();
        let region_count = self.diversity_manager.geographic_distribution.len();

        let max_subnet_peers = self.diversity_manager.subnet_distribution.values()
            .map(|peers| peers.len())
            .max()
            .unwrap_or(0);

        let max_asn_peers = self.diversity_manager.asn_distribution.values()
            .map(|peers| peers.len())
            .max()
            .unwrap_or(0);

        let max_region_peers = self.diversity_manager.geographic_distribution.values()
            .map(|peers| peers.len())
            .max()
            .unwrap_or(0);

        PeerDiversityStats {
            total_peers: self.peers.len(),
            connected_peers: self.connected_peers.len(),
            subnet_count,
            asn_count,
            region_count,
            max_peers_per_subnet: max_subnet_peers,
            max_peers_per_asn: max_asn_peers,
            max_peers_per_region: max_region_peers,
        }
    }

    /// Get information about all currently connected peers
    pub fn get_connected_peer_infos(&self) -> Vec<PeerInfo> {
        self.connected_peers.iter()
            .filter_map(|peer_id| self.peers.get(peer_id).cloned())
            .collect()
    }

    /// Get information about a specific peer
    pub fn get_peer_info(&self, peer_id: &PeerId) -> Option<PeerInfo> {
        self.peers.get(peer_id).cloned()
    }

    /// Get all known peers (connected and disconnected)
    pub fn get_all_peer_infos(&self) -> Vec<PeerInfo> {
        self.peers.values().cloned().collect()
    }

    /// Check if a peer is currently connected
    pub fn is_connected(&self, peer_id: &PeerId) -> bool {
        self.connected_peers.contains(peer_id)
    }

    /// Get count of connected peers
    pub fn connected_peer_count(&self) -> usize {
        self.connected_peers.len()
    }

    /// Get count of all known peers
    pub fn total_peer_count(&self) -> usize {
        self.peers.len()
    }

    /// Check for peers that need rotation to prevent eclipse attacks
    pub fn check_for_rotation_need(&mut self) -> bool {
        // Check if we need to force rotation
        if self.last_rotation_time.elapsed() >= self.forced_rotation_interval {
            return true;
        }

        // Also check for signs of an eclipse attack
        // (Implement heuristics for detecting potential eclipse attacks)

        false
    }

    /// Perform forced peer rotation
    pub fn perform_forced_rotation(&mut self) -> (Vec<PeerId>, usize) {
        let rotation_plan = self.create_rotation_plan();

        if let Some((to_disconnect, _to_connect)) = rotation_plan {
            self.last_rotation_time = Instant::now();
            return (to_disconnect, to_disconnect.len());
        }

        (vec![], 0)
    }

    /// Record block announcement from peer
    pub fn record_block_announcement(&mut self, peer_id: &PeerId, valid: bool) {
        if let Some(peer) = self.peers.get_mut(peer_id) {
            if valid {
                peer.behavior_patterns.valid_blocks_announced += 1;
            } else {
                peer.behavior_patterns.invalid_blocks_announced += 1;
            }
            peer.update_score();
        }
    }

    /// Record transaction relay from peer
    pub fn record_transaction_relay(&mut self, peer_id: &PeerId, valid: bool) {
        if let Some(peer) = self.peers.get_mut(peer_id) {
            if valid {
                peer.behavior_patterns.valid_txns_relayed += 1;
            } else {
                peer.behavior_patterns.invalid_txns_relayed += 1;
            }
            peer.update_score();
        }
    }

    /// Record protocol violation by peer
    pub fn record_protocol_violation(&mut self, peer_id: &PeerId, violation: &str) {
        if let Some(peer) = self.peers.get_mut(peer_id) {
            peer.behavior_patterns.protocol_violations += 1;
            peer.behavior_patterns.record_unusual_pattern(violation);
            peer.update_score();

            // If severe violations, ban the peer
            if peer.behavior_patterns.protocol_violations > 5 {
                let ip = peer.ip;
                if let Some(rate_limit) = self.rate_limits.get_mut(&ip) {
                    rate_limit.ban(
                        Duration::from_secs(3600),
                        &format!("Multiple protocol violations: {}", violation)
                    );
                }
            }
        }
    }

    /// Record message response time
    pub fn record_response_time(&mut self, peer_id: &PeerId, time_ms: u64) {
        if let Some(peer) = self.peers.get_mut(peer_id) {
            peer.behavior_patterns.record_response_time(time_ms);
            // Update latency score
            let avg_time = peer.behavior_patterns.average_response_time();
            if let Some(avg) = avg_time {
                // Latency score inversely proportional to response time
                // 0ms -> 5.0, 1000ms -> 0.0
                peer.score.latency_score = (1000.0 - avg.min(1000.0)) / 200.0;
            }
            peer.update_score();
        }
    }

    /// Set the client version for a peer
    pub fn set_client_version(&mut self, peer_id: &PeerId, version: String) {
        if let Some(peer) = self.peers.get_mut(peer_id) {
            peer.behavior_patterns.client_version = Some(version);
        }
    }
}

/// Statistics about peer diversity
#[derive(Debug, Clone)]
pub struct PeerDiversityStats {
    /// Total number of known peers
    pub total_peers: usize,
    /// Number of currently connected peers
    pub connected_peers: usize,
    /// Number of subnets represented
    pub subnet_count: usize,
    /// Number of ASNs represented
    pub asn_count: usize,
    /// Number of geographic regions represented
    pub region_count: usize,
    /// Maximum peers per subnet
    pub max_peers_per_subnet: usize,
    /// Maximum peers per ASN
    pub max_peers_per_asn: usize,
    /// Maximum peers per region
    pub max_peers_per_region: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{Ipv4Addr, Ipv6Addr};

    #[test]
    fn test_ip_subnet_contains() {
        // IPv4 test
        let subnet = IpSubnet {
            base: IpAddr::V4(Ipv4Addr::new(192, 168, 1, 0)),
            mask_bits: 24,
        };

        assert!(subnet.contains(&IpAddr::V4(Ipv4Addr::new(192, 168, 1, 100))));
        assert!(subnet.contains(&IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1))));
        assert!(!subnet.contains(&IpAddr::V4(Ipv4Addr::new(192, 168, 2, 1))));

        // IPv6 test
        let subnet_v6 = IpSubnet {
            base: IpAddr::V6(Ipv6Addr::new(0x2001, 0xdb8, 0, 0, 0, 0, 0, 0)),
            mask_bits: 48,
        };

        assert!(subnet_v6.contains(&IpAddr::V6(Ipv6Addr::new(0x2001, 0xdb8, 0, 0, 0, 0, 0, 1))));
        assert!(!subnet_v6.contains(&IpAddr::V6(Ipv6Addr::new(0x2001, 0xdb9, 0, 0, 0, 0, 0, 0))));
    }

    #[test]
    fn test_rate_limit_tracking() {
        let mut rate_info = RateLimitInfo::new(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1)));

        // Add 3 attempts
        for _ in 0..3 {
            rate_info.record_attempt();
        }

        // Not rate limited yet
        assert!(!rate_info.is_rate_limited(5));

        // Add 3 more
        for _ in 0..3 {
            rate_info.record_attempt();
        }

        // Now it should be rate limited
        assert!(rate_info.is_rate_limited(5));

        // Ban the IP
        rate_info.ban(Duration::from_secs(300), "Test ban");

        // Should be rate limited due to ban
        assert!(rate_info.is_rate_limited(10));
    }

    #[test]
    fn test_peer_score_calculation() {
        let mut peer_info = PeerInfo::new(
            PeerId::random(),
            IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1)),
            8333,
        );

        // Add some connection history
        peer_info.record_connection();
        peer_info.record_disconnection(Some("Test disconnect".to_string()));
        peer_info.record_connection();

        // Add some exchanges
        peer_info.successful_exchanges = 8;
        peer_info.failed_exchanges = 2;

        // Update score
        peer_info.update_score();

        // Check score components
        assert!(peer_info.score.base_score > 0.0);
        assert!(peer_info.score.stability_score > 0.0);
        assert!(peer_info.score.behavior_score > 0.0);
        assert!(peer_info.score.total() > 0.0);
    }
}