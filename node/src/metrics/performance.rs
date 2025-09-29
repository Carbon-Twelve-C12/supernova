use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::future::Future;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};
use tokio::time;

/// Performance metric types
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum MetricType {
    /// Transaction processing time
    TransactionProcessing,
    /// Transaction validation time
    TransactionValidation,
    /// Block processing time
    BlockProcessing,
    /// Block validation time
    BlockValidation,
    /// Database read operation
    DatabaseRead,
    /// Database write operation
    DatabaseWrite,
    /// Storage operation
    StorageOperation,
    /// Mempool operations
    Mempool,
    /// Network operations
    Network,
    /// Network latency
    NetworkLatency,
    /// Peer connections
    PeerConnection,
    /// Synchronization
    Synchronization,
    /// API request processing
    ApiRequest,
    /// Lightning Network operations
    Lightning,
    /// Custom metric
    Custom(String),
}

/// Performance measurement data point
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricDataPoint {
    /// Metric type
    pub metric_type: MetricType,
    /// Value in milliseconds
    pub value_ms: f64,
    /// Timestamp when recorded
    pub timestamp: u64,
    /// Additional context information
    pub context: Option<String>,
}

/// Collection of performance metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceMetrics {
    /// Maximum number of historical points to store per metric
    pub max_history_size: usize,
    /// Metrics collection
    pub metrics: std::collections::HashMap<MetricType, VecDeque<MetricDataPoint>>,
}

impl PerformanceMetrics {
    /// Create a new performance metrics collector
    pub fn new(max_history_size: usize) -> Self {
        Self {
            max_history_size,
            metrics: std::collections::HashMap::new(),
        }
    }

    /// Add a metric data point
    pub fn add_metric(&mut self, metric_type: MetricType, value_ms: f64, context: Option<String>) {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let data_point = MetricDataPoint {
            metric_type: metric_type.clone(),
            value_ms,
            timestamp,
            context,
        };

        let metric_history = self.metrics.entry(metric_type).or_default();

        // Add new data point
        metric_history.push_back(data_point);

        // Prune if necessary
        while metric_history.len() > self.max_history_size {
            metric_history.pop_front();
        }
    }

    /// Get average for a specific metric type
    pub fn get_average(&self, metric_type: &MetricType) -> Option<f64> {
        if let Some(history) = self.metrics.get(metric_type) {
            if history.is_empty() {
                return None;
            }

            let sum: f64 = history.iter().map(|dp| dp.value_ms).sum();
            Some(sum / history.len() as f64)
        } else {
            None
        }
    }

    /// Get percentile value for a specific metric type
    pub fn get_percentile(&self, metric_type: &MetricType, percentile: f64) -> Option<f64> {
        if !(0.0..=100.0).contains(&percentile) {
            return None;
        }

        if let Some(history) = self.metrics.get(metric_type) {
            if history.is_empty() {
                return None;
            }

            let mut values: Vec<f64> = history.iter().map(|dp| dp.value_ms).collect();
            values.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

            let index = (values.len() as f64 * percentile / 100.0) as usize;
            if index >= values.len() {
                Some(values[values.len() - 1])
            } else {
                Some(values[index])
            }
        } else {
            None
        }
    }

    /// Get the latest value for a specific metric type
    pub fn get_latest(&self, metric_type: &MetricType) -> Option<f64> {
        if let Some(history) = self.metrics.get(metric_type) {
            history.back().map(|dp| dp.value_ms)
        } else {
            None
        }
    }

