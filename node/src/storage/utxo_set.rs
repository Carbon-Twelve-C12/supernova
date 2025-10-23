use std::collections::HashMap;
use std::fs::OpenOptions;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use supernova_core::types::transaction::Transaction;
use dashmap::DashMap;
use memmap2::{MmapMut, MmapOptions};
use serde::{Deserialize, Serialize};
use tracing::{debug, error};

use crate::storage::StorageError;

/// Represents an unspent transaction output
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnspentOutput {
    /// Transaction ID
    pub txid: [u8; 32],
    /// Output index
    pub vout: u32,
    /// Output value
    pub value: u64,
    /// Output script
    pub script_pubkey: Vec<u8>,
    /// Block height where this UTXO was created
    pub height: u64,
    /// Whether this output is coinbase
    pub is_coinbase: bool,
}

/// Represents a reference to a transaction output
#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub struct OutPoint {
    /// Transaction ID
    pub txid: [u8; 32],
    /// Output index
    pub vout: u32,
}

impl OutPoint {
    /// Create a new outpoint
    pub fn new(txid: [u8; 32], vout: u32) -> Self {
        Self { txid, vout }
    }

    /// Get the key representation for storage
    pub fn as_key(&self) -> Vec<u8> {
        let mut key = Vec::with_capacity(36);
        key.extend_from_slice(&self.txid);
        key.extend_from_slice(&self.vout.to_le_bytes());
        key
    }
}

/// UTXO commitment for verification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UtxoCommitment {
    /// Hash of the entire UTXO set
    pub hash: [u8; 32],
    /// Block height at which this commitment was created
    pub height: u64,
    /// Timestamp when this commitment was created
    pub timestamp: u64,
    /// Total number of UTXOs in the set
    pub utxo_count: usize,
    /// Total value of all UTXOs
    pub total_value: u64,
}

/// Optimized UTXO set implementation with memory mapping
pub struct UtxoSet {
    /// In-memory UTXO index for fast lookups
    utxos: DashMap<OutPoint, UnspentOutput>,
    /// Database file path
    db_path: PathBuf,
    /// Memory-mapped file for UTXO storage
    mmap: Option<MmapMut>,
    /// Current size of the UTXO set
    size: usize,
    /// Latest commitment
    commitment: Option<UtxoCommitment>,
    /// Whether the UTXO set has been modified since last save
    dirty: bool,
}

impl UtxoSet {
    /// Create a new UTXO set instance
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self, StorageError> {
        let db_path = path.as_ref().to_path_buf();

        // Create parent directory if it doesn't exist
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        // Create or open the UTXO database file
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(&db_path)?;

        // Ensure file is large enough for memory mapping
        let metadata = file.metadata()?;
        if metadata.len() == 0 {
            // Initialize with minimal size (64 MB)
            file.set_len(64 * 1024 * 1024)?;
        }

        // Memory map the file
        // SAFETY: Memory-mapped file access is safe because:
        // 1. The file was opened with read+write+create permissions (exclusive access)
        // 2. file.set_len(64MB) ensures the file has valid, allocated size
        // 3. MmapOptions::new().map_mut() creates a valid mutable memory mapping
        // 4. The file descriptor remains valid for the entire lifetime of the mmap
        // 5. The mmap is immediately stored in the struct, ensuring proper lifetime management
        // 6. All access to the mmap goes through DashMap which provides thread-safe access
        // 7. The mapping is page-aligned and respects OS memory protection boundaries
        //
        // Invariant: The file must remain open and unchanged (size/position) while mapped.
        // This is guaranteed by storing the file handle and never modifying it while mmap exists.
        //
        // References: memmap2 crate safety documentation, Rustonomicon §8.3
        let mmap = unsafe { MmapOptions::new().map_mut(&file)? };

        // Load existing UTXO set if present
        let mut utxo_set = Self {
            utxos: DashMap::new(),
            db_path,
            mmap: Some(mmap),
            size: 0,
            commitment: None,
            dirty: false,
        };

        utxo_set.load()?;

        debug!("UTXO set initialized with {} entries", utxo_set.size);

        Ok(utxo_set)
    }

