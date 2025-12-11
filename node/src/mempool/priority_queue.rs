//! Transaction Priority Queue for Optimized Block Building
//!
//! This module implements a multi-factor priority queue for transaction ordering
//! in the mempool, optimizing block template generation for miners.

use supernova_core::types::transaction::Transaction;
use hex;
use std::cmp::Ordering;
use std::collections::BinaryHeap;
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tokio::sync::RwLock;
use tracing::{debug, warn};

/// Priority queue entry containing transaction and metadata
#[derive(Debug, Clone)]
pub struct PriorityQueueEntry {
    /// Transaction hash for quick lookup
    pub tx_hash: [u8; 32],
    /// Transaction reference
    pub transaction: Arc<Transaction>,
    /// Calculated priority score
    pub priority_score: u64,
    /// Fee rate in novas per byte
    pub fee_rate: u64,
    /// Environmental score (0-100)
    pub environmental_score: u8,
    /// Age in minutes since entry
    pub age_minutes: u64,
    /// Whether this is a Lightning channel update
    pub is_lightning_update: bool,
    /// Timestamp when entry was created
    pub created_at: SystemTime,
    /// Transaction size in bytes
    pub size: usize,
}

impl PriorityQueueEntry {
    /// Create a new priority queue entry
    pub fn new(
        transaction: Arc<Transaction>,
        fee_rate: u64,
        environmental_score: u8,
        age_minutes: u64,
        is_lightning_update: bool,
        size: usize,
    ) -> Self {
        let tx_hash = transaction.hash();
        let priority_score = Self::calculate_priority_score(
            fee_rate,
            environmental_score,
            age_minutes,
            is_lightning_update,
        );

        Self {
            tx_hash,
            transaction,
            priority_score,
            fee_rate,
            environmental_score,
            age_minutes,
            is_lightning_update,
            created_at: SystemTime::now(),
            size,
        }
    }

    /// Calculate priority score using multi-factor formula
    /// score = (fee_rate * 1000) + (env_score * 100) + (age_minutes * 10) + (lightning_boost)
    fn calculate_priority_score(
        fee_rate: u64,
        environmental_score: u8,
        age_minutes: u64,
        is_lightning_update: bool,
    ) -> u64 {
        let fee_component = fee_rate.saturating_mul(1000);
        let env_component = environmental_score as u64 * 100;
        let age_component = age_minutes.saturating_mul(10);
        let lightning_boost = if is_lightning_update { 5000 } else { 0 };

        fee_component
            .saturating_add(env_component)
            .saturating_add(age_component)
            .saturating_add(lightning_boost)
    }

    /// Update priority score (for dynamic re-prioritization)
    pub fn update_priority_score(&mut self) {
        // Recalculate age
        let elapsed = self
            .created_at
            .elapsed()
            .unwrap_or_default()
            .as_secs()
            / 60;
        self.age_minutes = elapsed;

        // Recalculate score
        self.priority_score = Self::calculate_priority_score(
            self.fee_rate,
            self.environmental_score,
            self.age_minutes,
            self.is_lightning_update,
        );
    }

    /// Update fee rate and recalculate priority
    pub fn update_fee_rate(&mut self, new_fee_rate: u64) {
        self.fee_rate = new_fee_rate;
        self.update_priority_score();
    }
}

/// BinaryHeap uses max-heap, so we reverse ordering for highest priority first
impl Ord for PriorityQueueEntry {
    fn cmp(&self, other: &Self) -> Ordering {
        // Higher priority score = higher priority
        self.priority_score
            .cmp(&other.priority_score)
            .then_with(|| {
                // Tie-breaker: prefer smaller transactions with same priority
                other.size.cmp(&self.size)
            })
    }
}

