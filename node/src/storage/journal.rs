//! Journal and write-ahead log implementation for SuperNova blockchain
//! 
//! This module provides journaling functionality for recording all write operations before they hit the
//! main database, ensuring durability and recoverability.

use std::io::{Write, Read, Seek, SeekFrom};
use std::fs::{self, File, OpenOptions};
use std::path::{Path, PathBuf};
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;
use tokio::io::{AsyncWriteExt, AsyncReadExt, AsyncSeekExt};
use serde::{Serialize, Deserialize};
use bincode;
use crc32fast::Hasher;
use thiserror::Error;
use tracing::{info, warn, error, debug};
use super::database::StorageError;

/// Maximum size of the WAL file before rotation (100MB)
const MAX_WAL_SIZE: u64 = 100 * 1024 * 1024;

/// WAL entry types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum JournalEntry {
    /// Block write operation
    BlockWrite {
        hash: [u8; 32],
        data: Vec<u8>,
    },
    /// Transaction write operation
    TransactionWrite {
        hash: [u8; 32],
        data: Vec<u8>,
    },
    /// UTXO write operation
    UtxoWrite {
        tx_hash: [u8; 32],
        index: u32,
        data: Vec<u8>,
    },
    /// UTXO delete operation
    UtxoDelete {
        tx_hash: [u8; 32],
        index: u32,
    },
    /// Metadata write operation
    MetadataWrite {
        key: Vec<u8>,
        value: Vec<u8>,
    },
    /// Height index update
    HeightIndexWrite {
        height: u64,
        block_hash: [u8; 32],
    },
    /// Batch operation marker
    BatchStart {
        batch_id: u64,
        timestamp: u64,
    },
    /// Batch commit marker
    BatchCommit {
        batch_id: u64,
    },
    /// Batch rollback marker
    BatchRollback {
        batch_id: u64,
    },
    /// Checkpoint marker
    Checkpoint {
        height: u64,
        hash: [u8; 32],
        timestamp: u64,
    },
}

/// WAL entry with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
struct WalEntry {
    /// Sequence number for ordering
    sequence: u64,
    /// Timestamp when entry was created
    timestamp: u64,
    /// The actual journal entry
    entry: JournalEntry,
    /// CRC32 checksum for corruption detection
    checksum: u32,
}

impl WalEntry {
    /// Create a new WAL entry
    fn new(sequence: u64, entry: JournalEntry) -> Self {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        
        let mut wal_entry = Self {
            sequence,
            timestamp,
            entry,
            checksum: 0,
        };
        
        // Calculate checksum
        wal_entry.checksum = wal_entry.calculate_checksum();
        wal_entry
    }
    
    /// Calculate CRC32 checksum for the entry
    fn calculate_checksum(&self) -> u32 {
        let data = bincode::serialize(&(&self.sequence, &self.timestamp, &self.entry))
            .unwrap_or_default();
        crc32fast::hash(&data)
    }
    
    /// Verify the checksum
    fn verify_checksum(&self) -> bool {
        self.checksum == self.calculate_checksum()
    }
}

/// WAL error types
#[derive(Error, Debug)]
pub enum WalError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Serialization error: {0}")]
    Serialization(#[from] bincode::Error),
    #[error("Corrupted entry at sequence {sequence}")]
    CorruptedEntry { sequence: u64 },
    #[error("WAL file corrupted")]
    FileCorrupted,
    #[error("WAL rotation failed")]
    RotationFailed,
}

/// Write-ahead log implementation
pub struct WriteAheadLog {
    /// Path to the WAL directory
    wal_dir: PathBuf,
    /// Current WAL file
    current_file: RwLock<Option<File>>,
    /// Current sequence number
    sequence: RwLock<u64>,
    /// Pending entries buffer
    pending_entries: RwLock<VecDeque<WalEntry>>,
    /// Whether WAL is in recovery mode
    recovery_mode: RwLock<bool>,
    /// Current batch ID
    current_batch: RwLock<Option<u64>>,
}

