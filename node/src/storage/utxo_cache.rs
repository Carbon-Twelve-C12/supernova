//! UTXO LRU Cache Implementation
//!
//! SCALABILITY MODULE (P1-003): High-performance LRU cache for hot UTXOs.
//!
//! This module provides a memory-efficient caching layer for UTXO lookups.
//! Hot UTXOs (frequently accessed, recently created) are kept in memory
//! while cold UTXOs are read from disk on demand.
//!
//! Key features:
//! - LRU eviction with configurable memory limit
//! - Write-through caching with dirty tracking
//! - Batch flush operations for efficiency
//! - Cache statistics and hit rate monitoring
//!
//! Design based on Bitcoin Core's CCoinsViewCache pattern.

use dashmap::DashMap;
use lru::LruCache;
use parking_lot::{Mutex, RwLock};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::num::NonZeroUsize;
use std::path::Path;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tracing::{debug, error, info, trace, warn};

use super::utxo_set::{OutPoint, UnspentOutput, UtxoCommitment, UtxoSet};
use super::StorageError;

// ============================================================================
// Configuration
// ============================================================================

/// Configuration for the UTXO cache
#[derive(Debug, Clone)]
pub struct UtxoCacheConfig {
    /// Maximum memory usage for the cache in bytes (default: 512 MB)
    pub max_memory_bytes: usize,
    /// Number of entries before triggering flush (default: 10,000)
    pub flush_threshold: usize,
    /// Enable write-back caching (lazy writes) vs write-through
    pub write_back: bool,
    /// Maximum time between flushes in seconds
    pub max_flush_interval_secs: u64,
    /// Target cache hit rate (for monitoring)
    pub target_hit_rate: f64,
    /// Enable cache statistics collection
    pub collect_stats: bool,
}

impl Default for UtxoCacheConfig {
    fn default() -> Self {
        Self {
            max_memory_bytes: 512 * 1024 * 1024, // 512 MB
            flush_threshold: 10_000,
            write_back: true,
            max_flush_interval_secs: 300, // 5 minutes
            target_hit_rate: 0.90,        // 90% target
            collect_stats: true,
        }
    }
}

// ============================================================================
// Cache Entry
// ============================================================================

/// State of a cache entry
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CacheEntryState {
    /// Entry exists and matches database
    Clean,
    /// Entry has been modified and needs to be written
    Dirty,
    /// Entry has been deleted (tombstone)
    Deleted,
    /// Entry is fresh (newly created, not in database)
    Fresh,
}

/// A cached UTXO entry
#[derive(Debug, Clone)]
pub struct CacheEntry {
    /// The UTXO data (None if deleted)
    pub utxo: Option<UnspentOutput>,
    /// Entry state
    pub state: CacheEntryState,
    /// Estimated memory usage of this entry
    pub memory_size: usize,
    /// Last access time (for debugging)
    pub last_access: Instant,
}

impl CacheEntry {
    /// Create a new clean entry
    pub fn clean(utxo: UnspentOutput) -> Self {
        let memory_size = Self::estimate_size(&utxo);
        Self {
            utxo: Some(utxo),
            state: CacheEntryState::Clean,
            memory_size,
            last_access: Instant::now(),
        }
    }

    /// Create a new fresh entry (not yet in database)
    pub fn fresh(utxo: UnspentOutput) -> Self {
        let memory_size = Self::estimate_size(&utxo);
        Self {
            utxo: Some(utxo),
            state: CacheEntryState::Fresh,
            memory_size,
            last_access: Instant::now(),
        }
    }

    /// Create a deleted entry (tombstone)
    pub fn deleted() -> Self {
        Self {
            utxo: None,
            state: CacheEntryState::Deleted,
            memory_size: 64, // Minimal tombstone size
            last_access: Instant::now(),
        }
    }

