use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};
use thiserror::Error;

/// Errors related to security mitigation systems
#[derive(Debug, Error)]
pub enum SecurityError {
    #[error("Connection rate limit exceeded for {0}")]
    RateLimitExceeded(IpAddr),
    
    #[error("Peer banned: {0}")]
    PeerBanned(String),
    
    #[error("Diversity requirements not met: {0}")]
    DiversityRequirementsFailed(String),
    
    #[error("Identity verification failed: {0}")]
    IdentityVerificationFailed(String),
    
    #[error("Internal error: {0}")]
    InternalError(String),
}

/// IP subnet representation for diversity tracking
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct IpSubnet {
    prefix: Vec<u8>,
    mask: u8,
}

impl IpSubnet {
    /// Create a new subnet from an IP address and mask
    pub fn new(addr: IpAddr, mask: u8) -> Self {
        let prefix = match addr {
            IpAddr::V4(ipv4) => ipv4.octets().to_vec(),
            IpAddr::V6(ipv6) => ipv6.octets().to_vec(),
        };
        
        Self { prefix, mask }
    }
    
    /// Check if an IP address belongs to this subnet
    pub fn contains(&self, addr: IpAddr) -> bool {
        let addr_bytes = match addr {
            IpAddr::V4(ipv4) => ipv4.octets().to_vec(),
            IpAddr::V6(ipv6) => ipv6.octets().to_vec(),
        };
        
        if addr_bytes.len() != self.prefix.len() {
            return false;
        }
        
        let byte_mask = self.mask / 8;
        let remainder_bits = self.mask % 8;
        
        // Check full bytes
        for i in 0..byte_mask as usize {
            if i >= self.prefix.len() || i >= addr_bytes.len() {
                break;
            }
            
            if self.prefix[i] != addr_bytes[i] {
                return false;
            }
        }
        
        // Check remaining bits
        if remainder_bits > 0 && byte_mask as usize < self.prefix.len() {
            let mask = 0xFF_u8 << (8 - remainder_bits);
            if (self.prefix[byte_mask as usize] & mask) != (addr_bytes[byte_mask as usize] & mask) {
                return false;
            }
        }
        
        true
    }
}

/// Connection strategy for peer diversity
#[derive(Debug, Clone)]
pub enum ConnectionStrategy {
    /// Balance connections across subnets
    BalanceAcrossSubnets,
    /// Prioritize geographic diversity
    GeographicDiversity,
    /// Prioritize ASN diversity
    AsnDiversity,
    /// Custom balance based on provided weights
    Custom(HashMap<String, f64>),
}

/// Autonomous System Number information
#[derive(Debug, Clone)]
pub struct AsnInfo {
    pub asn: u32,
    pub organization: String,
    pub country: String,
}

/// Peer identity and metadata
#[derive(Debug, Clone)]
pub struct PeerIdentity {
    pub peer_id: String,
    pub ip_addr: IpAddr,
    pub asn: Option<AsnInfo>,
    pub geographic_region: Option<String>,
    pub last_verified: Instant,
    pub verification_status: VerificationStatus,
}

/// Verification status for peers
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VerificationStatus {
    Unverified,
    Pending,
    Verified,
    Failed,
}

/// Manager for peer diversity across the network
pub struct PeerDiversityManager {
    subnet_distribution: HashMap<IpSubnet, usize>,
    asn_distribution: HashMap<u32, usize>,
    geographic_distribution: HashMap<String, usize>,
    min_diversity_score: f64,
    connection_strategy: ConnectionStrategy,
    peers: HashMap<String, PeerIdentity>,
}

impl PeerDiversityManager {
    /// Create a new peer diversity manager
    pub fn new(min_diversity_score: f64, strategy: ConnectionStrategy) -> Self {
        Self {
            subnet_distribution: HashMap::new(),
            asn_distribution: HashMap::new(),
            geographic_distribution: HashMap::new(),
            min_diversity_score,
            connection_strategy: strategy,
            peers: HashMap::new(),
        }
    }
    
