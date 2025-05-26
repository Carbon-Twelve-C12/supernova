use dashmap::DashMap;
use btclib::types::transaction::Transaction;
use std::time::{Duration, SystemTime};
use crate::config;
use crate::api::types::{MempoolInfo, MempoolTransaction, TransactionFees, TransactionValidationResult, MempoolTransactionSubmissionResponse};
use hex;

/// Configuration for the transaction memory pool
#[derive(Debug, Clone)]
pub struct MempoolConfig {
    /// Maximum number of transactions in the pool
    pub max_size: usize,
    /// Maximum age of a transaction before expiry (in seconds)
    pub max_age: u64,
    /// Minimum fee rate (satoshis per byte) for acceptance
    pub min_fee_rate: u64,
    /// Whether Replace-By-Fee is enabled
    pub enable_rbf: bool,
    /// Minimum fee increase required for RBF (as a percentage)
    pub min_rbf_fee_increase: f64,
}

impl From<config::MempoolConfig> for MempoolConfig {
    fn from(config: config::MempoolConfig) -> Self {
        Self {
            max_size: config.max_size,
            max_age: config.transaction_timeout.as_secs(),
            min_fee_rate: config.min_fee_rate as u64,
            enable_rbf: config.enable_rbf,
            min_rbf_fee_increase: config.min_rbf_fee_increase,
        }
    }
}

