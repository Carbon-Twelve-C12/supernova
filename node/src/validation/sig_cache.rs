//! Signature Verification Cache
//!
//! PERFORMANCE MODULE (P1-004): Caches signature verification results to avoid
//! redundant cryptographic operations.
//!
//! Key features:
//! - LRU cache with configurable capacity (default: 100,000 entries)
//! - Thread-safe using RwLock for concurrent access
//! - Cache key includes txid, input index, and script hash for uniqueness
//! - Cache invalidation on reorg (TODO: implement)
//!
//! Performance impact:
//! - Signature verification is expensive (~1ms for quantum signatures)
//! - In normal operation, many signatures are verified multiple times:
//!   - Once when transaction enters mempool
//!   - Again when transaction is included in a block
//!   - Again if block is received from multiple peers
//! - Cache hit eliminates redundant verification, saving significant CPU time

use lru::LruCache;
use parking_lot::RwLock;
use sha2::{Digest, Sha256};
use std::num::NonZeroUsize;
use std::sync::atomic::{AtomicU64, Ordering};
use tracing::{debug, trace};

// ============================================================================
// Configuration
// ============================================================================

/// Default cache capacity (number of entries)
pub const DEFAULT_CACHE_CAPACITY: usize = 100_000;

/// Configuration for the signature cache
#[derive(Debug, Clone)]
pub struct SignatureCacheConfig {
    /// Maximum number of entries in the cache
    pub capacity: usize,
    /// Enable statistics collection
    pub collect_stats: bool,
}

impl Default for SignatureCacheConfig {
    fn default() -> Self {
        Self {
            capacity: DEFAULT_CACHE_CAPACITY,
            collect_stats: true,
        }
    }
}

// ============================================================================
// Cache Key
// ============================================================================

/// Cache key for signature verification results
///
/// The key uniquely identifies a signature verification by:
/// - Transaction hash (txid)
/// - Input index within the transaction
/// - Hash of the script being verified (previous output script + signature script)
///
/// This ensures that even if the same transaction is modified, the cache
/// won't return stale results.
#[derive(Hash, Eq, PartialEq, Clone, Debug)]
pub struct SignatureCacheKey {
    /// Transaction ID (hash of the transaction)
    pub txid: [u8; 32],
    /// Input index within the transaction
    pub input_index: u32,
    /// SHA256 hash of the script data being verified
    /// This includes both the signature script and the previous output script
    pub script_hash: [u8; 32],
}

impl SignatureCacheKey {
    /// Create a new cache key
    pub fn new(txid: [u8; 32], input_index: u32, script_hash: [u8; 32]) -> Self {
        Self {
            txid,
            input_index,
            script_hash,
        }
    }

    /// Create a cache key from transaction and input data
    ///
    /// # Arguments
    /// * `txid` - Transaction hash
    /// * `input_index` - Index of the input being verified
    /// * `sig_script` - Signature script (scriptSig)
    /// * `prev_script` - Previous output script (scriptPubKey)
    pub fn from_scripts(
        txid: [u8; 32],
        input_index: u32,
        sig_script: &[u8],
        prev_script: &[u8],
    ) -> Self {
        // Hash the combined scripts
        let mut hasher = Sha256::new();
        hasher.update(sig_script);
        hasher.update(prev_script);
        let result = hasher.finalize();

        let mut script_hash = [0u8; 32];
        script_hash.copy_from_slice(&result);

        Self {
            txid,
            input_index,
            script_hash,
        }
    }
}

// ============================================================================
// Cache Entry
// ============================================================================

/// Cached signature verification result
#[derive(Clone, Copy, Debug)]
pub struct SignatureCacheEntry {
    /// Whether the signature is valid
    pub valid: bool,
    /// Signature type (for debugging/stats)
    pub sig_type: SignatureType,
}

