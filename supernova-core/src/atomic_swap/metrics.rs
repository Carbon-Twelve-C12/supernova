//! Prometheus metrics for atomic swap monitoring
//!
//! Provides comprehensive metrics for monitoring atomic swap operations,
//! performance, and system health.
//!
//! The metric collectors are owned by a process-wide [`AtomicSwapMetrics`]
//! instance that is lazily initialized via [`init_metrics`]. Before init,
//! recording helpers no-op (logged at debug level) rather than panic, so
//! the node does not crash if metrics setup fails.

use prometheus::{
    Counter, CounterVec, Gauge, GaugeVec, Histogram, HistogramOpts, HistogramVec, IntCounter,
    IntCounterVec, IntGauge, IntGaugeVec, Opts, Registry,
};
use std::sync::OnceLock;
use std::time::Instant;
use tracing::debug;

use crate::monitoring::MetricsError;

/// Owned bundle of every atomic-swap Prometheus collector.
pub struct AtomicSwapMetrics {
    pub registry: Registry,

    // Counter metrics
    pub swaps_initiated: IntCounter,
    pub swaps_by_state: IntCounterVec,
    pub swaps_completed: IntCounter,
    pub swaps_failed: IntCounterVec,
    pub swaps_refunded: IntCounterVec,

    // Gauge metrics
    pub active_swaps: IntGauge,
    pub pending_btc_confirmations: IntGaugeVec,
    pub pending_nova_confirmations: IntGaugeVec,

    // Histogram metrics
    pub swap_duration: Histogram,
    pub time_to_first_confirmation: HistogramVec,
    pub bitcoin_tx_fee: Histogram,
    pub nova_tx_fee: Histogram,
    pub swap_amount_btc: Histogram,
    pub swap_amount_nova: Histogram,

    // Performance metrics
    pub rpc_latency: HistogramVec,
    pub monitor_iteration_time: Histogram,

    // Cache metrics
    pub cache_hit_rate: Gauge,
    pub cache_operations: CounterVec,

    // Error metrics
    pub errors_by_type: IntCounterVec,

    // Network metrics
    pub bitcoin_block_height: IntGauge,
    pub nova_block_height: IntGauge,
    pub websocket_connections: IntGauge,
    pub websocket_messages: IntCounterVec,
}

