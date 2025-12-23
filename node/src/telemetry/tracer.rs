//! Distributed Tracer Implementation

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;

/// Tracer configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TracerConfig {
    /// Service name for tracing
    pub service_name: String,
    /// Service version
    pub service_version: String,
    /// Environment (prod, staging, dev)
    pub environment: String,
    /// Sampling rate (0.0 to 1.0)
    pub sampling_rate: f64,
    /// Enable trace propagation
    pub propagation_enabled: bool,
    /// Maximum spans per trace
    pub max_spans_per_trace: usize,
    /// Span batch size for export
    pub batch_size: usize,
    /// Export interval
    pub export_interval_ms: u64,
}

impl Default for TracerConfig {
    fn default() -> Self {
        Self {
            service_name: "supernova-node".to_string(),
            service_version: env!("CARGO_PKG_VERSION").to_string(),
            environment: "development".to_string(),
            sampling_rate: 1.0,
            propagation_enabled: true,
            max_spans_per_trace: 1000,
            batch_size: 100,
            export_interval_ms: 5000,
        }
    }
}

/// Unique trace identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TraceId([u8; 16]);

impl TraceId {
    /// Generate a new random trace ID
    pub fn new() -> Self {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        Self(rng.gen())
    }

    /// Create from bytes
    pub fn from_bytes(bytes: [u8; 16]) -> Self {
        Self(bytes)
    }

    /// Get as bytes
    pub fn as_bytes(&self) -> &[u8; 16] {
        &self.0
    }

    /// Convert to hex string
    pub fn to_hex(&self) -> String {
        hex::encode(self.0)
    }

    /// Parse from hex string
    pub fn from_hex(s: &str) -> Option<Self> {
        let bytes = hex::decode(s).ok()?;
        if bytes.len() != 16 {
            return None;
        }
        let mut arr = [0u8; 16];
        arr.copy_from_slice(&bytes);
        Some(Self(arr))
    }
}

impl Default for TraceId {
    fn default() -> Self {
        Self::new()
    }
}

/// Unique span identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SpanId([u8; 8]);

impl SpanId {
    /// Generate a new random span ID
    pub fn new() -> Self {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        Self(rng.gen())
    }

    /// Create from bytes
    pub fn from_bytes(bytes: [u8; 8]) -> Self {
        Self(bytes)
    }

    /// Get as bytes
    pub fn as_bytes(&self) -> &[u8; 8] {
        &self.0
    }

    /// Convert to hex string
    pub fn to_hex(&self) -> String {
        hex::encode(self.0)
    }

    /// Parse from hex string
    pub fn from_hex(s: &str) -> Option<Self> {
        let bytes = hex::decode(s).ok()?;
        if bytes.len() != 8 {
            return None;
        }
        let mut arr = [0u8; 8];
        arr.copy_from_slice(&bytes);
        Some(Self(arr))
    }
}

impl Default for SpanId {
    fn default() -> Self {
        Self::new()
    }
}

/// Span status
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpanStatus {
    /// Unset status
    Unset,
    /// Operation completed successfully
    Ok,
    /// Operation failed
    Error,
}

/// Span kind
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpanKind {
    /// Internal operation
    Internal,
    /// Server handling a request
    Server,
    /// Client making a request
    Client,
    /// Producer sending a message
    Producer,
    /// Consumer receiving a message
    Consumer,
}

/// A span representing a unit of work
#[derive(Debug, Clone)]
pub struct Span {
    /// Trace ID this span belongs to
    pub trace_id: TraceId,
    /// Unique span ID
    pub span_id: SpanId,
    /// Parent span ID (if any)
    pub parent_span_id: Option<SpanId>,
    /// Operation name
    pub name: String,
    /// Span kind
    pub kind: SpanKind,
    /// Start time (Unix timestamp in nanoseconds)
    pub start_time_ns: u64,
    /// End time (Unix timestamp in nanoseconds)
    pub end_time_ns: Option<u64>,
    /// Span status
    pub status: SpanStatus,
    /// Status message
    pub status_message: Option<String>,
    /// Attributes
    pub attributes: HashMap<String, AttributeValue>,
    /// Events within the span
    pub events: Vec<SpanEvent>,
    /// Links to other spans
    pub links: Vec<SpanLink>,
}

impl Span {
    /// Create a new span
    pub fn new(name: &str, trace_id: TraceId, parent_span_id: Option<SpanId>) -> Self {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64;

        Self {
            trace_id,
            span_id: SpanId::new(),
            parent_span_id,
            name: name.to_string(),
            kind: SpanKind::Internal,
            start_time_ns: now,
            end_time_ns: None,
            status: SpanStatus::Unset,
            status_message: None,
            attributes: HashMap::new(),
            events: Vec::new(),
            links: Vec::new(),
        }
    }

    /// Set span kind
    pub fn with_kind(mut self, kind: SpanKind) -> Self {
        self.kind = kind;
        self
    }

