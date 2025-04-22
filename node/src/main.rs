use node::network::{P2PNetwork, NetworkCommand, NetworkEvent};
use node::storage::{ChainState, BlockchainDB, BackupManager, RecoveryManager};
use node::mempool::{TransactionPool, TransactionPrioritizer};
use node::mempool::prioritization::PrioritizationConfig;
use node::config::NodeConfig;
use node::storage::corruption::{CorruptionHandler, CorruptionError, CorruptionType};
use tracing::{info, error, warn, debug};
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};
use std::path::PathBuf;
use std::time::Duration;
use btclib::types::block::Block;
use btclib::types::transaction::Transaction;
use node::network::sync::{ChainSync, DefaultSyncMetrics};
use hex;
use std::thread::JoinHandle;
use btclib::monitoring::mempool::MempoolMetrics;

// Add NodeCommand enum for safe communication with the node
enum NodeCommand {
    ReloadConfig,
    // Add other commands as needed
}

struct Node {
    config: Arc<Mutex<NodeConfig>>,
    mempool: Arc<TransactionPool>,
    prioritizer: Arc<Mutex<TransactionPrioritizer>>,
    network: P2PNetwork,
    chain_state: ChainState,
    backup_manager: Arc<BackupManager>,
    corruption_handler: Arc<Mutex<CorruptionHandler>>,
    mempool_metrics: Option<MempoolMetrics>,
}

impl Node {
    async fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let config = NodeConfig::load()?;
        config.validate().map_err(|e| format!("Configuration error: {}", e))?;
        let config = Arc::new(Mutex::new(config));

        let mempool_config = config.lock().await.mempool.clone();
        let mempool = Arc::new(TransactionPool::new(node::mempool::MempoolConfig::from(mempool_config.clone())));
        
        let prioritizer_config = PrioritizationConfig::from(mempool_config);
        let prioritizer = Arc::new(Mutex::new(TransactionPrioritizer::new(prioritizer_config)));

        let db = Arc::new(BlockchainDB::new(&config.lock().await.storage.db_path)?);
        
        // Initialize the corruption handler early in the startup process
        let backup_dir = config.lock().await.backup.backup_dir.clone();
        let corruption_handler = Arc::new(Mutex::new(CorruptionHandler::new(
            Arc::clone(&db),
            backup_dir.clone(),
        )));
        
        // Perform database integrity check and automatic corruption handling
        if let Err(e) = Self::check_and_repair_database(&corruption_handler).await {
            error!("Critical database corruption detected: {}", e);
            return Err(e.into());
        }
        
        let chain_state = ChainState::new(Arc::clone(&db))?;

        let backup_config = config.lock().await.backup.clone();
        let backup_manager = Arc::new(BackupManager::new(
            Arc::clone(&db),
            backup_dir,
            backup_config.max_backups,
            backup_config.backup_interval,
        ));

        let mut recovery_manager = RecoveryManager::new(
            Arc::clone(&db),
            backup_config.backup_dir.clone(),
            chain_state.clone(),
        );

        if backup_config.verify_on_startup {
            if let Err(e) = recovery_manager.verify_and_recover().await {
                error!("Database verification failed: {}", e);
                return Err(e.into());
            }
        }

        // Initialize network with genesis hash and network ID
        let (network, _, _) = P2PNetwork::new(
            None,
            chain_state.get_genesis_hash(),
            &config.lock().await.node.chain_id
        ).await?;

        // Initialize mempool metrics if metrics are enabled
        let mempool_metrics = if config.lock().await.node.metrics_enabled {
            match prometheus::Registry::new() {
                Ok(registry) => {
                    match MempoolMetrics::new(&registry, "supernova") {
                        Ok(metrics) => Some(metrics),
                        Err(e) => {
                            warn!("Failed to create mempool metrics: {}", e);
                            None
                        }
                    }
                },
                Err(e) => {
                    warn!("Failed to create metrics registry: {}", e);
                    None
                }
            }
        } else {
            None
        };

