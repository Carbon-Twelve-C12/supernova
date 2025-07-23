use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use prometheus::{
    Registry, IntGauge, IntCounter, IntCounterVec, 
    Histogram, HistogramVec, Opts, HistogramOpts
};
use tokio::sync::RwLock;
use log::{info, debug, warn};
use chrono::{DateTime, Utc};

use crate::types::block::Block;
use crate::types::transaction::Transaction;
use crate::storage::chain_state::ChainState;
use crate::mempool::transaction_pool::TransactionPool;
use crate::environmental::emissions::EmissionsTracker;

/// Metrics related to blockchain health and performance
pub struct BlockchainMetrics {
    /// Registry for all metrics
    registry: Registry,
    
    /// Block height
    block_height: IntGauge,
    
    /// Total blocks processed
    blocks_processed: IntCounter,
    
    /// Block processing time histogram
    block_processing_time: Histogram,
    
    /// Blocks with quantum signatures
    blocks_with_quantum_signatures: IntCounter,
    
    /// Number of transactions by type
    transactions_by_type: IntCounterVec,
    
    /// Transaction verification time histogram
    transaction_verification_time: Histogram,
    
    /// Mempool size (number of transactions)
    mempool_size: IntGauge,
    
    /// Mempool memory usage
    mempool_memory_usage: IntGauge,
    
    /// Current network hashrate estimate
    network_hashrate: IntGauge,
    
    /// Blocks by difficulty histogram
    blocks_by_difficulty: Histogram,
    
    /// Orphaned blocks counter
    orphaned_blocks: IntCounter,
    
    /// Stale blocks counter
    stale_blocks: IntCounter,
    
    /// Chain reorganization counter
    chain_reorgs: IntCounter,
    
    /// Chain reorganization depth histogram
    reorg_depth: Histogram,
    
    /// Fork length histogram
    fork_length: Histogram,
    
    /// Block propagation time histogram
    block_propagation_time: Histogram,
    
    /// Transaction propagation time histogram
    transaction_propagation_time: Histogram,
    
    /// Carbon emissions per block
    carbon_emissions_per_block: Histogram,
    
    /// Green energy percentage
    green_energy_percentage: IntGauge,
    
    /// Invalid blocks received
    invalid_blocks: IntCounterVec,
    
    /// Verification failures by reason
    verification_failures: IntCounterVec,
    
    /// Blocks produced with renewable energy
    renewable_energy_blocks: IntCounter,
    
    /// P2P messages processed
    p2p_messages_processed: IntCounterVec,
    
    /// UTXO set size
    utxo_set_size: IntGauge,
    
    /// UTXO operations per block histogram
    utxo_operations_per_block: Histogram,
    
    /// Chain state database size
    chain_state_db_size: IntGauge,
    
    /// Last block time
    last_block_time: Arc<RwLock<DateTime<Utc>>>,
    
    /// Quantum signatures percentage
    quantum_signatures_percentage: IntGauge,
    
    /// Lightning Network channels
    lightning_channels: IntGauge,
    
    /// Block fee percentages
    fees_per_block: Histogram,
    
    /// Consensus decisions by policy
    consensus_decisions: IntCounterVec,
    
    /// Tracking data for block times
    block_times: Arc<RwLock<Vec<(DateTime<Utc>, DateTime<Utc>)>>>,
    
    /// Tracking data for transaction propagation
    transaction_first_seen: Arc<RwLock<HashMap<[u8; 32], Instant>>>,
}