    /// Estimate memory size of a UTXO entry
    fn estimate_size(utxo: &UnspentOutput) -> usize {
        // Base struct size + script size + overhead
        std::mem::size_of::<UnspentOutput>()
            + utxo.script_pubkey.len()
            + 64 // Overhead for HashMap entry, pointers, etc.
    }

    /// Check if entry needs to be written to database
    pub fn needs_flush(&self) -> bool {
        matches!(
            self.state,
            CacheEntryState::Dirty | CacheEntryState::Fresh | CacheEntryState::Deleted
        )
    }
}

// ============================================================================
// Cache Statistics
// ============================================================================

/// Statistics for cache monitoring
#[derive(Debug, Clone, Default)]
pub struct CacheStatistics {
    /// Total cache hits
    pub hits: u64,
    /// Total cache misses
    pub misses: u64,
    /// Current entry count
    pub entry_count: usize,
    /// Current memory usage in bytes
    pub memory_usage: usize,
    /// Number of dirty entries
    pub dirty_count: usize,
    /// Number of flushes performed
    pub flush_count: u64,
    /// Total entries flushed
    pub entries_flushed: u64,
    /// Average flush time in milliseconds
    pub avg_flush_time_ms: f64,
    /// Peak memory usage
    pub peak_memory: usize,
}

impl CacheStatistics {
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
// UTXO Cache
// ============================================================================

/// High-performance LRU cache for UTXO lookups
///
/// SECURITY: The cache maintains consistency with the underlying database.
/// All modifications go through the cache, which tracks dirty state.
/// Periodic flushes ensure durability without impacting performance.
pub struct UtxoCache {
    /// LRU cache for quick access ordering
    cache: Mutex<LruCache<OutPoint, CacheEntry>>,
    /// Underlying persistent storage
    db: Arc<RwLock<UtxoSet>>,
    /// Configuration
    config: UtxoCacheConfig,
    /// Current memory usage (atomic for lock-free reads)
    memory_usage: AtomicUsize,
    /// Cache statistics
    stats: RwLock<CacheStatistics>,
    /// Last flush time
    last_flush: Mutex<Instant>,
    /// Dirty entries set (for efficient flush)
    dirty_set: Mutex<HashSet<OutPoint>>,
}

impl UtxoCache {
    /// Create a new UTXO cache
    ///
    /// # Arguments
    /// * `db` - The underlying UTXO database
    /// * `config` - Cache configuration
    pub fn new(db: Arc<RwLock<UtxoSet>>, config: UtxoCacheConfig) -> Self {
        // Calculate cache capacity based on memory limit
        // Assume average entry size of ~150 bytes
        let estimated_entry_size = 150;
        let capacity = config.max_memory_bytes / estimated_entry_size;
        let capacity = NonZeroUsize::new(capacity.max(1000)).unwrap();

        info!(
            "Initializing UTXO cache: max_memory={}MB, capacity=~{} entries",
            config.max_memory_bytes / (1024 * 1024),
            capacity
        );

        Self {
            cache: Mutex::new(LruCache::new(capacity)),
            db,
            config,
            memory_usage: AtomicUsize::new(0),
            stats: RwLock::new(CacheStatistics::default()),
            last_flush: Mutex::new(Instant::now()),
            dirty_set: Mutex::new(HashSet::new()),
        }
    }

    /// Get a UTXO by outpoint
    ///
    /// Checks cache first, then falls back to database.
    /// Automatically promotes entries in LRU order.
    pub fn get(&self, outpoint: &OutPoint) -> Option<UnspentOutput> {
        // Try cache first
        {
            let mut cache = self.cache.lock();
            if let Some(entry) = cache.get(outpoint) {
                // Update stats
                if self.config.collect_stats {
                    self.stats.write().hits += 1;
                }

                // Return cached value
                return match entry.state {
                    CacheEntryState::Deleted => None,
                    _ => entry.utxo.clone(),
                };
            }
        }

        // Cache miss - fetch from database
        if self.config.collect_stats {
            self.stats.write().misses += 1;
        }

        // Read from database
        let db = self.db.read();
        let utxo = db.get(outpoint)?;

        // Cache the result
        self.insert_clean(*outpoint, utxo.clone());

        Some(utxo)
    }

