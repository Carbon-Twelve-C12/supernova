use prometheus::{
    Registry, IntGauge, IntGaugeVec, IntCounter, IntCounterVec, 
    Gauge, GaugeVec, Histogram, HistogramVec, Opts
};
use std::sync::Arc;
use std::time::Duration;
use crate::monitoring::MetricsError;
use tracing::{info, warn, error, debug};

/// Consensus metrics collector
pub struct ConsensusMetrics {
    /// Time to validate a block in milliseconds
    block_validation_time: Histogram,
    /// Time to validate a transaction in microseconds
    transaction_validation_time: Histogram,
    /// Number of forks observed
    fork_count: IntCounter,
    /// Chain reorganization depth
    reorg_depth: Histogram,
    /// Current chain work
    chain_work: Gauge,
    /// Validation operations per second
    validation_ops_per_second: Gauge,
    /// Block validation rates
    block_validation_result: IntCounterVec,
    /// Transaction validation rates
    transaction_validation_result: IntCounterVec,
    /// Verification operations count by type
    verification_ops: IntCounterVec,
    /// Chain state metrics
    chain_state: GaugeVec,
    /// Number of nodes with identical chain tip
    nodes_with_identical_tip: IntGauge,
    /// Total validation operations count
    total_validation_ops: IntCounter,
}

impl ConsensusMetrics {
    /// Create a new consensus metrics collector
    pub fn new(registry: &Registry, namespace: &str) -> Result<Self, MetricsError> {
        // Block validation time
        let block_validation_time = Histogram::new(
            Opts::new("block_validation_time_ms", "Time to validate a block in milliseconds")
                .namespace(namespace.to_string())
                .subsystem("consensus"),
            vec![1.0, 5.0, 10.0, 50.0, 100.0, 500.0, 1000.0, 5000.0, 10000.0],
        )?;
        registry.register(Box::new(block_validation_time.clone()))?;
        
        // Transaction validation time
        let transaction_validation_time = Histogram::new(
            Opts::new("transaction_validation_time_us", "Time to validate a transaction in microseconds")
                .namespace(namespace.to_string())
                .subsystem("consensus"),
            vec![10.0, 50.0, 100.0, 500.0, 1000.0, 5000.0, 10000.0, 50000.0, 100000.0],
        )?;
        registry.register(Box::new(transaction_validation_time.clone()))?;
        
        // Fork count
        let fork_count = IntCounter::new(
            Opts::new("fork_count", "Number of forks observed")
                .namespace(namespace.to_string())
                .subsystem("consensus"),
        )?;
        registry.register(Box::new(fork_count.clone()))?;
        
        // Reorganization depth
        let reorg_depth = Histogram::new(
            Opts::new("reorg_depth", "Chain reorganization depth")
                .namespace(namespace.to_string())
                .subsystem("consensus"),
            vec![1.0, 2.0, 3.0, 4.0, 5.0, 10.0, 20.0, 50.0, 100.0],
        )?;
        registry.register(Box::new(reorg_depth.clone()))?;
        
        // Chain work
        let chain_work = Gauge::new(
            Opts::new("chain_work", "Current chain work")
                .namespace(namespace.to_string())
                .subsystem("consensus"),
        )?;
        registry.register(Box::new(chain_work.clone()))?;
        
        // Validation operations per second
        let validation_ops_per_second = Gauge::new(
            Opts::new("validation_ops_per_second", "Validation operations per second")
                .namespace(namespace.to_string())
                .subsystem("consensus"),
        )?;
        registry.register(Box::new(validation_ops_per_second.clone()))?;
        
        // Block validation results
        let block_validation_result = IntCounterVec::new(
            Opts::new("block_validation_result", "Block validation results")
                .namespace(namespace.to_string())
                .subsystem("consensus"),
            &["result"],
        )?;
        registry.register(Box::new(block_validation_result.clone()))?;
        
        // Transaction validation results
        let transaction_validation_result = IntCounterVec::new(
            Opts::new("transaction_validation_result", "Transaction validation results")
                .namespace(namespace.to_string())
                .subsystem("consensus"),
            &["result"],
        )?;
        registry.register(Box::new(transaction_validation_result.clone()))?;
        
        // Verification operations
        let verification_ops = IntCounterVec::new(
            Opts::new("verification_ops", "Verification operations count by type")
                .namespace(namespace.to_string())
                .subsystem("consensus"),
            &["type"],
        )?;
        registry.register(Box::new(verification_ops.clone()))?;
        
        // Chain state metrics
        let chain_state = GaugeVec::new(
            Opts::new("chain_state", "Chain state metrics")
                .namespace(namespace.to_string())
                .subsystem("consensus"),
            &["metric"],
        )?;
        registry.register(Box::new(chain_state.clone()))?;
        
        // Nodes with identical tip
        let nodes_with_identical_tip = IntGauge::new(
            Opts::new("nodes_with_identical_tip", "Number of nodes with identical chain tip")
                .namespace(namespace.to_string())
                .subsystem("consensus"),
        )?;
        registry.register(Box::new(nodes_with_identical_tip.clone()))?;
        
        // Total validation operations
        let total_validation_ops = IntCounter::new(
            Opts::new("total_validation_ops", "Total validation operations count")
                .namespace(namespace.to_string())
                .subsystem("consensus"),
        )?;
        registry.register(Box::new(total_validation_ops.clone()))?;
        
        Ok(Self {
            block_validation_time,
            transaction_validation_time,
            fork_count,
            reorg_depth,
            chain_work,
            validation_ops_per_second,
            block_validation_result,
            transaction_validation_result,
            verification_ops,
            chain_state,
            nodes_with_identical_tip,
            total_validation_ops,
        })
    }
    
