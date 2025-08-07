use crate::api::types::LogEntry;
use chrono::Utc;
use std::sync::{Arc, Mutex};
use std::collections::VecDeque;

lazy_static::lazy_static! {
    static ref LOG_BUFFER: Arc<Mutex<VecDeque<LogEntry>>> = Arc::new(Mutex::new(VecDeque::with_capacity(10000)));
}

/// Get recent logs based on filters
pub fn get_recent_logs(
    level: &str,
    component: Option<&str>,
    limit: usize,
    offset: usize,
) -> Vec<LogEntry> {
    let buffer = match LOG_BUFFER.lock() {
        Ok(b) => b,
        Err(_) => return Vec::new(), // Return empty on lock poisoned
    };
    
    buffer.iter()
        .filter(|log| {
            // Filter by level
            if !level.is_empty() && log.level.to_lowercase() != level.to_lowercase() {
                return false;
            }
            
            // Filter by component if specified
            if let Some(comp) = component {
                if !log.component.contains(comp) {
                    return false;
                }
            }
            
            true
        })
        .skip(offset)
        .take(limit)
        .cloned()
        .collect()
}

/// Add a log entry to the buffer
pub fn add_log_entry(level: &str, component: &str, message: String) {
    let mut buffer = match LOG_BUFFER.lock() {
        Ok(b) => b,
        Err(_) => return, // Skip logging on lock poisoned
    };
    
    // Remove oldest entries if buffer is full
    if buffer.len() >= 10000 {
        buffer.pop_front();
    }
    
    buffer.push_back(LogEntry {
        timestamp: Utc::now().timestamp() as u64,
        level: level.to_string(),
        component: component.to_string(),
        message,
        context: None,
    });
}

/// Initialize the logging system
pub fn init_logging() {
    // Set up tracing subscriber that also writes to our buffer
    use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
    
    let fmt_layer = tracing_subscriber::fmt::layer()
        .with_target(true)
        .with_thread_ids(true)
        .with_level(true);
    
    let buffer_layer = BufferLayer;
    
    tracing_subscriber::registry()
        .with(fmt_layer)
        .with(buffer_layer)
        .init();
}

/// Custom tracing layer that writes to our log buffer
struct BufferLayer;

impl<S> tracing_subscriber::Layer<S> for BufferLayer
where
    S: tracing::Subscriber,
{
    fn on_event(
        &self,
        event: &tracing::Event<'_>,
        _ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        use tracing_subscriber::field::Visit;
        
        struct Visitor {
            message: String,
        }
        
        impl Visit for Visitor {
            fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
                if field.name() == "message" {
                    self.message = format!("{:?}", value);
                }
            }
        }
        
        let mut visitor = Visitor {
            message: String::new(),
        };
        
        event.record(&mut visitor);
        
        let level = match *event.metadata().level() {
            tracing::Level::ERROR => "ERROR",
            tracing::Level::WARN => "WARN",
            tracing::Level::INFO => "INFO",
            tracing::Level::DEBUG => "DEBUG",
            tracing::Level::TRACE => "TRACE",
        };
        
        let component = event.metadata().target();
        
        add_log_entry(level, component, visitor.message);
    }
} 