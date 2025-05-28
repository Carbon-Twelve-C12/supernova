// This file intentionally left blank to be rewritten from scratch

use libp2p::{
    core::muxing::StreamMuxerBox,
    core::transport::Boxed,
    gossipsub::{self, MessageId, TopicHash},
    identity, 
    noise,
    swarm::{Swarm, SwarmEvent, NetworkBehaviour},
    tcp, yamux, PeerId, Transport, Multiaddr,
    core::{ConnectedPoint, upgrade},
};
use crate::network::{
    protocol::{Message, Protocol, PublishError, message_id_from_content},
    connection::{ConnectionManager, ConnectionEvent, ConnectionState},
    peer::{PeerInfo, PeerManager, PeerState, PeerMetadata},
    peer_diversity::{PeerDiversityManager, ConnectionStrategy, IpSubnet},
    message::{MessageHandler, NetworkMessage, MessageEvent},
    discovery::{PeerDiscovery, DiscoveryEvent},
    eclipse_prevention::{EclipsePreventionSystem, EclipsePreventionConfig, EclipseRiskLevel},
    rate_limiter::{NetworkRateLimiter, RateLimitConfig, RateLimitError, RateLimitMetrics},
    MAX_PEERS, MAX_INBOUND_CONNECTIONS, MAX_OUTBOUND_CONNECTIONS,
};
use btclib::{Block, BlockHeader, Transaction};
use std::{
    error::Error,
    net::IpAddr,
    collections::{HashMap, HashSet},
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::sync::{mpsc, RwLock};
use tokio::task::JoinHandle;
use tokio::time::sleep;
use tracing::{debug, info, warn, error, trace};
use dashmap::DashMap;
use futures::stream::StreamExt;
use rand::{Rng, RngCore, rngs::OsRng};
use sha2::{Sha256, Digest};
use byteorder::{ByteOrder, BigEndian};
use crate::api::types::{NetworkInfo, PeerInfo as ApiPeerInfo, ConnectionCount, BandwidthUsage, PeerAddResponse};

// Constants for network behavior
const MIN_PEERS: usize = 3;
const BAN_DURATION: Duration = Duration::from_secs(3600); // 1 hour
const CONNECTION_TIMEOUT: Duration = Duration::from_secs(30);
const RECONNECT_INTERVAL: Duration = Duration::from_secs(60);
const DISCOVERY_INTERVAL: Duration = Duration::from_secs(300);
const STATUS_BROADCAST_INTERVAL: Duration = Duration::from_secs(180);

/// Challenge difficulty for Sybil protection (number of leading zero bits)
const DEFAULT_CHALLENGE_DIFFICULTY: u8 = 16;

/// Challenge timeout in seconds
const CHALLENGE_TIMEOUT_SECS: u64 = 30;

/// Identity verification challenge
#[derive(Debug, Clone)]
pub struct IdentityChallenge {
    /// Random challenge data
    pub challenge: Vec<u8>,
    /// Required difficulty (leading zero bits)
    pub difficulty: u8,
    /// When the challenge was issued
    pub issued_at: Instant,
    /// Timeout duration
    pub timeout: Duration,
}

/// Status of peer identity verification
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IdentityVerificationStatus {
    /// Peer has not been challenged
    NotVerified,
    /// Challenge has been issued but not completed
    ChallengeIssued(Instant),
    /// Peer has successfully completed a challenge
    Verified(Instant),
    /// Peer has failed a challenge
    VerificationFailed(String),
}

/// Network commands received from other components
#[derive(Debug, Clone)]
pub enum NetworkCommand {
    /// Start listening on a multiaddress
    StartListening(Multiaddr),
    
    /// Dial a specific peer address
    Dial(PeerId, Multiaddr),
    
    /// Connect to a peer by address string
    ConnectToPeer(String),
    
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
    
    /// Add a peer to the trusted peers list
    AddTrustedPeer(PeerId),
    
    /// Remove a peer from the trusted peers list
    RemoveTrustedPeer(PeerId),
    
    /// Start the network
    Start,
    
    /// Stop the network
    Stop,
}

/// Network events sent to other components
#[derive(Debug, Clone)]
pub enum NetworkEvent {
    /// New peer connected
    NewPeer(PeerId),
    
    /// Peer disconnected
    PeerLeft(PeerId),
    
    /// Peer connected with info
    PeerConnected(PeerInfo),
    
    /// Peer disconnected
    PeerDisconnected(PeerId),
    
    /// Message received from peer
    MessageReceived {
        peer_id: PeerId,
        message: Message,
    },
    
    /// Message sent to peer
    MessageSent {
        peer_id: PeerId,
        message: Message,
    },
    
    /// Network error
    Error {
        peer_id: Option<PeerId>,
        error: String,
    },
    
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
        checkpoints: Vec<crate::network::protocol::Checkpoint>,
        from_peer: Option<PeerId>,
    },
    
    /// Network started
    Started,
    
    /// Network stopped
    Stopped,
    
    /// Listening on address
    Listening(Multiaddr),
    
    /// General network error
    NetworkError(String),
}

/// Enhanced P2P network implementation with peer management
pub struct P2PNetwork {
    /// LibP2P swarm
    swarm: Arc<RwLock<Option<Swarm<gossipsub::Gossipsub>>>>,
    /// Local peer ID
    local_peer_id: PeerId,
    /// Protocol handler
    protocol: Protocol,
    /// Command receiver
    command_receiver: Arc<RwLock<Option<mpsc::Receiver<NetworkCommand>>>>,
    /// Event sender channel
    event_sender: mpsc::Sender<NetworkEvent>,
    /// Peer manager
    peer_manager: Arc<PeerManager>,
    /// Connection manager
    connection_manager: Arc<RwLock<ConnectionManager>>,
    /// Diversity manager for Sybil protection
    diversity_manager: Arc<PeerDiversityManager>,
    /// Eclipse prevention system
    eclipse_prevention: Arc<EclipsePreventionSystem>,
    /// Network rate limiter
    rate_limiter: Arc<NetworkRateLimiter>,
    /// Message handler
    message_handler: MessageHandler,
    /// Peer discovery system
    discovery: Arc<RwLock<Option<PeerDiscovery>>>,
    /// Network statistics
    stats: Arc<RwLock<NetworkStats>>,
    /// Genesis hash for chain identification
    genesis_hash: [u8; 32],
    /// Network ID string
    network_id: String,
    /// Bootstrap nodes
    bootstrap_nodes: Vec<(PeerId, Multiaddr)>,
    /// Trusted peers that are always connected
    trusted_peers: Arc<RwLock<HashSet<PeerId>>>,
    /// Network task handle
    network_task: Arc<RwLock<Option<JoinHandle<()>>>>,
    /// Is the network running
    running: Arc<RwLock<bool>>,
    /// Identity verification challenges
    identity_challenges: Arc<RwLock<HashMap<PeerId, IdentityChallenge>>>,
    
    /// Peer verification status
    verification_status: Arc<RwLock<HashMap<PeerId, IdentityVerificationStatus>>>,
    
    /// Challenge difficulty for identity verification (leading zero bits)
    challenge_difficulty: u8,
    
    /// Whether to require identity verification
    require_verification: bool,
    
    /// Connected peers with their information
    connected_peers: Arc<RwLock<HashMap<PeerId, PeerInfo>>>,
    
    /// Banned peers with ban expiry times
    banned_peers: Arc<RwLock<HashMap<PeerId, Instant>>>,
    
    /// Message routing table for efficient message delivery
    message_routes: Arc<RwLock<HashMap<PeerId, Vec<PeerId>>>>,
    
    /// Bandwidth tracking
    bandwidth_tracker: Arc<RwLock<BandwidthTracker>>,
}

