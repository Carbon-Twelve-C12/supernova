use sled::{self, Db, IVec};
use std::path::Path;
use std::sync::Arc;
use thiserror::Error;
use btclib::types::block::{Block, BlockHeader};
use btclib::types::transaction::Transaction;
use std::path::PathBuf;
use std::time::{SystemTime, Duration, UNIX_EPOCH};
use serde::{Serialize, Deserialize};
use std::sync::RwLock;
use sha2::{Digest, Sha256};

const BLOCKS_TREE: &str = "blocks";
const TXNS_TREE: &str = "transactions";
const UTXO_TREE: &str = "utxos";
const METADATA_TREE: &str = "metadata";
const BLOCK_HEIGHT_INDEX_TREE: &str = "block_height_index";
const TX_INDEX_TREE: &str = "tx_index";
const HEADERS_TREE: &str = "headers";
const PENDING_BLOCKS_TREE: &str = "pending_blocks";
const PENDING_BLOCKS_META_TREE: &str = "pending_blocks_meta";
const PENDING_BLOCKS_INDEX_TREE: &str = "pending_blocks_index";

/// Metadata about a pending block
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingBlockMetadata {
    /// Block hash
    pub hash: [u8; 32],
    /// Block height (if known)
    pub height: Option<u64>,
    /// Time the block was received
    pub received_time: u64,
    /// Priority (higher is more important)
    pub priority: u32,
    /// Source peer ID
    pub source: Option<String>,
    /// Whether the block has been validated
    pub validated: bool,
    /// Validation result (if validated)
    pub valid: Option<bool>,
    /// Whether dependencies have been requested
    pub dependencies_requested: bool,
    /// Missing dependency hashes
    pub missing_dependencies: Vec<[u8; 32]>,
}

impl PendingBlockMetadata {
    /// Create new metadata for a pending block
    pub fn new(hash: [u8; 32], height: Option<u64>, source: Option<String>) -> Self {
        Self {
            hash,
            height,
            received_time: SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs(),
            priority: 0, // Will be set when inserted
            source,
            validated: false,
            valid: None,
            dependencies_requested: false,
            missing_dependencies: Vec::new(),
        }
    }
    
    /// Check if the block has expired
    pub fn is_expired(&self, expiry_time: Duration) -> bool {
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
        let age = now.saturating_sub(self.received_time);
        age > expiry_time.as_secs()
    }
}

/// Configuration for the blockchain database
#[derive(Debug, Clone)]
pub struct BlockchainDBConfig {
    /// Cache size in bytes
    pub cache_size: usize,
    /// Whether to use compression
    pub use_compression: bool,
    /// Flush interval in milliseconds
    pub flush_interval_ms: Option<u64>,
    /// Maximum number of pending blocks
    pub max_pending_blocks: usize,
    /// Expiry time for pending blocks
    pub pending_block_expiry: Duration,
    /// Whether to use bloom filters
    pub use_bloom_filters: bool,
    /// Bloom filter false positive rate
    pub bloom_filter_fpr: f64,
    /// Bloom filter expected item count
    pub bloom_filter_capacity: usize,
}

impl Default for BlockchainDBConfig {
    fn default() -> Self {
        Self {
            cache_size: 512 * 1024 * 1024, // 512MB
            use_compression: true,
            flush_interval_ms: Some(1000), // 1 second
            max_pending_blocks: 5000,
            pending_block_expiry: Duration::from_secs(3600), // 1 hour
            use_bloom_filters: true,
            bloom_filter_fpr: 0.01, // 1% false positive rate
            bloom_filter_capacity: 1_000_000, // Expect 1 million items
        }
    }
}

/// Simple bloom filter implementation
#[derive(Debug, Clone)]
pub struct BloomFilter {
    bits: Vec<u8>,
    hash_count: usize,
    size: usize,
}

impl BloomFilter {
    pub fn new(capacity: usize, false_positive_rate: f64) -> Self {
        // Calculate optimal filter size and hash count
        let size = Self::optimal_size(capacity, false_positive_rate);
        let hash_count = Self::optimal_hash_count(size, capacity);
        
        Self {
            bits: vec![0; (size + 7) / 8], // Convert bits to bytes, rounded up
            hash_count,
            size,
        }
    }
    
    /// Calculate optimal size in bits for the bloom filter
    fn optimal_size(capacity: usize, false_positive_rate: f64) -> usize {
        let ln2_squared = std::f64::consts::LN_2.powi(2);
        (-1.0 * capacity as f64 * false_positive_rate.ln() / ln2_squared).ceil() as usize
    }
    
    /// Calculate optimal number of hash functions
    fn optimal_hash_count(size: usize, capacity: usize) -> usize {
        let m_over_n = size as f64 / capacity as f64;
        (m_over_n * std::f64::consts::LN_2).ceil() as usize
    }
    
    /// Insert an item into the bloom filter
    pub fn insert(&mut self, data: &[u8]) {
        for i in 0..self.hash_count {
            let hash = self.hash(data, i);
            let pos = hash % self.size;
            let byte_pos = pos / 8;
            let bit_pos = pos % 8;
            self.bits[byte_pos] |= 1 << bit_pos;
        }
    }
    
    /// Check if an item might be in the bloom filter
    pub fn contains(&self, data: &[u8]) -> bool {
        for i in 0..self.hash_count {
            let hash = self.hash(data, i);
            let pos = hash % self.size;
            let byte_pos = pos / 8;
            let bit_pos = pos % 8;
            if self.bits[byte_pos] & (1 << bit_pos) == 0 {
                return false;
            }
        }
        true
    }
    
