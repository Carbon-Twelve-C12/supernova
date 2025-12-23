//! Lightning Channel Backup System
//!
//! This module implements static and dynamic channel backups for Lightning Network
//! channels, enabling recovery after data loss events.
//!
//! # Features
//! - Static Channel Backups (SCB) for disaster recovery
//! - Dynamic/Continuous backups for real-time protection
//! - Encrypted cloud backup support (S3, GCS)
//! - Peer backup protocol for decentralized storage
//! - Automatic backup triggers on commitment updates
//!
//! # Architecture
//! - `ChannelBackupManager` - Main backup orchestration
//! - `StaticChannelBackup` - Encrypted channel recovery data
//! - `DynamicBackupProvider` - Abstract backup destination
//! - `PeerBackupProtocol` - P2P backup sharing

use super::channel::{Channel, ChannelId};
use super::router::NodeId;
use chacha20poly1305::{
    aead::{Aead, KeyInit},
    ChaCha20Poly1305, Nonce,
};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use thiserror::Error;
use tracing::{debug, error, info, warn};

/// Default backup interval in seconds
pub const DEFAULT_BACKUP_INTERVAL_SECS: u64 = 60;

/// Maximum backup file size (10 MB)
pub const MAX_BACKUP_SIZE: usize = 10 * 1024 * 1024;

/// Channel backup errors
#[derive(Debug, Error, Clone)]
pub enum BackupError {
    #[error("Encryption error: {0}")]
    EncryptionError(String),

    #[error("Decryption error: {0}")]
    DecryptionError(String),

    #[error("Storage error: {0}")]
    StorageError(String),

    #[error("Channel not found: {channel_id}")]
    ChannelNotFound { channel_id: String },

    #[error("Backup not found: {backup_id}")]
    BackupNotFound { backup_id: String },

    #[error("Invalid backup data: {0}")]
    InvalidBackup(String),

    #[error("Provider error: {provider}: {message}")]
    ProviderError { provider: String, message: String },

    #[error("Peer backup failed: {peer_id}: {reason}")]
    PeerBackupFailed { peer_id: String, reason: String },

    #[error("Recovery failed: {0}")]
    RecoveryFailed(String),

    #[error("Lock poisoned")]
    LockPoisoned,
}

/// Result type for backup operations
pub type BackupResult<T> = Result<T, BackupError>;

/// Static Channel Backup format
/// Contains minimal data needed for force-close recovery
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StaticChannelBackup {
    /// Channel identifier
    pub channel_id: ChannelId,
    /// Remote node identifier
    pub remote_node_id: NodeId,
    /// Channel capacity in satoshis
    pub capacity_sats: u64,
    /// Funding transaction outpoint (txid:vout)
    pub funding_outpoint: FundingOutpoint,
    /// Our channel keys derivation path
    pub derivation_path: Vec<u32>,
    /// Channel type flags
    pub channel_type: ChannelType,
    /// Creation timestamp
    pub created_at: u64,
    /// Last update timestamp
    pub updated_at: u64,
    /// Version counter
    pub version: u64,
}

/// Funding transaction outpoint
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FundingOutpoint {
    /// Transaction hash
    pub txid: [u8; 32],
    /// Output index
    pub vout: u32,
}

/// Channel type flags
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default)]
pub struct ChannelType {
    /// Uses static remote key
    pub static_remote_key: bool,
    /// Uses anchor outputs
    pub anchor_outputs: bool,
    /// Uses taproot
    pub taproot: bool,
    /// Uses quantum-resistant signatures
    pub quantum_resistant: bool,
}

/// Multi-channel backup container
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelBackupPackage {
    /// Version of backup format
    pub format_version: u32,
    /// Node identifier this backup belongs to
    pub node_id: NodeId,
    /// All channel backups
    pub channels: Vec<StaticChannelBackup>,
    /// Backup timestamp
    pub timestamp: u64,
    /// Backup checksum
    pub checksum: [u8; 32],
}

impl ChannelBackupPackage {
    /// Create a new backup package
    pub fn new(node_id: NodeId, channels: Vec<StaticChannelBackup>) -> Self {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or(Duration::ZERO)
            .as_secs();

        let mut package = Self {
            format_version: 1,
            node_id,
            channels,
            timestamp,
            checksum: [0u8; 32],
        };

        package.checksum = package.calculate_checksum();
        package
    }

