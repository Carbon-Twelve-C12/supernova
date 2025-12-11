//! Eclipse Attack Prevention System for Supernova
//!
//! This module implements multiple layers of protection against eclipse attacks:
//! 1. IP diversity requirements - limits connections per subnet/ASN
//! 2. Anchor connections - persistent connections to trusted peers
//! 3. Peer rotation - periodic replacement of peers
//! 4. Behavioral analysis - detection of eclipse attack patterns
//! 5. Proof-of-work challenges - make Sybil attacks expensive

use libp2p::PeerId;
use rand::{thread_rng, Rng};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet, VecDeque};
use std::net::{IpAddr, Ipv4Addr};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

/// Configuration for eclipse attack prevention
#[derive(Debug, Clone)]
pub struct EclipsePreventionConfig {
    /// Minimum number of anchor connections to maintain
    pub min_anchor_connections: usize,

    /// Maximum percentage of connections from same /24 subnet (IPv4) or /48 (IPv6)
    pub max_subnet_percentage: f64,

    /// Maximum percentage of connections from same ASN
    pub max_asn_percentage: f64,

    /// Maximum percentage of inbound connections
    pub max_inbound_percentage: f64,

    /// Peer rotation interval
    pub rotation_interval: Duration,

    /// Percentage of peers to rotate
    pub rotation_percentage: f64,

    /// Enable proof-of-work challenges
    pub require_pow_challenge: bool,

    /// Difficulty for PoW challenges (leading zero bits)
    pub pow_difficulty: u8,

    /// Enable behavioral analysis
    pub enable_behavioral_analysis: bool,

    /// Eclipse attack detection threshold
    pub eclipse_detection_threshold: f64,

    /// Minimum connections before enforcing diversity
    pub min_connections_for_diversity: usize,

    /// Maximum connections from same geographic region
    pub max_region_percentage: f64,

    /// Ban duration for malicious peers
    pub ban_duration: Duration,
}

impl Default for EclipsePreventionConfig {
    fn default() -> Self {
        Self {
            min_anchor_connections: 4,
            max_subnet_percentage: 0.15,  // Max 15% from same subnet
            max_asn_percentage: 0.25,     // Max 25% from same ASN
            max_inbound_percentage: 0.67, // Max 67% inbound
            rotation_interval: Duration::from_secs(3600), // 1 hour
            rotation_percentage: 0.25,    // Rotate 25% of peers
            require_pow_challenge: true,
            pow_difficulty: 20, // ~1 second on average CPU
            enable_behavioral_analysis: true,
            eclipse_detection_threshold: 0.8,
            min_connections_for_diversity: 8,
            max_region_percentage: 0.4, // Max 40% from same region
            ban_duration: Duration::from_secs(86400), // 24 hours
        }
    }
}

/// Tracks information about connected peers for eclipse prevention
#[derive(Debug, Clone)]
pub struct PeerConnectionInfo {
    pub peer_id: PeerId,
    pub ip_address: IpAddr,
    pub subnet: String,
    pub asn: Option<u32>,
    pub region: Option<String>,
    pub is_inbound: bool,
    pub is_anchor: bool,
    pub connected_at: Instant,
    pub last_useful_at: Instant,
    pub behavior_score: f64,
    pub pow_completed: bool,
}

/// Behavioral patterns that indicate eclipse attack
#[derive(Debug, Clone, Default)]
pub struct EclipseAttackIndicators {
    /// Rapid connection attempts from similar IPs
    pub connection_flooding: bool,

    /// Many peers advertising same addresses
    pub address_convergence: bool,

    /// Peers refusing to relay transactions
    pub transaction_censorship: bool,

    /// Peers providing conflicting chain data
    pub chain_manipulation: bool,

    /// Sudden loss of peer diversity
    pub diversity_collapse: bool,

    /// Peers exhibiting coordinated behavior
    pub coordinated_behavior: bool,

    /// Timestamp of detection
    pub detected_at: Option<Instant>,
}

/// Eclipse prevention system
pub struct EclipsePreventionSystem {
    config: EclipsePreventionConfig,

    /// Current peer connections
    connections: Arc<RwLock<HashMap<PeerId, PeerConnectionInfo>>>,

    /// Anchor peers (trusted, persistent connections)
    anchor_peers: Arc<RwLock<HashSet<PeerId>>>,