    /// Generate a report of all metrics
    pub fn generate_report(&self) -> serde_json::Value {
        let mut report = serde_json::Map::new();

        for (metric_type, history) in &self.metrics {
            let metric_name = match metric_type {
                MetricType::TransactionProcessing => "transaction_processing",
                MetricType::TransactionValidation => "transaction_validation",
                MetricType::BlockProcessing => "block_processing",
                MetricType::BlockValidation => "block_validation",
                MetricType::DatabaseRead => "database_read",
                MetricType::DatabaseWrite => "database_write",
                MetricType::StorageOperation => "storage_operation",
                MetricType::Mempool => "mempool",
                MetricType::Network => "network",
                MetricType::NetworkLatency => "network_latency",
                MetricType::PeerConnection => "peer_connection",
                MetricType::Synchronization => "synchronization",
                MetricType::ApiRequest => "api_request",
                MetricType::Lightning => "lightning",
                MetricType::Custom(name) => name,
            };

            let average = self.get_average(metric_type).unwrap_or(0.0);
            let p95 = self.get_percentile(metric_type, 95.0).unwrap_or(0.0);
            let p99 = self.get_percentile(metric_type, 99.0).unwrap_or(0.0);
            let latest = self.get_latest(metric_type).unwrap_or(0.0);

            let mut metric_report = serde_json::Map::new();
            metric_report.insert(
                "average_ms".to_string(),
                serde_json::Value::Number(
                    serde_json::Number::from_f64(average).unwrap_or(serde_json::Number::from(0)),
                ),
            );
            metric_report.insert(
                "p95_ms".to_string(),
                serde_json::Value::Number(
                    serde_json::Number::from_f64(p95).unwrap_or(serde_json::Number::from(0)),
                ),
            );
            metric_report.insert(
                "p99_ms".to_string(),
                serde_json::Value::Number(
                    serde_json::Number::from_f64(p99).unwrap_or(serde_json::Number::from(0)),
                ),
            );
            metric_report.insert(
                "latest_ms".to_string(),
                serde_json::Value::Number(
                    serde_json::Number::from_f64(latest).unwrap_or(serde_json::Number::from(0)),
                ),
            );
            metric_report.insert(
                "samples".to_string(),
                serde_json::Value::Number(serde_json::Number::from(history.len())),
            );

            report.insert(
                metric_name.to_string(),
                serde_json::Value::Object(metric_report),
            );
        }

        serde_json::Value::Object(report)
    }

    /// Clear all metrics data
    pub fn clear(&mut self) {
        self.metrics.clear();
    }
}

/// Thread-safe performance metrics collector
#[derive(Debug, Clone)]
pub struct PerformanceMonitor {
    /// Internal metrics storage
    metrics: Arc<RwLock<PerformanceMetrics>>,
}

impl PerformanceMonitor {
    /// Create a new performance monitor
    pub fn new(max_history_size: usize) -> Self {
        Self {
            metrics: Arc::new(RwLock::new(PerformanceMetrics::new(max_history_size))),
        }
    }

    /// Record execution time of a function and store as a metric
    pub fn record_execution_time<F, T>(
        &self,
        metric_type: MetricType,
        context: Option<String>,
        f: F,
    ) -> T
    where
        F: FnOnce() -> T,
    {
        let start = Instant::now();
        let result = f();
        let elapsed = start.elapsed();

        // Record the metric
        if let Ok(mut metrics) = self.metrics.write() {
            metrics.add_metric(
                metric_type,
                elapsed.as_secs_f64() * 1000.0, // Convert to milliseconds
                context,
            );
        }

        result
    }

    /// Record execution time of an async function and store as a metric
    pub async fn record_async_execution_time<F, T>(
        &self,
        metric_type: MetricType,
        context: Option<String>,
        f: F,
    ) -> T
    where
        F: Future<Output = T>,
    {
        let start = Instant::now();
        let result = f.await;
        let elapsed = start.elapsed();

        // Record the metric
        if let Ok(mut metrics) = self.metrics.write() {
            metrics.add_metric(
                metric_type,
                elapsed.as_secs_f64() * 1000.0, // Convert to milliseconds
                context,
            );
        }

        result
    }

    /// Get a performance report
    pub fn get_report(&self) -> serde_json::Value {
        if let Ok(metrics) = self.metrics.read() {
            metrics.generate_report()
        } else {
            serde_json::Value::Null
        }
    }