    /// Add a new UTXO to the cache
    ///
    /// The UTXO is marked as fresh (not yet in database) and will be
    /// written during the next flush.
    pub fn add(&self, outpoint: OutPoint, utxo: UnspentOutput) {
        let entry = CacheEntry::fresh(utxo);
        let memory_size = entry.memory_size;

        {
            let mut cache = self.cache.lock();
            
            // Check if we need to evict entries
            self.maybe_evict(&mut cache, memory_size);

            // Insert the new entry
            if let Some(old_entry) = cache.put(outpoint, entry) {
                // Subtract old entry's memory
                self.memory_usage
                    .fetch_sub(old_entry.memory_size, Ordering::Relaxed);
            }

            // Add new entry's memory
            self.memory_usage.fetch_add(memory_size, Ordering::Relaxed);

            // Track dirty entry
            self.dirty_set.lock().insert(outpoint);
        }

        // Update stats
        if self.config.collect_stats {
            let mut stats = self.stats.write();
            stats.entry_count = self.entry_count();
            stats.memory_usage = self.memory_usage();
            stats.dirty_count = self.dirty_count();
            stats.peak_memory = stats.peak_memory.max(stats.memory_usage);
        }

        // Check if we should flush
        self.maybe_auto_flush();
    }

    /// Mark a UTXO as spent (remove it)
    ///
    /// The UTXO is marked as deleted in the cache. The actual deletion
    /// from the database happens during flush.
    ///
    /// # Returns
    /// * `Some(UnspentOutput)` - The spent UTXO
    /// * `None` - If the UTXO doesn't exist
    pub fn spend(&self, outpoint: &OutPoint) -> Option<UnspentOutput> {
        // First, try to get the UTXO (this ensures it's in cache)
        let utxo = self.get(outpoint)?;

        // Mark as deleted in cache
        {
            let mut cache = self.cache.lock();
            let entry = CacheEntry::deleted();
            let memory_change = entry.memory_size;

            if let Some(old_entry) = cache.put(*outpoint, entry) {
                // Subtract old entry's memory, add tombstone memory
                let old_size = old_entry.memory_size;
                if old_size > memory_change {
                    self.memory_usage
                        .fetch_sub(old_size - memory_change, Ordering::Relaxed);
                } else {
                    self.memory_usage
                        .fetch_add(memory_change - old_size, Ordering::Relaxed);
                }
            } else {
                self.memory_usage.fetch_add(memory_change, Ordering::Relaxed);
            }

            // Track dirty entry
            self.dirty_set.lock().insert(*outpoint);
        }

        // Update stats
        if self.config.collect_stats {
            let mut stats = self.stats.write();
            stats.dirty_count = self.dirty_count();
        }

        // Check if we should flush
        self.maybe_auto_flush();

        Some(utxo)
    }

    /// Check if a UTXO exists
    pub fn contains(&self, outpoint: &OutPoint) -> bool {
        self.get(outpoint).is_some()
    }

