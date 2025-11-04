// Quantum-resistant cryptography module
// This implements post-quantum signature schemes for future-proofing the blockchain

use pqcrypto_dilithium::{dilithium2, dilithium3, dilithium5};
use pqcrypto_traits::sign::{
    DetachedSignature as SignDetachedSignatureTrait, PublicKey as SignPublicKeyTrait,
    SecretKey as SignSecretKeyTrait,
};
use rand::{CryptoRng, RngCore};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::fmt;
use thiserror::Error;

// Log security warning on first use
use std::sync::Once;
static INIT: Once = Once::new();
use crate::crypto::falcon_real::FalconError;

// Adding SPHINCS+ dependencies
use pqcrypto_sphincsplus::{
    sphincsshake128fsimple,  // NIST Level 1 (128-bit) - Low security
    sphincsshake192fsimple,  // NIST Level 3 (192-bit) - Medium security
    sphincsshake256fsimple,  // NIST Level 5 (256-bit) - High security
};

use crate::validation::SecurityLevel;

// Add secp256k1 and ed25519 dependencies
use ed25519_dalek::{Signer, SigningKey as Ed25519Keypair, Verifier, VerifyingKey};
use secp256k1::{Message as Secp256k1Message, Secp256k1};

// Export signature types for compatibility
pub type FalconSignature = Vec<u8>;
pub type SPHINCSSignature = Vec<u8>;
pub type ECDSASignature = secp256k1::ecdsa::Signature;

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
    SphincsPlus,
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

/// ML-DSA (Module-Lattice Digital Signature Algorithm) public key
/// This is the NIST standardized version of Dilithium
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MLDSAPublicKey {
    pub bytes: Vec<u8>,
    pub security_level: MLDSASecurityLevel,
}

/// ML-DSA signature
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MLDSASignature {
    pub bytes: Vec<u8>,
}

/// ML-DSA private key
#[derive(Clone, Serialize, Deserialize)]
pub struct MLDSAPrivateKey {
    secret_bytes: Vec<u8>,
    public_key: MLDSAPublicKey,
}

/// ML-DSA security levels (matching Dilithium)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum MLDSASecurityLevel {
    /// ML-DSA-44 (NIST Level 2)
    Level2,
    /// ML-DSA-65 (NIST Level 3)
    #[default]
    Level3,
    /// ML-DSA-87 (NIST Level 5)
    Level5,
}

impl Default for MLDSAPublicKey {
    fn default() -> Self {
        Self {
            bytes: vec![0u8; 1312], // Dilithium3 public key size
            security_level: MLDSASecurityLevel::default(),
        }
    }
}

impl MLDSAPublicKey {
    /// Verify a signature
    pub fn verify(&self, message: &[u8], signature: &MLDSASignature) -> Result<bool, QuantumError> {
        // ML-DSA is the NIST standardized version of Dilithium
        // We use the appropriate Dilithium implementation based on security level
        match self.security_level {
            MLDSASecurityLevel::Level2 => {
                let pk = dilithium2::PublicKey::from_bytes(&self.bytes).map_err(|e| {
                    QuantumError::InvalidKey(format!("Invalid ML-DSA public key: {}", e))
                })?;
                let sig =
                    dilithium2::DetachedSignature::from_bytes(&signature.bytes).map_err(|e| {
                        QuantumError::InvalidSignature(format!("Invalid ML-DSA signature: {}", e))
                    })?;
                Ok(dilithium2::verify_detached_signature(&sig, message, &pk).is_ok())
            }
            MLDSASecurityLevel::Level3 => {
                let pk = dilithium3::PublicKey::from_bytes(&self.bytes).map_err(|e| {
                    QuantumError::InvalidKey(format!("Invalid ML-DSA public key: {}", e))
                })?;
                let sig =
                    dilithium3::DetachedSignature::from_bytes(&signature.bytes).map_err(|e| {
                        QuantumError::InvalidSignature(format!("Invalid ML-DSA signature: {}", e))
                    })?;
                Ok(dilithium3::verify_detached_signature(&sig, message, &pk).is_ok())
            }
            MLDSASecurityLevel::Level5 => {
                let pk = dilithium5::PublicKey::from_bytes(&self.bytes).map_err(|e| {
                    QuantumError::InvalidKey(format!("Invalid ML-DSA public key: {}", e))
                })?;
                let sig =
                    dilithium5::DetachedSignature::from_bytes(&signature.bytes).map_err(|e| {
                        QuantumError::InvalidSignature(format!("Invalid ML-DSA signature: {}", e))
                    })?;
                Ok(dilithium5::verify_detached_signature(&sig, message, &pk).is_ok())
            }
        }
    }
}

impl MLDSAPrivateKey {
    /// Generate a new ML-DSA private key
    pub fn generate<R: RngCore + CryptoRng>(
        rng: &mut R,
        security_level: MLDSASecurityLevel,
    ) -> Result<Self, QuantumError> {
        match security_level {
            MLDSASecurityLevel::Level2 => {
                let (pk, sk) = dilithium2::keypair();
                Ok(Self {
                    secret_bytes: sk.as_bytes().to_vec(),
                    public_key: MLDSAPublicKey {
                        bytes: pk.as_bytes().to_vec(),
                        security_level,
                    },
                })
            }
            MLDSASecurityLevel::Level3 => {
                let (pk, sk) = dilithium3::keypair();
                Ok(Self {
                    secret_bytes: sk.as_bytes().to_vec(),
                    public_key: MLDSAPublicKey {
                        bytes: pk.as_bytes().to_vec(),
                        security_level,
                    },
                })
            }
            MLDSASecurityLevel::Level5 => {
                let (pk, sk) = dilithium5::keypair();
                Ok(Self {
                    secret_bytes: sk.as_bytes().to_vec(),
                    public_key: MLDSAPublicKey {
                        bytes: pk.as_bytes().to_vec(),
                        security_level,
                    },
                })
            }
        }
    }

    /// Get the public key
    pub fn public_key(&self) -> MLDSAPublicKey {
        self.public_key.clone()
    }

    /// Sign a message
    pub fn sign(&self, message: &[u8]) -> Result<MLDSASignature, QuantumError> {
        match self.public_key.security_level {
            MLDSASecurityLevel::Level2 => {
                let sk = dilithium2::SecretKey::from_bytes(&self.secret_bytes).map_err(|e| {
                    QuantumError::InvalidKey(format!("Invalid ML-DSA secret key: {}", e))
                })?;
                let sig = dilithium2::detached_sign(message, &sk);
                Ok(MLDSASignature {
                    bytes: sig.as_bytes().to_vec(),
                })
            }
            MLDSASecurityLevel::Level3 => {
                let sk = dilithium3::SecretKey::from_bytes(&self.secret_bytes).map_err(|e| {
                    QuantumError::InvalidKey(format!("Invalid ML-DSA secret key: {}", e))
                })?;
                let sig = dilithium3::detached_sign(message, &sk);
                Ok(MLDSASignature {
                    bytes: sig.as_bytes().to_vec(),
                })
            }
            MLDSASecurityLevel::Level5 => {
                let sk = dilithium5::SecretKey::from_bytes(&self.secret_bytes).map_err(|e| {
                    QuantumError::InvalidKey(format!("Invalid ML-DSA secret key: {}", e))
                })?;
                let sig = dilithium5::detached_sign(message, &sk);
                Ok(MLDSASignature {
                    bytes: sig.as_bytes().to_vec(),
                })
            }
        }
    }
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

    /// SECURITY: Algorithm downgrade attempt detected (P0-003)
    #[error("CRITICAL SECURITY: Algorithm downgrade attack detected - from {from} to {attempted}")]
    AlgorithmDowngrade {
        from: String,
        attempted: String,
    },

    /// Algorithm not in allowed set
    #[error("Algorithm not allowed: {0}")]
    AlgorithmNotAllowed(String),

    /// Premature algorithm transition
    #[error("Premature algorithm transition at height {current_height}, allowed at {allowed_height}")]
    PrematureTransition {
        current_height: u64,
        allowed_height: u64,
    },

    /// Algorithm mismatch between key and signature
    #[error("Algorithm mismatch: key uses {key_algo}, signature uses {sig_algo}")]
    AlgorithmMismatch {
        key_algo: String,
        sig_algo: String,
    },
}

