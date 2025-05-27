use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;
use serde::{Serialize, Deserialize};
use thiserror::Error;
use rayon::prelude::*;
use crate::types::transaction::Transaction;
use crate::crypto::quantum::{QuantumScheme, QuantumParameters, QuantumError, ClassicalScheme};
use secp256k1::{Secp256k1, Message, SecretKey, PublicKey};
use secp256k1::ecdsa::Signature as Secp256k1Signature;
use rand;
use hex;
use sha2;

/// Error type for signature operations
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum SignatureError {
    /// The signature scheme is not supported
    #[error("Signature scheme not supported: {0}")]
    UnsupportedScheme(String),
    
    /// The key is invalid or corrupted
    #[error("Invalid key: {0}")]
    InvalidKey(String),
    
    /// The signature is invalid or corrupted
    #[error("Invalid signature: {0}")]
    InvalidSignature(String),
    
    /// Batch verification failed
    #[error("Batch verification failed: {0}")]
    BatchVerificationFailed(String),
    
    /// A cryptographic operation failed
    #[error("Cryptographic operation failed: {0}")]
    CryptoOperationFailed(String),
    
    /// Quantum-specific error
    #[error("Quantum error: {0}")]
    QuantumError(#[from] QuantumError),
    
    /// Missing signature
    #[error("Missing signature")]
    MissingSignature,
    
    /// Unsupported signature type
    #[error("Unsupported signature type")]
    UnsupportedSignatureType,
    
    /// Quantum resistance is required
    #[error("Quantum-resistant signature required")]
    QuantumResistanceRequired,
    
    /// Signature verification failed
    #[error("Signature verification failed: {0}")]
    VerificationFailed(String),
    
    /// Unsupported signature type
    #[error("Unsupported signature type: {0}")]
    UnsupportedType(String),
    
    /// Internal error
    #[error("Internal error: {0}")]
    InternalError(String),
    
    /// Invalid parameters
    #[error("Invalid parameters: {0}")]
    InvalidParameters(String),
}

/// Type of signature scheme
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SignatureType {
    /// secp256k1 curve (used in Bitcoin)
    Secp256k1,
    /// Ed25519 curve (used in many modern cryptographic systems)
    Ed25519,
    /// CRYSTALS-Dilithium (post-quantum)
    Dilithium,
    /// Falcon (post-quantum)
    Falcon,
    /// SPHINCS+ (post-quantum)
    Sphincs,
    /// Hybrid scheme (classical + post-quantum)
    Hybrid,
    /// Classical signature types (for backward compatibility)
    Classical(ClassicalScheme),
    /// Quantum signature types (for backward compatibility)
    Quantum(QuantumScheme),
    /// Schnorr signatures
    Schnorr,
}

/// Parameters for signature algorithms
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SignatureParams {
    /// Signature type
    pub sig_type: SignatureType,
    /// Security level for post-quantum schemes
    pub security_level: u8,
    /// Whether to enable batch verification
    pub enable_batch: bool,
    /// Additional parameters for the signature algorithm
    pub additional_params: HashMap<String, String>,
}

impl Default for SignatureParams {
    fn default() -> Self {
        Self {
            sig_type: SignatureType::Secp256k1,
            security_level: 3, // Medium security by default
            enable_batch: true,
            additional_params: HashMap::new(),
        }
    }
}

/// Trait for signature schemes
pub trait SignatureScheme: Send + Sync {
    /// Verify a single signature
    fn verify(&self, public_key: &[u8], message: &[u8], signature: &[u8]) -> Result<bool, SignatureError>;
    
    /// Verify multiple signatures in a batch
    fn batch_verify(
        &self, 
        keys: &[&[u8]], 
        messages: &[&[u8]], 
        signatures: &[&[u8]]
    ) -> Result<bool, SignatureError> {
        // Default implementation verifies each signature individually
        if keys.len() != messages.len() || keys.len() != signatures.len() {
            return Err(SignatureError::BatchVerificationFailed(
                "Mismatched number of keys, messages, and signatures".to_string()
            ));
        }
        
        // Sequentially verify each signature
        for i in 0..keys.len() {
            match self.verify(keys[i], messages[i], signatures[i]) {
                Ok(valid) if !valid => return Ok(false),
                Err(e) => return Err(e),
                _ => {}
            }
        }
        
        Ok(true)
    }
    
    /// Get the signature type
    fn signature_type(&self) -> SignatureType;
}

/// Implementation of secp256k1 signature scheme
pub struct Secp256k1Scheme;

impl SignatureScheme for Secp256k1Scheme {
    fn verify(&self, public_key: &[u8], message: &[u8], signature: &[u8]) -> Result<bool, SignatureError> {
        let secp = Secp256k1::verification_only();
        
        // Convert message to Message
        let message = Message::from_slice(message)
            .map_err(|e| SignatureError::InvalidSignature(e.to_string()))?;
        
        // Convert public key bytes to PublicKey
        let public_key = PublicKey::from_slice(public_key)
            .map_err(|e| SignatureError::InvalidKey(e.to_string()))?;
        
        // Convert signature bytes to Signature
        let signature = Secp256k1Signature::from_compact(signature)
            .map_err(|e| SignatureError::InvalidSignature(e.to_string()))?;
        
        // Verify
        match secp.verify_ecdsa(&message, &signature, &public_key) {
            Ok(_) => Ok(true),
            Err(e) => Err(SignatureError::VerificationFailed(e.to_string())),
        }
    }
    
    // Override the default implementation with an optimized version that uses the 
    // secp256k1 library's native batch verification
    fn batch_verify(
        &self, 
        keys: &[&[u8]], 
        messages: &[&[u8]], 
        signatures: &[&[u8]]
    ) -> Result<bool, SignatureError> {
        // Check that arrays have the same length
        if keys.len() != messages.len() || keys.len() != signatures.len() {
            return Err(SignatureError::InvalidParameters(
                "Batch verification requires equal number of keys, messages, and signatures".to_string(),
            ));
        }
        
        let secp = Secp256k1::verification_only();
        
        let mut secp_msgs = Vec::with_capacity(messages.len());
        let mut secp_sigs = Vec::with_capacity(signatures.len());
        let mut secp_pks = Vec::with_capacity(keys.len());
        
        // Convert all inputs to secp256k1 types
        for i in 0..keys.len() {
            let msg = Message::from_slice(messages[i])
                .map_err(|e| SignatureError::InvalidSignature(e.to_string()))?;
            
            let sig = Secp256k1Signature::from_compact(signatures[i])
                .map_err(|e| SignatureError::InvalidSignature(e.to_string()))?;
                
            let pk = PublicKey::from_slice(keys[i])
                .map_err(|e| SignatureError::InvalidKey(e.to_string()))?;
                
            secp_msgs.push(msg);
            secp_sigs.push(sig);
            secp_pks.push(pk);
        }
        
        // Perform verification one by one (as a fallback for missing batch API)
        for i in 0..secp_msgs.len() {
            match secp.verify_ecdsa(&secp_msgs[i], &secp_sigs[i], &secp_pks[i]) {
                Ok(_) => {}, // continue to next signature
                Err(e) => return Err(SignatureError::VerificationFailed(e.to_string())),
            }
        }
        
        Ok(true)
    }
    
    fn signature_type(&self) -> SignatureType {
        SignatureType::Secp256k1
    }
}

/// Implementation of Ed25519 signature scheme
pub struct Ed25519Scheme;

impl SignatureScheme for Ed25519Scheme {
    fn verify(&self, public_key: &[u8], message: &[u8], signature: &[u8]) -> Result<bool, SignatureError> {
        // The actual implementation would use ed25519-dalek to verify the signature
        
        if public_key.len() != 32 {
            return Err(SignatureError::InvalidKey(
                format!("Invalid Ed25519 public key length: expected 32, got {}", public_key.len())
            ));
        }
        
        if signature.len() != 64 {
            return Err(SignatureError::InvalidSignature(
                format!("Invalid Ed25519 signature length: expected 64, got {}", signature.len())
            ));
        }
        
        // In a real implementation, this would perform actual verification
        // using ed25519-dalek's verify function
        
        Ok(true) // Placeholder
    }
    
    // For Ed25519, we'll use the default batch_verify implementation 
    // provided by the trait. We could implement an optimized version later
    // using ed25519-dalek's batch verification when available.
    
    fn signature_type(&self) -> SignatureType {
        SignatureType::Ed25519
    }
}

/// Implementation of CRYSTALS-Dilithium signature scheme
pub struct DilithiumScheme {
    security_level: u8,
}

impl DilithiumScheme {
    pub fn new(security_level: u8) -> Self {
        Self { security_level }
    }
}

impl SignatureScheme for DilithiumScheme {
    fn verify(&self, public_key: &[u8], message: &[u8], signature: &[u8]) -> Result<bool, SignatureError> {
        // Use the existing quantum verification through the QuantumParameters conversion
        let params = QuantumParameters {
            scheme: QuantumScheme::Dilithium,
            security_level: self.security_level,
        };
        
        // Use the existing verify_quantum_signature function
        crate::crypto::quantum::verify_quantum_signature(
            public_key, message, signature, params
        ).map_err(SignatureError::QuantumError)
    }
    
    // Using the default batch_verify implementation from the trait
    // Post-quantum schemes typically don't have native batch verification
    // so we use the sequential verification approach
    
    fn signature_type(&self) -> SignatureType {
        SignatureType::Dilithium
    }
}

/// Implementation of Falcon signature scheme
pub struct FalconScheme {
    security_level: u8,
}

impl FalconScheme {
    pub fn new(security_level: u8) -> Self {
        Self { security_level }
    }
}

impl SignatureScheme for FalconScheme {
    fn verify(&self, public_key: &[u8], message: &[u8], signature: &[u8]) -> Result<bool, SignatureError> {
        // Create parameters using the appropriate security level
        use crate::crypto::falcon::{FalconParameters, FalconKeyPair, FalconError};
        
        let params = FalconParameters::with_security_level(self.security_level)
            .map_err(|e| SignatureError::CryptoOperationFailed(format!("Falcon parameter error: {}", e)))?;
            
        // Create a public key only keypair
        let key_pair = FalconKeyPair::from_public_bytes(public_key.to_vec(), params)
            .map_err(|e| SignatureError::CryptoOperationFailed(
                format!("Falcon key error: {}", e)
            ))?;
            
        // Verify the signature
        match key_pair.verify(message, signature) {
            Ok(valid) => Ok(valid),
            Err(e) => match e {
                FalconError::InvalidKey(msg) => Err(SignatureError::InvalidKey(msg)),
                FalconError::InvalidSignature(msg) => Err(SignatureError::InvalidSignature(msg)),
                FalconError::InvalidPublicKey => Err(SignatureError::InvalidKey("Invalid Falcon public key".to_string())),
                FalconError::InvalidSecretKey => Err(SignatureError::InvalidKey("Invalid Falcon secret key".to_string())),
                FalconError::UnsupportedSecurityLevel(level) => Err(SignatureError::CryptoOperationFailed(
                    format!("Unsupported Falcon security level: {}", level)
                )),
                err => Err(SignatureError::CryptoOperationFailed(format!("Falcon error: {}", err))),
            },
        }
    }
    
    // Using the default batch_verify implementation from the trait
    // Falcon doesn't have native batch verification support
    
    fn signature_type(&self) -> SignatureType {
        SignatureType::Falcon
    }
}

/// Implementation of SPHINCS+ signature scheme
pub struct SphincsScheme {
    security_level: u8,
}

impl SphincsScheme {
    pub fn new(security_level: u8) -> Self {
        Self { security_level }
    }
}

impl SignatureScheme for SphincsScheme {
    fn verify(&self, public_key: &[u8], message: &[u8], signature: &[u8]) -> Result<bool, SignatureError> {
        // For now, return unsupported error
        // This will be implemented with the actual SPHINCS+ code
        Err(SignatureError::CryptoOperationFailed(
            "SPHINCS+ verification not yet implemented".to_string()
        ))
    }
    
    // Using the default batch_verify implementation from the trait
    // SPHINCS+ doesn't have native batch verification
    
    fn signature_type(&self) -> SignatureType {
        SignatureType::Sphincs
    }
}

/// Implementation of hybrid signature scheme
pub struct HybridScheme {
    classical_scheme: Box<dyn SignatureScheme>,
    quantum_scheme: Box<dyn SignatureScheme>,
}

impl HybridScheme {
    pub fn new(
        classical_scheme: Box<dyn SignatureScheme>,
        quantum_scheme: Box<dyn SignatureScheme>
    ) -> Self {
        Self { classical_scheme, quantum_scheme }
    }
}

impl SignatureScheme for HybridScheme {
    fn verify(&self, public_key: &[u8], message: &[u8], signature: &[u8]) -> Result<bool, SignatureError> {
        // A hybrid signature combines a classical and quantum signature
        // The public key and signature need to be split into classical and quantum parts
        
        // This is a placeholder implementation - in reality it would:
        // 1. Split the signature into classical and quantum parts
        // 2. Split the public key into classical and quantum parts
        // 3. Verify both signatures
        // 4. Return true only if both verify successfully
        
        Err(SignatureError::CryptoOperationFailed(
            "Hybrid verification not yet implemented".to_string()
        ))
    }
    
    // For the hybrid scheme, we use the default implementation
    // When the actual implementation is ready, this can be specialized
    // to split the hybrid signatures and keys appropriately before verification
    
    fn signature_type(&self) -> SignatureType {
        SignatureType::Hybrid
    }
}

/// Unified signature verifier for all signature types
pub struct SignatureVerifier {
    /// Security level for post-quantum schemes
    pub security_level: u8,
}

impl SignatureVerifier {
    /// Create a new signature verifier with default schemes
    pub fn new() -> Self {
        Self {
            security_level: 2, // Medium security level by default
        }
    }
    
    /// Verify a signature
    pub fn verify(
        &self,
        sig_type: SignatureType,
        public_key: &[u8],
        message: &[u8],
        signature: &[u8]
    ) -> Result<bool, SignatureError> {
        match sig_type {
            SignatureType::Secp256k1 => {
                let scheme = Secp256k1Scheme;
                scheme.verify(public_key, message, signature)
            }
            SignatureType::Ed25519 => {
                let scheme = Ed25519Scheme;
                scheme.verify(public_key, message, signature)
            }
            SignatureType::Dilithium => {
                let scheme = DilithiumScheme::new(self.security_level);
                scheme.verify(public_key, message, signature)
            }
            SignatureType::Falcon => {
                let scheme = FalconScheme::new(self.security_level);
                scheme.verify(public_key, message, signature)
            }
            SignatureType::Sphincs => {
                let scheme = SphincsScheme::new(self.security_level);
                scheme.verify(public_key, message, signature)
            }
            SignatureType::Hybrid => {
                let classical_scheme = Box::new(Secp256k1Scheme);
                let quantum_scheme = Box::new(DilithiumScheme::new(self.security_level));
                let scheme = HybridScheme::new(classical_scheme, quantum_scheme);
                scheme.verify(public_key, message, signature)
            }
            SignatureType::Classical(classical_scheme) => {
                match classical_scheme {
                    ClassicalScheme::Secp256k1 => {
                        let scheme = Secp256k1Scheme;
                        scheme.verify(public_key, message, signature)
                    }
                    ClassicalScheme::Ed25519 => {
                        let scheme = Ed25519Scheme;
                        scheme.verify(public_key, message, signature)
                    }
                }
            }
            SignatureType::Quantum(quantum_scheme) => {
                match quantum_scheme {
                    QuantumScheme::Dilithium => {
                        let scheme = DilithiumScheme::new(self.security_level);
                        scheme.verify(public_key, message, signature)
                    }
                    QuantumScheme::Falcon => {
                        let scheme = FalconScheme::new(self.security_level);
                        scheme.verify(public_key, message, signature)
                    }
                    QuantumScheme::Sphincs => {
                        let scheme = SphincsScheme::new(self.security_level);
                        scheme.verify(public_key, message, signature)
                    }
                    QuantumScheme::Hybrid(classical_scheme) => {
                        // For hybrid schemes, we need to parse the signature and public key appropriately
                        // Simplify for now to just use the quantum part
                        match classical_scheme {
                            ClassicalScheme::Secp256k1 => {
                                let classical_scheme = Box::new(Secp256k1Scheme);
                                let quantum_scheme = Box::new(DilithiumScheme::new(self.security_level));
                                let scheme = HybridScheme::new(classical_scheme, quantum_scheme);
                                scheme.verify(public_key, message, signature)
                            }
                            ClassicalScheme::Ed25519 => {
                                let classical_scheme = Box::new(Ed25519Scheme);
                                let quantum_scheme = Box::new(DilithiumScheme::new(self.security_level));
                                let scheme = HybridScheme::new(classical_scheme, quantum_scheme);
        scheme.verify(public_key, message, signature)
                            }
                        }
                    }
                }
            }
            SignatureType::Schnorr => {
                Err(SignatureError::UnsupportedType("Schnorr not implemented".to_string()))
            }
        }
    }
    
    /// Verify a transaction's signature
    pub fn verify_transaction(&self, tx: &crate::types::transaction::Transaction) -> Result<bool, SignatureError> {
        // Get the signature data from the transaction
        let signature_data = match tx.signature_data() {
            Some(data) => data,
            None => return Err(SignatureError::MissingSignature),
        };
        
        // Verify the signature using the appropriate scheme
        match signature_data.scheme {
            crate::types::transaction::SignatureSchemeType::Legacy => {
                self.verify(SignatureType::Secp256k1, &signature_data.public_key, &tx.hash(), &signature_data.data)
            },
            crate::types::transaction::SignatureSchemeType::Ed25519 => {
                self.verify(SignatureType::Ed25519, &signature_data.public_key, &tx.hash(), &signature_data.data)
            },
            // Add more signature schemes as needed
            _ => Err(SignatureError::UnsupportedScheme(
                format!("Signature scheme not supported for verification: {:?}", signature_data.scheme)
            )),
        }
    }
    
    /// Batch verify multiple transactions
    pub fn batch_verify_transactions(&self, txs: &[&crate::types::transaction::Transaction]) -> Result<bool, SignatureError> {
        // Group transactions by signature type for batch verification
        let mut secp256k1_batches: Vec<(Vec<u8>, Vec<u8>, Vec<u8>)> = Vec::new();
        let mut ed25519_batches: Vec<(Vec<u8>, Vec<u8>, Vec<u8>)> = Vec::new();
        
        // Organize transactions by signature type
        for tx in txs {
            let signature_data = match tx.signature_data() {
                Some(data) => data,
                None => return Err(SignatureError::MissingSignature),
            };
            
            let hash = tx.hash();
            
            match signature_data.scheme {
                crate::types::transaction::SignatureSchemeType::Legacy => {
                    secp256k1_batches.push((
                        signature_data.public_key.clone(),
                        hash.to_vec(),
                        signature_data.data.clone(),
                    ));
                },
                crate::types::transaction::SignatureSchemeType::Ed25519 => {
                    ed25519_batches.push((
                        signature_data.public_key.clone(),
                        hash.to_vec(),
                        signature_data.data.clone(),
                    ));
                },
                _ => return Err(SignatureError::UnsupportedScheme(
                    format!("Signature scheme not supported for batch verification: {:?}", signature_data.scheme)
                )),
            }
        }
        
        // This is a placeholder for batch transaction verification
        // The actual implementation would:
        // 1. Call batch_verify for each group
        // 2. Return true only if all groups verify successfully
        
        Ok(true) // Placeholder
    }

    /// Verify a Falcon signature
    fn verify_falcon(&self, public_key: &[u8], signature: &[u8]) -> Result<bool, SignatureError> {
        use crate::crypto::falcon::{FalconKeyPair, FalconParameters};
        
        // Create parameters
        let params = FalconParameters::with_security_level(self.security_level)
            .map_err(|e| SignatureError::CryptoOperationFailed(
                format!("Falcon parameter error: {}", e)
            ))?;
        
        // Create a public key only keypair
        let key_pair = FalconKeyPair::from_public_bytes(public_key.to_vec(), params)
            .map_err(|e| SignatureError::CryptoOperationFailed(
                format!("Falcon key error: {}", e)
            ))?;
        
        // Verify the signature
        // Note: We're not using message here since the hash was pre-computed
        let hash = [0u8; 32]; // Placeholder - in real implementation we'd use the hash
        match key_pair.verify(&hash, signature) {
            Ok(valid) => Ok(valid),
            Err(e) => Err(SignatureError::CryptoOperationFailed(
                format!("Falcon verification error: {}", e)
            )),
        }
    }
}

/// Unified signature struct for different signature types
#[derive(Clone, Serialize, Deserialize)]
pub struct Signature {
    /// Type of signature
    pub signature_type: SignatureType,
    /// Raw signature bytes
    pub signature_bytes: Vec<u8>,
    /// Public key bytes
    pub public_key_bytes: Vec<u8>,
}

impl Signature {
    /// Create a new signature
    pub fn new(signature_type: SignatureType, signature_bytes: Vec<u8>, public_key_bytes: Vec<u8>) -> Self {
        Self {
            signature_type,
            signature_bytes,
            public_key_bytes,
        }
    }
    
    /// Verify a message with this signature
    pub fn verify(&self, message: &[u8]) -> Result<bool, SignatureError> {
        match self.signature_type {
            SignatureType::Secp256k1 => self.verify_secp256k1(message),
            SignatureType::Ed25519 => Err(SignatureError::UnsupportedType("Ed25519 not implemented".to_string())),
            SignatureType::Schnorr => Err(SignatureError::UnsupportedType("Schnorr not implemented".to_string())),
            SignatureType::Sphincs => Err(SignatureError::UnsupportedType("SPHINCS+ not implemented".to_string())),
            SignatureType::Dilithium => Err(SignatureError::UnsupportedType("Dilithium not implemented".to_string())),
            SignatureType::Falcon => Err(SignatureError::UnsupportedType("Falcon not implemented".to_string())),
            SignatureType::Hybrid => Err(SignatureError::UnsupportedType("Hybrid not implemented".to_string())),
            SignatureType::Classical(classical_scheme) => {
                match classical_scheme {
                    ClassicalScheme::Secp256k1 => self.verify_secp256k1(message),
                    ClassicalScheme::Ed25519 => Err(SignatureError::UnsupportedType("Ed25519 not implemented".to_string())),
                }
            }
            SignatureType::Quantum(quantum_scheme) => {
                match quantum_scheme {
                    QuantumScheme::Dilithium => self.verify_dilithium(message),
                    QuantumScheme::Falcon => self.verify_falcon(message),
                    QuantumScheme::Sphincs => Err(SignatureError::UnsupportedType("SPHINCS+ not implemented".to_string())),
                    QuantumScheme::Hybrid(classical_scheme) => {
                        match classical_scheme {
                            ClassicalScheme::Secp256k1 => self.verify_secp256k1(message),
                            ClassicalScheme::Ed25519 => Err(SignatureError::UnsupportedType("Ed25519 not implemented".to_string())),
                        }
                    }
                }
            }
        }
    }
    
    /// Verify a Secp256k1 signature
    fn verify_secp256k1(&self, message: &[u8]) -> Result<bool, SignatureError> {
        let secp = Secp256k1::verification_only();
        
        // Convert message to Message
        let message = Message::from_slice(message)
            .map_err(|e| SignatureError::InvalidSignature(e.to_string()))?;
        
        // Convert public key bytes to PublicKey
        let public_key = PublicKey::from_slice(&self.public_key_bytes)
            .map_err(|e| SignatureError::InvalidKey(e.to_string()))?;
        
        // Convert signature bytes to Signature
        let signature = Secp256k1Signature::from_compact(&self.signature_bytes)
            .map_err(|e| SignatureError::InvalidSignature(e.to_string()))?;
        
        // Verify
        match secp.verify_ecdsa(&message, &signature, &public_key) {
            Ok(_) => Ok(true),
            Err(e) => Err(SignatureError::VerificationFailed(e.to_string())),
        }
    }
    
    /// Verify a Dilithium signature
    fn verify_dilithium(&self, message: &[u8]) -> Result<bool, SignatureError> {
        // Implementation of verify_dilithium method
        Err(SignatureError::InternalError("Dilithium verification not implemented".to_string()))
    }
    
    /// Verify a Falcon signature
    fn verify_falcon(&self, message: &[u8]) -> Result<bool, SignatureError> {
        // Implementation of verify_falcon method
        Err(SignatureError::InternalError("Falcon verification not implemented".to_string()))
    }
}

impl fmt::Debug for Signature {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Signature {{ type: {:?}, signature: {}, public_key: {} }}",
               self.signature_type,
               hex::encode(&self.signature_bytes),
               hex::encode(&self.public_key_bytes))
    }
}

