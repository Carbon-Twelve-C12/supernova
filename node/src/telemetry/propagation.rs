//! Trace Context Propagation
//!
//! Implements W3C Trace Context propagation for distributed tracing.

use super::tracer::{SpanId, TraceId};
use std::collections::HashMap;

/// W3C Trace Context header names
pub const TRACEPARENT_HEADER: &str = "traceparent";
pub const TRACESTATE_HEADER: &str = "tracestate";

/// Trace context for propagation across service boundaries
#[derive(Debug, Clone)]
pub struct TraceContext {
    /// Version of the trace context format
    pub version: u8,
    /// Trace ID
    pub trace_id: TraceId,
    /// Parent span ID
    pub parent_span_id: SpanId,
    /// Trace flags
    pub flags: TraceFlags,
    /// Additional vendor-specific state
    pub trace_state: Option<String>,
}

/// Trace flags
#[derive(Debug, Clone, Copy, Default)]
pub struct TraceFlags(u8);

impl TraceFlags {
    /// No flags set
    pub const NONE: Self = Self(0x00);
    /// Sampled flag
    pub const SAMPLED: Self = Self(0x01);

    /// Check if sampled flag is set
    pub fn is_sampled(&self) -> bool {
        self.0 & 0x01 != 0
    }

    /// Set sampled flag
    pub fn set_sampled(&mut self, sampled: bool) {
        if sampled {
            self.0 |= 0x01;
        } else {
            self.0 &= !0x01;
        }
    }

    /// Get raw flags value
    pub fn as_u8(&self) -> u8 {
        self.0
    }
}

impl TraceContext {
    /// Create a new trace context
    pub fn new(trace_id: TraceId, parent_span_id: SpanId, sampled: bool) -> Self {
        let mut flags = TraceFlags::default();
        flags.set_sampled(sampled);

        Self {
            version: 0,
            trace_id,
            parent_span_id,
            flags,
            trace_state: None,
        }
    }

    /// Parse from W3C traceparent header
    pub fn from_traceparent(header: &str) -> Option<Self> {
        let parts: Vec<&str> = header.split('-').collect();
        if parts.len() != 4 {
            return None;
        }

        let version = u8::from_str_radix(parts[0], 16).ok()?;
        if version > 0 && parts.len() < 4 {
            return None; // Future versions may have more fields
        }

        let trace_id = TraceId::from_hex(parts[1])?;
        let parent_span_id = SpanId::from_hex(parts[2])?;
        let flags = u8::from_str_radix(parts[3], 16).ok()?;

        Some(Self {
            version,
            trace_id,
            parent_span_id,
            flags: TraceFlags(flags),
            trace_state: None,
        })
    }

    /// Convert to W3C traceparent header
    pub fn to_traceparent(&self) -> String {
        format!(
            "{:02x}-{}-{}-{:02x}",
            self.version,
            self.trace_id.to_hex(),
            self.parent_span_id.to_hex(),
            self.flags.as_u8()
        )
    }

    /// Set trace state
    pub fn with_trace_state(mut self, state: String) -> Self {
        self.trace_state = Some(state);
        self
    }
}

/// Propagator for injecting and extracting trace context
pub struct TraceContextPropagator;

impl TraceContextPropagator {
    /// Inject trace context into headers
    pub fn inject(context: &TraceContext, headers: &mut HashMap<String, String>) {
        headers.insert(TRACEPARENT_HEADER.to_string(), context.to_traceparent());
        if let Some(ref state) = context.trace_state {
            headers.insert(TRACESTATE_HEADER.to_string(), state.clone());
        }
    }

    /// Extract trace context from headers
    pub fn extract(headers: &HashMap<String, String>) -> Option<TraceContext> {
        let traceparent = headers.get(TRACEPARENT_HEADER)?;
        let mut context = TraceContext::from_traceparent(traceparent)?;

        if let Some(tracestate) = headers.get(TRACESTATE_HEADER) {
            context.trace_state = Some(tracestate.clone());
        }

        Some(context)
    }

