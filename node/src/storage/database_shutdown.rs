//! Database Shutdown Handler for Supernova
//! 
//! This module implements proper database shutdown procedures to prevent
//! corruption on node restart. It ensures all data is properly flushed,
//! transactions are committed, and the database is cleanly closed.

use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::{info, warn, error};
use super::database::{BlockchainDB, StorageError};
use super::journal::{WriteAheadLog, JournalEntry};
use std::path::Path;

/// Configuration for database shutdown procedures
#[derive(Debug, Clone)]
pub struct ShutdownConfig {
    /// Maximum time to wait for pending operations
    pub operation_timeout: Duration,
    /// Whether to create a final checkpoint on shutdown
    pub create_final_checkpoint: bool,
    /// Whether to compact the database before shutdown
    pub compact_on_shutdown: bool,
    /// Whether to verify integrity before shutdown
    pub verify_before_shutdown: bool,
    /// Grace period for in-flight transactions
    pub grace_period: Duration,
}

impl Default for ShutdownConfig {
    fn default() -> Self {
        Self {
            operation_timeout: Duration::from_secs(30),
            create_final_checkpoint: true,
            compact_on_shutdown: false, // Disabled by default for speed
            verify_before_shutdown: true,
            grace_period: Duration::from_secs(5),
        }
    }
}

/// Handles safe database shutdown procedures
pub struct DatabaseShutdownHandler {
    /// Reference to the database
    db: Arc<RwLock<BlockchainDB>>,
    /// Write-ahead log for crash recovery
    wal: Arc<RwLock<WriteAheadLog>>,
    /// Shutdown configuration
    config: ShutdownConfig,
    /// Tracks if shutdown is in progress
    shutdown_in_progress: Arc<RwLock<bool>>,
}

impl DatabaseShutdownHandler {
    /// Create a new shutdown handler
    pub fn new(
        db: Arc<RwLock<BlockchainDB>>,
        wal: Arc<RwLock<WriteAheadLog>>,
        config: ShutdownConfig,
    ) -> Self {
        Self {
            db,
            wal,
            config,
            shutdown_in_progress: Arc::new(RwLock::new(false)),
        }
    }
    
    /// Perform a graceful database shutdown
    pub async fn shutdown(&self) -> Result<(), StorageError> {
        let start_time = Instant::now();
        
        // Check if shutdown is already in progress
        {
            let mut in_progress = self.shutdown_in_progress.write().await;
            if *in_progress {
                warn!("Database shutdown already in progress");
                return Ok(());
            }
            *in_progress = true;
        }
        
        info!("Starting graceful database shutdown...");
        
        // Step 1: Stop accepting new writes
        self.stop_new_writes().await?;
        
        // Step 2: Wait for pending operations with grace period
        self.wait_for_pending_operations().await?;
        
        // Step 3: Flush all pending writes to disk
        self.flush_pending_writes().await?;
        
        // Step 4: Create final checkpoint if enabled
        if self.config.create_final_checkpoint {
            self.create_final_checkpoint().await?;
        }
        
        // Step 5: Verify database integrity if enabled
        if self.config.verify_before_shutdown {
            self.verify_integrity().await?;
        }
        
        // Step 6: Compact database if enabled
        if self.config.compact_on_shutdown {
            self.compact_database().await?;
        }
        
        // Step 7: Close write-ahead log
        self.close_wal().await?;
        
        // Step 8: Final sync and close database
        self.close_database().await?;
        
        let duration = start_time.elapsed();
        info!("Database shutdown completed in {:?}", duration);
        
        // Reset shutdown flag
        *self.shutdown_in_progress.write().await = false;
        
        Ok(())
    }
    
    /// Stop accepting new write operations
    async fn stop_new_writes(&self) -> Result<(), StorageError> {
        info!("Stopping new write operations...");
        
        let db = self.db.read().await;
        
        // Set a flag in metadata to indicate shutdown in progress
        db.store_metadata(b"shutdown_in_progress", b"true")?;
        
        // This would typically be handled by setting an atomic flag
        // that all write operations check before proceeding
        
        Ok(())
    }
    
