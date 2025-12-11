//! API rate limiting middleware
//!
//! This module provides rate limiting functionality for the supernova API.
//! 
//! SECURITY FIX (P0-002): Added standard X-RateLimit-* headers to all responses:
//! - X-RateLimit-Limit: Maximum requests allowed in window
//! - X-RateLimit-Remaining: Requests remaining in current window
//! - X-RateLimit-Reset: Unix timestamp when window resets
//! - Retry-After: Seconds until rate limit resets (on 429 responses)

use actix_web::{
    dev::{forward_ready, Service, ServiceRequest, ServiceResponse, Transform},
    http::{header::HeaderValue, StatusCode},
    HttpResponse, ResponseError,
};
use serde_json::json;
use std::collections::HashMap;
use std::future::{ready, Ready};
use std::rc::Rc;
use std::sync::Mutex;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tracing::warn;

/// Rate limit error
#[derive(Debug)]
pub struct RateLimitError {
    pub rate: u32,
    pub window_secs: u64,
}

impl std::fmt::Display for RateLimitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Rate limit exceeded: {} requests per {} seconds",
            self.rate, self.window_secs
        )
    }
}

impl ResponseError for RateLimitError {
    fn status_code(&self) -> StatusCode {
        StatusCode::TOO_MANY_REQUESTS
    }

    fn error_response(&self) -> HttpResponse {
        // Include Retry-After header
        let mut res = HttpResponse::TooManyRequests();
        res.insert_header(("Retry-After", self.window_secs.to_string()));

        res.json(json!({
            "success": false,
            "error": format!("Rate limit exceeded: {} requests per {} seconds", self.rate, self.window_secs),
            "retry_after": self.window_secs
        }))
    }
}

/// Entry in the rate limiter store
struct RateLimitEntry {
    /// Timestamp of first request in current window
    first_request: Instant,
    /// Unix timestamp when window resets
    window_reset_time: u64,
    /// Number of requests in current window
    count: u32,
}

/// Rate limit info to include in response headers
#[derive(Clone)]
struct RateLimitInfo {
    /// Maximum requests allowed in window
    limit: u32,
    /// Remaining requests in current window
    remaining: u32,
    /// Unix timestamp when window resets
    reset: u64,
}

/// Rate limiter state shared between requests
struct RateLimiterState {
    /// Request counts by client IP
    clients: Mutex<HashMap<String, RateLimitEntry>>,
    /// Maximum requests per window
    rate: u32,
    /// Window duration in seconds
    window_secs: u64,
}

/// Rate limiting middleware
pub struct RateLimiter {
    state: Rc<RateLimiterState>,
}

impl RateLimiter {
    /// Create a new rate limiter with the given rate (requests per minute)
    pub fn new(rate: u32) -> Self {
        Self {
            state: Rc::new(RateLimiterState {
                clients: Mutex::new(HashMap::new()),
                rate,
                window_secs: 60, // 1 minute window
            }),
        }
    }

    /// Create a new rate limiter with custom window duration
    pub fn with_window(rate: u32, window_secs: u64) -> Self {
        Self {
            state: Rc::new(RateLimiterState {
                clients: Mutex::new(HashMap::new()),
                rate,
                window_secs,
            }),
        }
    }
}

impl<S, B> Transform<S, ServiceRequest> for RateLimiter
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = actix_web::Error>,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = actix_web::Error;
    type Transform = RateLimiterMiddleware<S>;
    type InitError = ();
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(RateLimiterMiddleware {
            service,
            state: self.state.clone(),
        }))
    }
}

/// Rate limiting middleware service
pub struct RateLimiterMiddleware<S> {
    service: S,
    state: Rc<RateLimiterState>,
}

