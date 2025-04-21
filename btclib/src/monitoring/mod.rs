use prometheus::{
    Encoder, TextEncoder, Registry,
    Counter, CounterVec, Gauge, GaugeVec, Histogram, HistogramVec,
    IntCounter, IntCounterVec, IntGauge, IntGaugeVec,
    Opts, Result as PrometheusResult, HistogramOpts
};
use serde::{Serialize, Deserialize};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use thiserror::Error;
use tracing::{info, warn, error, debug};

pub mod system;
pub mod blockchain;
pub mod network;
pub mod consensus;
pub mod mempool;

/// Errors related to metrics operations
#[derive(Debug, Error)]
pub enum MetricsError {
    #[error("Prometheus error: {0}")]
    PrometheusError(String),
    
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
    
    #[error("Configuration error: {0}")]
    ConfigError(String),
    
    #[error("Metric initialization error: {0}")]
    InitializationError(String),
    
    #[error("Exporter error: {0}")]
    ExporterError(String),
}

impl From<prometheus::Error> for MetricsError {
    fn from(err: prometheus::Error) -> Self {
        MetricsError::PrometheusError(err.to_string())
    }
}

/// Configuration for the metrics system
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsConfig {
    /// Whether metrics are enabled
    pub enabled: bool,
    
    /// Metrics namespace (prefix for all metrics)
    pub namespace: String,
    
    /// Global labels to add to all metrics
    pub global_labels: HashMap<String, String>,
    
    /// HTTP endpoint for metrics scraping (e.g., "0.0.0.0:9090")
    pub endpoint: Option<String>,
    
    /// Push gateway URL (e.g., "http://pushgateway:9091")
    pub push_gateway: Option<String>,
    
    /// Push interval in seconds
    pub push_interval_secs: Option<u64>,
    
    /// Whether to enable trace sampling
    pub enable_tracing: bool,
    
    /// Trace sampling rate (0.0 - 1.0)
    pub trace_sampling_rate: f64,
    
    /// Whether to enable system metrics collection
    pub enable_system_metrics: bool,
    
    /// How often to collect system metrics (in seconds)
    pub system_metrics_interval_secs: u64,
}

impl Default for MetricsConfig {
    fn default() -> Self {
        let mut global_labels = HashMap::new();
        global_labels.insert("version".to_string(), env!("CARGO_PKG_VERSION").to_string());
        
        Self {
            enabled: true,
            namespace: "supernova".to_string(),
            global_labels,
            endpoint: Some("127.0.0.1:9090".to_string()),
            push_gateway: None,
            push_interval_secs: None,
            enable_tracing: false,
            trace_sampling_rate: 0.1,
            enable_system_metrics: true,
            system_metrics_interval_secs: 15,
        }
    }
}

/// A handle to a registered Prometheus metric
#[derive(Debug, Clone)]
pub enum MetricHandle {
    Counter(Arc<Counter>),
    IntCounter(Arc<IntCounter>),
    Gauge(Arc<Gauge>),
    IntGauge(Arc<IntGauge>),
    Histogram(Arc<Histogram>),
    CounterVec(Arc<CounterVec>),
    IntCounterVec(Arc<IntCounterVec>),
    GaugeVec(Arc<GaugeVec>),
    IntGaugeVec(Arc<IntGaugeVec>),
    HistogramVec(Arc<HistogramVec>),
}

/// Metrics registry for the application
pub struct MetricsRegistry {
    /// Registry for all metrics
    registry: Registry,
    
    /// Configuration
    config: MetricsConfig,
    
    /// Handle to the HTTP server (if enabled)
    _server_handle: Option<tokio::task::JoinHandle<()>>,
    
    /// Handle to the push client (if enabled)
    _push_handle: Option<tokio::task::JoinHandle<()>>,
    
    /// Registered metrics
    metrics: HashMap<String, MetricHandle>,
    
    /// System metrics collector
    system_metrics: Option<system::SystemMetrics>,
    
    /// Blockchain metrics
    blockchain_metrics: Option<blockchain::BlockchainMetrics>,
    
    /// Network metrics
    network_metrics: Option<network::NetworkMetrics>,
    
    /// Consensus metrics
    consensus_metrics: Option<consensus::ConsensusMetrics>,
    
    /// Mempool metrics
    mempool_metrics: Option<mempool::MempoolMetrics>,
}

