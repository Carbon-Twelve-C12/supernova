use libp2p::{
    core::muxing::StreamMuxerBox,
    core::transport::Boxed,
    futures::StreamExt,
    gossipsub,
    identify, identity, kad,
    mdns::Mdns,
    noise,
    swarm::{Swarm, SwarmEvent, NetworkBehaviour},
    tcp, yamux, PeerId, Transport,
};
use crate::network::protocol::{Message, Protocol, PublishError, Checkpoint};
use crate::network::sync::{ChainSync, SyncStats};
use btclib::types::{Block, BlockHeader, Transaction};
use std::error::Error;
use std::time::{Duration, Instant, SystemTime};
use std::collections::{HashMap, HashSet};
use tokio::sync::mpsc;
use tracing::{info, warn, error, debug};
use dashmap::DashMap;
use std::sync::Arc;
use sha2::{Sha256, Digest};

// Constants for network behavior
const PEER_PING_INTERVAL: Duration = Duration::from_secs(60);
const PEER_PURGE_INTERVAL: Duration = Duration::from_secs(600); // 10 minutes
const MAX_INBOUND_CONNECTIONS: usize = 128;
const MAX_OUTBOUND_CONNECTIONS: usize = 8;
const MIN_PEERS: usize = 3;
const MAX_BANNED_PEERS: usize = 1000;
const BAN_DURATION: Duration = Duration::from_secs(3600); // 1 hour
const MESSAGE_CACHE_SIZE: usize = 1000;
const MESSAGE_CACHE_TTL: Duration = Duration::from_secs(60);

/// Enhanced P2P network implementation with peer management
pub struct P2PNetwork {
    swarm: Swarm<ComposedBehaviour>,
    local_peer_id: PeerId,
    protocol: Protocol,
    command_receiver: mpsc::Receiver<NetworkCommand>,
    event_sender: mpsc::Sender<NetworkEvent>,
    peers: DashMap<PeerId, PeerInfo>,
    banned_peers: DashMap<PeerId, Instant>,
    message_cache: DashMap<Vec<u8>, Instant>, // Cache to prevent duplicate processing
    stats: NetworkStats,
    genesis_hash: [u8; 32],
    network_id: String,
}

/// Combined network behavior using libp2p components
#[derive(NetworkBehaviour)]
#[behaviour(out_event = "ComposedEvent")]
struct ComposedBehaviour {
    gossipsub: gossipsub::Behaviour,
    kad: kad::Behaviour<kad::store::MemoryStore>,
    identify: identify::Behaviour,
    mdns: Mdns,
}

/// Events from the combined network behavior
#[derive(Debug)]
enum ComposedEvent {
    Gossipsub(gossipsub::Event),
    Kad(kad::Event),
    Identify(identify::Event),
    Mdns(mdns::Event),
}

impl From<gossipsub::Event> for ComposedEvent {
    fn from(event: gossipsub::Event) -> Self {
        ComposedEvent::Gossipsub(event)
    }
}

impl From<kad::Event> for ComposedEvent {
    fn from(event: kad::Event) -> Self {
        ComposedEvent::Kad(event)
    }
}

impl From<identify::Event> for ComposedEvent {
    fn from(event: identify::Event) -> Self {
        ComposedEvent::Identify(event)
    }
}

impl From<mdns::Event> for ComposedEvent {
    fn from(event: mdns::Event) -> Self {
        ComposedEvent::Mdns(event)
    }
}

/// Network commands received from other components
#[derive(Debug, Clone)]
pub enum NetworkCommand {
    /// Start listening on a multiaddress
    StartListening(String),
    
    /// Dial a specific peer address
    Dial(String),
    
    /// Broadcast a message to all peers
    Broadcast(Message),
    
    /// Send a message to a specific peer
    SendToPeer {
        peer_id: PeerId,
        message: Message,
    },
    
    /// Disconnect from a peer
    DisconnectPeer(PeerId),
    
    /// Announce a block to the network
    AnnounceBlock {
        block: Block,
        height: u64,
        total_difficulty: u64,
    },
    
    /// Announce a transaction to the network
    AnnounceTransaction {
        transaction: Transaction,
        fee_rate: u64,
    },
    
    /// Request headers within a height range
    RequestHeaders {
        start_height: u64,
        end_height: u64,
        preferred_peer: Option<PeerId>,
    },
    
    /// Request blocks by their hashes
    RequestBlocks {
        block_hashes: Vec<[u8; 32]>,
        preferred_peer: Option<PeerId>,
    },
    
    /// Request blocks by height range
    RequestBlocksByHeight {
        start_height: u64,
        end_height: u64,
        preferred_peer: Option<PeerId>,
    },
    
    /// Announce node status
    AnnounceStatus {
        version: u32,
        height: u64,
        best_hash: [u8; 32],
        total_difficulty: u64,
    },
    
