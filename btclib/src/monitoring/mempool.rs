use prometheus::{
    Registry, IntGauge, IntGaugeVec, IntCounter, IntCounterVec, 
    Gauge, GaugeVec, Histogram, HistogramVec, Opts
};
use std::sync::Arc;
use std::time::Duration;
use crate::monitoring::MetricsError;
use tracing::{info, warn, error, debug};

/// Mempool metrics collector
pub struct MempoolMetrics {
    /// Number of transactions in the mempool
    mempool_size: IntGauge,
    /// Total memory usage in bytes
    memory_usage: IntGauge,
    /// Transaction fee rates (satoshis per byte)
    fee_rates: HistogramVec,
    /// Transaction age in seconds
    transaction_age: Histogram,
    /// Transactions by type
    transactions_by_type: IntGaugeVec,
    /// Transactions by fee bucket (satoshis per byte)
    transactions_by_fee: IntGaugeVec,
    /// Transactions rejected
    transactions_rejected: IntCounterVec,
    /// Transactions expired
    transactions_expired: IntCounter,
    /// Transactions replaced (RBF)
    transactions_replaced: IntCounter,
    /// Mempool limiting events
    mempool_limiting_events: IntCounterVec,
    /// Minimum fee rate required
    minimum_fee_rate: Gauge,
    /// Conflicting transaction count
    conflicting_transactions: IntGauge,
    /// Fee rate distribution
    fee_rate_percentiles: GaugeVec,
}

impl MempoolMetrics {
    /// Create a new mempool metrics collector
    pub fn new(registry: &Registry, namespace: &str) -> Result<Self, MetricsError> {
        // Mempool size
        let mempool_size = IntGauge::new(
            Opts::new("mempool_size", "Number of transactions in the mempool")
                .namespace(namespace.to_string())
                .subsystem("mempool"),
        )?;
        registry.register(Box::new(mempool_size.clone()))?;
        
        // Memory usage
        let memory_usage = IntGauge::new(
            Opts::new("memory_usage_bytes", "Total memory usage in bytes")
                .namespace(namespace.to_string())
                .subsystem("mempool"),
        )?;
        registry.register(Box::new(memory_usage.clone()))?;
        
        // Fee rates
        let fee_rates = HistogramVec::new(
            Opts::new("fee_rates_sat_per_byte", "Transaction fee rates (satoshis per byte)")
                .namespace(namespace.to_string())
                .subsystem("mempool"),
            &["type"],
            vec![1.0, 2.0, 5.0, 10.0, 20.0, 50.0, 100.0, 200.0, 500.0, 1000.0],
        )?;
        registry.register(Box::new(fee_rates.clone()))?;
        
        // Transaction age
        let transaction_age = Histogram::new(
            Opts::new("transaction_age_seconds", "Transaction age in seconds")
                .namespace(namespace.to_string())
                .subsystem("mempool"),
            vec![60.0, 300.0, 600.0, 1800.0, 3600.0, 10800.0, 21600.0, 43200.0, 86400.0],
        )?;
        registry.register(Box::new(transaction_age.clone()))?;
        
        // Transactions by type
        let transactions_by_type = IntGaugeVec::new(
            Opts::new("transactions_by_type", "Transactions by type")
                .namespace(namespace.to_string())
                .subsystem("mempool"),
            &["type"],
        )?;
        registry.register(Box::new(transactions_by_type.clone()))?;
        
        // Transactions by fee
        let transactions_by_fee = IntGaugeVec::new(
            Opts::new("transactions_by_fee", "Transactions by fee bucket (satoshis per byte)")
                .namespace(namespace.to_string())
                .subsystem("mempool"),
            &["fee_bucket"],
        )?;
        registry.register(Box::new(transactions_by_fee.clone()))?;
        
        // Transactions rejected
        let transactions_rejected = IntCounterVec::new(
            Opts::new("transactions_rejected", "Transactions rejected")
                .namespace(namespace.to_string())
                .subsystem("mempool"),
            &["reason"],
        )?;
        registry.register(Box::new(transactions_rejected.clone()))?;
        
        // Transactions expired
        let transactions_expired = IntCounter::new(
            Opts::new("transactions_expired", "Transactions expired")
                .namespace(namespace.to_string())
                .subsystem("mempool"),
        )?;
        registry.register(Box::new(transactions_expired.clone()))?;
        
        // Transactions replaced
        let transactions_replaced = IntCounter::new(
            Opts::new("transactions_replaced", "Transactions replaced (RBF)")
                .namespace(namespace.to_string())
                .subsystem("mempool"),
        )?;
        registry.register(Box::new(transactions_replaced.clone()))?;
        
        // Mempool limiting events
        let mempool_limiting_events = IntCounterVec::new(
            Opts::new("mempool_limiting_events", "Mempool limiting events")
                .namespace(namespace.to_string())
                .subsystem("mempool"),
            &["type"],
        )?;
        registry.register(Box::new(mempool_limiting_events.clone()))?;
        
        // Minimum fee rate
        let minimum_fee_rate = Gauge::new(
            Opts::new("minimum_fee_rate", "Minimum fee rate required")
                .namespace(namespace.to_string())
                .subsystem("mempool"),
        )?;
        registry.register(Box::new(minimum_fee_rate.clone()))?;
        
        // Conflicting transactions
        let conflicting_transactions = IntGauge::new(
            Opts::new("conflicting_transactions", "Conflicting transaction count")
                .namespace(namespace.to_string())
                .subsystem("mempool"),
        )?;
        registry.register(Box::new(conflicting_transactions.clone()))?;
        
        // Fee rate percentiles
        let fee_rate_percentiles = GaugeVec::new(
            Opts::new("fee_rate_percentiles", "Fee rate distribution")
                .namespace(namespace.to_string())
                .subsystem("mempool"),
            &["percentile"],
        )?;
        registry.register(Box::new(fee_rate_percentiles.clone()))?;
        
        Ok(Self {
            mempool_size,
            memory_usage,
            fee_rates,
            transaction_age,
            transactions_by_type,
            transactions_by_fee,
            transactions_rejected,
            transactions_expired,
            transactions_replaced,
            mempool_limiting_events,
            minimum_fee_rate,
            conflicting_transactions,
            fee_rate_percentiles,
        })
    }
    