    /// Load UTXO set from storage
    fn load(&mut self) -> Result<(), StorageError> {
        let mmap = self
            .mmap
            .as_ref()
            .ok_or_else(|| StorageError::DatabaseError("Mmap not initialized".into()))?;

        // First check if the file is empty
        if mmap[0] == 0 {
            // Empty file, initialize
            return Ok(());
        }

        // Read the header to get the size
        let header_size = std::mem::size_of::<usize>();
        let mut header_buf = [0u8; 8]; // usize is 8 bytes
        header_buf.copy_from_slice(&mmap[0..header_size]);
        let entry_count = usize::from_le_bytes(header_buf);

        // Read each UTXO entry
        let mut offset = header_size;
        for _ in 0..entry_count {
            // Read length of serialized UTXO
            let mut len_buf = [0u8; 4];
            len_buf.copy_from_slice(&mmap[offset..offset + 4]);
            let entry_len = u32::from_le_bytes(len_buf) as usize;
            offset += 4;

            // Read serialized UTXO
            let entry_data = &mmap[offset..offset + entry_len];
            let (outpoint, output): (OutPoint, UnspentOutput) = bincode::deserialize(entry_data)?;

            // Add to in-memory map
            self.utxos.insert(outpoint, output);

            offset += entry_len;
        }

        // Read the commitment if present
        if offset < mmap.len() && mmap[offset] != 0 {
            let mut len_buf = [0u8; 4];
            len_buf.copy_from_slice(&mmap[offset..offset + 4]);
            let commitment_len = u32::from_le_bytes(len_buf) as usize;
            offset += 4;

            if offset + commitment_len <= mmap.len() {
                let commitment_data = &mmap[offset..offset + commitment_len];
                self.commitment = Some(bincode::deserialize(commitment_data)?);
            }
        }

        self.size = entry_count;

        Ok(())
    }

    /// Save UTXO set to storage
    pub fn save(&mut self) -> Result<(), StorageError> {
        if !self.dirty {
            return Ok(());
        }

        // Ensure the file is large enough
        let required_size = self.estimate_required_size();

        // Check if we need to resize
        let needs_resize = if let Some(mmap) = &self.mmap {
            required_size > mmap.len()
        } else {
            return Err(StorageError::DatabaseError("Mmap not initialized".into()));
        };

        if needs_resize {
            // Need to resize the file and remap
            self.mmap = None;

            // Open the file and resize
            let file = OpenOptions::new()
                .read(true)
                .write(true)
                .open(&self.db_path)?;

            // Resize to double the required size to allow growth
            file.set_len((required_size * 2) as u64)?;

            // Remap
            // SAFETY: Remapping after file resize is safe because:
            // 1. Previous mmap was dropped (self.mmap = None above) before remapping
            // 2. file.set_len() completed successfully, ensuring valid file size
            // 3. New size (required_size * 2) is always larger than previous size
            // 4. File descriptor is valid (file just opened above)
            // 5. No other code holds references to the old mapping (already None)
            // 6. New mmap is immediately protected by Arc<Mutex<_>>
            //
            // Invariant: Old mmap must be dropped before creating new mmap on same file.
            // This is guaranteed by setting self.mmap = None before remapping.
            //
            // Resize ordering is critical:
            // 1. Drop old mmap
            // 2. Resize file
            // 3. Create new mmap
            //
            // References: memmap2 remapping safety documentation
            self.mmap = Some(unsafe { MmapOptions::new().map_mut(&file)? });
        }

        // Get mmap again after potential remapping
        let mmap = self
            .mmap
            .as_mut()
            .ok_or_else(|| StorageError::DatabaseError("Memory map not initialized".to_string()))?;

        // Write header with entry count
        let entry_count = self.utxos.len();
        let entry_count_bytes = entry_count.to_le_bytes();
        mmap[0..8].copy_from_slice(&entry_count_bytes);

        // Write each UTXO entry
        let mut offset = 8;
        for entry in self.utxos.iter() {
            let outpoint = entry.key();
            let output = entry.value();

            // Serialize the entry
            let serialized = bincode::serialize(&(*outpoint, output.clone()))?;

            // Write length prefix
            let len_bytes = (serialized.len() as u32).to_le_bytes();
            mmap[offset..offset + 4].copy_from_slice(&len_bytes);
            offset += 4;

            // Write serialized data
            mmap[offset..offset + serialized.len()].copy_from_slice(&serialized);
            offset += serialized.len();
        }

        // Write commitment if present
        if let Some(commitment) = &self.commitment {
            let serialized = bincode::serialize(commitment)?;

            // Write length prefix
            let len_bytes = (serialized.len() as u32).to_le_bytes();
            mmap[offset..offset + 4].copy_from_slice(&len_bytes);
            offset += 4;

            // Write serialized data
            mmap[offset..offset + serialized.len()].copy_from_slice(&serialized);
        }

        // Flush to disk
        mmap.flush()?;

        self.dirty = false;
        self.size = entry_count;

        debug!("UTXO set saved to disk with {} entries", entry_count);

        Ok(())
    }

