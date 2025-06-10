use std::collections::{HashMap, HashSet};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

use dashmap::DashMap;
use libp2p::PeerId;
use libp2p::multiaddr::Protocol;
use rand::{Rng, thread_rng};
use tracing::{debug, info, warn};

/// Represents a subnet for diversity tracking
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct IpSubnet {
    /// Base IP address for the subnet
    base: IpAddr,
    /// Subnet mask bits
    mask_bits: u8,
}

impl IpSubnet {
    /// Create a subnet from an IP address with default mask
    pub fn from_ip(ip: IpAddr) -> Self {
        match ip {
            IpAddr::V4(_) => Self {
                base: ip,
                mask_bits: 24, // /24 for IPv4
            },
            IpAddr::V6(_) => Self {
                base: ip,
                mask_bits: 48, // /48 for IPv6
            },
        }
    }

    /// Create a subnet with specific mask bits
    pub fn new(ip: IpAddr, mask_bits: u8) -> Self {
        Self {
            base: ip,
            mask_bits,
        }
    }

    /// Check if an IP address belongs to this subnet
    pub fn contains(&self, ip: IpAddr) -> bool {
        match (self.base, ip) {
            (IpAddr::V4(base), IpAddr::V4(target)) => {
                let mask = !0u32 << (32 - self.mask_bits);
                let base_bits = u32::from_be_bytes(base.octets());
                let target_bits = u32::from_be_bytes(target.octets());
                (base_bits & mask) == (target_bits & mask)
            }
            (IpAddr::V6(base), IpAddr::V6(target)) => {
                let mask = !0u128 << (128 - self.mask_bits);
                let base_bits = u128::from_be_bytes(base.octets());
                let target_bits = u128::from_be_bytes(target.octets());
                (base_bits & mask) == (target_bits & mask)
            }
            _ => false, // Different IP versions
        }
    }
}

/// Connection strategy for peer diversity management
#[derive(Debug, Clone)]
pub enum ConnectionStrategy {
    /// Maximize diversity across all dimensions
    MaximizeDiversity,
    /// Maintain minimum diversity while prioritizing performance
    BalancedDiversity,
    /// Prioritize specific geographic regions
    GeographicFocus(Vec<String>),
}

/// Enhanced configuration for Eclipse attack prevention
#[derive(Debug, Clone)]
pub struct EclipsePreventionConfig {
    /// Minimum number of outbound connections to maintain
    pub min_outbound_connections: usize,
    /// Force peer rotation at this interval
    pub rotation_interval: Duration,
    /// Whether to enable automatic peer rotation
    pub enable_automatic_rotation: bool,
    /// Maximum connections per subnet
    pub max_connections_per_subnet: usize,
    /// Maximum connections per ASN
    pub max_connections_per_asn: usize,
    /// Maximum connections per region
    pub max_connections_per_region: usize,
    /// Ratio of inbound to outbound connections
    pub max_inbound_ratio: f64,
}

impl Default for EclipsePreventionConfig {
    fn default() -> Self {
        Self {
            min_outbound_connections: 8,
            rotation_interval: Duration::from_secs(3600), // 1 hour
            enable_automatic_rotation: true,
            max_connections_per_subnet: 3,
            max_connections_per_asn: 8,
            max_connections_per_region: 15,
            max_inbound_ratio: 3.0,
        }
    }
}

/// Manages peer diversity to prevent Sybil and Eclipse attacks
pub struct PeerDiversityManager {
    /// Track distribution of peers across subnets
    subnet_distribution: DashMap<IpSubnet, usize>,
    /// Track distribution across Autonomous System Numbers (if available)
    asn_distribution: DashMap<u32, usize>,
    /// Track geographic distribution of peers
    geographic_distribution: DashMap<String, usize>,
    /// Map peers to their network information
    peer_info: DashMap<PeerId, PeerNetworkInfo>,
    /// Minimum diversity score required
    min_diversity_score: f64,
    /// Connection strategy in use
    connection_strategy: ConnectionStrategy,
    /// Maximum connection rate per minute per subnet
    max_connection_rate: usize,
    /// Connection attempt tracking for rate limiting
    connection_attempts: Arc<RwLock<HashMap<IpSubnet, Vec<Instant>>>>,
    /// Eclipse prevention configuration
    eclipse_config: EclipsePreventionConfig,
    /// Last peer rotation time
    last_rotation: Instant,
    /// Inbound connections map to track inbound/outbound ratio
    inbound_connections: DashMap<PeerId, bool>,
    /// Protected peers (never rotated)
    protected_peers: HashSet<PeerId>,
    /// Suspicious subnet patterns
    suspicious_subnets: DashMap<IpSubnet, usize>,
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
    /// Time when peer was first seen
    pub first_seen: Instant,
    /// Time of most recent connection
    pub last_connection: Instant,
    /// Connection score based on behavior
    pub connection_score: f64,
    /// Whether this is an inbound connection
    pub is_inbound: bool,
    /// Peer behavior flags
    pub behavior_flags: PeerBehaviorFlags,
}