    /// Flush all dirty entries to the database
    ///
    /// # Returns
    /// * `Ok(usize)` - Number of entries flushed
    /// * `Err` - If flush fails
    pub fn flush(&self) -> Result<usize, StorageError> {
        let start = Instant::now();
        let mut flushed = 0;

        // Get dirty entries
        let dirty_entries: Vec<OutPoint> = {
            let dirty_set = self.dirty_set.lock();
            dirty_set.iter().cloned().collect()
        };

        if dirty_entries.is_empty() {
            return Ok(0);
        }

        debug!("Flushing {} dirty UTXO entries to database", dirty_entries.len());

        // Acquire write lock on database
        let mut db = self.db.write();

        // Process each dirty entry
        {
            let mut cache = self.cache.lock();
            let mut dirty_set = self.dirty_set.lock();

            for outpoint in dirty_entries {
                if let Some(entry) = cache.peek(&outpoint) {
                    match entry.state {
                        CacheEntryState::Fresh | CacheEntryState::Dirty => {
                            // Write to database
                            if let Some(ref utxo) = entry.utxo {
                                db.add(outpoint, utxo.clone());
                                flushed += 1;
                            }
                        }
                        CacheEntryState::Deleted => {
                            // Remove from database
                            db.remove(&outpoint);
                            flushed += 1;
                            // Also remove tombstone from cache to free memory
                            if let Some(old) = cache.pop(&outpoint) {
                                self.memory_usage.fetch_sub(old.memory_size, Ordering::Relaxed);
                            }
                        }
                        CacheEntryState::Clean => {
                            // Nothing to do
                        }
                    }

                    // Mark as clean (if still in cache)
                    if let Some(entry) = cache.peek_mut(&outpoint) {
                        if entry.state != CacheEntryState::Deleted {
                            entry.state = CacheEntryState::Clean;
                        }
                    }
                }

                dirty_set.remove(&outpoint);
            }
        }

        // Save to disk
        db.save()?;

        // Update flush time
        *self.last_flush.lock() = Instant::now();

        // Update stats
        if self.config.collect_stats {
            let elapsed = start.elapsed();
            let mut stats = self.stats.write();
            stats.flush_count += 1;
            stats.entries_flushed += flushed as u64;
            stats.dirty_count = self.dirty_count();
            // Update average flush time (exponential moving average)
            let flush_ms = elapsed.as_secs_f64() * 1000.0;
            stats.avg_flush_time_ms = stats.avg_flush_time_ms * 0.9 + flush_ms * 0.1;
        }

        debug!("Flushed {} entries in {:?}", flushed, start.elapsed());

        Ok(flushed)
    }

    /// Get current memory usage in bytes
    pub fn memory_usage(&self) -> usize {
        self.memory_usage.load(Ordering::Relaxed)
    }

    /// Get number of entries in cache
    pub fn entry_count(&self) -> usize {
        self.cache.lock().len()
    }

    /// Get number of dirty entries
    pub fn dirty_count(&self) -> usize {
        self.dirty_set.lock().len()
    }

    /// Get cache statistics
    pub fn statistics(&self) -> CacheStatistics {
        let stats = self.stats.read();
        CacheStatistics {
            hits: stats.hits,
            misses: stats.misses,
            entry_count: self.entry_count(),
            memory_usage: self.memory_usage(),
            dirty_count: self.dirty_count(),
            flush_count: stats.flush_count,
            entries_flushed: stats.entries_flushed,
            avg_flush_time_ms: stats.avg_flush_time_ms,
            peak_memory: stats.peak_memory,
        }
    }

    /// Clear all entries from the cache (doesn't affect database)
    pub fn clear(&self) {
        let mut cache = self.cache.lock();
        cache.clear();
        self.memory_usage.store(0, Ordering::Relaxed);
        self.dirty_set.lock().clear();

        if self.config.collect_stats {
            let mut stats = self.stats.write();
            stats.entry_count = 0;
            stats.memory_usage = 0;
            stats.dirty_count = 0;
        }
    }

    // ========================================================================
    // Internal Methods
    // ========================================================================

    /// Insert a clean entry (from database read)
    fn insert_clean(&self, outpoint: OutPoint, utxo: UnspentOutput) {
        let entry = CacheEntry::clean(utxo);
        let memory_size = entry.memory_size;

        let mut cache = self.cache.lock();

        // Check if we need to evict entries
        self.maybe_evict(&mut cache, memory_size);

        // Insert the entry
        if let Some(old_entry) = cache.put(outpoint, entry) {
            self.memory_usage
                .fetch_sub(old_entry.memory_size, Ordering::Relaxed);
        }
        self.memory_usage.fetch_add(memory_size, Ordering::Relaxed);
    }

