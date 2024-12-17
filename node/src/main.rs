use crate::network::{P2PNetwork, NetworkCommand, NetworkEvent};
use crate::storage::{ChainState, BlockchainDB, BackupManager, RecoveryManager};
use crate::mempool::{TransactionPool, TransactionPrioritizer};
use crate::config::NodeConfig;
use tracing::{info, error, warn};
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};
use std::path::PathBuf;
use std::time::Duration;
use bincode;

struct Node {
    config: Arc<Mutex<NodeConfig>>,
    mempool: Arc<TransactionPool>,
    prioritizer: Arc<Mutex<TransactionPrioritizer>>,
    network: P2PNetwork,
    chain_state: ChainState,
    backup_manager: Arc<BackupManager>,
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
        let chain_state = ChainState::new(Arc::clone(&db))?;

        let backup_config = config.lock().await.backup.clone();
        let backup_manager = Arc::new(BackupManager::new(
            Arc::clone(&db),
            backup_config.backup_dir.clone(),
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

        let (network, _, _) = P2PNetwork::new().await?;

        Ok(Self {
            config,
            mempool,
            prioritizer,
            network,
            chain_state,
            backup_manager,
        })
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
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    let node = Arc::new(Node::new().await?);
    let node_clone = Arc::clone(&node);

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

    let (mut network, command_tx, mut event_rx) = P2PNetwork::new().await?;
    let mut sync = network::ChainSync::new(node.chain_state.clone(), command_tx.clone());

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

    let event_handle = tokio::spawn(async move {
        while let Some(event) = event_rx.recv().await {
            match event {
                NetworkEvent::NewPeer(peer_id) => {
                    info!("New peer connected: {}", peer_id);
                }
                NetworkEvent::PeerLeft(peer_id) => {
                    info!("Peer disconnected: {}", peer_id);
                }
                NetworkEvent::NewBlock(block_data) => {
                    match bincode::deserialize(&block_data) {
                        Ok((block, height, total_difficulty)) => {
                            if let Err(e) = sync.handle_new_block(block, height, total_difficulty).await {
                                error!("Failed to process new block: {}", e);
                            }
                        }
                        Err(e) => error!("Failed to deserialize block: {}", e),
                    }
                }
                NetworkEvent::NewTransaction(tx_data) => {
                    match bincode::deserialize(&tx_data) {
                        Ok(transaction) => {
                            if let Err(e) = node_clone.handle_new_transaction(transaction).await {
                                error!("Failed to process transaction: {}", e);
                            }
                        }
                        Err(e) => error!("Failed to deserialize transaction: {}", e),
                    }
                }
            }
        }
    });

    let network_handle = tokio::spawn(async move {
        if let Err(e) = network.run().await {
            error!("Network error: {}", e);
        }
    });

    let config_lock = node.config.lock().await;
    command_tx.send(NetworkCommand::StartListening(
        config_lock.network.listen_addr.clone()
    )).await?;
    drop(config_lock);

    let node_clone = Arc::clone(&node);
    let maintenance_handle = tokio::spawn(async move {
        let mut interval = tokio::time::interval(node_clone.config.lock().await.mempool.transaction_timeout);
        loop {
            interval.tick().await;
            let removed = node_clone.mempool.clear_expired();
            if removed > 0 {
                info!("Cleared {} expired transactions from mempool", removed);
            }
        }
    });

    tokio::signal::ctrl_c().await?;
    info!("Shutting down...");

    event_handle.abort();
    network_handle.abort();
    maintenance_handle.abort();
    config_handle.abort();
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

    Ok(())
}