    /// Add an attribute
    pub fn set_attribute(&mut self, key: &str, value: impl Into<AttributeValue>) {
        self.attributes.insert(key.to_string(), value.into());
    }

    /// Add an event
    pub fn add_event(&mut self, name: &str) {
        self.events.push(SpanEvent {
            name: name.to_string(),
            timestamp_ns: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos() as u64,
            attributes: HashMap::new(),
        });
    }

    /// Add an event with attributes
    pub fn add_event_with_attributes(
        &mut self,
        name: &str,
        attributes: HashMap<String, AttributeValue>,
    ) {
        self.events.push(SpanEvent {
            name: name.to_string(),
            timestamp_ns: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos() as u64,
            attributes,
        });
    }

    /// Set status to OK
    pub fn set_ok(&mut self) {
        self.status = SpanStatus::Ok;
    }

    /// Set status to error
    pub fn set_error(&mut self, message: &str) {
        self.status = SpanStatus::Error;
        self.status_message = Some(message.to_string());
    }

    /// End the span
    pub fn end(&mut self) {
        if self.end_time_ns.is_none() {
            self.end_time_ns = Some(
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_nanos() as u64,
            );
        }
    }

    /// Get span duration in nanoseconds
    pub fn duration_ns(&self) -> Option<u64> {
        self.end_time_ns.map(|end| end.saturating_sub(self.start_time_ns))
    }

    /// Check if span is ended
    pub fn is_ended(&self) -> bool {
        self.end_time_ns.is_some()
    }
}

/// Attribute value types
#[derive(Debug, Clone)]
pub enum AttributeValue {
    String(String),
    Int(i64),
    Float(f64),
    Bool(bool),
    StringArray(Vec<String>),
    IntArray(Vec<i64>),
}

impl From<&str> for AttributeValue {
    fn from(s: &str) -> Self {
        AttributeValue::String(s.to_string())
    }
}

impl From<String> for AttributeValue {
    fn from(s: String) -> Self {
        AttributeValue::String(s)
    }
}

impl From<i64> for AttributeValue {
    fn from(i: i64) -> Self {
        AttributeValue::Int(i)
    }
}

impl From<i32> for AttributeValue {
    fn from(i: i32) -> Self {
        AttributeValue::Int(i as i64)
    }
}

impl From<f64> for AttributeValue {
    fn from(f: f64) -> Self {
        AttributeValue::Float(f)
    }
}

impl From<bool> for AttributeValue {
    fn from(b: bool) -> Self {
        AttributeValue::Bool(b)
    }
}

/// An event within a span
#[derive(Debug, Clone)]
pub struct SpanEvent {
    /// Event name
    pub name: String,
    /// Timestamp in nanoseconds
    pub timestamp_ns: u64,
    /// Event attributes
    pub attributes: HashMap<String, AttributeValue>,
}

/// A link to another span
#[derive(Debug, Clone)]
pub struct SpanLink {
    /// Linked trace ID
    pub trace_id: TraceId,
    /// Linked span ID
    pub span_id: SpanId,
    /// Link attributes
    pub attributes: HashMap<String, AttributeValue>,
}

/// Main tracer for creating and managing spans
pub struct Tracer {
    config: TracerConfig,
    /// Completed spans waiting for export
    pending_spans: Arc<RwLock<Vec<Span>>>,
    /// Active span count
    active_spans: AtomicU64,
    /// Total spans created
    total_spans: AtomicU64,
    /// Spans exported
    exported_spans: AtomicU64,
}

impl Tracer {
    /// Create a new tracer
    pub fn new(config: TracerConfig) -> Self {
        Self {
            config,
            pending_spans: Arc::new(RwLock::new(Vec::new())),
            active_spans: AtomicU64::new(0),
            total_spans: AtomicU64::new(0),
            exported_spans: AtomicU64::new(0),
        }
    }

    /// Start a new trace with a root span
    pub fn start_trace(&self, operation_name: &str) -> Option<Span> {
        if !self.should_sample() {
            return None;
        }

        let trace_id = TraceId::new();
        let span = Span::new(operation_name, trace_id, None);

        self.active_spans.fetch_add(1, Ordering::Relaxed);
        self.total_spans.fetch_add(1, Ordering::Relaxed);

        Some(span)
    }

    /// Start a child span
    pub fn start_span(&self, operation_name: &str, parent: &Span) -> Option<Span> {
        if !self.should_sample() {
            return None;
        }

        let span = Span::new(operation_name, parent.trace_id, Some(parent.span_id));

        self.active_spans.fetch_add(1, Ordering::Relaxed);
        self.total_spans.fetch_add(1, Ordering::Relaxed);

        Some(span)
    }

