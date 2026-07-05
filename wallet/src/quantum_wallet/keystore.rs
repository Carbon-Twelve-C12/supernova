// Quantum Key Management System
// Secure storage and management of ML-DSA keypairs

use argon2::{
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Algorithm, Argon2, Params, Version,
};
use pqcrypto_dilithium::dilithium5;
use pqcrypto_traits::sign::{
    DetachedSignature, PublicKey as PqPublicKey, SecretKey as PqSecretKey,
};
use rand::rngs::OsRng;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use thiserror::Error;
use zeroize::Zeroize;

use super::address::Address;
use super::hd_derivation::QuantumHDConfig;

/// Construct an Argon2id hasher with the project-wide OWASP-aligned parameters
/// (64 MiB, t=3, p=4) drawn from `QuantumHDConfig`, rather than the library
/// defaults (m=19 MiB, t=2, p=1). This keeps the keystore passphrase barrier
/// consistent with `hdwallet.rs`, `storage.rs`, and the documented standard.
///
/// The full PHC string is stored on hashing, so verification reads the
/// parameters back from that string and remains compatible with hashes
/// produced under any parameters.
fn keystore_argon2<'a>() -> Result<Argon2<'a>, KeystoreError> {
    let params = Params::new(
        QuantumHDConfig::ARGON2_MEMORY_KB,
        QuantumHDConfig::ARGON2_ITERATIONS,
        QuantumHDConfig::ARGON2_PARALLELISM,
        None,
    )
    .map_err(|e| KeystoreError::EncryptionError(format!("Argon2 params invalid: {}", e)))?;
    Ok(Argon2::new(Algorithm::Argon2id, Version::V0x13, params))
}

#[derive(Error, Debug)]
pub enum KeystoreError {
    #[error("Keystore is locked")]
    Locked,
    
    #[error("Invalid passphrase")]
    InvalidPassphrase,
    
    #[error("Key not found for address: {0}")]
    KeyNotFound(String),
    
    #[error("Keypair generation failed: {0}")]
    GenerationFailed(String),
    
    #[error("Encryption error: {0}")]
    EncryptionError(String),
    
    #[error("Serialization error: {0}")]
    SerializationError(String),
}

/// Quantum-resistant keypair
#[derive(Clone, Serialize, Deserialize)]
pub struct KeyPair {
    /// ML-DSA public key (~1952 bytes for Dilithium5)
    pub public_key: Vec<u8>,
    /// ML-DSA secret key (~4864 bytes for Dilithium5)
    pub secret_key: Vec<u8>,
    /// Derived address
    pub address: Address,
    /// Optional label for organization
    pub label: Option<String>,
    /// Creation timestamp
    pub created_at: u64,
}

// Manual Debug impl: never expose the raw secret key bytes. A derived Debug
// would print `secret_key: Vec<u8>` verbatim, leaking the ML-DSA secret through
// any direct or transitive Debug-formatting (logs, error contexts, etc.).
impl std::fmt::Debug for KeyPair {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("KeyPair")
            .field("public_key", &self.public_key)
            .field(
                "secret_key",
                &format_args!("<redacted {} bytes>", self.secret_key.len()),
            )
            .field("address", &self.address)
            .field("label", &self.label)
            .field("created_at", &self.created_at)
            .finish()
    }
}

impl KeyPair {
    /// Generate a new quantum-resistant keypair
    pub fn generate(label: Option<String>) -> Result<Self, KeystoreError> {
        // Generate ML-DSA (Dilithium5) keypair for maximum security
        let (pk, sk) = dilithium5::keypair();
        
        let public_key = pk.as_bytes().to_vec();
        let secret_key = sk.as_bytes().to_vec();
        
        // Derive address from public key using SHA3-512
        let address = Address::from_public_key(&public_key)
            .map_err(|e| KeystoreError::GenerationFailed(e.to_string()))?;
        
        let created_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|e| KeystoreError::GenerationFailed(e.to_string()))?
            .as_secs();
        
