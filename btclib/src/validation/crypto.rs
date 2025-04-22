use crate::crypto::{
    signature::{SignatureType, SignatureError, SignatureVerifier},
    quantum::{QuantumScheme, ClassicalScheme}
};

use std::collections::HashMap;

/// Modes for signature validation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValidationMode {
    /// Standard validation for each signature
    Standard,
    /// Strict mode that requires quantum resistance
    StrictQuantumResistant,
    /// Performance mode that prioritizes speed over security
    Performance,
}

impl Default for ValidationMode {
    fn default() -> Self {
        ValidationMode::Standard
    }
}

/// Validates cryptographic signatures with support for batch verification
pub struct SignatureValidator {
    mode: ValidationMode,
    verifier: SignatureVerifier,
    acceptable_schemes: Vec<SignatureType>,
}

impl SignatureValidator {
    /// Create a new signature validator with default settings
    pub fn new() -> Self {
        Self {
            mode: ValidationMode::default(),
            verifier: SignatureVerifier::new(),
            acceptable_schemes: vec![
                SignatureType::Classical(ClassicalScheme::Secp256k1),
                SignatureType::Classical(ClassicalScheme::Ed25519),
                SignatureType::Quantum(QuantumScheme::Dilithium),
                SignatureType::Quantum(QuantumScheme::Falcon),
            ],
        }
    }
    
    /// Create a validator with a specific validation mode
    pub fn with_mode(mode: ValidationMode) -> Self {
        let mut validator = Self::new();
        validator.mode = mode;
        
        // Adjust acceptable schemes based on mode
        match mode {
            ValidationMode::StrictQuantumResistant => {
                validator.acceptable_schemes = vec![
                    SignatureType::Quantum(QuantumScheme::Dilithium),
                    SignatureType::Quantum(QuantumScheme::Falcon),
                    SignatureType::Quantum(QuantumScheme::Sphincs),
                ];
            },
            ValidationMode::Performance => {
                // Prioritize faster signature schemes
                validator.acceptable_schemes = vec![
                    SignatureType::Classical(ClassicalScheme::Ed25519),
                    SignatureType::Classical(ClassicalScheme::Secp256k1),
                    SignatureType::Quantum(QuantumScheme::Falcon), // Falcon is typically faster than Dilithium
                ];
            },
            _ => {} // Keep defaults for standard mode
        }
        
        validator
    }
    
    /// Set custom acceptable signature schemes
    pub fn with_schemes(mut self, schemes: Vec<SignatureType>) -> Self {
        self.acceptable_schemes = schemes;
        self
    }
    
    /// Validate a single signature
    pub fn validate(
        &self,
        signature_type: SignatureType,
        public_key: &[u8],
        message: &[u8],
        signature: &[u8],
    ) -> Result<bool, SignatureError> {
        // Check if signature type is acceptable based on mode
        if !self.acceptable_schemes.contains(&signature_type) {
            return Err(SignatureError::UnsupportedSignatureType);
        }
        
        // In strict mode, verify quantum resistance
        if self.mode == ValidationMode::StrictQuantumResistant {
            match signature_type {
                SignatureType::Quantum(_) => {}, // Quantum signature is acceptable
                _ => return Err(SignatureError::QuantumResistanceRequired),
            }
        }
        
        // Verify the signature
        self.verifier.verify(signature_type, public_key, message, signature)
    }
    
    /// Batch verify multiple signatures for efficiency
    /// 
    /// Returns a map of (signature_index -> validation_result)
    pub fn batch_validate(
        &self,
        signatures: Vec<(SignatureType, &[u8], &[u8], &[u8])>,
    ) -> HashMap<usize, Result<bool, SignatureError>> {
        let mut results = HashMap::new();
        
        for (index, (sig_type, public_key, message, signature)) in signatures.into_iter().enumerate() {
            let result = self.validate(sig_type, public_key, message, signature);
            results.insert(index, result);
        }
        
        results
    }
    
    /// Get the current validation mode
    pub fn mode(&self) -> ValidationMode {
        self.mode
    }
    
    /// Get the list of acceptable signature schemes
    pub fn acceptable_schemes(&self) -> &[SignatureType] {
        &self.acceptable_schemes
    }
} 