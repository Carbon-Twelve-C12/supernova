//! Atomic UTXO Set Implementation for Supernova
//! 
//! This module provides thread-safe, atomic UTXO operations to prevent
//! double-spending and ensure consistency in concurrent environments.

use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock, Mutex};
use std::collections::{HashMap, HashSet};
use std::fs::{File, OpenOptions};
use std::io::{self, Read, Write, Seek, SeekFrom};

use serde::{Serialize, Deserialize};
use btclib::types::transaction::{Transaction, TransactionOutput};
use tracing::{debug, info, warn, error};

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
}

/// Atomic transaction for UTXO operations
pub struct UtxoTransaction {
    /// Outputs to be spent (removed)
    pub inputs: Vec<OutPoint>,
    /// New outputs to be created
    pub outputs: Vec<(OutPoint, UnspentOutput)>,
}

/// Thread-safe, atomic UTXO set implementation
pub struct AtomicUtxoSet {
    /// In-memory UTXO index protected by RwLock for concurrent reads
    utxos: Arc<RwLock<HashMap<OutPoint, UnspentOutput>>>,
    /// Transaction lock to ensure atomic operations
    tx_lock: Arc<Mutex<()>>,
    /// Spent outputs tracking to prevent double-spending
    spent_outputs: Arc<RwLock<HashSet<OutPoint>>>,
    /// Pending transactions that haven't been committed
    pending_txs: Arc<Mutex<HashMap<[u8; 32], UtxoTransaction>>>,
    /// Database file path
    db_path: PathBuf,
    /// Write-ahead log for crash recovery
    wal_path: PathBuf,
}

impl AtomicUtxoSet {
    /// Create a new atomic UTXO set
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self, StorageError> {
        let db_path = path.as_ref().to_path_buf();
        let wal_path = db_path.with_extension("wal");
        
        // Create parent directory if it doesn't exist
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        
        let utxo_set = Self {
            utxos: Arc::new(RwLock::new(HashMap::new())),
            tx_lock: Arc::new(Mutex::new(())),
            spent_outputs: Arc::new(RwLock::new(HashSet::new())),
            pending_txs: Arc::new(Mutex::new(HashMap::new())),
            db_path,
            wal_path,
        };
        
        // Load existing UTXO set if present
        utxo_set.load()?;
        
        Ok(utxo_set)
    }
    
    /// Load UTXO set from disk
    fn load(&self) -> Result<(), StorageError> {
        // First, replay any WAL entries for crash recovery
        self.replay_wal()?;
        
        // Then load the main database
        if self.db_path.exists() {
            let mut file = File::open(&self.db_path)?;
            let mut contents = Vec::new();
            file.read_to_end(&mut contents)?;
            
            if !contents.is_empty() {
                let loaded_utxos: HashMap<OutPoint, UnspentOutput> = 
                    bincode::deserialize(&contents)?;
                
                let mut utxos = self.utxos.write().unwrap();
                *utxos = loaded_utxos;
                
                info!("Loaded {} UTXOs from disk", utxos.len());
            }
        }
        
        Ok(())
    }
    
    /// Replay write-ahead log for crash recovery
    fn replay_wal(&self) -> Result<(), StorageError> {
        if !self.wal_path.exists() {
            return Ok(());
        }
        
        info!("Replaying WAL for crash recovery");
        
        let file = File::open(&self.wal_path)?;
        let mut reader = std::io::BufReader::new(file);
        
        // Read and apply each WAL entry
        loop {
            match bincode::deserialize_from::<_, UtxoTransaction>(&mut reader) {
                Ok(tx) => {
                    // Apply the transaction
                    self.apply_transaction_internal(&tx)?;
                }
                Err(_) => break, // End of WAL
            }
        }
        
        // Clear WAL after replay
        std::fs::remove_file(&self.wal_path).ok();
        
        Ok(())
    }
    
    /// Begin a new UTXO transaction
    pub fn begin_transaction(&self) -> UtxoTransactionBuilder {
        UtxoTransactionBuilder::new(self)
    }
    
    /// Process a blockchain transaction atomically
    pub fn process_transaction(
        &self,
        tx: &Transaction,
        height: u64,
        is_coinbase: bool,
    ) -> Result<(), StorageError> {
        let txid = tx.hash();
        
        // Build the UTXO transaction
        let mut utxo_tx = UtxoTransaction {
            inputs: Vec::new(),
            outputs: Vec::new(),
        };
        
        // Collect inputs to spend (except for coinbase)
        if !is_coinbase {
            for input in tx.inputs() {
                let outpoint = OutPoint::new(
                    input.prev_tx_hash(),
                    input.prev_output_index(),
                );
                utxo_tx.inputs.push(outpoint);
            }
        }
        
        // Collect outputs to create
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
            utxo_tx.outputs.push((outpoint, unspent_output));
        }
        
