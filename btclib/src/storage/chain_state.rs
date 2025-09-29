use log::error;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex, RwLock};
use thiserror::Error;

use crate::consensus::fork_resolution_v2::ProofOfWorkForkResolver;
use crate::consensus::secure_fork_resolution::SecureForkConfig;
use crate::storage::utxo_set::{UtxoCommitment, UtxoEntry, UtxoSet};
use crate::types::block::{Block, BlockHeader};
use crate::types::transaction::OutPoint;
use std::cmp::Ordering;

/// Errors that can occur in chain state operations
#[derive(Debug, Error)]
pub enum ChainStateError {
    #[error("Block already exists in chain state: {0}")]
    BlockAlreadyExists(String),

    #[error("Block not found in chain state: {0}")]
    BlockNotFound(String),

    #[error("Invalid block: {0}")]
    InvalidBlock(String),

    #[error("Chain reorganization failed: {0}")]
    ReorganizationFailed(String),

    #[error("Storage error: {0}")]
    StorageError(String),

    #[error("UTXO set error: {0}")]
    UtxoError(String),

    #[error("Genesis block mismatch")]
    GenesisBlockMismatch,

    #[error("Invalid checkpoint")]
    InvalidCheckpoint,

    #[error("Internal error: {0}")]
    InternalError(String),
}

/// Result type for chain state operations
pub type ChainStateResult<T> = Result<T, ChainStateError>;

/// Fork resolution policy for handling competing chains
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ForkResolutionPolicy {
    /// Follow the chain with the most accumulated work (default)
    MostWork,

    /// Follow the chain with the most blocks
    MostBlocks,

    /// Follow the chain with the first-seen blocks
    FirstSeen,
}

/// Configuration for chain state management
#[derive(Debug, Clone)]
pub struct ChainStateConfig {
    /// Maximum number of blocks to keep in memory
    pub max_memory_blocks: usize,

    /// Fork resolution policy
    pub fork_resolution_policy: ForkResolutionPolicy,

    /// Block height interval for checkpoints
    pub checkpoint_interval: u32,

    /// Maximum fork length to track
    pub max_fork_length: u32,

    /// Maximum block headers to keep in memory
    pub max_headers: usize,
}

impl Default for ChainStateConfig {
    fn default() -> Self {
        Self {
            max_memory_blocks: 1000,
            fork_resolution_policy: ForkResolutionPolicy::MostWork,
            checkpoint_interval: 10_000,
            max_fork_length: 100,
            max_headers: 100_000,
        }
    }
}

/// A checkpoint for verifying chain integrity
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Checkpoint {
    /// Block height of this checkpoint
    pub height: u32,

    /// Block hash at this height
    pub hash: [u8; 32],

    /// UTXO commitment at this point
    pub utxo_commitment: UtxoCommitment,
}

/// Manages blockchain state including the active chain and forks
pub struct ChainState {
    /// Configuration
    config: ChainStateConfig,

    /// Known block headers by hash
    headers: Arc<RwLock<HashMap<[u8; 32], BlockHeader>>>,

    /// Block height to hash mapping (for each height, ordered from active chain to forks)
    height_map: Arc<RwLock<HashMap<u32, Vec<[u8; 32]>>>>,

    /// Current blockchain height
    current_height: Arc<RwLock<u32>>,

    /// Current tip hash
    current_tip: Arc<RwLock<[u8; 32]>>,

    /// Known forks (fork tip hash => common ancestor height)
    forks: Arc<RwLock<HashMap<[u8; 32], u32>>>,

    /// Hash of blocks we've already processed
    processed_blocks: Arc<RwLock<HashSet<[u8; 32]>>>,

    /// Checkpoints for integrity verification
    checkpoints: Arc<RwLock<HashMap<u32, Checkpoint>>>,

    /// UTXO set reference
    utxo_set: Arc<UtxoSet>,

    /// Proof of work fork resolver
    fork_resolver: Arc<Mutex<ProofOfWorkForkResolver>>,
}

