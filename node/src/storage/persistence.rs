use super::database::{BlockchainDB, StorageError};
use supernova_core::types::block::Block;
use supernova_core::types::transaction::{Transaction, TransactionOutput};
use crate::blockchain::checkpoint::{validate_checkpoint, can_reorganize_below};
use crate::blockchain::invalidation::{InvalidBlockTracker, InvalidBlockTrackerConfig, InvalidationReason};
use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Duration;
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::{debug, error, info, warn};

const MAX_REORG_DEPTH: u64 = 100;
const MAX_FORK_DISTANCE: u64 = 6;
const FORK_CHOICE_WINDOW: u64 = 10;
const STALE_TIP_THRESHOLD: Duration = Duration::from_secs(3600);

// Add the missing BlockNotFound variant to StorageError in persistence.rs
impl From<&'static str> for StorageError {
    fn from(error: &'static str) -> Self {
        StorageError::DatabaseError(error.to_string())
    }
}

#[derive(Clone)]
pub struct ChainState {
    db: Arc<BlockchainDB>,
    current_height: u64,
    best_block_hash: [u8; 32],
    chain_work: HashMap<[u8; 32], u128>,
    fork_points: HashSet<[u8; 32]>,
    last_reorg_time: SystemTime,
    reorg_count: u64,
    active_forks: HashMap<[u8; 32], ForkInfo>,
    last_block_time: SystemTime,
    rejected_reorgs: u64,
    invalid_block_tracker: Arc<InvalidBlockTracker>,
}

#[derive(Debug)]
pub struct ReorganizationEvent {
    pub old_tip: [u8; 32],
    pub new_tip: [u8; 32],
    pub fork_point: [u8; 32],
    pub fork_height: u64,
    pub blocks_disconnected: u64,
    pub blocks_connected: u64,
    pub timestamp: SystemTime,
    pub time_since_last_reorg: Duration,
    pub fork_choice_reason: ForkChoiceReason,
}

/// Information tracked for each active fork
#[derive(Debug, Clone)]
pub struct ForkInfo {
    pub fork_point_hash: [u8; 32],
    pub fork_point_height: u64,
    pub tip_hash: [u8; 32],
    pub tip_height: u64,
    pub chain_work: u128,
    pub blocks_added: u64,
    pub first_seen: SystemTime,
    pub last_updated: SystemTime,
    pub is_active: bool,
}

/// Reason for choosing a particular fork
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ForkChoiceReason {
    /// Fork has higher accumulated work (primary decision factor)
    HigherChainWork,
    /// First-seen fork in case of equal chain work
    FirstSeen,
    /// Fork with more blocks in case of equal chain work
    MoreBlocks,
    /// Fork with better connectivity (more peers supporting it)
    BetterConnectivity,
    /// Fork with better transaction efficiency
    BetterTransactionEfficiency,
    /// Fork with better block propagation metrics
    BetterPropagation,
    /// Manual override by operator
    ManualSelection,
}

impl ChainState {
    pub fn new(db: Arc<BlockchainDB>) -> Result<Self, StorageError> {
        // Read height as big-endian bytes (to match how we write it)
        let current_height = match db.get_metadata(b"height")? {
            Some(height_bytes) => {
                if height_bytes.len() >= 8 {
                    u64::from_be_bytes(height_bytes[..8].try_into()
                        .map_err(|_| {
                            // ENHANCED ERROR CONTEXT: Height metadata conversion failure during persistence load
                            // This is the FINAL error context enhancement for P2-001!
                            StorageError::DatabaseError(format!(
                                "Invalid height metadata during persistence initialization. \
                                 Expected 8 bytes for u64 blockchain height, got {} bytes. \
                                 This indicates database corruption of the 'height' metadata key. \
                                 Cannot determine blockchain state. Database recovery required. \
                                 Raw bytes: {}",
                                height_bytes.len(),
                                hex::encode(&height_bytes[..height_bytes.len().min(16)])
                            ))
                        })?)
                } else {
                    0
                }
            }
            None => 0,
        };

        let best_block_hash = match db.get_metadata(b"best_hash")? {
            Some(hash_bytes) => {
                let mut hash = [0u8; 32];
                if hash_bytes.len() >= 32 {
                    hash.copy_from_slice(&hash_bytes[..32]);
                }
                hash
            }
            None => [0u8; 32],
        };

        tracing::debug!(
            "ChainState initialized: height={}, best_hash={}",
            current_height,
            hex::encode(&best_block_hash[..8])
        );

        Ok(Self {
            db,
            current_height,
            best_block_hash,
            chain_work: HashMap::new(),
            fork_points: HashSet::new(),
            last_reorg_time: SystemTime::now(),
            reorg_count: 0,
            active_forks: HashMap::new(),
            last_block_time: SystemTime::now(),
            rejected_reorgs: 0,
            invalid_block_tracker: Arc::new(InvalidBlockTracker::new(InvalidBlockTrackerConfig::default())),
        })
    }

    pub fn get_height(&self) -> u64 {
        self.current_height
    }

    pub fn get_best_block_hash(&self) -> [u8; 32] {
        self.best_block_hash
    }

    /// Get the invalid block tracker
    pub fn invalid_block_tracker(&self) -> Arc<InvalidBlockTracker> {
        self.invalid_block_tracker.clone()
    }