    /// Update mempool size
    pub fn update_mempool_size(&self, size: i64) {
        self.mempool_size.set(size);
    }
    
    /// Update memory usage
    pub fn update_memory_usage(&self, bytes: i64) {
        self.memory_usage.set(bytes);
    }
    
    /// Register a transaction fee rate
    pub fn register_fee_rate(&self, tx_type: &str, fee_rate: f64) {
        self.fee_rates.with_label_values(&[tx_type]).observe(fee_rate);
    }
    
    /// Update transaction age for reporting
    pub fn update_transaction_age(&self, age: Duration) {
        self.transaction_age.observe(age.as_secs_f64());
    }
    
    /// Update transactions by type
    pub fn update_transactions_by_type(&self, tx_type: &str, count: i64) {
        self.transactions_by_type.with_label_values(&[tx_type]).set(count);
    }
    
    /// Update transactions by fee bucket
    pub fn update_transactions_by_fee(&self, fee_bucket: &str, count: i64) {
        self.transactions_by_fee.with_label_values(&[fee_bucket]).set(count);
    }
    
    /// Register a rejected transaction
    pub fn register_transaction_rejected(&self, reason: &str) {
        self.transactions_rejected.with_label_values(&[reason]).inc();
    }
    
    /// Register an expired transaction
    pub fn register_transaction_expired(&self) {
        self.transactions_expired.inc();
    }
    
    /// Register a replaced transaction (RBF)
    pub fn register_transaction_replaced(&self) {
        self.transactions_replaced.inc();
    }
    
    /// Register a mempool limiting event
    pub fn register_mempool_limiting_event(&self, event_type: &str) {
        self.mempool_limiting_events.with_label_values(&[event_type]).inc();
        
        warn!("Mempool limiting event: {}", event_type);
    }
    
    /// Update minimum fee rate
    pub fn update_minimum_fee_rate(&self, fee_rate: f64) {
        self.minimum_fee_rate.set(fee_rate);
    }
    
    /// Update conflicting transactions count
    pub fn update_conflicting_transactions(&self, count: i64) {
        self.conflicting_transactions.set(count);
    }
    
    /// Update fee rate percentiles
    pub fn update_fee_rate_percentiles(&self, percentiles: &[(String, f64)]) {
        for (percentile, value) in percentiles {
            self.fee_rate_percentiles.with_label_values(&[percentile]).set(*value);
        }
    }
    
    /// Get mempool metrics as a formatted string
    pub fn get_summary(&self) -> String {
        let mut summary = String::new();
        summary.push_str("=== Mempool Metrics Summary ===\n");
        
        // Basic statistics
        summary.push_str(&format!("Mempool Size: {} transactions\n", self.mempool_size.get()));
        summary.push_str(&format!("Memory Usage: {:.2} MB\n", self.memory_usage.get() as f64 / 1_000_000.0));
        summary.push_str(&format!("Minimum Fee Rate: {:.2} sat/byte\n", self.minimum_fee_rate.get()));
        
        // Transactions by fee bucket
        summary.push_str("Fee Distribution:\n");
        
        // Fee buckets would be populated in a real implementation
        // For now we'll use preset buckets
        let fee_buckets = [
            "0-1", "1-2", "2-5", "5-10", "10-20", 
            "20-50", "50-100", "100-200", "200+"
        ];
        
        for bucket in fee_buckets {
            let count = self.transactions_by_fee.with_label_values(&[bucket]).get();
            summary.push_str(&format!("  {}: {} txs\n", bucket, count));
        }
        
        // Rejection and expiration stats
        let total_rejected: u64 = self.transactions_rejected
            .get_metric_with_label_values(&["invalid_signature"]).map(|m| m.get()).unwrap_or(0) +
            self.transactions_rejected
            .get_metric_with_label_values(&["double_spend"]).map(|m| m.get()).unwrap_or(0) +
            self.transactions_rejected
            .get_metric_with_label_values(&["fee_too_low"]).map(|m| m.get()).unwrap_or(0) +
            self.transactions_rejected
            .get_metric_with_label_values(&["other"]).map(|m| m.get()).unwrap_or(0);
        
        summary.push_str(&format!("\nRejected Transactions: {}\n", total_rejected));
        summary.push_str(&format!("Expired Transactions: {}\n", self.transactions_expired.get()));
        summary.push_str(&format!("Replaced Transactions: {}\n", self.transactions_replaced.get()));
        
        summary
    }
} 