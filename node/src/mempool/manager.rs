//! Complete Memory Pool Management for Supernova
//!
//! This module provides comprehensive mempool management including orphan handling,
//! package acceptance, CPFP support, and integration with the priority queue.

use crate::mempool::error::MempoolError;
use crate::mempool::pool::MempoolConfig;
use crate::mempool::priority_queue::{PriorityQueueConfig, TransactionPriorityQueue};
use supernova_core::types::transaction::{Transaction, TransactionInput};
use dashmap::DashMap;
use hex;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

/// Input reference for tracking dependencies
#[derive(Debug, Clone, Hash, Eq, PartialEq)]
struct InputRef {
    tx_hash: [u8; 32],
    output_index: u32,
}

/// Enhanced mempool entry with dependency tracking
#[derive(Debug, Clone)]
struct EnhancedMempoolEntry {
    transaction: Arc<Transaction>,
    timestamp: SystemTime,
    fee_rate: u64,
    size: usize,
    fee: u64,
    /// Ancestor transaction hashes
    ancestors: HashSet<[u8; 32]>,
    /// Descendant transaction hashes
    descendants: HashSet<[u8; 32]>,
    /// Input references for dependency tracking
    inputs: Vec<InputRef>,
    /// Whether this is an orphan transaction
    is_orphan: bool,
    /// Environmental score for priority calculation
    environmental_score: u8,
    /// Whether this is a Lightning channel update
    is_lightning_update: bool,
}

impl EnhancedMempoolEntry {
    fn new(
        transaction: Arc<Transaction>,
        fee_rate: u64,
        size: usize,
        fee: u64,
        environmental_score: u8,
        is_lightning_update: bool,
    ) -> Self {
        let inputs: Vec<InputRef> = transaction
            .inputs()
            .iter()
            .map(|input| InputRef {
                tx_hash: input.prev_tx_hash(),
                output_index: input.prev_output_index(),
            })
            .collect();

        Self {
            transaction,
            timestamp: SystemTime::now(),
            fee_rate,
            size,
            fee,
            ancestors: HashSet::new(),
            descendants: HashSet::new(),
            inputs,
            is_orphan: false,
            environmental_score,
            is_lightning_update,
        }
    }

    /// Calculate package fee rate (including ancestors)
    fn package_fee_rate(&self, entries: &DashMap<[u8; 32], EnhancedMempoolEntry>) -> u64 {
        let mut total_fee = self.fee;
        let mut total_size = self.size;

        for ancestor_hash in &self.ancestors {
            if let Some(ancestor) = entries.get(ancestor_hash) {
                total_fee += ancestor.fee;
                total_size += ancestor.size;
            }
        }

        if total_size == 0 {
            return 0;
        }
        total_fee / total_size as u64
    }

    /// Calculate CPFP fee rate (including descendants)
    fn cpfp_fee_rate(&self, entries: &DashMap<[u8; 32], EnhancedMempoolEntry>) -> u64 {
        let mut total_fee = self.fee;
        let mut total_size = self.size;

        for descendant_hash in &self.descendants {
            if let Some(descendant) = entries.get(descendant_hash) {
                total_fee += descendant.fee;
                total_size += descendant.size;
            }
        }

        if total_size == 0 {
            return 0;
        }
        total_fee / total_size as u64
    }
}

/// Orphan transaction pool
struct OrphanPool {
    /// Orphan transactions indexed by their missing input
    orphans: DashMap<InputRef, Vec<[u8; 32]>>,
    /// Orphan transaction entries
    entries: DashMap<[u8; 32], EnhancedMempoolEntry>,
    /// Maximum orphan pool size
    max_size: usize,
}

impl OrphanPool {
    fn new(max_size: usize) -> Self {
        Self {
            orphans: DashMap::new(),
            entries: DashMap::new(),
            max_size,
        }
    }

    /// Add an orphan transaction
    fn add_orphan(&self, tx_hash: [u8; 32], entry: EnhancedMempoolEntry) {
        // Check size limit
        if self.entries.len() >= self.max_size {
            // Evict oldest orphan
            if let Some(oldest_entry) = self
                .entries
                .iter()
                .min_by_key(|e| e.value().timestamp)
            {
                self.remove_orphan(*oldest_entry.key());
            }
        }

        // Index by missing inputs
        for input_ref in &entry.inputs {
            self.orphans
                .entry(input_ref.clone())
                .or_insert_with(Vec::new)
                .push(tx_hash);
        }

        self.entries.insert(tx_hash, entry);
    }