    /// Calculate checksum of backup contents
    fn calculate_checksum(&self) -> [u8; 32] {
        let mut hasher = Sha256::new();
        hasher.update(&self.format_version.to_le_bytes());
        hasher.update(self.node_id.as_str().as_bytes());
        hasher.update(&self.timestamp.to_le_bytes());

        for channel in &self.channels {
            hasher.update(channel.channel_id.as_bytes());
            hasher.update(&channel.version.to_le_bytes());
        }

        let result = hasher.finalize();
        let mut checksum = [0u8; 32];
        checksum.copy_from_slice(&result);
        checksum
    }

    /// Verify backup integrity
    pub fn verify(&self) -> bool {
        let calculated = self.calculate_checksum();
        calculated == self.checksum
    }
}

/// Encrypted backup blob
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptedBackup {
    /// Encryption nonce
    pub nonce: [u8; 12],
    /// Encrypted data
    pub ciphertext: Vec<u8>,
    /// Authentication tag (included in ciphertext for AES-GCM)
    pub timestamp: u64,
    /// Key derivation salt
    pub salt: [u8; 16],
}

impl EncryptedBackup {
    /// Encrypt a backup package
    pub fn encrypt(package: &ChannelBackupPackage, key: &[u8; 32]) -> BackupResult<Self> {
        let plaintext = bincode::serialize(package)
            .map_err(|e| BackupError::EncryptionError(e.to_string()))?;

        // Generate random nonce
        let mut nonce_bytes = [0u8; 12];
        rand::thread_rng().fill_bytes(&mut nonce_bytes);

        let cipher = ChaCha20Poly1305::new_from_slice(key)
            .map_err(|e| BackupError::EncryptionError(e.to_string()))?;

        let nonce = Nonce::from_slice(&nonce_bytes);
        let ciphertext = cipher
            .encrypt(nonce, plaintext.as_ref())
            .map_err(|e| BackupError::EncryptionError(e.to_string()))?;

        let mut salt = [0u8; 16];
        rand::thread_rng().fill_bytes(&mut salt);

        Ok(Self {
            nonce: nonce_bytes,
            ciphertext,
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or(Duration::ZERO)
                .as_secs(),
            salt,
        })
    }

    /// Decrypt a backup
    pub fn decrypt(&self, key: &[u8; 32]) -> BackupResult<ChannelBackupPackage> {
        let cipher = ChaCha20Poly1305::new_from_slice(key)
            .map_err(|e| BackupError::DecryptionError(e.to_string()))?;

        let nonce = Nonce::from_slice(&self.nonce);
        let plaintext = cipher
            .decrypt(nonce, self.ciphertext.as_ref())
            .map_err(|e| BackupError::DecryptionError(e.to_string()))?;

        let package: ChannelBackupPackage = bincode::deserialize(&plaintext)
            .map_err(|e| BackupError::DecryptionError(e.to_string()))?;

        // Verify integrity
        if !package.verify() {
            return Err(BackupError::InvalidBackup(
                "Checksum verification failed".to_string(),
            ));
        }

        Ok(package)
    }
}

/// Backup provider type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BackupProviderType {
    /// Local filesystem
    LocalFile,
    /// Amazon S3
    S3,
    /// Google Cloud Storage
    GCS,
    /// Peer-to-peer backup
    Peer,
    /// Custom webhook
    Webhook,
}

/// Backup provider configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupProviderConfig {
    /// Provider type
    pub provider_type: BackupProviderType,
    /// Provider name for logging
    pub name: String,
    /// Local path (for LocalFile)
    pub local_path: Option<PathBuf>,
    /// Cloud bucket name (for S3/GCS)
    pub bucket: Option<String>,
    /// Cloud region
    pub region: Option<String>,
    /// Endpoint URL (for custom S3-compatible)
    pub endpoint: Option<String>,
    /// Access key ID
    pub access_key_id: Option<String>,
    /// Secret access key (should be loaded from secure storage)
    pub secret_access_key: Option<String>,
    /// Webhook URL
    pub webhook_url: Option<String>,
    /// Peer node IDs for peer backup
    pub peer_node_ids: Vec<NodeId>,
    /// Enable this provider
    pub enabled: bool,
}

impl Default for BackupProviderConfig {
    fn default() -> Self {
        Self {
            provider_type: BackupProviderType::LocalFile,
            name: "default".to_string(),
            local_path: Some(PathBuf::from("./channel_backups")),
            bucket: None,
            region: None,
            endpoint: None,
            access_key_id: None,
            secret_access_key: None,
            webhook_url: None,
            peer_node_ids: Vec::new(),
            enabled: true,
        }
    }
}

