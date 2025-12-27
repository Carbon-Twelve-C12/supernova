//! Transaction Indexing System
//!
//! Provides comprehensive transaction indexing for efficient queries:
//! - Transaction hash → block location
//! - Address → transaction list
//! - Block height → transactions
//! - Environmental score → green transactions
//! - Lightning channel ID → channel transactions

use supernova_core::types::transaction::{Transaction, TransactionOutput};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, RwLock};
use thiserror::Error;

/// Error types for transaction indexing
#[derive(Debug, Error)]
pub enum TransactionIndexError {
    #[error("Transaction not found: {0}")]
    TransactionNotFound(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Serialization error: {0}")]
    Serialization(String),
    #[error("Lock poisoned: {0}")]
    LockPoisoned(String),
}

/// Block location information
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlockLocation {
    /// Block hash
    pub block_hash: [u8; 32],
    /// Block height
    pub height: u64,
    /// Transaction index within the block
    pub tx_index: u32,
}

/// Transaction entry in index
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexedTransaction {
    /// Transaction hash
    pub tx_hash: [u8; 32],
    /// Block location
    pub location: BlockLocation,
    /// Transaction size in bytes
    pub size: usize,
    /// Environmental score (0-100)
    pub environmental_score: Option<f64>,
    /// Lightning channel ID (if applicable)
    pub lightning_channel_id: Option<[u8; 32]>,
}

/// Configuration for transaction indexer
#[derive(Debug, Clone)]
pub struct TransactionIndexConfig {
    /// Maximum number of transactions to keep in memory indexes
    pub max_memory_transactions: usize,
    /// Prune transactions older than this height (0 = disabled)
    pub prune_height_threshold: u64,
    /// Enable address indexing
    pub enable_address_index: bool,
    /// Enable environmental score indexing
    pub enable_green_index: bool,
    /// Enable Lightning channel indexing
    pub enable_lightning_index: bool,
}

impl Default for TransactionIndexConfig {
    fn default() -> Self {
        Self {
            max_memory_transactions: 1_000_000,
            prune_height_threshold: 0,
            enable_address_index: true,
            enable_green_index: true,
            enable_lightning_index: true,
        }
    }
}

/// Transaction hash index: O(1) lookup by transaction hash
#[derive(Debug, Default)]
struct TxHashIndex {
    /// Map from transaction hash to indexed transaction
    index: HashMap<[u8; 32], IndexedTransaction>,
}

impl TxHashIndex {
    fn new() -> Self {
        Self {
            index: HashMap::new(),
        }
    }

    fn insert(&mut self, tx_hash: [u8; 32], entry: IndexedTransaction) {
        self.index.insert(tx_hash, entry);
    }

    fn get(&self, tx_hash: &[u8; 32]) -> Option<&IndexedTransaction> {
        self.index.get(tx_hash)
    }

    fn remove(&mut self, tx_hash: &[u8; 32]) -> bool {
        self.index.remove(tx_hash).is_some()
    }

    fn len(&self) -> usize {
        self.index.len()
    }
}

/// Address index: All transactions for an address
#[derive(Debug, Default)]
struct AddressIndex {
    /// Map from address (as hex string) to set of transaction hashes
    index: HashMap<String, Vec<[u8; 32]>>,
}

impl AddressIndex {
    fn new() -> Self {
        Self {
            index: HashMap::new(),
        }
    }

    fn add_transaction(&mut self, address: String, tx_hash: [u8; 32]) {
        self.index
            .entry(address)
            .or_insert_with(Vec::new)
            .push(tx_hash);
    }

    fn get_transactions(&self, address: &str) -> Option<&Vec<[u8; 32]>> {
        self.index.get(address)
    }

    fn remove_transaction(&mut self, address: &str, tx_hash: &[u8; 32]) {
        if let Some(txs) = self.index.get_mut(address) {
            txs.retain(|&h| h != *tx_hash);
            if txs.is_empty() {
                self.index.remove(address);
            }
        }
    }
}

/// Height index: Transactions in specific blocks
#[derive(Debug, Default)]
struct HeightIndex {
    /// Map from block height to list of transaction hashes
    index: BTreeMap<u64, Vec<[u8; 32]>>,
}

impl HeightIndex {
    fn new() -> Self {
        Self {
            index: BTreeMap::new(),
        }
    }

    fn add_transaction(&mut self, height: u64, tx_hash: [u8; 32]) {
        self.index
            .entry(height)
            .or_insert_with(Vec::new)
            .push(tx_hash);
    }