impl BlockchainMetrics {
    /// Create a new blockchain metrics system
    pub fn new() -> Self {
        let registry = Registry::new();
        
        // Create block related metrics
        let block_height = IntGauge::new("supernova_block_height", "Current blockchain height").unwrap();
        let blocks_processed = IntCounter::new("supernova_blocks_processed", "Total blocks processed").unwrap();
        let block_processing_time = Histogram::with_opts(
            HistogramOpts::new(
                "supernova_block_processing_time", 
                "Time to process blocks in milliseconds"
            ).buckets(vec![10.0, 50.0, 100.0, 250.0, 500.0, 1000.0, 2500.0, 5000.0, 10000.0])
        ).unwrap();
        
        // Create transaction related metrics
        let transactions_by_type = IntCounterVec::new(
            Opts::new("supernova_transactions", "Number of transactions by type"),
            &["type"]
        ).unwrap();
        
        let transaction_verification_time = Histogram::with_opts(
            HistogramOpts::new(
                "supernova_transaction_verification_time", 
                "Time to verify transactions in milliseconds"
            ).buckets(vec![1.0, 5.0, 10.0, 25.0, 50.0, 100.0, 250.0, 500.0, 1000.0])
        ).unwrap();
        
        // Create mempool metrics
        let mempool_size = IntGauge::new("supernova_mempool_size", "Number of transactions in mempool").unwrap();
        let mempool_memory_usage = IntGauge::new("supernova_mempool_memory", "Mempool memory usage in bytes").unwrap();
        
        // Create consensus metrics
        let network_hashrate = IntGauge::new("supernova_network_hashrate", "Network hashrate in TH/s").unwrap();
        let blocks_by_difficulty = Histogram::with_opts(
            HistogramOpts::new(
                "supernova_blocks_by_difficulty", 
                "Blocks by difficulty"
            ).buckets(vec![100.0, 1000.0, 10000.0, 100000.0, 1000000.0, 10000000.0])
        ).unwrap();
        
        // Create chain metrics
        let orphaned_blocks = IntCounter::new("supernova_orphaned_blocks", "Number of orphaned blocks").unwrap();
        let stale_blocks = IntCounter::new("supernova_stale_blocks", "Number of stale blocks").unwrap();
        let chain_reorgs = IntCounter::new("supernova_chain_reorgs", "Number of chain reorganizations").unwrap();
        let reorg_depth = Histogram::with_opts(
            HistogramOpts::new(
                "supernova_reorg_depth", 
                "Depth of chain reorganizations"
            ).buckets(vec![1.0, 2.0, 3.0, 4.0, 5.0, 10.0, 20.0, 50.0])
        ).unwrap();
        
        let fork_length = Histogram::with_opts(
            HistogramOpts::new(
                "supernova_fork_length", 
                "Length of forks"
            ).buckets(vec![1.0, 2.0, 3.0, 4.0, 5.0, 10.0, 20.0, 50.0])
        ).unwrap();
        
        // Create network metrics
        let block_propagation_time = Histogram::with_opts(
            HistogramOpts::new(
                "supernova_block_propagation_time", 
                "Block propagation time in milliseconds"
            ).buckets(vec![50.0, 100.0, 250.0, 500.0, 1000.0, 2500.0, 5000.0, 10000.0])
        ).unwrap();
        
        let transaction_propagation_time = Histogram::with_opts(
            HistogramOpts::new(
                "supernova_transaction_propagation_time", 
                "Transaction propagation time in milliseconds"
            ).buckets(vec![10.0, 50.0, 100.0, 250.0, 500.0, 1000.0, 2500.0, 5000.0])
        ).unwrap();
        
        // Create environmental metrics
        let carbon_emissions_per_block = Histogram::with_opts(
            HistogramOpts::new(
                "supernova_carbon_emissions_per_block", 
                "Carbon emissions per block in kg CO2e"
            ).buckets(vec![1.0, 5.0, 10.0, 50.0, 100.0, 500.0, 1000.0, 5000.0])
        ).unwrap();
        
        let green_energy_percentage = IntGauge::new(
            "supernova_green_energy_percentage", 
            "Percentage of network energy from renewable sources"
        ).unwrap();
        
        // Create security metrics
        let invalid_blocks = IntCounterVec::new(
            Opts::new("supernova_invalid_blocks", "Number of invalid blocks by reason"),
            &["reason"]
        ).unwrap();
        
        let verification_failures = IntCounterVec::new(
            Opts::new("supernova_verification_failures", "Verification failures by reason"),
            &["reason"]
        ).unwrap();
        
        // Create quantum metrics
        let blocks_with_quantum_signatures = IntCounter::new(
            "supernova_blocks_with_quantum_signatures", 
            "Number of blocks with quantum-resistant signatures"
        ).unwrap();
        
        let quantum_signatures_percentage = IntGauge::new(
            "supernova_quantum_signatures_percentage", 
            "Percentage of transactions using quantum-resistant signatures"
        ).unwrap();
        
        // Additional metrics
        let renewable_energy_blocks = IntCounter::new(
            "supernova_renewable_energy_blocks", 
            "Number of blocks mined with renewable energy"
        ).unwrap();
        
        let p2p_messages_processed = IntCounterVec::new(
            Opts::new("supernova_p2p_messages", "P2P messages processed by type"),
            &["type"]
        ).unwrap();
        
        let utxo_set_size = IntGauge::new("supernova_utxo_set_size", "Number of UTXOs in the current set").unwrap();
        
        let utxo_operations_per_block = Histogram::with_opts(
            HistogramOpts::new(
                "supernova_utxo_operations_per_block", 
                "UTXO operations per block"
            ).buckets(vec![10.0, 50.0, 100.0, 500.0, 1000.0, 5000.0, 10000.0])
        ).unwrap();
        
        let chain_state_db_size = IntGauge::new(
            "supernova_chain_state_db_size", 
            "Chain state database size in bytes"
        ).unwrap();
        
        let lightning_channels = IntGauge::new(
            "supernova_lightning_channels", 
            "Number of Lightning Network channels"
        ).unwrap();
        
        let fees_per_block = Histogram::with_opts(
            HistogramOpts::new(
                "supernova_fees_per_block", 
                "Fees per block in satoshis"
            ).buckets(vec![
                1000.0, 5000.0, 10000.0, 50000.0, 100000.0, 500000.0, 1000000.0, 5000000.0
            ])
        ).unwrap();
        
        let consensus_decisions = IntCounterVec::new(
            Opts::new("supernova_consensus_decisions", "Consensus decisions by policy"),
            &["policy"]
        ).unwrap();
        
        // Register all metrics
        registry.register(Box::new(block_height.clone())).unwrap();
        registry.register(Box::new(blocks_processed.clone())).unwrap();
        registry.register(Box::new(block_processing_time.clone())).unwrap();
        registry.register(Box::new(transactions_by_type.clone())).unwrap();
        registry.register(Box::new(transaction_verification_time.clone())).unwrap();
        registry.register(Box::new(mempool_size.clone())).unwrap();
        registry.register(Box::new(mempool_memory_usage.clone())).unwrap();
        registry.register(Box::new(network_hashrate.clone())).unwrap();
        registry.register(Box::new(blocks_by_difficulty.clone())).unwrap();
        registry.register(Box::new(orphaned_blocks.clone())).unwrap();
        registry.register(Box::new(stale_blocks.clone())).unwrap();
        registry.register(Box::new(chain_reorgs.clone())).unwrap();
        registry.register(Box::new(reorg_depth.clone())).unwrap();
        registry.register(Box::new(fork_length.clone())).unwrap();
        registry.register(Box::new(block_propagation_time.clone())).unwrap();
        registry.register(Box::new(transaction_propagation_time.clone())).unwrap();
        registry.register(Box::new(carbon_emissions_per_block.clone())).unwrap();
        registry.register(Box::new(green_energy_percentage.clone())).unwrap();
        registry.register(Box::new(invalid_blocks.clone())).unwrap();
        registry.register(Box::new(verification_failures.clone())).unwrap();
        registry.register(Box::new(blocks_with_quantum_signatures.clone())).unwrap();
        registry.register(Box::new(quantum_signatures_percentage.clone())).unwrap();
        registry.register(Box::new(renewable_energy_blocks.clone())).unwrap();
        registry.register(Box::new(p2p_messages_processed.clone())).unwrap();
        registry.register(Box::new(utxo_set_size.clone())).unwrap();
        registry.register(Box::new(utxo_operations_per_block.clone())).unwrap();
        registry.register(Box::new(chain_state_db_size.clone())).unwrap();
        registry.register(Box::new(lightning_channels.clone())).unwrap();
        registry.register(Box::new(fees_per_block.clone())).unwrap();
        registry.register(Box::new(consensus_decisions.clone())).unwrap();
        
        Self {
            registry,
            block_height,
            blocks_processed,
            block_processing_time,
            blocks_with_quantum_signatures,
            transactions_by_type,
            transaction_verification_time,
            mempool_size,
            mempool_memory_usage,
            network_hashrate,
            blocks_by_difficulty,
            orphaned_blocks,
            stale_blocks,
            chain_reorgs,
            reorg_depth,
            fork_length,
            block_propagation_time,
            transaction_propagation_time,
            carbon_emissions_per_block,
            green_energy_percentage,
            invalid_blocks,
            verification_failures,
            renewable_energy_blocks,
            p2p_messages_processed,
            utxo_set_size,
            utxo_operations_per_block,
            chain_state_db_size,
            last_block_time: Arc::new(RwLock::new(Utc::now())),
            quantum_signatures_percentage,
            lightning_channels,
            fees_per_block,
            consensus_decisions,
            block_times: Arc::new(RwLock::new(Vec::new())),
            transaction_first_seen: Arc::new(RwLock::new(HashMap::new())),
        }
    }
    
