//! API rate limiting middleware
//!
//! This module provides rate limiting functionality for the supernova API.

use std::collections::HashMap;
use std::future::{ready, Ready};
use std::rc::Rc;
use std::sync::Mutex;
use std::task::{Context, Poll};
use std::time::{Duration, Instant};
use actix_web::{
    dev::{forward_ready, Service, ServiceRequest, ServiceResponse, Transform},
    http::StatusCode,
    HttpResponse, ResponseError,
};
use tracing::{debug, warn};
use serde_json::json;

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
    /// Number of requests in current window
    count: u32,
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
    type Future = std::pin::Pin<Box<dyn std::future::Future<Output = Result<Self::Response, Self::Error>>>>;

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
        let mut clients = match self.state.clients.lock() {
            Ok(clients) => clients,
            Err(_) => {
                // Lock is poisoned, continue without rate limiting
                return Poll::Ready(Ok(()));
            }
        };
        
        // Clean up expired entries
        clients.retain(|_, entry| {
            now.duration_since(entry.first_request) < window
        });
        
        // Get current client's entry
        let entry = clients.entry(ip.clone()).or_insert_with(|| RateLimitEntry {
            first_request: now,
            count: 0,
        });

        // Reset entry if window has expired
        if now.duration_since(entry.first_request) >= window {
            entry.first_request = now;
            entry.count = 0;
        }

        // Check if rate limit exceeded
        if entry.count >= self.state.rate {
            warn!("Rate limit exceeded for client IP {}", ip);
            let error = RateLimitError {
                rate: self.state.rate,
                window_secs: self.state.window_secs,
            };
            return Box::pin(async move { Err(error.into()) });
        }

        // Increment request count
        entry.count += 1;
        drop(clients); // Release mutex

        // Forward request to next middleware/handler
        let fut = self.service.call(req);
        Box::pin(async move {
            let res = fut.await?;
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
            let req = TestRequest::get().uri("/swagger-ui/index.html").to_request();
            let resp = call_service(&app, req).await;
            assert_eq!(resp.status(), StatusCode::OK);
        }
    }
} 