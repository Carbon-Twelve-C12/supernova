//! Cryptographic primitives for atomic swaps
//! 
//! This module provides hash functions and other cryptographic operations
//! needed for atomic swap implementation.

use crate::atomic_swap::error::HTLCError;
use serde::{Serialize, Deserialize};
use sha2::{Sha256, Digest};
use blake3;

/// Supported hash functions for HTLCs
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum HashFunction {
    /// SHA-256 for Bitcoin compatibility
    SHA256,
    /// BLAKE3 for Supernova native operations
    BLAKE3,
    /// SHA3-256 as an alternative option
    SHA3_256,
}

/// Hash lock structure for HTLCs
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HashLock {
    /// Type of hash function used
    pub hash_type: HashFunction,
    /// The hash value (32 bytes)
    pub hash_value: [u8; 32],
    /// The preimage (only known to initiator initially)
    pub preimage: Option<[u8; 32]>,
}

impl HashLock {
    /// Create a new hash lock with a generated preimage
    pub fn new(hash_type: HashFunction) -> Result<Self, HTLCError> {
        let preimage = generate_secure_random_32();
        let hash_value = compute_hash_with_type(&preimage, &hash_type)?;
        
        Ok(Self {
            hash_type,
            hash_value,
            preimage: Some(preimage),
        })
    }
    
    /// Create a hash lock from an existing hash (no preimage)
    pub fn from_hash(hash_type: HashFunction, hash_value: [u8; 32]) -> Self {
        Self {
            hash_type,
            hash_value,
            preimage: None,
        }
    }
    
    /// Verify a preimage against this hash lock
    pub fn verify_preimage(&self, preimage: &[u8; 32]) -> Result<bool, HTLCError> {
        let computed_hash = compute_hash_with_type(preimage, &self.hash_type)?;
        Ok(computed_hash == self.hash_value)
    }
}

/// Generate cryptographically secure random 32 bytes
pub fn generate_secure_random_32() -> [u8; 32] {
    use rand::RngCore;
    let mut rng = rand::rngs::OsRng;
    let mut bytes = [0u8; 32];
    rng.fill_bytes(&mut bytes);
    bytes
}

/// Compute hash using SHA-256 (default for atomic swaps)
pub fn compute_hash(data: &[u8]) -> Result<[u8; 32], HTLCError> {
    compute_hash_with_type(data, &HashFunction::SHA256)
}

/// Compute hash with specified hash function
pub fn compute_hash_with_type(data: &[u8], hash_type: &HashFunction) -> Result<[u8; 32], HTLCError> {
    match hash_type {
        HashFunction::SHA256 => {
            let mut hasher = Sha256::new();
            hasher.update(data);
            let result = hasher.finalize();
            let mut hash = [0u8; 32];
            hash.copy_from_slice(&result);
            Ok(hash)
        },
        HashFunction::BLAKE3 => {
            let hash = blake3::hash(data);
            Ok(*hash.as_bytes())
        },
        HashFunction::SHA3_256 => {
            use sha3::{Sha3_256, Digest};
            let mut hasher = Sha3_256::new();
            hasher.update(data);
            let result = hasher.finalize();
            let mut hash = [0u8; 32];
            hash.copy_from_slice(&result);
            Ok(hash)
        },
    }
}

/// Time-lock cryptography utilities
pub mod timelock {
    use super::*;
    
    /// Compute a time-locked hash
    pub fn compute_timelock_hash(
        secret: &[u8; 32],
        timestamp: u64,
        participant_pubkey: &[u8],
    ) -> Result<[u8; 32], HTLCError> {
        let mut data = Vec::new();
        data.extend_from_slice(secret);
        data.extend_from_slice(&timestamp.to_le_bytes());
        data.extend_from_slice(participant_pubkey);
        
        compute_hash(&data)
    }
}

/// Signature adapter for bridging different signature schemes
pub mod signature_adapter {
    use super::*;
    use crate::crypto::{ECDSASignature, MLDSASignature};
    
    /// Bitcoin-compatible signature
    #[derive(Clone, Debug, Serialize, Deserialize)]
    pub struct BitcoinSignature {
        pub r: [u8; 32],
        pub s: [u8; 32],
        pub sighash_type: u8,
    }
    
    /// Supernova quantum-resistant signature types
    #[derive(Clone, Debug, Serialize, Deserialize)]
    pub enum SupernovaSignature {
        MLDSA65(MLDSASignature),
        Falcon512(crate::crypto::FalconSignature),
        SPHINCS256(crate::crypto::SPHINCSSignature),
        Hybrid {
            classical: ECDSASignature,
            quantum: Box<SupernovaSignature>,
        },
    }
    
