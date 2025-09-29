//! Network Rate Limiting Module for Supernova
//!
//! This module provides comprehensive rate limiting for network operations
//! to prevent DoS attacks and ensure fair resource usage.

use serde_json;
use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};
use thiserror::Error;
use tokio::sync::Semaphore;
use tracing::{debug, error, warn};

/// Rate limiting errors
#[derive(Debug, Error)]
pub enum RateLimitError {
    #[error("Rate limit exceeded for {0}: {1} requests in {2:?}")]
    RateLimitExceeded(IpAddr, usize, Duration),

    #[error("IP {0} is banned until {1:?}")]
    IpBanned(IpAddr, Instant),

    #[error("Subnet {0} rate limit exceeded")]
    SubnetRateLimitExceeded(String),

    #[error("Global rate limit exceeded")]
    GlobalRateLimitExceeded,
}

/// Configuration for rate limiting
#[derive(Debug, Clone)]
pub struct RateLimitConfig {
    /// Maximum requests per IP per window
    pub per_ip_limit: usize,
    /// Window duration for IP rate limiting
    pub ip_window: Duration,
    /// Maximum requests per subnet per window
    pub per_subnet_limit: usize,
    /// Window duration for subnet rate limiting
    pub subnet_window: Duration,
    /// Ban duration for repeated violations
    pub ban_duration: Duration,
    /// Number of violations before ban
    pub violations_before_ban: usize,
    /// Global rate limit (total requests per second)
    pub global_rps: usize,
    /// Maximum concurrent connections
    pub max_concurrent_connections: usize,
    /// Enable circuit breaker
    pub circuit_breaker_enabled: bool,
    /// Circuit breaker threshold (error rate)
    pub circuit_breaker_threshold: f64,
    /// Circuit breaker reset timeout
    pub circuit_breaker_timeout: Duration,
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            per_ip_limit: 100,
            ip_window: Duration::from_secs(60),
            per_subnet_limit: 1000,
            subnet_window: Duration::from_secs(60),
            ban_duration: Duration::from_secs(3600), // 1 hour
            violations_before_ban: 5,
            global_rps: 10000,
            max_concurrent_connections: 1000,
            circuit_breaker_enabled: true,
            circuit_breaker_threshold: 0.5,
            circuit_breaker_timeout: Duration::from_secs(30),
        }
    }
}

/// Rate limit tracking for an IP address
#[derive(Debug)]
struct IpRateLimit {
    /// Request timestamps within the current window
    requests: Vec<Instant>,
    /// Number of violations
    violations: usize,
    /// Ban expiry time if banned
    banned_until: Option<Instant>,
    /// Last cleanup time
    last_cleanup: Instant,
}

impl IpRateLimit {
    fn new() -> Self {
        Self {
            requests: Vec::new(),
            violations: 0,
            banned_until: None,
            last_cleanup: Instant::now(),
        }
    }

    /// Clean up old requests outside the window
    fn cleanup(&mut self, window: Duration) {
        let now = Instant::now();
        if now.duration_since(self.last_cleanup) > window {
            self.requests.retain(|&t| now.duration_since(t) <= window);
            self.last_cleanup = now;
        }
    }

    /// Check if currently banned
    fn is_banned(&self) -> bool {
        self.banned_until.map_or(false, |t| Instant::now() < t)
    }

    /// Record a request
    fn record_request(&mut self, window: Duration) -> Result<(), RateLimitError> {
        self.cleanup(window);
        self.requests.push(Instant::now());
        Ok(())
    }

    /// Get current request count
    fn request_count(&self, window: Duration) -> usize {
        let now = Instant::now();
        self.requests
            .iter()
            .filter(|&&t| now.duration_since(t) <= window)
            .count()
    }
}

/// Circuit breaker state
#[derive(Debug)]
enum CircuitState {
    Closed,
    Open { until: Instant },
    HalfOpen,
}

