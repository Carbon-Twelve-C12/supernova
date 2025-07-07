use libp2p::{
    PeerId,
    multiaddr::Multiaddr,
};
use std::{
    collections::HashMap,
    net::IpAddr,
    time::{Duration, Instant},
    sync::{Arc, Mutex},
};
use dashmap::DashMap;
use crate::network::peer_diversity::IpSubnet;
use tracing::{debug, info, warn};

/// State of a peer connection
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PeerState {
    /// Peer is being dialed
    Dialing,
    /// Connection successfully established
    Connected,
    /// Connected and handshake complete
    Ready,
    /// Connection lost
    Disconnected,
    /// Peer is banned
    Banned,
}

/// Detailed information about a peer
#[derive(Debug, Clone)]
pub struct PeerInfo {
    /// Unique peer ID
    pub peer_id: PeerId,
    /// Current state of the peer
    pub state: PeerState,
    /// Known addresses for this peer
    pub addresses: Vec<Multiaddr>,
    /// When the peer was first discovered
    pub first_seen: Instant,
    /// When the peer was last seen
    pub last_seen: Instant,
    /// When we last sent data to this peer
    pub last_sent: Option<Instant>,
    /// Connection direction (true if inbound)
    pub is_inbound: bool,
    /// Protocol version reported by the peer
    pub protocol_version: Option<u32>,
    /// User agent reported by the peer
    pub user_agent: Option<String>,
    /// Blockchain height reported by the peer
    pub height: Option<u64>,
    /// Best block hash reported by the peer
    pub best_hash: Option<[u8; 32]>,
    /// Total difficulty reported by the peer
    pub total_difficulty: Option<u64>,
    /// Network addresses (IPs) associated with this peer
    pub network_info: Option<PeerNetworkInfo>,
    /// Reputation score (higher is better)
    pub reputation: i32,
    /// Number of failed connection attempts
    pub failed_attempts: u32,
    /// Ping latency in milliseconds
    pub ping_ms: Option<u64>,
    /// Whether the peer has been verified (basic handshake complete)
    pub verified: bool,
    /// Service flags (bitfield indicating supported services)
    pub services: u64,
    /// Total bytes sent to this peer
    pub bytes_sent: u64,
    /// Total bytes received from this peer
    pub bytes_received: u64,
    /// Extended information about the peer
    pub metadata: PeerMetadata,
}

/// Network information about a peer
#[derive(Debug, Clone)]
pub struct PeerNetworkInfo {
    /// IP address of the peer
    pub ip: IpAddr,
    /// Subnet the peer belongs to
    pub subnet: IpSubnet,
    /// Autonomous System Number (if known)
    pub asn: Option<u32>,
    /// Geographic region (if known)
    pub region: Option<String>,
}

/// Extended metadata about a peer
#[derive(Debug, Clone, Default)]
pub struct PeerMetadata {
    /// Number of blocks received from this peer
    pub blocks_received: u64,
    /// Number of transactions received from this peer
    pub transactions_received: u64,
    /// Number of invalid messages received from this peer
    pub invalid_messages: u64,
    /// Number of successful requests to this peer
    pub successful_requests: u64,
    /// Number of failed requests to this peer
    pub failed_requests: u64,
    /// Custom attributes for this peer
    pub attributes: HashMap<String, String>,
}

/// Reasons for banning a peer
#[derive(Debug, Clone)]
pub enum BanReason {
    /// Peer sent invalid blocks
    InvalidBlocks,
    /// Peer sent invalid transactions
    InvalidTransactions,
    /// Peer sent too many invalid messages
    TooManyInvalidMessages,
    /// Peer is misbehaving
    Misbehavior,
    /// Peer has incompatible protocol version
    IncompatibleVersion,
    /// Peer is spamming
    Spamming,
    /// Manual ban
    Manual(String),
    /// Other reason
    Other(String),
}

