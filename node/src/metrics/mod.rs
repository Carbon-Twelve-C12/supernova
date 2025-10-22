use metrics::{counter, histogram};
use metrics_exporter_prometheus::PrometheusBuilder;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

// Macro definitions moved to the top
macro_rules! register_counter {
    ($name:expr, $help:expr) => {
        metrics::register_counter!($name)
    };
}

macro_rules! register_gauge {
    ($name:expr, $help:expr) => {
        metrics::register_gauge!($name)
    };
}

macro_rules! register_histogram {
    ($name:expr, $help:expr) => {
        metrics::register_histogram!($name)
    };
}

// Backup-related metrics
#[derive(Clone)]
pub struct BackupMetrics {
    backup_duration: metrics::Histogram,
    backup_size: metrics::Histogram,
    total_backups: metrics::Counter,
    failed_backups: metrics::Counter,
    last_backup_time: metrics::Gauge,
    verification_duration: metrics::Histogram,
    failed_verifications: metrics::Counter,
    last_verification_success: metrics::Gauge,
}

impl Default for BackupMetrics {
    fn default() -> Self {
        Self::new()
    }
}

impl BackupMetrics {
    pub fn new() -> Self {
        Self {
            backup_duration: register_histogram!(
                "backup_duration_seconds",
                "Duration of backup operations"
            ),
            backup_size: register_histogram!("backup_size_bytes", "Size of backup files"),
            total_backups: register_counter!("total_backups", "Total number of backups created"),
            failed_backups: register_counter!("failed_backups", "Number of failed backup attempts"),
            last_backup_time: register_gauge!("last_backup_timestamp", "Timestamp of last backup"),
            verification_duration: register_histogram!(
                "backup_verification_duration_seconds",
                "Duration of backup verification"
            ),
            failed_verifications: register_counter!(
                "failed_verifications",
                "Number of failed backup verifications"
            ),
            last_verification_success: register_gauge!(
                "last_verification_success",
                "Success status of last verification"
            ),
        }
    }

    pub fn record_backup_start(&self) -> BackupOperation {
        BackupOperation {
            start_time: Instant::now(),
            metrics: self,
        }
    }

    pub fn record_backup_failure(&self) {
        self.failed_backups.increment(1);
    }

    pub fn record_verification_start(&self) -> VerificationOperation {
        VerificationOperation {
            start_time: Instant::now(),
            metrics: self,
        }
    }

    pub fn record_verification_failure(&self) {
        self.failed_verifications.increment(1);
        self.last_verification_success.set(0.0);
    }

    pub fn record_verification_success(&self) {
        self.last_verification_success.set(1.0);
    }

    /// Record verification
    pub fn record_verification(duration_secs: f64, success: bool) {
        histogram!("backup_verification_duration_seconds", duration_secs);

        if !success {
            counter!("backup_verification_failures_total", 1);
        }
    }
}

pub struct BackupOperation<'a> {
    start_time: Instant,
    metrics: &'a BackupMetrics,
}

impl<'a> BackupOperation<'a> {
    pub fn complete(self, size_bytes: u64) {
        let duration = self.start_time.elapsed().as_secs_f64();
        self.metrics.backup_duration.record(duration);
        self.metrics.backup_size.record(size_bytes as f64);
        self.metrics.total_backups.increment(1);
        self.metrics.last_backup_time.set(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs() as f64)
                .unwrap_or(0.0),
        );
    }
}

pub struct VerificationOperation<'a> {
    start_time: Instant,
    metrics: &'a BackupMetrics,
}

impl<'a> VerificationOperation<'a> {
    pub fn complete(self) {
        let duration = self.start_time.elapsed().as_secs_f64();
        self.metrics.verification_duration.record(duration);
    }
}

pub mod collector;
pub mod performance;
pub mod privacy;     // Metrics privacy filtering
pub mod registry;
pub mod types;

pub use collector::MetricsCollector;
pub use performance::{MetricType, PerformanceMonitor};
pub use privacy::{MetricsPrivacyFilter, MetricsPrivacyLevel, MetricsPrivacyConfig};
pub use registry::MetricsRegistry;
pub use types::{MetricValue, SystemMetrics};

