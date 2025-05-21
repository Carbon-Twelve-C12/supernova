use std::collections::{HashMap, HashSet};
use std::net::IpAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use serde::{Serialize, Deserialize};
use thiserror::Error;
use log::{debug, info, warn, error};

/// Errors that can occur in security management
#[derive(Error, Debug)]
pub enum SecurityError {
    #[error("Rate limit exceeded for {0}")]
    RateLimitExceeded(IpAddr),
    
    #[error("Connection rejected due to diversity requirements")]
    DiversityRequirementFailed,
    
    #[error("Challenge verification failed")]
    ChallengeVerificationFailed,
    
    #[error("Configuration error: {0}")]
    ConfigurationError(String),
    
    #[error("Invalid peer: {0}")]
    InvalidPeer(String),
    
    #[error("Too many connections from subnet {0}")]
    TooManySubnetConnections(String),
    
    #[error("Suspicious behavior detected: {0}")]
    SuspiciousBehavior(String),
    
    #[error("Storage error: {0}")]
    StorageError(String),
}

/// Result type for security operations
pub type SecurityResult<T> = Result<T, SecurityError>;

/// Peer connection information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerInfo {
    /// Peer ID
    pub id: String,
    
    /// IP address
    pub address: IpAddr,
    
    /// Subnet identifier (/24 for IPv4, /64 for IPv6)
    pub subnet: String,
    
    /// Autonomous System Number (ASN)
    pub asn: Option<u32>,
    
    /// Geographic region
    pub region: Option<String>,
    
    /// User agent string
    pub user_agent: String,
    
    /// Protocol version
    pub protocol_version: u32,
    
    /// Connection time
    pub connection_time: Instant,
    
    /// Outbound connection (if false, it's inbound)
    pub is_outbound: bool,
    
    /// Whether this is a protected peer
    pub is_protected: bool,
}

/// Challenge for suspicious peers
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerChallenge {
    /// Challenge ID
    pub id: String,
    
    /// Difficulty target
    pub difficulty: u32,
    
    /// Nonce prefix
    pub nonce_prefix: [u8; 16],
    
    /// Timestamp
    pub timestamp: u64,
    
    /// Expiration time
    pub expires_at: u64,
}

/// Connection decision
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConnectionDecision {
    /// Accept the connection
    Accept,
    
    /// Reject the connection with reason
    Reject(RejectReason),
    
    /// Challenge the peer before accepting
    Challenge(PeerChallenge),
}

/// Rejection reason
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RejectReason {
    /// Rate limit exceeded
    RateLimitExceeded,
    
    /// Diversity requirements not met
    DiversityRequirements,
    
    /// Failed challenge
    FailedChallenge,
    
    /// Banned peer
    Banned,
    
    /// Too many connections
    TooManyConnections,
    
    /// Protocol version mismatch
    ProtocolMismatch,
    
    /// Suspicious behavior
    SuspiciousBehavior,
}

/// Configuration for the security manager
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityConfig {
    /// Whether security features are enabled
    pub enabled: bool,
    
    /// Maximum number of peers per subnet
    pub max_peers_per_subnet: u32,
    
    /// Maximum number of peers per ASN
    pub max_peers_per_asn: u32,
    
    /// Maximum number of peers per region
    pub max_peers_per_region: u32,
    
    /// Minimum diversity score to accept a connection
    pub min_diversity_score: f64,
    
    /// Whether to enable peer challenges
    pub enable_peer_challenges: bool,
    
    /// Difficulty for peer challenges
    pub challenge_difficulty: u32,
    
    /// Rate limit window in seconds
    pub rate_limit_window_secs: u64,
    
    /// Maximum connection attempts per minute from an IP
    pub max_connection_attempts_per_min: u32,
    
    /// Interval for rotating peers (in seconds)
    pub peer_rotation_interval_secs: u64,
    
    /// Percentage of peers to rotate
    pub peer_rotation_percentage: f64,
    
    /// Whether to enforce IP diversity
    pub enforce_ip_diversity: bool,
    
    /// Minimum outbound connections
    pub min_outbound_connections: u32,
    
    /// Maximum inbound connections
    pub max_inbound_connections: u32,
    
    /// Minimum score for peers to be kept
    pub min_peer_score: f64,
    
    /// Ban threshold for peer score
    pub ban_threshold: f64,
    
    /// Ban duration in seconds
    pub ban_duration_secs: u64,
}

