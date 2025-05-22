//! Metrics collection and reporting utilities

use prometheus::{
    Registry, IntGauge, IntGaugeVec, IntCounter, IntCounterVec, 
    Gauge, GaugeVec, Histogram, HistogramVec, Opts, HistogramOpts
};
use lazy_static::lazy_static;
use std::sync::Arc;

/// Default histogram buckets for timing operations
pub const DEFAULT_BUCKETS: &[f64] = &[
    0.001, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0
];

lazy_static! {
    pub static ref REGISTRY: Registry = Registry::new();
    
    // Blockchain metrics
    pub static ref BLOCK_HEIGHT: IntGauge = IntGauge::new(
        "supernova_block_height", "Current blockchain height"
    ).expect("Failed to create block height metric");
    
    pub static ref BLOCK_TIME: Histogram = Histogram::with_opts(
        HistogramOpts::new(
            "supernova_block_time_seconds", 
            "Time between blocks in seconds"
        ).buckets(vec![1.0, 5.0, 10.0, 30.0, 60.0, 120.0, 300.0, 600.0])
    ).expect("Failed to create block time metric");
    
    pub static ref BLOCKS_MINED: IntCounter = IntCounter::new(
        "supernova_blocks_mined", 
        "Total number of blocks mined by this node"
    ).expect("Failed to create blocks mined metric");
    
    // Network metrics
    pub static ref CONNECTED_PEERS: IntGauge = IntGauge::new(
        "supernova_connected_peers",
        "Number of connected peers"
    ).expect("Failed to create connected peers metric");
    
    pub static ref RECEIVED_BYTES: IntCounter = IntCounter::new(
        "supernova_received_bytes",
        "Total bytes received from peers"
    ).expect("Failed to create received bytes metric");
    
    pub static ref SENT_BYTES: IntCounter = IntCounter::new(
        "supernova_sent_bytes",
        "Total bytes sent to peers"
    ).expect("Failed to create sent bytes metric");
    
    // Mempool metrics
    pub static ref MEMPOOL_SIZE: IntGauge = IntGauge::new(
        "supernova_mempool_size",
        "Number of transactions in the mempool"
    ).expect("Failed to create mempool size metric");
    
    pub static ref MEMPOOL_BYTES: IntGauge = IntGauge::new(
        "supernova_mempool_bytes",
        "Size of the mempool in bytes"
    ).expect("Failed to create mempool bytes metric");
    
    // Environmental metrics
    pub static ref ENERGY_CONSUMPTION: Gauge = Gauge::new(
        "supernova_energy_consumption_kwh",
        "Estimated energy consumption in kWh"
    ).expect("Failed to create energy consumption metric");
    
    pub static ref CARBON_EMISSIONS: Gauge = Gauge::new(
        "supernova_carbon_emissions_tons",
        "Estimated carbon emissions in tons of CO2e"
    ).expect("Failed to create carbon emissions metric");
    
    pub static ref RENEWABLE_PERCENTAGE: Gauge = Gauge::new(
        "supernova_renewable_percentage",
        "Percentage of energy from renewable sources"
    ).expect("Failed to create renewable percentage metric");
}

/// Initialize all metrics
pub fn init_metrics() {
    // Register all metrics with the registry
    REGISTRY.register(Box::new(BLOCK_HEIGHT.clone())).unwrap();
    REGISTRY.register(Box::new(BLOCK_TIME.clone())).unwrap();
    REGISTRY.register(Box::new(BLOCKS_MINED.clone())).unwrap();
    REGISTRY.register(Box::new(CONNECTED_PEERS.clone())).unwrap();
    REGISTRY.register(Box::new(RECEIVED_BYTES.clone())).unwrap();
    REGISTRY.register(Box::new(SENT_BYTES.clone())).unwrap();
    REGISTRY.register(Box::new(MEMPOOL_SIZE.clone())).unwrap();
    REGISTRY.register(Box::new(MEMPOOL_BYTES.clone())).unwrap();
    REGISTRY.register(Box::new(ENERGY_CONSUMPTION.clone())).unwrap();
    REGISTRY.register(Box::new(CARBON_EMISSIONS.clone())).unwrap();
    REGISTRY.register(Box::new(RENEWABLE_PERCENTAGE.clone())).unwrap();
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