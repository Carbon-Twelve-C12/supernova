//! Hash functions for the Supernova blockchain

use sha2::{Sha256, Sha512, Digest};
use blake3;
use std::fmt::Debug;
use hex;
use std::fmt;
use serde::{Serialize, Deserialize};

/// Hash trait for different hashing algorithms
pub trait Hash: Debug + Send + Sync {
    /// Hash data and return the digest
    fn hash(&self, data: &[u8]) -> Vec<u8>;
    
    /// Get the name of the hash algorithm
    fn algorithm_name(&self) -> &'static str;
    
    /// Get the output size of the hash in bytes
    fn output_size(&self) -> usize;
    
    /// Hash multiple data items by concatenating them
    fn hash_multiple(&self, data: &[&[u8]]) -> Vec<u8> {
        // Combine all data into a single buffer
        let mut combined = Vec::new();
        for d in data {
            combined.extend_from_slice(d);
        }
        self.hash(&combined)
    }
}

/// SHA-256 hash implementation
#[derive(Debug, Clone)]
pub struct Sha256Hash;

impl Hash for Sha256Hash {
    fn hash(&self, data: &[u8]) -> Vec<u8> {
        let mut hasher = Sha256::new();
        hasher.update(data);
        hasher.finalize().to_vec()
    }
    
    fn algorithm_name(&self) -> &'static str {
        "SHA-256"
    }
    
    fn output_size(&self) -> usize {
        32 // 256 bits = 32 bytes
    }
}

/// SHA-512 hash implementation
#[derive(Debug, Clone)]
pub struct Sha512Hash;

impl Hash for Sha512Hash {
    fn hash(&self, data: &[u8]) -> Vec<u8> {
        let mut hasher = Sha512::new();
        hasher.update(data);
        hasher.finalize().to_vec()
    }
    
    fn algorithm_name(&self) -> &'static str {
        "SHA-512"
    }
    
    fn output_size(&self) -> usize {
        64 // 512 bits = 64 bytes
    }
}

/// BLAKE3 hash implementation - quantum-resistant
#[derive(Debug, Clone)]
pub struct Blake3Hash;

impl Hash for Blake3Hash {
    fn hash(&self, data: &[u8]) -> Vec<u8> {
        let hash = blake3::hash(data);
        hash.as_bytes().to_vec()
    }
    
    fn algorithm_name(&self) -> &'static str {
        "BLAKE3"
    }
    
    fn output_size(&self) -> usize {
        32 // 256 bits = 32 bytes
    }
}

/// Double SHA-256 hash implementation (used in Bitcoin)
#[derive(Debug, Clone)]
pub struct DoubleSha256Hash;

impl Hash for DoubleSha256Hash {
    fn hash(&self, data: &[u8]) -> Vec<u8> {
        let mut hasher1 = Sha256::new();
        hasher1.update(data);
        let first_hash = hasher1.finalize();
        
        let mut hasher2 = Sha256::new();
        hasher2.update(&first_hash);
        hasher2.finalize().to_vec()
    }
    
    fn algorithm_name(&self) -> &'static str {
        "Double-SHA-256"
    }
    
    fn output_size(&self) -> usize {
        32 // 256 bits = 32 bytes
    }
}

/// Available hash algorithms in supernovaHash
#[derive(Debug, Clone)]
pub enum HashAlgorithm {
    Sha256(Sha256Hash),
    Sha512(Sha512Hash),
    Blake3(Blake3Hash),
    DoubleSha256(DoubleSha256Hash),
}

impl Hash for HashAlgorithm {
    fn hash(&self, data: &[u8]) -> Vec<u8> {
        match self {
            HashAlgorithm::Sha256(h) => h.hash(data),
            HashAlgorithm::Sha512(h) => h.hash(data),
            HashAlgorithm::Blake3(h) => h.hash(data),
            HashAlgorithm::DoubleSha256(h) => h.hash(data),
        }
    }
    
    fn algorithm_name(&self) -> &'static str {
        match self {
            HashAlgorithm::Sha256(h) => h.algorithm_name(),
            HashAlgorithm::Sha512(h) => h.algorithm_name(),
            HashAlgorithm::Blake3(h) => h.algorithm_name(),
            HashAlgorithm::DoubleSha256(h) => h.algorithm_name(),
        }
    }
    
    fn output_size(&self) -> usize {
        match self {
            HashAlgorithm::Sha256(h) => h.output_size(),
            HashAlgorithm::Sha512(h) => h.output_size(),
            HashAlgorithm::Blake3(h) => h.output_size(),
            HashAlgorithm::DoubleSha256(h) => h.output_size(),
        }
    }
}

/// supernovaHash - A composite hash that combines multiple algorithms 
/// for increased quantum resistance
#[derive(Debug, Clone)]
pub struct supernovaHash {
    /// The primary hash algorithm
    pub primary: HashAlgorithm,
    /// The secondary hash algorithm for additional security
    pub secondary: HashAlgorithm,
}

impl supernovaHash {
    /// Create a new supernovaHash with default algorithms (BLAKE3 + SHA-256)
    pub fn new() -> Self {
        Self {
            primary: HashAlgorithm::Blake3(Blake3Hash),
            secondary: HashAlgorithm::Sha256(Sha256Hash),
        }
    }
    
