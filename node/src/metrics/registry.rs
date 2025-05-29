use std::time::{Duration, Instant};
use metrics::{Counter, Gauge, Histogram, Key, KeyName, Unit};
use metrics_exporter_prometheus::{PrometheusBuilder, PrometheusHandle};
use tracing::{debug, info, warn, error};

pub struct MetricsRegistry {
    // System metrics
    pub system_metrics: SystemMetrics,
    // Blockchain metrics
    pub blockchain_metrics: BlockchainMetrics,
    // Network metrics
    pub network_metrics: NetworkMetrics,
    // Consensus metrics
    pub consensus_metrics: ConsensusMetrics,
    // Mempool metrics
    pub mempool_metrics: MempoolMetrics,
    // Lightning Network metrics
    pub lightning_metrics: LightningMetrics,
    // Handle to Prometheus exporter (if configured)
    prometheus_handle: Option<PrometheusHandle>,
}

impl MetricsRegistry {
    /// Create a new metrics registry with default configuration
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let builder = PrometheusBuilder::new();
        let handle = builder
            .install_recorder()?;
        
        info!("Metrics system initialized with Prometheus exporter");
        
        // Initialize all metric groups
        let system_metrics = SystemMetrics::new();
        let blockchain_metrics = BlockchainMetrics::new();
        let network_metrics = NetworkMetrics::new();
        let consensus_metrics = ConsensusMetrics::new();
        let mempool_metrics = MempoolMetrics::new();
        let lightning_metrics = LightningMetrics::new();
        
        Ok(Self {
            system_metrics,
            blockchain_metrics,
            network_metrics,
            consensus_metrics,
            mempool_metrics,
            lightning_metrics,
            prometheus_handle: Some(handle),
        })
    }
    
    /// Create a new metrics registry with custom configuration
    pub fn with_config(config: MetricsConfig) -> Result<Self, Box<dyn std::error::Error>> {
        let mut builder = PrometheusBuilder::new();
        
        // Set endpoint if configured
        if let Some(endpoint) = &config.endpoint {
            builder = builder.listen_address(endpoint.parse()?);
        }
        
        let handle = builder.install_recorder()?;
        
        info!("Metrics system initialized with custom configuration");
        
        // Initialize all metric groups
        let system_metrics = SystemMetrics::new();
        let blockchain_metrics = BlockchainMetrics::new();
        let network_metrics = NetworkMetrics::new();
        let consensus_metrics = ConsensusMetrics::new();
        let mempool_metrics = MempoolMetrics::new();
        let lightning_metrics = LightningMetrics::new();
        
        Ok(Self {
            system_metrics,
            blockchain_metrics,
            network_metrics,
            consensus_metrics,
            mempool_metrics,
            lightning_metrics,
            prometheus_handle: Some(handle),
        })
    }
    
    /// Create a disabled metrics registry (for testing or when metrics are disabled)
    pub fn disabled() -> Self {
        // Initialize dummy metric groups
        let system_metrics = SystemMetrics::new();
        let blockchain_metrics = BlockchainMetrics::new();
        let network_metrics = NetworkMetrics::new();
        let consensus_metrics = ConsensusMetrics::new();
        let mempool_metrics = MempoolMetrics::new();
        let lightning_metrics = LightningMetrics::new();
        
        info!("Metrics system initialized in disabled mode");
        
        Self {
            system_metrics,
            blockchain_metrics,
            network_metrics,
            consensus_metrics,
            mempool_metrics,
            lightning_metrics,
            prometheus_handle: None,
        }
    }
    
    /// Check if metrics are enabled
    pub fn is_enabled(&self) -> bool {
        self.prometheus_handle.is_some()
    }
}

/// Configuration for metrics system
#[derive(Debug, Clone)]
pub struct MetricsConfig {
    /// Namespace for metrics (default: "supernova")
    pub namespace: Option<String>,
    /// Global labels to add to all metrics
    pub global_labels: std::collections::HashMap<String, String>,
    /// HTTP endpoint for Prometheus scraping (e.g., "0.0.0.0:9090")
    pub endpoint: Option<String>,
    /// Push gateway URL
    pub push_gateway: Option<String>,
    /// Push interval
    pub push_interval: Option<Duration>,
}

