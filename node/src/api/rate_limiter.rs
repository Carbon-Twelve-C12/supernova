//! API Rate Limiting - DoS Protection
//!
//! Rate limiting for API endpoints
//! 
//! This module prevents denial-of-service attacks through API flooding
//! by implementing per-IP and per-endpoint rate limiting using a token bucket algorithm.

use dashmap::DashMap;
use std::net::IpAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

// ============================================================================
// API Rate Limiting Configuration
// ============================================================================

/// API rate limiting configuration
pub struct ApiRateLimitConfig;

impl ApiRateLimitConfig {
    /// Maximum requests per IP per minute (global limit)
    /// 
    /// SECURITY: Prevents single IP from flooding all endpoints.
    pub const MAX_REQUESTS_PER_IP_PER_MINUTE: usize = 60;
    
    /// Maximum requests per endpoint per IP per minute
    /// 
    /// SECURITY: Prevents targeting specific expensive endpoints.
    pub const MAX_REQUESTS_PER_ENDPOINT_PER_MINUTE: usize = 30;
    
    /// Rate limit window duration
    pub const RATE_LIMIT_WINDOW: Duration = Duration::from_secs(60); // 1 minute
    
    /// Maximum concurrent requests per IP
    /// 
    /// SECURITY: Prevents request queue saturation.
    pub const MAX_CONCURRENT_REQUESTS_PER_IP: usize = 5;
    
    /// Expensive endpoint multiplier
    /// 
    /// Endpoints like `generate` count as multiple requests.
    pub const EXPENSIVE_ENDPOINT_MULTIPLIER: usize = 10;
    
    /// Batch request maximum size
    /// 
    /// SECURITY: Prevents batch request abuse.
    pub const MAX_BATCH_SIZE: usize = 10;
}

/// Per-IP rate limit state using token bucket algorithm
#[derive(Debug)]
struct IpRateLimit {
    /// Tokens available (requests allowed)
    tokens: usize,
    /// Last token refill time
    last_refill: Instant,
    /// Request count in current window
    request_count: usize,
    /// Concurrent request count
    concurrent_requests: usize,
}

impl IpRateLimit {
    fn new() -> Self {
        Self {
            tokens: ApiRateLimitConfig::MAX_REQUESTS_PER_IP_PER_MINUTE,
            last_refill: Instant::now(),
            request_count: 0,
            concurrent_requests: 0,
        }
    }
    
    /// Check and consume tokens for a request
    /// 
    /// Returns true if request allowed, false if rate limited
    fn check_and_consume(&mut self, cost: usize) -> bool {
        let now = Instant::now();
        
        // Refill tokens if window expired
        if now.duration_since(self.last_refill) >= ApiRateLimitConfig::RATE_LIMIT_WINDOW {
            self.tokens = ApiRateLimitConfig::MAX_REQUESTS_PER_IP_PER_MINUTE;
            self.last_refill = now;
            self.request_count = 0;
        }
        
        // Check if we have enough tokens
        if self.tokens >= cost {
            self.tokens -= cost;
            self.request_count += 1;
            true
        } else {
            false
        }
    }
}

/// Per-endpoint rate limit tracker
#[derive(Debug)]
struct EndpointRateLimit {
    /// Request count in current window
    request_count: usize,
    /// Window start time
    window_start: Instant,
}

impl EndpointRateLimit {
    fn new() -> Self {
        Self {
            request_count: 0,
            window_start: Instant::now(),
        }
    }
    
    fn check_and_update(&mut self) -> bool {
        let now = Instant::now();
        
        // Reset window if expired
        if now.duration_since(self.window_start) >= ApiRateLimitConfig::RATE_LIMIT_WINDOW {
            self.request_count = 0;
            self.window_start = now;
        }
        
        // Check if under limit
        if self.request_count < ApiRateLimitConfig::MAX_REQUESTS_PER_ENDPOINT_PER_MINUTE {
            self.request_count += 1;
            true
        } else {
            false
        }
    }
}

/// API Rate Limiter with token bucket algorithm
/// 
/// SECURITY: Multi-layered DoS protection:
/// 1. Per-IP global rate limiting
/// 2. Per-endpoint rate limiting
/// 3. Concurrent request limiting
/// 4. Expensive endpoint cost multipliers
pub struct ApiRateLimiter {
    /// Per-IP rate limits
    ip_limits: Arc<DashMap<IpAddr, IpRateLimit>>,
    
    /// Per-endpoint per-IP rate limits (key: "IP:endpoint")
    endpoint_limits: Arc<DashMap<String, EndpointRateLimit>>,
    
    /// Statistics
    total_requests: Arc<AtomicU64>,
    rate_limited_requests: Arc<AtomicU64>,
}