    /// Initialize the chain state with a genesis block
    pub fn initialize_with_genesis(&mut self, genesis_block: Block) -> Result<(), StorageError> {
        // Check if already initialized
        if self.current_height > 0 {
            return Err(StorageError::DatabaseError(
                "Chain already initialized".to_string(),
            ));
        }

        // Store genesis block with bloom filter and cache updates
        let genesis_hash = genesis_block.hash();
        tracing::info!("Initializing genesis block {}", hex::encode(&genesis_hash[..8]));
        
        let genesis_data = bincode::serialize(&genesis_block)
            .map_err(|e| StorageError::DatabaseError(format!("Genesis serialization failed: {}", e)))?;
        self.db.store_block(&genesis_hash, &genesis_data)?;
        self.db.flush()?;
        
        tracing::info!("Genesis block stored successfully");
        
        // Set genesis hash in metadata
        self.db.store_metadata(b"genesis_hash", &genesis_hash)?;
        
        // Initialize chain state for genesis
        self.best_block_hash = genesis_hash;
        self.current_height = 0; // Genesis is height 0
        
        // Store height as big-endian bytes (to match get_height() which reads as big-endian)
        self.db.set_metadata(b"height", &self.current_height.to_be_bytes())?;
        self.db.set_metadata(b"best_hash", &genesis_hash)?;
        
        // Process genesis transactions to create initial UTXO set
        self.process_block_transactions(&genesis_block)?;
        
        self.db.flush()?;

        Ok(())
    }

    /// Add a block to the chain
    pub async fn add_block(&mut self, block: &Block) -> Result<(), StorageError> {
        // Process the block using existing async logic
        let block_clone = block.clone();
        self.process_block(block_clone).await?;
        Ok(())
    }

    /// Get a block by hash
    pub fn get_block(&self, hash: &[u8; 32]) -> Option<Block> {
        self.db.get_block(hash).ok().flatten()
    }

    /// Get the genesis block hash
    pub fn get_genesis_hash(&self) -> [u8; 32] {
        // Fetch the genesis block hash from database or use a cached value
        if let Ok(Some(hash)) = self.db.get_metadata(b"genesis_hash") {
            let mut result = [0u8; 32];
            if hash.len() == 32 {
                result.copy_from_slice(&hash);
                return result;
            }
        }

        // Fallback to zeros if not found
        [0u8; 32]
    }

    /// Get total difficulty of the current chain tip
    pub fn get_total_difficulty(&self) -> u64 {
        if let Ok(Some(difficulty_bytes)) = self.db.get_metadata(b"total_difficulty") {
            if let Ok(difficulty) = bincode::deserialize::<u64>(&difficulty_bytes) {
                return difficulty;
            }
        }

        // Default value if not found
        0
    }

    /// Update total difficulty when adding a new block
    fn update_total_difficulty(&mut self, new_block_difficulty: u64) -> Result<(), StorageError> {
        let current_difficulty = self.get_total_difficulty();
        let new_total = current_difficulty.saturating_add(new_block_difficulty);

        let difficulty_bytes =
            bincode::serialize(&new_total).map_err(StorageError::Serialization)?;

        self.db
            .store_metadata(b"total_difficulty", &difficulty_bytes)?;
        Ok(())
    }

    /// Get time since last block was added
    pub fn time_since_last_block(&self) -> Duration {
        SystemTime::now()
            .duration_since(self.last_block_time)
            .unwrap_or_else(|_| Duration::from_secs(0))
    }

    /// Check if the current tip is considered stale
    pub fn is_tip_stale(&self) -> bool {
        self.time_since_last_block() > STALE_TIP_THRESHOLD
    }

    /// Get number of active forks
    pub fn get_active_fork_count(&self) -> usize {
        self.active_forks.len()
    }

    /// Get stats about rejected reorganizations
    pub fn get_rejected_reorg_count(&self) -> u64 {
        self.rejected_reorgs
    }

    /// Get information about active forks
    pub fn get_active_forks(&self) -> Vec<ForkInfo> {
        self.active_forks.values().cloned().collect()
    }

    /// Get the difficulty target for the next block
    pub fn get_difficulty_target(&self) -> u32 {
        // This is a placeholder. In a real implementation, this would involve a
        // sophisticated algorithm based on the timestamps and difficulties of
        // previous blocks.
        // For now, we'll just return a constant value.
        0x1f00ffff // A reasonably low difficulty for testing
    }

    /// Get the current network difficulty as a float
    pub fn get_current_difficulty(&self) -> f64 {
        // Convert target bits to difficulty
        let target = self.get_difficulty_target();

        // Bitcoin difficulty calculation
        // The maximum target (difficulty 1) is 0x1d00ffff in compact form
        // This represents: 0x00000000ffff0000000000000000000000000000000000000000000000000000

        // Convert compact bits to actual target value
        let exponent = (target >> 24) & 0xff;
        let mantissa = target & 0x00ffffff;

        if mantissa == 0 || exponent == 0 {
            return 1.0; // Minimum difficulty
        }

        // Calculate actual target value as f64 to avoid overflow
        let current_target: f64 = if exponent <= 3 {
            (mantissa >> (8 * (3 - exponent))) as f64
        } else {
            (mantissa as f64) * 2f64.powi((8 * (exponent - 3)) as i32)
        };

        if current_target == 0.0 {
            return 1.0; // Prevent division by zero
        }

        // Max target for difficulty 1 (0x1d00ffff)
        // This is 0xffff * 2^(8*(0x1d-3)) = 65535 * 2^208
        let max_target: f64 = 65535.0 * 2f64.powi(208);

        // Calculate difficulty
        max_target / current_target
    }