    /// Get the Prometheus registry
    pub fn registry(&self) -> &Registry {
        &self.registry
    }
    
    /// Record a new block
    pub async fn record_block(&self, block: &Block, processing_time_ms: u64, chain_state: &ChainState, emissions_tracker: &EmissionsTracker) {
        // Update block height and count
        let height = block.header.height;
        self.block_height.set(height as i64);
        self.blocks_processed.inc();
        
        // Record processing time
        self.block_processing_time.observe(processing_time_ms as f64);
        
        // Record block timestamp
        let block_time = block.header.timestamp as i64;
        // self.block_timestamps.observe(block_time as f64); // This line was removed as per the new_code
        
        // Record block difficulty (bits field represents difficulty)
        self.blocks_by_difficulty.observe(block.header.bits as f64);
        
        // Record environmental metrics if available
        if let Ok(emissions) = emissions_tracker.calculate_block_emissions(block).await {
            // Record carbon footprint
            self.carbon_emissions_per_block.observe(emissions.tonnes_co2e * 1000.0);  // Convert to kg
            
            // Record renewable energy percentage
            if let Some(renewable_percentage) = emissions.renewable_percentage {
                self.green_energy_percentage.set(renewable_percentage as i64);
                
                // Record if block was mined with majority renewable energy
                if renewable_percentage > 50.0 {
                    self.renewable_energy_blocks.inc();
                }
            }
        }
        
        // Track transaction types
        let tx_count = block.transactions.len();
        self.transactions_by_type.with_label_values(&["total"]).inc_by(tx_count as u64);
        
        let mut quantum_tx_count = 0;
        for tx in &block.transactions {
            if tx.has_quantum_signatures() {
                quantum_tx_count += 1;
                self.transactions_by_type.with_label_values(&["quantum"]).inc();
            } else {
                self.transactions_by_type.with_label_values(&["classical"]).inc();
            }
        }
        
        // Record quantum signature metrics
        if quantum_tx_count > 0 {
            self.blocks_with_quantum_signatures.inc();
        }
        
        if tx_count > 0 {
            self.quantum_signatures_percentage.set((quantum_tx_count * 100 / tx_count) as i64);
        }
        
        // Record block times
        let now = Utc::now();
        {
            let mut last_time = self.last_block_time.write().await;
            let time_diff = now.signed_duration_since(*last_time).num_milliseconds();
            
            // Track block times for analysis
            let mut block_times = self.block_times.write().await;
            block_times.push((*last_time, now));
            // Keep only the last 1000 blocks
            if block_times.len() > 1000 {
                block_times.remove(0);
            }
            
            // Update last block time
            *last_time = now;
            
            // Calculate block interval
            // Only log if it's not the first block (to avoid huge intervals during startup)
            if time_diff > 0 && time_diff < 3600000 { // Less than 1 hour
                debug!("Block interval: {}ms", time_diff);
            }
        }
        
        // Get UTXO stats if available
        let utxo_count = chain_state.get_utxo_count();
        self.utxo_set_size.set(utxo_count as i64);
        
        // Get database size if available
        let db_size = chain_state.get_database_size();
        self.chain_state_db_size.set(db_size as i64);
        
        // Record UTXO operations
        // In a real implementation, we would count actual additions and removals
        let utxo_ops = block.transactions.iter()
            .map(|tx| tx.inputs().len() + tx.outputs().len())
            .sum::<usize>();
        
        self.utxo_operations_per_block.observe(utxo_ops as f64);
        
        // Calculate average transaction complexity
        let total_io_count: usize = block.transactions.iter()
            .map(|tx| tx.inputs().len() + tx.outputs().len())
            .sum();
        let avg_io_count = total_io_count / block.transactions.len().max(1);
        // self.transaction_io_count.observe(avg_io_count as f64); // This line was removed as per the new_code
        
        // Log summary
        info!(
            "Block {} processed: {} txs, {}ms, {:.2} kg CO2",
            height,
            block.transactions.len(), // Changed from tx_count to block.transactions.len()
            processing_time_ms,
            self.carbon_emissions_per_block.get_sample_count() as f64 / 1000.0
        );
    }
    
