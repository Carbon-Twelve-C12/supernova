use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use sha2::{Sha256, Digest};
use thiserror::Error;
use serde::{Serialize, Deserialize};

use crate::types::block::{Block, BlockHeader};
use crate::types::transaction::Transaction;
use crate::storage::chain_state::Checkpoint;

/// Error types for backup operations
#[derive(Debug, Error)]
pub enum BackupError {
    #[error("IO error: {0}")]
    IoError(#[from] io::Error),
    
    #[error("Invalid backup file: {0}")]
    InvalidBackup(String),
    
    #[error("Backup not found: {0}")]
    BackupNotFound(String),
    
    #[error("Integrity check failed: {0}")]
    IntegrityFailed(String),
    
    #[error("Serialization error: {0}")]
    SerializationError(String),
}

/// Result type for backup operations
pub type BackupResult<T> = Result<T, BackupError>;

/// Type of backup
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BackupType {
    /// Full backup of all blockchain data
    Full,
    
    /// Incremental backup since last full backup
    Incremental,
    
    /// UTXO set snapshot
    UtxoSnapshot,
    
    /// Chain state backup (with checkpoints only)
    ChainState,
}

/// Configuration for backup operations
#[derive(Debug, Clone)]
pub struct BackupConfig {
    /// Directory for backup storage
    pub backup_dir: PathBuf,
    
    /// Maximum number of backups to keep
    pub max_backups: usize,
    
    /// Interval for full backups (in blocks)
    pub full_backup_interval: u32,
    
    /// Interval for incremental backups (in blocks)
    pub incremental_backup_interval: u32,
    
    /// Whether to compress backups
    pub compress_backups: bool,
    
    /// Whether to encrypt backups (would require additional key management)
    pub encrypt_backups: bool,
}

impl Default for BackupConfig {
    fn default() -> Self {
        Self {
            backup_dir: PathBuf::from("backups"),
            max_backups: 10,
            full_backup_interval: 10_000,
            incremental_backup_interval: 1_000,
            compress_backups: true,
            encrypt_backups: false,
        }
    }
}

/// Metadata about a backup file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupMetadata {
    /// Type of backup
    pub backup_type: BackupType,
    
    /// Timestamp of backup creation
    pub timestamp: u64,
    
    /// Block height at backup time
    pub block_height: u32,
    
    /// Block hash at backup time
    pub block_hash: [u8; 32],
    
    /// SHA-256 hash of backup data (for integrity verification)
    pub data_hash: [u8; 32],
    
    /// Size of backup in bytes
    pub size: u64,
    
    /// Whether the backup is compressed
    pub compressed: bool,
    
    /// Whether the backup is encrypted
    pub encrypted: bool,
    
    /// For incremental backups, the backup ID of the base backup
    pub base_backup_id: Option<String>,
}

/// Reference to a backup file
#[derive(Debug, Clone)]
pub struct BackupReference {
    /// Unique ID of the backup
    pub id: String,
    
    /// Path to the backup file
    pub path: PathBuf,
    
    /// Metadata about the backup
    pub metadata: BackupMetadata,
}

/// Manages blockchain data backups and recovery
pub struct BackupManager {
    /// Configuration for backups
    config: BackupConfig,
    
    /// Cache of known backups
    backups: Arc<Mutex<HashMap<String, BackupReference>>>,
    
    /// Last successful backup height
    last_backup_height: Arc<Mutex<u32>>,
    
    /// Last full backup height
    last_full_backup_height: Arc<Mutex<u32>>,
}

impl BackupManager {
    /// Create a new backup manager
    pub fn new(config: BackupConfig) -> BackupResult<Self> {
        // Create backup directory if it doesn't exist
        fs::create_dir_all(&config.backup_dir)?;
        
        let manager = Self {
            config,
            backups: Arc::new(Mutex::new(HashMap::new())),
            last_backup_height: Arc::new(Mutex::new(0)),
            last_full_backup_height: Arc::new(Mutex::new(0)),
        };
        
        // Load existing backups
        manager.scan_backups()?;
        
        Ok(manager)
    }
    
