/// Validation subsystem for SuperNova blockchain
/// 
/// Provides tools for validating transactions, blocks, and signatures
/// with customizable policy settings for both cryptographic and emissions compliance.

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

/// Error types for validation
#[derive(Debug, thiserror::Error)]
pub enum ValidationError {
    #[error("Generic validation error: {0}")]
    Generic(String),
    
    #[error("Transaction input not found: {0}")]
    InputNotFound(String),
    
    #[error("Duplicate transaction input")]
    DuplicateInput,
    
    #[error("Invalid signature: {0}")]
    InvalidSignature(String),
    
    #[error("Invalid script: {0}")]
    InvalidScript(String),
    
    #[error("Transaction fee too low")]
    FeeTooLow,
    
    #[error("Balance mismatch: {0}")]
    BalanceMismatch(String),
    
    #[error("Block size exceeds maximum")]
    BlockSizeExceeded,
    
    #[error("Invalid merkle root")]
    InvalidMerkleRoot,
    
    #[error("Invalid proof of work")]
    InvalidProofOfWork,
    
    #[error("Block timestamp too far in future")]
    TimestampTooFar,
    
    #[error("Block hash doesn't meet difficulty target")]
    DifficultyTargetNotMet,
    
    #[error("Consensus rule violation: {0}")]
    ConsensusRuleViolation(String),
    
    #[error("Double spend attempt")]
    DoubleSpend,
    
    #[error("Quantum security compromised: {0}")]
    QuantumSecurityCompromised(String),
    
    #[error("Transaction validation error: {0}")]
    TransactionValidation(#[from] Box<dyn std::error::Error + Send + Sync>),
    
    // Add the missing validation error variants
    #[error("Invalid structure: {0}")]
    InvalidStructure(String),
    
    #[error("Cryptographic error: {0}")]
    CryptoError(String),
    
    #[error("Invalid format: {0}")]
    InvalidFormat(String),
    
    #[error("Emissions compliance error: {0}")]
    EmissionsCompliance(String),
    
    #[error("Quantum error: {0}")]
    QuantumError(String),
    
    #[error("Missing signature")]
    MissingSignature,
    
    #[error("Format error: {0}")]
    FormatError(String),
    
    #[error("Configuration error: {0}")]
    ConfigError(String),
    
    #[error("Validation rule violated: {0}")]
    RuleViolation(String),
    
    #[error("Environmental policy violation: {0}")]
    EnvironmentalPolicyViolation(String),
    
    #[error("Security policy violation: {0}")]
    SecurityPolicyViolation(String),
    
    #[error("General validation failure: {0}")]
    GeneralFailure(String),
}

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
}; 