impl ChainState {
    /// Create a new chain state
    pub fn new(config: ChainStateConfig, utxo_set: Arc<UtxoSet>) -> Self {
        let fork_config = SecureForkConfig::default();

        Self {
            config,
            headers: Arc::new(RwLock::new(HashMap::new())),
            height_map: Arc::new(RwLock::new(HashMap::new())),
            current_height: Arc::new(RwLock::new(0)),
            current_tip: Arc::new(RwLock::new([0; 32])),
            forks: Arc::new(RwLock::new(HashMap::new())),
            processed_blocks: Arc::new(RwLock::new(HashSet::new())),
            checkpoints: Arc::new(RwLock::new(HashMap::new())),
            utxo_set,
            fork_resolver: Arc::new(Mutex::new(ProofOfWorkForkResolver::new(
                fork_config.max_fork_depth,
            ))),
        }
    }

    /// Initialize the chain state with a genesis block
    pub fn initialize_with_genesis(&self, genesis: Block) -> ChainStateResult<()> {
        let genesis_hash = genesis.hash();

        // Ensure we're starting fresh
        {
            let height = self
                .current_height
                .read()
                .map_err(|e| ChainStateError::StorageError(e.to_string()))?;
            if *height > 0 {
                return Err(ChainStateError::StorageError(
                    "Chain state already initialized".to_string(),
                ));
            }
        }

        // Add genesis header
        {
            let mut headers = self
                .headers
                .write()
                .map_err(|e| ChainStateError::StorageError(e.to_string()))?;
            headers.insert(genesis_hash, genesis.header().clone());
        }

        // Update height map
        {
            let mut height_map = self
                .height_map
                .write()
                .map_err(|e| ChainStateError::StorageError(e.to_string()))?;
            height_map.insert(0, vec![genesis_hash]);
        }

        // Set as current tip
        {
            let mut current_tip = self
                .current_tip
                .write()
                .map_err(|e| ChainStateError::StorageError(e.to_string()))?;
            *current_tip = genesis_hash;
        }

        // Mark as processed
        {
            let mut processed = self
                .processed_blocks
                .write()
                .map_err(|e| ChainStateError::StorageError(e.to_string()))?;
            processed.insert(genesis_hash);
        }

        // Process genesis outputs to UTXO set
        for (index, output) in genesis.transactions()[0].outputs().iter().enumerate() {
            let outpoint = OutPoint {
                txid: genesis.transactions()[0].hash(),
                vout: index as u32,
            };

            let utxo_entry = UtxoEntry {
                outpoint,
                output: output.clone(),
                height: 0,
                is_coinbase: true,
                is_confirmed: true,
            };

            // Add to UTXO set
            self.utxo_set
                .add(utxo_entry)
                .map_err(ChainStateError::UtxoError)?;
        }

        // Create genesis checkpoint
        self.create_checkpoint(0, &genesis_hash)?;

        Ok(())
    }

    /// Get the current blockchain height
    pub fn get_height(&self) -> ChainStateResult<u32> {
        let height = self
            .current_height
            .read()
            .map_err(|e| ChainStateError::StorageError(e.to_string()))?;
        Ok(*height)
    }

    /// Get the current tip hash
    pub fn get_tip(&self) -> ChainStateResult<[u8; 32]> {
        let tip = self
            .current_tip
            .read()
            .map_err(|e| ChainStateError::StorageError(e.to_string()))?;
        Ok(*tip)
    }

    /// Get a block header by hash
    pub fn get_header(&self, hash: &[u8; 32]) -> ChainStateResult<Option<BlockHeader>> {
        let headers = self
            .headers
            .read()
            .map_err(|e| ChainStateError::StorageError(e.to_string()))?;
        Ok(headers.get(hash).cloned())
    }

    /// Get a block header by height (from active chain)
    pub fn get_header_by_height(&self, height: u32) -> ChainStateResult<Option<BlockHeader>> {
        // Get hash at height
        let hash = {
            let height_map = self
                .height_map
                .read()
                .map_err(|e| ChainStateError::StorageError(e.to_string()))?;

            if let Some(hashes) = height_map.get(&height) {
                if hashes.is_empty() {
                    return Ok(None);
                }
                // First hash is on active chain
                hashes[0]
            } else {
                return Ok(None);
            }
        };

        // Get header
        self.get_header(&hash)
    }