impl Default for SecurityConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_peers_per_subnet: 3,
            max_peers_per_asn: 8,
            max_peers_per_region: 15,
            min_diversity_score: 0.7,
            enable_peer_challenges: true,
            challenge_difficulty: 20,
            rate_limit_window_secs: 60,
            max_connection_attempts_per_min: 10,
            peer_rotation_interval_secs: 21600, // 6 hours
            peer_rotation_percentage: 0.25, // 25%
            enforce_ip_diversity: true,
            min_outbound_connections: 8,
            max_inbound_connections: 125,
            min_peer_score: 0.5,
            ban_threshold: -100.0,
            ban_duration_secs: 86400, // 24 hours
        }
    }
}

/// Tracker for diversity of connections
pub struct DiversityTracker {
    /// Maximum peers per subnet
    max_peers_per_subnet: u32,
    
    /// Minimum diversity score
    min_diversity_score: f64,
    
    /// Count of peers per subnet
    subnet_counts: HashMap<String, u32>,
    
    /// Count of peers per ASN
    asn_counts: HashMap<u32, u32>,
    
    /// Count of peers per region
    region_counts: HashMap<String, u32>,
}

impl DiversityTracker {
    /// Create a new diversity tracker
    pub fn new(max_peers_per_subnet: u32, min_diversity_score: f64) -> Self {
        Self {
            max_peers_per_subnet,
            min_diversity_score,
            subnet_counts: HashMap::new(),
            asn_counts: HashMap::new(),
            region_counts: HashMap::new(),
        }
    }
    
    /// Check if a new connection can be accepted
    pub fn can_accept_connection(&mut self, peer_info: &PeerInfo) -> bool {
        // Check subnet limit
        let subnet_count = self.subnet_counts.entry(peer_info.subnet.clone()).or_insert(0);
        if *subnet_count >= self.max_peers_per_subnet {
            return false;
        }
        
        // Check ASN limit if available
        if let Some(asn) = peer_info.asn {
            let asn_count = self.asn_counts.entry(asn).or_insert(0);
            if *asn_count >= self.max_peers_per_subnet * 3 {
                return false;
            }
        }
        
        // Check region limit if available
        if let Some(region) = &peer_info.region {
            let region_count = self.region_counts.entry(region.clone()).or_insert(0);
            if *region_count >= self.max_peers_per_subnet * 5 {
                return false;
            }
        }
        
        // Calculate diversity score
        let diversity_score = self.calculate_diversity_score();
        if diversity_score < self.min_diversity_score {
            // Only accept connections that improve diversity
            let potential_score = self.calculate_potential_diversity_score(peer_info);
            if potential_score <= diversity_score {
                return false;
            }
        }
        
        // Connection is accepted, update counters
        *subnet_count += 1;
        
        if let Some(asn) = peer_info.asn {
            *self.asn_counts.entry(asn).or_insert(0) += 1;
        }
        
        if let Some(region) = &peer_info.region {
            *self.region_counts.entry(region.clone()).or_insert(0) += 1;
        }
        
        true
    }
    
    /// Record a disconnection
    pub fn record_disconnection(&mut self, peer_info: &PeerInfo) {
        // Update subnet count
        if let Some(count) = self.subnet_counts.get_mut(&peer_info.subnet) {
            if *count > 0 {
                *count -= 1;
            }
        }
        
        // Update ASN count
        if let Some(asn) = peer_info.asn {
            if let Some(count) = self.asn_counts.get_mut(&asn) {
                if *count > 0 {
                    *count -= 1;
                }
            }
        }
        
        // Update region count
        if let Some(region) = &peer_info.region {
            if let Some(count) = self.region_counts.get_mut(region) {
                if *count > 0 {
                    *count -= 1;
                }
            }
        }
    }
    