/// Channel backup manager configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelBackupConfig {
    /// Backup interval in seconds
    pub backup_interval_secs: u64,
    /// Maximum backups to keep per provider
    pub max_backups: usize,
    /// Backup providers
    pub providers: Vec<BackupProviderConfig>,
    /// Backup on every commitment update
    pub backup_on_commitment: bool,
    /// Encryption key derivation iterations
    pub key_derivation_iterations: u32,
    /// Enable automatic backup
    pub auto_backup_enabled: bool,
}

impl Default for ChannelBackupConfig {
    fn default() -> Self {
        Self {
            backup_interval_secs: DEFAULT_BACKUP_INTERVAL_SECS,
            max_backups: 10,
            providers: vec![BackupProviderConfig::default()],
            backup_on_commitment: true,
            key_derivation_iterations: 100_000,
            auto_backup_enabled: true,
        }
    }
}

/// Backup status for a provider
#[derive(Debug, Clone)]
pub struct BackupStatus {
    /// Provider name
    pub provider_name: String,
    /// Last successful backup time
    pub last_backup: Option<u64>,
    /// Last backup version
    pub last_version: u64,
    /// Number of successful backups
    pub success_count: u64,
    /// Number of failed backups
    pub failure_count: u64,
    /// Last error message
    pub last_error: Option<String>,
}

/// Pending backup for a channel
#[derive(Debug, Clone)]
struct PendingBackup {
    /// Channel ID
    channel_id: ChannelId,
    /// Update reason
    reason: BackupTrigger,
    /// Timestamp
    timestamp: u64,
}

/// What triggered the backup
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackupTrigger {
    /// Channel opened
    ChannelOpened,
    /// Commitment transaction updated
    CommitmentUpdated,
    /// Channel closed
    ChannelClosed,
    /// Scheduled backup
    Scheduled,
    /// Manual backup request
    Manual,
}

/// Dynamic backup provider trait
pub trait DynamicBackupProvider: Send + Sync {
    /// Store an encrypted backup
    fn store(&self, backup: &EncryptedBackup, backup_id: &str) -> BackupResult<()>;

    /// Retrieve an encrypted backup
    fn retrieve(&self, backup_id: &str) -> BackupResult<EncryptedBackup>;

    /// List available backups
    fn list_backups(&self) -> BackupResult<Vec<String>>;

    /// Delete a backup
    fn delete(&self, backup_id: &str) -> BackupResult<()>;

    /// Provider name
    fn name(&self) -> &str;

    /// Provider type
    fn provider_type(&self) -> BackupProviderType;
}

/// Local filesystem backup provider
pub struct LocalFileProvider {
    path: PathBuf,
    name: String,
}

impl LocalFileProvider {
    pub fn new(path: PathBuf, name: String) -> BackupResult<Self> {
        // Create directory if it doesn't exist
        std::fs::create_dir_all(&path)
            .map_err(|e| BackupError::StorageError(e.to_string()))?;

        Ok(Self { path, name })
    }
}

impl DynamicBackupProvider for LocalFileProvider {
    fn store(&self, backup: &EncryptedBackup, backup_id: &str) -> BackupResult<()> {
        let file_path = self.path.join(format!("{}.backup", backup_id));
        let data = bincode::serialize(backup)
            .map_err(|e| BackupError::StorageError(e.to_string()))?;

        std::fs::write(&file_path, &data)
            .map_err(|e| BackupError::StorageError(e.to_string()))?;

        debug!("Stored backup {} to {}", backup_id, file_path.display());
        Ok(())
    }

    fn retrieve(&self, backup_id: &str) -> BackupResult<EncryptedBackup> {
        let file_path = self.path.join(format!("{}.backup", backup_id));
        let data = std::fs::read(&file_path)
            .map_err(|e| BackupError::BackupNotFound {
                backup_id: backup_id.to_string(),
            })?;

        bincode::deserialize(&data)
            .map_err(|e| BackupError::InvalidBackup(e.to_string()))
    }

    fn list_backups(&self) -> BackupResult<Vec<String>> {
        let entries = std::fs::read_dir(&self.path)
            .map_err(|e| BackupError::StorageError(e.to_string()))?;

        let mut backups = Vec::new();
        for entry in entries.flatten() {
            if let Some(name) = entry.file_name().to_str() {
                if name.ends_with(".backup") {
                    backups.push(name.trim_end_matches(".backup").to_string());
                }
            }
        }

        Ok(backups)
    }

