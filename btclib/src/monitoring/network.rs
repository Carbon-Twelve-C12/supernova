use prometheus::{
    Registry, IntGaugeVec, IntCounterVec, GaugeVec, HistogramVec, Opts, HistogramOpts
};
use std::time::Duration;
use std::collections::HashMap;
use crate::monitoring::MetricsError;
use tracing::{warn, debug};

/// Network metrics collector
pub struct NetworkMetrics {
    /// Number of connected peers
    peer_count: IntGaugeVec,
    /// Peer connection duration
    peer_connection_time: GaugeVec,
    /// Messages received count
    messages_received: IntCounterVec,
    /// Messages sent count
    messages_sent: IntCounterVec,
    /// Message size in bytes
    message_size: HistogramVec,
    /// Message processing latency
    message_latency: HistogramVec,
    /// Bandwidth usage in bytes
    bandwidth_usage: GaugeVec,
    /// Peer scoring
    peer_score: GaugeVec,
    /// Connection errors
    connection_errors: IntCounterVec,
    /// Peer geolocation metrics
    peer_geolocation: IntGaugeVec,
    /// Network latency by region
    network_latency: GaugeVec,
}

impl NetworkMetrics {
    /// Create a new network metrics collector
    pub fn new(registry: &Registry, namespace: &str) -> Result<Self, MetricsError> {
        // Peer count
        let peer_count = IntGaugeVec::new(
            Opts::new("peer_count", "Number of connected peers")
                .namespace(namespace.to_string())
                .subsystem("network"),
            &["type"],
        )?;
        registry.register(Box::new(peer_count.clone()))?;
        
        // Peer connection time
        let peer_connection_time = GaugeVec::new(
            Opts::new("peer_connection_time_seconds", "Peer connection duration in seconds")
                .namespace(namespace.to_string())
                .subsystem("network"),
            &["peer_id"],
        )?;
        registry.register(Box::new(peer_connection_time.clone()))?;
        
        // Messages received
        let messages_received = IntCounterVec::new(
            Opts::new("messages_received", "Number of messages received")
                .namespace(namespace.to_string())
                .subsystem("network"),
            &["message_type"],
        )?;
        registry.register(Box::new(messages_received.clone()))?;
        
        // Messages sent
        let messages_sent = IntCounterVec::new(
            Opts::new("messages_sent", "Number of messages sent")
                .namespace(namespace.to_string())
                .subsystem("network"),
            &["message_type"],
        )?;
        registry.register(Box::new(messages_sent.clone()))?;
        
        // Message size
        let message_size = HistogramVec::new(
            HistogramOpts::new("message_size_bytes", "Message size in bytes")
                .namespace(namespace.to_string())
                .subsystem("network")
                .buckets(vec![100.0, 500.0, 1000.0, 5000.0, 10000.0, 50000.0, 100000.0, 500000.0]),
            &["message_type"],
        )?;
        registry.register(Box::new(message_size.clone()))?;
        
        // Message latency
        let message_latency = HistogramVec::new(
            HistogramOpts::new("message_latency_ms", "Message processing latency in milliseconds")
                .namespace(namespace.to_string())
                .subsystem("network")
                .buckets(vec![1.0, 5.0, 10.0, 50.0, 100.0, 500.0, 1000.0, 5000.0]),
            &["message_type"],
        )?;
        registry.register(Box::new(message_latency.clone()))?;
        
        // Bandwidth usage
        let bandwidth_usage = GaugeVec::new(
            Opts::new("bandwidth_bytes_per_sec", "Bandwidth usage in bytes per second")
                .namespace(namespace.to_string())
                .subsystem("network"),
            &["direction"],
        )?;
        registry.register(Box::new(bandwidth_usage.clone()))?;
        
        // Peer score
        let peer_score = GaugeVec::new(
            Opts::new("peer_score", "Peer scoring")
                .namespace(namespace.to_string())
                .subsystem("network"),
            &["peer_id"],
        )?;
        registry.register(Box::new(peer_score.clone()))?;
        
        // Connection errors
        let connection_errors = IntCounterVec::new(
            Opts::new("connection_errors", "Connection errors")
                .namespace(namespace.to_string())
                .subsystem("network"),
            &["error_type"],
        )?;
        registry.register(Box::new(connection_errors.clone()))?;
        
        // Peer geolocation
        let peer_geolocation = IntGaugeVec::new(
            Opts::new("peer_geolocation", "Peer distribution by region")
                .namespace(namespace.to_string())
                .subsystem("network"),
            &["region"],
        )?;
        registry.register(Box::new(peer_geolocation.clone()))?;
        
        // Network latency by region
        let network_latency = GaugeVec::new(
            Opts::new("network_latency_ms", "Network latency by region in milliseconds")
                .namespace(namespace.to_string())
                .subsystem("network"),
            &["region"],
        )?;
        registry.register(Box::new(network_latency.clone()))?;
        
        Ok(Self {
            peer_count,
            peer_connection_time,
            messages_received,
            messages_sent,
            message_size,
            message_latency,
            bandwidth_usage,
            peer_score,
            connection_errors,
            peer_geolocation,
            network_latency,
        })
    }
    
