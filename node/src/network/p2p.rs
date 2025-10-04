// This file intentionally left blank to be rewritten from scratch

use crate::{
    api::types::{BandwidthUsage, ConnectionCount, NetworkInfo, PeerAddResponse},
    network::{
        behaviour::{SupernovaBehaviour, SupernovaBehaviourEvent},
        discovery::PeerDiscovery,
        eclipse_prevention::EclipseRiskLevel,
        identity_verification::IdentityVerificationSystem,
        peer::{self, PeerInfo, PeerState},
        peer_manager::{ConnectionLimits, PeerManager},
        protocol::Message,
        rate_limiter::{NetworkRateLimiter as RateLimiter, RateLimitConfig, RateLimitMetrics},
    },
};
use btclib::{Block, BlockHeader, Transaction};
use futures::StreamExt;
use libp2p::{
    core::transport::Transport as CoreTransport,
    gossipsub::{self, Behaviour as Gossipsub, TopicHash},
    identify::{self, Behaviour as Identify},
    identity,
    kad::{store::MemoryStore, Behaviour as Kademlia},
    mdns::{self, tokio::Behaviour as Mdns},
    noise,
    swarm::{Swarm, SwarmBuilder, SwarmEvent},
    tcp, yamux, Multiaddr, PeerId, Transport,
};
use sha2::{Digest, Sha256};
use std::{
    collections::{HashMap, HashSet},
    error::Error,
    net::IpAddr,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use tokio::sync::{mpsc, RwLock};
use tokio::task::JoinHandle;
use tracing::{debug, info, warn};

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
    SendToPeer { peer_id: PeerId, message: Message },

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
    MessageReceived { peer_id: PeerId, message: Message },

    /// Message sent to peer
    MessageSent { peer_id: PeerId, message: Message },

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

    /// Peer banned
    PeerBanned(PeerId),

    /// Peer added to trusted list
    PeerAddedToTrusted(PeerId),

    /// Peer removed from trusted list
    PeerRemovedFromTrusted(PeerId),

    /// Peers received in response to GetPeers
    PeersReceived(Vec<PeerId>),
}

/// Commands that can be sent to the swarm thread
#[derive(Debug)]
enum SwarmCommand {
    Dial(Multiaddr),
    Publish(TopicHash, Vec<u8>),
    Stop,
}

/// Wrapper for swarm events that can be sent across threads
#[derive(Debug)]
enum SwarmEventWrapper {
    ConnectionEstablished {
        peer_id: PeerId,
        endpoint: String,
    },
    ConnectionClosed {
        peer_id: PeerId,
    },
    IncomingConnection {
        local_addr: Multiaddr,
    },
    Message {
        peer_id: PeerId,
        topic: String,
        data: Vec<u8>,
    },
    Discovered {
        peers: Vec<PeerId>,
    },
}

impl SwarmEventWrapper {
    /// Convert a SwarmEvent to a wrapper that can be sent across threads
    fn from_event(
        event: SwarmEvent<SupernovaBehaviourEvent, Box<dyn Error + Send>>,
    ) -> Result<Self, Box<dyn Error>> {
        match event {
            SwarmEvent::ConnectionEstablished {
                peer_id, endpoint, ..
            } => Ok(SwarmEventWrapper::ConnectionEstablished {
                peer_id,
                endpoint: endpoint.get_remote_address().to_string(),
            }),
            SwarmEvent::ConnectionClosed { peer_id, .. } => {
                Ok(SwarmEventWrapper::ConnectionClosed { peer_id })
            }
            SwarmEvent::IncomingConnection { local_addr, .. } => {
                Ok(SwarmEventWrapper::IncomingConnection { local_addr })
            }
            SwarmEvent::Behaviour(behaviour_event) => {
                // Handle behaviour-specific events
                match behaviour_event {
                    SupernovaBehaviourEvent::Gossipsub(gossipsub_event) => match gossipsub_event {
                        gossipsub::Event::Message {
                            propagation_source,
                            message,
                            ..
                        } => Ok(SwarmEventWrapper::Message {
                            peer_id: propagation_source,
                            topic: message.topic.to_string(),
                            data: message.data,
                        }),
                        _ => Err("Unhandled gossipsub event".into()),
                    },
                    SupernovaBehaviourEvent::Mdns(mdns_event) => match mdns_event {
                        mdns::Event::Discovered(list) => {
                            let peers: Vec<PeerId> =
                                list.into_iter().map(|(peer_id, _)| peer_id).collect();
                            Ok(SwarmEventWrapper::Discovered { peers })
                        }
                        _ => Err("Unhandled mdns event".into()),
                    },
                    _ => Err("Unhandled behaviour event".into()),
                }
            }
            _ => Err("Unhandled swarm event".into()),
        }
    }
}

/// P2P network implementation
pub struct P2PNetwork {
    /// Local peer ID
    local_peer_id: PeerId,
    /// Persistent keypair for this node
    keypair: identity::Keypair,
    /// The libp2p swarm
    swarm: Arc<RwLock<Option<Swarm<SupernovaBehaviour>>>>,
    /// Channel to send commands to the swarm thread
    swarm_cmd_tx: Arc<RwLock<Option<mpsc::Sender<SwarmCommand>>>>,
    /// Bootstrap nodes
    bootstrap_nodes: Vec<Multiaddr>,
    /// Network event sender
    event_sender: mpsc::Sender<NetworkEvent>,
    /// Command receiver
    command_receiver: Arc<RwLock<Option<mpsc::Receiver<NetworkCommand>>>>,
    /// Network statistics
    stats: Arc<RwLock<NetworkStats>>,
    /// Running flag
    running: Arc<RwLock<bool>>,
    /// Connected peers
    connected_peers: Arc<RwLock<HashMap<PeerId, PeerInfo>>>,
    /// Peer discovery
    discovery: Arc<RwLock<Option<PeerDiscovery>>>,
    /// Configured listen address (e.g., "0.0.0.0:8333")
    listen_address: String,
    /// Network task handle
    network_task: Arc<RwLock<Option<JoinHandle<()>>>>,
    /// Bandwidth tracker
    bandwidth_tracker: Arc<Mutex<BandwidthTracker>>,
    /// Rate limiter
    rate_limiter: Arc<RateLimiter>,
    /// Banned peers
    banned_peers: Arc<RwLock<HashMap<PeerId, Instant>>>,
    /// Trusted peers
    trusted_peers: Arc<RwLock<HashSet<PeerId>>>,
    /// Network ID
    network_id: String,
    /// Peer manager
    peer_manager: Arc<PeerManager>,
    /// Challenge difficulty for identity verification
    challenge_difficulty: u8,
    /// Whether identity verification is required
    require_identity_verification: bool,
    /// Identity verification system
    identity_system: Arc<IdentityVerificationSystem>,
    /// Storage backend
    storage: Arc<dyn crate::storage::Storage>,
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

