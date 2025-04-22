use crate::{
    crypto::{signature::{SignatureVerifier, SignatureType}, quantum::QuantumScheme},
    environmental::emissions::EmissionsTracker,
    types::transaction::Transaction,
};

use std::fmt;

/// Defines the result of transaction validation
#[derive(Debug, Clone)]
pub struct ValidationResult {
    /// Whether the transaction is valid
    pub is_valid: bool,
    /// Signature verification status
    pub signature_valid: bool,
    /// Emissions compliance status
    pub emissions_compliant: bool,
    /// Detailed validation message
    pub message: String,
}

/// Configuration for transaction validation
#[derive(Debug, Clone)]
pub struct ValidationConfig {
    /// Whether to check emissions compliance
    pub check_emissions: bool,
    /// Maximum allowed carbon intensity (g CO2e/byte)
    pub max_carbon_intensity: Option<f64>,
    /// Whether to require carbon neutrality
    pub require_carbon_neutral: bool,
    /// Signature types that are acceptable
    pub acceptable_signatures: Vec<SignatureType>,
    /// Whether to require quantum resistance
    pub require_quantum_resistance: bool,
}

impl Default for ValidationConfig {
    fn default() -> Self {
        Self {
            check_emissions: false,
            max_carbon_intensity: None,
            require_carbon_neutral: false,
            acceptable_signatures: vec![
                SignatureType::Classical(crate::crypto::quantum::ClassicalScheme::Secp256k1),
                SignatureType::Classical(crate::crypto::quantum::ClassicalScheme::Ed25519),
                SignatureType::Quantum(QuantumScheme::Dilithium),
                SignatureType::Quantum(QuantumScheme::Falcon),
            ],
            require_quantum_resistance: false,
        }
    }
}

/// Errors that can occur during transaction validation
#[derive(Debug)]
pub enum ValidationError {
    /// Missing signature
    MissingSignature,
    /// Invalid signature
    InvalidSignature(String),
    /// Emissions threshold exceeded
    EmissionsThresholdExceeded(f64, f64),
    /// Carbon neutrality required but not met
    CarbonNeutralityRequired,
    /// Quantum resistance required but not provided
    QuantumResistanceRequired,
    /// Unsupported signature type
    UnsupportedSignatureType(SignatureType),
    /// Failed to calculate emissions
    EmissionsCalculationFailed(String),
    /// Other validation error
    Other(String),
}

impl fmt::Display for ValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ValidationError::MissingSignature => write!(f, "Transaction is missing a signature"),
            ValidationError::InvalidSignature(msg) => write!(f, "Invalid signature: {}", msg),
            ValidationError::EmissionsThresholdExceeded(actual, max) => 
                write!(f, "Carbon intensity of {}g CO2e/byte exceeds maximum threshold of {}g CO2e/byte", 
                    actual, max),
            ValidationError::CarbonNeutralityRequired => 
                write!(f, "Transaction is not carbon neutral as required by validation policy"),
            ValidationError::QuantumResistanceRequired => 
                write!(f, "Transaction requires quantum-resistant signature but none was provided"),
            ValidationError::UnsupportedSignatureType(sig_type) => 
                write!(f, "Unsupported signature type: {:?}", sig_type),
            ValidationError::EmissionsCalculationFailed(msg) => 
                write!(f, "Failed to calculate emissions: {}", msg),
            ValidationError::Other(msg) => write!(f, "{}", msg),
        }
    }
}

impl std::error::Error for ValidationError {}

/// Transaction validator that combines cryptographic and emissions validation
pub struct TransactionValidator {
    config: ValidationConfig,
    emissions_tracker: Option<EmissionsTracker>,
    signature_verifier: SignatureVerifier,
}

impl TransactionValidator {
    /// Create a new transaction validator with default configuration
    pub fn new() -> Self {
        Self {
            config: ValidationConfig::default(),
            emissions_tracker: None,
            signature_verifier: SignatureVerifier::new(),
        }
    }