impl PartialOrd for PriorityQueueEntry {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for PriorityQueueEntry {
    fn eq(&self, other: &Self) -> bool {
        self.tx_hash == other.tx_hash
    }
}

impl Eq for PriorityQueueEntry {}

/// Configuration for the priority queue
#[derive(Debug, Clone)]
pub struct PriorityQueueConfig {
    /// Maximum number of transactions in the queue
    pub max_size: usize,
    /// Minimum fee rate for inclusion (novas per byte)
    pub min_fee_rate: u64,
    /// Re-prioritization interval in seconds
    pub reprioritization_interval: Duration,
    /// Maximum age before eviction (in minutes)
    pub max_age_minutes: u64,
    /// Lightning boost multiplier
    pub lightning_boost: u64,
}

impl Default for PriorityQueueConfig {
    fn default() -> Self {
        Self {
            max_size: 10000,
            min_fee_rate: 1,
            reprioritization_interval: Duration::from_secs(60),
            max_age_minutes: 72 * 60, // 72 hours
            lightning_boost: 5000,
        }
    }
}

/// Priority queue metrics
#[derive(Debug, Clone, Default)]
pub struct PriorityQueueMetrics {
    /// Total transactions added
    pub total_added: u64,
    /// Total transactions removed
    pub total_removed: u64,
    /// Total transactions evicted
    pub total_evicted: u64,
    /// Average priority score
    pub average_priority_score: f64,
    /// Queue size
    pub current_size: usize,
    /// Number of re-prioritizations performed
    pub reprioritizations: u64,
}

/// Transaction Priority Queue for optimized block building
pub struct TransactionPriorityQueue {
    /// Binary heap for O(log n) insertion/removal
    queue: Arc<RwLock<BinaryHeap<PriorityQueueEntry>>>,
    /// Lookup map for O(1) transaction access
    lookup: Arc<RwLock<std::collections::HashMap<[u8; 32], PriorityQueueEntry>>>,
    /// Configuration
    config: PriorityQueueConfig,
    /// Metrics
    metrics: Arc<RwLock<PriorityQueueMetrics>>,
    /// Last re-prioritization time
    last_reprioritization: Arc<RwLock<SystemTime>>,
}

impl TransactionPriorityQueue {
    /// Create a new priority queue
    pub fn new(config: PriorityQueueConfig) -> Self {
        Self {
            queue: Arc::new(RwLock::new(BinaryHeap::new())),
            lookup: Arc::new(RwLock::new(std::collections::HashMap::new())),
            config,
            metrics: Arc::new(RwLock::new(PriorityQueueMetrics::default())),
            last_reprioritization: Arc::new(RwLock::new(SystemTime::now())),
        }
    }

    /// Add a transaction to the priority queue
    pub async fn add_transaction(
        &self,
        transaction: Arc<Transaction>,
        fee_rate: u64,
        environmental_score: u8,
        is_lightning_update: bool,
    ) -> Result<(), String> {
        let tx_hash = transaction.hash();

        // Validate environmental score range (0-100)
        if environmental_score > 100 {
            return Err(format!(
                "Environmental score {} exceeds maximum 100",
                environmental_score
            ));
        }

        // Check minimum fee rate
        if fee_rate < self.config.min_fee_rate {
            return Err(format!(
                "Fee rate {} below minimum {}",
                fee_rate, self.config.min_fee_rate
            ));
        }

        // Calculate transaction size
        let size = bincode::serialize(transaction.as_ref())
            .map_err(|e| format!("Serialization error: {}", e))?
            .len();

        // Check if transaction already exists
        {
            let lookup = self.lookup.read().await;
            if lookup.contains_key(&tx_hash) {
                return Err("Transaction already in queue".to_string());
            }
        }

        // Create entry with age 0 (will be updated on re-prioritization)
        let entry = PriorityQueueEntry::new(
            transaction,
            fee_rate,
            environmental_score,
            0,
            is_lightning_update,
            size,
        );

        // Check queue size and evict if necessary
        {
            let mut queue = self.queue.write().await;
            let mut lookup = self.lookup.write().await;

            // Evict lowest priority transactions if at capacity
            while queue.len() >= self.config.max_size {
                if let Some(lowest) = queue.pop() {
                    lookup.remove(&lowest.tx_hash);
                    let mut metrics = self.metrics.write().await;
                    metrics.total_evicted += 1;
                    metrics.total_removed += 1;
                    debug!("Evicted transaction {} from priority queue", hex::encode(lowest.tx_hash));
                } else {
                    break;
                }
            }

            // Add new entry
            let entry_clone = entry.clone();
            queue.push(entry);
            lookup.insert(tx_hash, entry_clone);

            // Update metrics
            let mut metrics = self.metrics.write().await;
            metrics.total_added += 1;
            metrics.current_size = queue.len();
        }

        Ok(())
    }