    /// Calculate diversity score based on Shannon entropy
    fn calculate_diversity_score(&self) -> f64 {
        let subnet_score = calculate_entropy(&self.subnet_counts);
        let asn_score = calculate_entropy(&self.asn_counts);
        let region_score = calculate_entropy(&self.region_counts);
        
        // Weighted average of scores
        (subnet_score * 0.5 + asn_score * 0.3 + region_score * 0.2)
            .min(1.0)
            .max(0.0)
    }
    
    /// Calculate potential diversity score if a peer is added
    fn calculate_potential_diversity_score(&self, peer_info: &PeerInfo) -> f64 {
        let mut subnet_counts = self.subnet_counts.clone();
        *subnet_counts.entry(peer_info.subnet.clone()).or_insert(0) += 1;
        
        let mut asn_counts = self.asn_counts.clone();
        if let Some(asn) = peer_info.asn {
            *asn_counts.entry(asn).or_insert(0) += 1;
        }
        
        let mut region_counts = self.region_counts.clone();
        if let Some(region) = &peer_info.region {
            *region_counts.entry(region.clone()).or_insert(0) += 1;
        }
        
        let subnet_score = calculate_entropy(&subnet_counts);
        let asn_score = calculate_entropy(&asn_counts);
        let region_score = calculate_entropy(&region_counts);
        
        (subnet_score * 0.5 + asn_score * 0.3 + region_score * 0.2)
            .min(1.0)
            .max(0.0)
    }
}

/// Rate limiter for connection attempts
pub struct RateLimiter {
    /// Window size in seconds
    window_secs: u64,
    
    /// Maximum attempts per window
    max_attempts: u32,
    
    /// Counters by IP
    counters: HashMap<IpAddr, Vec<Instant>>,
}

impl RateLimiter {
    /// Create a new rate limiter
    pub fn new(window_secs: u64, max_attempts: u32) -> Self {
        Self {
            window_secs,
            max_attempts,
            counters: HashMap::new(),
        }
    }
    
    /// Check if an IP can make another connection and record the attempt
    pub fn check_and_record(&mut self, ip: IpAddr) -> bool {
        let now = Instant::now();
        let window_duration = Duration::from_secs(self.window_secs);
        
        // Get or create the entry for this IP
        let timestamps = self.counters.entry(ip).or_insert_with(Vec::new);
        
        // Remove expired timestamps
        timestamps.retain(|t| now.duration_since(*t) < window_duration);
        
        // Check if we're over the limit
        if timestamps.len() >= self.max_attempts as usize {
            return false;
        }
        
        // Record this attempt
        timestamps.push(now);
        
        true
    }
    
    /// Clear old data
    pub fn cleanup(&mut self) {
        let now = Instant::now();
        let window_duration = Duration::from_secs(self.window_secs);
        
        self.counters.retain(|_, timestamps| {
            timestamps.retain(|t| now.duration_since(*t) < window_duration);
            !timestamps.is_empty()
        });
    }
}

/// System for challenging suspicious peers
pub struct ChallengeSystem {
    /// Whether challenges are enabled
    enabled: bool,
    
    /// Difficulty target for challenges
    difficulty: u32,
    
    /// Active challenges
    active_challenges: HashMap<String, PeerChallenge>,
}

impl ChallengeSystem {
    /// Create a new challenge system
    pub fn new(enabled: bool, difficulty: u32) -> Self {
        Self {
            enabled,
            difficulty,
            active_challenges: HashMap::new(),
        }
    }
    
    /// Generate a new challenge for a peer
    pub fn generate_challenge(&mut self) -> PeerChallenge {
        // In a real implementation, this would generate a proper PoW challenge
        // For now, we'll create a placeholder
        let id = format!("challenge-{}", rand::random::<u64>());
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        
        let mut nonce_prefix = [0u8; 16];
        rand::RngCore::fill_bytes(&mut rand::thread_rng(), &mut nonce_prefix);
        
        let challenge = PeerChallenge {
            id: id.clone(),
            difficulty: self.difficulty,
            nonce_prefix,
            timestamp,
            expires_at: timestamp + 60, // 1 minute to solve
        };
        
        self.active_challenges.insert(id, challenge.clone());
        
        challenge
    }
    
