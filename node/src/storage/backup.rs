use super::database::{BlockchainDB, StorageError};
use super::persistence::ChainState;
use crate::metrics::BackupMetrics;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH, Duration};
use std::collections::{HashMap, HashSet};
use tokio::fs;
use tokio::sync::mpsc;
use tracing::{info, warn, error};
use std::sync::Arc;
use futures::future::join_all;
use sha2::{Sha256, Digest};

const CHECKPOINT_INTERVAL: u64 = 10000;
const PARALLEL_VERIFICATION_CHUNKS: usize = 4;
const MAX_RECOVERY_ATTEMPTS: usize = 3;
const INCREMENTAL_REBUILD_BATCH: usize = 1000;

#[derive(Debug, Clone)]
pub struct RecoveryCheckpoint {
    pub height: u64,
    pub block_hash: [u8; 32],
    pub utxo_hash: [u8; 32],
    pub timestamp: u64,
}

pub struct RecoveryManager {
    db: Arc<BlockchainDB>,
    backup_dir: PathBuf,
    chain_state: ChainState,
    metrics: BackupMetrics,
    checkpoints: HashMap<u64, RecoveryCheckpoint>,
    last_checkpoint: Option<RecoveryCheckpoint>,
}

pub struct BackupManager {
    db: Arc<BlockchainDB>,
    backup_dir: PathBuf,
    max_backups: usize,
    backup_interval: Duration,
    metrics: BackupMetrics,
}

impl BackupManager {
    pub fn new(
        db: Arc<BlockchainDB>, 
        backup_dir: PathBuf, 
        max_backups: usize,
        backup_interval: Duration,
    ) -> Self {
        Self {
            db,
            backup_dir,
            max_backups,
            backup_interval,
            metrics: BackupMetrics::new(),
        }
    }

    pub async fn verify_existing_backups(&self) -> Result<(), StorageError> {
        info!("Starting verification of existing backups");
        
        let mut entries = fs::read_dir(&self.backup_dir).await?;
        let mut verified_count = 0;
        let mut failed_count = 0;

        let failed_dir = self.backup_dir.join("failed");
        fs::create_dir_all(&failed_dir).await?;

        while let Some(entry) = entries.next_entry().await? {
            if entry.file_type().await?.is_file() {
                let path = entry.path();
                info!("Verifying backup: {:?}", path);
                
                let verification = self.metrics.record_verification_start();
                match self.verify_backup(&path).await {
                    Ok(true) => {
                        info!("Backup verification successful: {:?}", path);
                        verified_count += 1;
                        self.metrics.record_verification_success();
                    }
                    Ok(false) => {
                        warn!("Backup verification failed: {:?}", path);
                        failed_count += 1;
                        self.metrics.record_verification_failure();
                        
                        if let Some(file_name) = path.file_name() {
                            let failed_path = failed_dir.join(file_name);
                            if let Err(e) = fs::rename(&path, &failed_path).await {
                                error!("Failed to move corrupt backup: {}", e);
                            }
                        }
                    }
                    Err(e) => {
                        error!("Error verifying backup {:?}: {}", path, e);
                        failed_count += 1;
                        self.metrics.record_verification_failure();
                    }
                }
                verification.complete();
            }
        }

        info!("Backup verification complete: {} succeeded, {} failed", verified_count, failed_count);
        Ok(())
    }

    pub async fn start_automated_backups(&self) -> Result<(), StorageError> {
        info!("Starting automated backup system");
        
        if let Err(e) = self.verify_existing_backups().await {
            error!("Failed to verify existing backups: {}", e);
        }
        
        let mut interval = tokio::time::interval(self.backup_interval);
        
        loop {
            interval.tick().await;
            let backup_operation = self.metrics.record_backup_start();
            
            match self.create_backup().await {
                Ok(path) => {
                    if let Ok(metadata) = fs::metadata(&path).await {
                        backup_operation.complete(metadata.len());
                        info!("Created backup at {:?}", path);
                    }
                }
                Err(e) => {
                    error!("Backup creation failed: {}", e);
                    self.metrics.record_backup_failure();
                }
            }
            
            if let Err(e) = self.cleanup_old_backups().await {
                error!("Backup cleanup failed: {}", e);
            }
        }
    }

