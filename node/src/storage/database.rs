use sled::{self, Db, IVec};
use std::path::Path;
use std::sync::Arc;
use thiserror::Error;
use btclib::types::{Block, Transaction};

const BLOCKS_TREE: &str = "blocks";
const TXNS_TREE: &str = "transactions";
const UTXO_TREE: &str = "utxos";
const METADATA_TREE: &str = "metadata";
const BLOCK_HEIGHT_INDEX_TREE: &str = "block_height_index";
const TX_INDEX_TREE: &str = "tx_index";

pub struct BlockchainDB {
    db: Arc<Db>,
    blocks: sled::Tree,
    transactions: sled::Tree,
    utxos: sled::Tree,
    metadata: sled::Tree,
    block_height_index: sled::Tree,
    tx_index: sled::Tree,
}

impl BlockchainDB {
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self, StorageError> {
        let db = sled::open(path)?;
        
        Ok(Self {
            blocks: db.open_tree(BLOCKS_TREE)?,
            transactions: db.open_tree(TXNS_TREE)?,
            utxos: db.open_tree(UTXO_TREE)?,
            metadata: db.open_tree(METADATA_TREE)?,
            block_height_index: db.open_tree(BLOCK_HEIGHT_INDEX_TREE)?,
            tx_index: db.open_tree(TX_INDEX_TREE)?,
            db: Arc::new(db),
        })
    }

    pub fn path(&self) -> &Path {
        self.db.path()
    }

    /// Store a block in the database
    pub fn store_block(&self, block: &Block) -> Result<(), StorageError> {
        let block_hash = block.hash();
        let encoded = bincode::serialize(block)?;
        self.blocks.insert(block_hash, encoded)?;
        Ok(())
    }

    /// Retrieve a block by its hash
    pub fn get_block(&self, block_hash: &[u8; 32]) -> Result<Option<Block>, StorageError> {
        if let Some(data) = self.blocks.get(block_hash)? {
            let block: Block = bincode::deserialize(&data)?;
            Ok(Some(block))
        } else {
            Ok(None)
        }
    }

    /// Store a transaction in the database
    pub fn store_transaction(&self, tx: &Transaction) -> Result<(), StorageError> {
        let tx_hash = tx.hash();
        let encoded = bincode::serialize(tx)?;
        self.transactions.insert(tx_hash, encoded)?;
        Ok(())
    }

    /// Retrieve a transaction by its hash
    pub fn get_transaction(&self, tx_hash: &[u8; 32]) -> Result<Option<Transaction>, StorageError> {
        if let Some(data) = self.transactions.get(tx_hash)? {
            let tx: Transaction = bincode::deserialize(&data)?;
            Ok(Some(tx))
        } else {
            Ok(None)
        }
    }

    /// Store UTXO data
    pub fn store_utxo(&self, tx_hash: &[u8; 32], index: u32, output: &[u8]) -> Result<(), StorageError> {
        let key = create_utxo_key(tx_hash, index);
        self.utxos.insert(key, output)?;
        Ok(())
    }

    /// Remove a spent UTXO
    pub fn remove_utxo(&self, tx_hash: &[u8; 32], index: u32) -> Result<(), StorageError> {
        let key = create_utxo_key(tx_hash, index);
        self.utxos.remove(key)?;
        Ok(())
    }

    /// Store chain metadata
    pub fn store_metadata(&self, key: &[u8], value: &[u8]) -> Result<(), StorageError> {
        self.metadata.insert(key, value)?;
        Ok(())
    }

    /// Store block height to hash mapping
    pub fn store_block_height_index(&self, height: u64, block_hash: &[u8; 32]) -> Result<(), StorageError> {
        self.block_height_index.insert(&height.to_be_bytes(), block_hash)?;
        Ok(())
    }

    /// Get block hash by height
    pub fn get_block_by_height(&self, height: u64) -> Result<Option<Block>, StorageError> {
        if let Some(hash) = self.block_height_index.get(&height.to_be_bytes())? {
            self.get_block(hash.as_ref().try_into().unwrap())
        } else {
            Ok(None)
        }
    }

    /// Implement block pruning
    pub fn prune_blocks(&self, height: u64) -> Result<(), StorageError> {
        let mut pruned_count = 0;
        for i in 0..height {
            if let Some(hash) = self.block_height_index.get(&i.to_be_bytes())? {
                self.blocks.remove(hash.as_ref())?;
                self.block_height_index.remove(&i.to_be_bytes())?;
                pruned_count += 1;
            }
        }
        self.store_metadata(b"pruned_height", &height.to_be_bytes())?;
        tracing::info!("Pruned {} blocks up to height {}", pruned_count, height);
        Ok(())
    }

    /// Compact the database to reclaim space
    pub fn compact(&self) -> Result<(), StorageError> {
        self.blocks.compact_range(..)?.wait();
        self.transactions.compact_range(..)?.wait();
        self.utxos.compact_range(..)?.wait();
        self.block_height_index.compact_range(..)?.wait();
        self.tx_index.compact_range(..)?.wait();
        Ok(())
    }

    /// Flush all pending writes to disk
    pub fn flush(&self) -> Result<(), StorageError> {
        self.db.flush()?;
        Ok(())
    }
}

fn create_utxo_key(tx_hash: &[u8; 32], index: u32) -> Vec<u8> {
    let mut key = Vec::with_capacity(36);
    key.extend_from_slice(tx_hash);
    key.extend_from_slice(&index.to_be_bytes());
    key
}

#[derive(Debug, Error)]
pub enum StorageError {
    #[error("Database error: {0}")]
    Database(#[from] sled::Error),
    #[error("Serialization error: {0}")]
    Serialization(#[from] bincode::Error),
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_block_storage() -> Result<(), StorageError> {
        let temp_dir = tempdir().unwrap();
        let db = BlockchainDB::new(temp_dir.path())?;

        let block = Block::new(1, [0u8; 32], Vec::new(), 0);
        let block_hash = block.hash();

        db.store_block(&block)?;
        let retrieved = db.get_block(&block_hash)?.unwrap();

        assert_eq!(block.hash(), retrieved.hash());
        Ok(())
    }

    #[test]
    fn test_transaction_storage() -> Result<(), StorageError> {
        let temp_dir = tempdir().unwrap();
        let db = BlockchainDB::new(temp_dir.path())?;

        let tx = Transaction::new(1, Vec::new(), Vec::new(), 0);
        let tx_hash = tx.hash();

        db.store_transaction(&tx)?;
        let retrieved = db.get_transaction(&tx_hash)?.unwrap();

        assert_eq!(tx.hash(), retrieved.hash());
        Ok(())
    }
}