    /// Create a new supernovaHash with custom algorithms
    pub fn with_algorithms(primary: HashAlgorithm, secondary: HashAlgorithm) -> Self {
        Self {
            primary,
            secondary,
        }
    }
}

impl Default for supernovaHash {
    fn default() -> Self {
        Self::new()
    }
}

impl Hash for supernovaHash {
    fn hash(&self, data: &[u8]) -> Vec<u8> {
        // Hash with primary algorithm
        let primary_hash = self.primary.hash(data);
        
        // Hash the primary result with the secondary algorithm
        let secondary_hash = self.secondary.hash(&primary_hash);
        
        // XOR the two hashes together to combine them
        // If the output sizes are different, use the smaller one
        let min_size = std::cmp::min(primary_hash.len(), secondary_hash.len());
        let mut combined = Vec::with_capacity(min_size);
        
        for i in 0..min_size {
            combined.push(primary_hash[i] ^ secondary_hash[i]);
        }
        
        combined
    }
    
    fn algorithm_name(&self) -> &'static str {
        "supernovaHash"
    }
    
    fn output_size(&self) -> usize {
        std::cmp::min(self.primary.output_size(), self.secondary.output_size())
    }
}

/// Convenience function for double SHA-256 hash (Bitcoin compatible)
pub fn double_sha256(data: &[u8]) -> Vec<u8> {
    let hasher = DoubleSha256Hash;
    hasher.hash(data)
}

/// Convert a hash to hexadecimal string
pub fn hash_to_hex(hash: &[u8]) -> String {
    hex::encode(hash)
}

/// A 256-bit hash
#[derive(Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CryptoHash([u8; 32]);

impl CryptoHash {
    /// Create a new hash from bytes
    pub fn new(bytes: [u8; 32]) -> Self {
        CryptoHash(bytes)
    }
    
    /// Get the bytes of the hash
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
    
    /// Convert to a hex string
    pub fn to_hex(&self) -> String {
        hex::encode(&self.0)
    }
    
    /// Create from a hex string
    pub fn from_hex(hex_str: &str) -> Result<Self, hex::FromHexError> {
        let bytes = hex::decode(hex_str)?;
        if bytes.len() != 32 {
            return Err(hex::FromHexError::InvalidStringLength);
        }
        let mut arr = [0u8; 32];
        arr.copy_from_slice(&bytes);
        Ok(CryptoHash(arr))
    }
}

impl fmt::Debug for CryptoHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "CryptoHash({})", self.to_hex())
    }
}

impl fmt::Display for CryptoHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_hex())
    }
}

impl Default for CryptoHash {
    fn default() -> Self {
        CryptoHash([0u8; 32])
    }
}

impl AsRef<[u8]> for CryptoHash {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

impl From<[u8; 32]> for CryptoHash {
    fn from(bytes: [u8; 32]) -> Self {
        CryptoHash(bytes)
    }
}

impl From<CryptoHash> for [u8; 32] {
    fn from(hash: CryptoHash) -> Self {
        hash.0
    }
}

/// Compute SHA256(data)
pub fn sha256(data: &[u8]) -> CryptoHash {
    let hash = Sha256::digest(data);
    let mut result = [0u8; 32];
    result.copy_from_slice(&hash);
    CryptoHash(result)
}

/// Compute SHA256(SHA256(data))
pub fn hash256(data: &[u8]) -> [u8; 32] {
    let first_hash = Sha256::digest(data);
    let second_hash = Sha256::digest(&first_hash);
    let mut result = [0u8; 32];
    result.copy_from_slice(&second_hash);
    result
}

/// Compute RIPEMD160(SHA256(data))
pub fn hash160(data: &[u8]) -> [u8; 20] {
    use ripemd::{Ripemd160, Digest as RipemdDigest};
    
    let sha256_hash = Sha256::digest(data);
    let ripemd_hash = Ripemd160::digest(&sha256_hash);
    
    let mut result = [0u8; 20];
    result.copy_from_slice(&ripemd_hash);
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_sha256() {
        let hasher = Sha256Hash;
        let data = b"Hello, supernova!";
        let hash = hasher.hash(data);
        
        // Known SHA-256 hash for the input
        assert_eq!(hash.len(), 32);
    }
    
    #[test]
    fn test_blake3() {
        let hasher = Blake3Hash;
        let data = b"Hello, supernova!";
        let hash = hasher.hash(data);
        
        assert_eq!(hash.len(), 32);
    }
    
    #[test]
    fn test_double_sha256() {
        let hasher = DoubleSha256Hash;
        let data = b"Hello, supernova!";
        let hash = hasher.hash(data);
        
        assert_eq!(hash.len(), 32);
    }
    
    #[test]
    fn test_supernova_hash() {
        let hasher = supernovaHash::new();
        let data = b"Hello, supernova!";
        let hash = hasher.hash(data);
        
        assert_eq!(hash.len(), 32);
    }
    
    #[test]
    fn test_hash_multiple() {
        let hasher = Sha256Hash;
        let data1 = b"Hello";
        let data2 = b", supernova!";
        
        let hash1 = hasher.hash_multiple(&[data1, data2]);
        let hash2 = hasher.hash(b"Hello, supernova!");
        
        assert_eq!(hash1, hash2);
    }
} 