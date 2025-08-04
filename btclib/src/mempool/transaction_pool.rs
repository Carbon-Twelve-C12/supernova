use std::collections::{HashMap, HashSet};
use std::sync::{Arc, RwLock, Mutex};
use std::time::{Duration, Instant};
use thiserror::Error;
use dashmap::DashMap;
use std::fmt;
use hex;

use crate::types::transaction::{Transaction, TransactionOutput};
use crate::types::transaction_dependency::TransactionDependencyGraph;

/// Maximum age for a transaction to stay in the mempool (1 week)
const MAX_TRANSACTION_AGE: Duration = Duration::from_secs(7 * 24 * 60 * 60);

/// Error types for mempool operations
#[derive(Debug, Error)]
pub enum MempoolError {
    /// Transaction already exists in the mempool
    #[error("Transaction already exists in mempool")]
    AlreadyExists,
    
    /// Transaction has missing inputs (not in UTXO set or mempool)
    #[error("Transaction has missing inputs")]
    MissingInputs,
    
    /// Transaction is invalid
    #[error("Transaction is invalid: {0}")]
    InvalidTransaction(String),
    
    /// Transaction fee is too low
    #[error("Transaction fee too low (min: {min_fee_rate} sat/byte, actual: {actual_fee_rate} sat/byte)")]
    FeeTooLow {
        min_fee_rate: u64,
        actual_fee_rate: u64,
    },
    
    /// Transaction would exceed maximum mempool size
    #[error("Transaction would exceed maximum mempool size")]
    PoolFull,
    
    /// Transaction not found in mempool
    #[error("Transaction not found in mempool")]
    NotFound,
    
    /// Transaction conflicts with existing mempool transaction
    #[error("Transaction conflicts with existing mempool transaction")]
    Conflict,
    
    /// General error
    #[error("Mempool error: {0}")]
    Other(String),
}

/// Entry in the mempool for a transaction
#[derive(Debug, Clone)]
pub struct MempoolEntry {
    /// The transaction itself
    pub transaction: Transaction,
    /// When the transaction was added to the mempool
    pub time_added: Instant,
    /// Fee in satoshis
    pub fee: u64,
    /// Fee rate in satoshis per byte
    pub fee_rate: u64,
    /// Size in bytes
    pub size: usize,
    /// Ancestor package size in bytes
    pub ancestor_size: usize,
    /// Ancestor package fee in satoshis
    pub ancestor_fee: u64,
    /// Ancestor package fee rate in satoshis per byte
    pub ancestor_fee_rate: u64,
}

/// Configuration for the transaction pool
#[derive(Debug, Clone)]
pub struct TransactionPoolConfig {
    /// Maximum size of the mempool in bytes
    pub max_size_bytes: usize,
    /// Minimum fee rate in satoshis per byte
    pub min_fee_rate: u64,
    /// Maximum number of orphan transactions
    pub max_orphan_transactions: usize,
    /// Whether to enable replace-by-fee
    pub enable_replace_by_fee: bool,
    /// Minimum fee rate increment for replace-by-fee (percentage)
    pub rbf_min_fee_increment: u8,
}

impl Default for TransactionPoolConfig {
    fn default() -> Self {
        Self {
            max_size_bytes: 300 * 1024 * 1024, // 300 MB
            min_fee_rate: 1,                   // 1 sat/byte
            max_orphan_transactions: 100,
            enable_replace_by_fee: true,
            rbf_min_fee_increment: 10,         // 10% higher fee required for RBF
        }
    }
}

/// Thread-safe transaction pool (mempool)
pub struct TransactionPool {
    /// Configuration for the pool
    config: TransactionPoolConfig,
    /// Map of transaction hash to transaction entry
    transactions: DashMap<[u8; 32], MempoolEntry>,
    /// Orphan transactions (missing inputs)
    orphans: Arc<RwLock<HashMap<[u8; 32], Transaction>>>,
    /// Dependency graph for transactions
    dependency_graph: Arc<Mutex<TransactionDependencyGraph>>,
    /// Function to get a UTXO from the blockchain
    get_utxo: Arc<dyn Fn(&[u8; 32], u32) -> Option<TransactionOutput> + Send + Sync>,
    /// Current mempool size in bytes
    size_bytes: Arc<RwLock<usize>>,
}