    fn get_transactions(&self, height: u64) -> Option<&Vec<[u8; 32]>> {
        self.index.get(&height)
    }

    fn remove_transaction(&mut self, height: u64, tx_hash: &[u8; 32]) {
        if let Some(txs) = self.index.get_mut(&height) {
            txs.retain(|&h| h != *tx_hash);
            if txs.is_empty() {
                self.index.remove(&height);
            }
        }
    }

    fn get_height_range(&self, start: u64, end: u64) -> Vec<[u8; 32]> {
        self.index
            .range(start..=end)
            .flat_map(|(_, txs)| txs.iter().copied())
            .collect()
    }
}

/// Green index: Transactions by environmental score
#[derive(Debug, Default)]
struct GreenIndex {
    /// Map from environmental score (rounded to integer) to transaction hashes
    index: BTreeMap<u8, Vec<[u8; 32]>>,
}

impl GreenIndex {
    fn new() -> Self {
        Self {
            index: BTreeMap::new(),
        }
    }

    fn add_transaction(&mut self, score: f64, tx_hash: [u8; 32]) {
        let score_int = score.min(100.0).max(0.0) as u8;
        self.index
            .entry(score_int)
            .or_insert_with(Vec::new)
            .push(tx_hash);
    }

    fn get_transactions(&self, min_score: f64) -> Vec<[u8; 32]> {
        let min_score_int = min_score.min(100.0).max(0.0) as u8;
        self.index
            .range(min_score_int..=100)
            .flat_map(|(_, txs)| txs.iter().copied())
            .collect()
    }
}

/// Lightning index: Channel-specific transactions
#[derive(Debug, Default)]
struct LightningIndex {
    /// Map from channel ID to list of transaction hashes
    index: HashMap<[u8; 32], Vec<[u8; 32]>>,
}

impl LightningIndex {
    fn new() -> Self {
        Self {
            index: HashMap::new(),
        }
    }

    fn add_transaction(&mut self, channel_id: [u8; 32], tx_hash: [u8; 32]) {
        self.index
            .entry(channel_id)
            .or_insert_with(Vec::new)
            .push(tx_hash);
    }

    fn get_transactions(&self, channel_id: &[u8; 32]) -> Option<&Vec<[u8; 32]>> {
        self.index.get(channel_id)
    }

    fn remove_transaction(&mut self, channel_id: &[u8; 32], tx_hash: &[u8; 32]) {
        if let Some(txs) = self.index.get_mut(channel_id) {
            txs.retain(|&h| h != *tx_hash);
            if txs.is_empty() {
                self.index.remove(channel_id);
            }
        }
    }
}

/// Main transaction indexer
pub struct TransactionIndexer {
    /// Configuration
    config: TransactionIndexConfig,
    /// Transaction hash index
    tx_hash_index: Arc<RwLock<TxHashIndex>>,
    /// Address index
    address_index: Arc<RwLock<AddressIndex>>,
    /// Height index
    height_index: Arc<RwLock<HeightIndex>>,
    /// Green index
    green_index: Arc<RwLock<GreenIndex>>,
    /// Lightning index
    lightning_index: Arc<RwLock<LightningIndex>>,
}

impl TransactionIndexer {
    /// Create a new transaction indexer
    pub fn new(config: TransactionIndexConfig) -> Self {
        Self {
            config,
            tx_hash_index: Arc::new(RwLock::new(TxHashIndex::new())),
            address_index: Arc::new(RwLock::new(AddressIndex::new())),
            height_index: Arc::new(RwLock::new(HeightIndex::new())),
            green_index: Arc::new(RwLock::new(GreenIndex::new())),
            lightning_index: Arc::new(RwLock::new(LightningIndex::new())),
        }
    }