    /// Ban a peer for misbehavior
    BanPeer {
        peer_id: PeerId,
        reason: String,
        duration: Option<Duration>,
    },
}

/// Network events sent to other components
#[derive(Debug, Clone)]
pub enum NetworkEvent {
    /// New peer connected
    NewPeer(PeerId),
    
    /// Peer disconnected
    PeerLeft(PeerId),
    
    /// Received a new block
    NewBlock {
        block: Block,
        height: u64,
        total_difficulty: u64,
        from_peer: Option<PeerId>,
    },
    
    /// Received a new transaction
    NewTransaction {
        transaction: Transaction,
        fee_rate: u64,
        from_peer: Option<PeerId>,
    },
    
    /// Received block headers
    BlockHeaders {
        headers: Vec<BlockHeader>,
        total_difficulty: u64,
        from_peer: Option<PeerId>,
    },
    
    /// Received blocks in response to a request
    BlocksReceived {
        blocks: Vec<Block>,
        total_difficulty: u64,
        from_peer: Option<PeerId>,
    },
    
    /// Received peer status update
    PeerStatus {
        peer_id: PeerId,
        version: u32,
        height: u64,
        best_hash: [u8; 32],
        total_difficulty: u64,
    },
    
    /// Received checkpoint information
    CheckpointsReceived {
        checkpoints: Vec<Checkpoint>,
        from_peer: Option<PeerId>,
    },
}

/// Information about a connected peer
#[derive(Debug, Clone)]
struct PeerInfo {
    // Basic info
    first_seen: Instant,
    last_seen: Instant,
    address: Option<String>,
    client_version: Option<String>,
    
    // Chain state
    height: u64,
    best_hash: Option<[u8; 32]>,
    total_difficulty: u64,
    genesis_hash: Option<[u8; 32]>,
    
    // Scoring
    score: i32,
    ping_time_ms: Option<u64>,
    successful_requests: u64,
    failed_requests: u64,
    blocks_received: u64,
    invalid_blocks: u64,
    invalid_messages: u64,
    
    // Connection type
    inbound: bool,
}

impl PeerInfo {
    fn new(inbound: bool) -> Self {
        let now = Instant::now();
        Self {
            first_seen: now,
            last_seen: now,
            address: None,
            client_version: None,
            height: 0,
            best_hash: None,
            total_difficulty: 0,
            genesis_hash: None,
            score: 0,
            ping_time_ms: None,
            successful_requests: 0,
            failed_requests: 0,
            blocks_received: 0,
            invalid_blocks: 0,
            invalid_messages: 0,
            inbound,
        }
    }
    
    fn update_seen(&mut self) {
        self.last_seen = Instant::now();
    }
    
    fn update_score(&mut self, delta: i32) {
        self.score = (self.score + delta).clamp(-100, 100);
    }
}

/// Network statistics for monitoring
#[derive(Debug, Clone, Default)]
pub struct NetworkStats {
    // Connections
    peers_connected: usize,
    inbound_connections: usize,
    outbound_connections: usize,
    connection_attempts: usize,
    
    // Messages
    messages_sent: u64,
    messages_received: u64,
    blocks_announced: u64,
    transactions_announced: u64,
    
    // Sync
    headers_received: u64,
    blocks_received: u64,
    invalid_messages: u64,
    
    // Bans
    peers_banned: u64,
}

impl P2PNetwork {
    /// Create a new P2P network instance
    pub async fn new(
        keypair: Option<identity::Keypair>,
        genesis_hash: [u8; 32],
        network_id: &str,
    ) -> Result<(Self, mpsc::Sender<NetworkCommand>, mpsc::Receiver<NetworkEvent>), Box<dyn Error>> {
        // Generate keypair if not provided
        let id_keys = keypair.unwrap_or_else(|| identity::Keypair::generate_ed25519());
        let local_peer_id = PeerId::from(id_keys.public());
        info!("Local peer id: {}", local_peer_id);
        
        // Create protocol handler
        let protocol = Protocol::new(id_keys.clone())?;

        // Create network transport
        let transport = build_transport(id_keys.clone())?;
        
        // Create network behavior
        let behaviour = build_behaviour(id_keys.clone(), protocol.gossipsub().clone()).await?;
        
        // Create libp2p swarm
        let swarm = Swarm::new(transport, behaviour, local_peer_id.clone());
        
        // Create communication channels
        let (command_sender, command_receiver) = mpsc::channel(128);
        let (event_sender, event_receiver) = mpsc::channel(128);
        
        Ok((
            Self {
                swarm,
                local_peer_id,
                protocol,
                command_receiver,
                event_sender,
                peers: DashMap::new(),
                banned_peers: DashMap::new(),
                message_cache: DashMap::new(),
                stats: NetworkStats::default(),
                genesis_hash,
                network_id: network_id.to_string(),
            },
            command_sender,
            event_receiver,
        ))
    }
    