/// Flags that track specific peer behaviors for security monitoring
#[derive(Debug, Clone, Default)]
pub struct PeerBehaviorFlags {
    /// Sent too many address messages
    pub address_flooding: bool,
    /// Sent multiple conflicting headers
    pub conflicting_headers: bool,
    /// Attempts to poison peer routing tables
    pub routing_poisoning: bool,
    /// Suspicious connection patterns
    pub suspicious_connection_pattern: bool,
    /// Aggressively advertises peers
    pub aggressive_advertising: bool,
}

impl PeerDiversityManager {
    /// Create a new peer diversity manager with default settings
    pub fn new() -> Self {
        Self {
            subnet_distribution: DashMap::new(),
            asn_distribution: DashMap::new(),
            geographic_distribution: DashMap::new(),
            peer_info: DashMap::new(),
            min_diversity_score: 0.6, // Default minimum diversity score
            connection_strategy: ConnectionStrategy::BalancedDiversity,
            max_connection_rate: 10, // Maximum 10 connections per minute per subnet
            connection_attempts: Arc::new(RwLock::new(HashMap::new())),
            eclipse_config: EclipsePreventionConfig::default(),
            last_rotation: Instant::now(),
            inbound_connections: DashMap::new(),
            protected_peers: HashSet::new(),
            suspicious_subnets: DashMap::new(),
        }
    }

    /// Create a new peer diversity manager with custom settings
    pub fn with_config(min_diversity_score: f64, strategy: ConnectionStrategy, max_rate: usize) -> Self {
        Self {
            subnet_distribution: DashMap::new(),
            asn_distribution: DashMap::new(),
            geographic_distribution: DashMap::new(),
            peer_info: DashMap::new(),
            min_diversity_score,
            connection_strategy: strategy,
            max_connection_rate: max_rate,
            connection_attempts: Arc::new(RwLock::new(HashMap::new())),
            eclipse_config: EclipsePreventionConfig::default(),
            last_rotation: Instant::now(),
            inbound_connections: DashMap::new(),
            protected_peers: HashSet::new(),
            suspicious_subnets: DashMap::new(),
        }
    }
    
    /// Set custom Eclipse prevention configuration
    pub fn set_eclipse_prevention_config(&mut self, config: EclipsePreventionConfig) {
        self.eclipse_config = config;
    }
    
    /// Add a peer to the protected list (never rotated)
    pub fn add_protected_peer(&mut self, peer_id: PeerId) {
        self.protected_peers.insert(peer_id);
    }
    
    /// Remove a peer from the protected list
    pub fn remove_protected_peer(&mut self, peer_id: &PeerId) {
        self.protected_peers.remove(peer_id);
    }