impl TransactionPool {
    /// Create a new transaction pool with the given configuration
    pub fn new(
        config: TransactionPoolConfig,
        get_utxo: impl Fn(&[u8; 32], u32) -> Option<TransactionOutput> + Send + Sync + 'static,
    ) -> Self {
        Self {
            config,
            transactions: DashMap::new(),
            orphans: Arc::new(RwLock::new(HashMap::new())),
            dependency_graph: Arc::new(Mutex::new(TransactionDependencyGraph::new())),
            get_utxo: Arc::new(get_utxo),
            size_bytes: Arc::new(RwLock::new(0)),
        }
    }
    
    /// Add a transaction to the pool
    pub fn add_transaction(&self, tx: Transaction) -> Result<(), MempoolError> {
        let tx_hash = tx.hash();
        
        // Check if transaction is already in the pool
        if self.transactions.contains_key(&tx_hash) {
            return Err(MempoolError::AlreadyExists);
        }
        
        // Validate the transaction (basic structural validation)
        if !tx.validate(&self.get_utxo_or_mempool()) {
            return Err(MempoolError::InvalidTransaction("Failed basic validation".to_string()));
        }
        
        // Calculate transaction size
        let size = tx.calculate_size();
        
        // Check if adding this transaction would exceed max mempool size
        {
            let current_size = *self.size_bytes.read()
                .map_err(|e| MempoolError::Other(format!("Lock poisoned: {}", e)))?;
            if current_size + size > self.config.max_size_bytes {
                // Try to make room by removing low-fee transactions
                if !self.make_room(size) {
                    return Err(MempoolError::PoolFull);
                }
            }
        }
        
        // Calculate fee and fee rate
        let fee = match tx.calculate_fee(&self.get_utxo_or_mempool()) {
            Some(fee) => fee,
            None => {
                // This could be an orphan transaction (missing inputs)
                return self.handle_orphan(tx);
            }
        };
        
        let fee_rate = fee / size as u64;
        
        // Check if fee rate meets minimum requirement
        if fee_rate < self.config.min_fee_rate {
            return Err(MempoolError::FeeTooLow {
                min_fee_rate: self.config.min_fee_rate,
                actual_fee_rate: fee_rate,
            });
        }
        
        // Check for conflicts and handle replace-by-fee if enabled
        self.check_conflicts(&tx, fee_rate)?;
        
        // Create mempool entry
        let entry = MempoolEntry {
            transaction: tx.clone(),
            time_added: Instant::now(),
            fee,
            fee_rate,
            size,
            // These will be updated later
            ancestor_size: size,
            ancestor_fee: fee,
            ancestor_fee_rate: fee_rate,
        };
        
        // Update dependency graph
        {
            let mut graph = self.dependency_graph.lock()
                .map_err(|e| MempoolError::Other(format!("Lock poisoned: {}", e)))?;
            let mempool_txs: HashMap<[u8; 32], Transaction> = self.transactions
                .iter()
                .map(|entry| (*entry.key(), entry.value().transaction.clone()))
                .collect();
            
            graph.add_transaction(&tx, &mempool_txs);
        }
        
        // Update ancestor metrics for this transaction and its descendants
        self.update_ancestor_metrics(&tx_hash);
        
        // Add to mempool
        self.transactions.insert(tx_hash, entry);
        
        // Update mempool size
        {
            let mut size_bytes = self.size_bytes.write()
                .map_err(|e| MempoolError::Other(format!("Lock poisoned: {}", e)))?;
            *size_bytes += size;
        }
        
        // Process orphans that might depend on this transaction
        self.process_orphans(&tx_hash);
        
        Ok(())
    }
    
    /// Remove a transaction from the pool
    pub fn remove_transaction(&self, tx_hash: &[u8; 32]) -> Result<Transaction, MempoolError> {
        // Remove from the transaction map
        let entry = match self.transactions.remove(tx_hash) {
            Some((_, entry)) => entry,
            None => return Err(MempoolError::NotFound),
        };
        
        // Update mempool size
        {
            let mut size_bytes = self.size_bytes.write()
                .map_err(|e| MempoolError::Other(format!("Lock poisoned: {}", e)))?;
            *size_bytes -= entry.size;
        }
        
        // Update dependency graph
        {
            let mut graph = self.dependency_graph.lock()
                .map_err(|e| MempoolError::Other(format!("Lock poisoned: {}", e)))?;
            graph.remove_transaction(tx_hash);
        }
        
        Ok(entry.transaction)
    }
    