    /// Run the network event loop
    pub async fn run(&mut self) -> Result<(), Box<dyn Error>> {
        let mut periodic_tasks_interval = tokio::time::interval(Duration::from_secs(15));
        
        // Subscribe to protocol topics
        self.protocol.subscribe_to_topics()?;
        
        loop {
            tokio::select! {
                // Handle network events from libp2p
                event = self.swarm.select_next_some() => {
                    if let Err(e) = self.handle_swarm_event(event).await {
                        error!("Error handling swarm event: {}", e);
                    }
                },
                
                // Handle commands from other components
                Some(command) = self.command_receiver.recv() => {
                    if let Err(e) = self.handle_command(command).await {
                        error!("Error handling command: {}", e);
                    }
                },
                
                // Periodic tasks: ping peers, clean up, etc.
                _ = periodic_tasks_interval.tick() => {
                    self.perform_periodic_tasks().await;
                }
                
                // No more incoming commands, end the loop
                else => break,
            }
        }
        
        info!("Network event loop terminated");
        Ok(())
    }
    
    /// Handle events from the swarm
    async fn handle_swarm_event(&mut self, event: SwarmEvent<ComposedEvent>) -> Result<(), Box<dyn Error>> {
        match event {
            // Gossipsub events
            SwarmEvent::Behaviour(ComposedEvent::Gossipsub(gossipsub::Event::Message {
                propagation_source,
                message_id,
                message,
            })) => {
                // Avoid processing duplicate messages
                let msg_hash = Sha256::digest(&message.data).to_vec();
                if self.message_cache.contains_key(&msg_hash) {
                    return Ok(());
                }
                
                // Insert into cache to prevent duplicate processing
                self.message_cache.insert(msg_hash, Instant::now());
                
                // Process the message
                match bincode::deserialize::<Message>(&message.data) {
                    Ok(msg) => {
                        self.handle_protocol_message(msg, Some(propagation_source)).await?;
                        
                        // Update peer stats
                        if let Some(mut peer) = self.peers.get_mut(&propagation_source) {
                            peer.update_seen();
                            peer.successful_requests += 1;
                            peer.update_score(1); // Small positive score for valid message
                        }
                    },
                    Err(e) => {
                        warn!("Failed to deserialize message from {}: {}", propagation_source, e);
                        
                        // Update peer stats for invalid message
                        if let Some(mut peer) = self.peers.get_mut(&propagation_source) {
                            peer.invalid_messages += 1;
                            peer.update_score(-1); // Small negative score for invalid message
                        }
                    }
                }
                
                self.stats.messages_received += 1;
            },
            
            // Kademlia events
            SwarmEvent::Behaviour(ComposedEvent::Kad(kad::Event::OutboundQueryCompleted { result, .. })) => {
                if let kad::QueryResult::GetProviders(Ok(provider_peers)) = result {
                    for peer in provider_peers.providers {
                        if !self.is_peer_banned(&peer) && !self.is_peer_connected(&peer) {
                            if let Err(e) = self.swarm.dial(peer.clone()) {
                                warn!("Failed to dial discovered peer {}: {}", peer, e);
                            } else {
                                debug!("Dialing discovered peer: {}", peer);
                            }
                        }
                    }
                }
            },
            
            // MDNS discovery events
            SwarmEvent::Behaviour(ComposedEvent::Mdns(mdns::Event::Discovered(list))) => {
                for (peer_id, addr) in list {
                    if self.is_peer_banned(&peer_id) {
                        continue;
                    }
                    
                    if !self.is_peer_connected(&peer_id) {
                        info!("Discovered peer via mDNS: {} at {}", peer_id, addr);
                        if let Err(e) = self.swarm.dial(peer_id.clone()) {
                            warn!("Failed to dial mDNS peer {}: {}", peer_id, e);
                        }
                    }
                }
            },
            
            // MDNS expired events
            SwarmEvent::Behaviour(ComposedEvent::Mdns(mdns::Event::Expired(list))) => {
                for (peer_id, _) in list {
                    debug!("mDNS peer expired: {}", peer_id);
                }
            },
            
            // Identify events - get peer information
            SwarmEvent::Behaviour(ComposedEvent::Identify(identify::Event::Received { peer_id, info })) => {
                if let Some(mut peer_info) = self.peers.get_mut(&peer_id) {
                    peer_info.client_version = Some(info.agent_version);
                    debug!("Identified peer {}: {}", peer_id, info.agent_version);
                    
                    // Check network compatibility
                    if info.protocol_version != self.network_id {
                        warn!("Peer {} on different network: {}", peer_id, info.protocol_version);
                        self.disconnect_peer(peer_id).await?;
                    }
                }
            },
            
            // New connection established
            SwarmEvent::ConnectionEstablished { peer_id, endpoint, .. } => {
                let is_inbound = endpoint.is_listener();
                
                // Check if we're exceeding connection limits
                let inbound_count = self.count_inbound_peers();
                let outbound_count = self.count_outbound_peers();
                
                if (is_inbound && inbound_count >= MAX_INBOUND_CONNECTIONS) ||
                   (!is_inbound && outbound_count >= MAX_OUTBOUND_CONNECTIONS) {
                    debug!("Connection limits reached, disconnecting {}", peer_id);
                    self.disconnect_peer(peer_id).await?;
                    return Ok(());
                }
                
                if self.is_peer_banned(&peer_id) {
                    debug!("Rejecting connection from banned peer: {}", peer_id);
                    self.disconnect_peer(peer_id).await?;
                    return Ok(());
                }
                
                // Add peer to our list
                let peer_info = PeerInfo::new(is_inbound);
                self.peers.insert(peer_id.clone(), peer_info);
                
                info!("Connected to peer: {} ({})", peer_id, if is_inbound { "inbound" } else { "outbound" });
                
                // Update connection stats
                if is_inbound {
                    self.stats.inbound_connections += 1;
                } else {
                    self.stats.outbound_connections += 1;
                }
                self.stats.peers_connected = self.peers.len();
                
                // Notify other components
                self.event_sender.send(NetworkEvent::NewPeer(peer_id.clone())).await?;
                
                // Exchange status with the new peer
                self.send_status_to_peer(&peer_id).await?;
            },
            
            // Connection closed
            SwarmEvent::ConnectionClosed { peer_id, .. } => {
                if self.peers.contains_key(&peer_id) {
                    info!("Disconnected from peer: {}", peer_id);
                    
                    // Update stats based on connection type
                    if let Some(peer_info) = self.peers.get(&peer_id) {
                        if peer_info.inbound {
                            self.stats.inbound_connections = self.stats.inbound_connections.saturating_sub(1);
                        } else {
                            self.stats.outbound_connections = self.stats.outbound_connections.saturating_sub(1);
                        }
                    }
                    
                    // Remove peer
                    self.peers.remove(&peer_id);
                    self.stats.peers_connected = self.peers.len();
                    
                    // Notify other components
                    self.event_sender.send(NetworkEvent::PeerLeft(peer_id)).await?;
                }
            },
            
            // Listen events
            SwarmEvent::NewListenAddr { address, .. } => {
                info!("Listening on: {}", address);
            },
            
            // Other events
            _ => {}
        }
        
        Ok(())
    }
    