/// Network statistics for monitoring
#[derive(Debug, Clone, Default)]
pub struct NetworkStats {
    // Connections
    pub peers_connected: usize,
    pub inbound_connections: usize,
    pub outbound_connections: usize,
    pub connection_attempts: usize,
    
    // Messages
    pub messages_sent: u64,
    pub messages_received: u64,
    pub blocks_announced: u64,
    pub transactions_announced: u64,
    
    // Sync
    pub headers_received: u64,
    pub blocks_received: u64,
    pub invalid_messages: u64,
    
    // Bans
    pub peers_banned: u64,
    
    // Performance
    pub avg_latency_ms: f64,
    pub message_throughput: f64,
}

/// Bandwidth tracking for network monitoring
#[derive(Debug, Default)]
pub struct BandwidthTracker {
    pub bytes_sent: u64,
    pub bytes_received: u64,
    pub messages_sent: u64,
    pub messages_received: u64,
    pub start_time: Option<Instant>,
}

impl BandwidthTracker {
    pub fn new() -> Self {
        Self {
            start_time: Some(Instant::now()),
            ..Default::default()
        }
    }
    
    pub fn record_sent(&mut self, bytes: u64) {
        self.bytes_sent += bytes;
        self.messages_sent += 1;
    }
    
    pub fn record_received(&mut self, bytes: u64) {
        self.bytes_received += bytes;
        self.messages_received += 1;
    }
    
    pub fn get_rates(&self, period_secs: u64) -> (f64, f64) {
        if let Some(start) = self.start_time {
            let elapsed = start.elapsed().as_secs().max(1);
            let actual_period = period_secs.min(elapsed);
            
            let send_rate = self.bytes_sent as f64 / actual_period as f64;
            let recv_rate = self.bytes_received as f64 / actual_period as f64;
            
            (send_rate, recv_rate)
        } else {
            (0.0, 0.0)
        }
    }
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
        
        // Create communication channels
        let (command_sender, command_receiver) = mpsc::channel(128);
        let (event_sender, event_receiver) = mpsc::channel(128);
        
        // Create peer manager
        let peer_manager = Arc::new(PeerManager::new());
        
        // Create diversity manager for Sybil protection
        let diversity_manager = Arc::new(PeerDiversityManager::with_config(
            0.6, // Minimum diversity score
            ConnectionStrategy::BalancedDiversity,
            10,  // Max connection attempts per minute
        ));
        
        // Create eclipse prevention system
        let eclipse_config = EclipsePreventionConfig::default();
        let eclipse_prevention = Arc::new(EclipsePreventionSystem::new(eclipse_config));
        
        // Create network rate limiter
        let rate_limit_config = RateLimitConfig::default();
        let rate_limiter = Arc::new(NetworkRateLimiter::new(rate_limit_config));
        
        // Create message handler
        let message_handler = MessageHandler::new();
        
        // Create connection manager
        let connection_manager = ConnectionManager::new(
            Arc::clone(&peer_manager),
            Arc::clone(&diversity_manager),
            MAX_INBOUND_CONNECTIONS,
            MAX_OUTBOUND_CONNECTIONS,
        );
        