    /// Wait for pending operations to complete
    async fn wait_for_pending_operations(&self) -> Result<(), StorageError> {
        info!("Waiting for pending operations to complete...");
        
        let deadline = Instant::now() + self.config.operation_timeout;
        
        // In a real implementation, this would track active operations
        // For now, we'll just wait for the grace period
        tokio::time::sleep(self.config.grace_period).await;
        
        if Instant::now() > deadline {
            warn!("Timeout waiting for pending operations");
        }
        
        Ok(())
    }
    
    /// Flush all pending writes to disk
    async fn flush_pending_writes(&self) -> Result<(), StorageError> {
        info!("Flushing pending writes to disk...");
        
        let db = self.db.read().await;
        
        // Flush the database
        db.flush()?;
        
        // Ensure all async operations complete
        db.async_flush().await?;
        
        Ok(())
    }
    
    /// Create a final checkpoint for recovery
    async fn create_final_checkpoint(&self) -> Result<(), StorageError> {
        info!("Creating final shutdown checkpoint...");
        
        let db = self.db.read().await;
        
        // Get current state
        let height = db.get_height()?;
        let best_hash = db.get_block_hash_by_height(height)?
            .ok_or(StorageError::KeyNotFound("Best block hash".to_string()))?;
        
        // Store shutdown checkpoint metadata
        let checkpoint_data = serde_json::json!({
            "type": "shutdown_checkpoint",
            "height": height,
            "best_hash": hex::encode(best_hash),
            "timestamp": chrono::Utc::now().timestamp(),
            "clean_shutdown": true,
        });
        
        db.store_metadata(
            b"last_shutdown_checkpoint",
            checkpoint_data.to_string().as_bytes()
        )?;
        
        // Flush checkpoint to ensure it's persisted
        db.flush()?;
        
        info!("Created shutdown checkpoint at height {}", height);
        
        Ok(())
    }
    
    /// Verify database integrity before shutdown
    async fn verify_integrity(&self) -> Result<(), StorageError> {
        info!("Verifying database integrity before shutdown...");
        
        let db = self.db.read().await;
        
        // Quick integrity check
        let result = db.verify_integrity(
            super::database::IntegrityCheckLevel::Quick,
            false // Don't repair, just check
        )?;
        
        if !result.passed {
            error!("Database integrity check failed: {} issues found", result.issues.len());
            // Log critical issues
            for issue in result.issues.iter().filter(|i| i.is_critical) {
                error!("Critical issue: {} in tree {}", issue.description, issue.tree);
            }
            // We still allow shutdown but log the issues
        } else {
            info!("Database integrity check passed");
        }
        
        Ok(())
    }
    
    /// Compact the database to reclaim space
    async fn compact_database(&self) -> Result<(), StorageError> {
        info!("Compacting database before shutdown...");
        
        let db = self.db.read().await;
        
        // This can take a while, so we do it with a timeout
        let compact_result = tokio::time::timeout(
            Duration::from_secs(60),
            tokio::task::spawn_blocking({
                let db = db.db().clone();
                move || -> Result<(), StorageError> {
                    // Note: sled doesn't have a built-in compact method
                    // Compaction happens automatically in the background
                    // We can force a flush to ensure all data is written
                    db.flush()?;
                    Ok(())
                }
            })
        ).await;
        
        match compact_result {
            Ok(Ok(Ok(()))) => info!("Database flush completed"),
            Ok(Ok(Err(e))) => warn!("Database flush failed: {}", e),
            Ok(Err(e)) => warn!("Database flush task failed: {}", e),
            Err(_) => warn!("Database flush timed out"),
        }
        
        Ok(())
    }
    
    /// Close the write-ahead log
    async fn close_wal(&self) -> Result<(), StorageError> {
        info!("Closing write-ahead log...");
        
        let mut wal = self.wal.write().await;
        
        // Ensure all entries are flushed
        wal.flush().await?;
        
        // Mark the log as cleanly closed
        wal.mark_clean_shutdown().await?;
        
        Ok(())
    }
    
