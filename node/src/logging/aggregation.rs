//! Log Aggregation and Shipping Module
//!
//! Provides async log shipping to various backends:
//! - Loki (Grafana)
//! - Elasticsearch
//! - File-based aggregation
//!
//! Supports JSON structured logging format.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::mpsc;
use tokio::sync::RwLock;

/// Log aggregation backend type
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum LogBackend {
    /// Grafana Loki
    Loki,
    /// Elasticsearch
    Elasticsearch,
    /// Local file
    File,
    /// Console only (no shipping)
    Console,
    /// No-op
    None,
}

/// Configuration for log aggregation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogAggregationConfig {
    /// Backend type
    pub backend: LogBackend,
    /// Backend endpoint URL
    pub endpoint: Option<String>,
    /// API key or credentials
    pub api_key: Option<String>,
    /// Batch size for shipping
    pub batch_size: usize,
    /// Maximum queue size
    pub max_queue_size: usize,
    /// Flush interval in milliseconds
    pub flush_interval_ms: u64,
    /// Enable compression
    pub compression: bool,
    /// Retention period in days (for file backend)
    pub retention_days: u32,
    /// Log file path (for file backend)
    pub file_path: Option<String>,
    /// Additional labels/tags
    pub labels: HashMap<String, String>,
    /// Index name (for Elasticsearch)
    pub index_name: Option<String>,
}

impl Default for LogAggregationConfig {
    fn default() -> Self {
        let mut labels = HashMap::new();
        labels.insert("service".to_string(), "supernova-node".to_string());
        labels.insert(
            "version".to_string(),
            env!("CARGO_PKG_VERSION").to_string(),
        );

        Self {
            backend: LogBackend::Console,
            endpoint: None,
            api_key: None,
            batch_size: 100,
            max_queue_size: 10000,
            flush_interval_ms: 5000,
            compression: true,
            retention_days: 7,
            file_path: None,
            labels,
            index_name: Some("supernova-logs".to_string()),
        }
    }
}

/// Structured log entry for aggregation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StructuredLogEntry {
    /// Timestamp in RFC3339 format
    pub timestamp: String,
    /// Unix timestamp in nanoseconds
    #[serde(rename = "@timestamp")]
    pub timestamp_ns: u64,
    /// Log level
    pub level: String,
    /// Logger name/component
    pub logger: String,
    /// Log message
    pub message: String,
    /// Service name
    pub service: String,
    /// Host name
    pub host: String,
    /// Additional fields
    #[serde(flatten)]
    pub fields: HashMap<String, serde_json::Value>,
    /// Trace ID (if available)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trace_id: Option<String>,
    /// Span ID (if available)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub span_id: Option<String>,
}

impl StructuredLogEntry {
    /// Create a new structured log entry
    pub fn new(level: &str, logger: &str, message: String) -> Self {
        let now = SystemTime::now();
        let timestamp_ns = now
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64;

        let timestamp = chrono::Utc::now().to_rfc3339();

        let host = hostname::get()
            .ok()
            .and_then(|h| h.into_string().ok())
            .unwrap_or_else(|| "unknown".to_string());

        Self {
            timestamp,
            timestamp_ns,
            level: level.to_string(),
            logger: logger.to_string(),
            message,
            service: "supernova-node".to_string(),
            host,
            fields: HashMap::new(),
            trace_id: None,
            span_id: None,
        }
    }

    /// Add a field to the log entry
    pub fn with_field(mut self, key: &str, value: impl Into<serde_json::Value>) -> Self {
        self.fields.insert(key.to_string(), value.into());
        self
    }

    /// Add trace context
    pub fn with_trace(mut self, trace_id: String, span_id: Option<String>) -> Self {
        self.trace_id = Some(trace_id);
        self.span_id = span_id;
        self
    }

    /// Convert to JSON string
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }

    /// Convert to compact JSON string
    pub fn to_json_compact(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }
}

/// Log aggregator for async shipping
pub struct LogAggregator {
    config: LogAggregationConfig,
    sender: mpsc::Sender<StructuredLogEntry>,
    receiver: Arc<RwLock<Option<mpsc::Receiver<StructuredLogEntry>>>>,
    /// Logs shipped count
    shipped_count: AtomicU64,
    /// Logs dropped count
    dropped_count: AtomicU64,
    /// Running flag
    running: Arc<RwLock<bool>>,
}

impl LogAggregator {
    /// Create a new log aggregator
    pub fn new(config: LogAggregationConfig) -> Self {
        let (sender, receiver) = mpsc::channel(config.max_queue_size);

        Self {
            config,
            sender,
            receiver: Arc::new(RwLock::new(Some(receiver))),
            shipped_count: AtomicU64::new(0),
            dropped_count: AtomicU64::new(0),
            running: Arc::new(RwLock::new(false)),
        }
    }

