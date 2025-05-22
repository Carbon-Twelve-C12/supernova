/// Utility functions and data structures for SuperNova blockchain
/// 
/// Provides various utility tools used throughout the codebase,
/// including Merkle trees, serialization helpers, and common algorithms.

// Utility module for Supernova blockchain

// Export modules
pub mod merkle;
pub mod ascii_art;
pub mod hex;
pub mod metrics;
pub mod logging;

// Re-export commonly used utilities
pub use merkle::{MerkleTree, MerkleProof, MerkleError};
pub use hex::{hex_to_bytes, bytes_to_hex};

use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha20Rng;
use sha2::{Sha256, Digest};

/// Generate a random seed from a string
pub fn seed_from_string(input: &str) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(input.as_bytes());
    let result = hasher.finalize();
    let mut seed = [0u8; 32];
    seed.copy_from_slice(&result);
    seed
}

/// Generate a deterministic random number from a seed and counter
pub fn deterministic_random(seed: &[u8; 32], counter: u64) -> f64 {
    let counter_bytes = counter.to_le_bytes();
    let mut combined = Vec::with_capacity(seed.len() + counter_bytes.len());
    combined.extend_from_slice(seed);
    combined.extend_from_slice(&counter_bytes);
    
    let mut hasher = Sha256::new();
    hasher.update(&combined);
    let hash = hasher.finalize();
    
    let mut seed_array = [0u8; 32];
    seed_array.copy_from_slice(&hash);
    
    let mut rng = ChaCha20Rng::from_seed(seed_array);
    rng.gen::<f64>()
}

/// Format a large number with thousand separators
pub fn format_with_commas(num: u64) -> String {
    let num_str = num.to_string();
    let mut result = String::new();
    let mut count = 0;
    
    for c in num_str.chars().rev() {
        if count > 0 && count % 3 == 0 {
            result.push(',');
        }
        result.push(c);
        count += 1;
    }
    
    result.chars().rev().collect()
}

/// Truncate a hash for display
pub fn truncate_hash(hash: &[u8]) -> String {
    let hash_hex = bytes_to_hex(hash);
    if hash_hex.len() <= 8 {
        hash_hex
    } else {
        format!("{}...{}", &hash_hex[0..4], &hash_hex[hash_hex.len()-4..])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_seed_from_string() {
        let seed1 = seed_from_string("test1");
        let seed2 = seed_from_string("test2");
        
        assert_ne!(seed1, seed2);
        assert_eq!(seed1, seed_from_string("test1"));
    }
    
    #[test]
    fn test_deterministic_random() {
        let seed = seed_from_string("test_seed");
        
        let rand1 = deterministic_random(&seed, 1);
        let rand2 = deterministic_random(&seed, 2);
        
        assert_ne!(rand1, rand2);
        assert_eq!(rand1, deterministic_random(&seed, 1));
    }
    
    #[test]
    fn test_format_with_commas() {
        assert_eq!(format_with_commas(1000), "1,000");
        assert_eq!(format_with_commas(1000000), "1,000,000");
        assert_eq!(format_with_commas(123456789), "123,456,789");
        assert_eq!(format_with_commas(0), "0");
    }
    
    #[test]
    fn test_truncate_hash() {
        let hash = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16];
        let truncated = truncate_hash(&hash);
        assert_eq!(truncated, "0102...0f10");
    }
}