    /// Register a new peer with the diversity manager
    pub fn register_peer(&mut self, peer_id: String, ip_addr: IpAddr) -> Result<(), SecurityError> {
        // Create subnet from IP
        let subnet = IpSubnet::new(ip_addr, 24); // Use /24 subnet for IPv4
        
        // Update subnet distribution
        *self.subnet_distribution.entry(subnet).or_insert(0) += 1;
        
        // Create basic peer identity
        let peer_identity = PeerIdentity {
            peer_id: peer_id.clone(),
            ip_addr,
            asn: None,
            geographic_region: None,
            last_verified: Instant::now(),
            verification_status: VerificationStatus::Unverified,
        };
        
        self.peers.insert(peer_id, peer_identity);
        
        // Check if we need to disconnect some peers to improve diversity
        let diversity_score = self.evaluate_diversity();
        if diversity_score < self.min_diversity_score {
            if let Some(peer_to_disconnect) = self.suggest_disconnection() {
                return Err(SecurityError::DiversityRequirementsFailed(
                    format!("Need to disconnect peer {} to improve diversity", peer_to_disconnect)
                ));
            }
        }
        
        Ok(())
    }
    
    /// Update peer with additional information
    pub fn update_peer_info(
        &mut self,
        peer_id: &str,
        asn: Option<AsnInfo>,
        geographic_region: Option<String>,
    ) -> Result<(), SecurityError> {
        let peer = self.peers.get_mut(peer_id).ok_or_else(|| {
            SecurityError::InternalError(format!("Peer {} not found", peer_id))
        })?;
        
        // Update ASN distribution if provided
        if let Some(asn_info) = asn.as_ref() {
            // Remove from old ASN count if exists
            if let Some(old_asn) = peer.asn.as_ref() {
                if let Some(count) = self.asn_distribution.get_mut(&old_asn.asn) {
                    *count = count.saturating_sub(1);
                }
            }
            
            // Add to new ASN count
            *self.asn_distribution.entry(asn_info.asn).or_insert(0) += 1;
            
            // Update peer
            peer.asn = asn;
        }
        
        // Update geographic distribution if provided
        if let Some(region) = geographic_region.as_ref() {
            // Remove from old region count if exists
            if let Some(old_region) = peer.geographic_region.as_ref() {
                if let Some(count) = self.geographic_distribution.get_mut(old_region) {
                    *count = count.saturating_sub(1);
                }
            }
            
            // Add to new region count
            *self.geographic_distribution.entry(region.clone()).or_insert(0) += 1;
            
            // Update peer
            peer.geographic_region = geographic_region;
        }
        
        Ok(())
    }
    
    /// Evaluate network diversity score
    pub fn evaluate_diversity(&self) -> f64 {
        // Calculate entropy across subnets
        let subnet_entropy = self.calculate_entropy(&self.subnet_distribution);
        
        // Calculate entropy across ASNs
        let asn_entropy = self.calculate_entropy(&self.asn_distribution);
        
        // Calculate entropy across geographic regions
        let geo_entropy = self.calculate_entropy(&self.geographic_distribution);
        
        // Weight the different entropy values based on connection strategy
        match &self.connection_strategy {
            ConnectionStrategy::BalanceAcrossSubnets => subnet_entropy,
            ConnectionStrategy::GeographicDiversity => {
                subnet_entropy * 0.3 + asn_entropy * 0.2 + geo_entropy * 0.5
            }
            ConnectionStrategy::AsnDiversity => {
                subnet_entropy * 0.3 + asn_entropy * 0.5 + geo_entropy * 0.2
            }
            ConnectionStrategy::Custom(weights) => {
                let subnet_weight = weights.get("subnet").cloned().unwrap_or(0.33);
                let asn_weight = weights.get("asn").cloned().unwrap_or(0.33);
                let geo_weight = weights.get("geo").cloned().unwrap_or(0.34);
                
                subnet_entropy * subnet_weight + asn_entropy * asn_weight + geo_entropy * geo_weight
            }
        }
    }
    