impl MetricsRegistry {
    /// Create a new metrics registry
    pub async fn new(config: MetricsConfig) -> Result<Self, MetricsError> {
        if !config.enabled {
            return Ok(Self::disabled(config));
        }
        
        let registry = Registry::new();
        
        // Initialize various metric groups
        let system_metrics = if config.enable_system_metrics {
            Some(system::SystemMetrics::new(&registry, &config.namespace)?)
        } else {
            None
        };
        
        let blockchain_metrics = Some(blockchain::BlockchainMetrics::new(&registry, &config.namespace)?);
        let network_metrics = Some(network::NetworkMetrics::new(&registry, &config.namespace)?);
        let consensus_metrics = Some(consensus::ConsensusMetrics::new(&registry, &config.namespace)?);
        let mempool_metrics = Some(mempool::MempoolMetrics::new(&registry, &config.namespace)?);
        
        // Start HTTP server if endpoint is configured
        let _server_handle = if let Some(endpoint) = &config.endpoint {
            match endpoint.parse::<SocketAddr>() {
                Ok(addr) => {
                    let registry_clone = registry.clone();
                    let handle = tokio::spawn(async move {
                        if let Err(e) = start_http_server(addr, registry_clone).await {
                            error!("Metrics HTTP server error: {}", e);
                        }
                    });
                    
                    info!("Metrics HTTP server listening on {}", endpoint);
                    Some(handle)
                }
                Err(e) => {
                    warn!("Failed to parse metrics endpoint {}: {}", endpoint, e);
                    None
                }
            }
        } else {
            None
        };
        
        // Start push client if push gateway is configured
        let _push_handle = if let Some(push_gateway) = &config.push_gateway {
            if let Some(interval_secs) = config.push_interval_secs {
                let push_gateway = push_gateway.clone();
                let registry_clone = registry.clone();
                let namespace = config.namespace.clone();
                
                let handle = tokio::spawn(async move {
                    push_metrics_loop(push_gateway, registry_clone, namespace, Duration::from_secs(interval_secs)).await;
                });
                
                info!("Metrics push client sending to {} every {} seconds", push_gateway, interval_secs);
                Some(handle)
            } else {
                warn!("Push gateway configured but push interval not set, push gateway disabled");
                None
            }
        } else {
            None
        };
        
        // Start system metrics collection if enabled
        if let Some(system_metrics) = &system_metrics {
            let interval = Duration::from_secs(config.system_metrics_interval_secs);
            system_metrics.start_collection(interval)?;
        }
        
        info!("Metrics system initialized with namespace: {}", config.namespace);
        
        Ok(Self {
            registry,
            config,
            _server_handle,
            _push_handle,
            metrics: HashMap::new(),
            system_metrics,
            blockchain_metrics,
            network_metrics,
            consensus_metrics,
            mempool_metrics,
        })
    }
    
    /// Create a disabled metrics registry
    pub fn disabled(config: MetricsConfig) -> Self {
        info!("Metrics system initialized in disabled mode");
        
        Self {
            registry: Registry::new(),
            config,
            _server_handle: None,
            _push_handle: None,
            metrics: HashMap::new(),
            system_metrics: None,
            blockchain_metrics: None,
            network_metrics: None,
            consensus_metrics: None,
            mempool_metrics: None,
        }
    }
    
    /// Check if metrics are enabled
    pub fn is_enabled(&self) -> bool {
        self.config.enabled
    }
    
    /// Register a new counter
    pub fn register_counter(&mut self, name: &str, help: &str) -> Result<Arc<Counter>, MetricsError> {
        if !self.config.enabled {
            // Return a noop counter when metrics are disabled
            return Ok(Arc::new(Counter::new(name, help)?));
        }
        
        let counter = Counter::with_opts(
            Opts::new(name, help)
                .namespace(self.config.namespace.clone())
        )?;
        
        self.registry.register(Box::new(counter.clone()))?;
        
        let counter_arc = Arc::new(counter);
        self.metrics.insert(name.to_string(), MetricHandle::Counter(counter_arc.clone()));
        
        Ok(counter_arc)
    }
    
    /// Register a new integer counter
    pub fn register_int_counter(&mut self, name: &str, help: &str) -> Result<Arc<IntCounter>, MetricsError> {
        if !self.config.enabled {
            // Return a noop counter when metrics are disabled
            return Ok(Arc::new(IntCounter::new(name, help)?));
        }
        
        let counter = IntCounter::with_opts(
            Opts::new(name, help)
                .namespace(self.config.namespace.clone())
        )?;
        
        self.registry.register(Box::new(counter.clone()))?;
        
        let counter_arc = Arc::new(counter);
        self.metrics.insert(name.to_string(), MetricHandle::IntCounter(counter_arc.clone()));
        
        Ok(counter_arc)
    }
    
