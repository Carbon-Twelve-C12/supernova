//! Bloom Filter Optimization for SPV and Bandwidth Reduction
//!
//! This module implements optimized bloom filters for efficient transaction
//! and block filtering, enabling lightweight SPV clients and reduced bandwidth.

use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// Simple hash function for bloom filters (using FNV-1a as fallback)
/// In production, this should use Murmur3, but we'll use a simple hash for now
mod bloom_hash {
    pub fn hash(data: &[u8], seed: u32) -> u64 {
        // FNV-1a hash with seed variation
        let mut hash: u64 = 0xcbf29ce484222325 ^ (seed as u64);
        for &byte in data {
            hash ^= byte as u64;
            hash = hash.wrapping_mul(0x100000001b3);
        }
        hash
    }
}

/// Bloom filter error types
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BloomFilterError {
    InvalidFalsePositiveRate,
    InvalidSize,
    InvalidHashCount,
    SerializationError(String),
}

impl std::fmt::Display for BloomFilterError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BloomFilterError::InvalidFalsePositiveRate => {
                write!(f, "False positive rate must be between 0.0 and 1.0")
            }
            BloomFilterError::InvalidSize => write!(f, "Filter size must be greater than 0"),
            BloomFilterError::InvalidHashCount => {
                write!(f, "Hash count must be greater than 0")
            }
            BloomFilterError::SerializationError(msg) => {
                write!(f, "Serialization error: {}", msg)
            }
        }
    }
}

impl std::error::Error for BloomFilterError {}

/// Base bloom filter implementation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SupernovaBloomFilter {
    /// Bit array (compressed for network transmission)
    bits: Vec<u8>,
    /// Number of hash functions
    hash_count: u32,
    /// Number of bits in the filter
    bit_count: usize,
    /// Number of elements added
    element_count: usize,
    /// Tweak for hash function variation
    tweak: u32,
}

impl SupernovaBloomFilter {
    /// Calculate optimal filter size given expected elements and false positive rate
    /// Formula: m = -(n * ln(p)) / (ln(2)^2)
    /// where n = expected elements, p = false positive rate
    pub fn calculate_optimal_size(expected_elements: usize, false_positive_rate: f64) -> usize {
        if expected_elements == 0 || false_positive_rate <= 0.0 || false_positive_rate >= 1.0 {
            return 1024; // Default size
        }

        let n = expected_elements as f64;
        let p = false_positive_rate;
        let m = -(n * p.ln()) / (2.0_f64.ln().powi(2));
        m.ceil() as usize
    }

    /// Calculate optimal number of hash functions
    /// Formula: k = (m / n) * ln(2)
    /// where m = filter size, n = expected elements
    pub fn calculate_optimal_hash_count(filter_size: usize, expected_elements: usize) -> u32 {
        if expected_elements == 0 {
            return 1;
        }

        let m = filter_size as f64;
        let n = expected_elements as f64;
        let k = (m / n) * 2.0_f64.ln();
        k.ceil().max(1.0).min(50.0) as u32 // Cap at 50 hash functions
    }

    /// Create a new bloom filter
    pub fn new(
        expected_elements: usize,
        false_positive_rate: f64,
        tweak: u32,
    ) -> Result<Self, BloomFilterError> {
        if false_positive_rate <= 0.0 || false_positive_rate >= 1.0 {
            return Err(BloomFilterError::InvalidFalsePositiveRate);
        }

        let bit_count = Self::calculate_optimal_size(expected_elements, false_positive_rate);
        let hash_count = Self::calculate_optimal_hash_count(bit_count, expected_elements);
        let byte_count = (bit_count + 7) / 8; // Round up to bytes
        let bits = vec![0u8; byte_count];

        Ok(Self {
            bits,
            hash_count,
            bit_count,
            element_count: 0,
            tweak,
        })
    }

    /// Create bloom filter with explicit size and hash count
    pub fn with_size(
        bit_count: usize,
        hash_count: u32,
        tweak: u32,
    ) -> Result<Self, BloomFilterError> {
        if bit_count == 0 {
            return Err(BloomFilterError::InvalidSize);
        }
        if hash_count == 0 {
            return Err(BloomFilterError::InvalidHashCount);
        }

        let byte_count = (bit_count + 7) / 8;
        let bits = vec![0u8; byte_count];

        Ok(Self {
            bits,
            hash_count,
            bit_count,
            element_count: 0,
            tweak,
        })
    }