    /// Remove an orphan transaction
    fn remove_orphan(&self, tx_hash: [u8; 32]) -> Option<EnhancedMempoolEntry> {
        if let Some((_, entry)) = self.entries.remove(&tx_hash) {
            // Remove from orphan index
            for input_ref in &entry.inputs {
                if let Some(mut orphans_list) = self.orphans.get_mut(input_ref) {
                    orphans_list.retain(|&h| h != tx_hash);
                    if orphans_list.is_empty() {
                        self.orphans.remove(input_ref);
                    }
                }
            }
            Some(entry)
        } else {
            None
        }
    }

    /// Get orphans waiting for a specific input
    fn get_orphans_for_input(&self, input_ref: &InputRef) -> Vec<[u8; 32]> {
        self.orphans
            .get(input_ref)
            .map(|v| v.value().clone())
            .unwrap_or_default()
    }

    /// Check if transaction is orphan
    fn is_orphan(&self, tx_hash: &[u8; 32]) -> bool {
        self.entries.contains_key(tx_hash)
    }
}

/// Complete Memory Pool Manager
pub struct MempoolManager {
    /// Main transaction pool
    transactions: Arc<DashMap<[u8; 32], EnhancedMempoolEntry>>,
    /// Orphan transaction pool
    orphan_pool: Arc<RwLock<OrphanPool>>,
    /// Priority queue for block template generation
    priority_queue: Arc<TransactionPriorityQueue>,
    /// Configuration
    config: MempoolConfig,
    /// Memory usage in bytes
    memory_usage: Arc<RwLock<usize>>,
    /// Maximum memory usage
    max_memory_bytes: usize,
    /// Transaction expiration time (default 14 days)
    expiration_time: Duration,
}

impl MempoolManager {
    /// Create a new mempool manager
    pub fn new(config: MempoolConfig) -> Self {
        let priority_config = PriorityQueueConfig {
            max_size: config.max_size,
            min_fee_rate: config.min_fee_rate,
            reprioritization_interval: Duration::from_secs(60),
            max_age_minutes: (config.max_age / 60) as u64,
            lightning_boost: 5000,
        };

        Self {
            transactions: Arc::new(DashMap::new()),
            orphan_pool: Arc::new(RwLock::new(OrphanPool::new(config.max_size / 10))),
            priority_queue: Arc::new(TransactionPriorityQueue::new(priority_config)),
            config,
            memory_usage: Arc::new(RwLock::new(0)),
            max_memory_bytes: 100 * 1024 * 1024, // 100 MB default
            expiration_time: Duration::from_secs(14 * 24 * 3600), // 14 days
        }
    }

    /// Add a transaction to the mempool
    pub async fn add_transaction(
        &self,
        transaction: Transaction,
        fee_rate: u64,
        environmental_score: u8,
        is_lightning_update: bool,
    ) -> Result<(), MempoolError> {
        let tx_hash = transaction.hash();
        let tx_arc = Arc::new(transaction);

        // Check if already exists
        if self.transactions.contains_key(&tx_hash) {
            return Err(MempoolError::TransactionExists(hex::encode(tx_hash)));
        }

        // Calculate transaction size and fee
        let size = bincode::serialize(tx_arc.as_ref())
            .map_err(|e| MempoolError::SerializationError(e.to_string()))?
            .len();
        let fee = fee_rate * size as u64;

        // Check minimum fee rate
        if fee_rate < self.config.min_fee_rate {
            return Err(MempoolError::FeeTooLow {
                required: self.config.min_fee_rate,
                provided: fee_rate,
            });
        }

        // Check memory limit
        {
            let mut mem_usage = self.memory_usage.write().await;
            if *mem_usage + size > self.max_memory_bytes {
                // Try to evict low-fee transactions
                if !self.evict_for_memory(size).await {
                    return Err(MempoolError::MemoryLimitExceeded {
                        current: *mem_usage,
                        max: self.max_memory_bytes,
                        tx_size: size,
                    });
                }
            }
            *mem_usage += size;
        }

        // Check if transaction is orphan (missing inputs)
        let missing_inputs = self.check_missing_inputs(tx_arc.as_ref()).await;
        if !missing_inputs.is_empty() {
            // Add to orphan pool
            let mut entry = EnhancedMempoolEntry::new(
                tx_arc,
                fee_rate,
                size,
                fee,
                environmental_score,
                is_lightning_update,
            );
            entry.is_orphan = true;

            let orphan_pool = self.orphan_pool.write().await;
            orphan_pool.add_orphan(tx_hash, entry);
            return Ok(());
        }

        // Create entry and calculate ancestors
        let mut entry = EnhancedMempoolEntry::new(
            tx_arc.clone(),
            fee_rate,
            size,
            fee,
            environmental_score,
            is_lightning_update,
        );

        // Calculate ancestors
        self.calculate_ancestors(&mut entry).await;

        // Validate ancestor/descendant limits
        if !self.validate_package_limits(&entry).await {
            return Err(MempoolError::InvalidTransaction(
                "Package limits exceeded".to_string(),
            ));
        }

        // Add to main pool
        self.transactions.insert(tx_hash, entry.clone());

        // Update descendants of ancestors
        self.update_ancestor_descendants(&tx_hash, &entry.ancestors).await;

        // Add to priority queue
        if let Err(e) = self
            .priority_queue
            .add_transaction(
                tx_arc,
                fee_rate,
                environmental_score,
                is_lightning_update,
            )
            .await
        {
            warn!("Failed to add transaction to priority queue: {}", e);
        }

        // Try to reconnect orphans
        self.reconnect_orphans(&tx_hash).await;

        Ok(())
    }

