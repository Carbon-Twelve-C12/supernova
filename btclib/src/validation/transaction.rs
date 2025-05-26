// Transaction validation module for SuperNova blockchain

use serde::{Serialize, Deserialize};
use crate::types::transaction::{Transaction, TransactionOutput, SignatureSchemeType};
use crate::crypto::signature::{SignatureType, SignatureParams};
use crate::crypto::quantum::{QuantumScheme, QuantumKeyPair, QuantumParameters};
use crate::validation::crypto::{CryptoValidator, CryptoValidationConfig};
use crate::validation::{SecurityLevel, ValidationError, ValidationMetrics};

/// Transaction validation results
#[derive(Debug)]
pub enum ValidationResult {
    /// Transaction is valid
    Valid,
    
    /// Transaction is invalid
    Invalid(ValidationError),
    
    /// Transaction validation had non-critical issues
    SoftFail(ValidationError),
}

/// Configuration for transaction validation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationConfig {
    /// Minimum transaction version
    pub min_version: u32,
    
    /// Maximum transaction size (in bytes)
    pub max_size: usize,
    
    /// Maximum number of inputs
    pub max_inputs: usize,
    
    /// Maximum number of outputs
    pub max_outputs: usize,
    
    /// Security level for validation
    pub security_level: SecurityLevel,
    
    /// Require quantum resistance
    pub require_quantum_resistance: bool,
    
    /// Enable ZKP verification
    pub enable_zkp: bool,
    
    /// Maximum dust output value (in satoshis)
    pub dust_threshold: u64,
    
    /// Allow zero-value outputs
    pub allow_zero_value: bool,
    
    /// Minimum fee rate (satoshis per byte)
    pub min_fee_rate: u64,
    
    /// Maximum fee rate (satoshis per byte) - to prevent fee sniping
    pub max_fee_rate: u64,
}

impl Default for ValidationConfig {
    fn default() -> Self {
        Self {
            min_version: 1,
            max_size: 1_000_000, // 1MB
            max_inputs: 10_000,
            max_outputs: 10_000,
            security_level: SecurityLevel::Standard,
            require_quantum_resistance: false,
            enable_zkp: true,
            dust_threshold: 546, // Similar to Bitcoin's dust threshold
            allow_zero_value: false,
            min_fee_rate: 1,
            max_fee_rate: 1_000_000, // Very high to effectively disable the check
        }
    }
}

/// Transaction validator for standard and extended transactions
pub struct TransactionValidator {
    /// Configuration
    pub config: ValidationConfig,
    
    /// Crypto validator for signature verification
    crypto_validator: CryptoValidator,
}

impl TransactionValidator {
    /// Create a new transaction validator with default config
    pub fn new() -> Self {
        let config = ValidationConfig::default();
        let crypto_config = CryptoValidationConfig {
            security_level: config.security_level,
            require_quantum_resistance: config.require_quantum_resistance,
            min_signature_security_level: 2,
            allowed_signature_types: Vec::new(), // Will be populated in constructor
        };
        
        Self {
            config,
            crypto_validator: CryptoValidator::new(crypto_config),
        }
    }

    /// Create a new transaction validator with custom config
    pub fn with_config(config: ValidationConfig) -> Self {
        let crypto_config = CryptoValidationConfig {
            security_level: config.security_level,
            require_quantum_resistance: config.require_quantum_resistance,
            min_signature_security_level: 2,
            allowed_signature_types: Vec::new(), // Will be populated in constructor
        };
        
        Self {
            config,
            crypto_validator: CryptoValidator::new(crypto_config),
        }
    }
    
