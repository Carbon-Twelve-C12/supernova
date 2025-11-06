pub mod advanced;
pub mod behaviour;
pub mod block_propagation;
pub mod bloom_filter;
pub mod compact_block;
pub mod connection;
pub mod peer_identity;
pub mod discovery;
pub mod eclipse_prevention;
pub mod identity_verification;
pub mod message;
pub mod network_proxy;
pub mod p2p;
pub mod peer;
pub mod peer_diversity;
pub mod peer_manager;
pub mod protocol;
pub mod rate_limiter;
pub mod sync;

#[cfg(test)]
pub mod eclipse_prevention_tests;
#[cfg(test)]
pub mod rate_limiter_tests;

use libp2p::PeerId;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};

// Re-export network types for external use
pub use behaviour::SupernovaBehaviour;
pub use connection::ConnectionState;
pub use discovery::DiscoveryEvent;
pub use message::NetworkMessage;
pub use network_proxy::NetworkProxy;
pub use p2p::{
    NetworkCommand, NetworkEvent, NetworkHealth, NetworkStats as P2PNetworkStats, P2PNetwork,
};
pub use peer::{PeerInfo, PeerMetadata, PeerState};
pub use protocol::{Message as ProtocolMessage, ProtocolError};
pub use rate_limiter::{NetworkRateLimiter, RateLimitConfig, RateLimitError};

/// Maximum number of peers to connect to
pub const MAX_PEERS: usize = 50;

/// Maximum number of inbound connections allowed
pub const MAX_INBOUND_CONNECTIONS: usize = 128;

/// Maximum number of outbound connections to maintain
pub const MAX_OUTBOUND_CONNECTIONS: usize = 24;

/// Target number of outbound connections for diversity
pub const TARGET_OUTBOUND_CONNECTIONS: usize = 8;

/// Connection timeout in seconds
pub const CONNECTION_TIMEOUT_SECS: u64 = 30;

/// Initialize the network with the given configuration
pub async fn initialize_network(
    config: &crate::config::NetworkConfig,
    genesis_hash: [u8; 32],
) -> Result<
    (
        P2PNetwork,
        mpsc::Sender<NetworkCommand>,
        mpsc::Receiver<NetworkEvent>,
    ),
    Box<dyn std::error::Error>,
> {
    // Load or generate persistent node identity
    let data_dir = std::path::PathBuf::from("./data");
    let keypair = peer_identity::load_or_generate_keypair(&data_dir)
        .map_err(|e| format!("Failed to load peer identity: {}", e))?;

    // Initialize P2P network with the keypair
    let (mut network, command_tx, event_rx) =
        crate::network::p2p::P2PNetwork::new(
            Some(keypair), 
            genesis_hash, 
            &config.network_id,
            Some(config.listen_addr.clone()), // Pass listen address
        ).await?;

    // Add bootstrap nodes if configured
    if !config.bootstrap_nodes.is_empty() {
        info!("Loading {} bootstrap nodes from config: {:?}", 
            config.bootstrap_nodes.len(), config.bootstrap_nodes);
        
        for bootstrap_str in &config.bootstrap_nodes {
            // Parse using P2PNetwork's parser that handles peer_id@ip:port format
            match parse_bootstrap_address(bootstrap_str) {
                Ok(multiaddr) => {
                    // Extract peer ID from multiaddr if present
                    let peer_id = multiaddr.iter()
                        .find_map(|proto| {
                            if let libp2p::multiaddr::Protocol::P2p(id) = proto {
                                Some(id)
                            } else {
                                None
                            }
                        })
                        .unwrap_or_else(PeerId::random);
                    
                    network.add_bootstrap_node(peer_id, multiaddr.clone());
                    info!("Added bootstrap node: {} as {}", bootstrap_str, multiaddr);
                }
                Err(e) => {
                    warn!("Failed to parse bootstrap node {}: {}", bootstrap_str, e);
                }
            }
        }
    } else {
        info!("No bootstrap nodes configured");
    }

    info!(
        "P2P network initialized with peer ID: {}",
        network.local_peer_id()
    );

    Ok((network, command_tx, event_rx))
}

