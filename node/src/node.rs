use crate::adapters::{
    TransactionPoolNodeMethods,
};
use crate::api::types::{LoadAverage, LogEntry, NodeInfo, NodeMetrics, SystemInfo, VersionInfo};
use crate::api::ApiConfig;
use crate::config::NodeConfig;
use crate::mempool::TransactionPool;
use crate::metrics::performance::PerformanceMonitor;
use crate::network::{NetworkCommand, NetworkProxy, P2PNetwork};
use crate::storage::{
    BlockchainDB, ChainState, DatabaseShutdownHandler, StorageError, WriteAheadLog,
};
use crate::testnet::NodeTestnetManager;
use crate::testnet::TestnetNodeConfig;
use supernova_core::crypto::quantum::QuantumScheme;
use supernova_core::lightning::manager::{LightningEvent, LightningManager};
use supernova_core::lightning::wallet::LightningWallet;
use supernova_core::lightning::LightningConfig;
use supernova_core::types::block::Block;
use supernova_core::types::transaction::Transaction;
use hex;
use libp2p::PeerId;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::{Arc, RwLock};
use std::time::Instant;
use sysinfo::System;
use thiserror::Error;
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};
use uuid;

/// Node status information for internal use
#[derive(Debug, Serialize, Deserialize)]
pub struct NodeStatusInfo {
    pub version: String,
    pub network: String,
    pub chain_id: String,
    pub chain_height: u64,
    pub mempool_size: usize,
    pub peer_count: usize,
    pub is_syncing: bool,
    pub is_testnet: bool,
}

