//! API error types and error handling

use std::fmt;
use actix_web::{HttpResponse, ResponseError};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// API Result type alias
pub type Result<T> = std::result::Result<T, ApiError>;

/// API error response structure
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ErrorResponse {
    /// Error code
    pub code: u16,
    /// Error message
    pub message: String,
    /// Additional error details (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<String>,
}

/// API error types
#[derive(Error, Debug)]
pub enum ApiError {
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

impl ResponseError for ApiError {
    fn error_response(&self) -> HttpResponse {
        let status_code = self.status_code();
        let error_response = ErrorResponse {
            code: status_code.as_u16(),
            message: self.to_string(),
            details: None,
        };
        
        HttpResponse::build(status_code)
            .json(error_response)
    }
    
    fn status_code(&self) -> actix_web::http::StatusCode {
        use actix_web::http::StatusCode;
        
        match self {
            ApiError::NotFound(_) => StatusCode::NOT_FOUND,
            ApiError::BadRequest(_) => StatusCode::BAD_REQUEST,
            ApiError::InternalError(_) => StatusCode::INTERNAL_SERVER_ERROR,
            ApiError::DatabaseError(_) => StatusCode::INTERNAL_SERVER_ERROR,
            ApiError::NodeSyncing => StatusCode::SERVICE_UNAVAILABLE,
            ApiError::BlockchainError(_) => StatusCode::BAD_REQUEST,
            ApiError::TransactionError(_) => StatusCode::BAD_REQUEST,
            ApiError::MiningError(_) => StatusCode::BAD_REQUEST,
            ApiError::NetworkError(_) => StatusCode::BAD_REQUEST,
            ApiError::EnvironmentalError(_) => StatusCode::BAD_REQUEST,
            ApiError::LightningError(_) => StatusCode::BAD_REQUEST,
            ApiError::WalletError(_) => StatusCode::BAD_REQUEST,
            ApiError::AuthorizationError(_) => StatusCode::UNAUTHORIZED,
            ApiError::RateLimitExceeded => StatusCode::TOO_MANY_REQUESTS,
            ApiError::ServiceUnavailable(_) => StatusCode::SERVICE_UNAVAILABLE,
        }
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
        ApiError::InternalError(err.to_string())
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