    // Bandwidth
    pub bytes_sent: u64,
    pub bytes_received: u64,
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
        listen_addr: Option<String>,
    ) -> Result<
        (
            Self,
            mpsc::Sender<NetworkCommand>,
            mpsc::Receiver<NetworkEvent>,
        ),
        Box<dyn Error>,
    > {
        // Generate keypair if not provided
        let id_keys = keypair.unwrap_or_else(identity::Keypair::generate_ed25519);
        let local_peer_id = PeerId::from(id_keys.public());
        info!("Local peer id: {}", local_peer_id);

        // Create communication channels
        let (command_sender, command_receiver) = mpsc::channel(128);
        let (event_sender, event_receiver) = mpsc::channel(128);

        // Create storage backend
        let storage: Arc<dyn crate::storage::Storage> =
            Arc::new(crate::storage::MemoryStorage::new());

        Ok((
            Self {
                local_peer_id,
                keypair: id_keys, // Store persistent keypair
                swarm: Arc::new(RwLock::new(None)),
                swarm_cmd_tx: Arc::new(RwLock::new(None)),
                bootstrap_nodes: Vec::new(),
                event_sender,
                command_receiver: Arc::new(RwLock::new(Some(command_receiver))),
                stats: Arc::new(RwLock::new(NetworkStats::default())),
                running: Arc::new(RwLock::new(false)),
                connected_peers: Arc::new(RwLock::new(HashMap::new())),
                discovery: Arc::new(RwLock::new(None)),
                listen_address: listen_addr.unwrap_or_else(|| "0.0.0.0:8333".to_string()),
                network_task: Arc::new(RwLock::new(None)),
                bandwidth_tracker: Arc::new(Mutex::new(BandwidthTracker::new())),
                rate_limiter: Arc::new(RateLimiter::new(RateLimitConfig::default())),
                banned_peers: Arc::new(RwLock::new(HashMap::new())),
                trusted_peers: Arc::new(RwLock::new(HashSet::new())),
                network_id: network_id.to_string(),
                peer_manager: Arc::new(PeerManager::new(
                    storage.clone(),
                    ConnectionLimits::default(),
                )),
                challenge_difficulty: DEFAULT_CHALLENGE_DIFFICULTY,
                require_identity_verification: true,
                identity_system: Arc::new(IdentityVerificationSystem::new(
                    DEFAULT_CHALLENGE_DIFFICULTY,
                    true,
                )),
                storage,
            },
            command_sender,
            event_receiver,
        ))
    }
    
    /// Extract port number from listen address configuration
    fn extract_port_from_config(&self) -> Option<u16> {
        // Handle formats: "0.0.0.0:8333", "/ip4/0.0.0.0/tcp/8333", "8333"
        if let Some(port_str) = self.listen_address.split(':').last() {
            port_str.parse::<u16>().ok()
        } else if self.listen_address.contains("/tcp/") {
            self.listen_address
                .split("/tcp/")
                .last()
                .and_then(|s| s.parse::<u16>().ok())
        } else {
            self.listen_address.parse::<u16>().ok()
        }
    }
    
    /// Parse bootstrap peer string to multiaddr
    /// 
    /// Supports formats:
    /// - "12D3KooW...@207.154.213.122:8333" (peer_id@ip:port)
    /// - "/ip4/207.154.213.122/tcp/8333/p2p/12D3KooW..." (full multiaddr)
    /// - "207.154.213.122:8333" (legacy format - will use peer discovery)
    fn parse_bootstrap_peer(peer_str: &str) -> Result<Multiaddr, String> {
        // Try parsing as multiaddr first
        if let Ok(addr) = peer_str.parse::<Multiaddr>() {
            return Ok(addr);
        }
        
        // Try parsing as peer_id@ip:port format
        if peer_str.contains('@') {
            let parts: Vec<&str> = peer_str.split('@').collect();
            if parts.len() == 2 {
                let peer_id_str = parts[0];
                let socket_addr = parts[1];
                
                // Parse peer ID
                let peer_id = peer_id_str.parse::<PeerId>()
                    .map_err(|e| format!("Invalid peer ID: {}", e))?;
                
                // Parse socket address
                if let Some(colon_pos) = socket_addr.rfind(':') {
                    let ip = &socket_addr[..colon_pos];
                    let port = &socket_addr[colon_pos + 1..];
                    
                    // Build multiaddr
                    let multiaddr = format!("/ip4/{}/tcp/{}/p2p/{}", ip, port, peer_id)
                        .parse::<Multiaddr>()
                        .map_err(|e| format!("Failed to build multiaddr: {}", e))?;
                    
                    return Ok(multiaddr);
                }
            }
        }
        
        // Legacy format: ip:port without peer ID
        // Build multiaddr without peer ID (will rely on identify protocol)
        if let Some(colon_pos) = peer_str.rfind(':') {
            let ip = &peer_str[..colon_pos];
            let port = &peer_str[colon_pos + 1..];
            
            let multiaddr = format!("/ip4/{}/tcp/{}", ip, port)
                .parse::<Multiaddr>()
                .map_err(|e| format!("Failed to build multiaddr: {}", e))?;
            
            warn!("Bootstrap peer {} has no peer ID - connection may be unreliable", peer_str);
            return Ok(multiaddr);
        }
        
        Err(format!("Invalid bootstrap peer format: {}", peer_str))
    }

    /// Get the local peer ID
    pub fn local_peer_id(&self) -> PeerId {
        self.local_peer_id
    }

    /// Get the network ID
    pub fn network_id(&self) -> &str {
        &self.network_id
    }

    /// Add a bootstrap node
    pub fn add_bootstrap_node(&mut self, peer_id: PeerId, addr: Multiaddr) {
        self.bootstrap_nodes.push(addr);
    }

    /// Add multiple bootstrap nodes
    pub fn add_bootstrap_nodes(&mut self, nodes: Vec<(PeerId, Multiaddr)>) {
        self.bootstrap_nodes
            .extend(nodes.into_iter().map(|(_, addr)| addr));
    }
    
    /// Get number of configured bootstrap nodes
    pub fn bootstrap_count(&self) -> usize {
        self.bootstrap_nodes.len()
    }

    /// Add a trusted peer
    pub async fn add_trusted_peer(&self, peer_id: PeerId) {
        let mut trusted = self.trusted_peers.write().await;
        trusted.insert(peer_id);
    }

    /// Disconnect from a peer
    pub async fn disconnect_from_peer(&self, peer_id: &PeerId) -> Result<(), String> {
        // Send disconnect command
        if let Some(tx) = self.swarm_cmd_tx.read().await.as_ref() {
            // For now, we don't have a specific disconnect command in SwarmCommand
            // In a real implementation, we would add a Disconnect variant
            debug!("Disconnecting from peer: {}", peer_id);

            // Remove from connected peers
            self.connected_peers.write().await.remove(peer_id);

            // Update stats
            self.stats.write().await.peers_connected =
                self.stats.write().await.peers_connected.saturating_sub(1);

            // Send event
            let _ = self
                .event_sender
                .send(NetworkEvent::PeerDisconnected(*peer_id))
                .await;

            Ok(())
        } else {
            Err("Network not running".to_string())
        }
    }

    /// Start the network
    pub async fn start(&self) -> Result<(), Box<dyn Error>> {
        // Set running flag
        *self.running.write().await = true;

        // Initialize swarm if not already done
        if self.swarm.read().await.is_none() {
            // Use persistent keypair (DO NOT generate new one)
            let id_keys = self.keypair.clone();

            // Build transport
            let transport = build_transport(id_keys.clone())?;

            // Create individual behaviours
            let gossipsub_config = gossipsub::ConfigBuilder::default()
                .validation_mode(gossipsub::ValidationMode::Strict)
                .message_id_fn(|msg| {
                    use std::hash::{Hash, Hasher};
                    let mut hasher = std::collections::hash_map::DefaultHasher::new();
                    msg.data.hash(&mut hasher);
                    gossipsub::MessageId::from(hasher.finish().to_string())
                })
                .build()
                .map_err(|e| format!("Failed to build gossipsub config: {}", e))?;

            let gossipsub = Gossipsub::new(
                gossipsub::MessageAuthenticity::Signed(id_keys.clone()),
                gossipsub_config,
            )
            .map_err(|e| format!("Failed to create gossipsub: {}", e))?;

            let store = MemoryStore::new(self.local_peer_id);
            let kademlia = Kademlia::new(self.local_peer_id, store);

            let mdns = Mdns::new(mdns::Config::default(), self.local_peer_id)?;

            let identify = Identify::new(
                identify::Config::new("/supernova/1.0.0".to_string(), id_keys.public())
                    .with_agent_version("supernova/1.0.0".to_string()),
            );

            // Create behaviour
            let behaviour =
                SupernovaBehaviour::new(self.local_peer_id, gossipsub, kademlia, mdns, identify);

            // Create swarm
            let swarm =
                SwarmBuilder::with_tokio_executor(transport, behaviour, self.local_peer_id).build();

            *self.swarm.write().await = Some(swarm);
        }

        // Start listening on configured address (NOT random port)
        // Extract port from config (format: "0.0.0.0:8333" or "/ip4/0.0.0.0/tcp/8333")
        let listen_port = self.extract_port_from_config()
            .unwrap_or(8333); // Default to 8333 if parsing fails
        
        let listen_addr: Multiaddr = format!("/ip4/0.0.0.0/tcp/{}", listen_port)
            .parse()
            .map_err(|e| format!("Failed to parse listen address: {}", e))?;
        
        info!("Binding P2P network to {}", listen_addr);
        
        if let Some(swarm) = self.swarm.write().await.as_mut() {
            swarm.listen_on(listen_addr.clone())
                .map_err(|e| format!("Failed to listen on {}: {}", listen_addr, e))?;
        } else {
            return Err("Swarm not initialized".into());
        }

        // Start network event loop in a way that handles non-Send types
        self.start_network_loop_with_channels().await?;

        // Send started event
        let _ = self.event_sender.send(NetworkEvent::Started).await;

        info!("P2P network started");
        
        // Dial bootstrap peers
        self.dial_bootstrap_peers().await?;
        
        Ok(())
    }
    
    /// Dial all configured bootstrap peers
    async fn dial_bootstrap_peers(&self) -> Result<(), Box<dyn Error>> {
        if self.bootstrap_nodes.is_empty() {
            info!("No bootstrap nodes configured");
            return Ok(());
        }
        
        info!("Dialing {} bootstrap peers", self.bootstrap_nodes.len());
        
        for multiaddr in &self.bootstrap_nodes {
            info!("Attempting to dial bootstrap peer: {}", multiaddr);
            
            if let Some(tx) = self.swarm_cmd_tx.read().await.as_ref() {
                if let Err(e) = tx.send(SwarmCommand::Dial(multiaddr.clone())).await {
                    warn!("Failed to send dial command for {}: {}", multiaddr, e);
                } else {
                    info!("Dial command sent for {}", multiaddr);
                }
            } else {
                warn!("Swarm command channel not initialized");
            }
        }
        
        Ok(())
    }

    /// Start network loop using channels to avoid Send trait issues
    async fn start_network_loop_with_channels(&self) -> Result<(), Box<dyn Error>> {
        // Create channels for communication between threads
        let (swarm_cmd_tx, mut swarm_cmd_rx) = mpsc::channel::<SwarmCommand>(100);
        let (swarm_event_tx, mut swarm_event_rx) = mpsc::channel::<SwarmEventWrapper>(100);

        let event_sender = self.event_sender.clone();
        let stats = Arc::clone(&self.stats);
        let connected_peers = Arc::clone(&self.connected_peers);
        let bandwidth_tracker = Arc::clone(&self.bandwidth_tracker);
        let rate_limiter = Arc::clone(&self.rate_limiter);
        let banned_peers = Arc::clone(&self.banned_peers);
        let running = Arc::clone(&self.running);

        // Take ownership of the swarm
        let mut swarm = self
            .swarm
            .write()
            .await
            .take()
            .ok_or("Swarm not initialized")?;

        // Spawn the swarm handler in a dedicated thread
        std::thread::spawn(move || {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("Failed to create runtime for swarm thread");

            runtime.block_on(async move {
                loop {
                    tokio::select! {
                        // Handle commands to the swarm
                        Some(cmd) = swarm_cmd_rx.recv() => {
                            match cmd {
                                SwarmCommand::Dial(addr) => {
                                    let _ = swarm.dial(addr);
                                }
                                SwarmCommand::Publish(topic, data) => {
                                    let _ = swarm.behaviour_mut().gossipsub.publish(topic, data);
                                }
                                SwarmCommand::Stop => {
                                    break;
                                }
                            }
                        }

                        // Handle swarm events
                        event = swarm.next() => {
                            if let Some(event) = event {
                                // Handle the event and create wrapper
                                match event {
                                    SwarmEvent::ConnectionEstablished { peer_id, endpoint, .. } => {
                                        let wrapped = SwarmEventWrapper::ConnectionEstablished {
                                            peer_id,
                                            endpoint: endpoint.get_remote_address().to_string(),
                                        };
                                        let _ = swarm_event_tx.send(wrapped).await;
                                    }
                                    SwarmEvent::ConnectionClosed { peer_id, .. } => {
                                        let wrapped = SwarmEventWrapper::ConnectionClosed { peer_id };
                                        let _ = swarm_event_tx.send(wrapped).await;
                                    }
                                    SwarmEvent::IncomingConnection { local_addr, .. } => {
                                        let wrapped = SwarmEventWrapper::IncomingConnection { local_addr };
                                        let _ = swarm_event_tx.send(wrapped).await;
                                    }
                                    SwarmEvent::Behaviour(behaviour_event) => {
                                        match behaviour_event {
                                            SupernovaBehaviourEvent::Gossipsub(gossipsub_event) => {
                                                if let gossipsub::Event::Message { propagation_source, message, .. } = gossipsub_event {
                                                    let wrapped = SwarmEventWrapper::Message {
                                                        peer_id: propagation_source,
                                                        topic: message.topic.to_string(),
                                                        data: message.data,
                                                    };
                                                    let _ = swarm_event_tx.send(wrapped).await;
                                                }
                                            }
                                            SupernovaBehaviourEvent::Mdns(mdns_event) => {
                                                if let mdns::Event::Discovered(list) = mdns_event {
                                                    let peers: Vec<PeerId> = list.into_iter().map(|(peer_id, _)| peer_id).collect();
                                                    let wrapped = SwarmEventWrapper::Discovered { peers };
                                                    let _ = swarm_event_tx.send(wrapped).await;
                                                }
                                            }
                                            _ => {}
                                        }
                                    }
                                    _ => {}
                                }
                            }
                        }
                    }
                }
            });
        });

        // Store the command sender for other methods to use
        self.swarm_cmd_tx.write().await.replace(swarm_cmd_tx);

        // Handle events and commands in the main async context
        let command_receiver = Arc::clone(&self.command_receiver);
        let swarm_cmd_tx = self
            .swarm_cmd_tx
            .read()
            .await
            .clone()
            .ok_or_else(|| Box::<dyn Error>::from("Swarm command sender not initialized"))?;

        let task = tokio::spawn(async move {
            let mut command_rx = command_receiver
                .write()
                .await
                .take()
                .expect("Command receiver should be available");
            let mut rate_limit_cleanup_interval = tokio::time::interval(Duration::from_secs(300));
            let mut ban_cleanup_interval = tokio::time::interval(Duration::from_secs(60));

            loop {
                if !*running.read().await {
                    let _ = swarm_cmd_tx.send(SwarmCommand::Stop).await;
                    break;
                }

                tokio::select! {
                    // Process network commands
                    Some(cmd) = command_rx.recv() => {
                        Self::handle_command_with_channels(
                            cmd,
                            &swarm_cmd_tx,
                            &event_sender,
                            &stats,
                            &connected_peers,
                            &bandwidth_tracker,
                        ).await;
                    }

                    // Process swarm events
                    Some(event) = swarm_event_rx.recv() => {
                        Self::handle_wrapped_swarm_event(
                            event,
                            &event_sender,
                            &stats,
                            &connected_peers,
                            &bandwidth_tracker,
                        ).await;
                    }

                    // Periodic cleanups
                    _ = rate_limit_cleanup_interval.tick() => {
                        rate_limiter.cleanup();
                        let metrics = rate_limiter.metrics();
                        info!("Rate limiter metrics - Total: {}, Rejected: {}, Banned: {}",
                              metrics.total_requests, metrics.rejected_requests, metrics.banned_ips);
                    }

                    _ = ban_cleanup_interval.tick() => {
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

    /// Handle commands using channels
    async fn handle_command_with_channels(
        cmd: NetworkCommand,
        swarm_cmd_tx: &mpsc::Sender<SwarmCommand>,
        event_sender: &mpsc::Sender<NetworkEvent>,
        stats: &Arc<RwLock<NetworkStats>>,
        connected_peers: &Arc<RwLock<HashMap<PeerId, PeerInfo>>>,
        bandwidth_tracker: &Arc<Mutex<BandwidthTracker>>,
    ) {
        // Helper to broadcast a message
        async fn broadcast_message(
            message: &Message,
            swarm_cmd_tx: &mpsc::Sender<SwarmCommand>,
            stats: &Arc<RwLock<NetworkStats>>,
            bandwidth_tracker: &Arc<Mutex<BandwidthTracker>>,
        ) {
            let topic = match message {
                Message::Block { .. } | Message::NewBlock { .. } => TopicHash::from_raw("blocks"),
                Message::Transaction { .. } => TopicHash::from_raw("transactions"),
                Message::Headers { .. } => TopicHash::from_raw("headers"),
                Message::Status { .. } | Message::GetStatus => TopicHash::from_raw("status"),
                Message::GetMempool { .. } | Message::Mempool { .. } => {
                    TopicHash::from_raw("mempool")
                }
                _ => TopicHash::from_raw("general"),
            };

            if let Ok(data) = bincode::serialize(&message) {
                let data_len = data.len();
                let _ = swarm_cmd_tx.send(SwarmCommand::Publish(topic, data)).await;

                // Update stats
                let mut stats_guard = stats.write().await;
                stats_guard.messages_sent += 1;
                stats_guard.bytes_sent += data_len as u64;

                // Track bandwidth
                if let Ok(mut tracker) = bandwidth_tracker.lock() {
                    tracker.record_sent(data_len as u64);
                }
            }
        }
        match cmd {
            NetworkCommand::ConnectToPeer(addr_str) => {
                // Parse bootstrap peer (supports peer_id@ip:port format)
                match Self::parse_bootstrap_peer(&addr_str) {
                    Ok(multiaddr) => {
                        info!("Dialing bootstrap peer: {}", multiaddr);
                        let swarm_guard = swarm_cmd_tx.send(SwarmCommand::Dial(multiaddr.clone())).await;
                        if swarm_guard.is_ok() {
                            let mut stats_guard = stats.write().await;
                            stats_guard.connection_attempts += 1;
                        } else {
                            warn!("Failed to send dial command to swarm for {}", multiaddr);
                        }
                    }
                    Err(e) => {
                        warn!("Invalid peer address format {}: {}", addr_str, e);
                        let _ = event_sender
                            .send(NetworkEvent::Error {
                                peer_id: None,
                                error: format!("Invalid address format: {}", addr_str),
                            })
                            .await;
                    }
                }
            }

            NetworkCommand::Broadcast(message) => {
                broadcast_message(&message, swarm_cmd_tx, stats, bandwidth_tracker).await;
            }

            NetworkCommand::SendToPeer { peer_id, message } => {
                // For direct messages, we'd need to implement a custom protocol
                // For now, we'll use gossipsub for all messages
                let topic = TopicHash::from_raw("messages");
                let data = bincode::serialize(&(peer_id.to_string(), message)).unwrap_or_default();
                let _ = swarm_cmd_tx.send(SwarmCommand::Publish(topic, data)).await;
            }

            NetworkCommand::AnnounceBlock {
                block,
                height,
                total_difficulty,
            } => {
                let message = Message::NewBlock {
                    block_data: bincode::serialize(&block).unwrap_or_default(),
                    height,
                    total_difficulty,
                };

                broadcast_message(&message, swarm_cmd_tx, stats, bandwidth_tracker).await;

                let mut stats_guard = stats.write().await;
                stats_guard.blocks_announced += 1;
            }

            NetworkCommand::AnnounceTransaction {
                transaction,
                fee_rate,
            } => {
                let message = Message::Transaction {
                    transaction: bincode::serialize(&transaction).unwrap_or_default(),
                };

                broadcast_message(&message, swarm_cmd_tx, stats, bandwidth_tracker).await;

                let mut stats_guard = stats.write().await;
                stats_guard.transactions_announced += 1;
            }

            NetworkCommand::RequestHeaders {
                start_height,
                end_height,
                preferred_peer,
            } => {
                let message = Message::GetHeaders {
                    start_height,
                    end_height,
                };

                if let Some(peer_id) = preferred_peer {
                    // Handle SendToPeer directly without recursion
                    let topic = TopicHash::from_raw("messages");
                    let data =
                        bincode::serialize(&(peer_id.to_string(), message)).unwrap_or_default();
                    let _ = swarm_cmd_tx.send(SwarmCommand::Publish(topic, data)).await;
                } else {
                    broadcast_message(&message, swarm_cmd_tx, stats, bandwidth_tracker).await;
                }
            }

            NetworkCommand::RequestBlocks {
                block_hashes,
                preferred_peer,
            } => {
                let message = Message::GetBlocksByHash { block_hashes };

                if let Some(peer_id) = preferred_peer {
                    // Handle SendToPeer directly without recursion
                    let topic = TopicHash::from_raw("messages");
                    let data =
                        bincode::serialize(&(peer_id.to_string(), message)).unwrap_or_default();
                    let _ = swarm_cmd_tx.send(SwarmCommand::Publish(topic, data)).await;
                } else {
                    broadcast_message(&message, swarm_cmd_tx, stats, bandwidth_tracker).await;
                }
            }

            NetworkCommand::RequestBlocksByHeight {
                start_height,
                end_height,
                preferred_peer,
            } => {
                let message = Message::GetBlocksByHeight {
                    start_height,
                    end_height,
                };

                if let Some(peer_id) = preferred_peer {
                    // Handle SendToPeer directly without recursion
                    let topic = TopicHash::from_raw("messages");
                    let data =
                        bincode::serialize(&(peer_id.to_string(), message)).unwrap_or_default();
                    let _ = swarm_cmd_tx.send(SwarmCommand::Publish(topic, data)).await;
                } else {
                    broadcast_message(&message, swarm_cmd_tx, stats, bandwidth_tracker).await;
                }
            }

            NetworkCommand::AnnounceStatus {
                version,
                height,
                best_hash,
                total_difficulty,
            } => {
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

                broadcast_message(&message, swarm_cmd_tx, stats, bandwidth_tracker).await;
            }

            NetworkCommand::BanPeer {
                peer_id,
                reason,
                duration,
            } => {
                connected_peers.write().await.remove(&peer_id);
                let _ = event_sender.send(NetworkEvent::PeerBanned(peer_id)).await;
            }

            NetworkCommand::AddTrustedPeer(peer_id) => {
                let _ = event_sender
                    .send(NetworkEvent::PeerAddedToTrusted(peer_id))
                    .await;
            }

            NetworkCommand::RemoveTrustedPeer(peer_id) => {
                let _ = event_sender
                    .send(NetworkEvent::PeerRemovedFromTrusted(peer_id))
                    .await;
            }

            NetworkCommand::Start => {
                let _ = event_sender.send(NetworkEvent::Started).await;
            }

            NetworkCommand::Stop => {
                let _ = event_sender.send(NetworkEvent::Stopped).await;
            }

            _ => {
                // Handle other commands as needed
                debug!("Unhandled network command: {:?}", cmd);
            }
        }
    }

    /// Handle wrapped swarm events
    async fn handle_wrapped_swarm_event(
        event: SwarmEventWrapper,
        event_sender: &mpsc::Sender<NetworkEvent>,
        stats: &Arc<RwLock<NetworkStats>>,
        connected_peers: &Arc<RwLock<HashMap<PeerId, PeerInfo>>>,
        bandwidth_tracker: &Arc<Mutex<BandwidthTracker>>,
    ) {
        match event {
            SwarmEventWrapper::ConnectionEstablished { peer_id, endpoint } => {
                let peer_info = PeerInfo {
                    peer_id,
                    state: PeerState::Connected,
                    addresses: vec![],
                    first_seen: Instant::now(),
                    last_seen: Instant::now(),
                    last_sent: None,
                    is_inbound: true, // We'll assume inbound for now
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
                    metadata: peer::PeerMetadata::default(),
                };

                connected_peers
                    .write()
                    .await
                    .insert(peer_id, peer_info.clone());
                stats.write().await.peers_connected += 1;

                let _ = event_sender
                    .send(NetworkEvent::PeerConnected(peer_info))
                    .await;
                info!("Peer connected: {} at {}", peer_id, endpoint);
            }
            SwarmEventWrapper::ConnectionClosed { peer_id } => {
                connected_peers.write().await.remove(&peer_id);
                stats.write().await.peers_connected =
                    stats.write().await.peers_connected.saturating_sub(1);

                let _ = event_sender
                    .send(NetworkEvent::PeerDisconnected(peer_id))
                    .await;
                info!("Peer disconnected: {}", peer_id);
            }
            SwarmEventWrapper::IncomingConnection { local_addr } => {
                debug!("Incoming connection on {}", local_addr);
            }
            SwarmEventWrapper::Message {
                peer_id,
                topic,
                data,
            } => {
                // Update stats
                stats.write().await.messages_received += 1;
                stats.write().await.bytes_received += data.len() as u64;

                // Track bandwidth
                if let Ok(mut tracker) = bandwidth_tracker.lock() {
                    tracker.record_received(data.len() as u64);
                }

                // Update peer info
                if let Some(peer_info) = connected_peers.write().await.get_mut(&peer_id) {
                    peer_info.metadata.transactions_received += 1;
                    peer_info.last_seen = Instant::now();
                }

                // Deserialize and forward the message
                if let Ok(message) = bincode::deserialize::<Message>(&data) {
                    let _ = event_sender
                        .send(NetworkEvent::MessageReceived { peer_id, message })
                        .await;
                }
            }
            SwarmEventWrapper::Discovered { peers } => {
                for peer_id in peers {
                    debug!("Discovered peer: {}", peer_id);
                }
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

    /// Get peer count (alias for get_peer_count)
    pub async fn peer_count(&self) -> usize {
        self.get_peer_count().await
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
            info!(
                "Identity verification challenge difficulty set to {}",
                difficulty
            );
        } else {
            warn!(
                "Invalid challenge difficulty: {}, keeping current setting: {}",
                difficulty, self.challenge_difficulty
            );
        }
    }

    /// Enable or disable identity verification requirement
    pub fn set_require_verification(&mut self, require: bool) {
        self.require_identity_verification = require;
        info!(
            "Identity verification requirement {}",
            if require { "enabled" } else { "disabled" }
        );
    }

    /// Generate a new identity verification challenge for a peer
    pub async fn generate_challenge(&self, peer_id: &PeerId) -> IdentityChallenge {
        // Use the identity verification system to create a challenge
        let challenge = self.identity_system.create_challenge(peer_id).await;

        // Convert to our IdentityChallenge type
        IdentityChallenge {
            challenge: challenge.nonce.to_vec(),
            difficulty: challenge.difficulty,
            issued_at: challenge.created_at,
            timeout: challenge.expires_in,
        }
    }

    /// Verify a challenge response
    pub async fn verify_challenge(&self, peer_id: &PeerId, solution: &[u8]) -> bool {
        // Use the identity verification system to verify the challenge
        self.identity_system
            .verify_challenge(peer_id, solution)
            .await
    }

    /// Check if a peer has been verified
    pub async fn is_peer_verified(&self, peer_id: &PeerId) -> bool {
        self.identity_system.is_verified(peer_id).await
    }

    /// Get network info for API
    pub async fn get_network_info(&self) -> Result<NetworkInfo, Box<dyn Error>> {
        let stats = self.get_stats().await;
        let bandwidth = self.bandwidth_tracker.lock().map_err(|e| {
            Box::<dyn Error>::from(format!("Bandwidth tracker lock poisoned: {}", e))
        })?;
        let rates = bandwidth.get_rates(60);
        let (bytes_sent, bytes_received) = (bandwidth.bytes_sent, bandwidth.bytes_received);

        // Collect information from connected peers
        let connected_peers = self.connected_peers.read().await;
        let peer_count = connected_peers.len();

        // Determine if we're listening
        let swarm_guard = self.swarm.read().await;
        let listening = if let Some(swarm) = swarm_guard.as_ref() {
            !swarm.listeners().collect::<Vec<_>>().is_empty()
        } else {
            false
        };

        // Lock swarm to collect local addresses
        let swarm_guard = self.swarm.read().await;
        let local_addresses = if let Some(swarm) = swarm_guard.as_ref() {
            swarm
                .listeners()
                .map(|addr| crate::api::types::NetworkAddress {
                    address: addr.to_string(),
                    port: 0, // Port will be extracted from the address string in practice
                    score: 0,
                })
                .collect()
        } else {
            vec![]
        };
        drop(swarm_guard);

        // Try to detect external IP from connected peers
        let external_ip = self.detect_external_ip(&connected_peers).await;

        // Calculate average ping time from connected peers
        let ping_times: Vec<f64> = connected_peers
            .values()
            .filter_map(|peer| peer.ping_ms.map(|ms| ms as f64))
            .collect();

        let avg_ping_time = if !ping_times.is_empty() {
            ping_times.iter().sum::<f64>() / ping_times.len() as f64
        } else {
            0.0
        };

        // Count connections by direction
        let inbound_count = connected_peers
            .values()
            .filter(|peer| peer.is_inbound)
            .count();
        let outbound_count = peer_count - inbound_count;

        Ok(NetworkInfo {
            version: env!("CARGO_PKG_VERSION").to_string(),
            protocol_version: 1,
            connections: peer_count as u32,
            inbound_connections: inbound_count as u32,
            outbound_connections: outbound_count as u32,
            network: self.network_id.clone(),
            is_listening: listening,
            accepts_incoming: listening,
            local_addresses,
            external_ip,
            network_stats: crate::api::types::NetworkStats {
                total_bytes_sent: bytes_sent,
                total_bytes_received: bytes_received,
                upload_rate: rates.0,
                download_rate: rates.1,
                ping_time: avg_ping_time,
            },
        })
    }

    /// Detect external IP from connected peers
    async fn detect_external_ip(
        &self,
        connected_peers: &HashMap<PeerId, PeerInfo>,
    ) -> Option<String> {
        // Try to infer external IP from peer connections
        // In a real P2P network, peers often report what IP they see us as
        for peer in connected_peers.values() {
            // If we have inbound connections, use the first valid IP we find
            if peer.is_inbound {
                for addr in &peer.addresses {
                    if let Some(ip) = extract_ip_from_multiaddr(addr) {
                        // Filter out local/private IPs
                        match ip {
                            IpAddr::V4(ipv4) => {
                                if !ipv4.is_private() && !ipv4.is_loopback() {
                                    return Some(ip.to_string());
                                }
                            }
                            IpAddr::V6(ipv6) => {
                                if !ipv6.is_loopback() && !ipv6.is_unspecified() {
                                    return Some(ip.to_string());
                                }
                            }
                        }
                    }
                }
            }
        }
        None
    }

    /// Get connection count for API
    pub async fn get_connection_count(&self) -> Result<ConnectionCount, Box<dyn Error>> {
        let stats = self.get_stats().await;

        Ok(ConnectionCount {
            total: (stats.inbound_connections + stats.outbound_connections) as u32,
            inbound: stats.inbound_connections as u32,
            outbound: stats.outbound_connections as u32,
        })
    }

    /// Get peers for API
    pub async fn get_peers(&self) -> Result<Vec<crate::api::types::PeerInfo>, Box<dyn Error>> {
        let peers = self.peer_manager.get_connected_peers().await;

        let mut api_peers = Vec::new();
        for (idx, peer_info) in peers.into_iter().enumerate() {
            let api_peer = crate::api::types::PeerInfo {
                id: idx as u64, // Use index as numeric ID
                address: if let Some(addr) = peer_info.addresses.first() {
                    addr.to_string()
                } else {
                    peer_info.peer_id.to_string()
                },
                direction: if peer_info.is_inbound {
                    "inbound".to_string()
                } else {
                    "outbound".to_string()
                },
                connected_time: peer_info.first_seen.elapsed().as_secs(),
                last_send: peer_info
                    .last_sent
                    .map(|t| t.elapsed().as_secs())
                    .unwrap_or(0),
                last_recv: peer_info.last_seen.elapsed().as_secs(),
                bytes_sent: peer_info.bytes_sent,
                bytes_received: peer_info.bytes_received,
                ping_time: peer_info.ping_ms.map(|ms| ms as f64),
                version: peer_info.protocol_version.unwrap_or(0).to_string(),
                user_agent: peer_info
                    .user_agent
                    .clone()
                    .unwrap_or_else(|| "unknown".to_string()),
                height: peer_info.height.unwrap_or(0),
                services: self.format_service_flags(peer_info.services),
                banned: matches!(peer_info.state, PeerState::Banned),
                reputation_score: peer_info.reputation as f64,
            };
            api_peers.push(api_peer);
        }

        Ok(api_peers)
    }

    /// Get specific peer for API
    pub async fn get_peer(
        &self,
        peer_id: &str,
    ) -> Result<Option<crate::api::types::PeerInfo>, Box<dyn Error>> {
        // Parse peer ID
        let peer_id = peer_id
            .parse::<PeerId>()
            .map_err(|e| format!("Invalid peer ID: {}", e))?;

        let connected_peers = self.connected_peers.read().await;
        if let Some(peer_info) = connected_peers.get(&peer_id) {
            // Generate a numeric ID based on peer_id hash
            let id = {
                let hash = &peer_info.peer_id.to_bytes()[..8];
                u64::from_be_bytes(hash.try_into().unwrap_or([0; 8]))
            };

            let api_peer = crate::api::types::PeerInfo {
                id,
                address: if let Some(addr) = peer_info.addresses.first() {
                    addr.to_string()
                } else {
                    peer_info.peer_id.to_string()
                },
                direction: if peer_info.is_inbound {
                    "inbound".to_string()
                } else {
                    "outbound".to_string()
                },
                connected_time: peer_info.first_seen.elapsed().as_secs(),
                last_send: peer_info
                    .last_sent
                    .map(|t| t.elapsed().as_secs())
                    .unwrap_or(0),
                last_recv: peer_info.last_seen.elapsed().as_secs(),
                bytes_sent: peer_info.bytes_sent,
                bytes_received: peer_info.bytes_received,
                ping_time: peer_info.ping_ms.map(|ms| ms as f64),
                version: peer_info.protocol_version.unwrap_or(0).to_string(),
                user_agent: peer_info
                    .user_agent
                    .clone()
                    .unwrap_or_else(|| "unknown".to_string()),
                height: peer_info.height.unwrap_or(0),
                services: self.format_service_flags(peer_info.services),
                banned: matches!(peer_info.state, PeerState::Banned),
                reputation_score: peer_info.reputation as f64,
            };

            Ok(Some(api_peer))
        } else {
            Ok(None)
        }
    }

    /// Format service flags for display
    fn format_service_flags(&self, services: u64) -> String {
        let mut flags = Vec::new();

        // Common Bitcoin-style service flags
        if services & 0x01 != 0 {
            flags.push("NETWORK");
        }
        if services & 0x02 != 0 {
            flags.push("GETUTXO");
        }
        if services & 0x04 != 0 {
            flags.push("BLOOM");
        }
        if services & 0x08 != 0 {
            flags.push("WITNESS");
        }
        if services & 0x400 != 0 {
            flags.push("NETWORK_LIMITED");
        }

        // Supernova-specific flags
        if services & 0x1000 != 0 {
            flags.push("QUANTUM");
        }
        if services & 0x2000 != 0 {
            flags.push("LIGHTNING");
        }
        if services & 0x4000 != 0 {
            flags.push("ENVIRONMENTAL");
        }

        flags.join(",")
    }

    /// Add peer for API
    pub async fn add_peer(
        &self,
        address: &str,
        permanent: bool,
    ) -> Result<PeerAddResponse, Box<dyn Error>> {
        // Parse the multiaddress
        let addr: Multiaddr = address
            .parse()
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
                        error: None,
                        peer_id: None, // Will be filled when connection is established
                    })
                }
                Err(e) => {
                    warn!("Failed to dial {}: {}", address, e);
                    Ok(PeerAddResponse {
                        success: false,
                        error: Some(format!("Failed to initiate connection: {}", e)),
                        peer_id: None,
                    })
                }
            }
        } else {
            Ok(PeerAddResponse {
                success: false,
                error: Some("Network not initialized".to_string()),
                peer_id: None,
            })
        }
    }

    /// Remove peer for API
    pub async fn remove_peer(&self, peer_id: &str) -> Result<bool, Box<dyn Error>> {
        // Parse peer ID
        let peer_id = peer_id
            .parse::<PeerId>()
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
        let bandwidth = self.bandwidth_tracker.lock().map_err(|e| {
            Box::<dyn Error>::from(format!("Bandwidth tracker lock poisoned: {}", e))
        })?;
        let (upload_rate, download_rate) = bandwidth.get_rates(period);

        Ok(BandwidthUsage {
            total_sent: bandwidth.bytes_sent,
            total_received: bandwidth.bytes_received,
            upload_rate,
            download_rate,
            peak_upload_rate: upload_rate, // For now, use current rate as peak
            peak_download_rate: download_rate, // For now, use current rate as peak
        })
    }

    /// Ban a peer for misbehavior
    pub async fn ban_peer(&self, peer_id: &PeerId, reason: &str, duration: Option<Duration>) {
        let ban_until = Instant::now() + duration.unwrap_or(BAN_DURATION);

        // Add to banned peers
        self.banned_peers.write().await.insert(*peer_id, ban_until);

        // Remove from connected peers
        self.connected_peers.write().await.remove(peer_id);

        // Update stats
        let mut stats = self.stats.write().await;
        stats.peers_banned += 1;

        warn!(
            "Banned peer {} for {}: duration {:?}",
            peer_id, reason, duration
        );
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
            peer_info.height = Some(height);
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
            )
            .await;
        }
    }

    /// Handle incoming ping
    pub async fn handle_ping(&self, peer_id: &PeerId, nonce: u64) {
        let pong_message = Message::Pong(nonce);

        // Send pong response
        Self::send_to_peer_static(
            *peer_id,
            pong_message,
            &self.swarm,
            &self.stats,
            &self.bandwidth_tracker,
        )
        .await;
    }

    /// Handle incoming pong
    pub async fn handle_pong(&self, peer_id: &PeerId, nonce: u64) {
        // Update peer latency
        if let Some(peer_info) = self.connected_peers.write().await.get_mut(peer_id) {
            // In a real implementation, we'd calculate latency based on ping timestamp
            peer_info.ping_ms = Some(50); // Placeholder
            peer_info.last_seen = Instant::now();
        }

        debug!("Received pong from {} with nonce {}", peer_id, nonce);
    }

    /// Static method to broadcast a message
    async fn broadcast_message_static(
        message: Message,
        swarm: &Arc<RwLock<Option<Swarm<SupernovaBehaviour>>>>,
        stats: &Arc<RwLock<NetworkStats>>,
        bandwidth_tracker: &Arc<Mutex<BandwidthTracker>>,
    ) {
        // In the channel-based architecture, we can't directly access the swarm
        // This is a placeholder implementation
        debug!("Broadcasting message: {:?}", message);

        // Update stats
        if let Ok(data) = bincode::serialize(&message) {
            stats.write().await.messages_sent += 1;
            stats.write().await.bytes_sent += data.len() as u64;
            if let Ok(mut tracker) = bandwidth_tracker.lock() {
                tracker.record_sent(data.len() as u64);
            }
        }
    }

    /// Static method to send a message to a specific peer
    async fn send_to_peer_static(
        peer_id: PeerId,
        message: Message,
        swarm: &Arc<RwLock<Option<Swarm<SupernovaBehaviour>>>>,
        stats: &Arc<RwLock<NetworkStats>>,
        bandwidth_tracker: &Arc<Mutex<BandwidthTracker>>,
    ) {
        // In the channel-based architecture, we can't directly access the swarm
        // This is a placeholder implementation
        debug!("Sending message to {}: {:?}", peer_id, message);

        // Update stats
        if let Ok(data) = bincode::serialize(&message) {
            stats.write().await.messages_sent += 1;
            stats.write().await.bytes_sent += data.len() as u64;
            if let Ok(mut tracker) = bandwidth_tracker.lock() {
                tracker.record_sent(data.len() as u64);
            }
        }
    }

    /// Request blocks from peers
    pub async fn request_blocks(
        &self,
        block_hashes: Vec<[u8; 32]>,
        preferred_peer: Option<PeerId>,
    ) {
        let message = Message::GetBlocksByHash { block_hashes };

        if let Some(peer_id) = preferred_peer {
            // Send to specific peer
            Self::send_to_peer_static(
                peer_id,
                message,
                &self.swarm,
                &self.stats,
                &self.bandwidth_tracker,
            )
            .await;
        } else {
            // Broadcast to all peers
            Self::broadcast_message_static(
                message,
                &self.swarm,
                &self.stats,
                &self.bandwidth_tracker,
            )
            .await;
        }
    }

    /// Request headers from peers
    pub async fn request_headers(
        &self,
        start_height: u64,
        end_height: u64,
        preferred_peer: Option<PeerId>,
    ) {
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
            )
            .await;
        } else {
            // Broadcast to all peers
            Self::broadcast_message_static(
                message,
                &self.swarm,
                &self.stats,
                &self.bandwidth_tracker,
            )
            .await;
        }
    }

    /// Announce our status to the network
    pub async fn announce_status(
        &self,
        version: u32,
        height: u64,
        best_hash: [u8; 32],
        total_difficulty: u64,
    ) {
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

        Self::broadcast_message_static(message, &self.swarm, &self.stats, &self.bandwidth_tracker)
            .await;
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

        // Get current peer count
        let peer_count = self.get_peer_count().await;

        // Simple rotation logic: if we have too many peers, disconnect some
        if peer_count > 50 {
            // Get peers to disconnect (lowest scores)
            let peers_to_disconnect = self.peer_manager.get_peers_to_disconnect(5).await;

            info!("Rotating {} peers", peers_to_disconnect.len());

            // Disconnect selected peers
            for peer_id in peers_to_disconnect {
                info!("Disconnecting peer {} for rotation", peer_id);
                if let Err(e) = self.disconnect_from_peer(&peer_id).await {
                    warn!("Failed to disconnect peer {}: {}", peer_id, e);
                }
            }
        }

        Ok(())
    }

    /// Get eclipse attack risk level
    pub async fn get_eclipse_risk_level(&self) -> EclipseRiskLevel {
        // Simple risk assessment based on network diversity
        let connected_peers = self.connected_peers.read().await;
        let diversity = self.calculate_network_diversity(&connected_peers).await;

        if diversity < 0.3 {
            EclipseRiskLevel::High
        } else if diversity < 0.6 {
            EclipseRiskLevel::Medium
        } else {
            EclipseRiskLevel::Low
        }
    }

    /// Handle identity challenge response
    pub async fn handle_identity_challenge_response(
        &self,
        peer_id: &PeerId,
        solution: &[u8],
    ) -> bool {
        self.identity_system
            .verify_challenge(peer_id, solution)
            .await
    }

    /// Update peer behavior score based on actions
    pub async fn update_peer_behavior(&self, peer_id: &PeerId, delta: f64) {
        if let Err(e) = self.peer_manager.update_peer_score(peer_id, delta).await {
            warn!("Failed to update peer score: {}", e);
        }
    }

    /// Record peer advertisements for eclipse detection
    pub async fn record_peer_advertisements(
        &self,
        from_peer: PeerId,
        advertised_peers: Vec<PeerId>,
    ) {
        // Simple implementation: just log for now
        debug!(
            "Peer {} advertised {} peers",
            from_peer,
            advertised_peers.len()
        );
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
                warn!(
                    "Connection from {} rejected by rate limiter: {}",
                    socket_addr, e
                );
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
        self.rate_limiter = Arc::new(RateLimiter::new(config));
    }

    /// Create P2P network instance for compatibility
    pub fn new_simple() -> Self {
        let (event_sender, _) = mpsc::channel(1);
        let storage: Arc<dyn crate::storage::Storage> =
            Arc::new(crate::storage::MemoryStorage::new());

        let keypair = identity::Keypair::generate_ed25519();
        let local_peer_id = PeerId::from(keypair.public());
        
        Self {
            local_peer_id,
            keypair,
            swarm: Arc::new(RwLock::new(None)),
            swarm_cmd_tx: Arc::new(RwLock::new(None)),
            bootstrap_nodes: Vec::new(),
            event_sender,
            command_receiver: Arc::new(RwLock::new(None)),
            stats: Arc::new(RwLock::new(NetworkStats::default())),
            running: Arc::new(RwLock::new(false)),
            connected_peers: Arc::new(RwLock::new(HashMap::new())),
            discovery: Arc::new(RwLock::new(None)),
            listen_address: "0.0.0.0:8333".to_string(),
            network_task: Arc::new(RwLock::new(None)),
            bandwidth_tracker: Arc::new(Mutex::new(BandwidthTracker::new())),
            rate_limiter: Arc::new(RateLimiter::new(RateLimitConfig::default())),
            banned_peers: Arc::new(RwLock::new(HashMap::new())),
            trusted_peers: Arc::new(RwLock::new(HashSet::new())),
            network_id: "supernova".to_string(),
            peer_manager: Arc::new(PeerManager::new(
                storage.clone(),
                ConnectionLimits::default(),
            )),
            challenge_difficulty: DEFAULT_CHALLENGE_DIFFICULTY,
            require_identity_verification: true,
            identity_system: Arc::new(IdentityVerificationSystem::new(
                DEFAULT_CHALLENGE_DIFFICULTY,
                true,
            )),
            storage,
        }
    }

    /// Get peer count synchronously (blocking)
    pub fn peer_count_sync(&self) -> usize {
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async { self.get_peer_count().await })
        })
    }

    /// Check if the node is currently syncing
    pub fn is_syncing(&self) -> bool {
        // Check if we have peers and if our height is significantly behind
        // This is a simplified implementation
        let stats = self.get_stats_sync();

        // Consider syncing if we have peers but low block count
        stats.peers_connected > 0 && stats.blocks_received < 100
    }

    /// Get sync progress (0.0 to 1.0)
    pub fn get_sync_progress(&self) -> f64 {
        // This is a simplified implementation
        // In a real implementation, this would check actual sync state
        if self.is_syncing() {
            let stats = self.get_stats_sync();

            // Simple progress based on blocks received
            (stats.blocks_received as f64 / 1000.0).min(0.99)
        } else {
            1.0
        }
    }

    /// Broadcast a transaction to all peers
    pub fn broadcast_transaction(&self, tx: &Transaction) {
        let tx_bytes = bincode::serialize(tx).unwrap_or_default();
        let message = Message::Transaction {
            transaction: tx_bytes,
        };

        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                Self::broadcast_message_static(
                    message,
                    &self.swarm,
                    &self.stats,
                    &self.bandwidth_tracker,
                )
                .await;
            })
        });
    }

    /// Broadcast a block to all peers
    pub fn broadcast_block(&self, block: &Block) {
        let message = Message::Block(block.clone());

        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                Self::broadcast_message_static(
                    message,
                    &self.swarm,
                    &self.stats,
                    &self.bandwidth_tracker,
                )
                .await;
            })
        });
    }

    /// Get network statistics synchronously
    pub fn get_stats_sync(&self) -> NetworkStats {
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async { self.get_stats().await })
        })
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
) -> Result<
    libp2p::core::transport::Boxed<(PeerId, libp2p::core::muxing::StreamMuxerBox)>,
    Box<dyn Error>,
