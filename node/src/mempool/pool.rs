use crate::api::types::{
    MempoolInfo, MempoolTransaction, TransactionFees, TransactionValidationResult,
};
use crate::config;
use crate::mempool::error::MempoolError;
use crate::mempool::rate_limiter::MempoolRateLimiter;
use supernova_core::types::transaction::Transaction;
use dashmap::DashMap;
use hex;
use parking_lot::Mutex;
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tracing::debug;

/// Configuration for the transaction memory pool
#[derive(Debug, Clone)]
pub struct MempoolConfig {
    /// Maximum number of transactions in the pool
    pub max_size: usize,
    /// Maximum age of a transaction before expiry (in seconds)
    pub max_age: u64,
    /// Minimum fee rate (novas per byte) for acceptance
    pub min_fee_rate: u64,
    /// Maximum fee rate (novas per byte) for acceptance
    /// SECURITY (P1-002): Prevents fee sniping attacks and protects users from excessive fees
    pub max_fee_rate: u64,
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
            max_fee_rate: config.max_fee_rate as u64, // SECURITY (P1-002): Wire max_fee_rate
            enable_rbf: config.enable_rbf,
            min_rbf_fee_increase: config.min_rbf_fee_increase,
        }
    }
}

impl Default for MempoolConfig {
    fn default() -> Self {
        Self {
            max_size: 5000,             // Default to 5000 transactions
            max_age: 72 * 3600,         // 72 hours in seconds
            min_fee_rate: 1,            // 1 nova per byte
            max_fee_rate: 100000,       // SECURITY (P1-002): 100K novas/byte max prevents fee sniping
            enable_rbf: true,           // Enable RBF by default
            min_rbf_fee_increase: 10.0, // 10% minimum fee increase
        }
    }
}

/// Entry in the mempool containing a transaction and metadata
#[derive(Debug)]
struct MempoolEntry {
    transaction: Transaction,
    timestamp: SystemTime,
    fee_rate: u64, // Novas per byte
    size: usize,   // Size in bytes
}

/// Thread-safe transaction pool implementation
pub struct TransactionPool {
    /// Main storage using DashMap for thread-safety
    transactions: DashMap<[u8; 32], MempoolEntry>,
    /// SECURITY (R3-53): Index of spent outputs `(prev_tx_hash, prev_output_index)`
    /// -> spending tx hash, used to atomically reject conflicting double-spends at
    /// the mempool admission boundary. Kept consistent with `transactions` on every
    /// insert/remove/evict/expire/replace, all serialized by `modification_lock`.
    spent_outputs: DashMap<([u8; 32], u32), [u8; 32]>,
    /// SECURITY (R3-53): Serializes the check-then-act admission critical section so
    /// two concurrent adds spending the SAME UTXO cannot both pass the conflict check
    /// and both be inserted (TOCTOU double-spend). Held only around mempool mutations.
    modification_lock: Mutex<()>,
    /// Configuration settings
    config: MempoolConfig,
    /// DoS protection rate limiter (SECURITY FIX P1-003)
    rate_limiter: Arc<MempoolRateLimiter>,
}

impl TransactionPool {
    /// Create a new transaction pool with given configuration
    pub fn new(config: MempoolConfig) -> Self {
        Self {
            transactions: DashMap::new(),
            spent_outputs: DashMap::new(),
            modification_lock: Mutex::new(()),
            config,
            rate_limiter: Arc::new(MempoolRateLimiter::new()),
        }
    }

    /// Extract the `(prev_tx_hash, prev_output_index)` references a transaction
    /// spends. Used to maintain the `spent_outputs` double-spend index.
    fn input_refs(transaction: &Transaction) -> Vec<([u8; 32], u32)> {
        transaction
            .inputs()
            .iter()
            .map(|input| (input.prev_tx_hash(), input.prev_output_index()))
            .collect()
    }

    /// Add a transaction to the pool
    pub fn add_transaction(
        &self,
        transaction: Transaction,
        fee_rate: u64,
    ) -> Result<(), MempoolError> {
        // Call the extended version with no peer tracking
        self.add_transaction_from_peer(transaction, fee_rate, None)
    }
    