impl AtomicSwapMetrics {
    /// Construct every collector and register it with a fresh registry.
    /// Only fails on Prometheus-side errors (duplicate name, malformed opts),
    /// which for compile-time constants indicates programmer error.
    pub fn new() -> Result<Self, MetricsError> {
        let registry = Registry::new();

        let swaps_initiated = IntCounter::new(
            "atomic_swaps_initiated_total",
            "Total number of atomic swaps initiated",
        )?;
        let swaps_by_state = IntCounterVec::new(
            Opts::new("atomic_swaps_state_total", "Total number of swaps by state"),
            &["state"],
        )?;
        let swaps_completed = IntCounter::new(
            "atomic_swaps_completed_total",
            "Total number of successfully completed swaps",
        )?;
        let swaps_failed = IntCounterVec::new(
            Opts::new("atomic_swaps_failed_total", "Total number of failed swaps"),
            &["reason"],
        )?;
        let swaps_refunded = IntCounterVec::new(
            Opts::new(
                "atomic_swaps_refunded_total",
                "Total number of refunded swaps",
            ),
            &["chain", "reason"],
        )?;

        let active_swaps =
            IntGauge::new("atomic_swaps_active", "Number of currently active swaps")?;
        let pending_btc_confirmations = IntGaugeVec::new(
            Opts::new(
                "atomic_swaps_pending_btc_confirmations",
                "Number of pending Bitcoin confirmations",
            ),
            &["swap_id"],
        )?;
        let pending_nova_confirmations = IntGaugeVec::new(
            Opts::new(
                "atomic_swaps_pending_nova_confirmations",
                "Number of pending Supernova confirmations",
            ),
            &["swap_id"],
        )?;

        let swap_duration = Histogram::with_opts(
            HistogramOpts::new("atomic_swap_duration_seconds", "Time taken to complete a swap")
                .buckets(vec![60.0, 300.0, 600.0, 1800.0, 3600.0, 7200.0]),
        )?;
        let time_to_first_confirmation = HistogramVec::new(
            HistogramOpts::new(
                "atomic_swap_time_to_first_confirmation_seconds",
                "Time to first confirmation",
            )
            .buckets(vec![10.0, 30.0, 60.0, 120.0, 300.0, 600.0]),
            &["chain"],
        )?;
        let bitcoin_tx_fee = Histogram::with_opts(
            HistogramOpts::new(
                "atomic_swap_bitcoin_tx_fee_sats",
                "Bitcoin transaction fee in satoshis",
            )
            .buckets(vec![1000.0, 5000.0, 10000.0, 50000.0, 100000.0]),
        )?;
        let nova_tx_fee = Histogram::with_opts(
            HistogramOpts::new("atomic_swap_nova_tx_fee_units", "Supernova transaction fee")
                .buckets(vec![100.0, 500.0, 1000.0, 5000.0, 10000.0]),
        )?;
        let swap_amount_btc = Histogram::with_opts(
            HistogramOpts::new("atomic_swap_amount_btc_sats", "Bitcoin amount in swaps").buckets(
                vec![10000.0, 100000.0, 1000000.0, 10000000.0, 100000000.0],
            ),
        )?;
        let swap_amount_nova = Histogram::with_opts(
            HistogramOpts::new("atomic_swap_amount_nova_units", "Supernova amount in swaps")
                .buckets(vec![1000000.0, 10000000.0, 100000000.0, 1000000000.0]),
        )?;

        let rpc_latency = HistogramVec::new(
            HistogramOpts::new("atomic_swap_rpc_latency_seconds", "RPC method latency")
                .buckets(vec![0.001, 0.005, 0.01, 0.05, 0.1, 0.5, 1.0]),
            &["method"],
        )?;
        let monitor_iteration_time = Histogram::with_opts(
            HistogramOpts::new(
                "atomic_swap_monitor_iteration_seconds",
                "Monitor loop iteration time",
            )
            .buckets(vec![0.1, 0.5, 1.0, 2.0, 5.0]),
        )?;

        let cache_hit_rate =
            Gauge::new("atomic_swap_cache_hit_rate", "Cache hit rate (0.0 to 1.0)")?;
        let cache_operations = CounterVec::new(
            Opts::new(
                "atomic_swap_cache_operations_total",
                "Total cache operations",
            ),
            &["operation", "cache_type"],
        )?;

        let errors_by_type = IntCounterVec::new(
            Opts::new("atomic_swap_errors_total", "Total errors by type"),
            &["error_type"],
        )?;

        let bitcoin_block_height = IntGauge::new(
            "atomic_swap_bitcoin_block_height",
            "Current Bitcoin block height",
        )?;
        let nova_block_height = IntGauge::new(
            "atomic_swap_nova_block_height",
            "Current Supernova block height",
        )?;
        let websocket_connections = IntGauge::new(
            "atomic_swap_websocket_connections",
            "Number of active WebSocket connections",
        )?;
        let websocket_messages = IntCounterVec::new(
            Opts::new(
                "atomic_swap_websocket_messages_total",
                "Total WebSocket messages",
            ),
            &["direction", "message_type"],
        )?;

        registry.register(Box::new(swaps_initiated.clone()))?;
        registry.register(Box::new(swaps_by_state.clone()))?;
        registry.register(Box::new(swaps_completed.clone()))?;
        registry.register(Box::new(swaps_failed.clone()))?;
        registry.register(Box::new(swaps_refunded.clone()))?;
        registry.register(Box::new(active_swaps.clone()))?;
        registry.register(Box::new(pending_btc_confirmations.clone()))?;
        registry.register(Box::new(pending_nova_confirmations.clone()))?;
        registry.register(Box::new(swap_duration.clone()))?;
        registry.register(Box::new(time_to_first_confirmation.clone()))?;
        registry.register(Box::new(bitcoin_tx_fee.clone()))?;
        registry.register(Box::new(nova_tx_fee.clone()))?;
        registry.register(Box::new(swap_amount_btc.clone()))?;
        registry.register(Box::new(swap_amount_nova.clone()))?;
        registry.register(Box::new(rpc_latency.clone()))?;
        registry.register(Box::new(monitor_iteration_time.clone()))?;
        registry.register(Box::new(cache_hit_rate.clone()))?;
        registry.register(Box::new(cache_operations.clone()))?;
        registry.register(Box::new(errors_by_type.clone()))?;
        registry.register(Box::new(bitcoin_block_height.clone()))?;
        registry.register(Box::new(nova_block_height.clone()))?;
        registry.register(Box::new(websocket_connections.clone()))?;
        registry.register(Box::new(websocket_messages.clone()))?;

        Ok(Self {
            registry,
            swaps_initiated,
            swaps_by_state,
            swaps_completed,
            swaps_failed,
            swaps_refunded,
            active_swaps,
            pending_btc_confirmations,
            pending_nova_confirmations,
            swap_duration,
            time_to_first_confirmation,
            bitcoin_tx_fee,
            nova_tx_fee,
            swap_amount_btc,
            swap_amount_nova,
            rpc_latency,
            monitor_iteration_time,
            cache_hit_rate,
            cache_operations,
            errors_by_type,
            bitcoin_block_height,
            nova_block_height,
            websocket_connections,
            websocket_messages,
        })
    }
}

static METRICS: OnceLock<AtomicSwapMetrics> = OnceLock::new();

/// Initialize the global metrics singleton. Idempotent: a second successful
/// call is a no-op. Returns an error only if the first initialization fails.
pub fn init_metrics() -> Result<(), MetricsError> {
    if METRICS.get().is_some() {
        return Ok(());
    }
    let metrics = AtomicSwapMetrics::new()?;
    // `set` returns Err if someone else won the race; that's not an error.
    let _ = METRICS.set(metrics);
    Ok(())
}