    /// Get a transaction from the pool
    pub fn get_transaction(&self, tx_hash: &[u8; 32]) -> Option<Transaction> {
        self.transactions.get(tx_hash).map(|entry| entry.transaction.clone())
    }
    
    /// Get all transactions in the pool
    pub fn get_all_transactions(&self) -> Vec<Transaction> {
        self.transactions.iter().map(|entry| entry.transaction.clone()).collect()
    }
    
    /// Get transactions sorted by fee rate (highest first)
    pub fn get_sorted_transactions(&self) -> Vec<Transaction> {
        let mut entries: Vec<_> = self.transactions.iter().map(|r| r.value().clone()).collect();
        
        // Sort by fee rate (highest first)
        entries.sort_by(|a, b| b.fee_rate.cmp(&a.fee_rate));
        
        // Return just the transactions
        entries.into_iter().map(|entry| entry.transaction).collect()
    }
    
    /// Get transactions in order of priority (highest fee rate first)
    pub fn get_prioritized_transactions(&self, max_size: usize) -> Vec<Transaction> {
        let mut entries: Vec<_> = self.transactions.iter().map(|r| r.value().clone()).collect();
        
        // Sort by fee rate (highest first)
        entries.sort_by(|a, b| b.fee_rate.cmp(&a.fee_rate));
        
        // Collect transactions up to max_size
        let mut result = Vec::new();
        let mut total_size = 0;
        
        for entry in entries {
            if total_size + entry.size <= max_size {
                result.push(entry.transaction.clone());
                total_size += entry.size;
            } else if max_size > 0 && result.is_empty() {
                // Include at least one transaction even if it exceeds max_size
                result.push(entry.transaction.clone());
                break;
            } else {
                break;
            }
        }
        
        result
    }
    
    /// Get transactions in topological order (dependencies first)
    pub fn get_topological_transactions(&self) -> Vec<Transaction> {
        // Get the topological order from the dependency graph
        let graph = match self.dependency_graph.lock() {
            Ok(g) => g,
            Err(e) => {
                log::error!("Failed to acquire dependency graph lock: {}", e);
                return Vec::new();
            }
        };
        let order = graph.get_topological_order();
        
        // Convert to transactions
        order.iter()
            .filter_map(|tx_hash| self.get_transaction(tx_hash))
            .collect()
    }
    
    /// Remove expired transactions from the pool
    pub fn remove_expired(&self) -> usize {
        let now = Instant::now();
        let mut removed = 0;
        
        // Find expired transactions
        let expired: Vec<[u8; 32]> = self.transactions
            .iter()
            .filter(|r| now.duration_since(r.value().time_added) > MAX_TRANSACTION_AGE)
            .map(|r| *r.key())
            .collect();
        
        // Remove them
        for tx_hash in expired {
            if self.remove_transaction(&tx_hash).is_ok() {
                removed += 1;
            }
        }
        
        // Also clean orphans
        {
            let mut orphans = match self.orphans.write() {
                Ok(o) => o,
                Err(e) => {
                    log::error!("Failed to acquire orphans write lock: {}", e);
                    return removed;
                }
            };
            let before = orphans.len();
            
            // Keep only the maximum number of most recent orphans
            if orphans.len() > self.config.max_orphan_transactions {
                // This is not the most efficient way, but it's simple and orphan handling
                // is not a critical performance path
                let mut orphans_vec: Vec<_> = orphans.drain().collect();
                orphans_vec.sort_by_key(|(_, tx)| tx.hash()[0] as u64); // Simple deterministic ordering
                
                orphans.extend(orphans_vec.into_iter().take(self.config.max_orphan_transactions));
            }
            
            removed += before - orphans.len();
        }
        
        removed
    }
    
    /// Get the current mempool size in bytes
    pub fn size(&self) -> usize {
        match self.size_bytes.read() {
            Ok(size) => *size,
            Err(e) => {
                log::error!("Failed to read mempool size: {}", e);
                0
            }
        }
    }
    
    /// Get the number of transactions in the pool
    pub fn count(&self) -> usize {
        self.transactions.len()
    }
    
    /// Get the number of transactions in the pool (alias for count)
    pub fn get_transaction_count(&self) -> usize {
        self.transactions.len()
    }
    