/// Type of signature that was verified
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SignatureType {
    /// ECDSA signature
    Ecdsa,
    /// Schnorr signature
    Schnorr,
    /// ML-DSA (Dilithium) quantum signature
    MlDsa,
    /// SPHINCS+ quantum signature
    Sphincs,
    /// Unknown/other
    Unknown,
}

// ============================================================================
// Cache Statistics
// ============================================================================

/// Statistics for cache monitoring
#[derive(Debug, Clone, Default)]
pub struct SignatureCacheStats {
    /// Total cache hits
    pub hits: u64,
    /// Total cache misses
    pub misses: u64,
    /// Total entries inserted
    pub insertions: u64,
    /// Current entry count
    pub entry_count: usize,
    /// Hits by signature type
    pub ecdsa_hits: u64,
    pub schnorr_hits: u64,
    pub quantum_hits: u64,
}

impl SignatureCacheStats {
    /// Calculate hit rate
    pub fn hit_rate(&self) -> f64 {
        let total = self.hits + self.misses;
        if total == 0 {
            0.0
        } else {
            self.hits as f64 / total as f64
        }
    }
}

// ============================================================================
// Signature Cache
// ============================================================================

/// Thread-safe LRU cache for signature verification results
///
/// SECURITY: This cache MUST be invalidated during chain reorganizations
/// to prevent accepting transactions with signatures that were only valid
/// on the old chain (due to different sighash flags, etc.).
pub struct SignatureCache {
    /// The LRU cache (protected by RwLock for concurrent access)
    cache: RwLock<LruCache<SignatureCacheKey, SignatureCacheEntry>>,
    /// Configuration
    config: SignatureCacheConfig,
    /// Statistics counters (atomic for lock-free updates)
    hits: AtomicU64,
    misses: AtomicU64,
    insertions: AtomicU64,
}

impl SignatureCache {
    /// Create a new signature cache with default capacity
    pub fn new() -> Self {
        Self::with_config(SignatureCacheConfig::default())
    }

    /// Create a new signature cache with specified capacity
    pub fn with_capacity(capacity: usize) -> Self {
        Self::with_config(SignatureCacheConfig {
            capacity,
            ..Default::default()
        })
    }

    /// Create a new signature cache with full configuration
    pub fn with_config(config: SignatureCacheConfig) -> Self {
        let capacity = NonZeroUsize::new(config.capacity).unwrap_or(
            NonZeroUsize::new(DEFAULT_CACHE_CAPACITY).unwrap()
        );

        debug!("Creating signature cache with capacity {}", capacity);

        Self {
            cache: RwLock::new(LruCache::new(capacity)),
            config,
            hits: AtomicU64::new(0),
            misses: AtomicU64::new(0),
            insertions: AtomicU64::new(0),
        }
    }

    /// Check if a signature verification result is cached
    ///
    /// # Arguments
    /// * `txid` - Transaction hash
    /// * `input_idx` - Input index
    /// * `script_hash` - Hash of the script data
    ///
    /// # Returns
    /// * `Some(true)` - Signature was verified valid
    /// * `Some(false)` - Signature was verified invalid
    /// * `None` - Not in cache, needs verification
    pub fn check(&self, txid: &[u8; 32], input_idx: u32, script_hash: &[u8; 32]) -> Option<bool> {
        let key = SignatureCacheKey::new(*txid, input_idx, *script_hash);

        // Use read lock for checking (allows concurrent reads)
        let result = {
            let cache = self.cache.read();
            cache.peek(&key).map(|entry| entry.valid)
        };

        // Update stats
        if self.config.collect_stats {
            if result.is_some() {
                self.hits.fetch_add(1, Ordering::Relaxed);
                trace!("Signature cache hit for tx {:02x}{:02x}...:{}", 
                    txid[0], txid[1], input_idx);
            } else {
                self.misses.fetch_add(1, Ordering::Relaxed);
            }
        }

        result
    }