    /// Handle incoming protocol messages
    async fn handle_protocol_message(&mut self, message: Message, from_peer: Option<PeerId>) -> Result<(), Box<dyn Error>> {
        match message {
            // Process received blocks
            Message::NewBlock { block_data, height, total_difficulty } => {
                // Deserialize block
                match bincode::deserialize::<Block>(&block_data) {
                    Ok(block) => {
                        debug!("Received block at height {} from {:?}", height, from_peer);
                        
                        // Update peer metrics
                        if let Some(peer_id) = &from_peer {
                            if let Some(mut peer) = self.peers.get_mut(peer_id) {
                                peer.blocks_received += 1;
                                peer.height = height;
                                peer.total_difficulty = total_difficulty;
                                
                                // Update best hash if available
                                let block_hash = block.hash();
                                peer.best_hash = Some(block_hash);
                            }
                        }
                        
                        // Forward to other components
                        self.event_sender.send(NetworkEvent::NewBlock {
                            block,
                            height,
                            total_difficulty,
                            from_peer,
                        }).await?;
                        
                        self.stats.blocks_received += 1;
                    },
                    Err(e) => {
                        warn!("Failed to deserialize block from {:?}: {}", from_peer, e);
                        
                        // Update peer metrics for invalid block
                        if let Some(peer_id) = &from_peer {
                            if let Some(mut peer) = self.peers.get_mut(peer_id) {
                                peer.invalid_blocks += 1;
                                peer.update_score(-5); // Larger negative score for invalid block
                                
                                // Consider banning peer for too many invalid blocks
                                if peer.invalid_blocks > 3 {
                                    self.ban_peer(peer_id.clone(), "Too many invalid blocks", None).await?;
                                }
                            }
                        }
                    }
                }
            },
            
            // Process received transactions
            Message::BroadcastTransaction(transaction) => {
                debug!("Received transaction from {:?}", from_peer);
                
                // Calculate fee rate (simplified)
                let tx_size = bincode::serialize(&transaction)?.len() as u64;
                let fee_rate = 1; // Simplified, should calculate actual fee
                
                // Forward to other components
                self.event_sender.send(NetworkEvent::NewTransaction {
                    transaction,
                    fee_rate,
                    from_peer,
                }).await?;
            },
            
            // Process transaction announcements
            Message::TransactionAnnouncement { tx_hash, fee_rate } => {
                debug!("Received transaction announcement from {:?}: {}", from_peer, hex::encode(&tx_hash[..4]));
                
                // We could request the full transaction here if needed
            },
            
            // Process headers
            Message::Headers { headers, total_difficulty } => {
                debug!("Received {} headers from {:?}", headers.len(), from_peer);
                
                // Forward to other components
                self.event_sender.send(NetworkEvent::BlockHeaders {
                    headers,
                    total_difficulty,
                    from_peer,
                }).await?;
                
                self.stats.headers_received += headers.len() as u64;
            },
            
            // Process block responses
            Message::BlockResponse { blocks, total_difficulty } => {
                debug!("Received {} blocks from {:?}", blocks.len(), from_peer);
                
                // Forward to other components
                self.event_sender.send(NetworkEvent::BlocksReceived {
                    blocks,
                    total_difficulty,
                    from_peer,
                }).await?;
                
                self.stats.blocks_received += blocks.len() as u64;
                
                // Update peer metrics
                if let Some(peer_id) = &from_peer {
                    if let Some(mut peer) = self.peers.get_mut(peer_id) {
                        peer.blocks_received += blocks.len() as u64;
                        peer.successful_requests += 1;
                        peer.update_score(1);
                    }
                }
            },
            
            // Process status updates
            Message::Status { version, height, best_hash, total_difficulty, head_timestamp } => {
                debug!("Received status from {:?}: height={}", from_peer, height);
                
                if let Some(peer_id) = &from_peer {
                    // Update peer information
                    if let Some(mut peer) = self.peers.get_mut(peer_id) {
                        peer.height = height;
                        peer.best_hash = Some(best_hash);
                        peer.total_difficulty = total_difficulty;
                    }
                    
                    // Forward status to other components
                    self.event_sender.send(NetworkEvent::PeerStatus {
                        peer_id: peer_id.clone(),
                        version,
                        height,
                        best_hash,
                        total_difficulty,
                    }).await?;
                }
            },
            
            // Process checkpoint information
            Message::Checkpoints { checkpoints } => {
                debug!("Received {} checkpoints from {:?}", checkpoints.len(), from_peer);
                
                // Forward to other components
                self.event_sender.send(NetworkEvent::CheckpointsReceived {
                    checkpoints,
                    from_peer,
                }).await?;
            },
            
            // Handle requests that need responses
            Message::GetStatus => {
                if let Some(peer_id) = from_peer {
                    self.send_status_to_peer(&peer_id).await?;
                }
            },
            
            Message::GetHeaders { start_height, end_height } => {
                // This would be handled by the sync component which would prepare headers
                // and send them back to the requesting peer
                debug!("Headers request from {:?}: {}-{}", from_peer, start_height, end_height);
            },
            
            Message::GetBlocks { block_hashes } => {
                // This would be handled by the sync component which would prepare blocks
                debug!("Blocks request from {:?}: {} hashes", from_peer, block_hashes.len());
            },
            
            Message::GetBlocksByHeight { start_height, end_height } => {
                debug!("Blocks by height request from {:?}: {}-{}", from_peer, start_height, end_height);
            },
            
            Message::GetMempool => {
                debug!("Mempool request from {:?}", from_peer);
            },
            
            Message::GetCheckpoints { since_timestamp } => {
                debug!("Checkpoints request from {:?} since {}", from_peer, since_timestamp);
            },
            
            // Handle ping/pong for latency measurement
            Message::Ping(timestamp) => {
                if let Some(peer_id) = from_peer {
                    // Respond with pong
                    self.send_to_peer(&peer_id, Message::Pong(timestamp)).await?;
                }
            },
            
            Message::Pong(timestamp) => {
                if let Some(peer_id) = &from_peer {
                    // Calculate latency
                    let now = SystemTime::now()
                        .duration_since(SystemTime::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_millis() as u64;
                    
                    let latency = now.saturating_sub(timestamp);
                    
                    if let Some(mut peer) = self.peers.get_mut(peer_id) {
                        peer.ping_time_ms = Some(latency);
                        debug!("Ping to {:?}: {}ms", peer_id, latency);
                    }
                }
            },
            
            // Handle other message types
            _ => {
                debug!("Unhandled message type from {:?}", from_peer);
            }
        }
        
        Ok(())
    }
    
    /// Handle commands from other components
    async fn handle_command(&mut self, command: NetworkCommand) -> Result<(), Box<dyn Error>> {
        match command {
            // Start listening on address
            NetworkCommand::StartListening(addr) => {
                match addr.parse() {
                    Ok(multiaddr) => {
                        match self.swarm.listen_on(multiaddr) {
                            Ok(_) => {
                                info!("Started listening on {}", addr);
                            },
                            Err(e) => {
                                error!("Failed to start listening on {}: {}", addr, e);
                                return Err(e.into());
                            }
                        }
                    },
                    Err(e) => {
                        error!("Invalid multiaddress {}: {}", addr, e);
                        return Err(Box::new(std::io::Error::new(std::io::ErrorKind::InvalidInput, e)));
                    }
                }
            },
            
            // Dial a peer by address
            NetworkCommand::Dial(addr) => {
                match addr.parse() {
                    Ok(multiaddr) => {
                        match self.swarm.dial(multiaddr.clone()) {
                            Ok(_) => {
                                info!("Dialing {}", addr);
                                self.stats.connection_attempts += 1;
                            },
                            Err(e) => {
                                error!("Failed to dial {}: {}", addr, e);
                                return Err(e.into());
                            }
                        }
                    },
                    Err(e) => {
                        error!("Invalid multiaddress {}: {}", addr, e);
                        return Err(Box::new(std::io::Error::new(std::io::ErrorKind::InvalidInput, e)));
                    }
                }
            },
            
            // Broadcast a message to all peers
            NetworkCommand::Broadcast(message) => {
                match self.protocol.broadcast(message) {
                    Ok(_) => {
                        self.stats.messages_sent += 1;
                    },
                    Err(e) => {
                        error!("Failed to broadcast message: {}", e);
                        return Err(Box::new(e));
                    }
                }
            },
            
            // Send a message to a specific peer
            NetworkCommand::SendToPeer { peer_id, message } => {
                self.send_to_peer(&peer_id, message).await?;
            },
            
            // Disconnect from a peer
            NetworkCommand::DisconnectPeer(peer_id) => {
                self.disconnect_peer(peer_id).await?;
            },
            
            // Announce a new block
            NetworkCommand::AnnounceBlock { block, height, total_difficulty } => {
                match self.protocol.announce_block(&block, height, total_difficulty) {
                    Ok(_) => {
                        debug!("Announced block at height {}", height);
                        self.stats.blocks_announced += 1;
                    },
                    Err(e) => {
                        error!("Failed to announce block: {}", e);
                        return Err(Box::new(e));
                    }
                }
            },
            
            // Announce a new transaction
            NetworkCommand::AnnounceTransaction { transaction, fee_rate } => {
                match self.protocol.announce_transaction(&transaction, fee_rate) {
                    Ok(_) => {
                        debug!("Announced transaction {}", hex::encode(&transaction.hash()[..4]));
                        self.stats.transactions_announced += 1;
                    },
                    Err(e) => {
                        error!("Failed to announce transaction: {}", e);
                        return Err(Box::new(e));
                    }
                }
            },
            
            // Request headers
            NetworkCommand::RequestHeaders { start_height, end_height, preferred_peer } => {
                if let Some(peer_id) = preferred_peer {
                    // Request from specific peer
                    self.send_to_peer(&peer_id, Message::GetHeaders {
                        start_height,
                        end_height,
                    }).await?;
                } else {
                    // Broadcast to all peers
                    match self.protocol.request_headers(start_height, end_height) {
                        Ok(_) => {
                            debug!("Requested headers from {} to {}", start_height, end_height);
                        },
                        Err(e) => {
                            error!("Failed to request headers: {}", e);
                            return Err(Box::new(e));
                        }
                    }
                }
            },
            
            // Request blocks by hash
            NetworkCommand::RequestBlocks { block_hashes, preferred_peer } => {
                if let Some(peer_id) = preferred_peer {
                    // Request from specific peer
                    self.send_to_peer(&peer_id, Message::GetBlocks {
                        block_hashes,
                    }).await?;
                } else {
                    // Broadcast to all peers
                    match self.protocol.request_blocks(block_hashes) {
                        Ok(_) => {
                            debug!("Requested blocks by hash");
                        },
                        Err(e) => {
                            error!("Failed to request blocks: {}", e);
                            return Err(Box::new(e));
                        }
                    }
                }
            },
            
            // Request blocks by height
            NetworkCommand::RequestBlocksByHeight { start_height, end_height, preferred_peer } => {
                if let Some(peer_id) = preferred_peer {
                    // Request from specific peer
                    self.send_to_peer(&peer_id, Message::GetBlocksByHeight {
                        start_height,
                        end_height,
                    }).await?;
                } else {
                    // Broadcast to all peers
                    match self.protocol.request_blocks_by_height(start_height, end_height) {
                        Ok(_) => {
                            debug!("Requested blocks from {} to {}", start_height, end_height);
                        },
                        Err(e) => {
                            error!("Failed to request blocks by height: {}", e);
                            return Err(Box::new(e));
                        }
                    }
                }
            },
            
            // Announce node status
            NetworkCommand::AnnounceStatus { version, height, best_hash, total_difficulty } => {
                // Get current timestamp
                let head_timestamp = SystemTime::now()
                    .duration_since(SystemTime::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();
                
                match self.protocol.broadcast_status(version, height, best_hash, total_difficulty, head_timestamp) {
                    Ok(_) => {
                        debug!("Announced status: height={}, td={}", height, total_difficulty);
                    },
                    Err(e) => {
                        error!("Failed to announce status: {}", e);
                        return Err(Box::new(e));
                    }
                }
            },
            
            // Ban a peer
            NetworkCommand::BanPeer { peer_id, reason, duration } => {
                self.ban_peer(peer_id, &reason, duration).await?;
            },
        }
        
        Ok(())
    }
    
    /// Perform periodic tasks like peer pinging, cleanup, etc.
    async fn perform_periodic_tasks(&mut self) {
        self.clean_message_cache();
        self.clean_banned_peers();
        
        // Ensure minimum number of connections
        if self.peers.len() < MIN_PEERS {
            if let Err(e) = self.ensure_minimum_connections().await {
                warn!("Failed to ensure minimum connections: {}", e);
            }
        }
        
        // Ping peers to measure latency
        for peer_id in self.get_connected_peers() {
            let now = SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64;
            
            if let Err(e) = self.send_to_peer(&peer_id, Message::Ping(now)).await {
                warn!("Failed to ping peer {}: {}", peer_id, e);
            }
        }
    }
    
    /// Ensure we have the minimum number of connections
    async fn ensure_minimum_connections(&mut self) -> Result<(), Box<dyn Error>> {
        // This would typically connect to bootstrap nodes or use Kademlia discovery
        debug!("Ensuring minimum connections...");
        Ok(())
    }
    
    /// Send a message to a specific peer
    async fn send_to_peer(&mut self, peer_id: &PeerId, message: Message) -> Result<(), Box<dyn Error>> {
        if !self.is_peer_connected(peer_id) {
            return Err(Box::new(std::io::Error::new(
                std::io::ErrorKind::NotConnected,
                format!("Peer {} not connected", peer_id)
            )));
        }
        
        if self.is_peer_banned(peer_id) {
            return Err(Box::new(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                format!("Peer {} is banned", peer_id)
            )));
        }
        
        match self.protocol.send_to_peer(peer_id, message) {
            Ok(_) => {
                self.stats.messages_sent += 1;
                Ok(())
            },
            Err(e) => {
                error!("Failed to send message to {}: {}", peer_id, e);
                Err(Box::new(e))
            }
        }
    }
    
    /// Send current status to a peer
    async fn send_status_to_peer(&mut self, peer_id: &PeerId) -> Result<(), Box<dyn Error>> {
        // This would normally get the current chain state from the chain component
        // For now, we use placeholder values
        let version = 1;
        let height = 0;
        let best_hash = self.genesis_hash;
        let total_difficulty = 0;
        let head_timestamp = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        
        let message = Message::Status {
            version,
            height,
            best_hash,
            total_difficulty,
            head_timestamp,
        };
        
        self.send_to_peer(peer_id, message).await
    }
    
    /// Disconnect from a peer
    async fn disconnect_peer(&mut self, peer_id: PeerId) -> Result<(), Box<dyn Error>> {
        debug!("Disconnecting from peer: {}", peer_id);
        
        // In libp2p, we can't directly close a connection
        // Instead, we need to mark it as banned temporarily
        self.swarm.ban_peer_id(peer_id);
        
        Ok(())
    }
    
    /// Ban a peer for a specified duration
    async fn ban_peer(&mut self, peer_id: PeerId, reason: &str, duration: Option<Duration>) -> Result<(), Box<dyn Error>> {
        let ban_duration = duration.unwrap_or(BAN_DURATION);
        info!("Banning peer {} for {}: {}", peer_id, humanize_duration(ban_duration), reason);
        
        // Disconnect the peer
        self.disconnect_peer(peer_id.clone()).await?;
        
        // Add to banned list with expiration time
        self.banned_peers.insert(peer_id, Instant::now() + ban_duration);
        self.stats.peers_banned += 1;
        
        Ok(())
    }
    
    /// Check if a peer is currently banned
    fn is_peer_banned(&self, peer_id: &PeerId) -> bool {
        if let Some(expiry) = self.banned_peers.get(peer_id) {
            if expiry.value() > &Instant::now() {
                return true;
            }
        }
        false
    }
    
    /// Check if a peer is currently connected
    fn is_peer_connected(&self, peer_id: &PeerId) -> bool {
        self.peers.contains_key(peer_id)
    }
    
    /// Get a list of all connected peers
    fn get_connected_peers(&self) -> Vec<PeerId> {
        self.peers.iter().map(|entry| entry.key().clone()).collect()
    }
    
    /// Count the number of inbound peers
    fn count_inbound_peers(&self) -> usize {
        self.peers.iter().filter(|entry| entry.value().inbound).count()
    }
    
    /// Count the number of outbound peers
    fn count_outbound_peers(&self) -> usize {
        self.peers.iter().filter(|entry| !entry.value().inbound).count()
    }
    
    /// Clean up expired banned peers
    fn clean_banned_peers(&mut self) {
        let now = Instant::now();
        let expired: Vec<PeerId> = self.banned_peers
            .iter()
            .filter(|entry| entry.value() < &now)
            .map(|entry| entry.key().clone())
            .collect();
        
        for peer_id in expired {
            self.banned_peers.remove(&peer_id);
        }
        
        // Keep banned peers list manageable
        if self.banned_peers.len() > MAX_BANNED_PEERS {
            // Remove oldest bans first
            let mut peers_with_time: Vec<(PeerId, Instant)> = self.banned_peers
                .iter()
                .map(|entry| (entry.key().clone(), *entry.value()))
                .collect();
            
            peers_with_time.sort_by(|a, b| a.1.cmp(&b.1));
            
            let to_remove = peers_with_time.len() - MAX_BANNED_PEERS;
            for (peer_id, _) in peers_with_time.iter().take(to_remove) {
                self.banned_peers.remove(peer_id);
            }
        }
    }
    
    /// Clean up old message cache entries
    fn clean_message_cache(&mut self) {
        let now = Instant::now();
        let expired: Vec<Vec<u8>> = self.message_cache
            .iter()
            .filter(|entry| now.duration_since(*entry.value()) > MESSAGE_CACHE_TTL)
            .map(|entry| entry.key().clone())
            .collect();
        
        for key in expired {
            self.message_cache.remove(&key);
        }
        
        // Keep cache size manageable
        if self.message_cache.len() > MESSAGE_CACHE_SIZE {
            // This is inefficient but simple - in production, use a time-based data structure
            let mut msgs_with_time: Vec<(Vec<u8>, Instant)> = self.message_cache
                .iter()
                .map(|entry| (entry.key().clone(), *entry.value()))
                .collect();
            
            msgs_with_time.sort_by(|a, b| a.1.cmp(&b.1));
            
            let to_remove = msgs_with_time.len() - MESSAGE_CACHE_SIZE;
            for (key, _) in msgs_with_time.iter().take(to_remove) {
                self.message_cache.remove(key);
            }
        }
    }
    
    /// Get network statistics
    pub fn get_stats(&self) -> NetworkStats {
        self.stats.clone()
    }
}

/// Build the libp2p transport stack
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
        .timeout(Duration::from_secs(20))
        .boxed())
}

