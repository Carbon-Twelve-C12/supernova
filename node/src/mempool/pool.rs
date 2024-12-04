use std::sync::Arc;
use dashmap::DashMap;
use btclib::types::transaction::Transaction;
use std::time::{Duration, SystemTime};

/// Configuration for the mempool
#[derive(Debug, Clone)]
pub struct MempoolConfig {
    /// Maximum number of transactions in the pool
    max_size: usize,
    /// Maximum age of a transaction before expiry (in seconds)
    max_age: u64,
    /// Minimum fee rate (satoshis per byte) for acceptance
    min_fee_rate: u64,
}

impl Default for MempoolConfig {
    fn default() -> Self {
        Self {
            max_size: 5000,         // Default to 5000 transactions
            max_age: 72 * 3600,     // 72 hours in seconds
            min_fee_rate: 1,        // 1 satoshi per byte
        }
    }
}

/// Entry in the mempool containing a transaction and metadata
#[derive(Debug)]
struct MempoolEntry {
    transaction: Transaction,
    timestamp: SystemTime,
    fee_rate: u64,    // Satoshis per byte
    size: usize,      // Size in bytes
}

/// Thread-safe transaction pool implementation
pub struct TransactionPool {
    /// Main storage using DashMap for thread-safety
    transactions: DashMap<[u8; 32], MempoolEntry>,
    /// Configuration settings
    config: MempoolConfig,
}

impl TransactionPool {
    /// Create a new transaction pool with given configuration
    pub fn new(config: MempoolConfig) -> Self {
        Self {
            transactions: DashMap::new(),
            config,
        }
    }

    /// Add a transaction to the pool
    pub fn add_transaction(&self, transaction: Transaction, fee_rate: u64) -> Result<(), MempoolError> {
        let tx_hash = transaction.hash();
        
        // Check if transaction already exists
        if self.transactions.contains_key(&tx_hash) {
            return Err(MempoolError::DuplicateTransaction);
        }

        // Check pool size limit
        if self.transactions.len() >= self.config.max_size {
            return Err(MempoolError::PoolFull);
        }

        // Check minimum fee rate
        if fee_rate < self.config.min_fee_rate {
            return Err(MempoolError::FeeTooLow);
        }

        // Calculate transaction size
        let tx_size = bincode::serialize(&transaction)
            .map_err(|_| MempoolError::SerializationError)?
            .len();

        // Create and insert new entry
        let entry = MempoolEntry {
            transaction,
            timestamp: SystemTime::now(),
            fee_rate,
            size: tx_size,
        };

        self.transactions.insert(tx_hash, entry);
        Ok(())
    }

    /// Remove a transaction from the pool
    pub fn remove_transaction(&self, tx_hash: &[u8; 32]) -> Option<Transaction> {
        self.transactions.remove(tx_hash).map(|(_, entry)| entry.transaction)
    }

    /// Get a transaction by its hash
    pub fn get_transaction(&self, tx_hash: &[u8; 32]) -> Option<Transaction> {
        self.transactions.get(tx_hash).map(|entry| entry.transaction.clone())
    }

    /// Clear expired transactions from the pool
    pub fn clear_expired(&self) -> usize {
        let now = SystemTime::now();
        let max_age = Duration::from_secs(self.config.max_age);
        let mut removed = 0;

        self.transactions.retain(|_, entry| {
            let age = now.duration_since(entry.timestamp).unwrap_or(Duration::ZERO);
            if age > max_age {
                removed += 1;
                false
            } else {
                true
            }
        });

        removed
    }

    /// Check for double-spend attempts
    pub fn check_double_spend(&self, transaction: &Transaction) -> bool {
        // Get all input references from the new transaction
        let new_inputs: Vec<_> = transaction.inputs()
            .iter()
            .map(|input| (input.prev_tx_hash(), input.prev_output_index()))
            .collect();

        // Check if any existing transaction uses the same inputs
        for entry in self.transactions.iter() {
            let existing_inputs: Vec<_> = entry.transaction.inputs()
                .iter()
                .map(|input| (input.prev_tx_hash(), input.prev_output_index()))
                .collect();

            // Check for any overlap in inputs
            for input in &new_inputs {
                if existing_inputs.contains(input) {
                    return true;
                }
            }
        }

        false
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
}

#[derive(Debug, thiserror::Error)]
pub enum MempoolError {
    #[error("Transaction already exists in mempool")]
    DuplicateTransaction,
    #[error("Mempool is full")]
    PoolFull,
    #[error("Transaction fee rate is too low")]
    FeeTooLow,
    #[error("Double spend detected")]
    DoubleSpend,
    #[error("Failed to serialize transaction")]
    SerializationError,
}

#[cfg(test)]
mod tests {
    use super::*;
    use btclib::types::transaction::{TransactionInput, TransactionOutput};

    fn create_test_transaction(prev_hash: [u8; 32], value: u64) -> Transaction {
        Transaction::new(
            1,
            vec![TransactionInput::new(prev_hash, 0, vec![], 0xffffffff)],
            vec![TransactionOutput::new(value, vec![])],
            0,
        )
    }

    #[test]
    fn test_add_and_get_transaction() {
        let config = MempoolConfig::default();
        let pool = TransactionPool::new(config);
        
        let tx = create_test_transaction([1u8; 32], 50_000_000);
        let tx_hash = tx.hash();
        
        assert!(pool.add_transaction(tx.clone(), 2).is_ok());
        assert_eq!(pool.get_transaction(&tx_hash).unwrap(), tx);
    }

    #[test]
    fn test_double_spend_detection() {
        let config = MempoolConfig::default();
        let pool = TransactionPool::new(config);
        
        // Add first transaction
        let tx1 = create_test_transaction([1u8; 32], 50_000_000);
        assert!(pool.add_transaction(tx1, 2).is_ok());
        
        // Try to add second transaction spending same output
        let tx2 = create_test_transaction([1u8; 32], 40_000_000);
        assert!(pool.check_double_spend(&tx2));
    }

    #[test]
    fn test_fee_rate_sorting() {
        let config = MempoolConfig::default();
        let pool = TransactionPool::new(config);
        
        let tx1 = create_test_transaction([1u8; 32], 50_000_000);
        let tx2 = create_test_transaction([2u8; 32], 40_000_000);
        
        pool.add_transaction(tx1.clone(), 1).unwrap();
        pool.add_transaction(tx2.clone(), 2).unwrap();
        
        let sorted = pool.get_sorted_transactions();
        assert_eq!(sorted[0], tx2); // Higher fee rate should be first
        assert_eq!(sorted[1], tx1);
    }
}