    /// Process a new block
    pub fn process_block(&self, block: Block) -> ChainStateResult<bool> {
        let block_hash = block.hash();

        // Check if already processed
        {
            let processed = self
                .processed_blocks
                .read()
                .map_err(|e| ChainStateError::StorageError(e.to_string()))?;
            if processed.contains(&block_hash) {
                return Ok(false); // Already processed, not an error
            }
        }

        // Verify basic block structure (more validation would happen in validation layer)
        if !block.validate() {
            return Err(ChainStateError::InvalidBlock(
                "Block failed basic validation".to_string(),
            ));
        }

        // Get the previous block header
        let prev_header = self
            .get_header(block.prev_block_hash())?
            .ok_or_else(|| ChainStateError::BlockNotFound(hex::encode(block.prev_block_hash())))?;

        // Calculate height of this block
        let block_height = prev_header.height() + 1;

        // Store header in memory
        {
            let mut headers = self
                .headers
                .write()
                .map_err(|e| ChainStateError::StorageError(e.to_string()))?;
            if headers.len() >= self.config.max_headers {
                // Prune old headers if needed
                // Implementation would remove oldest non-checkpoint headers
            }
            headers.insert(block_hash, block.header().clone());
        }

        // Update height map
        {
            let mut height_map = self
                .height_map
                .write()
                .map_err(|e| ChainStateError::StorageError(e.to_string()))?;
            // Convert block_height from u64 to u32 for the height_map
            let height_u32 = block_height.try_into().map_err(|_| {
                ChainStateError::InvalidBlock("Block height exceeds u32 maximum".to_string())
            })?;
            height_map.entry(height_u32).or_default().push(block_hash);
        }

        // Check if this is a better chain according to secure fork resolution
        let current_tip = self.get_tip()?;
        let current_height = self.get_height()?;
        // Convert current_height to u64 for comparison
        let current_height_u64: u64 = current_height.into();

        let should_reorg = if block_height > current_height_u64 {
            // New block is higher, potential reorg
            true
        } else if block_height == current_height_u64 {
            // Same height, use secure fork resolution
            // Create header getter closure
            let headers_ref = self.headers.clone();
            let get_header = move |hash: &[u8; 32]| -> Option<BlockHeader> {
                headers_ref.read().ok()?.get(hash).cloned()
            };

            // Use secure fork resolver
            let resolver = self.fork_resolver.lock().map_err(|e| {
                ChainStateError::StorageError(format!("Fork resolver lock poisoned: {}", e))
            })?;

            match resolver.compare_chains(&block_hash, &current_tip, get_header) {
                Ok(ordering) => match ordering {
                    Ordering::Greater => true, // New chain has more work
                    Ordering::Less => false,   // Current chain has more work
                    Ordering::Equal => {
                        // Equal work - use deterministic tiebreaker
                        // (resolver handles this internally, but for clarity)
                        false
                    }
                },
                Err(e) => {
                    // Log error but don't fail - default to keeping current chain
                    log::warn!("Fork resolution error: {}, keeping current chain", e);
                    false
                }
            }
        } else {
            // New block is lower height, no reorg
            false
        };

        if should_reorg {
            // Handle chain reorganization
            // Convert block_height from u64 to u32 for handle_reorg
            let height_u32 = block_height.try_into().map_err(|_| {
                ChainStateError::InvalidBlock("Block height exceeds u32 maximum".to_string())
            })?;
            self.handle_reorg(&block_hash, height_u32)?;
        } else if *block.prev_block_hash() != current_tip {
            // This is a fork, track it
            self.track_fork(&block_hash, &block)?;
        }

        // Mark as processed
        {
            let mut processed = self
                .processed_blocks
                .write()
                .map_err(|e| ChainStateError::StorageError(e.to_string()))?;
            processed.insert(block_hash);
        }

        // Create checkpoint if needed
        // Convert block_height and checkpoint_interval to the same type for comparison
        let height_u32 = block_height.try_into().map_err(|_| {
            ChainStateError::InvalidBlock("Block height exceeds u32 maximum".to_string())
        })?;
        if height_u32 % self.config.checkpoint_interval == 0 {
            self.create_checkpoint(height_u32, &block_hash)?;
        }

        Ok(should_reorg)
    }