    /// Record a transaction verification
    pub fn record_transaction_verification(&self, tx: &Transaction, verification_time_ms: u64, result: bool) {
        // Record verification time
        self.transaction_verification_time.observe(verification_time_ms as f64);
        
        // Record result
        if !result {
            self.verification_failures.with_label_values(&["transaction"]).inc();
        }
        
        // Record quantum signature usage
        if tx.has_quantum_signatures() {
            self.transactions_by_type.with_label_values(&["quantum_verified"]).inc();
        } else {
            self.transactions_by_type.with_label_values(&["classical_verified"]).inc();
        }
    }
    
    /// Record mempool stats
    pub fn record_mempool_stats(&self, mempool: &TransactionPool) {
        // Record mempool size
        self.mempool_size.set(mempool.len() as i64);
        
        // Record memory usage (approximate)
        let memory_usage = mempool.memory_usage();
        self.mempool_memory_usage.set(memory_usage as i64);
    }
    
    /// Record network hashrate
    pub fn record_network_hashrate(&self, hashrate_th_s: u64) {
        self.network_hashrate.set(hashrate_th_s as i64);
    }
    
    /// Record a chain reorganization
    pub fn record_chain_reorg(&self, reorg_depth: u32) {
        self.chain_reorgs.inc();
        self.reorg_depth.observe(reorg_depth as f64);
        
        warn!("Chain reorganization detected with depth {}", reorg_depth);
    }
    
