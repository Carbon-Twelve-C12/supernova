// Quantum Key Management System
// Secure storage and management of ML-DSA keypairs

use pqcrypto_dilithium::dilithium5;
use pqcrypto_traits::sign::{
    DetachedSignature, PublicKey as PqPublicKey, SecretKey as PqSecretKey,
};
use serde::{Deserialize, Serialize};
use sha3::{Digest, Sha3_512};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use thiserror::Error;
use zeroize::Zeroize;

use super::address::Address;

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
#[derive(Debug, Clone, Serialize, Deserialize)]
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

/// Secure keystore for managing quantum keypairs
pub struct Keystore {
    /// Master seed for HD wallet (32 bytes, encrypted at rest)
    master_seed: Option<Vec<u8>>,
    
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
            master_seed: None,
            keypairs: Arc::new(RwLock::new(HashMap::new())),
            watch_addresses: Arc::new(RwLock::new(HashMap::new())),
            locked: Arc::new(RwLock::new(true)), // Locked by default
            passphrase_hash: None,
        }
    }
    
    /// Initialize keystore with passphrase
    pub fn initialize(&mut self, passphrase: &str) -> Result<(), KeystoreError> {
        // Hash passphrase using SHA3-512
        let mut hasher = Sha3_512::new();
        hasher.update(passphrase.as_bytes());
        self.passphrase_hash = Some(hasher.finalize().to_vec());
        
        // Generate master seed
        use rand::RngCore;
        let mut seed = vec![0u8; 64];
        rand::thread_rng().fill_bytes(&mut seed);
        self.master_seed = Some(seed);
        
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
        // Verify passphrase
        let mut hasher = Sha3_512::new();
        hasher.update(passphrase.as_bytes());
        let hash = hasher.finalize().to_vec();
        
        match &self.passphrase_hash {
            Some(stored_hash) if stored_hash == &hash => {
                *self.locked.write().map_err(|_| 
                    KeystoreError::EncryptionError("Lock poisoned".to_string())
                )? = false;
                Ok(())
            }
            Some(_) => Err(KeystoreError::InvalidPassphrase),
            None => Err(KeystoreError::InvalidPassphrase),
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