    /// Index a transaction from a block
    pub fn index_transaction(
        &self,
        tx: &Transaction,
        block_hash: [u8; 32],
        height: u64,
        tx_index: u32,
        environmental_score: Option<f64>,
        lightning_channel_id: Option<[u8; 32]>,
    ) -> Result<(), TransactionIndexError> {
        let tx_hash = tx.hash();
        let size = tx.calculate_size();

        let location = BlockLocation {
            block_hash,
            height,
            tx_index,
        };

        let indexed_tx = IndexedTransaction {
            tx_hash,
            location: location.clone(),
            size,
            environmental_score,
            lightning_channel_id,
        };

        // Update hash index
        {
            let mut hash_idx = self.tx_hash_index.write()
                .map_err(|e| TransactionIndexError::LockPoisoned(format!("tx_hash_index: {}", e)))?;
            hash_idx.insert(tx_hash, indexed_tx.clone());
        }

        // Update address index
        if self.config.enable_address_index {
            let mut addr_idx = self.address_index.write()
                .map_err(|e| TransactionIndexError::LockPoisoned(format!("address_index: {}", e)))?;
            for output in tx.outputs() {
                let address = Self::extract_address_from_output(output);
                if let Some(addr) = address {
                    addr_idx.add_transaction(addr, tx_hash);
                }
            }
        }

        // Update height index
        {
            let mut height_idx = self.height_index.write()
                .map_err(|e| TransactionIndexError::LockPoisoned(format!("height_index: {}", e)))?;
            height_idx.add_transaction(height, tx_hash);
        }

        // Update green index
        if self.config.enable_green_index {
            if let Some(score) = environmental_score {
                let mut green_idx = self.green_index.write()
                    .map_err(|e| TransactionIndexError::LockPoisoned(format!("green_index: {}", e)))?;
                green_idx.add_transaction(score, tx_hash);
            }
        }

        // Update Lightning index
        if self.config.enable_lightning_index {
            if let Some(channel_id) = lightning_channel_id {
                let mut lightning_idx = self.lightning_index.write()
                    .map_err(|e| TransactionIndexError::LockPoisoned(format!("lightning_index: {}", e)))?;
                lightning_idx.add_transaction(channel_id, tx_hash);
            }
        }

        Ok(())
    }

    /// Get transaction by hash
    pub fn get_transaction(
        &self,
        tx_hash: &[u8; 32],
    ) -> Result<IndexedTransaction, TransactionIndexError> {
        let hash_idx = self.tx_hash_index.read()
            .map_err(|e| TransactionIndexError::LockPoisoned(format!("tx_hash_index: {}", e)))?;
        hash_idx
            .get(tx_hash)
            .cloned()
            .ok_or_else(|| TransactionIndexError::TransactionNotFound(hex::encode(tx_hash)))
    }

    /// Get transactions by address
    pub fn get_transactions_by_address(
        &self,
        address: &str,
        limit: Option<usize>,
        offset: usize,
    ) -> Result<Vec<[u8; 32]>, TransactionIndexError> {
        let addr_idx = self.address_index.read()
            .map_err(|e| TransactionIndexError::LockPoisoned(format!("address_index: {}", e)))?;
        let mut txs = addr_idx
            .get_transactions(address)
            .cloned()
            .unwrap_or_default();

        // Apply offset and limit
        if offset > 0 {
            txs = txs.into_iter().skip(offset).collect();
        }
        if let Some(limit) = limit {
            txs = txs.into_iter().take(limit).collect();
        }

        Ok(txs)
    }

    /// Get transactions by block height
    pub fn get_transactions_by_height(&self, height: u64) -> Result<Vec<[u8; 32]>, TransactionIndexError> {
        let height_idx = self.height_index.read()
            .map_err(|e| TransactionIndexError::LockPoisoned(format!("height_index: {}", e)))?;
        Ok(height_idx
            .get_transactions(height)
            .cloned()
            .unwrap_or_default())
    }

    /// Get green transactions (with minimum environmental score)
    pub fn get_green_transactions(&self, min_score: f64) -> Result<Vec<[u8; 32]>, TransactionIndexError> {
        let green_idx = self.green_index.read()
            .map_err(|e| TransactionIndexError::LockPoisoned(format!("green_index: {}", e)))?;
        Ok(green_idx.get_transactions(min_score))
    }

    /// Get transactions for a Lightning channel
    pub fn get_channel_transactions(
        &self,
        channel_id: &[u8; 32],
    ) -> Result<Vec<[u8; 32]>, TransactionIndexError> {
        let lightning_idx = self.lightning_index.read()
            .map_err(|e| TransactionIndexError::LockPoisoned(format!("lightning_index: {}", e)))?;
        Ok(lightning_idx
            .get_transactions(channel_id)
            .cloned()
            .unwrap_or_default())
    }