        Ok(Self {
            config,
            mempool,
            prioritizer,
            network,
            chain_state,
            backup_manager,
            corruption_handler,
            mempool_metrics,
        })
    }

    // New function to check database integrity and perform repairs if needed
    async fn check_and_repair_database(corruption_handler: &Arc<Mutex<CorruptionHandler>>) -> Result<(), CorruptionError> {
        info!("Performing database integrity check...");
        
        // Load checkpoints for improved corruption handling
        {
            let mut handler = corruption_handler.lock().await;
            if let Err(e) = handler.load_checkpoints().await {
                warn!("Failed to load corruption handler checkpoints: {}", e);
                // Continue anyway as this is not critical
            }
        }
        
        // Check database integrity
        let integrity_check_result = {
            let mut handler = corruption_handler.lock().await;
            handler.check_database_integrity().await?
        };
        
        if !integrity_check_result {
            warn!("Database corruption detected. Attempting automatic repair...");
            
            // Attempt automatic repair
            let repair_results = {
                let mut handler = corruption_handler.lock().await;
                handler.auto_repair().await?
            };
            
            // Log repair results
            let successful_repairs = repair_results.iter().filter(|r| r.success).count();
            let failed_repairs = repair_results.len() - successful_repairs;
            
            if failed_repairs > 0 {
                error!("Some database repairs failed: {}/{} issues could not be fixed automatically", 
                      failed_repairs, repair_results.len());
                
                // Check if critical repairs failed
                let critical_failures = repair_results.iter()
                    .filter(|r| !r.success && is_critical_corruption(&r.corruption_type))
                    .count();
                
                if critical_failures > 0 {
                    return Err(CorruptionError::CorruptionDetected(
                        format!("{} critical corruption issues could not be repaired", critical_failures)
                    ));
                }
            }
            
            if successful_repairs > 0 {
                info!("Successfully repaired {}/{} corruption issues", 
                     successful_repairs, repair_results.len());
            }
            
            // Perform a final integrity check to confirm repairs
            let final_check = {
                let mut handler = corruption_handler.lock().await;
                handler.check_database_integrity().await?
            };
            
            if !final_check && failed_repairs > 0 {
                warn!("Database still has some corruption issues after repair");
                // We continue with non-critical corruption as we've already filtered critical failures
            }
        } else {
            info!("Database integrity check passed successfully");
        }
        
        Ok(())
    }
    
    async fn handle_new_transaction(&self, transaction: Transaction) -> Result<(), Box<dyn std::error::Error>> {
        let tx_hash = transaction.hash();
        if self.mempool.get_transaction(&tx_hash).is_some() {
            return Ok(());
        }

        // Calculate fee rate with a dummy output lookup function
        let fee_rate = match transaction.calculate_fee_rate(|_, _| None) {
            Some(rate) => rate,
            None => return Err("Unable to calculate fee rate".into())
        };

        // Get config
        let config = self.config.lock().await;
        
        // Check minimum fee rate (converting from f64 to u64)
        if fee_rate < config.mempool.min_fee_rate as u64 {
            info!("Transaction {} rejected: fee rate too low ({} < {})",
                    hex::encode(&tx_hash[..4]), fee_rate, config.mempool.min_fee_rate);
            
            // Update metrics if available
            if let Some(metrics) = &self.mempool_metrics {
                metrics.increment_transactions_removed("low_fee");
            }
            
            return Err("Fee rate too low".into());
        }
        
        // First try as an RBF transaction if enabled
        if config.mempool.enable_rbf {
            match self.mempool.replace_transaction(transaction.clone(), fee_rate) {
                Ok(replaced_tx) => {
                    // Successfully processed as RBF
                    if let Some(old_tx) = replaced_tx {
                        // Single transaction replacement
                        let old_tx_hash = old_tx.hash();
                        info!("Replaced transaction {} with {} (RBF, fee rate: {} -> {})",
                              hex::encode(&old_tx_hash[..4]), 
                              hex::encode(&tx_hash[..4]),
                              self.prioritizer.lock().await.get_transaction_fee_rate(&old_tx_hash).unwrap_or(0),
                              fee_rate);
                        
                        // Remove old transaction from prioritizer
                        self.prioritizer.lock().await.remove_transaction(&old_tx_hash);
                        
                        // Update RBF metrics
                        if let Some(metrics) = &self.mempool_metrics {
                            metrics.increment_transactions_replaced();
                            metrics.increment_transactions_removed("rbf");
                            metrics.increment_transactions_added("rbf");
                            metrics.observe_fee_rate(fee_rate as f64, "rbf");
                        }
                    } else {
                        // Multiple transaction replacement
                        info!("Replaced multiple transactions with {} (RBF, fee rate: {})",
                              hex::encode(&tx_hash[..4]), fee_rate);
                        
                        // Update RBF metrics
                        if let Some(metrics) = &self.mempool_metrics {
                            metrics.increment_transactions_replaced();
                            metrics.increment_transactions_removed("rbf_package");
                            metrics.increment_transactions_added("rbf");
                            metrics.observe_fee_rate(fee_rate as f64, "rbf");
                        }
                    }
                    
                    // Add new transaction to prioritizer
                    self.prioritizer.lock().await.add_transaction(transaction, fee_rate, 0);
                    return Ok(());
                },
                Err(MempoolError::NoConflictingTransactions) => {
                    // Not an RBF, continue with regular add
                },
                Err(MempoolError::RbfDisabled) => {
                    // RBF is disabled in config, continue with regular add
                },
                Err(MempoolError::InsufficientFeeIncrease(required)) => {
                    // Log rejection due to insufficient fee increase
                    info!("Transaction {} rejected: insufficient fee increase for RBF ({} < {})",
                          hex::encode(&tx_hash[..4]), fee_rate, required);
                    
                    // Update metrics if available
                    if let Some(metrics) = &self.mempool_metrics {
                        metrics.increment_transactions_removed("insufficient_rbf_fee");
                    }
                    
                    return Err(format!("Insufficient fee increase for RBF, minimum required: {}", required).into());
                },
                Err(e) => {
                    // Other error
                    info!("Transaction {} rejected: RBF error: {}", hex::encode(&tx_hash[..4]), e);
                    
                    // Update metrics if available
                    if let Some(metrics) = &self.mempool_metrics {
                        metrics.increment_transactions_removed("rbf_error");
                    }
                    
                    return Err(format!("RBF error: {}", e).into());
                }
            }
        }
        
        // Regular transaction addition (not RBF or RBF failed)
        match self.mempool.add_transaction(transaction.clone(), fee_rate) {
            Ok(()) => {
                // Add to prioritizer
                self.prioritizer.lock().await.add_transaction(transaction, fee_rate, 0);
                
                // Update metrics if available
                if let Some(metrics) = &self.mempool_metrics {
                    metrics.increment_transactions_added("p2p");
                    metrics.observe_fee_rate(fee_rate as f64, "standard");
                    
                    // Update mempool size metrics
                    let mempool_size = self.mempool.transactions.len() as i64;
                    metrics.observe_mempool_size(mempool_size);
                }
                
                info!("Added transaction {} to mempool with fee rate {}", hex::encode(&tx_hash[..4]), fee_rate);
                Ok(())
            },
            Err(MempoolError::DoubleSpend) => {
                info!("Transaction {} rejected: double spend detected", hex::encode(&tx_hash[..4]));
                
                // Update metrics if available
                if let Some(metrics) = &self.mempool_metrics {
                    metrics.increment_transactions_removed("double_spend");
                }
                
                Err("Double spend detected".into())
            },
            Err(e) => {
                info!("Transaction {} rejected: {}", hex::encode(&tx_hash[..4]), e);
                
                // Update metrics if available
                if let Some(metrics) = &self.mempool_metrics {
                    metrics.increment_transactions_removed("other_error");
                }
                
                Err(format!("Mempool error: {}", e).into())
            }
        }
    }

    async fn handle_config_reload(&self) -> Result<(), Box<dyn std::error::Error>> {
        let mut config = self.config.lock().await;
        config.reload().await?;
        info!("Configuration reloaded successfully");
        Ok(())
    }
    
    async fn create_integrity_checkpoint(&self, height: u64, block_hash: [u8; 32]) -> Result<(), Box<dyn std::error::Error>> {
        self.corruption_handler.lock().await.create_checkpoint(height, block_hash).await?;
        info!("Created integrity checkpoint at height {}", height);
        Ok(())
    }
    
    async fn perform_database_maintenance(&self) -> Result<(), Box<dyn std::error::Error>> {
        info!("Performing database maintenance...");
        
        // Use get_db() to access the database instead of direct field access
        let db = self.chain_state.get_db();
        if let Err(e) = db.compact() {
            error!("Database compaction failed: {}", e);
            return Err(Box::new(e));
        }
        
        info!("Database maintenance completed");
        Ok(())
    }
}

