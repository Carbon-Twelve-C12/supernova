//! Metric types and data structures

use serde::{Deserialize, Serialize};

/// Metric value types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MetricValue {
    /// Counter value (monotonically increasing)
    Counter(u64),
    /// Gauge value (can increase or decrease)
    Gauge(f64),
    /// Histogram values
    Histogram {
        count: u64,
        sum: f64,
        buckets: Vec<(f64, u64)>,
    },
}

/// System metrics snapshot
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemMetrics {
    /// CPU usage percentage
    pub cpu_usage: f64,
    /// Memory usage in bytes
    pub memory_usage: u64,
    /// Disk usage in bytes
    pub disk_usage: u64,
    /// Network bytes sent
    pub network_bytes_sent: u64,
    /// Network bytes received
    pub network_bytes_received: u64,
    /// System uptime in seconds
    pub uptime: u64,
}

impl Default for SystemMetrics {
    fn default() -> Self {
        Self {
            cpu_usage: 0.0,
            memory_usage: 0,
            disk_usage: 0,
            network_bytes_sent: 0,
            network_bytes_received: 0,
            uptime: 0,
        }
    }
}