        Ok(Self {
            public_key,
            secret_key,
            address,
            label,
            created_at,
        })
    }
    
    /// Sign a message using ML-DSA
    pub fn sign(&self, message: &[u8]) -> Result<Vec<u8>, KeystoreError> {
        // Reconstruct secret key from bytes
        let sk = dilithium5::SecretKey::from_bytes(&self.secret_key)
            .map_err(|_| KeystoreError::GenerationFailed("Invalid secret key".to_string()))?;
        
        // Sign the message
        let signature = dilithium5::detached_sign(message, &sk);
        
        Ok(signature.as_bytes().to_vec())
    }
    
    /// Verify a signature
    pub fn verify(public_key: &[u8], message: &[u8], signature: &[u8]) -> bool {
        // Reconstruct public key
        let pk = match dilithium5::PublicKey::from_bytes(public_key) {
            Ok(pk) => pk,
            Err(_) => return false,
        };
        
        // Reconstruct signature
        let sig = match dilithium5::DetachedSignature::from_bytes(signature) {
            Ok(sig) => sig,
            Err(_) => return false,
        };
        
        // Verify
        dilithium5::verify_detached_signature(&sig, message, &pk).is_ok()
    }
}

impl Drop for KeyPair {
    fn drop(&mut self) {
        // Zero out sensitive data
        self.secret_key.zeroize();
    }
}

impl Keystore {
    /// Zero out sensitive material so the passphrase hash does not linger in
    /// freed heap memory (Security Review Checklist: "Key zeroization after use").
    fn zeroize_secrets(&mut self) {
        if let Some(hash) = self.passphrase_hash.as_mut() {
            hash.zeroize();
        }
    }
}

impl Drop for Keystore {
    fn drop(&mut self) {
        self.zeroize_secrets();
    }
}

/// Secure keystore for managing quantum keypairs
///
/// NOTE: This keystore is NOT hierarchical-deterministic. Each address is an
/// independent ML-DSA keypair generated from fresh OS entropy (see
/// `generate_address` / `KeyPair::generate`). There is no master seed from
/// which keys can be re-derived: pqcrypto-dilithium exposes no seeded keygen,
/// so `QuantumHDDerivation` cannot be wired in without a key-generation-model
/// change. Recovery therefore depends entirely on the encrypted, persisted
/// keypairs (see `storage.rs`) — there is no seed phrase that reconstructs
/// funds, and operators must back up the keystore itself.
pub struct Keystore {
    /// Active keypairs indexed by address
    keypairs: Arc<RwLock<HashMap<String, KeyPair>>>,
    
    /// Watch-only addresses (no private keys)
    watch_addresses: Arc<RwLock<HashMap<String, WatchAddress>>>,
    
    /// Is keystore currently locked?
    locked: Arc<RwLock<bool>>,
    
    /// Passphrase hash for verification (never store plaintext passphrase)
    passphrase_hash: Option<Vec<u8>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WatchAddress {
    pub address: String,
    pub label: Option<String>,
    pub added_at: u64,
}

impl Keystore {
    /// Create a new empty keystore
    pub fn new() -> Self {
        Self {
            keypairs: Arc::new(RwLock::new(HashMap::new())),
            watch_addresses: Arc::new(RwLock::new(HashMap::new())),
            locked: Arc::new(RwLock::new(true)), // Locked by default
            passphrase_hash: None,
        }
    }
    
    /// Initialize keystore with passphrase
    pub fn initialize(&mut self, passphrase: &str) -> Result<(), KeystoreError> {
        // Hash passphrase using Argon2id (memory-hard, resistant to GPU/ASIC attacks)
        let salt = SaltString::generate(&mut OsRng);
        let argon2 = keystore_argon2()?;
        let password_hash = argon2
            .hash_password(passphrase.as_bytes(), &salt)
            .map_err(|e| KeystoreError::EncryptionError(format!("Password hashing failed: {}", e)))?;

        // Store the full PHC string (includes algorithm, params, salt, and hash)
        self.passphrase_hash = Some(password_hash.to_string().into_bytes());

        // Unlock keystore
        *self.locked.write().map_err(|_|
            KeystoreError::EncryptionError("Lock poisoned".to_string())
        )? = false;

        Ok(())
    }
    
