use libp2p::{
    core::muxing::StreamMuxerBox,
    core::transport::Boxed,
    futures::StreamExt,
    identify, identity, kad,
    mdns::Mdns,
    multiaddr::Protocol as MultiAddrProtocol,
    noise,
    swarm::{Swarm, SwarmEvent},
    tcp, yamux, PeerId, Transport,
};
use std::error::Error;
use std::net::IpAddr;
use std::time::Duration;
use tokio::sync::mpsc;
use tracing::{info, warn, error};
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, RwLock};
use rand::thread_rng;
use std::time::Instant;

use super::peer_manager::{PeerManager, PeerInfo};

pub struct P2PNetwork {
    swarm: Swarm<ComposedBehaviour>,
    command_receiver: mpsc::Receiver<NetworkCommand>,
    event_sender: mpsc::Sender<NetworkEvent>,
    peer_manager: PeerManager,
}

#[derive(libp2p::swarm::NetworkBehaviour)]
#[behaviour(out_event = "ComposedEvent")]
struct ComposedBehaviour {
    kad: kad::Behaviour<kad::store::MemoryStore>,
    identify: identify::Behaviour,
    mdns: Mdns,
}

enum ComposedEvent {
    Kad(kad::Event),
    Identify(identify::Event),
    Mdns(mdns::Event),
}

pub enum NetworkCommand {
    StartListening(String),
    Dial(String),
    AnnounceBlock(Vec<u8>),
    AnnounceTransaction(Vec<u8>),
    GetPeerInfo,
    RotatePeers,
}

pub enum NetworkEvent {
    NewPeer(PeerId),
    PeerLeft(PeerId),
    NewBlock(Vec<u8>),
    NewTransaction(Vec<u8>),
    PeerInfo(Vec<PeerInfo>),
    PeerRotationPlan(Vec<PeerId>, Vec<PeerId>),
}

/// Configuration for eclipse attack prevention
#[derive(Debug, Clone)]
pub struct EclipsePreventionConfig {
    /// Minimum number of outbound connections to maintain
    pub min_outbound_connections: usize,
    /// Forced rotation interval in seconds
    pub forced_rotation_interval: u64,
    /// Enable automatic peer rotation
    pub enable_automatic_rotation: bool,
    /// Maximum peers per subnet
    pub max_peers_per_subnet: usize,
    /// Maximum peers per ASN
    pub max_peers_per_asn: usize,
    /// Maximum peers per region
    pub max_peers_per_region: usize,
    /// Maximum ratio of inbound to outbound connections
    pub max_inbound_ratio: f64,
}

impl Default for EclipsePreventionConfig {
    fn default() -> Self {
        Self {
            min_outbound_connections: 8,
            forced_rotation_interval: 3600, // 1 hour
            enable_automatic_rotation: true,
            max_peers_per_subnet: 3,
            max_peers_per_asn: 8,
            max_peers_per_region: 15,
            max_inbound_ratio: 3.0, // 3:1 max ratio
        }
    }
}

/// Network connection direction
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionDirection {
    /// Inbound connection (peer connected to us)
    Inbound,
    /// Outbound connection (we connected to peer)
    Outbound,
}

/// Network connection manager with Eclipse attack prevention
pub struct ConnectionManager {
    /// Currently connected peers with direction information
    connected_peers: HashMap<PeerId, ConnectionDirection>,
    /// Eclipse prevention configuration
    eclipse_config: EclipsePreventionConfig,
    /// Last rotation time
    last_rotation: Instant,
    /// Reference to peer manager
    peer_manager: Arc<RwLock<PeerManager>>,
    /// Network diversity tracker
    network_diversity: NetworkDiversityTracker,
    /// Forced connections (never rotated)
    protected_peers: HashSet<PeerId>,
}

/// Tracks network diversity for eclipse prevention
#[derive(Debug, Default)]
pub struct NetworkDiversityTracker {
    /// Subnets of connected peers
    pub subnets: HashMap<IpSubnet, usize>,
    /// ASNs of connected peers
    pub asns: HashMap<u32, usize>,
    /// Geographic regions of connected peers
    pub regions: HashMap<String, usize>,
    /// Last diversity score calculation
    pub last_score: f64,
    /// Last calculation time
    pub last_calculation: Instant,
}

impl NetworkDiversityTracker {
    /// Create new diversity tracker
    pub fn new() -> Self {
        Self {
            last_calculation: Instant::now(),
            ..Default::default()
        }
    }
    
