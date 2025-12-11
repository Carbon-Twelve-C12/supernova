//! Error Metrics Collection
//!
//! ERROR CONTEXT MODULE (P1-005): Exports error counts and types to Prometheus.
//!
//! This module provides:
//! - Error counters by type and component
//! - Error rate tracking
//! - Alert threshold monitoring
//! - Error context preservation for debugging
//!
//! All errors should be recorded through this module for observability.

use once_cell::sync::Lazy;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};
use tracing::{debug, warn};

// ============================================================================
// Constants
// ============================================================================

/// Default window for error rate calculation (5 minutes)
pub const ERROR_RATE_WINDOW: Duration = Duration::from_secs(300);

/// Threshold for high error rate alert (errors per minute)
pub const HIGH_ERROR_RATE_THRESHOLD: f64 = 10.0;

// ============================================================================
// Error Types for Classification
// ============================================================================

/// Classification of error types for metrics
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ErrorType {
    /// Network-related errors
    Network,
    /// Storage/database errors
    Storage,
    /// Validation errors (blocks, transactions)
    Validation,
    /// Consensus errors
    Consensus,
    /// Mempool errors
    Mempool,
    /// API/RPC errors
    Api,
    /// Cryptographic errors
    Crypto,
    /// Configuration errors
    Config,
    /// Resource exhaustion (memory, disk, connections)
    Resource,
    /// Internal/unexpected errors
    Internal,
    /// Timeout errors
    Timeout,
}

impl ErrorType {
    /// Get string representation for metrics labels
    pub fn as_str(&self) -> &'static str {
        match self {
            ErrorType::Network => "network",
            ErrorType::Storage => "storage",
            ErrorType::Validation => "validation",
            ErrorType::Consensus => "consensus",
            ErrorType::Mempool => "mempool",
            ErrorType::Api => "api",
            ErrorType::Crypto => "crypto",
            ErrorType::Config => "config",
            ErrorType::Resource => "resource",
            ErrorType::Internal => "internal",
            ErrorType::Timeout => "timeout",
        }
    }
}

/// Component where error originated
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ErrorComponent {
    /// Block processing
    BlockProcessor,
    /// Transaction validation
    TxValidator,
    /// P2P networking
    P2PNetwork,
    /// RPC/API server
    RpcServer,
    /// Mempool management
    MempoolManager,
    /// UTXO set operations
    UtxoSet,
    /// Chain state management
    ChainState,
    /// Mining/block production
    Mining,
    /// Lightning network
    Lightning,
    /// Environmental oracle
    Environmental,
    /// General/unknown
    General,
}

impl ErrorComponent {
    /// Get string representation for metrics labels
    pub fn as_str(&self) -> &'static str {
        match self {
            ErrorComponent::BlockProcessor => "block_processor",
            ErrorComponent::TxValidator => "tx_validator",
            ErrorComponent::P2PNetwork => "p2p_network",
            ErrorComponent::RpcServer => "rpc_server",
            ErrorComponent::MempoolManager => "mempool_manager",
            ErrorComponent::UtxoSet => "utxo_set",
            ErrorComponent::ChainState => "chain_state",
            ErrorComponent::Mining => "mining",
            ErrorComponent::Lightning => "lightning",
            ErrorComponent::Environmental => "environmental",
            ErrorComponent::General => "general",
        }
    }
}

// ============================================================================
// Error Record
// ============================================================================

/// Record of a single error occurrence
#[derive(Debug, Clone)]
pub struct ErrorRecord {
    /// Error type classification
    pub error_type: ErrorType,
    /// Component where error occurred
    pub component: ErrorComponent,
    /// Error message (truncated for storage)
    pub message: String,
    /// Timestamp of occurrence
    pub timestamp: Instant,
    /// Optional context (block hash, tx id, etc.)
    pub context: Option<String>,
}

impl ErrorRecord {
    /// Create a new error record
    pub fn new(
        error_type: ErrorType,
        component: ErrorComponent,
        message: impl Into<String>,
    ) -> Self {
        let mut msg = message.into();
        // Truncate long messages to prevent memory issues
        if msg.len() > 500 {
            msg.truncate(497);
            msg.push_str("...");
        }

        Self {
            error_type,
            component,
            message: msg,
            timestamp: Instant::now(),
            context: None,
        }
    }