impl<S, B> Service<ServiceRequest> for RateLimiterMiddleware<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = actix_web::Error>,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = actix_web::Error;
    type Future =
        std::pin::Pin<Box<dyn std::future::Future<Output = Result<Self::Response, Self::Error>>>>;

    forward_ready!(service);

    fn call(&self, req: ServiceRequest) -> Self::Future {
        // Skip rate limiting for OPTIONS requests (pre-flight CORS)
        if req.method() == actix_web::http::Method::OPTIONS {
            let fut = self.service.call(req);
            return Box::pin(async move {
                let res = fut.await?;
                Ok(res)
            });
        }

        // Get client IP address
        let ip = req
            .connection_info()
            .peer_addr()
            .unwrap_or("unknown")
            .to_string();

        // Skip rate limiting for documentation routes
        if req.path().starts_with("/swagger-ui") || req.path().starts_with("/api-docs") {
            let fut = self.service.call(req);
            return Box::pin(async move {
                let res = fut.await?;
                Ok(res)
            });
        }

        // Check rate limit
        let now = Instant::now();
        let window = Duration::from_secs(self.state.window_secs);
        let rate = self.state.rate;
        let window_secs = self.state.window_secs;
        
        let mut clients = match self.state.clients.lock() {
            Ok(clients) => clients,
            Err(_) => {
                // Lock is poisoned, continue without rate limiting
                let fut = self.service.call(req);
                return Box::pin(async move {
                    let res = fut.await?;
                    Ok(res)
                });
            }
        };

        // Clean up expired entries
        clients.retain(|_, entry| now.duration_since(entry.first_request) < window);

        // Calculate reset time
        let current_unix_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        // Get current client's entry
        let entry = clients.entry(ip.clone()).or_insert_with(|| RateLimitEntry {
            first_request: now,
            window_reset_time: current_unix_time + window_secs,
            count: 0,
        });

        // Reset entry if window has expired
        if now.duration_since(entry.first_request) >= window {
            entry.first_request = now;
            entry.window_reset_time = current_unix_time + window_secs;
            entry.count = 0;
        }

        // Calculate rate limit info for headers
        let rate_limit_info = RateLimitInfo {
            limit: rate,
            remaining: rate.saturating_sub(entry.count + 1),
            reset: entry.window_reset_time,
        };

        // Check if rate limit exceeded
        if entry.count >= rate {
            warn!("Rate limit exceeded for client IP {}", ip);
            let error = RateLimitError {
                rate,
                window_secs,
            };
            return Box::pin(async move { Err(error.into()) });
        }

        // Increment request count
        entry.count += 1;
        drop(clients); // Release mutex

        // Forward request to next middleware/handler
        let fut = self.service.call(req);
        Box::pin(async move {
            let mut res = fut.await?;
            
            // Add rate limit headers to response (P0-002)
            let headers = res.headers_mut();
            
            if let Ok(limit_val) = HeaderValue::from_str(&rate_limit_info.limit.to_string()) {
                headers.insert(
                    actix_web::http::header::HeaderName::from_static("x-ratelimit-limit"),
                    limit_val,
                );
            }
            
            if let Ok(remaining_val) = HeaderValue::from_str(&rate_limit_info.remaining.to_string()) {
                headers.insert(
                    actix_web::http::header::HeaderName::from_static("x-ratelimit-remaining"),
                    remaining_val,
                );
            }
            
            if let Ok(reset_val) = HeaderValue::from_str(&rate_limit_info.reset.to_string()) {
                headers.insert(
                    actix_web::http::header::HeaderName::from_static("x-ratelimit-reset"),
                    reset_val,
                );
            }
            
            Ok(res)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::{
        http::StatusCode,
        test::{call_service, init_service, TestRequest},
        web, App, HttpResponse,
    };

    async fn test_handler() -> HttpResponse {
        HttpResponse::Ok().body("success")
    }

    #[actix_web::test]
    async fn test_rate_limiter_under_limit() {
        let app = init_service(
            App::new()
                .wrap(RateLimiter::new(5)) // 5 requests per minute
                .route("/", web::get().to(test_handler)),
        )
        .await;

        // Make 5 requests (under limit)
        for _ in 0..5 {
            let req = TestRequest::get().uri("/").to_request();
            let resp = call_service(&app, req).await;
            assert_eq!(resp.status(), StatusCode::OK);
        }
    }

    #[actix_web::test]
    async fn test_rate_limiter_over_limit() {
        let app = init_service(
            App::new()
                .wrap(RateLimiter::new(5)) // 5 requests per minute
                .route("/", web::get().to(test_handler)),
        )
        .await;

        // Make 6 requests (over limit)
        for i in 0..6 {
            let req = TestRequest::get().uri("/").to_request();
            let resp = call_service(&app, req).await;

            if i < 5 {
                assert_eq!(resp.status(), StatusCode::OK);
            } else {
                assert_eq!(resp.status(), StatusCode::TOO_MANY_REQUESTS);
            }
        }
    }

    #[actix_web::test]
    async fn test_rate_limiter_documentation_routes() {
        let app = init_service(
            App::new()
                .wrap(RateLimiter::new(5)) // 5 requests per minute
                .route("/swagger-ui/index.html", web::get().to(test_handler)),
        )
        .await;

        // Make 10 requests to documentation (should bypass rate limiting)
        for _ in 0..10 {
            let req = TestRequest::get()
                .uri("/swagger-ui/index.html")
                .to_request();
            let resp = call_service(&app, req).await;
            assert_eq!(resp.status(), StatusCode::OK);
        }
    }

    #[actix_web::test]
    async fn test_rate_limit_headers_present() {
        let app = init_service(
            App::new()
                .wrap(RateLimiter::new(10)) // 10 requests per minute
                .route("/", web::get().to(test_handler)),
        )
        .await;

        let req = TestRequest::get().uri("/").to_request();
        let resp = call_service(&app, req).await;

        // Verify rate limit headers are present
        let headers = resp.headers();
        
        assert!(
            headers.contains_key("x-ratelimit-limit"),
            "X-RateLimit-Limit header should be present"
        );
        assert!(
            headers.contains_key("x-ratelimit-remaining"),
            "X-RateLimit-Remaining header should be present"
        );
        assert!(
            headers.contains_key("x-ratelimit-reset"),
            "X-RateLimit-Reset header should be present"
        );

        // Verify X-RateLimit-Limit value
        let limit = headers.get("x-ratelimit-limit")
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or(0);
        assert_eq!(limit, 10, "X-RateLimit-Limit should be 10");

        // Verify X-RateLimit-Remaining value (should be 9 after first request)
        let remaining = headers.get("x-ratelimit-remaining")
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or(0);
        assert_eq!(remaining, 9, "X-RateLimit-Remaining should be 9 after first request");
    }

    #[actix_web::test]
    async fn test_rate_limit_remaining_decrements() {
        let app = init_service(
            App::new()
                .wrap(RateLimiter::new(5)) // 5 requests per minute
                .route("/", web::get().to(test_handler)),
        )
        .await;

        // Make requests and verify remaining decrements
        for i in 0..4 {
            let req = TestRequest::get().uri("/").to_request();
            let resp = call_service(&app, req).await;
            
            let remaining = resp.headers()
                .get("x-ratelimit-remaining")
                .and_then(|v| v.to_str().ok())
                .and_then(|s| s.parse::<u32>().ok())
                .unwrap_or(99);
            
            assert_eq!(
                remaining, 
                4 - i as u32, 
                "Remaining should decrement with each request"
            );
        }
    }

    #[actix_web::test]
    async fn test_retry_after_on_429() {
        let app = init_service(
            App::new()
                .wrap(RateLimiter::with_window(2, 120)) // 2 requests per 2 minutes
                .route("/", web::get().to(test_handler)),
        )
        .await;

        // Exhaust rate limit
        for _ in 0..2 {
            let req = TestRequest::get().uri("/").to_request();
            let _ = call_service(&app, req).await;
        }

        // Third request should get 429 with Retry-After header
        let req = TestRequest::get().uri("/").to_request();
        let resp = call_service(&app, req).await;
        
        assert_eq!(resp.status(), StatusCode::TOO_MANY_REQUESTS);
        
        // Check Retry-After header is present
        let retry_after = resp.headers()
            .get("retry-after")
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.parse::<u64>().ok());
        
        assert!(retry_after.is_some(), "Retry-After header should be present on 429");
        assert_eq!(retry_after.unwrap(), 120, "Retry-After should match window_secs");
    }
}
