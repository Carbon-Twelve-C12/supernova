use super::database::{BlockchainDB, StorageError};
use btclib::types::block::Block;
use btclib::types::transaction::Transaction;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH, Duration};
use std::collections::{HashMap, HashSet};
use tracing::{info, warn, error};

const MAX_REORG_DEPTH: u64 = 100;
const MAX_FORK_DISTANCE: u64 = 6;

#[derive(Clone)]
pub struct ChainState {
    db: Arc<BlockchainDB>,
    current_height: u64,
    best_block_hash: [u8; 32],
    chain_work: HashMap<[u8; 32], u128>,
    fork_points: HashSet<[u8; 32]>,
    last_reorg_time: SystemTime,
    reorg_count: u64,
}

pub struct ReorganizationEvent {
    pub old_tip: [u8; 32],
    pub new_tip: [u8; 32],
    pub fork_point: [u8; 32],
    pub blocks_disconnected: u64,
    pub blocks_connected: u64,
    pub timestamp: SystemTime,
}

impl ChainState {
    pub fn new(db: Arc<BlockchainDB>) -> Result<Self, StorageError> {
        let current_height = match db.get_metadata(b"height")? {
            Some(height_bytes) => bincode::deserialize(&height_bytes)?,
            None => 0,
        };

        let best_block_hash = match db.get_metadata(b"best_hash")? {
            Some(hash_bytes) => {
                let mut hash = [0u8; 32];
                hash.copy_from_slice(&hash_bytes);
                hash
            }
            None => [0u8; 32],
        };

        Ok(Self {
            db,
            current_height,
            best_block_hash,
            chain_work: HashMap::new(),
            fork_points: HashSet::new(),
            last_reorg_time: SystemTime::now(),
            reorg_count: 0,
        })
    }

    pub fn get_height(&self) -> u64 {
        self.current_height
    }

    pub fn get_best_block_hash(&self) -> [u8; 32] {
        self.best_block_hash
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
        
        let difficulty_bytes = bincode::serialize(&new_total)
            .map_err(|e| StorageError::Serialization(e))?;
            
        self.db.store_metadata(b"total_difficulty", &difficulty_bytes)?;
        Ok(())
    }

    pub async fn process_block(&mut self, block: Block) -> Result<bool, StorageError> {
        let block_hash = block.hash();
        let prev_hash = block.prev_block_hash();

        if !self.validate_block(&block).await? {
            return Err(StorageError::InvalidBlock);
        }

        if self.db.get_block(&block_hash)?.is_some() {
            return Ok(false);
        }

        let new_chain_work = self.calculate_chain_work(&block)?;

        if prev_hash != self.best_block_hash {
            let current_work = self.chain_work.get(&self.best_block_hash).unwrap_or(&0);
            
            if new_chain_work > *current_work {
                let (fork_point, blocks_to_apply, blocks_to_disconnect) = 
                    self.find_fork_point(&block)?;

                if blocks_to_disconnect.len() as u64 > MAX_REORG_DEPTH {
                    warn!("Rejected deep reorganization: {} blocks", blocks_to_disconnect.len());
                    return Ok(false);
                }

                self.handle_chain_reorganization(&block, fork_point, blocks_to_apply, blocks_to_disconnect).await?;
                return Ok(true);
            } else {
                self.fork_points.insert(prev_hash);
            }
        }

        // Get the block's difficulty to update total difficulty
        let block_difficulty = calculate_block_work(block.target()) as u64;
        
        self.store_block(block)?;
        self.chain_work.insert(block_hash, new_chain_work);
        
        // Update the total difficulty
        self.update_total_difficulty(block_difficulty)?;

        Ok(true)
    }

    async fn validate_block(&self, block: &Block) -> Result<bool, StorageError> {
        if !block.validate() {
            return Ok(false);
        }

        if block.height() != self.current_height + 1 
            && block.prev_block_hash() != self.best_block_hash {
            let fork_distance = self.calculate_fork_distance(block)?;
            if fork_distance > MAX_FORK_DISTANCE {
                return Ok(false);
            }
        }

        for tx in block.transactions() {
            if !self.validate_transaction(tx).await? {
                return Ok(false);
            }
        }

        Ok(true)
    }

    async fn validate_transaction(&self, tx: &Transaction) -> Result<bool, StorageError> {
        let mut spent_outputs = HashSet::new();
        for input in tx.inputs() {
            let outpoint = (input.prev_tx_hash(), input.prev_output_index());
            if !spent_outputs.insert(outpoint) {
                return Ok(false);
            }

            if self.db.get_utxo(&input.prev_tx_hash(), input.prev_output_index())?.is_none() {
                return Ok(false);
            }
        }

        Ok(true)
    }

    fn find_fork_point(&self, new_tip: &Block) -> Result<(Block, Vec<Block>, Vec<Block>), StorageError> {
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
                current = self.db.get_block(&current.prev_block_hash())?.unwrap();
            } else {
                blocks_to_disconnect.push(main_chain.clone());
                main_chain = self.db.get_block(&main_chain.prev_block_hash())?.unwrap();
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
    ) -> Result<(), StorageError> {
        let reorg_event = ReorganizationEvent {
            old_tip: self.best_block_hash,
            new_tip: new_tip.hash(),
            fork_point: fork_point.hash(),
            blocks_disconnected: blocks_to_disconnect.len() as u64,
            blocks_connected: blocks_to_apply.len() as u64,
            timestamp: SystemTime::now(),
        };

        self.db.begin_transaction()?;

        for block in blocks_to_disconnect.iter().rev() {
            self.disconnect_block(block)?;
        }

        for block in blocks_to_apply.iter() {
            self.connect_block(block)?;
        }

        self.best_block_hash = new_tip.hash();
        self.current_height = new_tip.height();
        self.last_reorg_time = SystemTime::now();
        self.reorg_count += 1;

        self.db.commit_transaction()?;
        self.prune_fork_points()?;

        info!("Chain reorganization complete: {:?}", reorg_event);

        Ok(())
    }