    /// Validate a transaction
    pub fn validate(&self, tx: &Transaction) -> Result<ValidationResult, ValidationError> {
        let mut metrics = ValidationMetrics::default();
        let start_time = std::time::Instant::now();
        
        // Perform validation checks in order of complexity (cheaper checks first)
        
        // 1. Basic structure validation
        if let Err(err) = self.validate_structure(tx) {
            return Ok(ValidationResult::Invalid(err));
        }
        
        // 2. Size validation
        if let Err(err) = self.validate_size(tx) {
            return Ok(ValidationResult::Invalid(err));
        }
        
        // 3. Version check
        if tx.version() < self.config.min_version {
            return Ok(ValidationResult::Invalid(
                ValidationError::InvalidStructure(format!(
                    "Transaction version too low: {} (minimum: {})",
                    tx.version(), self.config.min_version
                ))
            ));
        }
        
        // 4. Check for dust outputs if configured
        if !self.config.allow_zero_value {
            for (i, output) in tx.outputs().iter().enumerate() {
                if output.amount() == 0 {
                    return Ok(ValidationResult::Invalid(
                        ValidationError::InvalidStructure(format!(
                            "Zero value output at index {}", i
                        ))
                    ));
                }
            }
        }
        
        if self.config.dust_threshold > 0 {
            for (i, output) in tx.outputs().iter().enumerate() {
                if output.amount() > 0 && output.amount() < self.config.dust_threshold {
                    return Ok(ValidationResult::Invalid(
                        ValidationError::InvalidStructure(format!(
                            "Dust output at index {}: {} (minimum: {})",
                            i, output.amount(), self.config.dust_threshold
                        ))
                    ));
                }
            }
        }
        
        // 5. Validate transaction signatures (this requires access to UTXO set)
        // Note: For full validation, we would need to pass in a function to retrieve previous outputs
        // and verify signatures against them. For now, we'll assume this validation happens elsewhere.
        
        // 6. For quantum resistance (if required), check signature scheme
        if self.config.require_quantum_resistance {
            if let Some(sig_data) = tx.signature_data() {
                match sig_data.scheme {
                    SignatureSchemeType::Legacy |
                    SignatureSchemeType::Ed25519 => {
                        return Ok(ValidationResult::Invalid(
                            ValidationError::InvalidSignature(
                                "Quantum-resistant signature required".to_string()
                            )
                        ));
                    },
                    // These schemes are quantum-resistant
                    SignatureSchemeType::Dilithium |
                    SignatureSchemeType::Falcon |
                    SignatureSchemeType::Sphincs |
                    SignatureSchemeType::Hybrid => {
                        // Acceptable quantum-resistant schemes
                    }
                }
            } else if tx.version() >= 2 {
                // Version 2+ transactions without signature data when quantum resistance is required
                return Ok(ValidationResult::Invalid(
                    ValidationError::InvalidSignature(
                        "Quantum-resistant signature required for v2+ transactions".to_string()
                    )
                ));
            }
        }
        
        // Update metrics
        metrics.total_validations += 1;
        let elapsed = start_time.elapsed().as_millis() as f64;
        metrics.avg_validation_time_ms = ((metrics.avg_validation_time_ms * (metrics.total_validations - 1) as f64) + elapsed) / metrics.total_validations as f64;
        if elapsed > metrics.max_validation_time_ms {
            metrics.max_validation_time_ms = elapsed;
        }
        
        // All validation passed
        Ok(ValidationResult::Valid)
    }
    
    /// Validate transaction structure
    fn validate_structure(&self, tx: &Transaction) -> Result<(), ValidationError> {
        // Check that transaction has at least one input and one output (except for special coinbase tx)
        if tx.inputs().is_empty() {
            return Err(ValidationError::InvalidStructure("Transaction has no inputs".to_string()));
        }
        
        if tx.outputs().is_empty() {
            return Err(ValidationError::InvalidStructure("Transaction has no outputs".to_string()));
        }
        
        // Check input and output limits
        if tx.inputs().len() > self.config.max_inputs {
            return Err(ValidationError::InvalidStructure(format!(
                "Too many inputs: {} (maximum: {})",
                tx.inputs().len(), self.config.max_inputs
            )));
        }
        
        if tx.outputs().len() > self.config.max_outputs {
            return Err(ValidationError::InvalidStructure(format!(
                "Too many outputs: {} (maximum: {})",
                tx.outputs().len(), self.config.max_outputs
            )));
        }
        
        // For non-coinbase transactions, verify inputs are not referencing empty txids
        if !tx.is_coinbase() {
            for (i, input) in tx.inputs().iter().enumerate() {
                if input.prev_tx_hash() == [0u8; 32] {
                    return Err(ValidationError::InvalidStructure(format!(
                        "Input {} references null transaction", i
                    )));
                }
            }
        }
        
        Ok(())
    }
    
    /// Validate transaction size
    fn validate_size(&self, tx: &Transaction) -> Result<(), ValidationError> {
        let size = tx.calculate_size();
        
        if size > self.config.max_size {
            return Err(ValidationError::InvalidStructure(format!(
                "Transaction too large: {} bytes (maximum: {} bytes)",
                size, self.config.max_size
            )));
        }
        
        Ok(())
    }
    