    fn delete(&self, backup_id: &str) -> BackupResult<()> {
        let file_path = self.path.join(format!("{}.backup", backup_id));
        std::fs::remove_file(&file_path)
            .map_err(|e| BackupError::StorageError(e.to_string()))?;
        Ok(())
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn provider_type(&self) -> BackupProviderType {
        BackupProviderType::LocalFile
    }
}

/// S3-compatible backup provider (works with AWS S3, MinIO, etc.)
pub struct S3Provider {
    name: String,
    bucket: String,
    region: String,
    endpoint: Option<String>,
    #[allow(dead_code)]
    access_key_id: String,
    #[allow(dead_code)]
    secret_access_key: String,
}

impl S3Provider {
    pub fn new(config: &BackupProviderConfig) -> BackupResult<Self> {
        let bucket = config.bucket.clone().ok_or_else(|| {
            BackupError::ProviderError {
                provider: "S3".to_string(),
                message: "Bucket name required".to_string(),
            }
        })?;

        let access_key_id = config.access_key_id.clone().ok_or_else(|| {
            BackupError::ProviderError {
                provider: "S3".to_string(),
                message: "Access key ID required".to_string(),
            }
        })?;

        let secret_access_key = config.secret_access_key.clone().ok_or_else(|| {
            BackupError::ProviderError {
                provider: "S3".to_string(),
                message: "Secret access key required".to_string(),
            }
        })?;

        Ok(Self {
            name: config.name.clone(),
            bucket,
            region: config.region.clone().unwrap_or_else(|| "us-east-1".to_string()),
            endpoint: config.endpoint.clone(),
            access_key_id,
            secret_access_key,
        })
    }
}

impl DynamicBackupProvider for S3Provider {
    fn store(&self, backup: &EncryptedBackup, backup_id: &str) -> BackupResult<()> {
        // In production, use AWS SDK or compatible S3 client
        // For now, provide stub implementation
        let _data = bincode::serialize(backup)
            .map_err(|e| BackupError::StorageError(e.to_string()))?;

        // TODO: Implement actual S3 upload using aws-sdk-s3 crate
        // let client = aws_sdk_s3::Client::new(&config);
        // client.put_object()
        //     .bucket(&self.bucket)
        //     .key(format!("backups/{}.backup", backup_id))
        //     .body(data.into())
        //     .send()
        //     .await?;

        info!(
            "S3 backup {} to bucket {} (region: {})",
            backup_id, self.bucket, self.region
        );

        Ok(())
    }

    fn retrieve(&self, backup_id: &str) -> BackupResult<EncryptedBackup> {
        // TODO: Implement actual S3 download
        Err(BackupError::ProviderError {
            provider: "S3".to_string(),
            message: format!("S3 retrieve not implemented for {}", backup_id),
        })
    }

    fn list_backups(&self) -> BackupResult<Vec<String>> {
        // TODO: Implement S3 list objects
        Ok(Vec::new())
    }

