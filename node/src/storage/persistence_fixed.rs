// Fixed version of persistence.rs - NO UNWRAPS, following Satoshi Standard
// This replaces all unwrap()/expect() with proper error handling

use std::path::{Path, PathBuf};
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write, Seek, SeekFrom};
use std::sync::{Arc, RwLock};
use std::collections::HashMap;
use serde::{Serialize, Deserialize};
use sha2::{Sha256, Digest};
use log::{info, warn, error};

use crate::error::{StorageError, StorageResult};

/// Persistent storage manager for blockchain data
pub struct PersistenceManager {
    /// Base directory for all storage
    base_path: PathBuf,
    /// Open file handles
    file_handles: Arc<RwLock<HashMap<String, File>>>,
    /// Configuration
    config: PersistenceConfig,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PersistenceConfig {
    /// Maximum file size before rotation
    pub max_file_size: u64,
    /// Enable compression
    pub compression_enabled: bool,
    /// Sync writes to disk
    pub sync_writes: bool,
    /// Cache size in MB
    pub cache_size_mb: usize,
}

impl Default for PersistenceConfig {
    fn default() -> Self {
        Self {
            max_file_size: 1024 * 1024 * 1024, // 1GB
            compression_enabled: true,
            sync_writes: true,
            cache_size_mb: 100,
        }
    }
}

impl PersistenceManager {
    /// Create a new persistence manager
    pub fn new(base_path: impl AsRef<Path>, config: PersistenceConfig) -> StorageResult<Self> {
        let base_path = base_path.as_ref().to_path_buf();

        // Create base directory if it doesn't exist
        fs::create_dir_all(&base_path)
            .map_err(|e| StorageError::Io(e))?;

        Ok(Self {
            base_path,
            file_handles: Arc::new(RwLock::new(HashMap::new())),
            config,
        })
    }

    /// Write data to a file
    pub fn write_file(&self, filename: &str, data: &[u8]) -> StorageResult<()> {
        let file_path = self.base_path.join(filename);

        // Create parent directory if needed
        if let Some(parent) = file_path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| StorageError::Io(e))?;
        }