    /// Hash function for bloom filter (based on FNV-1a with a seed)
    fn hash(&self, data: &[u8], seed: usize) -> usize {
        const FNV_PRIME: u64 = 1099511628211;
        const FNV_OFFSET_BASIS: u64 = 14695981039346656037;
        
        let mut hash = FNV_OFFSET_BASIS ^ (seed as u64);
        for byte in data {
            hash ^= *byte as u64;
            hash = hash.wrapping_mul(FNV_PRIME);
        }
        hash as usize
    }
    
    /// Clear the bloom filter
    pub fn clear(&mut self) {
        for byte in self.bits.iter_mut() {
            *byte = 0;
        }
    }
}

/// Batch operation for atomic database updates
pub struct BatchOperation {
    operations: Vec<BatchOp>,
}

enum BatchOp {
    Insert {
        tree: String,
        key: Vec<u8>,
        value: Vec<u8>,
    },
    Remove {
        tree: String,
        key: Vec<u8>,
    },
}

impl BatchOperation {
    pub fn new() -> Self {
        Self {
            operations: Vec::new(),
        }
    }
    
    pub fn insert<K, V>(&mut self, tree: &str, key: K, value: V) -> Result<(), StorageError>
    where
        K: AsRef<[u8]>,
        V: AsRef<[u8]>,
    {
        self.operations.push(BatchOp::Insert {
            tree: tree.to_string(),
            key: key.as_ref().to_vec(),
            value: value.as_ref().to_vec(),
        });
        
        Ok(())
    }
    
    pub fn remove<K>(&mut self, tree: &str, key: K) -> Result<(), StorageError>
    where
        K: AsRef<[u8]>,
    {
        self.operations.push(BatchOp::Remove {
            tree: tree.to_string(),
            key: key.as_ref().to_vec(),
        });
        
        Ok(())
    }
    
    pub fn is_empty(&self) -> bool {
        self.operations.is_empty()
    }
}

pub struct BlockchainDB {
    db: Arc<Db>,
    db_path: PathBuf,
    blocks: sled::Tree,
    transactions: sled::Tree,
    utxos: sled::Tree,
    metadata: sled::Tree,
    block_height_index: sled::Tree,
    tx_index: sled::Tree,
    headers: sled::Tree,
    pending_blocks: sled::Tree,
    pending_blocks_meta: sled::Tree,
    pending_blocks_index: sled::Tree,
    /// Expiry time for pending blocks
    pending_block_expiry: Duration,
    /// Maximum number of pending blocks
    max_pending_blocks: usize,
    /// Bloom filter for blocks (helps with quick negative lookups)
    block_filter: Arc<RwLock<BloomFilter>>,
    /// Bloom filter for transactions
    tx_filter: Arc<RwLock<BloomFilter>>,
    /// Database configuration
    config: BlockchainDBConfig,
}

impl BlockchainDB {
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self, StorageError> {
        Self::with_config(path, BlockchainDBConfig::default())
    }
    
    pub fn with_config<P: AsRef<Path>>(path: P, db_config: BlockchainDBConfig) -> Result<Self, StorageError> {
        let path_buf = path.as_ref().to_path_buf();
        
        // Configure sled database with optimized settings
        let config = sled::Config::new()
            .path(&path_buf)
            .cache_capacity(db_config.cache_size)
            .flush_every_ms(db_config.flush_interval_ms)
            .use_compression(db_config.use_compression)
            .mode(sled::Mode::HighThroughput); // Optimize for throughput
            
        let db = config.open()?;
        
        // Create bloom filters if enabled
        let (block_filter, tx_filter) = if db_config.use_bloom_filters {
            (
                Arc::new(RwLock::new(BloomFilter::new(
                    db_config.bloom_filter_capacity,
                    db_config.bloom_filter_fpr,
                ))),
                Arc::new(RwLock::new(BloomFilter::new(
                    db_config.bloom_filter_capacity,
                    db_config.bloom_filter_fpr,
                ))),
            )
        } else {
            (
                Arc::new(RwLock::new(BloomFilter::new(1, 1.0))), // Dummy filter
                Arc::new(RwLock::new(BloomFilter::new(1, 1.0))), // Dummy filter
            )
        };
        
        let mut blockchain_db = Self {
            blocks: db.open_tree(BLOCKS_TREE)?,
            transactions: db.open_tree(TXNS_TREE)?,
            utxos: db.open_tree(UTXO_TREE)?,
            metadata: db.open_tree(METADATA_TREE)?,
            block_height_index: db.open_tree(BLOCK_HEIGHT_INDEX_TREE)?,
            tx_index: db.open_tree(TX_INDEX_TREE)?,
            headers: db.open_tree(HEADERS_TREE)?,
            pending_blocks: db.open_tree(PENDING_BLOCKS_TREE)?,
            pending_blocks_meta: db.open_tree(PENDING_BLOCKS_META_TREE)?,
            pending_blocks_index: db.open_tree(PENDING_BLOCKS_INDEX_TREE)?,
            db_path: path_buf,
            db: Arc::new(db),
            pending_block_expiry: db_config.pending_block_expiry,
            max_pending_blocks: db_config.max_pending_blocks,
            block_filter,
            tx_filter,
            config: db_config,
        };
        
        // Initialize bloom filters with existing data if enabled
        if db_config.use_bloom_filters {
            blockchain_db.init_bloom_filters()?;
        }
        
        Ok(blockchain_db)
    }
    
    /// Initialize bloom filters with existing data
    fn init_bloom_filters(&mut self) -> Result<(), StorageError> {
        // Load existing blocks into the bloom filter
        for result in self.blocks.iter() {
            let (key, _) = result?;
            let mut block_filter = self.block_filter.write().unwrap();
            block_filter.insert(&key);
        }
        
        // Load existing transactions into the bloom filter
        for result in self.transactions.iter() {
            let (key, _) = result?;
            let mut tx_filter = self.tx_filter.write().unwrap();
            tx_filter.insert(&key);
        }
        
        Ok(())
    }

