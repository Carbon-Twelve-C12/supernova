use std::hash::{Hash, Hasher};
use siphasher::sip::SipHasher;
use std::marker::PhantomData;

/// A simple bloom filter implementation for fast, probabilistic set membership testing
#[derive(Debug, Clone)]
pub struct BloomFilter<T> {
    /// The bit array that represents the bloom filter
    bits: Vec<u8>,
    
    /// Number of hash functions to use
    hash_functions: usize,
    
    /// Number of bits in the filter
    num_bits: usize,
    
    /// Number of elements inserted
    num_elements: usize,
    
    /// Phantom data for type parameter
    _marker: PhantomData<T>,
}

impl<T: Hash> BloomFilter<T> {
    /// Create a new bloom filter with the specified capacity and false positive rate
    pub fn new(expected_elements: usize, false_positive_rate: f64) -> Self {
        // Calculate optimal filter size and number of hash functions
        let bits_per_elem = Self::calc_bits_per_elem(false_positive_rate);
        let num_bits = expected_elements * bits_per_elem;
        let num_bits = if num_bits == 0 { 1 } else { num_bits };
        
        let hash_functions = Self::calc_hash_functions(bits_per_elem);
        
        // We need to round up to the nearest multiple of 8 for the bytes
        let num_bytes = (num_bits + 7) / 8;
        
        Self {
            bits: vec![0; num_bytes],
            hash_functions,
            num_bits,
            num_elements: 0,
            _marker: PhantomData,
        }
    }
    
    /// Calculate the optimal number of bits per element based on desired false positive rate
    fn calc_bits_per_elem(false_positive_rate: f64) -> usize {
        let ln2_2 = std::f64::consts::LN_2 * std::f64::consts::LN_2;
        (-1.0 * false_positive_rate.ln() / ln2_2).ceil() as usize
    }
    
    /// Calculate the optimal number of hash functions based on bits per element
    fn calc_hash_functions(bits_per_elem: usize) -> usize {
        let ln2 = std::f64::consts::LN_2;
        (bits_per_elem as f64 * ln2).ceil() as usize
    }
    
    /// Insert an element into the bloom filter
    pub fn insert(&mut self, item: &T) {
        for i in 0..self.hash_functions {
            let bit_pos = self.get_bit_position(item, i);
            self.set_bit(bit_pos);
        }
        self.num_elements += 1;
    }
    
    /// Check if an element may be in the bloom filter
    /// Note: False positives are possible, but false negatives are not
    pub fn contains(&self, item: &T) -> bool {
        for i in 0..self.hash_functions {
            let bit_pos = self.get_bit_position(item, i);
            if !self.get_bit(bit_pos) {
                return false;
            }
        }
        true
    }
    
    /// Calculate the bit position for the given item and hash function index
    fn get_bit_position(&self, item: &T, hash_index: usize) -> usize {
        let mut hasher = SipHasher::new_with_keys(0x0123456789ABCDEF, hash_index as u64);
        item.hash(&mut hasher);
        let hash = hasher.finish();
        
        (hash as usize) % self.num_bits
    }
    
    /// Set a bit in the filter
    fn set_bit(&mut self, pos: usize) {
        let byte_pos = pos / 8;
        let bit_pos = pos % 8;
        self.bits[byte_pos] |= 1 << bit_pos;
    }
    
    /// Check if a bit is set in the filter
    fn get_bit(&self, pos: usize) -> bool {
        let byte_pos = pos / 8;
        let bit_pos = pos % 8;
        (self.bits[byte_pos] & (1 << bit_pos)) != 0
    }
    
    /// Get the estimated false positive rate based on current load
    pub fn false_positive_rate(&self) -> f64 {
        if self.num_elements == 0 {
            return 0.0;
        }
        
        let n = self.num_elements as f64;
        let m = self.num_bits as f64;
        let k = self.hash_functions as f64;
        
        (1.0 - (1.0 - 1.0/m).powf(k * n)).powf(k)
    }
    
    /// Get the number of elements inserted
    pub fn len(&self) -> usize {
        self.num_elements
    }
    
    /// Check if the filter is empty
    pub fn is_empty(&self) -> bool {
        self.num_elements == 0
    }
    
    /// Clear the filter
    pub fn clear(&mut self) {
        for byte in &mut self.bits {
            *byte = 0;
        }
        self.num_elements = 0;
    }
    
    /// Estimate the memory usage of the filter in bytes
    pub fn memory_usage(&self) -> usize {
        // Approximate overhead of the struct plus the bits vector
        std::mem::size_of::<Self>() + self.bits.len()
    }
}

/// A bloom filter optimized for UTXO and transaction set membership testing
#[derive(Debug, Clone)]
pub struct ChainBloomFilter {
    /// Internal bloom filter for byte arrays (transaction/outpoint hashes)
    filter: BloomFilter<Vec<u8>>,
}

impl ChainBloomFilter {
    /// Create a new chain bloom filter with the specified capacity and false positive rate
    pub fn new(expected_elements: usize, false_positive_rate: f64) -> Self {
        Self {
            filter: BloomFilter::new(expected_elements, false_positive_rate),
        }
    }
    
    /// Insert a transaction hash
    pub fn insert_txid(&mut self, txid: &[u8; 32]) {
        self.filter.insert(&txid.to_vec());
    }
    
