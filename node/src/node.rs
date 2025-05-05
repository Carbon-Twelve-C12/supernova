use crate::storage::{
    BackupManager, BlockchainDB, ChainState, 
    CheckpointManager, CheckpointConfig, CheckpointType,
    RecoveryManager, StorageError, UTXOSet
};
use crate::api::{ApiServer, ApiConfig};
use crate::network::NetworkManager;
use crate::storage::StorageManager;
use crate::mempool::MempoolManager;
use crate::config::NodeConfig;
use crate::environmental::EnvironmentalTracker;
use btclib::crypto::quantum::QuantumScheme;
use btclib::lightning::{LightningNetwork, LightningConfig, LightningNetworkError};
use btclib::lightning::wallet::LightningWallet;
use std::sync::{Arc, Mutex};
use tracing::{info, error, warn};

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
    /// API server instance
    pub api_server: Option<ApiServer>,
    /// Lightning Network integration
    lightning: Option<Arc<Mutex<LightningNetwork>>>,
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
            api_server: None,
            lightning: None,
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

    /// Start the API server
    pub async fn start_api(&mut self, bind_address: &str, port: u16) -> std::io::Result<()> {
        // Create API server with default configuration
        let api_server = ApiServer::new(Arc::new(self.clone()), bind_address, port);
        
        // Store the server instance
        self.api_server = Some(api_server.clone());
        
        // Start the server in a separate task
        let server_handle = api_server.start().await?;
        
        // Spawn a task to run the server
        tokio::spawn(async move {
            if let Err(e) = server_handle.await {
                error!("API server error: {}", e);
            }
        });
        
        info!("API server started on {}:{}", bind_address, port);
        Ok(())
    }

    /// Start the API server with custom configuration
    pub async fn start_api_with_config(&mut self, config: ApiConfig) -> std::io::Result<()> {
        // Create API server with custom configuration
        let api_server = ApiServer::new(Arc::new(self.clone()), &config.bind_address, config.port)
            .with_config(config.clone());
        
        // Store the server instance
        self.api_server = Some(api_server.clone());
        
        // Start the server in a separate task
        let server_handle = api_server.start().await?;
        
        // Spawn a task to run the server
        tokio::spawn(async move {
            if let Err(e) = server_handle.await {
                error!("API server error: {}", e);
            }
        });
        
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
        let lightning = LightningNetwork::new(config, wallet);
        
        // Store in node
        self.lightning = Some(Arc::new(Mutex::new(lightning)));
        
        info!("Lightning Network functionality initialized successfully");
        
        Ok(())
    }
    
    /// Get the Lightning Network manager
    pub fn lightning(&self) -> Option<Arc<Mutex<LightningNetwork>>> {
        self.lightning.clone()
    }
    
    /// Register the Lightning Network manager
    pub fn register_lightning(&mut self, lightning: LightningNetwork) {
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
        
        match lightning.open_channel(peer_id, capacity, push_amount, None).await {
            Ok(channel_id) => Ok(format!("{}", channel_id)),
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
        
        // Parse channel ID from string
        let channel_id = match channel_id.parse() {
            Ok(id) => id,
            Err(_) => return Err("Invalid channel ID format".to_string()),
        };
        
        match lightning.close_channel(&channel_id, force_close).await {
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
        
        match lightning.create_invoice(amount_msat, description, expiry_seconds) {
            Ok(invoice) => {
                // In a real implementation, this would encode as BOLT11
                Ok(format!("{}", invoice))
            },
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
        
        // Parse invoice from string (in a real implementation, this would parse BOLT11)
        let invoice = match invoice_str.parse() {
            Ok(invoice) => invoice,
            Err(_) => return Err("Invalid invoice format".to_string()),
        };
        
        match lightning.pay_invoice(&invoice).await {
            Ok(preimage) => Ok(format!("{}", hex::encode(preimage.into_inner()))),
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
        
        let channels = lightning.list_channels();
        let channel_ids = channels.iter().map(|id| format!("{}", id)).collect();
        
        Ok(channel_ids)
    }
    
    /// Get information about a specific channel
    pub fn get_channel_info(&self, channel_id: &str) -> Result<serde_json::Value, String> {
        let lightning = match &self.lightning {
            Some(lightning) => lightning,
            None => return Err("Lightning Network not initialized".to_string()),
        };
        
        let lightning = lightning.lock().unwrap();
        
        // Parse channel ID from string
        let channel_id = match channel_id.parse() {
            Ok(id) => id,
            Err(_) => return Err("Invalid channel ID format".to_string()),
        };
        
        match lightning.get_channel_info(&channel_id) {
            Some(info) => {
                // Convert channel info to JSON
                let json = serde_json::json!({
                    "id": channel_id.to_string(),
                    "state": format!("{:?}", info.state),
                    "capacity": info.capacity,
                    "local_balance_msat": info.local_balance_msat,
                    "remote_balance_msat": info.remote_balance_msat,
                    "is_public": info.is_public,
                    "pending_htlcs": info.pending_htlcs,
                    "uptime_seconds": info.uptime_seconds,
                    "update_count": info.update_count,
                });
                
                Ok(json)
            },
            None => Err(format!("Channel {} not found", channel_id)),
        }
    }
} 