    /// Add context to the error record
    pub fn with_context(mut self, context: impl Into<String>) -> Self {
        let mut ctx = context.into();
        if ctx.len() > 200 {
            ctx.truncate(197);
            ctx.push_str("...");
        }
        self.context = Some(ctx);
        self
    }
}

// ============================================================================
// Error Counter
// ============================================================================

/// Thread-safe error counter for a specific (type, component) pair
#[derive(Debug)]
struct ErrorCounter {
    /// Total count
    count: AtomicU64,
    /// Count in current window
    window_count: AtomicU64,
    /// Window start time
    window_start: RwLock<Instant>,
}

impl Default for ErrorCounter {
    fn default() -> Self {
        Self::new()
    }
}

impl ErrorCounter {
    fn new() -> Self {
        Self {
            count: AtomicU64::new(0),
            window_count: AtomicU64::new(0),
            window_start: RwLock::new(Instant::now()),
        }
    }

    fn increment(&self) {
        self.count.fetch_add(1, Ordering::Relaxed);

        // Check if we need to reset the window
        let now = Instant::now();
        let should_reset = {
            let start = self.window_start.read();
            now.duration_since(*start) >= ERROR_RATE_WINDOW
        };

        if should_reset {
            let mut start = self.window_start.write();
            // Double-check after acquiring write lock
            if now.duration_since(*start) >= ERROR_RATE_WINDOW {
                *start = now;
                self.window_count.store(1, Ordering::Relaxed);
            } else {
                self.window_count.fetch_add(1, Ordering::Relaxed);
            }
        } else {
            self.window_count.fetch_add(1, Ordering::Relaxed);
        }
    }

    fn total(&self) -> u64 {
        self.count.load(Ordering::Relaxed)
    }

    fn rate_per_minute(&self) -> f64 {
        let window_count = self.window_count.load(Ordering::Relaxed);
        let elapsed = {
            let start = self.window_start.read();
            Instant::now().duration_since(*start)
        };

        if elapsed.as_secs() == 0 {
            return 0.0;
        }

        (window_count as f64 / elapsed.as_secs_f64()) * 60.0
    }
}

// ============================================================================
// Error Metrics Registry
// ============================================================================

/// Global error metrics registry
pub struct ErrorMetrics {
    /// Counters by (error_type, component)
    counters: RwLock<HashMap<(ErrorType, ErrorComponent), ErrorCounter>>,
    /// Recent errors for debugging (circular buffer)
    recent_errors: RwLock<Vec<ErrorRecord>>,
    /// Maximum recent errors to keep
    max_recent: usize,
}

impl ErrorMetrics {
    /// Create a new error metrics registry
    pub fn new() -> Self {
        Self {
            counters: RwLock::new(HashMap::new()),
            recent_errors: RwLock::new(Vec::with_capacity(100)),
            max_recent: 100,
        }
    }

    /// Record an error
    pub fn record(&self, record: ErrorRecord) {
        let key = (record.error_type, record.component);

        // Increment counter
        {
            let counters = self.counters.read();
            if let Some(counter) = counters.get(&key) {
                counter.increment();
            } else {
                drop(counters);
                let mut counters = self.counters.write();
                counters.entry(key).or_insert_with(ErrorCounter::new).increment();
            }
        }

        // Add to recent errors
        {
            let mut recent = self.recent_errors.write();
            if recent.len() >= self.max_recent {
                recent.remove(0);
            }
            recent.push(record);
        }

        // Check for high error rate and warn
        let rate = self.error_rate(key.0, key.1);
        if rate > HIGH_ERROR_RATE_THRESHOLD {
            warn!(
                error_type = %key.0.as_str(),
                component = %key.1.as_str(),
                rate = %rate,
                "High error rate detected"
            );
        }
    }

    /// Record an error with simple parameters
    pub fn record_error(
        &self,
        error_type: ErrorType,
        component: ErrorComponent,
        message: impl Into<String>,
    ) {
        self.record(ErrorRecord::new(error_type, component, message));
    }

