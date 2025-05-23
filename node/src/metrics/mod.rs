use metrics::{Counter, Gauge, Histogram};
use metrics_exporter_prometheus::PrometheusBuilder;

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
    backup_duration: Histogram,
    backup_size: Histogram,
    total_backups: Counter,
    failed_backups: Counter,
    last_backup_time: Gauge,
    verification_duration: Histogram,
    failed_verifications: Counter,
    last_verification_success: Gauge,
}

impl BackupMetrics {
    pub fn new() -> Self {
        Self {
            backup_duration: register_histogram!("backup_duration_seconds", "Duration of backup operations"),
            backup_size: register_histogram!("backup_size_bytes", "Size of backup files"),
            total_backups: register_counter!("total_backups", "Total number of backups created"),
            failed_backups: register_counter!("failed_backups", "Number of failed backup attempts"),
            last_backup_time: register_gauge!("last_backup_timestamp", "Timestamp of last backup"),
            verification_duration: register_histogram!("backup_verification_duration_seconds", "Duration of backup verification"),
            failed_verifications: register_counter!("failed_verifications", "Number of failed backup verifications"),
            last_verification_success: register_gauge!("last_verification_success", "Success status of last verification"),
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
                .unwrap()
                .as_secs() as f64,
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

pub mod registry;

pub use registry::{
    MetricsRegistry, 
    MetricsConfig,
    SystemMetrics,
    BlockchainMetrics,
    NetworkMetrics,
    ConsensusMetrics,
    MempoolMetrics,
    TimedOperation,
};

use std::sync::Arc;
use std::time::{Duration, Instant};
use tracing::info;

// Singleton metrics instance for the application
static mut GLOBAL_METRICS: Option<Arc<MetricsRegistry>> = None;

/// Initialize the global metrics registry
pub fn init_metrics(config: Option<MetricsConfig>) -> Result<Arc<MetricsRegistry>, Box<dyn std::error::Error>> {
    let metrics = match config {
        Some(cfg) => MetricsRegistry::with_config(cfg)?,
        None => MetricsRegistry::new()?,
    };
    
    let metrics_arc = Arc::new(metrics);
    
    // Store in global variable (unsafe because of static mut)
    unsafe {
        GLOBAL_METRICS = Some(Arc::clone(&metrics_arc));
    }
    
    info!("Global metrics registry initialized");
    
    Ok(metrics_arc)
}

/// Get the global metrics registry
/// Returns None if it has not been initialized
pub fn global_metrics() -> Option<Arc<MetricsRegistry>> {
    unsafe {
        GLOBAL_METRICS.as_ref().map(Arc::clone)
    }
}

/// Create a timed operation using the global metrics
pub fn timed_operation<F>(callback: F) -> Option<TimedOperation<F>> 
where 
    F: FnOnce(f64),
{
    Some(TimedOperation::new(callback))
}

// pub mod prometheus;  // Temporarily disabled - missing file
pub mod performance;

// pub use prometheus::*;  // Temporarily disabled - missing file
pub use performance::*;

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