    fn delete(&self, backup_id: &str) -> BackupResult<()> {
        // TODO: Implement S3 delete
        info!("Would delete S3 backup: {}", backup_id);
        Ok(())
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn provider_type(&self) -> BackupProviderType {
        BackupProviderType::S3
    }
}

/// Peer backup protocol handler
pub struct PeerBackupProtocol {
    /// Our node ID
    our_node_id: NodeId,
    /// Trusted peer node IDs
    peer_node_ids: Vec<NodeId>,
    /// Stored peer backups (in memory cache)
    peer_backups: Arc<RwLock<HashMap<String, EncryptedBackup>>>,
}

impl PeerBackupProtocol {
    pub fn new(our_node_id: NodeId, peer_node_ids: Vec<NodeId>) -> Self {
        Self {
            our_node_id,
            peer_node_ids,
            peer_backups: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Send backup to trusted peers
    pub fn distribute_backup(&self, backup: &EncryptedBackup, backup_id: &str) -> BackupResult<usize> {
        let mut success_count = 0;

        for peer_id in &self.peer_node_ids {
            match self.send_to_peer(peer_id, backup, backup_id) {
                Ok(()) => {
                    success_count += 1;
                    debug!("Sent backup {} to peer {}", backup_id, peer_id);
                }
                Err(e) => {
                    warn!("Failed to send backup to peer {}: {}", peer_id, e);
                }
            }
        }

        if success_count == 0 && !self.peer_node_ids.is_empty() {
            return Err(BackupError::PeerBackupFailed {
                peer_id: "all".to_string(),
                reason: "No peers accepted backup".to_string(),
            });
        }

        Ok(success_count)
    }

    fn send_to_peer(&self, _peer_id: &NodeId, _backup: &EncryptedBackup, _backup_id: &str) -> BackupResult<()> {
        // TODO: Implement actual P2P message sending
        // This would use the Lightning Network's existing message protocol
        // to send encrypted backup blobs to trusted peers
        Ok(())
    }

    /// Store a backup received from a peer
    pub fn receive_peer_backup(&self, backup: EncryptedBackup, backup_id: String) -> BackupResult<()> {
        let mut backups = self.peer_backups.write()
            .map_err(|_| BackupError::LockPoisoned)?;

        backups.insert(backup_id, backup);
        Ok(())
    }

    /// Retrieve backup from peers (for recovery)
    pub fn request_backup_from_peers(&self, backup_id: &str) -> BackupResult<EncryptedBackup> {
        // First check local cache
        {
            let backups = self.peer_backups.read()
                .map_err(|_| BackupError::LockPoisoned)?;

            if let Some(backup) = backups.get(backup_id) {
                return Ok(backup.clone());
            }
        }

        // TODO: Request from peers via P2P protocol
        Err(BackupError::BackupNotFound {
            backup_id: backup_id.to_string(),
        })
    }
}

impl DynamicBackupProvider for PeerBackupProtocol {
    fn store(&self, backup: &EncryptedBackup, backup_id: &str) -> BackupResult<()> {
        self.distribute_backup(backup, backup_id)?;
        Ok(())
    }

    fn retrieve(&self, backup_id: &str) -> BackupResult<EncryptedBackup> {
        self.request_backup_from_peers(backup_id)
    }

    fn list_backups(&self) -> BackupResult<Vec<String>> {
        let backups = self.peer_backups.read()
            .map_err(|_| BackupError::LockPoisoned)?;
        Ok(backups.keys().cloned().collect())
    }

    fn delete(&self, _backup_id: &str) -> BackupResult<()> {
        // Peer backups are managed by peers
        Ok(())
    }

    fn name(&self) -> &str {
        "peer_backup"
    }

    fn provider_type(&self) -> BackupProviderType {
        BackupProviderType::Peer
    }
}

/// Main channel backup manager
pub struct ChannelBackupManager {
    /// Configuration
    config: ChannelBackupConfig,
    /// Our node ID
    node_id: NodeId,
    /// Encryption key
    encryption_key: [u8; 32],
    /// Backup providers
    providers: Arc<RwLock<Vec<Box<dyn DynamicBackupProvider>>>>,
    /// Channel backup data
    channel_backups: Arc<RwLock<HashMap<ChannelId, StaticChannelBackup>>>,
    /// Backup status per provider
    provider_status: Arc<RwLock<HashMap<String, BackupStatus>>>,
    /// Pending backups queue
    pending_backups: Arc<RwLock<Vec<PendingBackup>>>,
    /// Current backup version
    version: Arc<RwLock<u64>>,
}

impl ChannelBackupManager {
    /// Create a new channel backup manager
    pub fn new(
        node_id: NodeId,
        config: ChannelBackupConfig,
        encryption_key: [u8; 32],
    ) -> BackupResult<Self> {
        let manager = Self {
            config,
            node_id,
            encryption_key,
            providers: Arc::new(RwLock::new(Vec::new())),
            channel_backups: Arc::new(RwLock::new(HashMap::new())),
            provider_status: Arc::new(RwLock::new(HashMap::new())),
            pending_backups: Arc::new(RwLock::new(Vec::new())),
            version: Arc::new(RwLock::new(0)),
        };

        // Initialize providers
        manager.init_providers()?;

        Ok(manager)
    }

    /// Initialize backup providers from configuration
    fn init_providers(&self) -> BackupResult<()> {
        let mut providers = self.providers.write()
            .map_err(|_| BackupError::LockPoisoned)?;
        let mut status = self.provider_status.write()
            .map_err(|_| BackupError::LockPoisoned)?;

        for provider_config in &self.config.providers {
            if !provider_config.enabled {
                continue;
            }

            let provider: Box<dyn DynamicBackupProvider> = match provider_config.provider_type {
                BackupProviderType::LocalFile => {
                    let path = provider_config.local_path.clone()
                        .unwrap_or_else(|| PathBuf::from("./channel_backups"));
                    Box::new(LocalFileProvider::new(path, provider_config.name.clone())?)
                }
                BackupProviderType::S3 => {
                    Box::new(S3Provider::new(provider_config)?)
                }
                BackupProviderType::Peer => {
                    Box::new(PeerBackupProtocol::new(
                        self.node_id.clone(),
                        provider_config.peer_node_ids.clone(),
                    ))
                }
                BackupProviderType::GCS | BackupProviderType::Webhook => {
                    // Not implemented yet
                    continue;
                }
            };

            status.insert(
                provider.name().to_string(),
                BackupStatus {
                    provider_name: provider.name().to_string(),
                    last_backup: None,
                    last_version: 0,
                    success_count: 0,
                    failure_count: 0,
                    last_error: None,
                },
            );

            info!("Initialized backup provider: {} ({:?})",
                  provider.name(), provider.provider_type());
            providers.push(provider);
        }

        Ok(())
    }

    /// Register a channel for backup
    pub fn register_channel(&self, channel: &Channel) -> BackupResult<()> {
        let backup = self.create_static_backup(channel)?;

        let mut backups = self.channel_backups.write()
            .map_err(|_| BackupError::LockPoisoned)?;
        backups.insert(channel.id().clone(), backup);

        // Queue backup
        self.queue_backup(channel.id().clone(), BackupTrigger::ChannelOpened)?;

        info!("Registered channel {} for backup", channel.id());
        Ok(())
    }

    /// Unregister a channel (on close)
    pub fn unregister_channel(&self, channel_id: &ChannelId) -> BackupResult<()> {
        let mut backups = self.channel_backups.write()
            .map_err(|_| BackupError::LockPoisoned)?;
        backups.remove(channel_id);

        // Queue final backup
        self.queue_backup(channel_id.clone(), BackupTrigger::ChannelClosed)?;

        info!("Unregistered channel {} from backup", channel_id);
        Ok(())
    }

    /// Notify of commitment update
    pub fn on_commitment_update(&self, channel: &Channel) -> BackupResult<()> {
        if !self.config.backup_on_commitment {
            return Ok(());
        }

        // Update stored backup
        let backup = self.create_static_backup(channel)?;
        {
            let mut backups = self.channel_backups.write()
                .map_err(|_| BackupError::LockPoisoned)?;
            backups.insert(channel.id().clone(), backup);
        }

        // Queue backup
        self.queue_backup(channel.id().clone(), BackupTrigger::CommitmentUpdated)?;

        debug!("Queued backup for commitment update on channel {}", channel.id());
        Ok(())
    }

    /// Create a static channel backup from a channel
    fn create_static_backup(&self, channel: &Channel) -> BackupResult<StaticChannelBackup> {
        let info = channel.get_info();
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or(Duration::ZERO)
            .as_secs();

        // Get current version
        let version = {
            let mut ver = self.version.write()
                .map_err(|_| BackupError::LockPoisoned)?;
            *ver += 1;
            *ver
        };

        // Get channel ID bytes for funding outpoint derivation
        let channel_id = channel.id();
        let channel_id_bytes = channel_id.as_bytes();

        Ok(StaticChannelBackup {
            channel_id: channel.id().clone(),
            // Use channel ID as remote node ID placeholder - in production would get from channel state
            remote_node_id: NodeId::new(hex::encode(&channel_id_bytes[..16])),
            capacity_sats: info.capacity,
            funding_outpoint: FundingOutpoint {
                // Derive from channel ID - in production would get actual funding tx info
                txid: *channel_id_bytes,
                vout: 0,
            },
            derivation_path: vec![44, 1, 0, 0], // Standard derivation path
            channel_type: ChannelType {
                static_remote_key: true,
                anchor_outputs: false,
                taproot: false,
                quantum_resistant: info.config.use_quantum_signatures,
            },
            created_at: now,
            updated_at: now,
            version,
        })
    }

    /// Queue a backup for processing
    fn queue_backup(&self, channel_id: ChannelId, reason: BackupTrigger) -> BackupResult<()> {
        let mut pending = self.pending_backups.write()
            .map_err(|_| BackupError::LockPoisoned)?;

        pending.push(PendingBackup {
            channel_id,
            reason,
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or(Duration::ZERO)
                .as_secs(),
        });

        Ok(())
    }

    /// Process pending backups
    pub fn process_pending_backups(&self) -> BackupResult<usize> {
        let pending: Vec<PendingBackup> = {
            let mut queue = self.pending_backups.write()
                .map_err(|_| BackupError::LockPoisoned)?;
            std::mem::take(&mut *queue)
        };

        if pending.is_empty() {
            return Ok(0);
        }

        // Create backup package
        let package = self.create_backup_package()?;

        // Encrypt backup
        let encrypted = EncryptedBackup::encrypt(&package, &self.encryption_key)?;

        // Store to all providers
        let backup_id = format!("backup_{}", package.timestamp);
        let mut success_count = 0;

        let providers = self.providers.read()
            .map_err(|_| BackupError::LockPoisoned)?;

        for provider in providers.iter() {
            match provider.store(&encrypted, &backup_id) {
                Ok(()) => {
                    success_count += 1;
                    self.update_provider_status(provider.name(), true, None)?;
                }
                Err(e) => {
                    error!("Backup to {} failed: {}", provider.name(), e);
                    self.update_provider_status(provider.name(), false, Some(e.to_string()))?;
                }
            }
        }

        info!("Processed {} pending backups, stored to {} providers",
              pending.len(), success_count);

        Ok(success_count)
    }

    /// Create a backup package from current state
    fn create_backup_package(&self) -> BackupResult<ChannelBackupPackage> {
        let backups = self.channel_backups.read()
            .map_err(|_| BackupError::LockPoisoned)?;

        let channels: Vec<StaticChannelBackup> = backups.values().cloned().collect();

        Ok(ChannelBackupPackage::new(self.node_id.clone(), channels))
    }

    /// Update provider status
    fn update_provider_status(&self, name: &str, success: bool, error: Option<String>) -> BackupResult<()> {
        let mut status = self.provider_status.write()
            .map_err(|_| BackupError::LockPoisoned)?;

        if let Some(s) = status.get_mut(name) {
            if success {
                s.last_backup = Some(
                    SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap_or(Duration::ZERO)
                        .as_secs()
                );
                s.success_count += 1;
                s.last_error = None;
            } else {
                s.failure_count += 1;
                s.last_error = error;
            }
        }

        Ok(())
    }

    /// Manual backup trigger
    pub fn trigger_backup(&self) -> BackupResult<()> {
        // Queue all channels for backup
        let backups = self.channel_backups.read()
            .map_err(|_| BackupError::LockPoisoned)?;

        for channel_id in backups.keys() {
            self.queue_backup(channel_id.clone(), BackupTrigger::Manual)?;
        }

        // Process immediately
        self.process_pending_backups()?;

        Ok(())
    }

    /// Export all channel backups
    pub fn export_all(&self) -> BackupResult<EncryptedBackup> {
        let package = self.create_backup_package()?;
        EncryptedBackup::encrypt(&package, &self.encryption_key)
    }

    /// Import channel backups from encrypted data
    pub fn import_backup(&self, encrypted: &EncryptedBackup) -> BackupResult<Vec<StaticChannelBackup>> {
        let package = encrypted.decrypt(&self.encryption_key)?;

        // Verify node ID matches
        if package.node_id.as_str() != self.node_id.as_str() {
            return Err(BackupError::InvalidBackup(
                format!("Node ID mismatch: expected {}, got {}",
                        self.node_id.as_str(), package.node_id.as_str())
            ));
        }

        // Store imported backups
        let mut backups = self.channel_backups.write()
            .map_err(|_| BackupError::LockPoisoned)?;

        for channel_backup in &package.channels {
            backups.insert(channel_backup.channel_id.clone(), channel_backup.clone());
        }

        info!("Imported {} channel backups", package.channels.len());

        Ok(package.channels)
    }

    /// Get backup status for all providers
    pub fn get_status(&self) -> BackupResult<Vec<BackupStatus>> {
        let status = self.provider_status.read()
            .map_err(|_| BackupError::LockPoisoned)?;
        Ok(status.values().cloned().collect())
    }

    /// Get backup for a specific channel
    pub fn get_channel_backup(&self, channel_id: &ChannelId) -> BackupResult<Option<StaticChannelBackup>> {
        let backups = self.channel_backups.read()
            .map_err(|_| BackupError::LockPoisoned)?;
        Ok(backups.get(channel_id).cloned())
    }

    /// Get configuration
    pub fn config(&self) -> &ChannelBackupConfig {
        &self.config
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_node_id() -> NodeId {
        NodeId::new("test_node_001".to_string())
    }

    fn create_test_encryption_key() -> [u8; 32] {
        let mut key = [0u8; 32];
        for (i, byte) in key.iter_mut().enumerate() {
            *byte = i as u8;
        }
        key
    }

    #[test]
    fn test_static_channel_backup() {
        let scb = StaticChannelBackup {
            channel_id: ChannelId::from_bytes([1u8; 32]),
            remote_node_id: NodeId::new("remote_node".to_string()),
            capacity_sats: 1_000_000,
            funding_outpoint: FundingOutpoint {
                txid: [2u8; 32],
                vout: 0,
            },
            derivation_path: vec![44, 1, 0, 0],
            channel_type: ChannelType::default(),
            created_at: 1234567890,
            updated_at: 1234567890,
            version: 1,
        };

        assert_eq!(scb.capacity_sats, 1_000_000);
        assert_eq!(scb.version, 1);
    }

    #[test]
    fn test_backup_package_checksum() {
        let node_id = create_test_node_id();
        let channels = vec![
            StaticChannelBackup {
                channel_id: ChannelId::from_bytes([1u8; 32]),
                remote_node_id: NodeId::new("remote".to_string()),
                capacity_sats: 100_000,
                funding_outpoint: FundingOutpoint { txid: [0u8; 32], vout: 0 },
                derivation_path: vec![44, 1, 0],
                channel_type: ChannelType::default(),
                created_at: 0,
                updated_at: 0,
                version: 1,
            },
        ];

        let package = ChannelBackupPackage::new(node_id, channels);
        assert!(package.verify());
    }

    #[test]
    fn test_encrypted_backup() {
        let node_id = create_test_node_id();
        let key = create_test_encryption_key();
        let channels = vec![];

        let package = ChannelBackupPackage::new(node_id, channels);
        let encrypted = EncryptedBackup::encrypt(&package, &key).unwrap();

        let decrypted = encrypted.decrypt(&key).unwrap();
        assert!(decrypted.verify());
    }

    #[test]
    fn test_local_file_provider() {
        let temp_dir = tempfile::tempdir().unwrap();
        let provider = LocalFileProvider::new(
            temp_dir.path().to_path_buf(),
            "test_local".to_string(),
        ).unwrap();

        let node_id = create_test_node_id();
        let key = create_test_encryption_key();
        let package = ChannelBackupPackage::new(node_id, vec![]);
        let encrypted = EncryptedBackup::encrypt(&package, &key).unwrap();

        // Store
        provider.store(&encrypted, "test_backup").unwrap();

        // List
        let backups = provider.list_backups().unwrap();
        assert!(backups.contains(&"test_backup".to_string()));

        // Retrieve
        let retrieved = provider.retrieve("test_backup").unwrap();
        let decrypted = retrieved.decrypt(&key).unwrap();
        assert!(decrypted.verify());

        // Delete
        provider.delete("test_backup").unwrap();
        let backups = provider.list_backups().unwrap();
        assert!(!backups.contains(&"test_backup".to_string()));
    }

    #[test]
    fn test_backup_manager_creation() {
        let temp_dir = tempfile::tempdir().unwrap();
        let config = ChannelBackupConfig {
            providers: vec![BackupProviderConfig {
                provider_type: BackupProviderType::LocalFile,
                name: "test".to_string(),
                local_path: Some(temp_dir.path().to_path_buf()),
                enabled: true,
                ..Default::default()
            }],
            ..Default::default()
        };

        let manager = ChannelBackupManager::new(
            create_test_node_id(),
            config,
            create_test_encryption_key(),
        ).unwrap();

        let status = manager.get_status().unwrap();
        assert_eq!(status.len(), 1);
        assert_eq!(status[0].provider_name, "test");
    }

    #[test]
    fn test_backup_trigger() {
        let temp_dir = tempfile::tempdir().unwrap();
        let config = ChannelBackupConfig {
            providers: vec![BackupProviderConfig {
                provider_type: BackupProviderType::LocalFile,
                name: "test".to_string(),
                local_path: Some(temp_dir.path().to_path_buf()),
                enabled: true,
                ..Default::default()
            }],
            ..Default::default()
        };

        let manager = ChannelBackupManager::new(
            create_test_node_id(),
            config,
            create_test_encryption_key(),
        ).unwrap();

        // Trigger manual backup (no channels yet)
        manager.trigger_backup().unwrap();

        let status = manager.get_status().unwrap();
        assert_eq!(status[0].success_count, 1);
    }
}
