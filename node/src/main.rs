use crate::network::{P2PNetwork, NetworkCommand, NetworkEvent};
use crate::storage::{ChainState, BlockchainDB, BackupManager, RecoveryManager};
use crate::mempool::{TransactionPool, TransactionPrioritizer};
use crate::config::NodeConfig;
use crate::storage::corruption::{CorruptionHandler, CorruptionError};
use tracing::{info, error, warn, debug};
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};
use std::path::PathBuf;
use std::time::Duration;
use btclib::types::{Block, Transaction};
use crate::network::sync::{ChainSync, DefaultSyncMetrics};

struct Node {
    config: Arc<Mutex<NodeConfig>>,
    mempool: Arc<TransactionPool>,
    prioritizer: Arc<Mutex<TransactionPrioritizer>>,
    network: P2PNetwork,
    chain_state: ChainState,
    backup_manager: Arc<BackupManager>,
    corruption_handler: Arc<Mutex<CorruptionHandler>>,
}

impl Node {
    async fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let config = NodeConfig::load()?;
        config.validate().map_err(|e| format!("Configuration error: {}", e))?;
        let config = Arc::new(Mutex::new(config));

        let mempool_config = config.lock().await.mempool.clone();
        let mempool = Arc::new(TransactionPool::new(mempool_config));
        let prioritizer = Arc::new(Mutex::new(TransactionPrioritizer::new(
            config.lock().await.mempool.min_fee_rate
        )));

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