> {
    use libp2p::core::upgrade;

    let noise = noise::Config::new(&id_keys)?;
    let yamux_config = yamux::Config::default();

    let transport = tcp::tokio::Transport::new(tcp::Config::default().nodelay(true))
        .upgrade(upgrade::Version::V1)
        .authenticate(noise)
        .multiplex(yamux_config)
        .timeout(Duration::from_secs(20))
        .boxed();

    Ok(transport)
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
        let nonce_bytes = nonce.to_be_bytes();

        // Hash challenge + nonce
        let mut hasher = Sha256::new();
        hasher.update(challenge);
        hasher.update(nonce_bytes);
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

#[cfg(test)]
mod tests {
    use super::*;

    // A basic test for network creation
    #[tokio::test]
    async fn test_network_creation() {
        let (network, _, _) = P2PNetwork::new(None, [0u8; 32], "supernova-test", None)
            .await
            .unwrap();

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
        assert!(
            leading_zeros >= difficulty,
            "Solution doesn't meet difficulty: got {} bits, required {}",
            leading_zeros,
            difficulty
        );
    }

    #[tokio::test]
    async fn test_peer_management() {
        let (network, _, _) = P2PNetwork::new(None, [0u8; 32], "supernova-test", None)
            .await
            .unwrap();

        let peer_id = PeerId::random();

        // Test peer banning
        network
            .ban_peer(&peer_id, "test ban", Some(Duration::from_secs(60)))
            .await;
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
