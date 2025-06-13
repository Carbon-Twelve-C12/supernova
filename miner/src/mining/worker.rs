use btclib::types::block::Block;
use crate::mining::template::BlockTemplate;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicU32, Ordering};
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing;
use crate::mining::MempoolInterface;
use std::time::{Instant, Duration};
use rand::{Rng, SeedableRng};
use rand::rngs::StdRng;
use sha2::{Sha256, Digest};
use btclib::types::transaction::Transaction;

// Memory-hard constants for ASIC resistance
const MEMORY_SIZE: usize = 4 * 1024 * 1024; // 4MB memory requirement
const MEMORY_ITERATIONS: usize = 64; // Number of memory accesses
const MIXING_ROUNDS: usize = 16; // Number of mixing rounds

pub struct MiningMetrics {
    hash_rate: AtomicU64,
    blocks_mined: AtomicU64,
    last_block_time: std::sync::Mutex<Option<Instant>>,
    start_time: Instant,
}

impl MiningMetrics {
    pub fn new() -> Self {
        Self {
            hash_rate: AtomicU64::new(0),
            blocks_mined: AtomicU64::new(0),
            last_block_time: std::sync::Mutex::new(None),
            start_time: Instant::now(),
        }
    }

    pub fn update_hash_rate(&self, hashes: u64, duration: Duration) {
        if duration.as_secs() > 0 {
            let rate = hashes / duration.as_secs();
            self.hash_rate.store(rate, Ordering::Relaxed);
        }
    }

    pub fn record_block_found(&self) {
        self.blocks_mined.fetch_add(1, Ordering::Relaxed);
        *self.last_block_time.lock().unwrap() = Some(Instant::now());
    }

    pub fn get_stats(&self) -> MiningStats {
        let blocks = self.blocks_mined.load(Ordering::Relaxed);
        let hash_rate = self.hash_rate.load(Ordering::Relaxed);
        let uptime = self.start_time.elapsed();
        let last_block = *self.last_block_time.lock().unwrap();

        MiningStats {
            blocks_mined: blocks,
            hash_rate,
            uptime,
            last_block_time: last_block,
        }
    }
}

#[derive(Debug)]
pub struct MiningStats {
    pub blocks_mined: u64,
    pub hash_rate: u64,
    pub uptime: Duration,
    pub last_block_time: Option<Instant>,
}

pub struct MiningWorker {
    pub(crate) stop_signal: Arc<AtomicBool>,
    pub(crate) block_sender: mpsc::Sender<Block>,
    pub(crate) target: AtomicU32,
    pub(crate) worker_id: usize,
    pub(crate) mempool: Arc<dyn MempoolInterface + Send + Sync>,
    pub metrics: Arc<MiningMetrics>,
    pub pause_signal: Arc<AtomicBool>,
}