    /// Record an error with context
    pub fn record_error_with_context(
        &self,
        error_type: ErrorType,
        component: ErrorComponent,
        message: impl Into<String>,
        context: impl Into<String>,
    ) {
        self.record(ErrorRecord::new(error_type, component, message).with_context(context));
    }

    /// Get total error count for a (type, component) pair
    pub fn total_errors(&self, error_type: ErrorType, component: ErrorComponent) -> u64 {
        let counters = self.counters.read();
        counters
            .get(&(error_type, component))
            .map(|c| c.total())
            .unwrap_or(0)
    }

    /// Get error rate (per minute) for a (type, component) pair
    pub fn error_rate(&self, error_type: ErrorType, component: ErrorComponent) -> f64 {
        let counters = self.counters.read();
        counters
            .get(&(error_type, component))
            .map(|c| c.rate_per_minute())
            .unwrap_or(0.0)
    }

    /// Get all error counts as a map for metrics export
    pub fn all_counts(&self) -> HashMap<(ErrorType, ErrorComponent), u64> {
        let counters = self.counters.read();
        counters
            .iter()
            .map(|(k, v)| (*k, v.total()))
            .collect()
    }

    /// Get recent errors for debugging
    pub fn recent_errors(&self) -> Vec<ErrorRecord> {
        self.recent_errors.read().clone()
    }

    /// Get summary statistics
    pub fn summary(&self) -> ErrorMetricsSummary {
        let counters = self.counters.read();
        
        let mut total = 0u64;
        let mut by_type: HashMap<ErrorType, u64> = HashMap::new();
        let mut by_component: HashMap<ErrorComponent, u64> = HashMap::new();
        let mut high_rate_pairs = Vec::new();

        for ((err_type, component), counter) in counters.iter() {
            let count = counter.total();
            total += count;
            *by_type.entry(*err_type).or_default() += count;
            *by_component.entry(*component).or_default() += count;

            let rate = counter.rate_per_minute();
            if rate > HIGH_ERROR_RATE_THRESHOLD {
                high_rate_pairs.push((*err_type, *component, rate));
            }
        }

        ErrorMetricsSummary {
            total_errors: total,
            errors_by_type: by_type,
            errors_by_component: by_component,
            high_rate_alerts: high_rate_pairs,
        }
    }

    /// Clear all metrics (for testing)
    #[cfg(test)]
    pub fn clear(&self) {
        self.counters.write().clear();
        self.recent_errors.write().clear();
    }
}

impl Default for ErrorMetrics {
    fn default() -> Self {
        Self::new()
    }
}

/// Summary of error metrics
#[derive(Debug, Clone)]
pub struct ErrorMetricsSummary {
    /// Total errors across all types
    pub total_errors: u64,
    /// Errors grouped by type
    pub errors_by_type: HashMap<ErrorType, u64>,
    /// Errors grouped by component
    pub errors_by_component: HashMap<ErrorComponent, u64>,
    /// (type, component, rate) pairs with high error rates
    pub high_rate_alerts: Vec<(ErrorType, ErrorComponent, f64)>,
}

// ============================================================================
// Global Instance
// ============================================================================

/// Global error metrics instance
pub static GLOBAL_ERROR_METRICS: Lazy<ErrorMetrics> = Lazy::new(ErrorMetrics::new);

/// Record an error to the global metrics
pub fn record_error(
    error_type: ErrorType,
    component: ErrorComponent,
    message: impl Into<String>,
) {
    GLOBAL_ERROR_METRICS.record_error(error_type, component, message);
}

/// Record an error with context to the global metrics
pub fn record_error_with_context(
    error_type: ErrorType,
    component: ErrorComponent,
    message: impl Into<String>,
    context: impl Into<String>,
) {
    GLOBAL_ERROR_METRICS.record_error_with_context(error_type, component, message, context);
}

/// Get global error metrics summary
pub fn error_summary() -> ErrorMetricsSummary {
    GLOBAL_ERROR_METRICS.summary()
}

