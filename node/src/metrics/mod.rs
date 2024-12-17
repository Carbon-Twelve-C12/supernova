use metrics::{Counter, Gauge, Histogram, Key, KeyName, Unit};
use metrics_exporter_prometheus::PrometheusBuilder;
use once_cell::sync::Lazy;
use std::time::Instant;
use tracing::error;

/// Global metrics registry
static METRICS: Lazy<MetricsRegistry> = Lazy::new(|| {
    MetricsRegistry::new().expect("Failed to initialize metrics registry")
});

pub struct MetricsRegistry {
    _builder: metrics_exporter_prometheus::PrometheusBuilder,
}

impl MetricsRegistry {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let builder = PrometheusBuilder::new()
            .with_http_listener(([127, 0, 0, 1], 9000))
            .install()?;

        Ok(Self { _builder: builder })
    }
}

// Backup-related metrics
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

// Register metrics macros
macro_rules! register_counter {
    ($name:expr, $help:expr) => {
        metrics::counter!($name, $help)
    };
}

macro_rules! register_gauge {
    ($name:expr, $help:expr) => {
        metrics::gauge!($name, $help)
    };
}

macro_rules! register_histogram {
    ($name:expr, $help:expr) => {
        metrics::histogram!($name, $help)
    };
}