    /// Register a new peer with the diversity manager
    pub fn register_peer(&self, peer_id: PeerId, addr: &libp2p::Multiaddr, is_inbound: bool) -> bool {
        // Extract IP from multiaddr
        let ip = match Self::extract_ip_from_multiaddr(addr) {
            Some(ip) => ip,
            None => {
                debug!("Could not extract IP from multiaddr: {:?}", addr);
                return false;
            }
        };

        // Check connection rate limits
        let subnet = IpSubnet::from_ip(ip);
        if !self.check_connection_rate(&subnet) {
            warn!("Connection rate limit exceeded for subnet: {:?}", subnet);
            return false;
        }
        
        // Check if this would exceed per-subnet limits
        if is_inbound && self.would_exceed_subnet_limits(&subnet) {
            warn!("Connection would exceed subnet limits for: {:?}", subnet);
            return false;
        }

        // Get region and ASN info (in production would use GeoIP database)
        let region = self.get_region_for_ip(&ip);
        let asn = self.get_asn_for_ip(&ip);

        // Create peer info
        let now = Instant::now();
        let peer_info = PeerNetworkInfo {
            ip,
            subnet: subnet.clone(),
            asn,
            region: region.clone(),
            first_seen: now,
            last_connection: now,
            connection_score: 1.0, // Initial neutral score
            is_inbound,
            behavior_flags: PeerBehaviorFlags::default(),
        };

        // Update distribution maps
        self.subnet_distribution
            .entry(subnet)
            .and_modify(|count| *count += 1)
            .or_insert(1);

        if let Some(asn_value) = asn {
            self.asn_distribution
                .entry(asn_value)
                .and_modify(|count| *count += 1)
                .or_insert(1);
        }

        if let Some(region_value) = region {
            self.geographic_distribution
                .entry(region_value)
                .and_modify(|count| *count += 1)
                .or_insert(1);
        }

        // Store peer info
        self.peer_info.insert(peer_id, peer_info);
        
        // Update inbound connection tracking
        self.inbound_connections.insert(peer_id, is_inbound);
        
        true
    }

    /// Update a peer's score based on behavior
    pub fn update_peer_score(&self, peer_id: &PeerId, score_delta: f64) {
        if let Some(mut peer_info) = self.peer_info.get_mut(peer_id) {
            peer_info.connection_score += score_delta;
            debug!("Updated score for peer {:?} to {}", peer_id, peer_info.connection_score);
        }
    }
    
    /// Flag suspicious peer behavior
    pub fn flag_suspicious_behavior(&self, peer_id: &PeerId, behavior_type: SuspiciousBehavior) {
        if let Some(mut peer_info) = self.peer_info.get_mut(peer_id) {
            match behavior_type {
                SuspiciousBehavior::AddressFlooding => {
                    peer_info.behavior_flags.address_flooding = true;
                    peer_info.connection_score -= 5.0;
                },
                SuspiciousBehavior::ConflictingHeaders => {
                    peer_info.behavior_flags.conflicting_headers = true;
                    peer_info.connection_score -= 10.0;
                },
                SuspiciousBehavior::RoutingPoisoning => {
                    peer_info.behavior_flags.routing_poisoning = true;
                    peer_info.connection_score -= 20.0;
                    
                    // Mark subnet as suspicious
                    self.suspicious_subnets
                        .entry(peer_info.subnet.clone())
                        .and_modify(|count| *count += 1)
                        .or_insert(1);
                },
                SuspiciousBehavior::SuspiciousConnectionPattern => {
                    peer_info.behavior_flags.suspicious_connection_pattern = true;
                    peer_info.connection_score -= 5.0;
                },
                SuspiciousBehavior::AggressiveAdvertising => {
                    peer_info.behavior_flags.aggressive_advertising = true;
                    peer_info.connection_score -= 3.0;
                },
            }
            
            debug!("Flagged suspicious behavior {:?} for peer {:?}, new score: {}", 
                  behavior_type, peer_id, peer_info.connection_score);
        }
    }

    /// Check if a connection attempt is allowed under rate limiting
    fn check_connection_rate(&self, subnet: &IpSubnet) -> bool {
        let now = Instant::now();
        let one_minute_ago = now - Duration::from_secs(60);
        
        let mut attempts = self.connection_attempts.write().unwrap();
        
        // Clean up old attempts
        for (_, timestamps) in attempts.iter_mut() {
            timestamps.retain(|&timestamp| timestamp >= one_minute_ago);
        }
        
        // Check and update rate for this subnet
        let subnet_attempts = attempts.entry(subnet.clone()).or_insert_with(Vec::new);
        
        if subnet_attempts.len() >= self.max_connection_rate {
            return false;
        }
        
        subnet_attempts.push(now);
        true
    }
    
    /// Check if adding a connection would exceed subnet limits
    fn would_exceed_subnet_limits(&self, subnet: &IpSubnet) -> bool {
        // Check subnet connection limit
        if let Some(count) = self.subnet_distribution.get(subnet) {
            if *count >= self.eclipse_config.max_connections_per_subnet {
                return true;
            }
        }
        
        false
    }
    
