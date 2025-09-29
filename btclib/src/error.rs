//! Comprehensive Error Handling System for Supernova
//!
//! This module provides a unified error handling system that replaces unsafe unwrap() calls
//! throughout the codebase, preventing node crashes on unexpected input.

use std::error::Error as StdError;
use std::fmt;
use thiserror::Error;

/// Main error type for the Supernova blockchain
#[derive(Debug, Error)]
pub enum SupernovaError {
    /// Block-related errors
    #[error("Block error: {0}")]
    Block(#[from] BlockError),

    /// Transaction-related errors
    #[error("Transaction error: {0}")]
    Transaction(#[from] TransactionError),

    /// Consensus errors
    #[error("Consensus error: {0}")]
    Consensus(#[from] ConsensusError),

    /// Network errors
    #[error("Network error: {0}")]
    Network(#[from] NetworkError),

    /// Storage errors
    #[error("Storage error: {0}")]
    Storage(#[from] StorageError),

    /// Lightning Network errors
    #[error("Lightning error: {0}")]
    Lightning(#[from] LightningError),

    /// Environmental system errors
    #[error("Environmental error: {0}")]
    Environmental(#[from] EnvironmentalError),

    /// Mining errors
    #[error("Mining error: {0}")]
    Mining(#[from] MiningError),

    /// Serialization errors
    #[error("Serialization error: {0}")]
    Serialization(#[from] SerializationError),

    /// Cryptographic errors
    #[error("Cryptographic error: {0}")]
    Crypto(#[from] CryptoError),

    /// System errors
    #[error("System error: {0}")]
    System(#[from] SystemError),

    /// API errors
    #[error("API error: {0}")]
    Api(#[from] ApiError),

    /// Generic internal error
    #[error("Internal error: {0}")]
    Internal(String),
}

/// Result type alias for Supernova operations
pub type SupernovaResult<T> = Result<T, SupernovaError>;

/// Block-related errors
#[derive(Debug, Error)]
pub enum BlockError {
    #[error("Invalid block hash: {0}")]
    InvalidHash(String),

    #[error("Block not found: {0}")]
    NotFound(String),

    #[error("Invalid block header")]
    InvalidHeader,

    #[error("Block validation failed: {0}")]
    ValidationFailed(String),

    #[error("Block size exceeds maximum: {size} > {max_size}")]
    SizeExceeded { size: usize, max_size: usize },

    #[error("Invalid timestamp: {0}")]
    InvalidTimestamp(String),

    #[error("Invalid proof of work")]
    InvalidProofOfWork,

    #[error("Merkle root mismatch")]
    MerkleRootMismatch,
}

/// Transaction-related errors
#[derive(Debug, Error)]
pub enum TransactionError {
    #[error("Invalid transaction: {0}")]
    Invalid(String),

    #[error("Transaction not found: {0}")]
    NotFound(String),

    #[error("Double spend detected")]
    DoubleSpend,

    #[error("Insufficient funds")]
    InsufficientFunds,

    #[error("Invalid signature")]
    InvalidSignature,

    #[error("Script execution failed: {0}")]
    ScriptFailed(String),

    #[error("Fee overflow detected")]
    FeeOverflow,

    #[error("Invalid output amount: {0}")]
    InvalidOutputAmount(u64),
}

/// Consensus-related errors
#[derive(Debug, Error)]
pub enum ConsensusError {
    #[error("Fork resolution failed: {0}")]
    ForkResolutionFailed(String),

    #[error("Difficulty adjustment error: {0}")]
    DifficultyAdjustment(String),

    #[error("Timestamp validation failed: {0}")]
    TimestampValidation(String),

    #[error("Chain reorganization failed: {0}")]
    ReorganizationFailed(String),

    #[error("Invalid chain work")]
    InvalidChainWork,
}

/// Network-related errors
#[derive(Debug, Error)]
pub enum NetworkError {
    #[error("Peer connection failed: {0}")]
    ConnectionFailed(String),

    #[error("Message parsing error: {0}")]
    MessageParsing(String),

    #[error("Peer banned: {0}")]
    PeerBanned(String),

    #[error("Network timeout")]
    Timeout,

    #[error("Eclipse attack detected")]
    EclipseAttack,

    #[error("Invalid peer address: {0}")]
    InvalidAddress(String),
}

/// Storage-related errors
#[derive(Debug, Error)]
pub enum StorageError {
    #[error("Database error: {0}")]
    Database(String),

    #[error("Corruption detected: {0}")]
    Corruption(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization failed: {0}")]
    Serialization(String),

    #[error("Key not found: {0}")]
    KeyNotFound(String),

    #[error("Lock poisoned")]
    LockPoisoned,
}

/// Lightning Network errors
#[derive(Debug, Error)]
pub enum LightningError {
    #[error("Channel not found: {0}")]
    ChannelNotFound(String),

    #[error("Insufficient channel balance")]
    InsufficientBalance,

    #[error("HTLC timeout")]
    HtlcTimeout,

    #[error("Channel force closed")]
    ForceClosed,

    #[error("Invalid payment hash")]
    InvalidPaymentHash,

    #[error("Route not found")]
    RouteNotFound,

    #[error("Channel state error: {0}")]
    ChannelState(String),
}

/// Environmental system errors
#[derive(Debug, Error)]
pub enum EnvironmentalError {
    #[error("Oracle consensus failed")]
    OracleConsensusFailed,

    #[error("Invalid energy proof")]
    InvalidEnergyProof,

    #[error("Certificate verification failed")]
    CertificateVerificationFailed,

    #[error("Insufficient stake for oracle")]
    InsufficientStake,

    #[error("Oracle not registered")]
    OracleNotRegistered,
}

/// Mining-related errors
#[derive(Debug, Error)]
pub enum MiningError {
    #[error("Mining template creation failed: {0}")]
    TemplateCreation(String),

    #[error("Nonce exhausted")]
    NonceExhausted,

    #[error("Target adjustment failed")]
    TargetAdjustment,

    #[error("Invalid coinbase")]
    InvalidCoinbase,

    #[error("Block reward calculation error")]
    RewardCalculation,
}

/// Serialization errors
#[derive(Debug, Error)]
pub enum SerializationError {
    #[error("Encoding failed: {0}")]
    EncodingFailed(String),

    #[error("Decoding failed: {0}")]
    DecodingFailed(String),

    #[error("Invalid format: {0}")]
    InvalidFormat(String),

    #[error("Size limit exceeded")]
    SizeLimitExceeded,
}

/// Cryptographic errors
#[derive(Debug, Error)]
pub enum CryptoError {
    #[error("Signature verification failed")]
    SignatureVerificationFailed,

    #[error("Key generation failed")]
    KeyGenerationFailed,

    #[error("Invalid public key")]
    InvalidPublicKey,

    #[error("Invalid private key")]
    InvalidPrivateKey,

    #[error("Hash computation failed")]
    HashComputationFailed,

    #[error("Quantum signature error: {0}")]
    QuantumSignature(String),
}

/// System errors
#[derive(Debug, Error)]
pub enum SystemError {
    #[error("Time error: {0}")]
    Time(String),

    #[error("Thread panic: {0}")]
    ThreadPanic(String),

    #[error("Resource exhausted: {0}")]
    ResourceExhausted(String),

    #[error("Configuration error: {0}")]
    Configuration(String),

    #[error("Shutdown in progress")]
    ShuttingDown,
}

/// API errors
#[derive(Debug, Error)]
pub enum ApiError {
    #[error("Authentication failed")]
    AuthenticationFailed,

    #[error("Rate limit exceeded")]
    RateLimitExceeded,

    #[error("Invalid request: {0}")]
    InvalidRequest(String),

    #[error("Method not allowed")]
    MethodNotAllowed,

    #[error("Resource not found")]
    NotFound,

    #[error("Internal server error")]
    InternalError,
}

/// Extension trait for converting Results with detailed context
pub trait ResultExt<T> {
    /// Convert a Result to SupernovaResult with context
    fn to_supernova<C: fmt::Display>(self, context: C) -> SupernovaResult<T>;

    /// Convert a Result to SupernovaResult with lazy context
    fn to_supernova_with<F, C>(self, f: F) -> SupernovaResult<T>
    where
        F: FnOnce() -> C,
        C: fmt::Display;
}

impl<T, E> ResultExt<T> for Result<T, E>
where
    E: StdError + Send + Sync + 'static,
{
    fn to_supernova<C: fmt::Display>(self, context: C) -> SupernovaResult<T> {
        self.map_err(|e| SupernovaError::Internal(format!("{}: {}", context, e)))
    }

    fn to_supernova_with<F, C>(self, f: F) -> SupernovaResult<T>
    where
        F: FnOnce() -> C,
        C: fmt::Display,
    {
        self.map_err(|e| SupernovaError::Internal(format!("{}: {}", f(), e)))
    }
}

/// Safe unwrap alternative that provides context
pub trait SafeUnwrap<T> {
    /// Unwrap with a context message, converting to SupernovaError
    fn safe_unwrap<C: fmt::Display>(self, context: C) -> SupernovaResult<T>;
}

impl<T> SafeUnwrap<T> for Option<T> {
    fn safe_unwrap<C: fmt::Display>(self, context: C) -> SupernovaResult<T> {
        self.ok_or_else(|| SupernovaError::Internal(format!("Unwrap failed: {}", context)))
    }
}

/// Macro for safe field access with error propagation
#[macro_export]
macro_rules! safe_get {
    ($expr:expr, $field:ident, $err:expr) => {
        $expr.$field().ok_or_else(|| $err)?
    };
    ($expr:expr, $method:ident(), $err:expr) => {
        $expr.$method().ok_or_else(|| $err)?
    };
}

/// Error context builder for adding additional information
pub struct ErrorContext<E> {
    error: E,
    context: Vec<String>,
}

impl<E: StdError> ErrorContext<E> {
    /// Create a new error context
    pub fn new(error: E) -> Self {
        Self {
            error,
            context: Vec::new(),
        }
    }

    /// Add context to the error
    pub fn context<C: fmt::Display>(mut self, ctx: C) -> Self {
        self.context.push(ctx.to_string());
        self
    }

    /// Convert to SupernovaError
    pub fn build(self) -> SupernovaError {
        let mut msg = self.error.to_string();
        for ctx in self.context.iter().rev() {
            msg = format!("{}: {}", ctx, msg);
        }
        SupernovaError::Internal(msg)
    }
}

/// Helper function to handle system time errors
pub fn get_system_time() -> SupernovaResult<u64> {
    use std::time::{SystemTime, UNIX_EPOCH};

    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| SystemError::Time(e.to_string()).into())
        .map(|d| d.as_secs())
}

/// Helper function for safe serialization
pub fn safe_serialize<T: serde::Serialize>(value: &T) -> SupernovaResult<Vec<u8>> {
    bincode::serialize(value).map_err(|e| SerializationError::EncodingFailed(e.to_string()).into())
}

/// Helper function for safe deserialization
pub fn safe_deserialize<T: serde::de::DeserializeOwned>(data: &[u8]) -> SupernovaResult<T> {
    bincode::deserialize(data).map_err(|e| SerializationError::DecodingFailed(e.to_string()).into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_conversion() {
        let block_err = BlockError::NotFound("test".to_string());
        let supernova_err: SupernovaError = block_err.into();
        assert!(supernova_err.to_string().contains("Block error"));
    }

    #[test]
    fn test_result_ext() {
        fn failing_operation() -> Result<(), std::io::Error> {
            Err(std::io::Error::new(std::io::ErrorKind::NotFound, "test"))
        }

        let result = failing_operation().to_supernova("Operation failed");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Operation failed"));
    }

    #[test]
    fn test_safe_unwrap() {
        let opt: Option<i32> = None;
        let result = opt.safe_unwrap("Expected value");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Expected value"));
    }

    #[test]
    fn test_error_context() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file missing");
        let err = ErrorContext::new(io_err)
            .context("Reading configuration")
            .context("Starting node")
            .build();

        assert!(err.to_string().contains("Starting node"));
        assert!(err.to_string().contains("Reading configuration"));
        assert!(err.to_string().contains("file missing"));
    }

    #[test]
    fn test_safe_arithmetic() {
        let a: u64 = u64::MAX - 10;
        let b: u64 = 20;

        // Test that overflow is properly caught
        let result = a.checked_add(b);
        assert!(result.is_none());

        // Test the safe_add functionality (macro from errors module)
        // We'll test it properly by importing and using it
        use crate::errors::SupernovaError;
        let result = (|| -> Result<u64, SupernovaError> { Ok(crate::safe_add!(a, b)) })();

        assert!(result.is_err());
        match result.unwrap_err() {
            SupernovaError::ArithmeticOverflow(_) => {}
            _ => panic!("Wrong error type"),
        }
    }
}