    /// Add peer to diversity tracking
    pub fn add_peer(&mut self, info: &PeerInfo) {
        // Track subnet
        let subnet = IpSubnet::from_ip(info.ip);
        *self.subnets.entry(subnet).or_insert(0) += 1;
        
        // Track ASN if available
        if let Some(asn) = info.asn {
            *self.asns.entry(asn).or_insert(0) += 1;
        }
        
        // Track region if available
        if let Some(region) = &info.region {
            *self.regions.entry(region.clone()).or_insert(0) += 1;
        }
        
        // Mark as needing recalculation
        self.last_calculation = Instant::now();
    }
    
    /// Remove peer from diversity tracking
    pub fn remove_peer(&mut self, info: &PeerInfo) {
        // Untrack subnet
        let subnet = IpSubnet::from_ip(info.ip);
        if let Some(count) = self.subnets.get_mut(&subnet) {
            *count -= 1;
            if *count == 0 {
                self.subnets.remove(&subnet);
            }
        }
        
        // Untrack ASN if available
        if let Some(asn) = info.asn {
            if let Some(count) = self.asns.get_mut(&asn) {
                *count -= 1;
                if *count == 0 {
                    self.asns.remove(&asn);
                }
            }
        }
        
        // Untrack region if available
        if let Some(region) = &info.region {
            if let Some(count) = self.regions.get_mut(region) {
                *count -= 1;
                if *count == 0 {
                    self.regions.remove(region);
                }
            }
        }
        
        // Mark as needing recalculation
        self.last_calculation = Instant::now();
    }
    
    /// Calculate diversity score (0-1)
    pub fn calculate_diversity_score(&mut self) -> f64 {
        // Skip recalculation if recent
        if self.last_calculation.elapsed() < Duration::from_secs(60) {
            return self.last_score;
        }
        
        // Calculate entropy across different dimensions
        let subnet_entropy = Self::calculate_entropy(&self.subnets);
        let asn_entropy = Self::calculate_entropy(&self.asns);
        let region_entropy = Self::calculate_entropy(&self.regions);
        
        // Weight the entropies
        let score = subnet_entropy * 0.5 + asn_entropy * 0.3 + region_entropy * 0.2;
        
        // Normalize to 0-1
        self.last_score = score.min(1.0);
        self.last_calculation = Instant::now();
        
        self.last_score
    }
    
    /// Calculate entropy of a distribution
    fn calculate_entropy<K>(distribution: &HashMap<K, usize>) -> f64 {
        if distribution.is_empty() {
            return 0.0;
        }
        
        let total: usize = distribution.values().sum();
        if total == 0 {
            return 0.0;
        }
        
        let total_f64 = total as f64;
        
        // Calculate Shannon entropy: -sum(p_i * log(p_i))
        let mut entropy = 0.0;
        for &count in distribution.values() {
            let p = count as f64 / total_f64;
            if p > 0.0 {
                entropy -= p * p.log2();
            }
        }
        
        // Normalize by max entropy
        let max_entropy = (distribution.len() as f64).log2();
        if max_entropy > 0.0 {
            entropy / max_entropy
        } else {
            0.0
        }
    }
    
    /// Check if a new connection would exceed diversity limits
    pub fn would_exceed_limits(&self, info: &PeerInfo, config: &EclipsePreventionConfig) -> bool {
        // Check subnet limit
        let subnet = IpSubnet::from_ip(info.ip);
        if let Some(&count) = self.subnets.get(&subnet) {
            if count >= config.max_peers_per_subnet {
                return true;
            }
        }
        
        // Check ASN limit if available
        if let Some(asn) = info.asn {
            if let Some(&count) = self.asns.get(&asn) {
                if count >= config.max_peers_per_asn {
                    return true;
                }
            }
        }
        
        // Check region limit if available
        if let Some(region) = &info.region {
            if let Some(&count) = self.regions.get(region) {
                if count >= config.max_peers_per_region {
                    return true;
                }
            }
        }
        
        false
    }
}

impl ConnectionManager {
    /// Create a new connection manager
    pub fn new(peer_manager: Arc<RwLock<PeerManager>>, config: EclipsePreventionConfig) -> Self {
        Self {
            connected_peers: HashMap::new(),
            eclipse_config: config,
            last_rotation: Instant::now(),
            peer_manager,
            network_diversity: NetworkDiversityTracker::new(),
            protected_peers: HashSet::new(),
        }
    }
    
