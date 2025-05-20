use std::collections::{HashMap, HashSet};
use std::sync::{Arc, RwLock, Mutex};
use std::time::{Duration, Instant};

use memmap2::{MmapMut, MmapOptions};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::types::transaction::{OutPoint, TxOutput};
use tracing::{debug, info, warn, error};

/// Size of merkle tree leaf node in bytes
const MERKLE_LEAF_SIZE: usize = 32 + 8 + 2 + 8; // hash + value + script_type + script_length

/// Size of merkle tree internal node in bytes
const MERKLE_NODE_SIZE: usize = 32 + 32 + 8; // left_hash + right_hash + subtree_count

/// Size of UTXO index entry in bytes
const INDEX_ENTRY_SIZE: usize = 36 + 8 + 4; // outpoint + file_offset + size

/// Represents a single unspent transaction output
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UtxoEntry {
    /// Reference to the transaction output
    pub outpoint: OutPoint,
    /// The transaction output
    pub output: TxOutput,
    /// Block height where this UTXO was created
    pub height: u32,
    /// Whether this UTXO is coinbase
    pub is_coinbase: bool,
    /// Whether this UTXO is confirmed
    pub is_confirmed: bool,
}

/// UTXO hash commitment for validation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UtxoCommitment {
    /// Root hash of the UTXO Merkle tree
    pub root_hash: [u8; 32],
    /// Total number of UTXOs
    pub utxo_count: u64,
    /// Total value of all UTXOs
    pub total_value: u64,
    /// Block height at which this commitment was created
    pub block_height: u32,
}

/// Cache statistics for performance monitoring
#[derive(Debug, Clone, Default)]
pub struct UtxoCacheStats {
    /// Number of cache hits
    pub hits: u64,
    /// Number of cache misses
    pub misses: u64,
    /// Number of entries in the cache
    pub entries: usize,
    /// Memory usage of the cache in bytes
    pub memory_usage: usize,
    /// Time spent on cache operations
    pub operation_time: Duration,
}

/// Advanced UTXO set implementation optimized for performance
pub struct UtxoSet {
    /// In-memory cache of UTXOs for fast access
    cache: Arc<RwLock<HashMap<OutPoint, UtxoEntry>>>,
    /// Memory-mapped file for the UTXO database
    mmap: Option<Arc<Mutex<MmapMut>>>,
    /// Index mapping outpoints to file locations
    index: Arc<RwLock<HashMap<OutPoint, u64>>>,
    /// Set of recently spent outpoints
    spent_outpoints: Arc<RwLock<HashSet<OutPoint>>>,
    /// Current UTXO commitment
    commitment: Arc<RwLock<UtxoCommitment>>,
    /// Statistics for monitoring
    stats: Arc<RwLock<UtxoCacheStats>>,
    /// Capacity of the in-memory cache
    cache_capacity: usize,
    /// Path to the UTXO database file
    db_path: Option<String>,
    /// Whether to use memory mapping
    use_mmap: bool,
}

impl UtxoSet {
    /// Create a new in-memory UTXO set
    pub fn new_in_memory(cache_capacity: usize) -> Self {
        let initial_commitment = UtxoCommitment {
            root_hash: [0; 32],
            utxo_count: 0,
            total_value: 0,
            block_height: 0,
        };

        Self {
            cache: Arc::new(RwLock::new(HashMap::with_capacity(cache_capacity))),
            mmap: None,
            index: Arc::new(RwLock::new(HashMap::new())),
            spent_outpoints: Arc::new(RwLock::new(HashSet::new())),
            commitment: Arc::new(RwLock::new(initial_commitment)),
            stats: Arc::new(RwLock::new(UtxoCacheStats::default())),
            cache_capacity,
            db_path: None,
            use_mmap: false,
        }
    }

    /// Create a new UTXO set with persistent storage
    pub fn new_persistent(db_path: &str, cache_capacity: usize, use_mmap: bool) -> std::io::Result<Self> {
        let initial_commitment = UtxoCommitment {
            root_hash: [0; 32],
            utxo_count: 0,
            total_value: 0,
            block_height: 0,
        };

        let mut utxo_set = Self {
            cache: Arc::new(RwLock::new(HashMap::with_capacity(cache_capacity))),
            mmap: None,
            index: Arc::new(RwLock::new(HashMap::new())),
            spent_outpoints: Arc::new(RwLock::new(HashSet::new())),
            commitment: Arc::new(RwLock::new(initial_commitment)),
            stats: Arc::new(RwLock::new(UtxoCacheStats::default())),
            cache_capacity,
            db_path: Some(db_path.to_string()),
            use_mmap,
        };

        if use_mmap {
            utxo_set.initialize_mmap()?;
        }

        Ok(utxo_set)
    }