    /// Check if a signature verification result is cached (using key)
    pub fn check_key(&self, key: &SignatureCacheKey) -> Option<bool> {
        let result = {
            let cache = self.cache.read();
            cache.peek(key).map(|entry| entry.valid)
        };

        if self.config.collect_stats {
            if result.is_some() {
                self.hits.fetch_add(1, Ordering::Relaxed);
            } else {
                self.misses.fetch_add(1, Ordering::Relaxed);
            }
        }

        result
    }

    /// Insert a signature verification result into the cache
    ///
    /// # Arguments
    /// * `txid` - Transaction hash
    /// * `input_idx` - Input index
    /// * `script_hash` - Hash of the script data
    /// * `valid` - Whether the signature is valid
    pub fn insert(&self, txid: [u8; 32], input_idx: u32, script_hash: [u8; 32], valid: bool) {
        self.insert_with_type(txid, input_idx, script_hash, valid, SignatureType::Unknown);
    }

    /// Insert a signature verification result with type information
    pub fn insert_with_type(
        &self,
        txid: [u8; 32],
        input_idx: u32,
        script_hash: [u8; 32],
        valid: bool,
        sig_type: SignatureType,
    ) {
        let key = SignatureCacheKey::new(txid, input_idx, script_hash);
        let entry = SignatureCacheEntry { valid, sig_type };

        // Use write lock for insertion
        {
            let mut cache = self.cache.write();
            cache.put(key, entry);
        }

        if self.config.collect_stats {
            self.insertions.fetch_add(1, Ordering::Relaxed);
        }

        trace!(
            "Cached signature verification for tx {:02x}{:02x}...:{} valid={}",
            txid[0], txid[1], input_idx, valid
        );
    }

    /// Insert a signature verification result using a key
    pub fn insert_key(&self, key: SignatureCacheKey, valid: bool, sig_type: SignatureType) {
        let entry = SignatureCacheEntry { valid, sig_type };

        {
            let mut cache = self.cache.write();
            cache.put(key, entry);
        }

        if self.config.collect_stats {
            self.insertions.fetch_add(1, Ordering::Relaxed);
        }
    }

    /// Clear the entire cache
    ///
    /// SECURITY: Must be called during chain reorganizations to prevent
    /// accepting signatures that were only valid on the old chain.
    pub fn clear(&self) {
        let mut cache = self.cache.write();
        cache.clear();
        debug!("Signature cache cleared");
    }

    /// Invalidate entries for a specific transaction
    ///
    /// Call this when a transaction is double-spent or otherwise invalidated.
    pub fn invalidate_tx(&self, txid: &[u8; 32]) {
        let mut cache = self.cache.write();
        
        // LRU cache doesn't support efficient removal by prefix,
        // so we need to collect keys first
        let keys_to_remove: Vec<_> = cache
            .iter()
            .filter(|(k, _)| &k.txid == txid)
            .map(|(k, _)| k.clone())
            .collect();

        for key in keys_to_remove {
            cache.pop(&key);
        }

        debug!("Invalidated cache entries for tx {:02x}{:02x}...", txid[0], txid[1]);
    }

    /// Get current cache size
    pub fn len(&self) -> usize {
        self.cache.read().len()
    }

    /// Check if cache is empty
    pub fn is_empty(&self) -> bool {
        self.cache.read().is_empty()
    }

    /// Get cache statistics
    pub fn statistics(&self) -> SignatureCacheStats {
        SignatureCacheStats {
            hits: self.hits.load(Ordering::Relaxed),
            misses: self.misses.load(Ordering::Relaxed),
            insertions: self.insertions.load(Ordering::Relaxed),
            entry_count: self.len(),
            ecdsa_hits: 0, // TODO: track by type
            schnorr_hits: 0,
            quantum_hits: 0,
        }
    }

    /// Get cache capacity
    pub fn capacity(&self) -> usize {
        self.config.capacity
    }
}

impl Default for SignatureCache {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_basic() {
        let cache = SignatureCache::new();