    /// Add a new connection
    pub fn add_connection(&mut self, peer_id: PeerId, direction: ConnectionDirection, info: &PeerInfo) -> Result<(), String> {
        // Check if adding this peer would exceed diversity limits
        if direction == ConnectionDirection::Inbound && 
           self.network_diversity.would_exceed_limits(info, &self.eclipse_config) {
            return Err("Connection would exceed diversity limits".to_string());
        }
        
        // Check inbound/outbound ratio
        let (inbound, outbound) = self.count_connections_by_direction();
        if direction == ConnectionDirection::Inbound && 
           inbound as f64 >= outbound as f64 * self.eclipse_config.max_inbound_ratio {
            return Err("Too many inbound connections relative to outbound".to_string());
        }
        
        // Add to connected peers
        self.connected_peers.insert(peer_id, direction);
        
        // Update diversity tracking
        self.network_diversity.add_peer(info);
        
        Ok(())
    }
    
    /// Remove a connection
    pub fn remove_connection(&mut self, peer_id: &PeerId, info: &PeerInfo) {
        self.connected_peers.remove(peer_id);
        self.network_diversity.remove_peer(info);
    }
    
    /// Add peer to protected list (never rotated)
    pub fn protect_peer(&mut self, peer_id: PeerId) {
        self.protected_peers.insert(peer_id);
    }
    
    /// Remove peer from protected list
    pub fn unprotect_peer(&mut self, peer_id: &PeerId) {
        self.protected_peers.remove(peer_id);
    }
    
    /// Count connections by direction
    pub fn count_connections_by_direction(&self) -> (usize, usize) {
        let mut inbound = 0;
        let mut outbound = 0;
        
        for &direction in self.connected_peers.values() {
            match direction {
                ConnectionDirection::Inbound => inbound += 1,
                ConnectionDirection::Outbound => outbound += 1,
            }
        }
        
        (inbound, outbound)
    }
    
    /// Check if we need to rotate peers
    pub fn check_rotation_needed(&self) -> bool {
        // Check if rotation is enabled
        if !self.eclipse_config.enable_automatic_rotation {
            return false;
        }
        
        // Check if enough time has passed since last rotation
        if self.last_rotation.elapsed() < Duration::from_secs(self.eclipse_config.forced_rotation_interval) {
            return false;
        }
        
        // Check if we have enough connections to rotate
        let (_, outbound) = self.count_connections_by_direction();
        if outbound <= self.eclipse_config.min_outbound_connections {
            return false;
        }
        
        true
    }
    
    /// Create a rotation plan
    pub fn create_rotation_plan(&self) -> Option<(Vec<PeerId>, usize)> {
        // Check if rotation is needed
        if !self.check_rotation_needed() {
            return None;
        }
        
        // Find outbound peers eligible for rotation
        let mut candidates: Vec<PeerId> = self.connected_peers.iter()
            .filter(|(peer_id, &direction)| {
                direction == ConnectionDirection::Outbound && !self.protected_peers.contains(peer_id)
            })
            .map(|(peer_id, _)| *peer_id)
            .collect();
        
        // Shuffle candidates
        let mut rng = thread_rng();
        candidates.shuffle(&mut rng);
        
        // Calculate how many peers to rotate (20-30% of outbound connections)
        let (_, outbound) = self.count_connections_by_direction();
        let min_to_keep = self.eclipse_config.min_outbound_connections;
        let max_to_rotate = (outbound - min_to_keep).max(0);
        
        if max_to_rotate == 0 {
            return None;
        }
        
        // Rotate 20-30% of eligible connections
        let to_rotate = (max_to_rotate as f64 * thread_rng().gen_range(0.2..0.3)) as usize;
        if to_rotate == 0 {
            return None;
        }
        
        // Take the first N candidates
        let rotation_list = candidates.into_iter().take(to_rotate).collect();
        
        Some((rotation_list, to_rotate))
    }
    
    /// Perform peer rotation
    pub fn perform_rotation(&mut self) -> Option<Vec<PeerId>> {
        let rotation_plan = self.create_rotation_plan()?;
        self.last_rotation = Instant::now();
        Some(rotation_plan.0)
    }
    
    /// Get diversity score
    pub fn get_diversity_score(&mut self) -> f64 {
        self.network_diversity.calculate_diversity_score()
    }
    
    /// Get the number of connections
    pub fn connection_count(&self) -> usize {
        self.connected_peers.len()
    }
    
    /// Get inbound connection ratio
    pub fn inbound_ratio(&self) -> f64 {
        let (inbound, outbound) = self.count_connections_by_direction();
        if outbound == 0 {
            return 0.0;
        }
        inbound as f64 / outbound as f64
    }
}

