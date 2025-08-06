//! Atomic transaction pool implementation for Supernova
//!
//! This module provides a thread-safe, race-condition-free mempool implementation
//! that prevents double-spending attacks through atomic operations.

use dashmap::DashMap;
use btclib::types::transaction::{Transaction, TransactionInput};
use std::time::{Duration, SystemTime};
use std::sync::{Arc, Mutex, RwLock};
use std::collections::{HashMap, HashSet};
use parking_lot::{Mutex as PMutex, RwLock as PRwLock};
use crate::config;
use crate::api::types::{MempoolInfo, MempoolTransaction, TransactionFees, TransactionValidationResult, MempoolTransactionSubmissionResponse};
use crate::mempool::pool::MempoolConfig;
use crate::mempool::MempoolError;
use hex;

/// Input reference for tracking double-spends
#[derive(Debug, Clone, Hash, Eq, PartialEq)]
struct InputRef {
    tx_hash: [u8; 32],
    output_index: u32,
}

/// Entry in the atomic mempool
#[derive(Debug, Clone)]
struct AtomicMempoolEntry {
    transaction: Transaction,
    timestamp: SystemTime,
    fee_rate: u64,
    size: usize,
    inputs: Vec<InputRef>,
}

/// Global lock for critical mempool operations
/// This ensures atomicity across all mempool modifications
struct MempoolLock {
    /// Lock for adding/removing transactions
    modification_lock: PMutex<()>,
}

/// Thread-safe atomic transaction pool
pub struct AtomicTransactionPool {
    /// Main storage for transactions
    transactions: DashMap<[u8; 32], AtomicMempoolEntry>,
    /// Index of spent outputs to detect double-spends atomically
    spent_outputs: DashMap<InputRef, [u8; 32]>,
    /// Global operation lock for atomic modifications
    op_lock: Arc<MempoolLock>,
    /// Configuration
    config: MempoolConfig,
    /// Metrics for tracking operations
    metrics: Arc<RwLock<MempoolMetrics>>,
}

/// Metrics for monitoring mempool operations
#[derive(Debug, Default)]
struct MempoolMetrics {
    total_added: u64,
    total_removed: u64,
    double_spend_attempts: u64,
    rbf_replacements: u64,
}

impl AtomicTransactionPool {
    /// Create a new atomic transaction pool
    pub fn new(config: MempoolConfig) -> Self {
        Self {
            transactions: DashMap::new(),
            spent_outputs: DashMap::new(),
            op_lock: Arc::new(MempoolLock {
                modification_lock: PMutex::new(()),
            }),
            config,
            metrics: Arc::new(RwLock::new(MempoolMetrics::default())),
        }
    }

    /// Add a transaction to the pool atomically
    pub fn add_transaction(&self, transaction: Transaction, fee_rate: u64) -> Result<(), MempoolError> {
        // Acquire modification lock for atomic operation
        let _lock = self.op_lock.modification_lock.lock();
        
        let tx_hash = transaction.hash();
        
        // Check if transaction already exists
        if self.transactions.contains_key(&tx_hash) {
            return Err(MempoolError::TransactionExists(hex::encode(tx_hash)));
        }

        // Check pool size limit
        if self.transactions.len() >= self.config.max_size {
            return Err(MempoolError::MempoolFull { 
                current: self.transactions.len(), 
                max: self.config.max_size 
            });
        }

        // Check minimum fee rate
        if fee_rate < self.config.min_fee_rate {
            return Err(MempoolError::FeeTooLow { 
                required: self.config.min_fee_rate, 
                provided: fee_rate 
            });
        }

        // Calculate transaction size
        let tx_size = bincode::serialize(&transaction)
            .map_err(|e| MempoolError::SerializationError(e.to_string()))?
            .len();

        // Extract input references
        let inputs: Vec<InputRef> = transaction.inputs()
            .iter()
            .map(|input| InputRef {
                tx_hash: input.prev_tx_hash(),
                output_index: input.prev_output_index(),
            })
            .collect();

        // Atomically check for double-spends
        for input_ref in &inputs {
            if let Some(existing_tx) = self.spent_outputs.get(input_ref) {
                // Double-spend detected!
                self.metrics.write()
                    .map_err(|e| MempoolError::LockError(format!("Failed to acquire metrics lock: {}", e)))?
                    .double_spend_attempts += 1;
                return Err(MempoolError::DoubleSpend(hex::encode(*existing_tx.value())));
            }
        }

        // All checks passed, now atomically add the transaction
        
        // Mark all inputs as spent by this transaction
        for input_ref in &inputs {
            self.spent_outputs.insert(input_ref.clone(), tx_hash);
        }

        // Create and insert the entry
        let entry = AtomicMempoolEntry {
            transaction,
            timestamp: SystemTime::now(),
            fee_rate,
            size: tx_size,
            inputs,
        };

        self.transactions.insert(tx_hash, entry);
        
        // Update metrics
        self.metrics.write()
            .map_err(|e| MempoolError::LockError(format!("Failed to acquire metrics lock: {}", e)))?
            .total_added += 1;

        Ok(())
    }