/// Build the libp2p network behavior
async fn build_behaviour(
    id_keys: identity::Keypair,
    gossipsub: gossipsub::Behaviour,
) -> Result<ComposedBehaviour, Box<dyn Error>> {
    let local_peer_id = PeerId::from(id_keys.public());
    
    // Kademlia DHT for peer discovery
    let kad_store = kad::store::MemoryStore::new(local_peer_id);
    let kad_config = kad::Config::default();
    let kad_behaviour = kad::Behaviour::new(
        local_peer_id,
        kad_store,
        kad_config,
    );

    // Identify protocol for peer metadata
    let identify = identify::Behaviour::new(identify::Config::new(
        "supernova/1.0.0".into(),
        id_keys.public(),
    ));

    // mDNS for local network discovery
    let mdns = Mdns::new(Default::default()).await?;

    Ok(ComposedBehaviour {
        gossipsub,
        kad: kad_behaviour,
        identify,
        mdns,
    })
}

/// Format a duration in a human-readable way
fn humanize_duration(duration: Duration) -> String {
    let seconds = duration.as_secs();
    if seconds < 60 {
        format!("{} seconds", seconds)
    } else if seconds < 3600 {
        format!("{} minutes", seconds / 60)
    } else if seconds < 86400 {
        format!("{} hours", seconds / 3600)
    } else {
        format!("{} days", seconds / 86400)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    
    #[tokio::test]
    async fn test_network_creation() {
        let (network, _, _) = P2PNetwork::new(
            None, 
            [0u8; 32], 
            "supernova-test"
        ).await.unwrap();
        
        assert_eq!(network.peers.len(), 0);
    }
    
    #[tokio::test]
    async fn test_peer_management() {
        let (mut network, command_tx, _) = P2PNetwork::new(
            None, 
            [0u8; 32], 
            "supernova-test"
        ).await.unwrap();
        
        // Test peer banning
        let peer_id = PeerId::random();
        
        network.ban_peer(peer_id.clone(), "Test ban", None).await.unwrap();
        assert!(network.is_peer_banned(&peer_id));
        
        // Test message sending to non-existent peer
        let result = network.send_to_peer(&peer_id, Message::GetStatus).await;
        assert!(result.is_err());
    }
    
    #[tokio::test]
    async fn test_message_cache() {
        let (mut network, _, _) = P2PNetwork::new(
            None, 
            [0u8; 32], 
            "supernova-test"
        ).await.unwrap();
        
        // Add items to cache
        for i in 0..10 {
            let key = Sha256::digest(&i.to_le_bytes()).to_vec();
            network.message_cache.insert(key, Instant::now());
        }
        
        assert_eq!(network.message_cache.len(), 10);
        
        // Clean cache - nothing should be removed yet
        network.clean_message_cache();
        assert_eq!(network.message_cache.len(), 10);
    }
}