    /// Check if a transaction hash may be in the filter
    pub fn contains_txid(&self, txid: &[u8; 32]) -> bool {
        self.filter.contains(&txid.to_vec())
    }
    
    /// Insert an outpoint (txid + index)
    pub fn insert_outpoint(&mut self, txid: &[u8; 32], index: u32) {
        let mut data = txid.to_vec();
        data.extend_from_slice(&index.to_le_bytes());
        self.filter.insert(&data);
    }
    
    /// Check if an outpoint may be in the filter
    pub fn contains_outpoint(&self, txid: &[u8; 32], index: u32) -> bool {
        let mut data = txid.to_vec();
        data.extend_from_slice(&index.to_le_bytes());
        self.filter.contains(&data)
    }
    
    /// Get the estimated false positive rate
    pub fn false_positive_rate(&self) -> f64 {
        self.filter.false_positive_rate()
    }
    
    /// Get the number of elements in the filter
    pub fn len(&self) -> usize {
        self.filter.len()
    }
    
    /// Check if the filter is empty
    pub fn is_empty(&self) -> bool {
        self.filter.is_empty()
    }
    
    /// Clear the filter
    pub fn clear(&mut self) {
        self.filter.clear();
    }
    
    /// Estimate the memory usage of the filter in bytes
    pub fn memory_usage(&self) -> usize {
        self.filter.memory_usage()
    }
}

/// High-performance UTXO set filter for fast membership checking
#[derive(Debug, Clone)]
pub struct UtxoSetFilter {
    /// Bloom filter for UTXO presence testing
    utxo_filter: ChainBloomFilter,
    
    /// Bloom filter for spent outputs (prevents false positives by checking both)
    spent_filter: ChainBloomFilter,
}

impl UtxoSetFilter {
    /// Create a new UTXO set filter
    pub fn new(expected_utxos: usize, expected_spent: usize, false_positive_rate: f64) -> Self {
        Self {
            utxo_filter: ChainBloomFilter::new(expected_utxos, false_positive_rate),
            spent_filter: ChainBloomFilter::new(expected_spent, false_positive_rate),
        }
    }
    
    /// Add a UTXO to the filter
    pub fn add_utxo(&mut self, txid: &[u8; 32], index: u32) {
        self.utxo_filter.insert_outpoint(txid, index);
    }
    
    /// Mark a UTXO as spent
    pub fn mark_spent(&mut self, txid: &[u8; 32], index: u32) {
        self.spent_filter.insert_outpoint(txid, index);
    }
    
    /// Check if a UTXO may exist (not spent)
    pub fn may_exist(&self, txid: &[u8; 32], index: u32) -> bool {
        // UTXO might exist if it's in the UTXO filter and not in the spent filter
        self.utxo_filter.contains_outpoint(txid, index) && 
        !self.spent_filter.contains_outpoint(txid, index)
    }
    
    /// Reset the filters
    pub fn reset(&mut self) {
        self.utxo_filter.clear();
        self.spent_filter.clear();
    }
    
    /// Estimate the memory usage of both filters in bytes
    pub fn memory_usage(&self) -> usize {
        self.utxo_filter.memory_usage() + self.spent_filter.memory_usage()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_bloom_filter_basic() {
        let mut filter = BloomFilter::<String>::new(1000, 0.01);
        
        // Insert some values
        filter.insert(&"hello".to_string());
        filter.insert(&"world".to_string());
        filter.insert(&"test".to_string());
        
        // Check for presence
        assert!(filter.contains(&"hello".to_string()));
        assert!(filter.contains(&"world".to_string()));
        assert!(filter.contains(&"test".to_string()));
        
        // Check for absence
        assert!(!filter.contains(&"missing".to_string()));
        
        // Verify length
        assert_eq!(filter.len(), 3);
    }
    
    #[test]
    fn test_chain_bloom_filter() {
        let mut filter = ChainBloomFilter::new(1000, 0.01);
        
        // Create some test transaction IDs
        let txid1 = [1u8; 32];
        let txid2 = [2u8; 32];
        
        // Insert transactions
        filter.insert_txid(&txid1);
        
        // Test presence
        assert!(filter.contains_txid(&txid1));
        assert!(!filter.contains_txid(&txid2));
        
        // Test outpoints
        filter.insert_outpoint(&txid1, 0);
        assert!(filter.contains_outpoint(&txid1, 0));
        assert!(!filter.contains_outpoint(&txid1, 1));
    }
    
    #[test]
    fn test_utxo_set_filter() {
        let mut filter = UtxoSetFilter::new(1000, 500, 0.01);
        
        // Create test outpoints
        let txid1 = [1u8; 32];
        let txid2 = [2u8; 32];
        
        // Add UTXOs
        filter.add_utxo(&txid1, 0);
        filter.add_utxo(&txid1, 1);
        filter.add_utxo(&txid2, 0);
        
        // Check existence
        assert!(filter.may_exist(&txid1, 0));
        assert!(filter.may_exist(&txid1, 1));
        assert!(filter.may_exist(&txid2, 0));
        assert!(!filter.may_exist(&txid2, 1));
        
        // Mark spent
        filter.mark_spent(&txid1, 0);
        
        // Verify spent status
        assert!(!filter.may_exist(&txid1, 0));
        assert!(filter.may_exist(&txid1, 1));
    }
} 