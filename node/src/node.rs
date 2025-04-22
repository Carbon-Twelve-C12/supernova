use crate::storage::{
    BackupManager, BlockchainDB, ChainState, 
    CheckpointManager, CheckpointConfig, CheckpointType,
    RecoveryManager, StorageError, UTXOSet
};

pub struct Node {
    pub config: NodeConfig,
    pub chain_state: Arc<RwLock<ChainState>>,
    pub blockchain_db: Arc<RwLock<BlockchainDB>>,
    pub utxo_set: Arc<RwLock<UTXOSet>>,
    pub network_manager: Arc<NetworkManager>,
    pub block_validator: Arc<BlockValidator>,
    pub tx_validator: Arc<TransactionValidator>,
    pub backup_manager: Option<Arc<BackupManager>>,
    pub recovery_manager: Option<Arc<RecoveryManager>>,
    pub checkpoint_manager: Option<Arc<CheckpointManager>>,
    pub rpc_server: Option<Arc<RpcServer>>,
    pub is_running: Arc<AtomicBool>,
    pub mem_pool: Arc<RwLock<MemPool>>,
}

impl Node {
    pub fn new(config: NodeConfig) -> Result<Self, NodeError> {
        // ... existing code ...

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

        // ... existing code ...

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
        })
    }

    pub fn start(&self) -> Result<(), NodeError> {
        // ... existing code ...

        // Start checkpoint manager if enabled
        if let Some(checkpoint_manager) = &self.checkpoint_manager {
            checkpoint_manager.start()?;
        }

        // ... existing code ...
        
        Ok(())
    }

    pub fn stop(&self) -> Result<(), NodeError> {
        // ... existing code ...

        // Stop checkpoint manager if enabled
        if let Some(checkpoint_manager) = &self.checkpoint_manager {
            checkpoint_manager.stop()?;
        }

        // ... existing code ...
        
        Ok(())
    }

    // ... existing code ...
} 