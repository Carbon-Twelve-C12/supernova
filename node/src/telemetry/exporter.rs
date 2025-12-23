//! Telemetry Exporter
//!
//! Exports traces and metrics to various backends:
//! - OTLP (OpenTelemetry Protocol)
//! - Jaeger
//! - Zipkin
//! - Console (for debugging)

use super::tracer::{AttributeValue, Span, SpanKind, SpanStatus};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::sync::RwLock;

/// Exporter type
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ExporterType {
    /// OpenTelemetry Protocol
    Otlp,
    /// Jaeger
    Jaeger,
    /// Zipkin
    Zipkin,
    /// Console output (for debugging)
    Console,
    /// No-op exporter
    None,
}

/// Exporter configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExporterConfig {
    /// Exporter type
    pub exporter_type: ExporterType,
    /// Endpoint URL
    pub endpoint: Option<String>,
    /// Headers to include
    pub headers: HashMap<String, String>,
    /// Request timeout
    pub timeout_ms: u64,
    /// Batch size for export
    pub batch_size: usize,
    /// Max queue size
    pub max_queue_size: usize,
    /// Export interval in milliseconds
    pub export_interval_ms: u64,
}

impl Default for ExporterConfig {
    fn default() -> Self {
        Self {
            exporter_type: ExporterType::Console,
            endpoint: None,
            headers: HashMap::new(),
            timeout_ms: 10000,
            batch_size: 512,
            max_queue_size: 2048,
            export_interval_ms: 5000,
        }
    }
}

/// Main telemetry exporter
pub struct TelemetryExporter {
    config: ExporterConfig,
    client: Client,
    queue: Arc<RwLock<Vec<Span>>>,
    sender: mpsc::Sender<ExportCommand>,
}

enum ExportCommand {
    Export(Vec<Span>),
    Flush,
    Shutdown,
}