        // Apply the transaction atomically
        self.apply_transaction(utxo_tx)
    }
    
    /// Apply a UTXO transaction atomically
    pub fn apply_transaction(&self, tx: UtxoTransaction) -> Result<(), StorageError> {
        // Acquire transaction lock for atomicity
        let _tx_guard = self.tx_lock.lock().unwrap();
        
        // First, write to WAL for crash recovery
        self.write_to_wal(&tx)?;
        
        // Validate all inputs exist and aren't already spent
        {
            let utxos = self.utxos.read().unwrap();
            let spent = self.spent_outputs.read().unwrap();
            
            for input in &tx.inputs {
                // Check if UTXO exists
                if !utxos.contains_key(input) {
                    return Err(StorageError::Other(
                        format!("UTXO not found: {:?}", input)
                    ));
                }
                
                // Check if already spent
                if spent.contains(input) {
                    return Err(StorageError::Other(
                        format!("UTXO already spent: {:?}", input)
                    ));
                }
            }
        }
        
        // Apply the transaction
        self.apply_transaction_internal(&tx)?;
        
        // Clear WAL entry after successful application
        self.clear_wal()?;
        
        Ok(())
    }
    
    /// Internal method to apply transaction (used by both normal and WAL replay)
    fn apply_transaction_internal(&self, tx: &UtxoTransaction) -> Result<(), StorageError> {
        // Remove spent UTXOs
        {
            let mut utxos = self.utxos.write().unwrap();
            let mut spent = self.spent_outputs.write().unwrap();
            
            for input in &tx.inputs {
                utxos.remove(input);
                spent.insert(*input);
            }
        }
        
        // Add new UTXOs
        {
            let mut utxos = self.utxos.write().unwrap();
            
            for (outpoint, output) in &tx.outputs {
                utxos.insert(*outpoint, output.clone());
            }
        }
        
        Ok(())
    }
    
    /// Write transaction to WAL
    fn write_to_wal(&self, tx: &UtxoTransaction) -> Result<(), StorageError> {
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.wal_path)?;
        
        bincode::serialize_into(&mut file, tx)?;
        file.sync_all()?;
        
        Ok(())
    }
    
    /// Clear WAL after successful transaction
    fn clear_wal(&self) -> Result<(), StorageError> {
        if self.wal_path.exists() {
            std::fs::remove_file(&self.wal_path)?;
        }
        Ok(())
    }
    
    /// Get a UTXO by outpoint
    pub fn get(&self, outpoint: &OutPoint) -> Option<UnspentOutput> {
        let utxos = self.utxos.read().unwrap();
        utxos.get(outpoint).cloned()
    }
    
    /// Check if a UTXO exists and is unspent
    pub fn contains(&self, outpoint: &OutPoint) -> bool {
        let utxos = self.utxos.read().unwrap();
        let spent = self.spent_outputs.read().unwrap();
        
        utxos.contains_key(outpoint) && !spent.contains(outpoint)
    }
    
    /// Get multiple UTXOs atomically
    pub fn get_batch(&self, outpoints: &[OutPoint]) -> Vec<Option<UnspentOutput>> {
        let utxos = self.utxos.read().unwrap();
        
        outpoints.iter()
            .map(|outpoint| utxos.get(outpoint).cloned())
            .collect()
    }
    
    /// Validate that all inputs for a transaction exist
    pub fn validate_inputs(&self, tx: &Transaction) -> Result<bool, StorageError> {
        let utxos = self.utxos.read().unwrap();
        let spent = self.spent_outputs.read().unwrap();
        
        // Skip validation for coinbase
        if tx.inputs().is_empty() || tx.inputs()[0].prev_output_index() == 0xffffffff {
            return Ok(true);
        }
        
        for input in tx.inputs() {
            let outpoint = OutPoint::new(
                input.prev_tx_hash(),
                input.prev_output_index(),
            );
            
            // Check existence
            if !utxos.contains_key(&outpoint) {
                return Ok(false);
            }
            
            // Check if spent
            if spent.contains(&outpoint) {
                return Ok(false);
            }
        }
        
        Ok(true)
    }
    
    /// Save UTXO set to disk
    pub fn save(&self) -> Result<(), StorageError> {
        let utxos = self.utxos.read().unwrap();
        
        let serialized = bincode::serialize(&*utxos)?;
        
        // Write to temporary file first
        let temp_path = self.db_path.with_extension("tmp");
        let mut file = File::create(&temp_path)?;
        file.write_all(&serialized)?;
        file.sync_all()?;
        
        // Atomic rename
        std::fs::rename(&temp_path, &self.db_path)?;
        
        info!("Saved {} UTXOs to disk", utxos.len());
        
        Ok(())
    }
    
    /// Get current UTXO count
    pub fn len(&self) -> usize {
        let utxos = self.utxos.read().unwrap();
        utxos.len()
    }
    
    /// Get total value of all UTXOs
    pub fn total_value(&self) -> u64 {
        let utxos = self.utxos.read().unwrap();
        utxos.values()
            .map(|utxo| utxo.value)
            .fold(0u64, |acc, val| acc.saturating_add(val))
    }
    
    /// Clear spent outputs older than a certain height (pruning)
    pub fn prune_spent_outputs(&self, height_limit: u64) -> Result<usize, StorageError> {
        let mut spent = self.spent_outputs.write().unwrap();
        let initial_size = spent.len();
        
        // In a real implementation, we'd track the spend height
        // For now, we can't prune without that information
        // This is a placeholder for the interface
        
        Ok(0)
    }
}