    /// Register a new gauge
    pub fn register_gauge(&mut self, name: &str, help: &str) -> Result<Arc<Gauge>, MetricsError> {
        if !self.config.enabled {
            // Return a noop gauge when metrics are disabled
            return Ok(Arc::new(Gauge::with_opts(Opts::new(name, help))?));
        }
        
        let gauge = Gauge::with_opts(
            Opts::new(name, help)
                .namespace(self.config.namespace.clone())
        )?;
        
        self.registry.register(Box::new(gauge.clone()))?;
        
        let gauge_arc = Arc::new(gauge);
        self.metrics.insert(name.to_string(), MetricHandle::Gauge(gauge_arc.clone()));
        
        Ok(gauge_arc)
    }
    
    /// Register a new integer gauge
    pub fn register_int_gauge(&mut self, name: &str, help: &str) -> Result<Arc<IntGauge>, MetricsError> {
        if !self.config.enabled {
            // Return a noop gauge when metrics are disabled
            return Ok(Arc::new(IntGauge::with_opts(Opts::new(name, help))?));
        }
        
        let gauge = IntGauge::with_opts(
            Opts::new(name, help)
                .namespace(self.config.namespace.clone())
        )?;
        
        self.registry.register(Box::new(gauge.clone()))?;
        
        let gauge_arc = Arc::new(gauge);
        self.metrics.insert(name.to_string(), MetricHandle::IntGauge(gauge_arc.clone()));
        
        Ok(gauge_arc)
    }
    
    /// Register a new histogram
    pub fn register_histogram(
        &mut self,
        name: &str,
        help: &str,
        buckets: Vec<f64>,
    ) -> Result<Arc<Histogram>, MetricsError> {
        if !self.config.enabled {
            // Return a noop histogram when metrics are disabled
            return Ok(Arc::new(Histogram::with_opts(
                HistogramOpts::new(name, help)
            )?));
        }
        
        let histogram = Histogram::with_opts(
            HistogramOpts::new(name, help)
                .namespace(self.config.namespace.clone())
                .buckets(buckets)
        )?;
        
        self.registry.register(Box::new(histogram.clone()))?;
        
        let histogram_arc = Arc::new(histogram);
        self.metrics.insert(name.to_string(), MetricHandle::Histogram(histogram_arc.clone()));
        
        Ok(histogram_arc)
    }
    
    /// Get the system metrics
    pub fn system_metrics(&self) -> Option<&system::SystemMetrics> {
        self.system_metrics.as_ref()
    }
    
    /// Get the blockchain metrics
    pub fn blockchain_metrics(&self) -> Option<&blockchain::BlockchainMetrics> {
        self.blockchain_metrics.as_ref()
    }
    
    /// Get the network metrics
    pub fn network_metrics(&self) -> Option<&network::NetworkMetrics> {
        self.network_metrics.as_ref()
    }
    
    /// Get the consensus metrics
    pub fn consensus_metrics(&self) -> Option<&consensus::ConsensusMetrics> {
        self.consensus_metrics.as_ref()
    }
    
    /// Get the mempool metrics
    pub fn mempool_metrics(&self) -> Option<&mempool::MempoolMetrics> {
        self.mempool_metrics.as_ref()
    }
    
    /// Get all metrics as a string in Prometheus format
    pub fn get_metrics_as_string(&self) -> Result<String, MetricsError> {
        let encoder = TextEncoder::new();
        let metric_families = self.registry.gather();
        let mut buffer = Vec::new();
        
        encoder.encode(&metric_families, &mut buffer)
            .map_err(|e| MetricsError::ExporterError(e.to_string()))?;
        
        String::from_utf8(buffer)
            .map_err(|e| MetricsError::ExporterError(e.to_string()))
    }
}

