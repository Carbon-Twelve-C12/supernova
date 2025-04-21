use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

use dashmap::DashMap;
use libp2p::core::PeerId;
use libp2p::multiaddr::Protocol;
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
#[derive(Debug, Clone, Copy)]
pub enum ConnectionStrategy {
    /// Maximize diversity across all dimensions
    MaximizeDiversity,
    /// Maintain minimum diversity while prioritizing performance
    BalancedDiversity,
    /// Prioritize specific geographic regions
    GeographicFocus(Vec<String>),
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
}

/// Network information about a peer
#[derive(Debug, Clone)]
pub struct PeerNetworkInfo {
    /// IP address of the peer
    ip: IpAddr,
    /// Subnet the peer belongs to
    subnet: IpSubnet,
    /// Autonomous System Number (if known)
    asn: Option<u32>,
    /// Geographic region (if known)
    region: Option<String>,
    /// Time when peer was first seen
    first_seen: Instant,
    /// Time of most recent connection
    last_connection: Instant,
    /// Connection score based on behavior
    connection_score: f64,
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
        }
    }

    /// Register a new peer with the diversity manager
    pub fn register_peer(&self, peer_id: PeerId, addr: &libp2p::Multiaddr) -> bool {
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
        
        true
    }

    /// Update a peer's score based on behavior
    pub fn update_peer_score(&self, peer_id: &PeerId, score_delta: f64) {
        if let Some(mut peer_info) = self.peer_info.get_mut(peer_id) {
            peer_info.connection_score += score_delta;
            debug!("Updated score for peer {:?} to {}", peer_id, peer_info.connection_score);
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
    
    /// Suggest a peer to disconnect to improve diversity
    pub fn suggest_disconnection(&self) -> Option<PeerId> {
        // Find the most over-represented subnet/ASN
        if self.subnet_distribution.is_empty() {
            return None;
        }
        
        // Find subnet with most connections
        let max_subnet = self.subnet_distribution
            .iter()
            .max_by_key(|entry| *entry.value())?;
            
        if *max_subnet.value() <= 1 {
            return None; // No overrepresentation
        }
        
        // Find a peer in this subnet with the lowest score
        let candidate_peers: Vec<_> = self.peer_info.iter()
            .filter(|entry| entry.value().subnet == *max_subnet.key())
            .collect();
            
        candidate_peers.into_iter()
            .min_by(|a, b| a.value().connection_score.partial_cmp(&b.value().connection_score).unwrap())
            .map(|entry| *entry.key())
    }
    
    /// Recommend connection targets to improve diversity
    pub fn recommend_connection_targets(&self) -> Vec<PeerAddress> {
        // In a real implementation, this would use a peer database
        // For now, we'll create a placeholder implementation
        
        // This would identify under-represented regions/subnets
        // and suggest peers from those areas
        
        // Placeholder
        Vec::new()
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