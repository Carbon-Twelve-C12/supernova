use thiserror::Error;
use std::fmt;

/// Main error type for the supernova blockchain
#[derive(Error, Debug)]
pub enum supernovaError {
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
    
    #[error("Lock poisoned: {0}")]
    LockPoisoned(String),
    
    #[error("Time error: {0}")]
    TimeError(String),
    
    #[error("Arithmetic overflow: {0}")]
    ArithmeticOverflow(String),

    #[error("Other error: {0}")]
    Other(String),
}

/// Type alias for Result with supernovaError
pub type supernovaResult<T> = Result<T, supernovaError>;

/// Helper function to convert string errors to supernovaError
pub fn to_supernova_error<E: std::error::Error>(err: E, context: &str) -> supernovaError {
    supernovaError::Other(format!("{}: {}", context, err))
}

impl From<String> for supernovaError {
    fn from(err: String) -> Self {
        supernovaError::Other(err)
    }
}

impl From<&str> for supernovaError {
    fn from(err: &str) -> Self {
        supernovaError::Other(err.to_string())
    }
}

/// Safe unwrap extension trait
pub trait SafeUnwrap<T> {
    /// Unwrap with context, converting None to an error
    fn safe_unwrap<C: fmt::Display>(self, context: C) -> supernovaResult<T>;
}

impl<T> SafeUnwrap<T> for Option<T> {
    fn safe_unwrap<C: fmt::Display>(self, context: C) -> supernovaResult<T> {
        self.ok_or_else(|| supernovaError::Other(format!("Unwrap failed: {}", context)))
    }
}

/// Result extension trait for better error handling
pub trait ResultExt<T> {
    /// Add context to an error
    fn context<C: fmt::Display>(self, context: C) -> supernovaResult<T>;
}

impl<T, E> ResultExt<T> for Result<T, E>
where
    E: std::error::Error + Send + Sync + 'static,
{
    fn context<C: fmt::Display>(self, context: C) -> supernovaResult<T> {
        self.map_err(|e| supernovaError::Other(format!("{}: {}", context, e)))
    }
}

/// Safe lock acquisition macro
#[macro_export]
macro_rules! safe_lock {
    ($mutex:expr) => {
        $mutex.lock().map_err(|e| $crate::errors::supernovaError::LockPoisoned(format!("Lock poisoned: {}", e)))?
    };
    ($rwlock:expr, read) => {
        $rwlock.read().map_err(|e| $crate::errors::supernovaError::LockPoisoned(format!("RwLock read poisoned: {}", e)))?
    };
    ($rwlock:expr, write) => {
        $rwlock.write().map_err(|e| $crate::errors::supernovaError::LockPoisoned(format!("RwLock write poisoned: {}", e)))?
    };
}

/// Safe arithmetic macros
#[macro_export]
macro_rules! safe_add {
    ($a:expr, $b:expr) => {
        $a.checked_add($b).ok_or_else(|| $crate::errors::supernovaError::ArithmeticOverflow("Addition overflow".to_string()))?
    };
}

#[macro_export]
macro_rules! safe_sub {
    ($a:expr, $b:expr) => {
        $a.checked_sub($b).ok_or_else(|| $crate::errors::supernovaError::ArithmeticOverflow("Subtraction underflow".to_string()))?
    };
}

#[macro_export]
macro_rules! safe_mul {
    ($a:expr, $b:expr) => {
        $a.checked_mul($b).ok_or_else(|| $crate::errors::supernovaError::ArithmeticOverflow("Multiplication overflow".to_string()))?
    };
}

/// Helper function to safely get system time
pub fn get_system_time() -> supernovaResult<u64> {
    use std::time::{SystemTime, UNIX_EPOCH};
    
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| supernovaError::TimeError(e.to_string()))
        .map(|d| d.as_secs())
}

/// Safe serialization helper
pub fn safe_serialize<T: serde::Serialize>(value: &T) -> supernovaResult<Vec<u8>> {
    bincode::serialize(value)
        .map_err(supernovaError::Serialization)
}

/// Safe deserialization helper  
pub fn safe_deserialize<'a, T: serde::Deserialize<'a>>(data: &'a [u8]) -> supernovaResult<T> {
    bincode::deserialize(data)
        .map_err(supernovaError::Serialization)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_conversion() {
        let err = supernovaError::from("test error");
        assert!(matches!(err, supernovaError::Other(_)));
        
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
        let err = supernovaError::from(io_err);
        assert!(matches!(err, supernovaError::Io(_)));
    }
    
    #[test]
    fn test_safe_unwrap() {
        let opt: Option<i32> = None;
        let result = opt.safe_unwrap("expected value");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("expected value"));
        
        let some_opt = Some(42);
        let result = some_opt.safe_unwrap("should work");
        assert_eq!(result.unwrap(), 42);
    }
    
    #[test]
    fn test_safe_arithmetic() {
        let a: u64 = u64::MAX - 10;
        let b: u64 = 20;
        
        let result = (|| -> supernovaResult<u64> {
            Ok(safe_add!(a, b))
        })();
        
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), supernovaError::ArithmeticOverflow(_)));
    }
} 