/// Manager for tracking connected peers
pub struct PeerManager {
    /// All known peers with their information
    peers: DashMap<PeerId, PeerInfo>,
    /// Peers currently banned with expiration time
    banned_peers: DashMap<PeerId, (Instant, Duration, BanReason)>,
    /// Subnet to peer mapping for diversity tracking
    subnet_peers: DashMap<IpSubnet, Vec<PeerId>>,
    /// Region to peer mapping for geographic diversity
    region_peers: DashMap<String, Vec<PeerId>>,
    /// Maximum number of peer records to keep
    max_peer_records: usize,
    /// Default ban duration
    default_ban_duration: Duration,
    /// IP subnets that are banned
    banned_subnets: DashMap<IpSubnet, (Instant, Duration)>,
    /// Recent connection attempts by subnet, for rate limiting
    connection_attempts: Arc<Mutex<HashMap<IpSubnet, Vec<Instant>>>>,
    /// Maximum connection attempts per minute
    max_attempts_per_minute: usize,
}

impl PeerManager {
    /// Create a new peer manager
    pub fn new() -> Self {
        Self {
            peers: DashMap::new(),
            banned_peers: DashMap::new(),
            subnet_peers: DashMap::new(),
            region_peers: DashMap::new(),
            max_peer_records: 10000,
            default_ban_duration: Duration::from_secs(3600), // 1 hour default ban
            banned_subnets: DashMap::new(),
            connection_attempts: Arc::new(Mutex::new(HashMap::new())),
            max_attempts_per_minute: 5, // Max 5 attempts per minute per subnet
        }
    }
    
    /// Add or update a peer
    pub fn add_or_update_peer(&self, peer_id: PeerId, info: PeerInfo) {
        // Update network diversity tracking if applicable
        if let Some(network_info) = &info.network_info {
            // Update subnet tracking
            self.subnet_peers
                .entry(network_info.subnet.clone())
                .and_modify(|peers| {
                    if !peers.contains(&peer_id) {
                        peers.push(peer_id);
                    }
                })
                .or_insert_with(|| vec![peer_id]);
                
            // Update region tracking if available
            if let Some(region) = &network_info.region {
                self.region_peers
                    .entry(region.clone())
                    .and_modify(|peers| {
                        if !peers.contains(&peer_id) {
                            peers.push(peer_id);
                        }
                    })
                    .or_insert_with(|| vec![peer_id]);
            }
        }
        
        // Add or update peer info
        self.peers.insert(peer_id, info);
        
        // Clean up old peers if we're over the limit
        if self.peers.len() > self.max_peer_records {
            self.prune_old_peers();
        }
    }
    
    /// Get information about a peer
    pub fn get_peer(&self, peer_id: &PeerId) -> Option<PeerInfo> {
        self.peers.get(peer_id).map(|p| p.clone())
    }
    
    /// Update peer state
    pub fn update_peer_state(&self, peer_id: &PeerId, state: PeerState) -> bool {
        if let Some(mut peer) = self.peers.get_mut(peer_id) {
            peer.state = state;
            peer.last_seen = Instant::now();
            true
        } else {
            false
        }
    }
    
    /// Add an address for a peer
    pub fn add_peer_address(&self, peer_id: &PeerId, addr: Multiaddr) {
        if let Some(mut peer) = self.peers.get_mut(peer_id) {
            if !peer.addresses.contains(&addr) {
                peer.addresses.push(addr);
            }
        } else {
            // Create a new peer entry with this address
            let now = Instant::now();
            let peer_info = PeerInfo {
                peer_id: peer_id.clone(),
                state: PeerState::Disconnected,
                addresses: vec![addr],
                first_seen: now,
                last_seen: now,
                last_sent: None,
                is_inbound: false,
                protocol_version: None,
                user_agent: None,
                height: None,
                best_hash: None,
                total_difficulty: None,
                network_info: None,
                reputation: 0,
                failed_attempts: 0,
                ping_ms: None,
                verified: false,
                services: 0,
                bytes_sent: 0,
                bytes_received: 0,
                metadata: PeerMetadata::default(),
            };
            self.peers.insert(peer_id.clone(), peer_info);
        }
    }
    
    /// Update peer's blockchain information
    pub fn update_peer_chain_info(
        &self,
        peer_id: &PeerId,
        height: u64,
        best_hash: [u8; 32],
        total_difficulty: u64,
    ) {
        if let Some(mut peer) = self.peers.get_mut(peer_id) {
            peer.height = Some(height);
            peer.best_hash = Some(best_hash);
            peer.total_difficulty = Some(total_difficulty);
            peer.last_seen = Instant::now();
        }
    }
    