    /// Check for missing inputs (orphan detection)
    async fn check_missing_inputs(&self, transaction: &Transaction) -> Vec<InputRef> {
        let mut missing = Vec::new();

        for input in transaction.inputs() {
            let input_ref = InputRef {
                tx_hash: input.prev_tx_hash(),
                output_index: input.prev_output_index(),
            };

            // Check if input exists in mempool or blockchain
            if !self.transactions.contains_key(&input_ref.tx_hash) {
                // TODO: Check blockchain UTXO set
                missing.push(input_ref);
            }
        }

        missing
    }

    /// Calculate ancestor transactions
    async fn calculate_ancestors(&self, entry: &mut EnhancedMempoolEntry) {
        let mut ancestors = HashSet::new();
        let mut queue: Vec<InputRef> = entry.inputs.clone();

        while let Some(input_ref) = queue.pop() {
            if let Some(ancestor_entry) = self.transactions.get(&input_ref.tx_hash) {
                if ancestors.insert(input_ref.tx_hash) {
                    // Add ancestor's inputs to queue
                    queue.extend(ancestor_entry.inputs.clone());
                }
            }
        }

        entry.ancestors = ancestors;
    }

    /// Validate package limits (ancestor/descendant size and count)
    async fn validate_package_limits(&self, entry: &EnhancedMempoolEntry) -> bool {
        const MAX_ANCESTOR_SIZE: usize = 101_000; // ~100 KB
        const MAX_ANCESTOR_COUNT: usize = 25;
        const MAX_DESCENDANT_SIZE: usize = 101_000;
        const MAX_DESCENDANT_COUNT: usize = 25;

        // Check ancestor limits
        let ancestor_size: usize = entry
            .ancestors
            .iter()
            .filter_map(|h| self.transactions.get(h))
            .map(|e| e.size)
            .sum::<usize>()
            + entry.size;

        if entry.ancestors.len() > MAX_ANCESTOR_COUNT || ancestor_size > MAX_ANCESTOR_SIZE {
            return false;
        }

        // Check descendant limits (would be updated after insertion)
        // This is a simplified check
        true
    }

    /// Update descendants of ancestor transactions
    async fn update_ancestor_descendants(
        &self,
        tx_hash: &[u8; 32],
        ancestor_hashes: &HashSet<[u8; 32]>,
    ) {
        for ancestor_hash in ancestor_hashes {
            if let Some(mut ancestor) = self.transactions.get_mut(ancestor_hash) {
                ancestor.descendants.insert(*tx_hash);
            }
        }
    }