/// Circuit breaker for automatic failure recovery
#[derive(Debug)]
struct CircuitBreaker {
    state: CircuitState,
    failure_count: usize,
    success_count: usize,
    total_count: usize,
    last_reset: Instant,
    config: RateLimitConfig,
}

impl CircuitBreaker {
    fn new(config: RateLimitConfig) -> Self {
        Self {
            state: CircuitState::Closed,
            failure_count: 0,
            success_count: 0,
            total_count: 0,
            last_reset: Instant::now(),
            config,
        }
    }

    /// Check if requests are allowed
    fn is_allowed(&mut self) -> bool {
        match &self.state {
            CircuitState::Closed => true,
            CircuitState::Open { until } => {
                if Instant::now() >= *until {
                    self.state = CircuitState::HalfOpen;
                    true
                } else {
                    false
                }
            }
            CircuitState::HalfOpen => true,
        }
    }

    /// Record a successful request
    fn record_success(&mut self) {
        self.success_count += 1;
        self.total_count += 1;

        if let CircuitState::HalfOpen = self.state {
            // Close circuit after successful request in half-open state
            self.state = CircuitState::Closed;
            self.reset_counters();
        }
    }

    /// Record a failed request
    fn record_failure(&mut self) {
        self.failure_count += 1;
        self.total_count += 1;

        // Check if we should open the circuit
        if self.total_count > 10 {
            let error_rate = self.failure_count as f64 / self.total_count as f64;
            if error_rate >= self.config.circuit_breaker_threshold {
                self.state = CircuitState::Open {
                    until: Instant::now() + self.config.circuit_breaker_timeout,
                };
                warn!(
                    "Circuit breaker opened due to high error rate: {:.2}%",
                    error_rate * 100.0
                );
            }
        }

        // Reset counters periodically
        if Instant::now().duration_since(self.last_reset) > Duration::from_secs(300) {
            self.reset_counters();
        }
    }

    fn reset_counters(&mut self) {
        self.failure_count = 0;
        self.success_count = 0;
        self.total_count = 0;
        self.last_reset = Instant::now();
    }
}

/// Network rate limiter
pub struct NetworkRateLimiter {
    config: RateLimitConfig,
    /// Per-IP rate limits
    ip_limits: Arc<RwLock<HashMap<IpAddr, IpRateLimit>>>,
    /// Per-subnet rate limits (/24 for IPv4, /64 for IPv6)
    subnet_limits: Arc<RwLock<HashMap<String, IpRateLimit>>>,
    /// Global rate limiting semaphore
    global_semaphore: Arc<Semaphore>,
    /// Connection limiting semaphore
    connection_semaphore: Arc<Semaphore>,
    /// Circuit breaker
    circuit_breaker: Arc<RwLock<CircuitBreaker>>,
    /// Metrics
    metrics: Arc<RwLock<RateLimitMetrics>>,
}

/// Rate limiting metrics
#[derive(Debug, Default, Clone)]
pub struct RateLimitMetrics {
    pub total_requests: u64,
    pub rejected_requests: u64,
    pub banned_ips: usize,
    pub circuit_breaker_trips: u64,
    pub active_connections: usize,
    pub rate_limited_peers: usize,
    pub peak_connections: usize,
}

impl NetworkRateLimiter {
    /// Create a new network rate limiter
    pub fn new(config: RateLimitConfig) -> Self {
        let global_semaphore = Arc::new(Semaphore::new(config.global_rps));
        let connection_semaphore = Arc::new(Semaphore::new(config.max_concurrent_connections));

        Self {
            config: config.clone(),
            ip_limits: Arc::new(RwLock::new(HashMap::new())),
            subnet_limits: Arc::new(RwLock::new(HashMap::new())),
            global_semaphore,
            connection_semaphore,
            circuit_breaker: Arc::new(RwLock::new(CircuitBreaker::new(config))),
            metrics: Arc::new(RwLock::new(RateLimitMetrics::default())),
        }
    }

