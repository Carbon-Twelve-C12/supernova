use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, RwLock};

use bincode;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::types::block::{Block, BlockHeader};

/// Errors that can occur in block storage operations
#[derive(Debug, Error)]
pub enum BlockStoreError {
    #[error("IO error: {0}")]
    IoError(#[from] io::Error),

    #[error("Serialization error: {0}")]
    SerializationError(String),

    #[error("Block not found: {0}")]
    BlockNotFound(String),

    #[error("Index error: {0}")]
    IndexError(String),

    #[error("Invalid block data")]
    InvalidData,
}

/// Result type for block store operations
pub type BlockStoreResult<T> = Result<T, BlockStoreError>;

/// Configuration for block storage
#[derive(Debug, Clone)]
pub struct BlockStorageConfig {
    /// Base directory for block storage
    pub storage_dir: PathBuf,

    /// Maximum block file size in bytes
    pub max_file_size: u64,

    /// Number of blocks per file
    pub blocks_per_file: u32,

    /// Whether to use memory mapping
    pub use_mmap: bool,

    /// Whether to compress blocks
    pub compress_blocks: bool,

    /// Cache size for recently accessed blocks
    pub cache_size: usize,
}

impl Default for BlockStorageConfig {
    fn default() -> Self {
        Self {
            storage_dir: PathBuf::from("blocks"),
            max_file_size: 128 * 1024 * 1024, // 128 MB
            blocks_per_file: 1000,
            use_mmap: true,
            compress_blocks: true,
            cache_size: 100,
        }
    }
}

/// Location information for a stored block
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockLocation {
    /// File number where block is stored
    pub file_no: u32,

    /// Offset within the file
    pub offset: u64,

    /// Size of the block data in bytes
    pub size: u32,

    /// Whether the block data is compressed
    pub compressed: bool,
}

/// Manages persistent storage of blocks
pub struct BlockStore {
    /// Configuration for block storage
    config: BlockStorageConfig,

    /// Index of block hash to location
    index: Arc<RwLock<HashMap<[u8; 32], BlockLocation>>>,

    /// Current file number for writing
    current_file_no: Arc<RwLock<u32>>,

    /// Current offset in the active file
    current_offset: Arc<RwLock<u64>>,

    /// Open file handles (file_no -> file handle)
    files: Arc<RwLock<HashMap<u32, Arc<Mutex<File>>>>>,

    /// Cache of recently used blocks (hash -> block)
    cache: Arc<RwLock<HashMap<[u8; 32], Block>>>,
}

impl BlockStore {
    /// Create a new block store with the given configuration
    pub fn new(config: BlockStorageConfig) -> BlockStoreResult<Self> {
        // Create storage directory if it doesn't exist
        std::fs::create_dir_all(&config.storage_dir)?;

        // Initialize block store
        let block_store = Self {
            config,
            index: Arc::new(RwLock::new(HashMap::new())),
            current_file_no: Arc::new(RwLock::new(0)),
            current_offset: Arc::new(RwLock::new(0)),
            files: Arc::new(RwLock::new(HashMap::new())),
            cache: Arc::new(RwLock::new(HashMap::with_capacity(100))),
        };

        // Load index if it exists
        block_store.load_index()?;

        // Find the latest file to continue from
        block_store.initialize_files()?;

        Ok(block_store)
    }

    /// Initialize file handling by finding the latest file
    fn initialize_files(&self) -> BlockStoreResult<()> {
        let entries = std::fs::read_dir(&self.config.storage_dir)?;
        let mut max_file_no = 0;

        // Find the highest file number
        for entry in entries {
            if let Ok(entry) = entry {
                if let Some(file_name) = entry.file_name().to_str() {
                    if file_name.starts_with("blk") && file_name.ends_with(".dat") {
                        if let Ok(file_no) = file_name[3..file_name.len() - 4].parse::<u32>() {
                            max_file_no = std::cmp::max(max_file_no, file_no);
                        }
                    }
                }
            }
        }

        // Update current file number
        {
            let mut current_file_no = self.current_file_no.write().map_err(|_| {
                // ENHANCED ERROR CONTEXT: Lock acquisition failure during initialization
                // Poison occurs if another thread panicked while updating current_file_no
                BlockStoreError::IndexError(format!(
                    "Failed to acquire write lock on current_file_no during store initialization. \
                     Attempted to set file number to {}. Lock may be poisoned by previous panic.",
                    max_file_no
                ))
            })?;
            *current_file_no = max_file_no;
        }

        // Open the current file and get its size to set the offset
        let file_path = self.get_file_path(max_file_no)?;
        if file_path.exists() {
            let file = self.open_file(max_file_no)?;
            let file_size = file
                .lock()
                .map_err(|_| {
                    // ENHANCED ERROR CONTEXT: File lock failure during initialization
                    BlockStoreError::IndexError(format!(
                        "Failed to acquire file lock on blk{:05}.dat during initialization. \
                         Lock may be poisoned. This prevents determining file size for offset calculation.",
                        max_file_no
                    ))
                })?
                .metadata()?
                .len();

            // Update current offset
            {
                let mut current_offset = self.current_offset.write().map_err(|_| {
                    // ENHANCED ERROR CONTEXT: Offset update lock failure
                    BlockStoreError::IndexError(format!(
                        "Failed to acquire write lock on current_offset during initialization. \
                         Attempted to set offset to {} bytes for file {}. Lock may be poisoned.",
                        file_size, max_file_no
                    ))
                })?;
                *current_offset = file_size;
            }
        }

        Ok(())
    }

