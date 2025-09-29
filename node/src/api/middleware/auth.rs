//! API authentication middleware
//!
//! This module provides API key authentication for the Supernova API.
//! 
//! SECURITY: Authentication is MANDATORY. Empty API key lists are rejected to prevent bypass.

use std::future::{ready, Ready};
use std::rc::Rc;
use std::sync::Arc;
use actix_web::{
    dev::{forward_ready, Service, ServiceRequest, ServiceResponse, Transform},
    error::ErrorUnauthorized,
    http::header,
    Error,
};
use tracing::{warn, error};

use super::auth_rate_limiter::{AuthRateLimiter, AuthRateLimiterConfig, AuthBlockedError};

/// API authentication middleware
pub struct ApiAuth {
    api_keys: Rc<Vec<String>>,
    rate_limiter: Arc<AuthRateLimiter>,
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
        })
    }
    
    /// Create authentication middleware for testing only
    #[cfg(test)]
    pub fn new_unchecked(api_keys: Vec<String>) -> Self {
        Self {
            api_keys: Rc::new(api_keys),
            rate_limiter: Arc::new(AuthRateLimiter::new(AuthRateLimiterConfig::default())),
        }
    }
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
        }))
    }
}

/// API authentication middleware service
pub struct ApiAuthMiddleware<S> {
    service: S,
    api_keys: Rc<Vec<String>>,
    rate_limiter: Arc<AuthRateLimiter>,
}

impl<S, B> Service<ServiceRequest> for ApiAuthMiddleware<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error>,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type Future = std::pin::Pin<Box<dyn std::future::Future<Output = Result<Self::Response, Self::Error>>>>;

    forward_ready!(service);

    fn call(&self, req: ServiceRequest) -> Self::Future {
        // Get client IP for rate limiting
        let client_ip = req
            .connection_info()
            .peer_addr()
            .unwrap_or("unknown")
            .to_string();
        
        // Check if IP is blocked due to too many failed attempts
        if self.rate_limiter.is_blocked(&client_ip) {
            warn!("SECURITY: Blocked IP {} attempted authentication", client_ip);
            return Box::pin(async move {
                Err(AuthBlockedError {
                    block_duration_secs: 3600, // 1 hour default
                }.into())
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
            warn!(
                "Unauthorized API access attempt from {}",
                client_ip
            );
            
            Box::pin(async move {
                Err(ErrorUnauthorized("Invalid API key"))
            })
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
                .wrap(ApiAuth::new_unchecked(vec!["test-key".to_string()]))
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
                .wrap(ApiAuth::new_unchecked(vec!["test-key".to_string()]))
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
                .wrap(ApiAuth::new_unchecked(vec!["test-key".to_string()]))
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
        assert_eq!(result.err().unwrap(), "At least one API key must be configured for security");
        
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
        let auth = ApiAuth::new_unchecked(vec!["secure-key".to_string()]);
        
        let app = init_service(
            App::new()
                .wrap(auth)
                .route("/secure", web::get().to(test_handler)),
        )
        .await;

        // Request without auth header should fail
        let req = TestRequest::get()
            .uri("/secure")
            .to_request();

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