    /// Validate transaction fee rate
    pub fn validate_fee_rate(&self, tx: &Transaction, get_output: impl Fn(&[u8; 32], u32) -> Option<TransactionOutput>) -> Result<ValidationResult, ValidationError> {
        // Skip fee check for coinbase transactions
        if tx.is_coinbase() {
            return Ok(ValidationResult::Valid);
        }
        
        // Calculate the fee rate
        if let Some(fee_rate) = tx.calculate_fee_rate(&get_output) {
            // Check minimum fee rate
            if fee_rate < self.config.min_fee_rate {
                return Ok(ValidationResult::Invalid(
                    ValidationError::InvalidStructure(format!(
                        "Fee rate too low: {} satoshis/byte (minimum: {})",
                        fee_rate, self.config.min_fee_rate
                    ))
                ));
            }
            
            // Check maximum fee rate (prevent fee sniping)
            if fee_rate > self.config.max_fee_rate {
                return Ok(ValidationResult::SoftFail(
                    ValidationError::InvalidStructure(format!(
                        "Fee rate suspiciously high: {} satoshis/byte (maximum: {})",
                        fee_rate, self.config.max_fee_rate
                    ))
                ));
            }
            
            Ok(ValidationResult::Valid)
        } else {
            // Couldn't calculate fee rate (missing inputs or other error)
            Ok(ValidationResult::Invalid(
                ValidationError::InvalidStructure("Could not calculate fee rate".to_string())
            ))
        }
    }
    
    /// Validate transaction signatures
    pub fn validate_signatures(&self, tx: &Transaction, get_output: impl Fn(&[u8; 32], u32) -> Option<TransactionOutput>) -> Result<ValidationResult, ValidationError> {
        // For coinbase transactions, no signature validation is needed
        if tx.is_coinbase() {
            return Ok(ValidationResult::Valid);
        }
        
        // Check if transaction has extended signature data
        if let Some(sig_data) = tx.signature_data() {
            // Validate the extended signature (version 2+ transactions)
            // In a real implementation, we would parse the signature data and verify it
            // against the transaction hash. For now, we'll use the transaction's own verify method.
            let message_hash = tx.hash();
            
            // Convert SignatureSchemeType to our internal SignatureType
            let sig_type = match sig_data.scheme {
                SignatureSchemeType::Legacy => SignatureType::Secp256k1,
                SignatureSchemeType::Ed25519 => SignatureType::Ed25519,
                SignatureSchemeType::Dilithium => SignatureType::Dilithium,
                SignatureSchemeType::Falcon => SignatureType::Falcon,
                SignatureSchemeType::Sphincs => SignatureType::Sphincs,
                SignatureSchemeType::Hybrid => SignatureType::Hybrid,
            };
            
            // Create signature params
            let params = SignatureParams {
                sig_type,
                security_level: 1,
                enable_batch: false,
                additional_params: std::collections::HashMap::new(),
            };
        
        // Verify the signature
            match self.crypto_validator.validate_signature(
                &sig_data.data,
                &sig_data.public_key,
                &message_hash,
                params
            ) {
                Ok(valid) => {
                    if valid {
                        Ok(ValidationResult::Valid)
                    } else {
                        Ok(ValidationResult::Invalid(
                            ValidationError::InvalidSignature("Extended signature verification failed".to_string())
                        ))
                    }
                },
                Err(e) => Ok(ValidationResult::Invalid(e)),
            }
        } else {
            // For legacy transactions, verify each input signature
            let mut valid_inputs = 0;
            
            for (i, input) in tx.inputs().iter().enumerate() {
                // Get the previous output being spent
                let prev_output = match get_output(&input.prev_tx_hash(), input.prev_output_index()) {
                    Some(output) => output,
                    None => {
                        return Ok(ValidationResult::Invalid(
                            ValidationError::InvalidStructure(format!(
                                "Previous output not found for input {}", i
                            ))
                        ));
                    }
                };
                
                // Use the transaction's own verification logic
                if tx.verify_signature(&input.signature_script(), &prev_output.pub_key_script, i) {
                    valid_inputs += 1;
                } else {
                    return Ok(ValidationResult::Invalid(
                        ValidationError::InvalidSignature(format!(
                            "Signature verification failed for input {}", i
                        ))
                    ));
                }
            }
            
            // Ensure all inputs were validated
            if valid_inputs == tx.inputs().len() {
                Ok(ValidationResult::Valid)
            } else {
                Ok(ValidationResult::Invalid(
                    ValidationError::InvalidSignature(format!(
                        "Only {} of {} input signatures verified", valid_inputs, tx.inputs().len()
                    ))
                ))
            }
        }
    }
    