    /// Calculate entropy of a distribution
    fn calculate_entropy<K>(&self, distribution: &HashMap<K, usize>) -> f64 {
        if distribution.is_empty() {
            return 0.0;
        }
        
        let total: usize = distribution.values().sum();
        if total == 0 {
            return 0.0;
        }
        
        let total_f64 = total as f64;
        
        // Calculate entropy: -sum(p_i * log(p_i))
        distribution.values().fold(0.0, |entropy, &count| {
            let p = count as f64 / total_f64;
            if p > 0.0 {
                entropy - p * p.log2()
            } else {
                entropy
            }
        })
    }
    
    /// Suggest peer to disconnect to improve diversity
    pub fn suggest_disconnection(&self) -> Option<String> {
        // Find most over-represented subnet/ASN and suggest peer to remove
        match &self.connection_strategy {
            ConnectionStrategy::BalanceAcrossSubnets => {
                self.find_most_represented_peer_by_subnet()
            }
            ConnectionStrategy::GeographicDiversity => {
                self.find_most_represented_peer_by_geo().or_else(|| 
                    self.find_most_represented_peer_by_subnet()
                )
            }
            ConnectionStrategy::AsnDiversity => {
                self.find_most_represented_peer_by_asn().or_else(||
                    self.find_most_represented_peer_by_subnet()
                )
            }
            ConnectionStrategy::Custom(_) => {
                // Try all methods in sequence
                self.find_most_represented_peer_by_geo()
                    .or_else(|| self.find_most_represented_peer_by_asn())
                    .or_else(|| self.find_most_represented_peer_by_subnet())
            }
        }
    }
    
    /// Find the most over-represented peer by subnet
    fn find_most_represented_peer_by_subnet(&self) -> Option<String> {
        let mut max_count = 0;
        let mut max_subnet = None;
        
        for (subnet, count) in &self.subnet_distribution {
            if *count > max_count {
                max_count = *count;
                max_subnet = Some(subnet);
            }
        }
        
        // Find a peer in this subnet
        if let Some(subnet) = max_subnet {
            for (peer_id, identity) in &self.peers {
                if subnet.contains(identity.ip_addr) {
                    return Some(peer_id.clone());
                }
            }
        }
        
        None
    }
    
    /// Find the most over-represented peer by ASN
    fn find_most_represented_peer_by_asn(&self) -> Option<String> {
        let mut max_count = 0;
        let mut max_asn = None;
        
        for (asn, count) in &self.asn_distribution {
            if *count > max_count {
                max_count = *count;
                max_asn = Some(*asn);
            }
        }
        
        // Find a peer with this ASN
        if let Some(asn) = max_asn {
            for (peer_id, identity) in &self.peers {
                if let Some(asn_info) = &identity.asn {
                    if asn_info.asn == asn {
                        return Some(peer_id.clone());
                    }
                }
            }
        }
        
        None
    }
    
    /// Find the most over-represented peer by geographic region
    fn find_most_represented_peer_by_geo(&self) -> Option<String> {
        let mut max_count = 0;
        let mut max_region = None;
        
        for (region, count) in &self.geographic_distribution {
            if *count > max_count {
                max_count = *count;
                max_region = Some(region);
            }
        }
        
        // Find a peer in this region
        if let Some(region) = max_region {
            for (peer_id, identity) in &self.peers {
                if let Some(peer_region) = &identity.geographic_region {
                    if peer_region == region {
                        return Some(peer_id.clone());
                    }
                }
            }
        }
        
        None
    }
    