    /// Check rate limit for an incoming connection
    pub async fn check_connection(
        &self,
        addr: SocketAddr,
    ) -> Result<ConnectionPermit, RateLimitError> {
        let ip = addr.ip();

        // Update metrics
        {
            if let Ok(mut metrics) = self.metrics.write() {
                metrics.total_requests += 1;
            }
            // Continue even if metrics lock fails
        }

        // Check circuit breaker
        if self.config.circuit_breaker_enabled {
            match self.circuit_breaker.write() {
                Ok(mut breaker) => {
                    if !breaker.is_allowed() {
                        self.record_rejection();
                        return Err(RateLimitError::GlobalRateLimitExceeded);
                    }
                }
                Err(_) => {
                    // If lock fails, allow the request but log warning
                    warn!("Circuit breaker lock poisoned, allowing request");
                }
            }
        }

        // Check IP rate limit
        self.check_ip_limit(ip)?;

        // Check subnet rate limit
        self.check_subnet_limit(ip)?;

        // Acquire global rate limit permit
        let global_permit = self
            .global_semaphore
            .clone()
            .try_acquire_owned()
            .map_err(|_| {
                self.record_rejection();
                RateLimitError::GlobalRateLimitExceeded
            })?;

        // Acquire connection limit permit
        let connection_permit = self
            .connection_semaphore
            .clone()
            .try_acquire_owned()
            .map_err(|_| {
                self.record_rejection();
                RateLimitError::GlobalRateLimitExceeded
            })?;

        Ok(ConnectionPermit {
            _global: global_permit,
            _connection: connection_permit,
            rate_limiter: self.clone(),
        })
    }

    /// Check IP-specific rate limit
    fn check_ip_limit(&self, ip: IpAddr) -> Result<(), RateLimitError> {
        let mut limits = self.ip_limits.write().map_err(|_| {
            warn!("IP limits lock poisoned");
            RateLimitError::GlobalRateLimitExceeded
        })?;
        let limit = limits.entry(ip).or_insert_with(IpRateLimit::new);

        // Check if banned
        if limit.is_banned() {
            let banned_until = limit
                .banned_until
                .expect("is_banned() returned true but banned_until is None");
            return Err(RateLimitError::IpBanned(ip, banned_until));
        }

        // Check rate limit
        limit.cleanup(self.config.ip_window);
        let count = limit.request_count(self.config.ip_window);

        if count >= self.config.per_ip_limit {
            limit.violations += 1;

            // Ban if too many violations
            if limit.violations >= self.config.violations_before_ban {
                limit.banned_until = Some(Instant::now() + self.config.ban_duration);
                if let Ok(mut metrics) = self.metrics.write() {
                    metrics.banned_ips += 1;
                }
                warn!("Banned IP {} for repeated rate limit violations", ip);
            }

            return Err(RateLimitError::RateLimitExceeded(
                ip,
                count,
                self.config.ip_window,
            ));
        }

        // Record the request
        limit.record_request(self.config.ip_window)?;
        Ok(())
    }

    /// Check subnet rate limit
    fn check_subnet_limit(&self, ip: IpAddr) -> Result<(), RateLimitError> {
        let subnet = match ip {
            IpAddr::V4(ipv4) => {
                // /24 subnet for IPv4
                let octets = ipv4.octets();
                format!("{}.{}.{}.0/24", octets[0], octets[1], octets[2])
            }
            IpAddr::V6(ipv6) => {
                // /64 subnet for IPv6
                let segments = ipv6.segments();
                format!(
                    "{:x}:{:x}:{:x}:{:x}::/64",
                    segments[0], segments[1], segments[2], segments[3]
                )
            }
        };

        let mut limits = self.subnet_limits.write().map_err(|_| {
            warn!("Subnet limits lock poisoned");
            RateLimitError::GlobalRateLimitExceeded
        })?;
        let limit = limits
            .entry(subnet.clone())
            .or_insert_with(IpRateLimit::new);

        limit.cleanup(self.config.subnet_window);
        let count = limit.request_count(self.config.subnet_window);

        if count >= self.config.per_subnet_limit {
            return Err(RateLimitError::SubnetRateLimitExceeded(subnet));
        }

        limit.record_request(self.config.subnet_window)?;
        Ok(())
    }