    /// Add a transaction to the pool with DoS protection
    /// 
    /// SECURITY FIX (P1-003): Enhanced with per-peer rate limiting, memory caps,
    /// and eviction policy to prevent denial-of-service attacks.
    ///
    /// # Arguments
    /// * `transaction` - Transaction to add
    /// * `fee_rate` - Fee rate in novas per byte
    /// * `peer_id` - Optional peer identifier for rate limiting
    ///
    /// # Returns
    /// * `Ok(())` - Transaction added successfully
    /// * `Err(MempoolError)` - Transaction rejected due to validation or DoS protection
    pub fn add_transaction_from_peer(
        &self,
        transaction: Transaction,
        fee_rate: u64,
        peer_id: Option<&str>,
    ) -> Result<(), MempoolError> {
        // SECURITY (R3-53): Serialize the entire admission critical section
        // (conflict check -> eviction -> insert) so it is atomic. Without this,
        // two concurrent tasks admitting different-hash transactions that spend
        // the SAME UTXO could both pass the double-spend check and both insert,
        // leaving conflicting double-spends in the pool and enabling invalid block
        // templates / mempool-poisoning DoS. This is mempool admission policy only;
        // it changes no consensus rule, block validity, or wire/disk format.
        let _guard = self.modification_lock.lock();

        let tx_hash = transaction.hash();

        // Check if transaction already exists
        if self.transactions.contains_key(&tx_hash) {
            return Err(MempoolError::TransactionExists(hex::encode(tx_hash)));
        }

        // Calculate transaction size FIRST (needed for rate limiting)
        let tx_size = bincode::serialize(&transaction)
            .map_err(|e| MempoolError::SerializationError(e.to_string()))?
            .len();

        // CRITICAL SECURITY CHECK: Rate limiting and memory validation
        self.rate_limiter.check_rate_limit(peer_id, tx_size, fee_rate)?;

        // Check minimum fee rate (redundant but explicit)
        if fee_rate < self.config.min_fee_rate {
            return Err(MempoolError::FeeTooLow {
                required: self.config.min_fee_rate,
                provided: fee_rate,
            });
        }

        // SECURITY (P1-002): Check maximum fee rate to prevent fee sniping attacks
        // This protects users from accidentally paying excessive fees
        if fee_rate > self.config.max_fee_rate {
            return Err(MempoolError::FeeTooHigh {
                max_allowed: self.config.max_fee_rate,
                provided: fee_rate,
            });
        }

        // SECURITY (R3-12): Verify the transaction's cryptographic (post-quantum)
        // signature BEFORE accepting it into the pool or re-gossiping it.
        //
        // Without this, the relay boundary accepted transactions carrying a
        // missing / malformed / forged PQC signature and re-broadcast them
        // network-wide; they were only rejected at block-inclusion time. That
        // both violated full-stack PQC verification at the relay boundary and
        // enabled a cheap DoS / mempool-pollution vector.
        //
        // This is a strict subset of consensus authorization: it checks the
        // signature over the canonical sighash (fail-closed) but NOT the
        // key-to-output binding, which requires UTXO/prevout access unavailable
        // here. Full authorization is still enforced by block validation. This
        // is relay/mempool policy only and changes no consensus rule, block
        // validity, or wire/disk format. Ordered after the cheap rate-limit and
        // fee checks so an attacker is throttled before we spend CPU on crypto.
        transaction
            .verify_signature_only()
            .map_err(|e| MempoolError::InvalidTransaction(e.to_string()))?;

        // SECURITY (R3-53): Atomic double-spend rejection. Under `modification_lock`,
        // check whether ANY input this transaction spends is already spent by another
        // transaction in the pool. Because the lock is held across this check and the
        // subsequent insert, two conflicting different-hash transactions cannot race
        // past each other. O(inputs) via the `spent_outputs` index, not an O(n) scan.
        let input_refs = Self::input_refs(&transaction);
        for input_ref in &input_refs {
            if let Some(existing) = self.spent_outputs.get(input_ref) {
                return Err(MempoolError::DoubleSpend(hex::encode(*existing.value())));
            }
        }

        // Check pool size limit
        if self.transactions.len() >= self.config.max_size {
            // Try to evict lower-fee transaction if this one pays more
            if !self.try_evict_for_better_fee(fee_rate, tx_size)? {
                return Err(MempoolError::MempoolFull {
                    current: self.transactions.len(),
                    max: self.config.max_size,
                });
            }
        }

        // SECURITY (R3-53): Register every spent output BEFORE (or atomically with)
        // the insert so the double-spend index stays consistent with `transactions`.
        // Still under `modification_lock`, so no concurrent add observes a partial state.
        for input_ref in &input_refs {
            self.spent_outputs.insert(*input_ref, tx_hash);
        }

        // Create and insert new entry
        let entry = MempoolEntry {
            transaction,
            timestamp: SystemTime::now(),
            fee_rate,
            size: tx_size,
        };

        self.transactions.insert(tx_hash, entry);

        // Record addition for memory tracking
        self.rate_limiter.record_addition(tx_size);

        Ok(())
    }
    
