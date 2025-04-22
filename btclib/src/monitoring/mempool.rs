use prometheus::{
    Registry, IntGauge, IntGaugeVec, IntCounter, IntCounterVec, 
    Gauge, GaugeVec, Histogram, HistogramVec, Opts, HistogramOpts
};
use std::sync::Arc;
use std::time::Duration;
use crate::monitoring::MetricsError;
use tracing::{info, warn, error, debug};

/// Mempool metrics collector
pub struct MempoolMetrics {
    /// Number of transactions in the mempool
    mempool_size: IntGauge,
    /// Memory usage in bytes
    memory_usage: IntGauge,
    /// Fee rates
    fee_rates: HistogramVec,
    /// Transaction age
    transaction_age: Histogram,
    /// Transactions added
    transactions_added: IntCounterVec,
    /// Transactions removed
    transactions_removed: IntCounterVec,
    /// Transactions expired
    transactions_expired: IntCounter,
    /// Transactions replaced
    transactions_replaced: IntCounter,
    /// Minimum fee rate
    minimum_fee_rate: Gauge,
    /// Conflicting transactions
    conflicting_transactions: IntGauge,
}

impl MempoolMetrics {
    /// Create a new mempool metrics collector
    pub fn new(registry: &Registry, namespace: &str) -> Result<Self, MetricsError> {
        // Mempool size
        let mempool_size = IntGauge::with_opts(
            Opts::new("mempool_size", "Number of transactions in the mempool")
                .namespace(namespace.to_string())
                .subsystem("mempool")
        )?;
        registry.register(Box::new(mempool_size.clone()))?;
        
        // Memory usage
        let memory_usage = IntGauge::with_opts(
            Opts::new("memory_usage_bytes", "Total memory usage in bytes")
                .namespace(namespace.to_string())
                .subsystem("mempool")
        )?;
        registry.register(Box::new(memory_usage.clone()))?;
        
        // Fee rates
        let fee_rates = HistogramVec::new(
            HistogramOpts::new("fee_rates_sat_per_byte", "Transaction fee rates (satoshis per byte)")
                .namespace(namespace.to_string())
                .subsystem("mempool")
                .buckets(vec![1.0, 2.0, 5.0, 10.0, 20.0, 50.0, 100.0, 200.0, 500.0, 1000.0]),
            &["type"],
        )?;
        registry.register(Box::new(fee_rates.clone()))?;
        
        // Transaction age
        let transaction_age = Histogram::with_opts(
            HistogramOpts::new("transaction_age_minutes", "Transaction age in minutes")
                .namespace(namespace.to_string())
                .subsystem("mempool")
                .buckets(vec![1.0, 5.0, 10.0, 30.0, 60.0, 180.0, 360.0, 720.0, 1440.0, 2880.0])
        )?;
        registry.register(Box::new(transaction_age.clone()))?;
        
        // Transactions added
        let transactions_added = IntCounterVec::new(
            Opts::new("transactions_added", "Transactions added to mempool")
                .namespace(namespace.to_string())
                .subsystem("mempool"),
            &["source"],
        )?;
        registry.register(Box::new(transactions_added.clone()))?;
        
        // Transactions removed
        let transactions_removed = IntCounterVec::new(
            Opts::new("transactions_removed", "Transactions removed from mempool")
                .namespace(namespace.to_string())
                .subsystem("mempool"),
            &["reason"],
        )?;
        registry.register(Box::new(transactions_removed.clone()))?;
        
        // Transactions expired
        let transactions_expired = IntCounter::with_opts(
            Opts::new("transactions_expired", "Transactions expired")
                .namespace(namespace.to_string())
                .subsystem("mempool")
        )?;
        registry.register(Box::new(transactions_expired.clone()))?;
        
        // Transactions replaced
        let transactions_replaced = IntCounter::with_opts(
            Opts::new("transactions_replaced", "Transactions replaced (RBF)")
                .namespace(namespace.to_string())
                .subsystem("mempool")
        )?;
        registry.register(Box::new(transactions_replaced.clone()))?;
        
        // Minimum fee rate
        let minimum_fee_rate = Gauge::with_opts(
            Opts::new("minimum_fee_rate", "Minimum fee rate required")
                .namespace(namespace.to_string())
                .subsystem("mempool")
        )?;
        registry.register(Box::new(minimum_fee_rate.clone()))?;
        
        // Conflicting transactions
        let conflicting_transactions = IntGauge::with_opts(
            Opts::new("conflicting_transactions", "Conflicting transaction count")
                .namespace(namespace.to_string())
                .subsystem("mempool")
        )?;
        registry.register(Box::new(conflicting_transactions.clone()))?;
        
        Ok(Self {
            mempool_size,
            memory_usage,
            fee_rates,
            transaction_age,
            transactions_added,
            transactions_removed,
            transactions_expired,
            transactions_replaced,
            minimum_fee_rate,
            conflicting_transactions,
        })
    }
    
    /// Get the current mempool size
    pub fn observe_mempool_size(&self, size: i64) {
        self.mempool_size.set(size);
    }
    
    /// Set the current memory usage
    pub fn observe_memory_usage(&self, bytes: i64) {
        self.memory_usage.set(bytes);
    }
    
    /// Observe a transaction fee rate
    pub fn observe_fee_rate(&self, fee_rate: f64, tx_type: &str) {
        self.fee_rates.with_label_values(&[tx_type]).observe(fee_rate);
    }
    
    /// Observe a transaction's age
    pub fn observe_transaction_age(&self, age_minutes: f64) {
        self.transaction_age.observe(age_minutes);
    }
    
    /// Increment transactions added counter
    pub fn increment_transactions_added(&self, source: &str) {
        self.transactions_added.with_label_values(&[source]).inc();
    }
    
    /// Increment transactions removed counter
    pub fn increment_transactions_removed(&self, reason: &str) {
        self.transactions_removed.with_label_values(&[reason]).inc();
    }
    
    /// Increment transactions expired counter
    pub fn increment_transactions_expired(&self) {
        self.transactions_expired.inc();
    }
    
    /// Increment transactions replaced counter
    pub fn increment_transactions_replaced(&self) {
        self.transactions_replaced.inc();
    }
    
    /// Set the minimum fee rate
    pub fn set_minimum_fee_rate(&self, fee_rate: f64) {
        self.minimum_fee_rate.set(fee_rate);
    }
    
    /// Set the number of conflicting transactions
    pub fn set_conflicting_transactions(&self, count: i64) {
        self.conflicting_transactions.set(count);
    }
} 