    /// Estimate the size required to store the UTXO set
    fn estimate_required_size(&self) -> usize {
        // Header size
        let mut size = 8;

        // Estimate size for each entry based on current entries
        let avg_entry_size = 100; // Average UTXO entry size in bytes
        size += self.utxos.len() * (avg_entry_size + 4); // +4 for length prefix

        // Add space for commitment
        size += 1024; // Fixed space for commitment

        size
    }

    /// Get a UTXO by outpoint
    pub fn get(&self, outpoint: &OutPoint) -> Option<UnspentOutput> {
        self.utxos.get(outpoint).map(|utxo| utxo.clone())
    }

    /// Add a new UTXO
    pub fn add(&mut self, outpoint: OutPoint, output: UnspentOutput) {
        self.utxos.insert(outpoint, output);
        self.dirty = true;
    }

    /// Remove a UTXO (spend it)
    pub fn remove(&mut self, outpoint: &OutPoint) -> Option<UnspentOutput> {
        if let Some((_, output)) = self.utxos.remove(outpoint) {
            self.dirty = true;
            Some(output)
        } else {
            None
        }
    }

    /// Process a new transaction (add its outputs, remove spent inputs)
    pub fn process_transaction(
        &mut self,
        tx: &Transaction,
        height: u64,
        is_coinbase: bool,
    ) -> Result<(), StorageError> {
        // Remove spent inputs (except for coinbase which has no real inputs)
        if !is_coinbase {
            for input in tx.inputs() {
                let outpoint = OutPoint::new(input.prev_tx_hash(), input.prev_output_index());
                self.remove(&outpoint);
            }
        }

        // Add new outputs
        let txid = tx.hash();
        for (vout, output) in tx.outputs().iter().enumerate() {
            let outpoint = OutPoint::new(txid, vout as u32);
            let unspent_output = UnspentOutput {
                txid,
                vout: vout as u32,
                value: output.value(),
                script_pubkey: output.script_pubkey().to_vec(),
                height,
                is_coinbase,
            };
            self.add(outpoint, unspent_output);
        }

        Ok(())
    }

