use prometheus::{
    Registry, IntGauge, IntGaugeVec, IntCounter, IntCounterVec, 
    Gauge, GaugeVec, Histogram, HistogramVec, Opts, HistogramOpts
};
use std::sync::Arc;
use std::time::Duration;
use crate::monitoring::MetricsError;
use tracing::{info, warn, error, debug};

/// Consensus metrics collector
pub struct ConsensusMetrics {
    /// Block validation time
    block_validation_time: Histogram,
    /// Transaction validation time
    transaction_validation_time: Histogram,
    /// Fork count
    fork_count: IntCounter,
    /// Reorg depth
    reorg_depth: Histogram,
    /// Chain work
    chain_work: Gauge,
    /// Validation operations per second
    validation_ops_per_second: Gauge,
    /// Block validation results
    block_validation_result: IntCounterVec,
    /// Transaction validation results
    transaction_validation_result: IntCounterVec,
    /// Verification operations count by type
    verification_operations: IntCounterVec,
    /// Nodes with identical tip
    nodes_with_identical_tip: IntGauge,
    /// Total validation operations
    total_validation_ops: IntCounter,
}

impl ConsensusMetrics {
    /// Create a new consensus metrics collector
    pub fn new(registry: &Registry, namespace: &str) -> Result<Self, MetricsError> {
        // Block validation time
        let block_validation_time = Histogram::with_opts(
            HistogramOpts::new(
                "block_validation_time_ms",
                "Block validation time in milliseconds"
            )
            .namespace(namespace.to_string())
            .subsystem("consensus")
            .buckets(vec![1.0, 5.0, 10.0, 25.0, 50.0, 100.0, 250.0, 500.0, 1000.0, 2500.0, 5000.0])
        )?;
        registry.register(Box::new(block_validation_time.clone()))?;
        
        // Transaction validation time
        let transaction_validation_time = Histogram::with_opts(
            HistogramOpts::new(
                "transaction_validation_time_ms",
                "Transaction validation time in milliseconds"
            )
            .namespace(namespace.to_string())
            .subsystem("consensus")
            .buckets(vec![0.1, 0.5, 1.0, 2.5, 5.0, 10.0, 25.0, 50.0, 100.0, 250.0])
        )?;
        registry.register(Box::new(transaction_validation_time.clone()))?;
        
        // Fork count
        let fork_count = IntCounter::with_opts(
            Opts::new("fork_count", "Number of forks observed")
                .namespace(namespace.to_string())
                .subsystem("consensus")
        )?;
        registry.register(Box::new(fork_count.clone()))?;
        
        // Reorg depth
        let reorg_depth = Histogram::with_opts(
            HistogramOpts::new(
                "reorg_depth",
                "Reorganization depth"
            )
            .namespace(namespace.to_string())
            .subsystem("consensus")
            .buckets(vec![1.0, 2.0, 3.0, 4.0, 5.0, 10.0, 20.0, 50.0])
        )?;
        registry.register(Box::new(reorg_depth.clone()))?;
        
        // Chain work
        let chain_work = Gauge::with_opts(
            Opts::new("chain_work", "Current chain work")
                .namespace(namespace.to_string())
                .subsystem("consensus")
        )?;
        registry.register(Box::new(chain_work.clone()))?;
        
        // Validation operations per second
        let validation_ops_per_second = Gauge::with_opts(
            Opts::new("validation_ops_per_second", "Validation operations per second")
                .namespace(namespace.to_string())
                .subsystem("consensus")
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
        
        // Verification operations count by type
        let verification_operations = IntCounterVec::new(
            Opts::new("verification_operations", "Verification operations by type")
                .namespace(namespace.to_string())
                .subsystem("consensus"),
            &["type"],
        )?;
        registry.register(Box::new(verification_operations.clone()))?;
        
        // Nodes with identical tip
        let nodes_with_identical_tip = IntGauge::with_opts(
            Opts::new("nodes_with_identical_tip", "Number of nodes with identical chain tip")
                .namespace(namespace.to_string())
                .subsystem("consensus")
        )?;
        registry.register(Box::new(nodes_with_identical_tip.clone()))?;
        
        // Total validation operations
        let total_validation_ops = IntCounter::with_opts(
            Opts::new("total_validation_ops", "Total validation operations count")
                .namespace(namespace.to_string())
                .subsystem("consensus")
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
            verification_operations,
            nodes_with_identical_tip,
            total_validation_ops,
        })
    }
    
    /// Observe block validation time
    pub fn observe_block_validation_time(&self, duration_ms: f64) {
        self.block_validation_time.observe(duration_ms);
        debug!("Block validation time: {:.2}ms", duration_ms);
    }
    
    /// Observe transaction validation time
    pub fn observe_transaction_validation_time(&self, duration_ms: f64) {
        self.transaction_validation_time.observe(duration_ms);
    }
    
    /// Increment fork count
    pub fn increment_fork_count(&self) {
        self.fork_count.inc();
        info!("Fork detected. Total forks: {}", self.fork_count.get());
    }
    
    /// Observe reorganization depth
    pub fn observe_reorg_depth(&self, depth: f64) {
        self.reorg_depth.observe(depth);
        info!("Chain reorganization with depth: {}", depth);
    }
    
    /// Set chain work
    pub fn set_chain_work(&self, work: f64) {
        self.chain_work.set(work);
    }
    
    /// Set validation operations per second
    pub fn set_validation_ops_per_second(&self, ops_per_second: f64) {
        self.validation_ops_per_second.set(ops_per_second);
    }
    
    /// Record block validation result
    pub fn record_block_validation_result(&self, result: &str) {
        self.block_validation_result.with_label_values(&[result]).inc();
    }
    
    /// Record transaction validation result
    pub fn record_transaction_validation_result(&self, result: &str) {
        self.transaction_validation_result.with_label_values(&[result]).inc();
    }
    
    /// Record verification operation
    pub fn record_verification_operation(&self, operation_type: &str) {
        self.verification_operations.with_label_values(&[operation_type]).inc();
        self.total_validation_ops.inc();
    }
    
    /// Set number of nodes with identical tip
    pub fn set_nodes_with_identical_tip(&self, count: i64) {
        self.nodes_with_identical_tip.set(count);
    }
} 