    /// Set up periodic metrics collection in the background
    pub fn start_periodic_collection(&self, interval_ms: u64) -> tokio::task::JoinHandle<()> {
        let monitor = self.clone();

        tokio::spawn(async move {
            let mut interval = time::interval(Duration::from_millis(interval_ms));

            loop {
                interval.tick().await;

                // Collect system metrics
                let memory_usage = get_memory_usage();
                let cpu_usage = get_cpu_usage();

                if let Ok(mut metrics) = monitor.metrics.write() {
                    // Record memory usage
                    metrics.add_metric(
                        MetricType::Custom("memory_usage_mb".to_string()),
                        memory_usage,
                        None,
                    );

                    // Record CPU usage
                    metrics.add_metric(
                        MetricType::Custom("cpu_usage_percent".to_string()),
                        cpu_usage,
                        None,
                    );
                }
            }
        })
    }

    /// Clear all metrics
    pub fn clear_metrics(&self) {
        if let Ok(mut metrics) = self.metrics.write() {
            metrics.clear();
        }
    }
}

/// Get current memory usage in MB
fn get_memory_usage() -> f64 {
    #[cfg(target_os = "linux")]
    {
        if let Ok(content) = std::fs::read_to_string("/proc/self/status") {
            for line in content.lines() {
                if line.starts_with("VmRSS:") {
                    if let Some(kb_str) = line.split_whitespace().nth(1) {
                        if let Ok(kb) = kb_str.parse::<f64>() {
                            return kb / 1024.0; // Convert KB to MB
                        }
                    }
                }
            }
        }
        0.0
    }

    #[cfg(not(target_os = "linux"))]
    {
        // For non-Linux platforms, return a placeholder
        // In a real implementation, use platform-specific APIs
        0.0
    }
}

/// Get current CPU usage percentage
fn get_cpu_usage() -> f64 {
    // This is a simplified implementation
    // In a real implementation, measure CPU time over an interval
    // For accurate measurement, use platform-specific APIs
    0.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metrics_recording() {
        let mut metrics = PerformanceMetrics::new(100);

        // Add some test metrics
        metrics.add_metric(MetricType::DatabaseRead, 10.0, None);
        metrics.add_metric(MetricType::DatabaseRead, 20.0, None);
        metrics.add_metric(MetricType::DatabaseRead, 30.0, None);

        // Test average
        let avg = metrics.get_average(&MetricType::DatabaseRead).unwrap();
        assert_eq!(avg, 20.0);

        // Test percentile
        let p50 = metrics
            .get_percentile(&MetricType::DatabaseRead, 50.0)
            .unwrap();
        assert_eq!(p50, 20.0);

        // Test latest
        let latest = metrics.get_latest(&MetricType::DatabaseRead).unwrap();
        assert_eq!(latest, 30.0);
    }

    #[test]
    fn test_performance_monitor() {
        let monitor = PerformanceMonitor::new(100);

        // Record execution time
        let result = monitor.record_execution_time(
            MetricType::BlockValidation,
            Some("test".to_string()),
            || {
                std::thread::sleep(Duration::from_millis(10));
                42
            },
        );

        // Check the result
        assert_eq!(result, 42);

        // Get the report
        let report = monitor.get_report();
        assert!(report.is_object());

        // Check if block_validation metric is in the report
        if let serde_json::Value::Object(obj) = &report {
            assert!(obj.contains_key("block_validation"));
        }
    }

    #[tokio::test]
    async fn test_async_performance_monitor() {
        let monitor = PerformanceMonitor::new(100);

        // Record async execution time
        let result = monitor
            .record_async_execution_time(MetricType::Network, Some("test".to_string()), async {
                tokio::time::sleep(Duration::from_millis(10)).await;
                "test"
            })
            .await;

        // Check the result
        assert_eq!(result, "test");

        // Get the report
        let report = monitor.get_report();
        assert!(report.is_object());

        // Check if network metric is in the report
        if let serde_json::Value::Object(obj) = &report {
            assert!(obj.contains_key("network"));
        }
    }
}