    /// Get the path for a block file
    fn get_file_path(&self, file_no: u32) -> BlockStoreResult<PathBuf> {
        let file_name = format!("blk{:05}.dat", file_no);
        Ok(self.config.storage_dir.join(file_name))
    }

    /// Open a file for reading/writing
    fn open_file(&self, file_no: u32) -> BlockStoreResult<Arc<Mutex<File>>> {
        // Check if already open
        {
            let files = self.files.read().map_err(|_| {
                // ENHANCED ERROR CONTEXT: Files cache read lock failure
                BlockStoreError::IndexError(format!(
                    "Failed to acquire read lock on files cache when opening blk{:05}.dat. \
                     Lock may be poisoned. This prevents checking if file is already open.",
                    file_no
                ))
            })?;

            if let Some(file) = files.get(&file_no) {
                return Ok(Arc::clone(file));
            }
        }

        // Open the file
        let path = self.get_file_path(file_no)?;
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(path)?;

        let file_arc = Arc::new(Mutex::new(file));

        // Store in cache
        {
            let mut files = self.files.write().map_err(|_| {
                // ENHANCED ERROR CONTEXT: Files cache write lock failure
                BlockStoreError::IndexError(format!(
                    "Failed to acquire write lock on files cache when caching blk{:05}.dat. \
                     Lock may be poisoned. File was opened successfully but cannot be cached for reuse.",
                    file_no
                ))
            })?;

            files.insert(file_no, Arc::clone(&file_arc));
        }

        Ok(file_arc)
    }

    /// Store a block to disk
    pub fn store_block(&self, block: &Block) -> BlockStoreResult<()> {
        let block_hash = block.hash();

        // Check if already stored
        {
            let index = self.index.read().map_err(|_| {
                // ENHANCED ERROR CONTEXT: Index read lock failure during block storage
                BlockStoreError::IndexError(format!(
                    "Failed to acquire read lock on block index when storing block {}. \
                     Lock may be poisoned. Cannot verify if block already exists before writing.",
                    hex::encode(&block_hash[..8])
                ))
            })?;

            if index.contains_key(&block_hash) {
                return Ok(()); // Already stored
            }
        }

        // Serialize the block
        let block_data = bincode::serialize(block)
            .map_err(|e| BlockStoreError::SerializationError(e.to_string()))?;

        // Compress if configured
        let (data_to_write, compressed) = if self.config.compress_blocks {
            // In a real implementation, use a compression library like flate2 or zstd
            // For simplicity, we'll skip actual compression here
            (block_data, false)
        } else {
            (block_data, false)
        };

        // Get current file info
        let (file_no, offset) = {
            let file_no = *self.current_file_no.read().map_err(|_| {
                BlockStoreError::IndexError("Failed to acquire read lock".to_string())
            })?;

            let offset = *self.current_offset.read().map_err(|_| {
                BlockStoreError::IndexError("Failed to acquire read lock".to_string())
            })?;

            (file_no, offset)
        };

        // Check if we need a new file
        let need_new_file = offset + data_to_write.len() as u64 > self.config.max_file_size;

        let (file_no_to_use, offset_to_use) = if need_new_file {
            // Create a new file
            let new_file_no = file_no + 1;

            // Update current file number and reset offset
            {
                let mut current_file_no = self.current_file_no.write().map_err(|_| {
                    BlockStoreError::IndexError("Failed to acquire write lock".to_string())
                })?;
                *current_file_no = new_file_no;
            }

            {
                let mut current_offset = self.current_offset.write().map_err(|_| {
                    BlockStoreError::IndexError("Failed to acquire write lock".to_string())
                })?;
                *current_offset = 0;
            }

            (new_file_no, 0)
        } else {
            (file_no, offset)
        };

        // Open or get the file
        let file_arc = self.open_file(file_no_to_use)?;
        let mut file = file_arc
            .lock()
            .map_err(|_| BlockStoreError::IndexError("Failed to acquire file lock".to_string()))?;

        // Write data
        file.seek(SeekFrom::Start(offset_to_use))?;
        file.write_all(&data_to_write)?;

        // Create block location
        let location = BlockLocation {
            file_no: file_no_to_use,
            offset: offset_to_use,
            size: data_to_write.len() as u32,
            compressed,
        };

        // Update index
        {
            let mut index = self.index.write().map_err(|_| {
                BlockStoreError::IndexError("Failed to acquire write lock".to_string())
            })?;
            index.insert(block_hash, location);
        }

        // Update current offset
        {
            let mut current_offset = self.current_offset.write().map_err(|_| {
                BlockStoreError::IndexError("Failed to acquire write lock".to_string())
            })?;
            if file_no_to_use == file_no {
                *current_offset = offset_to_use + data_to_write.len() as u64;
            } else {
                *current_offset = data_to_write.len() as u64;
            }
        }

        // Add to cache
        {
            let mut cache = self.cache.write().map_err(|_| {
                BlockStoreError::IndexError("Failed to acquire write lock".to_string())
            })?;

            // Remove oldest if cache is full
            if cache.len() >= self.config.cache_size {
                if let Some(oldest) = cache.keys().next().cloned() {
                    cache.remove(&oldest);
                }
            }

            cache.insert(block_hash, block.clone());
        }

        // Sync index periodically (in production would do this asynchronously)
        if self.should_sync_index() {
            self.save_index()?;
        }

        Ok(())
    }