/// Start an HTTP server for metrics scraping
async fn start_http_server(addr: SocketAddr, registry: Registry) -> Result<(), MetricsError> {
    use hyper::{
        service::{make_service_fn, service_fn},
        Body, Request, Response, Server,
    };
    use std::convert::Infallible;
    
    let make_svc = make_service_fn(move |_conn| {
        let registry = registry.clone();
        
        async move {
            Ok::<_, Infallible>(service_fn(move |req: Request<Body>| {
                let registry = registry.clone();
                
                async move {
                    let response = match req.uri().path() {
                        "/metrics" => {
                            let encoder = TextEncoder::new();
                            let metric_families = registry.gather();
                            let mut buffer = Vec::new();
                            
                            match encoder.encode(&metric_families, &mut buffer) {
                                Ok(_) => Response::builder()
                                    .status(200)
                                    .header("Content-Type", encoder.format_type())
                                    .body(Body::from(buffer))
                                    .unwrap_or_else(|_| {
                                        Response::builder()
                                            .status(500)
                                            .body(Body::from("Failed to serialize metrics"))
                                            .unwrap()
                                    }),
                                Err(_) => Response::builder()
                                    .status(500)
                                    .body(Body::from("Failed to encode metrics"))
                                    .unwrap(),
                            }
                        },
                        _ => Response::builder()
                            .status(404)
                            .body(Body::from("Not found"))
                            .unwrap(),
                    };
                    
                    Ok::<_, Infallible>(response)
                }
            }))
        }
    });
    
    Server::bind(&addr)
        .serve(make_svc)
        .await
        .map_err(|e| MetricsError::IoError(std::io::Error::new(std::io::ErrorKind::Other, e)))
}

/// Push metrics to a Prometheus push gateway
async fn push_metrics_loop(
    push_gateway: String,
    registry: Registry,
    namespace: String,
    interval: Duration,
) {
    use prometheus::push::{Grouping, PushMetrics};
    
    let grouping = Grouping::new();
    
    loop {
        tokio::time::sleep(interval).await;
        
        match PushMetrics::new(&push_gateway, &grouping)
            .pushable_metrics(&registry)
            .expect("Failed to create pushable metrics") // This should never fail
            .push() {
            Ok(_) => {
                debug!("Successfully pushed metrics to {}", push_gateway);
            }
            Err(e) => {
                warn!("Failed to push metrics to {}: {}", push_gateway, e);
            }
        }
    }
}

/// Defines a span of execution with timing information
#[derive(Debug)]
pub struct TracingSpan {
    /// Name of the span
    name: String,
    /// Start time
    start_time: Instant,
    /// Parent span ID if any
    parent_span_id: Option<u64>,
    /// Span ID
    span_id: u64,
    /// Additional attributes
    attributes: HashMap<String, String>,
}

impl TracingSpan {
    /// Create a new tracing span
    pub fn new(name: &str, parent_span_id: Option<u64>) -> Self {
        // Generate a random span ID
        let span_id = rand::random::<u64>();
        
        Self {
            name: name.to_string(),
            start_time: Instant::now(),
            parent_span_id,
            span_id,
            attributes: HashMap::new(),
        }
    }
    
    /// Add an attribute to the span
    pub fn with_attribute(mut self, key: &str, value: &str) -> Self {
        self.attributes.insert(key.to_string(), value.to_string());
        self
    }
    
    /// Get the span ID
    pub fn span_id(&self) -> u64 {
        self.span_id
    }
    
    /// End the span and get the duration
    pub fn end(self) -> Duration {
        self.start_time.elapsed()
    }
}

/// Trace collector for distributed tracing
#[derive(Clone)]
pub struct TraceCollector {
    /// Whether tracing is enabled
    enabled: bool,
    /// Sampling rate (0.0 - 1.0)
    sampling_rate: f64,
    /// Active spans
    active_spans: Arc<RwLock<HashMap<u64, TracingSpan>>>,
}

impl TraceCollector {
    /// Create a new trace collector
    pub fn new(enabled: bool, sampling_rate: f64) -> Self {
        Self {
            enabled,
            sampling_rate,
            active_spans: Arc::new(RwLock::new(HashMap::new())),
        }
    }
    
    /// Check if tracing is enabled
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }
    
    /// Start a new span
    pub async fn start_span(&self, name: &str, parent_span_id: Option<u64>) -> Option<u64> {
        if !self.enabled {
            return None;
        }
        
        // Apply sampling rate
        if rand::random::<f64>() > self.sampling_rate {
            return None;
        }
        
        let span = TracingSpan::new(name, parent_span_id);
        let span_id = span.span_id();
        
        let mut spans = self.active_spans.write().await;
        spans.insert(span_id, span);
        
        Some(span_id)
    }
    
    /// End a span
    pub async fn end_span(&self, span_id: u64) -> Option<Duration> {
        if !self.enabled {
            return None;
        }
        
        let mut spans = self.active_spans.write().await;
        spans.remove(&span_id).map(|span| span.end())
    }
    
    /// Add an attribute to a span
    pub async fn add_attribute(&self, span_id: u64, key: &str, value: &str) -> bool {
        if !self.enabled {
            return false;
        }
        
        let mut spans = self.active_spans.write().await;
        if let Some(span) = spans.get_mut(&span_id) {
            span.attributes.insert(key.to_string(), value.to_string());
            true
        } else {
            false
        }
    }
} 