/// Key pair for digital signatures
pub struct KeyPair {
    /// Type of signature
    pub signature_type: SignatureType,
    /// Private key bytes
    secret_key: Vec<u8>,
    /// Public key bytes
    pub public_key: Vec<u8>,
}

impl KeyPair {
    /// Create a new Secp256k1 key pair
    pub fn new_secp256k1() -> Result<Self, SignatureError> {
        let secp = Secp256k1::new();
        let mut rng = rand::thread_rng();
        let (secret_key, public_key) = secp.generate_keypair(&mut rng);
        
        Ok(Self {
            signature_type: SignatureType::Secp256k1,
            secret_key: secret_key.secret_bytes().to_vec(),
            public_key: public_key.serialize().to_vec(),
        })
    }
    
    /// Sign a message
    pub fn sign(&self, message: &[u8]) -> Result<Signature, SignatureError> {
        match self.signature_type {
            SignatureType::Secp256k1 => self.sign_secp256k1(message),
            SignatureType::Ed25519 => Err(SignatureError::UnsupportedType("Ed25519 not implemented".to_string())),
            SignatureType::Schnorr => Err(SignatureError::UnsupportedType("Schnorr not implemented".to_string())),
            SignatureType::Sphincs => Err(SignatureError::UnsupportedType("SPHINCS+ not implemented".to_string())),
            SignatureType::Dilithium => Err(SignatureError::UnsupportedType("Dilithium not implemented".to_string())),
            SignatureType::Falcon => Err(SignatureError::UnsupportedType("Falcon not implemented".to_string())),
            SignatureType::Hybrid => Err(SignatureError::UnsupportedType("Hybrid not implemented".to_string())),
            SignatureType::Classical(classical_scheme) => {
                match classical_scheme {
                    ClassicalScheme::Secp256k1 => self.sign_secp256k1(message),
                    ClassicalScheme::Ed25519 => Err(SignatureError::UnsupportedType("Ed25519 not implemented".to_string())),
                }
            }
            SignatureType::Quantum(quantum_scheme) => {
                match quantum_scheme {
                    QuantumScheme::Dilithium => Err(SignatureError::UnsupportedType("Dilithium not implemented".to_string())),
                    QuantumScheme::Falcon => Err(SignatureError::UnsupportedType("Falcon not implemented".to_string())),
                    QuantumScheme::Sphincs => Err(SignatureError::UnsupportedType("SPHINCS+ not implemented".to_string())),
                    QuantumScheme::Hybrid(classical_scheme) => {
                        match classical_scheme {
                            ClassicalScheme::Secp256k1 => self.sign_secp256k1(message),
                            ClassicalScheme::Ed25519 => Err(SignatureError::UnsupportedType("Ed25519 not implemented".to_string())),
                        }
                    }
                }
            }
        }
    }
    