    /// Record a successful operation
    pub fn record_success(&self) {
        if self.config.circuit_breaker_enabled {
            if let Ok(mut breaker) = self.circuit_breaker.write() {
                breaker.record_success();
            }
        }
    }

    /// Record a failed operation
    pub fn record_failure(&self) {
        if self.config.circuit_breaker_enabled {
            if let Ok(mut breaker) = self.circuit_breaker.write() {
                breaker.record_failure();
            }
        }
    }

    /// Record a rejection
    fn record_rejection(&self) {
        if let Ok(mut metrics) = self.metrics.write() {
            metrics.rejected_requests += 1;
        }
    }

    /// Get current metrics
    pub fn metrics(&self) -> RateLimitMetrics {
        self.metrics.read().map(|m| m.clone()).unwrap_or_else(|_| {
            warn!("Metrics lock poisoned, returning default");
            RateLimitMetrics::default()
        })
    }

    /// Clean up old entries
    pub fn cleanup(&self) {
        // Clean up IP limits
        {
            if let Ok(mut limits) = self.ip_limits.write() {
                let now = Instant::now();
                limits.retain(|_, limit| {
                    // Keep if banned or has recent requests
                    limit.is_banned()
                        || limit
                            .requests
                            .iter()
                            .any(|&t| now.duration_since(t) <= self.config.ip_window * 2)
                });
            }
        }

        // Clean up subnet limits
        {
            if let Ok(mut limits) = self.subnet_limits.write() {
                let now = Instant::now();
                limits.retain(|_, limit| {
                    limit
                        .requests
                        .iter()
                        .any(|&t| now.duration_since(t) <= self.config.subnet_window * 2)
                });
            }
        }

        debug!("Rate limiter cleanup completed");
    }

    /// Get current rate limit metrics as JSON
    pub fn get_metrics_json(&self) -> serde_json::Value {
        let metrics = match self.metrics.read() {
            Ok(m) => m.clone(),
            Err(_) => {
                warn!("Metrics lock poisoned, returning default JSON");
                RateLimitMetrics::default()
            }
        };
        serde_json::json!({
            "total_requests": metrics.total_requests,
            "rejected_requests": metrics.rejected_requests,
            "active_connections": metrics.active_connections,
            "rate_limited_peers": metrics.rate_limited_peers,
            "peak_connections": metrics.peak_connections,
            "rejection_rate": if metrics.total_requests > 0 {
                metrics.rejected_requests as f64 / metrics.total_requests as f64
            } else {
                0.0
            }
        })
    }
}

impl Clone for NetworkRateLimiter {
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            ip_limits: self.ip_limits.clone(),
            subnet_limits: self.subnet_limits.clone(),
            global_semaphore: self.global_semaphore.clone(),
            connection_semaphore: self.connection_semaphore.clone(),
            circuit_breaker: self.circuit_breaker.clone(),
            metrics: self.metrics.clone(),
        }
    }
}

/// Permit for a connection that passed rate limiting
pub struct ConnectionPermit {
    _global: tokio::sync::OwnedSemaphorePermit,
    _connection: tokio::sync::OwnedSemaphorePermit,
    rate_limiter: NetworkRateLimiter,
}

impl ConnectionPermit {
    /// Record success when the connection completes successfully
    pub fn record_success(self) {
        self.rate_limiter.record_success();
    }

