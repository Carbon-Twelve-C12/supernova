use btclib::types::block::Block;
use super::worker::MiningWorker;
use super::template::{BlockTemplate, MempoolInterface};
use super::reward::EnvironmentalProfile;
use crate::difficulty::DifficultyAdjuster;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicU32, Ordering};
use tokio::sync::mpsc;
use tracing::{info, error};
use std::time::{Duration, Instant};
use async_trait::async_trait;
use btclib::types::transaction::Transaction;

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

    pub fn record_hash(&self, _worker_id: usize, hashes: u64) {
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

#[derive(Debug)]
pub struct MiningStats {
    pub hash_rate: u64,
    pub blocks_found: u64,
    pub active_workers: u64,
    pub uptime: Duration,
    pub last_hash: Option<Instant>,
}

#[derive(Clone)]
pub struct Miner {
    workers: Vec<Arc<MiningWorker>>,
    difficulty_adjuster: DifficultyAdjuster,
    stop_signal: Arc<AtomicBool>,
    block_sender: mpsc::Sender<Block>,
    num_threads: usize,
    mempool: Arc<dyn MempoolInterface + Send + Sync>,
    reward_address: Vec<u8>,
    metrics: Arc<MiningMetrics>,
    shared_template: Option<Arc<tokio::sync::Mutex<BlockTemplate>>>,
    template_refresh_signal: Arc<AtomicBool>,
    environmental_profile: Option<EnvironmentalProfile>,
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
        let template_refresh_signal = Arc::new(AtomicBool::new(false));

        let mut workers = Vec::with_capacity(num_threads);
        for i in 0..num_threads {
            workers.push(Arc::new(MiningWorker::new(
                Arc::clone(&stop_signal),
                tx.clone(),
                AtomicU32::new(initial_target),
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
            shared_template: None,
            template_refresh_signal,
            environmental_profile: None,
        }, rx)
    }

    pub async fn start_mining(
        &self,
        _version: u32,
        _prev_block_hash: [u8; 32],
        _current_height: u64,
    ) -> Result<(), String> {
        info!("Starting mining with {} workers at height {}", self.num_threads, _current_height);
        self.metrics.active_workers.store(self.num_threads as u64, Ordering::Relaxed);

        let template = BlockTemplate::new(
            _version,
            _prev_block_hash,
            self.difficulty_adjuster.get_current_target(),
            self.reward_address.clone(),
            self.mempool.as_ref(),
            _current_height,
            self.environmental_profile.as_ref(),
        ).await;
        let shared_template = Arc::new(tokio::sync::Mutex::new(template));
        
        let template_refresh_handle = self.start_template_refresh_task(
            Arc::clone(&shared_template),
            _version,
            _prev_block_hash,
            _current_height,
        );

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
            let template = Arc::clone(&shared_template);
            
            handles.push(tokio::spawn(async move {
                worker.mine_block_with_template(
                    _version,
                    _prev_block_hash,
                    reward_address,
                    template,
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
        template_refresh_handle.abort();
        Ok(())
    }
    
    fn start_template_refresh_task(
        &self,
        shared_template: Arc<tokio::sync::Mutex<BlockTemplate>>,
        _version: u32,
        _prev_block_hash: [u8; 32],
        _current_height: u64,
    ) -> tokio::task::JoinHandle<()> {
        let mempool = Arc::clone(&self.mempool);
        let _reward_address = self.reward_address.clone();
        let template_refresh_signal = Arc::clone(&self.template_refresh_signal);
        
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(3));
            
            loop {
                interval.tick().await;
                
                let needs_refresh = {
                    let template = shared_template.lock().await;
                    template.needs_refresh() || template_refresh_signal.load(Ordering::Relaxed)
                };
                
                if needs_refresh {
                    let mut template = shared_template.lock().await;
                    template.update_transactions(&*mempool).await;
                    template_refresh_signal.store(false, Ordering::Relaxed);
                    info!("Block template refreshed with new transactions");
                }
            }
        })
    }

    pub fn stop_mining(&self) {
        self.stop_signal.store(true, Ordering::Relaxed);
        info!("Mining stopped");
    }
    
    pub fn request_template_refresh(&self) {
        self.template_refresh_signal.store(true, Ordering::Relaxed);
    }
    
    pub fn adjust_difficulty(&mut self, height: u64, timestamp: u64, blocks_since_adjustment: u64) -> u32 {
        match self.difficulty_adjuster.adjust_difficulty(height, timestamp, blocks_since_adjustment) {
            Ok(new_target) => {
                for worker in &self.workers {
                    worker.update_target(new_target);
                }
                new_target
            },
            Err(e) => {
                error!("Failed to adjust difficulty: {}", e);
                // Return current target on error
                self.difficulty_adjuster.get_current_target()
            }
        }
    }

    // Helper method to get current target
    pub fn get_current_target(&self) -> u32 {
        self.difficulty_adjuster.get_current_target()
    }

    // Helper method to get metrics
    pub fn get_metrics(&self) -> Arc<MiningMetrics> {
        Arc::clone(&self.metrics)
    }

    /// Set the environmental profile for mining rewards
    pub fn set_environmental_profile(&mut self, profile: EnvironmentalProfile) {
        self.environmental_profile = Some(profile);
    }
}

impl MiningWorker {
    pub fn update_target(&self, new_target: u32) {
        self.target.store(new_target, Ordering::Relaxed);
    }
    
    pub async fn mine_block_with_template(
        &self,
        _version: u32,
        _prev_block_hash: [u8; 32],
        _reward_address: Vec<u8>,
        shared_template: Arc<tokio::sync::Mutex<BlockTemplate>>,
    ) -> Result<(), String> {
        let mut attempts = 0;
        let update_interval = 100_000;
        let metrics_interval = 1_000_000;
        let start_time = Instant::now();
        let mut current_nonce = self.worker_id as u32 * 1_000_000;

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

            let mut block = {
                let template = shared_template.lock().await;
                let mut block = template.create_block();
                
                for _ in 0..current_nonce {
                    block.increment_nonce();
                }
                block
            };

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
                current_nonce += 1;
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use async_trait::async_trait;
    use btclib::types::transaction::Transaction;

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
        
        // Clone miner for the spawned task
        let mining_miner = miner.clone();
        let mining_handle = tokio::spawn(async move {
            mining_miner.start_mining(1, [0u8; 32], 0).await.unwrap();
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