    pub async fn process_block(&mut self, block: Block) -> Result<bool, StorageError> {
        let block_hash = block.hash();
        let prev_hash = block.prev_block_hash();

        // Update last block time even if we don't accept the block
        self.last_block_time = SystemTime::now();

        if !self.validate_block(&block).await? {
            tracing::warn!("Block validation failed for block at height {}", block.height());
            return Err(StorageError::InvalidBlock);
        }

        if self.db.get_block(&block_hash)?.is_some() {
            tracing::debug!("Block already exists in database");
            // Block already exists, but still update fork info
            self.update_fork_info(&block)?;
            return Ok(false);
        }

        let new_chain_work = self.calculate_chain_work(&block)?;
        tracing::debug!("Calculated chain work: {}", new_chain_work);
        
        if *prev_hash != self.best_block_hash {
            tracing::info!(
                "Block on fork detected: prev={}, best={}",
                hex::encode(prev_hash),
                hex::encode(&self.best_block_hash)
            );
            let current_work = match self.chain_work.get(&self.best_block_hash) {
                Some(work) => *work,
                None => {
                    // If we don't have work for current tip, calculate it
                    if let Ok(Some(block)) = self.db.get_block(&self.best_block_hash) {
                        let work = self.calculate_chain_work(&block)?;
                        self.chain_work.insert(self.best_block_hash, work);
                        work
                    } else {
                        0
                    }
                }
            };

            // Update or create fork info regardless of whether we accept the block
            self.update_fork_info(&block)?;

            // Determine if we should switch to the new fork
            match new_chain_work.cmp(&current_work) {
                Ordering::Greater => {
                    // New chain has more work, attempt reorganization
                    let (fork_point, blocks_to_apply, blocks_to_disconnect) =
                        self.find_fork_point(&block)?;

                    // Check if reorganization is allowed below checkpoint height
                    if let Err(e) = can_reorganize_below(fork_point.height()) {
                        warn!(
                            "Rejected reorganization below checkpoint: {}",
                            e
                        );
                        self.rejected_reorgs += 1;
                        return Ok(false);
                    }

                    if blocks_to_disconnect.len() as u64 > MAX_REORG_DEPTH {
                        warn!(
                            "Rejected deep reorganization: {} blocks (max: {})",
                            blocks_to_disconnect.len(),
                            MAX_REORG_DEPTH
                        );
                        self.rejected_reorgs += 1;
                        return Ok(false);
                    }

                    let fork_choice_reason = ForkChoiceReason::HigherChainWork;
                    self.handle_chain_reorganization(
                        &block,
                        fork_point,
                        blocks_to_apply,
                        blocks_to_disconnect,
                        fork_choice_reason,
                    )
                    .await?;
                    return Ok(true);
                }
                Ordering::Equal => {
                    // Equal chain work - use secondary metrics to decide
                    // Look at the fork info to see which fork we saw first
                    let current_fork = self.active_forks.get(&self.best_block_hash).cloned();
                    let new_fork = self.active_forks.get(&block_hash).cloned();

                    if let (Some(current), Some(new)) = (current_fork, new_fork) {
                        // Prefer the fork we saw first
                        if new.first_seen < current.first_seen {
                            let (fork_point, blocks_to_apply, blocks_to_disconnect) =
                                self.find_fork_point(&block)?;

                            // Check if reorganization is allowed below checkpoint height
                            if let Err(e) = can_reorganize_below(fork_point.height()) {
                                warn!(
                                    "Rejected reorganization below checkpoint: {}",
                                    e
                                );
                                self.rejected_reorgs += 1;
                                return Ok(false);
                            }

                            if blocks_to_disconnect.len() as u64 > MAX_REORG_DEPTH {
                                warn!(
                                    "Rejected deep reorganization with equal work: {} blocks",
                                    blocks_to_disconnect.len()
                                );
                                self.rejected_reorgs += 1;
                                return Ok(false);
                            }

                            let fork_choice_reason = ForkChoiceReason::FirstSeen;
                            self.handle_chain_reorganization(
                                &block,
                                fork_point,
                                blocks_to_apply,
                                blocks_to_disconnect,
                                fork_choice_reason,
                            )
                            .await?;
                            return Ok(true);
                        }
                    }

                    // Add to our fork set, but don't switch
                    self.fork_points.insert(*prev_hash);
                }
                Ordering::Less => {
                    // Current chain has more work, just track this as a fork
                    self.fork_points.insert(*prev_hash);
                }
            }
        } else {
            // Direct extension of current chain
            let block_difficulty = calculate_block_work(extract_target_from_block(&block)) as u64;

            // Store block to database with bloom filter and cache updates
            tracing::debug!("Storing block {} at height {}", hex::encode(&block_hash[..8]), block.height());
            let block_data = bincode::serialize(&block)
                .map_err(|e| StorageError::DatabaseError(format!("Block serialization failed: {}", e)))?;
            self.db.store_block(&block_hash, &block_data)?;
            self.db.flush()?;
            
            self.chain_work.insert(block_hash, new_chain_work);

            // Update chain state
            self.current_height = block.height();
            self.best_block_hash = block_hash;
            
            // Store updated height and best hash as big-endian
            self.db.set_metadata(b"height", &self.current_height.to_be_bytes())?;
            self.db.set_metadata(b"best_hash", &block_hash)?;
            self.db.flush()?;
            
            // Process transactions to update UTXO set
            self.process_block_transactions(&block)?;
            
            tracing::info!("Block added to chain: height={}, hash={}", self.current_height, hex::encode(&block_hash[..8]));

            // Update total difficulty
            self.update_total_difficulty(block_difficulty)?;

            // Update fork info for direct extension
            self.update_fork_info(&block)?;

            return Ok(true);
        }

        // Store the block in our database, but don't update best chain
        let block_difficulty = calculate_block_work(extract_target_from_block(&block)) as u64;
        self.store_block(block)?;
        self.chain_work.insert(block_hash, new_chain_work);

        Ok(false)
    }

