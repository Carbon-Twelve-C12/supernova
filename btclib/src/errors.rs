use thiserror::Error;

/// General error types for the SuperNova blockchain
#[derive(Error, Debug)]
pub enum SuperNovaError {
    #[error("Validation error: {0}")]
    ValidationError(#[from] crate::validation::ValidationError),
    
    #[error("Crypto error: {0}")]
    CryptoError(String),
    
    #[error("Network error: {0}")]
    NetworkError(String),
    
    #[error("Storage error: {0}")]
    StorageError(String),
    
    #[error("Configuration error: {0}")]
    ConfigError(String),
    
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
    
    #[error("Serialization error: {0}")]
    SerializationError(String),
    
    #[error("Internal error: {0}")]
    InternalError(String),
}

/// Standard result type for SuperNova operations
pub type Result<T> = std::result::Result<T, SuperNovaError>;

/// Helper function to convert a string error to a SuperNovaError
pub fn internal_error<S: Into<String>>(msg: S) -> SuperNovaError {
    SuperNovaError::InternalError(msg.into())
}

/// Helper function to convert a validation error message to a SuperNovaError
pub fn validation_error<S: Into<String>>(msg: S) -> SuperNovaError {
    SuperNovaError::ValidationError(crate::validation::ValidationError::Generic(msg.into()))
}

/// Helper function to convert a crypto error message to a SuperNovaError
pub fn crypto_error<S: Into<String>>(msg: S) -> SuperNovaError {
    SuperNovaError::CryptoError(msg.into())
}

/// Helper function to convert a network error message to a SuperNovaError
pub fn network_error<S: Into<String>>(msg: S) -> SuperNovaError {
    SuperNovaError::NetworkError(msg.into())
}

/// Helper function to convert a storage error message to a SuperNovaError
pub fn storage_error<S: Into<String>>(msg: S) -> SuperNovaError {
    SuperNovaError::StorageError(msg.into())
}

/// Helper function to convert a configuration error message to a SuperNovaError
pub fn config_error<S: Into<String>>(msg: S) -> SuperNovaError {
    SuperNovaError::ConfigError(msg.into())
}

/// Helper function to convert a serialization error message to a SuperNovaError
pub fn serialization_error<S: Into<String>>(msg: S) -> SuperNovaError {
    SuperNovaError::SerializationError(msg.into())
} 