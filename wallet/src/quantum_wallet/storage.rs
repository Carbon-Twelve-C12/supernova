// Encrypted Wallet Storage for Supernova
// Persists wallet data securely with AES-256-GCM encryption

use aes_gcm::{
    aead::{Aead, KeyInit, OsRng},
    Aes256Gcm, Nonce,
};
use argon2::{Argon2, PasswordHasher};
use argon2::password_hash::SaltString;
use serde::{Deserialize, Serialize};
use sled::Db;
use std::path::Path;
use std::sync::Arc;
use thiserror::Error;
use zeroize::Zeroize;

use super::keystore::KeyPair;
use super::utxo_index::Utxo;

#[derive(Error, Debug)]
pub enum StorageError {
    #[error("Database error: {0}")]
    DatabaseError(String),
    
    #[error("Encryption error: {0}")]
    EncryptionError(String),
    
    #[error("Decryption error: {0}")]
    DecryptionError(String),
    
    #[error("Serialization error: {0}")]
    SerializationError(String),
    
    #[error("Key derivation error: {0}")]
    KeyDerivationError(String),
    
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
    
    #[error("Item not found: {0}")]
    NotFound(String),
}

/// Wallet storage backend
pub struct WalletStorage {
    /// Sled database handle
    db: Arc<Db>,
    
    /// Encryption cipher
    cipher: Option<Aes256Gcm>,
    
    /// Salt for key derivation
    salt: Vec<u8>,
}

#[derive(Debug, Serialize, Deserialize)]
struct EncryptedData {
    nonce: Vec<u8>,
    ciphertext: Vec<u8>,
}

#[derive(Debug, Serialize, Deserialize)]
struct WalletMetadata {
    version: u32,
    created_at: u64,
    last_modified: u64,
    network: String,
}

impl WalletStorage {
    /// Open wallet storage
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self, StorageError> {
        let db = sled::open(path)
            .map_err(|e| StorageError::DatabaseError(e.to_string()))?;
        
        // Load or generate salt
        let salt = match db.get(b"__salt__")
            .map_err(|e| StorageError::DatabaseError(e.to_string()))? 
        {
            Some(s) => s.to_vec(),
            None => {
                // Generate new salt
                let salt = SaltString::generate(&mut OsRng);
                let salt_str = salt.as_str();
                db.insert(b"__salt__", salt_str.as_bytes())
                    .map_err(|e| StorageError::DatabaseError(e.to_string()))?;
                salt_str.as_bytes().to_vec()
            }
        };
        
        Ok(Self {
            db: Arc::new(db),
            cipher: None,
            salt,
        })
    }
    
    /// Initialize encryption with passphrase
    pub fn unlock(&mut self, passphrase: &str) -> Result<(), StorageError> {
        // Derive encryption key from passphrase using Argon2
        let argon2 = Argon2::default();
        
        let salt = SaltString::from_b64(&String::from_utf8_lossy(&self.salt))
            .map_err(|e| StorageError::KeyDerivationError(e.to_string()))?;
        
        let password_hash = argon2.hash_password(passphrase.as_bytes(), &salt)
            .map_err(|e| StorageError::KeyDerivationError(e.to_string()))?;
        
        // Extract 32-byte key from hash
        let hash_output = password_hash.hash
            .ok_or_else(|| StorageError::KeyDerivationError("No hash produced".to_string()))?;
        let key_bytes = hash_output.as_bytes();
        
        let mut key = [0u8; 32];
        key.copy_from_slice(&key_bytes[..32]);
        
        // Create cipher
        self.cipher = Some(Aes256Gcm::new_from_slice(&key)
            .map_err(|e| StorageError::EncryptionError(e.to_string()))?);
        
        key.zeroize();
        
        Ok(())
    }
    
    /// Store keypair securely
    pub fn store_keypair(&self, address: &str, keypair: &KeyPair) -> Result<(), StorageError> {
        let cipher = self.cipher.as_ref()
            .ok_or_else(|| StorageError::EncryptionError("Wallet locked".to_string()))?;
        
        // Serialize keypair
        let data = bincode::serialize(keypair)
            .map_err(|e| StorageError::SerializationError(e.to_string()))?;
        
        // Encrypt
        let encrypted = self.encrypt_data(&data)?;
        
        // Store in database
        let key = format!("keypair_{}", address);
        self.db.insert(key.as_bytes(), bincode::serialize(&encrypted)
            .map_err(|e| StorageError::SerializationError(e.to_string()))?)
            .map_err(|e| StorageError::DatabaseError(e.to_string()))?;
        
        Ok(())
    }
    