    /// Check if we need to perform peer rotation
    pub fn check_rotation_needed(&self) -> bool {
        // If rotation is disabled, never rotate
        if !self.eclipse_config.enable_automatic_rotation {
            return false;
        }
        
        // Check if enough time has passed since last rotation
        if self.last_rotation.elapsed() >= self.eclipse_config.rotation_interval {
            return true;
        }
        
        // Also check for signs of an eclipse attack
        // 1. Sudden influx of connections from the same subnet
        for entry in self.subnet_distribution.iter() {
            if *entry.value() > self.eclipse_config.max_connections_per_subnet * 2 {
                warn!("Possible eclipse attack: Subnet {:?} has too many connections", entry.key());
                return true;
            }
        }
        
        // 2. Too many inbound connections relative to outbound
        let inbound_count = self.inbound_connections.iter().filter(|e| *e.value()).count();
        let outbound_count = self.inbound_connections.iter().filter(|e| !*e.value()).count();
        
        if outbound_count > 0 && inbound_count as f64 / outbound_count as f64 > self.eclipse_config.max_inbound_ratio {
            warn!("Possible eclipse attack: Too many inbound connections");
            return true;
        }
        
        // 3. Suspicious behavior detected
        if !self.suspicious_subnets.is_empty() {
            for entry in self.suspicious_subnets.iter() {
                if *entry.value() >= 2 {
                    warn!("Possible eclipse attack: Multiple suspicious behaviors from subnet {:?}", entry.key());
                    return true;
                }
            }
        }
        
        false
    }
    
    /// Create a rotation plan for peers
    /// Returns (peers_to_disconnect, number_to_disconnect)
    pub fn create_rotation_plan(&self) -> (Vec<PeerId>, usize) {
        let mut peers_to_disconnect = Vec::new();
        let mut disconnection_count = 0;
        
        // Determine how many peers to rotate (20-30% of connected peers)
        let connected_peers: Vec<_> = self.peer_info.iter()
            .filter(|entry| !self.protected_peers.contains(entry.key()))
            .map(|entry| (entry.key().clone(), entry.value().clone()))
            .collect();
            
        if connected_peers.is_empty() {
            return (Vec::new(), 0);
        }
        
        // Aim to rotate 20-30% of peers
        let target_rotation_count = (connected_peers.len() as f64 * 0.2).ceil() as usize;
        
        // First, identify overrepresented subnets
        let mut subnet_counts: HashMap<IpSubnet, Vec<(PeerId, f64)>> = HashMap::new();
        
        for (peer_id, info) in connected_peers.iter() {
            subnet_counts
                .entry(info.subnet.clone())
                .or_insert_with(Vec::new)
                .push((peer_id.clone(), info.connection_score));
        }
        
        // Sort subnets by count, descending
        let mut subnet_list: Vec<_> = subnet_counts
            .iter()
            .map(|(subnet, peers)| (subnet.clone(), peers.len()))
            .collect();
            
        subnet_list.sort_by(|a, b| b.1.cmp(&a.1));
        
        // First disconnect from overrepresented subnets
        for (subnet, count) in subnet_list {
            if count <= self.eclipse_config.max_connections_per_subnet {
                continue;
            }
            
            // Get peers from this subnet
            let mut subnet_peers = subnet_counts[&subnet].clone();
            
            // Sort by score, disconnect lowest scores first
            subnet_peers.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
            
            // Disconnect excess peers
            let excess_count = count - self.eclipse_config.max_connections_per_subnet;
            for i in 0..excess_count {
                if i < subnet_peers.len() {
                    let peer_id = subnet_peers[i].0.clone();
                    peers_to_disconnect.push(peer_id);
                    disconnection_count += 1;
                }
            }
        }
        
        // If we haven't reached target, disconnect additional peers based on score
        if disconnection_count < target_rotation_count {
            // Get remaining peers (exclude already selected)
            let mut remaining_peers: Vec<_> = connected_peers
                .iter()
                .filter(|(peer_id, _)| !peers_to_disconnect.contains(peer_id))
                .map(|(peer_id, info)| (peer_id.clone(), info.connection_score))
                .collect();
                
            // Sort by score, lowest first
            remaining_peers.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
            
            // Take additional peers up to target
            let additional_needed = target_rotation_count - disconnection_count;
            for i in 0..additional_needed {
                if i < remaining_peers.len() {
                    peers_to_disconnect.push(remaining_peers[i].0.clone());
                    disconnection_count += 1;
                }
            }
        }
        
        (peers_to_disconnect, disconnection_count)
    }
    