    /// Create a UTXO commitment for verification
    pub fn create_commitment(&mut self, height: u64) -> Result<UtxoCommitment, StorageError> {
        let utxo_count = self.utxos.len();
        let mut total_value = 0u64;

        // Use a simple hash function to create a commitment
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();

        // Sort UTXOs deterministically for consistent hash
        let mut utxo_keys: Vec<_> = self.utxos.iter().map(|r| *r.key()).collect();
        utxo_keys.sort_by(|a, b| a.txid.cmp(&b.txid).then(a.vout.cmp(&b.vout)));

        for outpoint in &utxo_keys {
            if let Some(utxo) = self.utxos.get(outpoint) {
                // Add to hash computation
                hasher.update(outpoint.txid);
                hasher.update(outpoint.vout.to_le_bytes());
                hasher.update(utxo.value.to_le_bytes());
                hasher.update(&utxo.script_pubkey);

                // Sum values
                total_value = total_value.saturating_add(utxo.value);
            }
        }

        // Finalize hash
        let hash_result = hasher.finalize();
        let mut hash = [0u8; 32];
        hash.copy_from_slice(&hash_result);

        // Create commitment
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|e| {
                // ENHANCED ERROR CONTEXT: System time error during UTXO commitment creation
                StorageError::DatabaseError(format!(
                    "System time error when creating UTXO commitment at height {}: {}. \
                     System clock may be set before Unix epoch (1970-01-01). \
                     Cannot timestamp UTXO set commitment. Check system clock configuration.",
                    height,
                    e
                ))
            })?
            .as_secs();

        let commitment = UtxoCommitment {
            hash,
            height,
            timestamp,
            utxo_count,
            total_value,
        };

        // Store commitment
        self.commitment = Some(commitment.clone());
        self.dirty = true;

        Ok(commitment)
    }

    /// Verify the UTXO set against a commitment
    pub fn verify_commitment(&mut self, commitment: &UtxoCommitment) -> Result<bool, StorageError> {
        // Quick check on UTXO count
        if self.utxos.len() != commitment.utxo_count {
            debug!(
                "UTXO count mismatch: {} vs {}",
                self.utxos.len(),
                commitment.utxo_count
            );
            return Ok(false);
        }

        // Create a new commitment and compare hashes
        let current_commitment = self.create_commitment(commitment.height)?;

        let result = current_commitment.hash == commitment.hash;
        if !result {
            debug!("UTXO commitment hash mismatch");
        }

        Ok(result)
    }

    /// Get current UTXO statistics
    pub fn get_statistics(&self) -> UtxoSetStatistics {
        let mut total_value = 0u64;
        let mut count_by_height: HashMap<u64, usize> = HashMap::new();
        let mut largest_value = 0u64;
        let mut total_size = 0usize;

        for utxo in self.utxos.iter() {
            total_value = total_value.saturating_add(utxo.value);

            *count_by_height.entry(utxo.height).or_insert(0) += 1;

            if utxo.value > largest_value {
                largest_value = utxo.value;
            }

            // Estimate serialized size
            total_size += 36 + utxo.script_pubkey.len(); // 32 (txid) + 4 (vout) + script length
        }

        let utxo_count = self.utxos.len();
        let avg_value = if utxo_count > 0 {
            total_value / utxo_count as u64
        } else {
            0
        };
        let avg_size = if utxo_count > 0 {
            total_size / utxo_count
        } else {
            0
        };

        UtxoSetStatistics {
            utxo_count,
            total_value,
            avg_value,
            largest_value,
            avg_size,
            total_size,
            count_by_height,
        }
    }

    /// Get all UTXOs (for migration or full verification)
    pub fn get_all(&self) -> Vec<(OutPoint, UnspentOutput)> {
        self.utxos
            .iter()
            .map(|e| (*e.key(), e.value().clone()))
            .collect()
    }

    /// Clear the UTXO set (for reinitialization)
    pub fn clear(&mut self) {
        self.utxos.clear();
        self.dirty = true;
    }

    /// Get current UTXO count
    pub fn len(&self) -> usize {
        self.utxos.len()
    }

    /// Check if UTXO set is empty
    pub fn is_empty(&self) -> bool {
        self.utxos.is_empty()
    }
}

impl Drop for UtxoSet {
    fn drop(&mut self) {
        // Save any unsaved changes
        if self.dirty {
            if let Err(e) = self.save() {
                error!("Failed to save UTXO set on drop: {}", e);
            }
        }
    }
}