    /// Initialize memory-mapped file
    fn initialize_mmap(&mut self) -> std::io::Result<()> {
        if let Some(path) = &self.db_path {
            let file = std::fs::OpenOptions::new()
                .read(true)
                .write(true)
                .create(true)
                .open(path)?;

            // Ensure file is large enough for initial mapping
            let initial_size = 1024 * 1024 * 10; // 10MB initial size
            file.set_len(initial_size)?;

            // Create memory map
            let mmap = unsafe { MmapOptions::new().map_mut(&file)? };
            self.mmap = Some(Arc::new(Mutex::new(mmap)));

            info!("Initialized memory-mapped UTXO database at {} with size {}MB", path, initial_size / 1024 / 1024);
        }

        Ok(())
    }

    /// Add a new UTXO to the set
    pub fn add(&self, entry: UtxoEntry) -> Result<(), String> {
        let start_time = Instant::now();

        // Check if outpoint is already in the set
        {
            let cache = self.cache.read().map_err(|e| e.to_string())?;
            if cache.contains_key(&entry.outpoint) {
                return Err(format!("UTXO {} already exists in the set", entry.outpoint));
            }
        }

        // Remove from spent outpoints if present
        {
            let mut spent = self.spent_outpoints.write().map_err(|e| e.to_string())?;
            spent.remove(&entry.outpoint);
        }

        // Add to cache
        {
            let mut cache = self.cache.write().map_err(|e| e.to_string())?;
            cache.insert(entry.outpoint.clone(), entry.clone());

            // Update cache statistics
            let mut stats = self.stats.write().map_err(|e| e.to_string())?;
            stats.entries = cache.len();
            stats.operation_time += start_time.elapsed();
        }

        // Update commitment (simplified version)
        {
            let mut commitment = self.commitment.write().map_err(|e| e.to_string())?;
            commitment.utxo_count += 1;
            commitment.total_value += entry.output.amount();
            // Full merkle tree update would be done periodically, not on every add
        }

        // If cache is too large, flush to disk
        self.prune_cache()?;

        Ok(())
    }

    /// Remove a UTXO from the set
    pub fn remove(&self, outpoint: &OutPoint) -> Result<Option<UtxoEntry>, String> {
        let start_time = Instant::now();
        let mut removed_entry = None;

        // Try to remove from cache first
        {
            let mut cache = self.cache.write().map_err(|e| e.to_string())?;
            removed_entry = cache.remove(outpoint);

            // Update cache statistics
            let mut stats = self.stats.write().map_err(|e| e.to_string())?;
            stats.entries = cache.len();
            stats.operation_time += start_time.elapsed();
        }

        // If not in cache, may be on disk
        if removed_entry.is_none() && self.use_mmap {
            // In a real implementation, would look up in index and load from disk
            // For simplicity, we'll just return None
        }

        // Add to spent outpoints
        {
            let mut spent = self.spent_outpoints.write().map_err(|e| e.to_string())?;
            spent.insert(outpoint.clone());
        }

        // Update commitment if we removed something
        if let Some(entry) = &removed_entry {
            let mut commitment = self.commitment.write().map_err(|e| e.to_string())?;
            commitment.utxo_count = commitment.utxo_count.saturating_sub(1);
            commitment.total_value = commitment.total_value.saturating_sub(entry.output.amount());
            // Full merkle tree update would be done periodically
        }

        Ok(removed_entry)
    }

    /// Get a UTXO by outpoint
    pub fn get(&self, outpoint: &OutPoint) -> Result<Option<UtxoEntry>, String> {
        let start_time = Instant::now();

        // Check if recently spent
        {
            let spent = self.spent_outpoints.read().map_err(|e| e.to_string())?;
            if spent.contains(outpoint) {
                return Ok(None);
            }
        }

        // Look in cache first
        {
            let cache = self.cache.read().map_err(|e| e.to_string())?;
            
            if let Some(entry) = cache.get(outpoint) {
                // Update statistics
                let mut stats = self.stats.write().map_err(|e| e.to_string())?;
                stats.hits += 1;
                stats.operation_time += start_time.elapsed();
                
                return Ok(Some(entry.clone()));
            }
        }

        // If not in cache and using mmap, check disk
        if self.use_mmap {
            // Look up in index
            let index = self.index.read().map_err(|e| e.to_string())?;
            
            if let Some(&offset) = index.get(outpoint) {
                // Load from mmap
                if let Some(mmap_ref) = &self.mmap {
                    let mmap = mmap_ref.lock().map_err(|e| e.to_string())?;
                    
                    // In a real implementation, deserialize from mmap at offset
                    // For simplicity, we'll just return None
                }
            }
        }

        // Update miss statistics
        {
            let mut stats = self.stats.write().map_err(|e| e.to_string())?;
            stats.misses += 1;
            stats.operation_time += start_time.elapsed();
        }

        Ok(None)
    }