    async fn create_backup(&self) -> Result<PathBuf, StorageError> {
        fs::create_dir_all(&self.backup_dir).await?;

        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        
        let backup_path = self.backup_dir
            .join(format!("supernova_backup_{}.db", timestamp));

        self.db.flush()?;
        fs::copy(self.db.path(), &backup_path).await?;

        let verification = self.metrics.record_verification_start();
        match self.verify_backup(&backup_path).await {
            Ok(true) => {
                self.metrics.record_verification_success();
                verification.complete();
                Ok(backup_path)
            }
            Ok(false) => {
                self.metrics.record_verification_failure();
                verification.complete();
                fs::remove_file(&backup_path).await?;
                Err(StorageError::BackupVerificationFailed)
            }
            Err(e) => {
                self.metrics.record_verification_failure();
                verification.complete();
                Err(e)
            }
        }
    }

    async fn cleanup_old_backups(&self) -> Result<(), StorageError> {
        let mut backups = vec![];
        let mut entries = fs::read_dir(&self.backup_dir).await?;
        while let Some(entry) = entries.next_entry().await? {
            if entry.file_type().await?.is_file() {
                if let Ok(metadata) = entry.metadata().await {
                    backups.push((entry.path(), metadata.modified()?));
                }
            }
        }

        backups.sort_by(|a, b| b.1.cmp(&a.1));

        for (path, _) in backups.iter().skip(self.max_backups) {
            fs::remove_file(path).await?;
            info!("Removed old backup: {:?}", path);
        }

        Ok(())
    }

    async fn verify_backup(&self, backup_path: &Path) -> Result<bool, StorageError> {
        let temp_dir = tempfile::tempdir()?;
        let temp_path = temp_dir.path().join("temp.db");

        fs::copy(backup_path, &temp_path).await?;

        let temp_db = BlockchainDB::new(&temp_path)?;
        let chain_state = ChainState::new(Arc::new(temp_db))?;

        let mut recovery_manager = RecoveryManager::new(
            Arc::new(BlockchainDB::new(&temp_path)?),
            self.backup_dir.clone(),
            chain_state,
        );

        recovery_manager.verify_database_integrity().await
    }
}

impl RecoveryManager {
    pub fn new(db: Arc<BlockchainDB>, backup_dir: PathBuf, chain_state: ChainState) -> Self {
        Self {
            db,
            backup_dir,
            chain_state,
            metrics: BackupMetrics::new(),
            checkpoints: HashMap::new(),
            last_checkpoint: None,
        }
    }

    pub async fn verify_and_recover(&mut self) -> Result<(), StorageError> {
        info!("Starting database integrity verification");
        
        let verification = self.metrics.record_verification_start();
        let integrity_result = self.verify_database_integrity().await?;
        verification.complete();

        if !integrity_result {
            warn!("Database integrity check failed. Starting recovery process.");
            self.metrics.record_verification_failure();
            
            for attempt in 1..=MAX_RECOVERY_ATTEMPTS {
                info!("Recovery attempt {} of {}", attempt, MAX_RECOVERY_ATTEMPTS);
                match self.perform_recovery().await {
                    Ok(()) => {
                        info!("Recovery successful on attempt {}", attempt);
                        self.create_checkpoint().await?;
                        return Ok(());
                    }
                    Err(e) if attempt == MAX_RECOVERY_ATTEMPTS => {
                        error!("All recovery attempts failed: {}", e);
                        return Err(e);
                    }
                    Err(e) => {
                        warn!("Recovery attempt {} failed: {}. Retrying...", attempt, e);
                        continue;
                    }
                }
            }
        } else {
            self.metrics.record_verification_success();
            self.create_checkpoint().await?;
        }

        Ok(())
    }

    async fn create_checkpoint(&mut self) -> Result<(), StorageError> {
        let height = self.chain_state.get_height();
        if self.last_checkpoint.as_ref().map_or(true, |cp| height - cp.height >= CHECKPOINT_INTERVAL) {
            let block_hash = self.chain_state.get_best_block_hash();
            let utxo_hash = self.calculate_utxo_hash().await?;
            
            let checkpoint = RecoveryCheckpoint {
                height,
                block_hash,
                utxo_hash,
                timestamp: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
            };

            self.checkpoints.insert(height, checkpoint.clone());
            self.last_checkpoint = Some(checkpoint);
            self.save_checkpoints().await?;
        }
        Ok(())
    }