    /// Reconnect orphan transactions when parent arrives
    async fn reconnect_orphans(&self, parent_hash: &[u8; 32]) {
        let orphan_pool = self.orphan_pool.read().await;

        // Find orphans waiting for this transaction's outputs
        let mut to_reconnect = Vec::new();

        for (i, _) in (0..256).enumerate() {
            let input_ref = InputRef {
                tx_hash: *parent_hash,
                output_index: i as u32,
            };

            let orphans = orphan_pool.get_orphans_for_input(&input_ref);
            to_reconnect.extend(orphans);
        }

        drop(orphan_pool);

        // Try to reconnect each orphan
        for orphan_hash in to_reconnect {
            let orphan_pool = self.orphan_pool.write().await;
            if let Some(mut entry) = orphan_pool.remove_orphan(orphan_hash) {
                drop(orphan_pool);

                // Check if still orphan
                let missing = self.check_missing_inputs(entry.transaction.as_ref()).await;
                if missing.is_empty() {
                    // No longer orphan, add to main pool
                    entry.is_orphan = false;
                    self.calculate_ancestors(&mut entry).await;

                    if self.validate_package_limits(&entry).await {
                        self.transactions.insert(orphan_hash, entry.clone());
                        self.update_ancestor_descendants(&orphan_hash, &entry.ancestors)
                            .await;

                        info!("Reconnected orphan transaction {}", hex::encode(orphan_hash));
                    }
                } else {
                    // Still orphan, put it back
                    let orphan_pool = self.orphan_pool.write().await;
                    orphan_pool.add_orphan(orphan_hash, entry);
                }
            }
        }
    }

    /// Evict transactions to free memory
    async fn evict_for_memory(&self, required_size: usize) -> bool {
        // Find lowest fee transactions to evict
        let mut candidates: Vec<([u8; 32], u64, usize)> = self
            .transactions
            .iter()
            .map(|e| (*e.key(), e.fee_rate, e.size))
            .collect();

        candidates.sort_by_key(|(_, fee_rate, _)| *fee_rate);

        let mut freed = 0;
        let mut to_remove = Vec::new();

        for (hash, _, size) in candidates {
            if freed >= required_size {
                break;
            }
            to_remove.push((hash, size));
            freed += size;
        }

        // Remove transactions
        let mut mem_usage = self.memory_usage.write().await;
        for (hash, size) in to_remove {
            self.transactions.remove(&hash);
            self.priority_queue.remove_transaction(&hash).await;
            *mem_usage -= size;
        }

        freed >= required_size
    }

    /// Replace transaction with higher fee (RBF)
    pub async fn replace_transaction(
        &self,
        new_transaction: Transaction,
        new_fee_rate: u64,
        environmental_score: u8,
        is_lightning_update: bool,
    ) -> Result<Option<Transaction>, MempoolError> {
        if !self.config.enable_rbf {
            return Err(MempoolError::InvalidTransaction(
                "RBF is disabled".to_string(),
            ));
        }

        let new_tx_hash = new_transaction.hash();
        let new_tx_arc = Arc::new(new_transaction);

        // Find conflicting transactions
        let conflicting: Vec<[u8; 32]> = self
            .transactions
            .iter()
            .filter(|e| {
                self.transactions_conflict(e.transaction.as_ref(), new_tx_arc.as_ref())
            })
            .map(|e| *e.key())
            .collect();

        if conflicting.is_empty() {
            return Err(MempoolError::InvalidTransaction(
                "No conflicting transactions found".to_string(),
            ));
        }

        // Calculate total fee of conflicting transactions
        let total_conflicting_fee: u64 = conflicting
            .iter()
            .filter_map(|h| self.transactions.get(h))
            .map(|e| e.fee)
            .sum();

        // Calculate new transaction fee
        let new_size = bincode::serialize(new_tx_arc.as_ref())
            .map_err(|e| MempoolError::SerializationError(e.to_string()))?
            .len();
        let new_fee = new_fee_rate * new_size as u64;

        // Check RBF fee increase requirement
        let min_increase = 1.0 + (self.config.min_rbf_fee_increase / 100.0);
        let min_required_fee = ((total_conflicting_fee as f64) * min_increase) as u64;

        if new_fee < min_required_fee {
            return Err(MempoolError::FeeTooLow {
                required: min_required_fee,
                provided: new_fee,
            });
        }

        // Remove conflicting transactions
        let mut removed = None;
        for hash in &conflicting {
            if let Some((_, entry)) = self.transactions.remove(hash) {
                if removed.is_none() {
                    removed = Some((*entry.transaction).clone());
                }
                self.priority_queue.remove_transaction(hash).await;
            }
        }

        // Add new transaction
        self.add_transaction(
            (*new_tx_arc).clone(),
            new_fee_rate,
            environmental_score,
            is_lightning_update,
        )
        .await?;

        Ok(removed)
    }