    /// Load keypair
    pub fn load_keypair(&self, address: &str) -> Result<KeyPair, StorageError> {
        let cipher = self.cipher.as_ref()
            .ok_or_else(|| StorageError::DecryptionError("Wallet locked".to_string()))?;
        
        // Load from database
        let key = format!("keypair_{}", address);
        let encrypted_bytes = self.db.get(key.as_bytes())
            .map_err(|e| StorageError::DatabaseError(e.to_string()))?
            .ok_or_else(|| StorageError::NotFound(format!("Keypair for {}", address)))?;
        
        let encrypted: EncryptedData = bincode::deserialize(&encrypted_bytes)
            .map_err(|e| StorageError::SerializationError(e.to_string()))?;
        
        // Decrypt
        let data = self.decrypt_data(&encrypted)?;
        
        // Deserialize
        let keypair = bincode::deserialize(&data)
            .map_err(|e| StorageError::SerializationError(e.to_string()))?;
        
        Ok(keypair)
    }
    
    /// Store UTXO
    pub fn store_utxo(&self, utxo: &Utxo) -> Result<(), StorageError> {
        let key = format!("utxo_{}:{}", hex::encode(&utxo.txid), utxo.vout);
        
        self.db.insert(
            key.as_bytes(),
            bincode::serialize(utxo)
                .map_err(|e| StorageError::SerializationError(e.to_string()))?
        ).map_err(|e| StorageError::DatabaseError(e.to_string()))?;
        
        Ok(())
    }
    
    /// Load UTXO
    pub fn load_utxo(&self, txid: &[u8; 32], vout: u32) -> Result<Utxo, StorageError> {
        let key = format!("utxo_{}:{}", hex::encode(txid), vout);
        
        let bytes = self.db.get(key.as_bytes())
            .map_err(|e| StorageError::DatabaseError(e.to_string()))?
            .ok_or_else(|| StorageError::NotFound(format!("UTXO {}:{}", hex::encode(txid), vout)))?;
        
        bincode::deserialize(&bytes)
            .map_err(|e| StorageError::SerializationError(e.to_string()))
    }
    
    /// Delete UTXO (when spent)
    pub fn delete_utxo(&self, txid: &[u8; 32], vout: u32) -> Result<(), StorageError> {
        let key = format!("utxo_{}:{}", hex::encode(txid), vout);
        
        self.db.remove(key.as_bytes())
            .map_err(|e| StorageError::DatabaseError(e.to_string()))?;
        
        Ok(())
    }
    
    /// List all keypair addresses
    pub fn list_addresses(&self) -> Result<Vec<String>, StorageError> {
        let mut addresses = Vec::new();
        
        for item in self.db.scan_prefix(b"keypair_") {
            let (key, _) = item.map_err(|e| StorageError::DatabaseError(e.to_string()))?;
            
            // Extract address from key
            if let Ok(key_str) = String::from_utf8(key.to_vec()) {
                if let Some(address) = key_str.strip_prefix("keypair_") {
                    addresses.push(address.to_string());
                }
            }
        }
        
        Ok(addresses)
    }
    
    /// List all UTXOs
    pub fn list_utxos(&self) -> Result<Vec<Utxo>, StorageError> {
        let mut utxos = Vec::new();
        
        for item in self.db.scan_prefix(b"utxo_") {
            let (_, value) = item.map_err(|e| StorageError::DatabaseError(e.to_string()))?;
            
            let utxo: Utxo = bincode::deserialize(&value)
                .map_err(|e| StorageError::SerializationError(e.to_string()))?;
            
            utxos.push(utxo);
        }
        
        Ok(utxos)
    }
    
    /// Store wallet metadata
    pub fn store_metadata(&self, metadata: &WalletMetadata) -> Result<(), StorageError> {
        self.db.insert(
            b"__metadata__",
            bincode::serialize(metadata)
                .map_err(|e| StorageError::SerializationError(e.to_string()))?
        ).map_err(|e| StorageError::DatabaseError(e.to_string()))?;
        
        Ok(())
    }
    
    /// Load wallet metadata
    pub fn load_metadata(&self) -> Result<WalletMetadata, StorageError> {
        let bytes = self.db.get(b"__metadata__")
            .map_err(|e| StorageError::DatabaseError(e.to_string()))?
            .ok_or_else(|| StorageError::NotFound("Metadata".to_string()))?;
        
        bincode::deserialize(&bytes)
            .map_err(|e| StorageError::SerializationError(e.to_string()))
    }
    
    // Helper methods for encryption
    
    fn encrypt_data(&self, data: &[u8]) -> Result<EncryptedData, StorageError> {
        let cipher = self.cipher.as_ref()
            .ok_or_else(|| StorageError::EncryptionError("Cipher not initialized".to_string()))?;
        
        // Generate random nonce
        let nonce_bytes = aes_gcm::aead::rand_core::RngCore::next_u64(&mut OsRng).to_le_bytes();
        let mut nonce_array = [0u8; 12];
        nonce_array[..8].copy_from_slice(&nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_array);
        
        // Encrypt
        let ciphertext = cipher.encrypt(nonce, data)
            .map_err(|e| StorageError::EncryptionError(e.to_string()))?;
        
        Ok(EncryptedData {
            nonce: nonce.to_vec(),
            ciphertext,
        })
    }
    