    async fn validate_block(&self, block: &Block) -> Result<bool, StorageError> {
        let block_hash = block.hash();
        
        // Check if block is already marked as invalid
        if self.invalid_block_tracker.is_permanently_invalid(&block_hash) {
            tracing::warn!("Block {} is permanently invalid, rejecting", hex::encode(&block_hash[..8]));
            return Ok(false);
        }

        // Check if parent is invalid
        let parent_hash = block.prev_block_hash();
        if self.invalid_block_tracker.is_permanently_invalid(parent_hash) {
            tracing::warn!("Parent block {} is invalid, marking block {} as invalid", 
                hex::encode(&parent_hash[..8]), hex::encode(&block_hash[..8]));
            self.invalid_block_tracker.mark_invalid(
                block_hash,
                InvalidationReason::ParentInvalid,
                Some(*parent_hash),
                Some(block.height()),
            ).map_err(|e| StorageError::DatabaseError(format!("Failed to mark block invalid: {}", e)))?;
            return Ok(false);
        }

        tracing::debug!(
            "Validating block: height={}, prev_hash={}",
            block.height(),
            hex::encode(block.prev_block_hash())
        );
        
        if !block.validate() {
            tracing::warn!("Block failed basic validation: height={}", block.height());
            self.invalid_block_tracker.mark_invalid(
                block_hash,
                InvalidationReason::InvalidStructure("Basic validation failed".to_string()),
                Some(*block.prev_block_hash()),
                Some(block.height()),
            ).map_err(|e| StorageError::DatabaseError(format!("Failed to mark block invalid: {}", e)))?;
            return Ok(false);
        }

        // Validate against checkpoints
        if let Err(e) = validate_checkpoint(block) {
            tracing::warn!("Checkpoint validation failed: {}", e);
            self.invalid_block_tracker.mark_invalid(
                block_hash,
                InvalidationReason::CheckpointViolation,
                Some(*block.prev_block_hash()),
                Some(block.height()),
            ).map_err(|e| StorageError::DatabaseError(format!("Failed to mark block invalid: {}", e)))?;
            return Ok(false);
        }

        if block.height() != self.current_height + 1
            && *block.prev_block_hash() != self.best_block_hash
        {
            let fork_distance = self.calculate_fork_distance(block)?;
            tracing::debug!("Fork block at height {}, distance={}", block.height(), fork_distance);
            
            if fork_distance > MAX_FORK_DISTANCE {
                tracing::warn!("Fork distance {} exceeds maximum {}", fork_distance, MAX_FORK_DISTANCE);
                self.invalid_block_tracker.mark_invalid(
                    block_hash,
                    InvalidationReason::ForkTooDeep,
                    Some(*block.prev_block_hash()),
                    Some(block.height()),
                ).map_err(|e| StorageError::DatabaseError(format!("Failed to mark block invalid: {}", e)))?;
                return Ok(false);
            }
        }

        for (i, tx) in block.transactions().iter().enumerate() {
            if !self.validate_transaction(tx).await? {
                tracing::warn!("Transaction {} failed validation in block {}", i, hex::encode(&block.hash()[..8]));
                self.invalid_block_tracker.mark_invalid(
                    block_hash,
                    InvalidationReason::TransactionValidation(format!("Transaction {} invalid", i)),
                    Some(*block.prev_block_hash()),
                    Some(block.height()),
                ).map_err(|e| StorageError::DatabaseError(format!("Failed to mark block invalid: {}", e)))?;
                return Ok(false);
            }
        }

        Ok(true)
    }

    /// Process transactions in a block to update UTXO set
    fn process_block_transactions(&mut self, block: &Block) -> Result<(), StorageError> {
        for tx in block.transactions() {
            let tx_hash = tx.hash();
            
            // Remove spent UTXOs (skip for coinbase as it has no real inputs)
            if !tx.is_coinbase() {
                for input in tx.inputs() {
                    let prev_tx = input.prev_tx_hash();
                    let prev_vout = input.prev_output_index();
                    
                    // Remove the spent UTXO
                    if let Err(e) = self.db.remove_utxo(&prev_tx, prev_vout) {
                        tracing::warn!("Failed to remove spent UTXO: {}", e);
                    }
                }
            }
            
            // Add new UTXOs from outputs
            for (vout, output) in tx.outputs().iter().enumerate() {
                let output_data = bincode::serialize(output)
                    .map_err(|e| StorageError::DatabaseError(format!("Output serialization failed: {}", e)))?;
                
                self.db.store_utxo(&tx_hash, vout as u32, &output_data)?;
            }
        }
        
        tracing::debug!("Processed {} transactions, updated UTXO set", block.transactions().len());
        Ok(())
    }

    async fn validate_transaction(&self, tx: &Transaction) -> Result<bool, StorageError> {
        // Skip UTXO validation for coinbase transactions
        if tx.is_coinbase() {
            return Ok(true);
        }
        
        let mut spent_outputs = HashSet::new();
        for (idx, input) in tx.inputs().iter().enumerate() {
            let outpoint = (input.prev_tx_hash(), input.prev_output_index());
            if !spent_outputs.insert(outpoint) {
                tracing::warn!("Double-spend detected in transaction: input {}", idx);
                return Ok(false);
            }

            if self
                .db
                .get_utxo(&input.prev_tx_hash(), input.prev_output_index())?
                .is_none()
            {
                tracing::warn!(
                    "UTXO not found for input {}: txid={}, vout={}",
                    idx,
                    hex::encode(input.prev_tx_hash()),
                    input.prev_output_index()
                );
                return Ok(false);
            }
        }

        Ok(true)
    }

    /// Retrieve a specific output from a block by transaction hash and output index
    /// Used during chain reorganization to restore spent UTXOs
    fn get_output_from_disconnected_block(
        &self,
        tx_hash: &[u8; 32],
        vout: u32,
    ) -> Result<TransactionOutput, StorageError> {
        let tip_height = self.get_height();
        
        // Search last 1000 blocks (should cover any reasonable reorg depth)
        for height in (tip_height.saturating_sub(1000)..=tip_height).rev() {
            // Get block hash for this height
            if let Ok(Some(block_hash)) = self.db.get_block_hash_by_height(height) {
                // Get the full block
                if let Some(block) = self.get_block(&block_hash) {
                    for tx in block.transactions() {
                        if tx.hash() == *tx_hash {
                            return tx.outputs()
                                .get(vout as usize)
                                .cloned()
                                .ok_or_else(|| {
                                    StorageError::DatabaseError(format!(
                                        "Output index {} not found in transaction {}",
                                        vout,
                                        hex::encode(tx_hash)
                                    ))
                                });
                        }
                    }
                }
            }
        }
        
        Err(StorageError::DatabaseError(format!(
            "Transaction {} not found in recent blocks (searched last 1000 blocks)",
            hex::encode(tx_hash)
        )))
    }