    /// Scan the backup directory and load metadata
    fn scan_backups(&self) -> BackupResult<()> {
        let entries = fs::read_dir(&self.config.backup_dir)?;
        let mut backups = self.backups.lock().map_err(|_| {
            BackupError::InvalidBackup("Failed to acquire mutex".to_string())
        })?;
        
        // Clear existing cache
        backups.clear();
        
        // Find the latest backup heights
        let mut max_backup_height = 0;
        let mut max_full_backup_height = 0;
        
        for entry in entries {
            if let Ok(entry) = entry {
                if let Some(file_name) = entry.file_name().to_str() {
                    if file_name.ends_with(".backup") && file_name.starts_with("supernova-") {
                        if let Ok(backup) = self.load_backup_metadata(&entry.path()) {
                            // Update max heights
                            if backup.metadata.block_height > max_backup_height {
                                max_backup_height = backup.metadata.block_height;
                            }
                            
                            if backup.metadata.backup_type == BackupType::Full && 
                               backup.metadata.block_height > max_full_backup_height {
                                max_full_backup_height = backup.metadata.block_height;
                            }
                            
                            // Store in cache
                            backups.insert(backup.id.clone(), backup);
                        }
                    }
                }
            }
        }
        
        // Update last backup heights
        {
            let mut last_height = self.last_backup_height.lock().map_err(|_| {
                BackupError::InvalidBackup("Failed to acquire mutex".to_string())
            })?;
            *last_height = max_backup_height;
        }
        
        {
            let mut last_full_height = self.last_full_backup_height.lock().map_err(|_| {
                BackupError::InvalidBackup("Failed to acquire mutex".to_string())
            })?;
            *last_full_height = max_full_backup_height;
        }
        
        Ok(())
    }
    
    /// Create a new backup at the given block height
    pub fn create_backup(&self, height: u32, hash: [u8; 32], checkpoints: &[Checkpoint], data: &[u8]) -> BackupResult<BackupReference> {
        // Determine backup type
        let backup_type = self.determine_backup_type(height)?;
        
        // Generate a unique ID for the backup
        let backup_id = self.generate_backup_id(backup_type, height);
        
        // Get the base backup ID if incremental
        let base_backup_id = if backup_type == BackupType::Incremental {
            self.get_latest_full_backup().map(|b| b.id.clone())
        } else {
            None
        };
        
        // Calculate data hash for integrity verification
        let data_hash = self.calculate_hash(data);
        
        // Compress the data if configured
        let (processed_data, compressed) = if self.config.compress_backups {
            // In a real implementation, compress the data
            // For simplicity, we'll skip compression here
            (data.to_vec(), false)
        } else {
            (data.to_vec(), false)
        };
        
        // Create backup metadata
        let metadata = BackupMetadata {
            backup_type,
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            block_height: height,
            block_hash: hash,
            data_hash,
            size: processed_data.len() as u64,
            compressed,
            encrypted: false,
            base_backup_id,
        };
        
        // Create backup file path
        let backup_path = self.config.backup_dir.join(format!("{}.backup", backup_id));
        
        // Write backup file
        let mut file = File::create(&backup_path)?;
        
        // Write metadata header (in a real implementation, would use a proper format)
        let metadata_bytes = bincode::serialize(&metadata).map_err(|e| {
            BackupError::SerializationError(e.to_string())
        })?;
        
        // Write metadata size and data
        file.write_all(&(metadata_bytes.len() as u32).to_le_bytes())?;
        file.write_all(&metadata_bytes)?;
        
        // Write the actual backup data
        file.write_all(&processed_data)?;
        file.sync_all()?;
        
        // Create backup reference
        let backup = BackupReference {
            id: backup_id,
            path: backup_path,
            metadata,
        };
        
        // Store in cache
        {
            let mut backups = self.backups.lock().map_err(|_| {
                BackupError::InvalidBackup("Failed to acquire mutex".to_string())
            })?;
            backups.insert(backup.id.clone(), backup.clone());
        }
        
        // Update last backup heights
        {
            let mut last_height = self.last_backup_height.lock().map_err(|_| {
                BackupError::InvalidBackup("Failed to acquire mutex".to_string())
            })?;
            *last_height = height;
        }
        
        if backup_type == BackupType::Full {
            let mut last_full_height = self.last_full_backup_height.lock().map_err(|_| {
                BackupError::InvalidBackup("Failed to acquire mutex".to_string())
            })?;
            *last_full_height = height;
        }
        
        // Prune old backups if needed
        self.prune_old_backups()?;
        
        Ok(backup)
    }
    