impl Default for MetricsConfig {
    fn default() -> Self {
        let mut global_labels = std::collections::HashMap::new();
        global_labels.insert("version".to_string(), env!("CARGO_PKG_VERSION").to_string());
        
        Self {
            namespace: Some("supernova".to_string()),
            global_labels,
            endpoint: Some("0.0.0.0:9090".to_string()),
            push_gateway: None,
            push_interval: None,
        }
    }
}

/// System-level metrics
pub struct SystemMetrics {
    /// CPU usage percentage
    cpu_usage: Gauge,
    /// Memory usage in bytes
    memory_usage: Gauge,
    /// Disk usage in bytes
    disk_usage: Gauge,
    /// Open file descriptors
    open_files: Gauge,
    /// System uptime
    uptime: Gauge,
}

impl SystemMetrics {
    /// Create new system metrics
    pub fn new() -> Self {
        Self {
            cpu_usage: metrics::gauge!("system_cpu_usage_percent"),
            memory_usage: metrics::gauge!("system_memory_usage_bytes"),
            disk_usage: metrics::gauge!("system_disk_usage_bytes"),
            open_files: metrics::gauge!("system_open_files"),
            uptime: metrics::gauge!("system_uptime_seconds"),
        }
    }
    
    /// Update CPU usage
    pub fn update_cpu_usage(&self, usage_percent: f64) {
        self.cpu_usage.set(usage_percent);
    }
    
    /// Update memory usage
    pub fn update_memory_usage(&self, usage_bytes: u64) {
        self.memory_usage.set(usage_bytes as f64);
    }
    
    /// Update disk usage
    pub fn update_disk_usage(&self, usage_bytes: u64) {
        self.disk_usage.set(usage_bytes as f64);
    }
    
    /// Update open files count
    pub fn update_open_files(&self, count: u64) {
        self.open_files.set(count as f64);
    }
    
    /// Update system uptime
    pub fn update_uptime(&self, uptime_seconds: u64) {
        self.uptime.set(uptime_seconds as f64);
    }
}

/// Blockchain-specific metrics
pub struct BlockchainMetrics {
    /// Current blockchain height
    height: Gauge,
    /// Total number of transactions processed
    total_transactions: Counter,
    /// Block processing time
    block_processing_time: Histogram,
    /// Block size in bytes
    block_size: Histogram,
    /// Transactions per block
    transactions_per_block: Histogram,
    /// Current difficulty
    difficulty: Gauge,
    /// Estimated hash rate
    hash_rate: Gauge,
    /// Time since last block
    time_since_last_block: Gauge,
}

impl BlockchainMetrics {
    /// Create new blockchain metrics
    pub fn new() -> Self {
        Self {
            height: metrics::gauge!("blockchain_height"),
            total_transactions: metrics::counter!("blockchain_total_transactions"),
            block_processing_time: metrics::histogram!("blockchain_block_processing_time_seconds"),
            block_size: metrics::histogram!("blockchain_block_size_bytes"),
            transactions_per_block: metrics::histogram!("blockchain_transactions_per_block"),
            difficulty: metrics::gauge!("blockchain_difficulty"),
            hash_rate: metrics::gauge!("blockchain_estimated_hash_rate"),
            time_since_last_block: metrics::gauge!("blockchain_time_since_last_block_seconds"),
        }
    }
    
    /// Update blockchain height
    pub fn update_height(&self, height: u64) {
        self.height.set(height as f64);
    }
    
    /// Increment transaction count
    pub fn add_transactions(&self, count: u64) {
        self.total_transactions.increment(count);
    }
    
    /// Record block processing time
    pub fn record_block_processing_time(&self, seconds: f64) {
        self.block_processing_time.record(seconds);
    }
    
    /// Record block size
    pub fn record_block_size(&self, size_bytes: u64) {
        self.block_size.record(size_bytes as f64);
    }
    