    /// Recommend connection targets to improve diversity
    pub fn recommend_connection_targets(&self) -> Vec<String> {
        // Identify under-represented network segments and recommend targets
        let mut recommendations = Vec::new();
        
        // Implementation depends on the specific peer recommendation system
        // This is a simplified version that would need to be expanded with actual
        // peer discovery mechanisms
        
        recommendations
    }
    
    /// Remove a peer from the diversity manager
    pub fn remove_peer(&mut self, peer_id: &str) -> Result<(), SecurityError> {
        let peer = self.peers.remove(peer_id).ok_or_else(|| {
            SecurityError::InternalError(format!("Peer {} not found", peer_id))
        })?;
        
        // Update subnet distribution
        let subnet = IpSubnet::new(peer.ip_addr, 24);
        if let Some(count) = self.subnet_distribution.get_mut(&subnet) {
            *count = count.saturating_sub(1);
            if *count == 0 {
                self.subnet_distribution.remove(&subnet);
            }
        }
        
        // Update ASN distribution
        if let Some(asn_info) = &peer.asn {
            if let Some(count) = self.asn_distribution.get_mut(&asn_info.asn) {
                *count = count.saturating_sub(1);
                if *count == 0 {
                    self.asn_distribution.remove(&asn_info.asn);
                }
            }
        }
        
        // Update geographic distribution
        if let Some(region) = &peer.geographic_region {
            if let Some(count) = self.geographic_distribution.get_mut(region) {
                *count = count.saturating_sub(1);
                if *count == 0 {
                    self.geographic_distribution.remove(region);
                }
            }
        }
        
        Ok(())
    }
}

/// Rate limiting for connections
pub struct ConnectionRateLimiter {
    limits: HashMap<IpSubnet, usize>,
    connections: HashMap<IpAddr, Vec<Instant>>,
    window_size: Duration,
}

impl ConnectionRateLimiter {
    /// Create a new connection rate limiter
    pub fn new(window_size: Duration) -> Self {
        Self {
            limits: HashMap::new(),
            connections: HashMap::new(),
            window_size,
        }
    }
    
    /// Set rate limit for a subnet
    pub fn set_limit(&mut self, subnet: IpSubnet, max_connections: usize) {
        self.limits.insert(subnet, max_connections);
    }
    
    /// Check if a connection is allowed from an IP address
    pub fn check_connection(&mut self, ip: IpAddr) -> Result<(), SecurityError> {
        // Get current time
        let now = Instant::now();
        
        // Update connection history
        let connection_times = self.connections.entry(ip).or_insert_with(Vec::new);
        
        // Remove expired connection timestamps
        connection_times.retain(|time| now.duration_since(*time) < self.window_size);
        
        // Find applicable subnet limits
        let mut applicable_limit = None;
        
        for (subnet, limit) in &self.limits {
            if subnet.contains(ip) {
                applicable_limit = Some(*limit);
                break;
            }
        }
        
        // Check if we're exceeding the limit
        if let Some(limit) = applicable_limit {
            if connection_times.len() >= limit {
                return Err(SecurityError::RateLimitExceeded(ip));
            }
        }
        
        // Record this connection attempt
        connection_times.push(now);
        
        Ok(())
    }
}

/// Eclipse attack prevention through connection management
pub struct EclipsePreventionManager {
    forced_rotation_interval: Duration,
    min_outbound_connections: usize,
    last_rotation: Instant,
    outbound_connections: HashMap<String, (IpAddr, Instant)>,
}

impl EclipsePreventionManager {
    /// Create a new eclipse prevention manager
    pub fn new(forced_rotation_interval: Duration, min_outbound_connections: usize) -> Self {
        Self {
            forced_rotation_interval,
            min_outbound_connections,
            last_rotation: Instant::now(),
            outbound_connections: HashMap::new(),
        }
    }
    
    /// Register an outbound connection
    pub fn register_outbound_connection(&mut self, peer_id: String, ip: IpAddr) {
        self.outbound_connections.insert(peer_id, (ip, Instant::now()));
    }
    
