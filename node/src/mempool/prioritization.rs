use crate::config;
use btclib::types::transaction::Transaction;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::time::SystemTime;

/// Configuration for transaction prioritization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrioritizationConfig {
    /// Whether to enable transaction prioritization
    pub enabled: bool,
    /// Minimum fee rate for ancestor package (novas per byte)
    pub min_ancestor_fee_rate: u64,
    /// Maximum ancestor package size in bytes
    pub max_ancestor_size: usize,
    /// Whether to consider descendant packages
    pub consider_descendants: bool,
    /// Fee rate decay factor per hour (0.0 to 1.0)
    pub fee_rate_decay: f64,
}

impl From<config::MempoolConfig> for PrioritizationConfig {
    fn from(config: config::MempoolConfig) -> Self {
        Self {
            max_ancestor_size: 101_000, // ~100KB
            min_ancestor_fee_rate: config.min_fee_rate as u64,
            enabled: true,
            consider_descendants: true,
            fee_rate_decay: 0.0,
        }
    }
}

impl Default for PrioritizationConfig {
    fn default() -> Self {
        Self {
            max_ancestor_size: 101_000, // ~100KB
            min_ancestor_fee_rate: 1,   // 1 nova/byte
            enabled: true,
            consider_descendants: true,
            fee_rate_decay: 0.0,
        }
    }
}

/// Entry for transaction prioritization
#[derive(Debug, Clone)]
pub struct PrioritizedTransaction {
    pub transaction: Transaction,
    pub fee_rate: u64,
    pub size: usize,
    pub timestamp: SystemTime,
    pub ancestors: HashSet<[u8; 32]>,
    pub descendants: HashSet<[u8; 32]>,
}

impl PrioritizedTransaction {
    pub fn new(transaction: Transaction, fee_rate: u64, size: usize) -> Self {
        Self {
            transaction,
            fee_rate,
            size,
            timestamp: SystemTime::now(),
            ancestors: HashSet::new(),
            descendants: HashSet::new(),
        }
    }

    /// Calculate effective fee rate including time decay
    pub fn effective_fee_rate(&self, config: &PrioritizationConfig) -> f64 {
        let age_hours = SystemTime::now()
            .duration_since(self.timestamp)
            .unwrap_or_default()
            .as_secs_f64()
            / 3600.0;

        let decay_factor: f64 = 1.0 - (config.fee_rate_decay * age_hours);
        self.fee_rate as f64 * decay_factor.max(0.0)
    }
}

pub struct TransactionPrioritizer {
    config: PrioritizationConfig,
    transactions: HashMap<[u8; 32], PrioritizedTransaction>,
}

impl TransactionPrioritizer {
    pub fn new(config: PrioritizationConfig) -> Self {
        Self {
            config,
            transactions: HashMap::new(),
        }
    }

    /// Add a transaction to the prioritizer
    pub fn add_transaction(
        &mut self,
        transaction: Transaction,
        fee_rate: u64,
        size: usize,
    ) -> bool {
        let tx_hash = transaction.hash();

        // Create prioritized transaction
        let mut ptx = PrioritizedTransaction::new(transaction, fee_rate, size);

        // Calculate ancestors
        let ancestors = self.calculate_ancestors(&ptx.transaction);
        if !self.validate_ancestor_limits(&ancestors, size) {
            return false;
        }
        ptx.ancestors = ancestors;

        // Update descendant information for ancestors
        for ancestor_hash in &ptx.ancestors {
            if let Some(ancestor) = self.transactions.get_mut(ancestor_hash) {
                ancestor.descendants.insert(tx_hash);
            }
        }

        self.transactions.insert(tx_hash, ptx);
        true
    }

    /// Calculate ancestors for a transaction
    fn calculate_ancestors(&self, transaction: &Transaction) -> HashSet<[u8; 32]> {
        let mut ancestors = HashSet::new();
        let mut queue: Vec<[u8; 32]> = transaction
            .inputs()
            .iter()
            .map(|input| input.prev_tx_hash())
            .filter(|hash| self.transactions.contains_key(hash))
            .collect();

        while let Some(tx_hash) = queue.pop() {
            if ancestors.insert(tx_hash) {
                if let Some(tx) = self.transactions.get(&tx_hash) {
                    queue.extend(
                        tx.transaction
                            .inputs()
                            .iter()
                            .map(|input| input.prev_tx_hash())
                            .filter(|hash| self.transactions.contains_key(hash)),
                    );
                }
            }
        }

        ancestors
    }