    /// Handle chain reorganization
    fn handle_reorg(&self, new_tip: &[u8; 32], new_height: u32) -> ChainStateResult<()> {
        // Find common ancestor
        let ancestor_height = self.find_fork_ancestor(new_tip)?;

        // Only allow limited reorgs for security
        let current_height = self.get_height()?;
        if current_height > ancestor_height
            && current_height - ancestor_height > self.config.max_fork_length
        {
            return Err(ChainStateError::ReorganizationFailed(
                "Fork too deep".to_string(),
            ));
        }

        // Roll back to ancestor height (would handle UTXOs, etc.)
        // This is simplified - real implementation would restore UTXOs and other state

        // Update tip and height
        {
            let mut tip = self
                .current_tip
                .write()
                .map_err(|e| ChainStateError::StorageError(e.to_string()))?;
            *tip = *new_tip;

            let mut height = self
                .current_height
                .write()
                .map_err(|e| ChainStateError::StorageError(e.to_string()))?;
            *height = new_height;
        }

        // Update active chain in height map
        // In a real implementation, would re-order blocks at each height

        Ok(())
    }

    /// Find the common ancestor between current chain and fork
    fn find_fork_ancestor(&self, fork_tip: &[u8; 32]) -> ChainStateResult<u32> {
        let mut current = *fork_tip;
        let mut fork_header = self
            .get_header(&current)?
            .ok_or_else(|| ChainStateError::BlockNotFound(hex::encode(current)))?;
        let mut fork_height = fork_header.height();

        // Traverse backwards along fork until we find a block in the main chain
        while fork_height > 0 {
            let height_map = self
                .height_map
                .read()
                .map_err(|e| ChainStateError::StorageError(e.to_string()))?;

            // Convert fork_height from u64 to u32 for height_map lookup
            let fork_height_u32: u32 = fork_height.try_into().map_err(|_| {
                ChainStateError::InvalidBlock("Block height exceeds u32 maximum".to_string())
            })?;

            if let Some(hashes) = height_map.get(&fork_height_u32) {
                if !hashes.is_empty() && hashes[0] == current {
                    // This block is on the main chain
                    return Ok(fork_height_u32);
                }
            }

            // Move to previous block
            current = *fork_header.prev_block_hash();
            fork_header = self
                .get_header(&current)?
                .ok_or_else(|| ChainStateError::BlockNotFound(hex::encode(current)))?;
            fork_height = fork_header.height();
        }

        // If we reached genesis, that's the common ancestor
        Ok(0)
    }

    /// Track a fork for potential future reorganization
    fn track_fork(&self, block_hash: &[u8; 32], block: &Block) -> ChainStateResult<()> {
        // Find fork point
        let ancestor_height = self.find_fork_ancestor(block_hash)?;

        // Store in forks map
        {
            let mut forks = self
                .forks
                .write()
                .map_err(|e| ChainStateError::StorageError(e.to_string()))?;
            forks.insert(*block_hash, ancestor_height);

            // Prune old forks if necessary
            if forks.len() > self.config.max_fork_length as usize {
                // Remove oldest fork (implementation would be more sophisticated)
            }
        }

        Ok(())
    }

    /// Create a checkpoint at the given height
    fn create_checkpoint(&self, height: u32, hash: &[u8; 32]) -> ChainStateResult<()> {
        // Get UTXO commitment
        let utxo_commitment = self
            .utxo_set
            .get_commitment()
            .map_err(ChainStateError::UtxoError)?;

        // Create checkpoint
        let checkpoint = Checkpoint {
            height,
            hash: *hash,
            utxo_commitment,
        };

        // Store checkpoint
        {
            let mut checkpoints = self
                .checkpoints
                .write()
                .map_err(|e| ChainStateError::StorageError(e.to_string()))?;
            checkpoints.insert(height, checkpoint);
        }

        Ok(())
    }

    /// Verify a checkpoint
    pub fn verify_checkpoint(&self, height: u32) -> ChainStateResult<bool> {
        // Get checkpoint
        let checkpoint = {
            let checkpoints = self
                .checkpoints
                .read()
                .map_err(|e| ChainStateError::StorageError(e.to_string()))?;
            if let Some(cp) = checkpoints.get(&height) {
                cp.clone()
            } else {
                return Err(ChainStateError::InvalidCheckpoint);
            }
        };

        // Verify block hash matches
        let header = self
            .get_header_by_height(height)?
            .ok_or(ChainStateError::InvalidCheckpoint)?;
        if header.hash() != checkpoint.hash {
            return Ok(false);
        }

        // In a real implementation, would also verify UTXO commitment

        Ok(true)
    }