    /// Evict entries if necessary to stay under memory limit
    fn maybe_evict(&self, cache: &mut LruCache<OutPoint, CacheEntry>, needed: usize) {
        let current = self.memory_usage.load(Ordering::Relaxed);
        let limit = self.config.max_memory_bytes;

        if current + needed <= limit {
            return;
        }

        trace!(
            "Cache memory pressure: {} + {} > {}, evicting",
            current,
            needed,
            limit
        );

        // Evict LRU entries until we have enough space
        // But don't evict dirty entries
        let target = limit - needed;
        let mut evicted = 0;

        while self.memory_usage.load(Ordering::Relaxed) > target {
            // Peek at LRU entry without removing
            let lru_key = {
                let mut iter = cache.iter();
                // Find first clean entry to evict
                loop {
                    if let Some((key, entry)) = iter.next() {
                        if !entry.needs_flush() {
                            break Some(*key);
                        }
                    } else {
                        break None;
                    }
                }
            };

            if let Some(key) = lru_key {
                if let Some(entry) = cache.pop(&key) {
                    self.memory_usage
                        .fetch_sub(entry.memory_size, Ordering::Relaxed);
                    evicted += 1;
                }
            } else {
                // No clean entries to evict, need to flush first
                warn!(
                    "Cache full with only dirty entries, consider increasing cache size or flush frequency"
                );
                break;
            }
        }

        if evicted > 0 {
            debug!("Evicted {} clean entries from cache", evicted);
        }
    }

    /// Check if we should auto-flush based on dirty count or time
    fn maybe_auto_flush(&self) {
        let dirty_count = self.dirty_count();
        let last_flush = *self.last_flush.lock();
        let elapsed = last_flush.elapsed();

        let should_flush = dirty_count >= self.config.flush_threshold
            || elapsed.as_secs() >= self.config.max_flush_interval_secs;

        if should_flush && self.config.write_back {
            if let Err(e) = self.flush() {
                error!("Auto-flush failed: {}", e);
            }
        }
    }
}

// ============================================================================
// UTXO Snapshot Support
// ============================================================================

/// UTXO snapshot for fast initial sync (AssumeUTXO-style)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UtxoSnapshot {
    /// Commitment of the UTXO set at snapshot time
    pub commitment: UtxoCommitment,
    /// Block hash at snapshot height
    pub block_hash: [u8; 32],
    /// All UTXO entries
    pub entries: Vec<(OutPoint, UnspentOutput)>,
    /// Snapshot creation timestamp
    pub created_at: u64,
    /// Snapshot format version
    pub version: u32,
}

impl UtxoSnapshot {
    /// Current snapshot format version
    pub const VERSION: u32 = 1;

    /// Create a snapshot from a UTXO cache
    pub fn create(
        cache: &UtxoCache,
        block_hash: [u8; 32],
        height: u64,
    ) -> Result<Self, StorageError> {
        // First, flush any dirty entries
        cache.flush()?;

        // Get all entries from the database
        let db = cache.db.read();
        let entries = db.get_all();

        // Create commitment
        let mut total_value = 0u64;
        for (_, utxo) in &entries {
            total_value = total_value.saturating_add(utxo.value);
        }

        // Calculate hash
        let mut hasher = Sha256::new();
        for (outpoint, utxo) in &entries {
            hasher.update(&outpoint.txid);
            hasher.update(&outpoint.vout.to_le_bytes());
            hasher.update(&utxo.value.to_le_bytes());
            hasher.update(&utxo.script_pubkey);
        }
        let hash_result = hasher.finalize();
        let mut hash = [0u8; 32];
        hash.copy_from_slice(&hash_result);

        let commitment = UtxoCommitment {
            hash,
            height,
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            utxo_count: entries.len(),
            total_value,
        };

        let created_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        Ok(Self {
            commitment,
            block_hash,
            entries,
            created_at,
            version: Self::VERSION,
        })
    }