    /// Validate ancestor limits
    fn validate_ancestor_limits(&self, ancestors: &HashSet<[u8; 32]>, tx_size: usize) -> bool {
        if ancestors.len() >= self.config.max_ancestor_size {
            return false;
        }

        let ancestor_size: usize = ancestors
            .iter()
            .filter_map(|hash| self.transactions.get(hash))
            .map(|tx| tx.size)
            .sum::<usize>()
            + tx_size;

        if ancestor_size >= self.config.max_ancestor_size {
            return false;
        }

        true
    }

    /// Get transactions sorted by priority
    pub fn get_prioritized_transactions(&self) -> Vec<&Transaction> {
        let mut txs: Vec<_> = self.transactions.values().collect();

        // Sort by effective fee rate
        txs.sort_by(|a, b| {
            b.effective_fee_rate(&self.config)
                .partial_cmp(&a.effective_fee_rate(&self.config))
                .unwrap_or(Ordering::Equal)
        });

        txs.iter().map(|ptx| &ptx.transaction).collect()
    }

    /// Calculate package fee rate for a set of transactions
    pub fn calculate_package_fee_rate(&self, tx_hashes: &[&[u8; 32]]) -> Option<u64> {
        let mut total_fee = 0u64;
        let mut total_size = 0usize;

        for &hash in tx_hashes {
            if let Some(tx) = self.transactions.get(hash) {
                total_fee += tx.fee_rate * tx.size as u64;
                total_size += tx.size;
            } else {
                return None;
            }
        }

        if total_size == 0 {
            None
        } else {
            Some(total_fee / total_size as u64)
        }
    }

    /// Get the fee rate of a transaction by its hash
    pub fn get_transaction_fee_rate(&self, tx_hash: &[u8; 32]) -> Option<u64> {
        self.transactions.get(tx_hash).map(|tx| tx.fee_rate)
    }

    /// Remove a transaction from the prioritizer
    pub fn remove_transaction(&mut self, tx_hash: &[u8; 32]) -> Option<Transaction> {
        if let Some(ptx) = self.transactions.remove(tx_hash) {
            // Update descendant information for ancestors
            for ancestor_hash in &ptx.ancestors {
                if let Some(ancestor) = self.transactions.get_mut(ancestor_hash) {
                    ancestor.descendants.remove(tx_hash);
                }
            }

            // Return the transaction
            Some(ptx.transaction)
        } else {
            None
        }
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
    fn test_transaction_prioritization() {
        let config = PrioritizationConfig::default();
        let mut prioritizer = TransactionPrioritizer::new(config);

        let tx1 = create_test_transaction([1u8; 32], 50_000_000);
        let tx2 = create_test_transaction([2u8; 32], 40_000_000);

        // Store hashes for comparison
        let tx1_hash = tx1.hash();
        let tx2_hash = tx2.hash();

        assert!(prioritizer.add_transaction(tx1, 1, 250));
        assert!(prioritizer.add_transaction(tx2, 2, 250));

        let sorted = prioritizer.get_prioritized_transactions();
        assert_eq!(sorted[0].hash(), tx2_hash); // Higher fee rate should be first
        assert_eq!(sorted[1].hash(), tx1_hash);
    }

    #[test]
    fn test_ancestor_limits() {
        let mut config = PrioritizationConfig::default();
        config.max_ancestor_size = 2;
        let mut prioritizer = TransactionPrioritizer::new(config);

        // Create chain of transactions
        let tx1 = create_test_transaction([1u8; 32], 50_000_000);
        let tx2 = create_test_transaction(tx1.hash(), 40_000_000);
        let tx3 = create_test_transaction(tx2.hash(), 30_000_000);

        assert!(prioritizer.add_transaction(tx1, 1, 250));
        assert!(prioritizer.add_transaction(tx2, 1, 250));
        assert!(!prioritizer.add_transaction(tx3, 1, 250)); // Should fail due to ancestor limit
    }
}
