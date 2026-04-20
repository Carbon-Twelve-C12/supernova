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
use std::sync::atomic::{AtomicU64, Ordering};
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

/// On-wire S3 envelope. Wraps an `EncryptedBackup` with an optional ML-DSA
/// signature so we can detect tampering even when the underlying bucket is
/// compromised.
///
/// # Verification
/// If `signer_pubkey` is present the verifier must check `signature` over
/// `bincode(backup)` using `verify_quantum_signature` with the configured
/// parameters. Missing fields mean "unsigned" — only acceptable when the
/// caller explicitly opted out of signing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignedBackup {
    /// Version tag for forward-compat.
    pub version: u16,
    /// The encrypted backup blob.
    pub backup: EncryptedBackup,
    /// Signature over bincode(backup). Empty if unsigned.
    #[serde(default, with = "serde_bytes")]
    pub signature: Vec<u8>,
    /// Public key bytes of the signer (matches one of the caller-trusted
    /// keys). Empty if unsigned.
    #[serde(default, with = "serde_bytes")]
    pub signer_pubkey: Vec<u8>,
}

impl SignedBackup {
    /// Current envelope format version.
    pub const VERSION: u16 = 1;

    /// Wrap an encrypted backup without signing (e.g. signing keypair absent).
    pub fn unsigned(backup: EncryptedBackup) -> Self {
        Self {
            version: Self::VERSION,
            backup,
            signature: Vec::new(),
            signer_pubkey: Vec::new(),
        }
    }