    /// Final database sync and close
    async fn close_database(&self) -> Result<(), StorageError> {
        info!("Performing final database sync and close...");
        
        let db = self.db.read().await;
        
        // Clear the shutdown flag
        db.store_metadata(b"shutdown_in_progress", b"false")?;
        
        // Store clean shutdown marker
        let shutdown_time = chrono::Utc::now().timestamp().to_string();
        db.store_metadata(b"last_clean_shutdown", shutdown_time.as_bytes())?;
        
        // Final flush
        db.flush()?;
        
        // Note: sled doesn't have an explicit close method, but dropping
        // the database handle will close it. The flush above ensures
        // all data is persisted.
        
        info!("Database closed successfully");
        
        Ok(())
    }
    
    /// Emergency shutdown procedure (faster but less safe)
    pub async fn emergency_shutdown(&self) -> Result<(), StorageError> {
        warn!("Performing emergency database shutdown!");
        
        let db = self.db.read().await;
        
        // Just flush and mark as unclean shutdown
        db.store_metadata(b"emergency_shutdown", b"true")?;
        db.flush()?;
        
        warn!("Emergency shutdown completed - database may require recovery on next start");
        
        Ok(())
    }
}

/// Database startup handler to detect and recover from improper shutdowns
pub struct DatabaseStartupHandler {
    /// Path to the database
    db_path: std::path::PathBuf,
    /// Write-ahead log for recovery
    wal: Arc<RwLock<WriteAheadLog>>,
}

impl DatabaseStartupHandler {
    /// Create a new startup handler
    pub fn new<P: AsRef<Path>>(db_path: P, wal: Arc<RwLock<WriteAheadLog>>) -> Self {
        Self {
            db_path: db_path.as_ref().to_path_buf(),
            wal,
        }
    }
    
    /// Check if the database was cleanly shut down
    pub async fn check_clean_shutdown(&self, db: &BlockchainDB) -> Result<bool, StorageError> {
        // Check shutdown flags
        if let Some(shutdown_flag) = db.get_metadata(b"shutdown_in_progress")? {
            if shutdown_flag == b"true" {
                warn!("Database was not cleanly shut down - shutdown was in progress");
                return Ok(false);
            }
        }
        
        if let Some(_) = db.get_metadata(b"emergency_shutdown")? {
            warn!("Database underwent emergency shutdown");
            // Clear the emergency flag
            db.store_metadata(b"emergency_shutdown", b"")?;
            return Ok(false);
        }
        
        if let Some(last_shutdown) = db.get_metadata(b"last_clean_shutdown")? {
            if let Ok(timestamp_str) = std::str::from_utf8(&last_shutdown) {
                if let Ok(timestamp) = timestamp_str.parse::<i64>() {
                    let shutdown_time = chrono::DateTime::from_timestamp(timestamp, 0)
                        .unwrap_or(chrono::DateTime::from_timestamp(0, 0).unwrap());
                    info!("Last clean shutdown was at {}", shutdown_time);
                    return Ok(true);
                }
            }
        }
        
        warn!("No clean shutdown record found");
        Ok(false)
    }
    
    /// Perform database recovery if needed
    pub async fn recover_if_needed(&self, db: &mut BlockchainDB) -> Result<(), StorageError> {
        let was_clean = self.check_clean_shutdown(db).await?;
        
        if !was_clean {
            info!("Starting database recovery...");
            
            // Step 1: Check and replay WAL
            self.replay_wal(db).await?;
            
            // Step 2: Verify and repair database integrity
            self.verify_and_repair(db).await?;
            
            // Step 3: Rebuild indices if needed
            self.rebuild_indices_if_needed(db).await?;
            
            // Step 4: Clear any stale locks or flags
            self.clear_stale_state(db).await?;
            
            info!("Database recovery completed");
        }
        
        Ok(())
    }
    