    /// Verify snapshot integrity
    pub fn verify(&self) -> bool {
        // Recalculate hash
        let mut hasher = Sha256::new();
        let mut total_value = 0u64;

        for (outpoint, utxo) in &self.entries {
            hasher.update(&outpoint.txid);
            hasher.update(&outpoint.vout.to_le_bytes());
            hasher.update(&utxo.value.to_le_bytes());
            hasher.update(&utxo.script_pubkey);
            total_value = total_value.saturating_add(utxo.value);
        }

        let hash_result = hasher.finalize();
        let mut calculated_hash = [0u8; 32];
        calculated_hash.copy_from_slice(&hash_result);

        // Verify hash and count
        calculated_hash == self.commitment.hash
            && self.entries.len() == self.commitment.utxo_count
            && total_value == self.commitment.total_value
    }

    /// Get snapshot size in bytes (estimated)
    pub fn size_bytes(&self) -> usize {
        // Header + entries
        std::mem::size_of::<Self>()
            + self
                .entries
                .iter()
                .map(|(_, utxo)| {
                    std::mem::size_of::<OutPoint>()
                        + std::mem::size_of::<UnspentOutput>()
                        + utxo.script_pubkey.len()
                })
                .sum::<usize>()
    }
}

/// Load a UTXO set from a snapshot
pub fn load_from_snapshot(
    db: &mut UtxoSet,
    snapshot: &UtxoSnapshot,
) -> Result<(), StorageError> {
    // Verify snapshot first
    if !snapshot.verify() {
        return Err(StorageError::DatabaseError(
            "Snapshot verification failed: hash mismatch".to_string(),
        ));
    }

    info!(
        "Loading UTXO snapshot with {} entries at height {}",
        snapshot.entries.len(),
        snapshot.commitment.height
    );

    // Clear existing data
    db.clear();

    // Load entries
    for (outpoint, utxo) in &snapshot.entries {
        db.add(*outpoint, utxo.clone());
    }

    // Save to disk
    db.save()?;

    info!(
        "Loaded {} UTXOs from snapshot, total value: {}",
        snapshot.entries.len(),
        snapshot.commitment.total_value
    );

    Ok(())
}

// ============================================================================
// Pruning Configuration
// ============================================================================

/// Configuration for UTXO/block pruning
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PruningConfig {
    /// Whether pruning is enabled
    pub enabled: bool,
    /// Keep this many recent blocks unpruned
    pub keep_blocks: u64,
    /// Target total storage size in GB
    pub target_size_gb: u64,
    /// Minimum blocks to keep regardless of size target
    pub min_blocks: u64,
}

