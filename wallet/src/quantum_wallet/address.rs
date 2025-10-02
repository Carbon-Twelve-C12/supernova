// Quantum-Resistant Address System for Supernova
// Bech32 encoding with 'nova' prefix for post-quantum public keys

use bech32::{self, FromBase32, ToBase32, Variant};
use serde::{Deserialize, Serialize};
use sha3::{Digest, Sha3_512};
use std::fmt;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum AddressError {
    #[error("Invalid address format: {0}")]
    InvalidFormat(String),
    
    #[error("Invalid address checksum")]
    InvalidChecksum,
    
    #[error("Unsupported address type: {0}")]
    UnsupportedType(String),
    
    #[error("Bech32 encoding error: {0}")]
    Bech32Error(String),
    
    #[error("Invalid public key")]
    InvalidPublicKey,
}

/// Address type for Supernova quantum-resistant addresses
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AddressType {
    /// Standard single-signature address
    Standard,
    /// Multisignature address (future)
    Multisig,
    /// Script hash address (future)
    ScriptHash,
}

/// Quantum-resistant address
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Address {
    /// Bech32-encoded address string
    address: String,
    /// Address type
    address_type: AddressType,
    /// Public key hash (32 bytes from SHA3-512)
    pubkey_hash: [u8; 32],
}

impl Address {
    /// Create address from quantum-resistant public key
    pub fn from_public_key(public_key: &[u8]) -> Result<Self, AddressError> {
        // Hash the public key using SHA3-512, then take first 32 bytes
        let mut hasher = Sha3_512::new();
        hasher.update(public_key);
        let full_hash = hasher.finalize();
        
        let mut pubkey_hash = [0u8; 32];
        pubkey_hash.copy_from_slice(&full_hash[..32]);
        
        // Encode as Bech32 with 'nova' prefix
        let address = bech32::encode("nova", pubkey_hash.to_base32(), Variant::Bech32m)
            .map_err(|e| AddressError::Bech32Error(e.to_string()))?;
        
        Ok(Self {
            address,
            address_type: AddressType::Standard,
            pubkey_hash,
        })
    }
    
    /// Parse address from string
    pub fn from_str(address: &str) -> Result<Self, AddressError> {
        // Decode Bech32
        let (hrp, data, variant) = bech32::decode(address)
            .map_err(|e| AddressError::Bech32Error(e.to_string()))?;
        
        // Verify HRP is 'nova'
        if hrp.as_str() != "nova" {
            return Err(AddressError::InvalidFormat(
                format!("Invalid prefix: expected 'nova', got '{}'", hrp)
            ));
        }
        
        // Verify variant
        if variant != Variant::Bech32m {
            return Err(AddressError::InvalidFormat(
                "Invalid Bech32 variant: must use Bech32m".to_string()
            ));
        }
        
        // Convert from base32
        let decoded = Vec::<u8>::from_base32(&data)
            .map_err(|e| AddressError::Bech32Error(e.to_string()))?;
        
        // Verify length
        if decoded.len() != 32 {
            return Err(AddressError::InvalidFormat(
                format!("Invalid address length: expected 32 bytes, got {}", decoded.len())
            ));
        }
        
        let mut pubkey_hash = [0u8; 32];
        pubkey_hash.copy_from_slice(&decoded);
        
        Ok(Self {
            address: address.to_string(),
            address_type: AddressType::Standard,
            pubkey_hash,
        })
    }
    
    /// Get address as string
    pub fn to_string(&self) -> String {
        self.address.clone()
    }
    
    /// Get public key hash
    pub fn pubkey_hash(&self) -> &[u8; 32] {
        &self.pubkey_hash
    }
    
    /// Get address type
    pub fn address_type(&self) -> AddressType {
        self.address_type
    }
    
    /// Validate address format
    pub fn validate(address: &str) -> bool {
        Self::from_str(address).is_ok()
    }
}

impl fmt::Display for Address {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.address)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pqcrypto_dilithium::dilithium5;
    
    #[test]
    fn test_address_from_public_key() {
        // Generate test keypair
        let (pk, _sk) = dilithium5::keypair();
        let public_key = pk.as_bytes();
        
        // Create address
        let address = Address::from_public_key(public_key).unwrap();
        
        // Verify format
        assert!(address.to_string().starts_with("nova1"));
        assert!(address.to_string().len() > 50); // Bech32m encoded 32 bytes
    }
    
    #[test]
    fn test_address_parsing() {
        // Generate test address
        let (pk, _sk) = dilithium5::keypair();
        let original = Address::from_public_key(pk.as_bytes()).unwrap();
        
        // Parse back from string
        let parsed = Address::from_str(&original.to_string()).unwrap();
        
        // Should match
        assert_eq!(original, parsed);
        assert_eq!(original.pubkey_hash(), parsed.pubkey_hash());
    }
    
    #[test]
    fn test_invalid_address_prefix() {
        let result = Address::from_str("btc1qinvalidprefix");
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), AddressError::InvalidFormat(_)));
    }
    
    #[test]
    fn test_address_validation() {
        let (pk, _sk) = dilithium5::keypair();
        let address = Address::from_public_key(pk.as_bytes()).unwrap();
        
        // Valid address
        assert!(Address::validate(&address.to_string()));
        
        // Invalid addresses
        assert!(!Address::validate("invalid"));
        assert!(!Address::validate("btc1qinvalid"));
        assert!(!Address::validate("nova1invalid"));
    }
    
    #[test]
    fn test_deterministic_address_from_same_pubkey() {
        let (pk, _sk) = dilithium5::keypair();
        
        // Generate address twice from same pubkey
        let addr1 = Address::from_public_key(pk.as_bytes()).unwrap();
        let addr2 = Address::from_public_key(pk.as_bytes()).unwrap();
        
        // Should be identical
        assert_eq!(addr1, addr2);
        assert_eq!(addr1.to_string(), addr2.to_string());
    }
}