    /// Record a stale block
    pub fn record_stale_block(&self) {
        self.stale_blocks.inc();
    }
    
    /// Record an orphaned block
    pub fn record_orphaned_block(&self) {
        self.orphaned_blocks.inc();
    }
    
    /// Record a fork
    pub fn record_fork(&self, fork_length: u32) {
        self.fork_length.observe(fork_length as f64);
    }
    
    /// Record block propagation time
    pub fn record_block_propagation(&self, block_hash: &[u8; 32], propagation_time_ms: u64) {
        self.block_propagation_time.observe(propagation_time_ms as f64);
    }
    
    /// Record first sight of a transaction
    pub async fn record_transaction_first_seen(&self, tx_hash: &[u8; 32]) {
        let mut first_seen = self.transaction_first_seen.write().await;
        first_seen.insert(*tx_hash, Instant::now());
        
        // Cleanup old entries if the map gets too large
        if first_seen.len() > 10000 {
            // Remove entries older than 1 hour
            let one_hour_ago = Instant::now() - Duration::from_secs(3600);
            first_seen.retain(|_, time| *time > one_hour_ago);
        }
    }
    
    /// Record transaction inclusion (propagation completed)
    pub async fn record_transaction_included(&self, tx_hash: &[u8; 32]) {
        let first_seen = self.transaction_first_seen.read().await;
        
        if let Some(time) = first_seen.get(tx_hash) {
            let propagation_time = time.elapsed().as_millis() as u64;
            self.transaction_propagation_time.observe(propagation_time as f64);
            
            // Log if propagation took a long time
            if propagation_time > 5000 {
                warn!("Transaction {} took {}ms to be included", hex::encode(tx_hash), propagation_time);
            }
        }
    }
    