    /// Record transactions per block
    pub fn record_transactions_per_block(&self, count: u64) {
        self.transactions_per_block.record(count as f64);
    }
    
    /// Update current difficulty
    pub fn update_difficulty(&self, difficulty: f64) {
        self.difficulty.set(difficulty);
    }
    
    /// Update estimated hash rate
    pub fn update_hash_rate(&self, hash_rate: f64) {
        self.hash_rate.set(hash_rate);
    }
    
    /// Update time since last block
    pub fn update_time_since_last_block(&self, seconds: f64) {
        self.time_since_last_block.set(seconds);
    }
}

/// Network-related metrics
pub struct NetworkMetrics {
    /// Number of connected peers
    connected_peers: Gauge,
    /// Number of inbound connections
    inbound_connections: Gauge,
    /// Number of outbound connections
    outbound_connections: Gauge,
    /// Bytes received
    bytes_received: Counter,
    /// Bytes sent
    bytes_sent: Counter,
    /// Messages received
    messages_received: Counter,
    /// Messages sent
    messages_sent: Counter,
    /// Connection attempts
    connection_attempts: Counter,
    /// Failed connection attempts
    failed_connection_attempts: Counter,
    /// Peer connection duration
    peer_connection_duration: Histogram,
    /// Message processing time
    message_processing_time: Histogram,
}

impl NetworkMetrics {
    /// Create new network metrics
    pub fn new() -> Self {
        Self {
            connected_peers: metrics::gauge!("network_connected_peers"),
            inbound_connections: metrics::gauge!("network_inbound_connections"),
            outbound_connections: metrics::gauge!("network_outbound_connections"),
            bytes_received: metrics::counter!("network_bytes_received"),
            bytes_sent: metrics::counter!("network_bytes_sent"),
            messages_received: metrics::counter!("network_messages_received"),
            messages_sent: metrics::counter!("network_messages_sent"),
            connection_attempts: metrics::counter!("network_connection_attempts"),
            failed_connection_attempts: metrics::counter!("network_failed_connection_attempts"),
            peer_connection_duration: metrics::histogram!("network_peer_connection_duration_seconds"),
            message_processing_time: metrics::histogram!("network_message_processing_time_seconds"),
        }
    }
    
    /// Update connected peers count
    pub fn update_connected_peers(&self, count: u64) {
        self.connected_peers.set(count as f64);
    }
    
    /// Update inbound connections count
    pub fn update_inbound_connections(&self, count: u64) {
        self.inbound_connections.set(count as f64);
    }
    
    /// Update outbound connections count
    pub fn update_outbound_connections(&self, count: u64) {
        self.outbound_connections.set(count as f64);
    }
    
    /// Add bytes received
    pub fn add_bytes_received(&self, bytes: u64) {
        self.bytes_received.increment(bytes);
    }
    
    /// Add bytes sent
    pub fn add_bytes_sent(&self, bytes: u64) {
        self.bytes_sent.increment(bytes);
    }
    
    /// Increment messages received
    pub fn add_message_received(&self) {
        self.messages_received.increment(1);
    }
    
    /// Increment messages sent
    pub fn add_message_sent(&self) {
        self.messages_sent.increment(1);
    }
    
    /// Record connection attempt
    pub fn record_connection_attempt(&self, success: bool) {
        self.connection_attempts.increment(1);
        if !success {
            self.failed_connection_attempts.increment(1);
        }
    }
    
    /// Record peer connection duration when a peer disconnects
    pub fn record_peer_connection_duration(&self, duration_seconds: f64) {
        self.peer_connection_duration.record(duration_seconds);
    }
    
    /// Record message processing time
    pub fn record_message_processing_time(&self, seconds: f64) {
        self.message_processing_time.record(seconds);
    }
}

/// Consensus-related metrics
pub struct ConsensusMetrics {
    /// Fork count
    fork_count: Counter,
    /// Reorganization count
    reorg_count: Counter,
    /// Reorganization depth
    reorg_depth: Histogram,
    /// Reorganization duration
    reorg_duration: Histogram,
    /// Number of orphaned blocks
    orphan_blocks: Counter,
    /// Invalid blocks received
    invalid_blocks: Counter,
}