    fn disconnect_block(&mut self, block: &Block) -> Result<(), StorageError> {
        for tx in block.transactions() {
            for (index, _) in tx.outputs().iter().enumerate() {
                self.db.remove_utxo(&tx.hash(), index as u32)?;
            }

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

        // Adjust total difficulty when disconnecting a block
        let block_difficulty = calculate_block_work(block.target()) as u64;
        let current_difficulty = self.get_total_difficulty();
        let new_total = current_difficulty.saturating_sub(block_difficulty);
        
        let difficulty_bytes = bincode::serialize(&new_total)?;
        self.db.store_metadata(b"total_difficulty", &difficulty_bytes)?;

        self.current_height -= 1;
        self.best_block_hash = block.prev_block_hash();
        
        self.db.store_metadata(b"height", &bincode::serialize(&self.current_height)?)?;
        self.db.store_metadata(b"best_hash", &self.best_block_hash)?;

        Ok(())
    }

    fn connect_block(&mut self, block: &Block) -> Result<(), StorageError> {
        let block_data = bincode::serialize(block)?;
        let block_hash = block.hash();
        
        self.db.store_block(&block_hash, &block_data)?;
        
        // Update total difficulty when connecting a block
        let block_difficulty = calculate_block_work(block.target()) as u64;
        self.update_total_difficulty(block_difficulty)?;
        
        Ok(())
    }

    fn store_block(&mut self, block: Block) -> Result<(), StorageError> {
        let block_hash = block.hash();
        let block_data = bincode::serialize(&block)?;
        
        self.db.store_block(&block_hash, &block_data)?;
        
        for tx in block.transactions() {
            let tx_hash = tx.hash();
            let tx_data = bincode::serialize(tx)?;
            self.db.store_transaction(&tx_hash, &tx_data)?;
        }
        
        self.current_height = block.height();
        self.best_block_hash = block_hash;
        
        self.db.store_metadata(b"height", &bincode::serialize(&self.current_height)?)?;
        self.db.store_metadata(b"best_hash", &block_hash)?;
        
        Ok(())
    }

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

    fn calculate_fork_distance(&self, block: &Block) -> Result<u64, StorageError> {
        let mut current = block.clone();
        let mut distance = 0;

        while current.height() > 0 {
            if self.db.get_block(&current.prev_block_hash())?.is_some() {
                return Ok(distance);
            }
            distance += 1;
            if let Some(prev_block) = self.db.get_block(&current.prev_block_hash())? {
                current = prev_block;
            } else {
                break;
            }
        }

        Ok(distance)
    }

    fn prune_fork_points(&mut self) -> Result<(), StorageError> {
        self.fork_points.retain(|hash| {
            if let Ok(Some(block)) = self.db.get_block(hash) {
                let age = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs() - block.timestamp();
                age < 86400
            } else {
                false
            }
        });
        Ok(())
    }

    pub fn get_block_at_height(&self, height: u64) -> Result<Block, StorageError> {
        let mut current_hash = self.best_block_hash;
        let mut current_height = self.current_height;

        while current_height > height {
            let block = self.db.get_block(&current_hash)?.unwrap();
            current_hash = block.prev_block_hash();
            current_height -= 1;
        }

        self.db.get_block(&current_hash)?
            .ok_or_else(|| StorageError::DatabaseError("Block not found".to_string()))
    }
}

fn calculate_block_work(target: u32) -> u128 {
    let max_target = u128::MAX;
    max_target / target as u128
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

        let genesis = Block::new(1, [0u8; 32], Vec::new(), u32::MAX);
        chain_state.store_block(genesis.clone())?;

        let fork_block = Block::new(1, genesis.hash(), Vec::new(), u32::MAX / 2);
        let reorg_successful = chain_state.process_block(fork_block.clone()).await?;

        assert!(reorg_successful);
        assert_eq!(chain_state.get_best_block_hash(), fork_block.hash());

        let mut deep_fork = fork_block.clone();
        for _ in 0..MAX_REORG_DEPTH + 1 {
            deep_fork = Block::new(
                deep_fork.height() + 1,
                deep_fork.hash(),
                Vec::new(),
                u32::MAX / 3,
            );
        }
        let deep_reorg_result = chain_state.process_block(deep_fork).await?;
        assert!(!deep_reorg_result);

        Ok(())
    }

    #[tokio::test]
    async fn test_fork_validation() -> Result<(), StorageError> {
        let temp_dir = tempdir().unwrap();
        let db = Arc::new(BlockchainDB::new(temp_dir.path())?);
        let mut chain_state = ChainState::new(db)?;

        let genesis = Block::new(1, [0u8; 32], Vec::new(), u32::MAX);
        chain_state.store_block(genesis.clone())?;

        let valid_fork = Block::new(2, genesis.hash(), Vec::new(), u32::MAX / 2);
        assert!(chain_state.validate_block(&valid_fork).await?);

        let mut invalid_fork = genesis.clone();
        for _ in 0..MAX_FORK_DISTANCE + 1 {
            invalid_fork = Block::new(
                invalid_fork.height() + 1,
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
        let genesis = Block::new(1, [0u8; 32], Vec::new(), u32::MAX);
        chain_state.process_block(genesis.clone()).await?;

        // Total difficulty should be increased
        assert!(chain_state.get_total_difficulty() > 0);

        Ok(())
    }
}