// Helper function to determine if a corruption type is critical
fn is_critical_corruption(corruption_type: &CorruptionType) -> bool {
    match corruption_type {
        // File level corruption is always critical
        CorruptionType::FileLevelCorruption => true,
        
        // For record corruption, only certain trees are critical
        CorruptionType::RecordCorruption { tree_name, .. } => {
            matches!(tree_name.as_str(), "blocks" | "headers" | "metadata" | "block_height_index")
        },
        
        // Index corruption between critical trees is critical
        CorruptionType::IndexCorruption { primary_tree, index_tree, .. } => {
            (primary_tree == "blocks" && index_tree == "block_height_index") ||
            (primary_tree == "headers" && index_tree == "block_height_index")
        },
        
        // Logical corruption affecting a range over certain size is critical
        CorruptionType::LogicalCorruption { affected_range, .. } => {
            affected_range.is_none() || 
            affected_range.map_or(false, |(start, end)| (end - start) > 1000)
        },
    }
}

// Move thread-unsafe operations to a dedicated task
async fn run_node_background_tasks(
    node: Node,
    mut cmd_rx: mpsc::Receiver<NodeCommand>,
) -> Result<(), Box<dyn std::error::Error>> {
    // Arc not needed anymore since this function runs on a single thread
    let node = node;
    
    // Process commands from the NodeCommand channel
    while let Some(cmd) = cmd_rx.recv().await {
        match cmd {
            NodeCommand::ReloadConfig => {
                if let Err(e) = node.handle_config_reload().await {
                    error!("Failed to reload configuration: {}", e);
                }
            },
            // Add other commands here as needed
        }
    }
    
    Ok(())
}