    /// Get all active forks
    pub fn get_forks(&self) -> ChainStateResult<Vec<([u8; 32], u32)>> {
        let forks = self
            .forks
            .read()
            .map_err(|e| ChainStateError::StorageError(e.to_string()))?;
        Ok(forks
            .iter()
            .map(|(hash, height)| (*hash, *height))
            .collect())
    }

    /// Get all checkpoints
    pub fn get_checkpoints(&self) -> ChainStateResult<Vec<Checkpoint>> {
        let checkpoints = self
            .checkpoints
            .read()
            .map_err(|e| ChainStateError::StorageError(e.to_string()))?;
        Ok(checkpoints.values().cloned().collect())
    }

    pub fn add_block(&self, block: &Block) -> ChainStateResult<()> {
        let block_hash = block.hash();

        // Check if block already exists
        if self.get_header(&block_hash)?.is_some() {
            return Ok(());
        }

        // Get the previous header
        let prev_header = self
            .get_header(block.prev_block_hash())?
            .ok_or_else(|| ChainStateError::BlockNotFound(hex::encode(block.prev_block_hash())))?;

        // Determine the height of this block
        let block_height = prev_header.height() + 1;

        // Update the header cache
        {
            let mut headers = self
                .headers
                .write()
                .map_err(|e| ChainStateError::InternalError(format!("Lock poisoned: {}", e)))?;
            headers.insert(block_hash, block.header().clone());
        }

        // Update height map
        {
            let mut height_map = self
                .height_map
                .write()
                .map_err(|e| ChainStateError::InternalError(format!("Lock poisoned: {}", e)))?;
            // Convert block_height to u32 for storage in height_map
            let height_u32 = u32::try_from(block_height).unwrap_or_else(|_| {
                log::warn!(
                    "Block height {} exceeds u32 max value, truncating",
                    block_height
                );
                u32::MAX
            });
            height_map.entry(height_u32).or_default().push(block_hash);
        }

        // Get current tip and height
        let current_tip = *self.current_tip.read().unwrap();
        let current_height = *self.current_height.read().unwrap();
        let current_height_u64 = u64::from(current_height);

        // Determine if we should reorganize the chain
        let should_reorg = if block_height > current_height_u64 {
            // Higher block is always better
            true
        } else if block_height == current_height_u64 {
            // Same height, choose based on difficulty
            if let Some(current_header) = self.get_header(&current_tip)? {
                // If difficulty is higher, this block is better
                block.header().bits() < current_header.bits()
            } else {
                // Current tip not found (shouldn't happen) - use new block
                true
            }
        } else {
            // Lower height, don't reorg
            false
        };

        // Perform the reorganization if needed
        if should_reorg {
            // Convert block_height to u32 for handle_reorg
            let height_u32 = u32::try_from(block_height).unwrap_or_else(|_| {
                log::warn!(
                    "Block height {} exceeds u32 max value, truncating",
                    block_height
                );
                u32::MAX
            });
            self.handle_reorg(&block_hash, height_u32)?;
        } else if *block.prev_block_hash() != current_tip {
            // Block belongs to a side chain, track it but don't reorg
            self.track_fork(&block_hash, block)?;
        }

        // Update UTXO set (simplified implementation)
        self.update_utxo_set(block)?;

        // Create checkpoint if needed
        let checkpoint_interval = u64::from(self.config.checkpoint_interval);
        if block_height % checkpoint_interval == 0 {
            // Convert block_height to u32 for create_checkpoint
            let height_u32 = u32::try_from(block_height).unwrap_or_else(|_| {
                log::warn!(
                    "Block height {} exceeds u32 max value, truncating",
                    block_height
                );
                u32::MAX
            });
            self.create_checkpoint(height_u32, &block_hash)?;
        }

        Ok(())
    }

