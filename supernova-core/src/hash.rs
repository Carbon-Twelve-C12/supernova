//! Hash utilities for the blockchain
//! Provides a simpler interface to the cryptographic hash functions

use crate::crypto::hash::{SupernovaHash, Blake3Hash, DoubleSha256Hash, Hash, Sha256Hash};
use sha2::{Digest, Sha256};

/// 256-bit hash value - fixed size array for blockchain use
pub type Hash256 = [u8; 32];

/// Default hash algorithm for general use in the blockchain
pub fn hash_default(data: &[u8]) -> Vec<u8> {
    let hasher = Sha256Hash;
    hasher.hash(data)
}

/// Double-SHA256 hash (Bitcoin compatible)
pub fn hash_double_sha256(data: &[u8]) -> Vec<u8> {
    let hasher = DoubleSha256Hash;
    hasher.hash(data)
}

/// Quantum-resistant hash (BLAKE3)
pub fn hash_quantum_resistant(data: &[u8]) -> Vec<u8> {
    let hasher = Blake3Hash;
    hasher.hash(data)
}

/// supernova's enhanced hash (combination of algorithms)
pub fn hash_supernova(data: &[u8]) -> Vec<u8> {
    let hasher = SupernovaHash::new();
    hasher.hash(data)
}

/// Convert a 32-byte hash into a fixed-size array
pub fn to_32_bytes(hash: &[u8]) -> [u8; 32] {
    let mut result = [0u8; 32];

    // Handle different length hashes
    if hash.len() >= 32 {
        result.copy_from_slice(&hash[..32]);
    } else {
        // If hash is shorter than 32 bytes, pad with zeros
        let hash_len = hash.len();
        result[..hash_len].copy_from_slice(hash);
    }

    result
}

/// Converts a hexadecimal string to a 32-byte array
pub fn hex_to_32_bytes(hex: &str) -> Result<[u8; 32], hex::FromHexError> {
    // Remove "0x" prefix if present
    let hex_str = hex.trim_start_matches("0x");

    let bytes = hex::decode(hex_str)?;
    let mut result = [0u8; 32];

    if bytes.len() >= 32 {
        result.copy_from_slice(&bytes[..32]);
    } else {
        // If bytes is shorter than 32 bytes, pad with zeros
        let bytes_len = bytes.len();
        result[..bytes_len].copy_from_slice(&bytes);
    }

    Ok(result)
}

/// Check if a hash meets a specified difficulty target
/// Returns true if the hash is below the target
pub fn meets_difficulty(hash: &[u8], target: &[u8]) -> bool {
    if hash.len() != target.len() {
        return false;
    }

    for i in 0..hash.len() {
        if hash[i] < target[i] {
            return true;
        } else if hash[i] > target[i] {
            return false;
        }
    }

    // Equal to target
    true
}

/// Compute SHA256(SHA256(data))
pub fn hash256(data: &[u8]) -> Hash256 {
    let first_hash = Sha256::digest(data);
    let second_hash = Sha256::digest(first_hash);

    let mut result = [0u8; 32];
    result.copy_from_slice(&second_hash);
    result
}

/// Compute RIPEMD160(SHA256(data))
pub fn hash160(data: &[u8]) -> [u8; 20] {
    use ripemd::{Digest as RipemdDigest, Ripemd160};

    let sha256_hash = Sha256::digest(data);
    let ripemd_hash = Ripemd160::digest(sha256_hash);

    let mut result = [0u8; 20];
    result.copy_from_slice(&ripemd_hash);
    result
}

/// Convert hash to hexadecimal string
pub fn hash_to_hex(hash_bytes: &[u8]) -> String {
    hex::encode(hash_bytes)
}

/// Perform double SHA-256 hash returning fixed-size array
pub fn double_sha256(data: &[u8]) -> Hash256 {
    to_32_bytes(&hash_double_sha256(data))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hash_functions() {
        let data = b"test data";

        // All hash functions should return valid output
        assert_eq!(hash_default(data).len(), 32);
        assert_eq!(hash_double_sha256(data).len(), 32);
        assert_eq!(hash_quantum_resistant(data).len(), 32);
        assert_eq!(hash_supernova(data).len(), 32);
    }

    #[test]
    fn test_to_32_bytes() {
        let hash = vec![1, 2, 3, 4, 5];
        let result = to_32_bytes(&hash);

        // First 5 bytes should match input
        assert_eq!(result[..5], [1, 2, 3, 4, 5]);
        // Rest should be zeros
        assert_eq!(result[5..], [0; 27]);
    }

    #[test]
    fn test_hex_to_32_bytes() {
        // Test valid hex string
        let result = hex_to_32_bytes("0102030405").unwrap();
        assert_eq!(result[..5], [1, 2, 3, 4, 5]);
        assert_eq!(result[5..], [0; 27]);

        // Test with 0x prefix
        let result = hex_to_32_bytes("0x0102030405").unwrap();
        assert_eq!(result[..5], [1, 2, 3, 4, 5]);

        // Test invalid hex string
        assert!(hex_to_32_bytes("not a hex string").is_err());
    }

    #[test]
    fn test_meets_difficulty() {
        let hash = [1, 0, 0, 0];
        let target = [2, 0, 0, 0];
        assert!(meets_difficulty(&hash, &target));

        let hash = [2, 0, 0, 0];
        let target = [1, 0, 0, 0];
        assert!(!meets_difficulty(&hash, &target));

        let hash = [1, 0, 0, 0];
        let target = [1, 0, 0, 0];
        assert!(meets_difficulty(&hash, &target));
    }
}
