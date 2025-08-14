/// Validation subsystem for supernova blockchain
/// 
/// Provides tools for validating transactions, blocks, and signatures
/// with customizable policy settings for both cryptographic and emissions compliance.

use serde::{Serialize, Deserialize};
use std::fmt;
use std::error::Error as StdError;

pub mod transaction;
pub mod crypto;
pub mod block;
pub mod unified_validation;

#[cfg(test)]
mod block_validation_tests;

/// Security level for cryptographic operations and validation
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SecurityLevel {
    /// Low security level (corresponds to security parameter 1)
    Low = 1,
    
    /// Medium security level (corresponds to security parameter 3)
    Medium = 3,
    
    /// High security level (corresponds to security parameter 5)
    High = 5,
    
    /// Standard security for transaction validation
    Standard = 10,
    
    /// Enhanced security with additional checks for transaction validation
    Enhanced = 20,
    
    /// Maximum security with thorough validation
    Maximum = 30,
}

// Allow usage as u8 for security level parameters
impl From<SecurityLevel> for u8 {
    fn from(level: SecurityLevel) -> Self {
        match level {
            SecurityLevel::Low => 1,
            SecurityLevel::Medium => 3,
            SecurityLevel::High => 5,
            SecurityLevel::Standard => 10,
            SecurityLevel::Enhanced => 20,
            SecurityLevel::Maximum => 30,
        }
    }
}

// Allow conversion from u8 to SecurityLevel
impl From<u8> for SecurityLevel {
    fn from(value: u8) -> Self {
        match value {
            1 => SecurityLevel::Low,
            3 => SecurityLevel::Medium,
            5 => SecurityLevel::High,
            10 => SecurityLevel::Standard,
            20 => SecurityLevel::Enhanced,
            30 => SecurityLevel::Maximum,
            // Default to Medium for other values
            _ => {
                if value < 3 {
                    SecurityLevel::Low
                } else if value < 5 {
                    SecurityLevel::Medium
                } else {
                    SecurityLevel::High
                }
            }
        }
    }
}

/// Metrics for validation operations
#[derive(Debug, Clone, Default)]
pub struct ValidationMetrics {
    /// Number of successful validations
    pub success_count: u64,
    
    /// Number of failed validations
    pub failure_count: u64,
    
    /// Average validation time in milliseconds
    pub avg_validation_time_ms: f64,
    
    /// Maximum validation time in milliseconds
    pub max_validation_time_ms: f64,
    
    /// Total validation operations performed
    pub total_validations: u64,
}

/// Error encountered during validation
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValidationError {
    /// Invalid block height
    InvalidBlockHeight(u64),
    
    /// Invalid timestamp
    InvalidTimestamp(u64),
    
    /// Invalid hash
    InvalidHash,
    
    /// Invalid merkle root
    InvalidMerkleRoot,
    
    /// Invalid difficulty
    InvalidDifficulty,
    
    /// Invalid nonce
    InvalidNonce(u64),
    
    /// Invalid signature
    InvalidSignature(String),
    
    /// Signature error
    SignatureError(String),
    
    /// Missing signature data
    MissingSignatureData,
    
    /// Invalid signature scheme
    InvalidSignatureScheme,
    
    /// Double spend
    DoubleSpend,
    
    /// Transaction not found
    TransactionNotFound(String),
    
    /// Block not found
    BlockNotFound(String),
    
    /// Output not found
    OutputNotFound,
    
    /// Database error
    DatabaseError(String),
    
    /// Invalid script
    InvalidScript(String),
    
    /// Chain error
    ChainError(String),
    
    /// Checkpoint error
    CheckpointError(String),
    
    /// Invalid structure
    InvalidStructure(String),
    
    /// Cryptographic error
    CryptoError(String),
    
    /// Generic error
    Generic(String),
}

impl fmt::Display for ValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ValidationError::InvalidBlockHeight(height) => write!(f, "Invalid block height: {}", height),
            ValidationError::InvalidTimestamp(timestamp) => write!(f, "Invalid timestamp: {}", timestamp),
            ValidationError::InvalidHash => write!(f, "Invalid hash"),
            ValidationError::InvalidMerkleRoot => write!(f, "Invalid merkle root"),
            ValidationError::InvalidDifficulty => write!(f, "Invalid difficulty"),
            ValidationError::InvalidNonce(nonce) => write!(f, "Invalid nonce: {}", nonce),
            ValidationError::InvalidSignature(msg) => write!(f, "Invalid signature: {}", msg),
            ValidationError::SignatureError(msg) => write!(f, "Signature error: {}", msg),
            ValidationError::MissingSignatureData => write!(f, "Missing signature data"),
            ValidationError::InvalidSignatureScheme => write!(f, "Invalid signature scheme"),
            ValidationError::DoubleSpend => write!(f, "Double spend"),
            ValidationError::TransactionNotFound(txid) => write!(f, "Transaction not found: {}", txid),
            ValidationError::BlockNotFound(hash) => write!(f, "Block not found: {}", hash),
            ValidationError::OutputNotFound => write!(f, "Output not found"),
            ValidationError::DatabaseError(msg) => write!(f, "Database error: {}", msg),
            ValidationError::InvalidScript(msg) => write!(f, "Invalid script: {}", msg),
            ValidationError::ChainError(msg) => write!(f, "Chain error: {}", msg),
            ValidationError::CheckpointError(msg) => write!(f, "Checkpoint error: {}", msg),
            ValidationError::InvalidStructure(msg) => write!(f, "Invalid structure: {}", msg),
            ValidationError::CryptoError(msg) => write!(f, "Cryptographic error: {}", msg),
            ValidationError::Generic(msg) => write!(f, "Validation error: {}", msg),
        }
    }
}

impl StdError for ValidationError {}

pub use transaction::{
    ValidationResult,
    ValidationConfig,
    TransactionValidator,
};

pub use crypto::{
    CryptoValidator,
    CryptoValidationConfig,
};

pub use block::{
    BlockValidator,
    BlockValidationConfig,
    BlockValidationError,
    BlockValidationResult,
    ValidationContext,
};

// Convenience functions for validation
use crate::types::Block;
use crate::types::transaction::Transaction;

/// Validate a block with default configuration
pub fn validate_block(block: &Block) -> Result<(), ValidationError> {
    let validator = block::BlockValidator::new();
    validator.validate_block(block)
        .map_err(|e| ValidationError::Generic(e.to_string()))
}

/// Validate a transaction with default configuration
pub fn validate_transaction(tx: &Transaction) -> Result<(), ValidationError> {
    let validator = transaction::TransactionValidator::new();
    match validator.validate(tx) {
        Ok(transaction::ValidationResult::Valid) => Ok(()),
        Ok(transaction::ValidationResult::Invalid(err)) => Err(err),
        Ok(transaction::ValidationResult::SoftFail(err)) => Err(err),
        Err(err) => Err(err),
    }
} 