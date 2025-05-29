//! Thread Safety Fix for Supernova Node
//! 
//! This module implements the thread safety fixes required for Phase 9
//! to allow the Node struct to be safely shared across threads in the API server.

use std::sync::{Arc, RwLock, Mutex};
use std::time::Instant;
use libp2p::PeerId;
use tokio::sync::RwLock as TokioRwLock;
use crate::node::Node;
use crate::api::ApiConfig;

/// Thread-safe wrapper for Node that can be shared across threads
pub struct ThreadSafeNode {
    inner: Arc<TokioRwLock<Node>>,
}

impl ThreadSafeNode {
    /// Create a new thread-safe node wrapper
    pub fn new(node: Node) -> Self {
        Self {
            inner: Arc::new(TokioRwLock::new(node)),
        }
    }
    
    /// Get a reference to the inner node for reading
    pub async fn read(&self) -> tokio::sync::RwLockReadGuard<'_, Node> {
        self.inner.read().await
    }
    
    /// Get a mutable reference to the inner node
    pub async fn write(&self) -> tokio::sync::RwLockWriteGuard<'_, Node> {
        self.inner.write().await
    }
    
    /// Clone the Arc for sharing across threads
    pub fn clone_arc(&self) -> Arc<TokioRwLock<Node>> {
        self.inner.clone()
    }
}

// Make ThreadSafeNode implement Clone for easy sharing
impl Clone for ThreadSafeNode {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

/// Alternative approach: Create a thread-safe API facade
/// that only exposes the methods needed by the API server
#[derive(Clone)]
pub struct NodeApiFacade {
    // Core components that need to be accessed by API
    config: Arc<crate::config::NodeConfig>,
    chain_state: Arc<std::sync::RwLock<crate::storage::ChainState>>,
    blockchain_db: Arc<std::sync::RwLock<crate::storage::BlockchainDB>>,
    network: Arc<crate::network::P2PNetwork>,
    mempool: Arc<crate::mempool::TransactionPool>,
    performance_monitor: Arc<crate::metrics::performance::PerformanceMonitor>,
    peer_id: libp2p::PeerId,
    start_time: std::time::Instant,
    is_running: Arc<std::sync::atomic::AtomicBool>,
    
    // Lightning network components (wrapped for thread safety)
    lightning_manager: Option<Arc<std::sync::Mutex<btclib::lightning::manager::LightningManager>>>,
}

impl NodeApiFacade {
    /// Create a new API facade from a Node
    pub fn from_node(node: &Node) -> Self {
        Self {
            config: Arc::new(node.config.clone()),
            chain_state: node.chain_state.clone(),
            blockchain_db: node.blockchain_db.clone(),
            network: node.network.clone(),
            mempool: node.mempool.clone(),
            performance_monitor: node.performance_monitor.clone(),
            peer_id: node.peer_id.clone(),
            start_time: node.start_time,
            is_running: node.is_running.clone(),
            lightning_manager: node.lightning().clone(),
        }
    }
    
    // Implement all the methods needed by the API routes
    
    /// Get node information
    pub fn get_info(&self) -> Result<crate::api::types::NodeInfo, String> {
        Ok(crate::api::types::NodeInfo {
            node_id: self.peer_id.to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            protocol_version: 1,
            network: self.config.network.clone(),
            height: self.get_height(),
            best_block_hash: hex::encode(self.get_best_block_hash()),
            connections: self.network.get_peer_count() as u32,
            synced: self.is_synced(),
            uptime: self.start_time.elapsed().as_secs(),
        })
    }
    
    /// Get system information
    pub fn get_system_info(&self) -> Result<crate::api::types::SystemInfo, String> {
        let mut sys = System::new_all();
        sys.refresh_all();
        
        let load_avg = sys.load_average();
        
        Ok(crate::api::types::SystemInfo {
            os: sys.long_os_version().unwrap_or_else(|| "Unknown".to_string()),
            arch: std::env::consts::ARCH.to_string(),
            cpu_count: sys.cpus().len() as u32,
            total_memory: sys.total_memory(),
            used_memory: sys.used_memory(),
            total_swap: sys.total_swap(),
            used_swap: sys.used_swap(),
            uptime: sys.uptime(),
            load_average: crate::api::types::LoadAverage {
                one: load_avg.one,
                five: load_avg.five,
                fifteen: load_avg.fifteen,
            },
        })
    }
    
    /// Get node status
    pub fn get_status(&self) -> Result<crate::api::types::NodeStatus, String> {
        Ok(crate::api::types::NodeStatus {
            state: if self.is_synced() { "synced".to_string() } else { "syncing".to_string() },
            height: self.get_height(),
            best_block_hash: hex::encode(self.get_best_block_hash()),
            peer_count: self.network.get_peer_count(),
            mempool_size: self.mempool.size(),
            is_mining: false, // TODO: Get from mining manager
            hashrate: 0, // TODO: Get from mining manager
            difficulty: 1.0, // TODO: Get from blockchain
            network_hashrate: 0, // TODO: Calculate network hashrate
        })
    }
    
    /// Check if synced
    pub fn is_synced(&self) -> bool {
        // Simplified implementation
        true
    }
    
    /// Get blockchain height
    pub fn get_height(&self) -> u64 {
        self.chain_state.read()
            .map(|state| state.get_height())
            .unwrap_or(0)
    }
    