    /// Find common ancestor of two chains
    pub fn find_common_ancestor(
        &self,
        fork_point: &[u8; 32],
        max_depth: usize,
    ) -> ChainStateResult<u32> {
        let fork_header = self
            .get_header(fork_point)?
            .ok_or_else(|| ChainStateError::BlockNotFound(hex::encode(fork_point)))?;

        let fork_height = fork_header.height();
        let fork_height_u32 = u32::try_from(fork_height).unwrap_or_else(|_| {
            log::warn!(
                "Fork height {} exceeds u32 max value, truncating",
                fork_height
            );
            u32::MAX
        });

        let height_map = self.height_map.read().unwrap();

        // Iterate backward from fork height
        for height in (0..=fork_height_u32).rev().take(max_depth) {
            if let Some(hashes) = height_map.get(&height) {
                if hashes.contains(fork_point) {
                    return Ok(height);
                }
            }
        }

        // If not found, return height 0
        Ok(0)
    }

    /// Get the hash of a block header at a specific height
    pub fn get_header_hash_at_height(&self, height: u64) -> Option<[u8; 32]> {
        let height_map = self.height_map.read().unwrap();
        height_map
            .get(&(height as u32))
            .and_then(|hashes| hashes.first().cloned())
    }

    /// Update the UTXO set with a new block
    fn update_utxo_set(&self, block: &Block) -> ChainStateResult<()> {
        // Simplified implementation
        // In a full implementation, this would update a persistent UTXO set
        // For each transaction in block:
        // 1. Remove spent outputs (inputs)
        // 2. Add new outputs

        // For now, we'll log the operation
        log::debug!("Updated UTXO set with block {}", hex::encode(block.hash()));

        Ok(())
    }

    // These methods provide compatibility between btclib and node layers

    /// Get the best block hash (adapter for node compatibility)
    /// This bridges the API gap between btclib's get_tip() and node's expectation
    pub fn get_best_block_hash(&self) -> [u8; 32] {
        // Use get_tip() but handle the Result properly
        self.get_tip().unwrap_or_else(|_| {
            // If we can't get the tip, return genesis hash as fallback
            log::error!("Failed to get chain tip, returning genesis hash");
            [0u8; 32]
        })
    }

    /// Get the current blockchain height as u64 (adapter for node compatibility)
    /// This bridges the API gap between btclib's Result<u32> and node's expectation of u64
    pub fn get_best_height(&self) -> u64 {
        // Use get_height() but convert Result<u32> to u64
        self.get_height().unwrap_or_else(|e| {
            log::error!("Failed to get chain height: {}, returning 0", e);
            0
        }) as u64
    }

    /// Get block header by height with u64 input (adapter for node compatibility)
    pub fn get_header_by_height_u64(&self, height: u64) -> Option<BlockHeader> {
        // Convert u64 to u32 safely, handling overflow
        let height_u32 = if height > u32::MAX as u64 {
            log::warn!("Height {} exceeds u32::MAX, clamping to u32::MAX", height);
            u32::MAX
        } else {
            height as u32
        };

        // Use the existing method with proper error handling
        match self.get_header_by_height(height_u32) {
            Ok(header) => header,
            Err(e) => {
                log::error!("Failed to get header at height {}: {}", height, e);
                None
            }
        }
    }

    /// Check if block exists by hash (adapter for node compatibility)
    pub fn contains_block(&self, hash: &[u8; 32]) -> bool {
        self.get_header(hash).unwrap_or(None).is_some()
    }

    /// Get the genesis block hash (adapter for node compatibility)
    pub fn get_genesis_hash(&self) -> [u8; 32] {
        // Get the hash at height 0
        if let Some(header) = self.get_header_by_height(0).ok().flatten() {
            header.hash()
        } else {
            // If we can't get genesis, return zero hash
            [0; 32]
        }
    }

    /// Get the current difficulty target from the best chain tip
    pub fn get_difficulty_target(&self) -> u32 {
        // Get the current tip header
        if let Ok(tip_hash) = self.get_tip() {
            if let Ok(Some(header)) = self.get_header(&tip_hash) {
                return header.bits();
            }
        }
        // Return default difficulty if we can't get the tip
        0x1d00ffff // Default Bitcoin difficulty
    }

    /// Get the current block count (chain height + 1)
    pub fn get_block_count(&self) -> u64 {
        let height = *self.current_height.read().unwrap();
        height as u64 + 1
    }

    /// Get the current UTXO count
    pub fn get_utxo_count(&self) -> usize {
        self.utxo_set.get_count()
    }