impl ConsensusMetrics {
    /// Create new consensus metrics
    pub fn new() -> Self {
        Self {
            fork_count: metrics::counter!("consensus_fork_count"),
            reorg_count: metrics::counter!("consensus_reorg_count"),
            reorg_depth: metrics::histogram!("consensus_reorg_depth_blocks"),
            reorg_duration: metrics::histogram!("consensus_reorg_duration_seconds"),
            orphan_blocks: metrics::counter!("consensus_orphan_blocks"),
            invalid_blocks: metrics::counter!("consensus_invalid_blocks"),
        }
    }
    
    /// Increment fork count
    pub fn increment_fork_count(&self) {
        self.fork_count.increment(1);
    }
    
    /// Record chain reorganization
    pub fn record_reorg(&self, depth: u64, duration_seconds: f64) {
        self.reorg_count.increment(1);
        self.reorg_depth.record(depth as f64);
        self.reorg_duration.record(duration_seconds);
    }
    
    /// Increment orphaned blocks
    pub fn increment_orphan_blocks(&self, count: u64) {
        self.orphan_blocks.increment(count);
    }
    
    /// Increment invalid blocks
    pub fn increment_invalid_blocks(&self, count: u64) {
        self.invalid_blocks.increment(count);
    }
}

/// Mempool-related metrics
pub struct MempoolMetrics {
    /// Current size of mempool
    size: Gauge,
    /// Number of transactions in the mempool
    transactions: Gauge,
    /// Bytes used by mempool
    bytes: Gauge,
    /// Maximum fee rate in mempool
    max_fee_rate: Gauge,
    /// Minimum fee rate in mempool
    min_fee_rate: Gauge,
    /// Median fee rate in mempool
    median_fee_rate: Gauge,
    /// Transactions added to mempool
    transactions_added: Counter,
    /// Transactions rejected from mempool
    transactions_rejected: Counter,
    /// Transactions removed from mempool (included in blocks)
    transactions_removed: Counter,
    /// Transactions expired from mempool
    transactions_expired: Counter,
}

impl MempoolMetrics {
    /// Create new mempool metrics
    pub fn new() -> Self {
        Self {
            size: metrics::gauge!("mempool_size"),
            transactions: metrics::gauge!("mempool_transactions"),
            bytes: metrics::gauge!("mempool_bytes"),
            max_fee_rate: metrics::gauge!("mempool_max_fee_rate"),
            min_fee_rate: metrics::gauge!("mempool_min_fee_rate"),
            median_fee_rate: metrics::gauge!("mempool_median_fee_rate"),
            transactions_added: metrics::counter!("mempool_transactions_added"),
            transactions_rejected: metrics::counter!("mempool_transactions_rejected"),
            transactions_removed: metrics::counter!("mempool_transactions_removed"),
            transactions_expired: metrics::counter!("mempool_transactions_expired"),
        }
    }
    
    /// Update mempool size
    pub fn update_size(&self, transaction_count: u64, bytes: u64) {
        self.transactions.set(transaction_count as f64);
        self.bytes.set(bytes as f64);
        self.size.set(transaction_count as f64); // For backward compatibility
    }
    
    /// Update fee rates
    pub fn update_fee_rates(&self, min: f64, max: f64, median: f64) {
        self.min_fee_rate.set(min);
        self.max_fee_rate.set(max);
        self.median_fee_rate.set(median);
    }
    
    /// Record transaction added to mempool
    pub fn record_transaction_added(&self) {
        self.transactions_added.increment(1);
    }
    
    /// Record transaction rejected from mempool
    pub fn record_transaction_rejected(&self) {
        self.transactions_rejected.increment(1);
    }
    
    /// Record transaction removed from mempool
    pub fn record_transaction_removed(&self) {
        self.transactions_removed.increment(1);
    }
    
