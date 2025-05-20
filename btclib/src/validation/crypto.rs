// Cryptographic validation module
//
// This module provides validation for cryptographic primitives.

use crate::crypto::signature::{SignatureType, SignatureError, SignatureParams, SignatureVerifier};
use crate::crypto::quantum::{QuantumScheme, ClassicalScheme, QuantumParameters, verify_quantum_signature};
use super::{ValidationError, SecurityLevel};

/// Validation mode for cryptographic operations
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValidationMode {
    /// Standard validation mode
    Standard,
    /// Permissive validation mode (allow some non-critical errors)
    Permissive,
    /// Strict validation mode (fail on any error)
    Strict,
}

/// Configuration for cryptographic validation
#[derive(Debug, Clone)]
pub struct CryptoValidationConfig {
    /// Security level for validation
    pub security_level: SecurityLevel,
    
    /// Whether quantum resistance is required 
    pub require_quantum_resistance: bool,
    
    /// Minimum signature security level
    pub min_signature_security_level: u8,
    
    /// Allowed signature types
    pub allowed_signature_types: Vec<SignatureType>,
}

impl Default for CryptoValidationConfig {
    fn default() -> Self {
        Self {
            security_level: SecurityLevel::Standard,
            require_quantum_resistance: false,
            min_signature_security_level: 2,
            allowed_signature_types: vec![
                SignatureType::Secp256k1,
                SignatureType::Ed25519,
                SignatureType::Classical(ClassicalScheme::Secp256k1),
                SignatureType::Classical(ClassicalScheme::Ed25519),
                SignatureType::Dilithium,
                SignatureType::Falcon,
                SignatureType::Sphincs,
                SignatureType::Quantum(QuantumScheme::Dilithium),
                SignatureType::Quantum(QuantumScheme::Falcon),
                SignatureType::Quantum(QuantumScheme::Sphincs),
            ],
        }
    }
}

/// Create a security level configuration based on a security level
pub fn create_config_for_security_level(level: SecurityLevel) -> CryptoValidationConfig {
    match level {
        SecurityLevel::Low => CryptoValidationConfig {
            security_level: level,
            require_quantum_resistance: false,
            min_signature_security_level: 1,
            allowed_signature_types: vec![
                SignatureType::Secp256k1,
                SignatureType::Ed25519,
                SignatureType::Classical(ClassicalScheme::Secp256k1),
                SignatureType::Classical(ClassicalScheme::Ed25519),
                SignatureType::Dilithium,
                SignatureType::Falcon,
                SignatureType::Quantum(QuantumScheme::Dilithium),
                SignatureType::Quantum(QuantumScheme::Falcon),
            ],
        },
        SecurityLevel::Standard => CryptoValidationConfig::default(),
        SecurityLevel::Medium => CryptoValidationConfig {
            security_level: level,
            require_quantum_resistance: false,
            min_signature_security_level: 3,
            allowed_signature_types: vec![
                    SignatureType::Classical(ClassicalScheme::Ed25519),
                    SignatureType::Classical(ClassicalScheme::Secp256k1),
                    SignatureType::Quantum(QuantumScheme::Falcon), // Falcon is typically faster than Dilithium
                    SignatureType::Quantum(QuantumScheme::Dilithium),
                    SignatureType::Quantum(QuantumScheme::Sphincs),
                    SignatureType::Dilithium,
                    SignatureType::Falcon,
                    SignatureType::Sphincs,
            ],
        },
        SecurityLevel::High => CryptoValidationConfig {
            security_level: level,
            require_quantum_resistance: true,
            min_signature_security_level: 5,
            allowed_signature_types: vec![
                SignatureType::Quantum(QuantumScheme::Dilithium),
                SignatureType::Quantum(QuantumScheme::Sphincs),
                SignatureType::Dilithium,
                SignatureType::Sphincs,
            ],
        },
        SecurityLevel::Maximum => CryptoValidationConfig {
            security_level: level,
            require_quantum_resistance: true,
            min_signature_security_level: 5,
            allowed_signature_types: vec![
                SignatureType::Quantum(QuantumScheme::Sphincs),
                SignatureType::Sphincs,
            ],
        },
        _ => CryptoValidationConfig::default(),
    }
}

/// Cryptographic validator for verifying signatures and other cryptographic operations
pub struct CryptoValidator {
    config: CryptoValidationConfig,
}

impl CryptoValidator {
    /// Create a new crypto validator
    pub fn new(config: CryptoValidationConfig) -> Self {
        Self { config }
    }
    
    /// Validate a signature type against security requirements
    pub fn validate_signature_type(&self, sig_type: SignatureType) -> Result<(), SignatureError> {
        if !self.config.allowed_signature_types.contains(&sig_type) {
            return Err(SignatureError::UnsupportedSignatureType);
        }
        
        // Check if quantum resistance is required
        if self.config.require_quantum_resistance {
            match sig_type {
                SignatureType::Quantum(_) => {}, // Quantum signature is acceptable
                SignatureType::Dilithium |
                SignatureType::Falcon |
                SignatureType::Sphincs => {}, // These are also quantum resistant
                _ => return Err(SignatureError::QuantumResistanceRequired),
            }
        }
        
        Ok(())
    }
    
    /// Validate a signature with the given parameters
    pub fn validate_signature(
        &self,
        signature: &[u8],
        public_key: &[u8],
        message: &[u8],
        params: SignatureParams,
    ) -> Result<bool, ValidationError> {
        // Check signature type
        self.validate_signature_type(params.sig_type)
            .map_err(|e| ValidationError::SignatureError(format!("Invalid signature type: {}", e)))?;
        
        // Check security level
        if params.security_level < self.config.min_signature_security_level {
            return Err(ValidationError::SignatureError(format!(
                "Signature security level too low: {} (minimum: {})",
                params.security_level, self.config.min_signature_security_level
            )));
        }
        
        // Determine the verification method based on the signature type
        match params.sig_type {
            SignatureType::Quantum(scheme) => {
                // For quantum signatures, use the dedicated verification function
                let quantum_params = QuantumParameters {
                    scheme,
                    security_level: params.security_level,
                };
                
                match verify_quantum_signature(public_key, message, signature, quantum_params) {
                    Ok(valid) => Ok(valid),
                    Err(e) => Err(ValidationError::SignatureError(
                        format!("Quantum signature verification error: {}", e)
                    )),
                }
            },
            // For classical or other signature types, use the general verifier
            _ => {
                let verifier = SignatureVerifier::new();
                
                match verifier.verify(params.sig_type, public_key, message, signature) {
                    Ok(valid) => Ok(valid),
                    Err(e) => Err(ValidationError::SignatureError(
                        format!("Signature verification error: {}", e)
                    )),
                }
            }
        }
    }
}

/// Alias for backward compatibility
pub type SignatureValidator = CryptoValidator;