    /// Check if connections need rotation
    pub fn check_rotation_needed(&self) -> bool {
        // Check if we've reached the forced rotation interval
        let now = Instant::now();
        now.duration_since(self.last_rotation) >= self.forced_rotation_interval
            || self.outbound_connections.len() < self.min_outbound_connections
    }
    
    /// Get peers that should be rotated out
    pub fn get_rotation_candidates(&self) -> Vec<String> {
        let now = Instant::now();
        let mut candidates = Vec::new();
        
        // Select the oldest connections for rotation
        let mut peers: Vec<_> = self.outbound_connections.iter().collect();
        
        // Sort by connection age (oldest first)
        peers.sort_by(|a, b| a.1.1.cmp(&b.1.1));
        
        // Take 25% of the oldest connections or at least 1
        let rotation_count = (self.outbound_connections.len() / 4).max(1);
        
        for (peer_id, _) in peers.iter().take(rotation_count) {
            candidates.push((*peer_id).clone());
        }
        
        candidates
    }
    
    /// Update last rotation time
    pub fn update_rotation_time(&mut self) {
        self.last_rotation = Instant::now();
    }
    
    /// Remove outbound connection
    pub fn remove_connection(&mut self, peer_id: &str) {
        self.outbound_connections.remove(peer_id);
    }
}

/// Long-range attack protection system
pub struct LongRangeAttackProtection {
    checkpoints: HashMap<u64, [u8; 32]>, // Height -> Block hash
    checkpoint_signers: Vec<[u8; 32]>,   // Public keys of trusted checkpoint signers
    signature_threshold: usize,          // Minimum signatures required to accept a checkpoint
}

impl LongRangeAttackProtection {
    /// Create a new long-range attack protection system
    pub fn new(signature_threshold: usize) -> Self {
        Self {
            checkpoints: HashMap::new(),
            checkpoint_signers: Vec::new(),
            signature_threshold,
        }
    }
    
    /// Add a trusted checkpoint signer
    pub fn add_checkpoint_signer(&mut self, public_key: [u8; 32]) {
        self.checkpoint_signers.push(public_key);
    }
    
    /// Add a checkpoint with height and block hash
    pub fn add_checkpoint(&mut self, height: u64, block_hash: [u8; 32]) {
        self.checkpoints.insert(height, block_hash);
    }
    
    /// Verify a block against checkpoints
    pub fn verify_block(&self, height: u64, block_hash: [u8; 32]) -> bool {
        if let Some(checkpoint_hash) = self.checkpoints.get(&height) {
            return *checkpoint_hash == block_hash;
        }
        
        // If no checkpoint exists for this height, it passes
        true
    }
    
    /// Verify a social consensus checkpoint
    pub fn verify_social_checkpoint(
        &self,
        height: u64,
        block_hash: [u8; 32],
        signatures: &[([u8; 32], [u8; 64])], // Vec of (public_key, signature) pairs
    ) -> bool {
        let mut valid_signatures = 0;
        
        for (public_key, signature) in signatures {
            // Check if this is a trusted signer
            if !self.checkpoint_signers.contains(public_key) {
                continue;
            }
            
            // Verify signature
            // This is a placeholder - actual implementation would use proper signature verification
            let is_valid = true; // verify_signature(public_key, &checkpoint_data, signature);
            
            if is_valid {
                valid_signatures += 1;
            }
        }
        
        valid_signatures >= self.signature_threshold
    }
}

/// Comprehensive security manager to coordinate all security features
pub struct SecurityManager {
    diversity_manager: Arc<RwLock<PeerDiversityManager>>,
    rate_limiter: Arc<RwLock<ConnectionRateLimiter>>,
    eclipse_prevention: Arc<RwLock<EclipsePreventionManager>>,
    long_range_protection: Arc<RwLock<LongRangeAttackProtection>>,
    // Additional attack mitigation systems can be added here
}