    /// Get best block hash
    pub fn get_best_block_hash(&self) -> [u8; 32] {
        self.chain_state.read()
            .map(|state| state.get_best_block_hash())
            .unwrap_or([0; 32])
    }
    
    /// Get performance metrics
    pub fn get_performance_metrics(&self) -> serde_json::Value {
        self.performance_monitor.get_report()
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
            "height": self.get_height(),
            "best_block_hash": hex::encode(self.get_best_block_hash()),
            "difficulty": 1.0, // TODO: Get from blockchain
            "total_work": "0", // TODO: Get from blockchain
            "chain_work": "0", // TODO: Get from blockchain
        })
    }
    
    /// Get Lightning Network statistics
    pub fn get_lightning_stats(&self) -> serde_json::Value {
        if let Some(lightning) = &self.lightning_manager {
            if let Ok(lightning) = lightning.lock() {
                // Use the LightningManager API to get comprehensive stats
                match lightning.get_info() {
                    Ok(info) => {
                        return serde_json::json!({
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
                        });
                    },
                    Err(_) => {
                        return serde_json::json!({
                            "enabled": true,
                            "error": "Failed to get Lightning Network info",
                        });
                    }
                }
            }
        }
        
        serde_json::json!({
            "enabled": false,
            "channel_count": 0,
            "total_capacity": 0,
            "local_balance": 0,
            "remote_balance": 0,
        })
    }
    
    /// Create invoice
    pub fn create_invoice(
        &self,
        amount_msat: u64,
        description: &str,
        expiry_seconds: u32,
    ) -> Result<String, String> {
        let lightning = self.lightning_manager.as_ref()
            .ok_or("Lightning Network not initialized")?;
        
        let lightning = lightning.lock().unwrap();
        
        match lightning.create_invoice(amount_msat, description, expiry_seconds, false) {
            Ok(response) => Ok(response.payment_request),
            Err(e) => Err(format!("Failed to create invoice: {}", e)),
        }
    }
    
    /// List channels
    pub fn list_channels(&self) -> Result<Vec<String>, String> {
        let lightning = self.lightning_manager.as_ref()
            .ok_or("Lightning Network not initialized")?;
        
        let lightning = lightning.lock().unwrap();
        
        match lightning.get_channels(false, true) {
            Ok(channels) => {
                let channel_ids = channels.iter().map(|ch| ch.channel_id.clone()).collect();
                Ok(channel_ids)
            },
            Err(e) => Err(format!("Failed to list channels: {}", e)),
        }
    }
    
    // Add async methods that need to interact with async components
    
    /// Open payment channel (async)
    pub async fn open_payment_channel(
        &self,
        peer_id: &str,
        capacity: u64,
        push_amount: u64,
    ) -> Result<String, String> {
        let lightning = self.lightning_manager.as_ref()
            .ok_or("Lightning Network not initialized")?;
        
        let lightning = lightning.lock().unwrap();
        
        match lightning.open_channel(peer_id, capacity, push_amount, false, None).await {
            Ok(response) => Ok(response.channel_id),
            Err(e) => Err(format!("Failed to open payment channel: {}", e)),
        }
    }
    
    /// Close payment channel (async)
    pub async fn close_payment_channel(
        &self,
        channel_id: &str,
        force_close: bool,
    ) -> Result<String, String> {
        let lightning = self.lightning_manager.as_ref()
            .ok_or("Lightning Network not initialized")?;
        
        let lightning = lightning.lock().unwrap();
        
        // Parse channel ID from string to u64
        let channel_id_u64: u64 = channel_id.parse()
            .map_err(|_| "Invalid channel ID format".to_string())?;
        
        match lightning.close_channel(&channel_id_u64.to_string(), force_close).await {
            Ok(success) => {
                if success {
                    Ok(format!("Channel {} closed successfully", channel_id))
                } else {
                    Err(format!("Failed to close channel {}", channel_id))
                }
            },
            Err(e) => Err(format!("Failed to close payment channel: {}", e)),
        }
    }
    
    /// Pay invoice (async)
    pub async fn pay_invoice(&self, invoice_str: &str) -> Result<String, String> {
        let lightning = self.lightning_manager.as_ref()
            .ok_or("Lightning Network not initialized")?;
        
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::NodeConfig;
    
    #[tokio::test]
    async fn test_thread_safe_node_wrapper() {
        let node = Node::new(NodeConfig::default()).unwrap();
        let safe_node = ThreadSafeNode::new(node);
        
        // Test cloning and sharing
        let safe_node_clone = safe_node.clone();
        
        // Test concurrent access
        let handle1 = tokio::spawn(async move {
            let node = safe_node.read().await;
            let _ = node.get_info();
        });
        
        let handle2 = tokio::spawn(async move {
            let node = safe_node_clone.read().await;
            let _ = node.get_status();
        });
        
        handle1.await.unwrap();
        handle2.await.unwrap();
    }
    
    #[test]
    fn test_node_api_facade() {
        let node = Node::new(NodeConfig::default()).unwrap();
        let facade = NodeApiFacade::from_node(&node);
        
        // Test that facade is Send + Sync
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<NodeApiFacade>();
        
        // Test basic methods
        let _ = facade.get_info();
        let _ = facade.get_status();
        let _ = facade.get_performance_metrics();
    }
} 