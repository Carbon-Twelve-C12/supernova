//! API authentication middleware
//!
//! This module provides API key authentication for the Supernova API.
//!
//! SECURITY: Authentication is MANDATORY. Empty API key lists are rejected to prevent bypass.

use actix_web::{
    dev::{forward_ready, Service, ServiceRequest, ServiceResponse, Transform},
    error::ErrorUnauthorized,
    http::header,
    Error,
};
use std::future::{ready, Ready};
use std::rc::Rc;
use std::sync::Arc;
use tracing::{error, warn};

use super::auth_rate_limiter::{AuthBlockedError, AuthRateLimiter, AuthRateLimiterConfig};

/// API authentication middleware
pub struct ApiAuth {
    api_keys: Rc<Vec<String>>,
    rate_limiter: Arc<AuthRateLimiter>,
    enabled: bool,
}

impl ApiAuth {
    /// Create a new authentication middleware with the given API keys
    ///
    /// # Security
    /// Requires at least one API key to prevent authentication bypass
    pub fn new(api_keys: Vec<String>) -> Result<Self, &'static str> {
        if api_keys.is_empty() {
            error!("SECURITY: Attempted to create ApiAuth with empty API key list");
            return Err("At least one API key must be configured for security");
        }

        // Validate API keys are not empty strings
        for key in &api_keys {
            if key.trim().is_empty() {
                error!("SECURITY: Attempted to use empty/whitespace API key");
                return Err("API keys cannot be empty or whitespace");
            }
        }

        Ok(Self {
            api_keys: Rc::new(api_keys),
            rate_limiter: Arc::new(AuthRateLimiter::new(AuthRateLimiterConfig::default())),
            enabled: true,
        })
    }

    /// Create authentication middleware from already-validated keys.
    ///
    /// # Safety invariant
    /// The caller must have already rejected empty / whitespace / placeholder
    /// keys (see `ApiServer::start`). This constructor exists to avoid the
    /// `Result` return type of [`ApiAuth::new`] on a per-worker hot path where
    /// the panic-free lint policy (`#![deny(clippy::expect_used)]`) forbids
    /// `.expect()` on values the compiler cannot prove infallible.
    ///
    /// Each invocation allocates a fresh [`AuthRateLimiter`]. For the
    /// per-worker `HttpServer` factory, prefer
    /// [`ApiAuth::from_validated_keys_with_rate_limiter`] with a single
    /// `Arc<AuthRateLimiter>` built outside the closure — otherwise every
    /// worker thread keeps its own `failed_attempts` map and the effective
    /// brute-force ceiling becomes `N_workers × max_failed_attempts`.
    pub fn from_validated_keys(api_keys: Vec<String>) -> Self {
        Self::from_validated_keys_with_rate_limiter(
            api_keys,
            Arc::new(AuthRateLimiter::new(AuthRateLimiterConfig::default())),
        )
    }

    /// Create authentication middleware from already-validated keys, reusing
    /// the supplied rate limiter. Pass the same `Arc` to every per-worker
    /// instance so the failed-attempt map is shared across workers.
    pub fn from_validated_keys_with_rate_limiter(
        api_keys: Vec<String>,
        rate_limiter: Arc<AuthRateLimiter>,
    ) -> Self {
        Self {
            api_keys: Rc::new(api_keys),
            rate_limiter,
            enabled: true,
        }
    }

    /// Disabled pass-through variant. Used when `ApiConfig::enable_auth` is
    /// `false` so the middleware stack keeps a uniform type across the two
    /// configurations without conditional `.wrap()` calls.
    pub fn disabled() -> Self {
        Self::disabled_with_rate_limiter(Arc::new(AuthRateLimiter::new(
            AuthRateLimiterConfig::default(),
        )))
    }

    /// Disabled variant that accepts a shared rate limiter. Kept symmetric
    /// with [`ApiAuth::from_validated_keys_with_rate_limiter`] so the
    /// per-worker factory can unconditionally clone one `Arc` regardless of
    /// whether auth is enabled.
    pub fn disabled_with_rate_limiter(rate_limiter: Arc<AuthRateLimiter>) -> Self {
        Self {
            api_keys: Rc::new(Vec::new()),
            rate_limiter,
            enabled: false,
        }
    }
}

/// Paths served publicly (no API key required). Liveness / readiness probes
/// and minimal chain-state read endpoints must be reachable by external
/// monitoring without shipping secrets.
const PUBLIC_PATH_PREFIXES: &[&str] = &[
    "/health",
    "/api/v1/node/version",
    "/api/v1/blockchain/info",
    "/api/v1/blockchain/height",
];

fn is_public_path(path: &str) -> bool {
    PUBLIC_PATH_PREFIXES
        .iter()
        .any(|prefix| path == *prefix || path.starts_with(&format!("{}/", prefix)))
}

impl<S, B> Transform<S, ServiceRequest> for ApiAuth
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error>,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type Transform = ApiAuthMiddleware<S>;
    type InitError = ();
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(ApiAuthMiddleware {
            service,
            api_keys: self.api_keys.clone(),
            rate_limiter: self.rate_limiter.clone(),
            enabled: self.enabled,
        }))
    }
}

/// API authentication middleware service
pub struct ApiAuthMiddleware<S> {
    service: S,
    api_keys: Rc<Vec<String>>,
    rate_limiter: Arc<AuthRateLimiter>,
    enabled: bool,
}