    /// Lock the keystore
    pub fn lock(&self) -> Result<(), KeystoreError> {
        *self.locked.write().map_err(|_| 
            KeystoreError::EncryptionError("Lock poisoned".to_string())
        )? = true;
        Ok(())
    }
    
    /// Unlock the keystore
    pub fn unlock(&self, passphrase: &str) -> Result<(), KeystoreError> {
        // Retrieve stored Argon2 hash
        let stored_hash_bytes = self.passphrase_hash
            .as_ref()
            .ok_or(KeystoreError::InvalidPassphrase)?;

        // Convert stored bytes back to PHC string
        let hash_str = String::from_utf8(stored_hash_bytes.clone())
            .map_err(|_| KeystoreError::InvalidPassphrase)?;

        // Parse the PHC string to get the PasswordHash
        let password_hash = PasswordHash::new(&hash_str)
            .map_err(|_| KeystoreError::InvalidPassphrase)?;

        // Verify passphrase using Argon2. Parameters are read from the stored
        // PHC string, so this remains compatible regardless of which parameters
        // produced the hash; we still construct the hasher with the project
        // standard for consistency.
        let argon2 = keystore_argon2()?;
        if argon2.verify_password(passphrase.as_bytes(), &password_hash).is_ok() {
            *self.locked.write().map_err(|_|
                KeystoreError::EncryptionError("Lock poisoned".to_string())
            )? = false;
            Ok(())
        } else {
            Err(KeystoreError::InvalidPassphrase)
        }
    }
    
    /// Check if keystore is locked
    pub fn is_locked(&self) -> bool {
        self.locked.read()
            .map(|l| *l)
            .unwrap_or(true)
    }
    
    /// Generate a new address
    pub fn generate_address(&self, label: Option<String>) -> Result<Address, KeystoreError> {
        if self.is_locked() {
            return Err(KeystoreError::Locked);
        }
        
        // Generate new keypair
        let keypair = KeyPair::generate(label)?;
        let address = keypair.address.clone();
        
        // Store keypair
        self.keypairs.write().map_err(|_| 
            KeystoreError::EncryptionError("Lock poisoned".to_string())
        )?.insert(address.to_string(), keypair);
        
        Ok(address)
    }
    
    /// Get keypair for an address
    pub fn get_keypair(&self, address: &str) -> Result<KeyPair, KeystoreError> {
        if self.is_locked() {
            return Err(KeystoreError::Locked);
        }
        
        self.keypairs.read().map_err(|_| 
            KeystoreError::EncryptionError("Lock poisoned".to_string())
        )?.get(address)
            .cloned()
            .ok_or_else(|| KeystoreError::KeyNotFound(address.to_string()))
    }
    
    /// List all addresses in keystore
    pub fn list_addresses(&self) -> Result<Vec<String>, KeystoreError> {
        let keypairs = self.keypairs.read().map_err(|_| 
            KeystoreError::EncryptionError("Lock poisoned".to_string())
        )?;
        
        Ok(keypairs.keys().cloned().collect())
    }
    
    /// Add watch-only address
    pub fn add_watch_address(&self, address: String, label: Option<String>) -> Result<(), KeystoreError> {
        let watch_addr = WatchAddress {
            address: address.clone(),
            label,
            added_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_err(|e| KeystoreError::GenerationFailed(e.to_string()))?
                .as_secs(),
        };
        
        self.watch_addresses.write().map_err(|_| 
            KeystoreError::EncryptionError("Lock poisoned".to_string())
        )?.insert(address, watch_addr);
        
        Ok(())
    }
    