    /// Get fee histogram for the mempool
    pub fn get_fee_histogram(&self) -> Vec<(u64, usize)> {
        // Create buckets for fee rates (in sats/byte)
        let buckets = vec![1, 2, 5, 10, 20, 50, 100, 200, 500, 1000];
        let mut histogram = Vec::new();
        
        for (i, &bucket) in buckets.iter().enumerate() {
            let count = self.transactions.iter()
                .filter(|entry| {
                    entry.fee_rate >= bucket && 
                    (i == buckets.len() - 1 || entry.fee_rate < buckets.get(i + 1).copied().unwrap_or(u64::MAX))
                })
                .count();
            
            if count > 0 {
                histogram.push((bucket, count));
            }
        }
        
        histogram
    }
    
    /// Get the fee for a transaction by its hash (hex string)
    pub fn get_transaction_fee(&self, tx_hash_hex: &str) -> Option<u64> {
        // Parse hex string to bytes
        if let Ok(hash_bytes) = hex::decode(tx_hash_hex) {
            if hash_bytes.len() == 32 {
                let mut tx_hash = [0u8; 32];
                tx_hash.copy_from_slice(&hash_bytes);
                
                // Get the transaction entry and return its fee
                self.transactions.get(&tx_hash).map(|entry| entry.fee)
            } else {
                None
            }
        } else {
            None
        }
    }
    
    /// Get the size in bytes for the mempool
    pub fn size_in_bytes(&self) -> usize {
        match self.size_bytes.read() {
            Ok(size) => *size,
            Err(e) => {
                log::error!("Failed to read mempool size: {}", e);
                0
            }
        }
    }
    
    /// Handle an orphan transaction (missing inputs)
    fn handle_orphan(&self, tx: Transaction) -> Result<(), MempoolError> {
        // Only store if we haven't reached maximum orphans
        let mut orphans = self.orphans.write()
            .map_err(|e| MempoolError::Other(format!("Lock poisoned: {}", e)))?;
        
        if orphans.len() < self.config.max_orphan_transactions {
            let tx_hash = tx.hash();
            orphans.insert(tx_hash, tx);
            Ok(())
        } else {
            Err(MempoolError::MissingInputs)
        }
    }
    
    /// Process orphans that might depend on a new transaction
    fn process_orphans(&self, tx_hash: &[u8; 32]) {
        // Find orphans that might depend on this transaction
        let orphans_to_process: Vec<Transaction>;
        
        {
            let mut orphans = match self.orphans.write() {
                Ok(o) => o,
                Err(e) => {
                    log::error!("Failed to acquire orphans write lock: {}", e);
                    return;
                }
            };
            let mut to_process: Vec<Transaction> = Vec::new();
            
            // This is inefficient but simple - we check all orphans
            // A more efficient implementation would maintain an index of orphans by input
            orphans_to_process = orphans
                .values()
                .filter(|tx| {
                    tx.inputs().iter().any(|input| {
                        input.prev_tx_hash() == *tx_hash
                    })
                })
                .cloned()
                .collect();
            
            // Remove the found orphans
            for orphan in &orphans_to_process {
                orphans.remove(&orphan.hash());
            }
        }
        
        // Try to add each orphan to the mempool
        for orphan in orphans_to_process {
            // Ignore errors - if it fails, it might still be an orphan or invalid
            let _ = self.add_transaction(orphan);
        }
    }
    
    /// Check if a transaction conflicts with existing mempool transactions
    fn check_conflicts(&self, tx: &Transaction, fee_rate: u64) -> Result<(), MempoolError> {
        // Find transactions that spend the same inputs
        let conflicts = self.find_conflicts(tx);
        
        if conflicts.is_empty() {
            return Ok(());
        }
        
        // If replace-by-fee is disabled, reject conflicting transactions
        if !self.config.enable_replace_by_fee {
            return Err(MempoolError::Conflict);
        }
        
        // Calculate the minimum fee rate required for replacement
        let min_replacement_fee_rate = self.calculate_min_replacement_fee_rate(&conflicts);
        
        // Check if the new transaction has a high enough fee rate
        if fee_rate < min_replacement_fee_rate {
            return Err(MempoolError::FeeTooLow {
                min_fee_rate: min_replacement_fee_rate,
                actual_fee_rate: fee_rate,
            });
        }
        
        // Remove conflicting transactions
        for tx_hash in conflicts {
            let _ = self.remove_transaction(&tx_hash);
        }
        
        Ok(())
    }
    
