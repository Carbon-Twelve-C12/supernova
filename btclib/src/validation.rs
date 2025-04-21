use thiserror::Error;
use std::sync::Arc;

use crate::types::transaction::Transaction;
use crate::types::extended_transaction::{QuantumTransaction, ConfidentialTransaction};
use crate::crypto::quantum::QuantumError;
use crate::crypto::zkp::ZkpError;
use crate::config::Config;

/// Error types for transaction validation
#[derive(Debug, Error)]
pub enum ValidationError {
    #[error("Transaction format error: {0}")]
    FormatError(String),
    
    #[error("Quantum signature error: {0}")]
    QuantumError(#[from] QuantumError),
    
    #[error("Zero-knowledge proof error: {0}")]
    ZkpError(#[from] ZkpError),
    
    #[error("Configuration error: {0}")]
    ConfigError(String),
    
    #[error("Invalid transaction: {0}")]
    InvalidTransaction(String),
}

/// Security level for transaction validation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SecurityLevel {
    /// Standard security checks
    Standard,
    
    /// Enhanced security with additional checks
    Enhanced,
    
    /// Maximum security with thorough validation
    Maximum,
}

/// Result of transaction validation
#[derive(Debug)]
pub struct ValidationResult {
    /// Overall validation result
    pub is_valid: bool,
    
    /// List of validation issues (if any)
    pub issues: Vec<String>,
    
    /// Security score (0-100)
    pub security_score: u8,
    
    /// Performance metrics for this validation
    pub metrics: ValidationMetrics,
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

/// Transaction validation service
pub struct ValidationService {
    /// Configuration
    config: Config,
    
    /// Security level for validation
    security_level: SecurityLevel,
}

impl ValidationService {
    /// Create a new validation service
    pub fn new(config: Config, security_level: SecurityLevel) -> Self {
        Self {
            config,
            security_level,
        }
    }
    
    /// Validate a standard transaction
    pub fn validate_transaction(&self, tx: &Transaction) -> Result<ValidationResult, ValidationError> {
        let start = std::time::Instant::now();
        let tx_serialized = bincode::serialize(tx)
            .map_err(|e| ValidationError::FormatError(e.to_string()))?;
        
        let mut issues = Vec::new();
        let mut security_score = 100;
        
        // Basic transaction checks
        if tx.inputs().is_empty() {
            issues.push("Transaction has no inputs".to_string());
            security_score -= 50;
        }
        
        if tx.outputs().is_empty() {
            issues.push("Transaction has no outputs".to_string());
            security_score -= 50;
        }
        
        // Size checks
        if tx_serialized.len() > self.config.max_tx_size {
            return Err(ValidationError::InvalidTransaction(
                format!("Transaction size exceeds maximum: {} > {}", 
                    tx_serialized.len(), self.config.max_tx_size)
            ));
        }
        
        // Security checks based on level
        if self.security_level != SecurityLevel::Standard {
            // Check for unusual transaction patterns
            if tx.outputs().len() > 100 {
                issues.push(format!("Unusually high number of outputs: {}", tx.outputs().len()));
                security_score -= 10;
            }
            
            // At maximum security, perform additional checks
            if self.security_level == SecurityLevel::Maximum {
                // Check for unusual output values
                for output in tx.outputs() {
                    if output.amount() > 1_000_000_000_000 { // 10,000 BTC
                        issues.push(format!("Unusually large output: {}", output.amount()));
                        security_score -= 5;
                    }
                }
            }
        }
        
        // Limit security score to valid range
        security_score = security_score.clamp(0, 100);
        
        let validation_time = start.elapsed();
        
        Ok(ValidationResult {
            is_valid: issues.is_empty() || security_score > 50,
            issues,
            security_score,
            metrics: ValidationMetrics {
                validation_time_ms: validation_time.as_millis() as u64,
                transaction_size: tx_serialized.len(),
                verification_ops: 1,
            },
        })
    }
    