    /// Remove a transaction from the pool atomically
    pub fn remove_transaction(&self, tx_hash: &[u8; 32]) -> Option<Transaction> {
        // Acquire modification lock for atomic operation
        let _lock = self.op_lock.modification_lock.lock();
        
        // Remove the transaction
        match self.transactions.remove(tx_hash) {
            Some((_, entry)) => {
                // Remove all spent output entries for this transaction
                for input_ref in &entry.inputs {
                    self.spent_outputs.remove(input_ref);
                }
                
                // Update metrics
                if let Ok(mut metrics) = self.metrics.write() {
                    metrics.total_removed += 1;
                } else {
                    tracing::error!("Failed to acquire metrics lock during transaction removal");
                }
                
                Some(entry.transaction)
            }
            None => None
        }
    }

    /// Replace transaction atomically (RBF)
    pub fn replace_transaction(&self, new_transaction: Transaction, fee_rate: u64) -> Result<Vec<Transaction>, MempoolError> {
        // Check if RBF is enabled
        if !self.config.enable_rbf {
            return Err(MempoolError::InvalidTransaction("Replace-By-Fee is disabled".to_string()));
        }

        // Acquire modification lock for atomic operation
        let _lock = self.op_lock.modification_lock.lock();
        
        let new_tx_hash = new_transaction.hash();
        
        // Check if the new transaction already exists
        if self.transactions.contains_key(&new_tx_hash) {
            return Err(MempoolError::TransactionExists(hex::encode(new_tx_hash)));
        }

        // Extract new transaction inputs
        let new_inputs: Vec<InputRef> = new_transaction.inputs()
            .iter()
            .map(|input| InputRef {
                tx_hash: input.prev_tx_hash(),
                output_index: input.prev_output_index(),
            })
            .collect();

        // Find all conflicting transactions
        let mut conflicting_txs = Vec::new();
        let mut conflicting_hashes = HashSet::new();
        
        for input_ref in &new_inputs {
            if let Some(existing_tx_hash) = self.spent_outputs.get(input_ref) {
                conflicting_hashes.insert(*existing_tx_hash.value());
            }
        }

        if conflicting_hashes.is_empty() {
            return Err(MempoolError::InvalidTransaction("No conflicting transactions found for RBF".to_string()));
        }

        // Calculate total fees of conflicting transactions
        let mut total_conflicting_fee = 0u64;
        let mut total_conflicting_size = 0usize;
        
        for tx_hash in &conflicting_hashes {
            if let Some(entry) = self.transactions.get(tx_hash) {
                // Use checked arithmetic to prevent overflow
                let tx_fee = match entry.fee_rate.checked_mul(entry.size as u64) {
                    Some(fee) => fee,
                    None => return Err(MempoolError::SerializationError("Fee calculation overflow".to_string())),
                };
                
                total_conflicting_fee = match total_conflicting_fee.checked_add(tx_fee) {
                    Some(total) => total,
                    None => return Err(MempoolError::SerializationError("Total fee overflow".to_string())),
                };
                
                total_conflicting_size = match total_conflicting_size.checked_add(entry.size) {
                    Some(total) => total,
                    None => return Err(MempoolError::SerializationError("Size overflow".to_string())),
                };
                
                conflicting_txs.push(entry.transaction.clone());
            }
        }

        // Calculate new transaction size and fee
        let new_tx_size = bincode::serialize(&new_transaction)
            .map_err(|e| MempoolError::SerializationError(e.to_string()))?
            .len();
            
        // Use checked multiplication for fee calculation
        let new_tx_fee = match fee_rate.checked_mul(new_tx_size as u64) {
            Some(fee) => fee,
            None => return Err(MempoolError::SerializationError("Fee calculation overflow".to_string())),
        };

        // Check if fee increase is sufficient
        let min_increase = 1.0 + (self.config.min_rbf_fee_increase / 100.0);
        let min_required_fee = ((total_conflicting_fee as f64) * min_increase) as u64;
        
        if new_tx_fee < min_required_fee {
            return Err(MempoolError::FeeTooLow { 
                required: min_required_fee, 
                provided: new_tx_fee 
            });
        }

        // Atomically remove all conflicting transactions
        for tx_hash in &conflicting_hashes {
            if let Some((_, entry)) = self.transactions.remove(tx_hash) {
                // Remove spent outputs
                for input_ref in &entry.inputs {
                    self.spent_outputs.remove(input_ref);
                }
            }
        }

        // Add the new transaction
        for input_ref in &new_inputs {
            self.spent_outputs.insert(input_ref.clone(), new_tx_hash);
        }

        let entry = AtomicMempoolEntry {
            transaction: new_transaction,
            timestamp: SystemTime::now(),
            fee_rate,
            size: new_tx_size,
            inputs: new_inputs,
        };

        self.transactions.insert(new_tx_hash, entry);
        
        // Update metrics
        self.metrics.write()
            .map_err(|e| MempoolError::LockError(format!("Failed to acquire metrics lock: {}", e)))?
            .rbf_replacements += 1;

        Ok(conflicting_txs)
    }