// NodeHandle for safe access to Node components
struct NodeHandle {
    chain_state: ChainState,
    mempool: Arc<TransactionPool>,
    backup_manager: Arc<BackupManager>,
    corruption_handler: Arc<Mutex<CorruptionHandler>>,
    config: Arc<Mutex<NodeConfig>>,
    mempool_metrics: Option<MempoolMetrics>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    // Create node instance
    let node = Node::new().await?;
    
    // Create a NodeHandle to store clones of data needed by various tasks
    let node_handle = NodeHandle {
        chain_state: node.chain_state.clone(),
        mempool: Arc::clone(&node.mempool),
        backup_manager: Arc::clone(&node.backup_manager),
        corruption_handler: Arc::clone(&node.corruption_handler),
        config: Arc::clone(&node.config),
        mempool_metrics: node.mempool_metrics,
    };
    
    // Set up a command channel for node operations
    let (node_cmd_tx, node_cmd_rx) = mpsc::channel::<NodeCommand>(32);
    
    // Run node in a dedicated task for thread safety
    let _node_task = tokio::task::spawn_local(run_node_background_tasks(
        node,
        node_cmd_rx
    ));
    
    // Create a multitasking runtime for the application
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;

    // Watch for configuration changes
    let config_rx = NodeConfig::watch_config().await?;
    let node_cmd_tx_clone = node_cmd_tx.clone();
    let config_handle = rt.spawn(async move {
        let mut config_rx = config_rx;
        while let Some(_) = config_rx.recv().await {
            info!("Configuration change detected");
            // Send command to node handler instead of accessing node directly
            if let Err(e) = node_cmd_tx_clone.send(NodeCommand::ReloadConfig).await {
                error!("Failed to send reload config command: {}", e);
            }
        }
    });

