// Quantum-resistant cryptography module
// This implements post-quantum signature schemes for future-proofing the blockchain

use std::fmt;
use serde::{Serialize, Deserialize};
use sha2::{Sha256, Digest};
use rand::{CryptoRng, RngCore};
use pqcrypto_dilithium::{dilithium2, dilithium3, dilithium5};
use pqcrypto_traits::sign::{PublicKey as PQPublicKey, SecretKey as PQSecretKey, DetachedSignature};
use thiserror::Error;
use crate::crypto::falcon::FalconError;

// Adding SPHINCS+ dependencies
use pqcrypto_sphincsplus::{
    sphincssha256128frobust, sphincssha256192frobust, sphincssha256256frobust
};

use crate::validation::SecurityLevel;

// Add secp256k1 and ed25519 dependencies
use secp256k1::{Secp256k1, Message as Secp256k1Message};
use ed25519_dalek::{Keypair as Ed25519Keypair, Signer, Verifier};

/// Mock implementation of dilithium functions to avoid conflicts with pqcrypto_dilithium
mod dilithium_mock {
    pub mod dilithium2 {
        pub const SIGNATUREBYTES: usize = 2420;
        
        pub fn verify_detached_signature(_signature: &[u8], _message: &[u8], _public_key: &[u8]) -> Result<(), ()> {
            // Mock implementation
            Ok(())
        }
    }
    
    pub mod dilithium3 {
        pub const SIGNATUREBYTES: usize = 3293;
        
        pub fn verify_detached_signature(_signature: &[u8], _message: &[u8], _public_key: &[u8]) -> Result<(), ()> {
            // Mock implementation
            Ok(())
        }
    }
    
    pub mod dilithium5 {
        pub const SIGNATUREBYTES: usize = 4595;
        
        pub fn verify_detached_signature(_signature: &[u8], _message: &[u8], _public_key: &[u8]) -> Result<(), ()> {
            // Mock implementation
            Ok(())
        }
    }
}

/// Classical cryptographic schemes for hybrid quantum signatures
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub enum ClassicalScheme {
    /// secp256k1 curve (used in Bitcoin)
    Secp256k1,
    /// Ed25519 curve (used in many modern cryptographic systems)
    Ed25519,
}

/// Quantum-resistant cryptographic schemes
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub enum QuantumScheme {
    /// CRYSTALS-Dilithium signature scheme
    Dilithium,
    /// FALCON signature scheme
    Falcon,
    /// SPHINCS+ signature scheme
    Sphincs,
    /// Hybrid scheme (classical + post-quantum)
    Hybrid(ClassicalScheme),
}

/// Parameters for quantum signatures
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuantumParameters {
    /// The quantum signature scheme to use
    pub scheme: QuantumScheme,
    /// Security level (higher = more secure but larger signatures)
    pub security_level: u8,
}

/// A quantum-resistant key pair
#[derive(Clone, Serialize, Deserialize)]
pub struct QuantumKeyPair {
    /// The public key
    pub public_key: Vec<u8>,
    /// The private key (sensitive information)
    pub secret_key: Vec<u8>,
    /// Parameters used for this key pair
    pub parameters: QuantumParameters,
}

/// A quantum-resistant signature
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuantumSignature {
    /// The signature bytes
    pub signature: Vec<u8>,
    /// Parameters used for this signature
    pub parameters: QuantumParameters,
}

/// Public key variants for different quantum schemes
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum QuantumPublicKey {
    /// Dilithium public key
    Dilithium(Vec<u8>),
    /// Falcon public key
    Falcon(Vec<u8>),
    /// Sphincs+ public key
    Sphincs(Vec<u8>),
    /// Hybrid public key (classical + quantum)
    Hybrid(ClassicalScheme, Vec<u8>, Vec<u8>),
}

/// Secret key variants for different quantum schemes
#[derive(Clone, Serialize, Deserialize)]
pub enum QuantumSecretKey {
    /// Dilithium secret key
    Dilithium(Vec<u8>),
    /// Falcon secret key
    Falcon(Vec<u8>),
    /// Sphincs+ secret key
    Sphincs(Vec<u8>),
    /// Hybrid secret key (classical + quantum)
    Hybrid(ClassicalScheme, Vec<u8>, Vec<u8>),
}

/// Dilithium public key wrapper
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DilithiumPublicKey {
    /// The raw bytes of the public key
    pub bytes: Vec<u8>,
    /// Security level
    pub security_level: u8,
}

/// Dilithium secret key wrapper
#[derive(Clone, Serialize, Deserialize)]
pub struct DilithiumSecretKey {
    /// The raw bytes of the secret key
    pub bytes: Vec<u8>,
    /// Security level
    pub security_level: u8,
}

/// Errors related to quantum operations
#[derive(Error, Debug, Clone, PartialEq, Eq)]
pub enum QuantumError {
    /// Unsupported quantum signature scheme
    #[error("Unsupported quantum scheme: {0}")]
    UnsupportedScheme(String),
    
    /// Invalid key
    #[error("Invalid quantum key: {0}")]
    InvalidKey(String),
    
    /// Invalid signature
    #[error("Invalid quantum signature: {0}")]
    InvalidSignature(String),
    