impl<S, B> Service<ServiceRequest> for ApiAuthMiddleware<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error>,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type Future =
        std::pin::Pin<Box<dyn std::future::Future<Output = Result<Self::Response, Self::Error>>>>;

    forward_ready!(service);

    fn call(&self, req: ServiceRequest) -> Self::Future {
        // Pass-through when auth is explicitly disabled. Operators that opt
        // into this path must also have pinned the server to a trusted
        // interface (the server's default is loopback-only).
        if !self.enabled {
            let fut = self.service.call(req);
            return Box::pin(async move {
                let res = fut.await?;
                Ok(res)
            });
        }

        // Get client IP for rate limiting
        let client_ip = req
            .connection_info()
            .peer_addr()
            .unwrap_or("unknown")
            .to_string();

        // Check if IP is blocked due to too many failed attempts
        if self.rate_limiter.is_blocked(&client_ip) {
            warn!(
                "SECURITY: Blocked IP {} attempted authentication",
                client_ip
            );
            return Box::pin(async move {
                Err(AuthBlockedError {
                    block_duration_secs: 3600, // 1 hour default
                }
                .into())
            });
        }

        // Skip authentication for OPTIONS requests (pre-flight CORS)
        if req.method() == actix_web::http::Method::OPTIONS {
            let fut = self.service.call(req);
            return Box::pin(async move {
                let res = fut.await?;
                Ok(res)
            });
        }

        // Skip authentication for documentation routes
        if req.path().starts_with("/swagger-ui") || req.path().starts_with("/api-docs") {
            let fut = self.service.call(req);
            return Box::pin(async move {
                let res = fut.await?;
                Ok(res)
            });
        }

        // Skip authentication for the public-by-design endpoints (liveness /
        // readiness probes and minimal read-only chain state). These must be
        // reachable without credentials for external monitoring.
        if is_public_path(req.path()) {
            let fut = self.service.call(req);
            return Box::pin(async move {
                let res = fut.await?;
                Ok(res)
            });
        }

        // Extract API key from Authorization header
        let auth_header = req.headers().get(header::AUTHORIZATION);

        // Check if API key is valid
        let is_authorized = match auth_header {
            Some(auth) => {
                if let Ok(auth_str) = auth.to_str() {
                    // Support "Bearer <token>" format for API keys
                    let api_key = if auth_str.starts_with("Bearer ") {
                        auth_str.strip_prefix("Bearer ").unwrap_or(auth_str)
                    } else {
                        auth_str
                    };

                    // SECURITY: Authentication is mandatory - no bypass allowed
                    self.api_keys.contains(&api_key.to_string())
                } else {
                    false
                }
            }
            None => false,
        };

        let rate_limiter = self.rate_limiter.clone();
        let client_ip_clone = client_ip.clone();

        if is_authorized {
            // Record successful authentication
            rate_limiter.record_successful_auth(&client_ip_clone);

            let fut = self.service.call(req);
            Box::pin(async move {
                let res = fut.await?;
                Ok(res)
            })
        } else {
            // Record failed authentication attempt
            rate_limiter.record_failed_attempt(&client_ip);

            // Log unauthorized access attempt
            warn!("Unauthorized API access attempt from {}", client_ip);

            Box::pin(async move { Err(ErrorUnauthorized("Invalid API key")) })
        }
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
    async fn test_auth_middleware_valid_key() {
        let app = init_service(
            App::new()
                .wrap(ApiAuth::from_validated_keys(vec!["test-key".to_string()]))
                .route("/", web::get().to(test_handler)),
        )
        .await;

        let req = TestRequest::get()
            .uri("/")
            .insert_header((header::AUTHORIZATION, "Bearer test-key"))
            .to_request();

        let resp = call_service(&app, req).await;
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[actix_web::test]
    async fn test_auth_middleware_invalid_key() {
        let app = init_service(
            App::new()
                .wrap(ApiAuth::from_validated_keys(vec!["test-key".to_string()]))
                .route("/", web::get().to(test_handler)),
        )
        .await;

        let req = TestRequest::get()
            .uri("/")
            .insert_header((header::AUTHORIZATION, "Bearer invalid-key"))
            .to_request();

        let resp = call_service(&app, req).await;
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }

    #[actix_web::test]
    async fn test_auth_middleware_swagger_no_key() {
        let app = init_service(
            App::new()
                .wrap(ApiAuth::from_validated_keys(vec!["test-key".to_string()]))
                .route("/swagger-ui/index.html", web::get().to(test_handler)),
        )
        .await;

        let req = TestRequest::get()
            .uri("/swagger-ui/index.html")
            .to_request();

        let resp = call_service(&app, req).await;
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[actix_web::test]
    async fn test_auth_middleware_rejects_empty_keys() {
        // Test that empty API key list is rejected
        let result = ApiAuth::new(vec![]);
        assert!(result.is_err());
        assert_eq!(
            result.err().unwrap(),
            "At least one API key must be configured for security"
        );

        // Test that empty string keys are rejected
        let result = ApiAuth::new(vec!["".to_string()]);
        assert!(result.is_err());

        // Test that whitespace keys are rejected
        let result = ApiAuth::new(vec!["   ".to_string()]);
        assert!(result.is_err());
    }

    #[actix_web::test]
    async fn test_no_auth_bypass() {
        // Ensure authentication cannot be bypassed
        let auth = ApiAuth::from_validated_keys(vec!["secure-key".to_string()]);

        let app = init_service(
            App::new()
                .wrap(auth)
                .route("/secure", web::get().to(test_handler)),
        )
        .await;

        // Request without auth header should fail
        let req = TestRequest::get().uri("/secure").to_request();

        let resp = call_service(&app, req).await;
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);

        // Request with wrong key should fail
        let req = TestRequest::get()
            .uri("/secure")
            .insert_header((header::AUTHORIZATION, "Bearer wrong-key"))
            .to_request();

        let resp = call_service(&app, req).await;
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }
}