    /// Find transactions that conflict with a new transaction
    fn find_conflicts(&self, tx: &Transaction) -> Vec<[u8; 32]> {
        let mut conflicts = Vec::new();
        
        // Check for input conflicts
        for input in tx.inputs() {
            let prev_tx_hash = input.prev_tx_hash();
            let prev_output_index = input.prev_output_index();
            
            // Check all mempool transactions for conflicts
            for entry in self.transactions.iter() {
                let mempool_tx = &entry.transaction;
                
                // Check if this mempool transaction spends the same input
                if mempool_tx.inputs().iter().any(|mempool_input| {
                    mempool_input.prev_tx_hash() == prev_tx_hash &&
                    mempool_input.prev_output_index() == prev_output_index
                }) {
                    conflicts.push(*entry.key());
                }
            }
        }
        
        conflicts
    }
    
    /// Calculate the minimum fee rate required for replacing transactions
    fn calculate_min_replacement_fee_rate(&self, conflicts: &[[u8; 32]]) -> u64 {
        let mut total_conflict_fee = 0u64;
        let mut total_conflict_size = 0usize;
        
        // Calculate the total fee and size of conflicting transactions
        for tx_hash in conflicts {
            if let Some(entry) = self.transactions.get(tx_hash) {
                total_conflict_fee += entry.fee;
                total_conflict_size += entry.size;
            }
        }
        
        if total_conflict_size == 0 {
            return self.config.min_fee_rate;
        }
        
        // Calculate base fee rate of conflicts
        let base_fee_rate = total_conflict_fee / total_conflict_size as u64;
        
        // Apply RBF increment (e.g., 10% higher)
        let increment = base_fee_rate * self.config.rbf_min_fee_increment as u64 / 100;
        
        std::cmp::max(
            base_fee_rate + increment,
            self.config.min_fee_rate
        )
    }
    
    /// Update ancestor metrics for a transaction and its descendants
    fn update_ancestor_metrics(&self, tx_hash: &[u8; 32]) {
        // Get the dependency graph
        let graph = match self.dependency_graph.lock() {
            Ok(g) => g,
            Err(e) => {
                log::error!("Failed to acquire dependency graph lock: {}", e);
                return;
            }
        };
        
        // Get all descendants of this transaction
        let descendants = graph.get_all_descendants(tx_hash);
        
        // Add the transaction itself
        let mut to_update = descendants;
        to_update.insert(*tx_hash);
        
        // Update each transaction's ancestor metrics
        for desc_hash in to_update {
            // Calculate package fee rate including all ancestors
            if let Some(package_fee_rate) = graph.calculate_package_fee_rate(
                &desc_hash,
                &self.transactions.iter().map(|r| (*r.key(), r.transaction.clone())).collect(),
                &self.get_utxo_or_mempool()
            ) {
                // Get ancestors
                let ancestors = graph.get_all_ancestors(&desc_hash);
                
                // Calculate total ancestor size and fee
                let mut ancestor_size = 0;
                let mut ancestor_fee = 0;
                
                if let Some(entry) = self.transactions.get(&desc_hash) {
                    ancestor_size += entry.size;
                    ancestor_fee += entry.fee;
                }
                
                for anc_hash in ancestors {
                    if let Some(entry) = self.transactions.get(&anc_hash) {
                        ancestor_size += entry.size;
                        ancestor_fee += entry.fee;
                    }
                }
                
                // Update the entry
                if let Some(mut entry) = self.transactions.get_mut(&desc_hash) {
                    entry.ancestor_size = ancestor_size;
                    entry.ancestor_fee = ancestor_fee;
                    entry.ancestor_fee_rate = package_fee_rate;
                }
            }
        }
    }
    
