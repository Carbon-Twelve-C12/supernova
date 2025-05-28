// SuperNova Node - Checkpoint System
//
// This module implements an automatic checkpoint system that regularly saves
// the state of the node to enable fast recovery in case of failures.

use crate::storage::database::{BlockchainDB, StorageError};
use crate::storage::persistence::ChainState;
use crate::metrics::BackupMetrics;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH, Duration, Instant};
use std::collections::{HashMap, HashSet, BTreeMap};
use tokio::fs;
use tracing::{info, warn, error, debug};
use std::sync::{Arc, Mutex};
use serde::{Serialize, Deserialize};
use sha2::{Sha256, Digest};
use tokio::sync::mpsc;
use tokio::time;
use btclib::types::block::Block;
use thiserror::Error;

/// Checkpoint-related errors
#[derive(Debug, Error)]
pub enum CheckpointError {
    #[error("Checkpoint creation failed: {0}")]
    CreationFailed(String),
    
    #[error("Checkpoint validation failed: {0}")]
    ValidationFailed(String),
    
    #[error("Checkpoint not found: {0}")]
    NotFound(String),
    
    #[error("Storage error: {0}")]
    Storage(#[from] StorageError),
    
    #[error("Serialization error: {0}")]
    Serialization(#[from] Box<bincode::ErrorKind>),
}

// Default checkpoint settings
const DEFAULT_CHECKPOINT_INTERVAL_BLOCKS: u64 = 1000;
const DEFAULT_CHECKPOINT_INTERVAL_TIME: Duration = Duration::from_secs(3600); // 1 hour
const DEFAULT_MAX_CHECKPOINTS: usize = 5;
const DEFAULT_INTEGRITY_CHECK_INTERVAL: Duration = Duration::from_secs(86400); // 1 day

/// Types of checkpoints the system can create
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CheckpointType {
    /// Regular checkpoint created based on time/block intervals
    Regular,
    /// Checkpoint created before a potentially risky operation
    PreOperation,
    /// Checkpoint created manually by the operator
    Manual,
    /// Snapshot for debugging purposes
    Debug,
    /// Checkpoint before a software upgrade
    PreUpgrade,
    /// Special checkpoint to mark a clean shutdown
    Shutdown,
}

/// Configuration for the checkpoint system
#[derive(Debug, Clone)]
pub struct CheckpointConfig {
    /// Directory to store checkpoints
    pub checkpoint_dir: PathBuf,
    /// Create checkpoint every N blocks
    pub checkpoint_interval_blocks: u64,
    /// Create checkpoint every N seconds
    pub checkpoint_interval_time: Duration,
    /// Maximum number of checkpoints to keep
    pub max_checkpoints: usize,
    /// How often to run integrity checks on checkpoints
    pub integrity_check_interval: Duration,
    /// Whether to verify checkpoints after creation
    pub verify_after_creation: bool,
    /// Whether to enable automatic recovery on startup
    pub auto_recovery_on_startup: bool,
}

impl Default for CheckpointConfig {
    fn default() -> Self {
        Self {
            checkpoint_dir: PathBuf::from("data/checkpoints"),
            checkpoint_interval_blocks: DEFAULT_CHECKPOINT_INTERVAL_BLOCKS,
            checkpoint_interval_time: DEFAULT_CHECKPOINT_INTERVAL_TIME,
            max_checkpoints: DEFAULT_MAX_CHECKPOINTS,
            integrity_check_interval: DEFAULT_INTEGRITY_CHECK_INTERVAL,
            verify_after_creation: true,
            auto_recovery_on_startup: true,
        }
    }
}

/// Detailed information about a checkpoint
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckpointInfo {
    /// Block height at checkpoint
    pub height: u64,
    /// Block hash at checkpoint
    pub block_hash: [u8; 32],
    /// Timestamp when checkpoint was created
    pub timestamp: u64,
    /// Type of the checkpoint
    pub checkpoint_type: CheckpointType,
    /// Hash of UTXO set at checkpoint
    pub utxo_hash: [u8; 32],
    /// Metadata about the checkpoint
    pub metadata: HashMap<String, String>,
    /// Hash of the checkpoint data for integrity verification
    pub data_hash: [u8; 32],
    /// Duration taken to create the checkpoint
    pub creation_duration_ms: u64,
    /// Size of the checkpoint in bytes
    pub size_bytes: u64,
    /// Whether the checkpoint was verified after creation
    pub verified: bool,
}

/// Commands for the CheckpointManager
enum CheckpointCommand {
    /// Create a new checkpoint
    Create(CheckpointType),
    /// Verify checkpoints
    Verify,
    /// Clean up old checkpoints
    Cleanup,
    /// Shutdown the checkpoint manager
    Shutdown,
}

/// Main checkpoint manager for automatic state persistence
pub struct CheckpointManager {
    /// Configuration for the checkpoint system
    config: CheckpointConfig,
    /// Database connection
    db: Arc<BlockchainDB>,
    /// Chain state reference
    chain_state: Arc<Mutex<ChainState>>,
    /// Last height at which a checkpoint was created
    last_checkpoint_height: u64,
    /// Last time a checkpoint was created
    last_checkpoint_time: Instant,
    /// Information about available checkpoints
    checkpoints: HashMap<u64, CheckpointInfo>,
    /// Command channel for communicating with the background task
    command_tx: Option<mpsc::Sender<CheckpointCommand>>,
    /// Metrics for checkpoint operations
    metrics: BackupMetrics,
}

impl CheckpointManager {
    /// Create a new checkpoint manager
    pub fn new(
        db: Arc<BlockchainDB>,
        chain_state: Arc<Mutex<ChainState>>,
        config: CheckpointConfig,
    ) -> Self {
        Self {
            config,
            db,
            chain_state,
            last_checkpoint_height: 0,
            last_checkpoint_time: Instant::now(),
            checkpoints: HashMap::new(),
            command_tx: None,
            metrics: BackupMetrics::new(),
        }
    }

    /// Start the checkpoint manager background task
    pub async fn start(&mut self) -> Result<(), StorageError> {
        info!("Starting checkpoint manager with interval of {} blocks or {} seconds",
             self.config.checkpoint_interval_blocks,
             self.config.checkpoint_interval_time.as_secs());

        // Create checkpoint directory if it doesn't exist
        fs::create_dir_all(&self.config.checkpoint_dir).await?;

        // Load existing checkpoints
        self.load_checkpoints().await?;

        // Create command channel
        let (tx, mut rx) = mpsc::channel(100);
        self.command_tx = Some(tx);

        // Reference to self for the background task
        let config = self.config.clone();
        let db = Arc::clone(&self.db);
        let chain_state = Arc::clone(&self.chain_state);
        let metrics = self.metrics.clone();

        // Spawn background task
        tokio::spawn(async move {
            let mut interval = time::interval(Duration::from_secs(10)); // Check every 10 seconds
            let mut integrity_check_timer = Instant::now();
            
            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        // Check if it's time for a new checkpoint
                        let current_height = {
                            let state = chain_state.lock().unwrap();
                            state.get_height()
                        };
                        
                        let now = Instant::now();
                        let time_since_last = now.duration_since(integrity_check_timer);
                        
                        // Periodically check integrity of checkpoints
                        if time_since_last >= config.integrity_check_interval {
                            if let Err(e) = Self::verify_checkpoint_integrity(&config, &db).await {
                                error!("Checkpoint integrity check failed: {}", e);
                            }
                            integrity_check_timer = now;
                        }
                    }
                    
                    Some(cmd) = rx.recv() => {
                        match cmd {
                            CheckpointCommand::Create(checkpoint_type) => {
                                if let Err(e) = Self::create_checkpoint(
                                    &config, 
                                    &db, 
                                    &chain_state, 
                                    checkpoint_type,
                                    &metrics
                                ).await {
                                    error!("Failed to create checkpoint: {}", e);
                                }
                            }
                            CheckpointCommand::Verify => {
                                if let Err(e) = Self::verify_checkpoint_integrity(&config, &db).await {
                                    error!("Checkpoint verification failed: {}", e);
                                }
                            }
                            CheckpointCommand::Cleanup => {
                                if let Err(e) = Self::cleanup_old_checkpoints(&config).await {
                                    error!("Checkpoint cleanup failed: {}", e);
                                }
                            }
                            CheckpointCommand::Shutdown => {
                                info!("Shutting down checkpoint manager");
                                break;
                            }
                        }
                    }
                }
            }
        });