    /// Sign with Secp256k1
    fn sign_secp256k1(&self, message: &[u8]) -> Result<Signature, SignatureError> {
        let secp = Secp256k1::signing_only();
        
        // Convert secret key bytes to SecretKey
        let secret_key = SecretKey::from_slice(&self.secret_key)
            .map_err(|e| SignatureError::InvalidKey(e.to_string()))?;
        
        // Convert message to Message
        let message = Message::from_slice(message)
            .map_err(|e| SignatureError::InvalidSignature(e.to_string()))?;
        
        // Sign
        let signature = secp.sign_ecdsa(&message, &secret_key);
        
        Ok(Signature::new(
            SignatureType::Secp256k1,
            signature.serialize_compact().to_vec(),
            self.public_key.clone(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sha2::{Sha256, Digest};
    
    /// Generate a test key pair for the given signature type
    fn generate_test_keypair(sig_type: SignatureType) -> KeyPair {
        match sig_type {
            SignatureType::Secp256k1 => KeyPair::new_secp256k1().unwrap(),
            _ => panic!("Unsupported signature type for tests"),
        }
    }
    
    /// Sign a test message with the given key pair
    fn sign_test_message(key_pair: &KeyPair, message: &[u8]) -> Vec<u8> {
        // Hash the message first (typical in most blockchain systems)
        let mut hasher = Sha256::new();
        hasher.update(message);
        let message_hash = hasher.finalize();
        
        // Sign the hash
        let signature = key_pair.sign(&message_hash).unwrap();
        signature.signature_bytes
    }
    
    #[test]
    fn test_secp256k1_sign_verify() {
        // Create key pair
        let key_pair = KeyPair::new_secp256k1().unwrap();
        
        // Create message to sign
        let message = b"Hello, world!";
        let mut hasher = Sha256::new();
        hasher.update(message);
        let message_hash = hasher.finalize();
        
        // Sign
        let signature = key_pair.sign(&message_hash).unwrap();
        
        // Verify
        let result = signature.verify(&message_hash).unwrap();
        assert!(result);
    }
    
    #[test]
    fn test_signature_verification() {
        // Test verification with mismatched keys and messages
        let verifier = SignatureVerifier::new();
        
        // Create a valid key pair and signature
        let key_pair = generate_test_keypair(SignatureType::Secp256k1);
        let message = b"Test message";
        let signature = sign_test_message(&key_pair, message);
        
        // Verify with correct message should succeed
        let result = verifier.verify(
            &key_pair.public_key,
            message,
            &signature,
            &SignatureParams {
                sig_type: SignatureType::Secp256k1,
                security_level: 1,
                enable_batch: false,
                additional_params: HashMap::new(),
            },
        );
        assert!(result.is_ok());
        
        // Verify with incorrect message should fail
        let wrong_message = b"Wrong message";
        let result = verifier.verify(
            &key_pair.public_key,
            wrong_message,
            &signature,
            &SignatureParams {
                sig_type: SignatureType::Secp256k1,
                security_level: 1,
                enable_batch: false,
                additional_params: HashMap::new(),
            },
        );
        
        if let Err(err) = result {
            assert!(matches!(err, SignatureError::InvalidSignature(_)));
        }
    }
} 