    /// Update peer reputation score
    pub fn update_peer_reputation(&self, peer_id: &PeerId, delta: i32) {
        if let Some(mut peer) = self.peers.get_mut(peer_id) {
            peer.reputation = (peer.reputation + delta).clamp(-100, 100);
            
            // Automatically ban peers with very low reputation
            if peer.reputation <= -80 {
                self.ban_peer(
                    peer_id, 
                    self.default_ban_duration, 
                    BanReason::Misbehavior
                );
            }
        }
    }
    
    /// Ban a peer for a specific duration
    pub fn ban_peer(&self, peer_id: &PeerId, duration: Duration, reason: BanReason) {
        let expiration = Instant::now() + duration;
        
        // Update peer state to banned
        if let Some(mut peer) = self.peers.get_mut(peer_id) {
            peer.state = PeerState::Banned;
            
            // Also ban subnet if appropriate
            if let Some(network_info) = &peer.network_info {
                if matches!(reason, 
                          BanReason::InvalidBlocks | 
                          BanReason::Spamming | 
                          BanReason::TooManyInvalidMessages) {
                    self.banned_subnets.insert(
                        network_info.subnet.clone(), 
                        (Instant::now(), duration)
                    );
                    
                    warn!("Banned subnet {:?} due to peer {} misbehavior: {:?}", 
                         network_info.subnet, peer_id, reason);
                }
            }
        }
        
        // Add to banned peers map
        self.banned_peers.insert(peer_id.clone(), (expiration, duration, reason.clone()));
        
        info!("Banned peer {} for {} seconds: {:?}", 
             peer_id, duration.as_secs(), reason);
    }
    
    /// Check if a peer is banned
    pub fn is_peer_banned(&self, peer_id: &PeerId) -> bool {
        if let Some(entry) = self.banned_peers.get(peer_id) {
            let (expiration, _, _) = entry.value();
            if expiration.elapsed().as_secs() > 0 {
                // Ban expired, remove it
                self.banned_peers.remove(peer_id);
                false
            } else {
                true
            }
        } else {
            false
        }
    }
    
    /// Check if a subnet is banned
    pub fn is_subnet_banned(&self, subnet: &IpSubnet) -> bool {
        if let Some(entry) = self.banned_subnets.get(subnet) {
            let (expiration, _) = entry.value();
            if expiration.elapsed().as_secs() > 0 {
                // Ban expired, remove it
                self.banned_subnets.remove(subnet);
                false
            } else {
                true
            }
        } else {
            false
        }
    }
    
    /// Record a failed connection attempt
    pub fn record_failed_attempt(&self, peer_id: &PeerId) {
        if let Some(mut peer) = self.peers.get_mut(peer_id) {
            peer.failed_attempts += 1;
            
            // If too many failures, reduce reputation
            if peer.failed_attempts > 3 {
                peer.reputation -= 5;
            }
            
            // If more than 10 failures, ban temporarily
            if peer.failed_attempts > 10 {
                self.ban_peer(
                    peer_id,
                    Duration::from_secs(1800), // 30 minutes
                    BanReason::Other("Too many failed connection attempts".to_string())
                );
            }
        }
    }
    
    /// Check if connection attempt is allowed for a subnet (rate limiting)
    pub fn is_connection_attempt_allowed(&self, subnet: &IpSubnet) -> bool {
        let now = Instant::now();
        let one_minute_ago = now - Duration::from_secs(60);
        
        // Check if subnet is banned
        if self.is_subnet_banned(subnet) {
            return false;
        }
        
        let mut attempts = self.connection_attempts.lock().unwrap();
        
        // Clean up old attempts
        for (_, timestamps) in attempts.iter_mut() {
            timestamps.retain(|&timestamp| timestamp > one_minute_ago);
        }
        
        // Check and update rate for this subnet
        let subnet_attempts = attempts.entry(subnet.clone()).or_insert_with(Vec::new);
        
        if subnet_attempts.len() >= self.max_attempts_per_minute {
            false
        } else {
            subnet_attempts.push(now);
            true
        }
    }
    