        // Open file for writing
        let mut file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&file_path)
            .map_err(|e| StorageError::Io(e))?;

        // Write data
        file.write_all(data)
            .map_err(|e| StorageError::Io(e))?;

        // Sync if configured
        if self.config.sync_writes {
            file.sync_all()
                .map_err(|e| StorageError::Io(e))?;
        }

        Ok(())
    }

    /// Read data from a file
    pub fn read_file(&self, filename: &str) -> StorageResult<Vec<u8>> {
        let file_path = self.base_path.join(filename);

        let mut file = File::open(&file_path)
            .map_err(|e| StorageError::Io(e))?;

        let mut data = Vec::new();
        file.read_to_end(&mut data)
            .map_err(|e| StorageError::Io(e))?;

        Ok(data)
    }

    /// Append data to a file
    pub fn append_file(&self, filename: &str, data: &[u8]) -> StorageResult<()> {
        let file_path = self.base_path.join(filename);

        // Check file size
        if let Ok(metadata) = fs::metadata(&file_path) {
            if metadata.len() + data.len() as u64 > self.config.max_file_size {
                self.rotate_file(filename)?;
            }
        }

        // Open file for appending
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&file_path)
            .map_err(|e| StorageError::Io(e))?;

        // Write data
        file.write_all(data)
            .map_err(|e| StorageError::Io(e))?;

        // Sync if configured
        if self.config.sync_writes {
            file.sync_all()
                .map_err(|e| StorageError::Io(e))?;
        }

        Ok(())
    }

    /// Delete a file
    pub fn delete_file(&self, filename: &str) -> StorageResult<()> {
        let file_path = self.base_path.join(filename);

        if file_path.exists() {
            fs::remove_file(&file_path)
                .map_err(|e| StorageError::Io(e))?;
        }

        // Remove from file handles if present
        let mut handles = self.file_handles.write()
            .map_err(|e| StorageError::DatabaseError(format!("Lock poisoned: {}", e)))?;
        handles.remove(filename);

        Ok(())
    }

    /// List files in a directory
    pub fn list_files(&self, dir: &str) -> StorageResult<Vec<String>> {
        let dir_path = self.base_path.join(dir);

        if !dir_path.exists() {
            return Ok(Vec::new());
        }

        let mut files = Vec::new();
        let entries = fs::read_dir(&dir_path)
            .map_err(|e| StorageError::Io(e))?;

        for entry in entries {
            let entry = entry.map_err(|e| StorageError::Io(e))?;
            let path = entry.path();

            if path.is_file() {
                if let Some(filename) = path.file_name() {
                    if let Some(filename_str) = filename.to_str() {
                        files.push(filename_str.to_string());
                    }
                }
            }
        }

        Ok(files)
    }

    /// Check if a file exists
    pub fn file_exists(&self, filename: &str) -> bool {
        self.base_path.join(filename).exists()
    }

    /// Get file size
    pub fn file_size(&self, filename: &str) -> StorageResult<u64> {
        let file_path = self.base_path.join(filename);

        let metadata = fs::metadata(&file_path)
            .map_err(|e| StorageError::Io(e))?;

        Ok(metadata.len())
    }

    /// Rotate a file (rename with timestamp)
    fn rotate_file(&self, filename: &str) -> StorageResult<()> {
        let file_path = self.base_path.join(filename);

        if file_path.exists() {
            let timestamp = chrono::Utc::now().timestamp();
            let rotated_name = format!("{}.{}", filename, timestamp);
            let rotated_path = self.base_path.join(rotated_name);

            fs::rename(&file_path, &rotated_path)
                .map_err(|e| StorageError::Io(e))?;

            info!("Rotated file {} to {}", filename, rotated_name);
        }

        Ok(())
    }

    /// Create a memory-mapped file
    pub fn create_mmap(&self, filename: &str, size: u64) -> StorageResult<memmap2::MmapMut> {
        let file_path = self.base_path.join(filename);

        // Create parent directory if needed
        if let Some(parent) = file_path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| StorageError::Io(e))?;
        }

        // Open or create file
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(&file_path)
            .map_err(|e| StorageError::Io(e))?;

        // Set file size
        file.set_len(size)
            .map_err(|e| StorageError::Io(e))?;

        // Create memory map
        unsafe {
            memmap2::MmapMut::map_mut(&file)
                .map_err(|e| StorageError::Io(io::Error::new(io::ErrorKind::Other, e)))
        }
    }

    /// Calculate checksum of a file
    pub fn calculate_checksum(&self, filename: &str) -> StorageResult<Vec<u8>> {
        let data = self.read_file(filename)?;

        let mut hasher = Sha256::new();
        hasher.update(&data);

        Ok(hasher.finalize().to_vec())
    }

    /// Verify file integrity
    pub fn verify_checksum(&self, filename: &str, expected_checksum: &[u8]) -> StorageResult<bool> {
        let actual_checksum = self.calculate_checksum(filename)?;
        Ok(actual_checksum == expected_checksum)
    }

    /// Atomic file write (write to temp file then rename)
    pub fn atomic_write(&self, filename: &str, data: &[u8]) -> StorageResult<()> {
        let file_path = self.base_path.join(filename);
        let temp_path = self.base_path.join(format!("{}.tmp", filename));

        // Write to temporary file
        let mut temp_file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&temp_path)
            .map_err(|e| StorageError::Io(e))?;

        temp_file.write_all(data)
            .map_err(|e| StorageError::Io(e))?;

        temp_file.sync_all()
            .map_err(|e| StorageError::Io(e))?;

        // Atomic rename
        fs::rename(&temp_path, &file_path)
            .map_err(|e| StorageError::Io(e))?;

        Ok(())
    }

    /// Get storage statistics
    pub fn get_stats(&self) -> StorageResult<StorageStats> {
        let mut total_size = 0u64;
        let mut file_count = 0u32;

        for entry in fs::read_dir(&self.base_path).map_err(|e| StorageError::Io(e))? {
            let entry = entry.map_err(|e| StorageError::Io(e))?;
            let metadata = entry.metadata().map_err(|e| StorageError::Io(e))?;

            if metadata.is_file() {
                total_size += metadata.len();
                file_count += 1;
            }
        }

        Ok(StorageStats {
            total_size,
            file_count,
            base_path: self.base_path.clone(),
        })
    }
}

#[derive(Debug, Clone)]
pub struct StorageStats {
    pub total_size: u64,
    pub file_count: u32,
    pub base_path: PathBuf,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_basic_operations() -> StorageResult<()> {
        let temp_dir = TempDir::new().map_err(|e| StorageError::Io(e))?;
        let manager = PersistenceManager::new(temp_dir.path(), PersistenceConfig::default())?;

        // Test write and read
        let data = b"Hello, Supernova!";
        manager.write_file("test.txt", data)?;

        let read_data = manager.read_file("test.txt")?;
        assert_eq!(data, &read_data[..]);

        // Test file exists
        assert!(manager.file_exists("test.txt"));
        assert!(!manager.file_exists("nonexistent.txt"));

        // Test delete
        manager.delete_file("test.txt")?;
        assert!(!manager.file_exists("test.txt"));

        Ok(())
    }

    #[test]
    fn test_atomic_write() -> StorageResult<()> {
        let temp_dir = TempDir::new().map_err(|e| StorageError::Io(e))?;
        let manager = PersistenceManager::new(temp_dir.path(), PersistenceConfig::default())?;

        let data = b"Atomic data";
        manager.atomic_write("atomic.txt", data)?;

        let read_data = manager.read_file("atomic.txt")?;
        assert_eq!(data, &read_data[..]);

        Ok(())
    }
}