//! Thread-safe API facade for the Node
//!
//! This module provides a thread-safe wrapper around the Node that can be safely
//! shared across threads in the API server.

use std::sync::Arc;
use std::sync::RwLock as StdRwLock;
use tokio::sync::RwLock;
use crate::node::{Node, NodeError};
use crate::api::types::*;
use crate::storage::{BlockchainDB, ChainState};
use crate::mempool::TransactionPool;
use btclib::types::transaction::Transaction;
use btclib::types::block::Block;
use sysinfo::{System, Disks};

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
    /// Peer ID
    peer_id: libp2p::PeerId,
    /// Start time
    start_time: std::time::Instant,
}

// Ensure ApiFacade is Send + Sync
static_assertions::assert_impl_all!(ApiFacade: Send, Sync);

impl ApiFacade {
    /// Create a new API facade from a Node
    pub fn new(node: &Node) -> Self {
        Self {
            config: node.config(),
            db: node.db(),
            chain_state: node.chain_state(),
            mempool: node.mempool(),
            peer_id: node.peer_id.clone(),
            start_time: node.start_time,
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
    
    /// Get node info
    pub fn get_node_info(&self) -> Result<NodeInfo, NodeError> {
        let chain_height = self.chain_state.read().unwrap().get_height() as u64;
        let best_block_hash = self.chain_state.read().unwrap().get_best_block_hash();
        
        Ok(NodeInfo {
            node_id: self.peer_id.to_string(),
            version: "0.1.0".to_string(),
            protocol_version: 1,
            network: "supernova-testnet".to_string(), 
            height: chain_height,
            best_block_hash: hex::encode(best_block_hash),
            connections: 0, // TODO: Implement proper connection tracking
            synced: true, // TODO: Get actual sync state
            uptime: System::uptime(),
        })
    }
    
    /// Get node status
    pub async fn get_status(&self) -> NodeStatus {
        let chain_height = self.chain_state.read().unwrap().get_height() as u64;
        let best_block_hash = self.chain_state.read().unwrap().get_best_block_hash();
        
        NodeStatus {
            state: "synced".to_string(), // TODO: Get actual sync state
            height: chain_height,
            best_block_hash: hex::encode(best_block_hash),
            peer_count: 0, // TODO: Get from network
            mempool_size: self.mempool.size(),
            is_mining: false, // TODO: Get from miner
            hashrate: 0,
            difficulty: 1.0,
            network_hashrate: 0,
        }
    }
    
    /// Get system info
    pub fn get_system_info(&self) -> Result<SystemInfo, NodeError> {
        let mut sys = System::new_all();
        
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
            git_commit: "unknown".to_string(), // TODO: Get from build info
            build_date: "unknown".to_string(), // TODO: Get from build info
            rust_version: "unknown".to_string(), // TODO: Get from build info
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
        
        Ok(NodeMetrics {
            uptime: self.start_time.elapsed().as_secs(),
            peer_count: 0, // TODO: Get from network
            block_height: self.chain_state.read().unwrap().get_height() as u64,
            mempool_size: self.mempool.size(),
            mempool_bytes: self.mempool.get_memory_usage() as usize,
            sync_progress: 1.0,
            network_bytes_sent: 0,
            network_bytes_received: 0,
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
        let updated_config: crate::config::NodeConfig = serde_json::from_value(new_config)
            .map_err(|e| NodeError::ConfigError(format!("Invalid config: {}", e)))?;
        
        updated_config.validate()
            .map_err(|e| NodeError::ConfigError(e.to_string()))?;
        
        let mut config = self.config.write().unwrap();
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
        }).map_err(|e| NodeError::StorageError(e))?;
        
        let metadata = std::fs::metadata(&backup_path)
            .map_err(|e| NodeError::IoError(e))?;
        
        Ok(BackupInfo {
            id: uuid::Uuid::new_v4().to_string(),
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
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
        let performance_metrics = serde_json::json!({
            "uptime": self.start_time.elapsed().as_secs(),
            "memory_usage": 0, // TODO: Implement
            "cpu_usage": 0.0, // TODO: Implement
        });
        
        // Get network stats
        let network_stats = serde_json::json!({
            "peer_count": 0, // TODO: Get from network
            "bytes_sent": 0,
            "bytes_received": 0,
        });
        
        // Get mempool stats
        let mempool_stats = serde_json::json!({
            "size": self.mempool.size(),
            "memory_usage": self.mempool.get_memory_usage(),
        });
        
        // Get blockchain stats
        let chain_height = self.chain_state.read().unwrap().get_height();
        let blockchain_stats = serde_json::json!({
            "height": chain_height,
            "best_block_hash": hex::encode(self.chain_state.read().unwrap().get_best_block_hash()),
        });
        
        // Get lightning stats
        let lightning_stats = serde_json::json!({
            "enabled": false, // TODO: Check if lightning is enabled
            "channels": 0,
            "peers": 0,
        });
        
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
    pub fn broadcast_transaction(&self, _tx: &Transaction) {
        // TODO: Need to communicate with network through channels
    }
} 