    /// Remove a transaction from the queue
    pub async fn remove_transaction(&self, tx_hash: &[u8; 32]) -> Option<Arc<Transaction>> {
        let mut queue = self.queue.write().await;
        let mut lookup = self.lookup.write().await;

        // Remove from lookup
        let entry = lookup.remove(tx_hash)?;

        // Rebuild queue without this transaction
        let mut new_queue = BinaryHeap::new();
        while let Some(e) = queue.pop() {
            if e.tx_hash != *tx_hash {
                new_queue.push(e);
            }
        }
        *queue = new_queue;

        // Update metrics
        let mut metrics = self.metrics.write().await;
        metrics.total_removed += 1;
        metrics.current_size = queue.len();

        Some(entry.transaction)
    }

    /// Get the highest priority transaction without removing it
    pub async fn peek(&self) -> Option<PriorityQueueEntry> {
        let queue = self.queue.read().await;
        queue.peek().cloned()
    }

    /// Pop the highest priority transaction
    pub async fn pop(&self) -> Option<PriorityQueueEntry> {
        let mut queue = self.queue.write().await;
        let mut lookup = self.lookup.write().await;

        if let Some(entry) = queue.pop() {
            lookup.remove(&entry.tx_hash);

            // Update metrics
            let mut metrics = self.metrics.write().await;
            metrics.total_removed += 1;
            metrics.current_size = queue.len();

            Some(entry)
        } else {
            None
        }
    }

    /// Get iterator over transactions in priority order (for block template creation)
    pub async fn iter_priority_order(&self) -> Vec<Arc<Transaction>> {
        let queue = self.queue.read().await;
        queue
            .iter()
            .map(|entry| Arc::clone(&entry.transaction))
            .collect()
    }

    /// Update fee rate for a transaction (for RBF)
    pub async fn update_fee_rate(
        &self,
        tx_hash: &[u8; 32],
        new_fee_rate: u64,
    ) -> Result<(), String> {
        let mut queue = self.queue.write().await;
        let mut lookup = self.lookup.write().await;

        // Find and update entry
        if let Some(entry) = lookup.get_mut(tx_hash) {
            entry.update_fee_rate(new_fee_rate);

            // Rebuild queue to maintain heap property
            let mut new_queue = BinaryHeap::new();
            for (_, entry) in lookup.iter() {
                new_queue.push(entry.clone());
            }
            *queue = new_queue;

            Ok(())
        } else {
            Err("Transaction not found in queue".to_string())
        }
    }

    /// Re-prioritize all transactions (updates age and recalculates scores)
    pub async fn reprioritize(&self) {
        let mut queue = self.queue.write().await;
        let mut lookup = self.lookup.write().await;

        // Update all entries
        for entry in lookup.values_mut() {
            entry.update_priority_score();
        }

        // Rebuild queue with updated priorities
        let mut new_queue = BinaryHeap::new();
        for (_, entry) in lookup.iter() {
            new_queue.push(entry.clone());
        }
        *queue = new_queue;

        // Update metrics
        let mut metrics = self.metrics.write().await;
        metrics.reprioritizations += 1;

        // Update last re-prioritization time
        *self.last_reprioritization.write().await = SystemTime::now();

        debug!("Re-prioritized {} transactions", lookup.len());
    }