    /// Perform forced peer rotation and return peers to disconnect
    pub fn perform_forced_rotation(&mut self) -> Vec<PeerId> {
        let (peers_to_disconnect, _count) = self.create_rotation_plan();
        
        // Update last rotation time
        self.last_rotation = Instant::now();
        
        // Log rotation
        if !peers_to_disconnect.is_empty() {
            info!("Performing peer rotation for eclipse prevention, disconnecting {} peers",
                 peers_to_disconnect.len());
        }
        
        peers_to_disconnect
    }

    /// Evaluate the current network diversity score
    pub fn evaluate_diversity(&self) -> f64 {
        // Calculate Shannon entropy across different distribution metrics
        let subnet_entropy = self.calculate_entropy(&self.subnet_distribution);
        let asn_entropy = self.calculate_entropy(&self.asn_distribution);
        let geo_entropy = self.calculate_entropy(&self.geographic_distribution);
        
        // Weight the different entropy scores
        // Higher entropy = more diverse = better
        let weighted_score = 
            subnet_entropy * 0.5 + 
            asn_entropy * 0.3 + 
            geo_entropy * 0.2;
            
        debug!("Diversity score: {:.4} (subnet: {:.4}, ASN: {:.4}, geo: {:.4})", 
            weighted_score, subnet_entropy, asn_entropy, geo_entropy);
            
        weighted_score
    }
    
    /// Calculate entropy of a distribution (Shannon entropy)
    fn calculate_entropy<K: std::hash::Hash + Eq, V: Into<f64> + Copy>(&self, 
                                                      distribution: &DashMap<K, V>) -> f64 {
        if distribution.is_empty() {
            return 0.0;
        }
        
        let total: f64 = distribution.iter().map(|entry| (*entry.value()).into()).sum();
        if total == 0.0 {
            return 0.0;
        }
        
        let mut entropy = 0.0;
        for entry in distribution.iter() {
            let p: f64 = (*entry.value()).into() / total;
            if p > 0.0 {
                entropy -= p * p.log2();
            }
        }
        
        // Normalize to 0-1 range
        let max_entropy = (distribution.len() as f64).log2();
        if max_entropy > 0.0 {
            entropy / max_entropy
        } else {
            0.0
        }
    }
    
    /// Get a list of underrepresented subnets for improved diversity
    pub fn get_underrepresented_subnets(&self) -> Vec<IpSubnet> {
        let mut subnet_counts: Vec<_> = self.subnet_distribution.iter()
            .map(|entry| (entry.key().clone(), *entry.value()))
            .collect();
            
        subnet_counts.sort_by_key(|&(_, count)| count);
        
        // Return the least represented subnets
        subnet_counts.iter()
            .take(5) // Take top 5 underrepresented subnets
            .map(|(subnet, _)| subnet.clone())
            .collect()
    }
    
    /// Suggest peers to connect to for optimal diversity
    pub fn suggest_connection_targets(&self, count: usize) -> Vec<PeerAddress> {
        // Identify underrepresented subnets, ASNs, regions
        let underrep_subnets = self.get_underrepresented_subnets();
        
        // In a real implementation, we would:
        // 1. Look through known peers (from peer database)
        // 2. Find peers from underrepresented subnets/ASNs/regions
        // 3. Prioritize those with good reputation
        // 4. Return a diverse set of targets
        
        // For now, return a placeholder
        Vec::new()
    }
    
