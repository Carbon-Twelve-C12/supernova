//! API authentication middleware
//!
//! This module provides API key authentication for the SuperNova API.

use std::future::{ready, Ready};
use std::rc::Rc;
use std::task::{Context, Poll};
use actix_web::{
    dev::{forward_ready, Service, ServiceRequest, ServiceResponse, Transform},
    error::ErrorUnauthorized,
    http::header,
    Error,
};
use tracing::{debug, warn};

/// API authentication middleware
pub struct ApiAuth {
    api_keys: Rc<Vec<String>>,
}

impl ApiAuth {
    /// Create a new authentication middleware with the given API keys
    pub fn new(api_keys: Vec<String>) -> Self {
        Self {
            api_keys: Rc::new(api_keys),
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
        }))
    }
}

/// API authentication middleware service
pub struct ApiAuthMiddleware<S> {
    service: S,
    api_keys: Rc<Vec<String>>,
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
                    
                    if self.api_keys.is_empty() {
                        // If no API keys configured, authentication is disabled
                        true
                    } else {
                        self.api_keys.contains(&api_key.to_string())
                    }
                } else {
                    false
                }
            }
            None => false,
        };

        if is_authorized {
            let fut = self.service.call(req);
            Box::pin(async move {
                let res = fut.await?;
                Ok(res)
            })
        } else {
            // Log unauthorized access attempt
            warn!(
                "Unauthorized API access attempt from {}",
                req.connection_info().peer_addr().unwrap_or("unknown")
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
                .wrap(ApiAuth::new(vec!["test-key".to_string()]))
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
                .wrap(ApiAuth::new(vec!["test-key".to_string()]))
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
                .wrap(ApiAuth::new(vec!["test-key".to_string()]))
                .route("/swagger-ui/index.html", web::get().to(test_handler)),
        )
        .await;

        let req = TestRequest::get()
            .uri("/swagger-ui/index.html")
            .to_request();

        let resp = call_service(&app, req).await;
        assert_eq!(resp.status(), StatusCode::OK);
    }
} 