    pub fn path(&self) -> &Path {
        &self.db_path
    }

    /// Store a block in the database
    pub fn store_block(&self, block_hash: &[u8; 32], block_data: &[u8]) -> Result<(), StorageError> {
        self.blocks.insert(block_hash, block_data)?;
        
        // Update bloom filter
        if self.config.use_bloom_filters {
            let mut block_filter = self.block_filter.write().unwrap();
            block_filter.insert(block_hash);
        }
        
        Ok(())
    }

    /// Retrieve a block by its hash
    pub fn get_block(&self, block_hash: &[u8; 32]) -> Result<Option<Block>, StorageError> {
        // Check bloom filter first for fast negative lookups
        if self.config.use_bloom_filters {
            let block_filter = self.block_filter.read().unwrap();
            if !block_filter.contains(block_hash) {
                return Ok(None);
            }
        }
        
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
    pub fn store_pending_block(
        &self, 
        block_hash: &[u8; 32], 
        block_data: &[u8],
        height: Option<u64>,
        source: Option<String>,
        priority: Option<u32>
    ) -> Result<(), StorageError> {
        // Create metadata
        let mut metadata = PendingBlockMetadata::new(
            *block_hash,
            height,
            source,
        );
        
        // Set priority (higher for blocks we explicitly requested or at known heights)
        metadata.priority = priority.unwrap_or_else(|| {
            if height.is_some() {
                // Higher priority for blocks with known height
                2
            } else {
                // Lower priority for blocks without height
                1
            }
        });
        
        // Store block data
        self.pending_blocks.insert(block_hash, block_data)?;
        
        // Store metadata
        let meta_data = bincode::serialize(&metadata)?;
        self.pending_blocks_meta.insert(block_hash, meta_data)?;
        
        // If height is known, index by height
        if let Some(h) = height {
            let height_key = h.to_be_bytes();
            self.pending_blocks_index.insert(&height_key, block_hash)?;
        }
        
        // Check if we need to prune old pending blocks
        self.prune_pending_blocks()?;
        
        Ok(())
    }

    /// Get a pending block by its hash
    pub fn get_pending_block(&self, block_hash: &[u8; 32]) -> Result<Option<Block>, StorageError> {
        // First check if metadata exists and block hasn't expired
        if let Some(meta_data) = self.pending_blocks_meta.get(block_hash)? {
            let metadata: PendingBlockMetadata = bincode::deserialize(&meta_data)?;
            
            // Check if expired
            if metadata.is_expired(self.pending_block_expiry) {
                // Remove expired block
                self.remove_pending_block(block_hash)?;
                return Err(StorageError::PendingBlockExpired);
            }
            
            // Get block data
            if let Some(data) = self.pending_blocks.get(block_hash)? {
                // Deserialize block
                match bincode::deserialize(&data) {
                    Ok(block) => Ok(Some(block)),
                    Err(e) => {
                        // Remove invalid block
                        self.remove_pending_block(block_hash)?;
                        Err(StorageError::SerializationError(e))
                    }
                }
            } else {
                // Metadata exists but block doesn't - clean up
                self.pending_blocks_meta.remove(block_hash)?;
                Ok(None)
            }
        } else {
            Ok(None)
        }
    }

    /// Get pending block metadata
    pub fn get_pending_block_metadata(&self, block_hash: &[u8; 32]) -> Result<Option<PendingBlockMetadata>, StorageError> {
        if let Some(meta_data) = self.pending_blocks_meta.get(block_hash)? {
            let metadata: PendingBlockMetadata = bincode::deserialize(&meta_data)?;
            Ok(Some(metadata))
        } else {
            Ok(None)
        }
    }

    /// Update pending block metadata
    pub fn update_pending_block_metadata(&self, metadata: &PendingBlockMetadata) -> Result<(), StorageError> {
        let meta_data = bincode::serialize(metadata)?;
        self.pending_blocks_meta.insert(&metadata.hash, meta_data)?;
        Ok(())
    }

    /// Remove a pending block once it's been processed
    pub fn remove_pending_block(&self, block_hash: &[u8; 32]) -> Result<(), StorageError> {
        // Get metadata to remove height index if present
        if let Some(meta_data) = self.pending_blocks_meta.get(block_hash)? {
            let metadata: PendingBlockMetadata = bincode::deserialize(&meta_data)?;
            
            // Remove height index if present
            if let Some(height) = metadata.height {
                let height_key = height.to_be_bytes();
                self.pending_blocks_index.remove(&height_key)?;
            }
        }
        
        // Remove block and metadata
        self.pending_blocks.remove(block_hash)?;
        self.pending_blocks_meta.remove(block_hash)?;
        
        Ok(())
    }

    /// Get a pending block by height
    pub fn get_pending_block_by_height(&self, height: u64) -> Result<Option<Block>, StorageError> {
        let height_key = height.to_be_bytes();
        
        if let Some(hash_bytes) = self.pending_blocks_index.get(&height_key)? {
            let mut block_hash = [0u8; 32];
            block_hash.copy_from_slice(&hash_bytes);
            self.get_pending_block(&block_hash)
        } else {
            Ok(None)
        }
    }

    /// Count the number of pending blocks
    pub fn count_pending_blocks(&self) -> Result<usize, StorageError> {
        Ok(self.pending_blocks.len())
    }

    /// Get all pending block hashes, sorted by priority
    pub fn get_pending_block_hashes(&self) -> Result<Vec<[u8; 32]>, StorageError> {
        let mut blocks = Vec::new();
        let mut priorities = Vec::new();
        
        // Collect all block hashes and their priorities
        for result in self.pending_blocks_meta.iter() {
            let (key, value) = result?;
            
            let mut block_hash = [0u8; 32];
            block_hash.copy_from_slice(&key);
            
            let metadata: PendingBlockMetadata = bincode::deserialize(&value)?;
            
            blocks.push((block_hash, metadata.priority));
            priorities.push(metadata.priority);
        }
        
        // Sort by priority (descending)
        blocks.sort_by(|a, b| b.1.cmp(&a.1));
        
        // Return just the hashes
        Ok(blocks.into_iter().map(|(hash, _)| hash).collect())
    }

    /// Prune expired pending blocks
    pub fn prune_expired_pending_blocks(&self) -> Result<usize, StorageError> {
        let mut pruned = 0;
        let mut to_remove = Vec::new();
        
        // Find expired blocks
        for result in self.pending_blocks_meta.iter() {
            let (key, value) = result?;
            
            let metadata: PendingBlockMetadata = bincode::deserialize(&value)?;
            
            if metadata.is_expired(self.pending_block_expiry) {
                let mut block_hash = [0u8; 32];
                block_hash.copy_from_slice(&key);
                to_remove.push(block_hash);
            }
        }
        
        // Remove expired blocks
        for hash in to_remove {
            self.remove_pending_block(&hash)?;
            pruned += 1;
        }
        
        Ok(pruned)
    }

    /// Set the expiry time for pending blocks
    pub fn set_pending_block_expiry(&mut self, expiry: Duration) {
        self.pending_block_expiry = expiry;
    }

    /// Set the maximum number of pending blocks
    pub fn set_max_pending_blocks(&mut self, max: usize) {
        self.max_pending_blocks = max;
    }

    /// Prune pending blocks if we have too many
    fn prune_pending_blocks(&self) -> Result<(), StorageError> {
        let count = self.pending_blocks.len();
        
        // If we're under the limit, no need to prune
        if count <= self.max_pending_blocks {
            return Ok(());
        }
        
        // First, try to remove expired blocks
        let pruned = self.prune_expired_pending_blocks()?;
        
        // If we're still over the limit, remove oldest blocks
        if (count - pruned) > self.max_pending_blocks {
            let to_prune = count - pruned - self.max_pending_blocks;
            
            // Get blocks sorted by priority
            let hashes = self.get_pending_block_hashes()?;
            
            // Remove lowest priority blocks first (they're at the end of the list)
            for hash in hashes.iter().rev().take(to_prune) {
                self.remove_pending_block(hash)?;
            }
        }
        
        Ok(())
    }

    /// Clear all pending blocks
    pub fn clear_pending_blocks(&self) -> Result<(), StorageError> {
        self.pending_blocks.clear()?;
        self.pending_blocks_meta.clear()?;
        self.pending_blocks_index.clear()?;
        Ok(())
    }

    /// Store a transaction in the database
    pub fn store_transaction(&self, tx_hash: &[u8; 32], tx_data: &[u8]) -> Result<(), StorageError> {
        self.transactions.insert(tx_hash, tx_data)?;
        
        // Update bloom filter
        if self.config.use_bloom_filters {
            let mut tx_filter = self.tx_filter.write().unwrap();
            tx_filter.insert(tx_hash);
        }
        
        Ok(())
    }

    /// Retrieve a transaction by its hash
    pub fn get_transaction(&self, tx_hash: &[u8; 32]) -> Result<Option<Transaction>, StorageError> {
        // Check bloom filter first for fast negative lookups
        if self.config.use_bloom_filters {
            let tx_filter = self.tx_filter.read().unwrap();
            if !tx_filter.contains(tx_hash) {
                return Ok(None);
            }
        }
        
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
        self.pending_blocks_meta.clear()?;
        self.pending_blocks_index.clear()?;
        self.metadata.clear()?;
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
        self.pending_blocks_meta.flush()?;
        self.pending_blocks_index.flush()?;
        
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

    /// Execute a batch operation atomically
    pub fn execute_batch(&self, batch: BatchOperation) -> Result<(), StorageError> {
        if batch.is_empty() {
            return Ok(());
        }
        
        // Create a transaction
        let txn = self.db.transaction();
        
        // Execute each operation in the batch
        for op in batch.operations {
            match op {
                BatchOp::Insert { tree, key, value } => {
                    let tree = self.db.open_tree(tree)?;
                    txn.insert(&tree, key.as_slice(), value.as_slice())?;
                }
                BatchOp::Remove { tree, key } => {
                    let tree = self.db.open_tree(tree)?;
                    txn.remove(&tree, key.as_slice())?;
                }
            }
        }
        
        // Commit the transaction
        txn.commit()?;
        
        Ok(())
    }

    /// Create a new batch operation
    pub fn create_batch(&self) -> BatchOperation {
        BatchOperation::new()
    }
    
    /// Set database configuration
    pub fn set_config(&mut self, config: BlockchainDBConfig) -> Result<(), StorageError> {
        // Update bloom filters if the configuration changed
        if config.use_bloom_filters != self.config.use_bloom_filters 
            || config.bloom_filter_capacity != self.config.bloom_filter_capacity 
            || config.bloom_filter_fpr != self.config.bloom_filter_fpr {
            
            if config.use_bloom_filters {
                // Create new bloom filters with the new settings
                self.block_filter = Arc::new(RwLock::new(BloomFilter::new(
                    config.bloom_filter_capacity,
                    config.bloom_filter_fpr,
                )));
                
                self.tx_filter = Arc::new(RwLock::new(BloomFilter::new(
                    config.bloom_filter_capacity,
                    config.bloom_filter_fpr,
                )));
                
                // Initialize with existing data
                self.init_bloom_filters()?;
            }
        }
        
        // Update pending block settings
        self.pending_block_expiry = config.pending_block_expiry;
        self.max_pending_blocks = config.max_pending_blocks;
        
        // Update internal configuration
        self.config = config;
        
        Ok(())
    }
    
    /// Get the current database configuration
    pub fn get_config(&self) -> &BlockchainDBConfig {
        &self.config
    }

    /// Verify the integrity of the database
    pub fn verify_integrity(
        &self,
        level: IntegrityCheckLevel,
        repair: bool,
    ) -> Result<IntegrityCheckResult, StorageError> {
        let start_time = std::time::Instant::now();
        let mut result = IntegrityCheckResult {
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            passed: true,
            check_level: level,
            issues: Vec::new(),
            duration_ms: 0,
            items_checked: 0,
        };
        
        // Basic check of critical metadata (all levels)
        self.verify_metadata(&mut result, repair)?;
        
        // For all levels except Quick, check block headers and indices
        if level != IntegrityCheckLevel::Quick {
            self.verify_block_headers(&mut result, repair)?;
            self.verify_indices(&mut result, repair)?;
        }
        
        // For Standard level and beyond, check block-transaction references
        if level >= IntegrityCheckLevel::Standard {
            self.verify_block_transactions(&mut result, repair)?;
        }
        
        // For Comprehensive level and beyond, check all transactions and UTXO set
        if level >= IntegrityCheckLevel::Comprehensive {
            self.verify_transactions(&mut result, repair)?;
            self.verify_utxo_set(&mut result, repair)?;
        }
        
        // For Deep level, perform cross-reference verification
        if level == IntegrityCheckLevel::Deep {
            self.verify_cross_references(&mut result, repair)?;
        }
        
        // Update result
        result.duration_ms = start_time.elapsed().as_millis() as u64;
        result.passed = result.issues.iter().all(|issue| !issue.is_critical);
        
        Ok(result)
    }
    
    /// Verify critical metadata
    fn verify_metadata(
        &self,
        result: &mut IntegrityCheckResult,
        repair: bool,
    ) -> Result<(), StorageError> {
        // Check if basic metadata exists
        let basic_keys = ["height", "best_block_hash"];
        
        for key in basic_keys.iter() {
            if self.metadata.get(key.as_bytes())?.is_none() {
                result.issues.push(IntegrityIssue {
                    issue_type: IntegrityIssueType::MissingItem,
                    description: format!("Missing critical metadata: {}", key),
                    key: Some(key.as_bytes().to_vec()),
                    tree: METADATA_TREE.to_string(),
                    is_critical: true,
                });
                
                if repair {
                    // Try to repair by scanning database to find best chain
                    if *key == "height" {
                        // Find highest valid height
                        let mut max_height = 0;
                        for result in self.block_height_index.iter() {
                            let (key, _) = result?;
                            if key.len() == 8 {
                                let mut height_bytes = [0u8; 8];
                                height_bytes.copy_from_slice(&key);
                                let height = u64::from_be_bytes(height_bytes);
                                if height > max_height {
                                    max_height = height;
                                }
                            }
                        }
                        
                        // If we found a valid height, repair it
                        if max_height > 0 {
                            self.metadata.insert("height".as_bytes(), &max_height.to_be_bytes())?;
                        }
                    }
                }
            }
            
            result.items_checked += 1;
        }
        
        Ok(())
    }
    
    /// Verify block headers and their consistency
    fn verify_block_headers(
        &self,
        result: &mut IntegrityCheckResult,
        repair: bool,
    ) -> Result<(), StorageError> {
        // Check if headers match their hash
        let mut headers_checked = 0;
        for item in self.headers.iter() {
            let (key, value) = item?;
            
            // Skip if key isn't 32 bytes (not a block hash)
            if key.len() != 32 {
                continue;
            }
            
            // Deserialize header
            match bincode::deserialize::<BlockHeader>(&value) {
                Ok(header) => {
                    // Calculate hash and verify it matches the key
                    let computed_hash = header.hash();
                    let mut stored_hash = [0u8; 32];
                    stored_hash.copy_from_slice(&key);
                    
                    if computed_hash != stored_hash {
                        result.issues.push(IntegrityIssue {
                            issue_type: IntegrityIssueType::HashMismatch,
                            description: format!(
                                "Block header hash mismatch. Stored: {}, Computed: {}",
                                hex::encode(&stored_hash[0..4]),
                                hex::encode(&computed_hash[0..4]),
                            ),
                            key: Some(key.to_vec()),
                            tree: HEADERS_TREE.to_string(),
                            is_critical: true,
                        });
                    }
                }
                Err(e) => {
                    result.issues.push(IntegrityIssue {
                        issue_type: IntegrityIssueType::InvalidFormat,
                        description: format!("Failed to deserialize block header: {}", e),
                        key: Some(key.to_vec()),
                        tree: HEADERS_TREE.to_string(),
                        is_critical: true,
                    });
                    
                    if repair {
                        // Remove invalid header
                        self.headers.remove(&key)?;
                    }
                }
            }
            
            headers_checked += 1;
            result.items_checked += 1;
        }
        
        Ok(())
    }
    
    /// Verify indices for consistency
    fn verify_indices(
        &self,
        result: &mut IntegrityCheckResult,
        repair: bool,
    ) -> Result<(), StorageError> {
        // Check height -> hash index consistency
        let mut batch = self.create_batch();
        let mut indices_checked = 0;
        
        for item in self.block_height_index.iter() {
            let (key, value) = item?;
            
            // Check if the hash exists
            if self.blocks.get(&value)?.is_none() {
                result.issues.push(IntegrityIssue {
                    issue_type: IntegrityIssueType::BrokenReference,
                    description: format!(
                        "Height index references non-existent block: {}",
                        hex::encode(&value[0..4])
                    ),
                    key: Some(key.to_vec()),
                    tree: BLOCK_HEIGHT_INDEX_TREE.to_string(),
                    is_critical: false,
                });
                
                if repair {
                    batch.remove(BLOCK_HEIGHT_INDEX_TREE, &key)?;
                }
            }
            
            indices_checked += 1;
            result.items_checked += 1;
        }
        
        // Check tx index consistency
        for item in self.tx_index.iter() {
            let (key, value) = item?;
            
            // Check if the transaction exists
            if self.transactions.get(&key)?.is_none() {
                result.issues.push(IntegrityIssue {
                    issue_type: IntegrityIssueType::BrokenReference,
                    description: format!(
                        "Transaction index references non-existent transaction: {}",
                        hex::encode(&key[0..4])
                    ),
                    key: Some(key.to_vec()),
                    tree: TX_INDEX_TREE.to_string(),
                    is_critical: false,
                });
                
                if repair {
                    batch.remove(TX_INDEX_TREE, &key)?;
                }
            }
            
            indices_checked += 1;
            result.items_checked += 1;
        }
        
        // Execute repairs if needed
        if repair && !batch.is_empty() {
            self.execute_batch(batch)?;
        }
        
        Ok(())
    }
    
    /// Verify block-transaction references
    fn verify_block_transactions(
        &self,
        result: &mut IntegrityCheckResult,
        repair: bool,
    ) -> Result<(), StorageError> {
        let mut blocks_checked = 0;
        let mut batch = self.create_batch();
        
        for item in self.blocks.iter() {
            let (key, value) = item?;
            
            // Deserialize block
            match bincode::deserialize::<Block>(&value) {
                Ok(block) => {
                    // Check if each transaction exists
                    for tx in block.transactions() {
                        let tx_hash = tx.hash();
                        
                        if self.transactions.get(&tx_hash)?.is_none() {
                            result.issues.push(IntegrityIssue {
                                issue_type: IntegrityIssueType::BrokenReference,
                                description: format!(
                                    "Block {} references non-existent transaction {}",
                                    hex::encode(&key[0..4]),
                                    hex::encode(&tx_hash[0..4])
                                ),
                                key: Some(key.to_vec()),
                                tree: BLOCKS_TREE.to_string(),
                                is_critical: true,
                            });
                            
                            if repair {
                                // Store missing transaction
                                let tx_data = bincode::serialize(tx)?;
                                batch.insert(TXNS_TREE, &tx_hash, tx_data)?;
                            }
                        }
                    }
                }
                Err(e) => {
                    result.issues.push(IntegrityIssue {
                        issue_type: IntegrityIssueType::InvalidFormat,
                        description: format!("Failed to deserialize block: {}", e),
                        key: Some(key.to_vec()),
                        tree: BLOCKS_TREE.to_string(),
                        is_critical: true,
                    });
                }
            }
            
            blocks_checked += 1;
            result.items_checked += 1;
        }
        
        // Execute repairs if needed
        if repair && !batch.is_empty() {
            self.execute_batch(batch)?;
        }
        
        Ok(())
    }
    
    /// Verify all transactions
    fn verify_transactions(
        &self,
        result: &mut IntegrityCheckResult,
        repair: bool,
    ) -> Result<(), StorageError> {
        let mut transactions_checked = 0;
        let mut batch = self.create_batch();
        
        for item in self.transactions.iter() {
            let (key, value) = item?;
            
            // Try to deserialize transaction
            match bincode::deserialize::<Transaction>(&value) {
                Ok(tx) => {
                    // Verify hash matches key
                    let computed_hash = tx.hash();
                    let mut stored_hash = [0u8; 32];
                    
                    if key.len() == 32 {
                        stored_hash.copy_from_slice(&key);
                        
                        if computed_hash != stored_hash {
                            result.issues.push(IntegrityIssue {
                                issue_type: IntegrityIssueType::HashMismatch,
                                description: format!(
                                    "Transaction hash mismatch. Stored: {}, Computed: {}",
                                    hex::encode(&stored_hash[0..4]),
                                    hex::encode(&computed_hash[0..4])
                                ),
                                key: Some(key.to_vec()),
                                tree: TXNS_TREE.to_string(),
                                is_critical: true,
                            });
                        }
                    } else {
                        result.issues.push(IntegrityIssue {
                            issue_type: IntegrityIssueType::InvalidFormat,
                            description: "Transaction key is not 32 bytes".to_string(),
                            key: Some(key.to_vec()),
                            tree: TXNS_TREE.to_string(),
                            is_critical: false,
                        });
                        
                        if repair {
                            // Remove invalid transaction and store with correct key
                            batch.remove(TXNS_TREE, &key)?;
                            batch.insert(TXNS_TREE, &computed_hash, &value)?;
                        }
                    }
                }
                Err(e) => {
                    result.issues.push(IntegrityIssue {
                        issue_type: IntegrityIssueType::InvalidFormat,
                        description: format!("Failed to deserialize transaction: {}", e),
                        key: Some(key.to_vec()),
                        tree: TXNS_TREE.to_string(),
                        is_critical: true,
                    });
                    
                    if repair {
                        batch.remove(TXNS_TREE, &key)?;
                    }
                }
            }
            
            transactions_checked += 1;
            result.items_checked += 1;
        }
        
        // Execute repairs if needed
        if repair && !batch.is_empty() {
            self.execute_batch(batch)?;
        }
        
        Ok(())
    }
    
    /// Verify UTXO set integrity
    fn verify_utxo_set(
        &self,
        result: &mut IntegrityCheckResult,
        repair: bool,
    ) -> Result<(), StorageError> {
        let mut utxos_checked = 0;
        let mut batch = self.create_batch();
        
        for item in self.utxos.iter() {
            let (key, value) = item?;
            
            // Try to parse UTXO key (tx_hash + output_index)
            if key.len() < 36 {
                result.issues.push(IntegrityIssue {
                    issue_type: IntegrityIssueType::InvalidFormat,
                    description: "UTXO key is invalid (too short)".to_string(),
                    key: Some(key.to_vec()),
                    tree: UTXO_TREE.to_string(),
                    is_critical: false,
                });
                
                if repair {
                    batch.remove(UTXO_TREE, &key)?;
                }
                continue;
            }
            
            // Extract tx_hash from key
            let mut tx_hash = [0u8; 32];
            tx_hash.copy_from_slice(&key[0..32]);
            
            // Check if the transaction exists
            if self.transactions.get(&tx_hash)?.is_none() {
                result.issues.push(IntegrityIssue {
                    issue_type: IntegrityIssueType::BrokenReference,
                    description: format!(
                        "UTXO references non-existent transaction: {}",
                        hex::encode(&tx_hash[0..4])
                    ),
                    key: Some(key.to_vec()),
                    tree: UTXO_TREE.to_string(),
                    is_critical: false,
                });
                
                if repair {
                    batch.remove(UTXO_TREE, &key)?;
                }
            }
            
            utxos_checked += 1;
            result.items_checked += 1;
        }
        
        // Execute repairs if needed
        if repair && !batch.is_empty() {
            self.execute_batch(batch)?;
        }
        
        Ok(())
    }
    
    /// Perform deep cross-reference verification
    fn verify_cross_references(
        &self,
        result: &mut IntegrityCheckResult,
        repair: bool,
    ) -> Result<(), StorageError> {
        // This is the most comprehensive check that verifies relationships
        // between different components of the database
        
        // 1. Verify block chain consistency (each block references a valid parent)
        self.verify_blockchain_consistency(result, repair)?;
        
        // 2. Verify UTXO set completeness (all UTXOs exist in their transactions)
        self.verify_utxo_completeness(result, repair)?;
        
        // 3. Verify best chain is valid
        self.verify_best_chain(result, repair)?;
        
        Ok(())
    }
    
    /// Verify that blocks form a valid chain
    fn verify_blockchain_consistency(
        &self,
        result: &mut IntegrityCheckResult,
        repair: bool,
    ) -> Result<(), StorageError> {
        let mut blocks_checked = 0;
        let mut orphaned_blocks = Vec::new();
        
        for item in self.blocks.iter() {
            let (key, value) = item?;
            
            if key.len() != 32 {
                continue;
            }
            
            match bincode::deserialize::<Block>(&value) {
                Ok(block) => {
                    // Skip genesis block (has no parent)
                    let prev_hash = block.prev_block_hash();
                    let is_genesis = prev_hash == [0u8; 32];
                    
                    if !is_genesis {
                        // Check if parent exists
                        if self.blocks.get(&prev_hash)?.is_none() {
                            result.issues.push(IntegrityIssue {
                                issue_type: IntegrityIssueType::BrokenReference,
                                description: format!(
                                    "Block {} references non-existent parent {}",
                                    hex::encode(&key[0..4]),
                                    hex::encode(&prev_hash[0..4])
                                ),
                                key: Some(key.to_vec()),
                                tree: BLOCKS_TREE.to_string(),
                                is_critical: false,
                            });
                            
                            orphaned_blocks.push(key.to_vec());
                        }
                    }
                }
                Err(_) => {
                    // Already handled in transaction verification
                }
            }
            
            blocks_checked += 1;
            result.items_checked += 1;
        }
        
        // If repairing, move orphaned blocks to the pending_blocks tree
        if repair && !orphaned_blocks.is_empty() {
            let mut batch = self.create_batch();
            
            for orphan_key in orphaned_blocks {
                if let Some(block_data) = self.blocks.get(&orphan_key)? {
                    // Create metadata for the pending block
                    let mut block_hash = [0u8; 32];
                    block_hash.copy_from_slice(&orphan_key);
                    
                    // Store as pending block
                    batch.insert(PENDING_BLOCKS_TREE, &orphan_key, &block_data)?;
                    
                    // Create metadata
                    let metadata = PendingBlockMetadata::new(block_hash, None, None);
                    let meta_data = bincode::serialize(&metadata)?;
                    batch.insert(PENDING_BLOCKS_META_TREE, &orphan_key, meta_data)?;
                    
                    // Remove from main blocks tree
                    batch.remove(BLOCKS_TREE, &orphan_key)?;
                }
            }
            
            // Execute batch
            self.execute_batch(batch)?;
        }
        
        Ok(())
    }
    
    /// Verify that all UTXOs are valid outputs in their transactions
    fn verify_utxo_completeness(
        &self,
        result: &mut IntegrityCheckResult,
        repair: bool,
    ) -> Result<(), StorageError> {
        let mut utxos_checked = 0;
        let mut invalid_utxos = Vec::new();
        
        for item in self.utxos.iter() {
            let (key, _) = item?;
            
            if key.len() < 36 {
                continue;
            }
            
            // Extract transaction hash and output index
            let mut tx_hash = [0u8; 32];
            tx_hash.copy_from_slice(&key[0..32]);
            let mut index_bytes = [0u8; 4];
            index_bytes.copy_from_slice(&key[32..36]);
            let output_index = u32::from_be_bytes(index_bytes);
            
            // Check if transaction exists and has this output
            match self.get_transaction(&tx_hash)? {
                Some(tx) => {
                    let outputs = tx.outputs();
                    if output_index as usize >= outputs.len() {
                        result.issues.push(IntegrityIssue {
                            issue_type: IntegrityIssueType::BrokenReference,
                            description: format!(
                                "UTXO references non-existent output {} in transaction {}",
                                output_index,
                                hex::encode(&tx_hash[0..4])
                            ),
                            key: Some(key.to_vec()),
                            tree: UTXO_TREE.to_string(),
                            is_critical: false,
                        });
                        
                        invalid_utxos.push(key.to_vec());
                    }
                }
                None => {
                    // Already handled in UTXO set verification
                    invalid_utxos.push(key.to_vec());
                }
            }
            
            utxos_checked += 1;
            result.items_checked += 1;
        }
        
        // Remove invalid UTXOs if repairing
        if repair && !invalid_utxos.is_empty() {
            let mut batch = self.create_batch();
            
            for utxo_key in invalid_utxos {
                batch.remove(UTXO_TREE, &utxo_key)?;
            }
            
            self.execute_batch(batch)?;
        }
        
        Ok(())
    }
    
    /// Verify that the best chain is valid
    fn verify_best_chain(
        &self,
        result: &mut IntegrityCheckResult,
        repair: bool,
    ) -> Result<(), StorageError> {
        // Get current best block hash
        if let Some(best_hash_data) = self.metadata.get("best_block_hash".as_bytes())? {
            let mut best_hash = [0u8; 32];
            if best_hash_data.len() == 32 {
                best_hash.copy_from_slice(&best_hash_data);
                
                // Check if this block exists
                if self.blocks.get(&best_hash)?.is_none() {
                    result.issues.push(IntegrityIssue {
                        issue_type: IntegrityIssueType::BrokenReference,
                        description: format!(
                            "Best block hash references non-existent block {}",
                            hex::encode(&best_hash[0..4])
                        ),
                        key: Some("best_block_hash".as_bytes().to_vec()),
                        tree: METADATA_TREE.to_string(),
                        is_critical: true,
                    });
                    
                    if repair {
                        // Find highest valid block and use that as best hash
                        if let Some(height_data) = self.metadata.get("height".as_bytes())? {
                            if height_data.len() == 8 {
                                let mut height_bytes = [0u8; 8];
                                height_bytes.copy_from_slice(&height_data);
                                let height = u64::from_be_bytes(height_bytes);
                                
                                // Try to find a valid block at this height or lower
                                for h in (0..=height).rev() {
                                    let height_key = h.to_be_bytes();
                                    if let Some(hash_data) = self.block_height_index.get(&height_key)? {
                                        if hash_data.len() == 32 {
                                            // Check if this block exists
                                            if self.blocks.get(&hash_data)?.is_some() {
                                                // Found a valid block, use it as best hash
                                                self.metadata.insert("best_block_hash".as_bytes(), &hash_data)?;
                                                
                                                // Update height if needed
                                                if h != height {
                                                    self.metadata.insert("height".as_bytes(), &h.to_be_bytes())?;
                                                }
                                                
                                                break;
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        
        result.items_checked += 1;
        Ok(())
    }
    
    /// Get the latest integrity check result
    pub fn get_latest_integrity_check(&self) -> Result<Option<IntegrityCheckResult>, StorageError> {
        if let Some(data) = self.metadata.get("latest_integrity_check".as_bytes())? {
            let result: IntegrityCheckResult = bincode::deserialize(&data)?;
            Ok(Some(result))
        } else {
            Ok(None)
        }
    }
    
    /// Store integrity check result
    pub fn store_integrity_check_result(&self, result: &IntegrityCheckResult) -> Result<(), StorageError> {
        let data = bincode::serialize(result)?;
        self.metadata.insert("latest_integrity_check".as_bytes(), &data)?;
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
    #[error("Key not found: {0}")]
    KeyNotFound(String),
    #[error("Checkpoint error: {0}")]
    CheckpointError(String),
    #[error("Pending block expired")]
    PendingBlockExpired,
    #[error("Pending block invalid")]
    PendingBlockInvalid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntegrityCheckResult {
    /// Timestamp of the check
    pub timestamp: u64,
    /// Whether the check passed
    pub passed: bool,
    /// Level of the check
    pub check_level: IntegrityCheckLevel,
    /// Detected issues (if any)
    pub issues: Vec<IntegrityIssue>,
    /// Duration of the check in milliseconds
    pub duration_ms: u64,
    /// Number of items verified
    pub items_checked: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum IntegrityCheckLevel {
    /// Fast check of critical structures only
    Quick,
    /// Standard check of important data
    Standard,
    /// Comprehensive check of all data
    Comprehensive,
    /// Deep verification with cross-references
    Deep,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntegrityIssue {
    /// Type of issue
    pub issue_type: IntegrityIssueType,
    /// Description of the issue
    pub description: String,
    /// Affected key (if applicable)
    pub key: Option<Vec<u8>>,
    /// Tree where the issue was found
    pub tree: String,
    /// Whether the issue is critical
    pub is_critical: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum IntegrityIssueType {
    /// Missing item that should exist
    MissingItem,
    /// Invalid data format
    InvalidFormat,
    /// Corrupted data
    Corrupted,
    /// Hash mismatch
    HashMismatch,
    /// Index inconsistency
    IndexInconsistency,
    /// Reference to non-existent item
    BrokenReference,
    /// Other issue
    Other,
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
        db.store_pending_block(&block_hash, &block_data, None, None, None)?;
        
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