    /// Load a block from disk
    pub fn load_block(&self, block_hash: &[u8; 32]) -> BlockStoreResult<Block> {
        // Check cache first
        {
            let cache = self.cache.read().map_err(|_| {
                BlockStoreError::IndexError("Failed to acquire read lock".to_string())
            })?;

            if let Some(block) = cache.get(block_hash) {
                return Ok(block.clone());
            }
        }

        // Lookup the file number and offset for this hash
        let (file_no, offset, size, compressed) = {
            let index = self.index.read().map_err(|_| {
                BlockStoreError::IndexError("Failed to acquire read lock".to_string())
            })?;

            match index.get(block_hash) {
                Some(loc) => (loc.file_no, loc.offset, loc.size, loc.compressed),
                None => return Err(BlockStoreError::BlockNotFound(hex::encode(block_hash))),
            }
        };

        // Open the file
        let file_arc = self.open_file(file_no)?;
        let mut file = file_arc
            .lock()
            .map_err(|_| BlockStoreError::IndexError("Failed to acquire file lock".to_string()))?;

        // Read the data
        file.seek(SeekFrom::Start(offset))?;
        let mut data = vec![0u8; size as usize];
        file.read_exact(&mut data)?;

        // Decompress if needed
        let block_data = if compressed {
            // In a real implementation, use decompression
            // For simplicity, we'll just use the data as is
            data
        } else {
            data
        };

        // Deserialize
        let block: Block = bincode::deserialize(&block_data)
            .map_err(|e| BlockStoreError::SerializationError(e.to_string()))?;

        // Verify hash
        if block.hash() != *block_hash {
            return Err(BlockStoreError::InvalidData);
        }

        // Add to cache
        {
            let mut cache = self.cache.write().map_err(|_| {
                BlockStoreError::IndexError("Failed to acquire write lock".to_string())
            })?;

            // Remove oldest if cache is full
            if cache.len() >= self.config.cache_size {
                if let Some(oldest) = cache.keys().next().cloned() {
                    cache.remove(&oldest);
                }
            }

            cache.insert(*block_hash, block.clone());
        }

        Ok(block)
    }

    /// Check if block exists in store
    pub fn has_block(&self, block_hash: &[u8; 32]) -> bool {
        // Check cache first
        {
            if let Ok(cache) = self.cache.read() {
                if cache.contains_key(block_hash) {
                    return true;
                }
            }
        }

        // Check index
        if let Ok(index) = self.index.read() {
            index.contains_key(block_hash)
        } else {
            false
        }
    }

    /// Save index to disk
    fn save_index(&self) -> BlockStoreResult<()> {
        let index_path = self.config.storage_dir.join("index.dat");

        // Get current index
        let index = self
            .index
            .read()
            .map_err(|_| BlockStoreError::IndexError("Failed to acquire read lock".to_string()))?;

        // Serialize
        let index_data = bincode::serialize(&*index)
            .map_err(|e| BlockStoreError::SerializationError(e.to_string()))?;

        // Write to temporary file first for safety
        let temp_path = index_path.with_extension("tmp");
        {
            let mut file = File::create(&temp_path)?;
            file.write_all(&index_data)?;
            file.sync_all()?;
        }

        // Rename to final path
        std::fs::rename(temp_path, index_path)?;

        Ok(())
    }

