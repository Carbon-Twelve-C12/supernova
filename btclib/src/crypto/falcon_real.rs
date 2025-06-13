// Falcon Post-Quantum Signature Implementation
// Based on NIST FIPS 204 and the Falcon specification
// This replaces the fraudulent SHA-256 based implementation

use rand::{CryptoRng, RngCore};
use thiserror::Error;
use serde::{Serialize, Deserialize};
use std::fmt;

// Import the actual Falcon implementation
// We'll use the pqcrypto-falcon crate which implements the Falcon algorithm
use pqcrypto_falcon::{falcon512, falcon1024};
use pqcrypto_traits::sign::{
    PublicKey as SignPublicKey,
    SecretKey as SignSecretKey,
    DetachedSignature as SignDetachedSignature,
};

/// Falcon signature scheme errors
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum FalconError {
    #[error("Invalid Falcon key: {0}")]
    InvalidKey(String),
    
    #[error("Invalid Falcon signature: {0}")]
    InvalidSignature(String),
    
    #[error("Unsupported Falcon security level: {0}")]
    UnsupportedSecurityLevel(u8),
    
    #[error("Falcon cryptographic operation failed: {0}")]
    CryptoOperationFailed(String),
    
    #[error("Invalid public key")]
    InvalidPublicKey,
    
    #[error("Invalid secret key")]
    InvalidSecretKey,
    
    #[error("Invalid message: {0}")]
    InvalidMessage(String),
    
    #[error("Key generation failed: {0}")]
    KeyGenerationFailed(String),
    
    #[error("Signing failed: {0}")]
    SigningFailed(String),
    
    #[error("Verification failed: {0}")]
    VerificationFailed(String),
}

/// Falcon security levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FalconSecurityLevel {
    /// Falcon-512 (NIST Level 1)
    Falcon512 = 1,
    /// Falcon-1024 (NIST Level 5)
    Falcon1024 = 5,
}

impl FalconSecurityLevel {
    /// Create from numeric level
    pub fn from_level(level: u8) -> Result<Self, FalconError> {
        match level {
            1 => Ok(FalconSecurityLevel::Falcon512),
            5 => Ok(FalconSecurityLevel::Falcon1024),
            _ => Err(FalconError::UnsupportedSecurityLevel(level)),
        }
    }
    
    /// Get the NIST security level
    pub fn nist_level(&self) -> u8 {
        *self as u8
    }
    
    /// Get human-readable name
    pub fn name(&self) -> &'static str {
        match self {
            FalconSecurityLevel::Falcon512 => "Falcon-512",
            FalconSecurityLevel::Falcon1024 => "Falcon-1024",
        }
    }
    
    /// Get expected public key length
    pub fn public_key_length(&self) -> usize {
        match self {
            FalconSecurityLevel::Falcon512 => falcon512::public_key_bytes(),
            FalconSecurityLevel::Falcon1024 => falcon1024::public_key_bytes(),
        }
    }
    
    /// Get expected secret key length
    pub fn secret_key_length(&self) -> usize {
        match self {
            FalconSecurityLevel::Falcon512 => falcon512::secret_key_bytes(),
            FalconSecurityLevel::Falcon1024 => falcon1024::secret_key_bytes(),
        }
    }
    
    /// Get expected signature length
    pub fn signature_length(&self) -> usize {
        match self {
            FalconSecurityLevel::Falcon512 => falcon512::signature_bytes(),
            FalconSecurityLevel::Falcon1024 => falcon1024::signature_bytes(),
        }
    }
}

/// Falcon key pair
pub struct FalconKeyPair {
    /// The public key bytes
    pub public_key: Vec<u8>,
    /// The secret key bytes
    pub secret_key: Vec<u8>,
    /// Security level
    pub security_level: FalconSecurityLevel,
}

impl FalconKeyPair {
    /// Generate a new Falcon key pair
    pub fn generate<R: CryptoRng + RngCore>(
        rng: &mut R,
        security_level: FalconSecurityLevel,
    ) -> Result<Self, FalconError> {
        match security_level {
            FalconSecurityLevel::Falcon512 => {
                let (pk, sk) = falcon512::keypair();
                Ok(Self {
                    public_key: pk.as_bytes().to_vec(),
                    secret_key: sk.as_bytes().to_vec(),
                    security_level,
                })
            }
            FalconSecurityLevel::Falcon1024 => {
                let (pk, sk) = falcon1024::keypair();
                Ok(Self {
                    public_key: pk.as_bytes().to_vec(),
                    secret_key: sk.as_bytes().to_vec(),
                    security_level,
                })
            }
        }
    }
    