    /// Validate a quantum-resistant transaction
    pub fn validate_quantum_transaction(
        &self, 
        tx: &QuantumTransaction,
        public_key: &[u8],
    ) -> Result<ValidationResult, ValidationError> {
        let start = std::time::Instant::now();
        
        // Check if quantum features are enabled
        if !self.config.crypto.quantum.enabled {
            return Err(ValidationError::ConfigError(
                "Quantum cryptography features are disabled".to_string()
            ));
        }
        
        let mut issues = Vec::new();
        let mut security_score = 100;
        let mut verification_ops = 1;
        
        // First validate the base transaction
        let base_result = self.validate_transaction(tx.transaction())?;
        issues.extend(base_result.issues.clone());
        security_score = security_score.min(base_result.security_score);
        
        // Validate quantum signature
        match tx.verify_signature(public_key) {
            Ok(true) => {
                // Signature is valid
            }
            Ok(false) => {
                issues.push("Quantum signature verification failed".to_string());
                security_score -= 50;
            }
            Err(e) => {
                return Err(ValidationError::QuantumError(e));
            }
        }
        
        verification_ops += 1;
        
        // Check scheme-specific security aspects
        match tx.scheme() {
            crate::crypto::quantum::QuantumScheme::Dilithium => {
                // Check security level adequacy
                if tx.security_level() < 3 && self.security_level == SecurityLevel::Maximum {
                    issues.push("Dilithium security level below recommended for maximum security".to_string());
                    security_score -= 20;
                }
            }
            crate::crypto::quantum::QuantumScheme::Sphincs => {
                // SPHINCS+ is slower but has stronger security guarantees
                if self.security_level == SecurityLevel::Enhanced || 
                   self.security_level == SecurityLevel::Maximum {
                    // Bonus for using SPHINCS+
                    security_score = (security_score + 5).min(100);
                }
            }
            crate::crypto::quantum::QuantumScheme::Hybrid(_) => {
                // Hybrid schemes provide best security
                if self.security_level == SecurityLevel::Maximum {
                    // Bonus for using hybrid scheme at maximum security
                    security_score = (security_score + 10).min(100);
                }
            }
            _ => {}
        }
        
        // Check signature size relative to transaction size
        let tx_serialized = bincode::serialize(tx.transaction())
            .map_err(|e| ValidationError::FormatError(e.to_string()))?;
        
        if tx.signature().len() > tx_serialized.len() * 2 && self.security_level != SecurityLevel::Standard {
            issues.push("Signature size is unusually large relative to transaction size".to_string());
            security_score -= 5;
        }
        
        // Limit security score to valid range
        security_score = security_score.clamp(0, 100);
        
        let validation_time = start.elapsed();
        
        Ok(ValidationResult {
            is_valid: issues.is_empty() || security_score > 50,
            issues,
            security_score,
            metrics: ValidationMetrics {
                validation_time_ms: validation_time.as_millis() as u64,
                transaction_size: tx_serialized.len() + tx.signature().len(),
                verification_ops,
            },
        })
    }
    
    /// Validate a confidential transaction
    pub fn validate_confidential_transaction(
        &self, 
        tx: &ConfidentialTransaction,
    ) -> Result<ValidationResult, ValidationError> {
        let start = std::time::Instant::now();
        
        // Check if ZKP features are enabled
        if !self.config.crypto.zkp.enabled {
            return Err(ValidationError::ConfigError(
                "Zero-knowledge proof features are disabled".to_string()
            ));
        }
        
        let mut issues = Vec::new();
        let mut security_score = 100;
        let mut verification_ops = 1;
        
        // Serialized size check
        let tx_serialized = bincode::serialize(tx)
            .map_err(|e| ValidationError::FormatError(e.to_string()))?;
        
        if tx_serialized.len() > self.config.max_tx_size {
            return Err(ValidationError::InvalidTransaction(
                format!("Transaction size exceeds maximum: {} > {}", 
                    tx_serialized.len(), self.config.max_tx_size)
            ));
        }
        
        // Basic structure checks
        if tx.inputs().is_empty() {
            issues.push("Transaction has no inputs".to_string());
            security_score -= 50;
        }
        
        if tx.conf_outputs().is_empty() {
            issues.push("Transaction has no outputs".to_string());
            security_score -= 50;
        }
        
        // Range proof verification
        if !tx.verify_range_proofs() {
            return Err(ValidationError::ZkpError(
                ZkpError::VerificationFailed("Range proof verification failed".to_string())
            ));
        }
        
        verification_ops += tx.conf_outputs().len() as u32;
        
        // Enhanced security checks
        if self.security_level != SecurityLevel::Standard {
            // Check for unusually large number of outputs
            if tx.conf_outputs().len() > self.config.crypto.zkp.max_range_proofs / 2 {
                issues.push(format!(
                    "High number of confidential outputs: {}", 
                    tx.conf_outputs().len()
                ));
                security_score -= 10;
            }
            
            // Check proof types and sizes
            for output in tx.conf_outputs() {
                match output.range_proof().proof_type {
                    crate::crypto::zkp::ZkpType::Bulletproof => {
                        // Bulletproofs are compact and efficient - preferred
                    }
                    crate::crypto::zkp::ZkpType::RangeProof => {
                        // Simple range proofs are less efficient
                        if self.security_level == SecurityLevel::Maximum {
                            issues.push("Using simple range proofs instead of Bulletproofs".to_string());
                            security_score -= 5;
                        }
                    }
                    _ => {
                        // Other proof types might be experimental
                        if self.security_level == SecurityLevel::Maximum {
                            issues.push(format!(
                                "Using non-standard proof type: {:?}", 
                                output.range_proof().proof_type
                            ));
                            security_score -= 10;
                        }
                    }
                }
                
                // Check commitment size
                if output.amount_commitment().value.len() != 32 {
                    issues.push("Non-standard commitment size".to_string());
                    security_score -= 10;
                }
            }
        }
        
        // Limit security score to valid range
        security_score = security_score.clamp(0, 100);
        
        let validation_time = start.elapsed();
        
        Ok(ValidationResult {
            is_valid: issues.is_empty() || security_score > 50,
            issues,
            security_score,
            metrics: ValidationMetrics {
                validation_time_ms: validation_time.as_millis() as u64,
                transaction_size: tx_serialized.len(),
                verification_ops,
            },
        })
    }
} 