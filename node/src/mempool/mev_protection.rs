//! MEV (Maximal Extractable Value) Protection Module
//!
//! This module implements protection against frontrunning and other MEV attacks
//! using a commit-reveal scheme and fair ordering mechanisms.

use crate::mempool::error::{MempoolError, MempoolResult};
use bincode;
use btclib::hash::Hash256;
use btclib::types::transaction::Transaction;
use rand::{thread_rng, Rng};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

/// Configuration for MEV protection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MEVProtectionConfig {
    /// Enable commit-reveal scheme
    pub enable_commit_reveal: bool,

    /// Time window for commit phase (seconds)
    pub commit_phase_duration: u64,

    /// Time window for reveal phase (seconds)
    pub reveal_phase_duration: u64,

    /// Enable transaction encryption in mempool
    pub enable_encrypted_mempool: bool,

    /// Enable fair ordering (randomized within fee tiers)
    pub enable_fair_ordering: bool,

    /// Maximum transactions per batch
    pub batch_size: usize,
}

impl Default for MEVProtectionConfig {
    fn default() -> Self {
        Self {
            enable_commit_reveal: true,
            commit_phase_duration: 10, // 10 seconds
            reveal_phase_duration: 5,  // 5 seconds
            enable_encrypted_mempool: true,
            enable_fair_ordering: true,
            batch_size: 100,
        }
    }
}

/// Committed transaction waiting for reveal
#[derive(Debug, Clone)]
struct CommittedTransaction {
    /// Commitment hash
    commitment: Hash256,

    /// Fee declared in commitment
    declared_fee: u64,

    /// Timestamp when committed
    commit_time: Instant,

    /// Optional sender identifier (for reveal matching)
    sender_id: Option<Vec<u8>>,
}

/// MEV Protection System
pub struct MEVProtection {
    /// Configuration
    config: MEVProtectionConfig,

    /// Committed transactions waiting for reveal
    commitments: Arc<RwLock<HashMap<Hash256, CommittedTransaction>>>,

    /// Revealed transactions ready for inclusion
    revealed_queue: Arc<RwLock<VecDeque<Transaction>>>,

    /// Encrypted transactions (if encrypted mempool enabled)
    encrypted_pool: Arc<RwLock<HashMap<Hash256, Vec<u8>>>>,

    /// Current batch being assembled
    current_batch: Arc<RwLock<Vec<Transaction>>>,

    /// Last batch assembly time
    last_batch_time: Arc<RwLock<Instant>>,
}

impl MEVProtection {
    /// Create new MEV protection system
    pub fn new(config: MEVProtectionConfig) -> Self {
        Self {
            config,
            commitments: Arc::new(RwLock::new(HashMap::new())),
            revealed_queue: Arc::new(RwLock::new(VecDeque::new())),
            encrypted_pool: Arc::new(RwLock::new(HashMap::new())),
            current_batch: Arc::new(RwLock::new(Vec::new())),
            last_batch_time: Arc::new(RwLock::new(Instant::now())),
        }
    }

    /// Submit a transaction commitment (phase 1 of commit-reveal)
    pub async fn submit_commitment(
        &self,
        tx_hash: Hash256,
        declared_fee: u64,
        commitment: Hash256,
        sender_id: Option<Vec<u8>>,
    ) -> MempoolResult<()> {
        if !self.config.enable_commit_reveal {
            return Err(MempoolError::InvalidTransaction(
                "Commit-reveal not enabled".to_string(),
            ));
        }

        let mut commitments = self.commitments.write().await;

        // Check if already committed
        if commitments.contains_key(&commitment) {
            return Err(MempoolError::DuplicateTransaction);
        }

        commitments.insert(
            commitment,
            CommittedTransaction {
                commitment,
                declared_fee,
                commit_time: Instant::now(),
                sender_id,
            },
        );

        Ok(())
    }

