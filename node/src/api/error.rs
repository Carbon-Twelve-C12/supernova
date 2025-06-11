//! API error types and error handling

use std::fmt;
use actix_web::{HttpResponse, ResponseError, Responder, HttpRequest};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use utoipa::ToSchema;

/// API error types with security-conscious error messages
#[derive(Debug, Serialize, Deserialize)]
pub struct ApiError {
    /// HTTP status code
    pub status: u16,
    /// Error message (sanitized for security)
    pub message: String,
    /// Error code for client handling
    pub code: String,
    /// Request ID for tracking (optional)
    pub request_id: Option<String>,
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
    /// Create a new API error with security-conscious message sanitization
    pub fn new(status: u16, message: &str) -> Self {
        Self {
            status,
            message: Self::sanitize_error_message(message),
            code: Self::status_to_code(status),
            request_id: None,
        }
    }
    
    /// Create an API error with a request ID for tracking
    pub fn with_request_id(status: u16, message: &str, request_id: String) -> Self {
        Self {
            status,
            message: Self::sanitize_error_message(message),
            code: Self::status_to_code(status),
            request_id: Some(request_id),
        }
    }
    
    /// Sanitize error messages to prevent information leakage
    fn sanitize_error_message(message: &str) -> String {
        // Remove potentially sensitive information from error messages
        let sanitized = message
            .replace("database", "storage")
            .replace("sql", "query")
            .replace("password", "credential")
            .replace("key", "identifier")
            .replace("secret", "credential")
            .replace("token", "credential")
            .replace("private", "internal")
            .replace("internal error", "service error");
            
        // Limit message length to prevent verbose error leakage
        if sanitized.len() > 200 {
            format!("{}...", &sanitized[..197])
        } else {
            sanitized
        }
    }
    
    /// Convert HTTP status to error code
    fn status_to_code(status: u16) -> String {
        match status {
            400 => "BAD_REQUEST".to_string(),
            401 => "UNAUTHORIZED".to_string(),
            403 => "FORBIDDEN".to_string(),
            404 => "NOT_FOUND".to_string(),
            409 => "CONFLICT".to_string(),
            422 => "UNPROCESSABLE_ENTITY".to_string(),
            429 => "RATE_LIMITED".to_string(),
            500 => "INTERNAL_ERROR".to_string(),
            502 => "BAD_GATEWAY".to_string(),
            503 => "SERVICE_UNAVAILABLE".to_string(),
            504 => "GATEWAY_TIMEOUT".to_string(),
            _ => "UNKNOWN_ERROR".to_string(),
        }
    }
    
    // Common error constructors for security
    pub fn bad_request<S: AsRef<str>>(message: S) -> Self {
        Self::new(400, message.as_ref())
    }
    
    pub fn unauthorized<S: AsRef<str>>(message: S) -> Self {
        Self::new(401, message.as_ref())
    }
    
    pub fn forbidden<S: AsRef<str>>(message: S) -> Self {
        Self::new(403, message.as_ref())
    }
    
    pub fn not_found<S: AsRef<str>>(message: S) -> Self {
        Self::new(404, message.as_ref())
    }
    
    pub fn conflict<S: AsRef<str>>(message: S) -> Self {
        Self::new(409, message.as_ref())
    }
    
    pub fn unprocessable_entity<S: AsRef<str>>(message: S) -> Self {
        Self::new(422, message.as_ref())
    }
    
    pub fn rate_limited<S: AsRef<str>>(message: S) -> Self {
        Self::new(429, message.as_ref())
    }
    
    pub fn internal_error<S: AsRef<str>>(message: S) -> Self {
        Self::new(500, message.as_ref())
    }
    
    pub fn service_unavailable<S: AsRef<str>>(message: S) -> Self {
        Self::new(503, message.as_ref())
    }
}

impl fmt::Display for ApiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "API Error {}: {}", self.status, self.message)
    }
}