    /// Signature verification failed
    #[error("Signature verification failed: {0}")]
    VerificationFailed(String),
    
    /// Signing operation failed
    #[error("Signing operation failed: {0}")]
    SigningFailed(String),
    
    /// Key generation failed
    #[error("Key generation failed: {0}")]
    KeyGenerationFailed(String),
    
    /// Unsupported security level
    #[error("Unsupported security level: {0}")]
    UnsupportedSecurityLevel(u8),
    
    /// Cryptographic operation failed
    #[error("Cryptographic operation failed: {0}")]
    CryptoOperationFailed(String),
}

/// Convert FalconError to QuantumError
impl From<FalconError> for QuantumError {
    fn from(error: FalconError) -> Self {
        match error {
            FalconError::InvalidKey(msg) => QuantumError::InvalidKey(msg),
            FalconError::InvalidSignature(msg) => QuantumError::InvalidSignature(msg),
            FalconError::UnsupportedSecurityLevel(level) => QuantumError::UnsupportedSecurityLevel(level),
            FalconError::CryptoOperationFailed(msg) => QuantumError::CryptoOperationFailed(msg),
            FalconError::InvalidPublicKey => QuantumError::InvalidKey("Invalid Falcon public key".to_string()),
            FalconError::InvalidSecretKey => QuantumError::InvalidKey("Invalid Falcon secret key".to_string()),
        }
    }
}

impl QuantumParameters {
    /// Create new quantum parameters with default values
    pub fn new(scheme: QuantumScheme) -> Self {
        Self {
            scheme,
            security_level: 3, // Medium security by default
        }
    }
    
    /// Create new quantum parameters with specified values
    pub fn with_security_level(scheme: QuantumScheme, security_level: u8) -> Self {
        Self {
            scheme,
            security_level,
        }
    }
    
    /// Get the expected signature length for these parameters
    pub fn expected_signature_length(&self) -> Result<usize, QuantumError> {
        match self.scheme {
            QuantumScheme::Dilithium => {
                match SecurityLevel::from(self.security_level) {
                    SecurityLevel::Low => Ok(dilithium_mock::dilithium2::SIGNATUREBYTES),
                    SecurityLevel::Medium => Ok(dilithium_mock::dilithium3::SIGNATUREBYTES),
                    SecurityLevel::High => Ok(dilithium_mock::dilithium5::SIGNATUREBYTES),
                    _ => Err(QuantumError::UnsupportedSecurityLevel(self.security_level)),
                }
            },
            QuantumScheme::Falcon => {
                // Placeholder for Falcon implementation
                Err(QuantumError::CryptoOperationFailed("Falcon signature length calculation not yet implemented".to_string()))
            },
            QuantumScheme::Sphincs => {
                match SecurityLevel::from(self.security_level) {
                    SecurityLevel::Low => Ok(sphincssha256128frobust::signature_bytes()),
                    SecurityLevel::Medium => Ok(sphincssha256192frobust::signature_bytes()),
                    SecurityLevel::High => Ok(sphincssha256256frobust::signature_bytes()),
                    _ => Err(QuantumError::UnsupportedSecurityLevel(self.security_level)),
                }
            },
            QuantumScheme::Hybrid(classical) => {
                // For hybrid, combine classical and quantum signature lengths
                let classical_len = match classical {
                    ClassicalScheme::Secp256k1 => 64, // r, s format
                    ClassicalScheme::Ed25519 => 64,
                };
                
                // Get quantum length and add
                let quantum_len = match SecurityLevel::from(self.security_level) {
                    SecurityLevel::Low => dilithium_mock::dilithium2::SIGNATUREBYTES,
                    SecurityLevel::Medium => dilithium_mock::dilithium3::SIGNATUREBYTES,
                    SecurityLevel::High => dilithium_mock::dilithium5::SIGNATUREBYTES,
                    _ => return Err(QuantumError::UnsupportedSecurityLevel(self.security_level)),
                };
                
                Ok(classical_len + quantum_len)
            }
        }
    }
}

impl QuantumKeyPair {
    /// Generate a new key pair
    pub fn generate<R: CryptoRng + RngCore>(
        rng: &mut R,
        parameters: QuantumParameters,
    ) -> Result<Self, QuantumError> {
        match parameters.scheme {
            QuantumScheme::Dilithium => Self::generate_dilithium(rng, parameters.security_level),
            QuantumScheme::Falcon => Self::generate_falcon(rng, parameters.security_level),
            QuantumScheme::Sphincs => Self::generate_sphincs(rng, parameters.security_level),
            QuantumScheme::Hybrid(classical) => 
                Self::generate_hybrid(rng, parameters.security_level, classical)
        }
    }
    