    /// Get peers from most overrepresented subnets
    pub fn get_overrepresented_peers(&self, count: usize) -> Vec<PeerId> {
        // Find subnets with too many connections
        let mut subnet_counts: Vec<_> = self.subnet_distribution.iter()
            .map(|entry| (entry.key().clone(), *entry.value()))
            .collect();
            
        // Sort descending by count
        subnet_counts.sort_by(|a, b| b.1.cmp(&a.1));
        
        let mut result = Vec::new();
        
        // For each overrepresented subnet
        for (subnet, subnet_count) in subnet_counts {
            if subnet_count <= self.eclipse_config.max_connections_per_subnet {
                continue;
            }
            
            // Find peers in this subnet sorted by score
            let mut subnet_peers: Vec<_> = self.peer_info.iter()
                .filter(|entry| entry.value().subnet == subnet && !self.protected_peers.contains(entry.key()))
                .map(|entry| (entry.key().clone(), entry.value().connection_score))
                .collect();
                
            // Sort by score (lowest first)
            subnet_peers.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
            
            // Add the worst-scoring peers from this subnet
            let to_take = subnet_count - self.eclipse_config.max_connections_per_subnet;
            for (peer_id, _) in subnet_peers.iter().take(to_take) {
                result.push(peer_id.clone());
                if result.len() >= count {
                    return result;
                }
            }
        }
        
        result
    }

    /// Extract IP address from a multiaddr
    fn extract_ip_from_multiaddr(addr: &libp2p::Multiaddr) -> Option<IpAddr> {
        for protocol in addr.iter() {
            match protocol {
                Protocol::Ip4(ip) => return Some(IpAddr::V4(ip)),
                Protocol::Ip6(ip) => return Some(IpAddr::V6(ip)),
                _ => continue,
            }
        }
        None
    }
    
    /// Get region for an IP address
    /// In production, would use a GeoIP database
    fn get_region_for_ip(&self, _ip: &IpAddr) -> Option<String> {
        // Placeholder - would use GeoIP lookup in production
        None
    }
    
    /// Get ASN for an IP address
    /// In production, would use an ASN database
    fn get_asn_for_ip(&self, _ip: &IpAddr) -> Option<u32> {
        // Placeholder - would use ASN lookup in production
        None
    }
}

/// Simplified peer address for recommendation
#[derive(Debug, Clone)]
pub struct PeerAddress {
    pub peer_id: Option<PeerId>,
    pub addr: libp2p::Multiaddr,
}

/// Types of suspicious behaviors to monitor
#[derive(Debug, Clone, Copy)]
pub enum SuspiciousBehavior {
    /// Flooding with address messages
    AddressFlooding,
    /// Sending conflicting block headers
    ConflictingHeaders,
    /// Attempting to poison routing tables
    RoutingPoisoning,
    /// Suspicious connection patterns
    SuspiciousConnectionPattern,
    /// Aggressive peer advertising
    AggressiveAdvertising,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::Ipv4Addr;
    use libp2p::multiaddr::Multiaddr;
    
    #[test]
    fn test_subnet_containment() {
        let subnet = IpSubnet::new(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 0)), 24);
        
        // These should be in the subnet
        assert!(subnet.contains(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1))));
        assert!(subnet.contains(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 254))));
        
        // These should not be in the subnet
        assert!(!subnet.contains(IpAddr::V4(Ipv4Addr::new(192, 168, 2, 1))));
        assert!(!subnet.contains(IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1))));
    }
    
    #[test]
    fn test_connection_rate_limiting() {
        let manager = PeerDiversityManager::with_config(0.6, ConnectionStrategy::BalancedDiversity, 2);
        let subnet = IpSubnet::new(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 0)), 24);
        
        // First two connections should be allowed
        assert!(manager.check_connection_rate(&subnet));
        assert!(manager.check_connection_rate(&subnet));
        
        // Third connection should be rate limited
        assert!(!manager.check_connection_rate(&subnet));
        
        // Different subnet should be allowed
        let other_subnet = IpSubnet::new(IpAddr::V4(Ipv4Addr::new(10, 0, 0, 0)), 24);
        assert!(manager.check_connection_rate(&other_subnet));
    }
    
    #[test]
    fn test_extract_ip_from_multiaddr() {
        let addr: Multiaddr = "/ip4/192.168.1.1/tcp/8000".parse().unwrap();
        assert_eq!(
            PeerDiversityManager::extract_ip_from_multiaddr(&addr),
            Some(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1)))
        );
        
        let addr_v6: Multiaddr = "/ip6/::1/tcp/8000".parse().unwrap();
        assert_eq!(
            PeerDiversityManager::extract_ip_from_multiaddr(&addr_v6),
            Some(IpAddr::V6(Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 1)))
        );
        
        let no_ip: Multiaddr = "/tcp/8000".parse().unwrap();
        assert_eq!(
            PeerDiversityManager::extract_ip_from_multiaddr(&no_ip),
            None
        );
    }
} 