    async fn calculate_utxo_hash(&self) -> Result<[u8; 32], StorageError> {
        let mut hasher = Sha256::new();
        let mut utxos = Vec::new();
        
        let mut current_hash = self.chain_state.get_best_block_hash();
        let mut height = self.chain_state.get_height();

        while height > 0 {
            let block = self.db.get_block(&current_hash)?.unwrap();
            for tx in block.transactions() {
                let tx_hash = tx.hash();
                for (index, output) in tx.outputs().iter().enumerate() {
                    utxos.push((tx_hash, index as u32, output));
                }
            }
            current_hash = block.prev_block_hash();
            height -= 1;
        }

        utxos.sort_by_key(|&(hash, index, _)| (hash, index));

        for (hash, index, output) in utxos {
            hasher.update(&hash);
            hasher.update(&index.to_le_bytes());
            hasher.update(&bincode::serialize(output)?);
        }

        Ok(hasher.finalize().into())
    }

    async fn save_checkpoints(&self) -> Result<(), StorageError> {
        let checkpoint_data = bincode::serialize(&self.checkpoints)?;
        self.db.store_metadata(b"checkpoints", &checkpoint_data)?;
        Ok(())
    }

    async fn verify_checkpoint(&self, checkpoint: &RecoveryCheckpoint) -> Result<bool, StorageError> {
        let block = self.db.get_block(&checkpoint.block_hash)?
            .ok_or_else(|| StorageError::DatabaseError("Checkpoint block not found".to_string()))?;

        if block.height() != checkpoint.height {
            return Ok(false);
        }

        let current_utxo_hash = self.calculate_utxo_hash().await?;
        Ok(current_utxo_hash == checkpoint.utxo_hash)
    }

    pub async fn verify_database_integrity(&self) -> Result<bool, StorageError> {
        if !self.db.path().exists() {
            return Ok(false);
        }

        let (blockchain_valid, utxo_valid) = tokio::join!(
            self.verify_blockchain_parallel(),
            self.verify_utxo_set()
        );

        Ok(blockchain_valid? && utxo_valid?)
    }

    async fn verify_blockchain_parallel(&self) -> Result<bool, StorageError> {
        let height = self.chain_state.get_height();
        let chunk_size = height / PARALLEL_VERIFICATION_CHUNKS as u64;
        let mut tasks = Vec::new();

        for i in 0..PARALLEL_VERIFICATION_CHUNKS {
            let start = i as u64 * chunk_size;
            let end = if i == PARALLEL_VERIFICATION_CHUNKS - 1 {
                height
            } else {
                (i as u64 + 1) * chunk_size
            };

            let db = Arc::clone(&self.db);
            let chain_state = self.chain_state.clone();
            tasks.push(tokio::spawn(async move {
                verify_blockchain_range(db, chain_state, start, end).await
            }));
        }

        for result in join_all(tasks).await {
            if !result.unwrap()? {
                return Ok(false);
            }
        }

        Ok(true)
    }

    async fn verify_utxo_set(&self) -> Result<bool, StorageError> {
        info!("Verifying UTXO set integrity");
        
        let mut verification_utxos = std::collections::HashMap::new();
        let mut current_hash = self.chain_state.get_best_block_hash();
        let mut height = self.chain_state.get_height();

        while height > 0 {
            let block = self.db.get_block(&current_hash)?.unwrap();

            for tx in block.transactions() {
                for input in tx.inputs() {
                    verification_utxos.remove(&(input.prev_tx_hash(), input.prev_output_index()));
                }

                let tx_hash = tx.hash();
                for (index, output) in tx.outputs().iter().enumerate() {
                    verification_utxos.insert(
                        (tx_hash, index as u32),
                        bincode::serialize(output)?,
                    );
                }
            }

            current_hash = block.prev_block_hash();
            height -= 1;
        }

        Ok(true)
    }