    /// Determine the type of backup to create
    fn determine_backup_type(&self, height: u32) -> BackupResult<BackupType> {
        let last_full_backup_height = {
            let height = self.last_full_backup_height.lock().map_err(|_| {
                BackupError::InvalidBackup("Failed to acquire mutex".to_string())
            })?;
            *height
        };
        
        if last_full_backup_height == 0 || 
           height >= last_full_backup_height + self.config.full_backup_interval {
            // We need a full backup
            Ok(BackupType::Full)
        } else {
            // Incremental backup
            Ok(BackupType::Incremental)
        }
    }
    
    /// Generate a unique ID for a backup
    fn generate_backup_id(&self, backup_type: BackupType, height: u32) -> String {
        let type_str = match backup_type {
            BackupType::Full => "full",
            BackupType::Incremental => "incr",
            BackupType::UtxoSnapshot => "utxo",
            BackupType::ChainState => "state",
        };
        
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        
        format!("supernova-{}-{}-{}", type_str, height, timestamp)
    }
    
    /// Calculate SHA-256 hash of data
    fn calculate_hash(&self, data: &[u8]) -> [u8; 32] {
        let mut hasher = Sha256::new();
        hasher.update(data);
        let result = hasher.finalize();
        
        let mut hash = [0u8; 32];
        hash.copy_from_slice(&result);
        hash
    }
    
    /// Load metadata from a backup file
    fn load_backup_metadata(&self, path: &Path) -> BackupResult<BackupReference> {
        let mut file = File::open(path)?;
        
        // Read metadata size
        let mut size_bytes = [0u8; 4];
        file.read_exact(&mut size_bytes)?;
        let metadata_size = u32::from_le_bytes(size_bytes) as usize;
        
        // Read metadata
        let mut metadata_bytes = vec![0u8; metadata_size];
        file.read_exact(&mut metadata_bytes)?;
        
        // Deserialize metadata
        let metadata: BackupMetadata = bincode::deserialize(&metadata_bytes).map_err(|e| {
            BackupError::SerializationError(e.to_string())
        })?;
        
        // Create backup reference
        let backup_id = path.file_stem()
            .and_then(|s| s.to_str())
            .ok_or_else(|| BackupError::InvalidBackup("Invalid file name".to_string()))?
            .to_string();
        
        Ok(BackupReference {
            id: backup_id,
            path: path.to_path_buf(),
            metadata,
        })
    }
    
    /// Restore from a backup
    pub fn restore_backup(&self, backup_id: &str) -> BackupResult<Vec<u8>> {
        // Find the backup
        let backup = {
            let backups = self.backups.lock().map_err(|_| {
                BackupError::InvalidBackup("Failed to acquire mutex".to_string())
            })?;
            
            backups.get(backup_id).cloned().ok_or_else(|| {
                BackupError::BackupNotFound(backup_id.to_string())
            })?
        };
        
        // Open the backup file
        let mut file = File::open(&backup.path)?;
        
        // Skip metadata header
        let mut size_bytes = [0u8; 4];
        file.read_exact(&mut size_bytes)?;
        let metadata_size = u32::from_le_bytes(size_bytes) as usize;
        file.seek(SeekFrom::Current(metadata_size as i64))?;
        
        // Read the backup data
        let mut data = Vec::new();
        file.read_to_end(&mut data)?;
        
        // Verify data integrity
        let data_hash = self.calculate_hash(&data);
        if data_hash != backup.metadata.data_hash {
            return Err(BackupError::IntegrityFailed(
                "Backup data hash mismatch".to_string()
            ));
        }
        
        // Handle compressed backups
        if backup.metadata.compressed {
            // In a real implementation, decompress the data
            // For simplicity, we'll skip decompression here
        }
        
        // For incremental backups, apply to base backup
        if backup.metadata.backup_type == BackupType::Incremental {
            if let Some(base_id) = &backup.metadata.base_backup_id {
                // Get the base backup
                let base_data = self.restore_backup(base_id)?;
                
                // Apply incremental changes
                // In a real implementation, this would merge the incremental changes
                // For simplicity, we'll just return the incremental data
                return Ok(data);
            }
        }
        
        Ok(data)
    }
    