    /// Load index from disk
    fn load_index(&self) -> BlockStoreResult<()> {
        let index_path = self.config.storage_dir.join("index.dat");

        if !index_path.exists() {
            return Ok(()); // No index file yet
        }

        // Read index file
        let mut file = File::open(index_path)?;
        let mut data = Vec::new();
        file.read_to_end(&mut data)?;

        // Deserialize
        let loaded_index: HashMap<[u8; 32], BlockLocation> = bincode::deserialize(&data)
            .map_err(|e| BlockStoreError::SerializationError(e.to_string()))?;

        // Update index
        {
            let mut index = self.index.write().map_err(|_| {
                BlockStoreError::IndexError("Failed to acquire write lock".to_string())
            })?;
            *index = loaded_index;
        }

        Ok(())
    }

    /// Decide if we should sync the index to disk
    fn should_sync_index(&self) -> bool {
        // In a real implementation, would use a smarter heuristic
        // For now, just return true periodically based on number of blocks
        let index_len = self.index.read().map(|index| index.len()).unwrap_or(0);
        index_len % 100 == 0
    }

    /// Get all blocks in the store (headers only for efficiency)
    pub fn get_all_blocks(&self) -> BlockStoreResult<Vec<(BlockHeader, [u8; 32])>> {
        let mut result = Vec::new();

        // Iterate through index (could be expensive for large blockchains)
        let index = self
            .index
            .read()
            .map_err(|_| BlockStoreError::IndexError("Failed to acquire read lock".to_string()))?;

        for hash in index.keys() {
            // Load block header (optimize by only loading headers)
            let block = self.load_block(hash)?;
            result.push((block.header().clone(), *hash));
        }

        Ok(result)
    }

    /// Get blocks with a specific hash prefix (debug/search tool)
    pub fn find_blocks_by_hash_prefix(&self, prefix: &[u8]) -> BlockStoreResult<Vec<[u8; 32]>> {
        let index = self
            .index
            .read()
            .map_err(|_| BlockStoreError::IndexError("Failed to acquire read lock".to_string()))?;

        let mut matching_hashes = Vec::new();

        for hash in index.keys() {
            if hash.starts_with(prefix) {
                matching_hashes.push(*hash);
            }
        }

        Ok(matching_hashes)
    }

    /// Run integrity check on the block store
    pub fn check_integrity(&self) -> BlockStoreResult<bool> {
        let index = self
            .index
            .read()
            .map_err(|_| BlockStoreError::IndexError("Failed to acquire read lock".to_string()))?;

        for hash in index.keys() {
            // Try to load the block
            match self.load_block(hash) {
                Ok(_) => {}
                Err(_) => {
                    return Ok(false);
                }
            }
        }

        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::transaction::Transaction;
    use tempfile::tempdir;

    #[test]
    fn test_store_and_load_block() {
        // Create temporary directory for test
        let temp_dir = tempdir().unwrap();

        // Create config
        let config = BlockStorageConfig {
            storage_dir: temp_dir.path().to_path_buf(),
            ..Default::default()
        };

        // Create block store
        let block_store = BlockStore::new(config).unwrap();

        // Create a test block
        let header = BlockHeader::new(
            1, [0u8; 32], [0; 32], // merkle_root
            0,       // timestamp
            0,       // bits
            0,       // nonce
        );
        let block = Block::new(header, vec![Transaction::new(1, vec![], vec![], 0)]);
        let block_hash = block.hash();

        // Store the block
        block_store.store_block(&block).unwrap();

        // Check if block exists
        assert!(block_store.has_block(&block_hash));

        // Load the block
        let loaded_block = block_store.load_block(&block_hash).unwrap();

        // Verify loaded block matches original
        assert_eq!(loaded_block.hash(), block_hash);
    }

    #[test]
    fn test_nonexistent_block() {
        // Create temporary directory for test
        let temp_dir = tempdir().unwrap();

        // Create config
        let config = BlockStorageConfig {
            storage_dir: temp_dir.path().to_path_buf(),
            ..Default::default()
        };

        // Create block store
        let block_store = BlockStore::new(config).unwrap();

        // Try to load a nonexistent block
        let result = block_store.load_block(&[1u8; 32]);
        assert!(result.is_err());
    }
}