    // Set up network and sync components
    let (mut network, command_tx, mut event_rx) = P2PNetwork::new(
        None,
        node_handle.chain_state.get_genesis_hash(),
        &node_handle.config.lock().await.node.chain_id
    ).await?;

    // Clone the command_tx for future use
    let command_tx_for_sync = command_tx.clone();
    let command_tx_for_network = command_tx.clone();

    // Initialize the enhanced sync system
    let db_for_sync = Arc::clone(node_handle.chain_state.get_db());
    let mut sync = ChainSync::new(
        node_handle.chain_state.clone(),
        db_for_sync,
        command_tx_for_sync
    );

    // Load checkpoints at startup
    if let Err(e) = sync.load_checkpoints().await {
        error!("Failed to load checkpoints: {}", e);
    }

    // Set up metrics for the sync system
    let metrics = Arc::new(DefaultSyncMetrics);
    sync = sync.with_metrics(metrics);

    // Create a sync handle
    let sync_handle = sync.clone();

    // Set up periodic sync timeout handler
    let mut sync_clone = sync_handle.clone();
    let sync_timeout_handle = rt.spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(5));
        loop {
            interval.tick().await;
            if let Err(e) = sync_clone.process_timeouts().await {
                error!("Error processing sync timeouts: {}", e);
            }
        }
    });

    // Spawn a thread to handle automated backups
    let backup_handle = spawn_backup_task(&node_handle);

    // Spawn a thread to handle periodic database maintenance
    let db_maintenance_handle = spawn_maintenance_task(&node_handle);

    // Reference to chain_state and corruption_handler for event handling
    let chain_state = node_handle.chain_state.clone();
    let corruption_handler = Arc::clone(&node_handle.corruption_handler);
    let transaction_handler = node_handle.mempool.clone();

    // Handle network events
    let event_handle = tokio::spawn(async move {
        while let Some(event) = event_rx.recv().await {
            match event {
                // Handle new peer connected
                NetworkEvent::NewPeer(peer_id) => {
                    info!("New peer connected: {}", peer_id);
                },
                
                // Handle peer disconnected
                NetworkEvent::PeerLeft(peer_id) => {
                    info!("Peer disconnected: {}", peer_id);
                },
                
                // Handle new block received
                NetworkEvent::NewBlock { block, height, total_difficulty, from_peer } => {
                    info!("Received new block at height {} from {:?}", height, from_peer);
                    
                    if let Err(e) = sync.handle_new_block(block.clone(), height, total_difficulty, from_peer.as_ref()).await {
                        error!("Failed to process new block: {}", e);
                        
                        // Penalize peer if the block was invalid
                        if let Some(peer_id) = from_peer {
                            command_tx.send(NetworkCommand::BanPeer {
                                peer_id,
                                reason: format!("Invalid block: {}", e),
                                duration: Some(Duration::from_secs(1800)),
                            }).await.ok();
                        }
                    } else {
                        // Create integrity checkpoint if applicable
                        // This creates checkpoints at specific block heights for improved recovery
                        if let Err(e) = corruption_handler.lock().await.create_checkpoint(height, block.hash()).await {
                            warn!("Failed to create integrity checkpoint: {}", e);
                        }
                    }
                },
                
                // Handle new transaction received
                NetworkEvent::NewTransaction { transaction, fee_rate, from_peer } => {
                    debug!("Received new transaction from {:?}", from_peer);
                    
                    // Calculate fee rate with a dummy output lookup function
                    match transaction.calculate_fee_rate(|_, _| None) {
                        Some(fee) => {
                            if let Err(e) = transaction_handler.add_transaction(transaction, fee) {
                                error!("Failed to process transaction: {}", e);
                            }
                        },
                        None => {
                            error!("Failed to calculate transaction fee rate");
                        }
                    }
                },
                
                // Handle block headers received
                NetworkEvent::BlockHeaders { headers, total_difficulty, from_peer } => {
                    debug!("Received {} headers", headers.len());
                    if let Err(e) = sync.handle_block_headers(headers, total_difficulty, from_peer.clone()).await {
                        warn!("Failed to handle headers: {}", e);
                    }
                },
                
                // Handle blocks received
                NetworkEvent::BlocksReceived { blocks, total_difficulty, from_peer } => {
                    debug!("Received {} blocks", blocks.len());
                    if let Err(e) = sync.handle_blocks(blocks, total_difficulty, from_peer.clone()).await {
                        warn!("Failed to handle blocks: {}", e);
                    }
                },
                
                // Handle peer status update
                NetworkEvent::PeerStatus { peer_id, version, height, best_hash, total_difficulty } => {
                    debug!("Peer {} status: height={}, td={}", peer_id, height, total_difficulty);
                    
                    // Check if we need to sync
                    let current_height = chain_state.get_height();
                    if height > current_height + 1 {
                        info!("Detected we're behind peer {} by {} blocks", peer_id, height - current_height);
                        
                        if let Err(e) = sync.start_sync(height, total_difficulty).await {
                            error!("Failed to start sync: {}", e);
                        }
                    }
                },
                
                // Handle checkpoint information
                NetworkEvent::CheckpointsReceived { checkpoints, from_peer } => {
                    info!("Received {} checkpoints from {:?}", checkpoints.len(), from_peer);
                    // Process checkpoints if needed
                },
            }
        }
    });

    // Start the network
    let network_handle = tokio::spawn(async move {
        if let Err(e) = network.run().await {
            error!("Network error: {}", e);
        }
    });

    // Start listening for connections
    let config_lock = node_handle.config.lock().await;
    command_tx_for_network.send(NetworkCommand::StartListening(
        config_lock.network.listen_addr.clone()
    )).await?;
    drop(config_lock);

    // Announce initial status
    command_tx_for_network.send(NetworkCommand::AnnounceStatus {
        version: 1,
        height: node_handle.chain_state.get_height(),
        best_hash: node_handle.chain_state.get_best_block_hash(),
        total_difficulty: node_handle.chain_state.get_total_difficulty(),
    }).await?;

    // Set up periodic tasks
    let mempool_config = node_handle.config.lock().await.mempool.transaction_timeout;
    let mut mempool_cleanup_interval = tokio::time::interval(mempool_config);
    let mut status_announcement_interval = tokio::time::interval(Duration::from_secs(120));
    
    // Main event loop
    loop {
        tokio::select! {
            // Periodic mempool cleanup
            _ = mempool_cleanup_interval.tick() => {
                let removed = node_handle.mempool.clear_expired();
                if removed > 0 {
                    info!("Cleared {} expired transactions from mempool", removed);
                    
                    // Update metrics if available
                    if let Some(metrics) = &node_handle.mempool_metrics {
                        // Track expired transactions in metrics
                        for _ in 0..removed {
                            metrics.increment_transactions_expired();
                            metrics.increment_transactions_removed("expired");
                        }
                        
                        // Update mempool size metrics
                        let mempool_size = node_handle.mempool.transactions.len() as i64;
                        metrics.observe_mempool_size(mempool_size);
                    }
                }
            },
            
            // Periodic status announcement
            _ = status_announcement_interval.tick() => {
                // Announce our current status to the network
                let current_height = node_handle.chain_state.get_height();
                let best_hash = node_handle.chain_state.get_best_block_hash();
                let total_difficulty = node_handle.chain_state.get_total_difficulty();
                
                command_tx_for_network.send(NetworkCommand::AnnounceStatus {
                    version: 1,
                    height: current_height,
                    best_hash,
                    total_difficulty,
                }).await.ok();
                
                // Log sync status
                let sync_stats = sync_handle.get_stats();
                info!("Sync status: {}. Current height: {}, Target height: {}", 
                     sync_stats.state, sync_stats.current_height, sync_stats.target_height);
                
                // Log fork information
                let fork_stats = sync_handle.get_fork_stats();
                let active_forks = fork_stats.get("active_forks").unwrap_or(&0);
                let max_fork_length = fork_stats.get("max_fork_length").unwrap_or(&0);
                let reorg_count = fork_stats.get("reorg_count").unwrap_or(&0);
                let rejected_reorgs = fork_stats.get("rejected_reorgs").unwrap_or(&0);
                
                if *active_forks > 0 {
                    info!("Fork status: {} active forks, max length: {}, reorgs: {}, rejected: {}", 
                         active_forks, max_fork_length, reorg_count, rejected_reorgs);
                }
                
                // Check if the tip is stale
                if sync_handle.check_for_stale_tip() {
                    let time_since_last = sync_handle.time_since_last_block().as_secs() / 60;
                    warn!("Chain tip is stale! No new blocks for {} minutes", time_since_last);
                }
            },
            
            // Handle shutdown signal
            _ = tokio::signal::ctrl_c() => {
                info!("Shutting down...");
                break;
            }
        }
    }

    // Clean shutdown
    sync_timeout_handle.abort();
    event_handle.abort();
    network_handle.abort();
    config_handle.abort();
    db_maintenance_handle.abort();
    
    if let Some(handle) = backup_handle {
        handle.abort();
    }

    let config_lock = node_handle.config.lock().await;
    if config_lock.backup.enable_automated_backups {
        match node_handle.backup_manager.create_backup().await {
            Ok(backup_path) => info!("Created final backup at {:?}", backup_path),
            Err(e) => warn!("Failed to create final backup during shutdown: {}", e),
        }
    }

    // Final database integrity checkpoint before shutdown
    {
        let height = node_handle.chain_state.get_height();
        let block_hash = node_handle.chain_state.get_best_block_hash();
        let mut handler = node_handle.corruption_handler.lock().await;
        if let Err(e) = handler.create_checkpoint(height, block_hash).await {
            warn!("Failed to create final integrity checkpoint: {}", e);
        } else {
            info!("Created final integrity checkpoint at height {}", height);
        }
    }

    info!("Shutdown complete");
    Ok(())
}