impl TelemetryExporter {
    /// Create a new exporter
    pub fn new(config: ExporterConfig) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_millis(config.timeout_ms))
            .build()
            .unwrap_or_default();

        let (sender, _receiver) = mpsc::channel(config.max_queue_size);

        Self {
            config,
            client,
            queue: Arc::new(RwLock::new(Vec::new())),
            sender,
        }
    }

    /// Queue a span for export
    pub async fn queue_span(&self, span: Span) {
        let mut queue = self.queue.write().await;
        queue.push(span);

        if queue.len() >= self.config.batch_size {
            let spans = std::mem::take(&mut *queue);
            drop(queue);
            let _ = self.sender.send(ExportCommand::Export(spans)).await;
        }
    }

    /// Queue multiple spans
    pub async fn queue_spans(&self, spans: Vec<Span>) {
        let mut queue = self.queue.write().await;
        queue.extend(spans);

        if queue.len() >= self.config.batch_size {
            let spans = std::mem::take(&mut *queue);
            drop(queue);
            let _ = self.sender.send(ExportCommand::Export(spans)).await;
        }
    }

    /// Force flush all queued spans
    pub async fn flush(&self) {
        let spans = {
            let mut queue = self.queue.write().await;
            std::mem::take(&mut *queue)
        };

        if !spans.is_empty() {
            self.export_spans(spans).await;
        }
    }

    /// Export spans to configured backend
    async fn export_spans(&self, spans: Vec<Span>) {
        match self.config.exporter_type {
            ExporterType::Console => self.export_console(&spans),
            ExporterType::Otlp => {
                if let Err(e) = self.export_otlp(&spans).await {
                    tracing::error!("OTLP export failed: {}", e);
                }
            }
            ExporterType::Jaeger => {
                if let Err(e) = self.export_jaeger(&spans).await {
                    tracing::error!("Jaeger export failed: {}", e);
                }
            }
            ExporterType::Zipkin => {
                if let Err(e) = self.export_zipkin(&spans).await {
                    tracing::error!("Zipkin export failed: {}", e);
                }
            }
            ExporterType::None => {}
        }
    }

    /// Export to console (for debugging)
    fn export_console(&self, spans: &[Span]) {
        for span in spans {
            let duration_ms = span
                .duration_ns()
                .map(|ns| ns / 1_000_000)
                .unwrap_or(0);

            let status = match span.status {
                SpanStatus::Ok => "OK",
                SpanStatus::Error => "ERROR",
                SpanStatus::Unset => "UNSET",
            };

            println!(
                "[TRACE] {} | {} | {}ms | {} | trace_id={} span_id={}",
                span.name,
                status,
                duration_ms,
                format_span_kind(span.kind),
                span.trace_id.to_hex(),
                span.span_id.to_hex()
            );

            for (key, value) in &span.attributes {
                println!("        {} = {:?}", key, value);
            }

            for event in &span.events {
                println!("        [EVENT] {}", event.name);
            }
        }
    }

    /// Export to OTLP endpoint
    async fn export_otlp(&self, spans: &[Span]) -> Result<(), String> {
        let endpoint = self
            .config
            .endpoint
            .as_ref()
            .ok_or("OTLP endpoint not configured")?;

        let payload = self.build_otlp_payload(spans);

        let mut request = self
            .client
            .post(endpoint)
            .header("Content-Type", "application/json");

        for (key, value) in &self.config.headers {
            request = request.header(key, value);
        }

        let response = request
            .json(&payload)
            .send()
            .await
            .map_err(|e| format!("Request failed: {}", e))?;

        if !response.status().is_success() {
            return Err(format!("OTLP returned status {}", response.status()));
        }

        tracing::debug!("Exported {} spans to OTLP", spans.len());
        Ok(())
    }

    /// Build OTLP JSON payload
    fn build_otlp_payload(&self, spans: &[Span]) -> serde_json::Value {
        let resource_spans: Vec<serde_json::Value> = spans
            .iter()
            .map(|span| {
                serde_json::json!({
                    "traceId": span.trace_id.to_hex(),
                    "spanId": span.span_id.to_hex(),
                    "parentSpanId": span.parent_span_id.map(|s| s.to_hex()),
                    "name": span.name,
                    "kind": span_kind_to_otlp(span.kind),
                    "startTimeUnixNano": span.start_time_ns.to_string(),
                    "endTimeUnixNano": span.end_time_ns.map(|t| t.to_string()),
                    "attributes": span.attributes.iter().map(|(k, v)| {
                        serde_json::json!({
                            "key": k,
                            "value": attribute_to_otlp(v)
                        })
                    }).collect::<Vec<_>>(),
                    "status": {
                        "code": span_status_to_otlp(span.status),
                        "message": span.status_message.as_deref().unwrap_or("")
                    }
                })
            })
            .collect();

        serde_json::json!({
            "resourceSpans": [{
                "resource": {
                    "attributes": []
                },
                "scopeSpans": [{
                    "scope": {
                        "name": "supernova-node"
                    },
                    "spans": resource_spans
                }]
            }]
        })
    }

    /// Export to Jaeger
    async fn export_jaeger(&self, spans: &[Span]) -> Result<(), String> {
        let endpoint = self
            .config
            .endpoint
            .as_ref()
            .ok_or("Jaeger endpoint not configured")?;

        // Jaeger Thrift format (simplified JSON)
        let payload = self.build_jaeger_payload(spans);

        let response = self
            .client
            .post(endpoint)
            .header("Content-Type", "application/json")
            .json(&payload)
            .send()
            .await
            .map_err(|e| format!("Request failed: {}", e))?;

        if !response.status().is_success() {
            return Err(format!("Jaeger returned status {}", response.status()));
        }

        tracing::debug!("Exported {} spans to Jaeger", spans.len());
        Ok(())
    }

    /// Build Jaeger payload
    fn build_jaeger_payload(&self, spans: &[Span]) -> serde_json::Value {
        let jaeger_spans: Vec<serde_json::Value> = spans
            .iter()
            .map(|span| {
                serde_json::json!({
                    "traceIdHigh": 0,
                    "traceIdLow": i64::from_be_bytes(span.trace_id.as_bytes()[8..16].try_into().unwrap_or([0;8])),
                    "spanId": i64::from_be_bytes(*span.span_id.as_bytes()),
                    "parentSpanId": span.parent_span_id.map(|s| i64::from_be_bytes(*s.as_bytes())).unwrap_or(0),
                    "operationName": span.name,
                    "startTime": span.start_time_ns / 1000, // microseconds
                    "duration": span.duration_ns().unwrap_or(0) / 1000,
                    "tags": span.attributes.iter().map(|(k, v)| {
                        serde_json::json!({
                            "key": k,
                            "type": "string",
                            "value": format!("{:?}", v)
                        })
                    }).collect::<Vec<_>>()
                })
            })
            .collect();

        serde_json::json!({
            "process": {
                "serviceName": "supernova-node",
                "tags": []
            },
            "spans": jaeger_spans
        })
    }

    /// Export to Zipkin
    async fn export_zipkin(&self, spans: &[Span]) -> Result<(), String> {
        let endpoint = self
            .config
            .endpoint
            .as_ref()
            .ok_or("Zipkin endpoint not configured")?;

        let payload = self.build_zipkin_payload(spans);

        let response = self
            .client
            .post(endpoint)
            .header("Content-Type", "application/json")
            .json(&payload)
            .send()
            .await
            .map_err(|e| format!("Request failed: {}", e))?;

        if !response.status().is_success() {
            return Err(format!("Zipkin returned status {}", response.status()));
        }

        tracing::debug!("Exported {} spans to Zipkin", spans.len());
        Ok(())
    }

    /// Build Zipkin payload
    fn build_zipkin_payload(&self, spans: &[Span]) -> Vec<serde_json::Value> {
        spans
            .iter()
            .map(|span| {
                serde_json::json!({
                    "traceId": span.trace_id.to_hex(),
                    "id": span.span_id.to_hex(),
                    "parentId": span.parent_span_id.map(|s| s.to_hex()),
                    "name": span.name,
                    "kind": span_kind_to_zipkin(span.kind),
                    "timestamp": span.start_time_ns / 1000, // microseconds
                    "duration": span.duration_ns().unwrap_or(0) / 1000,
                    "localEndpoint": {
                        "serviceName": "supernova-node"
                    },
                    "tags": span.attributes.iter().map(|(k, v)| {
                        (k.clone(), format!("{:?}", v))
                    }).collect::<HashMap<_, _>>()
                })
            })
            .collect()
    }

    /// Get configuration
    pub fn config(&self) -> &ExporterConfig {
        &self.config
    }
}

