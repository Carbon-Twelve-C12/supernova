use sled::{self, Db, IVec};
use std::path::Path;
use std::sync::Arc;
use thiserror::Error;
use btclib::types::{Block, Transaction, BlockHeader};

const BLOCKS_TREE: &str = "blocks";
const TXNS_TREE: &str = "transactions";
const UTXO_TREE: &str = "utxos";
const METADATA_TREE: &str = "metadata";
const BLOCK_HEIGHT_INDEX_TREE: &str = "block_height_index";
const TX_INDEX_TREE: &str = "tx_index";
const HEADERS_TREE: &str = "headers";
const PENDING_BLOCKS_TREE: &str = "pending_blocks";

pub struct BlockchainDB {
    db: Arc<Db>,
    blocks: sled::Tree,
    transactions: sled::Tree,
    utxos: sled::Tree,
    metadata: sled::Tree,
    block_height_index: sled::Tree,
    tx_index: sled::Tree,
    headers: sled::Tree,
    pending_blocks: sled::Tree,
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
            headers: db.open_tree(HEADERS_TREE)?,
            pending_blocks: db.open_tree(PENDING_BLOCKS_TREE)?,
            db: Arc::new(db),
        })
    }

    pub fn path(&self) -> &Path {
        self.db.path()
    }

    /// Store a block in the database
    pub fn store_block(&self, block_hash: &[u8; 32], block_data: &[u8]) -> Result<(), StorageError> {
        self.blocks.insert(block_hash, block_data)?;
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

    /// Store a block header in the database
    pub fn store_block_header(&self, header_hash: &[u8; 32], header_data: &[u8]) -> Result<(), StorageError> {
        self.headers.insert(header_hash, header_data)?;
        Ok(())
    }

    /// Retrieve a block header by its hash
    pub fn get_block_header(&self, header_hash: &[u8; 32]) -> Result<Option<BlockHeader>, StorageError> {
        if let Some(data) = self.headers.get(header_hash)? {
            let header: BlockHeader = bincode::deserialize(&data)?;
            Ok(Some(header))
        } else {
            Ok(None)
        }
    }

    /// Store a pending block during sync
    pub fn store_pending_block(&self, block_hash: &[u8; 32], block_data: &[u8]) -> Result<(), StorageError> {
        self.pending_blocks.insert(block_hash, block_data)?;
        Ok(())
    }

    /// Get a pending block by its hash
    pub fn get_pending_block(&self, block_hash: &[u8; 32]) -> Result<Option<Block>, StorageError> {
        if let Some(data) = self.pending_blocks.get(block_hash)? {
            let block: Block = bincode::deserialize(&data)?;
            Ok(Some(block))
        } else {
            Ok(None)
        }
    }

    /// Remove a pending block once it's been processed
    pub fn remove_pending_block(&self, block_hash: &[u8; 32]) -> Result<(), StorageError> {
        self.pending_blocks.remove(block_hash)?;
        Ok(())
    }

    /// Count the number of pending blocks
    pub fn count_pending_blocks(&self) -> Result<usize, StorageError> {
        Ok(self.pending_blocks.len())
    }

    /// Clear all pending blocks
    pub fn clear_pending_blocks(&self) -> Result<(), StorageError> {
        self.pending_blocks.clear()?;
        Ok(())
    }

    /// Store a transaction in the database
    pub fn store_transaction(&self, tx_hash: &[u8; 32], tx_data: &[u8]) -> Result<(), StorageError> {
        self.transactions.insert(tx_hash, tx_data)?;
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

    /// Get metadata by key
    pub fn get_metadata(&self, key: &[u8]) -> Result<Option<IVec>, StorageError> {
        Ok(self.metadata.get(key)?)
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

    /// Clear the database
    pub fn clear(&self) -> Result<(), StorageError> {
        self.blocks.clear()?;
        self.transactions.clear()?;
        self.utxos.clear()?;
        self.block_height_index.clear()?;
        self.tx_index.clear()?;
        self.headers.clear()?;
        self.pending_blocks.clear()?;
        Ok(())
    }

    /// Clear only the UTXO set
    pub fn clear_utxos(&self) -> Result<(), StorageError> {
        self.utxos.clear()?;
        Ok(())
    }

    /// Begin a transaction
    pub fn begin_transaction(&self) -> Result<(), StorageError> {
        // sled doesn't have explicit transaction begin/commit
        // This is a placeholder for transaction functionality
        Ok(())
    }

    /// Commit a transaction
    pub fn commit_transaction(&self) -> Result<(), StorageError> {
        // sled doesn't have explicit transaction begin/commit
        // This is a placeholder for transaction functionality
        self.db.flush()?;
        Ok(())
    }

    /// Compact the database to reclaim space
    pub fn compact(&self) -> Result<(), StorageError> {
        self.blocks.compact_range(..)?.wait();
        self.transactions.compact_range(..)?.wait();
        self.utxos.compact_range(..)?.wait();
        self.block_height_index.compact_range(..)?.wait();
        self.tx_index.compact_range(..)?.wait();
        self.headers.compact_range(..)?.wait();
        self.pending_blocks.compact_range(..)?.wait();
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
    #[error("Invalid block")]
    InvalidBlock,
    #[error("Invalid chain reorganization")]
    InvalidChainReorganization,
    #[error("Database error: {0}")]
    DatabaseError(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Database restore error")]
    RestoreError,
    #[error("Backup verification failed")]
    BackupVerificationFailed,
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

        let block_data = bincode::serialize(&block)?;
        db.store_block(&block_hash, &block_data)?;
        
        let retrieved = db.get_block(&block_hash)?.unwrap();
        assert_eq!(block.hash(), retrieved.hash());
        
        Ok(())
    }

    #[test]
    fn test_block_header_storage() -> Result<(), StorageError> {
        let temp_dir = tempdir().unwrap();
        let db = BlockchainDB::new(temp_dir.path())?;

        let header = BlockHeader::new(1, [0u8; 32], [0u8; 32], 0);
        let header_hash = header.hash();
        
        let header_data = bincode::serialize(&header)?;
        db.store_block_header(&header_hash, &header_data)?;
        
        let retrieved = db.get_block_header(&header_hash)?.unwrap();
        assert_eq!(header.hash(), retrieved.hash());
        
        Ok(())
    }

    #[test]
    fn test_pending_block_storage() -> Result<(), StorageError> {
        let temp_dir = tempdir().unwrap();
        let db = BlockchainDB::new(temp_dir.path())?;

        let block = Block::new(1, [0u8; 32], Vec::new(), 0);
        let block_hash = block.hash();
        
        let block_data = bincode::serialize(&block)?;
        db.store_pending_block(&block_hash, &block_data)?;
        
        assert_eq!(db.count_pending_blocks()?, 1);
        
        let retrieved = db.get_pending_block(&block_hash)?.unwrap();
        assert_eq!(block.hash(), retrieved.hash());
        
        db.remove_pending_block(&block_hash)?;
        assert_eq!(db.count_pending_blocks()?, 0);
        
        Ok(())
    }

    #[test]
    fn test_transaction_storage() -> Result<(), StorageError> {
        let temp_dir = tempdir().unwrap();
        let db = BlockchainDB::new(temp_dir.path())?;

        let tx = Transaction::new(1, Vec::new(), Vec::new(), 0);
        let tx_hash = tx.hash();

        let tx_data = bincode::serialize(&tx)?;
        db.store_transaction(&tx_hash, &tx_data)?;
        
        let retrieved = db.get_transaction(&tx_hash)?.unwrap();
        assert_eq!(tx.hash(), retrieved.hash());
        
        Ok(())
    }

    #[test]
    fn test_metadata_storage() -> Result<(), StorageError> {
        let temp_dir = tempdir().unwrap();
        let db = BlockchainDB::new(temp_dir.path())?;

        let key = b"test_key";
        let value = b"test_value";
        
        db.store_metadata(key, value)?;
        let retrieved = db.get_metadata(key)?.unwrap();
        
        assert_eq!(retrieved.as_ref(), value);
        
        Ok(())
    }
}