    /// Add an element to the filter
    pub fn add<T: AsRef<[u8]>>(&mut self, element: T) {
        let data = element.as_ref();
        for i in 0..self.hash_count {
            let hash = self.hash(data, i);
            let bit_index = (hash % self.bit_count as u64) as usize;
            let byte_index = bit_index / 8;
            let bit_offset = bit_index % 8;

            if byte_index < self.bits.len() {
                self.bits[byte_index] |= 1u8 << bit_offset;
            }
        }
        self.element_count += 1;
    }

    /// Check if an element might be in the filter
    pub fn contains<T: AsRef<[u8]>>(&self, element: T) -> bool {
        let data = element.as_ref();
        for i in 0..self.hash_count {
            let hash = self.hash(data, i);
            let bit_index = (hash % self.bit_count as u64) as usize;
            let byte_index = bit_index / 8;
            let bit_offset = bit_index % 8;

            if byte_index >= self.bits.len() {
                return false;
            }

            if (self.bits[byte_index] & (1u8 << bit_offset)) == 0 {
                return false;
            }
        }
        true
    }

    /// Merge another bloom filter into this one (OR operation)
    pub fn merge(&mut self, other: &Self) -> Result<(), BloomFilterError> {
        if self.bit_count != other.bit_count {
            return Err(BloomFilterError::InvalidSize);
        }
        if self.hash_count != other.hash_count {
            return Err(BloomFilterError::InvalidHashCount);
        }

        for (byte, other_byte) in self.bits.iter_mut().zip(other.bits.iter()) {
            *byte |= *other_byte;
        }

        self.element_count += other.element_count;
        Ok(())
    }

    /// Compress filter for network transmission (removes trailing zeros)
    pub fn compress(&self) -> Vec<u8> {
        let mut compressed = self.bits.clone();
        
        // Remove trailing zero bytes
        while let Some(&0) = compressed.last() {
            compressed.pop();
        }

        compressed
    }

    /// Decompress filter from network transmission
    pub fn decompress(compressed: &[u8], bit_count: usize) -> Vec<u8> {
        let byte_count = (bit_count + 7) / 8;
        let mut bits = vec![0u8; byte_count];
        
        let copy_len = compressed.len().min(byte_count);
        bits[..copy_len].copy_from_slice(&compressed[..copy_len]);
        
        bits
    }

    /// Hash function with seed variation
    fn hash(&self, data: &[u8], hash_index: u32) -> u64 {
        let seed = self.tweak.wrapping_add(hash_index);
        bloom_hash::hash(data, seed)
    }

    /// Get filter statistics
    pub fn stats(&self) -> BloomFilterStats {
        let estimated_false_positive_rate = if self.element_count > 0 {
            let k = self.hash_count as f64;
            let m = self.bit_count as f64;
            let n = self.element_count as f64;
            (1.0 - (-k * n / m).exp()).powf(k)
        } else {
            0.0
        };

        BloomFilterStats {
            bit_count: self.bit_count,
            hash_count: self.hash_count,
            element_count: self.element_count,
            estimated_false_positive_rate,
            compressed_size: self.compress().len(),
        }
    }

    /// Get raw bits (for testing)
    pub fn bits(&self) -> &[u8] {
        &self.bits
    }

    /// Get bit count
    pub fn bit_count(&self) -> usize {
        self.bit_count
    }

    /// Get hash count
    pub fn hash_count(&self) -> u32 {
        self.hash_count
    }

    /// Get element count
    pub fn element_count(&self) -> usize {
        self.element_count
    }

    /// Clear the filter
    pub fn clear(&mut self) {
        self.bits.fill(0);
        self.element_count = 0;
    }
}

/// Bloom filter statistics
#[derive(Debug, Clone)]
pub struct BloomFilterStats {
    pub bit_count: usize,
    pub hash_count: u32,
    pub element_count: usize,
    pub estimated_false_positive_rate: f64,
    pub compressed_size: usize,
}

/// SPV Bloom Filter for lightweight clients
#[derive(Debug, Clone)]
pub struct SPVBloomFilter {
    filter: SupernovaBloomFilter,
    addresses: HashSet<Vec<u8>>,
}

