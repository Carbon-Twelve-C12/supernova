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
use tracing::{info, warn, error, debug};
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, RwLock, Mutex};
use rand::{thread_rng, Rng};
use std::time::Instant;

use super::peer_manager::{PeerManager, PeerInfo};
use super::protocol::{Message, Protocol, BlockAnnouncement, BlockHeader};
use super::sync::ChainSync;
use crate::blockchain::{Block, BlockHeader as CoreBlockHeader};

pub struct P2PNetwork {
    swarm: Swarm<ComposedBehaviour>,
    command_receiver: mpsc::Receiver<NetworkCommand>,
    event_sender: mpsc::Sender<NetworkEvent>,
    peer_manager: PeerManager,
    chain_sync: Arc<Mutex<ChainSync>>,
    protocol: Protocol,
}

#[derive(libp2p::swarm::NetworkBehaviour)]
#[behaviour(out_event = "ComposedEvent")]
struct ComposedBehaviour {
    kad: kad::Behaviour<kad::store::MemoryStore>,
    identify: identify::Behaviour,
    mdns: Mdns,
    gossipsub: libp2p::gossipsub::Behaviour,
}

enum ComposedEvent {
    Kad(kad::Event),
    Identify(identify::Event),
    Mdns(mdns::Event),
    Gossipsub(libp2p::gossipsub::Event),
}

/// Commands that can be sent to the network service
#[derive(Debug, Clone)]
pub enum NetworkCommand {
    /// Start listening on specified address
    StartListening(String),
    /// Dial a specific peer address
    Dial(String),
    /// Announce a new block to the network
    AnnounceBlock(Vec<u8>, u64),
    /// Announce a new transaction to the network
    AnnounceTransaction(Vec<u8>, [u8; 32], u64),
    /// Get information about connected peers
    GetPeerInfo,
    /// Initiate peer rotation for diversity
    RotatePeers,
    /// Send a specific message to a peer
    SendMessage(PeerId, Message),
    /// Broadcast a message to all peers
    BroadcastMessage(Message),
    /// Begin chain synchronization to target height
    StartSync(u64),
}

/// Events emitted by the network service
#[derive(Debug, Clone)]
pub enum NetworkEvent {
    /// New peer connected
    NewPeer(PeerId),
    /// Peer disconnected
    PeerLeft(PeerId),
    /// New block received
    NewBlock(u64, Block),
    /// New transaction received
    NewTransaction(Vec<u8>),
    /// Peer information response
    PeerInfo(Vec<PeerInfo>),
    /// Peer rotation plan
    PeerRotationPlan(Vec<PeerId>, Vec<PeerId>),
    /// Headers received from peer
    HeadersReceived(PeerId, Vec<BlockHeader>, u64),
    /// Block received from peer
    BlockReceived(PeerId, u64, Block),
    /// Sync progress update
    SyncProgress(f64),
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
        
        // Create protocol instance
        let protocol = Protocol::new(id_keys.clone())?;
        
        // Create chain sync with a reference to the command sender for sending requests
        let sync_command_sender = command_sender.clone();
        let chain_sync = Arc::new(Mutex::new(ChainSync::new(sync_command_sender)));

