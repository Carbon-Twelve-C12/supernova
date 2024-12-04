use super::database::{BlockchainDB, StorageError};
use btclib::types::{Block, Transaction};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

pub struct ChainState {
    db: Arc<BlockchainDB>,
    current_height: u64,
    best_block_hash: [u8; 32],
}

impl ChainState {
    pub fn new(db: Arc<BlockchainDB>) -> Result<Self, StorageError> {
        let current_height = Self::load_height(&db)?;
        let best_block_hash = Self::load_best_hash(&db)?;

        Ok(Self {
            db,
            current_height,
            best_block_hash,
        })
    }

    /// Load the current chain height from storage
    fn load_height(db: &BlockchainDB) -> Result<u64, StorageError> {
        if let Some(height_bytes) = db.metadata.get("height")? {
            Ok(bincode::deserialize(&height_bytes)?)
        } else {
            Ok(0)
        }
    }

    /// Load the best block hash from storage
    fn load_best_hash(db: &BlockchainDB) -> Result<[u8; 32], StorageError> {
        if let Some(hash_bytes) = db.metadata.get("best_hash")? {
            Ok(bincode::deserialize(&hash_bytes)?)
        } else {
            Ok([0u8; 32])
        }
    }

    /// Store a new block and update chain state
    pub fn store_block(&mut self, block: Block) -> Result<(), StorageError> {
        let block_hash = block.hash();
        let block_height = block.height();

        // Store the block
        self.db.store_block(&block)?;
        
        // Store block height index
        self.db.store_block_height_index(block_height, &block_hash)?;

        // Update UTXOs
        self.update_utxo_set(&block)?;

        // Update chain height and best hash if this is the new tip
        if block_height > self.current_height {
            self.current_height = block_height;
            self.best_block_hash = block_hash;
            
            // Persist metadata
            self.db.store_metadata(b"height", &bincode::serialize(&self.current_height)?)?;
            self.db.store_metadata(b"best_hash", &block_hash)?;
        }

        Ok(())
    }

    /// Update the UTXO set for a new block
    fn update_utxo_set(&self, block: &Block) -> Result<(), StorageError> {
        // Remove spent outputs
        for tx in block.transactions() {
            for input in tx.inputs() {
                self.db.remove_utxo(&input.prev_tx_hash(), input.prev_output_index())?;
            }
        }

        // Add new outputs
        for tx in block.transactions() {
            let tx_hash = tx.hash();
            for (index, output) in tx.outputs().iter().enumerate() {
                self.db.store_utxo(
                    &tx_hash,
                    index as u32,
                    &bincode::serialize(output)?,
                )?;
            }
        }

        Ok(())
    }

    /// Verify and recover chain state if necessary
    pub async fn verify_and_recover(&mut self) -> Result<(), StorageError> {
        if !self.verify_chain_state()? {
            self.recover_chain_state().await?;
        }
        Ok(())
    }

    /// Verify the integrity of the chain state
    fn verify_chain_state(&self) -> Result<bool, StorageError> {
        // Verify best block exists
        if !self.db.get_block(&self.best_block_hash)?.is_some() {
            return Ok(false);
        }

        // Verify height matches best block
        if let Some(best_block) = self.db.get_block(&self.best_block_hash)? {
            if best_block.height() != self.current_height {
                return Ok(false);
            }
        }

        Ok(true)
    }

    /// Recover chain state by scanning the database
    async fn recover_chain_state(&mut self) -> Result<(), StorageError> {
        tracing::warn!("Initiating chain state recovery");

        // Scan all blocks to find the best chain
        let mut best_height = 0;
        let mut best_hash = [0u8; 32];

        let mut iter = self.db.blocks.iter();
        while let Some(Ok((key, value))) = iter.next() {
            let block: Block = bincode::deserialize(&value)?;
            if block.height() > best_height {
                best_height = block.height();
                best_hash.copy_from_slice(&key);
            }
        }

        // Update chain state
        self.current_height = best_height;
        self.best_block_hash = best_hash;

        // Persist recovered state
        self.db.store_metadata(b"height", &bincode::serialize(&self.current_height)?)?;
        self.db.store_metadata(b"best_hash", &self.best_block_hash)?;

        tracing::info!("Chain state recovered. Height: {}, Hash: {:?}", self.current_height, self.best_block_hash);
        Ok(())
    }

    pub fn get_height(&self) -> u64 {
        self.current_height
    }

    pub fn get_best_block_hash(&self) -> [u8; 32] {
        self.best_block_hash
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_chain_state_creation() -> Result<(), StorageError> {
        let temp_dir = tempdir().unwrap();
        let db = Arc::new(BlockchainDB::new(temp_dir.path())?);
        let chain_state = ChainState::new(db)?;
        
        assert_eq!(chain_state.get_height(), 0);
        assert_eq!(chain_state.get_best_block_hash(), [0u8; 32]);
        Ok(())
    }

    #[test]
    fn test_block_storage_and_retrieval() -> Result<(), StorageError> {
        let temp_dir = tempdir().unwrap();
        let db = Arc::new(BlockchainDB::new(temp_dir.path())?);
        let mut chain_state = ChainState::new(db)?;

        let block = Block::new(1, [0u8; 32], Vec::new(), 0);
        chain_state.store_block(block.clone())?;

        assert_eq!(chain_state.get_height(), 1);
        assert_eq!(chain_state.get_best_block_hash(), block.hash());
        Ok(())
    }
}