    /// Record failure when the connection fails
    pub fn record_failure(self) {
        self.rate_limiter.record_failure();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{Ipv4Addr, Ipv6Addr};

    #[tokio::test]
    async fn test_ip_rate_limiting() {
        let config = RateLimitConfig {
            per_ip_limit: 5,
            ip_window: Duration::from_secs(1),
            ..Default::default()
        };

        let limiter = NetworkRateLimiter::new(config);
        let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1)), 8080);

        // First 5 requests should succeed
        for _ in 0..5 {
            assert!(limiter.check_connection(addr).await.is_ok());
        }

        // 6th request should fail
        assert!(matches!(
            limiter.check_connection(addr).await,
            Err(RateLimitError::RateLimitExceeded(_, _, _))
        ));
    }

    #[tokio::test]
    async fn test_subnet_rate_limiting() {
        let config = RateLimitConfig {
            per_ip_limit: 100,
            per_subnet_limit: 10,
            subnet_window: Duration::from_secs(1),
            ..Default::default()
        };

        let limiter = NetworkRateLimiter::new(config);

        // Different IPs in same subnet
        for i in 1..=10 {
            let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 168, 1, i)), 8080);
            assert!(limiter.check_connection(addr).await.is_ok());
        }

        // 11th request from same subnet should fail
        let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 50)), 8080);
        assert!(matches!(
            limiter.check_connection(addr).await,
            Err(RateLimitError::SubnetRateLimitExceeded(_))
        ));
    }

    #[tokio::test]
    async fn test_ban_mechanism() {
        let config = RateLimitConfig {
            per_ip_limit: 2,
            ip_window: Duration::from_secs(1),
            violations_before_ban: 2,
            ban_duration: Duration::from_secs(2),
            ..Default::default()
        };

        let limiter = NetworkRateLimiter::new(config);
        let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1)), 8080);

        // First violation
        for _ in 0..2 {
            let _ = limiter.check_connection(addr).await;
        }
        assert!(limiter.check_connection(addr).await.is_err());

        // Wait and try again (second violation should trigger ban)
        tokio::time::sleep(Duration::from_millis(1100)).await;
        for _ in 0..2 {
            let _ = limiter.check_connection(addr).await;
        }

        // Now should be banned
        match limiter.check_connection(addr).await {
            Err(RateLimitError::IpBanned(_, _)) => {}
            _ => panic!("Expected IP to be banned"),
        }
    }

    #[tokio::test]
    async fn test_global_rate_limit() {
        let config = RateLimitConfig {
            per_ip_limit: 1000,
            global_rps: 5,
            ..Default::default()
        };

        let limiter = NetworkRateLimiter::new(config);
        let mut permits = Vec::new();

        // Acquire all permits
        for i in 0..5 {
            let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 168, 1, i)), 8080);
            permits.push(limiter.check_connection(addr).await.unwrap());
        }

        // Next request should fail
        let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 100)), 8080);
        assert!(matches!(
            limiter.check_connection(addr).await,
            Err(RateLimitError::GlobalRateLimitExceeded)
        ));

        // Drop a permit and try again
        permits.pop();
        assert!(limiter.check_connection(addr).await.is_ok());
    }

    #[test]
    fn test_circuit_breaker() {
        let config = RateLimitConfig {
            circuit_breaker_enabled: true,
            circuit_breaker_threshold: 0.5,
            circuit_breaker_timeout: Duration::from_secs(1),
            ..Default::default()
        };

        let mut breaker = CircuitBreaker::new(config);

        // Record some successes and failures
        for _ in 0..6 {
            breaker.record_success();
        }
        for _ in 0..6 {
            breaker.record_failure();
        }

        // Circuit should be open now (50% error rate)
        assert!(!breaker.is_allowed());

        // Wait for timeout
        std::thread::sleep(Duration::from_secs(1));

        // Should be half-open now
        assert!(breaker.is_allowed());

        // Success should close it
        breaker.record_success();
        assert!(breaker.is_allowed());
    }
}