    /// Check if two transactions conflict
    fn transactions_conflict(&self, tx1: &Transaction, tx2: &Transaction) -> bool {
        let inputs1: HashSet<InputRef> = tx1
            .inputs()
            .iter()
            .map(|i| InputRef {
                tx_hash: i.prev_tx_hash(),
                output_index: i.prev_output_index(),
            })
            .collect();

        tx2.inputs()
            .iter()
            .any(|i| {
                inputs1.contains(&InputRef {
                    tx_hash: i.prev_tx_hash(),
                    output_index: i.prev_output_index(),
                })
            })
    }

    /// Get transactions for block template (using priority queue)
    pub async fn get_block_template_transactions(
        &self,
        max_size: usize,
    ) -> Vec<Arc<Transaction>> {
        let mut result = Vec::new();
        let mut current_size = 0;

        // Get transactions from priority queue
        let priority_txs = self.priority_queue.iter_priority_order().await;

        for tx in priority_txs {
            let tx_size = bincode::serialize(tx.as_ref())
                .map(|bytes| bytes.len())
                .unwrap_or(0);

            if current_size + tx_size > max_size {
                break;
            }

            // Check if transaction still exists and is not expired
            let tx_hash = tx.hash();
            if let Some(entry) = self.transactions.get(&tx_hash) {
                if !self.is_expired(&entry).await {
                    result.push(tx);
                    current_size += tx_size;
                }
            }
        }

        result
    }

    /// Check if transaction is expired
    async fn is_expired(&self, entry: &EnhancedMempoolEntry) -> bool {
        entry
            .timestamp
            .elapsed()
            .map(|d| d > self.expiration_time)
            .unwrap_or(false)
    }

    /// Remove expired transactions
    pub async fn remove_expired(&self) -> usize {
        let mut removed = 0;
        
        // Collect expired transaction hashes
        let mut expired = Vec::new();
        for entry in self.transactions.iter() {
            if self.is_expired(entry.value()).await {
                expired.push(*entry.key());
            }
        }

        let mut mem_usage = self.memory_usage.write().await;
        for hash in expired {
            if let Some((_, entry)) = self.transactions.remove(&hash) {
                *mem_usage -= entry.size;
                self.priority_queue.remove_transaction(&hash).await;
                removed += 1;
            }
        }

        removed
    }

    /// Get CPFP package (parent + descendants)
    pub async fn get_cpfp_package(&self, tx_hash: &[u8; 32]) -> Vec<Arc<Transaction>> {
        let mut package = Vec::new();

        if let Some(entry) = self.transactions.get(tx_hash) {
            package.push(Arc::clone(&entry.transaction));

            // Add descendants
            for descendant_hash in &entry.descendants {
                if let Some(descendant) = self.transactions.get(descendant_hash) {
                    package.push(Arc::clone(&descendant.transaction));
                }
            }
        }

        package
    }

    /// Get package fee rate (for CPFP evaluation)
    pub async fn get_package_fee_rate(&self, tx_hash: &[u8; 32]) -> Option<u64> {
        if let Some(entry) = self.transactions.get(tx_hash) {
            Some(entry.cpfp_fee_rate(&self.transactions))
        } else {
            None
        }
    }

    /// Get mempool statistics
    pub async fn get_stats(&self) -> MempoolStats {
        let transactions_count = self.transactions.len();
        let orphan_count = self.orphan_pool.read().await.entries.len();
        let memory_usage = *self.memory_usage.read().await;
        let priority_metrics = self.priority_queue.get_metrics().await;

        MempoolStats {
            transaction_count: transactions_count,
            orphan_count,
            memory_usage_bytes: memory_usage,
            average_fee_rate: priority_metrics.average_priority_score as u64 / 1000,
            priority_queue_size: priority_metrics.current_size,
        }
    }
}

/// Mempool statistics
#[derive(Debug, Clone)]
pub struct MempoolStats {
    pub transaction_count: usize,
    pub orphan_count: usize,
    pub memory_usage_bytes: usize,
    pub average_fee_rate: u64,
    pub priority_queue_size: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use supernova_core::types::transaction::{Transaction, TransactionInput, TransactionOutput};

    fn create_test_transaction(prev_hash: [u8; 32], value: u64) -> Transaction {
        Transaction::new(
            1, // version
            vec![TransactionInput::new(prev_hash, 0, vec![], 0xffffffff)],
            vec![TransactionOutput::new(value, vec![])],
            0, // lock_time
        )
    }