impl SecurityManager {
    /// Create a new security manager
    pub fn new(
        min_diversity_score: f64,
        connection_strategy: ConnectionStrategy,
        rate_limit_window: Duration,
        rotation_interval: Duration,
        min_outbound_connections: usize,
        checkpoint_signature_threshold: usize,
    ) -> Self {
        let diversity_manager = PeerDiversityManager::new(min_diversity_score, connection_strategy);
        let rate_limiter = ConnectionRateLimiter::new(rate_limit_window);
        let eclipse_prevention = EclipsePreventionManager::new(rotation_interval, min_outbound_connections);
        let long_range_protection = LongRangeAttackProtection::new(checkpoint_signature_threshold);
        
        Self {
            diversity_manager: Arc::new(RwLock::new(diversity_manager)),
            rate_limiter: Arc::new(RwLock::new(rate_limiter)),
            eclipse_prevention: Arc::new(RwLock::new(eclipse_prevention)),
            long_range_protection: Arc::new(RwLock::new(long_range_protection)),
        }
    }
    
    /// Register a new peer connection
    pub fn register_peer_connection(&self, peer_id: String, ip_addr: IpAddr) -> Result<(), SecurityError> {
        // Check rate limiting
        {
            let mut rate_limiter = self.rate_limiter.write().unwrap();
            rate_limiter.check_connection(ip_addr)?;
        }
        
        // Register with diversity manager
        {
            let mut diversity_manager = self.diversity_manager.write().unwrap();
            diversity_manager.register_peer(peer_id, ip_addr)?;
        }
        
        Ok(())
    }
    
    /// Register an outbound connection
    pub fn register_outbound_connection(&self, peer_id: String, ip_addr: IpAddr) -> Result<(), SecurityError> {
        // Register the connection with eclipse prevention
        {
            let mut eclipse_prevention = self.eclipse_prevention.write().unwrap();
            eclipse_prevention.register_outbound_connection(peer_id.clone(), ip_addr);
        }
        
        // Also register with diversity manager
        {
            let mut diversity_manager = self.diversity_manager.write().unwrap();
            diversity_manager.register_peer(peer_id, ip_addr)?;
        }
        
        Ok(())
    }
    
    /// Check if outbound connections need rotation
    pub fn check_outbound_rotation(&self) -> Vec<String> {
        let eclipse_prevention = self.eclipse_prevention.read().unwrap();
        
        if eclipse_prevention.check_rotation_needed() {
            return eclipse_prevention.get_rotation_candidates();
        }
        
        Vec::new()
    }
    
    /// Remove a peer connection
    pub fn remove_peer_connection(&self, peer_id: &str) -> Result<(), SecurityError> {
        // Remove from diversity manager
        {
            let mut diversity_manager = self.diversity_manager.write().unwrap();
            diversity_manager.remove_peer(peer_id)?;
        }
        
        // Remove from eclipse prevention if it's an outbound connection
        {
            let mut eclipse_prevention = self.eclipse_prevention.write().unwrap();
            eclipse_prevention.remove_connection(peer_id);
        }
        
        Ok(())
    }
    
    /// Verify a block against known checkpoints
    pub fn verify_block_against_checkpoints(&self, height: u64, block_hash: [u8; 32]) -> bool {
        let long_range_protection = self.long_range_protection.read().unwrap();
        long_range_protection.verify_block(height, block_hash)
    }
    
    /// Add a checkpoint for long-range attack protection
    pub fn add_checkpoint(&self, height: u64, block_hash: [u8; 32]) {
        let mut long_range_protection = self.long_range_protection.write().unwrap();
        long_range_protection.add_checkpoint(height, block_hash);
    }
    
    /// Get peer diversity score
    pub fn get_diversity_score(&self) -> f64 {
        let diversity_manager = self.diversity_manager.read().unwrap();
        diversity_manager.evaluate_diversity()
    }
} 