    /// Verify a challenge solution
    pub fn verify_solution(&mut self, challenge_id: &str, nonce: &[u8]) -> bool {
        if !self.enabled {
            return true;
        }
        
        if let Some(challenge) = self.active_challenges.remove(challenge_id) {
            // Check if challenge has expired
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
                
            if now > challenge.expires_at {
                return false;
            }
            
            // Verify the solution
            // In a real implementation, this would check a proper PoW solution
            // For now, we'll simply check if the nonce is not empty
            !nonce.is_empty()
        } else {
            false
        }
    }
    
    /// Clear expired challenges
    pub fn cleanup(&mut self) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
            
        self.active_challenges.retain(|_, challenge| now <= challenge.expires_at);
    }
}

/// Security manager for the Supernova blockchain
pub struct SecurityManager {
    /// Configuration
    config: SecurityConfig,
    
    /// Peers currently connected
    peers: HashMap<String, PeerInfo>,
    
    /// Banned IP addresses with expiry time
    banned_ips: HashMap<IpAddr, u64>,
    
    /// Connection diversity tracker
    diversity_tracker: DiversityTracker,
    
    /// Rate limiter for connection attempts
    rate_limiter: RateLimiter,
    
    /// Challenge system for suspicious peers
    challenge_system: ChallengeSystem,
    
    /// Peer scores (for behavior evaluation)
    peer_scores: HashMap<String, f64>,
    
    /// Last rotation time
    last_rotation: Instant,
}

impl SecurityManager {
    /// Create a new security manager
    pub fn new(config: SecurityConfig) -> Self {
        Self {
            diversity_tracker: DiversityTracker::new(
                config.max_peers_per_subnet,
                config.min_diversity_score,
            ),
            rate_limiter: RateLimiter::new(
                config.rate_limit_window_secs,
                config.max_connection_attempts_per_min,
            ),
            challenge_system: ChallengeSystem::new(
                config.enable_peer_challenges,
                config.challenge_difficulty,
            ),
            peers: HashMap::new(),
            banned_ips: HashMap::new(),
            peer_scores: HashMap::new(),
            last_rotation: Instant::now(),
            config,
        }
    }
    
    /// Evaluate a new connection request
    pub fn evaluate_connection(&mut self, peer_info: &PeerInfo) -> ConnectionDecision {
        if !self.config.enabled {
            return ConnectionDecision::Accept;
        }
        
        // Check if banned
        if self.is_banned(&peer_info.address) {
            return ConnectionDecision::Reject(RejectReason::Banned);
        }
        
        // Check rate limits
        if !self.rate_limiter.check_and_record(peer_info.address) {
            return ConnectionDecision::Reject(RejectReason::RateLimitExceeded);
        }
        
        // Check connection limits
        let inbound_count = self.peers.values().filter(|p| !p.is_outbound).count();
        if !peer_info.is_outbound && inbound_count >= self.config.max_inbound_connections as usize {
            return ConnectionDecision::Reject(RejectReason::TooManyConnections);
        }
        
        // Check diversity requirements
        if self.config.enforce_ip_diversity && !self.diversity_tracker.can_accept_connection(peer_info) {
            return ConnectionDecision::Reject(RejectReason::DiversityRequirements);
        }
        
        // Issue challenge if needed
        if self.config.enable_peer_challenges && self.should_challenge(peer_info) {
            return ConnectionDecision::Challenge(self.challenge_system.generate_challenge());
        }
        
        ConnectionDecision::Accept
    }
    
    /// Register a new peer connection
    pub fn register_peer(&mut self, peer_info: PeerInfo) -> SecurityResult<()> {
        if !self.config.enabled {
            self.peers.insert(peer_info.id.clone(), peer_info);
            return Ok(());
        }
        
        // Initialize peer score
        self.peer_scores.insert(peer_info.id.clone(), 0.0);
        
        // Store peer info
        self.peers.insert(peer_info.id.clone(), peer_info);
        
        // Check if we need to rotate peers
        self.check_peer_rotation();
        
        Ok(())
    }
    
    /// Record peer disconnection
    pub fn disconnect_peer(&mut self, peer_id: &str) -> SecurityResult<()> {
        if let Some(peer_info) = self.peers.remove(peer_id) {
            self.diversity_tracker.record_disconnection(&peer_info);
            self.peer_scores.remove(peer_id);
        }
        
        Ok(())
    }
    