    // Generate Dilithium key pair
    fn generate_dilithium<R: CryptoRng + RngCore>(
        _rng: &mut R,
        security_level: u8,
    ) -> Result<Self, QuantumError> {
        match SecurityLevel::from(security_level) {
            SecurityLevel::Low => {
                let (pk, sk) = dilithium2::keypair();
                let public_key = DilithiumPublicKey {
                    bytes: pk.as_bytes().to_vec(),
                    security_level,
                };
                let secret_key = DilithiumSecretKey {
                    bytes: sk.as_bytes().to_vec(),
                    security_level,
                };
                
                Ok(Self {
                    public_key: public_key.bytes,
                    secret_key: secret_key.bytes,
                    parameters: QuantumParameters {
                        scheme: QuantumScheme::Dilithium,
                        security_level,
                    },
                })
            },
            SecurityLevel::Medium => {
                let (pk, sk) = dilithium3::keypair();
                let public_key = DilithiumPublicKey {
                    bytes: pk.as_bytes().to_vec(),
                    security_level,
                };
                let secret_key = DilithiumSecretKey {
                    bytes: sk.as_bytes().to_vec(),
                    security_level,
                };
                
                Ok(Self {
                    public_key: public_key.bytes,
                    secret_key: secret_key.bytes,
                    parameters: QuantumParameters {
                        scheme: QuantumScheme::Dilithium,
                        security_level,
                    },
                })
            },
            SecurityLevel::High => {
                let (pk, sk) = dilithium5::keypair();
                let public_key = DilithiumPublicKey {
                    bytes: pk.as_bytes().to_vec(),
                    security_level,
                };
                let secret_key = DilithiumSecretKey {
                    bytes: sk.as_bytes().to_vec(),
                    security_level,
                };
                
                Ok(Self {
                    public_key: public_key.bytes,
                    secret_key: secret_key.bytes,
                    parameters: QuantumParameters {
                        scheme: QuantumScheme::Dilithium,
                        security_level,
                    },
                })
            },
            _ => Err(QuantumError::UnsupportedSecurityLevel(security_level)),
        }
    }
    
    // Generate Falcon key pair
    fn generate_falcon<R: CryptoRng + RngCore>(
        rng: &mut R,
        security_level: u8,
    ) -> Result<Self, QuantumError> {
        // Use our new Falcon implementation
        use crate::crypto::falcon::{FalconKeyPair, FalconParameters};
        
        // Create Falcon parameters
        let params = FalconParameters::with_security_level(security_level)?;
        
        // Create a Falcon keypair
        match FalconKeyPair::generate(rng, params) {
            Ok(falcon_keypair) => {
                // Create a hybrid keypair with a classical and quantum component
                Ok(Self {
                    public_key: falcon_keypair.public_key.clone(),
                    secret_key: falcon_keypair.secret_key.clone(),
                    parameters: QuantumParameters {
                        scheme: QuantumScheme::Falcon,
                        security_level: falcon_keypair.parameters.security_level,
                    },
                })
            },
            Err(err) => {
                Err(QuantumError::CryptoOperationFailed(format!("Falcon key generation failed: {}", err)))
            }
        }
    }
    
    // Generate SPHINCS+ key pair
    fn generate_sphincs<R: CryptoRng + RngCore>(
        _rng: &mut R,
        security_level: u8,
    ) -> Result<Self, QuantumError> {
        match SecurityLevel::from(security_level) {
            SecurityLevel::Low => {
                // Use SHA-256, 128-bit security level, "fast" variant (f)
                let (pk, sk) = sphincssha256128frobust::keypair();
                
                Ok(Self {
                    public_key: pk.as_bytes().to_vec(),
                    secret_key: sk.as_bytes().to_vec(),
                    parameters: QuantumParameters {
                        scheme: QuantumScheme::Sphincs,
                        security_level,
                    },
                })
            },
            SecurityLevel::Medium => {
                // Use SHA-256, 192-bit security level, "fast" variant (f)
                let (pk, sk) = sphincssha256192frobust::keypair();
                
                Ok(Self {
                    public_key: pk.as_bytes().to_vec(),
                    secret_key: sk.as_bytes().to_vec(),
                    parameters: QuantumParameters {
                        scheme: QuantumScheme::Sphincs,
                        security_level,
                    },
                })
            },
            SecurityLevel::High => {
                // Use SHA-256, 256-bit security level, "fast" variant (f)
                let (pk, sk) = sphincssha256256frobust::keypair();
                
                Ok(Self {
                    public_key: pk.as_bytes().to_vec(),
                    secret_key: sk.as_bytes().to_vec(),
                    parameters: QuantumParameters {
                        scheme: QuantumScheme::Sphincs,
                        security_level,
                    },
                })
            },
            _ => Err(QuantumError::UnsupportedSecurityLevel(security_level)),
        }
    }
    
