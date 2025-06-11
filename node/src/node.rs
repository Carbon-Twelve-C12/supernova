use crate::storage::{
    BackupManager, BlockchainDB, ChainState, 
    CheckpointManager, CheckpointConfig, CheckpointType,
    RecoveryManager, StorageError, UtxoSet,
    DatabaseShutdownHandler, DatabaseStartupHandler, ShutdownConfig,
    WriteAheadLog, WalError
};
use crate::adapters::{
    ChainStateNodeMethods, BlockNodeMethods, TransactionPoolNodeMethods,
    ResultNodeMethods, CloneableReadGuard, SafeNumericConversion, 
    IVecConversion, WalletConversion
};
use crate::api::{ApiServer, ApiConfig};
use crate::network::P2PNetwork;
use crate::mempool::TransactionPool;
use crate::config::NodeConfig;
use crate::environmental::EnvironmentalMonitor;
use crate::api::types::{NodeInfo, SystemInfo, LogEntry, NodeStatus, VersionInfo, NodeMetrics, FaucetInfo, LoadAverage};
use btclib::crypto::quantum::QuantumScheme;
use btclib::lightning::{LightningConfig, LightningNetworkError};
use btclib::lightning::manager::{LightningManager, ManagerError, LightningEvent};
use btclib::lightning::wallet::LightningWallet;
use std::sync::{Arc, Mutex, RwLock, atomic::AtomicBool};
use std::time::{Instant, Duration};
use tracing::{info, error, warn, debug};
use crate::metrics::performance::{PerformanceMonitor, MetricType};
use thiserror::Error;
use libp2p::PeerId;
use sysinfo::{System, SystemExt, DiskExt, CpuExt};
use chrono::Utc;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use std::str::FromStr;
use serde::{Serialize, Deserialize};
use crate::testnet::NodeTestnetManager;
use btclib::types::transaction::Transaction;
use btclib::types::block::Block;
use hex;

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
    LightningError(LightningNetworkError),
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

impl From<btclib::lightning::LightningError> for NodeError {
    fn from(err: btclib::lightning::LightningError) -> Self {
        NodeError::LightningError(LightningNetworkError::from(err))
    }
}

