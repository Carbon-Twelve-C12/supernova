//! Mempool error types

use thiserror::Error;
use std::fmt;

/// Result type for mempool operations
pub type MempoolResult<T> = Result<T, MempoolError>;

/// Mempool error types
#[derive(Error, Debug)]
pub enum MempoolError {
    #[error("Transaction validation failed: {0}")]
    ValidationFailed(String),
    
    #[error("Transaction already exists: {0}")]
    TransactionExists(String),
    
    #[error("Transaction not found: {0}")]
    TransactionNotFound(String),
    
    #[error("Mempool full: current size {current}, max size {max}")]
    MempoolFull { current: usize, max: usize },
    
    #[error("Fee too low: required {required}, provided {provided}")]
    FeeTooLow { required: u64, provided: u64 },
    
    #[error("Double spend detected: {0}")]
    DoubleSpend(String),
    
    #[error("Transaction expired")]
    TransactionExpired,
    
    #[error("Invalid transaction: {0}")]
    InvalidTransaction(String),
    
    #[error("Storage error: {0}")]
    StorageError(String),
    
    #[error("Serialization error: {0}")]
    SerializationError(String),
    
    #[error("Lock error: {0}")]
    LockError(String),
    
    #[error("Internal error: {0}")]
    InternalError(String),
}

impl From<bincode::Error> for MempoolError {
    fn from(err: bincode::Error) -> Self {
        MempoolError::SerializationError(err.to_string())
    }
}

impl From<std::io::Error> for MempoolError {
    fn from(err: std::io::Error) -> Self {
        MempoolError::StorageError(err.to_string())
    }
} 