        Ok(Self {
            config,
            mempool,
            prioritizer,
            network,
            chain_state,
            backup_manager,
            corruption_handler,
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

        if self.mempool.check_double_spend(&transaction) {
            return Err("Double spend detected".into());
        }

        let tx_size = bincode::serialize(&transaction)?.len();
        let fee_rate = transaction.calculate_fee_rate()?;

        let config = self.config.lock().await;
        if fee_rate < config.mempool.min_fee_rate {
            return Err("Transaction fee rate too low".into());
        }

        self.mempool.add_transaction(transaction.clone(), fee_rate)?;
        
        let mut prioritizer = self.prioritizer.lock().await;
        prioritizer.add_transaction(transaction, fee_rate, tx_size);

        Ok(())
    }

    async fn handle_config_reload(&self) -> Result<(), Box<dyn std::error::Error>> {
        let mut config = self.config.lock().await;
        config.reload().await?;
        info!("Configuration reloaded successfully");
        Ok(())
    }
    
    // Create a checkpoint in the corruption handler at specific heights
    async fn create_integrity_checkpoint(&self, height: u64, block_hash: [u8; 32]) -> Result<(), Box<dyn std::error::Error>> {
        // Only create checkpoints at certain intervals to avoid excessive checkpointing
        if height % 10000 == 0 {
            info!("Creating database integrity checkpoint at height {}", height);
            let mut handler = self.corruption_handler.lock().await;
            handler.create_checkpoint(height, block_hash).await?;
        }
        Ok(())
    }
    
    // Periodic database maintenance function
    async fn perform_database_maintenance(&self) -> Result<(), Box<dyn std::error::Error>> {
        debug!("Performing periodic database maintenance");
        
        // Compact the database to reclaim space
        if let Err(e) = self.chain_state.db.compact() {
            warn!("Database compaction failed: {}", e);
        }
        
        // Run an integrity check if needed
        let should_check_integrity = {
            // Logic to determine if integrity check is needed based on time or events
            // For example, check every 24 hours or after certain number of blocks
            true // Simplified for this example
        };
        
        if should_check_integrity {
            debug!("Running periodic integrity check");
            let integrity_result = {
                let mut handler = self.corruption_handler.lock().await;
                handler.check_database_integrity().await
            };
            
            match integrity_result {
                Ok(true) => debug!("Periodic integrity check passed"),
                Ok(false) => {
                    warn!("Periodic integrity check detected issues, scheduling repair");
                    // Schedule repair for later (or do it immediately if critical)
                    let repair_results = {
                        let mut handler = self.corruption_handler.lock().await;
                        handler.auto_repair().await?
                    };
                    
                    let successful = repair_results.iter().filter(|r| r.success).count();
                    info!("Auto-repair completed: fixed {}/{} issues", 
                         successful, repair_results.len());
                },
                Err(e) => {
                    error!("Periodic integrity check failed: {}", e);
                }
            }
        }
        
        Ok(())
    }
}

// Helper function to determine if a corruption type is critical
fn is_critical_corruption(corruption_type: &crate::storage::corruption::CorruptionType) -> bool {
    use crate::storage::corruption::CorruptionType;
    
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

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    // Create node instance
    let node = Arc::new(Node::new().await?);
    let node_clone = Arc::clone(&node);

    // Watch for configuration changes
    let config_rx = NodeConfig::watch_config().await?;
    let config_node = Arc::clone(&node);
    let config_handle = tokio::spawn(async move {
        let mut config_rx = config_rx;
        while let Some(_) = config_rx.recv().await {
            info!("Configuration change detected");
            if let Err(e) = config_node.handle_config_reload().await {
                error!("Failed to reload configuration: {}", e);
            }
        }
    });

    // Set up network and sync components
    let (mut network, command_tx, mut event_rx) = P2PNetwork::new(
        None,
        node.chain_state.get_genesis_hash(),
        &node.config.lock().await.node.chain_id
    ).await?;

    // Initialize the enhanced sync system
    let db = Arc::clone(&node.chain_state.db);
    let mut sync = ChainSync::new(
        node.chain_state.clone(),
        db,
        command_tx.clone()
    );

    // Load checkpoints at startup
    if let Err(e) = sync.load_checkpoints().await {
        error!("Failed to load checkpoints: {}", e);
    }

    // Set up metrics for the sync system
    let metrics = Arc::new(DefaultSyncMetrics);
    sync = sync.with_metrics(metrics);

    // Set up periodic sync timeout handler
    let sync_clone = sync.clone();
    let sync_timeout_handle = tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(5));
        loop {
            interval.tick().await;
            if let Err(e) = sync_clone.process_timeouts().await {
                error!("Error processing sync timeouts: {}", e);
            }
        }
    });

    // Start automated backups if enabled
    let backup_handle = if node.config.lock().await.backup.enable_automated_backups {
        let backup_manager = Arc::clone(&node.backup_manager);
        Some(tokio::spawn(async move {
            if let Err(e) = backup_manager.start_automated_backups().await {
                error!("Backup system error: {}", e);
            }
        }))
    } else {
        info!("Automated backups are disabled");
        None
    };

    // Setup periodic database maintenance task
    let db_maintenance_node = Arc::clone(&node);
    let db_maintenance_handle = tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(3600)); // Run every hour
        loop {
            interval.tick().await;
            if let Err(e) = db_maintenance_node.perform_database_maintenance().await {
                error!("Database maintenance error: {}", e);
            }
        }
    });

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
                        if let Err(e) = node_clone.create_integrity_checkpoint(height, block.hash()).await {
                            warn!("Failed to create integrity checkpoint: {}", e);
                        }
                    }
                },
                
                // Handle new transaction received
                NetworkEvent::NewTransaction { transaction, fee_rate, from_peer } => {
                    debug!("Received new transaction from {:?}", from_peer);
                    
                    if let Err(e) = node_clone.handle_new_transaction(transaction).await {
                        error!("Failed to process transaction: {}", e);
                    }
                },
                
                // Handle block headers received
                NetworkEvent::BlockHeaders { headers, total_difficulty, from_peer } => {
                    info!("Received {} headers from {:?}", headers.len(), from_peer);
                    
                    if let Err(e) = sync.handle_headers(headers, from_peer.as_ref()).await {
                        error!("Failed to process headers: {}", e);
                        
                        if let Some(peer_id) = from_peer {
                            command_tx.send(NetworkCommand::BanPeer {
                                peer_id,
                                reason: format!("Invalid headers: {}", e),
                                duration: Some(Duration::from_secs(1800)),
                            }).await.ok();
                        }
                    }
                },
                
                // Handle blocks received
                NetworkEvent::BlocksReceived { blocks, total_difficulty, from_peer } => {
                    info!("Received {} blocks in response from {:?}", blocks.len(), from_peer);
                    
                    // Process each block
                    for block in blocks {
                        let height = block.height();
                        
                        if let Err(e) = sync.handle_new_block(block.clone(), height, total_difficulty, from_peer.as_ref()).await {
                            error!("Failed to process block from batch: {}", e);
                            break;
                        }
                        
                        // Create integrity checkpoint if applicable
                        if let Err(e) = node_clone.create_integrity_checkpoint(height, block.hash()).await {
                            warn!("Failed to create integrity checkpoint: {}", e);
                        }
                    }
                },
                
                // Handle peer status update
                NetworkEvent::PeerStatus { peer_id, version, height, best_hash, total_difficulty } => {
                    debug!("Peer {} status: height={}, td={}", peer_id, height, total_difficulty);
                    
                    // Check if we need to sync
                    let current_height = node_clone.chain_state.get_height();
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
    let config_lock = node.config.lock().await;
    command_tx.send(NetworkCommand::StartListening(
        config_lock.network.listen_addr.clone()
    )).await?;
    drop(config_lock);

    // Announce initial status
    command_tx.send(NetworkCommand::AnnounceStatus {
        version: 1,
        height: node.chain_state.get_height(),
        best_hash: node.chain_state.get_best_block_hash(),
        total_difficulty: node.chain_state.get_total_difficulty(),
    }).await?;

    // Set up periodic tasks
    let mut mempool_cleanup_interval = tokio::time::interval(node.config.lock().await.mempool.transaction_timeout);
    let mut status_announcement_interval = tokio::time::interval(Duration::from_secs(120));
    
    // Main event loop
    loop {
        tokio::select! {
            // Periodic mempool cleanup
            _ = mempool_cleanup_interval.tick() => {
                let removed = node.mempool.clear_expired();
                if removed > 0 {
                    info!("Cleared {} expired transactions from mempool", removed);
                }
            },
            
            // Periodic status announcement
            _ = status_announcement_interval.tick() => {
                // Announce our current status to the network
                let current_height = node.chain_state.get_height();
                let best_hash = node.chain_state.get_best_block_hash();
                let total_difficulty = node.chain_state.get_total_difficulty();
                
                command_tx.send(NetworkCommand::AnnounceStatus {
                    version: 1,
                    height: current_height,
                    best_hash,
                    total_difficulty,
                }).await.ok();
                
                // Log sync status
                let sync_stats = sync.get_stats();
                info!("Sync status: {}. Current height: {}, Target height: {}", 
                     sync_stats.state, sync_stats.current_height, sync_stats.target_height);
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

    let config_lock = node.config.lock().await;
    if config_lock.backup.enable_automated_backups {
        match node.backup_manager.create_backup().await {
            Ok(backup_path) => info!("Created final backup at {:?}", backup_path),
            Err(e) => warn!("Failed to create final backup during shutdown: {}", e),
        }
    }

    // Final database integrity checkpoint before shutdown
    {
        let height = node.chain_state.get_height();
        let block_hash = node.chain_state.get_best_block_hash();
        let mut handler = node.corruption_handler.lock().await;
        if let Err(e) = handler.create_checkpoint(height, block_hash).await {
            warn!("Failed to create final integrity checkpoint: {}", e);
        } else {
            info!("Created final integrity checkpoint at height {}", height);
        }
    }

    info!("Shutdown complete");
    Ok(())
}