/// Access the global metrics bundle, initializing it on first access if no
/// caller has already called [`init_metrics`].
///
/// Lazy initialization is necessary because nothing in the node startup
/// path calls `init_metrics` explicitly — the atomic-swap module is
/// feature-gated and its RPC impl could be constructed in arbitrary
/// orders. Without auto-init, every recording helper silently short-
/// circuits and the Prometheus counters stay at zero forever.
///
/// Returns `None` only if `AtomicSwapMetrics::new()` fails (which would
/// indicate a programmer error — every metric name is a compile-time
/// constant), in which case the failure is logged and recording helpers
/// remain no-ops. A subsequent call will retry.
pub fn metrics() -> Option<&'static AtomicSwapMetrics> {
    if let Some(m) = METRICS.get() {
        return Some(m);
    }
    match AtomicSwapMetrics::new() {
        Ok(m) => {
            // If another thread won the race, discard our instance and
            // return theirs. Either way `METRICS.get()` succeeds below.
            let _ = METRICS.set(m);
            METRICS.get()
        }
        Err(e) => {
            tracing::error!("atomic_swap metrics initialization failed: {}", e);
            None
        }
    }
}

/// Access the underlying registry for scraping. Returns `None` until
/// [`init_metrics`] has been called.
pub fn registry() -> Option<&'static Registry> {
    metrics().map(|m| &m.registry)
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
        if let Some(m) = metrics() {
            let duration = self.start.elapsed().as_secs_f64();
            m.rpc_latency
                .with_label_values(&[&self.method])
                .observe(duration);
        } else {
            debug!(
                "atomic_swap metrics uninitialized; dropping RpcTimer for {}",
                self.method
            );
        }
    }
}

/// Record a swap as initiated. Callers that previously read the
/// `SWAPS_INITIATED` static directly should switch to this helper.
pub fn record_swap_initiated() {
    if let Some(m) = metrics() {
        m.swaps_initiated.inc();
    }
}

/// Record a swap state transition
pub fn record_swap_state_transition(old_state: &str, new_state: &str) {
    let Some(m) = metrics() else { return };
    m.swaps_by_state.with_label_values(&[new_state]).inc();

    // Update active swaps gauge
    if new_state == "Active" || new_state == "BothFunded" {
        m.active_swaps.inc();
    } else if old_state == "Active" || old_state == "BothFunded" {
        m.active_swaps.dec();
    }
}

/// Record a swap completion
pub fn record_swap_completion(duration_secs: f64, btc_amount: u64, nova_amount: u64) {
    let Some(m) = metrics() else { return };
    m.swaps_completed.inc();
    m.swap_duration.observe(duration_secs);
    m.swap_amount_btc.observe(btc_amount as f64);
    m.swap_amount_nova.observe(nova_amount as f64);
}

/// Record a swap failure
pub fn record_swap_failure(reason: &str) {
    if let Some(m) = metrics() {
        m.swaps_failed.with_label_values(&[reason]).inc();
    }
}

/// Record a refund
pub fn record_refund(chain: &str, reason: &str) {
    if let Some(m) = metrics() {
        m.swaps_refunded.with_label_values(&[chain, reason]).inc();
    }
}

/// Record cache operation
pub fn record_cache_operation(operation: &str, cache_type: &str) {
    if let Some(m) = metrics() {
        m.cache_operations
            .with_label_values(&[operation, cache_type])
            .inc();
    }
}

/// Record an error
pub fn record_error(error_type: &str) {
    if let Some(m) = metrics() {
        m.errors_by_type.with_label_values(&[error_type]).inc();
    }
}

/// Update network heights
pub fn update_network_heights(bitcoin_height: u64, nova_height: u64) {
    if let Some(m) = metrics() {
        m.bitcoin_block_height.set(bitcoin_height as i64);
        m.nova_block_height.set(nova_height as i64);
    }
}

/// Update WebSocket metrics
pub fn update_websocket_connections(delta: i64) {
    let Some(m) = metrics() else { return };
    if delta > 0 {
        m.websocket_connections.add(delta);
    } else {
        m.websocket_connections.sub(-delta);
    }
}

pub fn record_websocket_message(direction: &str, message_type: &str) {
    if let Some(m) = metrics() {
        m.websocket_messages
            .with_label_values(&[direction, message_type])
            .inc();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metrics_construct_standalone() {
        // Construction is independent of the global singleton.
        let m = AtomicSwapMetrics::new().expect("metrics construction must succeed");
        assert!(m.registry.gather().len() >= 23);
    }

    #[test]
    fn test_init_is_idempotent() {
        // Two successful init calls must both return Ok, even though only
        // one actually populates the OnceLock.
        let first = init_metrics();
        let second = init_metrics();
        assert!(first.is_ok());
        assert!(second.is_ok());
        assert!(metrics().is_some());
    }

    #[test]
    fn test_recording_helpers_safe_pre_init() {
        // Before init, recording helpers must no-op without panic.
        // (This is best-effort: other tests in this module may have already
        // initialized the singleton via `test_init_is_idempotent`. What
        // matters is that the helpers never panic.)
        record_swap_initiated();
        record_swap_state_transition("", "Active");
        record_error("test_error");
    }
}