    /// Record transaction expired from mempool
    pub fn record_transaction_expired(&self) {
        self.transactions_expired.increment(1);
    }
}

/// Lightning Network-related metrics
pub struct LightningMetrics {
    /// Number of active payment channels
    active_channels: Gauge,
    /// Number of pending channels (opening/closing)
    pending_channels: Gauge,
    /// Number of channel opens initiated
    channel_opens: Counter,
    /// Number of channel closes initiated
    channel_closes: Counter,
    /// Number of successful payments
    payments_success: Counter,
    /// Number of failed payments
    payments_failed: Counter,
    /// Number of HTLCs currently in flight
    htlcs_in_flight: Gauge,
    /// Total capacity of all channels in satoshis
    total_capacity: Gauge,
    /// Local balance across all channels in satoshis
    local_balance: Gauge,
    /// Remote balance across all channels in satoshis
    remote_balance: Gauge,
    /// Payment routing fee income in millisatoshis
    routing_fee_income: Counter,
    /// Average payment path length
    payment_path_length: Histogram,
    /// Payment processing time (end-to-end)
    payment_processing_time: Histogram,
    /// Payment amounts in millisatoshis
    payment_amounts: Histogram,
    /// Number of routing failures
    routing_failures: Counter,
    /// Number of channel errors
    channel_errors: Counter,
    /// Number of forwarded payments
    forwarded_payments: Counter,
    /// Number of declined HTLCs
    declined_htlcs: Counter,
    /// Number of channel force-closes
    force_closes: Counter,
}

impl LightningMetrics {
    /// Create new Lightning Network metrics
    pub fn new() -> Self {
        Self {
            active_channels: metrics::gauge!("lightning_active_channels"),
            pending_channels: metrics::gauge!("lightning_pending_channels"),
            channel_opens: metrics::counter!("lightning_channel_opens"),
            channel_closes: metrics::counter!("lightning_channel_closes"),
            payments_success: metrics::counter!("lightning_payments_success"),
            payments_failed: metrics::counter!("lightning_payments_failed"),
            htlcs_in_flight: metrics::gauge!("lightning_htlcs_in_flight"),
            total_capacity: metrics::gauge!("lightning_total_capacity"),
            local_balance: metrics::gauge!("lightning_local_balance"),
            remote_balance: metrics::gauge!("lightning_remote_balance"),
            routing_fee_income: metrics::counter!("lightning_routing_fee_income"),
            payment_path_length: metrics::histogram!("lightning_payment_path_length"),
            payment_processing_time: metrics::histogram!("lightning_payment_processing_time"),
            payment_amounts: metrics::histogram!("lightning_payment_amounts"),
            routing_failures: metrics::counter!("lightning_routing_failures"),
            channel_errors: metrics::counter!("lightning_channel_errors"),
            forwarded_payments: metrics::counter!("lightning_forwarded_payments"),
            declined_htlcs: metrics::counter!("lightning_declined_htlcs"),
            force_closes: metrics::counter!("lightning_force_closes"),
        }
    }
    
    /// Update channel counts
    pub fn update_channel_counts(&self, active: u64, pending: u64) {
        self.active_channels.set(active as f64);
        self.pending_channels.set(pending as f64);
    }
    
    /// Record channel open
    pub fn record_channel_open(&self) {
        self.channel_opens.increment(1);
    }
    
    /// Record channel close
    pub fn record_channel_close(&self, force_close: bool) {
        self.channel_closes.increment(1);
        if force_close {
            self.force_closes.increment(1);
        }
    }
    
    /// Record payment outcome
    pub fn record_payment_outcome(&self, success: bool, amount_msat: u64, path_length: u64, processing_time_secs: f64) {
        if success {
            self.payments_success.increment(1);
        } else {
            self.payments_failed.increment(1);
        }
        
        self.payment_amounts.record(amount_msat as f64);
        self.payment_path_length.record(path_length as f64);
        self.payment_processing_time.record(processing_time_secs);
    }
    