    /// Reverse transactions from a disconnected block during chain reorganization
    /// This restores the UTXO set to the state before the block was added
    fn reverse_block_transactions(&mut self, block: &Block) -> Result<(), StorageError> {
        tracing::info!(
            "Reversing transactions from disconnected block at height {}",
            block.height()
        );

        // Process transactions in reverse order to properly unwind state
        for tx in block.transactions().iter().rev() {
            let tx_hash = tx.hash();

            if !tx.is_coinbase() {
                // Restore spent UTXOs from inputs
                for input in tx.inputs() {
                    let prev_tx = input.prev_tx_hash();
                    let prev_vout = input.prev_output_index();

                    // Retrieve the original output that was spent
                    let prev_output = self.get_output_from_disconnected_block(&prev_tx, prev_vout)?;

                    // Restore to UTXO set
                    let output_data = bincode::serialize(&prev_output).map_err(|e| {
                        StorageError::DatabaseError(format!(
                            "Failed to serialize output for restoration: {}",
                            e
                        ))
                    })?;

                    self.db.store_utxo(&prev_tx, prev_vout, &output_data)?;

                    tracing::debug!(
                        "Restored UTXO: {}:{} (amount: {} satoshis)",
                        hex::encode(&prev_tx[..8]),
                        prev_vout,
                        prev_output.value()
                    );
                }
            }

            // Remove created UTXOs from this transaction's outputs
            for (vout, output) in tx.outputs().iter().enumerate() {
                self.db.remove_utxo(&tx_hash, vout as u32)?;
                tracing::debug!(
                    "Removed UTXO: {}:{} (amount: {} satoshis)",
                    hex::encode(&tx_hash[..8]),
                    vout,
                    output.value()
                );
            }
        }

        tracing::info!(
            "Successfully reversed {} transactions from block at height {}",
            block.transactions().len(),
            block.height()
        );
        Ok(())
    }

    fn find_fork_point(
        &self,
        new_tip: &Block,
    ) -> Result<(Block, Vec<Block>, Vec<Block>), StorageError> {
        let mut blocks_to_apply = Vec::new();
        let mut blocks_to_disconnect = Vec::new();
        let mut current = new_tip.clone();
        let mut main_chain = self.get_block_at_height(self.current_height)?;

        while current.height() > 0 && main_chain.height() > 0 {
            if current.hash() == main_chain.hash() {
                return Ok((current, blocks_to_apply, blocks_to_disconnect));
            }

            if current.height() > main_chain.height() {
                blocks_to_apply.push(current.clone());
                let prev_hash = *current.prev_block_hash();
                current = self
                    .db
                    .get_block(&prev_hash)?
                    .ok_or(StorageError::DatabaseError("Block not found".to_string()))?;
            } else {
                blocks_to_disconnect.push(main_chain.clone());
                let prev_hash = *main_chain.prev_block_hash();
                main_chain = self
                    .db
                    .get_block(&prev_hash)?
                    .ok_or(StorageError::DatabaseError("Block not found".to_string()))?;
            }
        }

        Err(StorageError::InvalidChainReorganization)
    }

    async fn handle_chain_reorganization(
        &mut self,
        new_tip: &Block,
        fork_point: Block,
        blocks_to_apply: Vec<Block>,
        blocks_to_disconnect: Vec<Block>,
        fork_choice_reason: ForkChoiceReason,
    ) -> Result<(), StorageError> {
        let old_tip = self.best_block_hash;
        let time_since_last_reorg = SystemTime::now()
            .duration_since(self.last_reorg_time)
            .map_err(|e| StorageError::DatabaseError(e.to_string()))?;

        let _reorg_event = ReorganizationEvent {
            old_tip,
            new_tip: new_tip.hash(),
            fork_point: fork_point.hash(),
            fork_height: fork_point.height(),
            blocks_disconnected: blocks_to_disconnect.len() as u64,
            blocks_connected: blocks_to_apply.len() as u64,
            timestamp: SystemTime::now(),
            time_since_last_reorg,
            fork_choice_reason: fork_choice_reason.clone(),
        };

        // Log the reorganization event with detail appropriate to its size
        if blocks_to_disconnect.len() > 1 || blocks_to_apply.len() > 1 {
            info!("Chain reorganization: {} blocks disconnected, {} blocks connected at height {}. Reason: {:?}",
                blocks_to_disconnect.len(),
                blocks_to_apply.len(),
                fork_point.height(),
                fork_choice_reason);
        } else {
            debug!("Minor chain reorganization: {} blocks disconnected, {} blocks connected at height {}",
                blocks_to_disconnect.len(),
                blocks_to_apply.len(),
                fork_point.height());
        }

        self.db.begin_transaction()?;

        // Disconnect blocks from the current main chain
        for block in blocks_to_disconnect.iter().rev() {
            if let Err(e) = self.disconnect_block(block) {
                error!("Error disconnecting block during reorganization: {:?}", e);
                self.db.rollback_transaction()?;
                return Err(e);
            }
        }

        // Connect blocks from the new chain
        let mut total_difficulty_adjustment: u64 = 0;
        for block in blocks_to_apply.iter() {
            let block_difficulty = calculate_block_work(extract_target_from_block(block)) as u64;
            total_difficulty_adjustment += block_difficulty;

            if let Err(e) = self.connect_block(block) {
                error!("Error connecting block during reorganization: {:?}", e);
                // Critical error - rollback the transaction
                self.db.rollback_transaction()?;
                return Err(e);
            }
        }

        // Update chain state
        self.best_block_hash = new_tip.hash();
        self.current_height = new_tip.height();
        self.last_reorg_time = SystemTime::now();
        self.reorg_count += 1;

        // Update total difficulty for the reorg
        self.update_total_difficulty(total_difficulty_adjustment)?;

        // Commit the transaction
        if let Err(e) = self.db.commit_transaction() {
            error!("Failed to commit reorganization transaction: {:?}", e);
            return Err(e);
        }

        // Update fork points
        self.prune_fork_points()?;

        // Update fork info to reflect current state
        let new_tip_hash = new_tip.hash();
        let updates: Vec<([u8; 32], bool)> = self
            .active_forks
            .iter()
            .map(|(hash, fork)| {
                let is_active = *hash == new_tip_hash
                    || self
                        .is_ancestor_of(&fork.tip_hash, &new_tip_hash)
                        .unwrap_or(false);
                (*hash, is_active)
            })
            .collect();

        // Apply the updates
        for (hash, is_active) in updates {
            if let Some(fork) = self.active_forks.get_mut(&hash) {
                fork.is_active = is_active;
            }
        }

        // Log successful reorganization
        info!(
            "Chain reorganization complete: Activated fork with tip {} at height {}",
            hex::encode(&new_tip.hash()[..4]),
            new_tip.height()
        );

        // Clean up orphaned blocks (blocks whose parents are invalid)
        let chain_blocks: HashSet<[u8; 32]> = blocks_to_disconnect
            .iter()
            .map(|b| b.hash())
            .collect();
        if let Ok(orphaned) = self.invalid_block_tracker.cleanup_orphans(&chain_blocks) {
            if !orphaned.is_empty() {
                info!("Cleaned up {} orphaned blocks during reorganization", orphaned.len());
            }
        }

        Ok(())
    }