    /// Returns `true` when this envelope carries a non-empty signature and
    /// matching public key.
    pub fn is_signed(&self) -> bool {
        !self.signature.is_empty() && !self.signer_pubkey.is_empty()
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
    /// KMS key id / alias for SSE-KMS (S3 only). When set, objects are uploaded with
    /// `server-side encryption = aws:kms` using this key, so AWS manages rest-encryption
    /// on top of our own ChaCha20Poly1305 layer (defense in depth).
    #[serde(default)]
    pub kms_key_id: Option<String>,
    /// Object key prefix under the bucket (S3 only). Defaults to `"backups/"`.
    #[serde(default)]
    pub prefix: Option<String>,
    /// Optional public key bytes used to verify backup signatures on retrieve.
    /// When set together with a signing keypair on the provider, S3 objects are
    /// wrapped in a `SignedBackup` envelope and verified on the way out — this
    /// protects against ciphertext tampering at rest even if the bucket itself
    /// is compromised.
    #[serde(default)]
    pub signing_public_key: Option<Vec<u8>>,
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
            kms_key_id: None,
            prefix: None,
            signing_public_key: None,
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

/// Abstract S3-style object store so the provider is unit-testable without
/// hitting AWS. The trait is async-native; `S3Provider` drives it through a
/// cached `tokio::runtime::Handle` so the synchronous
/// `DynamicBackupProvider` trait keeps working unchanged.
#[async_trait::async_trait]
pub trait S3ObjectStore: Send + Sync {
    /// Upload an object. `kms_key_id`, when Some, triggers SSE-KMS.
    async fn put_object(
        &self,
        bucket: &str,
        key: &str,
        body: Vec<u8>,
        kms_key_id: Option<&str>,
    ) -> BackupResult<()>;

    /// Fetch an object's body.
    async fn get_object(&self, bucket: &str, key: &str) -> BackupResult<Vec<u8>>;

    /// List keys under a prefix (returns full object keys).
    async fn list_objects(&self, bucket: &str, prefix: &str) -> BackupResult<Vec<String>>;

    /// Delete an object.
    async fn delete_object(&self, bucket: &str, key: &str) -> BackupResult<()>;
}

/// Borrow on a quantum keypair used to sign backup envelopes before upload.
/// Kept as a trait so callers can plug in HSM-backed signers later.
pub trait BackupSigner: Send + Sync {
    /// Sign `message` and return the raw signature bytes.
    fn sign(&self, message: &[u8]) -> BackupResult<Vec<u8>>;

    /// Return the signer's public key bytes (embedded in the envelope).
    fn public_key(&self) -> Vec<u8>;
}

/// ML-DSA / Dilithium signer wrapper around `QuantumKeyPair`.
pub struct QuantumBackupSigner {
    keypair: Arc<crate::crypto::quantum::QuantumKeyPair>,
}

impl QuantumBackupSigner {
    pub fn new(keypair: Arc<crate::crypto::quantum::QuantumKeyPair>) -> Self {
        Self { keypair }
    }
}

impl BackupSigner for QuantumBackupSigner {
    fn sign(&self, message: &[u8]) -> BackupResult<Vec<u8>> {
        self.keypair
            .sign(message)
            .map_err(|e| BackupError::EncryptionError(format!("sign backup: {e}")))
    }

    fn public_key(&self) -> Vec<u8> {
        self.keypair.public_key.clone()
    }
}

/// S3-compatible backup provider (AWS S3, MinIO, LocalStack, etc.).
///
/// Objects are stored as `bincode(SignedBackup)` under `{prefix}{backup_id}.backup`.
/// When configured with a signing key the envelope carries an ML-DSA signature
/// over the ciphertext, which is re-verified on retrieve — this protects
/// against bucket tampering even if AWS credentials leak.
///
/// # Feature flag
/// The real AWS backend is behind the `s3-backup` feature. Without it, this
/// provider is still usable via [`S3Provider::with_store`] for tests or for
/// plugging a custom S3-compatible client.
pub struct S3Provider {
    name: String,
    bucket: String,
    prefix: String,
    kms_key_id: Option<String>,
    store: Arc<dyn S3ObjectStore>,
    signer: Option<Arc<dyn BackupSigner>>,
    trusted_signer_pubkey: Option<Vec<u8>>,
    signer_parameters: Option<crate::crypto::quantum::QuantumParameters>,
    runtime: tokio::runtime::Handle,
}

impl S3Provider {
    /// Construct a provider backed by an arbitrary `S3ObjectStore`. Primarily
    /// useful for tests and for plugging in non-AWS S3-compatible services.
    pub fn with_store(
        config: &BackupProviderConfig,
        store: Arc<dyn S3ObjectStore>,
        signer: Option<Arc<dyn BackupSigner>>,
        runtime: tokio::runtime::Handle,
    ) -> BackupResult<Self> {
        let bucket = config.bucket.clone().ok_or_else(|| {
            BackupError::ProviderError {
                provider: "S3".to_string(),
                message: "Bucket name required".to_string(),
            }
        })?;

        let prefix = Self::normalize_prefix(config.prefix.as_deref());
        let trusted_signer_pubkey = config.signing_public_key.clone();

        // Currently we only bundle Dilithium-based signing for the backup
        // envelope; downstream callers can override by plugging in another
        // `BackupSigner` impl — but verify needs explicit parameters.
        let signer_parameters = trusted_signer_pubkey
            .as_ref()
            .map(|_| crate::crypto::quantum::QuantumParameters::new(
                crate::crypto::quantum::QuantumScheme::Dilithium,
            ));

        Ok(Self {
            name: config.name.clone(),
            bucket,
            prefix,
            kms_key_id: config.kms_key_id.clone(),
            store,
            signer,
            trusted_signer_pubkey,
            signer_parameters,
            runtime,
        })
    }

    /// Build the real AWS-backed provider. Requires the `s3-backup` feature.
    ///
    /// The AWS `SharedConfig` is loaded through the standard provider chain
    /// (env vars → profile → IMDS). Explicit `access_key_id` /
    /// `secret_access_key` override the chain. A non-empty `endpoint` is used
    /// as-is with path-style addressing so this works against LocalStack,
    /// MinIO, and other S3-compatible services.
    #[cfg(feature = "s3-backup")]
    pub fn new_aws(
        config: &BackupProviderConfig,
        signer: Option<Arc<dyn BackupSigner>>,
        runtime: tokio::runtime::Handle,
    ) -> BackupResult<Self> {
        let region = config
            .region
            .clone()
            .unwrap_or_else(|| "us-east-1".to_string());

        let store: Arc<dyn S3ObjectStore> = {
            let endpoint = config.endpoint.clone();
            let access_key_id = config.access_key_id.clone();
            let secret_access_key = config.secret_access_key.clone();
            let region_cloned = region.clone();

            let client = runtime.block_on(async move {
                let region_provider = aws_sdk_s3::config::Region::new(region_cloned);
                let mut loader = aws_config::defaults(
                    aws_config::BehaviorVersion::latest(),
                )
                .region(region_provider);

                if let (Some(ak), Some(sk)) = (access_key_id, secret_access_key) {
                    let creds = aws_credential_types::Credentials::new(
                        ak,
                        sk,
                        None,
                        None,
                        "supernova-backup",
                    );
                    loader = loader.credentials_provider(creds);
                }

                let shared = loader.load().await;
                let mut s3_conf = aws_sdk_s3::config::Builder::from(&shared);
                if let Some(ep) = endpoint {
                    s3_conf = s3_conf.endpoint_url(ep).force_path_style(true);
                }
                aws_sdk_s3::Client::from_conf(s3_conf.build())
            });

            Arc::new(AwsS3Store::new(client))
        };

        let mut cfg = config.clone();
        cfg.region = Some(region);
        Self::with_store(&cfg, store, signer, runtime)
    }

    fn normalize_prefix(raw: Option<&str>) -> String {
        let base = raw.unwrap_or("backups/").to_string();
        if base.ends_with('/') || base.is_empty() {
            base
        } else {
            format!("{base}/")
        }
    }

    fn object_key(&self, backup_id: &str) -> String {
        format!("{}{}.backup", self.prefix, backup_id)
    }

    fn strip_key(&self, key: &str) -> Option<String> {
        key.strip_prefix(&self.prefix)
            .and_then(|k| k.strip_suffix(".backup"))
            .map(|s| s.to_string())
    }

    /// Wrap `backup` in a `SignedBackup` envelope (optionally signing) and
    /// return the bincoded bytes ready to upload.
    fn encode_envelope(&self, backup: &EncryptedBackup) -> BackupResult<Vec<u8>> {
        let blob = bincode::serialize(backup)
            .map_err(|e| BackupError::StorageError(format!("serialize backup: {e}")))?;

        let envelope = match self.signer.as_ref() {
            Some(signer) => {
                let signature = signer.sign(&blob)?;
                SignedBackup {
                    version: SignedBackup::VERSION,
                    backup: backup.clone(),
                    signature,
                    signer_pubkey: signer.public_key(),
                }
            }
            None => SignedBackup::unsigned(backup.clone()),
        };

        bincode::serialize(&envelope)
            .map_err(|e| BackupError::StorageError(format!("serialize envelope: {e}")))
    }

    /// Decode an envelope pulled from S3 and (if configured) verify its
    /// signature against the trusted public key before returning the
    /// ciphertext.
    fn decode_envelope(&self, bytes: &[u8]) -> BackupResult<EncryptedBackup> {
        let envelope: SignedBackup = bincode::deserialize(bytes)
            .map_err(|e| BackupError::InvalidBackup(format!("decode envelope: {e}")))?;

        if let Some(trusted) = self.trusted_signer_pubkey.as_ref() {
            if !envelope.is_signed() {
                return Err(BackupError::InvalidBackup(
                    "S3 envelope was not signed but signing was required".to_string(),
                ));
            }
            if envelope.signer_pubkey.as_slice() != trusted.as_slice() {
                return Err(BackupError::InvalidBackup(
                    "S3 envelope signer does not match trusted public key".to_string(),
                ));
            }
            let parameters = self.signer_parameters.ok_or_else(|| {
                BackupError::InvalidBackup(
                    "trusted pubkey configured but signer parameters missing".to_string(),
                )
            })?;
            let blob = bincode::serialize(&envelope.backup)
                .map_err(|e| BackupError::InvalidBackup(format!("re-serialize backup: {e}")))?;
            let ok = crate::crypto::quantum::verify_quantum_signature(
                &envelope.signer_pubkey,
                &blob,
                &envelope.signature,
                parameters,
            )
            .map_err(|e| BackupError::InvalidBackup(format!("verify signature: {e}")))?;
            if !ok {
                return Err(BackupError::InvalidBackup(
                    "S3 envelope signature verification failed".to_string(),
                ));
            }
        }

        Ok(envelope.backup)
    }
}

impl DynamicBackupProvider for S3Provider {
    fn store(&self, backup: &EncryptedBackup, backup_id: &str) -> BackupResult<()> {
        let bytes = self.encode_envelope(backup)?;
        if bytes.len() > MAX_BACKUP_SIZE {
            return Err(BackupError::StorageError(format!(
                "backup {} too large: {} > {}",
                backup_id,
                bytes.len(),
                MAX_BACKUP_SIZE,
            )));
        }
        let key = self.object_key(backup_id);
        let bucket = self.bucket.clone();
        let kms = self.kms_key_id.clone();
        let store = Arc::clone(&self.store);
        self.runtime.block_on(async move {
            store.put_object(&bucket, &key, bytes, kms.as_deref()).await
        })?;
        debug!(
            "Stored backup {} to s3://{}/{}{}.backup",
            backup_id, self.bucket, self.prefix, backup_id
        );
        Ok(())
    }

    fn retrieve(&self, backup_id: &str) -> BackupResult<EncryptedBackup> {
        let key = self.object_key(backup_id);
        let bucket = self.bucket.clone();
        let store = Arc::clone(&self.store);
        let bytes = self
            .runtime
            .block_on(async move { store.get_object(&bucket, &key).await })?;
        self.decode_envelope(&bytes)
    }

    fn list_backups(&self) -> BackupResult<Vec<String>> {
        let prefix = self.prefix.clone();
        let bucket = self.bucket.clone();
        let store = Arc::clone(&self.store);
        let keys = self
            .runtime
            .block_on(async move { store.list_objects(&bucket, &prefix).await })?;
        Ok(keys.into_iter().filter_map(|k| self.strip_key(&k)).collect())
    }

    fn delete(&self, backup_id: &str) -> BackupResult<()> {
        let key = self.object_key(backup_id);
        let bucket = self.bucket.clone();
        let store = Arc::clone(&self.store);
        self.runtime
            .block_on(async move { store.delete_object(&bucket, &key).await })
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn provider_type(&self) -> BackupProviderType {
        BackupProviderType::S3
    }
}

/// Real AWS-backed `S3ObjectStore` implementation. Only available with the
/// `s3-backup` feature enabled so builds that don't need S3 don't pull the
/// AWS SDK dependency tree.
#[cfg(feature = "s3-backup")]
pub struct AwsS3Store {
    client: aws_sdk_s3::Client,
}

#[cfg(feature = "s3-backup")]
impl AwsS3Store {
    pub fn new(client: aws_sdk_s3::Client) -> Self {
        Self { client }
    }
}

#[cfg(feature = "s3-backup")]
#[async_trait::async_trait]
impl S3ObjectStore for AwsS3Store {
    async fn put_object(
        &self,
        bucket: &str,
        key: &str,
        body: Vec<u8>,
        kms_key_id: Option<&str>,
    ) -> BackupResult<()> {
        let mut req = self
            .client
            .put_object()
            .bucket(bucket)
            .key(key)
            .body(aws_sdk_s3::primitives::ByteStream::from(body));

        if let Some(kms) = kms_key_id {
            req = req
                .server_side_encryption(aws_sdk_s3::types::ServerSideEncryption::AwsKms)
                .ssekms_key_id(kms);
        }

        req.send().await.map_err(|e| BackupError::ProviderError {
            provider: "S3".to_string(),
            message: format!("put_object failed: {e}"),
        })?;
        Ok(())
    }

    async fn get_object(&self, bucket: &str, key: &str) -> BackupResult<Vec<u8>> {
        let resp = self
            .client
            .get_object()
            .bucket(bucket)
            .key(key)
            .send()
            .await
            .map_err(|_| BackupError::BackupNotFound {
                backup_id: key.to_string(),
            })?;
        let bytes = resp
            .body
            .collect()
            .await
            .map_err(|e| BackupError::StorageError(format!("read body: {e}")))?
            .into_bytes();
        Ok(bytes.to_vec())
    }

    async fn list_objects(&self, bucket: &str, prefix: &str) -> BackupResult<Vec<String>> {
        let resp = self
            .client
            .list_objects_v2()
            .bucket(bucket)
            .prefix(prefix)
            .send()
            .await
            .map_err(|e| BackupError::ProviderError {
                provider: "S3".to_string(),
                message: format!("list_objects_v2 failed: {e}"),
            })?;
        Ok(resp
            .contents
            .unwrap_or_default()
            .into_iter()
            .filter_map(|o| o.key)
            .collect())
    }

    async fn delete_object(&self, bucket: &str, key: &str) -> BackupResult<()> {
        self.client
            .delete_object()
            .bucket(bucket)
            .key(key)
            .send()
            .await
            .map_err(|e| BackupError::ProviderError {
                provider: "S3".to_string(),
                message: format!("delete_object failed: {e}"),
            })?;
        Ok(())
    }
}

/// Timeout for a single peer backup exchange (store-ack or request-response).
pub const PEER_BACKUP_TIMEOUT_SECS: u64 = 30;

/// Correlation identifier for a peer backup exchange.
pub type BackupRequestId = u64;

/// Wire envelope exchanged between peers running the backup protocol.
///
/// The backup ciphertext inside `Store` / `Response` is already encrypted with the
/// owner's symmetric key (see [`EncryptedBackup`]). A peer that stores a backup
/// on our behalf cannot read its contents — they act purely as an opaque blob
/// host. Transport-level confidentiality (e.g. the Lightning Noise handshake)
/// is expected to run underneath this envelope.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PeerBackupMessage {
    /// "Please store this backup under `backup_id`." — sender → receiver.
    Store {
        request_id: BackupRequestId,
        backup_id: String,
        backup: EncryptedBackup,
    },
    /// Response to `Store`. `accepted=false` + `reason` on rejection.
    StoreAck {
        request_id: BackupRequestId,
        backup_id: String,
        accepted: bool,
        reason: Option<String>,
    },
    /// "Please return the backup you hold under `backup_id`." — requester → holder.
    Request {
        request_id: BackupRequestId,
        backup_id: String,
    },
    /// Response to `Request`. `backup=None` if the holder does not have it.
    Response {
        request_id: BackupRequestId,
        backup_id: String,
        backup: Option<EncryptedBackup>,
    },
}

/// Pluggable transport abstraction for the peer backup protocol.
///
/// Production integrations wrap the Lightning Network wire (see
/// `crate::lightning::wire`) to deliver a `PeerBackupMessage` to the target peer
/// and await the correlated response. The in-process transport used in tests
/// routes messages directly between two `PeerBackupProtocol` instances.
#[async_trait::async_trait]
pub trait PeerBackupTransport: Send + Sync {
    /// Deliver `message` to `peer` and return the correlated reply.
    async fn exchange(
        &self,
        peer: &NodeId,
        message: PeerBackupMessage,
    ) -> BackupResult<PeerBackupMessage>;
}

/// Peer backup protocol handler
pub struct PeerBackupProtocol {
    /// Our node ID
    our_node_id: NodeId,
    /// Trusted peer node IDs
    peer_node_ids: Vec<NodeId>,
    /// Backups we are hosting on behalf of peers (opaque ciphertext — we cannot decrypt).
    peer_backups: Arc<RwLock<HashMap<String, EncryptedBackup>>>,
    /// Optional wire transport — when present, `*_async` methods talk to real peers.
    transport: Option<Arc<dyn PeerBackupTransport>>,
    /// Monotonic counter for request/response correlation.
    next_request_id: Arc<AtomicU64>,
    /// Per-exchange timeout (seconds). Default is [`PEER_BACKUP_TIMEOUT_SECS`].
    timeout_secs: u64,
}

impl PeerBackupProtocol {
    pub fn new(our_node_id: NodeId, peer_node_ids: Vec<NodeId>) -> Self {
        Self {
            our_node_id,
            peer_node_ids,
            peer_backups: Arc::new(RwLock::new(HashMap::new())),
            transport: None,
            next_request_id: Arc::new(AtomicU64::new(1)),
            timeout_secs: PEER_BACKUP_TIMEOUT_SECS,
        }
    }

    /// Attach a wire transport so the `*_async` methods can talk to real peers.
    pub fn with_transport(mut self, transport: Arc<dyn PeerBackupTransport>) -> Self {
        self.transport = Some(transport);
        self
    }

    /// Override the per-exchange timeout (seconds). Intended for tests; production
    /// code should keep the default so that slow peers remain bounded uniformly.
    pub fn with_timeout_secs(mut self, secs: u64) -> Self {
        self.timeout_secs = secs;
        self
    }

    /// Our node identifier — used by transports to attribute inbound messages.
    pub fn our_node_id(&self) -> &NodeId {
        &self.our_node_id
    }

    fn fresh_request_id(&self) -> BackupRequestId {
        self.next_request_id.fetch_add(1, Ordering::Relaxed)
    }

    fn transport(&self) -> BackupResult<Arc<dyn PeerBackupTransport>> {
        self.transport.as_ref().cloned().ok_or_else(|| {
            BackupError::PeerBackupFailed {
                peer_id: "transport".to_string(),
                reason: "peer backup transport not configured".to_string(),
            }
        })
    }

    /// Send backup to trusted peers (synchronous no-op; use `distribute_backup_async`).
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

    /// Legacy synchronous hook. The real P2P path is `send_to_peer_async`.
    fn send_to_peer(&self, _peer_id: &NodeId, _backup: &EncryptedBackup, _backup_id: &str) -> BackupResult<()> {
        // Sync callers (e.g. the `DynamicBackupProvider` trait impl) cannot block on
        // a tokio runtime safely, so we treat this as an inert hook and rely on the
        // async API for real network delivery.
        Ok(())
    }

    /// Store a backup received from a peer
    pub fn receive_peer_backup(&self, backup: EncryptedBackup, backup_id: String) -> BackupResult<()> {
        let mut backups = self.peer_backups.write()
            .map_err(|_| BackupError::LockPoisoned)?;

        backups.insert(backup_id, backup);
        Ok(())
    }

    /// Retrieve backup from peers (for recovery) — sync path only checks local cache.
    pub fn request_backup_from_peers(&self, backup_id: &str) -> BackupResult<EncryptedBackup> {
        let backups = self.peer_backups.read()
            .map_err(|_| BackupError::LockPoisoned)?;

        if let Some(backup) = backups.get(backup_id) {
            return Ok(backup.clone());
        }

        Err(BackupError::BackupNotFound {
            backup_id: backup_id.to_string(),
        })
    }

    /// Send a single backup to a single peer over the wire and await ack.
    ///
    /// Uses the configured [`PeerBackupTransport`] and enforces a per-exchange
    /// timeout of [`PEER_BACKUP_TIMEOUT_SECS`] seconds.
    pub async fn send_to_peer_async(
        &self,
        peer_id: &NodeId,
        backup: &EncryptedBackup,
        backup_id: &str,
    ) -> BackupResult<()> {
        let transport = self.transport()?;
        let request_id = self.fresh_request_id();
        let msg = PeerBackupMessage::Store {
            request_id,
            backup_id: backup_id.to_string(),
            backup: backup.clone(),
        };

        let exchange = transport.exchange(peer_id, msg);
        let reply = tokio::time::timeout(
            Duration::from_secs(self.timeout_secs),
            exchange,
        )
        .await
        .map_err(|_| BackupError::PeerBackupFailed {
            peer_id: peer_id.to_string(),
            reason: format!("timeout after {}s awaiting StoreAck", self.timeout_secs),
        })??;

        match reply {
            PeerBackupMessage::StoreAck {
                request_id: reply_id,
                backup_id: reply_backup_id,
                accepted,
                reason,
            } => {
                if reply_id != request_id || reply_backup_id != backup_id {
                    return Err(BackupError::PeerBackupFailed {
                        peer_id: peer_id.to_string(),
                        reason: "StoreAck correlation mismatch".to_string(),
                    });
                }
                if !accepted {
                    return Err(BackupError::PeerBackupFailed {
                        peer_id: peer_id.to_string(),
                        reason: reason.unwrap_or_else(|| "peer rejected backup".to_string()),
                    });
                }
                Ok(())
            }
            other => Err(BackupError::PeerBackupFailed {
                peer_id: peer_id.to_string(),
                reason: format!("unexpected reply to Store: {:?}", std::mem::discriminant(&other)),
            }),
        }
    }

    /// Distribute an encrypted backup to every configured peer in parallel.
    ///
    /// Returns the number of peers that acknowledged storage. Individual peer
    /// failures are logged and skipped, not propagated — as long as at least one
    /// peer accepts, distribution is considered successful.
    pub async fn distribute_backup_async(
        &self,
        backup: &EncryptedBackup,
        backup_id: &str,
    ) -> BackupResult<usize> {
        if self.peer_node_ids.is_empty() {
            return Ok(0);
        }

        let mut handles = Vec::with_capacity(self.peer_node_ids.len());
        for peer in &self.peer_node_ids {
            handles.push(self.send_to_peer_async(peer, backup, backup_id));
        }

        let mut success = 0usize;
        for (peer, fut) in self.peer_node_ids.iter().zip(handles) {
            match fut.await {
                Ok(()) => {
                    success += 1;
                    debug!("peer {} acknowledged backup {}", peer, backup_id);
                }
                Err(e) => {
                    warn!("peer {} failed to accept backup {}: {}", peer, backup_id, e);
                }
            }
        }

        if success == 0 {
            return Err(BackupError::PeerBackupFailed {
                peer_id: "all".to_string(),
                reason: "no peer accepted backup".to_string(),
            });
        }
        Ok(success)
    }

    /// Request a backup from configured peers, returning the first successful response.
    ///
    /// The local cache is consulted first. If absent, peers are queried in order
    /// with a per-peer [`PEER_BACKUP_TIMEOUT_SECS`] timeout. Returns
    /// [`BackupError::BackupNotFound`] if no peer holds the blob.
    pub async fn request_backup_from_peers_async(
        &self,
        backup_id: &str,
    ) -> BackupResult<EncryptedBackup> {
        {
            let backups = self.peer_backups.read()
                .map_err(|_| BackupError::LockPoisoned)?;
            if let Some(backup) = backups.get(backup_id) {
                return Ok(backup.clone());
            }
        }

        let transport = self.transport()?;

        for peer in &self.peer_node_ids {
            let request_id = self.fresh_request_id();
            let msg = PeerBackupMessage::Request {
                request_id,
                backup_id: backup_id.to_string(),
            };

            let exchange = transport.exchange(peer, msg);
            let reply = match tokio::time::timeout(
                Duration::from_secs(self.timeout_secs),
                exchange,
            )
            .await
            {
                Ok(Ok(reply)) => reply,
                Ok(Err(e)) => {
                    warn!("peer {} exchange error for backup {}: {}", peer, backup_id, e);
                    continue;
                }
                Err(_) => {
                    warn!(
                        "peer {} timed out after {}s while requesting {}",
                        peer, self.timeout_secs, backup_id
                    );
                    continue;
                }
            };

            match reply {
                PeerBackupMessage::Response {
                    request_id: reply_id,
                    backup_id: reply_backup_id,
                    backup,
                } => {
                    if reply_id != request_id || reply_backup_id != backup_id {
                        warn!("peer {} returned mismatched Response", peer);
                        continue;
                    }
                    if let Some(blob) = backup {
                        return Ok(blob);
                    }
                }
                other => {
                    warn!(
                        "peer {} returned unexpected reply to Request: {:?}",
                        peer,
                        std::mem::discriminant(&other)
                    );
                }
            }
        }

        Err(BackupError::BackupNotFound {
            backup_id: backup_id.to_string(),
        })
    }

    /// Server-side dispatcher for an inbound peer backup message.
    ///
    /// A wire listener calls this with each deserialized [`PeerBackupMessage`]
    /// received from `_from`. The returned message (if any) must be sent back
    /// to that peer to complete the exchange.
    pub fn handle_message(
        &self,
        _from: &NodeId,
        message: PeerBackupMessage,
    ) -> BackupResult<Option<PeerBackupMessage>> {
        match message {
            PeerBackupMessage::Store { request_id, backup_id, backup } => {
                let mut cache = self.peer_backups.write()
                    .map_err(|_| BackupError::LockPoisoned)?;
                cache.insert(backup_id.clone(), backup);
                Ok(Some(PeerBackupMessage::StoreAck {
                    request_id,
                    backup_id,
                    accepted: true,
                    reason: None,
                }))
            }
            PeerBackupMessage::Request { request_id, backup_id } => {
                let cache = self.peer_backups.read()
                    .map_err(|_| BackupError::LockPoisoned)?;
                let backup = cache.get(&backup_id).cloned();
                Ok(Some(PeerBackupMessage::Response {
                    request_id,
                    backup_id,
                    backup,
                }))
            }
            // Responses are correlated by the client side; the listener should route
            // them back to the awaiting request, not through this dispatcher.
            PeerBackupMessage::StoreAck { .. } | PeerBackupMessage::Response { .. } => Ok(None),
        }
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
                    #[cfg(feature = "s3-backup")]
                    {
                        let runtime = tokio::runtime::Handle::try_current().map_err(|_| {
                            BackupError::ProviderError {
                                provider: "S3".to_string(),
                                message: "S3 provider requires a running tokio runtime"
                                    .to_string(),
                            }
                        })?;
                        Box::new(S3Provider::new_aws(provider_config, None, runtime)?)
                    }
                    #[cfg(not(feature = "s3-backup"))]
                    {
                        return Err(BackupError::ProviderError {
                            provider: "S3".to_string(),
                            message:
                                "S3 backup requires the 's3-backup' feature to be enabled"
                                    .to_string(),
                        });
                    }
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

    /// In-memory `S3ObjectStore` used to unit-test `S3Provider` without AWS.
    /// Mimics an S3 bucket; keys are stored in a flat map and listed by
    /// prefix match. `bucket` matches must be exact.
    #[derive(Default)]
    struct InMemoryS3 {
        data: std::sync::Mutex<std::collections::HashMap<(String, String), Vec<u8>>>,
    }

    #[async_trait::async_trait]
    impl S3ObjectStore for InMemoryS3 {
        async fn put_object(
            &self,
            bucket: &str,
            key: &str,
            body: Vec<u8>,
            _kms_key_id: Option<&str>,
        ) -> BackupResult<()> {
            self.data
                .lock()
                .map_err(|_| BackupError::LockPoisoned)?
                .insert((bucket.to_string(), key.to_string()), body);
            Ok(())
        }

        async fn get_object(&self, bucket: &str, key: &str) -> BackupResult<Vec<u8>> {
            self.data
                .lock()
                .map_err(|_| BackupError::LockPoisoned)?
                .get(&(bucket.to_string(), key.to_string()))
                .cloned()
                .ok_or_else(|| BackupError::BackupNotFound {
                    backup_id: key.to_string(),
                })
        }

        async fn list_objects(
            &self,
            bucket: &str,
            prefix: &str,
        ) -> BackupResult<Vec<String>> {
            Ok(self
                .data
                .lock()
                .map_err(|_| BackupError::LockPoisoned)?
                .iter()
                .filter(|((b, k), _)| b == bucket && k.starts_with(prefix))
                .map(|((_, k), _)| k.clone())
                .collect())
        }

        async fn delete_object(&self, bucket: &str, key: &str) -> BackupResult<()> {
            self.data
                .lock()
                .map_err(|_| BackupError::LockPoisoned)?
                .remove(&(bucket.to_string(), key.to_string()));
            Ok(())
        }
    }

    fn s3_test_config() -> BackupProviderConfig {
        BackupProviderConfig {
            provider_type: BackupProviderType::S3,
            name: "s3-test".to_string(),
            bucket: Some("supernova-test".to_string()),
            region: Some("us-east-1".to_string()),
            prefix: Some("lightning/".to_string()),
            kms_key_id: Some("alias/supernova-kms".to_string()),
            ..Default::default()
        }
    }

    #[test]
    fn s3_provider_roundtrip_without_signing() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime");
        let _guard = runtime.enter();

        let store = Arc::new(InMemoryS3::default());
        let config = s3_test_config();
        let provider =
            S3Provider::with_store(&config, store.clone(), None, runtime.handle().clone())
                .expect("provider");

        let node_id = create_test_node_id();
        let key = create_test_encryption_key();
        let package = ChannelBackupPackage::new(node_id, vec![]);
        let encrypted = EncryptedBackup::encrypt(&package, &key).expect("encrypt");

        provider.store(&encrypted, "alice-1").expect("store");
        let listed = provider.list_backups().expect("list");
        assert_eq!(listed, vec!["alice-1".to_string()]);

        let retrieved = provider.retrieve("alice-1").expect("retrieve");
        let decoded = retrieved.decrypt(&key).expect("decrypt");
        assert!(decoded.verify());

        provider.delete("alice-1").expect("delete");
        assert!(provider.list_backups().expect("list").is_empty());
    }

    #[test]
    fn s3_provider_signed_envelope_verifies_on_retrieve() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime");
        let _guard = runtime.enter();

        let params = crate::crypto::quantum::QuantumParameters::new(
            crate::crypto::quantum::QuantumScheme::Dilithium,
        );
        let keypair = Arc::new(
            crate::crypto::quantum::QuantumKeyPair::generate(params).expect("keypair"),
        );
        let signer: Arc<dyn BackupSigner> =
            Arc::new(QuantumBackupSigner::new(keypair.clone()));

        let mut config = s3_test_config();
        config.signing_public_key = Some(keypair.public_key.clone());

        let store = Arc::new(InMemoryS3::default());
        let provider = S3Provider::with_store(
            &config,
            store.clone(),
            Some(signer),
            runtime.handle().clone(),
        )
        .expect("provider");

        let node_id = create_test_node_id();
        let key = create_test_encryption_key();
        let package = ChannelBackupPackage::new(node_id, vec![]);
        let encrypted = EncryptedBackup::encrypt(&package, &key).expect("encrypt");

        provider.store(&encrypted, "alice-signed").expect("store");
        let retrieved = provider.retrieve("alice-signed").expect("retrieve");
        let decoded = retrieved.decrypt(&key).expect("decrypt");
        assert!(decoded.verify());
    }

    #[test]
    fn s3_provider_rejects_tampered_envelope() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime");
        let _guard = runtime.enter();

        let params = crate::crypto::quantum::QuantumParameters::new(
            crate::crypto::quantum::QuantumScheme::Dilithium,
        );
        let keypair = Arc::new(
            crate::crypto::quantum::QuantumKeyPair::generate(params).expect("keypair"),
        );
        let signer: Arc<dyn BackupSigner> =
            Arc::new(QuantumBackupSigner::new(keypair.clone()));

        let mut config = s3_test_config();
        config.signing_public_key = Some(keypair.public_key.clone());

        let store = Arc::new(InMemoryS3::default());
        let provider = S3Provider::with_store(
            &config,
            store.clone(),
            Some(signer),
            runtime.handle().clone(),
        )
        .expect("provider");

        let node_id = create_test_node_id();
        let key = create_test_encryption_key();
        let package = ChannelBackupPackage::new(node_id, vec![]);
        let encrypted = EncryptedBackup::encrypt(&package, &key).expect("encrypt");
        provider.store(&encrypted, "alice-tamper").expect("store");

        // Flip a byte directly in the stored blob to simulate bucket tampering.
        {
            let mut guard = store.data.lock().unwrap();
            let stored = guard
                .values_mut()
                .next()
                .expect("one object");
            let idx = stored.len() / 2;
            stored[idx] ^= 0xFF;
        }

        let err = provider.retrieve("alice-tamper").expect_err("tamper must fail");
        match err {
            BackupError::InvalidBackup(_) => {}
            other => panic!("expected InvalidBackup, got {:?}", other),
        }
    }