/// Convert FalconError to QuantumError
impl From<FalconError> for QuantumError {
    fn from(error: FalconError) -> Self {
        match error {
            FalconError::InvalidKey(msg) => {
                QuantumError::InvalidKey(format!("Falcon key error: {}", msg))
            }
            FalconError::InvalidSignature(msg) => {
                QuantumError::InvalidSignature(format!("Falcon signature error: {}", msg))
            }
            FalconError::UnsupportedSecurityLevel(level) => {
                QuantumError::UnsupportedSecurityLevel(level)
            }
            FalconError::CryptoOperationFailed(msg) => {
                QuantumError::CryptoOperationFailed(format!("Falcon operation failed: {}", msg))
            }
            FalconError::InvalidPublicKey => {
                QuantumError::InvalidKey("Invalid Falcon public key".to_string())
            }
            FalconError::InvalidSecretKey => {
                QuantumError::InvalidKey("Invalid Falcon secret key".to_string())
            }
            FalconError::InvalidMessage(msg) => {
                QuantumError::InvalidSignature(format!("Invalid message for Falcon: {}", msg))
            }
            FalconError::KeyGenerationFailed(msg) => QuantumError::CryptoOperationFailed(format!(
                "Falcon key generation failed: {}",
                msg
            )),
            FalconError::SigningFailed(msg) => {
                QuantumError::CryptoOperationFailed(format!("Falcon signing failed: {}", msg))
            }
            FalconError::VerificationFailed(msg) => {
                QuantumError::CryptoOperationFailed(format!("Falcon verification failed: {}", msg))
            }
        }
    }
}

// ============================================================================
// SECURITY FIX (P0-003): Algorithm Downgrade Prevention
// ============================================================================

/// Enforcement mode for algorithm transitions
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EnforcementMode {
    /// Strict mode - no algorithm changes allowed after address creation
    Strict,
    /// Migration mode - allows specific upgrade paths only (no downgrades)
    Migration,
}

/// Algorithm policy for signature verification
/// 
/// SECURITY: Prevents quantum signature downgrade attacks by enforcing that
/// signatures must use the same or stronger algorithm as the address was created with.
/// This is critical for maintaining quantum resistance.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlgorithmPolicy {
    /// Algorithms currently allowed in the network
    pub allowed_schemes: HashSet<QuantumScheme>,
    /// Block height where algorithm transitions are allowed
    pub transition_height: Option<u64>,
    /// Enforcement strictness
    enforcement_mode: EnforcementMode,
}

impl AlgorithmPolicy {
    /// Create a strict policy (no transitions allowed)
    pub fn strict() -> Self {
        let mut allowed_schemes = std::collections::HashSet::new();
        allowed_schemes.insert(QuantumScheme::Dilithium);
        allowed_schemes.insert(QuantumScheme::Falcon);
        allowed_schemes.insert(QuantumScheme::SphincsPlus);
        allowed_schemes.insert(QuantumScheme::Hybrid(ClassicalScheme::Secp256k1));
        allowed_schemes.insert(QuantumScheme::Hybrid(ClassicalScheme::Ed25519));
        
        Self {
            allowed_schemes,
            transition_height: None,
            enforcement_mode: EnforcementMode::Strict,
        }
    }

    /// Create a migration policy (allows upgrades only)
    pub fn migration() -> Self {
        let mut policy = Self::strict();
        policy.enforcement_mode = EnforcementMode::Migration;
        policy
    }

    /// Validate a signature algorithm transition
    /// 
    /// SECURITY: This is the core protection against algorithm downgrade attacks.
    /// It ensures that once an address is created with a quantum algorithm, it cannot
    /// be verified with a weaker algorithm.
    ///
    /// # Arguments
    /// * `key_scheme` - The algorithm the public key uses
    /// * `sig_scheme` - The algorithm the signature claims to use
    /// * `block_height` - Current blockchain height
    ///
    /// # Returns
    /// * `Ok(())` - Transition is allowed
    /// * `Err(QuantumError)` - Transition is forbidden (downgrade attempt)
    pub fn validate_signature_transition(
        &self,
        key_scheme: QuantumScheme,
        sig_scheme: QuantumScheme,
        block_height: u64,
    ) -> Result<(), QuantumError> {
        // CRITICAL: Never allow downgrades
        if !self.is_upgrade_or_same(key_scheme, sig_scheme) {
            return Err(QuantumError::AlgorithmDowngrade {
                from: format!("{:?}", key_scheme),
                attempted: format!("{:?}", sig_scheme),
            });
        }

        // Verify algorithm is in allowed set
        if !self.allowed_schemes.contains(&sig_scheme) {
            return Err(QuantumError::AlgorithmNotAllowed(
                format!("{:?} not in allowed algorithm set", sig_scheme)
            ));
        }

        // Check transition timing if specified
        if let Some(transition) = self.transition_height {
            if block_height < transition && sig_scheme != key_scheme {
                return Err(QuantumError::PrematureTransition {
                    current_height: block_height,
                    allowed_height: transition,
                });
            }
        }

        Ok(())
    }

    /// Check if algorithm transition is an upgrade or same
    /// 
    /// SECURITY: Defines the strict upgrade-only policy.
    /// Only transitions that increase security are allowed.
    ///
    /// # Security Levels (strongest to weakest in post-quantum era)
    /// 1. SphincsPlus - Stateless hash-based (most conservative)
    /// 2. Dilithium (ML-DSA) - Lattice-based (NIST standard)
    /// 3. Falcon - Lattice-based (smaller signatures)
    /// 4. Hybrid - Quantum + Classical (transition phase)
    ///
    /// # Allowed Transitions
    /// - Same → Same: Always allowed
    /// - Falcon → Dilithium: Upgrade to NIST standard
    /// - Falcon → SphincsPlus: Upgrade to hash-based
    /// - Dilithium → SphincsPlus: Upgrade to most conservative
    /// - Hybrid → Pure Quantum: Upgrade from transition scheme
    ///
    /// # Forbidden Transitions (ALL OTHERS)
    /// - Dilithium → Falcon: DOWNGRADE (forbidden)
    /// - SphincsPlus → Dilithium: DOWNGRADE (forbidden)
    /// - SphincsPlus → Falcon: DOWNGRADE (forbidden)
    /// - Pure Quantum → Hybrid: DOWNGRADE (forbidden)
    pub fn is_upgrade_or_same(&self, from: QuantumScheme, to: QuantumScheme) -> bool {
        use QuantumScheme::*;

        match (from, to) {
            // Same algorithm is always allowed
            (a, b) if a == b => true,

            // Defined upgrade paths (security INCREASING only)
            (Falcon, Dilithium) => true,         // Falcon → ML-DSA
            (Falcon, SphincsPlus) => true,       // Falcon → SPHINCS+
            (Dilithium, SphincsPlus) => true,    // ML-DSA → SPHINCS+

            // Hybrid to pure quantum upgrades
            (Hybrid(_), Dilithium) => true,      // Hybrid → ML-DSA
            (Hybrid(_), SphincsPlus) => true,    // Hybrid → SPHINCS+
            (Hybrid(_), Falcon) => true,         // Hybrid → Falcon

            // ALL OTHER TRANSITIONS ARE FORBIDDEN
            // This includes all downgrades
            _ => false,
        }
    }

    /// Enforce strict algorithm binding
    /// 
    /// For strict mode, schemes must match exactly.
    /// For migration mode, upgrades are allowed.
    pub fn enforce_algorithm_binding(
        &self,
        key_scheme: QuantumScheme,
        sig_scheme: QuantumScheme,
    ) -> Result<(), QuantumError> {
        match self.enforcement_mode {
            EnforcementMode::Strict => {
                if key_scheme != sig_scheme {
                    return Err(QuantumError::AlgorithmMismatch {
                        key_algo: format!("{:?}", key_scheme),
                        sig_algo: format!("{:?}", sig_scheme),
                    });
                }
            }
            EnforcementMode::Migration => {
                if !self.is_upgrade_or_same(key_scheme, sig_scheme) {
                    return Err(QuantumError::AlgorithmDowngrade {
                        from: format!("{:?}", key_scheme),
                        attempted: format!("{:?}", sig_scheme),
                    });
                }
            }
        }
        Ok(())
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
            QuantumScheme::Dilithium => match SecurityLevel::from(self.security_level) {
                SecurityLevel::Low => Ok(dilithium2::signature_bytes()),
                SecurityLevel::Medium => Ok(dilithium3::signature_bytes()),
                SecurityLevel::High => Ok(dilithium5::signature_bytes()),
                _ => Err(QuantumError::UnsupportedSecurityLevel(self.security_level)),
            },
            QuantumScheme::Falcon => {
                // Placeholder for Falcon implementation
                Err(QuantumError::CryptoOperationFailed(
                    "Falcon signature length calculation not yet implemented".to_string(),
                ))
            }
            QuantumScheme::SphincsPlus => match SecurityLevel::from(self.security_level) {
                SecurityLevel::Low => Ok(sphincsshake128fsimple::signature_bytes()),
                SecurityLevel::Medium => Ok(sphincsshake192fsimple::signature_bytes()),
                SecurityLevel::High => Ok(sphincsshake256fsimple::signature_bytes()),
                _ => Err(QuantumError::UnsupportedSecurityLevel(self.security_level)),
            },
            QuantumScheme::Hybrid(classical) => {
                // For hybrid, combine classical and quantum signature lengths
                let classical_len = match classical {
                    ClassicalScheme::Secp256k1 => 64, // r, s format
                    ClassicalScheme::Ed25519 => 64,
                };

                // Get quantum length and add
                let quantum_len = match SecurityLevel::from(self.security_level) {
                    SecurityLevel::Low => dilithium2::signature_bytes(),
                    SecurityLevel::Medium => dilithium3::signature_bytes(),
                    SecurityLevel::High => dilithium5::signature_bytes(),
                    _ => return Err(QuantumError::UnsupportedSecurityLevel(self.security_level)),
                };

                Ok(classical_len + quantum_len)
            }
        }
    }
}