    /// Try to evict a lower-fee transaction to make room
    /// 
    /// SECURITY: Implements fee-based eviction policy to prevent low-fee spam
    /// from blocking high-fee legitimate transactions.
    ///
    /// # Returns
    /// * `Ok(true)` - Evicted a transaction, room available
    /// * `Ok(false)` - No suitable transaction to evict
    /// * `Err(MempoolError)` - Error during eviction
    fn try_evict_for_better_fee(&self, new_fee_rate: u64, _new_size: usize) -> Result<bool, MempoolError> {
        // Find lowest fee transaction
        let mut lowest_fee_entry: Option<([u8; 32], u64, usize)> = None;
        
        for entry in self.transactions.iter() {
            let (hash, mempool_entry) = entry.pair();
            
            match &lowest_fee_entry {
                None => {
                    lowest_fee_entry = Some((*hash, mempool_entry.fee_rate, mempool_entry.size));
                }
                Some((_, current_lowest_fee, _)) => {
                    if mempool_entry.fee_rate < *current_lowest_fee {
                        lowest_fee_entry = Some((*hash, mempool_entry.fee_rate, mempool_entry.size));
                    }
                }
            }
        }
        
        // Evict if new transaction pays significantly more
        if let Some((evict_hash, evict_fee, evict_size)) = lowest_fee_entry {
            // Require new fee to be at least 2x the lowest fee
            if new_fee_rate >= evict_fee * 2 {
                // SECURITY (R3-53): Keep the double-spend index consistent by
                // deregistering the evicted transaction's spent outputs. Runs under
                // the caller's `modification_lock`, so no lock is acquired here.
                if let Some((_, evicted)) = self.transactions.remove(&evict_hash) {
                    for input_ref in Self::input_refs(&evicted.transaction) {
                        self.spent_outputs.remove(&input_ref);
                    }
                }
                self.rate_limiter.record_removal(evict_size);
                
                debug!(
                    "Evicted low-fee tx {:02x}... (fee: {}) for higher-fee tx (fee: {})",
                    evict_hash[0],
                    evict_fee,
                    new_fee_rate
                );
                
                return Ok(true);
            }
        }
        
        Ok(false)
    }

    /// Remove a transaction from the pool
    pub fn remove_transaction(&self, tx_hash: &[u8; 32]) -> Option<Transaction> {
        // SECURITY (R3-53): Serialize with the admission critical section so the
        // spent-output index and the transaction map are never observed inconsistent.
        let _guard = self.modification_lock.lock();
        match self.transactions.remove(tx_hash) {
            Some((_, entry)) => {
                // Deregister this transaction's spent outputs from the index.
                for input_ref in Self::input_refs(&entry.transaction) {
                    self.spent_outputs.remove(&input_ref);
                }
                // Update memory tracking
                self.rate_limiter.record_removal(entry.size);
                Some(entry.transaction)
            }
            None => None,
        }
    }

    /// Get a transaction by its hash
    pub fn get_transaction(&self, tx_hash: &[u8; 32]) -> Option<Transaction> {
        self.transactions
            .get(tx_hash)
            .map(|entry| entry.transaction.clone())
    }

    /// Get the absolute fee (in novas) the pool tracks for a transaction.
    ///
    /// Computed as `fee_rate * size` from the pool entry, matching the value
    /// surfaced by verbose mempool RPC responses. Returns `None` when the
    /// transaction is not in the pool.
    pub fn get_transaction_fee(&self, tx_hash: &[u8; 32]) -> Option<u64> {
        self.transactions
            .get(tx_hash)
            .map(|entry| entry.fee_rate.saturating_mul(entry.size as u64))
    }

