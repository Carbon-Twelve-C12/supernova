use thiserror::Error;

/// Main error type for the SuperNova blockchain
#[derive(Error, Debug)]
pub enum SuperNovaError {
    // Block-related errors
    #[error("Block validation error: {0}")]
    BlockValidation(#[from] crate::validation::BlockValidationError),

    // Transaction-related errors
    #[error("Transaction validation error: {0}")]
    TransactionValidation(#[from] crate::validation::ValidationError),

    // Consensus errors
    #[error("Consensus error: {0}")]
    Consensus(String),

    // Storage errors
    #[error("Storage error: {0}")]
    Storage(String),

    #[error("UTXO error: {0}")]
    Utxo(String),

    // Network errors
    #[error("Network error: {0}")]
    Network(String),

    // Environmental module errors
    #[error("Emissions tracking error: {0}")]
    Emissions(#[from] crate::environmental::emissions::EmissionsError),

    #[error("Environmental verification error: {0}")]
    EnvironmentalVerification(#[from] crate::environmental::verification::VerificationError),

    #[error("Treasury error: {0}")]
    Treasury(String),

    // Cryptography errors
    #[error("Cryptography error: {0}")]
    Crypto(String),

    #[error("Signature error: {0}")]
    Signature(#[from] crate::crypto::signature::SignatureError),

    // Lightning Network errors
    #[cfg(feature = "lightning")]
    #[error("Lightning error: {0}")]
    Lightning(String),

    // Mempool errors
    #[error("Mempool error: {0}")]
    Mempool(#[from] crate::mempool::MempoolError),

    // General errors
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serialization(#[from] bincode::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Configuration error: {0}")]
    Configuration(String),

    #[error("Other error: {0}")]
    Other(String),
}

/// Type alias for Result with SuperNovaError
pub type SuperNovaResult<T> = Result<T, SuperNovaError>;

/// Helper function to convert string errors to SuperNovaError
pub fn to_supernova_error<E: std::error::Error>(err: E, context: &str) -> SuperNovaError {
    SuperNovaError::Other(format!("{}: {}", context, err))
}

impl From<String> for SuperNovaError {
    fn from(err: String) -> Self {
        SuperNovaError::Other(err)
    }
}

impl From<&str> for SuperNovaError {
    fn from(err: &str) -> Self {
        SuperNovaError::Other(err.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_conversion() {
        let err = SuperNovaError::from("test error");
        assert!(matches!(err, SuperNovaError::Other(_)));
        
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
        let err = SuperNovaError::from(io_err);
        assert!(matches!(err, SuperNovaError::Io(_)));
    }
} 