    /// Record an invalid block
    pub fn record_invalid_block(&self, reason: &str) {
        self.invalid_blocks.with_label_values(&[reason]).inc();
        warn!("Invalid block received: {}", reason);
    }
    
    /// Record Lightning Network stats
    pub fn record_lightning_stats(&self, active_channels: u32) {
        self.lightning_channels.set(active_channels as i64);
    }
    
    /// Record a P2P message
    pub fn record_p2p_message(&self, message_type: &str) {
        self.p2p_messages_processed.with_label_values(&[message_type]).inc();
    }
    
    /// Get average block interval (in seconds) over the last N blocks
    pub async fn get_average_block_interval(&self, block_count: usize) -> Option<f64> {
        let block_times = self.block_times.read().await;
        
        if block_times.len() < 2 {
            return None;
        }
        
        let count = block_times.len().min(block_count);
        let start_idx = block_times.len() - count;
        
        let total_millis = block_times[start_idx..].windows(2)
            .map(|w| w[1].0.signed_duration_since(w[0].0).num_milliseconds() as u64)
            .sum::<u64>();
            
        if count > 1 {
            Some(total_millis as f64 / 1000.0 / (count - 1) as f64)
        } else {
            None
        }
    }
    
    /// Get estimated time to next block (in seconds)
    pub async fn get_estimated_time_to_next_block(&self) -> Option<f64> {
        if let Some(avg_interval) = self.get_average_block_interval(10).await {
            let last_time = self.last_block_time.read().await;
            let elapsed = Utc::now().signed_duration_since(*last_time).num_seconds() as f64;
            
            if elapsed > avg_interval {
                Some(0.0)
            } else {
                Some(avg_interval - elapsed)
            }
        } else {
            None
        }
    }
    
    /// Generate a metrics report
    pub async fn generate_report(&self) -> String {
        let mut report = String::new();
        
        report.push_str(&format!("Block height: {}\n", self.block_height.get()));
        report.push_str(&format!("Blocks processed: {}\n", self.blocks_processed.get()));
        report.push_str(&format!("Mempool size: {} transactions\n", self.mempool_size.get()));
        report.push_str(&format!("Network hashrate: {} TH/s\n", self.network_hashrate.get()));
        report.push_str(&format!("UTXO set size: {}\n", self.utxo_set_size.get()));
        report.push_str(&format!("Chain reorganizations: {}\n", self.chain_reorgs.get()));
        report.push_str(&format!("Quantum signature usage: {}%\n", self.quantum_signatures_percentage.get()));
        report.push_str(&format!("Green energy percentage: {}%\n", self.green_energy_percentage.get()));
        report.push_str(&format!("Renewable energy blocks: {}\n", self.renewable_energy_blocks.get()));
        report.push_str(&format!("Lightning Network channels: {}\n", self.lightning_channels.get()));
        
        if let Some(avg_interval) = self.get_average_block_interval(144).await {
            report.push_str(&format!("Average block interval (24h): {:.2} seconds\n", avg_interval));
        }
        
        if let Some(next_block) = self.get_estimated_time_to_next_block().await {
            report.push_str(&format!("Estimated time to next block: {:.2} seconds\n", next_block));
        }
        
        report
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_block_interval_calculation() {
        // Create a new metrics instance
        let metrics = BlockchainMetrics::new();
        
        // Create block times with 10 minute intervals
        let mut time = Utc::now();
        let mut block_times = Vec::new();
        
        for _ in 0..10 {
            let prev_time = time;
            time = time + chrono::Duration::seconds(600);
            block_times.push((prev_time, time));
        }
        
        // Add to metrics
        {
            let mut metrics_times = metrics.block_times.write().await;
            *metrics_times = block_times;
        }
        
        // Test interval calculation
        let avg_interval = metrics.get_average_block_interval(10).await;
        assert!(avg_interval.is_some());
        let interval = avg_interval.unwrap();
        assert!((interval - 600.0).abs() < 0.1);
    }
    
    #[test]
    fn test_registry_contains_all_metrics() {
        let metrics = BlockchainMetrics::new();
        let registry = metrics.registry();
        
        // Check that all metrics are registered
        assert!(registry.gather().len() > 20);
    }
} 