    fn disconnect_block(&mut self, block: &Block) -> Result<(), StorageError> {
        // Use reverse_block_transactions for proper UTXO unwinding
        self.reverse_block_transactions(block)?;

        // Adjust total difficulty when disconnecting a block
        let block_difficulty = calculate_block_work(extract_target_from_block(block)) as u64;
        let current_difficulty = self.get_total_difficulty();
        let new_total = current_difficulty.saturating_sub(block_difficulty);

        let difficulty_bytes = bincode::serialize(&new_total)?;
        self.db
            .store_metadata(b"total_difficulty", &difficulty_bytes)?;

        self.current_height -= 1;
        self.best_block_hash = *block.prev_block_hash();

        self.db
            .store_metadata(b"height", &bincode::serialize(&self.current_height)?)?;
        self.db
            .store_metadata(b"best_hash", &self.best_block_hash)?;

        Ok(())
    }

    fn connect_block(&mut self, block: &Block) -> Result<(), StorageError> {
        // Calculate total difficulty and block work
        let block_difficulty = calculate_block_work(extract_target_from_block(block)) as u64;

        // Update chain work
        let block_hash = block.hash();
        self.chain_work.insert(block_hash, block_difficulty as u128);

        // Update total difficulty
        self.update_total_difficulty(block_difficulty)?;

        // Update best block hash and height
        self.best_block_hash = block_hash;
        self.current_height += 1;

        Ok(())
    }

    fn store_block(&mut self, block: Block) -> Result<(), StorageError> {
        // Store the block in the database
        let block_hash = block.hash();
        
        // Serialize and store with bloom filter/cache updates
        let block_data = bincode::serialize(&block)
            .map_err(|e| StorageError::DatabaseError(format!("Block serialization failed: {}", e)))?;
        self.db.store_block(&block_hash, &block_data)?;
        self.db.flush()?;

        // Calculate block difficulty
        let block_difficulty = calculate_block_work(extract_target_from_block(&block)) as u64;

        // Update chain work
        self.chain_work.insert(block_hash, block_difficulty as u128);

        // Update total difficulty
        self.update_total_difficulty(block_difficulty)?;

        // Update best block hash and height if higher than current
        if self.current_height < block.height() {
            self.best_block_hash = block_hash;
            self.current_height = block.height();
            // Store height as big-endian bytes (to match get_height() which reads as big-endian)
            self.db.set_metadata(b"height", &self.current_height.to_be_bytes())?;
            self.db.set_metadata(b"best_hash", &block_hash)?;
            self.db.flush()?; // Flush metadata too
        }

        Ok(())
    }

    fn calculate_chain_work(&self, block: &Block) -> Result<u128, StorageError> {
        let mut total_work = 0_u128;
        let mut current = block.clone();

        tracing::debug!("Calculating chain work from height {}", current.height());

        while current.height() > 0 {
            total_work += calculate_block_work(extract_target_from_block(&current));

            // Get previous block
            let prev_hash = current.prev_block_hash();
            
            if let Ok(Some(prev_block)) = self.db.get_block(prev_hash) {
                current = prev_block;
            } else {
                tracing::error!(
                    "Chain work calculation failed: block at height {} not found (hash: {})",
                    current.height() - 1,
                    hex::encode(prev_hash)
                );
                return Err(StorageError::DatabaseError(format!(
                    "Previous block not found at height {}",
                    current.height() - 1
                )));
            }
        }

        tracing::debug!("Chain work calculated: total={}", total_work);
        Ok(total_work)
    }

    fn calculate_fork_distance(&self, block: &Block) -> Result<u64, StorageError> {
        let mut current = block.clone();
        let mut distance = 0;

        while current.height() > 0 {
            if self.db.get_block(current.prev_block_hash())?.is_some() {
                return Ok(distance);
            }
            distance += 1;
            if let Ok(Some(prev_block)) = self.db.get_block(current.prev_block_hash()) {
                current = prev_block;
            } else {
                break;
            }
        }

        Ok(distance)
    }

    fn prune_fork_points(&mut self) -> Result<(), StorageError> {
        // Create a temporary set of hashes to avoid the borrow checker issue
        let mut hashes_to_keep = HashSet::new();

        // First collect the hashes that should be kept
        for hash in &self.fork_points {
            if let Ok(Some(block)) = self.db.get_block(hash) {
                let age = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .map_err(|e| StorageError::DatabaseError(e.to_string()))?
                    .as_secs()
                    - self.header_timestamp(&block);

                if age < 86400 {
                    hashes_to_keep.insert(*hash);
                }
            }
        }

        // Now replace the fork_points with the filtered set
        self.fork_points = hashes_to_keep;

        Ok(())
    }