    /// Check if re-prioritization is needed
    pub async fn needs_reprioritization(&self) -> bool {
        let last = *self.last_reprioritization.read().await;
        last.elapsed()
            .map(|d| d >= self.config.reprioritization_interval)
            .unwrap_or(true)
    }

    /// Remove expired transactions
    pub async fn remove_expired(&self) -> usize {
        let mut queue = self.queue.write().await;
        let mut lookup = self.lookup.write().await;

        let mut removed_count = 0;

        // Collect expired transaction hashes
        let expired: Vec<[u8; 32]> = lookup
            .iter()
            .filter(|(_, entry)| {
                entry
                    .created_at
                    .elapsed()
                    .map(|d| d.as_secs() / 60 >= self.config.max_age_minutes)
                    .unwrap_or(false)
            })
            .map(|(hash, _)| *hash)
            .collect();

        // Remove expired transactions
        for tx_hash in &expired {
            lookup.remove(tx_hash);
            removed_count += 1;
        }

        // Rebuild queue without expired transactions
        let mut new_queue = BinaryHeap::new();
        for (_, entry) in lookup.iter() {
            new_queue.push(entry.clone());
        }
        *queue = new_queue;

        // Update metrics
        let mut metrics = self.metrics.write().await;
        metrics.total_evicted += removed_count as u64;
        metrics.total_removed += removed_count as u64;
        metrics.current_size = queue.len();

        if removed_count > 0 {
            debug!("Removed {} expired transactions", removed_count);
        }

        removed_count
    }

    /// Get queue size
    pub async fn len(&self) -> usize {
        self.queue.read().await.len()
    }

    /// Check if queue is empty
    pub async fn is_empty(&self) -> bool {
        self.queue.read().await.is_empty()
    }

    /// Get metrics
    pub async fn get_metrics(&self) -> PriorityQueueMetrics {
        let metrics = self.metrics.read().await;
        let queue = self.queue.read().await;

        // Calculate average priority score
        let total_score: u64 = queue.iter().map(|e| e.priority_score).sum();
        let count = queue.len();
        let average_priority_score = if count > 0 {
            total_score as f64 / count as f64
        } else {
            0.0
        };

        PriorityQueueMetrics {
            total_added: metrics.total_added,
            total_removed: metrics.total_removed,
            total_evicted: metrics.total_evicted,
            average_priority_score,
            current_size: count,
            reprioritizations: metrics.reprioritizations,
        }
    }