    // Generate hybrid key pair
    fn generate_hybrid<R: CryptoRng + RngCore>(
        rng: &mut R,
        security_level: u8,
        classical: ClassicalScheme,
    ) -> Result<Self, QuantumError> {
        // Generate quantum part - use Dilithium for the quantum component
        let quantum_keypair = match security_level {
            // Low security
            1 | 2 => {
                let (pk, sk) = dilithium2::keypair();
                (pk.as_bytes().to_vec(), sk.as_bytes().to_vec())
            },
            // Medium security (default)
            3 | 4 => {
                let (pk, sk) = dilithium3::keypair();
                (pk.as_bytes().to_vec(), sk.as_bytes().to_vec())
            },
            // High security
            5 => {
                let (pk, sk) = dilithium5::keypair();
                (pk.as_bytes().to_vec(), sk.as_bytes().to_vec())
            },
            _ => return Err(QuantumError::UnsupportedSecurityLevel(security_level)),
        };
        
        // Generate classical part
        let classical_keypair = match classical {
            ClassicalScheme::Secp256k1 => {
                // Create a secp256k1 keypair
                use rand::rngs::OsRng;
                
                // Create a new random seed for key generation
                let mut seed = [0u8; 32];
                OsRng.fill_bytes(&mut seed);
                
                // Create a private key from slice
                let secret_key = secp256k1::SecretKey::from_slice(&seed)
                    .map_err(|e| QuantumError::KeyGenerationFailed(format!("Secp256k1 key error: {}", e)))?;
                
                // Create the secp256k1 context
                let secp = secp256k1::Secp256k1::new();
                
                // Create public key from secret key
                let public_key = secp256k1::PublicKey::from_secret_key(&secp, &secret_key);
                
                // Return the key pair as (private_key, public_key) tuple
                (secret_key.secret_bytes().to_vec(), public_key.serialize().to_vec())
            },
            ClassicalScheme::Ed25519 => {
                // Create an Ed25519 keypair using OsRng which implements both RngCore and CryptoRng
                use rand::rngs::OsRng;
                let mut seed = [0u8; 32];
                OsRng.fill_bytes(&mut seed);
                let keypair = Ed25519Keypair::from_bytes(&seed)
                    .map_err(|_| QuantumError::KeyGenerationFailed("Ed25519 keypair generation failed".to_string()))?;
                (keypair.secret.as_bytes().to_vec(), keypair.public.as_bytes().to_vec())
            },
        };
        
        // Combine keys
        // Format: [classical_public_key_length (2 bytes)][classical_public_key][quantum_public_key]
        let mut combined_public_key = Vec::new();
        let classical_pk_len = classical_keypair.1.len() as u16;
        combined_public_key.extend_from_slice(&classical_pk_len.to_be_bytes());
        combined_public_key.extend_from_slice(&classical_keypair.1);
        combined_public_key.extend_from_slice(&quantum_keypair.0);
        
        // Format: [classical_private_key_length (2 bytes)][classical_private_key][quantum_private_key]
        let mut combined_secret_key = Vec::new();
        let classical_sk_len = classical_keypair.0.len() as u16;
        combined_secret_key.extend_from_slice(&classical_sk_len.to_be_bytes());
        combined_secret_key.extend_from_slice(&classical_keypair.0);
        combined_secret_key.extend_from_slice(&quantum_keypair.1);
        
        Ok(Self {
            public_key: combined_public_key,
            secret_key: combined_secret_key,
            parameters: QuantumParameters {
                scheme: QuantumScheme::Hybrid(classical),
                security_level,
            },
        })
    }
    