    /// Record block validation time
    pub fn record_block_validation_time(&self, validation_time: Duration) {
        self.block_validation_time.observe(validation_time.as_millis() as f64);
    }
    
    /// Record transaction validation time
    pub fn record_transaction_validation_time(&self, validation_time: Duration) {
        self.transaction_validation_time.observe(validation_time.as_micros() as f64);
    }
    
    /// Increment fork count
    pub fn increment_fork_count(&self) {
        self.fork_count.inc();
        debug!("Fork detected, total forks: {}", self.fork_count.get());
    }
    
    /// Record chain reorganization
    pub fn record_reorg(&self, depth: usize) {
        self.reorg_depth.observe(depth as f64);
        info!("Chain reorganization with depth {}", depth);
    }
    
    /// Update chain work
    pub fn update_chain_work(&self, work: f64) {
        self.chain_work.set(work);
    }
    
    /// Update validation operations per second
    pub fn update_validation_ops_per_second(&self, ops_per_second: f64) {
        self.validation_ops_per_second.set(ops_per_second);
    }
    
    /// Record block validation result
    pub fn record_block_validation_result(&self, is_valid: bool) {
        let result = if is_valid { "valid" } else { "invalid" };
        self.block_validation_result.with_label_values(&[result]).inc();
    }
    
    /// Record transaction validation result
    pub fn record_transaction_validation_result(&self, is_valid: bool) {
        let result = if is_valid { "valid" } else { "invalid" };
        self.transaction_validation_result.with_label_values(&[result]).inc();
    }
    
    /// Record verification operation
    pub fn record_verification_op(&self, op_type: &str) {
        self.verification_ops.with_label_values(&[op_type]).inc();
        self.total_validation_ops.inc();
    }
    
    /// Update chain state metric
    pub fn update_chain_state(&self, metric: &str, value: f64) {
        self.chain_state.with_label_values(&[metric]).set(value);
    }
    
    /// Update number of nodes with identical tip
    pub fn update_nodes_with_identical_tip(&self, count: i64) {
        self.nodes_with_identical_tip.set(count);
    }
    
    /// Calculate and update validation ops per second
    pub fn calculate_validation_rate(&self, window_duration: Duration, total_ops: u64) {
        if window_duration.as_secs() > 0 {
            let ops_per_second = total_ops as f64 / window_duration.as_secs_f64();
            self.update_validation_ops_per_second(ops_per_second);
        }
    }
    
    /// Get consensus metrics as a formatted string
    pub fn get_summary(&self) -> String {
        let mut summary = String::new();
        summary.push_str("=== Consensus Metrics Summary ===\n");
        
        // Basic consensus statistics
        summary.push_str(&format!("Chain Work: {:.2e}\n", self.chain_work.get()));
        summary.push_str(&format!("Validation Ops/s: {:.2f}\n", self.validation_ops_per_second.get()));
        summary.push_str(&format!("Total Validation Ops: {}\n", self.total_validation_ops.get()));
        summary.push_str(&format!("Forks Observed: {}\n", self.fork_count.get()));
        
        // Validation results
        let valid_blocks = self.block_validation_result.with_label_values(&["valid"]).get();
        let invalid_blocks = self.block_validation_result.with_label_values(&["invalid"]).get();
        let total_blocks = valid_blocks + invalid_blocks;
        let valid_percent = if total_blocks > 0 {
            (valid_blocks as f64 / total_blocks as f64) * 100.0
        } else {
            0.0
        };
        
        summary.push_str(&format!("Block Validation: {:.1}% valid ({} of {})\n", 
            valid_percent, valid_blocks, total_blocks));
        
        let valid_txs = self.transaction_validation_result.with_label_values(&["valid"]).get();
        let invalid_txs = self.transaction_validation_result.with_label_values(&["invalid"]).get();
        let total_txs = valid_txs + invalid_txs;
        let valid_tx_percent = if total_txs > 0 {
            (valid_txs as f64 / total_txs as f64) * 100.0
        } else {
            0.0
        };
        
        summary.push_str(&format!("Transaction Validation: {:.1}% valid ({} of {})\n", 
            valid_tx_percent, valid_txs, total_txs));
        
        // Verification operations breakdown would go here
        // This would be populated with actual data in a real implementation
        
        summary
    }
} 