/// Builder for atomic UTXO transactions
pub struct UtxoTransactionBuilder<'a> {
    utxo_set: &'a AtomicUtxoSet,
    inputs: Vec<OutPoint>,
    outputs: Vec<(OutPoint, UnspentOutput)>,
}

impl<'a> UtxoTransactionBuilder<'a> {
    fn new(utxo_set: &'a AtomicUtxoSet) -> Self {
        Self {
            utxo_set,
            inputs: Vec::new(),
            outputs: Vec::new(),
        }
    }
    
    /// Add an input to spend
    pub fn spend(mut self, outpoint: OutPoint) -> Self {
        self.inputs.push(outpoint);
        self
    }
    
    /// Add an output to create
    pub fn create(mut self, outpoint: OutPoint, output: UnspentOutput) -> Self {
        self.outputs.push((outpoint, output));
        self
    }
    
    /// Apply the transaction
    pub fn apply(self) -> Result<(), StorageError> {
        let tx = UtxoTransaction {
            inputs: self.inputs,
            outputs: self.outputs,
        };
        
        self.utxo_set.apply_transaction(tx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use std::thread;
    use std::sync::Arc;
    
    fn create_test_utxo(txid: [u8; 32], vout: u32, value: u64) -> (OutPoint, UnspentOutput) {
        let outpoint = OutPoint::new(txid, vout);
        let output = UnspentOutput {
            txid,
            vout,
            value,
            script_pubkey: vec![],
            height: 1,
            is_coinbase: false,
        };
        (outpoint, output)
    }
    
    #[test]
    fn test_atomic_operations() {
        let temp_dir = tempdir().unwrap();
        let utxo_set = AtomicUtxoSet::new(temp_dir.path().join("utxo.db")).unwrap();
        
        // Create a UTXO
        let (outpoint1, output1) = create_test_utxo([1; 32], 0, 1000);
        
        // Add it atomically
        utxo_set.begin_transaction()
            .create(outpoint1, output1.clone())
            .apply()
            .unwrap();
        
        // Verify it exists
        assert!(utxo_set.contains(&outpoint1));
        assert_eq!(utxo_set.get(&outpoint1).unwrap().value, 1000);
        
        // Try to spend it
        let (outpoint2, output2) = create_test_utxo([2; 32], 0, 900);
        
        utxo_set.begin_transaction()
            .spend(outpoint1)
            .create(outpoint2, output2)
            .apply()
            .unwrap();
        
        // Verify the first is spent and second exists
        assert!(!utxo_set.contains(&outpoint1));
        assert!(utxo_set.contains(&outpoint2));
        assert_eq!(utxo_set.get(&outpoint2).unwrap().value, 900);
    }
    
    #[test]
    fn test_double_spend_prevention() {
        let temp_dir = tempdir().unwrap();
        let utxo_set = AtomicUtxoSet::new(temp_dir.path().join("utxo.db")).unwrap();
        
        // Create a UTXO
        let (outpoint, output) = create_test_utxo([1; 32], 0, 1000);
        
        utxo_set.begin_transaction()
            .create(outpoint, output)
            .apply()
            .unwrap();
        
        // Spend it once
        let (outpoint2, output2) = create_test_utxo([2; 32], 0, 900);
        
        utxo_set.begin_transaction()
            .spend(outpoint)
            .create(outpoint2, output2)
            .apply()
            .unwrap();
        
        // Try to spend it again - should fail
        let (outpoint3, output3) = create_test_utxo([3; 32], 0, 900);
        
        let result = utxo_set.begin_transaction()
            .spend(outpoint) // Already spent!
            .create(outpoint3, output3)
            .apply();
        
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("already spent"));
    }
    
    #[test]
    fn test_concurrent_access() {
        let temp_dir = tempdir().unwrap();
        let utxo_set = Arc::new(
            AtomicUtxoSet::new(temp_dir.path().join("utxo.db")).unwrap()
        );
        
        // Create initial UTXOs
        for i in 0..10 {
            let (outpoint, output) = create_test_utxo([i; 32], 0, 1000);
            utxo_set.begin_transaction()
                .create(outpoint, output)
                .apply()
                .unwrap();
        }
        
        // Spawn multiple threads trying to spend the same UTXOs
        let mut handles = vec![];
        
        for thread_id in 0..5 {
            let utxo_set_clone = Arc::clone(&utxo_set);
            
            let handle = thread::spawn(move || {
                let mut successes = 0;
                
                // Each thread tries to spend UTXOs 0-9
                for i in 0..10 {
                    let input = OutPoint::new([i; 32], 0);
                    let (output_point, output) = create_test_utxo(
                        [100 + thread_id; 32], 
                        i, 
                        900
                    );
                    
                    let result = utxo_set_clone.begin_transaction()
                        .spend(input)
                        .create(output_point, output)
                        .apply();
                    
                    if result.is_ok() {
                        successes += 1;
                    }
                }
                
                successes
            });
            
            handles.push(handle);
        }
        
        // Collect results
        let mut total_successes = 0;
        for handle in handles {
            total_successes += handle.join().unwrap();
        }
        
        // Each UTXO can only be spent once
        assert_eq!(total_successes, 10);
    }
    
    #[test]
    fn test_persistence() {
        let temp_dir = tempdir().unwrap();
        let db_path = temp_dir.path().join("utxo.db");
        
        // Create and populate UTXO set
        {
            let utxo_set = AtomicUtxoSet::new(&db_path).unwrap();
            
            for i in 0..5 {
                let (outpoint, output) = create_test_utxo([i; 32], 0, 1000 * i as u64);
                utxo_set.begin_transaction()
                    .create(outpoint, output)
                    .apply()
                    .unwrap();
            }
            
            utxo_set.save().unwrap();
        }
        
        // Load and verify
        {
            let utxo_set = AtomicUtxoSet::new(&db_path).unwrap();
            
            assert_eq!(utxo_set.len(), 5);
            
            for i in 0..5 {
                let outpoint = OutPoint::new([i; 32], 0);
                assert!(utxo_set.contains(&outpoint));
                assert_eq!(utxo_set.get(&outpoint).unwrap().value, 1000 * i as u64);
            }
        }
    }
    
    #[test]
    fn test_crash_recovery() {
        let temp_dir = tempdir().unwrap();
        let db_path = temp_dir.path().join("utxo.db");
        let wal_path = temp_dir.path().join("utxo.wal");
        
        // Create initial state
        {
            let utxo_set = AtomicUtxoSet::new(&db_path).unwrap();
            
            let (outpoint1, output1) = create_test_utxo([1; 32], 0, 1000);
            utxo_set.begin_transaction()
                .create(outpoint1, output1)
                .apply()
                .unwrap();
            
            utxo_set.save().unwrap();
        }
        
        // Simulate crash during transaction
        {
            let utxo_set = AtomicUtxoSet::new(&db_path).unwrap();
            
            // Start a transaction
            let tx = UtxoTransaction {
                inputs: vec![OutPoint::new([1; 32], 0)],
                outputs: vec![create_test_utxo([2; 32], 0, 900)],
            };
            
            // Write to WAL but don't complete
            utxo_set.write_to_wal(&tx).unwrap();
            
            // Simulate crash (drop without completing)
        }
        
        // Verify WAL exists
        assert!(wal_path.exists());
        
        // Recovery on restart
        {
            let utxo_set = AtomicUtxoSet::new(&db_path).unwrap();
            
            // Should have replayed the WAL
            assert!(!utxo_set.contains(&OutPoint::new([1; 32], 0)));
            assert!(utxo_set.contains(&OutPoint::new([2; 32], 0)));
            assert_eq!(utxo_set.get(&OutPoint::new([2; 32], 0)).unwrap().value, 900);
            
            // WAL should be cleared
            assert!(!wal_path.exists());
        }
    }
} 