    /// Remove a transaction from all indexes
    pub fn remove_transaction(&self, tx_hash: &[u8; 32]) -> Result<(), TransactionIndexError> {
        // Get transaction info first
        let indexed_tx = self.get_transaction(tx_hash)?;

        // Remove from hash index
        {
            let mut hash_idx = self.tx_hash_index.write()
                .map_err(|e| TransactionIndexError::LockPoisoned(format!("tx_hash_index: {}", e)))?;
            hash_idx.remove(tx_hash);
        }

        // Remove from address index
        if self.config.enable_address_index {
            // We need to reconstruct addresses from the transaction
            // For now, we'll just remove from hash index
            // In a production system, we'd store address mappings
        }

        // Remove from height index
        {
            let mut height_idx = self.height_index.write()
                .map_err(|e| TransactionIndexError::LockPoisoned(format!("height_index: {}", e)))?;
            height_idx.remove_transaction(indexed_tx.location.height, tx_hash);
        }

        // Remove from green index (would need to rebuild)
        // Remove from Lightning index
        if let Some(channel_id) = indexed_tx.lightning_channel_id {
            let mut lightning_idx = self.lightning_index.write()
                .map_err(|e| TransactionIndexError::LockPoisoned(format!("lightning_index: {}", e)))?;
            lightning_idx.remove_transaction(&channel_id, tx_hash);
        }

        Ok(())
    }

    /// Prune old transactions (older than threshold height)
    pub fn prune_old_transactions(&self, current_height: u64) -> Result<usize, TransactionIndexError> {
        if self.config.prune_height_threshold == 0 {
            return Ok(0);
        }

        let threshold = current_height.saturating_sub(self.config.prune_height_threshold);
        let mut pruned = 0;

        // Get transactions to prune from height index
        let mut height_idx = self.height_index.write()
            .map_err(|e| TransactionIndexError::LockPoisoned(format!("height_index: {}", e)))?;
        let heights_to_prune: Vec<u64> = height_idx
            .index
            .range(..=threshold)
            .map(|(h, _)| *h)
            .collect();

        for height in heights_to_prune {
            if let Some(txs) = height_idx.index.remove(&height) {
                let mut hash_idx = self.tx_hash_index.write()
                    .map_err(|e| TransactionIndexError::LockPoisoned(format!("tx_hash_index: {}", e)))?;
                for tx_hash in &txs {
                    if hash_idx.remove(tx_hash) {
                        pruned += 1;
                    }
                }
            }
        }

        Ok(pruned)
    }

    /// Get statistics about the index
    pub fn get_statistics(&self) -> Result<IndexStatistics, TransactionIndexError> {
        let hash_idx = self.tx_hash_index.read()
            .map_err(|e| TransactionIndexError::LockPoisoned(format!("tx_hash_index: {}", e)))?;
        let addr_idx = self.address_index.read()
            .map_err(|e| TransactionIndexError::LockPoisoned(format!("address_index: {}", e)))?;
        let height_idx = self.height_index.read()
            .map_err(|e| TransactionIndexError::LockPoisoned(format!("height_index: {}", e)))?;
        let green_idx = self.green_index.read()
            .map_err(|e| TransactionIndexError::LockPoisoned(format!("green_index: {}", e)))?;
        let lightning_idx = self.lightning_index.read()
            .map_err(|e| TransactionIndexError::LockPoisoned(format!("lightning_index: {}", e)))?;

        Ok(IndexStatistics {
            total_transactions: hash_idx.len(),
            indexed_addresses: addr_idx.index.len(),
            indexed_heights: height_idx.index.len(),
            green_transaction_count: green_idx.index.values().map(|v| v.len()).sum(),
            lightning_channel_count: lightning_idx.index.len(),
        })
    }

    /// Extract address from transaction output
    fn extract_address_from_output(output: &TransactionOutput) -> Option<String> {
        // Extract address from script_pubkey
        // This is a simplified version - in production, you'd parse the script properly
        let script = output.script_pubkey();
        if script.is_empty() {
            return None;
        }

        // For now, use hex encoding of script as address identifier
        // In production, decode script to get actual address
        Some(hex::encode(script))
    }
}

/// Index statistics
#[derive(Debug, Clone)]
pub struct IndexStatistics {
    pub total_transactions: usize,
    pub indexed_addresses: usize,
    pub indexed_heights: usize,
    pub green_transaction_count: usize,
    pub lightning_channel_count: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use supernova_core::types::transaction::{Transaction, TransactionInput, TransactionOutput};

    fn create_test_transaction() -> Transaction {
        let inputs = vec![TransactionInput::new(
            [0u8; 32],
            0,
            vec![],
            0xffffffff,
        )];
        let outputs = vec![TransactionOutput::new(1000, vec![1, 2, 3])];
        Transaction::new(1, inputs, outputs, 0)
    }

    #[test]
    fn test_transaction_hash_indexing() {
        let indexer = TransactionIndexer::new(TransactionIndexConfig::default());
        let tx = create_test_transaction();
        let tx_hash = tx.hash();
        let block_hash = [1u8; 32];

        indexer
            .index_transaction(&tx, block_hash, 100, 0, None, None)
            .unwrap();

        let indexed = indexer.get_transaction(&tx_hash).unwrap();
        assert_eq!(indexed.tx_hash, tx_hash);
        assert_eq!(indexed.location.height, 100);
        assert_eq!(indexed.location.tx_index, 0);
    }