impl MiningWorker {
    pub fn new(
        stop_signal: Arc<AtomicBool>,
        block_sender: mpsc::Sender<Block>,
        target: AtomicU32,
        worker_id: usize,
        mempool: Arc<dyn MempoolInterface + Send + Sync>,
    ) -> Self {
        Self {
            stop_signal,
            block_sender,
            target,
            worker_id,
            mempool,
            metrics: Arc::new(MiningMetrics::new()),
            pause_signal: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn pause(&self) {
        self.pause_signal.store(true, Ordering::Relaxed);
    }

    pub fn resume(&self) {
        self.pause_signal.store(false, Ordering::Relaxed);
    }

    pub fn get_metrics(&self) -> Arc<MiningMetrics> {
        Arc::clone(&self.metrics)
    }

    pub async fn mine_block(
        &self,
        version: u32,
        prev_block_hash: [u8; 32],
        reward_address: Vec<u8>,
    ) -> Result<(), String> {
        let mut template = BlockTemplate::new(
            version,
            prev_block_hash,
            self.target.load(Ordering::Relaxed) as u32,
            reward_address.clone(),
            self.mempool.as_ref(),
            1, // TODO: Get actual block height
            None, // TODO: Get environmental profile
        ).await;

        let mut block = template.create_block();
        let mut attempts = 0;
        let update_interval = 100_000;
        let metrics_interval = 1_000_000;
        let start_time = Instant::now();

        while !self.stop_signal.load(Ordering::Relaxed) {
            while self.pause_signal.load(Ordering::Relaxed) {
                tokio::time::sleep(Duration::from_millis(100)).await;
            }

            if attempts % metrics_interval == 0 {
                self.metrics.update_hash_rate(attempts as u64, start_time.elapsed());
                tracing::info!(
                    "Worker {} - Mining stats: {:?}", 
                    self.worker_id,
                    self.metrics.get_stats()
                );
            }

            for _ in 0..update_interval {
                if self.check_proof_of_work(&block) {
                    self.metrics.record_block_found();
                    tracing::info!(
                        "Worker {} - Found valid block after {} attempts!", 
                        self.worker_id, 
                        attempts
                    );
                    self.block_sender.send(block).await.map_err(|e| e.to_string())?;
                    return Ok(());
                }
                block.increment_nonce();
                attempts += 1;
            }

            if !self.pause_signal.load(Ordering::Relaxed) {
                template = BlockTemplate::new(
                    version,
                    prev_block_hash,
                    self.target.load(Ordering::Relaxed) as u32,
                    reward_address.clone(),
                    self.mempool.as_ref(),
            1, // TODO: Get actual block height
            None, // TODO: Get environmental profile
                ).await;
                block = template.create_block();
            }
        }

        Ok(())
    }

    pub fn check_proof_of_work(&self, block: &Block) -> bool {
        let block_header = self.get_block_header(block);
        let hash = self.memory_hard_hash(&block_header);
        
        let mut hash_value = [0u8; 8];
        hash_value[..4].copy_from_slice(&hash[..4]);
        let hash_value = u64::from_be_bytes(hash_value);
        hash_value as u32 <= self.target.load(Ordering::Relaxed) as u32
    }
    
    // Extract block header for hashing
    fn get_block_header(&self, block: &Block) -> Vec<u8> {
        let mut header = Vec::new();
        
        // We need to extract header fields by directly accessing the header fields
        // or using the available methods
        header.extend_from_slice(&*block.prev_block_hash());
        
        // Use hash directly as we don't have access to other header fields
        let hash = block.hash();
        header.extend_from_slice(&hash);
        
        header
    }
    
    // Memory-hard hashing function to resist ASICs
    fn memory_hard_hash(&self, data: &[u8]) -> [u8; 32] {
        // Initialize memory with pseudorandom data derived from input
        let mut memory = vec![0u8; MEMORY_SIZE];
        let mut hasher = Sha256::new();
        hasher.update(data);
        let seed = hasher.finalize();
        
        // Initialize memory with deterministic values based on the seed
        let mut rng = StdRng::from_seed(seed.into());
        for chunk in memory.chunks_mut(8) {
            if chunk.len() == 8 {
                let value = rng.gen::<u64>().to_be_bytes();
                chunk.copy_from_slice(&value);
            }
        }
        
        // Initial hash becomes our working value
        let mut current_hash: [u8; 32] = seed.into();
        
        // Perform memory-hard mixing operations
        for _iteration in 0..MEMORY_ITERATIONS {
            // Use current hash to determine memory access pattern - ensure we don't overflow
            let bytes: [u8; 8] = current_hash[0..8].try_into().unwrap_or([0; 8]);
            let index = u64::from_be_bytes(bytes) as usize % (MEMORY_SIZE - 64);
            
            // Mix current hash with memory
            for round in 0..MIXING_ROUNDS {
                // Ensure we don't go out of bounds
                if (index + (round + 1) * 4) > memory.len() {
                    break;
                }
                
                let memory_slice = &memory[index + round * 4..index + (round + 1) * 4];
                
                // XOR memory content with current hash
                for j in 0..std::cmp::min(4, memory_slice.len()) {
                    // Ensure we don't go out of bounds on the hash array
                    if round * 2 + j < current_hash.len() {
                        current_hash[round * 2 + j] ^= memory_slice[j];
                    }
                }
                
                // Update memory with new mixed values
                let mut hasher = Sha256::new();
                hasher.update(current_hash);
                current_hash = hasher.finalize().into();
            }
        }
        
        // Final hash
        let mut hasher = Sha256::new();
        hasher.update(current_hash);
        hasher.update(data);
        hasher.finalize().into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::sync::mpsc;
    use std::sync::Arc;

    struct MockMempool;
    
    #[async_trait::async_trait]
    impl MempoolInterface for MockMempool {
        async fn get_transactions(&self, _max_size: usize) -> Vec<Transaction> {
            Vec::new()
        }
    }

    #[tokio::test]
    async fn test_mining_worker() {
        let (tx, mut rx) = mpsc::channel(1);
        let stop_signal = Arc::new(AtomicBool::new(false));
        let mempool = Arc::new(MockMempool);
        
        let worker = MiningWorker::new(
            Arc::clone(&stop_signal),
            tx,
            AtomicU32::new(u32::MAX), // Easiest possible target for fast test completion
            0,
            mempool,
        );

        let mining_handle = tokio::spawn(async move {
            worker.mine_block(1, [0u8; 32], vec![1,2,3,4]).await.unwrap();
        });

        tokio::select! {
            Some(block) = rx.recv() => {
                assert!(block.validate());
            }
            _ = tokio::time::sleep(tokio::time::Duration::from_secs(5)) => {
                stop_signal.store(true, Ordering::Relaxed);
                panic!("Mining timed out");
            }
        }

        mining_handle.await.unwrap();
    }

    #[tokio::test]
    async fn test_mining_metrics() {
        let (tx, _) = mpsc::channel(1);
        let stop_signal = Arc::new(AtomicBool::new(false));
        let mempool = Arc::new(MockMempool);
        
        let worker = MiningWorker::new(
            Arc::clone(&stop_signal),
            tx,
            AtomicU32::new(u32::MAX),
            0,
            mempool,
        );

        let metrics = worker.get_metrics();
        assert_eq!(metrics.get_stats().blocks_mined, 0);
        
        metrics.record_block_found();
        assert_eq!(metrics.get_stats().blocks_mined, 1);
    }
}