    /// Sign a message using the quantum-resistant secret key.
    pub fn sign(&self, message: &[u8]) -> Result<Vec<u8>, QuantumError> {
        match self.parameters.scheme {
            QuantumScheme::Dilithium => {
                match SecurityLevel::from(self.parameters.security_level) {
                    SecurityLevel::Low => {
                        let sk = dilithium2::SecretKey::from_bytes(&self.secret_key)
                            .map_err(|e| QuantumError::InvalidKey(format!("Invalid Dilithium secret key: {}", e)))?;
                        let signature = dilithium2::detached_sign(message, &sk);
                        Ok(signature.as_bytes().to_vec())
                    },
                    SecurityLevel::Medium => {
                        let sk = dilithium3::SecretKey::from_bytes(&self.secret_key)
                            .map_err(|e| QuantumError::InvalidKey(format!("Invalid Dilithium secret key: {}", e)))?;
                        let signature = dilithium3::detached_sign(message, &sk);
                        Ok(signature.as_bytes().to_vec())
                    },
                    SecurityLevel::High => {
                        let sk = dilithium5::SecretKey::from_bytes(&self.secret_key)
                            .map_err(|e| QuantumError::InvalidKey(format!("Invalid Dilithium secret key: {}", e)))?;
                        let signature = dilithium5::detached_sign(message, &sk);
                        Ok(signature.as_bytes().to_vec())
                    },
                    _ => Err(QuantumError::UnsupportedSecurityLevel(self.parameters.security_level)),
                }
            },
            QuantumScheme::Falcon => {
                // Use our new Falcon implementation
                use crate::crypto::falcon::{FalconKeyPair, FalconParameters};
                
                let params = FalconParameters::with_security_level(self.parameters.security_level)?;
                
                let falcon_keypair = FalconKeyPair {
                    public_key: self.public_key.clone(),
                    secret_key: self.secret_key.clone(),
                    parameters: params,
                };
                
                falcon_keypair.sign(message)
                    .map_err(|e| QuantumError::CryptoOperationFailed(format!("Falcon signing failed: {}", e)))
            },
            QuantumScheme::Sphincs => {
                match SecurityLevel::from(self.parameters.security_level) {
                    SecurityLevel::Low => {
                        let sk = sphincssha256128frobust::SecretKey::from_bytes(&self.secret_key)
                            .map_err(|e| QuantumError::InvalidKey(format!("Invalid SPHINCS+ secret key: {}", e)))?;
                        let signature = sphincssha256128frobust::detached_sign(message, &sk);
                        Ok(signature.as_bytes().to_vec())
                    },
                    SecurityLevel::Medium => {
                        let sk = sphincssha256192frobust::SecretKey::from_bytes(&self.secret_key)
                            .map_err(|e| QuantumError::InvalidKey(format!("Invalid SPHINCS+ secret key: {}", e)))?;
                        let signature = sphincssha256192frobust::detached_sign(message, &sk);
                        Ok(signature.as_bytes().to_vec())
                    },
                    SecurityLevel::High => {
                        let sk = sphincssha256256frobust::SecretKey::from_bytes(&self.secret_key)
                            .map_err(|e| QuantumError::InvalidKey(format!("Invalid SPHINCS+ secret key: {}", e)))?;
                        let signature = sphincssha256256frobust::detached_sign(message, &sk);
                        Ok(signature.as_bytes().to_vec())
                    },
                    _ => Err(QuantumError::UnsupportedSecurityLevel(self.parameters.security_level)),
                }
            },
            QuantumScheme::Hybrid(classical_scheme) => {
                // Split the secret key into classical and quantum parts
                if self.secret_key.len() < 2 {
                    return Err(QuantumError::InvalidKey("Invalid hybrid secret key format".to_string()));
                }
                
                let classical_sk_len = u16::from_be_bytes([self.secret_key[0], self.secret_key[1]]) as usize;
                if self.secret_key.len() < 2 + classical_sk_len {
                    return Err(QuantumError::InvalidKey("Invalid hybrid secret key format".to_string()));
                }
                
                let classical_sk = &self.secret_key[2..(2 + classical_sk_len)];
                let quantum_sk = &self.secret_key[(2 + classical_sk_len)..];
                
                // Generate classical signature
                let classical_sig = match classical_scheme {
                    ClassicalScheme::Secp256k1 => {
                        // Create ECDSA signature with secp256k1
                        let secp = Secp256k1::new();
                        let secret_key = secp256k1::SecretKey::from_slice(classical_sk)
                            .map_err(|e| QuantumError::InvalidKey(format!("Invalid secp256k1 key: {}", e)))?;
                        
                        let message_hash = Sha256::digest(message);
                        let secp_msg = Secp256k1Message::from_slice(&message_hash)
                            .map_err(|e| QuantumError::CryptoOperationFailed(format!("Invalid message hash: {}", e)))?;
                            
                        let sig = secp.sign_ecdsa(&secp_msg, &secret_key);
                        sig.serialize_der().to_vec()
                    },
                    ClassicalScheme::Ed25519 => {
                        // Use ed25519 for signing
                        if classical_sk.len() != 32 {
                            return Err(QuantumError::InvalidKey("Invalid Ed25519 secret key length".to_string()));
                        }
                        let mut expanded_key = [0u8; 64];
                        expanded_key[..32].copy_from_slice(classical_sk);
                        
                        let expanded_secret = ed25519_dalek::ExpandedSecretKey::from_bytes(&expanded_key)
                            .map_err(|e| QuantumError::InvalidKey(format!("Invalid Ed25519 key: {}", e)))?;
                            
                        let public_key = {
                            // Extract public key from combined public key
                            if self.public_key.len() < 2 {
                                return Err(QuantumError::InvalidKey("Invalid hybrid public key format".to_string()));
                            }
                            let classical_pk_len = u16::from_be_bytes([self.public_key[0], self.public_key[1]]) as usize;
                            if self.public_key.len() < 2 + classical_pk_len || classical_pk_len != 32 {
                                return Err(QuantumError::InvalidKey("Invalid hybrid public key format".to_string()));
                            }
                            ed25519_dalek::PublicKey::from_bytes(&self.public_key[2..2+classical_pk_len])
                                .map_err(|e| QuantumError::InvalidKey(format!("Invalid Ed25519 public key: {}", e)))?
                        };
                        
                        let sig = expanded_secret.sign(message, &public_key);
                        sig.to_bytes().to_vec()
                    },
                };
                
                // Generate quantum signature
                let quantum_sig = match SecurityLevel::from(self.parameters.security_level) {
                    SecurityLevel::Low => {
                        let sk = dilithium2::SecretKey::from_bytes(quantum_sk)
                            .map_err(|e| QuantumError::InvalidKey(format!("Invalid Dilithium secret key: {}", e)))?;
                        let sig = dilithium2::detached_sign(message, &sk);
                        sig.as_bytes().to_vec()
                    },
                    SecurityLevel::Medium => {
                        let sk = dilithium3::SecretKey::from_bytes(quantum_sk)
                            .map_err(|e| QuantumError::InvalidKey(format!("Invalid Dilithium secret key: {}", e)))?;
                        let sig = dilithium3::detached_sign(message, &sk);
                        sig.as_bytes().to_vec()
                    },
                    SecurityLevel::High => {
                        let sk = dilithium5::SecretKey::from_bytes(quantum_sk)
                            .map_err(|e| QuantumError::InvalidKey(format!("Invalid Dilithium secret key: {}", e)))?;
                        let sig = dilithium5::detached_sign(message, &sk);
                        sig.as_bytes().to_vec()
                    },
                    _ => return Err(QuantumError::UnsupportedSecurityLevel(self.parameters.security_level)),
                };
                
                // Combine signatures with length prefixes
                let mut combined_sig = Vec::new();
                let classical_sig_len = classical_sig.len() as u16;
                combined_sig.extend_from_slice(&classical_sig_len.to_be_bytes());
                combined_sig.extend_from_slice(&classical_sig);
                combined_sig.extend_from_slice(&quantum_sig);
                
                Ok(combined_sig)
            },
        }
    }
    
