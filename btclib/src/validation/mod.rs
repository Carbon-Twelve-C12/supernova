/// Validation subsystem for SuperNova blockchain
/// 
/// Provides tools for validating transactions, blocks, and signatures
/// with customizable policy settings for both cryptographic and emissions compliance.

pub mod transaction;
pub mod crypto;

pub use transaction::{
    ValidationResult,
    ValidationConfig,
    ValidationError,
    TransactionValidator,
};

pub use crypto::{
    ValidationMode,
    SignatureValidator,
}; 