    /// Replay write-ahead log entries
    async fn replay_wal(&self, db: &mut BlockchainDB) -> Result<(), StorageError> {
        info!("Replaying write-ahead log...");
        
        let mut wal = self.wal.write().await;
        let entries = wal.get_pending_entries().await?;
        
        if entries.is_empty() {
            info!("No pending WAL entries to replay");
            return Ok(());
        }
        
        info!("Found {} WAL entries to replay", entries.len());
        
        for entry in entries {
            match entry {
                JournalEntry::BlockWrite { hash, data } => {
                    db.store_block(&hash, &data)?;
                }
                JournalEntry::TransactionWrite { hash, data } => {
                    db.store_transaction(&hash, &data)?;
                }
                JournalEntry::MetadataWrite { key, value } => {
                    db.store_metadata(&key, &value)?;
                }
                _ => {
                    // Handle other entry types
                }
            }
        }
        
        // Clear the replayed entries
        wal.clear_pending().await?;
        
        info!("WAL replay completed");
        Ok(())
    }
    
    /// Verify and repair database integrity
    async fn verify_and_repair(&self, db: &mut BlockchainDB) -> Result<(), StorageError> {
        info!("Verifying database integrity...");
        
        let result = db.verify_integrity(
            super::database::IntegrityCheckLevel::Comprehensive,
            true // Enable repair
        )?;
        
        if !result.passed {
            warn!("Found {} integrity issues, attempted repairs", result.issues.len());
        } else {
            info!("Database integrity verified");
        }
        
        Ok(())
    }
    
    /// Rebuild indices if they're corrupted
    async fn rebuild_indices_if_needed(&self, db: &mut BlockchainDB) -> Result<(), StorageError> {
        // Check if indices need rebuilding by doing a quick consistency check
        let needs_rebuild = false; // TODO: Implement index consistency check
        
        if needs_rebuild {
            info!("Rebuilding database indices...");
            // TODO: Implement index rebuilding
        }
        
        Ok(())
    }
    
    /// Clear any stale state from improper shutdown
    async fn clear_stale_state(&self, db: &BlockchainDB) -> Result<(), StorageError> {
        // Clear shutdown in progress flag
        db.store_metadata(b"shutdown_in_progress", b"false")?;
        
        // Clear any other stale flags
        // TODO: Add more cleanup as needed
        
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    
    #[tokio::test]
    async fn test_clean_shutdown() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test_db");
        
        // Create database
        let db = BlockchainDB::new(&db_path).unwrap();
        let db = Arc::new(RwLock::new(db));
        
        // Create WAL
        let wal_path = temp_dir.path().join("wal");
        let wal = WriteAheadLog::new(&wal_path).await.unwrap();
        let wal = Arc::new(RwLock::new(wal));
        
        // Create shutdown handler
        let shutdown_handler = DatabaseShutdownHandler::new(
            db.clone(),
            wal.clone(),
            ShutdownConfig::default()
        );
        
        // Perform shutdown
        shutdown_handler.shutdown().await.unwrap();
        
        // Create new database and check if it was cleanly shut down
        let db2 = BlockchainDB::new(&db_path).unwrap();
        let startup_handler = DatabaseStartupHandler::new(&db_path, wal);
        
        let was_clean = startup_handler.check_clean_shutdown(&db2).await.unwrap();
        assert!(was_clean);
    }
    
    #[tokio::test]
    async fn test_recovery_after_crash() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test_db");
        
        // Create database and simulate crash (no clean shutdown)
        {
            let db = BlockchainDB::new(&db_path).unwrap();
            db.store_metadata(b"shutdown_in_progress", b"true").unwrap();
            // Drop without clean shutdown
        }
        
        // Create new database and check recovery
        let mut db = BlockchainDB::new(&db_path).unwrap();
        let wal_path = temp_dir.path().join("wal");
        let wal = WriteAheadLog::new(&wal_path).await.unwrap();
        let wal = Arc::new(RwLock::new(wal));
        
        let startup_handler = DatabaseStartupHandler::new(&db_path, wal);
        
        let was_clean = startup_handler.check_clean_shutdown(&db).await.unwrap();
        assert!(!was_clean);
        
        // Perform recovery
        startup_handler.recover_if_needed(&mut db).await.unwrap();
        
        // Verify recovery cleared the flag
        let flag = db.get_metadata(b"shutdown_in_progress").unwrap();
        assert_eq!(flag, Some(b"false".to_vec().into()));
    }
} 