impl ResponseError for ApiError {
    fn error_response(&self) -> HttpResponse {
        HttpResponse::build(actix_web::http::StatusCode::from_u16(self.status).unwrap_or(actix_web::http::StatusCode::INTERNAL_SERVER_ERROR))
            .json(self)
    }
}

/// Result type alias for API operations
pub type Result<T> = std::result::Result<T, ApiError>;

/// Legacy alias for compatibility
pub type ApiResult<T> = Result<T>;

/// Convert common error types to API errors with security considerations
impl From<std::io::Error> for ApiError {
    fn from(err: std::io::Error) -> Self {
        // Don't expose internal I/O error details
        Self::internal_error("Storage operation failed")
    }
}

impl From<serde_json::Error> for ApiError {
    fn from(err: serde_json::Error) -> Self {
        // Don't expose JSON parsing details that could reveal structure
        Self::bad_request("Invalid request format")
    }
}

impl From<bincode::Error> for ApiError {
    fn from(err: bincode::Error) -> Self {
        // Don't expose serialization details
        Self::internal_error("Data processing error")
    }
}

impl From<String> for ApiError {
    fn from(err: String) -> Self {
        // Convert string errors to internal errors
        Self::internal_error(&err)
    }
}

impl From<crate::node::NodeError> for ApiError {
    fn from(err: crate::node::NodeError) -> Self {
        // Convert node errors to appropriate API errors
        match err {
            crate::node::NodeError::NetworkError(msg) => Self::service_unavailable(&msg),
            crate::node::NodeError::ConfigError(msg) => Self::bad_request(&msg),
            crate::node::NodeError::StorageError(_) => Self::internal_error("Storage operation failed"),
            crate::node::NodeError::LightningError(_) => Self::service_unavailable("Lightning Network unavailable"),
            crate::node::NodeError::IoError(_) => Self::internal_error("I/O operation failed"),
            crate::node::NodeError::General(msg) => Self::internal_error(&msg),
        }
    }
}

/// Security middleware for rate limiting and request validation
pub struct SecurityMiddleware {
    /// Maximum requests per minute per IP
    pub rate_limit: u32,
    /// Request size limit in bytes
    pub max_request_size: usize,
    /// Enable request logging for security monitoring
    pub enable_logging: bool,
}

impl Default for SecurityMiddleware {
    fn default() -> Self {
        Self {
            rate_limit: 100, // 100 requests per minute per IP
            max_request_size: 1024 * 1024, // 1MB max request size
            enable_logging: true,
        }
    }
}

impl SecurityMiddleware {
    /// Validate request for security compliance
    pub fn validate_request(&self, request_size: usize, client_ip: &str) -> Result<()> {
        // Check request size limit
        if request_size > self.max_request_size {
            return Err(ApiError::bad_request("Request too large"));
        }
        
        // Additional security validations can be added here
        // - IP whitelist/blacklist checks
        // - Geographic restrictions
        // - User agent validation
        // - Request pattern analysis
        
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_error_message_sanitization() {
        let error = ApiError::new(500, "Database connection failed with password 'secret123'");
        assert!(!error.message.contains("password"));
        assert!(!error.message.contains("secret123"));
        assert!(error.message.contains("credential"));
    }
    
    #[test]
    fn test_error_message_length_limit() {
        let long_message = "a".repeat(300);
        let error = ApiError::new(500, &long_message);
        assert!(error.message.len() <= 200);
        assert!(error.message.ends_with("..."));
    }
    
    #[test]
    fn test_status_code_mapping() {
        let error = ApiError::bad_request("test");
        assert_eq!(error.status, 400);
        assert_eq!(error.code, "BAD_REQUEST");
    }
    
    #[test]
    fn test_security_middleware_validation() {
        let middleware = SecurityMiddleware::default();
        
        // Valid request
        assert!(middleware.validate_request(1000, "127.0.0.1").is_ok());
        
        // Request too large
        assert!(middleware.validate_request(2 * 1024 * 1024, "127.0.0.1").is_err());
    }
} 