    /// Connection history for pattern analysis
    connection_history: Arc<RwLock<VecDeque<ConnectionEvent>>>,

    /// Current attack indicators
    attack_indicators: Arc<RwLock<EclipseAttackIndicators>>,

    /// Banned peers and subnets
    banned_entities: Arc<RwLock<BannedEntities>>,

    /// Peer advertisement tracking
    peer_advertisements: Arc<RwLock<HashMap<PeerId, HashSet<PeerId>>>>,

    /// Last rotation time
    last_rotation: Arc<RwLock<Instant>>,

    /// PoW challenge cache
    pow_challenges: Arc<RwLock<HashMap<PeerId, PowChallenge>>>,
}

/// Connection event for history tracking
#[derive(Debug, Clone)]
struct ConnectionEvent {
    peer_id: PeerId,
    ip_address: IpAddr,
    event_type: ConnectionEventType,
    timestamp: Instant,
}

#[derive(Debug, Clone)]
enum ConnectionEventType {
    Connected,
    Disconnected,
    Rejected(String),
    Banned(String),
}

/// Banned entities tracker
#[derive(Debug, Default)]
struct BannedEntities {
    /// Banned peer IDs
    peers: HashMap<PeerId, BanInfo>,

    /// Banned IP addresses
    ips: HashMap<IpAddr, BanInfo>,

    /// Banned subnets
    subnets: HashMap<String, BanInfo>,

    /// Banned ASNs
    asns: HashMap<u32, BanInfo>,
}

#[derive(Debug, Clone)]
struct BanInfo {
    reason: String,
    banned_at: Instant,
    duration: Duration,
}

/// Proof-of-work challenge
#[derive(Debug, Clone)]
struct PowChallenge {
    nonce: [u8; 32],
    difficulty: u8,
    issued_at: Instant,
    completed: bool,
}