    /// Create from existing keys
    pub fn from_bytes(
        public_key: Vec<u8>,
        secret_key: Vec<u8>,
        security_level: FalconSecurityLevel,
    ) -> Result<Self, FalconError> {
        // Validate key lengths
        if public_key.len() != security_level.public_key_length() {
            return Err(FalconError::InvalidPublicKey);
        }
        
        if !secret_key.is_empty() && secret_key.len() != security_level.secret_key_length() {
            return Err(FalconError::InvalidSecretKey);
        }
        
        Ok(Self {
            public_key,
            secret_key,
            security_level,
        })
    }
    
    /// Sign a message using Falcon algorithm
    pub fn sign(&self, message: &[u8]) -> Result<Vec<u8>, FalconError> {
        if self.secret_key.is_empty() {
            return Err(FalconError::InvalidSecretKey);
        }
        
        if message.is_empty() {
            return Err(FalconError::InvalidMessage("Message cannot be empty".to_string()));
        }
        
        match self.security_level {
            FalconSecurityLevel::Falcon512 => {
                let sk = falcon512::SecretKey::from_bytes(&self.secret_key)
                    .map_err(|_| FalconError::InvalidSecretKey)?;
                let sig = falcon512::detached_sign(message, &sk);
                Ok(sig.as_bytes().to_vec())
            }
            FalconSecurityLevel::Falcon1024 => {
                let sk = falcon1024::SecretKey::from_bytes(&self.secret_key)
                    .map_err(|_| FalconError::InvalidSecretKey)?;
                let sig = falcon1024::detached_sign(message, &sk);
                Ok(sig.as_bytes().to_vec())
            }
        }
    }
    
    /// Verify a signature using Falcon algorithm
    pub fn verify(&self, message: &[u8], signature: &[u8]) -> Result<bool, FalconError> {
        if message.is_empty() {
            return Err(FalconError::InvalidMessage("Message cannot be empty".to_string()));
        }
        
        match self.security_level {
            FalconSecurityLevel::Falcon512 => {
                let pk = falcon512::PublicKey::from_bytes(&self.public_key)
                    .map_err(|_| FalconError::InvalidPublicKey)?;
                let sig = falcon512::DetachedSignature::from_bytes(signature)
                    .map_err(|_| FalconError::InvalidSignature("Invalid Falcon-512 signature".to_string()))?;
                
                match falcon512::verify_detached_signature(&sig, message, &pk) {
                    Ok(_) => Ok(true),
                    Err(_) => Ok(false),
                }
            }
            FalconSecurityLevel::Falcon1024 => {
                let pk = falcon1024::PublicKey::from_bytes(&self.public_key)
                    .map_err(|_| FalconError::InvalidPublicKey)?;
                let sig = falcon1024::DetachedSignature::from_bytes(signature)
                    .map_err(|_| FalconError::InvalidSignature("Invalid Falcon-1024 signature".to_string()))?;
                
                match falcon1024::verify_detached_signature(&sig, message, &pk) {
                    Ok(_) => Ok(true),
                    Err(_) => Ok(false),
                }
            }
        }
    }
    
    /// Get the public key bytes
    pub fn public_key_bytes(&self) -> &[u8] {
        &self.public_key
    }
    
    /// Get the secret key bytes (if available)
    pub fn secret_key_bytes(&self) -> Option<&[u8]> {
        if self.secret_key.is_empty() {
            None
        } else {
            Some(&self.secret_key)
        }
    }
}

impl fmt::Debug for FalconKeyPair {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FalconKeyPair")
            .field("public_key", &format!("[{} bytes]", self.public_key.len()))
            .field("secret_key", &"[REDACTED]")
            .field("security_level", &self.security_level)
            .finish()
    }
}

/// Parameters for Falcon operations
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct FalconParameters {
    pub security_level: FalconSecurityLevel,
}

impl FalconParameters {
    /// Create new parameters
    pub fn new(security_level: FalconSecurityLevel) -> Self {
        Self { security_level }
    }
    
    /// Create from numeric security level
    pub fn with_security_level(level: u8) -> Result<Self, FalconError> {
        Ok(Self {
            security_level: FalconSecurityLevel::from_level(level)?,
        })
    }
}