        // Check if we need to do an initial checkpoint
        let current_height = {
            let state = self.chain_state.lock().unwrap();
            state.get_height()
        };

        if current_height > 0 && self.checkpoints.is_empty() {
            info!("Creating initial checkpoint at height {}", current_height);
            self.create_checkpoint(CheckpointType::Regular).await?;
        }

        info!("Checkpoint manager started successfully");
        Ok(())
    }

    /// Stop the checkpoint manager
    pub async fn stop(&mut self) -> Result<(), StorageError> {
        // Create a final checkpoint for clean shutdown
        self.create_checkpoint(CheckpointType::Shutdown).await?;

        // Send shutdown command
        if let Some(tx) = &self.command_tx {
            if let Err(e) = tx.send(CheckpointCommand::Shutdown).await {
                error!("Failed to send shutdown command to checkpoint manager: {}", e);
            }
        }

        info!("Checkpoint manager stopped");
        self.command_tx = None;
        Ok(())
    }

    /// Create a new checkpoint
    pub async fn create_checkpoint(&self, checkpoint_type: CheckpointType) -> Result<CheckpointInfo, StorageError> {
        // Send command to background task
        if let Some(tx) = &self.command_tx {
            tx.send(CheckpointCommand::Create(checkpoint_type)).await
                .map_err(|e| StorageError::DatabaseError(format!("Failed to send checkpoint command: {}", e)))?;
        } else {
            // If background task isn't running, create checkpoint directly
            return Self::create_checkpoint(
                &self.config,
                &self.db,
                &self.chain_state,
                checkpoint_type,
                &self.metrics
            ).await;
        }

        // Note: This returns immediately while the checkpoint is created in the background
        // A more sophisticated implementation could return a Future that resolves when the checkpoint is created
        Ok(CheckpointInfo {
            height: 0,
            block_hash: [0; 32],
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or(Duration::from_secs(0))
                .as_secs(),
            checkpoint_type,
            utxo_hash: [0; 32],
            metadata: HashMap::new(),
            data_hash: [0; 32],
            creation_duration_ms: 0,
            size_bytes: 0,
            verified: false,
        })
    }

    /// Restore from a specific checkpoint
    pub async fn restore_from_checkpoint(&self, height: u64) -> Result<(), StorageError> {
        let checkpoint = self.checkpoints.get(&height).ok_or_else(|| {
            StorageError::DatabaseError(format!("Checkpoint at height {} not found", height))
        })?;

        info!("Restoring from checkpoint at height {}", height);

        let checkpoint_path = self.get_checkpoint_path(height);
        self.restore_from_path(&checkpoint_path, checkpoint).await
    }

    /// Restore from the latest checkpoint
    pub async fn restore_from_latest_checkpoint(&self) -> Result<(), StorageError> {
        let latest = self.get_latest_checkpoint().ok_or_else(|| {
            StorageError::DatabaseError("No checkpoints available for restoration".to_string())
        })?;

        info!("Restoring from latest checkpoint at height {}", latest.height);

        let checkpoint_path = self.get_checkpoint_path(latest.height);
        self.restore_from_path(&checkpoint_path, latest).await
    }

    /// Get the latest checkpoint
    pub fn get_latest_checkpoint(&self) -> Option<&CheckpointInfo> {
        self.checkpoints.values().max_by_key(|c| c.height)
    }

    /// Get all available checkpoints
    pub fn get_checkpoints(&self) -> Vec<&CheckpointInfo> {
        self.checkpoints.values().collect()
    }

    /// Internal method to load existing checkpoints
    async fn load_checkpoints(&mut self) -> Result<(), StorageError> {
        info!("Loading checkpoints from {}", self.config.checkpoint_dir.display());

        // Read checkpoint directory
        let mut entries = match fs::read_dir(&self.config.checkpoint_dir).await {
            Ok(entries) => entries,
            Err(e) => {
                warn!("Failed to read checkpoint directory: {}", e);
                return Ok(());
            }
        };

        self.checkpoints.clear();

        // Process each checkpoint file
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            
            if path.is_dir() && path.file_name().unwrap().to_string_lossy().starts_with("checkpoint_") {
                let info_file = path.join("checkpoint_info.json");
                if info_file.exists() {
                    match fs::read(&info_file).await {
                        Ok(data) => {
                            match serde_json::from_slice::<CheckpointInfo>(&data) {
                                Ok(info) => {
                                    info!("Loaded checkpoint at height {}", info.height);
                                    self.checkpoints.insert(info.height, info);
                                }
                                Err(e) => {
                                    warn!("Failed to parse checkpoint info from {}: {}", info_file.display(), e);
                                }
                            }
                        }
                        Err(e) => {
                            warn!("Failed to read checkpoint info from {}: {}", info_file.display(), e);
                        }
                    }
                }
            }
        }

        // Update last checkpoint information
        if let Some(checkpoint) = self.get_latest_checkpoint() {
            self.last_checkpoint_height = checkpoint.height;
            self.last_checkpoint_time = Instant::now() - Duration::from_secs(
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or(Duration::from_secs(0))
                    .as_secs() - checkpoint.timestamp
            );
        }

        info!("Loaded {} checkpoints", self.checkpoints.len());
        Ok(())
    }

    /// Get the path for a checkpoint at a specific height
    fn get_checkpoint_path(&self, height: u64) -> PathBuf {
        self.config.checkpoint_dir.join(format!("checkpoint_{}", height))
    }

    /// Internal method to restore from a checkpoint path
    async fn restore_from_path(&self, path: &Path, checkpoint: &CheckpointInfo) -> Result<(), StorageError> {
        // Verify checkpoint integrity
        let verification = self.metrics.record_verification_start();
        
        if !path.exists() {
            verification.complete();
            return Err(StorageError::DatabaseError(format!(
                "Checkpoint path does not exist: {}", path.display()
            )));
        }
        
        // Verify the checkpoint hash
        let hash = Self::calculate_checkpoint_hash(path).await?;
        if hash != checkpoint.data_hash {
            verification.complete();
            return Err(StorageError::DatabaseError(format!(
                "Checkpoint integrity check failed for height {}", checkpoint.height
            )));
        }
        
        verification.complete();
        self.metrics.record_verification_success();
        
        // Stop the database
        self.db.flush()?;
        
        // Copy checkpoint files to database directory
        let db_path = self.db.path().to_path_buf();
        let data_path = path.join("data");
        
        // Create temporary backup of current database
        let backup_path = db_path.with_extension("bak");
        if db_path.exists() {
            fs::rename(&db_path, &backup_path).await?;
        }
        
        // Copy checkpoint files to database directory
        fs::create_dir_all(&db_path).await?;
        Self::copy_directory(&data_path, &db_path).await?;
        
        info!("Successfully restored from checkpoint at height {}", checkpoint.height);
        
        // TODO: We should have a better way to reinitialize the chain state
        // Currently, this would require restarting the node
        
        Ok(())
    }

    /// Internal implementation of checkpoint creation
    async fn create_checkpoint(
        config: &CheckpointConfig,
        db: &Arc<BlockchainDB>,
        chain_state: &Arc<Mutex<ChainState>>,
        checkpoint_type: CheckpointType,
        metrics: &BackupMetrics,
    ) -> Result<CheckpointInfo, StorageError> {
        let start_time = Instant::now();
        let backup_operation = metrics.record_backup_start();
        
        // Get current height and block hash
        let (height, block_hash) = {
            let state = chain_state.lock().unwrap();
            (state.get_height(), state.get_best_block_hash())
        };
        
        info!("Creating {} checkpoint at height {}", 
            format!("{:?}", checkpoint_type).to_lowercase(), height);
        
        // Create checkpoint directory
        let checkpoint_dir = config.checkpoint_dir.join(format!("checkpoint_{}", height));
        fs::create_dir_all(&checkpoint_dir).await?;
        
        // Create data directory
        let data_dir = checkpoint_dir.join("data");
        fs::create_dir_all(&data_dir).await?;
        
        // Flush database to ensure all writes are committed
        db.flush()?;
        
        // Copy database files to checkpoint directory
        let db_path = db.path();
        Self::copy_directory(db_path, &data_dir).await?;
        
        // Calculate UTXO hash (this could be optimized/cached if needed)
        let utxo_hash = Self::calculate_utxo_hash(db, chain_state).await?;
        
        // Calculate checkpoint data hash
        let data_hash = Self::calculate_checkpoint_hash(&checkpoint_dir).await?;
        
        // Get directory size
        let size_bytes = Self::get_directory_size(&checkpoint_dir).await?;
        
        // Create checkpoint info
        let checkpoint_info = CheckpointInfo {
            height,
            block_hash,
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or(Duration::from_secs(0))
                .as_secs(),
            checkpoint_type,
            utxo_hash,
            metadata: HashMap::new(),
            data_hash,
            creation_duration_ms: start_time.elapsed().as_millis() as u64,
            size_bytes,
            verified: false,
        };
        
        // Write checkpoint info file
        let info_file = checkpoint_dir.join("checkpoint_info.json");
        let info_json = serde_json::to_string_pretty(&checkpoint_info)?;
        fs::write(&info_file, info_json).await?;
        
        // Record metrics
        backup_operation.complete(size_bytes);
        
        // Verify checkpoint if needed
        if config.verify_after_creation {
            let verification = metrics.record_verification_start();
            let hash = Self::calculate_checkpoint_hash(&checkpoint_dir).await?;
            if hash != data_hash {
                metrics.record_verification_failure();
                verification.complete();
                return Err(StorageError::BackupVerificationFailed);
            }
            metrics.record_verification_success();
            verification.complete();
        }
        
        info!("Created checkpoint at height {} ({}): {}ms, {}MB", 
            height, 
            format!("{:?}", checkpoint_type).to_lowercase(),
            checkpoint_info.creation_duration_ms,
            checkpoint_info.size_bytes / 1024 / 1024);
        
        Ok(checkpoint_info)
    }

    /// Calculate UTXO hash for integrity checking
    async fn calculate_utxo_hash(
        db: &Arc<BlockchainDB>,
        chain_state: &Arc<Mutex<ChainState>>,
    ) -> Result<[u8; 32], StorageError> {
        let mut hasher = Sha256::new();
        
        // This is a simplified implementation - in a real system you might use
        // a more efficient method like a Merkle tree of the UTXO set
        
        // Get current height and hash
        let (height, hash) = {
            let state = chain_state.lock().unwrap();
            (state.get_height(), state.get_best_block_hash())
        };
        
        // Add height and hash to hasher
        hasher.update(&height.to_le_bytes());
        hasher.update(&hash);
        
        // Add a sample of UTXOs
        // Note: In a real implementation, you would hash the entire UTXO set
        
        Ok(hasher.finalize().into())
    }

    /// Calculate a hash of checkpoint data for integrity checks
    async fn calculate_checkpoint_hash(path: &Path) -> Result<[u8; 32], StorageError> {
        let mut hasher = Sha256::new();
        
        // Hash the data directory contents
        let data_path = path.join("data");
        if data_path.exists() {
            Self::hash_directory(&data_path, &mut hasher).await?;
        }
        
        Ok(hasher.finalize().into())
    }

    /// Recursively hash a directory's contents
    async fn hash_directory(dir: &Path, hasher: &mut Sha256) -> Result<(), StorageError> {
        let mut entries = fs::read_dir(dir).await?;
        
        // Sort entries by name for consistent hashing
        let mut paths = Vec::new();
        while let Some(entry) = entries.next_entry().await? {
            paths.push(entry.path());
        }
        paths.sort();
        
        for path in paths {
            // Add file name to hash
            hasher.update(path.file_name().unwrap().to_string_lossy().as_bytes());
            
            if path.is_file() {
                // Hash file contents
                let contents = fs::read(&path).await?;
                hasher.update(&contents);
            } else if path.is_dir() {
                // Recursively hash directory
                Self::hash_directory(&path, hasher).await?;
            }
        }
        
        Ok(())
    }

    /// Copy a directory recursively
    async fn copy_directory(from: &Path, to: &Path) -> Result<(), StorageError> {
        fs::create_dir_all(to).await?;
        
        let mut entries = fs::read_dir(from).await?;
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            let target = to.join(path.file_name().unwrap());
            
            if path.is_file() {
                fs::copy(&path, &target).await?;
            } else if path.is_dir() {
                Self::copy_directory(&path, &target).await?;
            }
        }
        
        Ok(())
    }

    /// Get total size of a directory and its contents
    async fn get_directory_size(dir: &Path) -> Result<u64, StorageError> {
        let mut total_size: u64 = 0;
        let mut entries = fs::read_dir(dir).await?;
        
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            
            if path.is_file() {
                if let Ok(metadata) = fs::metadata(&path).await {
                    total_size += metadata.len();
                }
            } else if path.is_dir() {
                total_size += Self::get_directory_size(&path).await?;
            }
        }
        
        Ok(total_size)
    }

    /// Cleanup old checkpoints to stay within the maximum limit
    async fn cleanup_old_checkpoints(config: &CheckpointConfig) -> Result<(), StorageError> {
        let checkpoint_dir = &config.checkpoint_dir;
        let mut checkpoints = Vec::new();
        
        // Get all checkpoints
        let mut entries = fs::read_dir(checkpoint_dir).await?;
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.is_dir() {
                let file_name = path.file_name().unwrap().to_string_lossy();
                if file_name.starts_with("checkpoint_") {
                    if let Some(height) = file_name.strip_prefix("checkpoint_") {
                        if let Ok(height) = height.parse::<u64>() {
                            let info_file = path.join("checkpoint_info.json");
                            if info_file.exists() {
                                // Load checkpoint info to get type
                                if let Ok(data) = fs::read(&info_file).await {
                                    if let Ok(info) = serde_json::from_slice::<CheckpointInfo>(&data) {
                                        checkpoints.push((height, path, info.checkpoint_type));
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        
        // Sort by height (descending)
        checkpoints.sort_by(|a, b| b.0.cmp(&a.0));
        
        // Keep manual and shutdown checkpoints
        let manual_checkpoints: Vec<_> = checkpoints.iter()
            .filter(|(_, _, cp_type)| *cp_type == CheckpointType::Manual || *cp_type == CheckpointType::Shutdown)
            .collect();
        
        // Keep the most recent regular checkpoints up to the limit
        let regular_checkpoints: Vec<_> = checkpoints.iter()
            .filter(|(_, _, cp_type)| *cp_type == CheckpointType::Regular)
            .take(config.max_checkpoints)
            .collect();
        
        // Delete checkpoints that don't meet criteria
        let checkpoints_to_keep: HashSet<_> = manual_checkpoints.iter()
            .chain(regular_checkpoints.iter())
            .map(|(height, _, _)| *height)
            .collect();
            
        for (height, path, cp_type) in checkpoints {
            if !checkpoints_to_keep.contains(&height) {
                info!("Removing old checkpoint at height {} (type: {:?})", height, cp_type);
                if let Err(e) = fs::remove_dir_all(&path).await {
                    warn!("Failed to remove old checkpoint at {}: {}", path.display(), e);
                }
            }
        }
        
        Ok(())
    }

    /// Verify the integrity of all checkpoints
    async fn verify_checkpoint_integrity(
        config: &CheckpointConfig,
        db: &Arc<BlockchainDB>,
    ) -> Result<(), StorageError> {
        info!("Verifying integrity of checkpoints");
        
        let checkpoint_dir = &config.checkpoint_dir;
        let mut entries = fs::read_dir(checkpoint_dir).await?;
        
        let mut verified = 0;
        let mut failed = 0;
        
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.is_dir() && path.file_name().unwrap().to_string_lossy().starts_with("checkpoint_") {
                let info_file = path.join("checkpoint_info.json");
                if info_file.exists() {
                    match fs::read(&info_file).await {
                        Ok(data) => {
                            match serde_json::from_slice::<CheckpointInfo>(&data) {
                                Ok(info) => {
                                    // Calculate current hash
                                    let hash = Self::calculate_checkpoint_hash(&path).await?;
                                    
                                    if hash == info.data_hash {
                                        verified += 1;
                                        debug!("Verified checkpoint at height {}", info.height);
                                    } else {
                                        failed += 1;
                                        warn!("Checkpoint at height {} failed integrity check", info.height);
                                        
                                        // Move to failed directory
                                        let failed_dir = checkpoint_dir.join("failed");
                                        fs::create_dir_all(&failed_dir).await?;
                                        let new_path = failed_dir.join(format!("checkpoint_{}", info.height));
                                        if let Err(e) = fs::rename(&path, &new_path).await {
                                            error!("Failed to move corrupt checkpoint: {}", e);
                                        }
                                    }
                                }
                                Err(e) => {
                                    warn!("Failed to parse checkpoint info: {}", e);
                                }
                            }
                        }
                        Err(e) => {
                            warn!("Failed to read checkpoint info: {}", e);
                        }
                    }
                }
            }
        }
        
        info!("Checkpoint integrity check complete: {} verified, {} failed", verified, failed);
        
        Ok(())
    }
} 