    /// Get the database size in bytes (estimated)
    pub fn get_database_size(&self) -> u64 {
        // Estimate based on UTXO set size and block count
        let utxo_count = self.get_utxo_count() as u64;
        let block_count = self.get_block_count();

        // Rough estimates:
        // - Each UTXO entry: ~200 bytes
        // - Each block: ~1MB average
        // - Overhead: 20%
        let utxo_size = utxo_count * 200;
        let block_size = block_count * 1_000_000;
        let overhead = (utxo_size + block_size) / 5;

        utxo_size + block_size + overhead
    }
}

// Implement Clone for ChainState to support sharing across async tasks
impl Clone for ChainState {
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            headers: Arc::clone(&self.headers),
            height_map: Arc::clone(&self.height_map),
            current_height: Arc::clone(&self.current_height),
            current_tip: Arc::clone(&self.current_tip),
            forks: Arc::clone(&self.forks),
            processed_blocks: Arc::clone(&self.processed_blocks),
            checkpoints: Arc::clone(&self.checkpoints),
            utxo_set: Arc::clone(&self.utxo_set),
            fork_resolver: Arc::clone(&self.fork_resolver),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_common::*;

    // Helper function to create a test block
    fn create_test_block(prev_hash: [u8; 32], height: u32, target: u32) -> Block {
        // Create a proper block with coinbase transaction
        let coinbase_input = TransactionInput::new_coinbase(height.to_le_bytes().to_vec());
        let coinbase_output = TransactionOutput::new(50_000_000_000, vec![1, 2, 3, 4]);
        let coinbase_tx = Transaction::new(1, vec![coinbase_input], vec![coinbase_output], 0);

        // Use a test-friendly target if target is u32::MAX
        let safe_target = if target == u32::MAX {
            0x207fffff
        } else {
            target
        };

        Block::new_with_params(height, prev_hash, vec![coinbase_tx], safe_target)
    }

    #[test]
    fn test_initialize_with_genesis() {
        // Create UTXO set
        let utxo_set = Arc::new(UtxoSet::new_in_memory(1000));

        // Create chain state
        let chain_state = ChainState::new(ChainStateConfig::default(), utxo_set);

        // Create genesis block
        let genesis = create_test_block([0; 32], 0, u32::MAX);

        // Initialize chain state
        assert!(chain_state.initialize_with_genesis(genesis.clone()).is_ok());

        // Verify state
        assert_eq!(chain_state.get_height().unwrap(), 0);
        assert_eq!(chain_state.get_tip().unwrap(), genesis.hash());
    }

    #[test]
    fn test_process_block() {
        // Create UTXO set
        let utxo_set = Arc::new(UtxoSet::new_in_memory(1000));

        // Create chain state
        let chain_state = ChainState::new(ChainStateConfig::default(), utxo_set);

        // Create genesis block
        let genesis = create_test_block([0; 32], 0, u32::MAX);

        // Initialize chain state
        chain_state
            .initialize_with_genesis(genesis.clone())
            .unwrap();

        // Create new block
        let block1 = create_test_block(genesis.hash(), 1, u32::MAX);

        // Process block
        assert!(chain_state.process_block(block1.clone()).unwrap());

        // Verify state
        assert_eq!(chain_state.get_height().unwrap(), 1);
        assert_eq!(chain_state.get_tip().unwrap(), block1.hash());
    }

    #[test]
    fn test_fork_tracking() {
        // Create UTXO set
        let utxo_set = Arc::new(UtxoSet::new_in_memory(1000));

        // Create chain state
        let chain_state = ChainState::new(ChainStateConfig::default(), utxo_set);

        // Create genesis block
        let genesis = create_test_block([0; 32], 0, u32::MAX);

        // Initialize chain state
        chain_state
            .initialize_with_genesis(genesis.clone())
            .unwrap();

        // Create two competing blocks
        let block1a = create_test_block(genesis.hash(), 1, u32::MAX);
        let block1b = create_test_block(genesis.hash(), 1, u32::MAX - 1);

        // Process first block
        chain_state.process_block(block1a.clone()).unwrap();

        // Process competing block (fork)
        chain_state.process_block(block1b.clone()).unwrap();

        // Verify state
        assert_eq!(chain_state.get_height().unwrap(), 1);

        // Should have one fork
        assert_eq!(chain_state.get_forks().unwrap().len(), 1);
    }
}