impl SPVBloomFilter {
    /// Create a new SPV bloom filter
    pub fn new(expected_addresses: usize, false_positive_rate: f64) -> Result<Self, BloomFilterError> {
        let tweak = 0; // SPV filters use tweak 0
        let filter = SupernovaBloomFilter::new(expected_addresses, false_positive_rate, tweak)?;
        Ok(Self {
            filter,
            addresses: HashSet::new(),
        })
    }

    /// Add an address to the filter
    pub fn add_address(&mut self, address: &[u8]) {
        self.addresses.insert(address.to_vec());
        self.filter.add(address);
    }

    /// Check if an address might be in the filter
    pub fn contains_address(&self, address: &[u8]) -> bool {
        self.filter.contains(address)
    }

    /// Add a transaction ID to the filter
    pub fn add_transaction(&mut self, tx_id: &[u8; 32]) {
        self.filter.add(tx_id);
    }

    /// Check if a transaction might be in the filter
    pub fn contains_transaction(&self, tx_id: &[u8; 32]) -> bool {
        self.filter.contains(tx_id)
    }

    /// Get the underlying filter
    pub fn filter(&self) -> &SupernovaBloomFilter {
        &self.filter
    }

    /// Update filter (for dynamic updates)
    pub fn update(&mut self, new_addresses: Vec<Vec<u8>>) {
        self.filter.clear();
        self.addresses.clear();
        for address in new_addresses {
            self.add_address(&address);
        }
    }
}

/// Mempool Bloom Filter for transaction relay
#[derive(Debug, Clone)]
pub struct MempoolBloomFilter {
    filter: SupernovaBloomFilter,
    transaction_ids: HashSet<[u8; 32]>,
}

impl MempoolBloomFilter {
    /// Create a new mempool bloom filter
    pub fn new(expected_transactions: usize, false_positive_rate: f64) -> Result<Self, BloomFilterError> {
        let tweak = 1; // Mempool filters use tweak 1
        let filter = SupernovaBloomFilter::new(expected_transactions, false_positive_rate, tweak)?;
        Ok(Self {
            filter,
            transaction_ids: HashSet::new(),
        })
    }

    /// Add a transaction ID
    pub fn add_transaction(&mut self, tx_id: [u8; 32]) {
        self.transaction_ids.insert(tx_id);
        self.filter.add(&tx_id);
    }

    /// Check if a transaction might be in the filter
    pub fn contains_transaction(&self, tx_id: &[u8; 32]) -> bool {
        self.filter.contains(tx_id)
    }

    /// Get the underlying filter
    pub fn filter(&self) -> &SupernovaBloomFilter {
        &self.filter
    }

    /// Clear the filter
    pub fn clear(&mut self) {
        self.filter.clear();
        self.transaction_ids.clear();
    }
}

/// Address Bloom Filter for address tracking
#[derive(Debug, Clone)]
pub struct AddressBloomFilter {
    filter: SupernovaBloomFilter,
}

impl AddressBloomFilter {
    /// Create a new address bloom filter
    pub fn new(expected_addresses: usize, false_positive_rate: f64) -> Result<Self, BloomFilterError> {
        let tweak = 2; // Address filters use tweak 2
        let filter = SupernovaBloomFilter::new(expected_addresses, false_positive_rate, tweak)?;
        Ok(Self { filter })
    }

    /// Add an address
    pub fn add_address(&mut self, address: &[u8]) {
        self.filter.add(address);
    }

    /// Check if an address might be in the filter
    pub fn contains_address(&self, address: &[u8]) -> bool {
        self.filter.contains(address)
    }

    /// Get the underlying filter
    pub fn filter(&self) -> &SupernovaBloomFilter {
        &self.filter
    }
}

/// Lightning Bloom Filter for channel updates
#[derive(Debug, Clone)]
pub struct LightningBloomFilter {
    filter: SupernovaBloomFilter,
    channel_ids: HashSet<[u8; 32]>,
}

impl LightningBloomFilter {
    /// Create a new Lightning bloom filter
    pub fn new(expected_channels: usize, false_positive_rate: f64) -> Result<Self, BloomFilterError> {
        let tweak = 3; // Lightning filters use tweak 3
        let filter = SupernovaBloomFilter::new(expected_channels, false_positive_rate, tweak)?;
        Ok(Self {
            filter,
            channel_ids: HashSet::new(),
        })
    }