    /// Check if an IP is banned
    pub fn is_banned(&self, ip: &IpAddr) -> bool {
        if let Some(expiry) = self.banned_ips.get(ip) {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
                
            if now < *expiry {
                return true;
            }
        }
        
        false
    }
    
    /// Ban an IP address
    pub fn ban_ip(&mut self, ip: IpAddr, duration_secs: Option<u64>) -> SecurityResult<()> {
        let duration = duration_secs.unwrap_or(self.config.ban_duration_secs);
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
            
        let expiry = now + duration;
        self.banned_ips.insert(ip, expiry);
        
        // Disconnect any peers with this IP
        let to_disconnect: Vec<String> = self.peers.iter()
            .filter(|(_, p)| p.address == ip)
            .map(|(id, _)| id.clone())
            .collect();
            
        for peer_id in to_disconnect {
            self.disconnect_peer(&peer_id)?;
        }
        
        Ok(())
    }
    
    /// Update peer score for behavior evaluation
    pub fn update_peer_score(&mut self, peer_id: &str, delta: f64) -> SecurityResult<()> {
        let score = self.peer_scores.entry(peer_id.to_string()).or_insert(0.0);
        *score += delta;
        
        // Check if we need to ban this peer
        if *score <= self.config.ban_threshold {
            if let Some(peer_info) = self.peers.get(peer_id) {
                info!("Banning peer {} due to low score: {}", peer_id, *score);
                self.ban_ip(peer_info.address, None)?;
            }
        }
        
        Ok(())
    }
    
    /// Get peer info by ID
    pub fn get_peer_info(&self, peer_id: &str) -> Option<&PeerInfo> {
        self.peers.get(peer_id)
    }
    
    /// Get all connected peers
    pub fn get_all_peers(&self) -> Vec<&PeerInfo> {
        self.peers.values().collect()
    }
    
    /// Get inbound peer count
    pub fn inbound_peer_count(&self) -> usize {
        self.peers.values().filter(|p| !p.is_outbound).count()
    }
    
    /// Get outbound peer count
    pub fn outbound_peer_count(&self) -> usize {
        self.peers.values().filter(|p| p.is_outbound).count()
    }
    
    /// Get total peer count
    pub fn total_peer_count(&self) -> usize {
        self.peers.len()
    }
    
    /// Perform regular maintenance tasks
    pub fn maintenance(&mut self) {
        // Cleanup rate limiter
        self.rate_limiter.cleanup();
        
        // Cleanup challenge system
        self.challenge_system.cleanup();
        
        // Check for peer rotation
        self.check_peer_rotation();
        
        // Clean up expired bans
        self.cleanup_bans();
    }
    
    /// Check if a peer should be challenged
    fn should_challenge(&self, peer_info: &PeerInfo) -> bool {
        // In a real implementation, this would use heuristics to identify
        // suspicious peers for challenging.
        // 
        // For example, peers might be challenged if:
        // 1. They're from an overrepresented subnet
        // 2. Their connection history shows suspicious patterns
        // 3. They're from a region with a high rate of malicious activity
        
        // For now, we'll just return a random decision with low probability
        rand::random::<u32>() % 10 == 0
    }
    
    /// Check if we need to rotate peers and perform rotation if needed
    fn check_peer_rotation(&mut self) {
        if !self.config.enabled {
            return;
        }
        
        let now = Instant::now();
        let rotation_interval = Duration::from_secs(self.config.peer_rotation_interval_secs);
        
        if now.duration_since(self.last_rotation) >= rotation_interval {
            self.rotate_peers();
            self.last_rotation = now;
        }
    }
    
