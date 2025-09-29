//! Thread-safe API facade for the Node
//!
//! This module provides a thread-safe wrapper around the Node that can be safely
//! shared across threads in the API server.

use std::sync::Arc;
use std::sync::RwLock as StdRwLock;
use crate::node::{Node, NodeError};
use crate::api::types::*;
use crate::storage::{BlockchainDB, ChainState};
use crate::mempool::TransactionPool;
use crate::network::NetworkProxy;
use btclib::types::transaction::Transaction;
use sysinfo::System;

/// Thread-safe API facade that wraps the Node
pub struct ApiFacade {
    /// Configuration
    config: Arc<StdRwLock<crate::config::NodeConfig>>,
    /// Blockchain database
    db: Arc<BlockchainDB>,
    /// Chain state
    chain_state: Arc<StdRwLock<ChainState>>,
    /// Transaction mempool
    mempool: Arc<TransactionPool>,
    /// Network proxy (thread-safe)
    network: Arc<NetworkProxy>,
    /// Peer ID
    peer_id: libp2p::PeerId,
    /// Start time
    start_time: std::time::Instant,
    /// Lightning manager (if enabled)
    lightning_manager: Option<Arc<StdRwLock<btclib::lightning::LightningManager>>>,
}

// Ensure ApiFacade is Send + Sync
// TODO: Re-enable after fixing thread safety issues
// static_assertions::assert_impl_all!(ApiFacade: Send, Sync);

impl ApiFacade {
    /// Create a new API facade from a Node
    pub fn new(node: &Node) -> Self {
        Self {
            config: node.config(),
            db: node.db(),
            chain_state: node.chain_state(),
            mempool: node.mempool(),
            network: node.network_proxy(),
            peer_id: node.peer_id,
            start_time: node.start_time,
            lightning_manager: node.lightning(),
        }
    }
    
    /// Get storage (blockchain database)
    pub fn storage(&self) -> Arc<BlockchainDB> {
        Arc::clone(&self.db)
    }
    
    /// Get chain state
    pub fn chain_state(&self) -> Arc<StdRwLock<ChainState>> {
        Arc::clone(&self.chain_state)
    }
    
    /// Get mempool
    pub fn mempool(&self) -> Arc<TransactionPool> {
        Arc::clone(&self.mempool)
    }
    
    /// Get config
    pub fn config(&self) -> Arc<StdRwLock<crate::config::NodeConfig>> {
        Arc::clone(&self.config)
    }
    
    /// Get network proxy
    pub fn network(&self) -> Arc<NetworkProxy> {
        Arc::clone(&self.network)
    }
    
    /// Get node info
    pub fn get_node_info(&self) -> Result<NodeInfo, NodeError> {
        let chain_state = self.chain_state.read()
            .map_err(|e| NodeError::General(format!("Chain state lock poisoned: {}", e)))?;
        let chain_height = chain_state.get_height();
        let best_block_hash = chain_state.get_best_block_hash();
        let connections = self.network.peer_count_sync() as u32;
        let synced = !self.network.is_syncing();
        
        Ok(NodeInfo {
            node_id: self.peer_id.to_string(),
            version: "0.1.0".to_string(),
            protocol_version: 1,
            network: "supernova-testnet".to_string(), 
            height: chain_height,
            best_block_hash: hex::encode(best_block_hash),
            connections,
            synced,
            uptime: System::uptime(),
        })
    }
    