    fn decrypt_data(&self, encrypted: &EncryptedData) -> Result<Vec<u8>, StorageError> {
        let cipher = self.cipher.as_ref()
            .ok_or_else(|| StorageError::DecryptionError("Cipher not initialized".to_string()))?;
        
        // Reconstruct nonce
        let nonce = Nonce::from_slice(&encrypted.nonce);
        
        // Decrypt
        let plaintext = cipher.decrypt(nonce, encrypted.ciphertext.as_ref())
            .map_err(|e| StorageError::DecryptionError(e.to_string()))?;
        
        Ok(plaintext)
    }
    
    /// Store transaction
    pub fn store_transaction(&self, txid: &[u8; 32], transaction: &supernova_core::types::transaction::Transaction) -> Result<(), StorageError> {
        let key = format!("tx_{}", hex::encode(txid));
        
        self.db.insert(
            key.as_bytes(),
            bincode::serialize(transaction)
                .map_err(|e| StorageError::SerializationError(e.to_string()))?
        ).map_err(|e| StorageError::DatabaseError(e.to_string()))?;
        
        Ok(())
    }
    
    /// Load transaction
    pub fn load_transaction(&self, txid: &[u8; 32]) -> Result<supernova_core::types::transaction::Transaction, StorageError> {
        let key = format!("tx_{}", hex::encode(txid));
        
        let bytes = self.db.get(key.as_bytes())
            .map_err(|e| StorageError::DatabaseError(e.to_string()))?
            .ok_or_else(|| StorageError::NotFound(format!("Transaction {}", hex::encode(txid))))?;
        
        bincode::deserialize(&bytes)
            .map_err(|e| StorageError::SerializationError(e.to_string()))
    }
    
    /// Flush all pending writes to disk
    pub fn flush(&self) -> Result<(), StorageError> {
        self.db.flush()
            .map_err(|e| StorageError::DatabaseError(e.to_string()))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    
    #[test]
    fn test_storage_creation() {
        let temp_dir = TempDir::new().unwrap();
        let storage = WalletStorage::open(temp_dir.path().join("wallet.db")).unwrap();
        
        // Should create database successfully
        assert!(storage.db.was_recovered());
    }
    
    #[test]
    fn test_encryption_unlock() {
        let temp_dir = TempDir::new().unwrap();
        let mut storage = WalletStorage::open(temp_dir.path().join("wallet.db")).unwrap();
        
        // Unlock with passphrase
        storage.unlock("test_password").unwrap();
        
        // Should have cipher initialized
        assert!(storage.cipher.is_some());
    }
    
    #[test]
    fn test_keypair_storage() {
        let temp_dir = TempDir::new().unwrap();
        let mut storage = WalletStorage::open(temp_dir.path().join("wallet.db")).unwrap();
        storage.unlock("test_password").unwrap();
        
        // Generate keypair
        let keypair = KeyPair::generate(Some("test".to_string())).unwrap();
        let address = keypair.address.to_string();
        
        // Store keypair
        storage.store_keypair(&address, &keypair).unwrap();
        
        // Load keypair
        let loaded = storage.load_keypair(&address).unwrap();
        
        // Verify data matches
        assert_eq!(loaded.public_key, keypair.public_key);
        assert_eq!(loaded.secret_key, keypair.secret_key);
        assert_eq!(loaded.address, keypair.address);
    }
    
    #[test]
    fn test_address_listing() {
        let temp_dir = TempDir::new().unwrap();
        let mut storage = WalletStorage::open(temp_dir.path().join("wallet.db")).unwrap();
        storage.unlock("test_password").unwrap();
        
        // Store multiple keypairs
        for i in 0..3 {
            let keypair = KeyPair::generate(Some(format!("addr{}", i))).unwrap();
            storage.store_keypair(&keypair.address.to_string(), &keypair).unwrap();
        }
        
        // List addresses
        let addresses = storage.list_addresses().unwrap();
        assert_eq!(addresses.len(), 3);
    }
    
    #[test]
    fn test_metadata_storage() {
        let temp_dir = TempDir::new().unwrap();
        let storage = WalletStorage::open(temp_dir.path().join("wallet.db")).unwrap();
        
        let metadata = WalletMetadata {
            version: 1,
            created_at: 1704067200,
            last_modified: 1704067200,
            network: "testnet".to_string(),
        };
        
        // Store metadata
        storage.store_metadata(&metadata).unwrap();
        
        // Load metadata
        let loaded = storage.load_metadata().unwrap();
        assert_eq!(loaded.version, 1);
        assert_eq!(loaded.network, "testnet");
    }
}