impl From<btclib::lightning::wallet::WalletError> for NodeError {
    fn from(err: btclib::lightning::wallet::WalletError) -> Self {
        NodeError::LightningError(LightningNetworkError::WalletError(err))
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
    pub fn send_event(&self, event: LightningEvent) -> Result<(), mpsc::error::SendError<LightningEvent>> {
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
    /// Testnet manager (if enabled)
    testnet_manager: Option<Arc<NodeTestnetManager>>,
    /// Lightning Network manager
    lightning_manager: Option<Arc<RwLock<LightningManager>>>,
    /// Lightning event handler
    lightning_event_handler: Option<LightningEventHandler>,
    /// Lightning event processing task
    lightning_event_task: Option<JoinHandle<()>>,
    pub api_server: Option<ApiServer>,
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
        config.validate()
            .map_err(|e| NodeError::ConfigError(e.to_string()))?;
        
        // Initialize database
        let db = Arc::new(BlockchainDB::new(&config.storage.db_path)?);
        
        // Initialize chain state
        let chain_state = Arc::new(RwLock::new(ChainState::new(Arc::clone(&db))?));
        
        // Initialize genesis block if needed
        if chain_state.read().unwrap().get_height() == 0 {
            // Create genesis block
            let genesis_block = crate::blockchain::create_genesis_block(&config.node.chain_id);
            chain_state.write().unwrap().initialize_with_genesis(genesis_block)
                .map_err(|e| NodeError::StorageError(e.into()))?;
        }
        
        // Initialize mempool
        let mempool_config = crate::mempool::MempoolConfig::from(config.mempool.clone());
        let mempool = Arc::new(TransactionPool::new(mempool_config));
        
        // Initialize network
        let keypair = libp2p::identity::Keypair::generate_ed25519();
        let genesis_hash = chain_state.read().unwrap().get_genesis_hash();
        let (network, _command_tx, _event_rx) = P2PNetwork::new(
            Some(keypair),
            genesis_hash,
            &config.node.chain_id,
        ).await?;
        let network = Arc::new(network);
        
        // Initialize testnet manager if enabled
        let testnet_manager = if config.testnet.enabled {
            let testnet_config = crate::testnet::TestnetNodeConfig {
                enabled: config.testnet.enabled,
                network_id: config.node.chain_id.clone(),
                enable_faucet: config.testnet.enable_faucet,
                faucet_amount: config.testnet.faucet_amount,
                faucet_cooldown: config.testnet.faucet_cooldown,
                faucet_max_balance: config.testnet.faucet_max_balance,
                enable_test_mining: config.testnet.enable_test_mining,
                test_mining_difficulty: config.testnet.test_mining_difficulty,
                enable_network_simulation: false,
                simulated_latency_ms: 0,
                simulated_packet_loss: 0.0,
            };
            
            match NodeTestnetManager::new(testnet_config) {
                Ok(manager) => Some(Arc::new(manager)),
                Err(e) => {
                    tracing::warn!("Failed to initialize testnet manager: {}", e);
                    None
                }
            }
        } else {
            None
        };
        
        // Initialize Lightning Network if enabled
        let (lightning_manager, lightning_event_handler, lightning_event_task) = 
            if config.node.enable_lightning {
                // Create Lightning configuration
                let lightning_config = LightningConfig {
                    default_channel_capacity: 10_000_000, // 0.1 BTC
                    min_channel_capacity: 100_000,        // 0.001 BTC
                    max_channel_capacity: 1_000_000_000,  // 10 BTC
                    cltv_expiry_delta: 144,               // ~1 day
                    fee_base_msat: 1000,                  // 1 sat base fee
                    fee_proportional_millionths: 100,     // 0.01% proportional fee
                    use_quantum_signatures: config.node.enable_quantum_security,
                    quantum_scheme: if config.node.enable_quantum_security {
                        Some(QuantumScheme::Dilithium)
                    } else {
                        None
                    },
                    quantum_security_level: 3,
                };
                
                // Create Lightning wallet
                let lightning_wallet = LightningWallet::new(
                    vec![0u8; 32], // TODO: Use proper seed from node wallet
                    config.node.enable_quantum_security,
                    if config.node.enable_quantum_security {
                        Some(QuantumScheme::Dilithium)
                    } else {
                        None
                    },
                ).map_err(|e| NodeError::LightningError(LightningNetworkError::WalletError(e)))?;
                
                // Create Lightning manager
                let (lightning_manager, event_receiver) = LightningManager::new(
                    lightning_config,
                    lightning_wallet,
                ).map_err(|e| NodeError::General(format!("Lightning manager error: {}", e)))?;
                
                // Create event handler
                let (event_handler, event_receiver_2) = LightningEventHandler::new();
                
                // Spawn event processing task
                let manager_clone = Arc::new(RwLock::new(lightning_manager));
                let manager_for_task = Arc::clone(&manager_clone);
                let event_task = tokio::spawn(async move {
                    Self::process_lightning_events(manager_for_task, event_receiver).await;
                });
                
                (Some(manager_clone), Some(event_handler), Some(event_task))
            } else {
                (None, None, None)
            };
        
        Ok(Self {
            config: Arc::new(RwLock::new(config)),
            db,
            chain_state: Arc::clone(&chain_state),
            mempool,
            network,
            testnet_manager,
            lightning_manager,
            lightning_event_handler,
            lightning_event_task,
            api_server: None,
            api_config: ApiConfig::default(),
            peer_id: PeerId::random(),
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
        
        // Start network
        self.network.start().await
            .map_err(|e| NodeError::NetworkError(e.to_string()))?;
        
        // Start testnet manager if enabled
        if let Some(testnet) = &self.testnet_manager {
            testnet.start().await
                .map_err(|e| NodeError::TestnetError(e))?;
        }
        
        tracing::info!("Node started successfully");
        Ok(())
    }
    
    /// Stop the node
    pub async fn stop(&self) -> Result<(), NodeError> {
        tracing::info!("Stopping Supernova node...");
        
        // Stop network
        self.network.stop().await
            .map_err(|e| NodeError::NetworkError(e.to_string()))?;
        
        // Stop testnet manager if enabled
        if let Some(testnet) = &self.testnet_manager {
            testnet.stop()
                .map_err(|e| NodeError::TestnetError(e))?;
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
    
    /// Get network
    pub fn network(&self) -> Arc<P2PNetwork> {
        Arc::clone(&self.network)
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
        // Add to mempool first
        if let Err(e) = self.mempool.add_transaction(tx.clone()) {
            tracing::warn!("Failed to add transaction to mempool: {}", e);
            return;
        }
        
        // Broadcast to network
        self.network.broadcast_transaction(tx);
        tracing::info!("Broadcasting transaction: {:?}", tx.hash());
    }
    
    /// Process a new block
    pub async fn process_block(&self, block: Block) -> Result<(), NodeError> {
        tracing::info!("Processing block at height: {}", block.header.height);
        
        // Validate block
        if !block.validate() {
            return Err(NodeError::General("Block validation failed".to_string()));
        }
        
        // Add to chain state
        self.chain_state.write().unwrap().add_block(&block)
            .map_err(|e| NodeError::StorageError(e.into()))?;
        
        // Remove transactions from mempool
        for tx in block.transactions() {
            self.mempool.remove_transaction(&tx.hash());
        }
        
        // Store full block in database
        self.db.insert_block(&block)
            .map_err(|e| NodeError::StorageError(e))?;
        
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
        let config = self.config.read().unwrap();
        let chain_height = self.chain_state.read().unwrap().get_height() as u64;
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
        let config = self.config.read().unwrap();
        let chain_height = self.chain_state.read().unwrap().get_height() as u64;
        let best_block_hash = self.chain_state.read().unwrap().get_best_block_hash();
        let connections = self.network.peer_count_sync();
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
        use sysinfo::{System, SystemExt};
        let mut sys = System::new_all();
        
        let load_avg = sys.load_average();
        
        Ok(SystemInfo {
            os: sys.long_os_version().unwrap_or_else(|| "Unknown".to_string()),
            arch: std::env::consts::ARCH.to_string(),
            cpu_count: sys.cpus().len() as u32,
            total_memory: sys.total_memory(),
            used_memory: sys.used_memory(),
            total_swap: sys.total_swap(),
            used_swap: sys.used_swap(),
            uptime: sys.uptime(),
            load_average: LoadAverage {
                one: load_avg.one,
                five: load_avg.five,
                fifteen: load_avg.fifteen,
            },
        })
    }
    
    /// Get logs
    pub fn get_logs(&self, level: &str, component: Option<&str>, limit: usize, offset: usize) -> Result<Vec<LogEntry>, NodeError> {
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
    pub fn get_metrics(&self, period: u64) -> Result<NodeMetrics, NodeError> {
        use sysinfo::{System, SystemExt, DiskExt, CpuExt};
        let mut sys = System::new_all();
        sys.refresh_all();
        
        // Calculate CPU usage
        let cpu_usage = sys.global_cpu_info().cpu_usage() as f64;
        
        // Calculate memory usage
        let memory_usage = sys.used_memory();
        
        // Calculate disk usage (simplified - just get first disk)
        let disk_usage = sys.disks().first()
            .map(|disk| disk.total_space() - disk.available_space())
            .unwrap_or(0);
        
        // Get mempool size in bytes
        let mempool_bytes = self.mempool.get_memory_usage();
        
        // Get sync progress
        let sync_progress = if self.network.is_syncing() {
            self.network.get_sync_progress()
        } else {
            1.0
        };
        
        // Get network stats
        let network_stats = self.network.get_stats();
        
        Ok(NodeMetrics {
            uptime: self.start_time.elapsed().as_secs(),
            peer_count: self.network.peer_count_sync(),
            block_height: self.chain_state.read().unwrap().get_height() as u64,
            mempool_size: self.mempool.size(),
            mempool_bytes,
            sync_progress,
            network_bytes_sent: network_stats.bytes_sent,
            network_bytes_received: network_stats.bytes_received,
            cpu_usage,
            memory_usage,
            disk_usage,
        })
    }
    
    /// Get config
    pub fn get_config(&self) -> Result<serde_json::Value, NodeError> {
        let config = self.config.read().unwrap();
        serde_json::to_value(&*config)
            .map_err(|e| NodeError::ConfigError(e.to_string()))
    }
    
    /// Update config
    pub fn update_config(&self, new_config: serde_json::Value) -> Result<serde_json::Value, NodeError> {
        // Parse new config
        let updated_config: NodeConfig = serde_json::from_value(new_config)
            .map_err(|e| NodeError::ConfigError(format!("Invalid config: {}", e)))?;
        
        // Validate new config
        updated_config.validate()
            .map_err(|e| NodeError::ConfigError(e.to_string()))?;
        
        // Update config
        let mut config = self.config.write().unwrap();
        *config = updated_config;
        
        // Return updated config
        serde_json::to_value(&*config)
            .map_err(|e| NodeError::ConfigError(e.to_string()))
    }
    
    /// Create backup
    pub fn create_backup(&self, destination: Option<&str>, include_wallet: bool, encrypt: bool) -> Result<crate::api::types::BackupInfo, NodeError> {
        use crate::storage::backup::BackupManager;
        
        let backup_manager = BackupManager::new(self.db.clone());
        let backup_path = destination.unwrap_or("/tmp/supernova_backup");
        
        let backup_info = backup_manager.create_backup(backup_path, include_wallet, encrypt)
            .map_err(|e| NodeError::StorageError(e.into()))?;
        
        Ok(crate::api::types::BackupInfo {
            id: backup_info.id,
            timestamp: backup_info.timestamp,
            size: backup_info.size,
            path: backup_info.path,
            encrypted: backup_info.encrypted,
            includes_wallet: backup_info.includes_wallet,
        })
    }
    
    /// Get backup info
    pub fn get_backup_info(&self) -> Result<Vec<crate::api::types::BackupInfo>, NodeError> {
        use crate::storage::backup::BackupManager;
        
        let backup_manager = BackupManager::new(self.db.clone());
        let backups = backup_manager.list_backups()
            .map_err(|e| NodeError::StorageError(e.into()))?;
        
        Ok(backups.into_iter().map(|b| crate::api::types::BackupInfo {
            id: b.id,
            timestamp: b.timestamp,
            size: b.size,
            path: b.path,
            encrypted: b.encrypted,
            includes_wallet: b.includes_wallet,
        }).collect())
    }
    
    /// Restart node
    pub fn restart(&self) -> Result<(), NodeError> {
        // Signal restart to the main process
        std::process::Command::new(std::env::current_exe()?)
            .args(std::env::args().skip(1))
            .spawn()
            .map_err(|e| NodeError::IoError(e))?;
        
        // Shutdown current instance
        self.shutdown()?;
        
        Ok(())
    }
    
    /// Shutdown node
    pub fn shutdown(&self) -> Result<(), NodeError> {
        tracing::info!("Initiating node shutdown...");
        
        // Stop all services
        tokio::runtime::Handle::current().block_on(async {
            self.stop().await
        })?;
        
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
            "height": self.chain_state.read().unwrap().get_height(),
            "total_blocks": self.chain_state.read().unwrap().get_height(),
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
                LightningEvent::PaymentReceived(payment_hash, amount_msat) => {
                    info!("Lightning payment received: {} ({} msat)", 
                          payment_hash.to_hex(), amount_msat);
                }
                LightningEvent::PaymentSent(payment_hash, amount_msat) => {
                    info!("Lightning payment sent: {} ({} msat)", 
                          payment_hash.to_hex(), amount_msat);
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