/// Initialize metrics system
pub fn init_metrics() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize Prometheus exporter
    PrometheusBuilder::new().install()?;

    Ok(())
}
/// API metrics for tracking API performance
#[derive(Clone)]
pub struct ApiMetrics {
    /// Total API requests
    pub total_requests: metrics::Counter,
    /// Successful requests
    pub successful_requests: metrics::Counter,
    /// Failed requests
    pub failed_requests: metrics::Counter,
    /// Response time histogram
    pub response_time: metrics::Histogram,
    /// Active connections
    pub active_connections: metrics::Gauge,
    /// Requests per second
    pub requests_per_second: metrics::Gauge,
}

impl Default for ApiMetrics {
    fn default() -> Self {
        Self::new()
    }
}

impl ApiMetrics {
    pub fn new() -> Self {
        Self {
            total_requests: register_counter!("api_total_requests", "Total API requests"),
            successful_requests: register_counter!(
                "api_successful_requests",
                "Successful API requests"
            ),
            failed_requests: register_counter!("api_failed_requests", "Failed API requests"),
            response_time: register_histogram!("api_response_time_seconds", "API response time"),
            active_connections: register_gauge!("api_active_connections", "Active API connections"),
            requests_per_second: register_gauge!(
                "api_requests_per_second",
                "API requests per second"
            ),
        }
    }

    /// Record a successful request
    pub fn record_success(&self, response_time: Duration) {
        self.total_requests.increment(1);
        self.successful_requests.increment(1);
        self.response_time.record(response_time.as_secs_f64());
    }

    /// Record a failed request
    pub fn record_failure(&self, response_time: Duration) {
        self.total_requests.increment(1);
        self.failed_requests.increment(1);
        self.response_time.record(response_time.as_secs_f64());
    }

    /// Update active connections
    pub fn set_active_connections(&self, count: u64) {
        self.active_connections.set(count as f64);
    }

    /// Update requests per second
    pub fn update_requests_per_second(&self, rps: f64) {
        self.requests_per_second.set(rps);
    }
}

/// Thread-safe API metrics manager
pub struct ApiMetricsManager {
    metrics: Arc<Mutex<ApiMetrics>>,
    start_time: Instant,
}

impl ApiMetricsManager {
    /// Create new API metrics manager
    pub fn new() -> Self {
        Self {
            metrics: Arc::new(Mutex::new(ApiMetrics::new())),
            start_time: Instant::now(),
        }
    }

    /// Record a successful request
    pub fn record_success(&self, _endpoint: &str, response_time: Duration) {
        if let Ok(metrics) = self.metrics.lock() {
            metrics.record_success(response_time);
        }
    }

    /// Record a failed request
    pub fn record_failure(&self, _endpoint: &str, response_time: Duration) {
        if let Ok(metrics) = self.metrics.lock() {
            metrics.record_failure(response_time);
        }
    }

    /// Update active connections
    pub fn set_active_connections(&self, count: u64) {
        if let Ok(metrics) = self.metrics.lock() {
            metrics.set_active_connections(count);
        }
    }

    /// Get current metrics snapshot
    pub fn get_metrics(&self) -> ApiMetrics {
        self.metrics
            .lock()
            .map(|m| m.clone())
            .unwrap_or_else(|_| ApiMetrics::new())
    }

    /// Reset metrics
    pub fn reset(&self) {
        if let Ok(mut metrics) = self.metrics.lock() {
            *metrics = ApiMetrics::new();
        }
    }
}

impl Default for ApiMetricsManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_init_metrics() {
        let metrics = init_metrics(None).unwrap();
        assert!(metrics.is_enabled());

        let global = global_metrics().unwrap();
        assert!(global.is_enabled());
    }

    #[test]
    fn test_custom_config_metrics() {
        let mut config = MetricsConfig::default();
        config.namespace = Some("test".to_string());

        let metrics = init_metrics(Some(config)).unwrap();
        assert!(metrics.is_enabled());
    }
}
