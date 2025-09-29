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
            reward_addresses: {
                // Extract reward addresses from template (simplified)
                // In production, would parse coinbase transaction data
                vec![]
            },
        }
    }
}

/// Mining worker
#[derive(Debug)]
pub struct MiningWorker {
    id: usize,
    config: MiningConfig,
    mempool: std::sync::Arc<crate::mempool::TransactionPool>,
    block_sender: tokio::sync::mpsc::UnboundedSender<crate::types::Block>,
    running: std::sync::Arc<std::sync::atomic::AtomicBool>,
    thread_handle: Option<std::thread::JoinHandle<()>>,
}

impl MiningWorker {
    pub fn new(
        id: usize,
        config: MiningConfig,
        mempool: std::sync::Arc<crate::mempool::TransactionPool>,
        block_sender: tokio::sync::mpsc::UnboundedSender<crate::types::Block>,
    ) -> Self {
        Self { 
            id,
            config,
            mempool,
            block_sender,
            running: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
            thread_handle: None,
        }
    }

    pub fn start(&mut self, template: BlockTemplate) {
        if self.running.load(std::sync::atomic::Ordering::Relaxed) {
            return; // Already running
        }
        
        self.running.store(true, std::sync::atomic::Ordering::Relaxed);
        
        let id = self.id;
        let config = self.config.clone();
        let mempool = self.mempool.clone();
        let block_sender = self.block_sender.clone();
        let running = self.running.clone();
        
        let handle = std::thread::spawn(move || {
            Self::mining_loop(id, config, mempool, block_sender, running, template);
        });
        
        self.thread_handle = Some(handle);
    }

    pub fn stop(&mut self) {
        self.running.store(false, std::sync::atomic::Ordering::Relaxed);
        
        if let Some(handle) = self.thread_handle.take() {
            let _ = handle.join();
        }
    }
    
    fn mining_loop(
        id: usize,
        config: MiningConfig,
        mempool: std::sync::Arc<crate::mempool::TransactionPool>,
        block_sender: tokio::sync::mpsc::UnboundedSender<crate::types::Block>,
        running: std::sync::Arc<std::sync::atomic::AtomicBool>,
        template: BlockTemplate,
    ) {
        use crate::types::{Block, BlockHeader, Transaction, TransactionInput, TransactionOutput};
        
        use std::time::{SystemTime, UNIX_EPOCH};
        
        let mut nonce = 0u32;
        let mut hash_count = 0u64;
        let start_time = std::time::Instant::now();
        
        // Create coinbase transaction
        let coinbase_tx = Transaction::new(
            1,
            vec![TransactionInput::new_coinbase(format!("Mined by worker {}", id).into_bytes())],
            vec![TransactionOutput::new(50_000_000_00, vec![])], // 50 NOVA reward
            0,
        );
        
        // Get transactions from mempool
        let mut transactions = vec![coinbase_tx];
        let mempool_txs = mempool.get_sorted_transactions();
        transactions.extend(mempool_txs.into_iter().take(1000)); // Max 1000 transactions
        
        while running.load(std::sync::atomic::Ordering::Relaxed) {
            // Create block header
            let timestamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs();
            
            let header = BlockHeader::new(
                template.version,
                template.prev_hash,
                [0u8; 32], // Merkle root will be calculated
                timestamp,
                template.target,
                nonce,
            );
            
            // Create block
            let mut block = Block::new(header, transactions.clone());
            
            // Calculate and set merkle root
            let merkle_root = block.calculate_merkle_root();
            block.header.merkle_root = merkle_root;
            
            // Check if block meets difficulty target
            let hash = block.header.hash();
            let target = block.header.target();
            
            if hash <= target {
                // Found a valid block!
                tracing::info!("Worker {} found block with hash: {}", id, hex::encode(hash));
                let _ = block_sender.send(block);
                break;
            }
            
            nonce = nonce.wrapping_add(1);
            hash_count += 1;
            
            // Apply mining intensity throttling
            if config.mining_intensity < 1.0 {
                let sleep_time = ((1.0 - config.mining_intensity) * 1000.0) as u64;
                std::thread::sleep(std::time::Duration::from_micros(sleep_time));
            }
            
            // Log hashrate periodically
            if hash_count % 100000 == 0 {
                let elapsed = start_time.elapsed().as_secs_f64();
                let hashrate = (hash_count as f64 / elapsed) as u64;
                tracing::debug!("Worker {} hashrate: {} H/s", id, hashrate);
            }
        }
    }
}

impl Drop for MiningWorker {
    fn drop(&mut self) {
        self.stop();
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