impl P2PNetwork {
    pub async fn new() -> Result<(Self, mpsc::Sender<NetworkCommand>, mpsc::Receiver<NetworkEvent>), Box<dyn Error>> {
        let id_keys = identity::Keypair::generate_ed25519();
        let peer_id = PeerId::from(id_keys.public());
        info!("Local peer id: {}", peer_id);

        let transport = build_transport(id_keys.clone())?;
        let behaviour = build_behaviour(id_keys.clone()).await?;
        let swarm = Swarm::new(transport, behaviour, peer_id);

        let (command_sender, command_receiver) = mpsc::channel(32);
        let (event_sender, event_receiver) = mpsc::channel(32);
        let peer_manager = PeerManager::new();

        Ok((
            Self {
                swarm,
                command_receiver,
                event_sender,
                peer_manager,
            },
            command_sender,
            event_receiver,
        ))
    }

    pub async fn run(&mut self) {
        loop {
            tokio::select! {
                event = self.swarm.select_next_some() => self.handle_swarm_event(event).await,
                command = self.command_receiver.recv() => {
                    if let Some(cmd) = command {
                        self.handle_command(cmd).await;
                    } else {
                        break;
                    }
                }
            }
        }
    }

    async fn handle_swarm_event(&mut self, event: SwarmEvent<ComposedEvent>) {
        match event {
            SwarmEvent::Behaviour(ComposedEvent::Kad(kad::Event::OutboundQueryCompleted { result, .. })) => {
                match result {
                    kad::QueryResult::GetProviders(Ok(provider_peers)) => {
                        for peer in provider_peers.providers {
                            // Only dial if not violating diversity limits
                            self.try_dial_peer(peer).await;
                        }
                    }
                    _ => {}
                }
            }
            SwarmEvent::Behaviour(ComposedEvent::Mdns(mdns::Event::Discovered(list))) => {
                for (peer_id, addr) in list {
                    // Extract IP address and port from multiaddr
                    if let Some((ip, port)) = extract_ip_port(&addr) {
                        match self.peer_manager.try_add_connection(peer_id, ip, port) {
                            Ok(_) => {
                                self.event_sender.send(NetworkEvent::NewPeer(peer_id)).await.ok();
                                info!("New peer discovered: {}", peer_id);
                            }
                            Err(e) => {
                                warn!("Rejected peer connection from {}: {}", peer_id, e);
                                // If the peer was rejected for diversity reasons, we might
                                // disconnect it later in a controlled manner
                            }
                        }
                    }
                }
            }
            SwarmEvent::Behaviour(ComposedEvent::Mdns(mdns::Event::Expired(list))) => {
                for (peer_id, _) in list {
                    self.peer_manager.handle_disconnect(&peer_id, Some("MDNS expiration".to_string()));
                    self.event_sender.send(NetworkEvent::PeerLeft(peer_id)).await.ok();
                    info!("Peer connection expired: {}", peer_id);
                }
            }
            SwarmEvent::ConnectionEstablished { peer_id, endpoint, .. } => {
                if let Some(addr) = endpoint.get_remote_address() {
                    if let Some((ip, port)) = extract_ip_port(addr) {
                        match self.peer_manager.try_add_connection(peer_id, ip, port) {
                            Ok(_) => {
                                info!("Connection established with {}", peer_id);
                            }
                            Err(e) => {
                                warn!("Connection established but rejected by peer manager: {}", e);
                                // If diversity limits are violated, disconnect
                                if e.contains("diversity limits") {
                                    info!("Disconnecting peer due to diversity limits: {}", peer_id);
                                    self.swarm.disconnect_peer_id(peer_id);
                                }
                            }
                        }
                    }
                }
            }
            SwarmEvent::ConnectionClosed { peer_id, cause, .. } => {
                let reason = format!("Connection closed: {:?}", cause);
                self.peer_manager.handle_disconnect(&peer_id, Some(reason));
                info!("Connection closed with {}: {:?}", peer_id, cause);
            }
            _ => {}
        }
    }