// ============================================================================
// Prometheus Export
// ============================================================================

/// Export error metrics in Prometheus format
pub fn export_prometheus_metrics() -> String {
    let mut output = String::new();

    // Header
    output.push_str("# HELP supernova_errors_total Total number of errors by type and component\n");
    output.push_str("# TYPE supernova_errors_total counter\n");

    let counts = GLOBAL_ERROR_METRICS.all_counts();
    for ((error_type, component), count) in counts {
        output.push_str(&format!(
            "supernova_errors_total{{error_type=\"{}\",component=\"{}\"}} {}\n",
            error_type.as_str(),
            component.as_str(),
            count
        ));
    }

    // Error rate gauge
    output.push_str("\n# HELP supernova_error_rate_per_minute Current error rate per minute\n");
    output.push_str("# TYPE supernova_error_rate_per_minute gauge\n");

    let counters = GLOBAL_ERROR_METRICS.counters.read();
    for ((error_type, component), counter) in counters.iter() {
        let rate = counter.rate_per_minute();
        if rate > 0.0 {
            output.push_str(&format!(
                "supernova_error_rate_per_minute{{error_type=\"{}\",component=\"{}\"}} {:.2}\n",
                error_type.as_str(),
                component.as_str(),
                rate
            ));
        }
    }

    output
}

// ============================================================================
// Error Context Trait
// ============================================================================

/// Trait for errors that can provide metrics context
pub trait MetricsError {
    /// Get the error type classification
    fn error_type(&self) -> ErrorType;
    
    /// Get the component classification
    fn component(&self) -> ErrorComponent;
    
    /// Get a brief description for metrics
    fn metrics_description(&self) -> String;
}

/// Extension trait to record errors automatically
pub trait RecordableError: std::error::Error {
    /// Record this error to metrics
    fn record_to_metrics(&self, component: ErrorComponent) {
        record_error(
            ErrorType::Internal,
            component,
            self.to_string(),
        );
    }
}

// Implement for all std errors
impl<E: std::error::Error> RecordableError for E {}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_recording() {
        let metrics = ErrorMetrics::new();

        metrics.record_error(
            ErrorType::Network,
            ErrorComponent::P2PNetwork,
            "Connection failed",
        );

        assert_eq!(
            metrics.total_errors(ErrorType::Network, ErrorComponent::P2PNetwork),
            1
        );
    }

    #[test]
    fn test_error_with_context() {
        let metrics = ErrorMetrics::new();

        metrics.record_error_with_context(
            ErrorType::Validation,
            ErrorComponent::TxValidator,
            "Invalid signature",
            "tx_hash: abc123",
        );

        let recent = metrics.recent_errors();
        assert_eq!(recent.len(), 1);
        assert!(recent[0].context.is_some());
    }

    #[test]
    fn test_summary() {
        let metrics = ErrorMetrics::new();

        metrics.record_error(ErrorType::Network, ErrorComponent::P2PNetwork, "error1");
        metrics.record_error(ErrorType::Network, ErrorComponent::P2PNetwork, "error2");
        metrics.record_error(ErrorType::Storage, ErrorComponent::UtxoSet, "error3");

        let summary = metrics.summary();
        assert_eq!(summary.total_errors, 3);
        assert_eq!(summary.errors_by_type.get(&ErrorType::Network), Some(&2));
        assert_eq!(summary.errors_by_type.get(&ErrorType::Storage), Some(&1));
    }

    #[test]
    fn test_prometheus_export() {
        let _metrics = ErrorMetrics::new();
        
        // Record some errors to the global instance
        record_error(ErrorType::Api, ErrorComponent::RpcServer, "test error");

        let output = export_prometheus_metrics();
        assert!(output.contains("supernova_errors_total"));
    }

    #[test]
    fn test_message_truncation() {
        let metrics = ErrorMetrics::new();
        let long_message = "x".repeat(1000);

        metrics.record_error(
            ErrorType::Internal,
            ErrorComponent::General,
            long_message,
        );

        let recent = metrics.recent_errors();
        assert!(recent[0].message.len() <= 500);
        assert!(recent[0].message.ends_with("..."));
    }
}