impl EclipsePreventionSystem {
    /// Create a new eclipse prevention system
    pub fn new(config: EclipsePreventionConfig) -> Self {
        Self {
            config,
            connections: Arc::new(RwLock::new(HashMap::new())),
            anchor_peers: Arc::new(RwLock::new(HashSet::new())),
            connection_history: Arc::new(RwLock::new(VecDeque::with_capacity(1000))),
            attack_indicators: Arc::new(RwLock::new(EclipseAttackIndicators::default())),
            banned_entities: Arc::new(RwLock::new(BannedEntities::default())),
            peer_advertisements: Arc::new(RwLock::new(HashMap::new())),
            last_rotation: Arc::new(RwLock::new(Instant::now())),
            pow_challenges: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Check if a new connection should be allowed
    pub async fn should_allow_connection(
        &self,
        peer_id: &PeerId,
        ip_address: IpAddr,
        is_inbound: bool,
    ) -> Result<bool, String> {
        // Check if banned
        if self.is_banned(peer_id, &ip_address).await {
            return Err("Peer or IP is banned".to_string());
        }

        // Check diversity requirements
        if !self
            .check_diversity_requirements(&ip_address, is_inbound)
            .await?
        {
            return Err("Connection would violate diversity requirements".to_string());
        }

        // Check for connection flooding
        if self.detect_connection_flooding(&ip_address).await {
            self.ban_ip(
                ip_address,
                "Connection flooding detected",
                Duration::from_secs(3600),
            )
            .await;
            return Err("Connection flooding detected".to_string());
        }

        // If PoW is required and this is inbound, check if challenge completed
        if self.config.require_pow_challenge
            && is_inbound
            && !self.has_completed_pow_challenge(peer_id).await
        {
            return Err("PoW challenge not completed".to_string());
        }

        Ok(true)
    }

    /// Register a new connection
    pub async fn register_connection(
        &self,
        peer_id: PeerId,
        ip_address: IpAddr,
        is_inbound: bool,
        is_anchor: bool,
    ) -> Result<(), String> {
        let subnet = self.calculate_subnet(&ip_address);
        let asn = self.lookup_asn(&ip_address).await;
        let region = self.lookup_region(&ip_address).await;

        let info = PeerConnectionInfo {
            peer_id,
            ip_address,
            subnet,
            asn,
            region,
            is_inbound,
            is_anchor,
            connected_at: Instant::now(),
            last_useful_at: Instant::now(),
            behavior_score: 100.0,
            pow_completed: !is_inbound || !self.config.require_pow_challenge,
        };

        let mut connections = self.connections.write().await;
        connections.insert(peer_id, info);

        if is_anchor {
            let mut anchors = self.anchor_peers.write().await;
            anchors.insert(peer_id);
        }

        // Record connection event
        self.record_connection_event(peer_id, ip_address, ConnectionEventType::Connected)
            .await;

        // Check for eclipse attack indicators
        self.analyze_connection_patterns().await;

        Ok(())
    }

    /// Generate a PoW challenge for a peer
    pub async fn generate_pow_challenge(&self, peer_id: &PeerId) -> (Vec<u8>, u8) {
        let mut nonce = [0u8; 32];
        thread_rng().fill(&mut nonce);

        let challenge = PowChallenge {
            nonce,
            difficulty: self.config.pow_difficulty,
            issued_at: Instant::now(),
            completed: false,
        };

        let mut challenges = self.pow_challenges.write().await;
        challenges.insert(*peer_id, challenge);

        (nonce.to_vec(), self.config.pow_difficulty)
    }

    /// Verify a PoW challenge solution
    pub async fn verify_pow_challenge(&self, peer_id: &PeerId, solution: &[u8]) -> bool {
        let mut challenges = self.pow_challenges.write().await;

        if let Some(challenge) = challenges.get_mut(peer_id) {
            // Check if challenge expired (5 minutes)
            if challenge.issued_at.elapsed() > Duration::from_secs(300) {
                challenges.remove(peer_id);
                return false;
            }

            // Verify the solution
            let mut hasher = Sha256::new();
            hasher.update(challenge.nonce);
            hasher.update(solution);
            let hash = hasher.finalize();

            if count_leading_zeros(&hash) >= challenge.difficulty {
                challenge.completed = true;

                // Update connection info
                if let Some(connections) = self.connections.write().await.get_mut(peer_id) {
                    connections.pow_completed = true;
                }

                return true;
            }
        }

        false
    }

    /// Check if peer needs rotation
    pub async fn check_rotation_needed(&self) -> bool {
        let last_rotation = *self.last_rotation.read().await;

        // Check if rotation interval has passed
        if last_rotation.elapsed() >= self.config.rotation_interval {
            return true;
        }

        // Check for eclipse attack indicators
        let indicators = self.attack_indicators.read().await;
        if indicators.detected_at.is_some() {
            return true;
        }

        false
    }

    /// Get peers to disconnect during rotation
    pub async fn get_rotation_candidates(&self) -> Vec<PeerId> {
        let connections = self.connections.read().await;
        let anchor_peers = self.anchor_peers.read().await;

        // Don't rotate anchor peers
        let mut candidates: Vec<_> = connections
            .iter()
            .filter(|(peer_id, _)| !anchor_peers.contains(peer_id))
            .collect();

        // Sort by behavior score and connection time
        candidates.sort_by(|a, b| {
            let score_cmp = a.1.behavior_score.partial_cmp(&b.1.behavior_score).unwrap();
            if score_cmp == std::cmp::Ordering::Equal {
                a.1.connected_at.cmp(&b.1.connected_at)
            } else {
                score_cmp
            }
        });

        // Take bottom percentage
        let rotation_count = (candidates.len() as f64 * self.config.rotation_percentage) as usize;
        candidates
            .into_iter()
            .take(rotation_count)
            .map(|(peer_id, _)| *peer_id)
            .collect()
    }

    /// Update peer behavior score
    pub async fn update_behavior_score(&self, peer_id: &PeerId, delta: f64) {
        let mut connections = self.connections.write().await;
        if let Some(info) = connections.get_mut(peer_id) {
            info.behavior_score = (info.behavior_score + delta).max(0.0).min(100.0);

            // Ban if score too low
            if info.behavior_score < 10.0 {
                drop(connections);
                self.ban_peer(*peer_id, "Low behavior score", self.config.ban_duration)
                    .await;
            }
        }
    }

    /// Check diversity requirements
    async fn check_diversity_requirements(
        &self,
        ip_address: &IpAddr,
        is_inbound: bool,
    ) -> Result<bool, String> {
        let connections = self.connections.read().await;

        // Skip diversity checks if we have too few connections
        if connections.len() < self.config.min_connections_for_diversity {
            return Ok(true);
        }

        let total_connections = connections.len() as f64;
        let subnet = self.calculate_subnet(ip_address);

        // Check subnet diversity
        let subnet_count = connections
            .values()
            .filter(|info| info.subnet == subnet)
            .count() as f64;

        if subnet_count / total_connections >= self.config.max_subnet_percentage {
            return Err(format!(
                "Too many connections from subnet {}: {:.1}% (max {:.1}%)",
                subnet,
                (subnet_count / total_connections) * 100.0,
                self.config.max_subnet_percentage * 100.0
            ));
        }

        // Check ASN diversity
        if let Some(asn) = self.lookup_asn(ip_address).await {
            let asn_count = connections
                .values()
                .filter(|info| info.asn == Some(asn))
                .count() as f64;

            if asn_count / total_connections >= self.config.max_asn_percentage {
                return Err(format!(
                    "Too many connections from ASN {}: {:.1}% (max {:.1}%)",
                    asn,
                    (asn_count / total_connections) * 100.0,
                    self.config.max_asn_percentage * 100.0
                ));
            }
        }

        // Check inbound/outbound ratio
        if is_inbound {
            let inbound_count = connections.values().filter(|info| info.is_inbound).count() as f64;

            if inbound_count / total_connections >= self.config.max_inbound_percentage {
                return Err(format!(
                    "Too many inbound connections: {:.1}% (max {:.1}%)",
                    (inbound_count / total_connections) * 100.0,
                    self.config.max_inbound_percentage * 100.0
                ));
            }
        }

        Ok(true)
    }

    /// Detect connection flooding
    async fn detect_connection_flooding(&self, ip_address: &IpAddr) -> bool {
        let history = self.connection_history.read().await;
        let one_minute_ago = Instant::now() - Duration::from_secs(60);
        let subnet = self.calculate_subnet(ip_address);

        // Count recent connections from this subnet
        let recent_connections = history
            .iter()
            .filter(|event| {
                event.timestamp > one_minute_ago
                    && matches!(event.event_type, ConnectionEventType::Connected)
                    && self.calculate_subnet(&event.ip_address) == subnet
            })
            .count();

        // More than 10 connections per minute from same subnet is suspicious
        recent_connections > 10
    }

    /// Analyze connection patterns for eclipse attack
    async fn analyze_connection_patterns(&self) {
        if !self.config.enable_behavioral_analysis {
            return;
        }

        let connections = self.connections.read().await;
        let history = self.connection_history.read().await;
        let mut indicators = self.attack_indicators.write().await;

        // Reset indicators
        *indicators = EclipseAttackIndicators::default();

        // Check for rapid connections from similar IPs
        let recent_events: Vec<_> = history
            .iter()
            .filter(|e| e.timestamp > Instant::now() - Duration::from_secs(300))
            .collect();

        if recent_events.len() > 50 {
            indicators.connection_flooding = true;
        }

        // Check diversity collapse
        if connections.len() >= self.config.min_connections_for_diversity {
            let diversity_score = self.calculate_diversity_score(&connections).await;
            if diversity_score < 0.3 {
                indicators.diversity_collapse = true;
            }
        }

        // Check peer advertisement convergence
        let advertisements = self.peer_advertisements.read().await;
        if self.detect_advertisement_convergence(&advertisements).await {
            indicators.address_convergence = true;
        }

        // If multiple indicators present, mark as detected
        let indicator_count = [
            indicators.connection_flooding,
            indicators.address_convergence,
            indicators.transaction_censorship,
            indicators.chain_manipulation,
            indicators.diversity_collapse,
            indicators.coordinated_behavior,
        ]
        .iter()
        .filter(|&&x| x)
        .count();

        if indicator_count as f64 / 6.0 >= self.config.eclipse_detection_threshold {
            indicators.detected_at = Some(Instant::now());
        }
    }

    /// Calculate network diversity score
    async fn calculate_diversity_score(
        &self,
        connections: &HashMap<PeerId, PeerConnectionInfo>,
    ) -> f64 {
        if connections.is_empty() {
            return 1.0;
        }

        let total = connections.len() as f64;

        // Calculate subnet diversity (Shannon entropy)
        let mut subnet_counts: HashMap<String, usize> = HashMap::new();
        for info in connections.values() {
            *subnet_counts.entry(info.subnet.clone()).or_insert(0) += 1;
        }

        let subnet_entropy = subnet_counts
            .values()
            .map(|&count| {
                let p = count as f64 / total;
                -p * p.log2()
            })
            .sum::<f64>();

        // Normalize to 0-1 range
        let max_entropy = (total).log2();
        subnet_entropy / max_entropy
    }

    /// Detect advertisement convergence (many peers advertising same addresses)
    async fn detect_advertisement_convergence(
        &self,
        advertisements: &HashMap<PeerId, HashSet<PeerId>>,
    ) -> bool {
        if advertisements.len() < 10 {
            return false;
        }

        // Count how many peers advertise each peer
        let mut peer_advertisement_counts: HashMap<PeerId, usize> = HashMap::new();
        for advertised_peers in advertisements.values() {
            for peer in advertised_peers {
                *peer_advertisement_counts.entry(*peer).or_insert(0) += 1;
            }
        }

        // If any peer is advertised by more than 70% of peers, suspicious
        let threshold = (advertisements.len() as f64 * 0.7) as usize;
        peer_advertisement_counts
            .values()
            .any(|&count| count > threshold)
    }

    /// Record peer advertisements
    pub async fn record_peer_advertisement(
        &self,
        from_peer: PeerId,
        advertised_peers: Vec<PeerId>,
    ) {
        let mut advertisements = self.peer_advertisements.write().await;
        advertisements.insert(from_peer, advertised_peers.into_iter().collect());

        // Analyze patterns
        drop(advertisements);
        self.analyze_connection_patterns().await;
    }

    /// Check if entity is banned
    async fn is_banned(&self, peer_id: &PeerId, ip_address: &IpAddr) -> bool {
        let banned = self.banned_entities.read().await;
        let now = Instant::now();

        // Check peer ban
        if let Some(ban_info) = banned.peers.get(peer_id) {
            if now < ban_info.banned_at + ban_info.duration {
                return true;
            }
        }

        // Check IP ban
        if let Some(ban_info) = banned.ips.get(ip_address) {
            if now < ban_info.banned_at + ban_info.duration {
                return true;
            }
        }

        // Check subnet ban
        let subnet = self.calculate_subnet(ip_address);
        if let Some(ban_info) = banned.subnets.get(&subnet) {
            if now < ban_info.banned_at + ban_info.duration {
                return true;
            }
        }

        false
    }

    /// Ban a peer
    async fn ban_peer(&self, peer_id: PeerId, reason: &str, duration: Duration) {
        let mut banned = self.banned_entities.write().await;
        banned.peers.insert(
            peer_id,
            BanInfo {
                reason: reason.to_string(),
                banned_at: Instant::now(),
                duration,
            },
        );

        // Record ban event
        if let Some(info) = self.connections.read().await.get(&peer_id) {
            self.record_connection_event(
                peer_id,
                info.ip_address,
                ConnectionEventType::Banned(reason.to_string()),
            )
            .await;
        }
    }

    /// Ban an IP address
    async fn ban_ip(&self, ip: IpAddr, reason: &str, duration: Duration) {
        let mut banned = self.banned_entities.write().await;
        banned.ips.insert(
            ip,
            BanInfo {
                reason: reason.to_string(),
                banned_at: Instant::now(),
                duration,
            },
        );
    }

    /// Check if peer has completed PoW challenge
    async fn has_completed_pow_challenge(&self, peer_id: &PeerId) -> bool {
        if let Some(challenge) = self.pow_challenges.read().await.get(peer_id) {
            challenge.completed
        } else {
            false
        }
    }

    /// Record connection event
    async fn record_connection_event(
        &self,
        peer_id: PeerId,
        ip_address: IpAddr,
        event_type: ConnectionEventType,
    ) {
        let mut history = self.connection_history.write().await;

        history.push_back(ConnectionEvent {
            peer_id,
            ip_address,
            event_type,
            timestamp: Instant::now(),
        });

        // Keep only recent history
        while history.len() > 1000 {
            history.pop_front();
        }
    }

    /// Calculate subnet for an IP address
    fn calculate_subnet(&self, ip: &IpAddr) -> String {
        match ip {
            IpAddr::V4(ipv4) => {
                let octets = ipv4.octets();
                format!("{}.{}.{}.0/24", octets[0], octets[1], octets[2])
            }
            IpAddr::V6(ipv6) => {
                let segments = ipv6.segments();
                format!("{:x}:{:x}:{:x}::/48", segments[0], segments[1], segments[2])
            }
        }
    }

    /// Lookup ASN for IP (mock implementation)
    async fn lookup_asn(&self, _ip: &IpAddr) -> Option<u32> {
        // In production, this would query an ASN database
        None
    }

    /// Lookup geographic region for IP (mock implementation)
    async fn lookup_region(&self, _ip: &IpAddr) -> Option<String> {
        // In production, this would query a GeoIP database
        None
    }

    /// Get current eclipse attack risk level
    pub async fn get_eclipse_risk_level(&self) -> EclipseRiskLevel {
        let indicators = self.attack_indicators.read().await;
        let connections = self.connections.read().await;

        // Check if attack detected
        if indicators.detected_at.is_some() {
            return EclipseRiskLevel::Critical;
        }

        // Calculate risk factors
        let diversity_score = self.calculate_diversity_score(&connections).await;
        let anchor_ratio =
            self.anchor_peers.read().await.len() as f64 / connections.len().max(1) as f64;

        if diversity_score < 0.3 || anchor_ratio < 0.1 {
            EclipseRiskLevel::High
        } else if diversity_score < 0.5 || anchor_ratio < 0.2 {
            EclipseRiskLevel::Medium
        } else if diversity_score < 0.7 {
            EclipseRiskLevel::Low
        } else {
            EclipseRiskLevel::Minimal
        }
    }
}

/// Eclipse attack risk levels
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EclipseRiskLevel {
    Minimal,
    Low,
    Medium,
    High,
    Critical,
}

/// Count leading zero bits in a hash
fn count_leading_zeros(hash: &[u8]) -> u8 {
    let mut count = 0;
    for byte in hash {
        if *byte == 0 {
            count += 8;
        } else {
            count += byte.leading_zeros() as u8;
            break;
        }
    }
    count
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_diversity_requirements() {
        let config = EclipsePreventionConfig::default();
        let system = EclipsePreventionSystem::new(config);

        // Add some test connections
        for i in 0..10 {
            let peer_id = PeerId::random();
            let ip = IpAddr::V4(Ipv4Addr::new(192, 168, 1, i));
            system
                .register_connection(peer_id, ip, true, false)
                .await
                .unwrap();
        }

        // Try to add another from same subnet - should fail
        let peer_id = PeerId::random();
        let ip = IpAddr::V4(Ipv4Addr::new(192, 168, 1, 100));
        let result = system.should_allow_connection(&peer_id, ip, true).await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_pow_challenge() {
        let mut config = EclipsePreventionConfig::default();
        config.pow_difficulty = 8; // Lower difficulty for testing
        let system = EclipsePreventionSystem::new(config);

        let peer_id = PeerId::random();
        let (nonce, difficulty) = system.generate_pow_challenge(&peer_id).await;

        // Solve the challenge
        let mut solution = vec![0u8; 8];
        loop {
            thread_rng().fill(&mut solution[..]);

            let mut hasher = Sha256::new();
            hasher.update(&nonce);
            hasher.update(&solution);
            let hash = hasher.finalize();

            if count_leading_zeros(&hash) >= difficulty {
                break;
            }
        }

        // Verify solution
        assert!(system.verify_pow_challenge(&peer_id, &solution).await);
    }

    #[tokio::test]
    async fn test_connection_flooding_detection() {
        let config = EclipsePreventionConfig::default();
        let system = EclipsePreventionSystem::new(config);

        let subnet_ip = IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1));

        // Simulate rapid connections from same subnet
        for i in 0..15 {
            let peer_id = PeerId::random();
            let ip = IpAddr::V4(Ipv4Addr::new(192, 168, 1, i + 1));
            system
                .record_connection_event(peer_id, ip, ConnectionEventType::Connected)
                .await;
        }

        // Should detect flooding
        assert!(system.detect_connection_flooding(&subnet_ip).await);
    }
}