    /// Convert Bitcoin signature format for verification
    pub fn adapt_bitcoin_signature(sig: &BitcoinSignature) -> Result<secp256k1::ecdsa::Signature, HTLCError> {
        use secp256k1::ecdsa::Signature;
        
        let mut sig_bytes = [0u8; 64];
        sig_bytes[..32].copy_from_slice(&sig.r);
        sig_bytes[32..].copy_from_slice(&sig.s);
        
        Signature::from_compact(&sig_bytes)
            .map_err(|e| HTLCError::InvalidSignature)
    }
}

/// Merkle tree utilities for batch operations
pub mod merkle {
    use super::*;
    
    /// Compute Merkle root of multiple HTLCs
    pub fn compute_htlc_merkle_root(htlc_hashes: &[[u8; 32]]) -> Result<[u8; 32], HTLCError> {
        if htlc_hashes.is_empty() {
            return Err(HTLCError::Other("Empty HTLC list".to_string()));
        }
        
        if htlc_hashes.len() == 1 {
            return Ok(htlc_hashes[0]);
        }
        
        let mut current_level = htlc_hashes.to_vec();
        
        while current_level.len() > 1 {
            let mut next_level = Vec::new();
            
            for i in (0..current_level.len()).step_by(2) {
                if i + 1 < current_level.len() {
                    let mut combined = Vec::new();
                    combined.extend_from_slice(&current_level[i]);
                    combined.extend_from_slice(&current_level[i + 1]);
                    next_level.push(compute_hash(&combined)?);
                } else {
                    // Odd number of elements, promote the last one
                    next_level.push(current_level[i]);
                }
            }
            
            current_level = next_level;
        }
        
        Ok(current_level[0])
    }
    
    /// Generate Merkle proof for an HTLC
    pub fn generate_merkle_proof(
        htlc_hashes: &[[u8; 32]],
        index: usize,
    ) -> Result<Vec<[u8; 32]>, HTLCError> {
        if index >= htlc_hashes.len() {
            return Err(HTLCError::Other("Index out of bounds".to_string()));
        }
        
        let mut proof = Vec::new();
        let mut current_index = index;
        let mut current_level = htlc_hashes.to_vec();
        
        while current_level.len() > 1 {
            let sibling_index = if current_index % 2 == 0 {
                current_index + 1
            } else {
                current_index - 1
            };
            
            if sibling_index < current_level.len() {
                proof.push(current_level[sibling_index]);
            }
            
            // Move to next level
            let mut next_level = Vec::new();
            for i in (0..current_level.len()).step_by(2) {
                if i + 1 < current_level.len() {
                    let mut combined = Vec::new();
                    combined.extend_from_slice(&current_level[i]);
                    combined.extend_from_slice(&current_level[i + 1]);
                    next_level.push(compute_hash(&combined)?);
                } else {
                    next_level.push(current_level[i]);
                }
            }
            
            current_level = next_level;
            current_index /= 2;
        }
        
        Ok(proof)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_hash_lock_creation() {
        let hash_lock = HashLock::new(HashFunction::SHA256).unwrap();
        assert!(hash_lock.preimage.is_some());
        
        // Verify the hash matches the preimage
        let verified = hash_lock.verify_preimage(&hash_lock.preimage.unwrap()).unwrap();
        assert!(verified);
    }
    
    #[test]
    fn test_different_hash_functions() {
        let data = b"test data for hashing";
        
        let sha256_hash = compute_hash_with_type(data, &HashFunction::SHA256).unwrap();
        let blake3_hash = compute_hash_with_type(data, &HashFunction::BLAKE3).unwrap();
        let sha3_hash = compute_hash_with_type(data, &HashFunction::SHA3_256).unwrap();
        
        // Different hash functions should produce different results
        assert_ne!(sha256_hash, blake3_hash);
        assert_ne!(sha256_hash, sha3_hash);
        assert_ne!(blake3_hash, sha3_hash);
    }
    
    #[test]
    fn test_secure_random_generation() {
        let random1 = generate_secure_random_32();
        let random2 = generate_secure_random_32();
        
        // Should generate different values
        assert_ne!(random1, random2);
        
        // Should be 32 bytes
        assert_eq!(random1.len(), 32);
        assert_eq!(random2.len(), 32);
    }
    
    #[test]
    fn test_merkle_root_computation() {
        let hashes = vec![
            [1u8; 32],
            [2u8; 32],
            [3u8; 32],
            [4u8; 32],
        ];
        
        let root = merkle::compute_htlc_merkle_root(&hashes).unwrap();
        assert_ne!(root, [0u8; 32]);
        
        // Single element should return itself
        let single = merkle::compute_htlc_merkle_root(&hashes[..1]).unwrap();
        assert_eq!(single, hashes[0]);
    }
    
    #[test]
    fn test_merkle_proof_generation() {
        let hashes = vec![
            [1u8; 32],
            [2u8; 32],
            [3u8; 32],
            [4u8; 32],
        ];
        
        let proof = merkle::generate_merkle_proof(&hashes, 0).unwrap();
        assert!(!proof.is_empty());
        
        // For 4 elements, we should have 2 proof elements
        assert_eq!(proof.len(), 2);
    }
} 