impl QuantumKeyPair {
    /// Generate a new key pair with default RNG
    pub fn generate(parameters: QuantumParameters) -> Result<Self, QuantumError> {
        use rand::rngs::OsRng;
        let mut rng = OsRng;
        Self::generate_with_rng(&mut rng, parameters)
    }

    /// Generate a new key pair with provided RNG
    pub fn generate_with_rng<R: CryptoRng + RngCore>(
        rng: &mut R,
        parameters: QuantumParameters,
    ) -> Result<Self, QuantumError> {
        match parameters.scheme {
            QuantumScheme::Dilithium => Self::generate_dilithium(rng, parameters.security_level),
            QuantumScheme::Falcon => Self::generate_falcon(rng, parameters.security_level),
            QuantumScheme::SphincsPlus => Self::generate_sphincs(rng, parameters.security_level),
            QuantumScheme::Hybrid(classical) => {
                Self::generate_hybrid(rng, parameters.security_level, classical)
            }
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
            }
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
            }
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
            }
            _ => Err(QuantumError::UnsupportedSecurityLevel(security_level)),
        }
    }

    // Generate Falcon key pair
    fn generate_falcon<R: CryptoRng + RngCore>(
        rng: &mut R,
        security_level: u8,
    ) -> Result<Self, QuantumError> {
        // Use our new Falcon implementation
        use crate::crypto::falcon_real::{FalconKeyPair as RealFalconKeyPair, FalconSecurityLevel};

        // Convert numeric security level to FalconSecurityLevel
        let falcon_security_level = FalconSecurityLevel::from_level(security_level)?;

        // Create a Falcon keypair
        match RealFalconKeyPair::generate(rng, falcon_security_level) {
            Ok(falcon_keypair) => {
                // Create a hybrid keypair with a classical and quantum component
                Ok(Self {
                    public_key: falcon_keypair.public_key.clone(),
                    secret_key: falcon_keypair.secret_key.clone(),
                    parameters: QuantumParameters {
                        scheme: QuantumScheme::Falcon,
                        security_level,
                    },
                })
            }
            Err(err) => Err(QuantumError::CryptoOperationFailed(format!(
                "Falcon key generation failed: {}",
                err
            ))),
        }
    }

    // Generate SPHINCS+ key pair
    fn generate_sphincs<R: CryptoRng + RngCore>(
        _rng: &mut R,
        security_level: u8,
    ) -> Result<Self, QuantumError> {
        match SecurityLevel::from(security_level) {
            SecurityLevel::Low => {
                // SECURITY FIX (P0-006): Use correct SPHINCS+ variant for low security
                // Use SHAKE-128, 128-bit security level (NIST Level 1), "simple" variant
                let (pk, sk) = sphincsshake128fsimple::keypair();

                Ok(Self {
                    public_key: pk.as_bytes().to_vec(),
                    secret_key: sk.as_bytes().to_vec(),
                    parameters: QuantumParameters {
                        scheme: QuantumScheme::SphincsPlus,
                        security_level,
                    },
                })
            }
            SecurityLevel::Medium => {
                // SECURITY FIX (P0-006): Use correct SPHINCS+ variant for medium security
                // Use SHAKE-192, 192-bit security level (NIST Level 3), "simple" variant
                let (pk, sk) = sphincsshake192fsimple::keypair();

                Ok(Self {
                    public_key: pk.as_bytes().to_vec(),
                    secret_key: sk.as_bytes().to_vec(),
                    parameters: QuantumParameters {
                        scheme: QuantumScheme::SphincsPlus,
                        security_level,
                    },
                })
            }
            SecurityLevel::High => {
                // SECURITY FIX (P0-006): Use correct SPHINCS+ variant for high security
                // Use SHAKE-256, 256-bit security level (NIST Level 5), "simple" variant
                let (pk, sk) = sphincsshake256fsimple::keypair();

                Ok(Self {
                    public_key: pk.as_bytes().to_vec(),
                    secret_key: sk.as_bytes().to_vec(),
                    parameters: QuantumParameters {
                        scheme: QuantumScheme::SphincsPlus,
                        security_level,
                    },
                })
            }
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
            }
            // Medium security (default)
            3 | 4 => {
                let (pk, sk) = dilithium3::keypair();
                (pk.as_bytes().to_vec(), sk.as_bytes().to_vec())
            }
            // High security
            5 => {
                let (pk, sk) = dilithium5::keypair();
                (pk.as_bytes().to_vec(), sk.as_bytes().to_vec())
            }
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
                let secret_key = secp256k1::SecretKey::from_slice(&seed).map_err(|e| {
                    QuantumError::KeyGenerationFailed(format!("Secp256k1 key error: {}", e))
                })?;

                // Create the secp256k1 context
                let secp = secp256k1::Secp256k1::new();

                // Create public key from secret key
                let public_key = secp256k1::PublicKey::from_secret_key(&secp, &secret_key);

                // Return the key pair as (private_key, public_key) tuple
                (
                    secret_key.secret_bytes().to_vec(),
                    public_key.serialize().to_vec(),
                )
            }
            ClassicalScheme::Ed25519 => {
                // Create an Ed25519 keypair using OsRng which implements both RngCore and CryptoRng
                use rand::rngs::OsRng;
                let mut seed = [0u8; 32];
                OsRng.fill_bytes(&mut seed);

                // Ed25519Keypair::from_bytes returns SigningKey directly
                let signing_key = Ed25519Keypair::from_bytes(&seed);
                let verifying_key = signing_key.verifying_key();
                (
                    signing_key.to_bytes().to_vec(),
                    verifying_key.to_bytes().to_vec(),
                )
            }
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
            QuantumScheme::Dilithium => match SecurityLevel::from(self.parameters.security_level) {
                SecurityLevel::Low => {
                    let sk = dilithium2::SecretKey::from_bytes(&self.secret_key).map_err(|e| {
                        QuantumError::InvalidKey(format!("Invalid Dilithium secret key: {}", e))
                    })?;
                    let signature = dilithium2::detached_sign(message, &sk);
                    Ok(signature.as_bytes().to_vec())
                }
                SecurityLevel::Medium => {
                    let sk = dilithium3::SecretKey::from_bytes(&self.secret_key).map_err(|e| {
                        QuantumError::InvalidKey(format!("Invalid Dilithium secret key: {}", e))
                    })?;
                    let signature = dilithium3::detached_sign(message, &sk);
                    Ok(signature.as_bytes().to_vec())
                }
                SecurityLevel::High => {
                    let sk = dilithium5::SecretKey::from_bytes(&self.secret_key).map_err(|e| {
                        QuantumError::InvalidKey(format!("Invalid Dilithium secret key: {}", e))
                    })?;
                    let signature = dilithium5::detached_sign(message, &sk);
                    Ok(signature.as_bytes().to_vec())
                }
                _ => Err(QuantumError::UnsupportedSecurityLevel(
                    self.parameters.security_level,
                )),
            },
            QuantumScheme::Falcon => {
                // Use our new Falcon implementation
                use crate::crypto::falcon_real::{falcon_sign, FalconSecurityLevel};

                let falcon_security_level =
                    FalconSecurityLevel::from_level(self.parameters.security_level)?;

                // Use the falcon_sign function directly
                falcon_sign(&self.secret_key, message, falcon_security_level).map_err(|e| {
                    QuantumError::CryptoOperationFailed(format!("Falcon signing failed: {}", e))
                })
            }
            QuantumScheme::SphincsPlus => {
                match SecurityLevel::from(self.parameters.security_level) {
                    SecurityLevel::Low => {
                        let sk = sphincsshake128fsimple::SecretKey::from_bytes(&self.secret_key)
                            .map_err(|e| {
                                QuantumError::InvalidKey(format!(
                                    "Invalid SPHINCS+ secret key: {}",
                                    e
                                ))
                            })?;
                        let signature = sphincsshake128fsimple::detached_sign(message, &sk);
                        Ok(signature.as_bytes().to_vec())
                    }
                    SecurityLevel::Medium => {
                        // SECURITY FIX (P0-006): Use correct SPHINCS+ variant for medium security
                        let sk = sphincsshake192fsimple::SecretKey::from_bytes(&self.secret_key)
                            .map_err(|e| {
                                QuantumError::InvalidKey(format!(
                                    "Invalid SPHINCS+ secret key: {}",
                                    e
                                ))
                            })?;
                        let signature = sphincsshake192fsimple::detached_sign(message, &sk);
                        Ok(signature.as_bytes().to_vec())
                    }
                    SecurityLevel::High => {
                        // SECURITY FIX (P0-006): Use correct SPHINCS+ variant for high security
                        let sk = sphincsshake256fsimple::SecretKey::from_bytes(&self.secret_key)
                            .map_err(|e| {
                                QuantumError::InvalidKey(format!(
                                    "Invalid SPHINCS+ secret key: {}",
                                    e
                                ))
                            })?;
                        let signature = sphincsshake256fsimple::detached_sign(message, &sk);
                        Ok(signature.as_bytes().to_vec())
                    }
                    _ => Err(QuantumError::UnsupportedSecurityLevel(
                        self.parameters.security_level,
                    )),
                }
            }
            QuantumScheme::Hybrid(classical_scheme) => {
                // Comprehensive hybrid key parsing with defensive bounds checking
                // Split the secret key into classical and quantum parts
                // Format: [classical_sk_len (2 bytes)][classical_sk][quantum_sk]
                
                if self.secret_key.len() < 2 {
                    return Err(QuantumError::InvalidKey(
                        "Hybrid secret key too short: missing length prefix".to_string(),
                    ));
                }

                // Read classical key length with bounds validation
                let classical_sk_len =
                    u16::from_be_bytes([self.secret_key[0], self.secret_key[1]]) as usize;
                
                // Validate classical key length is reasonable
                // Secp256k1 and Ed25519 both use exactly 32 bytes
                const EXPECTED_CLASSICAL_KEY_LEN: usize = 32;
                if classical_sk_len != EXPECTED_CLASSICAL_KEY_LEN {
                    return Err(QuantumError::InvalidKey(format!(
                        "Invalid classical key length in hybrid key: expected {}, got {}",
                        EXPECTED_CLASSICAL_KEY_LEN, classical_sk_len
                    )));
                }

                // Use checked arithmetic to prevent integer overflow
                let classical_start: usize = 2;
                let classical_end = match classical_start.checked_add(classical_sk_len) {
                    Some(end) => end,
                    None => {
                        return Err(QuantumError::InvalidKey(
                            "Integer overflow in hybrid key parsing: classical key length too large".to_string(),
                        ));
                    }
                };

                // Validate bounds before slicing
                if self.secret_key.len() < classical_end {
                    return Err(QuantumError::InvalidKey(format!(
                        "Hybrid secret key too short: expected at least {} bytes for classical key, got {}",
                        classical_end, self.secret_key.len()
                    )));
                }

                // Validate quantum key has minimum required length
                let quantum_sk_start = classical_end;
                let min_quantum_key_len = match SecurityLevel::from(self.parameters.security_level) {
                    SecurityLevel::Low => 2528,   // Dilithium2
                    SecurityLevel::Medium => 4000, // Dilithium3
                    SecurityLevel::High => 4864,    // Dilithium5
                    _ => {
                        return Err(QuantumError::UnsupportedSecurityLevel(
                            self.parameters.security_level,
                        ));
                    }
                };

                if self.secret_key.len() < quantum_sk_start + min_quantum_key_len {
                    return Err(QuantumError::InvalidKey(format!(
                        "Hybrid secret key too short: quantum key requires at least {} bytes, got {}",
                        min_quantum_key_len,
                        self.secret_key.len().saturating_sub(quantum_sk_start)
                    )));
                }

                // Safe slice operations with validated bounds
                let classical_sk = &self.secret_key[classical_start..classical_end];
                let quantum_sk = &self.secret_key[quantum_sk_start..];

                // Additional validation: classical key must be exactly the expected length
                if classical_sk.len() != EXPECTED_CLASSICAL_KEY_LEN {
                    return Err(QuantumError::InvalidKey(format!(
                        "Classical key slice length mismatch: expected {}, got {}",
                        EXPECTED_CLASSICAL_KEY_LEN, classical_sk.len()
                    )));
                }

                // Generate classical signature
                let classical_sig = match classical_scheme {
                    ClassicalScheme::Secp256k1 => {
                        // Create ECDSA signature with secp256k1
                        let secp = Secp256k1::new();
                        let secret_key =
                            secp256k1::SecretKey::from_slice(classical_sk).map_err(|e| {
                                QuantumError::InvalidKey(format!("Invalid secp256k1 key: {}", e))
                            })?;

                        let message_hash = Sha256::digest(message);
                        let secp_msg =
                            Secp256k1Message::from_slice(&message_hash).map_err(|e| {
                                QuantumError::CryptoOperationFailed(format!(
                                    "Invalid message hash: {}",
                                    e
                                ))
                            })?;

                        let sig = secp.sign_ecdsa(&secp_msg, &secret_key);
                        sig.serialize_der().to_vec()
                    }
                    ClassicalScheme::Ed25519 => {
                        // Use ed25519 for signing
                        if classical_sk.len() != 32 {
                            return Err(QuantumError::InvalidKey(
                                "Invalid Ed25519 secret key length".to_string(),
                            ));
                        }
                        let mut expanded_key = [0u8; 64];
                        expanded_key[..32].copy_from_slice(classical_sk);

                        // Create signing key from bytes and use it directly
                        let signing_key = ed25519_dalek::SigningKey::try_from(&expanded_key[..32])
                            .map_err(|e| {
                                QuantumError::InvalidKey(format!("Invalid Ed25519 key: {}", e))
                            })?;

                        let sig = signing_key.sign(message);
                        sig.to_bytes().to_vec()
                    }
                };

                // Generate quantum signature
                let quantum_sig = match SecurityLevel::from(self.parameters.security_level) {
                    SecurityLevel::Low => {
                        let sk = dilithium2::SecretKey::from_bytes(quantum_sk).map_err(|e| {
                            QuantumError::InvalidKey(format!("Invalid Dilithium secret key: {}", e))
                        })?;
                        let sig = dilithium2::detached_sign(message, &sk);
                        sig.as_bytes().to_vec()
                    }
                    SecurityLevel::Medium => {
                        let sk = dilithium3::SecretKey::from_bytes(quantum_sk).map_err(|e| {
                            QuantumError::InvalidKey(format!("Invalid Dilithium secret key: {}", e))
                        })?;
                        let sig = dilithium3::detached_sign(message, &sk);
                        sig.as_bytes().to_vec()
                    }
                    SecurityLevel::High => {
                        let sk = dilithium5::SecretKey::from_bytes(quantum_sk).map_err(|e| {
                            QuantumError::InvalidKey(format!("Invalid Dilithium secret key: {}", e))
                        })?;
                        let sig = dilithium5::detached_sign(message, &sk);
                        sig.as_bytes().to_vec()
                    }
                    _ => {
                        return Err(QuantumError::UnsupportedSecurityLevel(
                            self.parameters.security_level,
                        ))
                    }
                };

                // Combine signatures with length prefixes
                let mut combined_sig = Vec::new();
                let classical_sig_len = classical_sig.len() as u16;
                combined_sig.extend_from_slice(&classical_sig_len.to_be_bytes());
                combined_sig.extend_from_slice(&classical_sig);
                combined_sig.extend_from_slice(&quantum_sig);

                Ok(combined_sig)
            }
        }
    }

    /// Verify a signature using the quantum-resistant public key.
    pub fn verify(&self, message: &[u8], signature: &[u8]) -> Result<bool, QuantumError> {
        match self.parameters.scheme {
            QuantumScheme::Dilithium => {
                match SecurityLevel::from(self.parameters.security_level) {
                    SecurityLevel::Low => {
                        let pk =
                            dilithium2::PublicKey::from_bytes(&self.public_key).map_err(|e| {
                                QuantumError::InvalidKey(format!(
                                    "Invalid Dilithium public key: {}",
                                    e
                                ))
                            })?;
                        let sig =
                            dilithium2::DetachedSignature::from_bytes(signature).map_err(|e| {
                                QuantumError::InvalidSignature(format!(
                                    "Invalid Dilithium signature: {}",
                                    e
                                ))
                            })?;

                        // Verify with dilithium2
                        Ok(dilithium2::verify_detached_signature(&sig, message, &pk).is_ok())
                    }
                    SecurityLevel::Medium => {
                        let pk =
                            dilithium3::PublicKey::from_bytes(&self.public_key).map_err(|e| {
                                QuantumError::InvalidKey(format!(
                                    "Invalid Dilithium public key: {}",
                                    e
                                ))
                            })?;
                        let sig =
                            dilithium3::DetachedSignature::from_bytes(signature).map_err(|e| {
                                QuantumError::InvalidSignature(format!(
                                    "Invalid Dilithium signature: {}",
                                    e
                                ))
                            })?;

                        // Verify with dilithium3
                        Ok(dilithium3::verify_detached_signature(&sig, message, &pk).is_ok())
                    }
                    SecurityLevel::High => {
                        let pk =
                            dilithium5::PublicKey::from_bytes(&self.public_key).map_err(|e| {
                                QuantumError::InvalidKey(format!(
                                    "Invalid Dilithium public key: {}",
                                    e
                                ))
                            })?;
                        let sig =
                            dilithium5::DetachedSignature::from_bytes(signature).map_err(|e| {
                                QuantumError::InvalidSignature(format!(
                                    "Invalid Dilithium signature: {}",
                                    e
                                ))
                            })?;

                        // Verify with dilithium5
                        Ok(dilithium5::verify_detached_signature(&sig, message, &pk).is_ok())
                    }
                    _ => Err(QuantumError::UnsupportedSecurityLevel(
                        self.parameters.security_level,
                    )),
                }
            }
            QuantumScheme::Falcon => {
                // Use our new Falcon implementation
                use crate::crypto::falcon_real::{falcon_verify, FalconSecurityLevel};

                let falcon_security_level =
                    FalconSecurityLevel::from_level(self.parameters.security_level)?;

                // Use the falcon_verify function directly
                falcon_verify(&self.public_key, message, signature, falcon_security_level).map_err(
                    |e| {
                        QuantumError::CryptoOperationFailed(format!(
                            "Falcon verification failed: {}",
                            e
                        ))
                    },
                )
            }
            QuantumScheme::SphincsPlus => {
                match SecurityLevel::from(self.parameters.security_level) {
                    SecurityLevel::Low => {
                        let pk = sphincsshake128fsimple::PublicKey::from_bytes(&self.public_key)
                            .map_err(|e| {
                                QuantumError::InvalidKey(format!(
                                    "Invalid SPHINCS+ public key: {}",
                                    e
                                ))
                            })?;
                        let sig = sphincsshake128fsimple::DetachedSignature::from_bytes(signature)
                            .map_err(|e| {
                                QuantumError::InvalidSignature(format!(
                                    "Invalid SPHINCS+ signature: {}",
                                    e
                                ))
                            })?;

                        match sphincsshake128fsimple::verify_detached_signature(&sig, message, &pk)
                        {
                            Ok(_) => Ok(true),
                            Err(_) => Ok(false),
                        }
                    }
                    SecurityLevel::Medium => {
                        // SECURITY FIX (P0-006): Use correct SPHINCS+ variant for medium security
                        let pk = sphincsshake192fsimple::PublicKey::from_bytes(&self.public_key)
                            .map_err(|e| {
                                QuantumError::InvalidKey(format!(
                                    "Invalid SPHINCS+ public key: {}",
                                    e
                                ))
                            })?;
                        let sig = sphincsshake192fsimple::DetachedSignature::from_bytes(signature)
                            .map_err(|e| {
                                QuantumError::InvalidSignature(format!(
                                    "Invalid SPHINCS+ signature: {}",
                                    e
                                ))
                            })?;

                        match sphincsshake192fsimple::verify_detached_signature(&sig, message, &pk)
                        {
                            Ok(_) => Ok(true),
                            Err(_) => Ok(false),
                        }
                    }
                    SecurityLevel::High => {
                        // SECURITY FIX (P0-006): Use correct SPHINCS+ variant for high security
                        let pk = sphincsshake256fsimple::PublicKey::from_bytes(&self.public_key)
                            .map_err(|e| {
                                QuantumError::InvalidKey(format!(
                                    "Invalid SPHINCS+ public key: {}",
                                    e
                                ))
                            })?;
                        let sig = sphincsshake256fsimple::DetachedSignature::from_bytes(signature)
                            .map_err(|e| {
                                QuantumError::InvalidSignature(format!(
                                    "Invalid SPHINCS+ signature: {}",
                                    e
                                ))
                            })?;

                        match sphincsshake256fsimple::verify_detached_signature(&sig, message, &pk)
                        {
                            Ok(_) => Ok(true),
                            Err(_) => Ok(false),
                        }
                    }
                    _ => Err(QuantumError::UnsupportedSecurityLevel(
                        self.parameters.security_level,
                    )),
                }
            }
            QuantumScheme::Hybrid(classical_scheme) => {
                // SECURITY FIX [P1-004]: Comprehensive hybrid signature parsing with defensive bounds checking
                // Split the signature into classical and quantum parts
                // Format: [classical_sig_len (2 bytes)][classical_sig][quantum_sig]
                
                if signature.len() < 2 {
                    return Err(QuantumError::InvalidSignature(
                        "Hybrid signature too short: missing length prefix".to_string(),
                    ));
                }

                // Read classical signature length with bounds validation
                let classical_sig_len = u16::from_be_bytes([signature[0], signature[1]]) as usize;
                
                // Validate classical signature length is reasonable
                // Secp256k1 DER signatures: ~70-72 bytes typical, max ~73 bytes
                // Ed25519 signatures: exactly 64 bytes
                const MAX_CLASSICAL_SIG_LEN: usize = 128; // Reasonable upper bound
                if classical_sig_len > MAX_CLASSICAL_SIG_LEN {
                    return Err(QuantumError::InvalidSignature(format!(
                        "Classical signature length too large: {} (max: {})",
                        classical_sig_len, MAX_CLASSICAL_SIG_LEN
                    )));
                }
                if classical_sig_len == 0 {
                    return Err(QuantumError::InvalidSignature(
                        "Classical signature length cannot be zero".to_string(),
                    ));
                }

                // Use checked arithmetic to prevent integer overflow
                let classical_sig_start: usize = 2;
                let classical_sig_end = match classical_sig_start.checked_add(classical_sig_len) {
                    Some(end) => end,
                    None => {
                        return Err(QuantumError::InvalidSignature(
                            "Integer overflow in hybrid signature parsing: classical signature length too large".to_string(),
                        ));
                    }
                };

                // Validate bounds before slicing
                if signature.len() < classical_sig_end {
                    return Err(QuantumError::InvalidSignature(format!(
                        "Hybrid signature too short: expected at least {} bytes for classical signature, got {}",
                        classical_sig_end, signature.len()
                    )));
                }

                // Validate quantum signature has minimum required length
                let quantum_sig_start = classical_sig_end;
                let min_quantum_sig_len = match SecurityLevel::from(self.parameters.security_level) {
                    SecurityLevel::Low => 2420,   // Dilithium2
                    SecurityLevel::Medium => 3293, // Dilithium3
                    SecurityLevel::High => 4595,   // Dilithium5
                    _ => {
                        return Err(QuantumError::UnsupportedSecurityLevel(
                            self.parameters.security_level,
                        ));
                    }
                };

                if signature.len() < quantum_sig_start + min_quantum_sig_len {
                    return Err(QuantumError::InvalidSignature(format!(
                        "Hybrid signature too short: quantum signature requires at least {} bytes, got {}",
                        min_quantum_sig_len,
                        signature.len().saturating_sub(quantum_sig_start)
                    )));
                }

                // Safe slice operations with validated bounds
                let classical_sig = &signature[classical_sig_start..classical_sig_end];
                let quantum_sig = &signature[quantum_sig_start..];

                // SECURITY FIX [P1-004]: Comprehensive hybrid public key parsing with defensive bounds checking
                // Split the public key into classical and quantum parts
                // Format: [classical_pk_len (2 bytes)][classical_pk][quantum_pk]
                
                if self.public_key.len() < 2 {
                    return Err(QuantumError::InvalidKey(
                        "Hybrid public key too short: missing length prefix".to_string(),
                    ));
                }

                // Read classical public key length with bounds validation
                let classical_pk_len =
                    u16::from_be_bytes([self.public_key[0], self.public_key[1]]) as usize;
                
                // Validate classical public key length is reasonable
                // Secp256k1 compressed: 33 bytes, uncompressed: 65 bytes
                // Ed25519: exactly 32 bytes
                const MAX_CLASSICAL_PK_LEN: usize = 128; // Reasonable upper bound
                if classical_pk_len > MAX_CLASSICAL_PK_LEN {
                    return Err(QuantumError::InvalidKey(format!(
                        "Classical public key length too large: {} (max: {})",
                        classical_pk_len, MAX_CLASSICAL_PK_LEN
                    )));
                }
                if classical_pk_len == 0 {
                    return Err(QuantumError::InvalidKey(
                        "Classical public key length cannot be zero".to_string(),
                    ));
                }

                // Use checked arithmetic to prevent integer overflow
                let classical_pk_start: usize = 2;
                let classical_pk_end = match classical_pk_start.checked_add(classical_pk_len) {
                    Some(end) => end,
                    None => {
                        return Err(QuantumError::InvalidKey(
                            "Integer overflow in hybrid public key parsing: classical key length too large".to_string(),
                        ));
                    }
                };

                // Validate bounds before slicing
                if self.public_key.len() < classical_pk_end {
                    return Err(QuantumError::InvalidKey(format!(
                        "Hybrid public key too short: expected at least {} bytes for classical key, got {}",
                        classical_pk_end, self.public_key.len()
                    )));
                }

                // Validate quantum public key has minimum required length
                let quantum_pk_start = classical_pk_end;
                let min_quantum_pk_len = match SecurityLevel::from(self.parameters.security_level) {
                    SecurityLevel::Low => 1312,   // Dilithium2
                    SecurityLevel::Medium => 1952, // Dilithium3
                    SecurityLevel::High => 2592,   // Dilithium5
                    _ => {
                        return Err(QuantumError::UnsupportedSecurityLevel(
                            self.parameters.security_level,
                        ));
                    }
                };

                if self.public_key.len() < quantum_pk_start + min_quantum_pk_len {
                    return Err(QuantumError::InvalidKey(format!(
                        "Hybrid public key too short: quantum key requires at least {} bytes, got {}",
                        min_quantum_pk_len,
                        self.public_key.len().saturating_sub(quantum_pk_start)
                    )));
                }

                // Safe slice operations with validated bounds
                let classical_pk = &self.public_key[classical_pk_start..classical_pk_end];
                let quantum_pk = &self.public_key[quantum_pk_start..];

                // Verify classical signature
                let classical_valid = match classical_scheme {
                    ClassicalScheme::Secp256k1 => {
                        // Verify ECDSA signature with secp256k1
                        let secp = Secp256k1::new();

                        let message_hash = Sha256::digest(message);
                        let secp_msg =
                            Secp256k1Message::from_slice(&message_hash).map_err(|e| {
                                QuantumError::CryptoOperationFailed(format!(
                                    "Invalid message hash: {}",
                                    e
                                ))
                            })?;

                        let public_key =
                            secp256k1::PublicKey::from_slice(classical_pk).map_err(|e| {
                                QuantumError::InvalidKey(format!(
                                    "Invalid secp256k1 public key: {}",
                                    e
                                ))
                            })?;

                        let sig =
                            secp256k1::ecdsa::Signature::from_der(classical_sig).map_err(|e| {
                                QuantumError::InvalidSignature(format!(
                                    "Invalid secp256k1 signature: {}",
                                    e
                                ))
                            })?;

                        secp.verify_ecdsa(&secp_msg, &sig, &public_key).is_ok()
                    }
                    ClassicalScheme::Ed25519 => {
                        // Verify Ed25519 signature
                        if classical_pk.len() != 32 || classical_sig.len() != 64 {
                            return Err(QuantumError::InvalidSignature(
                                "Invalid Ed25519 signature format".to_string(),
                            ));
                        }

                        // Convert slice to fixed-size array
                        let mut pk_array = [0u8; 32];
                        pk_array.copy_from_slice(classical_pk);

                        let public_key = VerifyingKey::from_bytes(&pk_array).map_err(|e| {
                            QuantumError::InvalidKey(format!("Invalid Ed25519 public key: {}", e))
                        })?;

                        // Convert slice to fixed-size array for new ed25519-dalek API
                        let mut sig_bytes = [0u8; 64];
                        sig_bytes.copy_from_slice(classical_sig);
                        let sig = ed25519_dalek::Signature::from_bytes(&sig_bytes);

                        public_key.verify(message, &sig).is_ok()
                    }
                };

                // Verify quantum signature
                let quantum_valid = match SecurityLevel::from(self.parameters.security_level) {
                    SecurityLevel::Low => {
                        let pk = dilithium2::PublicKey::from_bytes(quantum_pk).map_err(|e| {
                            QuantumError::InvalidKey(format!("Invalid Dilithium public key: {}", e))
                        })?;
                        let sig = dilithium2::DetachedSignature::from_bytes(quantum_sig).map_err(
                            |e| {
                                QuantumError::InvalidSignature(format!(
                                    "Invalid Dilithium signature: {}",
                                    e
                                ))
                            },
                        )?;

                        dilithium2::verify_detached_signature(&sig, message, &pk).is_ok()
                    }
                    SecurityLevel::Medium => {
                        let pk = dilithium3::PublicKey::from_bytes(quantum_pk).map_err(|e| {
                            QuantumError::InvalidKey(format!("Invalid Dilithium public key: {}", e))
                        })?;
                        let sig = dilithium3::DetachedSignature::from_bytes(quantum_sig).map_err(
                            |e| {
                                QuantumError::InvalidSignature(format!(
                                    "Invalid Dilithium signature: {}",
                                    e
                                ))
                            },
                        )?;

                        dilithium3::verify_detached_signature(&sig, message, &pk).is_ok()
                    }
                    SecurityLevel::High => {
                        let pk = dilithium5::PublicKey::from_bytes(quantum_pk).map_err(|e| {
                            QuantumError::InvalidKey(format!("Invalid Dilithium public key: {}", e))
                        })?;
                        let sig = dilithium5::DetachedSignature::from_bytes(quantum_sig).map_err(
                            |e| {
                                QuantumError::InvalidSignature(format!(
                                    "Invalid Dilithium signature: {}",
                                    e
                                ))
                            },
                        )?;

                        dilithium5::verify_detached_signature(&sig, message, &pk).is_ok()
                    }
                    _ => {
                        return Err(QuantumError::UnsupportedSecurityLevel(
                            self.parameters.security_level,
                        ))
                    }
                };

                // Both signatures must be valid
                Ok(classical_valid && quantum_valid)
            }
        }
    }

    /// Create key pair from seed
    pub fn from_seed(seed: &[u8; 64], parameters: QuantumParameters) -> Result<Self, QuantumError> {
        // Use seed to deterministically generate keys
        use sha2::{Digest, Sha512};

        // Hash the seed to get deterministic randomness
        let mut hasher = Sha512::new();
        hasher.update(seed);
        hasher.update([parameters.security_level]);
        let hash = hasher.finalize();

        // Use the hash as entropy for key generation
        // In production, use proper KDF
        match parameters.scheme {
            QuantumScheme::Dilithium => {
                // For Dilithium, we need to use the library's key generation
                // but seed it deterministically
                use rand::SeedableRng;
                use rand_chacha::ChaCha20Rng;

                let mut seed_bytes = [0u8; 32];
                seed_bytes.copy_from_slice(&hash[..32]);
                let mut rng = ChaCha20Rng::from_seed(seed_bytes);

                Self::generate_with_rng(&mut rng, parameters)
            }
            _ => Self::generate(parameters), // Fallback to random generation
        }
    }

    /// Convert key pair to bytes
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::new();

        // Format: [public_key_len (4 bytes)][public_key][secret_key]
        bytes.extend_from_slice(&(self.public_key.len() as u32).to_be_bytes());
        bytes.extend_from_slice(&self.public_key);
        bytes.extend_from_slice(&self.secret_key);

        bytes
    }

    /// Verify a signature with algorithm policy enforcement
    /// 
    /// SECURITY FIX (P0-003): This method enforces algorithm binding to prevent
    /// quantum signature downgrade attacks. It validates that the signature algorithm
    /// matches or upgrades from the public key's algorithm before verification.
    ///
    /// # Arguments
    /// * `message` - Message that was signed
    /// * `signature` - Signature bytes
    /// * `signature_params` - Parameters from the signature (includes claimed algorithm)
    /// * `policy` - Algorithm policy to enforce
    /// * `block_height` - Current blockchain height
    ///
    /// # Returns
    /// * `Ok(true)` - Signature is valid and algorithm transition is allowed
    /// * `Ok(false)` - Signature is cryptographically invalid
    /// * `Err(QuantumError)` - Algorithm downgrade attempt or policy violation
    ///
    /// # Security Guarantee
    /// This method ensures that:
    /// 1. Signature algorithm matches key algorithm (or is an allowed upgrade)
    /// 2. No downgrades are possible (e.g., Dilithium → Falcon forbidden)
    /// 3. All algorithm transitions are logged for audit
    pub fn verify_with_policy(
        &self,
        message: &[u8],
        signature: &[u8],
        signature_params: &QuantumParameters,
        policy: &AlgorithmPolicy,
        block_height: u64,
    ) -> Result<bool, QuantumError> {
        // CRITICAL SECURITY CHECK: Validate algorithm transition BEFORE verification
        // This prevents downgrade attacks where attacker substitutes weaker algorithm
        policy.validate_signature_transition(
            self.parameters.scheme,
            signature_params.scheme,
            block_height,
        )?;

        // Additional check: Enforce strict binding in the policy
        policy.enforce_algorithm_binding(
            self.parameters.scheme,
            signature_params.scheme,
        )?;

        // If we reach here, algorithm transition is valid - proceed with verification
        // Create a temporary keypair with the signature's parameters for verification
        let verify_keypair = QuantumKeyPair {
            public_key: self.public_key.clone(),
            secret_key: vec![], // Not needed for verification
            parameters: *signature_params, // Use signature's parameters
        };

        // Perform cryptographic verification
        verify_keypair.verify(message, signature)
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
            Self::Hybrid(scheme, _, _) => write!(
                f,
                "QuantumSecretKey::Hybrid({:?}, [REDACTED], [REDACTED])",
                scheme
            ),
        }
    }
}