impl Default for PruningConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            keep_blocks: 550,     // ~1 day of blocks at 2.5 min/block
            target_size_gb: 100,  // 100 GB target
            min_blocks: 288,      // ~12 hours minimum
        }
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn create_test_utxo(value: u64, height: u64) -> UnspentOutput {
        UnspentOutput {
            txid: [0u8; 32],
            vout: 0,
            value,
            script_pubkey: vec![1, 2, 3, 4],
            height,
            is_coinbase: false,
        }
    }

    fn create_test_cache() -> (UtxoCache, tempfile::TempDir) {
        let temp_dir = tempdir().unwrap();
        let db_path = temp_dir.path().join("utxo.db");
        let db = Arc::new(RwLock::new(UtxoSet::new(&db_path).unwrap()));

        let config = UtxoCacheConfig {
            max_memory_bytes: 1024 * 1024, // 1 MB
            flush_threshold: 100,
            write_back: true,
            max_flush_interval_secs: 60,
            target_hit_rate: 0.90,
            collect_stats: true,
        };

        (UtxoCache::new(db, config), temp_dir)
    }

    #[test]
    fn test_cache_add_get() {
        let (cache, _temp) = create_test_cache();

        // Add a UTXO
        let outpoint = OutPoint::new([1u8; 32], 0);
        let utxo = create_test_utxo(1000, 1);

        cache.add(outpoint, utxo.clone());

        // Retrieve it
        let retrieved = cache.get(&outpoint);
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().value, 1000);
    }

    #[test]
    fn test_cache_spend() {
        let (cache, _temp) = create_test_cache();

        // Add a UTXO
        let outpoint = OutPoint::new([1u8; 32], 0);
        let utxo = create_test_utxo(1000, 1);

        cache.add(outpoint, utxo);

        // Spend it
        let spent = cache.spend(&outpoint);
        assert!(spent.is_some());
        assert_eq!(spent.unwrap().value, 1000);

        // Should no longer exist
        assert!(cache.get(&outpoint).is_none());
    }

    #[test]
    fn test_cache_flush() {
        let (cache, _temp) = create_test_cache();

        // Add some UTXOs
        for i in 0..10 {
            let mut txid = [0u8; 32];
            txid[0] = i;
            let outpoint = OutPoint::new(txid, 0);
            let utxo = create_test_utxo(1000 * (i as u64 + 1), i as u64);
            cache.add(outpoint, utxo);
        }

        // Flush to database
        let flushed = cache.flush().unwrap();
        assert_eq!(flushed, 10);

        // Verify dirty count is now 0
        assert_eq!(cache.dirty_count(), 0);
    }

    #[test]
    fn test_cache_statistics() {
        let (cache, _temp) = create_test_cache();

        // Initial stats
        let stats = cache.statistics();
        assert_eq!(stats.hits, 0);
        assert_eq!(stats.misses, 0);

        // Add a UTXO
        let outpoint = OutPoint::new([1u8; 32], 0);
        let utxo = create_test_utxo(1000, 1);
        cache.add(outpoint, utxo);

        // Get (should hit)
        let _ = cache.get(&outpoint);
        let stats = cache.statistics();
        assert_eq!(stats.hits, 1);

        // Get non-existent (should miss)
        let _ = cache.get(&OutPoint::new([99u8; 32], 0));
        let stats = cache.statistics();
        assert_eq!(stats.misses, 1);

        // Check hit rate
        assert!(stats.hit_rate() > 0.0);
    }

    #[test]
    fn test_snapshot_create_verify() {
        let (cache, _temp) = create_test_cache();

        // Add some UTXOs
        for i in 0..5 {
            let mut txid = [0u8; 32];
            txid[0] = i;
            let outpoint = OutPoint::new(txid, 0);
            let utxo = create_test_utxo(1000 * (i as u64 + 1), i as u64);
            cache.add(outpoint, utxo);
        }

        // Flush to ensure all entries are in DB
        cache.flush().unwrap();

        // Create snapshot
        let snapshot = UtxoSnapshot::create(&cache, [0u8; 32], 5).unwrap();

        // Verify
        assert!(snapshot.verify());
        assert_eq!(snapshot.entries.len(), 5);
        assert_eq!(snapshot.commitment.utxo_count, 5);
        assert_eq!(snapshot.commitment.total_value, 15000); // 1000+2000+3000+4000+5000
    }

    #[test]
    fn test_cache_memory_limit() {
        let temp_dir = tempdir().unwrap();
        let db_path = temp_dir.path().join("utxo.db");
        let db = Arc::new(RwLock::new(UtxoSet::new(&db_path).unwrap()));

        // Very small cache for testing eviction
        let config = UtxoCacheConfig {
            max_memory_bytes: 2000, // Very small
            flush_threshold: 1000,
            write_back: true,
            max_flush_interval_secs: 60,
            target_hit_rate: 0.90,
            collect_stats: true,
        };

        let cache = UtxoCache::new(db, config);

        // Add many UTXOs (should trigger eviction)
        for i in 0..100 {
            let mut txid = [0u8; 32];
            txid[0] = i;
            let outpoint = OutPoint::new(txid, 0);
            let utxo = create_test_utxo(1000, i as u64);
            cache.add(outpoint, utxo);
        }

        // Memory usage should be controlled
        assert!(cache.memory_usage() <= 4000); // Allow some overhead
    }
}