    // Helper method to get timestamp from block header
    fn header_timestamp(&self, _block: &Block) -> u64 {
        // In a real implementation, this would access the timestamp directly
        // Here we're using a default value of current time - 1 hour
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
            - 3600
    }

    pub fn get_block_at_height(&self, height: u64) -> Result<Block, StorageError> {
        let mut current_hash = self.best_block_hash;
        let mut current_height = self.current_height;

        while current_height > height {
            let block = self
                .db
                .get_block(&current_hash)?
                .ok_or(StorageError::DatabaseError("Block not found".to_string()))?;
            current_hash = *block.prev_block_hash();
            current_height -= 1;
        }

        self.db
            .get_block(&current_hash)?
            .ok_or_else(|| StorageError::DatabaseError("Block not found".to_string()))
    }

    /// Get a reference to the underlying database
    pub fn get_db(&self) -> &Arc<BlockchainDB> {
        &self.db
    }

    /// Update information about active forks when a new block arrives
    fn update_fork_info(&mut self, block: &Block) -> Result<(), StorageError> {
        let block_hash = block.hash();
        let prev_hash = block.prev_block_hash();
        let now = SystemTime::now();

        // Check if this is an extension of an existing fork
        let prev_hash_clone = *prev_hash;

        // First check if we need to update an existing fork
        let fork_update = if let Some(fork) = self.active_forks.get(&prev_hash_clone) {
            // Create updated fork info
            Some(ForkInfo {
                fork_point_hash: fork.fork_point_hash,
                fork_point_height: fork.fork_point_height,
                tip_hash: block_hash,
                tip_height: block.height(),
                blocks_added: fork.blocks_added + 1,
                first_seen: fork.first_seen,
                last_updated: now,
                chain_work: self.calculate_chain_work(block)?,
                is_active: fork.is_active,
            })
        } else {
            None
        };

        // Apply the update if needed
        if let Some(updated_fork) = fork_update {
            // Update existing fork
            self.active_forks
                .insert(prev_hash_clone, updated_fork.clone());
            // Also store under the new block hash
            self.active_forks.insert(block_hash, updated_fork);

            debug!(
                "Extended fork to height {} with tip {}",
                block.height(),
                hex::encode(&block_hash[..4])
            );
        } else if *prev_hash == self.best_block_hash {
            // This is a direct extension of the main chain
            let fork_info = ForkInfo {
                fork_point_hash: *prev_hash,
                fork_point_height: self.current_height,
                tip_hash: block_hash,
                tip_height: block.height(),
                chain_work: self.calculate_chain_work(block)?,
                blocks_added: 1,
                first_seen: now,
                last_updated: now,
                is_active: true,
            };

            self.active_forks.insert(block_hash, fork_info);
        } else {
            // This is a new fork or extension of an unknown fork
            // Try to find fork point with main chain
            let mut current = block.clone();
            let mut fork_point_hash = [0u8; 32];
            let mut fork_point_height = 0;
            let mut blocks_on_fork = 1;

            // Work backwards until we find a common block with our main chain
            while current.height() > 0 {
                let prev_hash = current.prev_block_hash();

                if let Ok(Some(_)) = self.db.get_block(prev_hash) {
                    // Found a block we know about
                    fork_point_hash = *prev_hash;
                    if let Ok(Some(prev_block)) = self.db.get_block(prev_hash) {
                        fork_point_height = prev_block.height();
                    }
                    break;
                }

                blocks_on_fork += 1;

                if let Ok(Some(prev_block)) = self.db.get_block(prev_hash) {
                    current = prev_block;
                } else {
                    // We don't have the previous block, so we can't determine the fork point
                    break;
                }
            }

            let fork_info = ForkInfo {
                fork_point_hash,
                fork_point_height,
                tip_hash: block_hash,
                tip_height: block.height(),
                chain_work: self.calculate_chain_work(block)?,
                blocks_added: blocks_on_fork,
                first_seen: now,
                last_updated: now,
                is_active: true,
            };

            self.active_forks.insert(block_hash, fork_info);

            info!(
                "Detected new fork at height {} with {} blocks since fork point",
                block.height(),
                blocks_on_fork
            );
        }

        // Clean up old forks
        self.prune_inactive_forks();

        Ok(())
    }

    /// Remove forks that haven't been updated recently
    fn prune_inactive_forks(&mut self) {
        let now = SystemTime::now();
        let max_age = Duration::from_secs(86400); // 24 hours

        self.active_forks.retain(|_, fork| {
            if let Ok(age) = now.duration_since(fork.last_updated) {
                // Keep forks that have been updated recently or are still close to the main chain
                let height_difference = self.current_height.saturating_sub(fork.tip_height);
                age < max_age || height_difference < MAX_FORK_DISTANCE
            } else {
                true // Keep on duration error
            }
        });
    }

    /// Check if one block is an ancestor of another
    fn is_ancestor_of(
        &self,
        potential_ancestor_hash: &[u8; 32],
        descendant_hash: &[u8; 32],
    ) -> Result<bool, StorageError> {
        if potential_ancestor_hash == descendant_hash {
            return Ok(true);
        }

        let mut current_hash = *descendant_hash;

        // Walk back the chain until we find the ancestor or reach genesis
        loop {
            let current_block = match self.db.get_block(&current_hash)? {
                Some(block) => block,
                None => return Ok(false),
            };

            // Check if we've reached height 0 (genesis)
            if current_block.height() == 0 {
                return Ok(false);
            }

            // Get the previous hash
            let prev_hash = current_block.prev_block_hash();

            // Check if we found our ancestor
            if prev_hash == potential_ancestor_hash {
                return Ok(true);
            }

            // Move to previous block
            current_hash = *prev_hash;
        }
    }