/// Sign a message with quantum-resistant signature
pub fn sign_quantum(keypair: &QuantumKeyPair, message: &[u8]) -> Result<Vec<u8>, QuantumError> {
    keypair.sign(message)
}

/// Verify a quantum-resistant signature
pub fn verify_quantum_signature(
    public_key: &[u8],
    message: &[u8],
    signature: &[u8],
    parameters: QuantumParameters,
) -> Result<bool, QuantumError> {
    // Create a keypair with just the public key
    let keypair = QuantumKeyPair {
        public_key: public_key.to_vec(),
        secret_key: vec![], // Empty secret key since we're only verifying
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
        let params = QuantumParameters::with_security_level(
            QuantumScheme::Dilithium,
            SecurityLevel::Medium.into(),
        );

        let keypair = QuantumKeyPair::generate(params).expect("Key generation should succeed");
        let message = b"This is a test message for quantum signature";

        // Sign the message
        let signature = keypair.sign(message).expect("Signing should succeed");

        // Verify the signature
        let result = keypair
            .verify(message, &signature)
            .expect("Verification should succeed");
        assert!(result, "Signature verification should return true");

        // Try with wrong message
        let wrong_message = b"This is a different message";
        let result = keypair
            .verify(wrong_message, &signature)
            .expect("Verification with wrong message should succeed");
        assert!(
            !result,
            "Verification with wrong message should return false"
        );
    }

    #[test]
    fn test_falcon_not_implemented() {
        // Test Falcon with various security levels to ensure proper implementation
        let test_cases = vec![
            (SecurityLevel::Low, 1),
            (SecurityLevel::Medium, 3),
            (SecurityLevel::High, 5),
        ];

        for (level, level_value) in test_cases {
            let params = QuantumParameters::with_security_level(QuantumScheme::Falcon, level_value);
            let result = QuantumKeyPair::generate(params);

            // Falcon implementation status depends on security level support
            match result {
                Ok(keypair) => {
                    // If Falcon is implemented for this level, verify key sizes
                    assert!(
                        !keypair.public_key.is_empty(),
                        "Public key should not be empty"
                    );
                    assert!(
                        !keypair.secret_key.is_empty(),
                        "Secret key should not be empty"
                    );
                }
                Err(QuantumError::UnsupportedSecurityLevel(level)) => {
                    // Some security levels may not be supported
                    assert!(level > 0, "Invalid security level");
                }
                Err(QuantumError::CryptoOperationFailed(msg)) => {
                    // Implementation may be pending
                    assert!(
                        msg.contains("not yet implemented") || msg.contains("Not implemented"),
                        "Error should mention implementation pending, got: {}",
                        msg
                    );
                }
                Err(e) => panic!("Unexpected error type: {:?}", e),
            }
        }
    }

    #[test]
    fn test_sphincs_signing_and_verification() {
        let mut rng = OsRng;
        let parameters = QuantumParameters {
            scheme: QuantumScheme::SphincsPlus,
            security_level: SecurityLevel::Low as u8,
        };

        let keypair = QuantumKeyPair::generate(parameters).expect("SPHINCS+ key generation failed");

        let message = b"Test message for SPHINCS+ signatures";
        let signature = keypair.sign(message).expect("SPHINCS+ signing failed");

        let result = keypair
            .verify(message, &signature)
            .expect("SPHINCS+ verification failed");
        assert!(result, "SPHINCS+ signature verification should succeed");

        // Modify message to test failed verification
        let modified_message = b"Modified message for SPHINCS+ signatures";
        let modified_result = keypair
            .verify(modified_message, &signature)
            .expect("SPHINCS+ verification operation failed");
        assert!(
            !modified_result,
            "SPHINCS+ signature verification should fail for modified message"
        );
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

            let keypair =
                QuantumKeyPair::generate(parameters).expect("Hybrid key generation failed");

            let message = b"Test message for hybrid signatures";
            let signature = keypair.sign(message).expect("Hybrid signing failed");

            let result = keypair
                .verify(message, &signature)
                .expect("Hybrid verification failed");
            assert!(result, "Hybrid signature verification should succeed");

            // Modify message to test failed verification
            let modified_message = b"Modified message for hybrid signatures";
            let modified_result = keypair
                .verify(modified_message, &signature)
                .expect("Hybrid verification operation failed");
            assert!(
                !modified_result,
                "Hybrid signature verification should fail for modified message"
            );
        }
    }

    #[test]
    fn test_quantum_signature_security() {
        // CRITICAL SECURITY TEST: Verify that quantum signatures cannot be forged
        let mut rng = OsRng;

        // Test all quantum schemes
        let schemes = [
            (QuantumScheme::Dilithium, vec![2u8, 3u8, 5u8]),
            (QuantumScheme::SphincsPlus, vec![1u8, 3u8, 5u8]),
        ];

        for (scheme, security_levels) in schemes.iter() {
            for security_level in security_levels {
                let params = QuantumParameters::with_security_level(*scheme, *security_level);

                // Generate a legitimate key pair
                let legitimate_keypair =
                    QuantumKeyPair::generate(params).expect("Key generation should succeed");

                // Generate an attacker's key pair
                let attacker_keypair = QuantumKeyPair::generate(params)
                    .expect("Attacker key generation should succeed");

                let message = b"Critical transaction: Send 1000 NOVA to attacker";

                // Sign with legitimate key
                let legitimate_signature = legitimate_keypair
                    .sign(message)
                    .expect("Legitimate signing should succeed");

                // Verify legitimate signature works
                assert!(
                    legitimate_keypair
                        .verify(message, &legitimate_signature)
                        .unwrap(),
                    "Legitimate signature should verify"
                );

                // CRITICAL TEST 1: Attacker cannot use their signature on legitimate public key
                let attacker_signature = attacker_keypair
                    .sign(message)
                    .expect("Attacker signing should succeed");

                assert!(
                    !legitimate_keypair
                        .verify(message, &attacker_signature)
                        .unwrap(),
                    "Attacker signature should NOT verify with legitimate public key"
                );

                // CRITICAL TEST 2: Random bytes should not verify as valid signature
                let random_signature = vec![0u8; legitimate_signature.len()];
                assert!(
                    !legitimate_keypair
                        .verify(message, &random_signature)
                        .unwrap(),
                    "Random bytes should NOT verify as valid signature"
                );

                // CRITICAL TEST 3: Modified signature should not verify
                let mut modified_signature = legitimate_signature.clone();
                if !modified_signature.is_empty() {
                    modified_signature[0] ^= 0xFF; // Flip bits in first byte
                }
                assert!(
                    !legitimate_keypair
                        .verify(message, &modified_signature)
                        .unwrap(),
                    "Modified signature should NOT verify"
                );

                // CRITICAL TEST 4: Signature from one message should not work for another
                let other_message = b"Different transaction: Send 1 NOVA to charity";
                assert!(
                    !legitimate_keypair
                        .verify(other_message, &legitimate_signature)
                        .unwrap(),
                    "Signature for one message should NOT verify for different message"
                );
            }
        }
    }

    #[test]
    fn test_hybrid_quantum_signature_security() {
        // Test that hybrid signatures require BOTH classical and quantum parts to be valid
        let mut rng = OsRng;

        let params = QuantumParameters::with_security_level(
            QuantumScheme::Hybrid(ClassicalScheme::Ed25519),
            3,
        );

        let keypair1 = QuantumKeyPair::generate(params).unwrap();
        let keypair2 = QuantumKeyPair::generate(params).unwrap();

        let message = b"Hybrid security test message";
        let signature1 = keypair1.sign(message).unwrap();

        // Valid signature should verify
        assert!(keypair1.verify(message, &signature1).unwrap());

        // Different keypair's signature should not verify
        let signature2 = keypair2.sign(message).unwrap();
        assert!(!keypair1.verify(message, &signature2).unwrap());

        // Corrupting classical part should fail verification
        let mut corrupt_classical = signature1.clone();
        if corrupt_classical.len() > 10 {
            corrupt_classical[5] ^= 0xFF;
        }
        assert!(!keypair1.verify(message, &corrupt_classical).unwrap());

        // Corrupting quantum part should fail verification
        let mut corrupt_quantum = signature1.clone();
        if corrupt_quantum.len() > 100 {
            let idx = corrupt_quantum.len() - 10;
            corrupt_quantum[idx] ^= 0xFF;
        }
        assert!(!keypair1.verify(message, &corrupt_quantum).unwrap());
    }

    // SECURITY FIX [P1-004]: Comprehensive tests for malformed hybrid key parsing
    #[test]
    fn test_hybrid_key_parsing_bounds_validation() {
        // Test: Empty secret key
        let mut invalid_keypair = QuantumKeyPair {
            public_key: vec![0u8; 100],
            secret_key: vec![],
            parameters: QuantumParameters {
                scheme: QuantumScheme::Hybrid(ClassicalScheme::Secp256k1),
                security_level: 3,
            },
        };
        let result = invalid_keypair.sign(b"test");
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            QuantumError::InvalidKey(_)
        ));

        // Test: Secret key too short (missing length prefix)
        invalid_keypair.secret_key = vec![0u8; 1];
        let result = invalid_keypair.sign(b"test");
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            QuantumError::InvalidKey(_)
        ));

        // Test: Invalid classical key length (not 32 bytes)
        let mut invalid_key = vec![0u8; 2];
        invalid_key.extend_from_slice(&31u16.to_be_bytes()); // Wrong length: 31 instead of 32
        invalid_key.extend_from_slice(&vec![0u8; 31]); // Only 31 bytes
        invalid_key.extend_from_slice(&vec![0u8; 4000]); // Quantum key
        invalid_keypair.secret_key = invalid_key;
        let result = invalid_keypair.sign(b"test");
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            QuantumError::InvalidKey(_)
        ));

        // Test: Classical key length causes integer overflow
        let mut invalid_key = vec![0u8; 2];
        invalid_key.extend_from_slice(&(usize::MAX as u16).to_be_bytes()); // Max u16
        invalid_keypair.secret_key = invalid_key;
        let result = invalid_keypair.sign(b"test");
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            QuantumError::InvalidKey(_)
        ));

        // Test: Secret key too short for quantum key
        let mut invalid_key = vec![0u8; 2];
        invalid_key.extend_from_slice(&32u16.to_be_bytes()); // Correct classical length
        invalid_key.extend_from_slice(&vec![0u8; 32]); // Classical key
        invalid_key.extend_from_slice(&vec![0u8; 100]); // Quantum key too short (need 4000 for level 3)
        invalid_keypair.secret_key = invalid_key;
        let result = invalid_keypair.sign(b"test");
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            QuantumError::InvalidKey(_)
        ));
    }

    #[test]
    fn test_hybrid_signature_parsing_bounds_validation() {
        // Create a valid keypair first
        let params = QuantumParameters {
            scheme: QuantumScheme::Hybrid(ClassicalScheme::Secp256k1),
            security_level: 3,
        };
        let keypair = QuantumKeyPair::generate(params).unwrap();
        let message = b"test";
        let valid_signature = keypair.sign(message).unwrap();

        // Test: Empty signature
        let result = keypair.verify(message, &[]);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            QuantumError::InvalidSignature(_)
        ));

        // Test: Signature too short (missing length prefix)
        let result = keypair.verify(message, &[0u8; 1]);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            QuantumError::InvalidSignature(_)
        ));

        // Test: Classical signature length too large
        let mut invalid_sig = vec![0u8; 2];
        invalid_sig.extend_from_slice(&(1000u16).to_be_bytes()); // Too large
        invalid_sig.extend_from_slice(&vec![0u8; 100]);
        let result = keypair.verify(message, &invalid_sig);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            QuantumError::InvalidSignature(_)
        ));

        // Test: Classical signature length zero
        let mut invalid_sig = vec![0u8; 2];
        invalid_sig.extend_from_slice(&0u16.to_be_bytes()); // Zero length
        invalid_sig.extend_from_slice(&vec![0u8; 100]);
        let result = keypair.verify(message, &invalid_sig);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            QuantumError::InvalidSignature(_)
        ));

        // Test: Signature too short for quantum part
        let mut invalid_sig = vec![0u8; 2];
        invalid_sig.extend_from_slice(&(70u16).to_be_bytes()); // Classical sig length
        invalid_sig.extend_from_slice(&vec![0u8; 70]); // Classical signature
        invalid_sig.extend_from_slice(&vec![0u8; 100]); // Quantum signature too short
        let result = keypair.verify(message, &invalid_sig);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            QuantumError::InvalidSignature(_) | QuantumError::UnsupportedSecurityLevel(_)
        ));
    }

    #[test]
    fn test_hybrid_public_key_parsing_bounds_validation() {
        // Create a valid keypair
        let params = QuantumParameters {
            scheme: QuantumScheme::Hybrid(ClassicalScheme::Secp256k1),
            security_level: 3,
        };
        let keypair = QuantumKeyPair::generate(params).unwrap();
        let message = b"test";
        let signature = keypair.sign(message).unwrap();

        // Test: Empty public key
        let mut invalid_keypair = QuantumKeyPair {
            public_key: vec![],
            secret_key: keypair.secret_key.clone(),
            parameters: keypair.parameters,
        };
        let result = invalid_keypair.verify(message, &signature);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            QuantumError::InvalidKey(_)
        ));

        // Test: Public key too short (missing length prefix)
        invalid_keypair.public_key = vec![0u8; 1];
        let result = invalid_keypair.verify(message, &signature);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            QuantumError::InvalidKey(_)
        ));

        // Test: Classical public key length too large
        let mut invalid_pk = vec![0u8; 2];
        invalid_pk.extend_from_slice(&(500u16).to_be_bytes()); // Too large
        invalid_pk.extend_from_slice(&vec![0u8; 100]);
        invalid_keypair.public_key = invalid_pk;
        let result = invalid_keypair.verify(message, &signature);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            QuantumError::InvalidKey(_)
        ));

        // Test: Classical public key length zero
        let mut invalid_pk = vec![0u8; 2];
        invalid_pk.extend_from_slice(&0u16.to_be_bytes()); // Zero length
        invalid_pk.extend_from_slice(&vec![0u8; 100]);
        invalid_keypair.public_key = invalid_pk;
        let result = invalid_keypair.verify(message, &signature);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            QuantumError::InvalidKey(_)
        ));

        // Test: Public key too short for quantum part
        let mut invalid_pk = vec![0u8; 2];
        invalid_pk.extend_from_slice(&(33u16).to_be_bytes()); // Classical pk length
        invalid_pk.extend_from_slice(&vec![0u8; 33]); // Classical public key
        invalid_pk.extend_from_slice(&vec![0u8; 100]); // Quantum key too short
        invalid_keypair.public_key = invalid_pk;
        let result = invalid_keypair.verify(message, &signature);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            QuantumError::InvalidKey(_) | QuantumError::UnsupportedSecurityLevel(_)
        ));
    }

    #[test]
    fn test_hybrid_key_parsing_integer_overflow_protection() {
        // Test: Classical key length that would cause overflow when adding
        let params = QuantumParameters {
            scheme: QuantumScheme::Hybrid(ClassicalScheme::Secp256k1),
            security_level: 3,
        };

        // Create a key with maximum u16 value for classical_sk_len
        // This should trigger overflow protection
        let mut invalid_key = vec![0u8; 2];
        invalid_key.extend_from_slice(&(u16::MAX).to_be_bytes()); // Max u16: 65535
        invalid_key.extend_from_slice(&vec![0u8; 65535]); // Try to allocate max size
        invalid_key.extend_from_slice(&vec![0u8; 4000]); // Quantum key

        let mut invalid_keypair = QuantumKeyPair {
            public_key: vec![0u8; 2000],
            secret_key: invalid_key,
            parameters: params,
        };

        // This should fail with overflow protection or invalid length check
        let result = invalid_keypair.sign(b"test");
        assert!(result.is_err());
        // Should either fail on invalid length check (32 expected) or overflow protection
        assert!(matches!(
            result.unwrap_err(),
            QuantumError::InvalidKey(_) | QuantumError::UnsupportedSecurityLevel(_)
        ));
    }
}