    /// Check if address is in keystore (owned or watch-only)
    pub fn has_address(&self, address: &str) -> bool {
        self.keypairs.read()
            .map(|k| k.contains_key(address))
            .unwrap_or(false) ||
        self.watch_addresses.read()
            .map(|w| w.contains_key(address))
            .unwrap_or(false)
    }
    
    /// Load keypair from storage (used during wallet initialization)
    pub fn load_keypair(&self, address: String, keypair: KeyPair) -> Result<(), KeystoreError> {
        self.keypairs.write().map_err(|_| 
            KeystoreError::EncryptionError("Lock poisoned".to_string())
        )?.insert(address, keypair);
        
        Ok(())
    }
}

impl Default for Keystore {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_keypair_generation() {
        let keypair = KeyPair::generate(Some("test".to_string())).unwrap();
        
        // Verify public key size (Dilithium5)
        assert_eq!(keypair.public_key.len(), 2592);
        
        // Verify secret key size
        assert_eq!(keypair.secret_key.len(), 4864);
        
        // Verify address is valid
        assert!(keypair.address.to_string().starts_with("nova1"));
    }
    
    #[test]
    fn test_debug_redacts_secret_key() {
        let keypair = KeyPair::generate(Some("test".to_string())).unwrap();
        let debug_output = format!("{:?}", keypair);

        // The redaction marker must be present.
        assert!(
            debug_output.contains("<redacted"),
            "Debug output must redact the secret key"
        );

        // The raw secret key bytes must never appear in Debug output. The first
        // few bytes of the secret key, rendered as a Vec<u8> would, must be absent.
        let leaked_prefix = format!("{:?}", &keypair.secret_key[..8]);
        assert!(
            !debug_output.contains(&leaked_prefix),
            "Debug output must not contain raw secret key bytes"
        );

        // Non-secret fields should still be visible.
        assert!(debug_output.contains("public_key"));
        assert!(debug_output.contains("created_at"));
    }

    #[test]
    fn test_signature_verification() {
        let keypair = KeyPair::generate(None).unwrap();
        let message = b"Test message for signing";
        
        // Sign message
        let signature = keypair.sign(message).unwrap();
        
        // Verify signature
        assert!(KeyPair::verify(&keypair.public_key, message, &signature));
        
        // Verify fails with wrong message
        assert!(!KeyPair::verify(&keypair.public_key, b"wrong message", &signature));
        
        // Verify fails with wrong key
        let other_keypair = KeyPair::generate(None).unwrap();
        assert!(!KeyPair::verify(&other_keypair.public_key, message, &signature));
    }
    
    #[test]
    fn test_keystore_locking() {
        let mut keystore = Keystore::new();
        
        // Initialize with passphrase
        keystore.initialize("test_passphrase").unwrap();
        assert!(!keystore.is_locked());
        
        // Lock keystore
        keystore.lock().unwrap();
        assert!(keystore.is_locked());
        
        // Unlock with correct passphrase
        keystore.unlock("test_passphrase").unwrap();
        assert!(!keystore.is_locked());
        
        // Lock again
        keystore.lock().unwrap();
        
        // Unlock with wrong passphrase should fail
        assert!(keystore.unlock("wrong_passphrase").is_err());
        assert!(keystore.is_locked());
    }
    