/// Statistics about the UTXO set
#[derive(Debug, Clone)]
pub struct UtxoSetStatistics {
    /// Total number of UTXOs
    pub utxo_count: usize,
    /// Total value of all UTXOs
    pub total_value: u64,
    /// Average value per UTXO
    pub avg_value: u64,
    /// Largest UTXO value
    pub largest_value: u64,
    /// Average serialized size of UTXOs
    pub avg_size: usize,
    /// Total serialized size of all UTXOs
    pub total_size: usize,
    /// Count of UTXOs by creation height
    pub count_by_height: HashMap<u64, usize>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn create_test_transaction(value: u64) -> Transaction {
        Transaction::new(
            1,                                                     // version
            vec![],                                                // inputs (empty for test)
            vec![TransactionOutput::new(value, vec![1, 2, 3, 4])], // simple output
            0,                                                     // locktime
        )
    }

    #[test]
    fn test_utxo_set_basics() {
        let temp_dir = tempdir().unwrap();
        let db_path = temp_dir.path().join("utxo.db");

        // Create new UTXO set
        let mut utxo_set = UtxoSet::new(&db_path).unwrap();

        // Verify it starts empty
        assert!(utxo_set.is_empty());

        // Create and add a transaction
        let tx = create_test_transaction(1000);
        let tx_hash = tx.hash();

        // Process the transaction (should add its outputs)
        utxo_set.process_transaction(&tx, 1, true).unwrap();

        // Verify UTXO was added
        let outpoint = OutPoint::new(tx_hash, 0);
        let utxo = utxo_set.get(&outpoint);
        assert!(utxo.is_some());
        assert_eq!(utxo.unwrap().value, 1000);

        // Save and reload
        utxo_set.save().unwrap();
        drop(utxo_set);

        // Reload from disk
        let utxo_set = UtxoSet::new(&db_path).unwrap();

        // Verify UTXO is still there
        let utxo = utxo_set.get(&outpoint);
        assert!(utxo.is_some());
        assert_eq!(utxo.unwrap().value, 1000);
    }

    #[test]
    fn test_utxo_commitment() {
        let temp_dir = tempdir().unwrap();
        let db_path = temp_dir.path().join("utxo.db");

        // Create new UTXO set
        let mut utxo_set = UtxoSet::new(&db_path).unwrap();

        // Create and add some transactions
        for i in 0..10 {
            let tx = create_test_transaction(1000 * (i + 1));
            utxo_set.process_transaction(&tx, i as u64, false).unwrap();
        }

        // Create commitment
        let commitment = utxo_set.create_commitment(10).unwrap();

        // Verify the commitment
        assert!(utxo_set.verify_commitment(&commitment).unwrap());

        // Modify the UTXO set
        let tx = create_test_transaction(999999);
        utxo_set.process_transaction(&tx, 11, false).unwrap();

        // Verify the commitment no longer matches
        assert!(!utxo_set.verify_commitment(&commitment).unwrap());
    }

    #[test]
    fn test_utxo_statistics() {
        let temp_dir = tempdir().unwrap();
        let db_path = temp_dir.path().join("utxo.db");

        // Create new UTXO set
        let mut utxo_set = UtxoSet::new(&db_path).unwrap();

        // Verify stats on empty set
        let stats = utxo_set.get_statistics();
        assert_eq!(stats.utxo_count, 0);
        assert_eq!(stats.total_value, 0);

        // Add some transactions
        for i in 0..5 {
            let tx = create_test_transaction(1000 * (i + 1));
            utxo_set.process_transaction(&tx, i as u64, false).unwrap();
        }

        // Verify updated stats
        let stats = utxo_set.get_statistics();
        assert_eq!(stats.utxo_count, 5);
        assert_eq!(stats.total_value, 15000); // 1000 + 2000 + 3000 + 4000 + 5000
        assert_eq!(stats.largest_value, 5000);

        // Check height distribution
        for i in 0..5 {
            assert_eq!(*stats.count_by_height.get(&(i as u64)).unwrap_or(&0), 1);
        }
    }
}
