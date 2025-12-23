//! Distributed Tracing Module (OpenTelemetry Integration)
//!
//! Provides comprehensive distributed tracing capabilities:
//! - OpenTelemetry protocol support
//! - Trace context propagation
//! - Span creation and management
//! - Metrics export to various backends

mod tracer;
mod propagation;
mod exporter;

pub use tracer::{Tracer, TracerConfig, TracingMiddleware};
pub use propagation::{TraceContext, TraceContextPropagator};
pub use exporter::{TelemetryExporter, ExporterConfig, ExporterType};
