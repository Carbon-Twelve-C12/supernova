//! Mempool error types

use thiserror::Error;

/// Result type for mempool operations
pub type MempoolResult<T> = Result<T, MempoolError>;

/// Mempool error types
#[derive(Error, Debug)]
pub enum MempoolError {
    #[error("Transaction validation failed: {0}")]
    ValidationFailed(String),

    #[error("Transaction already exists: {0}")]
    TransactionExists(String),

    #[error("Duplicate transaction")]
    DuplicateTransaction,

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

    #[error("Transaction too large: {size} bytes exceeds max {max} bytes")]
    TransactionTooLarge { size: usize, max: usize },

    #[error("Storage error: {0}")]
    StorageError(String),

    #[error("Serialization error: {0}")]
    SerializationError(String),

    #[error("Lock error: {0}")]
    LockError(String),

    #[error("Internal error: {0}")]
    InternalError(String),

    // SECURITY (P1-003): DoS protection errors
    #[error("Rate limit exceeded for peer {peer}: {limit} txs/minute maximum")]
    RateLimitExceeded { peer: String, limit: usize },

    #[error("Memory limit exceeded: current {current} bytes + tx {tx_size} bytes > max {max} bytes")]
    MemoryLimitExceeded {
        current: usize,
        max: usize,
        tx_size: usize,
    },

    // SECURITY (P1-002): Ancestor/Descendant chain limits
    #[error("Ancestor chain too long: {count} ancestors exceeds limit of {limit}")]
    AncestorChainTooLong { count: usize, limit: usize },

    #[error("Descendant chain too long: {count} descendants exceeds limit of {limit}")]
    DescendantChainTooLong { count: usize, limit: usize },

    #[error("Ancestor chain size too large: {size} bytes exceeds limit of {limit} bytes")]
    AncestorSizeTooLarge { size: usize, limit: usize },

    #[error("Descendant chain size too large: {size} bytes exceeds limit of {limit} bytes")]
    DescendantSizeTooLarge { size: usize, limit: usize },

    #[error("RBF replacement would evict too many transactions: {count} > {limit}")]
    RbfTooManyEvictions { count: usize, limit: usize },

    #[error("Transaction relay rate limit exceeded for peer {peer}")]
    RelayRateLimitExceeded { peer: String },

    #[error("Fee overflow: {0}")]
    FeeOverflow(String),
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