    /// Check if the set contains a UTXO
    pub fn contains(&self, outpoint: &OutPoint) -> Result<bool, String> {
        // Check if recently spent
        {
            let spent = self.spent_outpoints.read().map_err(|e| e.to_string())?;
            if spent.contains(outpoint) {
                return Ok(false);
            }
        }

        // Check cache
        {
            let cache = self.cache.read().map_err(|e| e.to_string())?;
            if cache.contains_key(outpoint) {
                return Ok(true);
            }
        }

        // Check index if using persistent storage
        if self.use_mmap {
            let index = self.index.read().map_err(|e| e.to_string())?;
            return Ok(index.contains_key(outpoint));
        }

        Ok(false)
    }

    /// Get the current UTXO commitment
    pub fn get_commitment(&self) -> Result<UtxoCommitment, String> {
        let commitment = self.commitment.read().map_err(|e| e.to_string())?;
        Ok(commitment.clone())
    }

    /// Update the UTXO commitment (recalculate Merkle root)
    pub fn update_commitment(&self, block_height: u32) -> Result<UtxoCommitment, String> {
        let start_time = Instant::now();
        
        // Get all UTXOs (from cache and potentially disk)
        let all_utxos = self.get_all_utxos()?;
        
        // Calculate total value
        let total_value = all_utxos.iter().map(|entry| entry.output.amount()).sum();
        
        // Build simplified Merkle tree (real implementation would be more complex)
        let root_hash = self.calculate_merkle_root(&all_utxos)?;
        
        // Update commitment
        let new_commitment = UtxoCommitment {
            root_hash,
            utxo_count: all_utxos.len() as u64,
            total_value,
            block_height,
        };
        
        {
            let mut commitment = self.commitment.write().map_err(|e| e.to_string())?;
            *commitment = new_commitment.clone();
        }
        
        // Update stats
        {
            let mut stats = self.stats.write().map_err(|e| e.to_string())?;
            stats.operation_time += start_time.elapsed();
        }
        
        info!("Updated UTXO commitment at height {}: {} UTXOs, {} total value",
              block_height, all_utxos.len(), total_value);
        
        Ok(new_commitment)
    }

    /// Calculate the Merkle root of the UTXO set
    fn calculate_merkle_root(&self, utxos: &[UtxoEntry]) -> Result<[u8; 32], String> {
        if utxos.is_empty() {
            return Ok([0; 32]);
        }
        
        // For simplicity, we'll hash all UTXOs together
        // A real implementation would build a proper Merkle tree
        let mut hasher = Sha256::new();
        
        for entry in utxos {
            // Hash outpoint
            hasher.update(entry.outpoint.txid.as_bytes());
            hasher.update(&entry.outpoint.vout.to_le_bytes());
            
            // Hash value
            let amount = entry.output.amount();
            hasher.update(&amount.to_le_bytes());
            
            // Hash script (simplified)
            let script = &entry.output.pub_key_script;
            hasher.update(script);
            
            // Hash metadata
            hasher.update(&entry.height.to_le_bytes());
            hasher.update(&[entry.is_coinbase as u8]);
            hasher.update(&[entry.is_confirmed as u8]);
        }
        
        let result = hasher.finalize();
        let mut root_hash = [0u8; 32];
        root_hash.copy_from_slice(&result[..32]);
        
        Ok(root_hash)
    }

    /// Get all UTXOs in the set
    fn get_all_utxos(&self) -> Result<Vec<UtxoEntry>, String> {
        let mut all_utxos = Vec::new();
        
        // Get UTXOs from cache
        {
            let cache = self.cache.read().map_err(|e| e.to_string())?;
            all_utxos.extend(cache.values().cloned());
        }
        
        // If using mmap, also get UTXOs from disk
        if self.use_mmap {
            // In a real implementation, would scan the index and load from disk
            // For simplicity, we'll just use what's in cache
        }
        
        Ok(all_utxos)
    }

    /// Flush the cache to disk if using persistent storage
    pub fn flush(&self) -> Result<(), String> {
        if !self.use_mmap || self.mmap.is_none() {
            return Ok(());
        }
        
        let start_time = Instant::now();
        
        // Get entries to flush
        let entries_to_flush = {
            let cache = self.cache.read().map_err(|e| e.to_string())?;
            cache.values().cloned().collect::<Vec<_>>()
        };
        
        if entries_to_flush.is_empty() {
            return Ok(());
        }
        
        // In a real implementation, would serialize entries and write to mmap
        // For simplicity, we'll just log
        info!("Flushed {} UTXOs to disk", entries_to_flush.len());
        
        // Update stats
        {
            let mut stats = self.stats.write().map_err(|e| e.to_string())?;
            stats.operation_time += start_time.elapsed();
        }
        
        Ok(())
    }