    async fn handle_command(&mut self, command: NetworkCommand) {
        match command {
            NetworkCommand::StartListening(addr) => {
                if let Err(e) = self.swarm.listen_on(addr.parse().unwrap()) {
                    warn!("Failed to start listening: {}", e);
                }
            }
            NetworkCommand::Dial(addr) => {
                if let Err(e) = self.swarm.dial(addr.parse().unwrap()) {
                    warn!("Failed to dial address: {}", e);
                }
            }
            NetworkCommand::AnnounceBlock(data) => {
                // TODO: Implement block announcement
            }
            NetworkCommand::AnnounceTransaction(data) => {
                // TODO: Implement transaction announcement
            }
            NetworkCommand::GetPeerInfo => {
                // Collect info about all connected peers
                let peer_infos: Vec<PeerInfo> = self.peer_manager.get_connected_peer_infos();
                self.event_sender.send(NetworkEvent::PeerInfo(peer_infos)).await.ok();
            }
            NetworkCommand::RotatePeers => {
                // Request a peer rotation plan from the peer manager
                if let Some((to_disconnect, to_connect)) = self.peer_manager.create_rotation_plan() {
                    // Notify about the plan
                    self.event_sender.send(NetworkEvent::PeerRotationPlan(
                        to_disconnect.clone(), 
                        to_connect.clone()
                    )).await.ok();
                    
                    // Execute the plan
                    self.execute_rotation_plan(to_disconnect, to_connect).await;
                }
            }
        }
    }
    
    async fn try_dial_peer(&mut self, peer_id: PeerId) {
        // Check if we're already connected
        if self.swarm.is_connected(&peer_id) {
            return;
        }
        
        // Check if we have too many connections from this peer's subnet
        // For now, just dial - the connection will be evaluated when established
        if let Err(e) = self.swarm.dial(peer_id) {
            warn!("Failed to dial peer {}: {}", peer_id, e);
        }
    }
    
    async fn execute_rotation_plan(&mut self, to_disconnect: Vec<PeerId>, to_connect: Vec<PeerId>) {
        // Disconnect peers to improve diversity
        for peer_id in &to_disconnect {
            info!("Diversity rotation: disconnecting peer {}", peer_id);
            self.swarm.disconnect_peer_id(*peer_id);
            // The disconnection will be handled in the ConnectionClosed event
        }
        
        // Connect to new peers to improve diversity
        for peer_id in &to_connect {
            info!("Diversity rotation: connecting to peer {}", peer_id);
            if let Err(e) = self.swarm.dial(*peer_id) {
                warn!("Failed to dial peer for rotation {}: {}", peer_id, e);
            }
        }
    }
}

fn build_transport(
    id_keys: identity::Keypair,
) -> Result<Boxed<(PeerId, StreamMuxerBox)>, Box<dyn Error>> {
    let noise_keys = noise::Keypair::<noise::X25519Spec>::new()
        .into_authentic(&id_keys)
        .expect("Signing libp2p-noise static DH keypair failed.");

    Ok(tcp::TokioTcpConfig::new()
        .nodelay(true)
        .upgrade(libp2p::core::upgrade::Version::V1)
        .authenticate(noise::NoiseConfig::xx(noise_keys).into_authenticated())
        .multiplex(yamux::YamuxConfig::default())
        .boxed())
}

async fn build_behaviour(
    id_keys: identity::Keypair,
) -> Result<ComposedBehaviour, Box<dyn Error>> {
    let kad_store = kad::store::MemoryStore::new(id_keys.public().to_peer_id());
    let kad_config = kad::Config::default();
    let kad_behaviour = kad::Behaviour::new(
        id_keys.public().to_peer_id(),
        kad_store,
        kad_config,
    );

    let identify = identify::Behaviour::new(identify::Config::new(
        "supernova/1.0.0".into(),
        id_keys.public(),
    ));

    let mdns = Mdns::new(Default::default()).await?;

    Ok(ComposedBehaviour {
        kad: kad_behaviour,
        identify,
        mdns,
    })
}

// Helper function to extract IP and port from a multiaddr
fn extract_ip_port(addr: &libp2p::Multiaddr) -> Option<(IpAddr, u16)> {
    let mut iter = addr.iter();
    
    // Look for IP protocol in the multiaddr
    let ip = loop {
        match iter.next() {
            Some(MultiAddrProtocol::Ip4(ip)) => break IpAddr::V4(ip),
            Some(MultiAddrProtocol::Ip6(ip)) => break IpAddr::V6(ip),
            Some(_) => continue,
            None => return None,
        }
    };
    
    // Look for TCP or UDP protocol with port
    match iter.next() {
        Some(MultiAddrProtocol::Tcp(port)) => Some((ip, port)),
        Some(MultiAddrProtocol::Udp(port)) => Some((ip, port)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_network_creation() {
        let (network, _command_sender, _event_receiver) = P2PNetwork::new().await.unwrap();
        assert!(network.swarm.connected_peers().count() == 0);
    }
}