impl WriteAheadLog {
    /// Create a new WAL instance
    pub async fn new<P: AsRef<Path>>(wal_dir: P) -> Result<Self, WalError> {
        let wal_dir = wal_dir.as_ref().to_path_buf();
        
        // Create WAL directory if it doesn't exist
        tokio::fs::create_dir_all(&wal_dir).await?;
        
        let wal = Self {
            wal_dir,
            current_file: RwLock::new(None),
            sequence: RwLock::new(0),
            pending_entries: RwLock::new(VecDeque::new()),
            recovery_mode: RwLock::new(false),
            current_batch: RwLock::new(None),
        };
        
        // Initialize WAL
        wal.init().await?;
        
        Ok(wal)
    }
    
    /// Initialize the WAL
    async fn init(&self) -> Result<(), WalError> {
        // Find the latest WAL file
        let current_wal_path = self.get_current_wal_path();
        
        if current_wal_path.exists() {
            // Load existing WAL and get the last sequence number
            let last_sequence = self.load_wal_file(&current_wal_path).await?;
            *self.sequence.write().await = last_sequence + 1;
        }
        
        // Open or create the current WAL file
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&current_wal_path)?;
        
        *self.current_file.write().await = Some(file);
        
        info!("WAL initialized at {:?}", current_wal_path);
        