    /// Calculate metrics about the longest chain and active forks
    pub fn calculate_fork_metrics(&self) -> HashMap<String, u64> {
        let mut metrics = HashMap::new();

        metrics.insert("main_chain_height".to_string(), self.current_height);
        metrics.insert("active_forks".to_string(), self.active_forks.len() as u64);
        metrics.insert("fork_points".to_string(), self.fork_points.len() as u64);
        metrics.insert("reorg_count".to_string(), self.reorg_count);
        metrics.insert("rejected_reorgs".to_string(), self.rejected_reorgs);

        // Calculate maximum fork length
        let mut max_fork_length = 0;
        for fork in self.active_forks.values() {
            let fork_length = fork.tip_height.saturating_sub(fork.fork_point_height);
            if fork_length > max_fork_length {
                max_fork_length = fork_length;
            }
        }
        metrics.insert("max_fork_length".to_string(), max_fork_length);

        // Time since last block
        if let Ok(duration) = SystemTime::now().duration_since(self.last_block_time) {
            metrics.insert("seconds_since_last_block".to_string(), duration.as_secs());
        }

        metrics
    }

    /// Get all transactions for a given block hash
    pub fn get_transactions_for_block(&self, hash: &[u8; 32]) -> Option<Vec<Transaction>> {
        self.get_block(hash).map(|b| b.transactions().to_vec())
    }
}

// Function to extract target from a block's bits field
fn extract_target_from_block(block: &Block) -> u32 {
    // BlockHeader doesn't expose target directly, so we need to extract it
    // For our implementation, we'll use the hash of the block as a proxy for difficulty
    let hash = block.hash();
    let first_bytes = &hash[0..4];

    // Create a u32 from the first 4 bytes of the hash
    let mut target = 0u32;
    for (i, &byte) in first_bytes.iter().enumerate() {
        target |= (byte as u32) << (8 * i);
    }

    target
}

// Calculate work for a block based on its target (difficulty)
fn calculate_block_work(target: u32) -> u128 {
    // Use a more reasonable approach that doesn't overflow
    // The actual Bitcoin formula is 2^256 / (target+1), but we'll use a simplified version
    // that doesn't overflow u128

    // First ensure target is not 0 to avoid division by zero
    let safe_target = target.max(1) as u128;

    // Use a large but safe max_target value that won't overflow
    // 2^128 - 1 is the maximum value for u128
    let max_target = u128::MAX / 1000; // Use a fraction of max to avoid overflow

    // Calculate difficulty - with safeguards against overflow
    if safe_target <= 1 {
        return max_target; // Avoid division by extremely small numbers
    }

    // Calculate work as max_target / target
    max_target / safe_target
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_chain_reorganization() -> Result<(), StorageError> {
        let temp_dir = tempdir().unwrap();
        let db = Arc::new(BlockchainDB::new(temp_dir.path())?);
        let mut chain_state = ChainState::new(db)?;

        // Create a genesis block with a known hash
        let genesis = Block::new_with_params(1, [0u8; 32], Vec::new(), u32::MAX);
        chain_state.store_block(genesis.clone())?;

        // Update initial chain state with the genesis block
        chain_state.current_height = 1;
        chain_state.best_block_hash = genesis.hash();

        // First fork with higher difficulty (lower target = higher difficulty)
        let fork_block = Block::new_with_params(1, genesis.hash(), Vec::new(), u32::MAX / 2);

        // Process the fork block and check that it becomes the new best block
        let reorg_successful = chain_state.process_block(fork_block.clone()).await?;

        // Verify reorg was successful
        assert!(reorg_successful);
        assert_eq!(chain_state.get_best_block_hash(), fork_block.hash());

        // Create a fork too deep to be accepted
        let mut deep_fork = fork_block.clone();
        for _ in 0..MAX_REORG_DEPTH + 1 {
            let prev_hash = deep_fork.hash();
            deep_fork = Block::new_with_params(
                (deep_fork.height() + 1) as u32,
                prev_hash,
                Vec::new(),
                u32::MAX / 2,
            );
        }

        // This new fork should be too deep to be accepted
        let reorg_failed = !chain_state.process_block(deep_fork).await?;
        assert!(reorg_failed);

        Ok(())
    }

    #[tokio::test]
    async fn test_fork_validation() -> Result<(), StorageError> {
        let temp_dir = tempdir().unwrap();
        let db = Arc::new(BlockchainDB::new(temp_dir.path())?);
        let mut chain_state = ChainState::new(db)?;

        let genesis = Block::new_with_params(1, [0u8; 32], Vec::new(), u32::MAX);
        chain_state.store_block(genesis.clone())?;

        let valid_fork = Block::new_with_params(2, genesis.hash(), Vec::new(), u32::MAX / 2);
        assert!(chain_state.validate_block(&valid_fork).await?);

        let mut invalid_fork = genesis.clone();
        for _ in 0..MAX_FORK_DISTANCE + 1 {
            invalid_fork = Block::new_with_params(
                (invalid_fork.height() + 1) as u32,
                invalid_fork.hash(),
                Vec::new(),
                u32::MAX / 2,
            );
        }
        assert!(!chain_state.validate_block(&invalid_fork).await?);

        Ok(())
    }

    #[tokio::test]
    async fn test_total_difficulty() -> Result<(), StorageError> {
        let temp_dir = tempdir().unwrap();
        let db = Arc::new(BlockchainDB::new(temp_dir.path())?);
        let mut chain_state = ChainState::new(db)?;

        assert_eq!(chain_state.get_total_difficulty(), 0);

        // Create and add a block
        let genesis = Block::new_with_params(1, [0u8; 32], Vec::new(), u32::MAX);
        chain_state.process_block(genesis.clone()).await?;

        // Total difficulty should be increased
        assert!(chain_state.get_total_difficulty() > 0);

        Ok(())
    }
}