    /// Get all peers in a specific state
    pub fn get_peers_by_state(&self, state: PeerState) -> Vec<PeerInfo> {
        self.peers
            .iter()
            .filter(|p| p.state == state)
            .map(|p| p.clone())
            .collect()
    }
    
    /// Get all connected peers
    pub fn get_connected_peers(&self) -> Vec<PeerInfo> {
        self.peers
            .iter()
            .filter(|p| p.state == PeerState::Connected || p.state == PeerState::Ready)
            .map(|p| p.clone())
            .collect()
    }
    
    /// Get number of inbound connections
    pub fn inbound_connection_count(&self) -> usize {
        self.peers
            .iter()
            .filter(|p| {
                p.is_inbound && 
                (p.state == PeerState::Connected || p.state == PeerState::Ready)
            })
            .count()
    }
    
    /// Get number of outbound connections
    pub fn outbound_connection_count(&self) -> usize {
        self.peers
            .iter()
            .filter(|p| {
                !p.is_inbound && 
                (p.state == PeerState::Connected || p.state == PeerState::Ready)
            })
            .count()
    }
    
    /// Find peers for a specific subnet
    pub fn get_peers_by_subnet(&self, subnet: &IpSubnet) -> Vec<PeerId> {
        self.subnet_peers
            .get(subnet)
            .map(|p| p.clone())
            .unwrap_or_default()
    }
    
    /// Find best peers by reputation score
    pub fn get_best_peers(&self, limit: usize) -> Vec<PeerInfo> {
        let mut peers: Vec<_> = self.peers
            .iter()
            .filter(|p| {
                p.state == PeerState::Connected || 
                p.state == PeerState::Ready
            })
            .map(|p| p.clone())
            .collect();
            
        // Sort by reputation (highest first)
        peers.sort_by(|a, b| b.reputation.cmp(&a.reputation));
        
        // Take top N
        peers.into_iter().take(limit).collect()
    }
    
    /// Clean up and remove old peer records
    fn prune_old_peers(&self) {
        // Find peers that haven't been seen in a long time
        let now = Instant::now();
        let one_week_ago = now - Duration::from_secs(7 * 24 * 3600);
        
        let old_peers: Vec<_> = self.peers
            .iter()
            .filter(|p| {
                p.last_seen < one_week_ago && 
                p.state == PeerState::Disconnected
            })
            .map(|p| p.peer_id)
            .collect();
            
        // Remove old peers
        for peer_id in old_peers {
            self.peers.remove(&peer_id);
        }
    }
    
    /// Get the total number of known peers
    pub fn peer_count(&self) -> usize {
        self.peers.len()
    }
    
    /// Get the number of banned peers
    pub fn banned_peer_count(&self) -> usize {
        // Cleanup expired bans first
        let now = Instant::now();
        let expired: Vec<_> = self.banned_peers
            .iter()
            .filter(|entry| {
                entry.value().0 < now
            })
            .map(|entry| entry.key().clone())
            .collect();
            
        for peer_id in expired {
            self.banned_peers.remove(&peer_id);
        }
        
        self.banned_peers.len()
    }
    
    /// Get a list of banned peers with their expiration times
    pub fn get_banned_peers(&self) -> Vec<(PeerId, Instant, Duration, BanReason)> {
        self.banned_peers
            .iter()
            .map(|entry| {
                (
                    entry.key().clone(),
                    entry.value().0,
                    entry.value().1,
                    entry.value().2.clone()
                )
            })
            .collect()
    }
}