        Ok((
            Self {
                swarm,
                command_receiver,
                event_sender,
                peer_manager,
                chain_sync,
                protocol,
            },
            command_sender,
            event_receiver,
        ))
    }

    pub async fn run(&mut self) {
        // Subscribe to pubsub topics
        if let Err(e) = self.protocol.subscribe_to_topics() {
            error!("Failed to subscribe to topics: {}", e);
        }
        
        // Start the chain sync background process
        let chain_sync_clone = self.chain_sync.clone();
        let event_sender_clone = self.event_sender.clone();
        tokio::spawn(async move {
            let mut sync = chain_sync_clone.lock().unwrap();
            let mut interval = tokio::time::interval(Duration::from_secs(5));
            
            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        // Report sync progress
                        let progress = sync.get_sync_progress();
                        if let Err(e) = event_sender_clone.send(NetworkEvent::SyncProgress(progress)).await {
                            error!("Failed to send sync progress event: {}", e);
                        }
                    }
                }
            }
        });
        
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
            SwarmEvent::Behaviour(ComposedEvent::Gossipsub(gossipsub_event)) => {
                self.handle_gossipsub_event(gossipsub_event).await;
            }
            SwarmEvent::ConnectionEstablished { peer_id, endpoint, .. } => {
                if let Some(addr) = endpoint.get_remote_address() {
                    if let Some((ip, port)) = extract_ip_port(addr) {
                        match self.peer_manager.try_add_connection(peer_id, ip, port) {
                            Ok(_) => {
                                info!("Connection established with {}", peer_id);
                                
                                // Send handshake to the new peer
                                let handshake = Message::Handshake(super::protocol::HandshakeData {
                                    version: super::protocol::PROTOCOL_VERSION,
                                    user_agent: "supernova/1.0.0".to_string(),
                                    features: super::protocol::features::HEADERS_FIRST_SYNC | 
                                              super::protocol::features::PARALLEL_BLOCK_DOWNLOAD,
                                    height: {
                                        let sync = self.chain_sync.lock().unwrap();
                                        sync.height
                                    },
                                });
                                
                                self.send_message_to_peer(peer_id, handshake).await;
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

    async fn handle_gossipsub_event(&mut self, event: libp2p::gossipsub::Event) {
        match event {
            libp2p::gossipsub::Event::Message { 
                propagation_source: peer_id,
                message_id: _,
                message,
            } => {
                // Deserialize the message
                match bincode::deserialize::<Message>(&message.data) {
                    Ok(msg) => {
                        debug!("Received message: {} from peer {}", msg, peer_id);
                        self.handle_message(peer_id, msg).await;
                    }
                    Err(e) => {
                        warn!("Failed to deserialize message from {}: {}", peer_id, e);
                    }
                }
            }
            _ => {}
        }
    }

    async fn handle_message(&mut self, peer_id: PeerId, message: Message) {
        match message {
            Message::Handshake(handshake) => {
                info!("Received handshake from peer {}: v{}, height: {}", 
                     peer_id, handshake.version, handshake.height);
                
                // Record the peer's features and height
                self.peer_manager.set_peer_protocols(&peer_id, 
                    vec![format!("supernova/{}", handshake.version)]);
                
                // Update chain sync with peer's height
                {
                    let mut sync = self.chain_sync.lock().unwrap();
                    sync.update_peer_height(peer_id, handshake.height);
                    
                    // If peer has higher blocks, consider syncing
                    if handshake.height > sync.height {
                        // Don't start sync here directly to avoid race conditions
                        // Instead, periodically check for sync opportunities
                    }
                }
            }
            Message::Block(announcement) => {
                debug!("Received block announcement: height={}, hash={:?}", 
                      announcement.height, announcement.hash);
                
                // Process block announcement in chain sync
                let mut sync = self.chain_sync.lock().unwrap();
                if let Err(e) = sync.handle_block_announcement(peer_id, announcement).await {
                    warn!("Error handling block announcement: {}", e);
                }
            }
            Message::Transaction(tx) => {
                debug!("Received transaction announcement: hash={:?}", tx.hash);
                // Forward to transaction pool (not implemented yet)
            }
            Message::GetHeaders { start_height, count } => {
                debug!("Received GetHeaders request: start={}, count={}", start_height, count);
                
                // Get headers from chain sync
                let headers = {
                    let sync = self.chain_sync.lock().unwrap();
                    sync.get_headers_range(start_height, count).await
                        .unwrap_or_else(|e| {
                            warn!("Failed to get headers: {}", e);
                            Vec::new()
                        })
                };
                
                let response = Message::Headers {
                    headers,
                    start_height,
                };
                self.send_message_to_peer(peer_id, response).await;
            }
            Message::Headers { headers, start_height } => {
                info!("Received {} headers starting at height {} from peer {}", 
                     headers.len(), start_height, peer_id);
                
                // Process headers in chain sync
                {
                    let mut sync = self.chain_sync.lock().unwrap();
                    if let Err(e) = sync.handle_headers(peer_id, headers.clone()).await {
                        warn!("Error handling headers: {}", e);
                    }
                }
                
                // Notify listeners
                self.event_sender.send(NetworkEvent::HeadersReceived(
                    peer_id, headers, start_height
                )).await.ok();
            }
            Message::GetBlock { hash } => {
                debug!("Received GetBlock request for hash {:?}", hash);
                
                // Get block from chain sync
                let block_response = {
                    let sync = self.chain_sync.lock().unwrap();
                    sync.get_block_by_hash(&hash).await
                };
                
                if let Ok(Some((height, block_data))) = block_response {
                    let response = Message::Block { height, block: block_data };
                    self.send_message_to_peer(peer_id, response).await;
                } else {
                    debug!("Block not found for hash {:?}", hash);
                }
            }
            Message::Block { height, block } => {
                info!("Received block at height {} from peer {}", height, peer_id);
                
                // Convert to Block type
                // This is a placeholder - actual implementation would deserialize properly
                let block_data = Block { /* ... */ };
                
                // Process block in chain sync
                {
                    let mut sync = self.chain_sync.lock().unwrap();
                    if let Err(e) = sync.handle_block(peer_id, height, block_data.clone()).await {
                        warn!("Error handling block: {}", e);
                    }
                }
                
                // Notify listeners
                self.event_sender.send(NetworkEvent::BlockReceived(
                    peer_id, height, block_data
                )).await.ok();
            }
            Message::GetBlocks { start, end } => {
                debug!("Received GetBlocks request: start={}, end={}", start, end);
                
                // Get blocks from chain sync
                let blocks = {
                    let sync = self.chain_sync.lock().unwrap();
                    sync.get_blocks_range(start, end).await
                        .unwrap_or_else(|e| {
                            warn!("Failed to get blocks: {}", e);
                            Vec::new()
                        })
                };
                
                let response = Message::Blocks { blocks };
                self.send_message_to_peer(peer_id, response).await;
            }
            Message::Blocks { blocks } => {
                info!("Received {} blocks from peer {}", blocks.len(), peer_id);
                // Process multiple blocks in order
                for (height, block_data) in blocks {
                    // Convert to Block type
                    // This is a placeholder - actual implementation would deserialize properly
                    let block = Block { /* ... */ };
                    
                    // Process in chain sync
                    let mut sync = self.chain_sync.lock().unwrap();
                    if let Err(e) = sync.handle_block(peer_id, height, block.clone()).await {
                        warn!("Error handling block at height {}: {}", height, e);
                    }
                    
                    // Notify listeners
                    self.event_sender.send(NetworkEvent::BlockReceived(
                        peer_id, height, block
                    )).await.ok();
                }
            }
            Message::Ping(nonce) => {
                // Respond with pong
                let response = Message::Pong(nonce);
                self.send_message_to_peer(peer_id, response).await;
            }
            Message::Pong(_) => {
                // Update peer latency metrics
                // (not implemented yet)
            }
            Message::PeerDiscovery(discovery_msg) => {
                match discovery_msg {
                    super::protocol::PeerDiscoveryMessage::GetPeers => {
                        // Respond with known peers
                        let peer_infos = self.peer_manager.get_connected_peer_infos();
                        let protocol_peers = peer_infos.into_iter()
                            .map(|info| super::protocol::PeerInfo {
                                address: format!("{}:{}", info.ip, info.port),
                                last_seen: 0, // Use current time in actual implementation
                                features: 0,  // Set features in actual implementation
                            })
                            .collect();
                        
                        let response = Message::PeerDiscovery(
                            super::protocol::PeerDiscoveryMessage::Peers(protocol_peers)
                        );
                        self.send_message_to_peer(peer_id, response).await;
                    }
                    super::protocol::PeerDiscoveryMessage::Peers(peers) => {
                        // Process discovered peers
                        for peer_info in peers {
                            debug!("Discovered peer via exchange: {}", peer_info.address);
                            
                            // Parse address and add to peer manager for future connections
                            if let Ok(addr) = peer_info.address.parse::<String>() {
                                // Extract IP and port from address string (format: "ip:port")
                                if let Some((ip_str, port_str)) = addr.split_once(':') {
                                    if let (Ok(ip), Ok(port)) = (ip_str.parse::<IpAddr>(), port_str.parse::<u16>()) {
                                        // Create a new peer ID from the address (in real implementation, 
                                        // this would be provided in the peer info)
                                        let new_peer_id = PeerId::random();
                                        
                                        // Add to peer manager's known peers for future connections
                                        self.peer_manager.add_known_peer(new_peer_id, ip, port);
                                        
                                        // Consider dialing if we need more connections
                                        if self.peer_manager.should_connect_to_more_peers() {
                                            self.try_dial_peer(new_peer_id).await;
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            Message::Challenge(challenge_msg) => {
                // Handle challenge-response protocol
                match challenge_msg {
                    super::protocol::ChallengeMessage::Request { .. } => {
                        // Respond to challenge (not implemented yet)
                    }
                    super::protocol::ChallengeMessage::Response { .. } => {
                        // Verify challenge response (not implemented yet)
                    }
                    super::protocol::ChallengeMessage::Result { .. } => {
                        // Handle challenge result (not implemented yet)
                    }
                }
            }
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
            NetworkCommand::AnnounceBlock(data, height) => {
                if let Err(e) = self.protocol.publish_block(data, height) {
                    warn!("Failed to announce block: {}", e);
                }
            }
            NetworkCommand::AnnounceTransaction(data, hash, fee_rate) => {
                if let Err(e) = self.protocol.publish_transaction(data, hash, fee_rate) {
                    warn!("Failed to announce transaction: {}", e);
                }
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
            NetworkCommand::SendMessage(peer_id, message) => {
                self.send_message_to_peer(peer_id, message).await;
            }
            NetworkCommand::BroadcastMessage(message) => {
                self.broadcast_message(message).await;
            }
            NetworkCommand::StartSync(target_height) => {
                info!("Starting blockchain sync to height {}", target_height);
                let mut sync = self.chain_sync.lock().unwrap();
                if let Err(e) = sync.start_sync(target_height).await {
                    error!("Failed to start sync: {}", e);
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
    
    async fn send_message_to_peer(&mut self, peer_id: PeerId, message: Message) {
        match bincode::serialize(&message) {
            Ok(encoded) => {
                // This is a simplistic approach - in a real implementation, this would use a proper
                // messaging protocol based on the connection type (e.g., gossipsub, request-response, etc.)
                if let Err(e) = self.swarm.behaviour_mut().gossipsub.publish_any(peer_id, encoded) {
                    warn!("Failed to send message to peer {}: {}", peer_id, e);
                }
            }
            Err(e) => {
                warn!("Failed to serialize message: {}", e);
            }
        }
    }
    
    async fn broadcast_message(&mut self, message: Message) {
        // This is a simplistic broadcast implementation
        // In a real implementation, you would use a proper broadcast protocol
        let connected_peers: Vec<PeerId> = self.swarm.connected_peers().copied().collect();
        
        match bincode::serialize(&message) {
            Ok(encoded) => {
                for peer_id in connected_peers {
                    if let Err(e) = self.swarm.behaviour_mut().gossipsub.publish_any(peer_id, encoded.clone()) {
                        warn!("Failed to broadcast message to peer {}: {}", peer_id, e);
                    }
                }
            }
            Err(e) => {
                warn!("Failed to serialize broadcast message: {}", e);
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
    
    // Configure gossipsub
    let gossipsub_config = libp2p::gossipsub::ConfigBuilder::default()
        .heartbeat_interval(Duration::from_secs(1))
        .validation_mode(libp2p::gossipsub::ValidationMode::Strict)
        .build()
        .map_err(|e| format!("Failed to build gossipsub config: {}", e))?;
    
    let gossipsub = libp2p::gossipsub::Behaviour::new(
        libp2p::gossipsub::MessageAuthenticity::Signed(id_keys.clone()),
        gossipsub_config,
    )?;

    Ok(ComposedBehaviour {
        kad: kad_behaviour,
        identify,
        mdns,
        gossipsub,
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

/// Placeholder for Block type until we implement it properly
#[derive(Debug, Clone)]
pub struct Block {
    // Block fields will go here
}

impl Block {
    // Just a placeholder implementation
    pub fn hash(&self) -> [u8; 32] {
        [0; 32] // Return dummy hash for now
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