use super::database::{BlockchainDB, StorageError};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::fs;

pub struct BackupManager {
    db: BlockchainDB,
    backup_dir: PathBuf,
    max_backups: usize,
}

impl BackupManager {
    pub fn new(db: BlockchainDB, backup_dir: PathBuf, max_backups: usize) -> Self {
        Self {
            db,
            backup_dir,
            max_backups,
        }
    }

    /// Create a new backup
    pub async fn create_backup(&self) -> Result<PathBuf, StorageError> {
        // Flush pending writes
        self.db.flush()?;

        // Create backup filename with timestamp
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let backup_path = self.backup_dir.join(format!("backup_{}.db", timestamp));

        // Copy database files
        fs::copy(self.db.path(), &backup_path).await?;

        // Cleanup old backups
        self.cleanup_old_backups().await?;

        Ok(backup_path)
    }

    /// Restore from backup
    pub async fn restore_from_backup(&self, backup_path: &Path) -> Result<(), StorageError> {
        // Stop database operations
        self.db.flush()?;

        // Restore from backup
        fs::copy(backup_path, self.db.path()).await?;

        Ok(())
    }

    /// Clean up old backups, keeping only max_backups most recent
    async fn cleanup_old_backups(&self) -> Result<(), StorageError> {
        let mut backups = fs::read_dir(&self.backup_dir)
            .await?
            .filter_map(|entry| entry.ok())
            .collect::<Vec<_>>();

        if backups.len() > self.max_backups {
            // Sort by modification time
            backups.sort_by_key(|entry| entry.metadata().unwrap().modified().unwrap());

            // Remove oldest backups
            for entry in backups.iter().take(backups.len() - self.max_backups) {
                fs::remove_file(entry.path()).await?;
            }
        }

        Ok(())
    }
}