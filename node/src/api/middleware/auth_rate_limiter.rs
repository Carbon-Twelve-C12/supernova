//! Authentication rate limiting middleware
//!
//! This module provides specialized rate limiting for authentication attempts
//! to prevent brute force attacks on API keys.

use actix_web::{http::StatusCode, HttpResponse, ResponseError};
use serde_json::json;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};
use tracing::{error, warn};

/// Authentication attempt tracking
#[derive(Debug, Clone)]
pub struct AuthAttempt {
    /// Timestamp of first failed attempt
    first_attempt: Instant,
    /// Number of failed attempts
    failed_count: u32,
    /// Whether this IP is temporarily blocked
    blocked_until: Option<Instant>,
}

/// Authentication rate limiter configuration
#[derive(Debug, Clone)]
pub struct AuthRateLimiterConfig {
    /// Maximum failed attempts before temporary block
    pub max_failed_attempts: u32,
    /// Time window for counting failed attempts (seconds)
    pub attempt_window_secs: u64,
    /// Block duration after max failures (seconds)
    pub block_duration_secs: u64,
    /// Maximum authentication attempts per minute (successful or failed)
    pub max_attempts_per_minute: u32,
}

impl Default for AuthRateLimiterConfig {
    fn default() -> Self {
        Self {
            max_failed_attempts: 5,      // 5 failed attempts
            attempt_window_secs: 300,    // within 5 minutes
            block_duration_secs: 3600,   // blocks for 1 hour
            max_attempts_per_minute: 10, // max 10 auth attempts per minute
        }
    }
}

/// Authentication rate limiter
pub struct AuthRateLimiter {
    /// Failed attempt tracking by IP
    failed_attempts: Arc<RwLock<HashMap<String, AuthAttempt>>>,
    /// Configuration
    config: AuthRateLimiterConfig,
}

impl AuthRateLimiter {
    /// Create a new authentication rate limiter
    pub fn new(config: AuthRateLimiterConfig) -> Self {
        Self {
            failed_attempts: Arc::new(RwLock::new(HashMap::new())),
            config,
        }
    }

    /// Check if an IP is currently blocked
    pub fn is_blocked(&self, ip: &str) -> bool {
        let now = Instant::now();
        let attempts = match self.failed_attempts.read() {
            Ok(a) => a,
            Err(e) => {
                // On lock poisoning, assume blocked for safety
                tracing::error!("Failed to acquire read lock: {}", e);
                return true;
            }
        };

        if let Some(attempt) = attempts.get(ip) {
            if let Some(blocked_until) = attempt.blocked_until {
                return now < blocked_until;
            }
        }

        false
    }

    /// Record a failed authentication attempt
    pub fn record_failed_attempt(&self, ip: &str) {
        let now = Instant::now();
        let mut attempts = match self.failed_attempts.write() {
            Ok(a) => a,
            Err(e) => {
                tracing::error!("Failed to acquire write lock: {}", e);
                return;
            }
        };

        let attempt = attempts
            .entry(ip.to_string())
            .or_insert_with(|| AuthAttempt {
                first_attempt: now,
                failed_count: 0,
                blocked_until: None,
            });

        // Reset counter if window expired
        if now.duration_since(attempt.first_attempt)
            > Duration::from_secs(self.config.attempt_window_secs)
        {
            attempt.first_attempt = now;
            attempt.failed_count = 0;
        }

        attempt.failed_count += 1;

        // Block if max failures reached
        if attempt.failed_count >= self.config.max_failed_attempts {
            attempt.blocked_until =
                Some(now + Duration::from_secs(self.config.block_duration_secs));
            error!(
                "SECURITY: IP {} blocked for {} seconds after {} failed authentication attempts",
                ip, self.config.block_duration_secs, attempt.failed_count
            );
        } else {
            warn!(
                "Failed authentication attempt from IP {} ({}/{} attempts)",
                ip, attempt.failed_count, self.config.max_failed_attempts
            );
        }
    }

    /// Record a successful authentication
    pub fn record_successful_auth(&self, ip: &str) {
        let mut attempts = match self.failed_attempts.write() {
            Ok(a) => a,
            Err(e) => {
                tracing::error!("Failed to acquire write lock: {}", e);
                return;
            }
        };
        attempts.remove(ip);
    }

    /// Clean up expired entries
    pub fn cleanup(&self) {
        let now = Instant::now();
        let mut attempts = match self.failed_attempts.write() {
            Ok(a) => a,
            Err(e) => {
                tracing::error!("Failed to acquire write lock during cleanup: {}", e);
                return;
            }
        };

        attempts.retain(|_, attempt| {
            // Keep if blocked and block hasn't expired
            if let Some(blocked_until) = attempt.blocked_until {
                return now < blocked_until;
            }

            // Keep if within attempt window
            now.duration_since(attempt.first_attempt)
                < Duration::from_secs(self.config.attempt_window_secs)
        });
    }
}

/// Error response for blocked IPs
#[derive(Debug)]
pub struct AuthBlockedError {
    pub block_duration_secs: u64,
}

impl std::fmt::Display for AuthBlockedError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Too many failed authentication attempts. IP blocked for {} seconds.",
            self.block_duration_secs
        )
    }
}

impl ResponseError for AuthBlockedError {
    fn status_code(&self) -> StatusCode {
        StatusCode::TOO_MANY_REQUESTS
    }

    fn error_response(&self) -> HttpResponse {
        let mut res = HttpResponse::TooManyRequests();
        res.insert_header(("Retry-After", self.block_duration_secs.to_string()));

        res.json(json!({
            "success": false,
            "error": self.to_string(),
            "retry_after": self.block_duration_secs
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_auth_rate_limiter_blocking() {
        let config = AuthRateLimiterConfig {
            max_failed_attempts: 3,
            attempt_window_secs: 60,
            block_duration_secs: 300,
            max_attempts_per_minute: 10,
        };

        let limiter = AuthRateLimiter::new(config);
        let test_ip = "192.168.1.1";

        // Should not be blocked initially
        assert!(!limiter.is_blocked(test_ip));

        // Record failed attempts
        for _ in 0..3 {
            limiter.record_failed_attempt(test_ip);
        }

        // Should now be blocked
        assert!(limiter.is_blocked(test_ip));

        // Different IP should not be blocked
        assert!(!limiter.is_blocked("192.168.1.2"));
    }

    #[test]
    fn test_auth_rate_limiter_reset_on_success() {
        let config = AuthRateLimiterConfig::default();
        let limiter = AuthRateLimiter::new(config);
        let test_ip = "192.168.1.1";

        // Record some failed attempts
        limiter.record_failed_attempt(test_ip);
        limiter.record_failed_attempt(test_ip);

        // Record successful auth
        limiter.record_successful_auth(test_ip);

        // Counter should be reset, so we can fail more times
        for _ in 0..4 {
            limiter.record_failed_attempt(test_ip);
        }

        // Still not blocked (would be if counter wasn't reset)
        assert!(!limiter.is_blocked(test_ip));
    }
}