    /// Get a transaction by its hash (no lock needed for reads)
    pub fn get_transaction(&self, tx_hash: &[u8; 32]) -> Option<Transaction> {
        self.transactions.get(tx_hash).map(|entry| entry.transaction.clone())
    }

    /// Check if inputs are already spent (atomic check)
    pub fn check_double_spend(&self, transaction: &Transaction) -> bool {
        let inputs: Vec<InputRef> = transaction.inputs()
            .iter()
            .map(|input| InputRef {
                tx_hash: input.prev_tx_hash(),
                output_index: input.prev_output_index(),
            })
            .collect();

        // Check atomically
        for input_ref in &inputs {
            if self.spent_outputs.contains_key(input_ref) {
                return true;
            }
        }
        
        false
    }

    /// Clear expired transactions atomically
    pub fn clear_expired(&self) -> usize {
        let _lock = self.op_lock.modification_lock.lock();
        
        let now = SystemTime::now();
        let max_age = Duration::from_secs(self.config.max_age);
        let mut removed = 0;
        let mut to_remove = Vec::new();

        // Find expired transactions
        for entry in self.transactions.iter() {
            let age = now.duration_since(entry.timestamp).unwrap_or(Duration::ZERO);
            if age > max_age {
                to_remove.push(*entry.key());
            }
        }

        // Remove them atomically
        for tx_hash in to_remove {
            if let Some((_, entry)) = self.transactions.remove(&tx_hash) {
                // Remove spent outputs
                for input_ref in &entry.inputs {
                    self.spent_outputs.remove(input_ref);
                }
                removed += 1;
            }
        }

        if let Ok(mut metrics) = self.metrics.write() {
            metrics.total_removed += removed as u64;
        } else {
            tracing::error!("Failed to acquire metrics lock during expired transaction cleanup");
        }
        removed
    }

    /// Get all transactions sorted by fee rate
    pub fn get_sorted_transactions(&self) -> Vec<Transaction> {
        let mut entries: Vec<_> = self.transactions
            .iter()
            .map(|ref_multi| {
                (ref_multi.transaction.clone(), ref_multi.fee_rate)
            })
            .collect();

        // Sort by fee rate in descending order
        entries.sort_by(|a, b| b.1.cmp(&a.1));
        
        entries.into_iter().map(|(tx, _)| tx).collect()
    }