    #[test]
    fn test_passphrase_hash_uses_project_argon2_params() {
        // Regression guard (R5-94): the keystore passphrase hash must use the
        // project-wide OWASP-aligned Argon2id parameters (64 MiB, t=3, p=4),
        // not the library defaults (19 MiB, t=2, p=1). The full PHC string is
        // stored, so the parameters are recoverable and must match the standard.
        let mut keystore = Keystore::new();
        keystore.initialize("test_passphrase").unwrap();

        let hash_bytes = keystore.passphrase_hash.as_ref().unwrap();
        let hash_str = String::from_utf8(hash_bytes.clone()).unwrap();
        let parsed = PasswordHash::new(&hash_str).unwrap();

        // Algorithm must be Argon2id.
        assert_eq!(parsed.algorithm.as_str(), "argon2id");

        // Parameters embedded in the PHC string must match the standard.
        let params = Params::try_from(&parsed).unwrap();
        assert_eq!(params.m_cost(), QuantumHDConfig::ARGON2_MEMORY_KB);
        assert_eq!(params.t_cost(), QuantumHDConfig::ARGON2_ITERATIONS);
        assert_eq!(params.p_cost(), QuantumHDConfig::ARGON2_PARALLELISM);

        // And a round-trip unlock must still succeed with these parameters.
        keystore.lock().unwrap();
        keystore.unlock("test_passphrase").unwrap();
        assert!(!keystore.is_locked());
    }

    #[test]
    fn test_address_generation_when_locked() {
        let keystore = Keystore::new();
        
        // Should fail when locked
        assert!(matches!(
            keystore.generate_address(None),
            Err(KeystoreError::Locked)
        ));
    }
    
    #[test]
    fn test_address_generation_when_unlocked() {
        let mut keystore = Keystore::new();
        keystore.initialize("test_passphrase").unwrap();
        
        // Generate address
        let address = keystore.generate_address(Some("test".to_string())).unwrap();
        
        // Verify it's stored
        assert!(keystore.has_address(&address.to_string()));
        
        // Verify we can retrieve keypair
        let keypair = keystore.get_keypair(&address.to_string()).unwrap();
        assert_eq!(keypair.address, address);
        assert_eq!(keypair.label, Some("test".to_string()));
    }
    
    #[test]
    fn test_zeroize_secrets_clears_sensitive_material() {
        let mut keystore = Keystore::new();
        keystore.initialize("test_passphrase").unwrap();

        // Sanity: initialization populated the secret material.
        assert!(keystore
            .passphrase_hash
            .as_ref()
            .is_some_and(|h| !h.is_empty()));

        // Exercise the same routine the Drop impl runs.
        keystore.zeroize_secrets();

        // Buffer still exists but is fully zeroed.
        assert!(keystore
            .passphrase_hash
            .as_ref()
            .is_some_and(|h| h.iter().all(|&b| b == 0)));
    }

    #[test]
    fn test_keystore_has_no_master_seed_field() {
        // Regression guard (R5-36): the keystore is deliberately NOT
        // hierarchical-deterministic. Each address is an independent ML-DSA
        // keypair from fresh OS entropy, so two addresses generated from the
        // same initialized keystore must have unrelated public keys — there is
        // no seed that ties them together or that could reconstruct funds.
        let mut keystore = Keystore::new();
        keystore.initialize("test_passphrase").unwrap();

        let addr1 = keystore.generate_address(None).unwrap();
        let addr2 = keystore.generate_address(None).unwrap();
        assert_ne!(addr1, addr2, "each generated address must be distinct");

        let kp1 = keystore.get_keypair(&addr1.to_string()).unwrap();
        let kp2 = keystore.get_keypair(&addr2.to_string()).unwrap();
        assert_ne!(
            kp1.public_key, kp2.public_key,
            "independently generated keypairs must not share key material"
        );
    }

    #[test]
    fn test_watch_only_addresses() {
        let keystore = Keystore::new();
        let watch_addr = "nova1qtest123456789".to_string();
        
        // Add watch-only address
        keystore.add_watch_address(watch_addr.clone(), Some("watched".to_string())).unwrap();
        
        // Verify it's tracked
        assert!(keystore.has_address(&watch_addr));
        
        // Verify we cannot get keypair (watch-only)
        assert!(matches!(
            keystore.get_keypair(&watch_addr),
            Err(KeystoreError::KeyNotFound(_))
        ));
    }
}

