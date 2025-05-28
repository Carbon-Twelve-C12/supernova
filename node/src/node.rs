use crate::storage::{
    BackupManager, BlockchainDB, ChainState, 
    CheckpointManager, CheckpointConfig, CheckpointType,
    RecoveryManager, StorageError, UtxoSet,
    DatabaseShutdownHandler, DatabaseStartupHandler, ShutdownConfig,
    WriteAheadLog, WalError
};
use crate::api::{ApiServer, ApiConfig};
use crate::network::P2PNetwork;
use crate::mempool::TransactionPool;
use crate::config::NodeConfig;
use crate::environmental::EnvironmentalMonitor;
use crate::api::types::{NodeInfo, SystemInfo, LogEntry, NodeStatus, VersionInfo, NodeMetrics, FaucetInfo};
use btclib::crypto::quantum::QuantumScheme;
use btclib::lightning::{LightningConfig, LightningNetworkError};
use btclib::lightning::manager::{LightningManager, ManagerError, LightningEvent};
use btclib::lightning::wallet::LightningWallet;
use std::sync::{Arc, Mutex, RwLock, atomic::AtomicBool};
use std::time::Instant;
use tracing::{info, error, warn, debug};
use crate::metrics::performance::{PerformanceMonitor, MetricType};
use thiserror::Error;
use libp2p::PeerId;
use sysinfo::{System, SystemExt, CpuExt};
use chrono::Utc;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use std::str::FromStr;
use serde::{Serialize, Deserialize};