    /// Add a channel ID
    pub fn add_channel(&mut self, channel_id: [u8; 32]) {
        self.channel_ids.insert(channel_id);
        self.filter.add(&channel_id);
    }

    /// Check if a channel might be in the filter
    pub fn contains_channel(&self, channel_id: &[u8; 32]) -> bool {
        self.filter.contains(channel_id)
    }

    /// Get the underlying filter
    pub fn filter(&self) -> &SupernovaBloomFilter {
        &self.filter
    }
}

/// Network message types for bloom filter protocol
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FilterMessage {
    /// Load a bloom filter
    FilterLoad {
        filter: Vec<u8>,
        bit_count: usize,
        hash_count: u32,
        tweak: u32,
    },
    /// Add element to filter
    FilterAdd {
        element: Vec<u8>,
    },
    /// Clear the filter
    FilterClear,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bloom_filter_false_positive_rate() {
        let mut filter = SupernovaBloomFilter::new(1000, 0.001, 0).unwrap();
        
        // Add 1000 elements
        for i in 0i32..1000 {
            filter.add(&i.to_le_bytes());
        }

        let stats = filter.stats();
        assert!(stats.estimated_false_positive_rate < 0.002); // Should be close to 0.1%
    }

    #[test]
    fn test_optimal_size_calculation() {
        let size = SupernovaBloomFilter::calculate_optimal_size(1000, 0.001);
        assert!(size > 0);
        
        // Larger expected elements should require larger filter
        let size_large = SupernovaBloomFilter::calculate_optimal_size(10000, 0.001);
        assert!(size_large > size);
        
        // Lower false positive rate should require larger filter
        let size_low_fp = SupernovaBloomFilter::calculate_optimal_size(1000, 0.0001);
        assert!(size_low_fp > size);
    }

    #[test]
    fn test_filter_compression() {
        let mut filter = SupernovaBloomFilter::new(100, 0.01, 0).unwrap();
        
        // Add some elements
        for i in 0i32..50 {
            filter.add(&i.to_le_bytes());
        }

        let compressed = filter.compress();
        assert!(compressed.len() <= filter.bits().len());
    }

    #[test]
    fn test_filter_merge() {
        let mut filter1 = SupernovaBloomFilter::new(100, 0.01, 0).unwrap();
        let mut filter2 = SupernovaBloomFilter::new(100, 0.01, 0).unwrap();

        // Add different elements to each filter
        for i in 0i32..50 {
            filter1.add(&i.to_le_bytes());
        }
        for i in 50i32..100 {
            filter2.add(&i.to_le_bytes());
        }

        // Merge filters
        filter1.merge(&filter2).unwrap();

        // Both sets should be contained
        for i in 0i32..100 {
            assert!(filter1.contains(&i.to_le_bytes()));
        }
    }

    #[test]
    fn test_spv_client_filtering() {
        let mut spv_filter = SPVBloomFilter::new(100, 0.001).unwrap();

        // Add addresses
        let addr1 = b"address1";
        let addr2 = b"address2";
        spv_filter.add_address(addr1);
        spv_filter.add_address(addr2);

        // Should contain added addresses
        assert!(spv_filter.contains_address(addr1));
        assert!(spv_filter.contains_address(addr2));

        // Add transaction
        let tx_id = [1u8; 32];
        spv_filter.add_transaction(&tx_id);
        assert!(spv_filter.contains_transaction(&tx_id));
    }

    #[test]
    fn test_mempool_relay_filtering() {
        let mut mempool_filter = MempoolBloomFilter::new(1000, 0.01).unwrap();

        // Add transactions
        let tx1 = [1u8; 32];
        let tx2 = [2u8; 32];
        mempool_filter.add_transaction(tx1);
        mempool_filter.add_transaction(tx2);

        // Should contain added transactions
        assert!(mempool_filter.contains_transaction(&tx1));
        assert!(mempool_filter.contains_transaction(&tx2));

        // Clear filter
        mempool_filter.clear();
        assert!(!mempool_filter.contains_transaction(&tx1));
    }
}

