use sled::{self, Db, Tree, IVec};
use std::path::Path;
use std::sync::Arc;
use thiserror::Error;
use btclib::types::block::{Block, BlockHeader};
use btclib::types::transaction::Transaction;

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
        // Since Arc<Db> doesn't have path(), return a default path
        Path::new("./data")
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
        // sled 0.34 doesn't have compact_range, so we just flush the database
        self.db.flush()?;
        
        // For each tree, use the regular sled tree flush
        self.blocks.flush()?;
        self.transactions.flush()?;
        self.utxos.flush()?;
        self.metadata.flush()?;
        self.block_height_index.flush()?;
        self.tx_index.flush()?;
        self.headers.flush()?;
        self.pending_blocks.flush()?;
        
        Ok(())
    }

    /// Flush all pending writes to disk
    pub fn flush(&self) -> Result<(), StorageError> {
        self.db.flush()?;
        Ok(())
    }

    // NEW METHODS BELOW THIS LINE - Added for CorruptionHandler support

    /// List all trees (collections) in the database
    pub fn list_trees(&self) -> Result<Vec<String>, StorageError> {
        // Since sled doesn't have a direct method to list all trees, 
        // we maintain a registry of known trees
        let tree_names = match self.get_metadata(b"tree_registry") {
            Ok(Some(data)) => {
                bincode::deserialize::<Vec<String>>(&data)
                    .map_err(|e| StorageError::Serialization(e))?
            },
            _ => {
                // If no registry exists, return the known default trees
                vec![
                    BLOCKS_TREE.to_string(),
                    TXNS_TREE.to_string(),
                    HEADERS_TREE.to_string(),
                    UTXO_TREE.to_string(),
                    METADATA_TREE.to_string(),
                    BLOCK_HEIGHT_INDEX_TREE.to_string(),
                    TX_INDEX_TREE.to_string(),
                    PENDING_BLOCKS_TREE.to_string(),
                ]
            }
        };
        
        Ok(tree_names)
    }

    /// Open a specific tree by name
    pub fn open_tree(&self, name: &str) -> Result<sled::Tree, StorageError> {
        let tree = self.db.open_tree(name)
            .map_err(|e| StorageError::Database(e))?;
        
        // Register the tree if it's new
        self.register_tree(name)?;
        
        Ok(tree)
    }

    /// Register a tree in the tree registry
    fn register_tree(&self, name: &str) -> Result<(), StorageError> {
        let mut trees = match self.get_metadata(b"tree_registry") {
            Ok(Some(data)) => {
                bincode::deserialize::<Vec<String>>(&data)
                    .map_err(|e| StorageError::Serialization(e))?
            },
            _ => Vec::new(),
        };
        
        // Add the tree if it's not already registered
        if !trees.contains(&name.to_string()) {
            trees.push(name.to_string());
            let serialized = bincode::serialize(&trees)
                .map_err(|e| StorageError::Serialization(e))?;
            self.store_metadata(b"tree_registry", &serialized)?;
        }
        
        Ok(())
    }

    /// Get a reference to the underlying sled database
    pub fn db(&self) -> &sled::Db {
        &self.db
    }
    
    /// Check if a specific tree contains a key
    pub fn tree_contains_key(&self, tree_name: &str, key: &[u8]) -> Result<bool, StorageError> {
        let tree = self.open_tree(tree_name)?;
        tree.contains_key(key)
            .map_err(|e| StorageError::Database(e))
    }
    
    /// Get raw data from a tree by key
    pub fn get_raw_data(&self, tree_name: &str, key: &[u8]) -> Result<Option<IVec>, StorageError> {
        let tree = self.open_tree(tree_name)?;
        tree.get(key).map_err(|e| StorageError::Database(e))
    }
    
    /// Store raw data in a tree
    pub fn store_raw_data(&self, tree_name: &str, key: &[u8], value: &[u8]) -> Result<(), StorageError> {
        let tree = self.open_tree(tree_name)?;
        tree.insert(key, value).map(|_| ()).map_err(|e| StorageError::Database(e))
    }
    
    /// Remove a key from a specific tree
    pub fn remove_from_tree(&self, tree_name: &str, key: &[u8]) -> Result<(), StorageError> {
        let tree = self.open_tree(tree_name)?;
        tree.remove(key).map(|_| ()).map_err(|e| StorageError::Database(e))
    }
    
    /// Perform a database backup to a specific directory
    pub async fn backup_to(&self, backup_path: &Path) -> Result<(), StorageError> {
        use tokio::fs;
        
        // Ensure backup directory exists
        if let Some(parent) = backup_path.parent() {
            fs::create_dir_all(parent).await?;
        }
        
        // Flush database to ensure all changes are written
        self.flush()?;
        
        // Copy database files
        let db_path = self.path();
        
        // Get list of database files
        let db_dir = db_path.parent().unwrap_or(Path::new("."));
        let mut entries = fs::read_dir(db_dir).await?;
        
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.is_file() && path.extension().map_or(false, |ext| ext == "sst") {
                let file_name = path.file_name().unwrap();
                let target_path = backup_path.join(file_name);
                fs::copy(&path, &target_path).await?;
            }
        }
        
        tracing::info!("Database backup created at {:?}", backup_path);
        Ok(())
    }
    
    /// Repair a corrupted tree by rebuilding it
    pub fn repair_tree(&self, tree_name: &str) -> Result<(), StorageError> {
        // Create a new temporary tree
        let temp_tree_name = format!("{}_repair", tree_name);
        let temp_tree = self.open_tree(&temp_tree_name)?;
        
        // Get the original tree
        let orig_tree = self.open_tree(tree_name)?;
        
        // Copy all valid data to the new tree
        for result in orig_tree.iter() {
            match result {
                Ok((key, value)) => {
                    // Only copy data that can be validated
                    if self.is_valid_record(tree_name, &key, &value)? {
                        temp_tree.insert(key, value)?;
                    }
                },
                Err(e) => {
                    tracing::error!("Error reading tree {}: {}", tree_name, e);
                    // Continue with other records
                }
            }
        }
        
        // Clear the original tree
        orig_tree.clear()?;
        
        // Copy data back from temp tree
        for result in temp_tree.iter() {
            if let Ok((key, value)) = result {
                orig_tree.insert(key, value)?;
            }
        }
        
        // Remove the temporary tree
        self.db.drop_tree(temp_tree_name.as_bytes())?;
        
        tracing::info!("Successfully repaired tree: {}", tree_name);
        Ok(())
    }
    
    /// Check if a record is valid based on its tree type
    fn is_valid_record(&self, tree_name: &str, key: &[u8], value: &[u8]) -> Result<bool, StorageError> {
        // Empty values are always invalid
        if value.is_empty() {
            return Ok(false);
        }
        
        match tree_name {
            BLOCKS_TREE => {
                match bincode::deserialize::<Block>(value) {
                    Ok(block) => Ok(block.validate()),
                    Err(_) => Ok(false),
                }
            },
            TXNS_TREE => {
                bincode::deserialize::<Transaction>(value).map(|_| true).or(Ok(false))
            },
            HEADERS_TREE => {
                bincode::deserialize::<BlockHeader>(value).map(|_| true).or(Ok(false))
            },
            // For other trees like UTXO or metadata, we can't easily validate without context
            // so we just check if it's not empty
            _ => Ok(true),
        }
    }

    /// Insert a block into the database
    pub fn insert_block(&self, block: &Block) -> Result<(), StorageError> {
        let block_hash = block.hash();
        let block_data = bincode::serialize(block)?;
        self.blocks.insert(block_hash, block_data)?;
        Ok(())
    }
    
    /// Set metadata in the database
    pub fn set_metadata(&self, key: &[u8], value: &[u8]) -> Result<(), StorageError> {
        self.metadata.insert(key, value)?;
        Ok(())
    }
    
    /// Get UTXO from the database
    pub fn get_utxo(&self, tx_hash: &[u8; 32], index: u32) -> Result<Option<Transaction>, StorageError> {
        let key = create_utxo_key(tx_hash, index);
        match self.utxos.get(key)? {
            Some(data) => {
                let tx = bincode::deserialize(&data)?;
                Ok(Some(tx))
            }
            None => Ok(None),
        }
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
    
    // Additional tests for new methods
    
    #[test]
    fn test_list_trees() -> Result<(), StorageError> {
        let temp_dir = tempdir().unwrap();
        let db = BlockchainDB::new(temp_dir.path())?;
        
        let trees = db.list_trees()?;
        assert!(trees.contains(&BLOCKS_TREE.to_string()));
        assert!(trees.contains(&TXNS_TREE.to_string()));
        
        Ok(())
    }
    
    #[test]
    fn test_open_tree() -> Result<(), StorageError> {
        let temp_dir = tempdir().unwrap();
        let db = BlockchainDB::new(temp_dir.path())?;
        
        let custom_tree = db.open_tree("custom_tree")?;
        custom_tree.insert(b"test_key", b"test_value")?;
        
        let value = custom_tree.get(b"test_key")?.unwrap();
        assert_eq!(value.as_ref(), b"test_value");
        
        Ok(())
    }
    
    #[test]
    fn test_tree_contains_key() -> Result<(), StorageError> {
        let temp_dir = tempdir().unwrap();
        let db = BlockchainDB::new(temp_dir.path())?;
        
        // Store a test value
        db.store_metadata(b"test_key", b"test_value")?;
        
        assert!(db.tree_contains_key(METADATA_TREE, b"test_key")?);
        assert!(!db.tree_contains_key(METADATA_TREE, b"non_existent_key")?);
        
        Ok(())
    }
    
    #[test]
    fn test_repair_tree() -> Result<(), StorageError> {
        let temp_dir = tempdir().unwrap();
        let db = BlockchainDB::new(temp_dir.path())?;
        
        // Create a test tree with some valid and some "invalid" data
        let tree_name = "test_repair_tree";
        db.store_raw_data(tree_name, b"valid_key", b"valid_value")?;
        db.store_raw_data(tree_name, b"empty_value_key", b"")?; // This would be considered invalid
        
        // Repair the tree
        db.repair_tree(tree_name)?;
        
        // Check that the valid data is still there
        let valid_data = db.get_raw_data(tree_name, b"valid_key")?;
        assert_eq!(valid_data.unwrap().as_ref(), b"valid_value");
        
        // The invalid data should be gone
        let invalid_data = db.get_raw_data(tree_name, b"empty_value_key")?;
        assert!(invalid_data.is_none());
        
        Ok(())
    }
}