    /// Clear all transactions
    pub async fn clear(&self) {
        let mut queue = self.queue.write().await;
        let mut lookup = self.lookup.write().await;

        queue.clear();
        lookup.clear();

        // Reset metrics
        let mut metrics = self.metrics.write().await;
        metrics.current_size = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use supernova_core::types::transaction::{Transaction, TransactionInput, TransactionOutput};

    fn create_test_transaction(value: u64) -> Arc<Transaction> {
        Arc::new(Transaction::new(
            1, // version
            vec![TransactionInput::new([1u8; 32], 0, vec![], 0xffffffff)],
            vec![TransactionOutput::new(value, vec![])],
            0, // lock_time
        ))
    }

    #[tokio::test]
    async fn test_priority_ordering() {
        let queue = TransactionPriorityQueue::new(PriorityQueueConfig::default());

        // Add transactions with different fee rates
        let tx1 = create_test_transaction(100_000_000);
        let tx2 = create_test_transaction(200_000_000);
        let tx3 = create_test_transaction(300_000_000);

        queue
            .add_transaction(Arc::clone(&tx1), 10, 50, false)
            .await
            .unwrap();
        queue
            .add_transaction(Arc::clone(&tx2), 20, 50, false)
            .await
            .unwrap();
        queue
            .add_transaction(Arc::clone(&tx3), 30, 50, false)
            .await
            .unwrap();

        // Highest fee rate should be popped first
        let first = queue.pop().await.unwrap();
        assert_eq!(first.fee_rate, 30);

        let second = queue.pop().await.unwrap();
        assert_eq!(second.fee_rate, 20);

        let third = queue.pop().await.unwrap();
        assert_eq!(third.fee_rate, 10);
    }

    #[tokio::test]
    async fn test_environmental_bonus() {
        let queue = TransactionPriorityQueue::new(PriorityQueueConfig::default());

        let tx1 = create_test_transaction(100_000_000);
        let tx2 = create_test_transaction(200_000_000);

        // Same fee rate, but tx2 has higher environmental score
        queue
            .add_transaction(Arc::clone(&tx1), 10, 50, false)
            .await
            .unwrap();
        queue
            .add_transaction(Arc::clone(&tx2), 10, 80, false)
            .await
            .unwrap();

        // Higher environmental score should be popped first
        let first = queue.pop().await.unwrap();
        assert_eq!(first.environmental_score, 80);
    }

    #[tokio::test]
    async fn test_age_factor_prevents_starvation() {
        let queue = TransactionPriorityQueue::new(PriorityQueueConfig::default());

        let tx1 = create_test_transaction(100_000_000);
        let tx2 = create_test_transaction(200_000_000);

        // Add tx1 with lower fee
        queue
            .add_transaction(Arc::clone(&tx1), 10, 50, false)
            .await
            .unwrap();

        // Simulate aging by updating created_at
        {
            let mut lookup = queue.lookup.write().await;
            if let Some(entry) = lookup.get_mut(&tx1.hash()) {
                entry.created_at = SystemTime::now() - Duration::from_secs(3600); // 1 hour ago
                entry.age_minutes = 60;
                entry.update_priority_score();
            }
        }

        // Add tx2 with higher fee but newer
        queue
            .add_transaction(Arc::clone(&tx2), 15, 50, false)
            .await
            .unwrap();

        // Re-prioritize to update scores
        queue.reprioritize().await;

        // Old transaction should have higher priority due to age bonus
        let first = queue.pop().await.unwrap();
        assert_eq!(first.tx_hash, tx1.hash());
    }

    #[tokio::test]
    async fn test_lightning_priority_boost() {
        let queue = TransactionPriorityQueue::new(PriorityQueueConfig::default());

        let tx1 = create_test_transaction(100_000_000);
        let tx2 = create_test_transaction(200_000_000);

        // tx1 has higher fee but tx2 is Lightning update
        queue
            .add_transaction(Arc::clone(&tx1), 20, 50, false)
            .await
            .unwrap();
        queue
            .add_transaction(Arc::clone(&tx2), 10, 50, true)
            .await
            .unwrap();

        // Lightning transaction should be popped first despite lower fee
        let first = queue.pop().await.unwrap();
        assert!(first.is_lightning_update);
        assert_eq!(first.fee_rate, 10);
    }

    #[tokio::test]
    async fn test_dynamic_reprioritization() {
        let queue = TransactionPriorityQueue::new(PriorityQueueConfig::default());

        let tx1 = create_test_transaction(100_000_000);
        queue
            .add_transaction(Arc::clone(&tx1), 10, 50, false)
            .await
            .unwrap();

        // Update fee rate
        queue.update_fee_rate(&tx1.hash(), 30).await.unwrap();

        // Verify priority score was updated
        let entry = queue.peek().await.unwrap();
        assert_eq!(entry.fee_rate, 30);
        assert!(entry.priority_score > 30000); // Should be higher with new fee rate
    }

    #[tokio::test]
    async fn test_queue_size_limits() {
        let mut config = PriorityQueueConfig::default();
        config.max_size = 3;
        let queue = TransactionPriorityQueue::new(config);

        // Add 5 transactions
        for i in 0..5 {
            let tx = create_test_transaction((i + 1) * 100_000_000);
            queue
                .add_transaction(Arc::clone(&tx), (i + 1) as u64, 50, false)
                .await
                .unwrap();
        }

        // Queue should only contain 3 highest priority transactions
        assert_eq!(queue.len().await, 3);

        // Verify lowest fee transactions were evicted
        let metrics = queue.get_metrics().await;
        assert_eq!(metrics.total_evicted, 2);
    }
}

