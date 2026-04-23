//! Metrics collection and reporting utilities

use prometheus::{Gauge, Histogram, HistogramOpts, IntCounter, IntGauge, Registry};

use crate::monitoring::MetricsError;

/// Default histogram buckets for timing operations
pub const DEFAULT_BUCKETS: &[f64] = &[
    0.001, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0,
];

/// Core supernova metric collectors.
///
/// Construction is fallible because Prometheus rejects duplicate or
/// malformed metric definitions; for the compile-time constant names used
/// here, a runtime failure can only indicate programmer error.
pub struct Metrics {
    pub registry: Registry,

    // Blockchain metrics
    pub block_height: IntGauge,
    pub block_time: Histogram,
    pub blocks_mined: IntCounter,

    // Network metrics
    pub connected_peers: IntGauge,
    pub received_bytes: IntCounter,
    pub sent_bytes: IntCounter,

    // Mempool metrics
    pub mempool_size: IntGauge,
    pub mempool_bytes: IntGauge,

    // Environmental metrics
    pub energy_consumption: Gauge,
    pub carbon_emissions: Gauge,
    pub renewable_percentage: Gauge,
}

impl Metrics {
    /// Construct and register every collector. Returns
    /// [`MetricsError::Prometheus`] if the registry rejects a metric
    /// (duplicate name, malformed options, etc.).
    pub fn new() -> Result<Self, MetricsError> {
        let registry = Registry::new();

        let block_height =
            IntGauge::new("supernova_block_height", "Current blockchain height")?;
        let block_time = Histogram::with_opts(
            HistogramOpts::new(
                "supernova_block_time_seconds",
                "Time between blocks in seconds",
            )
            .buckets(vec![1.0, 5.0, 10.0, 30.0, 60.0, 120.0, 300.0, 600.0]),
        )?;
        let blocks_mined = IntCounter::new(
            "supernova_blocks_mined",
            "Total number of blocks mined by this node",
        )?;

        let connected_peers =
            IntGauge::new("supernova_connected_peers", "Number of connected peers")?;
        let received_bytes =
            IntCounter::new("supernova_received_bytes", "Total bytes received from peers")?;
        let sent_bytes =
            IntCounter::new("supernova_sent_bytes", "Total bytes sent to peers")?;

        let mempool_size = IntGauge::new(
            "supernova_mempool_size",
            "Number of transactions in the mempool",
        )?;
        let mempool_bytes =
            IntGauge::new("supernova_mempool_bytes", "Size of the mempool in bytes")?;

        let energy_consumption = Gauge::new(
            "supernova_energy_consumption_kwh",
            "Estimated energy consumption in kWh",
        )?;
        let carbon_emissions = Gauge::new(
            "supernova_carbon_emissions_tons",
            "Estimated carbon emissions in tons of CO2e",
        )?;
        let renewable_percentage = Gauge::new(
            "supernova_renewable_percentage",
            "Percentage of energy from renewable sources",
        )?;

        registry.register(Box::new(block_height.clone()))?;
        registry.register(Box::new(block_time.clone()))?;
        registry.register(Box::new(blocks_mined.clone()))?;
        registry.register(Box::new(connected_peers.clone()))?;
        registry.register(Box::new(received_bytes.clone()))?;
        registry.register(Box::new(sent_bytes.clone()))?;
        registry.register(Box::new(mempool_size.clone()))?;
        registry.register(Box::new(mempool_bytes.clone()))?;
        registry.register(Box::new(energy_consumption.clone()))?;
        registry.register(Box::new(carbon_emissions.clone()))?;
        registry.register(Box::new(renewable_percentage.clone()))?;

        Ok(Self {
            registry,
            block_height,
            block_time,
            blocks_mined,
            connected_peers,
            received_bytes,
            sent_bytes,
            mempool_size,
            mempool_bytes,
            energy_consumption,
            carbon_emissions,
            renewable_percentage,
        })
    }
}

/// Start a timer for measuring durations
pub struct Timer {
    start: std::time::Instant,
    histogram: Histogram,
}

impl Timer {
    /// Create a new timer for the given histogram
    pub fn new(histogram: Histogram) -> Self {
        Self {
            start: std::time::Instant::now(),
            histogram,
        }
    }

    /// Observe the elapsed time
    pub fn observe(&self) {
        let elapsed = self.start.elapsed().as_secs_f64();
        self.histogram.observe(elapsed);
    }
}

impl Drop for Timer {
    fn drop(&mut self) {
        self.observe();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metrics_construct_and_register() {
        let metrics = Metrics::new().expect("metrics construction must succeed");
        // Every collector registered above should appear in the gather output.
        let gathered = metrics.registry.gather();
        assert!(gathered.len() >= 11);
    }
}
