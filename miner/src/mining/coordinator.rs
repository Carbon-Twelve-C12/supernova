use btclib::types::block::Block;
use btclib::types::transaction::{Transaction, TransactionInput, TransactionOutput};
use super::worker::MiningWorker;
use super::template::{BlockTemplate, MempoolInterface, BLOCK_MAX_SIZE};
use crate::difficulty::DifficultyAdjuster;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use tokio::sync::mpsc;
use tracing::{info, error, warn};
use async_trait::async_trait;
use std::time::{Duration, Instant};

pub struct MiningMetrics {
    total_hash_rate: AtomicU64,
    blocks_found: AtomicU64,
    active_workers: AtomicU64,
    start_time: Instant,
    last_hash_time: std::sync::Mutex<Option<Instant>>,
}

impl MiningMetrics {
    pub fn new() -> Self {
        Self {
            total_hash_rate: AtomicU64::new(0),
            blocks_found: AtomicU64::new(0),
            active_workers: AtomicU64::new(0),
            start_time: Instant::now(),
            last_hash_time: std::sync::Mutex::new(None),
        }
    }

    pub fn record_hash(&self, worker_id: usize, hashes: u64) {
        let current_rate = self.total_hash_rate.load(Ordering::Relaxed);
        self.total_hash_rate.store(current_rate + hashes, Ordering::Relaxed);
        *self.last_hash_time.lock().unwrap() = Some(Instant::now());
    }

    pub fn record_block(&self) {
        self.blocks_found.fetch_add(1, Ordering::Relaxed);
    }

    pub fn get_stats(&self) -> MiningStats {
        MiningStats {
            hash_rate: self.total_hash_rate.load(Ordering::Relaxed),
            blocks_found: self.blocks_found.load(Ordering::Relaxed),
            active_workers: self.active_workers.load(Ordering::Relaxed),
            uptime: self.start_time.elapsed(),
            last_hash: *self.last_hash_time.lock().unwrap(),
        }
    }
}

pub struct MiningStats {
    pub hash_rate: u64,
    pub blocks_found: u64,
    pub active_workers: u64,
    pub uptime: Duration,
    pub last_hash: Option<Instant>,
}

pub struct Miner {
    workers: Vec<Arc<MiningWorker>>,
    difficulty_adjuster: DifficultyAdjuster,
    stop_signal: Arc<AtomicBool>,
    block_sender: mpsc::Sender<Block>,
    num_threads: usize,
    mempool: Arc<dyn MempoolInterface + Send + Sync>,
    reward_address: Vec<u8>,
    metrics: Arc<MiningMetrics>,
}

impl Miner {
    pub fn new(
        num_threads: usize,
        initial_target: u32,
        mempool: Arc<dyn MempoolInterface + Send + Sync>,
        reward_address: Vec<u8>,
    ) -> (Self, mpsc::Receiver<Block>) {
        let (tx, rx) = mpsc::channel(100);
        let stop_signal = Arc::new(AtomicBool::new(false));
        let metrics = Arc::new(MiningMetrics::new());

        let mut workers = Vec::with_capacity(num_threads);
        for i in 0..num_threads {
            workers.push(Arc::new(MiningWorker::new(
                Arc::clone(&stop_signal),
                tx.clone(),
                initial_target,
                i,
                Arc::clone(&mempool),
            )));
        }

        (Self {
            workers,
            difficulty_adjuster: DifficultyAdjuster::new(initial_target),
            stop_signal,
            block_sender: tx,
            num_threads,
            mempool,
            reward_address,
            metrics,
        }, rx)
    }