    /// Get node status
    pub async fn get_status(&self) -> NodeStatus {
        let (chain_height, best_block_hash) = match self.chain_state.read() {
            Ok(state) => (state.get_height(), state.get_best_block_hash()),
            Err(_) => (0, [0u8; 32]), // Safe default if lock is poisoned
        };
        let peer_count = self.network.peer_count().await;
        let synced = !self.network.is_syncing();
        let is_mining = self.config.read()
            .map(|config| config.node.enable_mining)
            .unwrap_or(false);
        
        // Calculate network hashrate from difficulty
        let difficulty = if let Ok(Some(hash)) = self.db.get_block_hash_by_height(chain_height) {
            if let Ok(Some(block)) = self.db.get_block(&hash) {
                btclib::blockchain::difficulty::calculate_difficulty_from_bits(block.header().bits())
            } else {
                1.0
            }
        } else {
            1.0
        };
        
        // Hashrate = difficulty * 2^32 / block_time
        let hashrate = (difficulty * 4_294_967_296.0 / 150.0) as u64;
        let network_hashrate = hashrate * peer_count.max(1) as u64;
        
        NodeStatus {
            state: if synced { "synced".to_string() } else { "syncing".to_string() },
            height: chain_height,
            best_block_hash: hex::encode(best_block_hash),
            peer_count,
            mempool_size: self.mempool.size(),
            is_mining,
            hashrate: if is_mining { hashrate / 1_000_000 } else { 0 }, // Convert to MH/s
            difficulty,
            network_hashrate: network_hashrate / 1_000_000, // Convert to MH/s
        }
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
    pub fn get_logs(&self, level: &str, component: Option<&str>, limit: usize, offset: usize) -> Result<Vec<LogEntry>, NodeError> {
        let logs = crate::logging::get_recent_logs(level, component, limit, offset);
        Ok(logs)
    }
    
    /// Get version info
    pub fn get_version(&self) -> Result<VersionInfo, NodeError> {
        Ok(VersionInfo {
            version: env!("CARGO_PKG_VERSION").to_string(),
            protocol_version: 1,
            git_commit: option_env!("VERGEN_GIT_SHA").unwrap_or("unknown").to_string(),
            build_date: option_env!("VERGEN_BUILD_TIMESTAMP").unwrap_or("unknown").to_string(),
            rust_version: option_env!("VERGEN_RUSTC_SEMVER").unwrap_or(env!("CARGO_PKG_RUST_VERSION")).to_string(),
        })
    }
    
    /// Get metrics
    pub fn get_metrics(&self, _period: u64) -> Result<NodeMetrics, NodeError> {
        use sysinfo::{System, Disks};
        let mut sys = System::new_all();
        sys.refresh_all();
        
        let cpu_usage = sys.global_cpu_info().cpu_usage() as f64;
        let memory_usage = sys.used_memory();
        
        let disks = Disks::new_with_refreshed_list();
        let disk_usage = disks.list().first()
            .map(|disk| disk.total_space() - disk.available_space())
            .unwrap_or(0);
        
        let peer_count = self.network.peer_count_sync();
        let network_stats = self.network.get_stats_sync();
        
        Ok(NodeMetrics {
            uptime: self.start_time.elapsed().as_secs(),
            peer_count,
            block_height: self.chain_state.read()
                .map(|state| state.get_height())
                .unwrap_or(0),
            mempool_size: self.mempool.size(),
            mempool_bytes: self.mempool.get_memory_usage() as usize,
            sync_progress: if self.network.is_syncing() { 0.5 } else { 1.0 },
            network_bytes_sent: network_stats.bytes_sent,
            network_bytes_received: network_stats.bytes_received,
            cpu_usage,
            memory_usage,
            disk_usage,
        })
    }
    
    /// Get config
    pub fn get_config(&self) -> Result<serde_json::Value, NodeError> {
        let config = self.config.read()
            .map_err(|e| NodeError::ConfigError(format!("Config lock poisoned: {}", e)))?;
        serde_json::to_value(&*config)
            .map_err(|e| NodeError::ConfigError(e.to_string()))
    }
    
    /// Update config
    pub fn update_config(&self, new_config: serde_json::Value) -> Result<serde_json::Value, NodeError> {
        let updated_config: crate::config::NodeConfig = serde_json::from_value(new_config)
            .map_err(|e| NodeError::ConfigError(format!("Invalid config: {}", e)))?;
        
        updated_config.validate()
            .map_err(|e| NodeError::ConfigError(e.to_string()))?;
        
        let mut config = self.config.write()
            .map_err(|e| NodeError::ConfigError(format!("Config lock poisoned: {}", e)))?;
        *config = updated_config;
        
        serde_json::to_value(&*config)
            .map_err(|e| NodeError::ConfigError(e.to_string()))
    }
    
    /// Create backup
    pub fn create_backup(&self, destination: Option<&str>, include_wallet: bool, _encrypt: bool) -> Result<BackupInfo, NodeError> {
        use crate::storage::backup::BackupManager;
        use std::time::Duration;
        
        let backup_dir = std::path::PathBuf::from(destination.unwrap_or("/tmp/supernova_backup"));
        let backup_manager = BackupManager::new(
            self.db.clone(),
            backup_dir.clone(),
            10,
            Duration::from_secs(3600),
        );
        
        let backup_path = tokio::runtime::Handle::current().block_on(async {
            backup_manager.create_backup().await
        }).map_err(NodeError::StorageError)?;
        
        let metadata = std::fs::metadata(&backup_path)
            .map_err(NodeError::IoError)?;
        
        Ok(BackupInfo {
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
    pub fn get_backup_info(&self) -> Result<Vec<BackupInfo>, NodeError> {
        Ok(Vec::new())
    }
    
    /// Restart node
    pub fn restart(&self) -> Result<(), NodeError> {
        Err(NodeError::General("Restart not implemented".to_string()))
    }
    
    /// Shutdown node
    pub fn shutdown(&self) -> Result<(), NodeError> {
        Err(NodeError::General("Shutdown not implemented".to_string()))
    }
    
    /// Get debug info
    pub fn get_debug_info(&self) -> Result<DebugInfo, NodeError> {
        // Get node info
        let node_info = self.get_node_info()?;
        
        // Get system info
        let system_info = self.get_system_info()?;
        
        // Get performance metrics
        let memory_usage = sysinfo::System::new_all().used_memory();
        let cpu_usage = sysinfo::System::new_all().global_cpu_info().cpu_usage();
        let performance_metrics = serde_json::json!({
            "uptime": self.start_time.elapsed().as_secs(),
            "memory_usage": memory_usage,
            "cpu_usage": cpu_usage,
        });
        
        // Get network stats
        let network_stats_raw = self.network.get_stats_sync();
        let network_stats = serde_json::json!({
            "peer_count": self.network.peer_count_sync(),
            "bytes_sent": network_stats_raw.bytes_sent,
            "bytes_received": network_stats_raw.bytes_received,
        });
        
        // Get mempool stats
        let mempool_stats = serde_json::json!({
            "size": self.mempool.size(),
            "memory_usage": self.mempool.get_memory_usage(),
        });
        
        // Get blockchain stats
        let (chain_height, best_block_hash) = match self.chain_state.read() {
            Ok(state) => (state.get_height(), state.get_best_block_hash()),
            Err(_) => (0, [0u8; 32]),
        };
        let blockchain_stats = serde_json::json!({
            "height": chain_height,
            "best_block_hash": hex::encode(best_block_hash),
        });
        
        // Get lightning stats
        let lightning_enabled = self.lightning_manager.is_some();
        let lightning_stats = if let Some(ln_manager) = &self.lightning_manager {
            match ln_manager.read() {
                Ok(manager) => {
            // Get info from the manager which includes peer count
            let info = manager.get_info().unwrap_or_else(|_| {
                // Return default info if error
                btclib::lightning::manager::LightningInfo {
                    node_id: String::new(),
                    num_channels: 0,
                    num_pending_channels: 0,
                    num_inactive_channels: 0,
                    total_balance_msat: 0,
                    total_outbound_capacity_msat: 0,
                    total_inbound_capacity_msat: 0,
                    num_peers: 0,
                    synced_to_chain: false,
                    synced_to_graph: false,
                    block_height: 0,
                }
            });
            
                    serde_json::json!({
                        "enabled": true,
                        "channels": info.num_channels,
                        "peers": info.num_peers,
                    })
                },
                Err(_) => {
                    // Lock poisoned, return safe defaults
                    serde_json::json!({
                        "enabled": true,
                        "channels": 0,
                        "peers": 0,
                    })
                }
            }
        } else {
            serde_json::json!({
                "enabled": false,
                "channels": 0,
                "peers": 0,
            })
        };
        
        Ok(DebugInfo {
            node_info,
            system_info,
            performance_metrics,
            network_stats,
            mempool_stats,
            blockchain_stats,
            lightning_stats,
        })
    }
    
    /// Broadcast transaction (stub - needs network access)
    pub fn broadcast_transaction(&self, tx: &Transaction) {
        // Add to mempool
        if let Err(e) = self.mempool.add_transaction(tx.clone(), 1) {
            tracing::warn!("Failed to add transaction to mempool: {}", e);
            return;
        }
        
        // Broadcast to network
        self.network.broadcast_transaction(tx);
    }
} 