    /// Create a transaction validator with a specific configuration
    pub fn with_config(config: ValidationConfig) -> Self {
        Self {
            config,
            emissions_tracker: None,
            signature_verifier: SignatureVerifier::new(),
        }
    }

    /// Set the emissions tracker for this validator
    pub fn with_emissions_tracker(mut self, tracker: EmissionsTracker) -> Self {
        self.emissions_tracker = Some(tracker);
        self
    }

    /// Validate a transaction against the configured rules
    pub fn validate(&self, transaction: &Transaction) -> Result<ValidationResult, ValidationError> {
        let mut result = ValidationResult {
            is_valid: false,
            signature_valid: false,
            emissions_compliant: false,
            message: String::new(),
        };

        // Verify signature
        match self.verify_signature(transaction) {
            Ok(_) => {
                result.signature_valid = true;
            }
            Err(err) => {
                result.message = format!("Signature verification failed: {}", err);
                return Err(err);
            }
        }

        // Check emissions compliance if enabled
        if self.config.check_emissions {
            match self.verify_emissions(transaction) {
                Ok(_) => {
                    result.emissions_compliant = true;
                }
                Err(err) => {
                    result.message = format!("Emissions compliance failed: {}", err);
                    return Err(err);
                }
            }
        } else {
            // Skip emissions check if not enabled
            result.emissions_compliant = true;
        }

        // If we reached here, the transaction is valid
        result.is_valid = true;
        result.message = "Transaction validated successfully".to_string();
        Ok(result)
    }

    /// Verify the transaction signature
    fn verify_signature(&self, transaction: &Transaction) -> Result<(), ValidationError> {
        // Check if transaction has a signature
        let signature = transaction.signature().ok_or(ValidationError::MissingSignature)?;
        
        // Check if signature type is acceptable
        if !self.config.acceptable_signatures.contains(&signature.signature_type) {
            return Err(ValidationError::UnsupportedSignatureType(signature.signature_type));
        }
        
        // Check if quantum resistance is required
        if self.config.require_quantum_resistance {
            match signature.signature_type {
                SignatureType::Quantum(_) => {}, // Quantum signature is fine
                _ => return Err(ValidationError::QuantumResistanceRequired),
            }
        }
        
        // Verify the signature
        match self.signature_verifier.verify(
            signature.signature_type,
            &signature.public_key,
            transaction.hash().as_bytes(),
            &signature.signature,
        ) {
            Ok(true) => Ok(()),
            Ok(false) => Err(ValidationError::InvalidSignature("Signature verification failed".to_string())),
            Err(err) => Err(ValidationError::InvalidSignature(err.to_string())),
        }
    }

    /// Verify emissions compliance
    fn verify_emissions(&self, transaction: &Transaction) -> Result<(), ValidationError> {
        // Ensure we have an emissions tracker
        let tracker = self.emissions_tracker.as_ref().ok_or_else(|| {
            ValidationError::EmissionsCalculationFailed("No emissions tracker configured".to_string())
        })?;

        // Check carbon intensity
        if let Some(max_intensity) = self.config.max_carbon_intensity {
            let actual_intensity = transaction.carbon_intensity(tracker)
                .map_err(|e| ValidationError::EmissionsCalculationFailed(e.to_string()))?;
            
            if actual_intensity > max_intensity {
                return Err(ValidationError::EmissionsThresholdExceeded(actual_intensity, max_intensity));
            }
        }

        // Check carbon neutrality
        if self.config.require_carbon_neutral {
            let is_neutral = transaction.is_carbon_neutral(tracker)
                .map_err(|e| ValidationError::EmissionsCalculationFailed(e.to_string()))?;
            
            if !is_neutral {
                return Err(ValidationError::CarbonNeutralityRequired);
            }
        }

        Ok(())
    }
} 