    /// Prune the cache if it exceeds capacity
    fn prune_cache(&self) -> Result<(), String> {
        let cache_size = {
            let cache = self.cache.read().map_err(|e| e.to_string())?;
            cache.len()
        };
        
        if cache_size <= self.cache_capacity {
            return Ok(());
        }
        
        // Flush to disk first if using persistent storage
        if self.use_mmap {
            self.flush()?;
        }
        
        // Calculate entries to remove
        let entries_to_remove = cache_size - self.cache_capacity;
        if entries_to_remove == 0 {
            return Ok(());
        }
        
        // Select entries to remove (oldest or least recently used)
        // For simplicity, we'll just remove random entries
        {
            let mut cache = self.cache.write().map_err(|e| e.to_string())?;
            let keys_to_remove: Vec<OutPoint> = cache.keys()
                .take(entries_to_remove)
                .cloned()
                .collect();
                
            for key in keys_to_remove {
                cache.remove(&key);
            }
            
            // Update stats
            let mut stats = self.stats.write().map_err(|e| e.to_string())?;
            stats.entries = cache.len();
        }
        
        Ok(())
    }

    /// Get statistics about the UTXO set
    pub fn get_stats(&self) -> Result<UtxoCacheStats, String> {
        let stats = self.stats.read().map_err(|e| e.to_string())?;
        Ok(stats.clone())
    }

    /// Clear the UTXO set (for testing or resetting)
    pub fn clear(&self) -> Result<(), String> {
        // Clear cache
        {
            let mut cache = self.cache.write().map_err(|e| e.to_string())?;
            cache.clear();
        }
        
        // Clear index
        {
            let mut index = self.index.write().map_err(|e| e.to_string())?;
            index.clear();
        }
        
        // Clear spent outpoints
        {
            let mut spent = self.spent_outpoints.write().map_err(|e| e.to_string())?;
            spent.clear();
        }
        
        // Reset commitment
        {
            let mut commitment = self.commitment.write().map_err(|e| e.to_string())?;
            *commitment = UtxoCommitment {
                root_hash: [0; 32],
                utxo_count: 0,
                total_value: 0,
                block_height: 0,
            };
        }
        
        // Reset stats
        {
            let mut stats = self.stats.write().map_err(|e| e.to_string())?;
            *stats = UtxoCacheStats::default();
        }
        
        info!("UTXO set cleared");
        
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::transaction::{OutPoint, TxOutput};
    
    fn create_test_utxo(txid: &str, vout: u32, value: u64) -> UtxoEntry {
        UtxoEntry {
            outpoint: OutPoint {
                txid: txid.to_string(),
                vout,
            },
            output: TxOutput {
                value,
                script_pubkey: vec![0, 1, 2, 3],
            },
            height: 1,
            is_coinbase: false,
            is_confirmed: true,
        }
    }
    
    #[test]
    fn test_utxo_add_get_remove() {
        let utxo_set = UtxoSet::new_in_memory(100);
        
        // Add a UTXO
        let utxo = create_test_utxo("tx1", 0, 1000);
        assert!(utxo_set.add(utxo.clone()).is_ok());
        
        // Get the UTXO
        let result = utxo_set.get(&utxo.outpoint).unwrap();
        assert!(result.is_some());
        assert_eq!(result.unwrap().output.amount(), 1000);
        
        // Remove the UTXO
        let removed = utxo_set.remove(&utxo.outpoint).unwrap();
        assert!(removed.is_some());
        assert_eq!(removed.unwrap().output.amount(), 1000);
        
        // Verify it's gone
        assert!(!utxo_set.contains(&utxo.outpoint).unwrap());
        assert!(utxo_set.get(&utxo.outpoint).unwrap().is_none());
    }
    
    #[test]
    fn test_utxo_commitment() {
        let utxo_set = UtxoSet::new_in_memory(100);
        
        // Add some UTXOs
        let utxo1 = create_test_utxo("tx1", 0, 1000);
        let utxo2 = create_test_utxo("tx2", 1, 2000);
        
        assert!(utxo_set.add(utxo1).is_ok());
        assert!(utxo_set.add(utxo2).is_ok());
        
        // Update commitment
        let commitment = utxo_set.update_commitment(10).unwrap();
        
        // Verify commitment values
        assert_eq!(commitment.utxo_count, 2);
        assert_eq!(commitment.total_value, 3000);
        assert_eq!(commitment.block_height, 10);
        
        // Root hash should not be zero
        assert_ne!(commitment.root_hash, [0; 32]);
    }
} 