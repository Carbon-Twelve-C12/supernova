/// Validation subsystem for SuperNova blockchain
/// 
/// Provides tools for validating transactions, blocks, and signatures
/// with customizable policy settings for both cryptographic and emissions compliance.

use thiserror::Error;
use serde::{Serialize, Deserialize};

pub mod transaction;
pub mod crypto;
pub mod block;

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

/// Validation error types
#[derive(Debug, Clone, thiserror::Error)]
pub enum ValidationError {
    /// Invalid signature error
    #[error("Invalid signature: {0}")]
    InvalidSignature(String),
    
    /// Invalid script error
    #[error("Invalid script: {0}")]
    InvalidScript(String),
    
    /// Invalid structure error
    #[error("Invalid structure: {0}")]
    InvalidStructure(String),
    
    /// Emissions compliance error
    #[error("Emissions compliance error: {0}")]
    EmissionsCompliance(String),
    
    /// Quantum error
    #[error("Quantum error: {0}")]
    QuantumError(#[from] crate::crypto::quantum::QuantumError),
    
    /// Missing signature
    #[error("Missing signature")]
    MissingSignature,
    
    /// Format error
    #[error("Format error: {0}")]
    FormatError(String),
    
    /// Configuration error
    #[error("Configuration error: {0}")]
    ConfigError(String),
    
    /// Crypto error
    #[error("Cryptographic error: {0}")]
    CryptoError(String),
    
    /// Other error
    #[error("Validation error: {0}")]
    Other(String),
}

/// Performance metrics for validation
#[derive(Debug, Default)]
pub struct ValidationMetrics {
    /// Time taken for validation in milliseconds
    pub validation_time_ms: u64,
    
    /// Size of the transaction in bytes
    pub transaction_size: usize,
    
    /// Verification operations performed
    pub verification_ops: u32,
}

pub use transaction::{
    ValidationResult,
    ValidationConfig,
    TransactionValidator,
};

pub use crypto::{
    ValidationMode,
    SignatureValidator,
};

pub use block::{
    BlockValidator,
    BlockValidationConfig,
    BlockValidationError,
    BlockValidationResult,
}; 