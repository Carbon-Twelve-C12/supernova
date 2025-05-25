pub mod manager;

pub use manager::*;

// Re-export common types
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use hex;

/// Mining configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MiningConfig {
    /// Number of mining threads
    pub mining_threads: u32,
    /// Mining intensity (0.0 to 1.0)
    pub mining_intensity: f64,
    /// Target temperature (Celsius)
    pub target_temperature: Option<f64>,
    /// Enable green mining features
    pub green_mining_enabled: bool,
    /// Quantum-resistant mining
    pub quantum_resistant: bool,
    /// Custom mining algorithm parameters
    pub algorithm_params: HashMap<String, serde_json::Value>,
    /// Energy efficiency in J/TH
    pub energy_efficiency_j_th: f64,
}

impl Default for MiningConfig {
    fn default() -> Self {
        Self {
            mining_threads: 4, // Default to 4 threads
            mining_intensity: 1.0,
            target_temperature: Some(80.0),
            green_mining_enabled: true,
            quantum_resistant: false,
            algorithm_params: HashMap::new(),
            energy_efficiency_j_th: 50.0,
        }
    }
}

/// Block template for mining
#[derive(Debug, Clone)]
pub struct BlockTemplate {
    /// Block version
    pub version: u32,
    /// Previous block hash
    pub prev_hash: [u8; 32],
    /// Block difficulty target
    pub target: u32,
    /// Reward addresses
    pub reward_addresses: Vec<String>,
}

impl BlockTemplate {
    pub fn new(
        version: u32,
        prev_hash: [u8; 32],
        target: u32,
        reward_addresses: Vec<String>,
        _mempool: &dyn MempoolInterface,
    ) -> Self {
        Self {
            version,
            prev_hash,
            target,
            reward_addresses,
        }
    }
    
    /// Create a BlockTemplate from a MiningTemplate
    pub fn from_mining_template(template: &crate::mining::manager::MiningTemplate) -> Self {
        // Parse the previous hash from hex string
        let prev_hash = if template.prev_hash.len() == 64 {
            let mut hash = [0u8; 32];
            if let Ok(bytes) = hex::decode(&template.prev_hash) {
                if bytes.len() == 32 {
                    hash.copy_from_slice(&bytes);
                }
            }
            hash
        } else {
            [0u8; 32]
        };
        
        Self {
            version: template.version,
            prev_hash,
            target: template.target,
            reward_addresses: vec![], // TODO: Extract from template transactions
        }
    }
}

/// Mining worker
#[derive(Debug)]
pub struct MiningWorker {
    id: usize,
}

impl MiningWorker {
    pub fn new(
        id: usize,
        _config: MiningConfig,
        _mempool: std::sync::Arc<crate::mempool::TransactionPool>,
        _block_sender: tokio::sync::mpsc::UnboundedSender<crate::types::Block>,
    ) -> Self {
        Self { id }
    }

    pub fn start(&self) {
        // TODO: Implement mining worker
    }

    pub fn stop(&self) {
        // TODO: Implement mining worker stop
    }
}

/// Mining metrics
#[derive(Debug, Clone)]
pub struct MiningMetrics {
    pub hashrate: u64,
    pub blocks_found: u64,
    pub uptime: u64,
}

/// Mempool interface for mining
#[async_trait::async_trait]
pub trait MempoolInterface {
    async fn get_transactions(&self, max_size: usize) -> Vec<crate::types::transaction::Transaction>;
} 