    /// Make room in the mempool by removing low-fee transactions
    fn make_room(&self, required_space: usize) -> bool {
        let current_size = match self.size_bytes.read() {
            Ok(size) => *size,
            Err(e) => {
                log::error!("Failed to read mempool size: {}", e);
                return false;
            }
        };
        let target_size = current_size.saturating_sub(required_space);
        
        // Get transactions sorted by fee rate (lowest first)
        let mut entries: Vec<_> = self.transactions.iter().map(|r| {
            (r.key().clone(), r.value().fee_rate)
        }).collect();
        
        entries.sort_by_key(|entry| entry.1);
        
        // Remove transactions until we have enough space
        let mut freed_space = 0;
        let mut to_remove = Vec::new();
        
        for (tx_hash, _) in entries {
            if freed_space >= required_space {
                break;
            }
            
            if let Some(entry) = self.transactions.get(&tx_hash) {
                freed_space += entry.size;
                to_remove.push(tx_hash);
            }
        }
        
        // Actually remove the transactions
        for tx_hash in to_remove {
            let _ = self.remove_transaction(&tx_hash);
        }
        
        // Check if we made enough room
        match self.size_bytes.read() {
            Ok(size) => *size + required_space <= self.config.max_size_bytes,
            Err(e) => {
                log::error!("Failed to read mempool size: {}", e);
                false
            }
        }
    }
    
    /// Create a function that checks both UTXOs and mempool transactions
    fn get_utxo_or_mempool(&self) -> impl Fn(&[u8; 32], u32) -> Option<TransactionOutput> + '_ {
        |tx_hash, output_index| {
            // First try the blockchain UTXO set
            if let Some(output) = (self.get_utxo)(tx_hash, output_index) {
                return Some(output);
            }
            
            // Then check the mempool
            if let Some(entry) = self.transactions.get(tx_hash) {
                if let Some(outputs) = entry.transaction.outputs().get(output_index as usize) {
                    return Some(outputs.clone());
                }
            }
            
            None
        }
    }

    /// Get the best transactions for block creation
    pub fn get_best_transactions(&self, max_size: usize) -> Vec<Transaction> {
        let mut transactions: Vec<Transaction> = self.transactions
            .iter()
            .map(|entry| entry.value().transaction.clone())
            .collect();
        
        // Sort by priority (fee rate)
        let get_utxo = self.get_utxo_or_mempool();
        transactions.sort_by(|a, b| a.compare_by_priority(b, &get_utxo));
        
        // Select transactions up to max_size
        let mut selected = Vec::new();
        let mut total_size = 0;
        
        for tx in transactions {
            let size = tx.calculate_size();
            if total_size + size <= max_size {
                selected.push(tx);
                total_size += size;
            } else {
                break;
            }
        }
        
        selected
    }
    
    /// Get the number of transactions in the pool
    pub fn len(&self) -> usize {
        self.transactions.len()
    }
    
    /// Check if the pool is empty
    pub fn is_empty(&self) -> bool {
        self.transactions.is_empty()
    }
    
    /// Get the current memory usage of the pool
    pub fn memory_usage(&self) -> usize {
        match self.size_bytes.read() {
            Ok(size) => *size,
            Err(e) => {
                log::error!("Failed to read mempool size: {}", e);
                0
            }
        }
    }
}

impl fmt::Debug for TransactionPool {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let orphans_count = self.orphans.read()
            .map(|o| o.len())
            .unwrap_or(0);
        let size_bytes = self.size_bytes.read()
            .map(|s| *s)
            .unwrap_or(0);
            
        f.debug_struct("TransactionPool")
            .field("config", &self.config)
            .field("transactions_count", &self.transactions.len())
            .field("orphans_count", &orphans_count)
            .field("size_bytes", &size_bytes)
            .field("get_utxo", &std::any::type_name_of_val(&self.get_utxo))
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::transaction::{TransactionInput, TransactionOutput};
    
    // Helper function to create a test transaction
    fn create_test_tx(
        inputs: Vec<(Vec<u8>, u32)>,
        output_amount: u64
    ) -> Transaction {
        let tx_inputs = inputs.into_iter().map(|(prev_hash, index)| {
            let mut hash = [0u8; 32];
            hash[..prev_hash.len()].copy_from_slice(&prev_hash);
            TransactionInput::new(hash, index, vec![], 0xffffffff)
        }).collect();
        
        let outputs = vec![TransactionOutput::new(output_amount, vec![])];
        
        Transaction::new(1, tx_inputs, outputs, 0)
    }
    