    /// Update HTLC in-flight count
    pub fn update_htlcs_in_flight(&self, count: u64) {
        self.htlcs_in_flight.set(count as f64);
    }
    
    /// Update channel balances
    pub fn update_balances(&self, total_capacity: u64, local_balance: u64, remote_balance: u64) {
        self.total_capacity.set(total_capacity as f64);
        self.local_balance.set(local_balance as f64);
        self.remote_balance.set(remote_balance as f64);
    }
    
    /// Record routing fee income
    pub fn record_fee_income(&self, fee_msat: u64) {
        self.routing_fee_income.increment(fee_msat);
    }
    
    /// Record routing failure
    pub fn record_routing_failure(&self) {
        self.routing_failures.increment(1);
    }
    
    /// Record channel error
    pub fn record_channel_error(&self) {
        self.channel_errors.increment(1);
    }
    
    /// Record forwarded payment
    pub fn record_forwarded_payment(&self) {
        self.forwarded_payments.increment(1);
    }
    
    /// Record declined HTLC
    pub fn record_declined_htlc(&self) {
        self.declined_htlcs.increment(1);
    }
}

/// Helper for timing operations
pub struct TimedOperation<F>
where 
    F: FnOnce(f64),
{
    start_time: Instant,
    callback: Option<F>,
}

impl<F> TimedOperation<F>
where
    F: FnOnce(f64),
{
    /// Create a new timed operation
    pub fn new(callback: F) -> Self {
        Self {
            start_time: Instant::now(),
            callback: Some(callback),
        }
    }
    
    /// Complete the operation and call the callback with the elapsed time
    pub fn complete(mut self) {
        if let Some(callback) = self.callback.take() {
            let elapsed = self.start_time.elapsed().as_secs_f64();
            callback(elapsed);
        }
    }
}

impl<F> Drop for TimedOperation<F>
where
    F: FnOnce(f64),
{
    fn drop(&mut self) {
        if let Some(callback) = self.callback.take() {
            let elapsed = self.start_time.elapsed().as_secs_f64();
            callback(elapsed);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    
    #[test]
    fn test_system_metrics() {
        let metrics = SystemMetrics::new();
        
        // Test updating metrics
        metrics.update_cpu_usage(45.5);
        metrics.update_memory_usage(1024 * 1024 * 100); // 100 MB
        metrics.update_disk_usage(1024 * 1024 * 1024 * 10); // 10 GB
        metrics.update_open_files(100);
        metrics.update_uptime(3600); // 1 hour
        
        // No assertions needed - just testing that the calls don't panic
    }
    
    #[test]
    fn test_blockchain_metrics() {
        let metrics = BlockchainMetrics::new();
        
        // Test updating metrics
        metrics.update_height(12345);
        metrics.add_transactions(10);
        metrics.record_block_processing_time(0.5);
        metrics.record_block_size(1024 * 100); // 100 KB
        metrics.record_transactions_per_block(200);
        metrics.update_difficulty(1000000.0);
        metrics.update_hash_rate(1e12); // 1 TH/s
        metrics.update_time_since_last_block(60.0); // 1 minute
        
        // No assertions needed - just testing that the calls don't panic
    }
    
    #[test]
    fn test_timed_operation() {
        let mut recorded_duration = None;
        
        {
            let operation = TimedOperation::new(|duration| {
                recorded_duration = Some(duration);
            });
            
            // Simulate some work
            thread::sleep(Duration::from_millis(10));
            
            operation.complete();
        }
        
        assert!(recorded_duration.is_some());
        assert!(recorded_duration.unwrap() >= 0.01); // At least 10ms
    }
    
    #[test]
    fn test_timed_operation_drop() {
        let mut recorded_duration = None;
        
        {
            let _operation = TimedOperation::new(|duration| {
                recorded_duration = Some(duration);
            });
            
            // Simulate some work
            thread::sleep(Duration::from_millis(10));
            
            // Let it drop automatically
        }
        
        assert!(recorded_duration.is_some());
        assert!(recorded_duration.unwrap() >= 0.01); // At least 10ms
    }
} 