    #[tokio::test]
    async fn test_orphan_transaction_handling() {
        let config = MempoolConfig::default();
        let manager = MempoolManager::new(config);

        // Create child transaction (orphan)
        let parent_hash = [1u8; 32];
        let child_tx = create_test_transaction(parent_hash, 50_000_000);

        // Add orphan - should succeed
        assert!(manager
            .add_transaction(child_tx.clone(), 10, 50, false)
            .await
            .is_ok());

        let stats = manager.get_stats().await;
        assert_eq!(stats.orphan_count, 1);

        // Add parent - should reconnect orphan
        let parent_tx = create_test_transaction([0u8; 32], 100_000_000);
        assert!(manager
            .add_transaction(parent_tx.clone(), 10, 50, false)
            .await
            .is_ok());

        // Orphan should be reconnected
        let stats = manager.get_stats().await;
        assert_eq!(stats.orphan_count, 0);
        assert_eq!(stats.transaction_count, 2);
    }

    #[tokio::test]
    async fn test_package_acceptance() {
        let config = MempoolConfig::default();
        let manager = MempoolManager::new(config);

        // Create parent and child transactions
        let parent_tx = create_test_transaction([0u8; 32], 100_000_000);
        let parent_hash = parent_tx.hash();
        let child_tx = create_test_transaction(parent_hash, 50_000_000);

        // Add parent first
        assert!(manager
            .add_transaction(parent_tx, 10, 50, false)
            .await
            .is_ok());

        // Add child (package)
        assert!(manager
            .add_transaction(child_tx, 10, 50, false)
            .await
            .is_ok());

        let stats = manager.get_stats().await;
        assert_eq!(stats.transaction_count, 2);
    }

    #[tokio::test]
    async fn test_memory_limit_eviction() {
        let mut config = MempoolConfig::default();
        config.max_size = 100;
        let manager = MempoolManager::new(config);

        // Add many transactions
        for i in 0..150 {
            let tx = create_test_transaction([i as u8; 32], 100_000_000);
            let _ = manager.add_transaction(tx, i as u64, 50, false).await;
        }

        // Should evict low-fee transactions
        let stats = manager.get_stats().await;
        assert!(stats.transaction_count <= 100);
    }

    #[tokio::test]
    async fn test_transaction_expiration() {
        let config = MempoolConfig::default();
        let manager = MempoolManager::new(config);

        let tx = create_test_transaction([0u8; 32], 100_000_000);
        assert!(manager.add_transaction(tx.clone(), 10, 50, false).await.is_ok());

        // Manually expire (simulate)
        // In real implementation, expiration would be checked periodically
        let removed = manager.remove_expired().await;
        assert_eq!(removed, 0); // Not expired yet
    }

    #[tokio::test]
    async fn test_cpfp_package_evaluation() {
        let config = MempoolConfig::default();
        let manager = MempoolManager::new(config);

        // Create parent with low fee
        let parent_tx = create_test_transaction([0u8; 32], 100_000_000);
        let parent_hash = parent_tx.hash();
        assert!(manager
            .add_transaction(parent_tx, 1, 50, false)
            .await
            .is_ok());

        // Create child with high fee (CPFP)
        let child_tx = create_test_transaction(parent_hash, 50_000_000);
        assert!(manager
            .add_transaction(child_tx, 100, 50, false)
            .await
            .is_ok());

        // Get CPFP package fee rate
        let package_fee_rate = manager.get_package_fee_rate(&parent_hash).await;
        assert!(package_fee_rate.is_some());
        assert!(package_fee_rate.unwrap() > 1); // Should be higher than parent alone
    }

    #[tokio::test]
    async fn test_rbf_conflict_resolution() {
        let mut config = MempoolConfig::default();
        config.enable_rbf = true;
        config.min_rbf_fee_increase = 10.0;
        let manager = MempoolManager::new(config);

        // Add initial transaction
        let tx1 = create_test_transaction([0u8; 32], 100_000_000);
        assert!(manager.add_transaction(tx1.clone(), 10, 50, false).await.is_ok());

        // Replace with higher fee (RBF)
        let tx2 = create_test_transaction([0u8; 32], 100_000_000);
        let result = manager.replace_transaction(tx2, 15, 50, false).await;
        assert!(result.is_ok());

        let stats = manager.get_stats().await;
        assert_eq!(stats.transaction_count, 1); // Should still be 1
    }
}