    /// Queue a log entry for shipping
    pub async fn log(&self, entry: StructuredLogEntry) {
        if let Err(_) = self.sender.try_send(entry) {
            self.dropped_count.fetch_add(1, Ordering::Relaxed);
        }
    }

    /// Queue a simple log message
    pub async fn log_message(&self, level: &str, logger: &str, message: String) {
        let entry = StructuredLogEntry::new(level, logger, message);
        self.log(entry).await;
    }

    /// Start the shipping loop
    pub async fn start(&self) {
        let mut running = self.running.write().await;
        if *running {
            tracing::warn!("Log aggregator already running");
            return;
        }
        *running = true;
        drop(running);

        // Take the receiver
        let receiver = {
            let mut rx = self.receiver.write().await;
            rx.take()
        };

        let Some(mut receiver) = receiver else {
            tracing::error!("Log aggregator receiver already taken");
            return;
        };

        let config = self.config.clone();
        let shipped_count = &self.shipped_count;
        let running = Arc::clone(&self.running);

        tracing::info!(
            "Log aggregator started, shipping to {:?}",
            config.backend
        );

        let mut batch: Vec<StructuredLogEntry> = Vec::with_capacity(config.batch_size);
        let mut flush_interval = tokio::time::interval(Duration::from_millis(config.flush_interval_ms));

        loop {
            tokio::select! {
                Some(entry) = receiver.recv() => {
                    batch.push(entry);
                    if batch.len() >= config.batch_size {
                        let entries = std::mem::take(&mut batch);
                        Self::ship_batch(&config, entries, shipped_count).await;
                    }
                }
                _ = flush_interval.tick() => {
                    if !batch.is_empty() {
                        let entries = std::mem::take(&mut batch);
                        Self::ship_batch(&config, entries, shipped_count).await;
                    }

                    // Check if we should stop
                    let r = running.read().await;
                    if !*r {
                        break;
                    }
                }
            }
        }

        // Final flush
        if !batch.is_empty() {
            Self::ship_batch(&config, batch, shipped_count).await;
        }

        tracing::info!("Log aggregator stopped");
    }

    /// Stop the shipping loop
    pub async fn stop(&self) {
        let mut running = self.running.write().await;
        *running = false;
    }

    /// Ship a batch of logs
    async fn ship_batch(
        config: &LogAggregationConfig,
        entries: Vec<StructuredLogEntry>,
        shipped_count: &AtomicU64,
    ) {
        let count = entries.len();

        let result = match config.backend {
            LogBackend::Loki => Self::ship_to_loki(config, &entries).await,
            LogBackend::Elasticsearch => Self::ship_to_elasticsearch(config, &entries).await,
            LogBackend::File => Self::ship_to_file(config, &entries).await,
            LogBackend::Console => {
                for entry in &entries {
                    if let Ok(json) = entry.to_json() {
                        println!("{}", json);
                    }
                }
                Ok(())
            }
            LogBackend::None => Ok(()),
        };

        match result {
            Ok(()) => {
                shipped_count.fetch_add(count as u64, Ordering::Relaxed);
                tracing::debug!("Shipped {} log entries to {:?}", count, config.backend);
            }
            Err(e) => {
                tracing::error!("Failed to ship {} log entries: {}", count, e);
            }
        }
    }

    /// Ship logs to Grafana Loki
    async fn ship_to_loki(
        config: &LogAggregationConfig,
        entries: &[StructuredLogEntry],
    ) -> Result<(), String> {
        let endpoint = config
            .endpoint
            .as_ref()
            .ok_or("Loki endpoint not configured")?;

        // Build Loki push request format
        let mut streams: HashMap<String, Vec<(String, String)>> = HashMap::new();

        for entry in entries {
            let mut labels: Vec<String> = vec![
                format!("level=\"{}\"", entry.level),
                format!("service=\"{}\"", entry.service),
                format!("logger=\"{}\"", entry.logger),
            ];

            for (k, v) in &config.labels {
                labels.push(format!("{}=\"{}\"", k, v));
            }

            let label_str = format!("{{{}}}", labels.join(","));
            let timestamp_ns = entry.timestamp_ns.to_string();
            let message = entry.to_json().unwrap_or_else(|_| entry.message.clone());

            streams
                .entry(label_str)
                .or_default()
                .push((timestamp_ns, message));
        }

        let payload = serde_json::json!({
            "streams": streams.into_iter().map(|(labels, values)| {
                serde_json::json!({
                    "stream": serde_json::from_str::<serde_json::Value>(&labels.replace("{", "{\"").replace("}", "\"}").replace(",", "\",\"").replace("=", "\":")).unwrap_or_default(),
                    "values": values
                })
            }).collect::<Vec<_>>()
        });

        let client = reqwest::Client::new();
        let mut request = client
            .post(format!("{}/loki/api/v1/push", endpoint))
            .header("Content-Type", "application/json");

        if let Some(ref api_key) = config.api_key {
            request = request.header("Authorization", format!("Bearer {}", api_key));
        }

        let response = request
            .json(&payload)
            .send()
            .await
            .map_err(|e| format!("Loki request failed: {}", e))?;

        if !response.status().is_success() {
            return Err(format!("Loki returned status {}", response.status()));
        }

        Ok(())
    }