    /// Extract trace context from HTTP headers (case-insensitive)
    pub fn extract_from_http<'a, I>(headers: I) -> Option<TraceContext>
    where
        I: IntoIterator<Item = (&'a str, &'a str)>,
    {
        let mut traceparent: Option<&str> = None;
        let mut tracestate: Option<&str> = None;

        for (key, value) in headers {
            let key_lower = key.to_lowercase();
            if key_lower == TRACEPARENT_HEADER {
                traceparent = Some(value);
            } else if key_lower == TRACESTATE_HEADER {
                tracestate = Some(value);
            }
        }

        let mut context = TraceContext::from_traceparent(traceparent?)?;
        if let Some(state) = tracestate {
            context.trace_state = Some(state.to_string());
        }

        Some(context)
    }
}

/// B3 propagation format (Zipkin)
pub struct B3Propagator;

impl B3Propagator {
    /// B3 header names
    pub const TRACE_ID: &'static str = "X-B3-TraceId";
    pub const SPAN_ID: &'static str = "X-B3-SpanId";
    pub const SAMPLED: &'static str = "X-B3-Sampled";
    pub const PARENT_SPAN_ID: &'static str = "X-B3-ParentSpanId";

    /// Inject B3 headers
    pub fn inject(context: &TraceContext, headers: &mut HashMap<String, String>) {
        headers.insert(Self::TRACE_ID.to_string(), context.trace_id.to_hex());
        headers.insert(Self::SPAN_ID.to_string(), context.parent_span_id.to_hex());
        headers.insert(
            Self::SAMPLED.to_string(),
            if context.flags.is_sampled() { "1" } else { "0" }.to_string(),
        );
    }

    /// Extract B3 headers
    pub fn extract(headers: &HashMap<String, String>) -> Option<TraceContext> {
        let trace_id = TraceId::from_hex(headers.get(Self::TRACE_ID)?)?;
        let span_id = SpanId::from_hex(headers.get(Self::SPAN_ID)?)?;
        let sampled = headers
            .get(Self::SAMPLED)
            .map(|s| s == "1" || s.to_lowercase() == "true")
            .unwrap_or(true);

        Some(TraceContext::new(trace_id, span_id, sampled))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_traceparent_roundtrip() {
        let context = TraceContext::new(TraceId::new(), SpanId::new(), true);
        let header = context.to_traceparent();
        let parsed = TraceContext::from_traceparent(&header).unwrap();

        assert_eq!(context.trace_id, parsed.trace_id);
        assert_eq!(context.parent_span_id, parsed.parent_span_id);
        assert_eq!(context.flags.is_sampled(), parsed.flags.is_sampled());
    }

    #[test]
    fn test_traceparent_parse() {
        let header = "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01";
        let context = TraceContext::from_traceparent(header).unwrap();

        assert_eq!(context.version, 0);
        assert!(context.flags.is_sampled());
    }

    #[test]
    fn test_trace_flags() {
        let mut flags = TraceFlags::NONE;
        assert!(!flags.is_sampled());

        flags.set_sampled(true);
        assert!(flags.is_sampled());

        flags.set_sampled(false);
        assert!(!flags.is_sampled());
    }

    #[test]
    fn test_propagator_inject_extract() {
        let context = TraceContext::new(TraceId::new(), SpanId::new(), true);
        let mut headers = HashMap::new();

        TraceContextPropagator::inject(&context, &mut headers);
        let extracted = TraceContextPropagator::extract(&headers).unwrap();

        assert_eq!(context.trace_id, extracted.trace_id);
    }

    #[test]
    fn test_b3_propagator() {
        let context = TraceContext::new(TraceId::new(), SpanId::new(), true);
        let mut headers = HashMap::new();

        B3Propagator::inject(&context, &mut headers);
        let extracted = B3Propagator::extract(&headers).unwrap();

        assert_eq!(context.trace_id, extracted.trace_id);
        assert!(extracted.flags.is_sampled());
    }
}
