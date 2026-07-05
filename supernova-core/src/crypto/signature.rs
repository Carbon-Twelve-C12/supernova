use crate::crypto::quantum::{ClassicalScheme, QuantumError, QuantumParameters, QuantumScheme};
use ed25519_dalek::{Signature as Ed25519Signature, VerifyingKey};
use hex;
use pqcrypto_sphincsplus::{
    sphincsshake128fsimple, sphincsshake192fsimple, sphincsshake256fsimple,
};
use pqcrypto_traits::sign::{DetachedSignature, PublicKey as PqPublicKey};
use rand;
use secp256k1::ecdsa::Signature as Secp256k1Signature;
use secp256k1::{Message, PublicKey, Secp256k1, SecretKey};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use thiserror::Error;

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
    fn verify(
        &self,
        public_key: &[u8],
        message: &[u8],
        signature: &[u8],
    ) -> Result<bool, SignatureError>;

    /// Verify multiple signatures in a batch
    fn batch_verify(
        &self,
        keys: &[&[u8]],
        messages: &[&[u8]],
        signatures: &[&[u8]],
    ) -> Result<bool, SignatureError> {
        // Default implementation verifies each signature individually
        if keys.len() != messages.len() || keys.len() != signatures.len() {
            return Err(SignatureError::BatchVerificationFailed(
                "Mismatched number of keys, messages, and signatures".to_string(),
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
    fn verify(
        &self,
        public_key: &[u8],
        message: &[u8],
        signature: &[u8],
    ) -> Result<bool, SignatureError> {
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
        signatures: &[&[u8]],
    ) -> Result<bool, SignatureError> {
        // Check that arrays have the same length
        if keys.len() != messages.len() || keys.len() != signatures.len() {
            return Err(SignatureError::InvalidParameters(
                "Batch verification requires equal number of keys, messages, and signatures"
                    .to_string(),
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
                Ok(_) => {} // continue to next signature
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
    fn verify(
        &self,
        public_key: &[u8],
        message: &[u8],
        signature: &[u8],
    ) -> Result<bool, SignatureError> {
        // Validate public key length
        if public_key.len() != 32 {
            return Err(SignatureError::InvalidKey(format!(
                "Invalid Ed25519 public key length: expected 32, got {}",
                public_key.len()
            )));
        }

        // Validate signature length
        if signature.len() != 64 {
            return Err(SignatureError::InvalidSignature(format!(
                "Invalid Ed25519 signature length: expected 64, got {}",
                signature.len()
            )));
        }

        // Convert public key bytes to fixed-size array
        let pk_bytes: [u8; 32] = public_key
            .try_into()
            .map_err(|_| SignatureError::InvalidKey("Failed to convert public key".to_string()))?;

        // Convert signature bytes to fixed-size array
        let sig_bytes: [u8; 64] = signature
            .try_into()
            .map_err(|_| SignatureError::InvalidSignature("Failed to convert signature".to_string()))?;

        // Create verifying key from bytes
        let verifying_key = VerifyingKey::from_bytes(&pk_bytes)
            .map_err(|e| SignatureError::InvalidKey(format!("Invalid Ed25519 key: {}", e)))?;

        // Create signature from bytes
        let sig = Ed25519Signature::from_bytes(&sig_bytes);

        // Verify using strict verification (rejects malleable signatures)
        match verifying_key.verify_strict(message, &sig) {
            Ok(_) => Ok(true),
            Err(_) => Ok(false), // Invalid signature returns false, not error
        }
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
    fn verify(
        &self,
        public_key: &[u8],
        message: &[u8],
        signature: &[u8],
    ) -> Result<bool, SignatureError> {
        // Use the existing quantum verification through the QuantumParameters conversion
        let params = QuantumParameters {
            scheme: QuantumScheme::Dilithium,
            security_level: self.security_level,
        };

        // Use the existing verify_quantum_signature function
        crate::crypto::quantum::verify_quantum_signature(public_key, message, signature, params)
            .map_err(SignatureError::QuantumError)
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
    fn verify(
        &self,
        public_key: &[u8],
        message: &[u8],
        signature: &[u8],
    ) -> Result<bool, SignatureError> {
        use crate::crypto::falcon_real::{falcon_verify, FalconError, FalconSecurityLevel};

        // Convert numeric security level to FalconSecurityLevel using the same
        // lenient mapping as `crypto::quantum::falcon_security_level_for`
        // (via `SecurityLevel::from(u8)`). Falcon only defines NIST level 1
        // (Falcon-512) and level 5 (Falcon-1024), so `Low` maps to Falcon-512
        // and every other tier maps to Falcon-1024. A strict `from_level` would
        // reject the shared default `security_level` of 2 (=> Low => Falcon-512),
        // used by `KeyPair::sign_quantum`/`SignatureVerifier`, and break the
        // Falcon sign/verify round-trip (fail-closed, no forgery).
        use crate::validation::SecurityLevel;
        let nist_level = match SecurityLevel::from(self.security_level) {
            SecurityLevel::Low => 1,
            _ => 5,
        };
        let security_level = FalconSecurityLevel::from_level(nist_level).map_err(|e| {
            SignatureError::CryptoOperationFailed(format!("Invalid security level: {}", e))
        })?;

        // Verify the signature using the falcon_verify function
        match falcon_verify(public_key, message, signature, security_level) {
            Ok(valid) => Ok(valid),
            Err(e) => match e {
                FalconError::InvalidKey(msg) => Err(SignatureError::InvalidKey(msg)),
                FalconError::InvalidSignature(msg) => Err(SignatureError::InvalidSignature(msg)),
                FalconError::InvalidPublicKey => Err(SignatureError::InvalidKey(
                    "Invalid Falcon public key".to_string(),
                )),
                FalconError::InvalidSecretKey => Err(SignatureError::InvalidKey(
                    "Invalid Falcon secret key".to_string(),
                )),
                FalconError::UnsupportedSecurityLevel(level) => {
                    Err(SignatureError::CryptoOperationFailed(format!(
                        "Unsupported Falcon security level: {}",
                        level
                    )))
                }
                err => Err(SignatureError::CryptoOperationFailed(format!(
                    "Falcon error: {}",
                    err
                ))),
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
    fn verify(
        &self,
        public_key: &[u8],
        message: &[u8],
        signature: &[u8],
    ) -> Result<bool, SignatureError> {
        // Verify using the appropriate SPHINCS+ variant based on security level.
        //
        // The numeric `security_level` MUST be mapped through
        // `SecurityLevel::from(u8)` — the same conversion used by
        // `QuantumKeyPair::sign`/`verify` and `generate_sphincs` in
        // `crypto::quantum` — so that signing and verification agree on the
        // variant. A raw numeric match (e.g. 2 => 192f) would diverge from the
        // canonical mapping (2 => Low => 128f) and break the round-trip.
        use crate::validation::SecurityLevel;
        match SecurityLevel::from(self.security_level) {
            SecurityLevel::Low => {
                // Low/128-bit security: SPHINCS+-SHAKE-128f-simple
                let pk = sphincsshake128fsimple::PublicKey::from_bytes(public_key).map_err(
                    |_| SignatureError::InvalidKey("Invalid SPHINCS+ public key".to_string()),
                )?;
                let sig =
                    sphincsshake128fsimple::DetachedSignature::from_bytes(signature).map_err(
                        |_| {
                            SignatureError::InvalidSignature(
                                "Invalid SPHINCS+ signature".to_string(),
                            )
                        },
                    )?;
                Ok(sphincsshake128fsimple::verify_detached_signature(&sig, message, &pk).is_ok())
            }
            SecurityLevel::Medium => {
                // Medium/192-bit security: SPHINCS+-SHAKE-192f-simple
                let pk = sphincsshake192fsimple::PublicKey::from_bytes(public_key).map_err(
                    |_| SignatureError::InvalidKey("Invalid SPHINCS+ public key".to_string()),
                )?;
                let sig =
                    sphincsshake192fsimple::DetachedSignature::from_bytes(signature).map_err(
                        |_| {
                            SignatureError::InvalidSignature(
                                "Invalid SPHINCS+ signature".to_string(),
                            )
                        },
                    )?;
                Ok(sphincsshake192fsimple::verify_detached_signature(&sig, message, &pk).is_ok())
            }
            _ => {
                // High/256-bit security (default): SPHINCS+-SHAKE-256f-simple
                let pk = sphincsshake256fsimple::PublicKey::from_bytes(public_key).map_err(
                    |_| SignatureError::InvalidKey("Invalid SPHINCS+ public key".to_string()),
                )?;
                let sig =
                    sphincsshake256fsimple::DetachedSignature::from_bytes(signature).map_err(
                        |_| {
                            SignatureError::InvalidSignature(
                                "Invalid SPHINCS+ signature".to_string(),
                            )
                        },
                    )?;
                Ok(sphincsshake256fsimple::verify_detached_signature(&sig, message, &pk).is_ok())
            }
        }
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
        quantum_scheme: Box<dyn SignatureScheme>,
    ) -> Self {
        Self {
            classical_scheme,
            quantum_scheme,
        }
    }
}

impl SignatureScheme for HybridScheme {
    fn verify(
        &self,
        public_key: &[u8],
        message: &[u8],
        signature: &[u8],
    ) -> Result<bool, SignatureError> {
        // Hybrid signature format: [classical_sig_len: 2 bytes BE][classical_sig][quantum_sig]
        // Hybrid public key format: [classical_pk_len: 2 bytes BE][classical_pk][quantum_pk]

        // Parse signature: extract classical and quantum parts
        if signature.len() < 2 {
            return Err(SignatureError::InvalidSignature(
                "Hybrid signature too short: missing length prefix".to_string(),
            ));
        }
        let classical_sig_len = u16::from_be_bytes([signature[0], signature[1]]) as usize;
        if signature.len() < 2 + classical_sig_len {
            return Err(SignatureError::InvalidSignature(format!(
                "Hybrid signature too short: expected {} bytes for classical sig, got {}",
                classical_sig_len,
                signature.len() - 2
            )));
        }
        let classical_sig = &signature[2..2 + classical_sig_len];
        let quantum_sig = &signature[2 + classical_sig_len..];

        // Parse public key: extract classical and quantum parts
        if public_key.len() < 2 {
            return Err(SignatureError::InvalidKey(
                "Hybrid public key too short: missing length prefix".to_string(),
            ));
        }
        let classical_pk_len = u16::from_be_bytes([public_key[0], public_key[1]]) as usize;
        if public_key.len() < 2 + classical_pk_len {
            return Err(SignatureError::InvalidKey(format!(
                "Hybrid public key too short: expected {} bytes for classical pk, got {}",
                classical_pk_len,
                public_key.len() - 2
            )));
        }
        let classical_pk = &public_key[2..2 + classical_pk_len];
        let quantum_pk = &public_key[2 + classical_pk_len..];

        // Verify both signatures - both must pass for hybrid to be valid
        let classical_valid = self
            .classical_scheme
            .verify(classical_pk, message, classical_sig)?;
        let quantum_valid = self
            .quantum_scheme
            .verify(quantum_pk, message, quantum_sig)?;

        // Both signatures must be valid
        Ok(classical_valid && quantum_valid)
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

impl Default for SignatureVerifier {
    fn default() -> Self {
        Self::new()
    }
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
        signature: &[u8],
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
            SignatureType::Classical(classical_scheme) => match classical_scheme {
                ClassicalScheme::Secp256k1 => {
                    let scheme = Secp256k1Scheme;
                    scheme.verify(public_key, message, signature)
                }
                ClassicalScheme::Ed25519 => {
                    let scheme = Ed25519Scheme;
                    scheme.verify(public_key, message, signature)
                }
            },
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
                    QuantumScheme::SphincsPlus => {
                        let scheme = SphincsScheme::new(self.security_level);
                        scheme.verify(public_key, message, signature)
                    }
                    QuantumScheme::Hybrid(classical_scheme) => {
                        // For hybrid schemes, we need to parse the signature and public key appropriately
                        // Simplify for now to just use the quantum part
                        match classical_scheme {
                            ClassicalScheme::Secp256k1 => {
                                let classical_scheme = Box::new(Secp256k1Scheme);
                                let quantum_scheme =
                                    Box::new(DilithiumScheme::new(self.security_level));
                                let scheme = HybridScheme::new(classical_scheme, quantum_scheme);
                                scheme.verify(public_key, message, signature)
                            }
                            ClassicalScheme::Ed25519 => {
                                let classical_scheme = Box::new(Ed25519Scheme);
                                let quantum_scheme =
                                    Box::new(DilithiumScheme::new(self.security_level));
                                let scheme = HybridScheme::new(classical_scheme, quantum_scheme);
                                scheme.verify(public_key, message, signature)
                            }
                        }
                    }
                }
            }
            SignatureType::Schnorr => Err(SignatureError::UnsupportedType(
                "Schnorr not implemented".to_string(),
            )),
        }
    }

    /// Verify a transaction's signature
    pub fn verify_transaction(
        &self,
        tx: &crate::types::transaction::Transaction,
    ) -> Result<bool, SignatureError> {
        // Get the signature data from the transaction
        let signature_data = match tx.signature_data() {
            Some(data) => data,
            None => return Err(SignatureError::MissingSignature),
        };

        // Verify the signature using the appropriate scheme
        match signature_data.scheme {
            crate::types::transaction::SignatureSchemeType::Legacy => self.verify(
                SignatureType::Secp256k1,
                &signature_data.public_key,
                &tx.hash(),
                &signature_data.data,
            ),
            crate::types::transaction::SignatureSchemeType::Ed25519 => self.verify(
                SignatureType::Ed25519,
                &signature_data.public_key,
                &tx.hash(),
                &signature_data.data,
            ),
            // Add more signature schemes as needed
            _ => Err(SignatureError::UnsupportedScheme(format!(
                "Signature scheme not supported for verification: {:?}",
                signature_data.scheme
            ))),
        }
    }

    /// Batch verify multiple transactions
    pub fn batch_verify_transactions(
        &self,
        txs: &[&crate::types::transaction::Transaction],
    ) -> Result<bool, SignatureError> {
        // Real verification: a batch is valid only if every transaction in it
        // verifies. (Previously this grouped the transactions but never checked
        // any signature, returning a hardcoded `Ok(true)`.)
        for tx in txs {
            if !self.verify_transaction(tx)? {
                return Ok(false);
            }
        }
        Ok(true)
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
    pub fn new(
        signature_type: SignatureType,
        signature_bytes: Vec<u8>,
        public_key_bytes: Vec<u8>,
    ) -> Self {
        Self {
            signature_type,
            signature_bytes,
            public_key_bytes,
        }
    }

    /// Verify a message with this signature.
    ///
    /// Delegates to `SignatureVerifier`, which contains the real, working
    /// implementations for every advertised signature type (including the
    /// PQC schemes). Previously this inherent method hardcoded
    /// "not implemented" errors for Ed25519/Schnorr/Sphincs/Dilithium/Falcon/
    /// Hybrid regardless of `SignatureVerifier` actually supporting them,
    /// which meant any caller reaching for the more obviously-named
    /// `Signature::verify()` API would silently fail for every PQC scheme.
    pub fn verify(&self, message: &[u8]) -> Result<bool, SignatureError> {
        SignatureVerifier::new().verify(
            self.signature_type,
            &self.public_key_bytes,
            message,
            &self.signature_bytes,
        )
    }
}

impl fmt::Debug for Signature {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Signature {{ type: {:?}, signature: {}, public_key: {} }}",
            self.signature_type,
            hex::encode(&self.signature_bytes),
            hex::encode(&self.public_key_bytes)
        )
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
    ///
    /// PQC variants (Sphincs/Dilithium/Falcon/Hybrid) delegate to the real,
    /// tested `QuantumKeyPair::sign` implementation in `crate::crypto::quantum`,
    /// mirroring the delegation pattern used by `Signature::verify()` /
    /// `SignatureVerifier`. Previously these branches hardcoded "not
    /// implemented" errors even though a working signing implementation for
    /// every one of these schemes already existed elsewhere in the crate.
    pub fn sign(&self, message: &[u8]) -> Result<Signature, SignatureError> {
        match self.signature_type {
            SignatureType::Secp256k1 => self.sign_secp256k1(message),
            SignatureType::Ed25519 => Err(SignatureError::UnsupportedType(
                "Ed25519 not implemented".to_string(),
            )),
            SignatureType::Schnorr => Err(SignatureError::UnsupportedType(
                "Schnorr not implemented".to_string(),
            )),
            SignatureType::Sphincs => self.sign_quantum(message, QuantumScheme::SphincsPlus),
            SignatureType::Dilithium => self.sign_quantum(message, QuantumScheme::Dilithium),
            SignatureType::Falcon => self.sign_quantum(message, QuantumScheme::Falcon),
            SignatureType::Hybrid => {
                self.sign_quantum(message, QuantumScheme::Hybrid(ClassicalScheme::Secp256k1))
            }
            SignatureType::Classical(classical_scheme) => match classical_scheme {
                ClassicalScheme::Secp256k1 => self.sign_secp256k1(message),
                ClassicalScheme::Ed25519 => Err(SignatureError::UnsupportedType(
                    "Ed25519 not implemented".to_string(),
                )),
            },
            SignatureType::Quantum(quantum_scheme) => self.sign_quantum(message, quantum_scheme),
        }
    }

    /// Sign with a post-quantum scheme by delegating to the real
    /// `QuantumKeyPair::sign` implementation (see doc comment on `sign()`).
    fn sign_quantum(
        &self,
        message: &[u8],
        scheme: QuantumScheme,
    ) -> Result<Signature, SignatureError> {
        let keypair = crate::crypto::quantum::QuantumKeyPair {
            public_key: self.public_key.clone(),
            secret_key: self.secret_key.clone(),
            parameters: QuantumParameters {
                scheme,
                security_level: 2, // Medium security level by default, matching SignatureVerifier::new()
            },
        };

        let signature_bytes = keypair.sign(message)?;

        Ok(Signature::new(
            self.signature_type,
            signature_bytes,
            self.public_key.clone(),
        ))
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

/// Convenience function for signature verification
pub fn verify_signature(
    sig_type: SignatureType,
    public_key: &[u8],
    message: &[u8],
    signature: &[u8],
) -> Result<bool, SignatureError> {
    let verifier = SignatureVerifier::new();
    verifier.verify(sig_type, public_key, message, signature)
}

#[cfg(test)]
mod tests {
    use super::*;
    use sha2::{Digest, Sha256};

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

    /// Regression test for the F6 finding: `KeyPair::sign()` and
    /// `Signature::verify()` used to hardcode "not implemented" errors for
    /// every PQC scheme even though real, working implementations existed
    /// elsewhere in the crate (`QuantumKeyPair::sign` / `SignatureVerifier`).
    /// This exercises the full round trip through the inherent `KeyPair`/
    /// `Signature` API (not just the `SignatureVerifier`/`QuantumKeyPair`
    /// APIs directly) for a representative PQC scheme.
    #[test]
    fn test_dilithium_sign_verify_via_keypair() {
        use crate::crypto::quantum::{QuantumKeyPair, QuantumParameters};

        let params = QuantumParameters {
            scheme: QuantumScheme::Dilithium,
            security_level: 2,
        };
        let quantum_keypair =
            QuantumKeyPair::generate(params).expect("Dilithium key generation should succeed");

        // Construct the (separate) `crypto::signature::KeyPair` abstraction
        // directly from the real Dilithium key material, since `KeyPair`
        // only exposes a `new_secp256k1()` constructor publicly.
        let key_pair = KeyPair {
            signature_type: SignatureType::Dilithium,
            secret_key: quantum_keypair.secret_key.clone(),
            public_key: quantum_keypair.public_key.clone(),
        };

        let message = b"Dilithium signing must not be a dead stub";

        // Previously this returned Err(UnsupportedType("Dilithium not implemented"))
        let signature = key_pair
            .sign(message)
            .expect("Dilithium signing should delegate to the real implementation");
        assert_eq!(signature.signature_type, SignatureType::Dilithium);

        // Previously this also returned Err(UnsupportedType(...)) regardless
        // of the signature's validity.
        let valid = signature
            .verify(message)
            .expect("Dilithium verification should delegate to the real implementation");
        assert!(valid, "a genuine Dilithium signature must verify as valid");

        // A tampered message must not verify.
        let tampered = signature.verify(b"different message").unwrap_or(false);
        assert!(!tampered, "signature must not verify against a different message");
    }

    /// Regression test for the R3-2 finding: `SphincsScheme::verify` used a
    /// raw numeric match (`1 => 128f, 2 => 192f, 3 | _ => 256f`) that
    /// disagreed with `SecurityLevel::from(u8)` (2 => Low => 128f) used by
    /// `QuantumKeyPair::sign`/`generate_sphincs` everywhere else. Because
    /// `KeyPair::sign_quantum` signs SPHINCS+ at the default `security_level = 2`
    /// (128f, 32-byte public key) while `SignatureVerifier::new()` routed
    /// verification through `SphincsScheme::new(2)` -> 192f (48-byte public
    /// key), `PublicKey::from_bytes` failed on length and the round trip was
    /// broken. This exercises the full sign/verify round trip through the
    /// inherent `KeyPair`/`Signature` API for SPHINCS+.
    #[test]
    fn test_sphincs_sign_verify_via_keypair() {
        use crate::crypto::quantum::{QuantumKeyPair, QuantumParameters};

        // Default level used by KeyPair::sign_quantum / SignatureVerifier::new().
        let params = QuantumParameters {
            scheme: QuantumScheme::SphincsPlus,
            security_level: 2,
        };
        let quantum_keypair =
            QuantumKeyPair::generate(params).expect("SPHINCS+ key generation should succeed");

        let key_pair = KeyPair {
            signature_type: SignatureType::Sphincs,
            secret_key: quantum_keypair.secret_key.clone(),
            public_key: quantum_keypair.public_key.clone(),
        };

        let message = b"SPHINCS+ sign/verify variant mapping must agree";

        let signature = key_pair
            .sign(message)
            .expect("SPHINCS+ signing should delegate to the real implementation");
        assert_eq!(signature.signature_type, SignatureType::Sphincs);

        // Before the fix this returned Err(InvalidKey("Invalid SPHINCS+ public
        // key")) because verify selected the 192f variant for a 128f key.
        let valid = signature
            .verify(message)
            .expect("SPHINCS+ verification must select the same variant as signing");
        assert!(valid, "a genuine SPHINCS+ signature must verify as valid");

        // A tampered message must not verify.
        let tampered = signature.verify(b"different message").unwrap_or(false);
        assert!(
            !tampered,
            "signature must not verify against a different message"
        );
    }

    /// Regression test for the R3-3 finding: `FalconScheme::verify` converted
    /// the numeric level via the strict
    /// `FalconSecurityLevel::from_level`, which only accepts 1 or 5 and errors
    /// on the shared default `security_level = 2`. But `KeyPair::sign_quantum`
    /// signs Falcon at level 2, which `falcon_security_level_for`
    /// (`SecurityLevel::from(2)` => Low) maps to Falcon-512. So a genuine Falcon
    /// signature produced through the `KeyPair` API could never be verified via
    /// `Signature::verify()` / `SignatureVerifier` (fail-closed,
    /// `CryptoOperationFailed("...UnsupportedSecurityLevel(2)")`). This exercises
    /// the full Falcon sign/verify round trip through the inherent API.
    #[test]
    fn test_falcon_sign_verify_via_keypair() {
        use crate::crypto::quantum::{QuantumKeyPair, QuantumParameters};

        // Default level used by KeyPair::sign_quantum / SignatureVerifier::new().
        let params = QuantumParameters {
            scheme: QuantumScheme::Falcon,
            security_level: 2,
        };
        let quantum_keypair =
            QuantumKeyPair::generate(params).expect("Falcon key generation should succeed");

        let key_pair = KeyPair {
            signature_type: SignatureType::Falcon,
            secret_key: quantum_keypair.secret_key.clone(),
            public_key: quantum_keypair.public_key.clone(),
        };

        let message = b"Falcon sign/verify level mapping must agree";

        let signature = key_pair
            .sign(message)
            .expect("Falcon signing should delegate to the real implementation");
        assert_eq!(signature.signature_type, SignatureType::Falcon);

        // Before the fix this returned
        // Err(CryptoOperationFailed("Invalid security level: Unsupported
        // security level: 2")) because verify rejected level 2 outright.
        let valid = signature
            .verify(message)
            .expect("Falcon verification must accept the same level as signing");
        assert!(valid, "a genuine Falcon signature must verify as valid");

        // A tampered message must not verify.
        let tampered = signature.verify(b"different message").unwrap_or(false);
        assert!(
            !tampered,
            "signature must not verify against a different message"
        );
    }

    #[test]
    #[ignore] // Signature verification implementation pending
    fn test_signature_verification() {
        // Test verification with mismatched keys and messages
        let verifier = SignatureVerifier::new();

        // Create a valid key pair and signature
        let key_pair = generate_test_keypair(SignatureType::Secp256k1);
        let message = b"Test message";
        let signature = sign_test_message(&key_pair, message);

        // Verify with correct message should succeed
        let result = verifier.verify(
            SignatureType::Secp256k1,
            &key_pair.public_key,
            message,
            &signature,
        );
        assert!(result.is_ok());

        // Verify with incorrect message should fail
        let wrong_message = b"Wrong message";
        let result = verifier.verify(
            SignatureType::Secp256k1,
            &key_pair.public_key,
            wrong_message,
            &signature,
        );

        if let Err(err) = result {
            assert!(matches!(err, SignatureError::InvalidSignature(_)));
        }
    }
}