    /// Verify a signature using the quantum-resistant public key.
    pub fn verify(&self, message: &[u8], signature: &[u8]) -> Result<bool, QuantumError> {
        match self.parameters.scheme {
            QuantumScheme::Dilithium => {
                match SecurityLevel::from(self.parameters.security_level) {
                    SecurityLevel::Low => {
                        let pk = dilithium2::PublicKey::from_bytes(&self.public_key)
                            .map_err(|e| QuantumError::InvalidKey(format!("Invalid Dilithium public key: {}", e)))?;
                        let sig = dilithium2::DetachedSignature::from_bytes(signature)
                            .map_err(|e| QuantumError::InvalidSignature(format!("Invalid Dilithium signature: {}", e)))?;
                        
                        // Verify with dilithium2
                        Ok(dilithium2::verify_detached_signature(&sig, message, &pk).is_ok())
                    },
                    SecurityLevel::Medium => {
                        let pk = dilithium3::PublicKey::from_bytes(&self.public_key)
                            .map_err(|e| QuantumError::InvalidKey(format!("Invalid Dilithium public key: {}", e)))?;
                        let sig = dilithium3::DetachedSignature::from_bytes(signature)
                            .map_err(|e| QuantumError::InvalidSignature(format!("Invalid Dilithium signature: {}", e)))?;
                        
                        // Verify with dilithium3
                        Ok(dilithium3::verify_detached_signature(&sig, message, &pk).is_ok())
                    },
                    SecurityLevel::High => {
                        let pk = dilithium5::PublicKey::from_bytes(&self.public_key)
                            .map_err(|e| QuantumError::InvalidKey(format!("Invalid Dilithium public key: {}", e)))?;
                        let sig = dilithium5::DetachedSignature::from_bytes(signature)
                            .map_err(|e| QuantumError::InvalidSignature(format!("Invalid Dilithium signature: {}", e)))?;
                        
                        // Verify with dilithium5
                        Ok(dilithium5::verify_detached_signature(&sig, message, &pk).is_ok())
                    },
                    _ => Err(QuantumError::UnsupportedSecurityLevel(self.parameters.security_level)),
                }
            },
            QuantumScheme::Falcon => {
                // Use our new Falcon implementation
                use crate::crypto::falcon::{FalconKeyPair, FalconParameters};
                
                let params = FalconParameters::with_security_level(self.parameters.security_level)?;
                
                let falcon_keypair = FalconKeyPair {
                    public_key: self.public_key.clone(),
                    secret_key: self.secret_key.clone(),
                    parameters: params,
                };
                
                falcon_keypair.verify(message, signature)
                    .map_err(|e| QuantumError::CryptoOperationFailed(format!("Falcon verification failed: {}", e)))
            },
            QuantumScheme::Sphincs => {
                match SecurityLevel::from(self.parameters.security_level) {
                    SecurityLevel::Low => {
                        let pk = sphincssha256128frobust::PublicKey::from_bytes(&self.public_key)
                            .map_err(|e| QuantumError::InvalidKey(format!("Invalid SPHINCS+ public key: {}", e)))?;
                        let sig = sphincssha256128frobust::DetachedSignature::from_bytes(signature)
                            .map_err(|e| QuantumError::InvalidSignature(format!("Invalid SPHINCS+ signature: {}", e)))?;
                        
                        match sphincssha256128frobust::verify_detached_signature(&sig, message, &pk) {
                            Ok(_) => Ok(true),
                            Err(_) => Ok(false),
                        }
                    },
                    SecurityLevel::Medium => {
                        let pk = sphincssha256192frobust::PublicKey::from_bytes(&self.public_key)
                            .map_err(|e| QuantumError::InvalidKey(format!("Invalid SPHINCS+ public key: {}", e)))?;
                        let sig = sphincssha256192frobust::DetachedSignature::from_bytes(signature)
                            .map_err(|e| QuantumError::InvalidSignature(format!("Invalid SPHINCS+ signature: {}", e)))?;
                        
                        match sphincssha256192frobust::verify_detached_signature(&sig, message, &pk) {
                            Ok(_) => Ok(true),
                            Err(_) => Ok(false),
                        }
                    },
                    SecurityLevel::High => {
                        let pk = sphincssha256256frobust::PublicKey::from_bytes(&self.public_key)
                            .map_err(|e| QuantumError::InvalidKey(format!("Invalid SPHINCS+ public key: {}", e)))?;
                        let sig = sphincssha256256frobust::DetachedSignature::from_bytes(signature)
                            .map_err(|e| QuantumError::InvalidSignature(format!("Invalid SPHINCS+ signature: {}", e)))?;
                        
                        match sphincssha256256frobust::verify_detached_signature(&sig, message, &pk) {
                            Ok(_) => Ok(true),
                            Err(_) => Ok(false),
                        }
                    },
                    _ => Err(QuantumError::UnsupportedSecurityLevel(self.parameters.security_level)),
                }
            },
            QuantumScheme::Hybrid(classical_scheme) => {
                // Split the signature into classical and quantum parts
                if signature.len() < 2 {
                    return Err(QuantumError::InvalidSignature("Invalid hybrid signature format".to_string()));
                }
                
                let classical_sig_len = u16::from_be_bytes([signature[0], signature[1]]) as usize;
                if signature.len() < 2 + classical_sig_len {
                    return Err(QuantumError::InvalidSignature("Invalid hybrid signature format".to_string()));
                }
                
                let classical_sig = &signature[2..(2 + classical_sig_len)];
                let quantum_sig = &signature[(2 + classical_sig_len)..];
                
                // Split the public key into classical and quantum parts
                if self.public_key.len() < 2 {
                    return Err(QuantumError::InvalidKey("Invalid hybrid public key format".to_string()));
                }
                
                let classical_pk_len = u16::from_be_bytes([self.public_key[0], self.public_key[1]]) as usize;
                if self.public_key.len() < 2 + classical_pk_len {
                    return Err(QuantumError::InvalidKey("Invalid hybrid public key format".to_string()));
                }
                
                let classical_pk = &self.public_key[2..(2 + classical_pk_len)];
                let quantum_pk = &self.public_key[(2 + classical_pk_len)..];
                
                // Verify classical signature
                let classical_valid = match classical_scheme {
                    ClassicalScheme::Secp256k1 => {
                        // Verify ECDSA signature with secp256k1
                        let secp = Secp256k1::new();
                        
                        let message_hash = Sha256::digest(message);
                        let secp_msg = Secp256k1Message::from_slice(&message_hash)
                            .map_err(|e| QuantumError::CryptoOperationFailed(format!("Invalid message hash: {}", e)))?;
                            
                        let public_key = secp256k1::PublicKey::from_slice(classical_pk)
                            .map_err(|e| QuantumError::InvalidKey(format!("Invalid secp256k1 public key: {}", e)))?;
                            
                        let sig = secp256k1::ecdsa::Signature::from_der(classical_sig)
                            .map_err(|e| QuantumError::InvalidSignature(format!("Invalid secp256k1 signature: {}", e)))?;
                            
                        secp.verify_ecdsa(&secp_msg, &sig, &public_key).is_ok()
                    },
                    ClassicalScheme::Ed25519 => {
                        // Verify Ed25519 signature
                        if classical_pk.len() != 32 || classical_sig.len() != 64 {
                            return Err(QuantumError::InvalidSignature("Invalid Ed25519 signature format".to_string()));
                        }
                        
                        let public_key = ed25519_dalek::PublicKey::from_bytes(classical_pk)
                            .map_err(|e| QuantumError::InvalidKey(format!("Invalid Ed25519 public key: {}", e)))?;
                            
                        let sig = ed25519_dalek::Signature::from_bytes(classical_sig)
                            .map_err(|e| QuantumError::InvalidSignature(format!("Invalid Ed25519 signature: {}", e)))?;
                            
                        public_key.verify(message, &sig).is_ok()
                    },
                };
                
                // Verify quantum signature
                let quantum_valid = match SecurityLevel::from(self.parameters.security_level) {
                    SecurityLevel::Low => {
                        let pk = dilithium2::PublicKey::from_bytes(quantum_pk)
                            .map_err(|e| QuantumError::InvalidKey(format!("Invalid Dilithium public key: {}", e)))?;
                        let sig = dilithium2::DetachedSignature::from_bytes(quantum_sig)
                            .map_err(|e| QuantumError::InvalidSignature(format!("Invalid Dilithium signature: {}", e)))?;
                        
                        dilithium2::verify_detached_signature(&sig, message, &pk).is_ok()
                    },
                    SecurityLevel::Medium => {
                        let pk = dilithium3::PublicKey::from_bytes(quantum_pk)
                            .map_err(|e| QuantumError::InvalidKey(format!("Invalid Dilithium public key: {}", e)))?;
                        let sig = dilithium3::DetachedSignature::from_bytes(quantum_sig)
                            .map_err(|e| QuantumError::InvalidSignature(format!("Invalid Dilithium signature: {}", e)))?;
                        
                        dilithium3::verify_detached_signature(&sig, message, &pk).is_ok()
                    },
                    SecurityLevel::High => {
                        let pk = dilithium5::PublicKey::from_bytes(quantum_pk)
                            .map_err(|e| QuantumError::InvalidKey(format!("Invalid Dilithium public key: {}", e)))?;
                        let sig = dilithium5::DetachedSignature::from_bytes(quantum_sig)
                            .map_err(|e| QuantumError::InvalidSignature(format!("Invalid Dilithium signature: {}", e)))?;
                        
                        dilithium5::verify_detached_signature(&sig, message, &pk).is_ok()
                    },
                    _ => return Err(QuantumError::UnsupportedSecurityLevel(self.parameters.security_level)),
                };
                
                // Both signatures must be valid
                Ok(classical_valid && quantum_valid)
            },
        }
    }
}