        Ok((
            Self {
                swarm: Arc::new(RwLock::new(None)),
                local_peer_id,
                protocol,
                command_receiver: Arc::new(RwLock::new(Some(command_receiver))),
                event_sender,
                peer_manager,
                connection_manager: Arc::new(RwLock::new(connection_manager)),
                diversity_manager,
                eclipse_prevention,
                rate_limiter,
                message_handler,
                discovery: Arc::new(RwLock::new(None)),
                stats: Arc::new(RwLock::new(NetworkStats::default())),
                genesis_hash,
                network_id: network_id.to_string(),
                bootstrap_nodes: Vec::new(),
                trusted_peers: Arc::new(RwLock::new(HashSet::new())),
                network_task: Arc::new(RwLock::new(None)),
                running: Arc::new(RwLock::new(false)),
                identity_challenges: Arc::new(RwLock::new(HashMap::new())),
                verification_status: Arc::new(RwLock::new(HashMap::new())),
                challenge_difficulty: DEFAULT_CHALLENGE_DIFFICULTY,
                require_verification: true,
                connected_peers: Arc::new(RwLock::new(HashMap::new())),
                banned_peers: Arc::new(RwLock::new(HashMap::new())),
                message_routes: Arc::new(RwLock::new(HashMap::new())),
                bandwidth_tracker: Arc::new(RwLock::new(BandwidthTracker::new())),
            },
            command_sender,
            event_receiver,
        ))
    }
    
    /// Get the local peer ID
    pub fn local_peer_id(&self) -> PeerId {
        self.local_peer_id.clone()
    }
    
    /// Add a bootstrap node
    pub fn add_bootstrap_node(&mut self, peer_id: PeerId, addr: Multiaddr) {
        self.bootstrap_nodes.push((peer_id, addr));
    }
    
    /// Add multiple bootstrap nodes
    pub fn add_bootstrap_nodes(&mut self, nodes: Vec<(PeerId, Multiaddr)>) {
        self.bootstrap_nodes.extend(nodes);
    }
    
    /// Add a trusted peer
    pub async fn add_trusted_peer(&self, peer_id: PeerId) {
        let mut trusted = self.trusted_peers.write().await;
        trusted.insert(peer_id);
    }
    
    /// Start the network
    pub async fn start(&self) -> Result<(), Box<dyn Error>> {
        let mut running = self.running.write().await;
        if *running {
            return Ok(());
        }
        
        info!("Starting P2P network");
        
        // Initialize the swarm
        let keypair = identity::Keypair::generate_ed25519();
        let local_peer_id = PeerId::from(keypair.public());
        
        // Build transport
        let transport = build_transport(keypair.clone())?;
        
        // Configure gossipsub
        let gossipsub_config = gossipsub::GossipsubConfigBuilder::default()
            .heartbeat_interval(Duration::from_secs(10))
            .validation_mode(gossipsub::ValidationMode::Strict)
            .message_id_fn(message_id_from_content)
            .build()
            .map_err(|e| format!("Failed to build gossipsub config: {}", e))?;
            
        let gossipsub = gossipsub::Gossipsub::new(
            gossipsub::MessageAuthenticity::Signed(keypair.clone()),
            gossipsub_config,
        )?;
        
        // Create the swarm
        let mut swarm = Swarm::new(transport, gossipsub, local_peer_id);
        
        // Subscribe to topics
        if let Some(behaviour) = swarm.behaviour_mut().as_mut() {
            // Subscribe to topics
            let topics = [
                "blocks",
                "transactions",
                "headers",
                "status",
                "mempool",
            ];
            
            for topic in &topics {
                let topic = gossipsub::IdentTopic::new(topic);
                if let Err(e) = behaviour.subscribe(&topic) {
                    warn!("Failed to subscribe to topic {}: {}", topic, e);
                } else {
                    debug!("Subscribed to topic: {}", topic);
                }
            }
        }
        
        // Initialize peer discovery
        let (discovery, discovery_rx) = PeerDiscovery::new(
            &keypair,
            self.bootstrap_nodes.clone(),
            true, // Enable mDNS
        ).await?;
        
        *self.discovery.write().await = Some(discovery);
        
        // Store the swarm
        *self.swarm.write().await = Some(swarm);
        
        // Mark as running
        *running = true;
        
        // Send started event
        if let Err(e) = self.event_sender.send(NetworkEvent::Started).await {
            warn!("Failed to send network started event: {}", e);
        }
        
        // Start the main network loop
        self.start_network_loop().await?;
        
        info!("P2P network started");
        
        Ok(())
    }
    
    /// Start the main network event loop
    async fn start_network_loop(&self) -> Result<(), Box<dyn Error>> {
        let swarm = Arc::clone(&self.swarm);
        let command_receiver = Arc::clone(&self.command_receiver);
        let event_sender = self.event_sender.clone();
        let running = Arc::clone(&self.running);
        let stats = Arc::clone(&self.stats);
        let connected_peers = Arc::clone(&self.connected_peers);
        let bandwidth_tracker = Arc::clone(&self.bandwidth_tracker);
        let rate_limiter = Arc::clone(&self.rate_limiter);
        let banned_peers = Arc::clone(&self.banned_peers);
        
        let task = tokio::spawn(async move {
            let mut command_rx = command_receiver.write().await.take().unwrap();
            
            // Create interval timers
            let mut rate_limit_cleanup_interval = tokio::time::interval(Duration::from_secs(300)); // 5 minutes
            let mut ban_cleanup_interval = tokio::time::interval(Duration::from_secs(60)); // 1 minute
            
            loop {
                let is_running = *running.read().await;
                if !is_running {
                    break;
                }
                
                tokio::select! {
                    // Process network commands
                    command = command_rx.recv() => {
                        if let Some(cmd) = command {
                            Self::handle_command_static(
                                cmd,
                                &swarm,
                                &event_sender,
                                &stats,
                                &connected_peers,
                                &bandwidth_tracker,
                            ).await;
                        } else {
                            // Command channel closed, exit
                            break;
                        }
                    }
                    
                    // Process swarm events
                    _ = async {
                        let mut swarm_guard = swarm.write().await;
                        if let Some(swarm) = swarm_guard.as_mut() {
                            if let Some(event) = swarm.next().await {
                                Self::handle_swarm_event_static(
                                    event,
                                    &event_sender,
                                    &stats,
                                    &connected_peers,
                                    &bandwidth_tracker,
                                ).await;
                            }
                        }
                    } => {}
                    
                    // Periodic rate limiter cleanup
                    _ = rate_limit_cleanup_interval.tick() => {
                        debug!("Cleaning up rate limiter entries");
                        rate_limiter.cleanup();
                        
                        // Log rate limiter metrics
                        let metrics = rate_limiter.metrics();
                        info!("Rate limiter metrics - Total requests: {}, Rejected: {}, Banned IPs: {}",
                              metrics.total_requests, metrics.rejected_requests, metrics.banned_ips);
                    }
                    
                    // Periodic ban cleanup
                    _ = ban_cleanup_interval.tick() => {
                        debug!("Cleaning up expired bans");
                        let now = Instant::now();
                        banned_peers.write().await.retain(|_, ban_time| *ban_time > now);
                    }
                }
            }
            
            info!("Network event loop stopped");
        });
        
        *self.network_task.write().await = Some(task);
        
        Ok(())
    }
    
    /// Handle a network command (static version for async context)
    async fn handle_command_static(
        command: NetworkCommand,
        swarm: &Arc<RwLock<Option<Swarm<gossipsub::Gossipsub>>>>,
        event_sender: &mpsc::Sender<NetworkEvent>,
        stats: &Arc<RwLock<NetworkStats>>,
        connected_peers: &Arc<RwLock<HashMap<PeerId, PeerInfo>>>,
        bandwidth_tracker: &Arc<RwLock<BandwidthTracker>>,
    ) {
        match command {
            NetworkCommand::ConnectToPeer(addr_str) => {
                // Parse the address and attempt to connect
                if let Ok(addr) = addr_str.parse::<Multiaddr>() {
                    let mut swarm_guard = swarm.write().await;
                    if let Some(swarm) = swarm_guard.as_mut() {
                        match swarm.dial(addr.clone()) {
                        Ok(_) => {
                                debug!("Dialing peer at {}", addr);
                                let mut stats_guard = stats.write().await;
                                stats_guard.connection_attempts += 1;
                        }
                        Err(e) => {
                                warn!("Failed to dial peer at {}: {}", addr, e);
                                let _ = event_sender.send(NetworkEvent::Error {
                                    peer_id: None,
                                    error: format!("Failed to dial: {}", e),
                                }).await;
                            }
                        }
                    }
                } else {
                    warn!("Invalid peer address format: {}", addr_str);
                    let _ = event_sender.send(NetworkEvent::Error {
                        peer_id: None,
                        error: format!("Invalid address format: {}", addr_str),
                    }).await;
                }
            }
            
            NetworkCommand::Broadcast(message) => {
                Self::broadcast_message_static(
                    message,
                    swarm,
                    stats,
                    bandwidth_tracker,
                ).await;
            }
            
            NetworkCommand::SendToPeer { peer_id, message } => {
                Self::send_to_peer_static(
                    peer_id,
                    message,
                    swarm,
                    stats,
                    bandwidth_tracker,
                ).await;
            }
            
            NetworkCommand::AnnounceBlock { block, height, total_difficulty } => {
                let message = Message::NewBlock {
                    block_data: bincode::serialize(&block).unwrap_or_default(),
                    height,
                    total_difficulty,
                };
                
                Self::broadcast_message_static(
                    message,
                    swarm,
                    stats,
                    bandwidth_tracker,
                ).await;
                
                let mut stats_guard = stats.write().await;
                stats_guard.blocks_announced += 1;
            }
            
            NetworkCommand::AnnounceTransaction { transaction, fee_rate } => {
                let message = Message::Transaction {
                    transaction: bincode::serialize(&transaction).unwrap_or_default(),
                };
                
                Self::broadcast_message_static(
                    message,
                    swarm,
                    stats,
                    bandwidth_tracker,
                ).await;
                
                let mut stats_guard = stats.write().await;
                stats_guard.transactions_announced += 1;
            }
            
            _ => {
                // Handle other commands as needed
                debug!("Unhandled network command: {:?}", command);
            }
        }
    }
    
    /// Broadcast a message to all connected peers (static version)
    async fn broadcast_message_static(
        message: Message,
        swarm: &Arc<RwLock<Option<Swarm<gossipsub::Gossipsub>>>>,
        stats: &Arc<RwLock<NetworkStats>>,
        bandwidth_tracker: &Arc<RwLock<BandwidthTracker>>,
    ) {
        let mut swarm_guard = swarm.write().await;
        if let Some(swarm) = swarm_guard.as_mut() {
                    // Serialize the message
                    let encoded = match bincode::serialize(&message) {
                        Ok(data) => data,
                        Err(e) => {
                            warn!("Failed to serialize message: {}", e);
                    return;
                        }
                    };
                    
                    // Determine the topic
                    let topic_name = match &message {
                        Message::Block { .. } | Message::NewBlock { .. } => "blocks",
                        Message::Transaction { .. } => "transactions",
                        Message::GetHeaders { .. } | Message::Headers { .. } => "headers",
                        Message::Status { .. } | Message::GetStatus => "status",
                        Message::GetMempool { .. } | Message::Mempool { .. } => "mempool",
                        _ => "status", // Default
                    };
                    
                    let topic = gossipsub::IdentTopic::new(topic_name);
                    
                    // Publish the message
                    if let Some(behaviour) = swarm.behaviour_mut().as_mut() {
                match behaviour.publish(topic, encoded.clone()) {
                            Ok(msg_id) => {
                                debug!("Published message with ID: {:?}", msg_id);
                        let mut stats_guard = stats.write().await;
                        stats_guard.messages_sent += 1;
                        
                        let mut bandwidth_guard = bandwidth_tracker.write().await;
                        bandwidth_guard.record_sent(encoded.len() as u64);
                            }
                            Err(e) => {
                                warn!("Failed to publish message: {}", e);
                            }
                        }
                    }
                }
            }
    
    /// Send a message to a specific peer (static version)
    async fn send_to_peer_static(
        peer_id: PeerId,
        message: Message,
        swarm: &Arc<RwLock<Option<Swarm<gossipsub::Gossipsub>>>>,
        stats: &Arc<RwLock<NetworkStats>>,
        bandwidth_tracker: &Arc<RwLock<BandwidthTracker>>,
    ) {
        // For now, we'll broadcast the message since direct peer messaging
        // requires a different protocol setup
        debug!("Sending message to peer {} (via broadcast)", peer_id);
        Self::broadcast_message_static(message, swarm, stats, bandwidth_tracker).await;
    }
    
    /// Handle a libp2p swarm event (static version)
    async fn handle_swarm_event_static(
        event: SwarmEvent<gossipsub::GossipsubEvent>,
        event_sender: &mpsc::Sender<NetworkEvent>,
        stats: &Arc<RwLock<NetworkStats>>,
        connected_peers: &Arc<RwLock<HashMap<PeerId, PeerInfo>>>,
        bandwidth_tracker: &Arc<RwLock<BandwidthTracker>>,
    ) {
        match event {
            SwarmEvent::Behaviour(gossipsub::GossipsubEvent::Message { 
                propagation_source,
                message_id,
                message,
            }) => {
                // Deserialize the message
                match bincode::deserialize::<Message>(&message.data) {
                    Ok(msg) => {
                        // Process the message
                        let mut stats_guard = stats.write().await;
                        stats_guard.messages_received += 1;
                        drop(stats_guard);
                        
                        let mut bandwidth_guard = bandwidth_tracker.write().await;
                        bandwidth_guard.record_received(message.data.len() as u64);
                        drop(bandwidth_guard);
                        
                        Self::handle_protocol_message_static(
                            &propagation_source,
                            msg,
                            event_sender,
                            stats,
                        ).await;
                    }
                    Err(e) => {
                        warn!("Failed to deserialize message from {}: {}", propagation_source, e);
                        let mut stats_guard = stats.write().await;
                        stats_guard.invalid_messages += 1;
                    }
                }
            }
            SwarmEvent::NewListenAddr { address, .. } => {
                info!("Listening on {}", address);
                let _ = event_sender.send(NetworkEvent::Listening(address)).await;
            }
            SwarmEvent::ConnectionEstablished { 
                peer_id, 
                endpoint, 
                ..
            } => {
                info!("Connected to {}", peer_id);
                
                // Extract IP address from endpoint
                let ip_address = match &endpoint {
                    ConnectedPoint::Dialer { address, .. } => {
                        extract_ip_from_multiaddr(address)
                    }
                    ConnectedPoint::Listener { send_back_addr, .. } => {
                        extract_ip_from_multiaddr(send_back_addr)
                    }
                };
                
                // Create peer info
                let is_inbound = matches!(endpoint, ConnectedPoint::Listener { .. });
                let peer_info = PeerInfo {
                    peer_id: peer_id.clone(),
                    addresses: vec![], // Will be populated later
                    state: PeerState::Connected,
                    is_inbound,
                    protocol_version: 1,
                    user_agent: "supernova/1.0.0".to_string(),
                    height: 0,
                    best_hash: None,
                    ping_ms: None,
                    last_seen: Instant::now(),
                    reputation: 100,
                };
                
                // Store peer info
                connected_peers.write().await.insert(peer_id.clone(), peer_info.clone());
                
                // Notify about the new peer
                let _ = event_sender.send(NetworkEvent::PeerConnected(peer_info)).await;
                
                // Update statistics
                let mut stats_guard = stats.write().await;
                stats_guard.peers_connected += 1;
                if endpoint.is_dialer() {
                    stats_guard.outbound_connections += 1;
                } else {
                    stats_guard.inbound_connections += 1;
                }
                stats_guard.connection_attempts += 1;
            }
            SwarmEvent::ConnectionClosed { 
                peer_id, 
                cause, 
                ..
            } => {
                info!("Disconnected from {}: {:?}", peer_id, cause);
                
                // Remove peer info
                connected_peers.write().await.remove(&peer_id);
                
                // Notify about the disconnected peer
                let _ = event_sender.send(NetworkEvent::PeerDisconnected(peer_id)).await;
                
                // Update statistics
                let mut stats_guard = stats.write().await;
                stats_guard.peers_connected = stats_guard.peers_connected.saturating_sub(1);
            }
            SwarmEvent::OutgoingConnectionError { 
                peer_id, 
                error, 
                ..
            } => {
                if let Some(peer_id) = peer_id {
                    warn!("Failed to connect to {}: {}", peer_id, error);
                } else {
                    warn!("Failed to connect: {}", error);
                }
                
                let _ = event_sender.send(NetworkEvent::Error {
                    peer_id,
                    error: format!("Connection failed: {}", error),
                }).await;
            }
            // Other events would be handled in a full implementation
            _ => {}
        }
    }
    
    /// Handle a protocol message (static version)
    async fn handle_protocol_message_static(
        peer_id: &PeerId,
        message: Message,
        event_sender: &mpsc::Sender<NetworkEvent>,
        stats: &Arc<RwLock<NetworkStats>>,
    ) {
        match message {
            Message::NewBlock { block_data, height, total_difficulty } => {
                debug!("Received new block at height {} from {}", height, peer_id);
                
                let mut stats_guard = stats.write().await;
                stats_guard.blocks_received += 1;
                drop(stats_guard);
                
                // Try to deserialize the block
                match bincode::deserialize::<Block>(&block_data) {
                    Ok(block) => {
                        // Notify about the new block
                        let _ = event_sender.send(NetworkEvent::NewBlock {
                            block,
                            height,
                            total_difficulty,
                            from_peer: Some(peer_id.clone()),
                        }).await;
                    }
                    Err(e) => {
                        warn!("Failed to deserialize block from {}: {}", peer_id, e);
                        let mut stats_guard = stats.write().await;
                        stats_guard.invalid_messages += 1;
                    }
                }
            }
            Message::Transaction { transaction } => {
                debug!("Received transaction from {}", peer_id);
                
                let mut stats_guard = stats.write().await;
                stats_guard.transactions_announced += 1;
                drop(stats_guard);
                
                // Try to deserialize the transaction
                match bincode::deserialize::<Transaction>(&transaction) {
                    Ok(tx) => {
                        // Notify about the new transaction
                        let _ = event_sender.send(NetworkEvent::NewTransaction {
                            transaction: tx,
                            fee_rate: 0, // Would be calculated in a real implementation
                            from_peer: Some(peer_id.clone()),
                        }).await;
                    }
                    Err(e) => {
                        warn!("Failed to deserialize transaction from {}: {}", peer_id, e);
                        let mut stats_guard = stats.write().await;
                        stats_guard.invalid_messages += 1;
                    }
                }
            }
            Message::Headers { headers, total_difficulty } => {
                debug!("Received {} headers from {}", headers.len(), peer_id);
                
                let mut stats_guard = stats.write().await;
                stats_guard.headers_received += 1;
                drop(stats_guard);
                
                // Try to deserialize the headers
                let mut deserialized_headers = Vec::new();
                for header_data in headers {
                    match bincode::deserialize::<BlockHeader>(&header_data) {
                        Ok(header) => {
                            deserialized_headers.push(header);
                        }
                        Err(e) => {
                            warn!("Failed to deserialize header from {}: {}", peer_id, e);
                            let mut stats_guard = stats.write().await;
                            stats_guard.invalid_messages += 1;
                        }
                    }
                }
                
                // Notify about the headers
                if !deserialized_headers.is_empty() {
                    let _ = event_sender.send(NetworkEvent::BlockHeaders {
                        headers: deserialized_headers,
                        total_difficulty,
                        from_peer: Some(peer_id.clone()),
                    }).await;
                }
            }
            Message::Status { version, height, best_hash, total_difficulty, head_timestamp } => {
                debug!("Received status from {}: height={}, total_difficulty={}", 
                      peer_id, height, total_difficulty);
                
                // Notify about the peer status
                let _ = event_sender.send(NetworkEvent::PeerStatus {
                    peer_id: peer_id.clone(),
                    version,
                    height,
                    best_hash,
                    total_difficulty,
                }).await;
            }
            _ => {
                // Handle other message types
                debug!("Received message from {}: {:?}", peer_id, message);
                
                let _ = event_sender.send(NetworkEvent::MessageReceived {
                    peer_id: peer_id.clone(),
                    message,
                }).await;
            }
        }
    }
    
    /// Get network statistics
    pub async fn get_stats(&self) -> NetworkStats {
        self.stats.read().await.clone()
    }
    
    /// Get peer count
    pub async fn get_peer_count(&self) -> usize {
        self.connected_peers.read().await.len()
    }
    
    /// Stop the network
    pub async fn stop(&self) -> Result<(), Box<dyn Error>> {
        let mut running = self.running.write().await;
        *running = false;
        
        // Stop the network task
        if let Some(task) = self.network_task.write().await.take() {
            task.abort();
        }
        
        // Clear the swarm
        *self.swarm.write().await = None;
        
        // Send stopped event
        let _ = self.event_sender.send(NetworkEvent::Stopped).await;
        
        info!("P2P network stopped");
        Ok(())
    }
    
    /// Set challenge difficulty for Sybil protection
    pub fn set_challenge_difficulty(&mut self, difficulty: u8) {
        if difficulty > 0 && difficulty <= 24 {
            self.challenge_difficulty = difficulty;
            info!("Identity verification challenge difficulty set to {}", difficulty);
        } else {
            warn!("Invalid challenge difficulty: {}, keeping current setting: {}", 
                 difficulty, self.challenge_difficulty);
        }
    }
    
    /// Enable or disable identity verification requirement
    pub fn set_require_verification(&mut self, require: bool) {
        self.require_verification = require;
        info!("Identity verification requirement {}", if require { "enabled" } else { "disabled" });
    }
    
    /// Generate a new identity verification challenge for a peer
    pub async fn generate_challenge(&self, peer_id: &PeerId) -> IdentityChallenge {
        // Generate 32 bytes of random data
        let mut challenge_bytes = [0u8; 32];
        OsRng.fill_bytes(&mut challenge_bytes);
        
        // Create challenge
        let challenge = IdentityChallenge {
            challenge: challenge_bytes.to_vec(),
            difficulty: self.challenge_difficulty,
            issued_at: Instant::now(),
            timeout: Duration::from_secs(CHALLENGE_TIMEOUT_SECS),
        };
        
        // Store challenge
        self.identity_challenges.write().await.insert(peer_id.clone(), challenge.clone());
        
        // Update verification status
        self.verification_status.write().await.insert(
            peer_id.clone(), 
            IdentityVerificationStatus::ChallengeIssued(Instant::now())
        );
        
        debug!("Generated identity challenge for peer {}", peer_id);
        challenge
    }
    
    /// Verify a challenge response
    pub async fn verify_challenge(&self, peer_id: &PeerId, solution: &[u8]) -> bool {
        // Get the challenge
        let challenge = {
            let challenges = self.identity_challenges.read().await;
            challenges.get(peer_id).cloned()
        };
        
        let challenge = match challenge {
            Some(c) => c,
            None => {
                warn!("No challenge found for peer {}", peer_id);
                return false;
            }
        };
        
        // Check if challenge has expired
        if challenge.issued_at.elapsed() > challenge.timeout {
            warn!("Challenge for peer {} has expired", peer_id);
            self.verification_status.write().await.insert(
                peer_id.clone(),
                IdentityVerificationStatus::VerificationFailed("Challenge expired".to_string())
            );
            return false;
        }
        
        // Verify the solution (hash of challenge + solution should have required leading zeros)
        let mut hasher = Sha256::new();
        hasher.update(&challenge.challenge);
        hasher.update(solution);
        let hash = hasher.finalize();
        
        // Count leading zero bits
        let leading_zeros = count_leading_zero_bits(&hash);
        let success = leading_zeros >= challenge.difficulty;
        
        // Update verification status
        if success {
            debug!("Peer {} passed identity verification challenge", peer_id);
            self.verification_status.write().await.insert(
                peer_id.clone(),
                IdentityVerificationStatus::Verified(Instant::now())
            );
            
            // Remove challenge
            self.identity_challenges.write().await.remove(peer_id);
        } else {
            warn!("Peer {} failed identity verification challenge", peer_id);
            self.verification_status.write().await.insert(
                peer_id.clone(),
                IdentityVerificationStatus::VerificationFailed(
                    format!("Insufficient difficulty: got {} bits, required {}", 
                           leading_zeros, challenge.difficulty)
                )
            );
        }
        
        success
    }
    
    /// Check if a peer has been verified
    pub async fn is_peer_verified(&self, peer_id: &PeerId) -> bool {
        let status = self.verification_status.read().await;
        match status.get(peer_id) {
            Some(IdentityVerificationStatus::Verified(_)) => true,
            _ => false,
        }
    }

    /// Get network information for API
    pub async fn get_network_info(&self) -> Result<NetworkInfo, Box<dyn Error>> {
        let stats = self.get_stats().await;
        
        // Get listening addresses from swarm
        let listening_addresses = {
            let swarm_guard = self.swarm.read().await;
            if let Some(swarm) = swarm_guard.as_ref() {
                swarm.listeners().cloned().map(|addr| addr.to_string()).collect()
            } else {
                vec![]
            }
        };
        
        Ok(NetworkInfo {
            network_id: self.network_id.clone(),
            local_peer_id: self.local_peer_id.to_string(),
            listening_addresses,
            connected_peers: stats.peers_connected,
            inbound_connections: stats.inbound_connections,
            outbound_connections: stats.outbound_connections,
            total_bytes_sent: {
                let bandwidth = self.bandwidth_tracker.read().await;
                bandwidth.bytes_sent
            },
            total_bytes_received: {
                let bandwidth = self.bandwidth_tracker.read().await;
                bandwidth.bytes_received
            },
            uptime_seconds: {
                let bandwidth = self.bandwidth_tracker.read().await;
                bandwidth.start_time.map(|t| t.elapsed().as_secs()).unwrap_or(0)
            },
            version: "1.0.0".to_string(),
            protocol_version: 1,
        })
    }
    
    /// Get connection count for API
    pub async fn get_connection_count(&self) -> Result<ConnectionCount, Box<dyn Error>> {
        let stats = self.get_stats().await;
        
        Ok(ConnectionCount {
            total: stats.inbound_connections + stats.outbound_connections,
            inbound: stats.inbound_connections,
            outbound: stats.outbound_connections,
        })
    }
    
    /// Get peers for API
    pub async fn get_peers(&self, connection_state: Option<String>, verbose: bool) -> Result<Vec<ApiPeerInfo>, Box<dyn Error>> {
        let connected_peers = self.connected_peers.read().await;
        let mut api_peers = Vec::new();
        
        for (peer_id, peer_info) in connected_peers.iter() {
            // Filter by connection state if specified
            if let Some(ref state_filter) = connection_state {
                let peer_state = match peer_info.state {
                    PeerState::Connected => "connected",
                    PeerState::Ready => "ready",
                    PeerState::Disconnected => "disconnected",
                    PeerState::Banned => "banned",
                    PeerState::Dialing => "dialing",
                };
                
                if peer_state != state_filter {
                    continue;
                }
            }
            
            let api_peer = ApiPeerInfo {
                peer_id: peer_info.peer_id.to_string(),
                addresses: peer_info.addresses.iter().map(|a| a.to_string()).collect(),
                connection_status: match peer_info.state {
                    PeerState::Connected => crate::api::types::PeerConnectionStatus::Connected,
                    PeerState::Ready => crate::api::types::PeerConnectionStatus::Connected,
                    PeerState::Disconnected => crate::api::types::PeerConnectionStatus::Disconnected,
                    PeerState::Banned => crate::api::types::PeerConnectionStatus::Banned,
                    PeerState::Dialing => crate::api::types::PeerConnectionStatus::Connecting,
                },
                direction: if peer_info.is_inbound { "inbound".to_string() } else { "outbound".to_string() },
                protocol_version: peer_info.protocol_version,
                user_agent: peer_info.user_agent.clone(),
                height: peer_info.height,
                best_hash: peer_info.best_hash.map(|h| hex::encode(h)),
                ping_ms: peer_info.ping_ms,
                bytes_sent: 0, // TODO: Track actual bytes per peer
                bytes_received: 0, // TODO: Track actual bytes per peer
                last_seen: peer_info.last_seen.elapsed().as_secs(),
                reputation: peer_info.reputation,
                ban_score: 0, // TODO: Implement ban scoring
            };
            
            api_peers.push(api_peer);
        }
        
        Ok(api_peers)
    }
    
    /// Get specific peer for API
    pub async fn get_peer(&self, peer_id: &str) -> Result<Option<ApiPeerInfo>, Box<dyn Error>> {
        // Parse peer ID
        let peer_id = peer_id.parse::<PeerId>()
            .map_err(|e| format!("Invalid peer ID: {}", e))?;
        
        let connected_peers = self.connected_peers.read().await;
        if let Some(peer_info) = connected_peers.get(&peer_id) {
            let api_peer = ApiPeerInfo {
                peer_id: peer_info.peer_id.to_string(),
                addresses: peer_info.addresses.iter().map(|a| a.to_string()).collect(),
                connection_status: match peer_info.state {
                    PeerState::Connected => crate::api::types::PeerConnectionStatus::Connected,
                    PeerState::Ready => crate::api::types::PeerConnectionStatus::Connected,
                    PeerState::Disconnected => crate::api::types::PeerConnectionStatus::Disconnected,
                    PeerState::Banned => crate::api::types::PeerConnectionStatus::Banned,
                    PeerState::Dialing => crate::api::types::PeerConnectionStatus::Connecting,
                },
                direction: if peer_info.is_inbound { "inbound".to_string() } else { "outbound".to_string() },
                protocol_version: peer_info.protocol_version,
                user_agent: peer_info.user_agent.clone(),
                height: peer_info.height,
                best_hash: peer_info.best_hash.map(|h| hex::encode(h)),
                ping_ms: peer_info.ping_ms,
                bytes_sent: 0, // TODO: Track actual bytes per peer
                bytes_received: 0, // TODO: Track actual bytes per peer
                last_seen: peer_info.last_seen.elapsed().as_secs(),
                reputation: peer_info.reputation,
                ban_score: 0, // TODO: Implement ban scoring
            };
            
            Ok(Some(api_peer))
        } else {
            Ok(None)
        }
    }
    
    /// Add peer for API
    pub async fn add_peer(&self, address: &str, permanent: bool) -> Result<PeerAddResponse, Box<dyn Error>> {
        // Parse the multiaddress
        let addr: Multiaddr = address.parse()
            .map_err(|e| format!("Invalid address format: {}", e))?;
        
        // Attempt to dial the peer
        let mut swarm_guard = self.swarm.write().await;
        if let Some(swarm) = swarm_guard.as_mut() {
            match swarm.dial(addr.clone()) {
                Ok(_) => {
                    debug!("Initiated connection to {}", address);
                    
                    // Update stats
                    let mut stats = self.stats.write().await;
                    stats.connection_attempts += 1;
                    
                    Ok(PeerAddResponse {
                        success: true,
                        message: format!("Connection initiated to {}", address),
                        peer_id: None, // Will be filled when connection is established
                    })
                }
                Err(e) => {
                    warn!("Failed to dial {}: {}", address, e);
                    Ok(PeerAddResponse {
                        success: false,
                        message: format!("Failed to initiate connection: {}", e),
                        peer_id: None,
                    })
                }
            }
        } else {
            Ok(PeerAddResponse {
                success: false,
                message: "Network not initialized".to_string(),
                peer_id: None,
            })
        }
    }
    
    /// Remove peer for API
    pub async fn remove_peer(&self, peer_id: &str) -> Result<bool, Box<dyn Error>> {
        // Parse peer ID
        let peer_id = peer_id.parse::<PeerId>()
            .map_err(|e| format!("Invalid peer ID: {}", e))?;
        
        // Check if peer exists
        let peer_exists = self.connected_peers.read().await.contains_key(&peer_id);
        
        if peer_exists {
            // Disconnect from the peer
            let mut swarm_guard = self.swarm.write().await;
            if let Some(swarm) = swarm_guard.as_mut() {
                // In a full implementation, we would have a way to disconnect specific peers
                // For now, we'll just remove them from our tracking
                debug!("Disconnecting from peer {}", peer_id);
            }
            
            // Remove from connected peers
            self.connected_peers.write().await.remove(&peer_id);
            
            Ok(true)
        } else {
            Ok(false)
        }
    }
    
    /// Get bandwidth usage for API
    pub async fn get_bandwidth_usage(&self, period: u64) -> Result<BandwidthUsage, Box<dyn Error>> {
        let bandwidth = self.bandwidth_tracker.read().await;
        let (upload_rate, download_rate) = bandwidth.get_rates(period);
        
        Ok(BandwidthUsage {
            period_seconds: period,
            bytes_sent: bandwidth.bytes_sent,
            bytes_received: bandwidth.bytes_received,
            total_bytes: bandwidth.bytes_sent + bandwidth.bytes_received,
            upload_rate,
            download_rate,
        })
    }
    
    /// Ban a peer for misbehavior
    pub async fn ban_peer(&self, peer_id: &PeerId, reason: &str, duration: Option<Duration>) {
        let ban_until = Instant::now() + duration.unwrap_or(BAN_DURATION);
        
        // Add to banned peers
        self.banned_peers.write().await.insert(peer_id.clone(), ban_until);
        
        // Remove from connected peers
        self.connected_peers.write().await.remove(peer_id);
        
        // Update stats
        let mut stats = self.stats.write().await;
        stats.peers_banned += 1;
        
        warn!("Banned peer {} for {}: duration {:?}", peer_id, reason, duration);
    }
    
    /// Check if a peer is banned
    pub async fn is_peer_banned(&self, peer_id: &PeerId) -> bool {
        let banned_peers = self.banned_peers.read().await;
        if let Some(ban_until) = banned_peers.get(peer_id) {
            Instant::now() < *ban_until
        } else {
            false
        }
    }
    
    /// Clean up expired bans
    pub async fn cleanup_expired_bans(&self) {
        let now = Instant::now();
        let mut banned_peers = self.banned_peers.write().await;
        banned_peers.retain(|_, ban_until| now < *ban_until);
    }
    
    /// Get connected peer IDs
    pub async fn get_connected_peer_ids(&self) -> Vec<PeerId> {
        self.connected_peers.read().await.keys().cloned().collect()
    }
    
    /// Update peer information
    pub async fn update_peer_info(&self, peer_id: &PeerId, height: u64, best_hash: [u8; 32]) {
        let mut connected_peers = self.connected_peers.write().await;
        if let Some(peer_info) = connected_peers.get_mut(peer_id) {
            peer_info.height = height;
            peer_info.best_hash = Some(best_hash);
            peer_info.last_seen = Instant::now();
        }
    }
    
    /// Send ping to all connected peers
    pub async fn ping_peers(&self) {
        let peer_ids: Vec<PeerId> = self.connected_peers.read().await.keys().cloned().collect();
        
        for peer_id in peer_ids {
            let ping_message = Message::Ping(rand::random::<u64>());
            
            // Send ping message
            Self::send_to_peer_static(
                peer_id,
                ping_message,
                &self.swarm,
                &self.stats,
                &self.bandwidth_tracker,
            ).await;
        }
    }
    
    /// Handle incoming ping
    pub async fn handle_ping(&self, peer_id: &PeerId, nonce: u64) {
        let pong_message = Message::Pong(nonce);
        
        // Send pong response
        Self::send_to_peer_static(
            peer_id.clone(),
            pong_message,
            &self.swarm,
            &self.stats,
            &self.bandwidth_tracker,
        ).await;
    }
    
    /// Handle incoming pong
    pub async fn handle_pong(&self, peer_id: &PeerId, nonce: u64) {
        // Update peer latency information
        // In a full implementation, we would track ping times
        debug!("Received pong from {} with nonce {}", peer_id, nonce);
        
        // Update last seen time
        let mut connected_peers = self.connected_peers.write().await;
        if let Some(peer_info) = connected_peers.get_mut(peer_id) {
            peer_info.last_seen = Instant::now();
            // TODO: Calculate and store ping time
        }
    }
    
    /// Request blocks from peers
    pub async fn request_blocks(&self, block_hashes: Vec<[u8; 32]>, preferred_peer: Option<PeerId>) {
        let message = Message::GetBlocks(block_hashes);
        
        if let Some(peer_id) = preferred_peer {
            // Send to specific peer
            Self::send_to_peer_static(
                peer_id,
                message,
                &self.swarm,
                &self.stats,
                &self.bandwidth_tracker,
            ).await;
        } else {
            // Broadcast to all peers
            Self::broadcast_message_static(
                message,
                &self.swarm,
                &self.stats,
                &self.bandwidth_tracker,
            ).await;
        }
    }
    
    /// Request headers from peers
    pub async fn request_headers(&self, start_height: u64, end_height: u64, preferred_peer: Option<PeerId>) {
        let message = Message::GetHeaders {
            start_height,
            end_height,
        };
        
        if let Some(peer_id) = preferred_peer {
            // Send to specific peer
            Self::send_to_peer_static(
                peer_id,
                message,
                &self.swarm,
                &self.stats,
                &self.bandwidth_tracker,
            ).await;
        } else {
            // Broadcast to all peers
            Self::broadcast_message_static(
                message,
                &self.swarm,
                &self.stats,
                &self.bandwidth_tracker,
            ).await;
        }
    }
    
    /// Announce our status to the network
    pub async fn announce_status(&self, version: u32, height: u64, best_hash: [u8; 32], total_difficulty: u64) {
        let message = Message::Status {
            version,
            height,
            best_hash,
            total_difficulty,
            head_timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        };
        
        Self::broadcast_message_static(
            message,
            &self.swarm,
            &self.stats,
            &self.bandwidth_tracker,
        ).await;
    }
    
    /// Get network health metrics
    pub async fn get_network_health(&self) -> NetworkHealth {
        let stats = self.get_stats().await;
        let connected_peers = self.connected_peers.read().await;
        let banned_peers = self.banned_peers.read().await;
        
        NetworkHealth {
            connected_peers: stats.peers_connected,
            banned_peers: banned_peers.len(),
            message_success_rate: if stats.messages_sent > 0 {
                (stats.messages_received as f64) / (stats.messages_sent as f64)
            } else {
                0.0
            },
            average_latency_ms: stats.avg_latency_ms,
            network_diversity: self.calculate_network_diversity(&connected_peers).await,
        }
    }
    
    /// Calculate network diversity score
    async fn calculate_network_diversity(&self, peers: &HashMap<PeerId, PeerInfo>) -> f64 {
        // Simple diversity calculation based on unique IP subnets
        let mut subnets = HashSet::new();
        
        for peer in peers.values() {
            for addr in &peer.addresses {
                if let Ok(socket_addr) = addr.to_string().parse::<std::net::SocketAddr>() {
                    let ip = socket_addr.ip();
                    match ip {
                        std::net::IpAddr::V4(ipv4) => {
                            // Use /24 subnet for IPv4
                            let subnet = ipv4.octets()[0..3].to_vec();
                            subnets.insert(subnet);
                        }
                        std::net::IpAddr::V6(ipv6) => {
                            // Use /64 subnet for IPv6
                            let subnet = ipv6.octets()[0..8].to_vec();
                            subnets.insert(subnet);
                        }
                    }
                }
            }
        }
        
        if peers.is_empty() {
            0.0
        } else {
            subnets.len() as f64 / peers.len() as f64
        }
    }
    
    /// Perform peer rotation for eclipse attack prevention
    pub async fn perform_peer_rotation(&self) -> Result<(), Box<dyn Error>> {
        info!("Starting peer rotation for eclipse prevention");
        
        // Check if rotation is needed
        if !self.eclipse_prevention.check_rotation_needed().await {
            debug!("Peer rotation not needed at this time");
            return Ok(());
        }
        
        // Get peers to disconnect
        let rotation_candidates = self.eclipse_prevention.get_rotation_candidates().await;
        info!("Rotating {} peers", rotation_candidates.len());
        
        // Disconnect selected peers
        for peer_id in rotation_candidates {
            info!("Disconnecting peer {} for rotation", peer_id);
            self.command_receiver.write().await.send(NetworkCommand::DisconnectPeer(peer_id))?;
        }
        
        // Connect to new diverse peers
        // In production, this would use peer discovery to find new peers
        // that improve network diversity
        
        Ok(())
    }
    
    /// Get eclipse attack risk level
    pub async fn get_eclipse_risk_level(&self) -> EclipseRiskLevel {
        self.eclipse_prevention.get_eclipse_risk_level().await
    }
    
    /// Handle identity challenge response
    pub async fn handle_identity_challenge_response(&self, peer_id: &PeerId, solution: &[u8]) -> bool {
        self.eclipse_prevention.verify_pow_challenge(peer_id, solution).await
    }
    
    /// Update peer behavior score based on actions
    pub async fn update_peer_behavior(&self, peer_id: &PeerId, delta: f64) {
        self.eclipse_prevention.update_behavior_score(peer_id, delta).await;
    }
    
    /// Record peer advertisements for eclipse detection
    pub async fn record_peer_advertisements(&self, from_peer: PeerId, advertised_peers: Vec<PeerId>) {
        self.eclipse_prevention.record_peer_advertisement(from_peer, advertised_peers).await;
    }
    
    /// Check if an incoming connection should be allowed
    pub async fn should_allow_connection(&self, socket_addr: std::net::SocketAddr) -> bool {
        // Check rate limiting
        match self.rate_limiter.check_connection(socket_addr).await {
            Ok(permit) => {
                // Connection allowed by rate limiter
                debug!("Connection from {} passed rate limiting", socket_addr);
                // Record success
                permit.record_success();
                true
            }
            Err(e) => {
                // Connection rejected by rate limiter
                warn!("Connection from {} rejected by rate limiter: {}", socket_addr, e);
                self.rate_limiter.record_failure();
                false
            }
        }
    }
    
    /// Get rate limiter metrics
    pub fn get_rate_limiter_metrics(&self) -> RateLimitMetrics {
        self.rate_limiter.metrics()
    }
    
    /// Configure rate limiting
    pub async fn configure_rate_limiting(&mut self, config: RateLimitConfig) {
        self.rate_limiter = Arc::new(NetworkRateLimiter::new(config));
    }
}