/// Helper function to parse bootstrap address
/// Supports: "peer_id@ip:port", "/ip4/ip/tcp/port/p2p/peer_id", or "ip:port"
pub fn parse_bootstrap_address(addr_str: &str) -> Result<libp2p::Multiaddr, String> {
    use libp2p::Multiaddr;
    
    // Try parsing as multiaddr first
    if let Ok(addr) = addr_str.parse::<Multiaddr>() {
        return Ok(addr);
    }
    
    // Try parsing as peer_id@ip:port format
    if addr_str.contains('@') {
        let parts: Vec<&str> = addr_str.split('@').collect();
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
    if let Some(colon_pos) = addr_str.rfind(':') {
        let ip = &addr_str[..colon_pos];
        let port = &addr_str[colon_pos + 1..];
        
        let multiaddr = format!("/ip4/{}/tcp/{}", ip, port)
            .parse::<Multiaddr>()
            .map_err(|e| format!("Failed to build multiaddr: {}", e))?;
        
        return Ok(multiaddr);
    }
    
    Err(format!("Invalid bootstrap peer format: {}", addr_str))
}


/// Network manager for handling all network operations
pub struct NetworkManager {
    /// P2P network instance
    p2p_network: Arc<P2PNetwork>,
    /// Command sender for network operations
    command_sender: mpsc::Sender<NetworkCommand>,
    /// Event receiver for network events
    event_receiver: Arc<tokio::sync::RwLock<Option<mpsc::Receiver<NetworkEvent>>>>,
    /// Network statistics
    stats: Arc<tokio::sync::RwLock<NetworkStats>>,
    /// Connected peers
    connected_peers: Arc<tokio::sync::RwLock<HashMap<PeerId, PeerInfo>>>,
    /// Network configuration
    config: crate::config::NetworkConfig,
    /// Is the network running
    is_running: Arc<std::sync::atomic::AtomicBool>,
    /// Event processing task handle
    event_task: Arc<tokio::sync::RwLock<Option<tokio::task::JoinHandle<()>>>>,
    /// Transaction mempool for processing received transactions
    mempool: Option<Arc<crate::mempool::TransactionPool>>,
    /// Chain state for processing received blocks
    chain_state: Option<Arc<std::sync::RwLock<crate::storage::ChainState>>>,
}

impl NetworkManager {
    /// Create a new network manager
    pub async fn new(
        config: crate::config::NetworkConfig,
        genesis_hash: [u8; 32],
    ) -> Result<Self, Box<dyn std::error::Error>> {
        info!("Creating network manager with config: {:?}", config);

        let (p2p_network, command_sender, event_receiver) =
            initialize_network(&config, genesis_hash).await?;

        Ok(Self {
            p2p_network: Arc::new(p2p_network),
            command_sender,
            event_receiver: Arc::new(tokio::sync::RwLock::new(Some(event_receiver))),
            stats: Arc::new(tokio::sync::RwLock::new(NetworkStats::default())),
            connected_peers: Arc::new(tokio::sync::RwLock::new(HashMap::new())),
            config,
            is_running: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            event_task: Arc::new(tokio::sync::RwLock::new(None)),
            mempool: None, // Will be set later
            chain_state: None, // Will be set later
        })
    }
    
    /// Set mempool reference for processing received transactions
    pub fn set_mempool(&mut self, mempool: Arc<crate::mempool::TransactionPool>) {
        self.mempool = Some(mempool);
    }
    
    /// Set chain state reference for processing received blocks
    pub fn set_chain_state(&mut self, chain_state: Arc<std::sync::RwLock<crate::storage::ChainState>>) {
        self.chain_state = Some(chain_state);
    }

    /// Start the network manager
    pub async fn start(&self) -> Result<(), Box<dyn std::error::Error>> {
        info!("Starting network manager");

        self.is_running
            .store(true, std::sync::atomic::Ordering::SeqCst);

        // Start the P2P network (NetworkManager doesn't use proxy requests)
        self.p2p_network.start(None).await?;

        // Parse the listen address and extract port
        let listen_port = if let Some(port_str) = self.config.listen_addr.split('/').last() {
            port_str.parse::<u16>().unwrap_or(8000)
        } else {
            8000
        };

        // Start listening on configured address
        let listen_addr = format!("/ip4/0.0.0.0/tcp/{}", listen_port)
            .parse::<libp2p::Multiaddr>()
            .map_err(|e| format!("Invalid listen address: {}", e))?;

        self.command_sender
            .send(NetworkCommand::StartListening(listen_addr))
            .await
            .map_err(|e| format!("Failed to send listen command: {}", e))?;

        // Start event processing loop
        let event_receiver = Arc::clone(&self.event_receiver);
        let stats = Arc::clone(&self.stats);
        let peers = Arc::clone(&self.connected_peers);
        let is_running = Arc::clone(&self.is_running);
        let mempool = self.mempool.clone();
        let chain_state = self.chain_state.clone();

        let task = tokio::spawn(async move {
            Self::event_processing_loop(event_receiver, stats, peers, is_running, mempool, chain_state).await;
        });

        *self.event_task.write().await = Some(task);

        // Connect to bootstrap peers
        for peer_addr in &self.config.bootstrap_nodes {
            if let Err(e) = self.connect_to_peer(peer_addr).await {
                warn!("Failed to connect to bootstrap peer {}: {}", peer_addr, e);
            }
        }

        info!("Network manager started successfully");
        Ok(())
    }

    /// Stop the network manager
    pub async fn stop(&self) -> Result<(), Box<dyn std::error::Error>> {
        info!("Stopping network manager");

        self.is_running
            .store(false, std::sync::atomic::Ordering::SeqCst);

        // Stop the event processing task
        if let Some(task) = self.event_task.write().await.take() {
            task.abort();
        }

        // Stop the P2P network
        self.p2p_network.stop().await?;

        info!("Network manager stopped");
        Ok(())
    }

    /// Connect to a peer
    pub async fn connect_to_peer(&self, peer_addr: &str) -> Result<(), String> {
        debug!("Connecting to peer: {}", peer_addr);

        let command = NetworkCommand::ConnectToPeer(peer_addr.to_string());
        self.command_sender
            .send(command)
            .await
            .map_err(|e| format!("Failed to send connect command: {}", e))?;

        Ok(())
    }

    /// Disconnect from a peer
    pub async fn disconnect_from_peer(&self, peer_id: &PeerId) -> Result<(), String> {
        debug!("Disconnecting from peer: {}", peer_id);

        let command = NetworkCommand::DisconnectPeer(*peer_id);
        self.command_sender
            .send(command)
            .await
            .map_err(|e| format!("Failed to send disconnect command: {}", e))?;

        Ok(())
    }

    /// Broadcast a message to all connected peers
    pub async fn broadcast_message(&self, message: ProtocolMessage) -> Result<(), String> {
        debug!("Broadcasting message to all peers");

        let command = NetworkCommand::Broadcast(message);
        self.command_sender
            .send(command)
            .await
            .map_err(|e| format!("Failed to send broadcast command: {}", e))?;

        Ok(())
    }

    /// Send a message to a specific peer
    pub async fn send_to_peer(
        &self,
        peer_id: &PeerId,
        message: ProtocolMessage,
    ) -> Result<(), String> {
        debug!("Sending message to peer: {}", peer_id);

        let command = NetworkCommand::SendToPeer {
            peer_id: *peer_id,
            message,
        };
        self.command_sender
            .send(command)
            .await
            .map_err(|e| format!("Failed to send message command: {}", e))?;

        Ok(())
    }

    /// Announce a block to the network
    pub async fn announce_block(
        &self,
        block: supernova_core::types::block::Block,
        height: u64,
        total_difficulty: u64,
    ) -> Result<(), String> {
        debug!("Announcing block at height {} to network", height);

        let command = NetworkCommand::AnnounceBlock {
            block,
            height,
            total_difficulty,
        };
        self.command_sender
            .send(command)
            .await
            .map_err(|e| format!("Failed to send announce block command: {}", e))?;

        Ok(())
    }

    /// Announce a transaction to the network
    pub async fn announce_transaction(
        &self,
        transaction: supernova_core::types::transaction::Transaction,
        fee_rate: u64,
    ) -> Result<(), String> {
        debug!("Announcing transaction to network");

        let command = NetworkCommand::AnnounceTransaction {
            transaction,
            fee_rate,
        };
        self.command_sender
            .send(command)
            .await
            .map_err(|e| format!("Failed to send announce transaction command: {}", e))?;

        Ok(())
    }

    /// Request blocks from the network
    pub async fn request_blocks(
        &self,
        block_hashes: Vec<[u8; 32]>,
        preferred_peer: Option<PeerId>,
    ) -> Result<(), String> {
        debug!("Requesting {} blocks from network", block_hashes.len());

        let command = NetworkCommand::RequestBlocks {
            block_hashes,
            preferred_peer,
        };
        self.command_sender
            .send(command)
            .await
            .map_err(|e| format!("Failed to send request blocks command: {}", e))?;

        Ok(())
    }

    /// Request headers from the network
    pub async fn request_headers(
        &self,
        start_height: u64,
        end_height: u64,
        preferred_peer: Option<PeerId>,
    ) -> Result<(), String> {
        debug!(
            "Requesting headers from {} to {} from network",
            start_height, end_height
        );

        let command = NetworkCommand::RequestHeaders {
            start_height,
            end_height,
            preferred_peer,
        };
        self.command_sender
            .send(command)
            .await
            .map_err(|e| format!("Failed to send request headers command: {}", e))?;

        Ok(())
    }

    /// Ban a peer for misbehavior
    pub async fn ban_peer(
        &self,
        peer_id: &PeerId,
        reason: String,
        duration: Option<Duration>,
    ) -> Result<(), String> {
        warn!("Banning peer {} for: {}", peer_id, reason);

        let command = NetworkCommand::BanPeer {
            peer_id: *peer_id,
            reason,
            duration,
        };
        self.command_sender
            .send(command)
            .await
            .map_err(|e| format!("Failed to send ban peer command: {}", e))?;

        Ok(())
    }

    /// Get network statistics
    pub async fn get_stats(&self) -> NetworkStats {
        self.stats.read().await.clone()
    }

    /// Get connected peers
    pub async fn get_connected_peers(&self) -> Vec<PeerInfo> {
        self.connected_peers
            .read()
            .await
            .values()
            .cloned()
            .collect()
    }

    /// Get peer count
    pub async fn get_peer_count(&self) -> usize {
        self.connected_peers.read().await.len()
    }

    /// Get local peer ID
    pub fn get_local_peer_id(&self) -> PeerId {
        self.p2p_network.local_peer_id()
    }

    /// Check if the network is running
    pub fn is_running(&self) -> bool {
        self.is_running.load(std::sync::atomic::Ordering::SeqCst)
    }

    /// Get network health metrics
    pub async fn get_network_health(&self) -> NetworkHealth {
        self.p2p_network.get_network_health().await
    }

    /// Get P2P network reference for API access
    pub fn p2p_network(&self) -> &Arc<P2PNetwork> {
        &self.p2p_network
    }

    /// Event processing loop
    async fn event_processing_loop(
        event_receiver: Arc<tokio::sync::RwLock<Option<mpsc::Receiver<NetworkEvent>>>>,
        stats: Arc<tokio::sync::RwLock<NetworkStats>>,
        peers: Arc<tokio::sync::RwLock<HashMap<PeerId, PeerInfo>>>,
        is_running: Arc<std::sync::atomic::AtomicBool>,
        mempool: Option<Arc<crate::mempool::TransactionPool>>,
        chain_state: Option<Arc<std::sync::RwLock<crate::storage::ChainState>>>,
    ) {
        info!("Starting network event processing loop");

        let receiver = {
            let mut guard = event_receiver.write().await;
            guard.take()
        };

        if let Some(mut rx) = receiver {
            while is_running.load(std::sync::atomic::Ordering::SeqCst) {
                match rx.recv().await {
                    Some(event) => {
                        Self::handle_network_event(event, &stats, &peers, &mempool, &chain_state).await;
                    }
                    None => {
                        warn!("Network event channel closed");
                        break;
                    }
                }
            }
        }

        info!("Network event processing loop stopped");
    }

    /// Handle a network event
    async fn handle_network_event(
        event: NetworkEvent,
        stats: &Arc<tokio::sync::RwLock<NetworkStats>>,
        peers: &Arc<tokio::sync::RwLock<HashMap<PeerId, PeerInfo>>>,
        mempool: &Option<Arc<crate::mempool::TransactionPool>>,
        chain_state: &Option<Arc<std::sync::RwLock<crate::storage::ChainState>>>,
    ) {
        match event {
            NetworkEvent::PeerConnected(peer_info) => {
                info!("Peer connected: {}", peer_info.peer_id);
                peers.write().await.insert(peer_info.peer_id, peer_info);

                // Update stats
                let mut stats_guard = stats.write().await;
                stats_guard.total_connections += 1;
                stats_guard.active_connections += 1;
            }
            NetworkEvent::PeerDisconnected(peer_id) => {
                info!("Peer disconnected: {}", peer_id);
                peers.write().await.remove(&peer_id);

                // Update stats
                let mut stats_guard = stats.write().await;
                stats_guard.active_connections = stats_guard.active_connections.saturating_sub(1);
            }
            NetworkEvent::MessageReceived { peer_id, message } => {
                debug!("Message received from {}: {:?}", peer_id, message);

                // Update stats
                let mut stats_guard = stats.write().await;
                stats_guard.messages_received += 1;
                stats_guard.bytes_received += message.size_hint();
            }
            NetworkEvent::MessageSent { peer_id, message } => {
                debug!("Message sent to {}: {:?}", peer_id, message);

                // Update stats
                let mut stats_guard = stats.write().await;
                stats_guard.messages_sent += 1;
                stats_guard.bytes_sent += message.size_hint();
            }
            NetworkEvent::Error { peer_id, error } => {
                warn!("Network error with peer {:?}: {}", peer_id, error);

                // Update stats
                let mut stats_guard = stats.write().await;
                stats_guard.connection_errors += 1;
            }
            NetworkEvent::NewBlock {
                block,
                height,
                total_difficulty,
                from_peer,
            } => {
                info!(
                    "Received new block at height {} from peer {:?}",
                    height, from_peer
                );

                // Update stats
                let mut stats_guard = stats.write().await;
                stats_guard.blocks_received += 1;
                drop(stats_guard);

                // Process the block if chain_state is available using spawn_blocking for thread safety
                if let Some(chain) = chain_state {
                    let chain_clone = Arc::clone(&chain);
                    let block_clone = block.clone();
                    let block_hash_local = block.hash();
                    let block_height = block.height();
                    
                    match tokio::task::spawn_blocking(move || {
                        tokio::runtime::Handle::current().block_on(async move {
                            match chain_clone.write() {
                                Ok(mut chain_guard) => chain_guard.add_block(&block_clone).await,
                                Err(e) => Err(crate::storage::StorageError::DatabaseError(
                                    format!("Lock poisoned: {}", e)
                                )),
                            }
                        })
                    }).await {
                        Ok(Ok(_)) => {
                            info!("Successfully added received block {} at height {} to chain", 
                                hex::encode(&block_hash_local[..8]), block_height);
                        }
                        Ok(Err(e)) => {
                            warn!("Failed to add received block to chain: {}", e);
                        }
                        Err(e) => {
                            error!("Task join error processing block: {}", e);
                        }
                    }
                } else {
                    debug!("Chain state not available, cannot process received block");
                }
            }
            NetworkEvent::NewTransaction {
                transaction,
                fee_rate,
                from_peer,
            } => {
                debug!("Received new transaction from peer {:?}", from_peer);

                // Update stats
                let mut stats_guard = stats.write().await;
                stats_guard.transactions_received += 1;
                drop(stats_guard);

                // Add transaction to mempool if available
                if let Some(pool) = mempool {
                    let tx_hash = transaction.hash();
                    
                    // Check if already in mempool
                    if pool.get_transaction(&tx_hash).is_some() {
                        debug!("Transaction {} already in mempool, ignoring", hex::encode(&tx_hash[..8]));
                        return;
                    }
                    
                    match pool.add_transaction(transaction, fee_rate) {
                        Ok(_) => {
                            info!("Added received transaction {} to mempool", hex::encode(&tx_hash[..8]));
                        }
                        Err(e) => {
                            warn!("Failed to add received transaction to mempool: {}", e);
                        }
                    }
                } else {
                    debug!("Mempool not available, cannot process received transaction");
                }
            }
            NetworkEvent::BlockHeaders {
                headers,
                total_difficulty,
                from_peer,
            } => {
                debug!(
                    "Received {} headers from peer {:?}",
                    headers.len(),
                    from_peer
                );

                // Update stats
                let mut stats_guard = stats.write().await;
                stats_guard.headers_received += headers.len() as u64;
            }
            NetworkEvent::Started => {
                info!("Network started successfully");
            }
            NetworkEvent::Stopped => {
                info!("Network stopped");
            }
            NetworkEvent::Listening(addr) => {
                info!("Network listening on {}", addr);
            }
            _ => {
                // Handle other events as needed
                debug!("Unhandled network event: {:?}", event);
            }
        }
    }
}

/// Network statistics
#[derive(Debug, Clone, Default)]
pub struct NetworkStats {
    /// Total number of connections made
    pub total_connections: u64,
    /// Current active connections
    pub active_connections: u64,
    /// Total messages sent
    pub messages_sent: u64,
    /// Total messages received
    pub messages_received: u64,
    /// Total bytes sent
    pub bytes_sent: u64,
    /// Total bytes received
    pub bytes_received: u64,
    /// Connection errors
    pub connection_errors: u64,
    /// Network uptime in seconds
    pub uptime_seconds: u64,
    /// Blocks received
    pub blocks_received: u64,
    /// Transactions received
    pub transactions_received: u64,
    /// Headers received
    pub headers_received: u64,
}

/// Trait for message size estimation
trait MessageSizeHint {
    fn size_hint(&self) -> u64;
}

impl MessageSizeHint for ProtocolMessage {
    fn size_hint(&self) -> u64 {
        // Simplified size estimation
        match self {
            ProtocolMessage::Ping(_) => 8,
            ProtocolMessage::Pong(_) => 8,
            ProtocolMessage::GetHeaders { .. } => 32,
            ProtocolMessage::Headers { headers, .. } => headers.len() as u64 * 80, // 80 bytes per header
            ProtocolMessage::GetBlocks(_) => 256,
            ProtocolMessage::Block(_) => 1024 * 1024, // 1MB estimate
            ProtocolMessage::Transaction { transaction } => transaction.len() as u64, // 512 bytes estimate
            ProtocolMessage::GetData(_) => 256,
            _ => 64, // Default estimate
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_network_config_default() {
        let config = NetworkConfig::default();
        assert_eq!(config.network_id, "supernova");
        assert_eq!(config.listen_port, 8333);
        assert_eq!(config.max_peers, MAX_PEERS);
    }

    #[tokio::test]
    async fn test_network_manager_creation() {
        let config = NetworkConfig::default();
        let genesis_hash = [0u8; 32];

        let manager = NetworkManager::new(config, genesis_hash).await;
        assert!(manager.is_ok());
    }

    #[test]
    fn test_network_stats() {
        let mut stats = NetworkStats::default();
        stats.total_connections = 5;
        stats.active_connections = 3;

        assert_eq!(stats.total_connections, 5);
        assert_eq!(stats.active_connections, 3);
    }

    #[test]
    fn test_message_size_hint() {
        let ping_msg = ProtocolMessage::Ping(12345);
        assert_eq!(ping_msg.size_hint(), 8);

        let pong_msg = ProtocolMessage::Pong(54321);
        assert_eq!(pong_msg.size_hint(), 8);
    }
}

// Implementation to be continued...