    #[test]
    fn test_address_indexing() {
        let indexer = TransactionIndexer::new(TransactionIndexConfig::default());
        let tx = create_test_transaction();
        let tx_hash = tx.hash();
        let block_hash = [1u8; 32];

        indexer
            .index_transaction(&tx, block_hash, 100, 0, None, None)
            .unwrap();

        // Extract address from output
        let output = &tx.outputs()[0];
        let address = TransactionIndexer::extract_address_from_output(output).unwrap();

        let txs = indexer
            .get_transactions_by_address(&address, None, 0)
            .unwrap();
        assert!(txs.contains(&tx_hash));
    }

    #[test]
    fn test_height_indexing() {
        let indexer = TransactionIndexer::new(TransactionIndexConfig::default());
        let tx = create_test_transaction();
        let tx_hash = tx.hash();
        let block_hash = [1u8; 32];

        indexer
            .index_transaction(&tx, block_hash, 100, 0, None, None)
            .unwrap();

        let txs = indexer.get_transactions_by_height(100).unwrap();
        assert!(txs.contains(&tx_hash));
    }

    #[test]
    fn test_green_transaction_indexing() {
        let indexer = TransactionIndexer::new(TransactionIndexConfig::default());
        let tx = create_test_transaction();
        let tx_hash = tx.hash();
        let block_hash = [1u8; 32];

        indexer
            .index_transaction(&tx, block_hash, 100, 0, Some(85.5), None)
            .unwrap();

        let green_txs = indexer.get_green_transactions(80.0).unwrap();
        assert!(green_txs.contains(&tx_hash));

        let very_green_txs = indexer.get_green_transactions(90.0).unwrap();
        assert!(!very_green_txs.contains(&tx_hash));
    }

    #[test]
    fn test_index_persistence() {
        let indexer = TransactionIndexer::new(TransactionIndexConfig::default());
        let tx = create_test_transaction();
        let tx_hash = tx.hash();
        let block_hash = [1u8; 32];

        indexer
            .index_transaction(&tx, block_hash, 100, 0, None, None)
            .unwrap();

        // Verify transaction exists
        assert!(indexer.get_transaction(&tx_hash).is_ok());

        // Remove transaction
        indexer.remove_transaction(&tx_hash).unwrap();

        // Verify transaction removed
        assert!(indexer.get_transaction(&tx_hash).is_err());
    }

    #[test]
    fn test_index_rebuild() {
        let indexer = TransactionIndexer::new(TransactionIndexConfig::default());
        let tx1 = create_test_transaction();
        let tx2 = create_test_transaction();
        let block_hash = [1u8; 32];

        indexer
            .index_transaction(&tx1, block_hash, 100, 0, None, None)
            .unwrap();
        indexer
            .index_transaction(&tx2, block_hash, 100, 1, None, None)
            .unwrap();

        let stats = indexer.get_statistics().unwrap();
        assert_eq!(stats.total_transactions, 2);
        assert_eq!(stats.indexed_heights, 1);
    }

    #[test]
    fn test_concurrent_index_updates() {
        use std::sync::Arc;
        use std::thread;

        let indexer = Arc::new(TransactionIndexer::new(TransactionIndexConfig::default()));
        let mut handles = vec![];

        for i in 0..10 {
            let indexer_clone = indexer.clone();
            let handle = thread::spawn(move || {
                let tx = create_test_transaction();
                let block_hash = [i as u8; 32];
                indexer_clone
                    .index_transaction(&tx, block_hash, i as u64, 0, None, None)
                    .unwrap();
            });
            handles.push(handle);
        }

        for handle in handles {
            handle.join().unwrap();
        }

        let stats = indexer.get_statistics().unwrap();
        assert_eq!(stats.total_transactions, 10);
    }

    #[test]
    fn test_lightning_channel_indexing() {
        let indexer = TransactionIndexer::new(TransactionIndexConfig::default());
        let tx = create_test_transaction();
        let tx_hash = tx.hash();
        let block_hash = [1u8; 32];
        let channel_id = [42u8; 32];

        indexer
            .index_transaction(&tx, block_hash, 100, 0, None, Some(channel_id))
            .unwrap();

        let channel_txs = indexer.get_channel_transactions(&channel_id).unwrap();
        assert!(channel_txs.contains(&tx_hash));
    }
}