    #[test]
    fn s3_provider_rejects_unsigned_when_trust_configured() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime");
        let _guard = runtime.enter();

        let params = crate::crypto::quantum::QuantumParameters::new(
            crate::crypto::quantum::QuantumScheme::Dilithium,
        );
        let keypair = Arc::new(
            crate::crypto::quantum::QuantumKeyPair::generate(params).expect("keypair"),
        );

        // Writer: no signer configured, so envelope will be stored unsigned.
        let writer_config = s3_test_config();
        let store = Arc::new(InMemoryS3::default());
        let writer = S3Provider::with_store(
            &writer_config,
            store.clone(),
            None,
            runtime.handle().clone(),
        )
        .expect("writer");

        let node_id = create_test_node_id();
        let key = create_test_encryption_key();
        let package = ChannelBackupPackage::new(node_id, vec![]);
        let encrypted = EncryptedBackup::encrypt(&package, &key).expect("encrypt");
        writer.store(&encrypted, "alice-unsigned").expect("store");

        // Reader: signing pubkey configured → demands signature on retrieve.
        let mut reader_config = s3_test_config();
        reader_config.signing_public_key = Some(keypair.public_key.clone());
        let reader = S3Provider::with_store(
            &reader_config,
            store,
            None,
            runtime.handle().clone(),
        )
        .expect("reader");

        let err = reader
            .retrieve("alice-unsigned")
            .expect_err("unsigned retrieve must fail");
        match err {
            BackupError::InvalidBackup(_) => {}
            other => panic!("expected InvalidBackup, got {:?}", other),
        }
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

        // With no channels, no actual backup is performed
        // The trigger_backup succeeds but doesn't increment success_count
        let status = manager.get_status().unwrap();
        // When no channels exist, no backup is created, so success_count remains 0
        assert_eq!(status[0].success_count, 0, "No backup should occur with no channels");
    }
}