fn format_span_kind(kind: SpanKind) -> &'static str {
    match kind {
        SpanKind::Internal => "INTERNAL",
        SpanKind::Server => "SERVER",
        SpanKind::Client => "CLIENT",
        SpanKind::Producer => "PRODUCER",
        SpanKind::Consumer => "CONSUMER",
    }
}

fn span_kind_to_otlp(kind: SpanKind) -> i32 {
    match kind {
        SpanKind::Internal => 1,
        SpanKind::Server => 2,
        SpanKind::Client => 3,
        SpanKind::Producer => 4,
        SpanKind::Consumer => 5,
    }
}

fn span_kind_to_zipkin(kind: SpanKind) -> &'static str {
    match kind {
        SpanKind::Server => "SERVER",
        SpanKind::Client => "CLIENT",
        SpanKind::Producer => "PRODUCER",
        SpanKind::Consumer => "CONSUMER",
        SpanKind::Internal => "CLIENT", // Zipkin doesn't have INTERNAL
    }
}

fn span_status_to_otlp(status: SpanStatus) -> i32 {
    match status {
        SpanStatus::Unset => 0,
        SpanStatus::Ok => 1,
        SpanStatus::Error => 2,
    }
}

fn attribute_to_otlp(attr: &AttributeValue) -> serde_json::Value {
    match attr {
        AttributeValue::String(s) => serde_json::json!({"stringValue": s}),
        AttributeValue::Int(i) => serde_json::json!({"intValue": i.to_string()}),
        AttributeValue::Float(f) => serde_json::json!({"doubleValue": f}),
        AttributeValue::Bool(b) => serde_json::json!({"boolValue": b}),
        AttributeValue::StringArray(arr) => serde_json::json!({
            "arrayValue": {
                "values": arr.iter().map(|s| serde_json::json!({"stringValue": s})).collect::<Vec<_>>()
            }
        }),
        AttributeValue::IntArray(arr) => serde_json::json!({
            "arrayValue": {
                "values": arr.iter().map(|i| serde_json::json!({"intValue": i.to_string()})).collect::<Vec<_>>()
            }
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exporter_config_default() {
        let config = ExporterConfig::default();
        assert_eq!(config.exporter_type, ExporterType::Console);
        assert_eq!(config.batch_size, 512);
    }

    #[test]
    fn test_exporter_creation() {
        let config = ExporterConfig::default();
        let _exporter = TelemetryExporter::new(config);
    }

    #[tokio::test]
    async fn test_console_export() {
        let config = ExporterConfig {
            exporter_type: ExporterType::Console,
            ..Default::default()
        };

        let exporter = TelemetryExporter::new(config);

        let trace_id = super::super::tracer::TraceId::new();
        let span = Span::new("test_operation", trace_id, None);

        exporter.queue_span(span).await;
        exporter.flush().await;
    }
}
