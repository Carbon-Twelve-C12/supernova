use std::collections::{HashMap, HashSet};
use std::sync::{Arc, RwLock};
use serde::{Serialize, Deserialize};
use thiserror::Error;

use crate::types::block::{Block, BlockHeader};
use crate::types::transaction::Transaction;
use crate::storage::utxo_set::{UtxoSet, UtxoEntry, UtxoCommitment};

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
}

impl ChainState {
    /// Create a new chain state
    pub fn new(config: ChainStateConfig, utxo_set: Arc<UtxoSet>) -> Self {
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
        }
    }
    
    /// Initialize the chain state with a genesis block
    pub fn initialize_with_genesis(&self, genesis: Block) -> ChainStateResult<()> {
        let genesis_hash = genesis.hash();
        
        // Ensure we're starting fresh
        {
            let height = self.current_height.read().map_err(|e| ChainStateError::StorageError(e.to_string()))?;
            if *height > 0 {
                return Err(ChainStateError::StorageError("Chain state already initialized".to_string()));
            }
        }
        
        // Add genesis header
        {
            let mut headers = self.headers.write().map_err(|e| ChainStateError::StorageError(e.to_string()))?;
            headers.insert(genesis_hash, genesis.header().clone());
        }
        
        // Update height map
        {
            let mut height_map = self.height_map.write().map_err(|e| ChainStateError::StorageError(e.to_string()))?;
            height_map.insert(0, vec![genesis_hash]);
        }
        
        // Set as current tip
        {
            let mut current_tip = self.current_tip.write().map_err(|e| ChainStateError::StorageError(e.to_string()))?;
            *current_tip = genesis_hash;
        }
        
        // Mark as processed
        {
            let mut processed = self.processed_blocks.write().map_err(|e| ChainStateError::StorageError(e.to_string()))?;
            processed.insert(genesis_hash);
        }
        
        // Process genesis outputs to UTXO set
        for (index, output) in genesis.transactions()[0].outputs().iter().enumerate() {
            let outpoint = crate::types::transaction::OutPoint {
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
            self.utxo_set.add(utxo_entry).map_err(|e| ChainStateError::UtxoError(e))?;
        }
        
        // Create genesis checkpoint
        self.create_checkpoint(0, &genesis_hash)?;
        
        Ok(())
    }
    
    /// Get the current blockchain height
    pub fn get_height(&self) -> ChainStateResult<u32> {
        let height = self.current_height.read().map_err(|e| ChainStateError::StorageError(e.to_string()))?;
        Ok(*height)
    }
    
    /// Get the current tip hash
    pub fn get_tip(&self) -> ChainStateResult<[u8; 32]> {
        let tip = self.current_tip.read().map_err(|e| ChainStateError::StorageError(e.to_string()))?;
        Ok(*tip)
    }
    
    /// Get a block header by hash
    pub fn get_header(&self, hash: &[u8; 32]) -> ChainStateResult<Option<BlockHeader>> {
        let headers = self.headers.read().map_err(|e| ChainStateError::StorageError(e.to_string()))?;
        Ok(headers.get(hash).cloned())
    }
    
    /// Get a block header by height (from active chain)
    pub fn get_header_by_height(&self, height: u32) -> ChainStateResult<Option<BlockHeader>> {
        // Get hash at height
        let hash = {
            let height_map = self.height_map.read().map_err(|e| ChainStateError::StorageError(e.to_string()))?;
            
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
            let processed = self.processed_blocks.read().map_err(|e| ChainStateError::StorageError(e.to_string()))?;
            if processed.contains(&block_hash) {
                return Ok(false); // Already processed, not an error
            }
        }
        
        // Verify basic block structure (more validation would happen in validation layer)
        if !block.validate() {
            return Err(ChainStateError::InvalidBlock("Block failed basic validation".to_string()));
        }
        
        // Get the previous block header
        let prev_header = self.get_header(&block.prev_block_hash())?
            .ok_or_else(|| ChainStateError::BlockNotFound(hex::encode(block.prev_block_hash())))?;
        
        // Calculate height of this block
        let block_height = prev_header.height() + 1;
        
        // Store header in memory
        {
            let mut headers = self.headers.write().map_err(|e| ChainStateError::StorageError(e.to_string()))?;
            if headers.len() >= self.config.max_headers {
                // Prune old headers if needed
                // Implementation would remove oldest non-checkpoint headers
            }
            headers.insert(block_hash, block.header().clone());
        }
        
        // Update height map
        {
            let mut height_map = self.height_map.write().map_err(|e| ChainStateError::StorageError(e.to_string()))?;
            height_map.entry(block_height).or_default().push(block_hash);
        }
        
        // Check if this is a better chain according to policy
        let current_tip = self.get_tip()?;
        let current_height = self.get_height()?;
        let should_reorg = if block_height > current_height {
            // New block is higher, potential reorg
            true
        } else if block_height == current_height {
            // Same height, use fork resolution policy
            match self.config.fork_resolution_policy {
                ForkResolutionPolicy::MostWork => {
                    // Compare accumulated work (simplified - use target as proxy)
                    block.header().target() < self.get_header(&current_tip)?.unwrap().target()
                },
                ForkResolutionPolicy::FirstSeen => false, // Stick with what we saw first
                ForkResolutionPolicy::MostBlocks => false, // Equal blocks, stick with current
            }
        } else {
            // New block is lower height, no reorg
            false
        };
        
        if should_reorg {
            // Handle chain reorganization
            self.handle_reorg(&block_hash, block_height)?;
        } else if block.prev_block_hash() != current_tip {
            // This is a fork, track it
            self.track_fork(&block_hash, &block)?;
        }
        
        // Mark as processed
        {
            let mut processed = self.processed_blocks.write().map_err(|e| ChainStateError::StorageError(e.to_string()))?;
            processed.insert(block_hash);
        }
        
        // Create checkpoint if needed
        if block_height % self.config.checkpoint_interval == 0 {
            self.create_checkpoint(block_height, &block_hash)?;
        }
        
        Ok(should_reorg)
    }
    
    /// Handle chain reorganization
    fn handle_reorg(&self, new_tip: &[u8; 32], new_height: u32) -> ChainStateResult<()> {
        // Find common ancestor
        let ancestor_height = self.find_fork_ancestor(new_tip)?;
        
        // Only allow limited reorgs for security
        let current_height = self.get_height()?;
        if current_height > ancestor_height && 
           current_height - ancestor_height > self.config.max_fork_length {
            return Err(ChainStateError::ReorganizationFailed("Fork too deep".to_string()));
        }
        
        // Roll back to ancestor height (would handle UTXOs, etc.)
        // This is simplified - real implementation would restore UTXOs and other state
        
        // Update tip and height
        {
            let mut tip = self.current_tip.write().map_err(|e| ChainStateError::StorageError(e.to_string()))?;
            *tip = *new_tip;
            
            let mut height = self.current_height.write().map_err(|e| ChainStateError::StorageError(e.to_string()))?;
            *height = new_height;
        }
        
        // Update active chain in height map
        // In a real implementation, would re-order blocks at each height
        
        Ok(())
    }
    
    /// Find the common ancestor between current chain and fork
    fn find_fork_ancestor(&self, fork_tip: &[u8; 32]) -> ChainStateResult<u32> {
        let mut current = *fork_tip;
        let mut fork_header = self.get_header(&current)?.ok_or_else(|| 
            ChainStateError::BlockNotFound(hex::encode(current)))?;
        let mut fork_height = fork_header.height();
        
        // Traverse backwards along fork until we find a block in the main chain
        while fork_height > 0 {
            let height_map = self.height_map.read().map_err(|e| ChainStateError::StorageError(e.to_string()))?;
            
            if let Some(hashes) = height_map.get(&fork_height) {
                if !hashes.is_empty() && hashes[0] == current {
                    // This block is on the main chain
                    return Ok(fork_height);
                }
            }
            
            // Move to previous block
            current = fork_header.prev_block_hash();
            fork_header = self.get_header(&current)?.ok_or_else(|| 
                ChainStateError::BlockNotFound(hex::encode(current)))?;
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
            let mut forks = self.forks.write().map_err(|e| ChainStateError::StorageError(e.to_string()))?;
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
        let utxo_commitment = self.utxo_set.get_commitment()
            .map_err(|e| ChainStateError::UtxoError(e))?;
        
        // Create checkpoint
        let checkpoint = Checkpoint {
            height,
            hash: *hash,
            utxo_commitment,
        };
        
        // Store checkpoint
        {
            let mut checkpoints = self.checkpoints.write().map_err(|e| ChainStateError::StorageError(e.to_string()))?;
            checkpoints.insert(height, checkpoint);
        }
        
        Ok(())
    }
    
    /// Verify a checkpoint
    pub fn verify_checkpoint(&self, height: u32) -> ChainStateResult<bool> {
        // Get checkpoint
        let checkpoint = {
            let checkpoints = self.checkpoints.read().map_err(|e| ChainStateError::StorageError(e.to_string()))?;
            if let Some(cp) = checkpoints.get(&height) {
                cp.clone()
            } else {
                return Err(ChainStateError::InvalidCheckpoint);
            }
        };
        
        // Verify block hash matches
        let header = self.get_header_by_height(height)?.ok_or(ChainStateError::InvalidCheckpoint)?;
        if header.hash() != checkpoint.hash {
            return Ok(false);
        }
        
        // In a real implementation, would also verify UTXO commitment
        
        Ok(true)
    }
    
    /// Get all active forks
    pub fn get_forks(&self) -> ChainStateResult<Vec<([u8; 32], u32)>> {
        let forks = self.forks.read().map_err(|e| ChainStateError::StorageError(e.to_string()))?;
        Ok(forks.iter().map(|(hash, height)| (*hash, *height)).collect())
    }
    
    /// Get all checkpoints
    pub fn get_checkpoints(&self) -> ChainStateResult<Vec<Checkpoint>> {
        let checkpoints = self.checkpoints.read().map_err(|e| ChainStateError::StorageError(e.to_string()))?;
        Ok(checkpoints.values().cloned().collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    // Helper function to create a test block
    fn create_test_block(prev_hash: [u8; 32], height: u32, target: u32) -> Block {
        // In a real test, would create a proper block
        Block::new(
            1,
            prev_hash,
            vec![Transaction::new(1, vec![], vec![], 0)],
            target,
        )
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
        chain_state.initialize_with_genesis(genesis.clone()).unwrap();
        
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
        chain_state.initialize_with_genesis(genesis.clone()).unwrap();
        
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