impl Default for MempoolConfig {
    fn default() -> Self {
        Self {
            max_size: 5000,         // Default to 5000 transactions
            max_age: 72 * 3600,     // 72 hours in seconds
            min_fee_rate: 1,        // 1 satoshi per byte
            enable_rbf: true,       // Enable RBF by default
            min_rbf_fee_increase: 10.0, // 10% minimum fee increase
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
        match self.transactions.remove(tx_hash) {
            Some((_, entry)) => Some(entry.transaction),
            None => None
        }
    }

    /// Get a transaction by its hash
    pub fn get_transaction(&self, tx_hash: &[u8; 32]) -> Option<Transaction> {
        match self.transactions.get(tx_hash) {
            Some(entry) => Some(entry.transaction.clone()),
            None => None
        }
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

    /// Clear all transactions from the pool
    pub fn clear_all(&self) -> Result<(), MempoolError> {
        self.transactions.clear();
        Ok(())
    }

    /// Attempt to replace an existing transaction with a higher-fee version (RBF)
    pub fn replace_transaction(&self, new_transaction: Transaction, fee_rate: u64) -> Result<Option<Transaction>, MempoolError> {
        // Check if RBF is enabled
        if !self.config.enable_rbf {
            return Err(MempoolError::RbfDisabled);
        }
        
        let tx_hash = new_transaction.hash();
        
        // Check if the transaction already exists
        if self.transactions.contains_key(&tx_hash) {
            return Err(MempoolError::DuplicateTransaction);
        }
        
        // Find transactions in the mempool that have inputs overlapping with the new transaction
        let conflicting_txs: Vec<([u8; 32], MempoolEntry)> = self.find_conflicting_transactions(&new_transaction);
        
        if conflicting_txs.is_empty() {
            // No conflicts, this is not an RBF but a new transaction
            return Err(MempoolError::NoConflictingTransactions);
        }
        
        // Calculate the total fee of the conflicting transactions
        let total_conflicting_size: usize = conflicting_txs.iter().map(|(_, entry)| entry.size).sum();
        let total_conflicting_fee: u64 = conflicting_txs.iter().map(|(_, entry)| entry.fee_rate * entry.size as u64).sum();
        
        // Calculate the new transaction size
        let new_tx_size = match bincode::serialize(&new_transaction) {
            Ok(bytes) => bytes.len(),
            Err(_) => return Err(MempoolError::SerializationError),
        };
        
        // Calculate the new transaction fee
        let new_tx_fee = fee_rate * new_tx_size as u64;
        
        // Check if the new transaction's fee is sufficiently higher than the conflicting transactions
        let min_increase = 1.0 + (self.config.min_rbf_fee_increase / 100.0);
        let min_required_fee = ((total_conflicting_fee as f64) * min_increase) as u64;
        
        if new_tx_fee < min_required_fee {
            return Err(MempoolError::InsufficientFeeIncrease(min_required_fee));
        }
        
        // Remove all conflicting transactions
        let mut removed_txs = Vec::new();
        for (hash, entry) in conflicting_txs {
            if let Some((_, entry)) = self.transactions.remove(&hash) {
                removed_txs.push(entry.transaction);
            }
        }
        
        // Add the new transaction
        let new_entry = MempoolEntry {
            transaction: new_transaction.clone(),
            timestamp: SystemTime::now(),
            fee_rate,
            size: new_tx_size,
        };
        
        self.transactions.insert(tx_hash, new_entry);
        
        // If we only replaced one transaction, return it
        if removed_txs.len() == 1 {
            Ok(Some(removed_txs.remove(0)))
        } else {
            // We replaced multiple transactions, return None to indicate this wasn't a 1:1 replacement
            Ok(None)
        }
    }
    
    /// Find transactions in the mempool that have inputs overlapping with the new transaction
    fn find_conflicting_transactions(&self, transaction: &Transaction) -> Vec<([u8; 32], MempoolEntry)> {
        // Get all input references from the new transaction
        let new_inputs: Vec<_> = transaction.inputs()
            .iter()
            .map(|input| (input.prev_tx_hash(), input.prev_output_index()))
            .collect();
        
        let mut conflicting_txs = Vec::new();
        
        // Check all transactions in the mempool for conflicts
        for entry in self.transactions.iter() {
            let tx_hash = *entry.key();
            let existing_inputs: Vec<_> = entry.transaction.inputs()
                .iter()
                .map(|input| (input.prev_tx_hash(), input.prev_output_index()))
                .collect();
            
            // Check for any overlap in inputs
            for input in &new_inputs {
                if existing_inputs.contains(input) {
                    conflicting_txs.push((tx_hash, entry.clone()));
                    break;
                }
            }
        }
        
        conflicting_txs
    }

    /// Get mempool information for API
    pub fn get_info(&self) -> MempoolInfo {
        let transaction_count = self.transactions.len();
        let total_size: usize = self.transactions.iter().map(|entry| entry.size).sum();
        let total_fee: u64 = self.transactions.iter().map(|entry| entry.fee_rate * entry.size as u64).sum();
        
        // Calculate average fee rate
        let avg_fee_rate = if transaction_count > 0 {
            total_fee / total_size as u64
        } else {
            0
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
    
    /// Get transactions with pagination and sorting
    pub fn get_transactions(&self, limit: usize, offset: usize, sort: &str) -> Result<Vec<MempoolTransaction>, MempoolError> {
        let mut entries: Vec<_> = self.transactions.iter().map(|entry| {
            let tx_hash = *entry.key();
            let entry_val = entry.value();
            MempoolTransaction {
                txid: hex::encode(tx_hash),
                size: entry_val.size,
                fee: entry_val.fee_rate * entry_val.size as u64,
                fee_rate: entry_val.fee_rate,
                time: entry_val.timestamp.duration_since(SystemTime::UNIX_EPOCH).unwrap_or_default().as_secs(),
            }
        }).collect();
        
        // Sort based on the sort parameter
        match sort {
            "fee_desc" => entries.sort_by(|a, b| b.fee.cmp(&a.fee)),
            "fee_asc" => entries.sort_by(|a, b| a.fee.cmp(&b.fee)),
            "time_desc" => entries.sort_by(|a, b| b.time.cmp(&a.time)),
            "time_asc" => entries.sort_by(|a, b| a.time.cmp(&b.time)),
            "size_desc" => entries.sort_by(|a, b| b.size.cmp(&a.size)),
            "size_asc" => entries.sort_by(|a, b| a.size.cmp(&b.size)),
            _ => entries.sort_by(|a, b| b.fee_rate.cmp(&a.fee_rate)), // Default to fee_rate desc
        }
        
        // Apply pagination
        let start = offset.min(entries.len());
        let end = (offset + limit).min(entries.len());
        
        Ok(entries[start..end].to_vec())
    }
    
    /// Get a specific transaction by hex string ID
    pub fn get_transaction(&self, txid: &str) -> Result<Option<MempoolTransaction>, MempoolError> {
        // Parse hex string to bytes
        let tx_hash_bytes = hex::decode(txid).map_err(|_| MempoolError::SerializationError)?;
        if tx_hash_bytes.len() != 32 {
            return Err(MempoolError::SerializationError);
        }
        
        let mut tx_hash = [0u8; 32];
        tx_hash.copy_from_slice(&tx_hash_bytes);
        
        if let Some(entry) = self.transactions.get(&tx_hash) {
            Ok(Some(MempoolTransaction {
                txid: txid.to_string(),
                size: entry.size,
                fee: entry.fee_rate * entry.size as u64,
                fee_rate: entry.fee_rate,
                time: entry.timestamp.duration_since(SystemTime::UNIX_EPOCH).unwrap_or_default().as_secs(),
            }))
        } else {
            Ok(None)
        }
    }
    
    /// Submit a transaction from raw bytes
    pub fn submit_transaction(&self, raw_tx: &[u8], allow_high_fees: bool) -> Result<String, MempoolError> {
        // Deserialize the transaction
        let transaction: Transaction = bincode::deserialize(raw_tx)
            .map_err(|_| MempoolError::SerializationError)?;
        
        let tx_hash = transaction.hash();
        
        // Calculate a basic fee rate (this is simplified)
        let tx_size = raw_tx.len();
        let fee_rate = self.config.min_fee_rate; // Simplified fee calculation
        
        // Check for high fees if not allowed
        if !allow_high_fees && fee_rate > self.config.min_fee_rate * 10 {
            return Err(MempoolError::FeeTooLow);
        }
        
        // Add to mempool
        self.add_transaction(transaction, fee_rate)?;
        
        Ok(hex::encode(tx_hash))
    }
    
    /// Validate a transaction without adding it to mempool
    pub fn validate_transaction(&self, raw_tx: &[u8]) -> Result<TransactionValidationResult, MempoolError> {
        // Deserialize the transaction
        let transaction: Transaction = bincode::deserialize(raw_tx)
            .map_err(|_| MempoolError::SerializationError)?;
        
        let tx_hash = transaction.hash();
        
        // Check if already in mempool
        if self.transactions.contains_key(&tx_hash) {
            return Ok(TransactionValidationResult {
                valid: false,
                error: Some("Transaction already in mempool".to_string()),
                fee_rate: None,
                size: Some(raw_tx.len()),
            });
        }
        
        // Check for double spend
        if self.check_double_spend(&transaction) {
            return Ok(TransactionValidationResult {
                valid: false,
                error: Some("Double spend detected".to_string()),
                fee_rate: None,
                size: Some(raw_tx.len()),
            });
        }
        
        // Basic validation passed
        let fee_rate = self.config.min_fee_rate; // Simplified
        
        Ok(TransactionValidationResult {
            valid: true,
            error: None,
            fee_rate: Some(fee_rate),
            size: Some(raw_tx.len()),
        })
    }
    
    /// Estimate fee for target confirmation
    pub fn estimate_fee(&self, target_conf: u32) -> Result<TransactionFees, MempoolError> {
        // Simple fee estimation based on current mempool state
        let transaction_count = self.transactions.len();
        
        let (low_priority, normal_priority, high_priority) = if transaction_count == 0 {
            // Empty mempool, use minimum rates
            (self.config.min_fee_rate, self.config.min_fee_rate * 2, self.config.min_fee_rate * 5)
        } else {
            // Calculate percentiles from current mempool
            let mut fee_rates: Vec<u64> = self.transactions.iter().map(|entry| entry.fee_rate).collect();
            fee_rates.sort();
            
            let len = fee_rates.len();
            let low = fee_rates[len / 4].max(self.config.min_fee_rate);
            let normal = fee_rates[len / 2].max(self.config.min_fee_rate * 2);
            let high = fee_rates[len * 3 / 4].max(self.config.min_fee_rate * 5);
            
            (low, normal, high)
        };
        
        // Adjust based on target confirmation time
        let multiplier = match target_conf {
            1 => 2.0,      // Next block - high priority
            2..=3 => 1.5,  // 2-3 blocks - normal priority  
            4..=6 => 1.0,  // 4-6 blocks - normal
            _ => 0.8,      // 7+ blocks - low priority
        };
        
        Ok(TransactionFees {
            low_priority: (low_priority as f64 * multiplier * 0.8) as u64,
            normal_priority: (normal_priority as f64 * multiplier) as u64,
            high_priority: (high_priority as f64 * multiplier * 1.2) as u64,
            target_blocks: target_conf,
        })
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
    #[error("Replace-By-Fee is disabled")]
    RbfDisabled,
    #[error("No conflicting transactions found for RBF")]
    NoConflictingTransactions,
    #[error("Fee increase insufficient for RBF, minimum required: {0}")]
    InsufficientFeeIncrease(u64),
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
        
        // Compare transaction hashes instead of transactions directly
        let tx_from_pool = pool.get_transaction(&tx_hash).unwrap();
        assert_eq!(tx_from_pool.hash(), tx.hash());
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
        
        // Store hashes for comparison
        let tx1_hash = tx1.hash();
        let tx2_hash = tx2.hash();
        
        pool.add_transaction(tx1.clone(), 1).unwrap();
        pool.add_transaction(tx2.clone(), 2).unwrap();
        
        let sorted = pool.get_sorted_transactions();
        
        // Compare transaction hashes instead of transactions directly
        assert_eq!(sorted[0].hash(), tx2_hash); // Higher fee rate should be first
        assert_eq!(sorted[1].hash(), tx1_hash);
    }

    #[test]
    fn test_rbf_transaction() {
        let config = MempoolConfig {
            enable_rbf: true,
            min_rbf_fee_increase: 10.0,
            ..MempoolConfig::default()
        };
        let pool = TransactionPool::new(config);
        
        // Add first transaction with low fee
        let tx1 = create_test_transaction([1u8; 32], 50_000_000);
        let tx1_hash = tx1.hash();
        pool.add_transaction(tx1.clone(), 1).unwrap();
        
        // Create replacement transaction with same inputs but higher fee
        let tx2 = create_test_transaction([1u8; 32], 50_000_000);
        let tx2_hash = tx2.hash();
        
        // Replace should succeed with higher fee rate
        assert!(pool.replace_transaction(tx2.clone(), 2).is_ok());
        
        // Original transaction should be gone
        assert!(pool.get_transaction(&tx1_hash).is_none());
        
        // New transaction should be present
        assert!(pool.get_transaction(&tx2_hash).is_some());
    }
    
    #[test]
    fn test_rbf_disabled() {
        let config = MempoolConfig {
            enable_rbf: false,
            ..MempoolConfig::default()
        };
        let pool = TransactionPool::new(config);
        
        // Add first transaction
        let tx1 = create_test_transaction([1u8; 32], 50_000_000);
        pool.add_transaction(tx1.clone(), 1).unwrap();
        
        // Create replacement transaction
        let tx2 = create_test_transaction([1u8; 32], 50_000_000);
        
        // RBF should fail when disabled
        assert!(matches!(pool.replace_transaction(tx2, 2), Err(MempoolError::RbfDisabled)));
    }
    
    #[test]
    fn test_rbf_insufficient_fee() {
        let config = MempoolConfig {
            enable_rbf: true,
            min_rbf_fee_increase: 50.0, // 50% increase required
            ..MempoolConfig::default()
        };
        let pool = TransactionPool::new(config);
        
        // Add first transaction
        let tx1 = create_test_transaction([1u8; 32], 50_000_000);
        pool.add_transaction(tx1.clone(), 10).unwrap();
        
        // Create replacement transaction with not enough fee increase
        let tx2 = create_test_transaction([1u8; 32], 50_000_000);
        
        // 10% increase is not enough
        assert!(matches!(
            pool.replace_transaction(tx2.clone(), 11), 
            Err(MempoolError::InsufficientFeeIncrease(_))
        ));
        
        // 60% increase should work
        assert!(pool.replace_transaction(tx2, 16).is_ok());
    }
}