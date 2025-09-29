use actix_web::{
    dev::{forward_ready, Service, ServiceRequest, ServiceResponse, Transform},
    Error, HttpMessage, HttpResponse,
    http::header::{HeaderValue, AUTHORIZATION},
};
use futures_util::future::LocalBoxFuture;
use std::{
    future::{ready, Ready},
    rc::Rc,
};

/// List of public endpoints that don't require authentication
const PUBLIC_ENDPOINTS: &[&str] = &[
    "/health",
    "/api/v1/blockchain/info",
    "/api/v1/blockchain/height",
    "/api/v1/faucet/request", // Rate limited separately
];

/// Mandatory authentication middleware
pub struct MandatoryAuth {
    api_key: String,
    enabled: bool,
}

impl MandatoryAuth {
    pub fn new(api_key: String, enabled: bool) -> Self {
        Self { api_key, enabled }
    }
}

impl<S, B> Transform<S, ServiceRequest> for MandatoryAuth
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type InitError = ();
    type Transform = MandatoryAuthMiddleware<S>;
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(MandatoryAuthMiddleware {
            service: Rc::new(service),
            api_key: self.api_key.clone(),
            enabled: self.enabled,
        }))
    }
}

pub struct MandatoryAuthMiddleware<S> {
    service: Rc<S>,
    api_key: String,
    enabled: bool,
}

impl<S, B> Service<ServiceRequest> for MandatoryAuthMiddleware<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type Future = LocalBoxFuture<'static, Result<Self::Response, Self::Error>>;

    forward_ready!(service);

    fn call(&self, req: ServiceRequest) -> Self::Future {
        // Skip auth for disabled or public endpoints
        if !self.enabled || PUBLIC_ENDPOINTS.contains(&req.path()) {
            let fut = self.service.call(req);
            return Box::pin(async move { fut.await });
        }

        // Extract authorization header
        let auth_header = req.headers().get(AUTHORIZATION);

        let api_key = self.api_key.clone();
        let service = self.service.clone();

        Box::pin(async move {
            // Validate authorization
            let is_authorized = match auth_header {
                Some(header_value) => validate_auth_header(header_value, &api_key),
                None => false,
            };

            if !is_authorized {
                return Ok(req.into_response(
                    HttpResponse::Unauthorized()
                        .json(serde_json::json!({
                            "error": "Authentication required",
                            "message": "Please provide a valid API key in the Authorization header",
                            "code": "AUTH_REQUIRED"
                        }))
                ));
            }

            // Proceed with authenticated request
            service.call(req).await
        })
    }
}

fn validate_auth_header(header_value: &HeaderValue, expected_key: &str) -> bool {
    if let Ok(auth_str) = header_value.to_str() {
        // Support both "Bearer <token>" and direct token
        if auth_str.starts_with("Bearer ") {
            return &auth_str[7..] == expected_key;
        }
        return auth_str == expected_key;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::{test, web, App, HttpRequest, HttpResponse};

    async fn protected_handler(_req: HttpRequest) -> HttpResponse {
        HttpResponse::Ok().json(serde_json::json!({"status": "authorized"}))
    }

    #[actix_web::test]
    async fn test_mandatory_auth_blocks_unauthorized() {
        let app = test::init_service(
            App::new()
                .wrap(MandatoryAuth::new("test-key".to_string(), true))
                .route("/api/protected", web::get().to(protected_handler))
        ).await;

        let req = test::TestRequest::get()
            .uri("/api/protected")
            .to_request();

        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 401);
    }

    #[actix_web::test]
    async fn test_mandatory_auth_allows_authorized() {
        let app = test::init_service(
            App::new()
                .wrap(MandatoryAuth::new("test-key".to_string(), true))
                .route("/api/protected", web::get().to(protected_handler))
        ).await;

        let req = test::TestRequest::get()
            .uri("/api/protected")
            .insert_header(("Authorization", "Bearer test-key"))
            .to_request();

        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);
    }

    #[actix_web::test]
    async fn test_public_endpoints_bypass_auth() {
        let app = test::init_service(
            App::new()
                .wrap(MandatoryAuth::new("test-key".to_string(), true))
                .route("/health", web::get().to(|| async { HttpResponse::Ok().body("OK") }))
        ).await;

        let req = test::TestRequest::get()
            .uri("/health")
            .to_request();

        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);
    }
}