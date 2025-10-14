//! Prometheus metrics for atomic swap monitoring
//!
//! Provides comprehensive metrics for monitoring atomic swap operations,
//! performance, and system health.

use lazy_static::lazy_static;
use prometheus::{
    Counter, CounterVec, Gauge, GaugeVec, Histogram, HistogramOpts, HistogramVec, IntCounter,
    IntCounterVec, IntGauge, IntGaugeVec, Opts, Registry,
};
use std::time::Instant;

lazy_static! {
    /// Global metrics registry
    pub static ref METRICS_REGISTRY: Registry = Registry::new();

    // Counter metrics

    /// Total number of swaps initiated
    pub static ref SWAPS_INITIATED: IntCounter = IntCounter::new(
        "atomic_swaps_initiated_total",
        "Total number of atomic swaps initiated"
    ).unwrap();

    /// Total number of swaps by state
    pub static ref SWAPS_BY_STATE: IntCounterVec = IntCounterVec::new(
        Opts::new("atomic_swaps_state_total", "Total number of swaps by state"),
        &["state"]
    ).unwrap();

    /// Total number of swaps completed successfully
    pub static ref SWAPS_COMPLETED: IntCounter = IntCounter::new(
        "atomic_swaps_completed_total",
        "Total number of successfully completed swaps"
    ).unwrap();

    /// Total number of swaps failed
    pub static ref SWAPS_FAILED: IntCounterVec = IntCounterVec::new(
        Opts::new("atomic_swaps_failed_total", "Total number of failed swaps"),
        &["reason"]
    ).unwrap();

    /// Total number of refunds
    pub static ref SWAPS_REFUNDED: IntCounterVec = IntCounterVec::new(
        Opts::new("atomic_swaps_refunded_total", "Total number of refunded swaps"),
        &["chain", "reason"]
    ).unwrap();

    // Gauge metrics

    /// Number of active swaps
    pub static ref ACTIVE_SWAPS: IntGauge = IntGauge::new(
        "atomic_swaps_active",
        "Number of currently active swaps"
    ).unwrap();

    /// Number of pending Bitcoin confirmations
    pub static ref PENDING_BTC_CONFIRMATIONS: IntGaugeVec = IntGaugeVec::new(
        Opts::new("atomic_swaps_pending_btc_confirmations", "Number of pending Bitcoin confirmations"),
        &["swap_id"]
    ).unwrap();

    /// Number of pending Supernova confirmations
    pub static ref PENDING_NOVA_CONFIRMATIONS: IntGaugeVec = IntGaugeVec::new(
        Opts::new("atomic_swaps_pending_nova_confirmations", "Number of pending Supernova confirmations"),
        &["swap_id"]
    ).unwrap();

    // Histogram metrics

    /// Swap completion time
    pub static ref SWAP_DURATION: Histogram = Histogram::with_opts(
        HistogramOpts::new("atomic_swap_duration_seconds", "Time taken to complete a swap")
            .buckets(vec![60.0, 300.0, 600.0, 1800.0, 3600.0, 7200.0])
    ).unwrap();

    /// Time to first confirmation
    pub static ref TIME_TO_FIRST_CONFIRMATION: HistogramVec = HistogramVec::new(
        HistogramOpts::new("atomic_swap_time_to_first_confirmation_seconds", "Time to first confirmation")
            .buckets(vec![10.0, 30.0, 60.0, 120.0, 300.0, 600.0]),
        &["chain"]
    ).unwrap();

    /// Bitcoin transaction fee
    pub static ref BITCOIN_TX_FEE: Histogram = Histogram::with_opts(
        HistogramOpts::new("atomic_swap_bitcoin_tx_fee_sats", "Bitcoin transaction fee in satoshis")
            .buckets(vec![1000.0, 5000.0, 10000.0, 50000.0, 100000.0])
    ).unwrap();

    /// Supernova transaction fee
    pub static ref NOVA_TX_FEE: Histogram = Histogram::with_opts(
        HistogramOpts::new("atomic_swap_nova_tx_fee_units", "Supernova transaction fee")
            .buckets(vec![100.0, 500.0, 1000.0, 5000.0, 10000.0])
    ).unwrap();

    /// Swap amounts
    pub static ref SWAP_AMOUNT_BTC: Histogram = Histogram::with_opts(
        HistogramOpts::new("atomic_swap_amount_btc_sats", "Bitcoin amount in swaps")
            .buckets(vec![10000.0, 100000.0, 1000000.0, 10000000.0, 100000000.0])
    ).unwrap();

    pub static ref SWAP_AMOUNT_NOVA: Histogram = Histogram::with_opts(
        HistogramOpts::new("atomic_swap_amount_nova_units", "Supernova amount in swaps")
            .buckets(vec![1000000.0, 10000000.0, 100000000.0, 1000000000.0])
    ).unwrap();

    // Performance metrics

    /// RPC method latency
    pub static ref RPC_LATENCY: HistogramVec = HistogramVec::new(
        HistogramOpts::new("atomic_swap_rpc_latency_seconds", "RPC method latency")
            .buckets(vec![0.001, 0.005, 0.01, 0.05, 0.1, 0.5, 1.0]),
        &["method"]
    ).unwrap();

    /// Monitor loop iteration time
    pub static ref MONITOR_ITERATION_TIME: Histogram = Histogram::with_opts(
        HistogramOpts::new("atomic_swap_monitor_iteration_seconds", "Monitor loop iteration time")
            .buckets(vec![0.1, 0.5, 1.0, 2.0, 5.0])
    ).unwrap();

    // Cache metrics

    /// Cache hit rate
    pub static ref CACHE_HIT_RATE: Gauge = Gauge::new(
        "atomic_swap_cache_hit_rate",
        "Cache hit rate (0.0 to 1.0)"
    ).unwrap();

    /// Cache operations
    pub static ref CACHE_OPERATIONS: CounterVec = CounterVec::new(
        Opts::new("atomic_swap_cache_operations_total", "Total cache operations"),
        &["operation", "cache_type"]
    ).unwrap();

    // Error metrics

    /// Errors by type
    pub static ref ERRORS_BY_TYPE: IntCounterVec = IntCounterVec::new(
        Opts::new("atomic_swap_errors_total", "Total errors by type"),
        &["error_type"]
    ).unwrap();

    // Network metrics

    /// Bitcoin network height
    pub static ref BITCOIN_BLOCK_HEIGHT: IntGauge = IntGauge::new(
        "atomic_swap_bitcoin_block_height",
        "Current Bitcoin block height"
    ).unwrap();

    /// Supernova network height
    pub static ref NOVA_BLOCK_HEIGHT: IntGauge = IntGauge::new(
        "atomic_swap_nova_block_height",
        "Current Supernova block height"
    ).unwrap();

    /// WebSocket connections
    pub static ref WEBSOCKET_CONNECTIONS: IntGauge = IntGauge::new(
        "atomic_swap_websocket_connections",
        "Number of active WebSocket connections"
    ).unwrap();

    /// WebSocket messages sent
    pub static ref WEBSOCKET_MESSAGES: IntCounterVec = IntCounterVec::new(
        Opts::new("atomic_swap_websocket_messages_total", "Total WebSocket messages"),
        &["direction", "message_type"]
    ).unwrap();
}