    #[test]
    fn test_add_and_get_transaction() {
        let get_utxo = |tx_hash: &[u8; 32], index: u32| {
            // Only return UTXOs for specific inputs
            if tx_hash[0] == 1 && index == 0 {
                Some(TransactionOutput::new(100_000, vec![]))
            } else {
                None
            }
        };
        
        let config = TransactionPoolConfig::default();
        let mempool = TransactionPool::new(config, get_utxo);
        
        // Create a valid transaction
        let tx = create_test_tx(vec![(vec![1], 0)], 90_000); // 10_000 fee
        let tx_hash = tx.hash();
        
        // Add to mempool
        assert!(mempool.add_transaction(tx.clone()).is_ok());
        
        // Verify it's in the mempool
        assert!(mempool.get_transaction(&tx_hash).is_some());
        
        // Try to add it again
        assert!(matches!(
            mempool.add_transaction(tx.clone()),
            Err(MempoolError::AlreadyExists)
        ));
        
        // Check count and size
        assert_eq!(mempool.count(), 1);
        assert!(mempool.size() > 0);
    }
    
    #[test]
    fn test_fee_too_low() {
        let get_utxo = |tx_hash: &[u8; 32], index: u32| {
            // Only return UTXOs for specific inputs
            if tx_hash[0] == 1 && index == 0 {
                Some(TransactionOutput::new(100_000, vec![]))
            } else {
                None
            }
        };
        
        // Set high minimum fee rate
        let mut config = TransactionPoolConfig::default();
        config.min_fee_rate = 100; // 100 sat/byte
        
        let mempool = TransactionPool::new(config, get_utxo);
        
        // Create a transaction with low fee
        let tx = create_test_tx(vec![(vec![1], 0)], 99_000); // 1_000 fee, but large enough tx
        
        // Try to add it
        let result = mempool.add_transaction(tx);
        
        // Should fail due to low fee rate
        assert!(matches!(result, Err(MempoolError::FeeTooLow { .. })));
    }
    
    #[test]
    fn test_replace_by_fee() {
        let get_utxo = |tx_hash: &[u8; 32], index: u32| {
            // Only return UTXOs for specific inputs
            if tx_hash[0] == 1 && index == 0 {
                Some(TransactionOutput::new(100_000, vec![]))
            } else {
                None
            }
        };
        
        // Enable RBF with 10% increment
        let mut config = TransactionPoolConfig::default();
        config.enable_replace_by_fee = true;
        config.rbf_min_fee_increment = 10;
        
        let mempool = TransactionPool::new(config, get_utxo);
        
        // Create first transaction
        let tx1 = create_test_tx(vec![(vec![1], 0)], 90_000); // 10_000 fee
        
        // Add to mempool
        assert!(mempool.add_transaction(tx1.clone()).is_ok());
        
        // Create second transaction with slightly higher fee, spending same input
        let tx2 = create_test_tx(vec![(vec![1], 0)], 89_000); // 11_000 fee
        
        // Add to mempool - should replace tx1
        assert!(mempool.add_transaction(tx2.clone()).is_ok());
        
        // Verify tx1 was replaced with tx2
        assert!(mempool.get_transaction(&tx1.hash()).is_none());
        assert!(mempool.get_transaction(&tx2.hash()).is_some());
    }
    
    #[test]
    fn test_orphan_handling() {
        let get_utxo = |tx_hash: &[u8; 32], _index: u32| {
            // No UTXOs for this test
            None
        };
        
        let config = TransactionPoolConfig::default();
        let mempool = TransactionPool::new(config, get_utxo);
        
        // Create an orphan transaction (spending unknown input)
        let tx1 = create_test_tx(vec![(vec![1], 0)], 90_000);
        
        // Should be accepted as an orphan
        assert!(mempool.add_transaction(tx1.clone()).is_ok());
        
        // Should not be in the main mempool
        assert!(mempool.get_transaction(&tx1.hash()).is_none());
        
        // Now create a transaction that the orphan depends on
        let tx2 = create_test_tx(vec![(vec![2], 0)], 95_000);
        let mut tx2_hash = [0u8; 32];
        tx2_hash[0] = 1; // Match the input of tx1
        
        // Manually insert tx2 to simulate it being mined
        let entry = MempoolEntry {
            transaction: tx2.clone(),
            time_added: Instant::now(),
            fee: 5_000,
            fee_rate: 5,
            size: 1000,
            ancestor_size: 1000,
            ancestor_fee: 5_000,
            ancestor_fee_rate: 5,
        };
        
        mempool.transactions.insert(tx2_hash, entry);
        
        // Process orphans
        mempool.process_orphans(&tx2_hash);
        
        // Now tx1 should be in the mempool
        assert!(mempool.get_transaction(&tx1.hash()).is_some());
    }
} 