    /// Get the latest backup
    pub fn get_latest_backup(&self) -> Option<BackupReference> {
        let backups = self.backups.lock().ok()?;
        
        backups.values()
            .max_by_key(|b| b.metadata.block_height)
            .cloned()
    }
    
    /// Get the latest full backup
    pub fn get_latest_full_backup(&self) -> Option<BackupReference> {
        let backups = self.backups.lock().ok()?;
        
        backups.values()
            .filter(|b| b.metadata.backup_type == BackupType::Full)
            .max_by_key(|b| b.metadata.block_height)
            .cloned()
    }
    
    /// Check if a backup should be created at the given height
    pub fn should_create_backup(&self, height: u32) -> BackupResult<bool> {
        let last_backup_height = {
            let height = self.last_backup_height.lock().map_err(|_| {
                BackupError::InvalidBackup("Failed to acquire mutex".to_string())
            })?;
            *height
        };
        
        let last_full_backup_height = {
            let height = self.last_full_backup_height.lock().map_err(|_| {
                BackupError::InvalidBackup("Failed to acquire mutex".to_string())
            })?;
            *height
        };
        
        // Check if we need a full backup
        if last_full_backup_height == 0 || 
           height >= last_full_backup_height + self.config.full_backup_interval {
            return Ok(true);
        }
        
        // Check if we need an incremental backup
        if height >= last_backup_height + self.config.incremental_backup_interval {
            return Ok(true);
        }
        
        Ok(false)
    }
    
    /// Verify the integrity of a backup
    pub fn verify_backup(&self, backup_id: &str) -> BackupResult<bool> {
        // Find the backup
        let backup = {
            let backups = self.backups.lock().map_err(|_| {
                BackupError::InvalidBackup("Failed to acquire mutex".to_string())
            })?;
            
            backups.get(backup_id).cloned().ok_or_else(|| {
                BackupError::BackupNotFound(backup_id.to_string())
            })?
        };
        
        // Open the backup file
        let mut file = File::open(&backup.path)?;
        
        // Skip metadata header
        let mut size_bytes = [0u8; 4];
        file.read_exact(&mut size_bytes)?;
        let metadata_size = u32::from_le_bytes(size_bytes) as usize;
        file.seek(SeekFrom::Current(metadata_size as i64))?;
        
        // Read the backup data
        let mut data = Vec::new();
        file.read_to_end(&mut data)?;
        
        // Verify data integrity
        let data_hash = self.calculate_hash(&data);
        Ok(data_hash == backup.metadata.data_hash)
    }
    
    /// Prune old backups to stay within the configured limit
    fn prune_old_backups(&self) -> BackupResult<()> {
        let backups = self.backups.lock().map_err(|_| {
            BackupError::InvalidBackup("Failed to acquire mutex".to_string())
        })?;
        
        // Check if we need to prune
        if backups.len() <= self.config.max_backups {
            return Ok(());
        }
        
        // Sort backups by age (oldest first)
        let mut backup_list: Vec<_> = backups.values().collect();
        backup_list.sort_by_key(|b| b.metadata.timestamp);
        
        // Determine how many to remove
        let to_remove = backups.len() - self.config.max_backups;
        
        // Remove the oldest backups, but keep at least one full backup
        let mut full_backup_count = backup_list.iter()
            .filter(|b| b.metadata.backup_type == BackupType::Full)
            .count();
            
        let mut removed = 0;
        
        for backup in backup_list {
            // Don't remove the last full backup
            if backup.metadata.backup_type == BackupType::Full && full_backup_count <= 1 {
                continue;
            }
            
            // Remove the backup file
            if let Err(e) = fs::remove_file(&backup.path) {
                // Just log the error and continue
                eprintln!("Failed to remove backup file: {}", e);
            } else {
                // Update counters
                if backup.metadata.backup_type == BackupType::Full {
                    full_backup_count -= 1;
                }
                removed += 1;
            }
            
            // Check if we've removed enough
            if removed >= to_remove {
                break;
            }
        }
        
        // Rescan backups to update the cache
        drop(backups); // Release the lock before rescanning
        self.scan_backups()?;
        
        Ok(())
    }
    