/// Initialize all metrics with the registry
pub fn init_metrics() -> Result<(), Box<dyn std::error::Error>> {
    // Register counter metrics
    METRICS_REGISTRY.register(Box::new(SWAPS_INITIATED.clone()))?;
    METRICS_REGISTRY.register(Box::new(SWAPS_BY_STATE.clone()))?;
    METRICS_REGISTRY.register(Box::new(SWAPS_COMPLETED.clone()))?;
    METRICS_REGISTRY.register(Box::new(SWAPS_FAILED.clone()))?;
    METRICS_REGISTRY.register(Box::new(SWAPS_REFUNDED.clone()))?;

    // Register gauge metrics
    METRICS_REGISTRY.register(Box::new(ACTIVE_SWAPS.clone()))?;
    METRICS_REGISTRY.register(Box::new(PENDING_BTC_CONFIRMATIONS.clone()))?;
    METRICS_REGISTRY.register(Box::new(PENDING_NOVA_CONFIRMATIONS.clone()))?;

    // Register histogram metrics
    METRICS_REGISTRY.register(Box::new(SWAP_DURATION.clone()))?;
    METRICS_REGISTRY.register(Box::new(TIME_TO_FIRST_CONFIRMATION.clone()))?;
    METRICS_REGISTRY.register(Box::new(BITCOIN_TX_FEE.clone()))?;
    METRICS_REGISTRY.register(Box::new(NOVA_TX_FEE.clone()))?;
    METRICS_REGISTRY.register(Box::new(SWAP_AMOUNT_BTC.clone()))?;
    METRICS_REGISTRY.register(Box::new(SWAP_AMOUNT_NOVA.clone()))?;

    // Register performance metrics
    METRICS_REGISTRY.register(Box::new(RPC_LATENCY.clone()))?;
    METRICS_REGISTRY.register(Box::new(MONITOR_ITERATION_TIME.clone()))?;

    // Register cache metrics
    METRICS_REGISTRY.register(Box::new(CACHE_HIT_RATE.clone()))?;
    METRICS_REGISTRY.register(Box::new(CACHE_OPERATIONS.clone()))?;

    // Register error metrics
    METRICS_REGISTRY.register(Box::new(ERRORS_BY_TYPE.clone()))?;

    // Register network metrics
    METRICS_REGISTRY.register(Box::new(BITCOIN_BLOCK_HEIGHT.clone()))?;
    METRICS_REGISTRY.register(Box::new(NOVA_BLOCK_HEIGHT.clone()))?;
    METRICS_REGISTRY.register(Box::new(WEBSOCKET_CONNECTIONS.clone()))?;
    METRICS_REGISTRY.register(Box::new(WEBSOCKET_MESSAGES.clone()))?;

    Ok(())
}

