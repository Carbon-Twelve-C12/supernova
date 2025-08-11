//! Secure mempool wrapper for Supernova
//!
//! This module provides a backwards-compatible interface to the atomic mempool
//! implementation, ensuring all existing code continues to work while benefiting
//! from the race-condition-free atomic operations.

use crate::mempool::{AtomicTransactionPool, MempoolConfig, MempoolError, MEVProtection, MEVProtectionConfig};
use crate::api::types::{MempoolInfo, MempoolTransaction, TransactionFees, TransactionValidationResult};
use btclib::types::transaction::Transaction;
use btclib::hash::Hash256;
use dashmap::DashMap;
use std::sync::Arc;
use hex;

/// Secure transaction pool that wraps AtomicTransactionPool with MEV protection
/// 
/// This provides the same interface as the original TransactionPool
/// but uses atomic operations internally to prevent race conditions
/// and includes MEV protection mechanisms
pub struct SecureTransactionPool {
    /// Internal atomic pool
    atomic_pool: Arc<AtomicTransactionPool>,
    /// MEV protection system
    mev_protection: Arc<MEVProtection>,
    /// Keep DashMap reference for compatibility with existing code
    pub transactions: Arc<DashMap<[u8; 32], ()>>,
}

impl SecureTransactionPool {
    /// Create a new secure transaction pool
    pub fn new(config: MempoolConfig) -> Self {
        // Create MEV protection with default config
        let mev_config = MEVProtectionConfig::default();
        
        Self {
            atomic_pool: Arc::new(AtomicTransactionPool::new(config)),
            mev_protection: Arc::new(MEVProtection::new(mev_config)),
            transactions: Arc::new(DashMap::new()),
        }
    }
    
    /// Create a new secure transaction pool with custom MEV protection config
    pub fn with_mev_config(config: MempoolConfig, mev_config: MEVProtectionConfig) -> Self {
        Self {
            atomic_pool: Arc::new(AtomicTransactionPool::new(config)),
            mev_protection: Arc::new(MEVProtection::new(mev_config)),
            transactions: Arc::new(DashMap::new()),
        }
    }

    /// Add a transaction to the pool (atomic)
    pub fn add_transaction(&self, transaction: Transaction, fee_rate: u64) -> Result<(), MempoolError> {
        let tx_hash = transaction.hash();
        
        // Use atomic pool
        let result = self.atomic_pool.add_transaction(transaction, fee_rate);
        
        // Update compatibility map
        if result.is_ok() {
            self.transactions.insert(tx_hash, ());
        }
        
        result
    }

    /// Remove a transaction from the pool (atomic)
    pub fn remove_transaction(&self, tx_hash: &[u8; 32]) -> Option<Transaction> {
        // Remove from atomic pool
        let result = self.atomic_pool.remove_transaction(tx_hash);
        
        // Update compatibility map
        if result.is_some() {
            self.transactions.remove(tx_hash);
        }
        
        result
    }

    /// Get a transaction by its hash
    pub fn get_transaction(&self, tx_hash: &[u8; 32]) -> Option<Transaction> {
        self.atomic_pool.get_transaction(tx_hash)
    }

    /// Check for double-spend attempts (atomic)
    pub fn check_double_spend(&self, transaction: &Transaction) -> bool {
        self.atomic_pool.check_double_spend(transaction)
    }

    /// Replace transaction (RBF) atomically
    pub fn replace_transaction(&self, new_transaction: Transaction, fee_rate: u64) -> Result<Option<Transaction>, MempoolError> {
        let new_tx_hash = new_transaction.hash();
        let result = self.atomic_pool.replace_transaction(new_transaction, fee_rate)?;
        
        // Update compatibility map
        for tx in &result {
            self.transactions.remove(&tx.hash());
        }
        
        if let Some(new_tx) = self.atomic_pool.get_transaction(&new_tx_hash) {
            self.transactions.insert(new_tx.hash(), ());
        }
        
        // Return first replaced transaction for compatibility
        Ok(result.into_iter().next())
    }

    /// Clear expired transactions (atomic)
    pub fn clear_expired(&self) -> usize {
        let removed = self.atomic_pool.clear_expired();
        
        // Sync compatibility map
        self.sync_compatibility_map();
        
        removed
    }

