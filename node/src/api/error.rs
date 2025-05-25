//! API error types and error handling

use std::fmt;
use actix_web::{HttpResponse, ResponseError};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use utoipa::ToSchema;

/// API error response
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct ApiError {
    /// Error code
    pub code: u16,
    /// Error message
    pub message: String,
    /// Optional error details
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<String>,
}

/// API error types
#[derive(Error, Debug)]
pub enum ApiErrorType {
    /// Not found error
    #[error("Resource not found: {0}")]
    NotFound(String),
    
    /// Invalid request parameters
    #[error("Invalid request: {0}")]
    BadRequest(String),
    
    /// Internal server error
    #[error("Internal server error: {0}")]
    InternalError(String),
    
    /// Database error
    #[error("Database error: {0}")]
    DatabaseError(String),
    
    /// Node is syncing
    #[error("Node is syncing")]
    NodeSyncing,
    
    /// Blockchain error
    #[error("Blockchain error: {0}")]
    BlockchainError(String),
    
    /// Transaction error
    #[error("Transaction error: {0}")]
    TransactionError(String),
    
    /// Mining error
    #[error("Mining error: {0}")]
    MiningError(String),
    
    /// Network error
    #[error("Network error: {0}")]
    NetworkError(String),
    
    /// Environmental error
    #[error("Environmental error: {0}")]
    EnvironmentalError(String),
    
    /// Lightning Network error
    #[error("Lightning Network error: {0}")]
    LightningError(String),
    
    /// Wallet error
    #[error("Wallet error: {0}")]
    WalletError(String),
    
    /// Authorization error
    #[error("Authorization error: {0}")]
    AuthorizationError(String),
    
    /// Rate limiting error
    #[error("Rate limit exceeded")]
    RateLimitExceeded,
    
    /// Service unavailable
    #[error("Service unavailable: {0}")]
    ServiceUnavailable(String),
}

impl ApiError {
    /// Create a new API error
    pub fn new(code: u16, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            details: None,
        }
    }

    /// Create a new API error with details
    pub fn with_details(code: u16, message: impl Into<String>, details: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            details: Some(details.into()),
        }
    }

    /// Create a bad request error (400)
    pub fn bad_request(message: impl Into<String>) -> Self {
        Self::new(400, message)
    }

    /// Create an unauthorized error (401)
    pub fn unauthorized(message: impl Into<String>) -> Self {
        Self::new(401, message)
    }

    /// Create a forbidden error (403)
    pub fn forbidden(message: impl Into<String>) -> Self {
        Self::new(403, message)
    }

    /// Create a not found error (404)
    pub fn not_found(message: impl Into<String>) -> Self {
        Self::new(404, message)
    }

    /// Create a method not allowed error (405)
    pub fn method_not_allowed(message: impl Into<String>) -> Self {
        Self::new(405, message)
    }

    /// Create a conflict error (409)
    pub fn conflict(message: impl Into<String>) -> Self {
        Self::new(409, message)
    }

    /// Create an unprocessable entity error (422)
    pub fn unprocessable_entity(message: impl Into<String>) -> Self {
        Self::new(422, message)
    }

    /// Create an internal server error (500)
    pub fn internal_error(message: impl Into<String>) -> Self {
        Self::new(500, message)
    }

    /// Create a not implemented error (501)
    pub fn not_implemented(message: impl Into<String>) -> Self {
        Self::new(501, message)
    }

    /// Create a bad gateway error (502)
    pub fn bad_gateway(message: impl Into<String>) -> Self {
        Self::new(502, message)
    }

    /// Create a service unavailable error (503)
    pub fn service_unavailable(message: impl Into<String>) -> Self {
        Self::new(503, message)
    }

    /// Create a gateway timeout error (504)
    pub fn gateway_timeout(message: impl Into<String>) -> Self {
        Self::new(504, message)
    }
}

impl fmt::Display for ApiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl ResponseError for ApiError {
    fn error_response(&self) -> HttpResponse {
        let status = match self.code {
            400 => actix_web::http::StatusCode::BAD_REQUEST,
            401 => actix_web::http::StatusCode::UNAUTHORIZED,
            403 => actix_web::http::StatusCode::FORBIDDEN,
            404 => actix_web::http::StatusCode::NOT_FOUND,
            405 => actix_web::http::StatusCode::METHOD_NOT_ALLOWED,
            409 => actix_web::http::StatusCode::CONFLICT,
            422 => actix_web::http::StatusCode::UNPROCESSABLE_ENTITY,
            500 => actix_web::http::StatusCode::INTERNAL_SERVER_ERROR,
            501 => actix_web::http::StatusCode::NOT_IMPLEMENTED,
            502 => actix_web::http::StatusCode::BAD_GATEWAY,
            503 => actix_web::http::StatusCode::SERVICE_UNAVAILABLE,
            504 => actix_web::http::StatusCode::GATEWAY_TIMEOUT,
            _ => actix_web::http::StatusCode::INTERNAL_SERVER_ERROR,
        };

        HttpResponse::build(status).json(self)
    }
}

/// Conversion from storage errors
impl From<crate::storage::StorageError> for ApiError {
    fn from(err: crate::storage::StorageError) -> Self {
        ApiError::DatabaseError(err.to_string())
    }
}

/// Conversion from blockchain errors
impl From<btclib::types::BlockchainError> for ApiError {
    fn from(err: btclib::types::BlockchainError) -> Self {
        ApiError::BlockchainError(err.to_string())
    }
}

/// Conversion from transaction errors
impl From<btclib::types::transaction::TransactionError> for ApiError {
    fn from(err: btclib::types::transaction::TransactionError) -> Self {
        ApiError::TransactionError(err.to_string())
    }
}

/// Conversion from environmental errors
impl From<btclib::environmental::EmissionsError> for ApiError {
    fn from(err: btclib::environmental::EmissionsError) -> Self {
        ApiError::EnvironmentalError(err.to_string())
    }
}

/// Conversion from std::io errors
impl From<std::io::Error> for ApiError {
    fn from(err: std::io::Error) -> Self {
        ApiError::internal_error(err.to_string())
    }
}

/// Conversion from serde_json errors
impl From<serde_json::Error> for ApiError {
    fn from(err: serde_json::Error) -> Self {
        ApiError::BadRequest(format!("JSON error: {}", err))
    }
}

/// Conversion from Lightning Network errors
impl From<btclib::lightning::LightningError> for ApiError {
    fn from(err: btclib::lightning::LightningError) -> Self {
        ApiError::LightningError(err.to_string())
    }
}

/// Type alias for API results
pub type ApiResult<T> = Result<HttpResponse, ApiError>;

impl From<String> for ApiError {
    fn from(err: String) -> Self {
        ApiError::internal_error(err)
    }
}

impl From<&str> for ApiError {
    fn from(err: &str) -> Self {
        ApiError::internal_error(err.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::http::StatusCode;
    
    #[test]
    fn test_error_status_codes() {
        assert_eq!(ApiError::NotFound("test".into()).status_code(), StatusCode::NOT_FOUND);
        assert_eq!(ApiError::BadRequest("test".into()).status_code(), StatusCode::BAD_REQUEST);
        assert_eq!(ApiError::RateLimitExceeded.status_code(), StatusCode::TOO_MANY_REQUESTS);
    }
    
    #[test]
    fn test_error_response_format() {
        let error = ApiError::NotFound("resource".into());
        let response = error.error_response();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }
} 