use super::database::{BlockchainDB, StorageError};
use btclib::types::{Block, Transaction};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use std::collections::HashMap;
use tracing::{info, warn};

pub struct ChainState {
    db: Arc<BlockchainDB>,
    current_height: u64,
    best_block_hash: [u8; 32],
    // Track work for each chain tip
    chain_work: HashMap<[u8; 32], u128>,
}

impl ChainState {
    // ... [existing new, load_height, load_best_hash methods remain the same]

    /// Process a new block and handle potential reorganization
    pub async fn process_block(&mut self, block: Block) -> Result<bool, StorageError> {
        let block_hash = block.hash();
        let block_height = block.height();
        let prev_hash = block.prev_block_hash();

        // Validate block
        if !block.validate() {
            return Err(StorageError::InvalidBlock);
        }

        // Check if block already exists
        if self.db.get_block(&block_hash)?.is_some() {
            return Ok(false);
        }

        // Calculate total work of the new chain
        let new_chain_work = self.calculate_chain_work(&block)?;

        // If this is a fork, check if we need to reorganize
        if prev_hash != self.best_block_hash {
            let current_work = self.chain_work.get(&self.best_block_hash).unwrap_or(&0);
            
            if new_chain_work > *current_work {
                // New chain has more work, perform reorganization
                self.handle_chain_reorganization(&block).await?;
                return Ok(true);
            }
        }

        // Store block normally
        self.store_block(block)?;
        self.chain_work.insert(block_hash, new_chain_work);

        Ok(true)
    }

    /// Handle chain reorganization
    async fn handle_chain_reorganization(&mut self, new_tip: &Block) -> Result<(), StorageError> {
        info!("Starting chain reorganization");

        // Find common ancestor
        let (fork_point, blocks_to_apply, blocks_to_disconnect) = 
            self.find_fork_point(new_tip)?;

        // Disconnect blocks from current chain
        for block in blocks_to_disconnect {
            self.disconnect_block(&block)?;
        }

        // Connect blocks from new chain
        for block in blocks_to_apply {
            self.connect_block(&block)?;
        }

        info!("Chain reorganization complete. New tip: {:?}", new_tip.hash());
        Ok(())
    }

    /// Find the common ancestor between current chain and fork
    fn find_fork_point(&self, new_tip: &Block) -> Result<(Block, Vec<Block>, Vec<Block>), StorageError> {
        let mut blocks_to_apply = Vec::new();
        let mut blocks_to_disconnect = Vec::new();
        let mut current = new_tip.clone();
        let mut main_chain = self.get_block_at_height(self.current_height)?;

        // Walk back both chains until we find common ancestor
        while current.height() > 0 && main_chain.height() > 0 {
            if current.hash() == main_chain.hash() {
                // Found common ancestor
                return Ok((current, blocks_to_apply, blocks_to_disconnect));
            }

            if current.height() > main_chain.height() {
                blocks_to_apply.push(current.clone());
                current = self.db.get_block(&current.prev_block_hash())?.unwrap();
            } else {
                blocks_to_disconnect.push(main_chain.clone());
                main_chain = self.db.get_block(&main_chain.prev_block_hash())?.unwrap();
            }
        }

        Err(StorageError::InvalidChainReorganization)
    }

    /// Disconnect a block from the chain
    fn disconnect_block(&mut self, block: &Block) -> Result<(), StorageError> {
        // Restore UTXOs spent by this block
        for tx in block.transactions() {
            // Remove outputs created by this block
            for (index, _) in tx.outputs().iter().enumerate() {
                self.db.remove_utxo(&tx.hash(), index as u32)?;
            }

            // Restore previously spent outputs
            for input in tx.inputs() {
                if let Some(prev_tx) = self.db.get_transaction(&input.prev_tx_hash())? {
                    let output = prev_tx.outputs()[input.prev_output_index() as usize].clone();
                    self.db.store_utxo(
                        &input.prev_tx_hash(),
                        input.prev_output_index(),
                        &bincode::serialize(&output)?,
                    )?;
                }
            }
        }

        // Update chain state
        self.current_height -= 1;
        self.best_block_hash = block.prev_block_hash();
        
        // Persist updated state
        self.db.store_metadata(b"height", &bincode::serialize(&self.current_height)?)?;
        self.db.store_metadata(b"best_hash", &self.best_block_hash)?;

        Ok(())
    }

    /// Connect a block to the chain
    fn connect_block(&mut self, block: &Block) -> Result<(), StorageError> {
        self.store_block(block.clone())?;
        Ok(())
    }

    /// Calculate total chain work
    fn calculate_chain_work(&self, block: &Block) -> Result<u128, StorageError> {
        let mut total_work = 0u128;
        let mut current = block.clone();

        while current.height() > 0 {
            total_work += calculate_block_work(current.target());
            
            if let Some(prev_block) = self.db.get_block(&current.prev_block_hash())? {
                current = prev_block;
            } else {
                break;
            }
        }

        Ok(total_work)
    }

    // ... [rest of your existing methods remain the same]
}

/// Calculate work for a single block (inverse of target)
fn calculate_block_work(target: u32) -> u128 {
    let max_target = u128::MAX;
    max_target / target as u128
}

// Add to your StorageError enum:
#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    // ... [existing variants]
    #[error("Invalid block")]
    InvalidBlock,
    #[error("Invalid chain reorganization")]
    InvalidChainReorganization,
}

// ... [existing tests remain the same]

#[cfg(test)]
mod tests {
    use super::*;

    // ... [existing tests remain]

    #[tokio::test]
    async fn test_chain_reorganization() -> Result<(), StorageError> {
        let temp_dir = tempdir().unwrap();
        let db = Arc::new(BlockchainDB::new(temp_dir.path())?);
        let mut chain_state = ChainState::new(db)?;

        // Create initial chain
        let genesis = Block::new(1, [0u8; 32], Vec::new(), u32::MAX);
        chain_state.store_block(genesis.clone())?;

        // Create fork with more work
        let fork_block = Block::new(1, genesis.hash(), Vec::new(), u32::MAX / 2);  // More difficult
        let reorg_successful = chain_state.process_block(fork_block.clone()).await?;

        assert!(reorg_successful);
        assert_eq!(chain_state.get_best_block_hash(), fork_block.hash());
        Ok(())
    }
}