    /// Start a span with a specific trace context
    pub fn start_span_with_context(
        &self,
        operation_name: &str,
        trace_id: TraceId,
        parent_span_id: Option<SpanId>,
    ) -> Option<Span> {
        if !self.should_sample() {
            return None;
        }

        let span = Span::new(operation_name, trace_id, parent_span_id);

        self.active_spans.fetch_add(1, Ordering::Relaxed);
        self.total_spans.fetch_add(1, Ordering::Relaxed);

        Some(span)
    }

    /// End a span and queue for export
    pub async fn end_span(&self, mut span: Span) {
        span.end();
        self.active_spans.fetch_sub(1, Ordering::Relaxed);

        let mut pending = self.pending_spans.write().await;
        pending.push(span);

        // Auto-flush if batch size reached
        if pending.len() >= self.config.batch_size {
            drop(pending);
            self.flush().await;
        }
    }

    /// Flush pending spans (would export to backend)
    pub async fn flush(&self) {
        let spans = {
            let mut pending = self.pending_spans.write().await;
            std::mem::take(&mut *pending)
        };

        let count = spans.len() as u64;
        if count > 0 {
            // In production, this would export to Jaeger/Zipkin/OTLP
            tracing::debug!("Exporting {} spans", count);
            self.exported_spans.fetch_add(count, Ordering::Relaxed);
        }
    }

    /// Check if we should sample this trace
    fn should_sample(&self) -> bool {
        if self.config.sampling_rate >= 1.0 {
            return true;
        }
        if self.config.sampling_rate <= 0.0 {
            return false;
        }

        use rand::Rng;
        let mut rng = rand::thread_rng();
        rng.gen::<f64>() < self.config.sampling_rate
    }

    /// Get tracer statistics
    pub fn stats(&self) -> TracerStats {
        TracerStats {
            active_spans: self.active_spans.load(Ordering::Relaxed),
            total_spans: self.total_spans.load(Ordering::Relaxed),
            exported_spans: self.exported_spans.load(Ordering::Relaxed),
        }
    }

    /// Get configuration
    pub fn config(&self) -> &TracerConfig {
        &self.config
    }
}

/// Tracer statistics
#[derive(Debug, Clone)]
pub struct TracerStats {
    /// Currently active spans
    pub active_spans: u64,
    /// Total spans created
    pub total_spans: u64,
    /// Spans exported
    pub exported_spans: u64,
}

/// Middleware for automatic span creation on HTTP requests
pub struct TracingMiddleware {
    tracer: Arc<Tracer>,
}

impl TracingMiddleware {
    /// Create new tracing middleware
    pub fn new(tracer: Arc<Tracer>) -> Self {
        Self { tracer }
    }

    /// Get reference to tracer
    pub fn tracer(&self) -> &Arc<Tracer> {
        &self.tracer
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trace_id_generation() {
        let id1 = TraceId::new();
        let id2 = TraceId::new();
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_trace_id_hex_roundtrip() {
        let id = TraceId::new();
        let hex = id.to_hex();
        let parsed = TraceId::from_hex(&hex).unwrap();
        assert_eq!(id, parsed);
    }

    #[test]
    fn test_span_creation() {
        let trace_id = TraceId::new();
        let span = Span::new("test_operation", trace_id, None);
        assert_eq!(span.name, "test_operation");
        assert!(span.parent_span_id.is_none());
        assert!(!span.is_ended());
    }

    #[test]
    fn test_span_end() {
        let trace_id = TraceId::new();
        let mut span = Span::new("test_operation", trace_id, None);
        span.end();
        assert!(span.is_ended());
        assert!(span.duration_ns().is_some());
    }

    #[test]
    fn test_span_attributes() {
        let trace_id = TraceId::new();
        let mut span = Span::new("test_operation", trace_id, None);
        span.set_attribute("http.method", "GET");
        span.set_attribute("http.status_code", 200i64);
        span.set_attribute("success", true);

        assert!(span.attributes.contains_key("http.method"));
    }

    #[tokio::test]
    async fn test_tracer_start_trace() {
        let config = TracerConfig::default();
        let tracer = Tracer::new(config);

        let span = tracer.start_trace("root_operation");
        assert!(span.is_some());

        let stats = tracer.stats();
        assert_eq!(stats.active_spans, 1);
        assert_eq!(stats.total_spans, 1);
    }

    #[tokio::test]
    async fn test_tracer_child_span() {
        let config = TracerConfig::default();
        let tracer = Tracer::new(config);

        let root = tracer.start_trace("root").unwrap();
        let child = tracer.start_span("child", &root);
        assert!(child.is_some());

        let child = child.unwrap();
        assert_eq!(child.trace_id, root.trace_id);
        assert_eq!(child.parent_span_id, Some(root.span_id));
    }

    #[tokio::test]
    async fn test_tracer_sampling() {
        let mut config = TracerConfig::default();
        config.sampling_rate = 0.0; // Never sample

        let tracer = Tracer::new(config);
        let span = tracer.start_trace("test");
        assert!(span.is_none());
    }
}