    /// Rotate a percentage of peers to prevent eclipse attacks
    fn rotate_peers(&mut self) {
        // Don't rotate peers if we don't have enough
        if self.peers.len() < 10 {
            return;
        }
        
        // Calculate number of peers to rotate
        let to_rotate = (self.peers.len() as f64 * self.config.peer_rotation_percentage) as usize;
        if to_rotate == 0 {
            return;
        }
        
        // Select non-protected peers for rotation, preferring those with lower scores
        let mut candidates: Vec<_> = self.peers.iter()
            .filter(|(_, p)| !p.is_protected)
            .map(|(id, p)| (id.clone(), p.clone()))
            .collect();
            
        // Sort by score (lowest first)
        candidates.sort_by(|(id_a, _), (id_b, _)| {
            let score_a = self.peer_scores.get(id_a).unwrap_or(&0.0);
            let score_b = self.peer_scores.get(id_b).unwrap_or(&0.0);
            score_a.partial_cmp(score_b).unwrap_or(std::cmp::Ordering::Equal)
        });
        
        // Take the first `to_rotate` candidates
        let to_disconnect: Vec<_> = candidates.iter()
            .take(to_rotate)
            .map(|(id, _)| id.clone())
            .collect();
            
        // Disconnect selected peers
        for peer_id in to_disconnect {
            // In a real implementation, we would send a disconnect message
            // and wait for the peer to close the connection
            if let Err(e) = self.disconnect_peer(&peer_id) {
                error!("Error disconnecting peer during rotation: {}", e);
            }
        }
        
        info!("Rotated {} peers for eclipse attack prevention", to_rotate);
    }
    
    /// Clean up expired bans
    fn cleanup_bans(&mut self) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
            
        self.banned_ips.retain(|_, expiry| *expiry > now);
    }
}

/// Calculate entropy (diversity) from a distribution of counts
fn calculate_entropy<K>(counts: &HashMap<K, u32>) -> f64 
where
    K: std::hash::Hash + Eq,
{
    if counts.is_empty() {
        return 0.0;
    }
    
    let total: u32 = counts.values().sum();
    if total == 0 {
        return 0.0;
    }
    
    let total_f64 = total as f64;
    
    // Calculate Shannon entropy
    let entropy: f64 = counts.values()
        .map(|&count| {
            let p = count as f64 / total_f64;
            if p > 0.0 {
                -p * p.log2()
            } else {
                0.0
            }
        })
        .sum();
        
    // Normalize by maximum possible entropy (log2(n))
    let max_entropy = (counts.len() as f64).log2();
    if max_entropy > 0.0 {
        entropy / max_entropy
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_diversity_tracker() {
        let mut tracker = DiversityTracker::new(3, 0.7);
        
        // Create peers in the same subnet
        let mut peers = vec![];
        for i in 0..5 {
            peers.push(PeerInfo {
                id: format!("peer-{}", i),
                address: "192.168.1.1".parse().unwrap(),
                subnet: "192.168.1.0/24".to_string(),
                asn: Some(12345),
                region: Some("us-west".to_string()),
                user_agent: "test".to_string(),
                protocol_version: 1,
                connection_time: Instant::now(),
                is_outbound: i % 2 == 0,
                is_protected: false,
            });
        }
        
        // First 3 peers in the same subnet should be accepted
        assert!(tracker.can_accept_connection(&peers[0]));
        assert!(tracker.can_accept_connection(&peers[1]));
        assert!(tracker.can_accept_connection(&peers[2]));
        
        // 4th peer in the same subnet should be rejected
        assert!(!tracker.can_accept_connection(&peers[3]));
        
        // Disconnecting one peer should allow another
        tracker.record_disconnection(&peers[0]);
        assert!(tracker.can_accept_connection(&peers[3]));
    }
    
    #[test]
    fn test_rate_limiter() {
        let mut limiter = RateLimiter::new(60, 3);
        let ip = "192.168.1.1".parse().unwrap();
        
        // First 3 attempts should be allowed
        assert!(limiter.check_and_record(ip));
        assert!(limiter.check_and_record(ip));
        assert!(limiter.check_and_record(ip));
        
        // 4th attempt should be rejected
        assert!(!limiter.check_and_record(ip));
    }
    
    #[test]
    fn test_challenge_system() {
        let mut system = ChallengeSystem::new(true, 20);
        
        // Generate a challenge
        let challenge = system.generate_challenge();
        
        // Verify with empty solution (should fail)
        assert!(!system.verify_solution(&challenge.id, &[]));
        
        // Verify with non-empty solution (should pass in our simplified test)
        assert!(system.verify_solution(&challenge.id, &[1, 2, 3]));
    }
} 