    /// Get mempool info
    pub fn get_info(&self) -> MempoolInfo {
        let transaction_count = self.transactions.len();
        
        // Calculate total size with overflow protection
        let total_size: usize = self.transactions
            .iter()
            .map(|entry| entry.size)
            .fold(0usize, |acc, size| acc.saturating_add(size));
            
        // Calculate total fee with overflow protection
        let total_fee: u64 = self.transactions
            .iter()
            .map(|entry| {
                entry.fee_rate
                    .checked_mul(entry.size as u64)
                    .unwrap_or(u64::MAX)
            })
            .fold(0u64, |acc, fee| acc.saturating_add(fee));
        
        // Calculate average fee rate safely
        let avg_fee_rate = if transaction_count > 0 && total_size > 0 {
            total_fee.saturating_div(total_size as u64)
        } else {
            0
        };

        let metrics = match self.metrics.read() {
            Ok(m) => m,
            Err(e) => {
                tracing::error!("Failed to acquire metrics lock for info: {}", e);
                // Return info without metrics data
                return MempoolInfo {
                    transaction_count,
                    total_size,
                    total_fee,
                    min_fee_rate: 0,
                    max_fee_rate: 0,
                    avg_fee_rate,
                };
            }
        };
        
        MempoolInfo {
            transaction_count,
            total_size,
            total_fee,
            min_fee_rate: self.config.min_fee_rate,
            max_fee_rate: self.transactions.iter().map(|entry| entry.fee_rate).max().unwrap_or(0),
            avg_fee_rate,
        }
    }

    /// Get current mempool size
    pub fn size(&self) -> usize {
        self.transactions.len()
    }

    /// Get metrics
    pub fn get_metrics(&self) -> (u64, u64, u64, u64) {
        match self.metrics.read() {
            Ok(metrics) => (
                metrics.total_added,
                metrics.total_removed,
                metrics.double_spend_attempts,
                metrics.rbf_replacements
            ),
            Err(e) => {
                tracing::error!("Failed to acquire metrics lock: {}", e);
                (0, 0, 0, 0)
            }
        }
    }