    /// Check if transaction outputs exceed inputs
    pub fn validate_output_value(&self, tx: &Transaction, get_output: impl Fn(&[u8; 32], u32) -> Option<TransactionOutput>) -> Result<ValidationResult, ValidationError> {
        // Skip for coinbase transactions which can create new coins
        if tx.is_coinbase() {
            return Ok(ValidationResult::Valid);
        }
        
        match tx.total_input(&get_output) {
            Some(total_in) => {
                let total_out = tx.total_output();
                
                if total_out > total_in {
                    return Ok(ValidationResult::Invalid(
                        ValidationError::InvalidStructure(format!(
                            "Outputs exceed inputs: {} > {}", total_out, total_in
                        ))
                    ));
                }
                
                // Calculate fee
                let fee = total_in - total_out;
                
                // Check for suspiciously high fees (more than 25% of transaction value)
                if fee > total_in / 4 && fee > 1_000_000 { // Only warn if fee > 0.01 NOVA
                    return Ok(ValidationResult::SoftFail(
                        ValidationError::InvalidStructure(format!(
                            "Suspiciously high fee: {} ({}% of input value)",
                            fee, (fee * 100) / total_in
                        ))
                    ));
                }
                
                Ok(ValidationResult::Valid)
            },
            None => {
                Ok(ValidationResult::Invalid(
                    ValidationError::InvalidStructure("Could not determine total input value".to_string())
                ))
            }
        }
    }
    
    /// Sign a transaction using quantum-resistant signature scheme
    pub fn sign_quantum_transaction(&self, transaction: &mut Transaction, scheme: QuantumScheme) -> Result<ValidationResult, ValidationError> {
        let keypair = QuantumKeyPair {
            public_key: vec![],
            secret_key: vec![],
            parameters: QuantumParameters {
                scheme,
                security_level: self.config.security_level.into(),
            },
        };
        
        // Sign the transaction using the selected signature scheme
        let msg = transaction.hash();
        
        match keypair.sign(&msg) {
            Ok(signature) => {
                // Create signature data for the transaction
                let sig_data = crate::types::transaction::TransactionSignatureData {
                    scheme: match scheme {
                        QuantumScheme::Dilithium => SignatureSchemeType::Dilithium,
                        QuantumScheme::Falcon => SignatureSchemeType::Falcon,
                        QuantumScheme::Sphincs => SignatureSchemeType::Sphincs,
                        QuantumScheme::Hybrid(_) => SignatureSchemeType::Hybrid,
                    },
                    security_level: self.config.security_level.into(),
                    data: signature,
                    public_key: keypair.public_key.to_vec(),
                };
                
                // Add the signature data to the transaction
                transaction.set_signature_data(sig_data);
                
                Ok(ValidationResult::Valid)
            },
            Err(e) => Ok(ValidationResult::Invalid(ValidationError::SignatureError(e.to_string()))),
        }
    }
    
    /// Verify a transaction using quantum-resistant signature scheme
    pub fn verify_quantum_transaction(&self, transaction: &Transaction) -> Result<ValidationResult, ValidationError> {
        if let Some(sig_data) = transaction.signature_data() {
            // Create parameters for verification
            let params = QuantumParameters {
                scheme: match sig_data.scheme {
                    SignatureSchemeType::Dilithium => QuantumScheme::Dilithium,
                    SignatureSchemeType::Falcon => QuantumScheme::Falcon,
                    SignatureSchemeType::Sphincs => QuantumScheme::Sphincs,
                    SignatureSchemeType::Hybrid => QuantumScheme::Hybrid(crate::crypto::quantum::ClassicalScheme::Secp256k1),
                    _ => return Ok(ValidationResult::Invalid(ValidationError::InvalidSignatureScheme)),
                },
                // Convert security level from u8 to the corresponding enum value
                security_level: sig_data.security_level,
            };
            
            // Get the message hash (transaction hash excluding signature data)
            let msg = transaction.hash();
            
            // Verify the signature
            match crate::crypto::quantum::verify_quantum_signature(
                &sig_data.public_key,
                &msg,
                &sig_data.data,
                params
            ) {
                Ok(true) => Ok(ValidationResult::Valid),
                Ok(false) => Ok(ValidationResult::Invalid(ValidationError::InvalidSignature("Signature verification failed".to_string()))),
                Err(e) => Ok(ValidationResult::Invalid(ValidationError::SignatureError(e.to_string()))),
            }
        } else {
            Ok(ValidationResult::Invalid(ValidationError::MissingSignatureData))
        }
    }
} 