/// Network health metrics
#[derive(Debug, Clone)]
pub struct NetworkHealth {
    pub connected_peers: usize,
    pub banned_peers: usize,
    pub message_success_rate: f64,
    pub average_latency_ms: f64,
    pub network_diversity: f64,
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
        .upgrade(upgrade::Version::V1)
        .authenticate(noise::NoiseConfig::xx(noise_keys).into_authenticated())
        .multiplex(yamux::YamuxConfig::default())
        .timeout(Duration::from_secs(20))
        .boxed())
}

/// Count the number of leading zero bits in a hash
fn count_leading_zero_bits(hash: &[u8]) -> u8 {
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

/// Extract IP address from multiaddr
fn extract_ip_from_multiaddr(addr: &Multiaddr) -> Option<IpAddr> {
    use libp2p::multiaddr::Protocol;
    
    for proto in addr.iter() {
        match proto {
            Protocol::Ip4(ip) => return Some(IpAddr::V4(ip)),
            Protocol::Ip6(ip) => return Some(IpAddr::V6(ip)),
            _ => {}
        }
    }
    None
}

/// Extract socket address from multiaddr
fn extract_socket_addr_from_multiaddr(addr: &Multiaddr) -> Option<std::net::SocketAddr> {
    use libp2p::multiaddr::Protocol;
    
    let mut ip_addr = None;
    let mut port = None;
    
    for proto in addr.iter() {
        match proto {
            Protocol::Ip4(ip) => ip_addr = Some(IpAddr::V4(ip)),
            Protocol::Ip6(ip) => ip_addr = Some(IpAddr::V6(ip)),
            Protocol::Tcp(p) => port = Some(p),
            _ => {}
        }
    }
    
    match (ip_addr, port) {
        (Some(ip), Some(p)) => Some(std::net::SocketAddr::new(ip, p)),
        _ => None,
    }
}

/// Solve a proof-of-work challenge with required difficulty
fn solve_pow_challenge(challenge: &[u8], difficulty: u8) -> Vec<u8> {
    let mut nonce = 0u64;
    let mut solution = Vec::new();
    
    loop {
        // Convert nonce to bytes
        let mut nonce_bytes = [0u8; 8];
        BigEndian::write_u64(&mut nonce_bytes, nonce);
        
        // Hash challenge + nonce
        let mut hasher = Sha256::new();
        hasher.update(challenge);
        hasher.update(&nonce_bytes);
        let hash = hasher.finalize();
        
        // Check if it meets difficulty
        if count_leading_zero_bits(&hash) >= difficulty {
            solution = nonce_bytes.to_vec();
            break;
        }
        
        nonce += 1;
    }
    
    solution
}

// Implement a simple constructor for P2PNetwork to satisfy the Node struct
impl P2PNetwork {
    pub fn new() -> Self {
        // This is a simplified constructor for compatibility
        // In practice, you should use the async `new` method
        let (event_sender, _) = mpsc::channel(1);
        
        Self {
            swarm: Arc::new(RwLock::new(None)),
            local_peer_id: PeerId::random(),
            protocol: Protocol::new(identity::Keypair::generate_ed25519()).unwrap(),
            command_receiver: Arc::new(RwLock::new(None)),
            event_sender,
            peer_manager: Arc::new(PeerManager::new()),
            connection_manager: Arc::new(RwLock::new(ConnectionManager::new(
                Arc::new(PeerManager::new()),
                Arc::new(PeerDiversityManager::with_config(0.6, ConnectionStrategy::BalancedDiversity, 10)),
                MAX_INBOUND_CONNECTIONS,
                MAX_OUTBOUND_CONNECTIONS,
            ))),
            diversity_manager: Arc::new(PeerDiversityManager::with_config(0.6, ConnectionStrategy::BalancedDiversity, 10)),
            eclipse_prevention: Arc::new(EclipsePreventionSystem::new(EclipsePreventionConfig::default())),
            rate_limiter: Arc::new(NetworkRateLimiter::new(RateLimitConfig::default())),
            message_handler: MessageHandler::new(),
            discovery: Arc::new(RwLock::new(None)),
            stats: Arc::new(RwLock::new(NetworkStats::default())),
            genesis_hash: [0; 32],
            network_id: "supernova".to_string(),
            bootstrap_nodes: Vec::new(),
            trusted_peers: Arc::new(RwLock::new(HashSet::new())),
            network_task: Arc::new(RwLock::new(None)),
            running: Arc::new(RwLock::new(false)),
            identity_challenges: Arc::new(RwLock::new(HashMap::new())),
            verification_status: Arc::new(RwLock::new(HashMap::new())),
            challenge_difficulty: DEFAULT_CHALLENGE_DIFFICULTY,
            require_verification: true,
            connected_peers: Arc::new(RwLock::new(HashMap::new())),
            banned_peers: Arc::new(RwLock::new(HashMap::new())),
            message_routes: Arc::new(RwLock::new(HashMap::new())),
            bandwidth_tracker: Arc::new(RwLock::new(BandwidthTracker::new())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    // A basic test for network creation
    #[tokio::test]
    async fn test_network_creation() {
        let (network, _, _) = P2PNetwork::new(
            None, 
            [0u8; 32], 
            "supernova-test"
        ).await.unwrap();
        
        let stats = network.get_stats().await;
        assert_eq!(stats.peers_connected, 0);
    }
    
    #[test]
    fn test_leading_zero_bits() {
        // All zeros should have 8 leading zero bits
        assert_eq!(count_leading_zero_bits(&[0]), 8);
        
        // 0x80 should have 0 leading zero bits
        assert_eq!(count_leading_zero_bits(&[0x80]), 0);
        
        // 0x40 should have 1 leading zero bit
        assert_eq!(count_leading_zero_bits(&[0x40]), 1);
        
        // 0x20 should have 2 leading zero bits
        assert_eq!(count_leading_zero_bits(&[0x20]), 2);
        
        // 0x01 should have 7 leading zero bits
        assert_eq!(count_leading_zero_bits(&[0x01]), 7);
        
        // 0x00, 0x80 should have 8 leading zero bits
        assert_eq!(count_leading_zero_bits(&[0x00, 0x80]), 8);
        
        // 0x00, 0x00, 0x00, 0x01 should have 31 leading zero bits
        assert_eq!(count_leading_zero_bits(&[0x00, 0x00, 0x00, 0x01]), 31);
    }
    
    #[test]
    fn test_solve_pow_challenge() {
        // Test with a low difficulty for quick testing
        let challenge = b"test challenge";
        let difficulty = 8; // Require 8 leading zero bits
        
        let solution = solve_pow_challenge(challenge, difficulty);
        
        // Verify solution
        let mut hasher = Sha256::new();
        hasher.update(challenge);
        hasher.update(&solution);
        let hash = hasher.finalize();
        
        let leading_zeros = count_leading_zero_bits(&hash);
        assert!(leading_zeros >= difficulty, 
               "Solution doesn't meet difficulty: got {} bits, required {}", 
               leading_zeros, difficulty);
    }
    
    #[tokio::test]
    async fn test_peer_management() {
        let (network, _, _) = P2PNetwork::new(
            None, 
            [0u8; 32], 
            "supernova-test"
        ).await.unwrap();
        
        let peer_id = PeerId::random();
        
        // Test peer banning
        network.ban_peer(&peer_id, "test ban", Some(Duration::from_secs(60))).await;
        assert!(network.is_peer_banned(&peer_id).await);
        
        // Test trusted peer management
        network.add_trusted_peer(peer_id.clone()).await;
        let trusted = network.trusted_peers.read().await;
        assert!(trusted.contains(&peer_id));
    }
    
    #[tokio::test]
    async fn test_bandwidth_tracking() {
        let mut tracker = BandwidthTracker::new();
        
        tracker.record_sent(1024);
        tracker.record_received(2048);
        
        assert_eq!(tracker.bytes_sent, 1024);
        assert_eq!(tracker.bytes_received, 2048);
        assert_eq!(tracker.messages_sent, 1);
        assert_eq!(tracker.messages_received, 1);
        
        let (send_rate, recv_rate) = tracker.get_rates(1);
        assert!(send_rate > 0.0);
        assert!(recv_rate > 0.0);
    }
}