/// Convenience function to sign with Falcon
pub fn falcon_sign(
    secret_key: &[u8],
    message: &[u8],
    security_level: FalconSecurityLevel,
) -> Result<Vec<u8>, FalconError> {
    match security_level {
        FalconSecurityLevel::Falcon512 => {
            let sk = falcon512::SecretKey::from_bytes(secret_key)
                .map_err(|_| FalconError::InvalidSecretKey)?;
            let sig = falcon512::detached_sign(message, &sk);
            Ok(sig.as_bytes().to_vec())
        }
        FalconSecurityLevel::Falcon1024 => {
            let sk = falcon1024::SecretKey::from_bytes(secret_key)
                .map_err(|_| FalconError::InvalidSecretKey)?;
            let sig = falcon1024::detached_sign(message, &sk);
            Ok(sig.as_bytes().to_vec())
        }
    }
}

/// Convenience function to verify Falcon signature
pub fn falcon_verify(
    public_key: &[u8],
    message: &[u8],
    signature: &[u8],
    security_level: FalconSecurityLevel,
) -> Result<bool, FalconError> {
    match security_level {
        FalconSecurityLevel::Falcon512 => {
            let pk = falcon512::PublicKey::from_bytes(public_key)
                .map_err(|_| FalconError::InvalidPublicKey)?;
            let sig = falcon512::DetachedSignature::from_bytes(signature)
                .map_err(|_| FalconError::InvalidSignature("Invalid signature".to_string()))?;
            
            match falcon512::verify_detached_signature(&sig, message, &pk) {
                Ok(_) => Ok(true),
                Err(_) => Ok(false),
            }
        }
        FalconSecurityLevel::Falcon1024 => {
            let pk = falcon1024::PublicKey::from_bytes(public_key)
                .map_err(|_| FalconError::InvalidPublicKey)?;
            let sig = falcon1024::DetachedSignature::from_bytes(signature)
                .map_err(|_| FalconError::InvalidSignature("Invalid signature".to_string()))?;
            
            match falcon1024::verify_detached_signature(&sig, message, &pk) {
                Ok(_) => Ok(true),
                Err(_) => Ok(false),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::rngs::OsRng;
    
    #[test]
    fn test_falcon512_key_generation() {
        let mut rng = OsRng;
        let keypair = FalconKeyPair::generate(&mut rng, FalconSecurityLevel::Falcon512)
            .expect("Falcon-512 key generation should succeed");
        
        assert_eq!(keypair.public_key.len(), falcon512::public_key_bytes());
        assert_eq!(keypair.secret_key.len(), falcon512::secret_key_bytes());
    }
    
    #[test]
    fn test_falcon1024_key_generation() {
        let mut rng = OsRng;
        let keypair = FalconKeyPair::generate(&mut rng, FalconSecurityLevel::Falcon1024)
            .expect("Falcon-1024 key generation should succeed");
        
        assert_eq!(keypair.public_key.len(), falcon1024::public_key_bytes());
        assert_eq!(keypair.secret_key.len(), falcon1024::secret_key_bytes());
    }
    
    #[test]
    fn test_falcon_sign_verify() {
        let mut rng = OsRng;
        let message = b"This is a test message for Falcon signatures";
        
        // Test Falcon-512
        let keypair512 = FalconKeyPair::generate(&mut rng, FalconSecurityLevel::Falcon512)
            .expect("Key generation should succeed");
        
        let signature = keypair512.sign(message)
            .expect("Signing should succeed");
        
        let valid = keypair512.verify(message, &signature)
            .expect("Verification should succeed");
        assert!(valid, "Valid signature should verify");
        
        // Test with wrong message
        let wrong_message = b"This is a different message";
        let invalid = keypair512.verify(wrong_message, &signature)
            .expect("Verification should succeed");
        assert!(!invalid, "Invalid signature should not verify");
        
        // Test Falcon-1024
        let keypair1024 = FalconKeyPair::generate(&mut rng, FalconSecurityLevel::Falcon1024)
            .expect("Key generation should succeed");
        
        let signature = keypair1024.sign(message)
            .expect("Signing should succeed");
        
        let valid = keypair1024.verify(message, &signature)
            .expect("Verification should succeed");
        assert!(valid, "Valid signature should verify");
    }
} 