    pub async fn start_mining(
        &self,
        version: u32,
        prev_block_hash: [u8; 32],
        current_height: u64,
    ) -> Result<(), String> {
        info!("Starting mining with {} workers", self.num_threads);
        self.metrics.active_workers.store(self.num_threads as u64, Ordering::Relaxed);

        let mut handles = Vec::new();
        let metrics = Arc::clone(&self.metrics);
        
        let metrics_handle = tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(10));
            loop {
                interval.tick().await;
                let stats = metrics.get_stats();
                info!("Mining stats: {:?}", stats);
            }
        });

        for worker in &self.workers {
            let worker = Arc::clone(worker);
            let reward_address = self.reward_address.clone();
            
            handles.push(tokio::spawn(async move {
                worker.mine_block(
                    version,
                    prev_block_hash,
                    reward_address,
                ).await
            }));
        }

        for handle in handles {
            if let Err(e) = handle.await {
                error!("Mining task error: {}", e);
                self.metrics.active_workers.fetch_sub(1, Ordering::Relaxed);
            }
        }

        metrics_handle.abort();
        Ok(())
    }

    pub fn stop_mining(&self) {
        self.stop_signal.store(true, Ordering::Relaxed);
        info!("Mining stopped");
    }

    pub fn adjust_difficulty(
        &mut self,
        current_height: u64,
        current_time: u64,
        blocks_since_adjustment: u64,
    ) {
        let new_target = self.difficulty_adjuster.adjust_difficulty(
            current_height,
            current_time,
            blocks_since_adjustment,
        );

        info!("Adjusting mining difficulty. New target: {:#x}", new_target);
        
        for worker in &self.workers {
            let worker_ptr = Arc::as_ptr(worker) as *mut MiningWorker;
            unsafe {
                (*worker_ptr).target = new_target;
            }
        }
    }

    pub fn get_current_target(&self) -> u32 {
        self.difficulty_adjuster.get_current_target()
    }

    pub fn set_reward_address(&mut self, address: Vec<u8>) {
        self.reward_address = address;
    }

    pub fn get_metrics(&self) -> Arc<MiningMetrics> {
        Arc::clone(&self.metrics)
    }

    pub fn pause_all_workers(&self) {
        for worker in &self.workers {
            worker.pause();
        }
    }

    pub fn resume_all_workers(&self) {
        for worker in &self.workers {
            worker.resume();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    struct MockMempool;
    
    #[async_trait]
    impl MempoolInterface for MockMempool {
        async fn get_transactions(&self, _max_size: usize) -> Vec<Transaction> {
            Vec::new()
        }
    }

    #[tokio::test]
    async fn test_miner_creation() {
        let mempool = Arc::new(MockMempool);
        let reward_address = vec![1, 2, 3, 4];
        let (miner, _rx) = Miner::new(4, 0x1d00ffff, mempool, reward_address);
        assert_eq!(miner.num_threads, 4);
        assert_eq!(miner.get_current_target(), 0x1d00ffff);
    }

    #[tokio::test]
    async fn test_mining_start_stop() {
        let mempool = Arc::new(MockMempool);
        let reward_address = vec![1, 2, 3, 4];
        let (miner, mut rx) = Miner::new(1, u32::MAX, mempool, reward_address);
        
        let mining_handle = tokio::spawn(async move {
            miner.start_mining(1, [0u8; 32], 0).await.unwrap();
        });

        tokio::select! {
            Some(block) = rx.recv() => {
                assert!(block.validate());
            }
            _ = tokio::time::sleep(tokio::time::Duration::from_secs(5)) => {
                miner.stop_mining();
            }
        }

        mining_handle.await.unwrap();
    }

    #[tokio::test]
    async fn test_difficulty_adjustment() {
        let mempool = Arc::new(MockMempool);
        let reward_address = vec![1, 2, 3, 4];
        let (mut miner, _rx) = Miner::new(1, 0x1d00ffff, mempool, reward_address);
        let initial_target = miner.get_current_target();

        miner.adjust_difficulty(2016, 60 * 1008, 2016);
        assert!(miner.get_current_target() < initial_target);
    }

    #[tokio::test]
    async fn test_mining_metrics() {
        let mempool = Arc::new(MockMempool);
        let reward_address = vec![1, 2, 3, 4];
        let (miner, _rx) = Miner::new(2, u32::MAX, mempool, reward_address);
        
        let metrics = miner.get_metrics();
        let initial_stats = metrics.get_stats();
        assert_eq!(initial_stats.blocks_found, 0);
        assert_eq!(initial_stats.hash_rate, 0);

        metrics.record_block();
        let updated_stats = metrics.get_stats();
        assert_eq!(updated_stats.blocks_found, 1);
    }
}