    async fn perform_recovery(&mut self) -> Result<(), StorageError> {
        info!("Starting database recovery process");

        self.load_checkpoints().await?;

        if let Some(checkpoint) = self.last_checkpoint.as_ref() {
            info!("Found checkpoint at height {}", checkpoint.height);
            if self.verify_checkpoint(checkpoint).await? {
                info!("Checkpoint verified, recovering from checkpoint");
                self.recover_from_checkpoint(checkpoint).await?;
                return Ok(());
            }
        }

        if let Some(backup_path) = self.find_latest_backup().await? {
            info!("Found backup at {:?}", backup_path);
            self.restore_from_backup(&backup_path).await?;
        } else {
            warn!("No backup found. Performing full chain reconstruction");
            self.reconstruct_chain().await?;
        }

        info!("Recovery process completed successfully");
        Ok(())
    }

    async fn find_latest_backup(&self) -> Result<Option<PathBuf>, StorageError> {
        let mut entries = fs::read_dir(&self.backup_dir).await?;
        let mut latest_backup: Option<(PathBuf, SystemTime)> = None;

        while let Some(entry) = entries.next_entry().await? {
            if entry.file_type().await?.is_file() {
                if let Ok(metadata) = entry.metadata().await {
                    if let Ok(modified) = metadata.modified() {
                        match &latest_backup {
                            None => latest_backup = Some((entry.path(), modified)),
                            Some((_, latest_time)) if modified > *latest_time => {
                                latest_backup = Some((entry.path(), modified));
                            }
                            _ => {}
                        }
                    }
                }
            }
        }

        Ok(latest_backup.map(|(path, _)| path))
    }

    async fn restore_from_backup(&mut self, backup_path: &Path) -> Result<(), StorageError> {
        info!("Restoring from backup: {:?}", backup_path);

        fs::copy(backup_path, self.db.path()).await?;

        if !self.verify_database_integrity().await? {
            error!("Restored database failed integrity check");
            return Err(StorageError::RestoreError);
        }

        Ok(())
    }

    async fn reconstruct_chain(&mut self) -> Result<(), StorageError> {
        info!("Reconstructing chain from genesis");

        self.db.clear()?;
        self.rebuild_utxo_set().await?;

        Ok(())
    }

    async fn rebuild_utxo_set(&mut self) -> Result<(), StorageError> {
        info!("Rebuilding UTXO set");

        self.db.clear_utxos()?;

        let mut current_hash = self.chain_state.get_best_block_hash();
        let mut height = self.chain_state.get_height();

        while height > 0 {
            let block = self.db.get_block(&current_hash)?.unwrap();
            
            for tx in block.transactions() {
                let tx_hash = tx.hash();
                for (index, output) in tx.outputs().iter().enumerate() {
                    self.db.store_utxo(
                        &tx_hash,
                        index as u32,
                        &bincode::serialize(output)?,
                    )?;
                }
            }

            current_hash = block.prev_block_hash();
            height -= 1;
        }

        Ok(())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Database restore error")]
    RestoreError,
    #[error("Backup verification failed")]
    BackupVerificationFailed,
    #[error("Serialization error: {0}")]
    SerializationError(#[from] bincode::Error),
    #[error("Database error: {0}")]
    DatabaseError(String),
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_recovery_manager() -> Result<(), StorageError> {
        let temp_dir = tempdir().unwrap();
        let backup_dir = tempdir().unwrap();
        
        let db = Arc::new(BlockchainDB::new(temp_dir.path())?);
        let chain_state = ChainState::new(Arc::clone(&db))?;
        
        let mut recovery_manager = RecoveryManager::new(
            db,
            backup_dir.path().to_path_buf(),
            chain_state,
        );

        assert!(recovery_manager.verify_database_integrity().await?);

        Ok(())
    }

    #[tokio::test]
    async fn test_backup_manager() -> Result<(), StorageError> {
        let temp_dir = tempdir().unwrap();
        let backup_dir = tempdir().unwrap();
        let db = Arc::new(BlockchainDB::new(temp_dir.path())?);
        
        let backup_manager = BackupManager::new(
            Arc::clone(&db),
            backup_dir.path().to_path_buf(),
            5,
            Duration::from_secs(3600),
        );

        let backup_path = backup_manager.create_backup().await?;
        assert!(backup_path.exists());

        assert!(backup_manager.verify_backup(&backup_path).await?);

        Ok(())
    }

    #[tokio::test]
    async fn test_backup_rotation() -> Result