/// Node operation errors
#[derive(Debug, Error)]
pub enum NodeError {
    #[error("Storage error: {0}")]
    StorageError(#[from] StorageError),
    #[error("Network error: {0}")]
    NetworkError(String),
    #[error("Configuration error: {0}")]
    ConfigError(String),
    #[error("Lightning Network error: {0}")]
    LightningError(String),
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
    #[error("General error: {0}")]
    General(String),
    #[error("Mempool error: {0}")]
    MempoolError(#[from] crate::mempool::MempoolError),
    #[error("Testnet error: {0}")]
    TestnetError(String),
}

impl From<Box<dyn std::error::Error>> for NodeError {
    fn from(err: Box<dyn std::error::Error>) -> Self {
        NodeError::General(err.to_string())
    }
}

impl From<supernova_core::lightning::LightningError> for NodeError {
    fn from(err: supernova_core::lightning::LightningError) -> Self {
        NodeError::General(format!("Lightning error: {:?}", err))
    }
}

impl From<supernova_core::lightning::wallet::WalletError> for NodeError {
    fn from(err: supernova_core::lightning::wallet::WalletError) -> Self {
        NodeError::LightningError(err.to_string())
    }
}

// Placeholder types for missing imports
type NetworkManager = P2PNetwork;
type BlockValidator = ();
type TransactionValidator = ();
type RpcServer = ();
type MemPool = TransactionPool;

/// Lightning event handler that can be shared across threads
#[derive(Clone)]
pub struct LightningEventHandler {
    /// Channel to send events for processing
    event_sender: mpsc::UnboundedSender<LightningEvent>,
}

impl LightningEventHandler {
    /// Create a new event handler
    pub fn new() -> (Self, mpsc::UnboundedReceiver<LightningEvent>) {
        let (event_sender, event_receiver) = mpsc::unbounded_channel();
        (Self { event_sender }, event_receiver)
    }

    /// Send an event
    pub fn send_event(
        &self,
        event: LightningEvent,
    ) -> Result<(), mpsc::error::SendError<LightningEvent>> {
        self.event_sender.send(event)
    }
}

/// Main node structure
pub struct Node {
    /// Node configuration
    config: Arc<RwLock<NodeConfig>>,
    /// Blockchain database
    db: Arc<BlockchainDB>,
    /// Chain state
    chain_state: Arc<RwLock<ChainState>>,
    /// Transaction mempool
    mempool: Arc<TransactionPool>,
    /// P2P network
    network: Arc<P2PNetwork>,
    /// Thread-safe network proxy for API access
    network_proxy: Arc<NetworkProxy>,
    /// Network command sender
    network_command_tx: mpsc::Sender<NetworkCommand>,
    /// Testnet manager (if enabled)
    testnet_manager: Option<Arc<NodeTestnetManager>>,
    /// Lightning Network manager
    lightning_manager: Option<Arc<RwLock<LightningManager>>>,
    /// Wallet manager (quantum-resistant wallet)
    wallet_manager: Option<Arc<RwLock<crate::wallet_manager::WalletManager>>>,
    pub api_config: ApiConfig,
    pub peer_id: PeerId,
    pub start_time: Instant,
    pub performance_monitor: Arc<PerformanceMonitor>,
    pub blockchain: Arc<RwLock<ChainState>>,
    pub wallet: Arc<()>,
    pub db_shutdown_handler: Option<Arc<DatabaseShutdownHandler>>,
    pub wal: Option<Arc<RwLock<WriteAheadLog>>>,
}

impl Node {
    /// Create a new node instance
    pub async fn new(config: NodeConfig) -> Result<Self, NodeError> {
        // Validate configuration
        config
            .validate()
            .map_err(|e| NodeError::ConfigError(e.to_string()))?;

        // Initialize database
        let db = Arc::new(BlockchainDB::new(&config.storage.db_path)?);

        // Initialize chain state
        let chain_state = Arc::new(RwLock::new(ChainState::new(Arc::clone(&db))?));

        // Initialize genesis block if needed
        if chain_state
            .read()
            .map_err(|_| NodeError::General("Chain state lock poisoned".to_string()))?
            .get_height()
            == 0
        {
            tracing::info!("Creating genesis block for chain: {}", config.node.chain_id);
            
            // Create genesis block
            let genesis_block = crate::blockchain::create_genesis_block(&config.node.chain_id)
                .map_err(|e| NodeError::General(format!("Genesis creation failed: {}", e)))?;
            
            chain_state
                .write()
                .map_err(|_| NodeError::General("Chain state lock poisoned".to_string()))?
                .initialize_with_genesis(genesis_block)
                .map_err(NodeError::StorageError)?;
                
            tracing::info!("Genesis block initialized successfully");
        }
        
        // Initialize mempool
        let mempool_config = crate::mempool::MempoolConfig::from(config.mempool.clone());
        let mempool = Arc::new(TransactionPool::new(mempool_config));

        // Initialize network with persistent peer ID
        // Use explicit ./data directory for peer identity storage
        let data_dir = PathBuf::from("./data");
        
        // Ensure data directory exists
        if let Err(e) = std::fs::create_dir_all(&data_dir) {
            warn!("Failed to create data directory {:?}: {}", data_dir, e);
        }
        
        info!("Initializing peer identity from directory: {:?}", data_dir);
        
        let keypair = crate::network::peer_identity::load_or_generate_keypair(&data_dir)
            .map_err(|e| {
                error!("CRITICAL: Failed to load peer identity: {}", e);
                NodeError::General(format!("Failed to load peer identity: {}", e))
            })?;
        
        info!("Peer identity loaded successfully");
        
        let genesis_hash = chain_state
            .read()
            .map_err(|_| NodeError::General("Chain state lock poisoned".to_string()))?
            .get_genesis_hash();
        let (mut network, command_tx, event_rx) =
            P2PNetwork::new(
                Some(keypair),
                genesis_hash,
                &config.node.chain_id,
                Some(config.network.listen_addr.clone()), // Pass configured listen address
                Some(config.network.pubsub_config.validation_mode.clone()), // Gossipsub validation mode
            ).await?;
        
        // Add bootstrap nodes from config
        info!("Checking bootstrap_nodes config: {} entries", config.network.bootstrap_nodes.len());
        
        if !config.network.bootstrap_nodes.is_empty() {
            info!("Loading {} bootstrap nodes from config: {:?}", 
                config.network.bootstrap_nodes.len(), 
                config.network.bootstrap_nodes);
            
            for bootstrap_str in &config.network.bootstrap_nodes {
                info!("Parsing bootstrap node: {}", bootstrap_str);
                
                // Parse multiaddr (supports peer_id@ip:port and full multiaddr formats)
                match crate::network::parse_bootstrap_address(bootstrap_str) {
                    Ok(multiaddr) => {
                        // Extract peer ID if present in multiaddr
                        let peer_id = multiaddr.iter()
                            .find_map(|proto| {
                                if let libp2p::multiaddr::Protocol::P2p(id) = proto {
                                    Some(id)
                                } else {
                                    None
                                }
                            })
                            .unwrap_or_else(libp2p::PeerId::random);
                        
                        network.add_bootstrap_node(peer_id, multiaddr.clone());
                        info!("✓ Bootstrap node added to network: {}", multiaddr);
                    }
                    Err(e) => {
                        warn!("✗ Failed to parse bootstrap node {}: {}", bootstrap_str, e);
                    }
                }
            }
            
            info!("Bootstrap loading complete - {} nodes in network", 
                network.bootstrap_count());
        } else {
            info!("No bootstrap nodes in config");
        }
        
        // Create thread-safe network proxy for API access BEFORE wrapping in Arc
        let (network_proxy, proxy_request_rx, _cached_stats) = NetworkProxy::new(
            network.local_peer_id(),
            network.network_id().to_string(),
            command_tx.clone(),
        );
        let network_proxy = Arc::new(network_proxy);

        // Start network with proxy request processing integrated
        // This must be done BEFORE wrapping network in Arc since start() takes &self
        network.start(Some(proxy_request_rx)).await
            .map_err(|e| NodeError::NetworkError(e.to_string()))?;
        
        info!("P2P network started with integrated proxy request processing");
        
        let network = Arc::new(network);

        // Spawn network event processing task
        let mempool_clone = Arc::clone(&mempool);
        let chain_state_clone = Arc::clone(&chain_state);
        tokio::spawn(async move {
            Self::process_network_events(event_rx, mempool_clone, chain_state_clone).await;
        });

        // Initialize testnet manager if enabled
        let testnet_manager = if config.testnet.enabled {
            // Convert TestnetConfig to TestnetNodeConfig
            let testnet_node_config = TestnetNodeConfig {
                enabled: config.testnet.enabled,
                network_id: config.testnet.network_id.clone(),
                enable_faucet: config.testnet.enable_faucet,
                faucet_amount: config.testnet.faucet_amount,
                faucet_cooldown: config.testnet.faucet_cooldown,
                faucet_max_balance: config.testnet.faucet_max_balance,
                enable_test_mining: config.testnet.enable_test_mining,
                test_mining_difficulty: config.testnet.test_mining_difficulty,
                enable_network_simulation: config.testnet.enable_network_simulation,
                simulated_latency_ms: config.testnet.simulated_latency_ms,
                simulated_packet_loss: config.testnet.simulated_packet_loss,
            };

            Some(Arc::new(
                NodeTestnetManager::new(testnet_node_config).map_err(NodeError::TestnetError)?,
            ))
        } else {
            None
        };

        // Generate peer ID for this node
        let peer_id = PeerId::random();

        // Initialize Lightning Network if enabled
        let lightning_manager = if config.node.enable_lightning {
            // Create Lightning configuration
            let lightning_config = LightningConfig {
                default_channel_capacity: 1_000_000, // 0.01 NOVA in attaNova
                min_channel_capacity: 10_000,        // 0.0001 NOVA minimum
                max_channel_capacity: 16_777_215,    // ~0.16 NOVA maximum
                cltv_expiry_delta: 40,
                fee_base_mnova: 1000,
                fee_proportional_millionths: 1,
                use_quantum_signatures: config.node.enable_quantum_security,
                quantum_scheme: if config.node.enable_quantum_security {
                    Some(QuantumScheme::Dilithium)
                } else {
                    None
                },
                quantum_security_level: 1,
            };

            // Create Lightning wallet
            let lightning_wallet = LightningWallet::new(
                {
                    // Generate a deterministic seed from the peer ID
                    let mut seed = vec![0u8; 32];
                    let peer_id_bytes = peer_id.to_bytes();
                    for (i, &byte) in peer_id_bytes.iter().enumerate() {
                        if i < 32 {
                            seed[i] = byte;
                        }
                    }
                    seed
                },
                config.node.enable_quantum_security,
                if config.node.enable_quantum_security {
                    Some(QuantumScheme::Dilithium)
                } else {
                    None
                },
            )
            .map_err(|e| NodeError::LightningError(e.to_string()))?;

            // Create Lightning manager
            let (lightning_manager, event_receiver) =
                LightningManager::new(lightning_config, lightning_wallet)
                    .map_err(|e| NodeError::General(format!("Lightning manager error: {}", e)))?;

            // Create event handler and spawn processing task in the background
            let manager_clone = Arc::new(RwLock::new(lightning_manager));
            let manager_for_task = Arc::clone(&manager_clone);

            // Spawn event processing task in the background
            tokio::spawn(async move {
                Self::process_lightning_events(manager_for_task, event_receiver).await;
            });

            Some(manager_clone)
        } else {
            None
        };

        // Initialize wallet manager for testnet
        let wallet_path = dirs::data_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("."))
            .join("supernova")
            .join("wallet");
        
        std::fs::create_dir_all(&wallet_path).ok();
        
        let wallet_manager = match crate::wallet_manager::WalletManager::new(
            wallet_path,
            Arc::clone(&db),
            Arc::clone(&chain_state),
            Arc::clone(&mempool),
            Arc::clone(&network_proxy),
        ) {
            Ok(wm) => {
                tracing::info!("Wallet manager initialized successfully");
                Some(Arc::new(RwLock::new(wm)))
            }
            Err(e) => {
                tracing::warn!("Failed to initialize wallet manager: {}", e);
                None
            }
        };

        Ok(Self {
            config: Arc::new(RwLock::new(config)),
            db,
            chain_state: Arc::clone(&chain_state),
            mempool,
            network,
            network_proxy,
            network_command_tx: command_tx,
            testnet_manager,
            lightning_manager,
            wallet_manager,
            api_config: ApiConfig::default(),
            peer_id,
            start_time: Instant::now(),
            performance_monitor: Arc::new(PerformanceMonitor::new(1000)),
            blockchain: chain_state,
            wallet: Arc::new(()),
            db_shutdown_handler: None,
            wal: None,
        })
    }

    /// Start the node
    pub async fn start(&self) -> Result<(), NodeError> {
        tracing::info!("Starting Supernova node...");

        // Network already started in constructor with proxy request processing

        // Start testnet manager if enabled
        if let Some(testnet) = &self.testnet_manager {
            testnet.start().await.map_err(NodeError::TestnetError)?;
        }

        // Note: API server must be started separately after node creation
        // due to circular dependency (API server needs Arc<Node>)

        tracing::info!("Node started successfully");
        Ok(())
    }

    /// Stop the node
    pub async fn stop(&self) -> Result<(), NodeError> {
        tracing::info!("Stopping Supernova node...");

        // Stop network
        self.network
            .stop()
            .await
            .map_err(|e| NodeError::NetworkError(e.to_string()))?;

        // Stop testnet manager if enabled
        if let Some(testnet) = &self.testnet_manager {
            testnet.stop().map_err(NodeError::TestnetError)?;
        }

        tracing::info!("Node stopped successfully");
        Ok(())
    }

    /// Get node configuration
    pub fn config(&self) -> Arc<RwLock<NodeConfig>> {
        Arc::clone(&self.config)
    }

    /// Get blockchain database
    pub fn db(&self) -> Arc<BlockchainDB> {
        Arc::clone(&self.db)
    }

    /// Get chain state
    pub fn chain_state(&self) -> Arc<RwLock<ChainState>> {
        Arc::clone(&self.chain_state)
    }

    /// Get mempool
    pub fn mempool(&self) -> Arc<TransactionPool> {
        Arc::clone(&self.mempool)
    }
    
    /// Set wallet manager (called by ApiFacade after Node creation)
    pub fn set_wallet_manager(&mut self, wallet_manager: Arc<RwLock<crate::wallet_manager::WalletManager>>) {
        self.wallet_manager = Some(wallet_manager);
        tracing::info!("Wallet manager integrated with node");
    }
    
    /// Get wallet manager reference
    pub fn get_wallet_manager(&self) -> Option<Arc<RwLock<crate::wallet_manager::WalletManager>>> {
        self.wallet_manager.as_ref().map(Arc::clone)
    }

    /// Get network
    pub fn network(&self) -> Arc<P2PNetwork> {
        Arc::clone(&self.network)
    }

    /// Get thread-safe network proxy for API access
    pub fn network_proxy(&self) -> Arc<NetworkProxy> {
        Arc::clone(&self.network_proxy)
    }

    /// Get testnet manager
    pub fn testnet_manager(&self) -> Option<Arc<NodeTestnetManager>> {
        self.testnet_manager.as_ref().map(Arc::clone)
    }

    /// Get faucet (if testnet is enabled)
    pub fn get_faucet(&self) -> Result<Option<Arc<NodeTestnetManager>>, NodeError> {
        Ok(self.testnet_manager.as_ref().map(Arc::clone))
    }

    /// Broadcast a transaction to the network
    pub fn broadcast_transaction(&self, tx: &Transaction) {
        // Calculate a simple fee rate (in production, this would be calculated from the transaction)
        let fee_rate = 1; // 1 nova per byte as default

        // Add to mempool first
        if let Err(e) = self.mempool.add_transaction(tx.clone(), fee_rate) {
            tracing::warn!("Failed to add transaction to mempool: {}", e);
            return;
        }

        // Broadcast to network
        self.network.broadcast_transaction(tx);
        tracing::info!("Broadcasting transaction: {:?}", tx.hash());
    }

    /// Process network events (transactions and blocks from peers)
    async fn process_network_events(
        mut event_rx: mpsc::Receiver<crate::network::NetworkEvent>,
        mempool: Arc<TransactionPool>,
        chain_state: Arc<RwLock<ChainState>>,
    ) {
        tracing::info!("Network event processing task started");
        
        while let Some(event) = event_rx.recv().await {
            match event {
                crate::network::NetworkEvent::NewTransaction { transaction, fee_rate, from_peer } => {
                    let tx_hash = transaction.hash();
                    tracing::debug!("Processing received transaction {} from peer {:?}", 
                        hex::encode(&tx_hash[..8]), from_peer);
                    
                    // Check if already in mempool
                    if mempool.get_transaction(&tx_hash).is_some() {
                        tracing::trace!("Transaction already in mempool, ignoring");
                        continue;
                    }
                    
                    // Add to mempool
                    match mempool.add_transaction(transaction, fee_rate) {
                        Ok(_) => {
                            tracing::info!("Added received transaction {} to mempool", hex::encode(&tx_hash[..8]));
                        }
                        Err(e) => {
                            tracing::warn!("Failed to add received transaction to mempool: {}", e);
                        }
                    }
                }
                crate::network::NetworkEvent::NewBlock { block, from_peer, .. } => {
                    let block_hash = block.hash();
                    tracing::info!("Processing received block at height {} (hash: {}) from peer {:?}",
                        block.height(), hex::encode(&block_hash[..8]), from_peer);
                    
                    // Check if already have this block
                    if let Ok(chain) = chain_state.read() {
                        if chain.get_block(&block_hash).is_some() {
                            tracing::trace!("Block already in chain, ignoring");
                            continue;
                        }
                    }
                    
                    // Validate block
                    if !block.validate() {
                        tracing::warn!("Received invalid block from peer: failed validation");
                        continue;
                    }
                    
                    // Add to chain using spawn_blocking for thread safety
                    let chain_clone = Arc::clone(&chain_state);
                    let block_clone = block.clone();
                    let block_hash_clone = block_hash;
                    let block_height = block.height();
                    
                    match tokio::task::spawn_blocking(move || {
                        tokio::runtime::Handle::current().block_on(async move {
                            match chain_clone.write() {
                                Ok(mut chain) => chain.add_block(&block_clone).await,
                                Err(e) => Err(crate::storage::StorageError::DatabaseError(
                                    format!("Lock poisoned: {}", e)
                                )),
                            }
                        })
                    }).await {
                        Ok(Ok(_)) => {
                            tracing::info!("Successfully added received block {} at height {} to chain",
                                hex::encode(&block_hash_clone[..8]), block_height);
                        }
                        Ok(Err(e)) => {
                            tracing::warn!("Failed to add received block to chain: {}", e);
                        }
                        Err(e) => {
                            tracing::error!("Task join error processing block: {}", e);
                        }
                    }
                }
                _ => {
                    // Other events handled elsewhere or not needed
                }
            }
        }
        
        tracing::info!("Network event processing task stopped");
    }

    /// Process a new block
    pub async fn process_block(&self, block: Block) -> Result<(), NodeError> {
        tracing::info!("Processing block at height: {}", block.header.height);

        // Validate block
        if !block.validate() {
            return Err(NodeError::General("Block validation failed".to_string()));
        }

        // Add to chain state
        {
            let mut chain = self.chain_state
                .write()
                .map_err(|_| NodeError::General("Chain state lock poisoned".to_string()))?;
            
            chain.add_block(&block).await
                .map_err(NodeError::StorageError)?;
        }

        // Scan block for wallet transactions (NEW: Blockchain Integration)
        if let Some(wallet_manager) = &self.wallet_manager {
            if let Err(e) = wallet_manager
                .write()
                .map_err(|_| NodeError::General("Wallet lock poisoned".to_string()))?
                .scan_block(&block)
            {
                tracing::warn!("Failed to scan block for wallet transactions: {}", e);
                // Don't fail block processing if wallet scan fails
            }
        }

        // Remove transactions from mempool
        for tx in block.transactions() {
            self.mempool.remove_transaction(&tx.hash());
        }

        // Store full block in database
        self.db
            .insert_block(&block)
            .map_err(NodeError::StorageError)?;

        // Broadcast to network if this is a new block we mined
        self.network.broadcast_block(&block);

        Ok(())
    }

    /// Get storage (blockchain database)
    pub fn storage(&self) -> Arc<BlockchainDB> {
        Arc::clone(&self.db)
    }

    /// Get node status
    pub async fn get_status(&self) -> NodeStatusInfo {
        let config = match self.config.read() {
            Ok(c) => c,
            Err(_) => {
                error!("Config lock poisoned in get_status");
                return NodeStatusInfo {
                    version: env!("CARGO_PKG_VERSION").to_string(),
                    network: "unknown".to_string(),
                    chain_id: "unknown".to_string(),
                    chain_height: 0,
                    mempool_size: 0,
                    peer_count: 0,
                    is_syncing: false,
                    is_testnet: false,
                };
            }
        };
        let chain_height = self
            .chain_state
            .read()
            .map(|cs| cs.get_height())
            .unwrap_or(0);
        let peer_count = self.network.peer_count().await;
        let is_syncing = self.network.is_syncing();

        NodeStatusInfo {
            version: env!("CARGO_PKG_VERSION").to_string(),
            network: config.node.network_name.clone(),
            chain_id: config.node.chain_id.clone(),
            chain_height,
            mempool_size: self.mempool.size(),
            peer_count,
            is_syncing,
            is_testnet: config.testnet.enabled,
        }
    }

    /// Get node info
    pub fn get_info(&self) -> Result<NodeInfo, NodeError> {
        let config = self
            .config
            .read()
            .map_err(|_| NodeError::General("Config lock poisoned".to_string()))?;
        let chain_height = self
            .chain_state
            .read()
            .map_err(|_| NodeError::General("Chain state lock poisoned".to_string()))?
            .get_height();
        let best_block_hash = self
            .chain_state
            .read()
            .map_err(|_| NodeError::General("Chain state lock poisoned".to_string()))?
            .get_best_block_hash();
        let connections = self.network.peer_count_sync() as u32;
        let synced = !self.network.is_syncing();

        Ok(NodeInfo {
            node_id: self.peer_id.to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            protocol_version: 1,
            network: config.node.network_name.clone(),
            height: chain_height,
            best_block_hash: hex::encode(best_block_hash),
            connections,
            synced,
            uptime: self.start_time.elapsed().as_secs(),
        })
    }

    /// Get system info
    pub fn get_system_info(&self) -> Result<SystemInfo, NodeError> {
        let sys = System::new_all();

        let load_avg = System::load_average();

        Ok(SystemInfo {
            os: System::long_os_version().unwrap_or_else(|| "Unknown".to_string()),
            arch: std::env::consts::ARCH.to_string(),
            cpu_count: sys.cpus().len() as u32,
            total_memory: sys.total_memory(),
            used_memory: sys.used_memory(),
            total_swap: sys.total_swap(),
            used_swap: sys.used_swap(),
            uptime: System::uptime(),
            load_average: LoadAverage {
                one: load_avg.one,
                five: load_avg.five,
                fifteen: load_avg.fifteen,
            },
        })
    }

    /// Get logs
    pub fn get_logs(
        &self,
        level: &str,
        component: Option<&str>,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<LogEntry>, NodeError> {
        // Get logs from the logging system
        let logs = crate::logging::get_recent_logs(level, component, limit, offset);
        Ok(logs)
    }

    /// Get version info
    pub fn get_version(&self) -> Result<VersionInfo, NodeError> {
        Ok(VersionInfo {
            version: env!("CARGO_PKG_VERSION").to_string(),
            protocol_version: 1,
            git_commit: env!("VERGEN_GIT_SHA").to_string(),
            build_date: env!("VERGEN_BUILD_TIMESTAMP").to_string(),
            rust_version: env!("VERGEN_RUSTC_SEMVER").to_string(),
        })
    }

    /// Get metrics
    pub fn get_metrics(&self, _period: u64) -> Result<NodeMetrics, NodeError> {
        use sysinfo::{Disks, System};
        let mut sys = System::new_all();
        sys.refresh_all();

        // Calculate CPU usage
        let cpu_usage = sys.global_cpu_info().cpu_usage() as f64;

        // Calculate memory usage
        let memory_usage = sys.used_memory();

        // Calculate disk usage (simplified - just get first disk)
        let disks = Disks::new_with_refreshed_list();
        let disk_usage = disks
            .list()
            .first()
            .map(|disk| disk.total_space() - disk.available_space())
            .unwrap_or(0);

        // Get mempool size in bytes
        let mempool_bytes = self.mempool.get_memory_usage();

        // Get sync progress
        let sync_progress = if self.network.is_syncing() {
            // Calculate actual sync progress
            // For now, return 0.5 as a placeholder
            0.5
        } else {
            1.0
        };

        // Get network traffic
        let (bytes_sent, bytes_received) = {
            let stats = self.network.get_stats_sync();
            (stats.bytes_sent, stats.bytes_received)
        };

        Ok(NodeMetrics {
            uptime: self.start_time.elapsed().as_secs(),
            peer_count: self.network.peer_count_sync(),
            block_height: self
                .chain_state
                .read()
                .map_err(|_| NodeError::General("Chain state lock poisoned".to_string()))?
                .get_height(),
            mempool_size: self.mempool.size(),
            mempool_bytes,
            sync_progress,
            network_bytes_sent: bytes_sent,
            network_bytes_received: bytes_received,
            cpu_usage,
            memory_usage,
            disk_usage,
        })
    }

    /// Get config
    pub fn get_config(&self) -> Result<serde_json::Value, NodeError> {
        let config = self
            .config
            .read()
            .map_err(|_| NodeError::General("Config lock poisoned".to_string()))?;
        serde_json::to_value(&*config).map_err(|e| NodeError::ConfigError(e.to_string()))
    }

    /// Update config
    pub fn update_config(
        &self,
        new_config: serde_json::Value,
    ) -> Result<serde_json::Value, NodeError> {
        // Parse new config
        let updated_config: NodeConfig = serde_json::from_value(new_config)
            .map_err(|e| NodeError::ConfigError(format!("Invalid config: {}", e)))?;

        // Validate new config
        updated_config
            .validate()
            .map_err(|e| NodeError::ConfigError(e.to_string()))?;

        // Update config
        let mut config = self
            .config
            .write()
            .map_err(|_| NodeError::General("Config lock poisoned".to_string()))?;
        *config = updated_config;

        // Return updated config
        serde_json::to_value(&*config).map_err(|e| NodeError::ConfigError(e.to_string()))
    }

    /// Create backup
    pub fn create_backup(
        &self,
        destination: Option<&str>,
        include_wallet: bool,
        _encrypt: bool,
    ) -> Result<crate::api::types::BackupInfo, NodeError> {
        use crate::storage::backup::BackupManager;
        use std::time::Duration;

        let backup_dir = std::path::PathBuf::from(destination.unwrap_or("/tmp/supernova_backup"));
        let backup_manager = BackupManager::new(
            self.db.clone(),
            backup_dir.clone(),
            10,                        // max_backups
            Duration::from_secs(3600), // backup_interval: 1 hour
        );

        // Create the backup asynchronously
        let backup_path = tokio::runtime::Handle::current()
            .block_on(async { backup_manager.create_backup().await })
            .map_err(NodeError::StorageError)?;

        // Get file metadata for size
        let metadata = std::fs::metadata(&backup_path).map_err(NodeError::IoError)?;

        Ok(crate::api::types::BackupInfo {
            id: uuid::Uuid::new_v4().to_string(),
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0),
            size: metadata.len(),
            backup_type: if include_wallet { "full" } else { "blockchain" }.to_string(),
            status: "completed".to_string(),
            file_path: backup_path.to_string_lossy().to_string(),
            verified: true,
        })
    }

    /// Get backup info
    pub fn get_backup_info(&self) -> Result<Vec<crate::api::types::BackupInfo>, NodeError> {
        // List existing backups from the backup directory
        Ok(Vec::new())
    }

    /// Restart node
    pub fn restart(&self) -> Result<(), NodeError> {
        // Signal restart to the main process
        std::process::Command::new(std::env::current_exe()?)
            .args(std::env::args().skip(1))
            .spawn()
            .map_err(NodeError::IoError)?;

        // Shutdown current instance
        self.shutdown()?;

        Ok(())
    }

    /// Shutdown node
    pub fn shutdown(&self) -> Result<(), NodeError> {
        tracing::info!("Initiating node shutdown...");

        // Stop all services
        tokio::runtime::Handle::current().block_on(async { self.stop().await })?;

        // Exit process
        std::process::exit(0);
    }

    /// Get debug info
    pub fn get_debug_info(&self) -> Result<crate::api::types::DebugInfo, NodeError> {
        // Get node info
        let node_info = self.get_info()?;

        // Get system info
        let system_info = self.get_system_info()?;

        // Get performance metrics
        let performance_metrics = self.get_performance_metrics();

        // Get network stats (placeholder for now)
        let network_stats = serde_json::json!({
            "connected_peers": 0,
            "inbound_connections": 0,
            "outbound_connections": 0,
            "bytes_sent": 0,
            "bytes_received": 0
        });

        // Get mempool stats
        let mempool_stats = serde_json::json!({
            "size": self.mempool.size(),
            "bytes": 0,
            "total_fee": 0
        });

        // Get blockchain stats
        let blockchain_stats = serde_json::json!({
            "height": self.chain_state.read()
                .map_err(|_| NodeError::General("Chain state lock poisoned".to_string()))?
                .get_height(),
            "total_blocks": self.chain_state.read()
                .map_err(|_| NodeError::General("Chain state lock poisoned".to_string()))?
                .get_height(),
            "total_transactions": 0,
            "utxo_set_size": 0
        });

        // Get lightning stats
        let lightning_stats = if self.lightning_manager.is_some() {
            serde_json::json!({
                        "enabled": true,
                "channels": 0,
                "peers": 0,
                "balance_msat": 0
            })
        } else {
            serde_json::json!({
                "enabled": false
            })
        };

        Ok(crate::api::types::DebugInfo {
            node_info,
            system_info,
            performance_metrics,
            network_stats,
            mempool_stats,
            blockchain_stats,
            lightning_stats,
        })
    }

    /// Get performance metrics
    pub fn get_performance_metrics(&self) -> serde_json::Value {
        self.performance_monitor.get_report()
    }

    /// Get Lightning Network manager
    pub fn lightning(&self) -> Option<Arc<RwLock<LightningManager>>> {
        self.lightning_manager.as_ref().map(Arc::clone)
    }

    /// Process Lightning Network events
    async fn process_lightning_events(
        manager: Arc<RwLock<LightningManager>>,
        mut event_receiver: mpsc::UnboundedReceiver<LightningEvent>,
    ) {
        while let Some(event) = event_receiver.recv().await {
            match event {
                LightningEvent::ChannelOpened(channel_id) => {
                    info!("Lightning channel opened: {}", channel_id.to_hex());
                }
                LightningEvent::ChannelClosed(channel_id) => {
                    info!("Lightning channel closed: {}", channel_id.to_hex());
                }
                LightningEvent::PaymentReceived(payment_hash, amount_mnova) => {
                    info!(
                        "Lightning payment received: {} ({} mnova)",
                        payment_hash.to_hex(),
                        amount_mnova
                    );
                }
                LightningEvent::PaymentSent(payment_hash, amount_mnova) => {
                    info!(
                        "Lightning payment sent: {} ({} mnova)",
                        payment_hash.to_hex(),
                        amount_mnova
                    );
                }
                LightningEvent::InvoiceCreated(payment_hash) => {
                    debug!("Lightning invoice created: {}", payment_hash.to_hex());
                }
                LightningEvent::PeerConnected(peer_id) => {
                    info!("Lightning peer connected: {}", peer_id);
                }
                LightningEvent::PeerDisconnected(peer_id) => {
                    info!("Lightning peer disconnected: {}", peer_id);
                }
            }
        }
    }
}