        Ok(())
    }
    
    /// Get the path to the current WAL file
    fn get_current_wal_path(&self) -> PathBuf {
        self.wal_dir.join("wal.current")
    }
    
    /// Get the path to an archived WAL file
    fn get_archive_wal_path(&self, timestamp: u64) -> PathBuf {
        self.wal_dir.join(format!("wal.{}.archive", timestamp))
    }
    
    /// Write an entry to the WAL
    pub async fn write_entry(&self, entry: JournalEntry) -> Result<u64, WalError> {
        // Get next sequence number
        let sequence = {
            let mut seq = self.sequence.write().await;
            let current = *seq;
            *seq += 1;
            current
        };
        
        // Create WAL entry
        let wal_entry = WalEntry::new(sequence, entry);
        
        // Serialize entry
        let data = bincode::serialize(&wal_entry)?;
        let size = (data.len() as u32).to_le_bytes();
        
        // Write to file
        let mut file_guard = self.current_file.write().await;
        if let Some(file) = file_guard.as_mut() {
            // Write size prefix and data
            file.write_all(&size)?;
            file.write_all(&data)?;
            file.flush()?;
        }
        
        // Add to pending entries if in recovery mode
        if *self.recovery_mode.read().await {
            self.pending_entries.write().await.push_back(wal_entry);
        }
        
        // Check if rotation is needed
        drop(file_guard);
        self.check_rotation().await?;
        
        Ok(sequence)
    }
    
    /// Start a batch operation
    pub async fn start_batch(&self) -> Result<u64, WalError> {
        let batch_id = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_micros() as u64;
        
        *self.current_batch.write().await = Some(batch_id);
        
        self.write_entry(JournalEntry::BatchStart {
            batch_id,
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        }).await?;
        
        Ok(batch_id)
    }
    
    /// Commit a batch operation
    pub async fn commit_batch(&self, batch_id: u64) -> Result<(), WalError> {
        self.write_entry(JournalEntry::BatchCommit { batch_id }).await?;
        *self.current_batch.write().await = None;
        Ok(())
    }
    
    /// Rollback a batch operation
    pub async fn rollback_batch(&self, batch_id: u64) -> Result<(), WalError> {
        self.write_entry(JournalEntry::BatchRollback { batch_id }).await?;
        *self.current_batch.write().await = None;
        Ok(())
    }
    
    /// Create a checkpoint
    pub async fn create_checkpoint(&self, height: u64, hash: [u8; 32]) -> Result<(), WalError> {
        self.write_entry(JournalEntry::Checkpoint {
            height,
            hash,
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        }).await?;
        
        // Flush to ensure checkpoint is persisted
        self.flush().await?;
        
        Ok(())
    }
    
    /// Flush all pending writes
    pub async fn flush(&self) -> Result<(), WalError> {
        let mut file_guard = self.current_file.write().await;
        if let Some(file) = file_guard.as_mut() {
            file.sync_all()?;
        }
        Ok(())
    }
    
    /// Check if WAL rotation is needed
    async fn check_rotation(&self) -> Result<(), WalError> {
        let current_path = self.get_current_wal_path();
        
        if let Ok(metadata) = tokio::fs::metadata(&current_path).await {
            if metadata.len() > MAX_WAL_SIZE {
                self.rotate_wal().await?;
            }
        }
        
        Ok(())
    }
    
    /// Rotate the WAL file
    async fn rotate_wal(&self) -> Result<(), WalError> {
        info!("Rotating WAL file...");
        
        // Close current file
        let mut file_guard = self.current_file.write().await;
        if let Some(mut file) = file_guard.take() {
            file.sync_all()?;
        }
        
        // Rename current WAL to archive
        let current_path = self.get_current_wal_path();
        let archive_path = self.get_archive_wal_path(
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs()
        );
        
        tokio::fs::rename(&current_path, &archive_path).await?;
        
        // Create new WAL file
        let new_file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&current_path)?;
        
        *file_guard = Some(new_file);
        
        info!("WAL rotated to {:?}", archive_path);
        
        Ok(())
    }
    
    /// Load a WAL file and return the last sequence number
    async fn load_wal_file(&self, path: &Path) -> Result<u64, WalError> {
        let mut file = File::open(path)?;
        let mut last_sequence = 0u64;
        let mut buffer = Vec::new();
        
        loop {
            // Read size prefix
            let mut size_bytes = [0u8; 4];
            match file.read_exact(&mut size_bytes) {
                Ok(_) => {},
                Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => break,
                Err(e) => return Err(e.into()),
            }
            
            let size = u32::from_le_bytes(size_bytes) as usize;
            
            // Read entry data
            buffer.resize(size, 0);
            file.read_exact(&mut buffer)?;
            
            // Deserialize and verify entry
            let entry: WalEntry = bincode::deserialize(&buffer)?;
            
            if !entry.verify_checksum() {
                return Err(WalError::CorruptedEntry { sequence: entry.sequence });
            }
            
            last_sequence = last_sequence.max(entry.sequence);
            
            // Store in pending entries if in recovery mode
            if *self.recovery_mode.read().await {
                self.pending_entries.write().await.push_back(entry);
            }
        }
        
        Ok(last_sequence)
    }
    
    /// Get all pending entries for recovery
    pub async fn get_pending_entries(&self) -> Result<Vec<JournalEntry>, WalError> {
        // Set recovery mode
        *self.recovery_mode.write().await = true;
        
        // Clear pending entries
        self.pending_entries.write().await.clear();
        
        // Load all WAL files
        let mut entries = Vec::new();
        
        // Load archived WAL files first
        let mut dir_entries = tokio::fs::read_dir(&self.wal_dir).await?;
        let mut archive_files = Vec::new();
        
        while let Some(entry) = dir_entries.next_entry().await? {
            let path = entry.path();
            if let Some(name) = path.file_name() {
                if let Some(name_str) = name.to_str() {
                    if name_str.starts_with("wal.") && name_str.ends_with(".archive") {
                        archive_files.push(path);
                    }
                }
            }
        }
        
        // Sort archive files by timestamp
        archive_files.sort();
        
        // Load archive files
        for archive_path in archive_files {
            self.load_wal_file(&archive_path).await?;
        }
        
        // Load current WAL file
        let current_path = self.get_current_wal_path();
        if current_path.exists() {
            self.load_wal_file(&current_path).await?;
        }
        
        // Extract entries from pending queue
        let pending = self.pending_entries.read().await;
        for wal_entry in pending.iter() {
            entries.push(wal_entry.entry.clone());
        }
        
        // Clear recovery mode
        *self.recovery_mode.write().await = false;
        
        Ok(entries)
    }
    
    /// Clear pending entries after recovery
    pub async fn clear_pending(&self) -> Result<(), WalError> {
        self.pending_entries.write().await.clear();
        
        // Delete all archive files
        let mut dir_entries = tokio::fs::read_dir(&self.wal_dir).await?;
        
        while let Some(entry) = dir_entries.next_entry().await? {
            let path = entry.path();
            if let Some(name) = path.file_name() {
                if let Some(name_str) = name.to_str() {
                    if name_str.starts_with("wal.") && name_str.ends_with(".archive") {
                        tokio::fs::remove_file(path)?;
                    }
                }
            }
        }
        
        Ok(())
    }
    
    /// Mark a clean shutdown in the WAL
    pub async fn mark_clean_shutdown(&self) -> Result<(), WalError> {
        // Write a special marker file
        let marker_path = self.wal_dir.join("clean_shutdown");
        tokio::fs::write(&marker_path, b"1").await?;
        
        // Flush the current WAL
        self.flush().await?;
        
        Ok(())
    }
    
    /// Check if the last shutdown was clean
    pub async fn was_clean_shutdown(&self) -> Result<bool, WalError> {
        let marker_path = self.wal_dir.join("clean_shutdown");
        
        if marker_path.exists() {
            // Remove the marker
            tokio::fs::remove_file(&marker_path)?;
            Ok(true)
        } else {
            Ok(false)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    
    #[tokio::test]
    async fn test_wal_basic_operations() {
        let temp_dir = TempDir::new().unwrap();
        let wal = WriteAheadLog::new(temp_dir.path()).await.unwrap();
        
        // Write some entries
        let seq1 = wal.write_entry(JournalEntry::MetadataWrite {
            key: b"test_key".to_vec(),
            value: b"test_value".to_vec(),
        }).await.unwrap();
        
        let seq2 = wal.write_entry(JournalEntry::BlockWrite {
            hash: [0u8; 32],
            data: vec![1, 2, 3, 4],
        }).await.unwrap();
        
        assert_eq!(seq1, 0);
        assert_eq!(seq2, 1);
        
        // Flush
        wal.flush().await.unwrap();
    }
    
    #[tokio::test]
    async fn test_wal_recovery() {
        let temp_dir = TempDir::new().unwrap();
        let wal_path = temp_dir.path();
        
        // Create WAL and write entries
        {
            let wal = WriteAheadLog::new(wal_path).await.unwrap();
            
            wal.write_entry(JournalEntry::BlockWrite {
                hash: [1u8; 32],
                data: vec![1, 2, 3],
            }).await.unwrap();
            
            wal.write_entry(JournalEntry::TransactionWrite {
                hash: [2u8; 32],
                data: vec![4, 5, 6],
            }).await.unwrap();
            
            wal.flush().await.unwrap();
        }
        
        // Create new WAL and recover entries
        {
            let wal = WriteAheadLog::new(wal_path).await.unwrap();
            let entries = wal.get_pending_entries().await.unwrap();
            
            assert_eq!(entries.len(), 2);
            
            match &entries[0] {
                JournalEntry::BlockWrite { hash, .. } => assert_eq!(hash, &[1u8; 32]),
                _ => panic!("Wrong entry type"),
            }
            
            match &entries[1] {
                JournalEntry::TransactionWrite { hash, .. } => assert_eq!(hash, &[2u8; 32]),
                _ => panic!("Wrong entry type"),
            }
        }
    }
    
    #[tokio::test]
    async fn test_wal_batch_operations() {
        let temp_dir = TempDir::new().unwrap();
        let wal = WriteAheadLog::new(temp_dir.path()).await.unwrap();
        
        // Start batch
        let batch_id = wal.start_batch().await.unwrap();
        
        // Write entries in batch
        wal.write_entry(JournalEntry::MetadataWrite {
            key: b"key1".to_vec(),
            value: b"value1".to_vec(),
        }).await.unwrap();
        
        wal.write_entry(JournalEntry::MetadataWrite {
            key: b"key2".to_vec(),
            value: b"value2".to_vec(),
        }).await.unwrap();
        
        // Commit batch
        wal.commit_batch(batch_id).await.unwrap();
        
        // Verify entries
        let entries = wal.get_pending_entries().await.unwrap();
        assert!(entries.len() >= 4); // BatchStart + 2 entries + BatchCommit
    }
} 