// Spawn a thread to handle automated backups
fn spawn_backup_task(node: &NodeHandle) -> Option<tokio::task::JoinHandle<()>> {
    if node.config.try_lock().ok()?.backup.enable_automated_backups {
        let backup_manager = Arc::clone(&node.backup_manager);
        Some(tokio::spawn(async move {
            if let Err(e) = backup_manager.start_automated_backups().await {
                error!("Automated backup task failed: {}", e);
            }
        }))
    } else {
        None
    }
}

// Spawn a thread to handle periodic database maintenance
fn spawn_maintenance_task(node: &NodeHandle) -> tokio::task::JoinHandle<()> {
    // Create a safe clone of just what we need
    let db = node.chain_state.get_db().clone();
    
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(3600));
        
        loop {
            interval.tick().await;
            // Just do database maintenance directly on the DB
            if let Err(e) = db.compact() {
                error!("Database maintenance failed: {}", e);
            } else {
                info!("Database maintenance completed successfully");
            }
        }
    })
}

// Spawn blockchain synchronization process
fn spawn_sync_task(
    db: Arc<BlockchainDB>, 
    chain_state: ChainState, 
    mut event_rx: mpsc::Receiver<NetworkEvent>
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut sync = ChainSync::new(chain_state, db, tokio::sync::mpsc::channel(128).0);
        
        // Load chain checkpoints for sync
        if let Err(e) = sync.load_checkpoints().await {
            error!("Failed to load checkpoints: {}", e);
        }
        
        // Start synchronization to a reasonable starting point
        if let Err(e) = sync.start_sync(0, 0).await {
            error!("Chain sync failed: {}", e);
        }
        
        // Process network events
        while let Some(event) = event_rx.recv().await {
            // Process events
            match event {
                // Handle new blocks
                NetworkEvent::NewBlock { block, .. } => {
                    debug!("Received new block: {}", hex::encode(&block.hash()[..4]));
                }
                // Handle other events
                _ => {}
            }
        }
    })
}