impl ApiRateLimiter {
    /// Create a new API rate limiter
    pub fn new() -> Self {
        Self {
            ip_limits: Arc::new(DashMap::new()),
            endpoint_limits: Arc::new(DashMap::new()),
            total_requests: Arc::new(AtomicU64::new(0)),
            rate_limited_requests: Arc::new(AtomicU64::new(0)),
        }
    }
    
    /// Check if request is allowed
    /// 
    /// SECURITY: Validates against all rate limit rules.
    ///
    /// # Arguments
    /// * `ip` - IP address of requester
    /// * `endpoint` - API endpoint being called
    /// * `is_expensive` - Whether this is an expensive operation
    ///
    /// # Returns
    /// * `Ok(())` - Request allowed
    /// * `Err(String)` - Request denied with reason
    pub fn check_rate_limit(
        &self,
        ip: IpAddr,
        endpoint: &str,
        is_expensive: bool,
    ) -> Result<(), String> {
        self.total_requests.fetch_add(1, Ordering::Relaxed);
        
        // Determine request cost
        let cost = if is_expensive {
            ApiRateLimitConfig::EXPENSIVE_ENDPOINT_MULTIPLIER
        } else {
            1
        };
        
        // Check 1: Per-IP global rate limit
        let mut ip_limit = self.ip_limits
            .entry(ip)
            .or_insert_with(IpRateLimit::new);
        
        if !ip_limit.check_and_consume(cost) {
            self.rate_limited_requests.fetch_add(1, Ordering::Relaxed);
            
            return Err(format!(
                "Rate limit exceeded: {} requests per minute maximum",
                ApiRateLimitConfig::MAX_REQUESTS_PER_IP_PER_MINUTE
            ));
        }
        
        // Check 2: Per-endpoint per-IP rate limit
        let endpoint_key = format!("{}:{}", ip, endpoint);
        let mut endpoint_limit = self.endpoint_limits
            .entry(endpoint_key.clone())
            .or_insert_with(EndpointRateLimit::new);
        
        if !endpoint_limit.check_and_update() {
            self.rate_limited_requests.fetch_add(1, Ordering::Relaxed);
            
            return Err(format!(
                "Endpoint rate limit exceeded: {} requests per minute per endpoint",
                ApiRateLimitConfig::MAX_REQUESTS_PER_ENDPOINT_PER_MINUTE
            ));
        }
        
        // Check 3: Concurrent request limit
        if ip_limit.concurrent_requests >= ApiRateLimitConfig::MAX_CONCURRENT_REQUESTS_PER_IP {
            self.rate_limited_requests.fetch_add(1, Ordering::Relaxed);
            
            return Err(format!(
                "Too many concurrent requests: {} maximum",
                ApiRateLimitConfig::MAX_CONCURRENT_REQUESTS_PER_IP
            ));
        }
        
        // Increment concurrent counter
        ip_limit.concurrent_requests += 1;
        
        Ok(())
    }
    
    /// Mark request as completed
    pub fn complete_request(&self, ip: IpAddr) {
        if let Some(mut limit) = self.ip_limits.get_mut(&ip) {
            limit.concurrent_requests = limit.concurrent_requests.saturating_sub(1);
        }
    }
    
    /// Get rate limiting statistics
    pub fn get_stats(&self) -> ApiRateLimitStats {
        ApiRateLimitStats {
            total_requests: self.total_requests.load(Ordering::Relaxed),
            rate_limited_requests: self.rate_limited_requests.load(Ordering::Relaxed),
            active_ip_limits: self.ip_limits.len(),
            active_endpoint_limits: self.endpoint_limits.len(),
        }
    }
    
    /// Clean up expired rate limit entries
    pub fn cleanup_expired(&self) {
        let now = Instant::now();
        
        // Clean up IP limits that haven't been used recently
        self.ip_limits.retain(|_, limit| {
            now.duration_since(limit.last_refill) < ApiRateLimitConfig::RATE_LIMIT_WINDOW * 10
        });
        
        // Clean up endpoint limits
        self.endpoint_limits.retain(|_, limit| {
            now.duration_since(limit.window_start) < ApiRateLimitConfig::RATE_LIMIT_WINDOW * 10
        });
    }
}

/// Rate limiting statistics
#[derive(Debug, Clone)]
pub struct ApiRateLimitStats {
    pub total_requests: u64,
    pub rate_limited_requests: u64,
    pub active_ip_limits: usize,
    pub active_endpoint_limits: usize,
}

/// List of expensive RPC methods that count as multiple requests
pub fn is_expensive_endpoint(method: &str) -> bool {
    matches!(
        method,
        "generate" | "generatetoaddress" | "getblocktemplate" | 
        "submitblock" | "sendrawtransaction" | "sendfrom"
    )
}

