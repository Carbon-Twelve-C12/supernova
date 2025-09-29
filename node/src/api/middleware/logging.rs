//! API logging middleware
//!
//! This module provides detailed request logging for the supernova API.

use actix_service::{Service, Transform};
use actix_web::{
    dev::{forward_ready, ServiceRequest, ServiceResponse},
    http::header,
    Error, HttpMessage,
};
use futures::future::{ready, Ready};
use std::rc::Rc;
use std::time::Instant;
use tracing::{debug, error, info};
use uuid::Uuid;

/// API logger middleware
pub struct ApiLogger {}

impl ApiLogger {
    /// Create a new logging middleware
    pub fn new() -> Self {
        Self {}
    }
}

impl Default for ApiLogger {
    fn default() -> Self {
        Self::new()
    }
}

impl<S, B> Transform<S, ServiceRequest> for ApiLogger
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type Transform = ApiLoggerMiddleware<S>;
    type InitError = ();
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(ApiLoggerMiddleware {
            service: Rc::new(service),
        }))
    }
}

/// API logger middleware service
pub struct ApiLoggerMiddleware<S> {
    service: Rc<S>,
}

impl<S, B> Service<ServiceRequest> for ApiLoggerMiddleware<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type Future =
        std::pin::Pin<Box<dyn std::future::Future<Output = Result<Self::Response, Self::Error>>>>;

    forward_ready!(service);

    fn call(&self, req: ServiceRequest) -> Self::Future {
        let start_time = Instant::now();
        let method = req.method().clone();
        let path = req.path().to_owned();
        let peer_addr = req
            .connection_info()
            .peer_addr()
            .unwrap_or("unknown")
            .to_owned();
        let user_agent = req
            .headers()
            .get(header::USER_AGENT)
            .and_then(|h| h.to_str().ok())
            .unwrap_or("unknown")
            .to_owned();
        let content_length = req
            .headers()
            .get(header::CONTENT_LENGTH)
            .and_then(|h| h.to_str().ok())
            .unwrap_or("0")
            .to_owned();

        // Generate a unique request ID
        let request_id = Uuid::new_v4().to_string();

        // Clone service because it's behind an Rc
        let service = self.service.clone();

        Box::pin(async move {
            // Add request ID to request extensions
            req.extensions_mut().insert(request_id.clone());

            // Log the incoming request
            debug!(
                "Request {} - {} {} - From {} - UA: {} - Size: {}",
                request_id, method, path, peer_addr, user_agent, content_length
            );

            // Process the request
            let result = service.call(req).await;

            // Get elapsed time
            let elapsed = start_time.elapsed();

            match &result {
                Ok(res) => {
                    // Log successful response
                    let status = res.status();
                    info!(
                        "Response {} - {} {} - Status {} - Completed in {:?}",
                        request_id,
                        method,
                        path,
                        status.as_u16(),
                        elapsed
                    );
                }
                Err(e) => {
                    // Log error response
                    error!(
                        "Response {} - {} {} - Error: {} - Completed in {:?}",
                        request_id, method, path, e, elapsed
                    );
                }
            }

            result
        })
    }
}

/// Request ID extraction helper for handlers
pub fn get_request_id(req: &actix_web::HttpRequest) -> String {
    req.extensions()
        .get::<String>()
        .cloned()
        .unwrap_or_else(|| "unknown".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::{
        http::StatusCode,
        test::{call_service, init_service, TestRequest},
        web, App, HttpResponse,
    };

    async fn test_handler(req: actix_web::HttpRequest) -> HttpResponse {
        let request_id = get_request_id(&req);
        assert!(!request_id.is_empty());
        HttpResponse::Ok().body("success")
    }

    async fn error_handler() -> HttpResponse {
        HttpResponse::InternalServerError().finish()
    }

    #[actix_web::test]
    async fn test_logger_middleware_request() {
        let app = init_service(
            App::new()
                .wrap(ApiLogger::new())
                .route("/", web::get().to(test_handler))
                .route("/error", web::get().to(error_handler)),
        )
        .await;

        // Test successful request
        let req = TestRequest::get()
            .uri("/")
            .insert_header((header::USER_AGENT, "test-agent"))
            .to_request();
        let resp = call_service(&app, req).await;
        assert_eq!(resp.status(), StatusCode::OK);

        // Test error request
        let req = TestRequest::get()
            .uri("/error")
            .insert_header((header::USER_AGENT, "test-agent"))
            .to_request();
        let resp = call_service(&app, req).await;
        assert_eq!(resp.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }
}
