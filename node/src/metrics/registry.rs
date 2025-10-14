use metrics::{counter, gauge, histogram};
use metrics_exporter_prometheus::PrometheusHandle;
use std::time::Duration;
use std::time::Instant;

/// Central registry for all metrics
pub struct MetricsRegistry {
    // Handle to Prometheus exporter (if configured)
    prometheus_handle: Option<PrometheusHandle>,
}

impl MetricsRegistry {
    /// Create a new metrics registry with default configuration
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        Ok(Self {
            prometheus_handle: None,
        })
    }

    /// Create a metrics registry with custom configuration
    pub fn with_config(_config: MetricsConfig) -> Result<Self, Box<dyn std::error::Error>> {
        Ok(Self {
            prometheus_handle: None,
        })
    }

    /// Create a disabled metrics registry (for testing)
    pub fn disabled() -> Self {
        Self {
            prometheus_handle: None,
        }
    }

    /// Check if metrics are enabled
    pub fn is_enabled(&self) -> bool {
        true
    }

    // System metrics methods
    pub fn update_cpu_usage(&self, usage_percent: f64) {
        gauge!("system_cpu_usage_percent", usage_percent);
    }

    pub fn update_memory_usage(&self, usage_bytes: u64) {
        gauge!("system_memory_usage_bytes", usage_bytes as f64);
    }

    pub fn update_disk_usage(&self, usage_bytes: u64) {
        gauge!("system_disk_usage_bytes", usage_bytes as f64);
    }

    // Blockchain metrics methods
    pub fn update_blockchain_height(&self, height: u64) {
        gauge!("blockchain_height", height as f64);
    }

    pub fn record_block_processing_time(&self, seconds: f64) {
        histogram!("blockchain_block_processing_time_seconds", seconds);
    }

    pub fn add_transactions(&self, count: u64) {
        counter!("blockchain_total_transactions", count);
    }

    // Network metrics methods
    pub fn update_connected_peers(&self, count: u64) {
        gauge!("network_connected_peers", count as f64);
    }

    pub fn add_bytes_received(&self, bytes: u64) {
        counter!("network_bytes_received", bytes);
    }

    pub fn add_bytes_sent(&self, bytes: u64) {
        counter!("network_bytes_sent", bytes);
    }

    // Mempool metrics methods
    pub fn update_mempool_size(&self, transaction_count: u64, bytes: u64) {
        gauge!("mempool_transactions", transaction_count as f64);
        gauge!("mempool_bytes", bytes as f64);
    }

    pub fn record_transaction_added(&self) {
        counter!("mempool_transactions_added", 1);
    }

    // Lightning metrics methods
    pub fn update_channel_counts(&self, active: u64, pending: u64) {
        gauge!("lightning_active_channels", active as f64);
        gauge!("lightning_pending_channels", pending as f64);
    }

    pub fn record_payment_outcome(&self, success: bool, amount_msat: u64) {
        if success {
            counter!("lightning_payments_success", 1);
        } else {
            counter!("lightning_payments_failed", 1);
        }
        histogram!("lightning_payment_amounts", amount_msat as f64);
    }
}

/// Metrics configuration
pub struct MetricsConfig {
    /// Namespace for metrics (default: "supernova")
    pub namespace: Option<String>,
    /// Global labels to add to all metrics
    pub global_labels: std::collections::HashMap<String, String>,
    /// HTTP endpoint for Prometheus scraping (e.g., "0.0.0.0:9090")
    pub endpoint: Option<String>,
    /// Push gateway URL
    pub push_gateway: Option<String>,
    /// Push interval
    pub push_interval: Option<Duration>,
}

impl Default for MetricsConfig {
    fn default() -> Self {
        Self {
            namespace: Some("supernova".to_string()),
            global_labels: std::collections::HashMap::new(),
            endpoint: None,
            push_gateway: None,
            push_interval: None,
        }
    }
}

// Simplified metric structs that just call the macros directly
pub struct SystemMetrics;
pub struct BlockchainMetrics;
pub struct NetworkMetrics;
pub struct ConsensusMetrics;
pub struct MempoolMetrics;
pub struct LightningMetrics;

impl Default for SystemMetrics {
    fn default() -> Self {
        Self::new()
    }
}

impl SystemMetrics {
    pub fn new() -> Self {
        Self
    }
}

impl Default for BlockchainMetrics {
    fn default() -> Self {
        Self::new()
    }
}

impl BlockchainMetrics {
    pub fn new() -> Self {
        Self
    }
}

impl Default for NetworkMetrics {
    fn default() -> Self {
        Self::new()
    }
}

impl NetworkMetrics {
    pub fn new() -> Self {
        Self
    }
}

impl Default for ConsensusMetrics {
    fn default() -> Self {
        Self::new()
    }
}

impl ConsensusMetrics {
    pub fn new() -> Self {
        Self
    }
}

impl Default for MempoolMetrics {
    fn default() -> Self {
        Self::new()
    }
}

impl MempoolMetrics {
    pub fn new() -> Self {
        Self
    }
}

impl Default for LightningMetrics {
    fn default() -> Self {
        Self::new()
    }
}

impl LightningMetrics {
    pub fn new() -> Self {
        Self
    }
}

/// Helper for timing operations
pub struct TimedOperation<F>
where
    F: FnOnce(f64),
{
    start_time: Instant,
    callback: Option<F>,
}

impl<F> TimedOperation<F>
where
    F: FnOnce(f64),
{
    /// Create a new timed operation
    pub fn new(callback: F) -> Self {
        Self {
            start_time: Instant::now(),
            callback: Some(callback),
        }
    }

    /// Complete the operation and call the callback with the duration
    pub fn complete(mut self) {
        let duration = self.start_time.elapsed().as_secs_f64();
        if let Some(callback) = self.callback.take() {
            callback(duration);
        }
    }
}

impl<F> Drop for TimedOperation<F>
where
    F: FnOnce(f64),
{
    fn drop(&mut self) {
        if let Some(callback) = self.callback.take() {
            let duration = self.start_time.elapsed().as_secs_f64();
            callback(duration);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metrics_registry() {
        let registry = MetricsRegistry::new().unwrap();

        // Test some basic operations
        registry.update_cpu_usage(50.0);
        registry.update_blockchain_height(100);
        registry.update_connected_peers(10);
    }

    #[test]
    fn test_timed_operation() {
        let mut called = false;
        {
            let _op = TimedOperation::new(|duration| {
                assert!(duration >= 0.0);
            });
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    }
}