/// Default implementation
impl Default for PeerManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use libp2p::PeerId;
    use std::net::{Ipv4Addr, IpAddr};
    
    fn create_test_peer(peer_id: PeerId, is_inbound: bool) -> PeerInfo {
        let now = Instant::now();
        PeerInfo {
            peer_id,
            state: PeerState::Connected,
            addresses: vec![],
            first_seen: now,
            last_seen: now,
            last_sent: None,
            is_inbound,
            protocol_version: Some(1),
            user_agent: Some("test-agent".to_string()),
            height: Some(100),
            best_hash: Some([0u8; 32]),
            total_difficulty: Some(1000),
            network_info: Some(PeerNetworkInfo {
                ip: IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1)),
                subnet: IpSubnet::new(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 0)), 24),
                asn: Some(64512),
                region: Some("test-region".to_string()),
            }),
            reputation: 0,
            failed_attempts: 0,
            ping_ms: Some(100),
            verified: true,
            services: 0,
            bytes_sent: 0,
            bytes_received: 0,
            metadata: PeerMetadata::default(),
        }
    }
    
    #[test]
    fn test_peer_management() {
        let manager = PeerManager::new();
        let peer_id = PeerId::random();
        let peer_info = create_test_peer(peer_id, false);
        
        // Add peer
        manager.add_or_update_peer(peer_id, peer_info.clone());
        
        // Get peer
        let retrieved = manager.get_peer(&peer_id).unwrap();
        assert_eq!(retrieved.peer_id, peer_id);
        
        // Update state
        assert!(manager.update_peer_state(&peer_id, PeerState::Ready));
        let updated = manager.get_peer(&peer_id).unwrap();
        assert_eq!(updated.state, PeerState::Ready);
        
        // Ban peer
        manager.ban_peer(&peer_id, Duration::from_secs(3600), BanReason::Manual("Test ban".to_string()));
        assert!(manager.is_peer_banned(&peer_id));
        
        // Check banned peers count
        assert_eq!(manager.banned_peer_count(), 1);
        
        // Get peers by state
        let ready_peers = manager.get_peers_by_state(PeerState::Ready);
        assert_eq!(ready_peers.len(), 0); // Should be 0 since the peer is now banned
        
        let banned_peers = manager.get_peers_by_state(PeerState::Banned);
        assert_eq!(banned_peers.len(), 1);
    }
    
    #[test]
    fn test_reputation_system() {
        let manager = PeerManager::new();
        let peer_id = PeerId::random();
        let peer_info = create_test_peer(peer_id, false);
        
        // Add peer
        manager.add_or_update_peer(peer_id, peer_info);
        
        // Update reputation
        manager.update_peer_reputation(&peer_id, 10);
        let peer = manager.get_peer(&peer_id).unwrap();
        assert_eq!(peer.reputation, 10);
        
        // Update again
        manager.update_peer_reputation(&peer_id, 20);
        let peer = manager.get_peer(&peer_id).unwrap();
        assert_eq!(peer.reputation, 30);
        
        // Test negative reputation
        manager.update_peer_reputation(&peer_id, -40);
        let peer = manager.get_peer(&peer_id).unwrap();
        assert_eq!(peer.reputation, -10);
        
        // Test clamping at -100
        manager.update_peer_reputation(&peer_id, -200);
        let peer = manager.get_peer(&peer_id).unwrap();
        assert_eq!(peer.reputation, -100);
        
        // Test auto-ban on low reputation
        let good_peer_id = PeerId::random();
        let good_peer_info = create_test_peer(good_peer_id, false);
        manager.add_or_update_peer(good_peer_id, good_peer_info);
        
        // Lower reputation to trigger auto-ban
        manager.update_peer_reputation(&good_peer_id, -85);
        assert!(manager.is_peer_banned(&good_peer_id));
    }
    
    #[test]
    fn test_connection_rate_limiting() {
        let manager = PeerManager::new();
        let subnet = IpSubnet::new(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 0)), 24);
        
        // First few attempts should be allowed
        assert!(manager.is_connection_attempt_allowed(&subnet));
        assert!(manager.is_connection_attempt_allowed(&subnet));
        assert!(manager.is_connection_attempt_allowed(&subnet));
        assert!(manager.is_connection_attempt_allowed(&subnet));
        assert!(manager.is_connection_attempt_allowed(&subnet));
        
        // Sixth attempt should be rate limited
        assert!(!manager.is_connection_attempt_allowed(&subnet));
        
        // Different subnet should be allowed
        let different_subnet = IpSubnet::new(IpAddr::V4(Ipv4Addr::new(10, 0, 0, 0)), 24);
        assert!(manager.is_connection_attempt_allowed(&different_subnet));
    }
} 