impl fmt::Debug for QuantumKeyPair {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("QuantumKeyPair")
            .field("public_key", &hex::encode(&self.public_key))
            .field("secret_key", &"[REDACTED]")
            .field("parameters", &self.parameters)
            .finish()
    }
}

impl fmt::Debug for QuantumSecretKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Dilithium(_) => f.write_str("QuantumSecretKey::Dilithium([REDACTED])"),
            Self::Falcon(_) => f.write_str("QuantumSecretKey::Falcon([REDACTED])"),
            Self::Sphincs(_) => f.write_str("QuantumSecretKey::Sphincs([REDACTED])"),
            Self::Hybrid(scheme, _, _) => write!(f, "QuantumSecretKey::Hybrid({:?}, [REDACTED], [REDACTED])", scheme),
        }
    }
}

/// Verify a quantum signature given a public key
pub fn verify_quantum_signature(
    public_key: &[u8],
    message: &[u8],
    signature: &[u8],
    parameters: QuantumParameters,
) -> Result<bool, QuantumError> {
    // Create a keypair with just the public key
    let keypair = QuantumKeyPair {
        public_key: public_key.to_vec(),
        secret_key: vec![],  // Empty secret key since we're only verifying
        parameters,
    };
    
    keypair.verify(message, signature)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::rngs::OsRng;
    
    #[test]
    fn test_dilithium_parameters() {
        let params1 = QuantumParameters::new(QuantumScheme::Dilithium);
        assert_eq!(params1.security_level, 3); // Default should be Medium
        
        let params2 = QuantumParameters::with_security_level(QuantumScheme::Dilithium, 5);
        assert_eq!(params2.security_level, 5); // Should be High
    }
    
    #[test]
    fn test_dilithium_signing_and_verification() {
        let mut rng = OsRng;
        let params = QuantumParameters::with_security_level(QuantumScheme::Dilithium, SecurityLevel::Medium.into());
        
        let keypair = QuantumKeyPair::generate(&mut rng, params).expect("Key generation should succeed");
        let message = b"This is a test message for quantum signature";
        
        // Sign the message
        let signature = keypair.sign(message).expect("Signing should succeed");
        
        // Verify the signature
        let result = keypair.verify(message, &signature).expect("Verification should succeed");
        assert!(result, "Signature verification should return true");
        
        // Try with wrong message
        let wrong_message = b"This is a different message";
        let result = keypair.verify(wrong_message, &signature).expect("Verification with wrong message should succeed");
        assert!(!result, "Verification with wrong message should return false");
    }
    
    #[test]
    fn test_falcon_not_implemented() {
        let mut rng = OsRng;
        let params = QuantumParameters::with_security_level(QuantumScheme::Falcon, SecurityLevel::Medium.into());
        
        let result = QuantumKeyPair::generate(&mut rng, params);
        assert!(result.is_err(), "Falcon should return not implemented error");
        
        if let Err(err) = result {
            match err {
                QuantumError::CryptoOperationFailed(msg) => {
                    assert!(msg.contains("not yet implemented"), "Error should mention implementation pending");
                },
                _ => panic!("Expected CryptoOperationFailed error"),
            }
        }
    }
    
    #[test]
    fn test_sphincs_signing_and_verification() {
        let mut rng = OsRng;
        let parameters = QuantumParameters {
            scheme: QuantumScheme::Sphincs,
            security_level: SecurityLevel::Low as u8,
        };
        
        let keypair = QuantumKeyPair::generate(&mut rng, parameters).expect("SPHINCS+ key generation failed");
        
        let message = b"Test message for SPHINCS+ signatures";
        let signature = keypair.sign(message).expect("SPHINCS+ signing failed");
        
        let result = keypair.verify(message, &signature).expect("SPHINCS+ verification failed");
        assert!(result, "SPHINCS+ signature verification should succeed");
        
        // Modify message to test failed verification
        let modified_message = b"Modified message for SPHINCS+ signatures";
        let modified_result = keypair.verify(modified_message, &signature).expect("SPHINCS+ verification operation failed");
        assert!(!modified_result, "SPHINCS+ signature verification should fail for modified message");
    }
    
    #[test]
    fn test_hybrid_signing_and_verification() {
        let mut rng = OsRng;
        
        // Test both hybrid schemes
        let test_schemes = [
            (ClassicalScheme::Secp256k1, SecurityLevel::Low as u8),
            (ClassicalScheme::Ed25519, SecurityLevel::Medium as u8),
        ];
        
        for (classical_scheme, security_level) in test_schemes.iter() {
            let parameters = QuantumParameters {
                scheme: QuantumScheme::Hybrid(*classical_scheme),
                security_level: *security_level,
            };
        
            let keypair = QuantumKeyPair::generate(&mut rng, parameters)
                .expect("Hybrid key generation failed");
            
            let message = b"Test message for hybrid signatures";
            let signature = keypair.sign(message).expect("Hybrid signing failed");
            
            let result = keypair.verify(message, &signature).expect("Hybrid verification failed");
            assert!(result, "Hybrid signature verification should succeed");
            
            // Modify message to test failed verification
            let modified_message = b"Modified message for hybrid signatures";
            let modified_result = keypair.verify(modified_message, &signature).expect("Hybrid verification operation failed");
            assert!(!modified_result, "Hybrid signature verification should fail for modified message");
        }
    }
} 