/// Timer for measuring operation duration
pub struct MetricTimer {
    start: Instant,
    metric: Histogram,
}

impl MetricTimer {
    /// Start a new timer
    pub fn start(metric: Histogram) -> Self {
        Self {
            start: Instant::now(),
            metric,
        }
    }

    /// Stop the timer and record the duration
    pub fn stop(self) {
        let duration = self.start.elapsed().as_secs_f64();
        self.metric.observe(duration);
    }
}

/// Helper to measure RPC latency
pub struct RpcTimer {
    start: Instant,
    method: String,
}

impl RpcTimer {
    pub fn start(method: &str) -> Self {
        Self {
            start: Instant::now(),
            method: method.to_string(),
        }
    }
}

impl Drop for RpcTimer {
    fn drop(&mut self) {
        let duration = self.start.elapsed().as_secs_f64();
        RPC_LATENCY
            .with_label_values(&[&self.method])
            .observe(duration);
    }
}

/// Record a swap state transition
pub fn record_swap_state_transition(old_state: &str, new_state: &str) {
    SWAPS_BY_STATE.with_label_values(&[new_state]).inc();

    // Update active swaps gauge
    if new_state == "Active" || new_state == "BothFunded" {
        ACTIVE_SWAPS.inc();
    } else if old_state == "Active" || old_state == "BothFunded" {
        ACTIVE_SWAPS.dec();
    }
}

/// Record a swap completion
pub fn record_swap_completion(duration_secs: f64, btc_amount: u64, nova_amount: u64) {
    SWAPS_COMPLETED.inc();
    SWAP_DURATION.observe(duration_secs);
    SWAP_AMOUNT_BTC.observe(btc_amount as f64);
    SWAP_AMOUNT_NOVA.observe(nova_amount as f64);
}

/// Record a swap failure
pub fn record_swap_failure(reason: &str) {
    SWAPS_FAILED.with_label_values(&[reason]).inc();
}

/// Record a refund
pub fn record_refund(chain: &str, reason: &str) {
    SWAPS_REFUNDED.with_label_values(&[chain, reason]).inc();
}

/// Record cache operation
pub fn record_cache_operation(operation: &str, cache_type: &str) {
    CACHE_OPERATIONS
        .with_label_values(&[operation, cache_type])
        .inc();
}

/// Record an error
pub fn record_error(error_type: &str) {
    ERRORS_BY_TYPE.with_label_values(&[error_type]).inc();
}

/// Update network heights
pub fn update_network_heights(bitcoin_height: u64, nova_height: u64) {
    BITCOIN_BLOCK_HEIGHT.set(bitcoin_height as i64);
    NOVA_BLOCK_HEIGHT.set(nova_height as i64);
}

/// Update WebSocket metrics
pub fn update_websocket_connections(delta: i64) {
    if delta > 0 {
        WEBSOCKET_CONNECTIONS.add(delta);
    } else {
        WEBSOCKET_CONNECTIONS.sub(-delta);
    }
}

pub fn record_websocket_message(direction: &str, message_type: &str) {
    WEBSOCKET_MESSAGES
        .with_label_values(&[direction, message_type])
        .inc();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metrics_initialization() {
        let result = init_metrics();
        // May fail if already initialized, which is ok

        // Test counter increment
        let before = SWAPS_INITIATED.get();
        SWAPS_INITIATED.inc();
        assert_eq!(SWAPS_INITIATED.get(), before + 1);
    }

    #[test]
    fn test_timer() {
        let timer = MetricTimer::start(SWAP_DURATION.clone());
        std::thread::sleep(std::time::Duration::from_millis(10));
        timer.stop();

        // Verify histogram was updated
        let metric_families = prometheus::gather();
        let swap_duration = metric_families
            .iter()
            .find(|mf| mf.get_name() == "atomic_swap_duration_seconds")
            .unwrap();

        assert!(swap_duration.get_metric().len() > 0);
    }
}