    /// Clear all transactions (atomic)
    pub fn clear_all(&self) -> Result<(), MempoolError> {
        let _lock = self.op_lock.modification_lock.lock();
        
        self.transactions.clear();
        self.spent_outputs.clear();
        
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use btclib::types::transaction::{TransactionInput, TransactionOutput};
    use std::thread;
    use std::sync::Arc;

    fn create_test_transaction(inputs: Vec<([u8; 32], u32)>, value: u64) -> Transaction {
        let tx_inputs = inputs.into_iter().map(|(prev_hash, index)| {
            TransactionInput::new(prev_hash, index, vec![], 0xffffffff)
        }).collect();

        let outputs = vec![TransactionOutput::new(value, vec![])];
        Transaction::new(1, tx_inputs, outputs, 0)
    }

    #[test]
    fn test_atomic_double_spend_prevention() {
        let config = MempoolConfig::default();
        let pool = Arc::new(AtomicTransactionPool::new(config));
        
        // Create two transactions spending the same output
        let tx1 = create_test_transaction(vec![([1u8; 32], 0)], 50_000_000);
        let tx2 = create_test_transaction(vec![([1u8; 32], 0)], 40_000_000);
        
        // Add first transaction
        assert!(pool.add_transaction(tx1.clone(), 2).is_ok());
        
        // Try to add second transaction - should fail
        assert!(matches!(
            pool.add_transaction(tx2, 2),
            Err(MempoolError::DoubleSpend(ref tx_id)) if tx_id == &hex::encode(tx1.hash())
        ));
        
        // Check metrics
        let (added, _, double_spends, _) = pool.get_metrics();
        assert_eq!(added, 1);
        assert_eq!(double_spends, 1);
    }

    #[test]
    fn test_concurrent_double_spend_attempts() {
        let config = MempoolConfig::default();
        let pool = Arc::new(AtomicTransactionPool::new(config));
        
        // Create multiple transactions spending the same output
        let transactions: Vec<_> = (0..10).map(|i| {
            create_test_transaction(vec![([1u8; 32], 0)], 50_000_000 - i * 1000)
        }).collect();
        
        // Try to add them concurrently
        let handles: Vec<_> = transactions.into_iter().map(|tx| {
            let pool_clone = Arc::clone(&pool);
            thread::spawn(move || {
                pool_clone.add_transaction(tx, 2)
            })
        }).collect();
        
        // Wait for all threads
        let results: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();
        
        // Exactly one should succeed
        let successes = results.iter().filter(|r| r.is_ok()).count();
        assert_eq!(successes, 1);
        
        // Pool should contain exactly one transaction
        assert_eq!(pool.size(), 1);
    }

    #[test]
    fn test_atomic_rbf_replacement() {
        let config = MempoolConfig {
            enable_rbf: true,
            min_rbf_fee_increase: 10.0,
            ..MempoolConfig::default()
        };
        let pool = Arc::new(AtomicTransactionPool::new(config));
        
        // Add initial transaction
        let tx1 = create_test_transaction(vec![([1u8; 32], 0)], 50_000_000);
        assert!(pool.add_transaction(tx1.clone(), 100).is_ok());
        
        // Replace with higher fee transaction
        let tx2 = create_test_transaction(vec![([1u8; 32], 0)], 49_000_000);
        let result = pool.replace_transaction(tx2.clone(), 120);
        
        assert!(result.is_ok());
        let replaced = result.unwrap();
        assert_eq!(replaced.len(), 1);
        assert_eq!(replaced[0].hash(), tx1.hash());
        
        // Check that old transaction is gone and new one is present
        assert!(pool.get_transaction(&tx1.hash()).is_none());
        assert!(pool.get_transaction(&tx2.hash()).is_some());
        
        // Check metrics
        let (_, _, _, rbf_replacements) = pool.get_metrics();
        assert_eq!(rbf_replacements, 1);
    }

    #[test]
    fn test_atomic_clear_expired() {
        let mut config = MempoolConfig::default();
        config.max_age = 0; // Expire immediately
        let pool = Arc::new(AtomicTransactionPool::new(config));
        
        // Add some transactions
        for i in 0..5 {
            let tx = create_test_transaction(vec![([i; 32], 0)], 50_000_000);
            assert!(pool.add_transaction(tx, 2).is_ok());
        }
        
        // Sleep to ensure transactions are expired
        thread::sleep(Duration::from_millis(10));
        
        // Clear expired
        let removed = pool.clear_expired();
        assert_eq!(removed, 5);
        assert_eq!(pool.size(), 0);
        
        // Verify spent outputs are also cleared
        assert_eq!(pool.spent_outputs.len(), 0);
    }

    #[test]
    fn test_concurrent_add_remove() {
        let config = MempoolConfig::default();
        let pool = Arc::new(AtomicTransactionPool::new(config));
        
        // Create unique transactions
        let transactions: Vec<_> = (0..100).map(|i| {
            create_test_transaction(vec![([i as u8; 32], i as u32)], 50_000_000)
        }).collect();
        
        let tx_hashes: Vec<_> = transactions.iter().map(|tx| tx.hash()).collect();
        
        // Add transactions concurrently
        let add_handles: Vec<_> = transactions.into_iter().enumerate().map(|(i, tx)| {
            let pool_clone = Arc::clone(&pool);
            thread::spawn(move || {
                pool_clone.add_transaction(tx, (i + 1) as u64)
            })
        }).collect();
        
        // Wait for adds to complete
        for h in add_handles {
            assert!(h.join().unwrap().is_ok());
        }
        
        assert_eq!(pool.size(), 100);
        
        // Remove half of them concurrently
        let remove_handles: Vec<_> = tx_hashes[..50].iter().map(|hash| {
            let pool_clone = Arc::clone(&pool);
            let hash = *hash;
            thread::spawn(move || {
                pool_clone.remove_transaction(&hash)
            })
        }).collect();
        
        // Wait for removes
        for h in remove_handles {
            assert!(h.join().unwrap().is_some());
        }
        
        assert_eq!(pool.size(), 50);
    }
} 