    /// Reveal a committed transaction (phase 2 of commit-reveal)
    pub async fn reveal_transaction(
        &self,
        transaction: Transaction,
        nonce: Vec<u8>,
    ) -> MempoolResult<()> {
        if !self.config.enable_commit_reveal {
            return self.add_transaction_direct(transaction).await;
        }

        // Calculate commitment hash
        let commitment = self.calculate_commitment(&transaction, &nonce)?;

        let mut commitments = self.commitments.write().await;

        // Find and verify commitment
        let committed = commitments.remove(&commitment).ok_or_else(|| {
            MempoolError::InvalidTransaction("No matching commitment found".to_string())
        })?;

        // Verify timing
        let elapsed = committed.commit_time.elapsed();
        let min_wait = Duration::from_secs(self.config.commit_phase_duration);
        let max_wait = min_wait + Duration::from_secs(self.config.reveal_phase_duration);

        if elapsed < min_wait {
            return Err(MempoolError::InvalidTransaction(
                "Reveal too early".to_string(),
            ));
        }

        if elapsed > max_wait {
            return Err(MempoolError::InvalidTransaction(
                "Reveal too late".to_string(),
            ));
        }

        // Verify declared fee matches actual fee
        let actual_fee = self.calculate_transaction_fee(&transaction)?;
        if actual_fee < committed.declared_fee {
            return Err(MempoolError::InvalidTransaction(
                "Actual fee less than declared".to_string(),
            ));
        }

        // Add to revealed queue
        let mut queue = self.revealed_queue.write().await;
        queue.push_back(transaction);

        Ok(())
    }

    /// Add transaction directly (bypass commit-reveal if disabled)
    async fn add_transaction_direct(&self, transaction: Transaction) -> MempoolResult<()> {
        let mut queue = self.revealed_queue.write().await;
        queue.push_back(transaction);
        Ok(())
    }

    /// Calculate commitment hash for a transaction
    fn calculate_commitment(&self, tx: &Transaction, nonce: &[u8]) -> MempoolResult<Hash256> {
        let mut hasher = Sha256::new();

        // Include transaction data
        let tx_bytes = bincode::serialize(tx).map_err(|e| {
            MempoolError::ValidationFailed(format!("Failed to serialize transaction: {}", e))
        })?;
        hasher.update(&tx_bytes);

        // Include nonce
        hasher.update(nonce);

        let result = hasher.finalize();
        let mut hash = [0u8; 32];
        hash.copy_from_slice(&result);
        Ok(hash)
    }

    /// Calculate transaction fee
    fn calculate_transaction_fee(&self, tx: &Transaction) -> MempoolResult<u64> {
        // Calculate fee using a placeholder output getter
        // In production, this would use the actual UTXO set
        let fee = tx.calculate_fee(|_tx_hash, _index| None).unwrap_or(0);
        Ok(fee)
    }

    /// Get next batch of transactions for block assembly
    pub async fn get_next_batch(&self, max_size: usize) -> Vec<Transaction> {
        let mut queue = self.revealed_queue.write().await;
        let mut batch = Vec::new();

        let batch_size = max_size.min(self.config.batch_size);

        // Drain transactions from revealed queue
        while batch.len() < batch_size && !queue.is_empty() {
            if let Some(tx) = queue.pop_front() {
                batch.push(tx);
            }
        }

        // Apply fair ordering if enabled
        if self.config.enable_fair_ordering {
            self.apply_fair_ordering(&mut batch);
        }

        // Update batch assembly time
        *self.last_batch_time.write().await = Instant::now();

        batch
    }