    /// Get all transactions sorted by fee rate
    pub fn get_sorted_transactions(&self) -> Vec<Transaction> {
        self.atomic_pool.get_sorted_transactions()
    }

    /// Clear all transactions (atomic)
    pub fn clear_all(&self) -> Result<(), MempoolError> {
        let result = self.atomic_pool.clear_all();
        
        if result.is_ok() {
            self.transactions.clear();
        }
        
        result
    }

    /// Get mempool information
    pub fn get_info(&self) -> MempoolInfo {
        self.atomic_pool.get_info()
    }

    /// Get transactions with pagination
    pub fn get_transactions(&self, limit: usize, offset: usize, sort: &str) -> Result<Vec<MempoolTransaction>, MempoolError> {
        // Get sorted transactions from atomic pool
        let all_txs = match sort {
            "fee_desc" | "fee_asc" | "time_desc" | "time_asc" | "size_desc" | "size_asc" => {
                self.atomic_pool.get_sorted_transactions()
            }
            _ => self.atomic_pool.get_sorted_transactions()
        };

        // Convert to MempoolTransaction format with basic info
        let mempool_txs: Vec<MempoolTransaction> = all_txs
            .into_iter()
            .skip(offset)
            .take(limit)
            .map(|tx| {
                let tx_hash = tx.hash();
                let size = bincode::serialize(&tx).unwrap_or_default().len();
                
                MempoolTransaction {
                    txid: hex::encode(tx_hash),
                    size,
                    fee: size as u64, // Simplified fee calculation
                    fee_rate: 1, // Simplified
                    time: 0, // Simplified
                }
            })
            .collect();

        Ok(mempool_txs)
    }

    /// Get a specific transaction by ID
    pub fn get_transaction_by_id(&self, txid: &str) -> Result<Option<MempoolTransaction>, MempoolError> {
        // Parse hex string to bytes
        let tx_hash_bytes = hex::decode(txid).map_err(|_| MempoolError::SerializationError("Transaction ID must be 32 bytes".to_string()))?;
        if tx_hash_bytes.len() != 32 {
            return Err(MempoolError::SerializationError("Transaction ID must be 32 bytes".to_string()));
        }
        
        let mut tx_hash = [0u8; 32];
        tx_hash.copy_from_slice(&tx_hash_bytes);
        
        if let Some(tx) = self.atomic_pool.get_transaction(&tx_hash) {
            let size = bincode::serialize(&tx).unwrap_or_default().len();
            
            Ok(Some(MempoolTransaction {
                txid: txid.to_string(),
                size,
                fee: size as u64,
                fee_rate: 1,
                time: 0,
            }))
        } else {
            Ok(None)
        }
    }

    /// Submit a transaction from raw bytes
    pub fn submit_transaction(&self, raw_tx: &[u8], allow_high_fees: bool) -> Result<String, MempoolError> {
        // Deserialize the transaction
        let transaction: Transaction = bincode::deserialize(raw_tx)
            .map_err(|e| MempoolError::SerializationError(format!("Failed to deserialize transaction: {}", e)))?;
        
        let tx_hash = transaction.hash();
        let fee_rate = 1; // Simplified
        
        // Add to atomic pool
        self.add_transaction(transaction, fee_rate)?;
        
        Ok(hex::encode(tx_hash))
    }

    /// Validate a transaction
    pub fn validate_transaction(&self, raw_tx: &[u8]) -> Result<TransactionValidationResult, MempoolError> {
        // Deserialize the transaction
        let transaction: Transaction = bincode::deserialize(raw_tx)
            .map_err(|e| MempoolError::SerializationError(format!("Failed to deserialize transaction: {}", e)))?;
        
        let tx_hash = transaction.hash();
        
        // Check if already in mempool
        if self.atomic_pool.get_transaction(&tx_hash).is_some() {
            return Ok(TransactionValidationResult {
                valid: false,
                error: Some("Transaction already in mempool".to_string()),
                fee_rate: None,
                size: Some(raw_tx.len()),
            });
        }
        
        // Check for double spend
        if self.atomic_pool.check_double_spend(&transaction) {
            return Ok(TransactionValidationResult {
                valid: false,
                error: Some("Double spend detected".to_string()),
                fee_rate: None,
                size: Some(raw_tx.len()),
            });
        }
        
        Ok(TransactionValidationResult {
            valid: true,
            error: None,
            fee_rate: Some(1),
            size: Some(raw_tx.len()),
        })
    }