    /// Get all available backups
    pub fn list_backups(&self) -> BackupResult<Vec<BackupReference>> {
        let backups = self.backups.lock().map_err(|_| {
            BackupError::InvalidBackup("Failed to acquire mutex".to_string())
        })?;
        
        Ok(backups.values().cloned().collect())
    }
    
    /// Convert a block to backup data (simplified serialization)
    pub fn block_to_backup_data(&self, block: &Block, checkpoints: &[Checkpoint]) -> BackupResult<Vec<u8>> {
        // In a real implementation, would use a proper serialization format
        // For simplicity, we'll just use bincode
        let data = bincode::serialize(&(block, checkpoints)).map_err(|e| {
            BackupError::SerializationError(e.to_string())
        })?;
        
        Ok(data)
    }
    
    /// Convert backup data to a block and checkpoints
    pub fn backup_data_to_block(&self, data: &[u8]) -> BackupResult<(Block, Vec<Checkpoint>)> {
        // In a real implementation, would use a proper deserialization format
        // For simplicity, we'll just use bincode
        let (block, checkpoints): (Block, Vec<Checkpoint>) = bincode::deserialize(data).map_err(|e| {
            BackupError::SerializationError(e.to_string())
        })?;
        
        Ok((block, checkpoints))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::transaction::Transaction;
    use tempfile::tempdir;
    
    #[test]
    fn test_backup_create_and_restore() {
        // Create temporary directory for test
        let temp_dir = tempdir().unwrap();
        
        // Create config
        let config = BackupConfig {
            backup_dir: temp_dir.path().to_path_buf(),
            ..Default::default()
        };
        
        // Create backup manager
        let backup_manager = BackupManager::new(config).unwrap();
        
        // Create a test block
        let header = BlockHeader::new(
            1,
            [0u8; 32],
            [0; 32], // merkle_root
            0, // timestamp
            0, // bits
            0, // nonce
        );
        let block = Block::new(header, vec![Transaction::new(1, vec![], vec![], 0)]);
        
        // Create test checkpoints
        let checkpoint = Checkpoint {
            height: 1,
            hash: block.hash(),
            utxo_commitment: crate::storage::utxo_set::UtxoCommitment {
                root_hash: [0u8; 32],
                utxo_count: 0,
                total_value: 0,
                block_height: 1,
            },
        };
        
        // Convert to backup data
        let backup_data = backup_manager.block_to_backup_data(&block, &[checkpoint.clone()]).unwrap();
        
        // Create backup
        let backup = backup_manager.create_backup(1, block.hash(), &[checkpoint], &backup_data).unwrap();
        
        // Verify backup was created
        assert_eq!(backup.metadata.block_height, 1);
        assert_eq!(backup.metadata.backup_type, BackupType::Full);
        
        // Restore the backup
        let restored_data = backup_manager.restore_backup(&backup.id).unwrap();
        
        // Convert back to block and checkpoints
        let (restored_block, restored_checkpoints) = backup_manager.backup_data_to_block(&restored_data).unwrap();
        
        // Verify restored data
        assert_eq!(restored_block.hash(), block.hash());
        assert_eq!(restored_checkpoints.len(), 1);
        assert_eq!(restored_checkpoints[0].height, 1);
    }
    
    #[test]
    fn test_backup_integrity_verification() {
        // Create temporary directory for test
        let temp_dir = tempdir().unwrap();
        
        // Create config
        let config = BackupConfig {
            backup_dir: temp_dir.path().to_path_buf(),
            ..Default::default()
        };
        
        // Create backup manager
        let backup_manager = BackupManager::new(config).unwrap();
        
        // Create a test block
        let header = BlockHeader::new(
            1,
            [0u8; 32],
            [0; 32], // merkle_root
            0, // timestamp
            0, // bits
            0, // nonce
        );
        let block = Block::new(header, vec![Transaction::new(1, vec![], vec![], 0)]);
        
        // Create backup
        let backup_data = backup_manager.block_to_backup_data(&block, &[]).unwrap();
        let backup = backup_manager.create_backup(1, block.hash(), &[], &backup_data).unwrap();
        
        // Verify backup integrity
        assert!(backup_manager.verify_backup(&backup.id).unwrap());
    }
} 