    /// Clear expired transactions from the pool
    pub fn clear_expired(&self) -> usize {
        // SECURITY (R3-53): Serialize with admission so the spent-output index stays
        // consistent while entries are pruned.
        let _guard = self.modification_lock.lock();
        let now = SystemTime::now();
        let max_age = Duration::from_secs(self.config.max_age);
        let mut removed = 0;

        self.transactions.retain(|_, entry| {
            let age = now
                .duration_since(entry.timestamp)
                .unwrap_or(Duration::ZERO);
            if age > max_age {
                // Deregister the expired transaction's spent outputs.
                for input_ref in Self::input_refs(&entry.transaction) {
                    self.spent_outputs.remove(&input_ref);
                }
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
        let new_inputs: Vec<_> = transaction
            .inputs()
            .iter()
            .map(|input| (input.prev_tx_hash(), input.prev_output_index()))
            .collect();

        // Check if any existing transaction uses the same inputs
        for entry in self.transactions.iter() {
            let existing_inputs: Vec<_> = entry
                .transaction
                .inputs()
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
        let mut entries: Vec<_> = self
            .transactions
            .iter()
            .map(|ref_multi| (ref_multi.transaction.clone(), ref_multi.fee_rate))
            .collect();

        // Sort by fee rate in descending order
        entries.sort_by(|a, b| b.1.cmp(&a.1));

        entries.into_iter().map(|(tx, _)| tx).collect()
    }

    /// Clear all transactions from the pool
    pub fn clear_all(&self) -> Result<(), MempoolError> {
        // SECURITY (R3-53): Clear both maps under the lock so they stay consistent.
        let _guard = self.modification_lock.lock();
        self.transactions.clear();
        self.spent_outputs.clear();
        Ok(())
    }

    /// Attempt to replace an existing transaction with a higher-fee version (RBF)
    pub fn replace_transaction(
        &self,
        new_transaction: Transaction,
        fee_rate: u64,
    ) -> Result<Option<Transaction>, MempoolError> {
        // Check if RBF is enabled
        if !self.config.enable_rbf {
            return Err(MempoolError::InvalidTransaction(
                "Replace-By-Fee is disabled".to_string(),
            ));
        }

        // SECURITY (R3-53): Serialize RBF with the admission critical section so the
        // conflict discovery, removals, and insert are atomic and the spent-output
        // index cannot be observed inconsistent by a concurrent add.
        let _guard = self.modification_lock.lock();

        let tx_hash = new_transaction.hash();

        // Check if the transaction already exists
        if self.transactions.contains_key(&tx_hash) {
            return Err(MempoolError::TransactionExists(hex::encode(tx_hash)));
        }

        // Find transactions in the mempool that have inputs overlapping with the new transaction
        let conflicting_txs: Vec<([u8; 32], MempoolEntry)> =
            self.find_conflicting_transactions(&new_transaction);

        if conflicting_txs.is_empty() {
            // No conflicts, this is not an RBF but a new transaction
            return Err(MempoolError::InvalidTransaction(
                "No conflicting transactions found for RBF".to_string(),
            ));
        }

        // Calculate the total fee of the conflicting transactions
        let _total_conflicting_size: usize =
            conflicting_txs.iter().map(|(_, entry)| entry.size).sum();
        let total_conflicting_fee: u64 = conflicting_txs
            .iter()
            .map(|(_, entry)| entry.fee_rate * entry.size as u64)
            .sum();

        // Calculate the new transaction size
        let new_tx_size = match bincode::serialize(&new_transaction) {
            Ok(bytes) => bytes.len(),
            Err(e) => return Err(MempoolError::SerializationError(e.to_string())),
        };

        // Calculate the new transaction fee
        let new_tx_fee = fee_rate * new_tx_size as u64;

        // Check if the new transaction's fee is sufficiently higher than the conflicting transactions
        let min_increase = 1.0 + (self.config.min_rbf_fee_increase / 100.0);
        let min_required_fee = ((total_conflicting_fee as f64) * min_increase) as u64;

        if new_tx_fee < min_required_fee {
            return Err(MempoolError::FeeTooLow {
                required: min_required_fee,
                provided: new_tx_fee,
            });
        }

        // Remove all conflicting transactions
        let mut removed_txs = Vec::new();
        for (hash, _entry) in conflicting_txs {
            if let Some((_, entry)) = self.transactions.remove(&hash) {
                // SECURITY (R3-53): Deregister the replaced transaction's spent outputs
                // so the index reflects only live transactions.
                for input_ref in Self::input_refs(&entry.transaction) {
                    self.spent_outputs.remove(&input_ref);
                }
                removed_txs.push(entry.transaction);
            }
        }

        // SECURITY (R3-53): Register the replacement transaction's spent outputs.
        for input_ref in Self::input_refs(&new_transaction) {
            self.spent_outputs.insert(input_ref, tx_hash);
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
    fn find_conflicting_transactions(
        &self,
        transaction: &Transaction,
    ) -> Vec<([u8; 32], MempoolEntry)> {
        // Get all input references from the new transaction
        let new_inputs: Vec<_> = transaction
            .inputs()
            .iter()
            .map(|input| (input.prev_tx_hash(), input.prev_output_index()))
            .collect();

        let mut conflicting_txs = Vec::new();

        // Check all transactions in the mempool for conflicts
        for entry in self.transactions.iter() {
            let tx_hash = *entry.key();
            let existing_inputs: Vec<_> = entry
                .transaction
                .inputs()
                .iter()
                .map(|input| (input.prev_tx_hash(), input.prev_output_index()))
                .collect();

            // Check for any overlap in inputs
            for input in &new_inputs {
                if existing_inputs.contains(input) {
                    conflicting_txs.push((
                        tx_hash,
                        MempoolEntry {
                            transaction: entry.transaction.clone(),
                            timestamp: entry.timestamp,
                            fee_rate: entry.fee_rate,
                            size: entry.size,
                        },
                    ));
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
        let total_fee: u64 = self
            .transactions
            .iter()
            .map(|entry| entry.fee_rate * entry.size as u64)
            .sum();

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
            max_fee_rate: self
                .transactions
                .iter()
                .map(|entry| entry.fee_rate)
                .max()
                .unwrap_or(0),
            avg_fee_rate,
        }
    }

    /// Get transactions with pagination and sorting
    pub fn get_transactions(
        &self,
        limit: usize,
        offset: usize,
        sort: &str,
    ) -> Result<Vec<MempoolTransaction>, MempoolError> {
        let mut entries: Vec<_> = self
            .transactions
            .iter()
            .map(|entry| {
                let tx_hash = *entry.key();
                let entry_val = entry.value();
                MempoolTransaction {
                    txid: hex::encode(tx_hash),
                    size: entry_val.size,
                    fee: entry_val.fee_rate * entry_val.size as u64,
                    fee_rate: entry_val.fee_rate,
                    time: entry_val
                        .timestamp
                        .duration_since(SystemTime::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs(),
                }
            })
            .collect();

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
    pub fn get_transaction_by_id(
        &self,
        txid: &str,
    ) -> Result<Option<MempoolTransaction>, MempoolError> {
        // Parse hex string to bytes
        let tx_hash_bytes = hex::decode(txid)
            .map_err(|e| MempoolError::SerializationError(format!("Invalid hex: {}", e)))?;
        if tx_hash_bytes.len() != 32 {
            return Err(MempoolError::SerializationError(
                "Transaction hash must be 32 bytes".to_string(),
            ));
        }

        let mut tx_hash = [0u8; 32];
        tx_hash.copy_from_slice(&tx_hash_bytes);

        if let Some(entry) = self.transactions.get(&tx_hash) {
            Ok(Some(MempoolTransaction {
                txid: txid.to_string(),
                size: entry.size,
                fee: entry.fee_rate * entry.size as u64,
                fee_rate: entry.fee_rate,
                time: entry
                    .timestamp
                    .duration_since(SystemTime::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs(),
            }))
        } else {
            Ok(None)
        }
    }

    /// Submit a transaction from raw bytes
    pub fn submit_transaction(
        &self,
        raw_tx: &[u8],
        allow_high_fees: bool,
    ) -> Result<String, MempoolError> {
        // Deserialize the transaction
        let transaction: Transaction = bincode::deserialize(raw_tx).map_err(|e| {
            MempoolError::SerializationError(format!("Failed to deserialize transaction: {}", e))
        })?;

        let tx_hash = transaction.hash();

        // Calculate a basic fee rate (this is simplified)
        let _tx_size = raw_tx.len();
        let fee_rate = self.config.min_fee_rate; // Simplified fee calculation

        // Check for high fees if not allowed
        if !allow_high_fees && fee_rate > self.config.min_fee_rate * 10 {
            return Err(MempoolError::FeeTooLow {
                required: self.config.min_fee_rate * 10,
                provided: fee_rate,
            });
        }

        // Add to mempool
        self.add_transaction(transaction, fee_rate)?;

        Ok(hex::encode(tx_hash))
    }

    /// Validate a transaction without adding it to mempool
    pub fn validate_transaction(
        &self,
        raw_tx: &[u8],
    ) -> Result<TransactionValidationResult, MempoolError> {
        // Deserialize the transaction
        let transaction: Transaction = bincode::deserialize(raw_tx).map_err(|e| {
            MempoolError::SerializationError(format!("Failed to deserialize transaction: {}", e))
        })?;

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
            (
                self.config.min_fee_rate,
                self.config.min_fee_rate * 2,
                self.config.min_fee_rate * 5,
            )
        } else {
            // Calculate percentiles from current mempool
            let mut fee_rates: Vec<u64> = self
                .transactions
                .iter()
                .map(|entry| entry.fee_rate)
                .collect();
            fee_rates.sort();

            let len = fee_rates.len();
            let low = fee_rates[len / 4].max(self.config.min_fee_rate);
            let normal = fee_rates[len / 2].max(self.config.min_fee_rate * 2);
            let high = fee_rates[len * 3 / 4].max(self.config.min_fee_rate * 5);

            (low, normal, high)
        };

        // Adjust based on target confirmation time
        let multiplier = match target_conf {
            1 => 2.0,     // Next block - high priority
            2..=3 => 1.5, // 2-3 blocks - normal priority
            4..=6 => 1.0, // 4-6 blocks - normal
            _ => 0.8,     // 7+ blocks - low priority
        };

        Ok(TransactionFees {
            low_priority: (low_priority as f64 * multiplier * 0.8) as u64,
            normal_priority: (normal_priority as f64 * multiplier) as u64,
            high_priority: (high_priority as f64 * multiplier * 1.2) as u64,
            target_blocks: target_conf,
        })
    }

    /// Get current mempool size (number of transactions)
    pub fn size(&self) -> usize {
        self.transactions.len()
    }

    /// Get current mempool size in bytes
    pub fn size_in_bytes(&self) -> usize {
        self.transactions.iter().map(|entry| entry.size).sum()
    }

    /// Get mempool memory usage in bytes
    pub fn get_memory_usage(&self) -> u64 {
        self.size_in_bytes() as u64
    }

    /// Get mempool transactions
    pub fn get_all_transactions(&self) -> Vec<Transaction> {
        self.transactions
            .iter()
            .map(|entry| entry.value().transaction.clone())
            .collect()
    }

    /// Get all mempool entries with their real per-transaction metadata.
    ///
    /// Unlike [`Self::get_all_transactions`], this returns the fee, fee rate and
    /// entry timestamp actually tracked by the pool, suitable for verbose
    /// mempool RPC responses. No pagination or sorting is applied.
    pub fn get_all_transaction_entries(&self) -> Vec<MempoolTransaction> {
        self.transactions
            .iter()
            .map(|entry| {
                let tx_hash = *entry.key();
                let entry_val = entry.value();
                MempoolTransaction {
                    txid: hex::encode(tx_hash),
                    size: entry_val.size,
                    fee: entry_val.fee_rate.saturating_mul(entry_val.size as u64),
                    fee_rate: entry_val.fee_rate,
                    time: entry_val
                        .timestamp
                        .duration_since(SystemTime::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs(),
                }
            })
            .collect()
    }

    /// Get all transactions for a given block
    pub fn get_transactions_for_block(
        &self,
        block: &supernova_core::types::block::Block,
    ) -> Vec<Transaction> {
        let mut transactions = Vec::new();
        for tx in block.transactions() {
            if let Some(mempool_tx) = self.get_transaction(&tx.hash()) {
                transactions.push(mempool_tx);
            }
        }
        transactions
    }

    /// Get fee histogram for the mempool
    pub fn get_fee_histogram(&self) -> Vec<(u64, usize)> {
        // Create buckets for fee rates (in novas/byte)
        let buckets = [1, 2, 5, 10, 20, 50, 100, 200, 500, 1000];
        let mut histogram = Vec::new();

        for (i, &bucket) in buckets.iter().enumerate() {
            let count = self
                .transactions
                .iter()
                .filter(|entry| {
                    let fee_rate = entry.fee_rate;
                    if i == buckets.len() - 1 {
                        // Last bucket: include all fees >= bucket
                        fee_rate >= bucket
                    } else {
                        // Not last bucket: include fees in range [bucket, next_bucket)
                        fee_rate >= bucket && fee_rate < buckets[i + 1]
                    }
                })
                .count();

            if count > 0 {
                histogram.push((bucket, count));
            }
        }

        histogram
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use supernova_core::crypto::quantum::{QuantumKeyPair, QuantumParameters, QuantumScheme};
    use supernova_core::types::transaction::{
        SignatureSchemeType, TransactionInput, TransactionOutput,
    };

    /// Build an UNSIGNED transaction. The mempool now rejects these at the relay
    /// boundary (R3-12), so this is only used to exercise that rejection path.
    fn create_unsigned_transaction(prev_hash: [u8; 32], value: u64) -> Transaction {
        Transaction::new(
            1,
            vec![TransactionInput::new(prev_hash, 0, vec![], 0xffffffff)],
            vec![TransactionOutput::new(value, vec![])],
            0,
        )
    }

    /// Build a transaction carrying a VALID post-quantum (Dilithium) signature so
    /// it passes the mempool's relay-boundary signature check (R3-12). Each call
    /// uses a fresh keypair; the tests here only depend on the signature being
    /// cryptographically valid, not on which key signed it.
    fn create_test_transaction(prev_hash: [u8; 32], value: u64) -> Transaction {
        let mut tx = create_unsigned_transaction(prev_hash, value);
        let params = QuantumParameters {
            scheme: QuantumScheme::Dilithium,
            security_level: 2, // maps to SecurityLevel::Low (Dilithium2)
        };
        let keypair = QuantumKeyPair::generate(params).expect("keypair generation");
        tx.sign(
            &keypair.secret_key,
            &keypair.public_key,
            SignatureSchemeType::Dilithium,
            2,
        )
        .expect("transaction signing");
        tx
    }

    #[test]
    fn test_add_and_get_transaction() {
        let config = MempoolConfig::default();
        let pool = TransactionPool::new(config);

        let tx = create_test_transaction([1u8; 32], 50_000_000);
        let tx_hash = tx.hash();

        // Fee rate must be >= 1000 (rate limiter MIN_FEE_RATE)
        assert!(pool.add_transaction(tx.clone(), 2000).is_ok());

        // Compare transaction hashes instead of transactions directly
        let tx_from_pool = pool.get_transaction(&tx_hash).unwrap();
        assert_eq!(tx_from_pool.hash(), tx.hash());
    }

    #[test]
    fn test_get_all_transaction_entries_reports_real_metadata() {
        let config = MempoolConfig::default();
        let pool = TransactionPool::new(config);

        let tx = create_test_transaction([7u8; 32], 50_000_000);
        let tx_hash = tx.hash();
        let fee_rate = 2000u64;
        let tx_size = bincode::serialize(&tx).unwrap().len();

        let before = std::time::SystemTime::now()
            .duration_since(std::time::SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        assert!(pool.add_transaction(tx, fee_rate).is_ok());

        let entries = pool.get_all_transaction_entries();
        assert_eq!(entries.len(), 1);
        let entry = &entries[0];

        // txid and size are the real values.
        assert_eq!(entry.txid, hex::encode(tx_hash));
        assert_eq!(entry.size, tx_size);

        // Fee and fee rate are real, not the old placeholder of 1000.
        assert_eq!(entry.fee_rate, fee_rate);
        assert_eq!(entry.fee, fee_rate * tx_size as u64);

        // Timestamp is a real entry time, not the old placeholder of 0.
        assert!(entry.time >= before);
        assert!(entry.time != 0);
    }

    #[test]
    fn test_get_transaction_fee_reports_real_fee() {
        let config = MempoolConfig::default();
        let pool = TransactionPool::new(config);

        let tx = create_test_transaction([11u8; 32], 50_000_000);
        let tx_hash = tx.hash();
        let fee_rate = 2000u64;
        let tx_size = bincode::serialize(&tx).unwrap().len();

        assert!(pool.add_transaction(tx, fee_rate).is_ok());

        // Real fee is fee_rate * size, not the old hardcoded placeholder of 1000.
        assert_eq!(
            pool.get_transaction_fee(&tx_hash),
            Some(fee_rate * tx_size as u64)
        );

        // Unknown transactions return None (handler falls back to 0).
        assert_eq!(pool.get_transaction_fee(&[0xabu8; 32]), None);
    }

    #[test]
    fn test_double_spend_detection() {
        let config = MempoolConfig::default();
        let pool = TransactionPool::new(config);

        // Add first transaction (fee rate >= 1000 for rate limiter)
        let tx1 = create_test_transaction([1u8; 32], 50_000_000);
        assert!(pool.add_transaction(tx1, 2000).is_ok());

        // Try to add second transaction spending same output
        let tx2 = create_test_transaction([1u8; 32], 40_000_000);
        assert!(pool.check_double_spend(&tx2));
    }

    /// SECURITY (R3-53): The admission path itself (not just the separate
    /// `check_double_spend` scan) must reject a second, different-hash transaction
    /// that spends an output already spent by a transaction in the pool. This is the
    /// TOCTOU double-spend guard: conflicting transactions must never coexist.
    #[test]
    fn test_add_rejects_conflicting_double_spend() {
        let pool = TransactionPool::new(MempoolConfig::default());

        // tx1 and tx2 spend the SAME input ([1u8;32], 0) but have different outputs,
        // hence different tx hashes.
        let tx1 = create_test_transaction([1u8; 32], 50_000_000);
        let tx2 = create_test_transaction([1u8; 32], 40_000_000);
        assert_ne!(tx1.hash(), tx2.hash(), "test setup: hashes must differ");
        let tx2_hash = tx2.hash();

        // First admission succeeds.
        assert!(pool.add_transaction(tx1, 2000).is_ok());

        // Second, conflicting admission must be rejected at the add boundary.
        let result = pool.add_transaction(tx2, 2000);
        assert!(
            matches!(result, Err(MempoolError::DoubleSpend(_))),
            "conflicting double-spend must be rejected by add path, got: {:?}",
            result
        );
        // And it must NOT have been inserted.
        assert!(pool.get_transaction(&tx2_hash).is_none());
        assert_eq!(pool.size(), 1);
    }

    /// SECURITY (R3-53): Removing a transaction must free its spent outputs so a later
    /// transaction spending the same UTXO can be admitted (index stays consistent).
    #[test]
    fn test_spent_output_index_freed_on_remove() {
        let pool = TransactionPool::new(MempoolConfig::default());

        let tx1 = create_test_transaction([2u8; 32], 50_000_000);
        let tx1_hash = tx1.hash();
        assert!(pool.add_transaction(tx1, 2000).is_ok());

        // Conflicting tx is rejected while tx1 is present.
        let tx2 = create_test_transaction([2u8; 32], 40_000_000);
        assert!(matches!(
            pool.add_transaction(tx2.clone(), 2000),
            Err(MempoolError::DoubleSpend(_))
        ));

        // After removing tx1, the same UTXO is free again.
        assert!(pool.remove_transaction(&tx1_hash).is_some());
        let tx2_hash = tx2.hash();
        assert!(pool.add_transaction(tx2, 2000).is_ok());
        assert!(pool.get_transaction(&tx2_hash).is_some());
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

        // Fee rates must be >= 1000 (rate limiter MIN_FEE_RATE)
        pool.add_transaction(tx1.clone(), 1000).unwrap();
        pool.add_transaction(tx2.clone(), 2000).unwrap();

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

        // Add first transaction with low fee (>= 1000 for rate limiter)
        let tx1 = create_test_transaction([1u8; 32], 50_000_000);
        let tx1_hash = tx1.hash();
        pool.add_transaction(tx1.clone(), 1000).unwrap();

        // Create replacement transaction with same inputs but DIFFERENT output (different hash)
        let tx2 = create_test_transaction([1u8; 32], 50_000_001); // Different value
        let tx2_hash = tx2.hash();

        // Replace should succeed with higher fee rate
        assert!(pool.replace_transaction(tx2.clone(), 2000).is_ok());

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

        // Add first transaction (fee rate >= 1000 for rate limiter)
        let tx1 = create_test_transaction([1u8; 32], 50_000_000);
        pool.add_transaction(tx1.clone(), 1000).unwrap();

        // Create replacement transaction
        let tx2 = create_test_transaction([1u8; 32], 50_000_000);

        // RBF should fail when disabled
        assert!(matches!(
            pool.replace_transaction(tx2, 2000),
            Err(MempoolError::InvalidTransaction(_))
        ));
    }

    #[test]
    fn test_rbf_insufficient_fee() {
        let config = MempoolConfig {
            enable_rbf: true,
            min_rbf_fee_increase: 50.0, // 50% increase required
            ..MempoolConfig::default()
        };
        let pool = TransactionPool::new(config);

        // Add first transaction (fee rate >= 1000 for rate limiter)
        let tx1 = create_test_transaction([1u8; 32], 50_000_000);
        pool.add_transaction(tx1.clone(), 10000).unwrap();

        // Create replacement transaction with DIFFERENT output (different hash)
        let tx2 = create_test_transaction([1u8; 32], 50_000_001);
        let tx3 = create_test_transaction([1u8; 32], 50_000_002);

        // 10% increase (11000) is not enough for 50% RBF requirement
        assert!(matches!(
            pool.replace_transaction(tx2.clone(), 11000),
            Err(MempoolError::FeeTooLow { .. })
        ));

        // 60% increase (16000) should work
        assert!(pool.replace_transaction(tx3, 16000).is_ok());
    }

    /// R3-12: the mempool must reject a transaction that carries NO signature
    /// before it is accepted or re-gossiped, closing the relay-pollution / DoS
    /// vector. Consensus already rejects it at block-inclusion time; this closes
    /// the earlier relay boundary too.
    #[test]
    fn test_rejects_unsigned_transaction() {
        let pool = TransactionPool::new(MempoolConfig::default());

        let tx = create_unsigned_transaction([7u8; 32], 50_000_000);
        let tx_hash = tx.hash();

        let result = pool.add_transaction(tx, 2000);
        assert!(
            matches!(result, Err(MempoolError::InvalidTransaction(_))),
            "unsigned tx must be rejected, got: {:?}",
            result
        );
        // And it must NOT have been inserted (so it cannot be re-gossiped).
        assert!(pool.get_transaction(&tx_hash).is_none());
    }

    /// R3-12: a transaction whose signature bytes have been tampered with must be
    /// rejected at the relay boundary (fail-closed crypto verification).
    #[test]
    fn test_rejects_forged_signature() {
        let pool = TransactionPool::new(MempoolConfig::default());

        // Start from a validly signed tx, then corrupt the signature payload.
        let mut tx = create_test_transaction([9u8; 32], 50_000_000);
        let mut sig = tx
            .signature_data()
            .expect("signed tx has signature data")
            .clone();
        // Flip a bit in the signature so cryptographic verification fails.
        sig.data[0] ^= 0xFF;
        tx.set_signature_data(sig);
        let tx_hash = tx.hash();

        let result = pool.add_transaction(tx, 2000);
        assert!(
            matches!(result, Err(MempoolError::InvalidTransaction(_))),
            "forged-signature tx must be rejected, got: {:?}",
            result
        );
        assert!(pool.get_transaction(&tx_hash).is_none());
    }

    /// R3-12 regression guard: a validly signed transaction is still accepted.
    #[test]
    fn test_accepts_validly_signed_transaction() {
        let pool = TransactionPool::new(MempoolConfig::default());

        let tx = create_test_transaction([3u8; 32], 50_000_000);
        let tx_hash = tx.hash();

        assert!(pool.add_transaction(tx, 2000).is_ok());
        assert!(pool.get_transaction(&tx_hash).is_some());
    }
}