    /// Apply fair ordering to prevent frontrunning
    fn apply_fair_ordering(&self, transactions: &mut Vec<Transaction>) {
        // Group transactions by fee tier
        let mut fee_tiers: HashMap<u64, Vec<Transaction>> = HashMap::new();

        for tx in transactions.drain(..) {
            let fee = tx.calculate_fee(|_tx_hash, _index| None).unwrap_or(0);
            let fee_tier = (fee / 1000) * 1000; // Round to nearest 1000
            fee_tiers.entry(fee_tier).or_default().push(tx);
        }

        // Sort tiers by fee (descending)
        let mut sorted_tiers: Vec<_> = fee_tiers.into_iter().collect();
        sorted_tiers.sort_by(|a, b| b.0.cmp(&a.0));

        // Randomize within each tier and rebuild transaction list
        let mut rng = thread_rng();
        for (_, mut tier_txs) in sorted_tiers {
            // Fisher-Yates shuffle for fair randomization within tier
            for i in (1..tier_txs.len()).rev() {
                let j = rng.gen_range(0..=i);
                tier_txs.swap(i, j);
            }
            transactions.extend(tier_txs);
        }
    }

    /// Clean up expired commitments
    pub async fn cleanup_expired_commitments(&self) {
        let mut commitments = self.commitments.write().await;
        let max_age = Duration::from_secs(
            self.config.commit_phase_duration + self.config.reveal_phase_duration,
        );

        commitments.retain(|_, committed| committed.commit_time.elapsed() <= max_age);
    }

    /// Get current MEV protection statistics
    pub async fn get_statistics(&self) -> MEVProtectionStats {
        MEVProtectionStats {
            pending_commitments: self.commitments.read().await.len(),
            revealed_transactions: self.revealed_queue.read().await.len(),
            encrypted_transactions: self.encrypted_pool.read().await.len(),
            config: self.config.clone(),
        }
    }
}

/// MEV protection statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MEVProtectionStats {
    pub pending_commitments: usize,
    pub revealed_transactions: usize,
    pub encrypted_transactions: usize,
    pub config: MEVProtectionConfig,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_commit_reveal_flow() {
        let config = MEVProtectionConfig::default();
        let mev_protection = MEVProtection::new(config);

        // Create a mock transaction
        let tx = Transaction::default();
        let tx_hash = [1u8; 32];
        let nonce = vec![42; 16];
        let commitment = mev_protection.calculate_commitment(&tx, &nonce).unwrap();

        // Phase 1: Commit
        mev_protection
            .submit_commitment(
                tx_hash, 1000, // declared fee
                commitment, None,
            )
            .await
            .unwrap();

        // Wait for commit phase
        tokio::time::sleep(Duration::from_secs(11)).await;

        // Phase 2: Reveal
        mev_protection.reveal_transaction(tx, nonce).await.unwrap();

        // Check transaction is in revealed queue
        let stats = mev_protection.get_statistics().await;
        assert_eq!(stats.revealed_transactions, 1);
        assert_eq!(stats.pending_commitments, 0);
    }

    #[test]
    fn test_fair_ordering() {
        let config = MEVProtectionConfig::default();
        let mev_protection = MEVProtection::new(config);

        // Create transactions with different fees
        let mut txs = vec![
            create_test_tx_with_fee(5000),
            create_test_tx_with_fee(5100),
            create_test_tx_with_fee(5200),
            create_test_tx_with_fee(3000),
            create_test_tx_with_fee(3100),
        ];

        mev_protection.apply_fair_ordering(&mut txs);

        // Verify high fee tier comes first
        let fee0 = txs[0].calculate_fee(|_, _| None).unwrap_or(0);
        let fee1 = txs[1].calculate_fee(|_, _| None).unwrap_or(0);
        let fee2 = txs[2].calculate_fee(|_, _| None).unwrap_or(0);
        assert!(fee0 >= 5000);
        assert!(fee1 >= 5000);
        assert!(fee2 >= 5000);

        // But order within tier should be randomized (not always the same)
        // This is probabilistic, so we just verify the grouping
    }

    fn create_test_tx_with_fee(fee: u64) -> Transaction {
        // Mock transaction with specific fee
        let mut tx = Transaction::default();
        // In real implementation, set fee properly
        tx
    }
}