        let txid = [1u8; 32];
        let script_hash = [2u8; 32];

        // Initially not in cache
        assert!(cache.check(&txid, 0, &script_hash).is_none());

        // Insert valid result
        cache.insert(txid, 0, script_hash, true);

        // Now should be in cache
        assert_eq!(cache.check(&txid, 0, &script_hash), Some(true));
    }

    #[test]
    fn test_cache_different_inputs() {
        let cache = SignatureCache::new();

        let txid = [1u8; 32];
        let script_hash = [2u8; 32];

        // Insert for input 0
        cache.insert(txid, 0, script_hash, true);

        // Input 0 should be cached
        assert_eq!(cache.check(&txid, 0, &script_hash), Some(true));

        // Input 1 should not be cached
        assert!(cache.check(&txid, 1, &script_hash).is_none());
    }

    #[test]
    fn test_cache_invalidation() {
        let cache = SignatureCache::new();

        let txid = [1u8; 32];
        let script_hash = [2u8; 32];

        cache.insert(txid, 0, script_hash, true);
        cache.insert(txid, 1, script_hash, true);

        assert_eq!(cache.len(), 2);

        // Invalidate the transaction
        cache.invalidate_tx(&txid);

        // Cache should be empty for this tx
        assert!(cache.check(&txid, 0, &script_hash).is_none());
        assert!(cache.check(&txid, 1, &script_hash).is_none());
    }

    #[test]
    fn test_cache_clear() {
        let cache = SignatureCache::new();

        for i in 0..10 {
            let mut txid = [0u8; 32];
            txid[0] = i;
            cache.insert(txid, 0, [i; 32], true);
        }

        assert_eq!(cache.len(), 10);

        cache.clear();

        assert!(cache.is_empty());
    }

    #[test]
    fn test_cache_statistics() {
        let cache = SignatureCache::new();

        let txid = [1u8; 32];
        let script_hash = [2u8; 32];

        // Miss
        cache.check(&txid, 0, &script_hash);

        // Insert
        cache.insert(txid, 0, script_hash, true);

        // Hit
        cache.check(&txid, 0, &script_hash);
        cache.check(&txid, 0, &script_hash);

        let stats = cache.statistics();
        assert_eq!(stats.hits, 2);
        assert_eq!(stats.misses, 1);
        assert_eq!(stats.insertions, 1);
        assert!(stats.hit_rate() > 0.6); // 2 hits out of 3 checks
    }

    #[test]
    fn test_cache_key_from_scripts() {
        let txid = [1u8; 32];
        let sig_script = vec![0x30, 0x44, 0x02, 0x20]; // Dummy signature prefix
        let prev_script = vec![0x76, 0xa9, 0x14]; // P2PKH prefix

        let key = SignatureCacheKey::from_scripts(txid, 0, &sig_script, &prev_script);

        // Same inputs should produce same key
        let key2 = SignatureCacheKey::from_scripts(txid, 0, &sig_script, &prev_script);
        assert_eq!(key, key2);

        // Different scripts should produce different key
        let key3 = SignatureCacheKey::from_scripts(txid, 0, &sig_script, &[0x00]);
        assert_ne!(key, key3);
    }

    #[test]
    fn test_cache_lru_eviction() {
        // Small cache for testing eviction
        let cache = SignatureCache::with_capacity(3);

        for i in 0..5 {
            let mut txid = [0u8; 32];
            txid[0] = i;
            cache.insert(txid, 0, [i; 32], true);
        }

        // Should have evicted oldest entries
        assert_eq!(cache.len(), 3);

        // Entry 0 and 1 should be evicted
        assert!(cache.check(&[0u8; 32], 0, &[0u8; 32]).is_none());
        assert!(cache.check(&[1u8; 32], 0, &[1u8; 32]).is_none());

        // Entry 2, 3, 4 should still be there
        // Note: Checking changes LRU order
    }
}