    /// Update the number of connected peers
    pub fn update_peer_count(&self, inbound: i64, outbound: i64, total: i64) {
        self.peer_count.with_label_values(&["inbound"]).set(inbound);
        self.peer_count.with_label_values(&["outbound"]).set(outbound);
        self.peer_count.with_label_values(&["total"]).set(total);
    }
    
    /// Register a new peer connection
    pub fn register_peer_connection(&self, peer_id: &str) {
        // Initialize connection time to 0
        self.peer_connection_time.with_label_values(&[peer_id]).set(0.0);
        
        debug!("Registered new peer connection: {}", peer_id);
    }
    
    /// Update peer connection time
    pub fn update_peer_connection_time(&self, peer_id: &str, connection_time: Duration) {
        self.peer_connection_time
            .with_label_values(&[peer_id])
            .set(connection_time.as_secs_f64());
    }
    
    /// Register a received message
    pub fn register_message_received(&self, message_type: &str, size: usize, latency: Duration) {
        self.messages_received.with_label_values(&[message_type]).inc();
        self.message_size.with_label_values(&[message_type]).observe(size as f64);
        self.message_latency.with_label_values(&[message_type]).observe(latency.as_millis() as f64);
    }
    
    /// Register a sent message
    pub fn register_message_sent(&self, message_type: &str, size: usize) {
        self.messages_sent.with_label_values(&[message_type]).inc();
        self.message_size.with_label_values(&[message_type]).observe(size as f64);
    }
    
    /// Update bandwidth usage
    pub fn update_bandwidth_usage(&self, incoming_bytes_per_sec: f64, outgoing_bytes_per_sec: f64) {
        self.bandwidth_usage.with_label_values(&["incoming"]).set(incoming_bytes_per_sec);
        self.bandwidth_usage.with_label_values(&["outgoing"]).set(outgoing_bytes_per_sec);
    }
    
    /// Update peer score
    pub fn update_peer_score(&self, peer_id: &str, score: f64) {
        self.peer_score.with_label_values(&[peer_id]).set(score);
    }
    
    /// Register a connection error
    pub fn register_connection_error(&self, error_type: &str) {
        self.connection_errors.with_label_values(&[error_type]).inc();
        
        warn!("Network connection error: {}", error_type);
    }
    
    /// Update peer geolocation distribution
    pub fn update_peer_geolocation(&self, geolocation_counts: &HashMap<String, i64>) {
        for (region, count) in geolocation_counts {
            self.peer_geolocation.with_label_values(&[region]).set(*count);
        }
    }
    
    /// Update network latency by region
    pub fn update_network_latency(&self, region: &str, latency_ms: f64) {
        self.network_latency.with_label_values(&[region]).set(latency_ms);
    }
    
    /// Get network metrics as a formatted string
    pub fn get_summary(&self) -> String {
        let mut summary = String::new();
        summary.push_str("=== Network Metrics Summary ===\n");
        
        // Peer counts
        summary.push_str(&format!("Peers: {} total ({} inbound, {} outbound)\n", 
            self.peer_count.with_label_values(&["total"]).get(),
            self.peer_count.with_label_values(&["inbound"]).get(),
            self.peer_count.with_label_values(&["outbound"]).get(),
        ));
        
        // Bandwidth usage
        summary.push_str(&format!("Bandwidth: {:.2} KB/s incoming, {:.2} KB/s outgoing\n",
            self.bandwidth_usage.with_label_values(&["incoming"]).get() / 1000.0,
            self.bandwidth_usage.with_label_values(&["outgoing"]).get() / 1000.0,
        ));
        
        // Message counts (top 5 types)
        summary.push_str("Message Counts:\n");
        
        // This would normally fetch actual data from the metrics registry
        // For now, just showing a placeholder message
        summary.push_str("  (Data would be dynamically populated in a real implementation)\n");
        
        // Connection errors
        summary.push_str("Recent Connection Errors:\n");
        summary.push_str("  (Data would be dynamically populated in a real implementation)\n");
        
        summary
    }
} 