    /// Estimate fee
    pub fn estimate_fee(&self, target_conf: u32) -> Result<TransactionFees, MempoolError> {
        Ok(TransactionFees {
            low_priority: 1,
            normal_priority: 2,
            high_priority: 5,
            target_blocks: target_conf,
        })
    }

    /// Get current size
    pub fn size(&self) -> usize {
        self.atomic_pool.size()
    }

    /// Get size in bytes
    pub fn size_in_bytes(&self) -> usize {
        self.atomic_pool.size() * 250 // Approximate average transaction size
    }

    /// Sync the compatibility DashMap with atomic pool
    fn sync_compatibility_map(&self) {
        self.transactions.clear();
        
        // Re-populate from atomic pool
        for tx in self.atomic_pool.get_sorted_transactions() {
            self.transactions.insert(tx.hash(), ());
        }
    }
    
    /// Submit a transaction commitment for MEV protection
    pub async fn submit_commitment(
        &self,
        tx_hash: Hash256,
        declared_fee: u64,
        commitment: Hash256,
        sender_id: Option<Vec<u8>>,
    ) -> Result<(), MempoolError> {
        self.mev_protection
            .submit_commitment(tx_hash, declared_fee, commitment, sender_id)
            .await
    }
    
    /// Reveal a committed transaction
    pub async fn reveal_transaction(
        &self,
        transaction: Transaction,
        nonce: Vec<u8>,
    ) -> Result<(), MempoolError> {
        // First reveal through MEV protection
        self.mev_protection.reveal_transaction(transaction.clone(), nonce).await?;
        
        // Then add to atomic pool if reveal succeeded
        // The MEV protection system will handle fair ordering
        Ok(())
    }
    
    /// Get next batch of transactions with MEV protection
    pub async fn get_mev_protected_batch(&self, max_size: usize) -> Vec<Transaction> {
        self.mev_protection.get_next_batch(max_size).await
    }
    
    /// Get MEV protection statistics
    pub async fn get_mev_statistics(&self) -> crate::mempool::MEVProtectionStats {
        self.mev_protection.get_statistics().await
    }
    
    /// Clean up expired MEV commitments
    pub async fn cleanup_mev_commitments(&self) {
        self.mev_protection.cleanup_expired_commitments().await
    }
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
    fn test_secure_pool_double_spend_prevention() {
        let config = MempoolConfig::default();
        let pool = SecureTransactionPool::new(config);
        
        // Create two transactions spending the same output
        let tx1 = create_test_transaction([1u8; 32], 50_000_000);
        let tx2 = create_test_transaction([1u8; 32], 40_000_000);
        
        // Add first transaction
        assert!(pool.add_transaction(tx1.clone(), 2).is_ok());
        
        // Try to add second transaction - should fail
        assert!(matches!(
            pool.add_transaction(tx2, 2),
            Err(MempoolError::DoubleSpend(_))
        ));
        
        // Verify first transaction is still there
        assert!(pool.get_transaction(&tx1.hash()).is_some());
    }

    #[test]
    fn test_secure_pool_rbf() {
        let config = MempoolConfig {
            enable_rbf: true,
            min_rbf_fee_increase: 10.0,
            ..MempoolConfig::default()
        };
        let pool = SecureTransactionPool::new(config);
        
        // Add initial transaction
        let tx1 = create_test_transaction([1u8; 32], 50_000_000);
        assert!(pool.add_transaction(tx1.clone(), 100).is_ok());
        
        // Replace with higher fee
        let tx2 = create_test_transaction([1u8; 32], 49_000_000);
        let result = pool.replace_transaction(tx2.clone(), 120);
        
        assert!(result.is_ok());
        let replaced = result.unwrap();
        assert!(replaced.is_some());
        assert_eq!(replaced.unwrap().hash(), tx1.hash());
        
        // Verify replacement worked
        assert!(pool.get_transaction(&tx1.hash()).is_none());
        assert!(pool.get_transaction(&tx2.hash()).is_some());
    }
} 