/// Node operation errors
#[derive(Error, Debug)]
pub enum NodeError {
    #[error("Storage error: {0}")]
    StorageError(#[from] StorageError),
    #[error("Network error: {0}")]
    NetworkError(String),
    #[error("Configuration error: {0}")]
    ConfigError(String),
    #[error("Lightning Network error: {0}")]
    LightningError(#[from] LightningNetworkError),
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
    #[error("General error: {0}")]
    General(String),
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

pub struct Node {
    pub config: NodeConfig,
    pub chain_state: Arc<RwLock<ChainState>>,
    pub blockchain_db: Arc<RwLock<BlockchainDB>>,
    pub utxo_set: Arc<RwLock<UtxoSet>>,
    pub network_manager: Arc<NetworkManager>,
    pub block_validator: Arc<BlockValidator>,
    pub tx_validator: Arc<TransactionValidator>,
    pub backup_manager: Option<Arc<BackupManager>>,
    pub recovery_manager: Option<Arc<RecoveryManager>>,
    pub checkpoint_manager: Option<Arc<CheckpointManager>>,
    pub rpc_server: Option<Arc<RpcServer>>,
    pub is_running: Arc<AtomicBool>,
    pub mem_pool: Arc<RwLock<MemPool>>,
    /// API server instance
    pub api_server: Option<ApiServer>,
    /// Lightning Network integration
    lightning: Option<Arc<Mutex<LightningManager>>>,
    /// Lightning Network event handler (thread-safe)
    lightning_event_handler: Option<LightningEventHandler>,
    /// Lightning event processing task handle
    lightning_event_task: Option<Arc<Mutex<JoinHandle<()>>>>,
    /// Performance monitor
    pub performance_monitor: Arc<PerformanceMonitor>,
    /// Node peer ID
    pub peer_id: PeerId,
    /// Node start time
    pub start_time: Instant,
    /// Network layer
    pub network: Arc<P2PNetwork>,
    /// Mempool
    pub mempool: Arc<TransactionPool>,
    /// Blockchain reference
    pub blockchain: Arc<ChainState>,
    /// Wallet reference (placeholder)
    pub wallet: Arc<()>,
    /// Database shutdown handler
    db_shutdown_handler: Option<Arc<DatabaseShutdownHandler>>,
    /// Write-ahead log
    wal: Option<Arc<RwLock<WriteAheadLog>>>,
}

impl Node {
    pub async fn new(config: NodeConfig) -> Result<Self, NodeError> {
        // Initialize core components
        let chain_state = Arc::new(RwLock::new(ChainState::new()));
        let blockchain_db = Arc::new(RwLock::new(BlockchainDB::new(&config.data_dir)?));
        let utxo_set = Arc::new(RwLock::new(UtxoSet::new()));
        let network_manager = Arc::new(P2PNetwork::new());
        let block_validator = Arc::new(());
        let tx_validator = Arc::new(());
        let backup_manager = None; // TODO: Initialize if needed
        let recovery_manager = None; // TODO: Initialize if needed
        let rpc_server = None; // TODO: Initialize if needed
        let mem_pool = Arc::new(RwLock::new(TransactionPool::new()));

        // Check if database was cleanly shut down last time
        {
            let db = blockchain_db.read().unwrap();
            
            // Check for clean shutdown marker
            let was_clean_shutdown = match db.get_metadata(b"last_clean_shutdown") {
                Ok(Some(data)) => {
                    // Check if shutdown was recent (within last hour)
                    if let Ok(timestamp_str) = std::str::from_utf8(&data) {
                        if let Ok(timestamp) = timestamp_str.parse::<i64>() {
                            let shutdown_time = chrono::DateTime::from_timestamp(timestamp, 0)
                                .unwrap_or(chrono::DateTime::from_timestamp(0, 0).unwrap());
                            info!("Last clean shutdown was at {}", shutdown_time);
                            true
                        } else {
                            false
                        }
                    } else {
                        false
                    }
                }
                _ => false,
            };
            
            // Check for unclean shutdown
            if let Ok(Some(data)) = db.get_metadata(b"shutdown_in_progress") {
                if data == b"true" {
                    warn!("Database was not cleanly shut down - shutdown was in progress");
                    
                    // Run integrity check
                    info!("Running database integrity check after unclean shutdown...");
                    match db.verify_integrity(
                        crate::storage::IntegrityCheckLevel::Comprehensive,
                        true // Enable repair
                    ) {
                        Ok(result) => {
                            if !result.passed {
                                error!("Database integrity check failed with {} issues", result.issues.len());
                                // In production, you might want to fail here or enter recovery mode
                            } else {
                                info!("Database integrity check passed");
                            }
                        }
                        Err(e) => {
                            error!("Failed to run integrity check: {}", e);
                            return Err(NodeError::StorageError(e));
                        }
                    }
                    
                    // Clear the flag
                    let _ = db.store_metadata(b"shutdown_in_progress", b"false");
                }
            }
            
            if !was_clean_shutdown {
                warn!("No record of clean shutdown found - database may require recovery");
            }
        }

        // Initialize checkpoint manager if enabled
        let checkpoint_manager = if config.checkpoints_enabled {
            let checkpoint_config = CheckpointConfig {
                checkpoint_interval: config.checkpoint_interval,
                checkpoint_type: CheckpointType::from_str(&config.checkpoint_type)
                    .unwrap_or(CheckpointType::Full),
                data_directory: config.data_dir.clone(),
            };
            
            Some(Arc::new(CheckpointManager::new(
                checkpoint_config,
                blockchain_db.clone(),
                chain_state.clone(),
            )?))
        } else {
            None
        };

        // Initialize performance monitor
        let performance_monitor = Arc::new(PerformanceMonitor::new(1000)); // Store 1000 data points per metric

        Ok(Self {
            config,
            chain_state,
            blockchain_db,
            utxo_set,
            network_manager,
            block_validator,
            tx_validator,
            backup_manager,
            recovery_manager,
            checkpoint_manager,
            rpc_server,
            is_running: Arc::new(AtomicBool::new(false)),
            mem_pool,
            api_server: None,
            lightning: None,
            lightning_event_handler: None,
            lightning_event_task: None,
            performance_monitor,
            peer_id: PeerId::random(),
            start_time: Instant::now(),
            network: Arc::new(P2PNetwork::new()),
            mempool: Arc::new(TransactionPool::new()),
            blockchain: Arc::new(ChainState::new()),
            wallet: Arc::new(()),
            db_shutdown_handler: None,
            wal: None,
        })
    }

    pub fn start(&self) -> Result<(), NodeError> {
        // ... existing code ...

        // Start checkpoint manager if enabled
        if let Some(checkpoint_manager) = &self.checkpoint_manager {
            checkpoint_manager.start()?;
        }

        // Start performance monitoring
        let monitor_clone = Arc::clone(&self.performance_monitor);
        tokio::spawn(async move {
            monitor_clone.start_periodic_collection(10000); // Collect system metrics every 10 seconds
        });
        
        // Optimize database for performance
        self.optimize_database_for_performance()?;

        // ... existing code ...
        
        Ok(())
    }

    pub fn stop(&self) -> Result<(), NodeError> {
        info!("Stopping node services...");
        
        // Stop Lightning Network tasks if running
        if let Some(task_handle) = &self.lightning_event_task {
            if let Ok(mut handle) = task_handle.lock() {
                handle.abort();
            }
        }

        // Stop checkpoint manager if enabled
        if let Some(checkpoint_manager) = &self.checkpoint_manager {
            checkpoint_manager.stop()?;
        }

        // Perform graceful database shutdown
        info!("Performing database shutdown procedures...");
        {
            let db = self.blockchain_db.read().unwrap();
            
            // Create final checkpoint metadata
            let height = db.get_height().unwrap_or(0);
            let timestamp = chrono::Utc::now().timestamp();
            
            let checkpoint_data = serde_json::json!({
                "type": "shutdown_checkpoint",
                "height": height,
                "timestamp": timestamp,
                "clean_shutdown": true,
            });
            
            // Store shutdown checkpoint
            if let Err(e) = db.store_metadata(
                b"last_shutdown_checkpoint",
                checkpoint_data.to_string().as_bytes()
            ) {
                error!("Failed to store shutdown checkpoint: {}", e);
            }
            
            // Mark as clean shutdown
            if let Err(e) = db.store_metadata(
                b"last_clean_shutdown",
                timestamp.to_string().as_bytes()
            ) {
                error!("Failed to store clean shutdown marker: {}", e);
            }
            
            // Clear any shutdown in progress flag
            if let Err(e) = db.store_metadata(b"shutdown_in_progress", b"false") {
                error!("Failed to clear shutdown flag: {}", e);
            }
            
            // Flush all pending writes
            info!("Flushing database to disk...");
            if let Err(e) = db.flush() {
                error!("Failed to flush database: {}", e);
                return Err(NodeError::StorageError(e));
            }
            
            // Compact if time permits (with timeout)
            info!("Performing quick database compaction...");
            match std::time::Instant::now() {
                start => {
                    if let Err(e) = db.compact() {
                        warn!("Database compaction failed: {}", e);
                    } else {
                        let duration = start.elapsed();
                        info!("Database compaction completed in {:?}", duration);
                    }
                }
            }
        }

        info!("Database shutdown procedures completed");
        
        Ok(())
    }

    /// Start the API server
    pub async fn start_api(self: Arc<Self>, bind_address: &str, port: u16) -> std::io::Result<()> {
        info!("Starting API server on {}:{}", bind_address, port);
        
        // Create API server with shared node reference
        let api_server = ApiServer::new(
            self.clone(),
            bind_address,
            port
        );
        
        // Start the server
        let server = api_server.start().await?;
        
        info!("API server started successfully on {}:{}", bind_address, port);
        
        // Run the server
        server.await?;
        
        Ok(())
    }

    /// Start the API server with custom configuration
    pub async fn start_api_with_config(self: Arc<Self>, config: ApiConfig) -> std::io::Result<()> {
        info!("Starting API server with custom config on {}:{}", config.bind_address, config.port);
        
        // Create API server with custom configuration
        let api_server = ApiServer::new(self.clone(), &config.bind_address, config.port)
            .with_config(config.clone());
        
        // Start the server
        let server = api_server.start().await?;
        
        // Run the server
        server.await?;
        
        info!("API server started on {}:{} with custom configuration", config.bind_address, config.port);
        Ok(())
    }

    /// Initialize Lightning Network functionality
    pub fn init_lightning(&mut self) -> Result<(), String> {
        info!("Initializing Lightning Network functionality");
        
        // Create Lightning wallet from node wallet
        let wallet = match LightningWallet::from_node_wallet(&self.wallet) {
            Ok(wallet) => wallet,
            Err(e) => {
                error!("Failed to create Lightning wallet: {}", e);
                return Err(format!("Failed to create Lightning wallet: {}", e));
            }
        };
        
        // Create Lightning configuration from node config
        let config = LightningConfig {
            use_quantum_signatures: self.config.use_quantum_signatures,
            quantum_scheme: self.config.quantum_scheme.clone(),
            quantum_security_level: self.config.quantum_security_level,
            ..LightningConfig::default()
        };
        
        // Create Lightning Network manager
        let (lightning, event_receiver) = match LightningManager::new(config, wallet) {
            Ok((manager, receiver)) => (manager, receiver),
            Err(e) => {
                error!("Failed to create Lightning Manager: {}", e);
                return Err(format!("Failed to create Lightning Manager: {}", e));
            }
        };
        
        // Create thread-safe event handler
        let (event_handler, internal_receiver) = LightningEventHandler::new();
        
        // Spawn task to forward events from Lightning Manager to our handler
        let event_sender = event_handler.event_sender.clone();
        let forward_task = tokio::spawn(async move {
            let mut receiver = event_receiver;
            while let Some(event) = receiver.recv().await {
                if let Err(e) = event_sender.send(event) {
                    error!("Failed to forward Lightning event: {}", e);
                    break;
                }
            }
        });
        
        // Spawn task to process Lightning events
        let process_task = tokio::spawn(async move {
            let mut receiver = internal_receiver;
            while let Some(event) = receiver.recv().await {
                // Process the event
                match event {
                    LightningEvent::ChannelOpened { channel_id, peer_id, .. } => {
                        info!("Lightning channel {} opened with peer {}", channel_id, peer_id);
                    }
                    LightningEvent::ChannelClosed { channel_id, .. } => {
                        info!("Lightning channel {} closed", channel_id);
                    }
                    LightningEvent::PaymentReceived { payment_hash, amount_msat, .. } => {
                        info!("Lightning payment received: {} msat (hash: {})", amount_msat, payment_hash);
                    }
                    LightningEvent::PaymentSent { payment_hash, amount_msat, .. } => {
                        info!("Lightning payment sent: {} msat (hash: {})", amount_msat, payment_hash);
                    }
                    _ => {
                        // Handle other events as needed
                        debug!("Lightning event: {:?}", event);
                    }
                }
            }
        });
        
        // Store in node
        self.lightning = Some(Arc::new(Mutex::new(lightning)));
        self.lightning_event_handler = Some(event_handler);
        self.lightning_event_task = Some(Arc::new(Mutex::new(process_task)));
        
        info!("Lightning Network functionality initialized successfully");
        
        Ok(())
    }
    
    /// Get the Lightning Network manager
    pub fn lightning(&self) -> Option<Arc<Mutex<LightningManager>>> {
        self.lightning.clone()
    }
    
    /// Register the Lightning Network manager
    pub fn register_lightning(&mut self, lightning: LightningManager) {
        self.lightning = Some(Arc::new(Mutex::new(lightning)));
    }
    
    /// Open a payment channel
    pub async fn open_payment_channel(
        &self,
        peer_id: &str,
        capacity: u64,
        push_amount: u64,
    ) -> Result<String, String> {
        let lightning = match &self.lightning {
            Some(lightning) => lightning,
            None => return Err("Lightning Network not initialized".to_string()),
        };
        
        let lightning = lightning.lock().unwrap();
        
        match lightning.open_channel(peer_id, capacity, push_amount, false, None).await {
            Ok(response) => Ok(response.channel_id),
            Err(e) => Err(format!("Failed to open payment channel: {}", e)),
        }
    }
    
    /// Close a payment channel
    pub async fn close_payment_channel(
        &self,
        channel_id: &str,
        force_close: bool,
    ) -> Result<String, String> {
        let lightning = match &self.lightning {
            Some(lightning) => lightning,
            None => return Err("Lightning Network not initialized".to_string()),
        };
        
        let lightning = lightning.lock().unwrap();
        
        // Parse channel ID from string to u64
        let channel_id_u64: u64 = match channel_id.parse() {
            Ok(id) => id,
            Err(_) => return Err("Invalid channel ID format".to_string()),
        };
        
        match lightning.close_channel(&channel_id_u64, force_close).await {
            Ok(tx) => Ok(format!("{}", hex::encode(tx.hash()))),
            Err(e) => Err(format!("Failed to close payment channel: {}", e)),
        }
    }
    
    /// Create a payment invoice
    pub fn create_invoice(
        &self,
        amount_msat: u64,
        description: &str,
        expiry_seconds: u32,
    ) -> Result<String, String> {
        let lightning = match &self.lightning {
            Some(lightning) => lightning,
            None => return Err("Lightning Network not initialized".to_string()),
        };
        
        let lightning = lightning.lock().unwrap();
        
        match lightning.create_invoice(amount_msat, description, expiry_seconds, false) {
            Ok(response) => Ok(response.payment_request),
            Err(e) => Err(format!("Failed to create invoice: {}", e)),
        }
    }
    
    /// Pay an invoice
    pub async fn pay_invoice(
        &self,
        invoice_str: &str,
    ) -> Result<String, String> {
        let lightning = match &self.lightning {
            Some(lightning) => lightning,
            None => return Err("Lightning Network not initialized".to_string()),
        };
        
        let lightning = lightning.lock().unwrap();
        
        match lightning.send_payment(invoice_str, None, 60, None).await {
            Ok(response) => {
                if let Some(preimage) = response.payment_preimage {
                    Ok(preimage)
                } else {
                    Err(format!("Payment failed: {}", response.payment_error.unwrap_or_else(|| "Unknown error".to_string())))
                }
            },
            Err(e) => Err(format!("Failed to pay invoice: {}", e)),
        }
    }
    
    /// List all active channels
    pub fn list_channels(&self) -> Result<Vec<String>, String> {
        let lightning = match &self.lightning {
            Some(lightning) => lightning,
            None => return Err("Lightning Network not initialized".to_string()),
        };
        
        let lightning = lightning.lock().unwrap();
        
        match lightning.get_channels(false, true) {
            Ok(channels) => {
                let channel_ids = channels.iter().map(|ch| ch.channel_id.clone()).collect();
                Ok(channel_ids)
            },
            Err(e) => Err(format!("Failed to list channels: {}", e)),
        }
    }
    
    /// Get information about a specific channel
    pub fn get_channel_info(&self, channel_id: &str) -> Result<serde_json::Value, String> {
        let lightning = match &self.lightning {
            Some(lightning) => lightning,
            None => return Err("Lightning Network not initialized".to_string()),
        };
        
        let lightning = lightning.lock().unwrap();
        
        match lightning.get_channel(channel_id) {
            Ok(Some(channel)) => {
                // Convert LightningChannel to JSON
                let json = serde_json::json!({
                    "id": channel.channel_id,
                    "remote_pubkey": channel.remote_pubkey,
                    "capacity": channel.capacity,
                    "local_balance": channel.local_balance,
                    "remote_balance": channel.remote_balance,
                    "commit_fee": channel.commit_fee,
                    "private": channel.private,
                    "initiator": channel.initiator,
                    "uptime": channel.uptime,
                    "lifetime": channel.lifetime,
                });
                
                Ok(json)
            },
            Ok(None) => Err(format!("Channel {} not found", channel_id)),
            Err(e) => Err(format!("Failed to get channel info: {}", e)),
        }
    }

    pub fn optimize_database_for_performance(&self) -> Result<(), NodeError> {
        // Wrap in performance monitor to track how long optimization takes
        self.performance_monitor.record_execution_time(
            MetricType::Custom("database_optimization".to_string()),
            None,
            || {
                // Optimize the database
                if let Err(e) = self.blockchain_db.optimize_for_performance() {
                    error!("Database optimization failed: {}", e);
                    return Err(NodeError::StorageError(e));
                }
                
                // Preload critical data
                if let Err(e) = self.blockchain_db.preload_critical_data() {
                    error!("Failed to preload critical data: {}", e);
                    return Err(NodeError::StorageError(e));
                }
                
                // Configure memory usage for optimal performance
                let available_memory = self.get_available_memory();
                let cache_budget_mb = (available_memory * 0.7) as usize; // Use up to 70% of available memory
                
                if let Err(e) = self.blockchain_db.optimize_caching(cache_budget_mb) {
                    error!("Failed to optimize caching: {}", e);
                    return Err(NodeError::StorageError(e));
                }
                
                info!("Database optimized for performance with {}MB cache budget", cache_budget_mb);
                Ok(())
            }
        )?;
        
        Ok(())
    }

    fn get_available_memory(&self) -> usize {
        // This is a simplified implementation
        // In a real implementation, use platform-specific APIs

        #[cfg(target_os = "linux")]
        {
            if let Ok(content) = std::fs::read_to_string("/proc/meminfo") {
                for line in content.lines() {
                    if line.starts_with("MemAvailable:") {
                        if let Some(kb_str) = line.split_whitespace().nth(1) {
                            if let Ok(kb) = kb_str.parse::<usize>() {
                                return kb / 1024; // Convert KB to MB
                            }
                        }
                    }
                }
            }
        }
        
        // Default to 1GB if can't determine
        1024
    }

    pub fn get_performance_metrics(&self) -> serde_json::Value {
        self.performance_monitor.get_report()
    }

    /// Get node information
    pub fn get_info(&self) -> Result<NodeInfo, String> {
        Ok(NodeInfo {
            node_id: self.peer_id.to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            protocol_version: 1,
            network: self.config.network.clone(),
            height: self.blockchain.get_height(),
            best_block_hash: hex::encode(self.blockchain.get_best_block_hash()),
            connections: self.network.get_peer_count() as u32,
            synced: self.is_synced(),
            uptime: self.start_time.elapsed().as_secs(),
        })
    }

    /// Get system information
    pub fn get_system_info(&self) -> Result<SystemInfo, String> {
        use sysinfo::{System, SystemExt, CpuExt};
        
        let mut sys = System::new_all();
        sys.refresh_all();
        
        Ok(SystemInfo {
            os: sys.long_os_version().unwrap_or_else(|| "Unknown".to_string()),
            arch: std::env::consts::ARCH.to_string(),
            cpu_count: sys.cpus().len() as u32,
            total_memory: sys.total_memory(),
            used_memory: sys.used_memory(),
            total_swap: sys.total_swap(),
            used_swap: sys.used_swap(),
            uptime: sys.uptime(),
            load_average: sys.load_average(),
        })
    }

    /// Get logs
    pub fn get_logs(&self, level: &str, component: Option<&str>, limit: usize, offset: usize) -> Result<Vec<LogEntry>, String> {
        // In a real implementation, this would read from a log storage system
        // For now, return empty logs
        Ok(Vec::new())
    }

    /// Get node status
    pub fn get_status(&self) -> Result<NodeStatus, String> {
        Ok(NodeStatus {
            state: if self.is_synced() { "synced".to_string() } else { "syncing".to_string() },
            height: self.blockchain.get_height(),
            best_block_hash: hex::encode(self.blockchain.get_best_block_hash()),
            peer_count: self.network.get_peer_count(),
            mempool_size: self.mempool.size(),
            is_mining: false, // TODO: Get from mining manager
            hashrate: 0, // TODO: Get from mining manager
            difficulty: 1.0, // TODO: Get from blockchain
            network_hashrate: 0, // TODO: Calculate network hashrate
        })
    }

    /// Get node version
    pub fn get_version(&self) -> Result<VersionInfo, String> {
        Ok(VersionInfo {
            version: env!("CARGO_PKG_VERSION").to_string(),
            protocol_version: 1,
            git_commit: option_env!("GIT_COMMIT").unwrap_or("unknown").to_string(),
            build_date: option_env!("BUILD_DATE").unwrap_or("unknown").to_string(),
            rust_version: env!("RUSTC_VERSION").to_string(),
        })
    }

    /// Restart the node
    pub fn restart(&self) -> Result<(), NodeError> {
        info!("Restarting node...");
        
        // Stop all services
        self.stop()?;
        
        // Wait a moment for cleanup
        std::thread::sleep(std::time::Duration::from_millis(100));
        
        // Start all services again
        self.start()?;
        
        info!("Node restarted successfully");
        Ok(())
    }

    /// Shutdown the node
    pub fn shutdown(&self) -> Result<(), NodeError> {
        info!("Shutting down node...");
        
        // Set running flag to false
        self.is_running.store(false, std::sync::atomic::Ordering::SeqCst);
        
        // Create final backup if configured
        if let Some(backup_manager) = &self.backup_manager {
            info!("Creating final backup before shutdown...");
            match backup_manager.create_backup() {
                Ok(path) => info!("Created final backup at {:?}", path),
                Err(e) => warn!("Failed to create final backup: {:?}", e),
            }
        }
        
        // Perform final database integrity check
        {
            let db = self.blockchain_db.read().unwrap();
            info!("Running quick integrity check before shutdown...");
            
            match db.verify_integrity(
                crate::storage::IntegrityCheckLevel::Quick,
                false // Don't repair, just check
            ) {
                Ok(result) => {
                    if result.passed {
                        info!("Database integrity check passed");
                    } else {
                        warn!("Database integrity issues detected: {} issues found", result.issues.len());
                        for issue in result.issues.iter().take(5) { // Show first 5 issues
                            warn!("  - {}: {}", issue.tree, issue.description);
                        }
                    }
                }
                Err(e) => {
                    warn!("Failed to run integrity check: {}", e);
                }
            }
        }
        
        // Stop all services (which includes database shutdown)
        self.stop()?;
        
        info!("Node shutdown complete");
        Ok(())
    }

    /// Get debug information
    pub fn get_debug_info(&self) -> Result<crate::api::types::DebugInfo, String> {
        Ok(crate::api::types::DebugInfo {
            node_info: self.get_info()?,
            system_info: self.get_system_info()?,
            performance_metrics: self.get_performance_metrics(),
            network_stats: self.get_network_stats(),
            mempool_stats: self.get_mempool_stats(),
            blockchain_stats: self.get_blockchain_stats(),
            lightning_stats: self.get_lightning_stats(),
        })
    }

    /// Get network statistics
    pub fn get_network_stats(&self) -> serde_json::Value {
        serde_json::json!({
            "peer_count": self.network.get_peer_count(),
            "inbound_connections": 0, // TODO: Get from network manager
            "outbound_connections": 0, // TODO: Get from network manager
            "bytes_sent": 0, // TODO: Get from network manager
            "bytes_received": 0, // TODO: Get from network manager
        })
    }

    /// Get mempool statistics
    pub fn get_mempool_stats(&self) -> serde_json::Value {
        serde_json::json!({
            "size": self.mempool.size(),
            "bytes": self.mempool.get_memory_usage(),
            "fee_histogram": [], // TODO: Get fee histogram
            "min_fee_rate": 1.0, // TODO: Get from mempool
            "max_fee_rate": 100.0, // TODO: Get from mempool
        })
    }

    /// Get blockchain statistics
    pub fn get_blockchain_stats(&self) -> serde_json::Value {
        serde_json::json!({
            "height": self.blockchain.get_height(),
            "best_block_hash": hex::encode(self.blockchain.get_best_block_hash()),
            "difficulty": 1.0, // TODO: Get from blockchain
            "total_work": "0", // TODO: Get from blockchain
            "chain_work": "0", // TODO: Get from blockchain
        })
    }

    /// Get Lightning Network statistics
    pub fn get_lightning_stats(&self) -> serde_json::Value {
        if let Some(lightning) = &self.lightning {
            let lightning = lightning.lock().unwrap();
            
            // Use the LightningManager API to get comprehensive stats
            match lightning.get_info() {
                Ok(info) => {
                    serde_json::json!({
                        "enabled": true,
                        "node_id": info.node_id,
                        "channel_count": info.num_channels,
                        "pending_channels": info.num_pending_channels,
                        "inactive_channels": info.num_inactive_channels,
                        "total_balance_msat": info.total_balance_msat,
                        "total_outbound_capacity_msat": info.total_outbound_capacity_msat,
                        "total_inbound_capacity_msat": info.total_inbound_capacity_msat,
                        "num_peers": info.num_peers,
                        "synced_to_chain": info.synced_to_chain,
                        "synced_to_graph": info.synced_to_graph,
                        "block_height": info.block_height,
                    })
                },
                Err(_) => {
                    serde_json::json!({
                        "enabled": true,
                        "error": "Failed to get Lightning Network info",
                        "channel_count": 0,
                        "total_capacity": 0,
                        "local_balance": 0,
                        "remote_balance": 0,
                    })
                }
            }
        } else {
            serde_json::json!({
                "enabled": false,
                "channel_count": 0,
                "total_capacity": 0,
                "local_balance": 0,
                "remote_balance": 0,
            })
        }
    }

    /// Get metrics
    pub fn get_metrics(&self, period: u64) -> Result<NodeMetrics, String> {
        Ok(NodeMetrics {
            uptime: self.start_time.elapsed().as_secs(),
            peer_count: self.network.get_peer_count(),
            block_height: self.blockchain.get_height(),
            mempool_size: self.mempool.size(),
            mempool_bytes: self.mempool.size_in_bytes(),
            sync_progress: if self.is_synced() { 1.0 } else { 0.5 }, // Simplified
            network_bytes_sent: 0, // TODO: Get from network layer
            network_bytes_received: 0, // TODO: Get from network layer
            cpu_usage: 0.0, // TODO: Get from system monitor
            memory_usage: 0, // TODO: Get from system monitor
            disk_usage: 0, // TODO: Get from system monitor
        })
    }

    /// Get configuration
    pub fn get_config(&self) -> Result<serde_json::Value, String> {
        serde_json::to_value(&self.config)
            .map_err(|e| format!("Failed to serialize config: {}", e))
    }

    /// Update configuration
    pub fn update_config(&self, new_config: serde_json::Value) -> Result<serde_json::Value, String> {
        // In a real implementation, this would validate and apply the new configuration
        // For now, just return the current config
        self.get_config()
    }

    /// Get faucet (for testnet)
    pub fn get_faucet(&self) -> Result<Option<FaucetInfo>, String> {
        // Return faucet info if this is a testnet node
        if self.config.network == "testnet" {
            Ok(Some(FaucetInfo {
                enabled: true,
                balance: 1000000000, // 10 NOVA
                max_request: 100000000, // 1 NOVA
                cooldown_seconds: 3600, // 1 hour
                requests_today: 0,
                daily_limit: 100,
            }))
        } else {
            Ok(None)
        }
    }

    /// Check if the node is synced
    pub fn is_synced(&self) -> bool {
        // Simplified sync check - in a real implementation this would be more sophisticated
        true
    }

    /// Get the current block height
    pub fn get_height(&self) -> u64 {
        self.blockchain.get_height()
    }

    /// Get the best block hash
    pub fn get_best_block_hash(&self) -> [u8; 32] {
        self.blockchain.get_best_block_hash()
    }

    /// Get storage reference
    pub fn storage(&self) -> &Arc<RwLock<BlockchainDB>> {
        &self.blockchain_db
    }

    /// Get mempool reference
    pub fn mempool(&self) -> &Arc<TransactionPool> {
        &self.mempool
    }

    /// Get environmental manager
    pub fn environmental_manager(&self) -> Option<&Arc<EnvironmentalMonitor>> {
        None // TODO: Add environmental tracker to Node
    }

    /// Broadcast transaction to network
    pub fn broadcast_transaction(&self, tx: &btclib::types::transaction::Transaction) {
        // TODO: Implement transaction broadcasting
        info!("Broadcasting transaction: {}", hex::encode(tx.hash()));
    }

    /// Create backup
    pub fn create_backup(&self, destination: Option<&str>, include_wallet: bool, encrypt: bool) -> Result<crate::api::types::BackupInfo, String> {
        // TODO: Implement actual backup creation
        Ok(crate::api::types::BackupInfo {
            id: format!("backup_{}", chrono::Utc::now().timestamp()),
            timestamp: chrono::Utc::now().timestamp() as u64,
            size: 1024 * 1024, // 1MB placeholder
            backup_type: "full".to_string(),
            status: "completed".to_string(),
            file_path: destination.unwrap_or("/tmp/backup.dat").to_string(),
            verified: true,
        })
    }

    /// Get backup information
    pub fn get_backup_info(&self) -> Result<Vec<crate::api::types::BackupInfo>, String> {
        // TODO: Implement actual backup info retrieval
        Ok(vec![])
    }
} 