    /// Ship logs to Elasticsearch
    async fn ship_to_elasticsearch(
        config: &LogAggregationConfig,
        entries: &[StructuredLogEntry],
    ) -> Result<(), String> {
        let endpoint = config
            .endpoint
            .as_ref()
            .ok_or("Elasticsearch endpoint not configured")?;

        let index = config
            .index_name
            .as_ref()
            .map(|s| s.as_str())
            .unwrap_or("supernova-logs");

        // Build bulk request
        let mut bulk_body = String::new();
        for entry in entries {
            // Index action
            bulk_body.push_str(&format!(
                "{{\"index\":{{\"_index\":\"{}\"}}}}\n",
                index
            ));
            // Document
            if let Ok(json) = entry.to_json() {
                bulk_body.push_str(&json);
                bulk_body.push('\n');
            }
        }

        let client = reqwest::Client::new();
        let mut request = client
            .post(format!("{}/_bulk", endpoint))
            .header("Content-Type", "application/x-ndjson");

        if let Some(ref api_key) = config.api_key {
            request = request.header("Authorization", format!("ApiKey {}", api_key));
        }

        let response = request
            .body(bulk_body)
            .send()
            .await
            .map_err(|e| format!("Elasticsearch request failed: {}", e))?;

        if !response.status().is_success() {
            return Err(format!(
                "Elasticsearch returned status {}",
                response.status()
            ));
        }

        Ok(())
    }

    /// Ship logs to file
    async fn ship_to_file(
        config: &LogAggregationConfig,
        entries: &[StructuredLogEntry],
    ) -> Result<(), String> {
        let file_path = config
            .file_path
            .as_ref()
            .ok_or("Log file path not configured")?;

        // Create directory if it doesn't exist
        if let Some(parent) = std::path::Path::new(file_path).parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|e| format!("Failed to create log directory: {}", e))?;
        }

        // Append to file
        use tokio::io::AsyncWriteExt;
        let mut file = tokio::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(file_path)
            .await
            .map_err(|e| format!("Failed to open log file: {}", e))?;

        for entry in entries {
            if let Ok(json) = entry.to_json() {
                file.write_all(json.as_bytes())
                    .await
                    .map_err(|e| format!("Failed to write log: {}", e))?;
                file.write_all(b"\n")
                    .await
                    .map_err(|e| format!("Failed to write newline: {}", e))?;
            }
        }

        file.flush()
            .await
            .map_err(|e| format!("Failed to flush log file: {}", e))?;

        Ok(())
    }

    /// Get statistics
    pub fn stats(&self) -> LogAggregatorStats {
        LogAggregatorStats {
            shipped_count: self.shipped_count.load(Ordering::Relaxed),
            dropped_count: self.dropped_count.load(Ordering::Relaxed),
            backend: self.config.backend,
        }
    }
}

/// Log aggregator statistics
#[derive(Debug, Clone)]
pub struct LogAggregatorStats {
    /// Number of logs shipped
    pub shipped_count: u64,
    /// Number of logs dropped
    pub dropped_count: u64,
    /// Current backend
    pub backend: LogBackend,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_structured_log_entry() {
        let entry = StructuredLogEntry::new("INFO", "test_logger", "Test message".to_string())
            .with_field("custom_field", "custom_value");

        assert_eq!(entry.level, "INFO");
        assert_eq!(entry.logger, "test_logger");
        assert_eq!(entry.message, "Test message");
        assert!(entry.fields.contains_key("custom_field"));
    }

    #[test]
    fn test_log_entry_json() {
        let entry = StructuredLogEntry::new("ERROR", "node", "Something went wrong".to_string());
        let json = entry.to_json().unwrap();

        assert!(json.contains("\"level\":\"ERROR\""));
        assert!(json.contains("\"logger\":\"node\""));
        assert!(json.contains("Something went wrong"));
    }

    #[test]
    fn test_config_default() {
        let config = LogAggregationConfig::default();
        assert_eq!(config.backend, LogBackend::Console);
        assert_eq!(config.batch_size, 100);
        assert!(config.labels.contains_key("service"));
    }

    #[tokio::test]
    async fn test_aggregator_creation() {
        let config = LogAggregationConfig::default();
        let aggregator = LogAggregator::new(config);

        let entry = StructuredLogEntry::new("INFO", "test", "Test".to_string());
        aggregator.log(entry).await;
    }
}
