//! Key Rotation Mechanism for Supernova
//!
//! This module implements automated key rotation for quantum-resistant keys.
//! Key rotation is essential for:
//! - Limiting the exposure window if a key is compromised
//! - Preparing for quantum computing advances
//! - Maintaining forward secrecy
//!
//! # Architecture
//! - `KeyRotationManager` - Central coordinator for key rotation
//! - `RotationPolicy` - Configurable rotation triggers
//! - `KeyMigrationTransaction` - On-chain key migration
//! - Grace periods for backward compatibility

use super::quantum::{QuantumKeyPair, QuantumParameters, QuantumScheme};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use thiserror::Error;
use tracing::{debug, error, info, warn};

/// Default key rotation interval in blocks (~1 week at 10-min blocks)
pub const DEFAULT_ROTATION_INTERVAL_BLOCKS: u64 = 1008;

/// Default key rotation interval in seconds (1 week)
pub const DEFAULT_ROTATION_INTERVAL_SECONDS: u64 = 7 * 24 * 60 * 60;

/// Grace period for accepting old keys after rotation (in blocks)
pub const DEFAULT_GRACE_PERIOD_BLOCKS: u64 = 144; // ~1 day

/// Grace period in seconds
pub const DEFAULT_GRACE_PERIOD_SECONDS: u64 = 24 * 60 * 60;

/// Maximum number of previous keys to retain
pub const MAX_RETAINED_KEYS: usize = 5;

/// Warning threshold before rotation (in blocks)
pub const ROTATION_WARNING_THRESHOLD: u64 = 72; // ~12 hours warning

/// Key rotation errors
#[derive(Debug, Error, Clone)]
pub enum KeyRotationError {
    #[error("Key not found: {0}")]
    KeyNotFound(String),

    #[error("Rotation in progress for key: {0}")]
    RotationInProgress(String),

    #[error("Key generation failed: {0}")]
    KeyGenerationFailed(String),

    #[error("Invalid rotation policy: {0}")]
    InvalidPolicy(String),

    #[error("Grace period expired for key: {0}")]
    GracePeriodExpired(String),

    #[error("Migration transaction failed: {0}")]
    MigrationFailed(String),

    #[error("Key already rotated at height {0}")]
    AlreadyRotated(u64),

    #[error("Lock poisoned")]
    LockPoisoned,

    #[error("Invalid key state: {0}")]
    InvalidKeyState(String),
}

/// Result type for key rotation operations
pub type KeyRotationResult<T> = Result<T, KeyRotationError>;

/// Key rotation trigger types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RotationTrigger {
    /// Time-based rotation
    Time,
    /// Block height-based rotation
    BlockHeight,
    /// Manual rotation request
    Manual,
    /// Security incident triggered rotation
    SecurityIncident,
    /// Quantum threat level change
    QuantumThreatUpgrade,
    /// Key compromise detected
    KeyCompromise,
}

/// Rotation policy configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RotationPolicy {
    /// Rotation interval in blocks
    pub interval_blocks: u64,
    /// Rotation interval in seconds (alternative)
    pub interval_seconds: u64,
    /// Grace period in blocks (old keys still valid)
    pub grace_period_blocks: u64,
    /// Grace period in seconds
    pub grace_period_seconds: u64,
    /// Whether to enable automatic rotation
    pub auto_rotate: bool,
    /// Maximum keys to retain for backward compatibility
    pub max_retained_keys: usize,
    /// Warning threshold before rotation
    pub warning_threshold_blocks: u64,
    /// Require on-chain migration transaction
    pub require_migration_tx: bool,
    /// Enable emergency rotation on threat detection
    pub emergency_rotation_enabled: bool,
}

impl Default for RotationPolicy {
    fn default() -> Self {
        Self {
            interval_blocks: DEFAULT_ROTATION_INTERVAL_BLOCKS,
            interval_seconds: DEFAULT_ROTATION_INTERVAL_SECONDS,
            grace_period_blocks: DEFAULT_GRACE_PERIOD_BLOCKS,
            grace_period_seconds: DEFAULT_GRACE_PERIOD_SECONDS,
            auto_rotate: true,
            max_retained_keys: MAX_RETAINED_KEYS,
            warning_threshold_blocks: ROTATION_WARNING_THRESHOLD,
            require_migration_tx: true,
            emergency_rotation_enabled: true,
        }
    }
}

/// State of a managed key
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum KeyState {
    /// Key is active and primary
    Active,
    /// Key is being rotated (new key being prepared)
    Rotating,
    /// Key is in grace period (still valid but deprecated)
    GracePeriod,
    /// Key is expired and no longer valid
    Expired,
    /// Key was revoked due to security incident
    Revoked,
}

/// Metadata for a managed key
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManagedKeyMetadata {
    /// Unique key identifier
    pub key_id: [u8; 32],
    /// Key state
    pub state: KeyState,
    /// Block height when key was created
    pub created_height: u64,
    /// Timestamp when key was created
    pub created_timestamp: u64,
    /// Block height when key was last rotated
    pub last_rotation_height: Option<u64>,
    /// Block height when grace period ends
    pub grace_period_end_height: Option<u64>,
    /// Quantum scheme used
    pub scheme: QuantumScheme,
    /// Rotation count (how many times this key type has rotated)
    pub rotation_count: u32,
    /// Owner identifier (address, channel ID, etc.)
    pub owner_id: String,
}

/// A managed key with its keypair and metadata
#[derive(Clone)]
pub struct ManagedKey {
    /// The quantum key pair
    pub keypair: QuantumKeyPair,
    /// Key metadata
    pub metadata: ManagedKeyMetadata,
    /// Previous keys (for backward compatibility)
    pub previous_keys: Vec<(QuantumKeyPair, ManagedKeyMetadata)>,
}

/// Key migration transaction for on-chain rotation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyMigrationTransaction {
    /// Old public key hash
    pub old_pubkey_hash: [u8; 32],
    /// New public key
    pub new_pubkey: Vec<u8>,
    /// New public key hash
    pub new_pubkey_hash: [u8; 32],
    /// Signature with old key proving ownership
    pub old_key_signature: Vec<u8>,
    /// Migration timestamp
    pub timestamp: u64,
    /// Block height target for migration
    pub target_height: u64,
    /// Grace period end height
    pub grace_end_height: u64,
    /// Additional metadata
    pub metadata: Option<Vec<u8>>,
}

impl KeyMigrationTransaction {
    /// Create migration message for signing
    pub fn migration_message(&self) -> Vec<u8> {
        let mut message = Vec::new();
        message.extend_from_slice(&self.old_pubkey_hash);
        message.extend_from_slice(&self.new_pubkey_hash);
        message.extend_from_slice(&self.target_height.to_le_bytes());
        message.extend_from_slice(&self.timestamp.to_le_bytes());
        message
    }
}

/// Rotation event for logging and notification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RotationEvent {
    /// Key ID that was rotated
    pub key_id: [u8; 32],
    /// Trigger that caused rotation
    pub trigger: RotationTrigger,
    /// Height when rotation occurred
    pub rotation_height: u64,
    /// Timestamp of rotation
    pub timestamp: u64,
    /// Previous key hash
    pub previous_key_hash: [u8; 32],
    /// New key hash
    pub new_key_hash: [u8; 32],
    /// Migration transaction hash (if applicable)
    pub migration_tx_hash: Option<[u8; 32]>,
}

/// Key Rotation Manager
pub struct KeyRotationManager {
    /// Rotation policy
    policy: RotationPolicy,
    /// Managed keys by ID
    keys: Arc<RwLock<HashMap<[u8; 32], ManagedKey>>>,
    /// Current blockchain height (updated externally)
    current_height: Arc<RwLock<u64>>,
    /// Rotation event history
    events: Arc<RwLock<Vec<RotationEvent>>>,
    /// Pending migration transactions
    pending_migrations: Arc<RwLock<Vec<KeyMigrationTransaction>>>,
    /// Quantum parameters for key generation
    quantum_params: QuantumParameters,
}

impl KeyRotationManager {
    /// Create a new key rotation manager
    pub fn new(policy: RotationPolicy, scheme: QuantumScheme) -> Self {
        let quantum_params = QuantumParameters {
            scheme,
            security_level: 3, // NIST Level 3
        };

        Self {
            policy,
            keys: Arc::new(RwLock::new(HashMap::new())),
            current_height: Arc::new(RwLock::new(0)),
            events: Arc::new(RwLock::new(Vec::new())),
            pending_migrations: Arc::new(RwLock::new(Vec::new())),
            quantum_params,
        }
    }

    /// Create with default policy
    pub fn with_defaults() -> Self {
        Self::new(RotationPolicy::default(), QuantumScheme::Dilithium)
    }

    /// Update current blockchain height
    pub fn update_height(&self, height: u64) -> KeyRotationResult<()> {
        let mut current = self
            .current_height
            .write()
            .map_err(|_| KeyRotationError::LockPoisoned)?;
        *current = height;
        Ok(())
    }

    /// Get current height
    pub fn get_current_height(&self) -> KeyRotationResult<u64> {
        let current = self
            .current_height
            .read()
            .map_err(|_| KeyRotationError::LockPoisoned)?;
        Ok(*current)
    }

    /// Register a new key for rotation management
    pub fn register_key(
        &self,
        owner_id: String,
        keypair: QuantumKeyPair,
    ) -> KeyRotationResult<[u8; 32]> {
        let current_height = self.get_current_height()?;
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or(Duration::ZERO)
            .as_secs();

        // Generate key ID from public key hash
        let key_id = Self::compute_key_id(&keypair.public_key);

        let metadata = ManagedKeyMetadata {
            key_id,
            state: KeyState::Active,
            created_height: current_height,
            created_timestamp: now,
            last_rotation_height: None,
            grace_period_end_height: None,
            scheme: self.quantum_params.scheme,
            rotation_count: 0,
            owner_id,
        };

        let managed_key = ManagedKey {
            keypair,
            metadata,
            previous_keys: Vec::new(),
        };

        let mut keys = self
            .keys
            .write()
            .map_err(|_| KeyRotationError::LockPoisoned)?;

        keys.insert(key_id, managed_key);

        info!(
            "Registered key {} for rotation management",
            hex::encode(&key_id[..8])
        );

        Ok(key_id)
    }

    /// Generate a new key and register it
    pub fn generate_and_register(&self, owner_id: String) -> KeyRotationResult<[u8; 32]> {
        let keypair = QuantumKeyPair::generate(self.quantum_params)
            .map_err(|e| KeyRotationError::KeyGenerationFailed(e.to_string()))?;

        self.register_key(owner_id, keypair)
    }

    /// Check if a key needs rotation
    pub fn needs_rotation(&self, key_id: &[u8; 32]) -> KeyRotationResult<bool> {
        if !self.policy.auto_rotate {
            return Ok(false);
        }

        let current_height = self.get_current_height()?;
        let keys = self
            .keys
            .read()
            .map_err(|_| KeyRotationError::LockPoisoned)?;

        let key = keys
            .get(key_id)
            .ok_or_else(|| KeyRotationError::KeyNotFound(hex::encode(key_id)))?;

        if key.metadata.state != KeyState::Active {
            return Ok(false);
        }

        let last_rotation = key
            .metadata
            .last_rotation_height
            .unwrap_or(key.metadata.created_height);

        let blocks_since_rotation = current_height.saturating_sub(last_rotation);
        Ok(blocks_since_rotation >= self.policy.interval_blocks)
    }

    /// Check if rotation warning should be issued
    pub fn rotation_warning_due(&self, key_id: &[u8; 32]) -> KeyRotationResult<bool> {
        let current_height = self.get_current_height()?;
        let keys = self
            .keys
            .read()
            .map_err(|_| KeyRotationError::LockPoisoned)?;

        let key = keys
            .get(key_id)
            .ok_or_else(|| KeyRotationError::KeyNotFound(hex::encode(key_id)))?;

        if key.metadata.state != KeyState::Active {
            return Ok(false);
        }

        let last_rotation = key
            .metadata
            .last_rotation_height
            .unwrap_or(key.metadata.created_height);

        let blocks_since_rotation = current_height.saturating_sub(last_rotation);
        let blocks_until_rotation = self
            .policy
            .interval_blocks
            .saturating_sub(blocks_since_rotation);

        Ok(blocks_until_rotation <= self.policy.warning_threshold_blocks)
    }

    /// Initiate key rotation
    pub fn rotate_key(
        &self,
        key_id: &[u8; 32],
        trigger: RotationTrigger,
    ) -> KeyRotationResult<KeyMigrationTransaction> {
        let current_height = self.get_current_height()?;
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or(Duration::ZERO)
            .as_secs();

        // Generate new key pair
        let new_keypair = QuantumKeyPair::generate(self.quantum_params)
            .map_err(|e| KeyRotationError::KeyGenerationFailed(e.to_string()))?;

        let new_key_id = Self::compute_key_id(&new_keypair.public_key);

        let mut keys = self
            .keys
            .write()
            .map_err(|_| KeyRotationError::LockPoisoned)?;

        let key = keys
            .get_mut(key_id)
            .ok_or_else(|| KeyRotationError::KeyNotFound(hex::encode(key_id)))?;

        if key.metadata.state == KeyState::Rotating {
            return Err(KeyRotationError::RotationInProgress(hex::encode(key_id)));
        }

        // Create migration transaction
        let migration_tx = KeyMigrationTransaction {
            old_pubkey_hash: *key_id,
            new_pubkey: new_keypair.public_key.clone(),
            new_pubkey_hash: new_key_id,
            old_key_signature: self.sign_migration(&key.keypair, key_id, &new_key_id)?,
            timestamp: now,
            target_height: current_height,
            grace_end_height: current_height + self.policy.grace_period_blocks,
            metadata: None,
        };

        // Store old key in history
        let old_metadata = key.metadata.clone();
        let old_keypair = key.keypair.clone();

        // Trim previous keys if needed
        if key.previous_keys.len() >= self.policy.max_retained_keys {
            key.previous_keys.remove(0);
        }
        key.previous_keys.push((old_keypair, old_metadata));

        // Update to new key
        key.keypair = new_keypair;
        key.metadata.key_id = new_key_id;
        key.metadata.state = KeyState::Rotating;
        key.metadata.last_rotation_height = Some(current_height);
        key.metadata.grace_period_end_height = Some(current_height + self.policy.grace_period_blocks);
        key.metadata.rotation_count += 1;

        // Re-insert with new key ID
        let managed_key = key.clone();
        keys.remove(key_id);
        keys.insert(new_key_id, managed_key);

        drop(keys);

        // Record event
        let event = RotationEvent {
            key_id: *key_id,
            trigger,
            rotation_height: current_height,
            timestamp: now,
            previous_key_hash: *key_id,
            new_key_hash: new_key_id,
            migration_tx_hash: Some(Self::compute_migration_hash(&migration_tx)),
        };

        self.record_event(event)?;

        // Store pending migration
        if self.policy.require_migration_tx {
            let mut pending = self
                .pending_migrations
                .write()
                .map_err(|_| KeyRotationError::LockPoisoned)?;
            pending.push(migration_tx.clone());
        }

        info!(
            "Key rotation initiated: {} -> {} (trigger: {:?})",
            hex::encode(&key_id[..8]),
            hex::encode(&new_key_id[..8]),
            trigger
        );

        Ok(migration_tx)
    }

    /// Complete rotation after grace period
    pub fn complete_rotation(&self, key_id: &[u8; 32]) -> KeyRotationResult<()> {
        let current_height = self.get_current_height()?;

        let mut keys = self
            .keys
            .write()
            .map_err(|_| KeyRotationError::LockPoisoned)?;

        let key = keys
            .get_mut(key_id)
            .ok_or_else(|| KeyRotationError::KeyNotFound(hex::encode(key_id)))?;

        if key.metadata.state != KeyState::Rotating {
            return Err(KeyRotationError::InvalidKeyState(format!(
                "Expected Rotating state, got {:?}",
                key.metadata.state
            )));
        }

        if let Some(grace_end) = key.metadata.grace_period_end_height {
            if current_height < grace_end {
                debug!(
                    "Grace period not ended yet for key {}: {} blocks remaining",
                    hex::encode(&key_id[..8]),
                    grace_end - current_height
                );
                return Ok(());
            }
        }

        key.metadata.state = KeyState::Active;
        key.metadata.grace_period_end_height = None;

        // Mark previous keys as expired
        for (_, prev_metadata) in &mut key.previous_keys {
            prev_metadata.state = KeyState::Expired;
        }

        info!(
            "Key rotation completed for key {}",
            hex::encode(&key_id[..8])
        );

        Ok(())
    }

    /// Emergency rotation (skip grace period)
    pub fn emergency_rotate(
        &self,
        key_id: &[u8; 32],
        trigger: RotationTrigger,
    ) -> KeyRotationResult<KeyMigrationTransaction> {
        if !self.policy.emergency_rotation_enabled {
            return Err(KeyRotationError::InvalidPolicy(
                "Emergency rotation not enabled".to_string(),
            ));
        }

        warn!(
            "Emergency rotation triggered for key {} due to {:?}",
            hex::encode(&key_id[..8]),
            trigger
        );

        // Rotate with zero grace period
        let migration_tx = self.rotate_key(key_id, trigger)?;

        // Immediately complete rotation (revoke old key)
        let mut keys = self
            .keys
            .write()
            .map_err(|_| KeyRotationError::LockPoisoned)?;

        if let Some(key) = keys.get_mut(&migration_tx.new_pubkey_hash) {
            key.metadata.state = KeyState::Active;
            key.metadata.grace_period_end_height = None;

            // Mark old keys as revoked (not just expired)
            for (_, prev_metadata) in &mut key.previous_keys {
                prev_metadata.state = KeyState::Revoked;
            }
        }

        Ok(migration_tx)
    }

    /// Verify a signature with rotation support (checks old keys during grace period)
    pub fn verify_with_rotation(
        &self,
        key_id: &[u8; 32],
        message: &[u8],
        signature: &[u8],
    ) -> KeyRotationResult<bool> {
        let keys = self
            .keys
            .read()
            .map_err(|_| KeyRotationError::LockPoisoned)?;

        // Try current key first
        if let Some(key) = keys.get(key_id) {
            if key.metadata.state == KeyState::Active || key.metadata.state == KeyState::Rotating {
                if key.keypair.verify(message, signature).unwrap_or(false) {
                    return Ok(true);
                }
            }

            // Try previous keys during grace period
            let current_height = self.get_current_height()?;
            for (prev_keypair, prev_metadata) in &key.previous_keys {
                if prev_metadata.state == KeyState::GracePeriod
                    || prev_metadata.state == KeyState::Expired
                {
                    // Check if still in grace period
                    if let Some(grace_end) = key.metadata.grace_period_end_height {
                        if current_height <= grace_end {
                            if prev_keypair.verify(message, signature).unwrap_or(false) {
                                debug!(
                                    "Verified with previous key {} (grace period)",
                                    hex::encode(&prev_metadata.key_id[..8])
                                );
                                return Ok(true);
                            }
                        }
                    }
                }
            }
        }

        // Try to find by iterating all keys (in case key_id is old)
        for (_, managed_key) in keys.iter() {
            for (prev_keypair, prev_metadata) in &managed_key.previous_keys {
                if &prev_metadata.key_id == key_id {
                    let current_height = self.get_current_height()?;
                    if let Some(grace_end) = managed_key.metadata.grace_period_end_height {
                        if current_height <= grace_end {
                            if prev_keypair.verify(message, signature).unwrap_or(false) {
                                debug!(
                                    "Verified with previous key {} via search",
                                    hex::encode(&key_id[..8])
                                );
                                return Ok(true);
                            }
                        } else {
                            return Err(KeyRotationError::GracePeriodExpired(hex::encode(key_id)));
                        }
                    }
                }
            }
        }

        Ok(false)
    }

    /// Get key by ID
    pub fn get_key(&self, key_id: &[u8; 32]) -> KeyRotationResult<Option<ManagedKey>> {
        let keys = self
            .keys
            .read()
            .map_err(|_| KeyRotationError::LockPoisoned)?;
        Ok(keys.get(key_id).cloned())
    }

    /// Get all keys needing rotation
    pub fn get_keys_needing_rotation(&self) -> KeyRotationResult<Vec<[u8; 32]>> {
        let keys = self
            .keys
            .read()
            .map_err(|_| KeyRotationError::LockPoisoned)?;

        let current_height = self.get_current_height()?;
        let mut needing_rotation = Vec::new();

        for (key_id, key) in keys.iter() {
            if key.metadata.state != KeyState::Active {
                continue;
            }

            let last_rotation = key
                .metadata
                .last_rotation_height
                .unwrap_or(key.metadata.created_height);

            if current_height.saturating_sub(last_rotation) >= self.policy.interval_blocks {
                needing_rotation.push(*key_id);
            }
        }

        Ok(needing_rotation)
    }

    /// Process all pending rotations
    pub fn process_pending_rotations(&self) -> KeyRotationResult<Vec<KeyMigrationTransaction>> {
        let keys_to_rotate = self.get_keys_needing_rotation()?;
        let mut migrations = Vec::new();

        for key_id in keys_to_rotate {
            match self.rotate_key(&key_id, RotationTrigger::BlockHeight) {
                Ok(migration) => migrations.push(migration),
                Err(e) => {
                    warn!("Failed to rotate key {}: {}", hex::encode(&key_id[..8]), e);
                }
            }
        }

        Ok(migrations)
    }

    /// Complete all rotations past grace period
    pub fn complete_pending_rotations(&self) -> KeyRotationResult<u32> {
        let keys = self
            .keys
            .read()
            .map_err(|_| KeyRotationError::LockPoisoned)?;

        let rotating_keys: Vec<[u8; 32]> = keys
            .iter()
            .filter(|(_, k)| k.metadata.state == KeyState::Rotating)
            .map(|(id, _)| *id)
            .collect();

        drop(keys);

        let mut completed = 0;
        for key_id in rotating_keys {
            if self.complete_rotation(&key_id).is_ok() {
                completed += 1;
            }
        }

        Ok(completed)
    }

    /// Get pending migrations
    pub fn get_pending_migrations(&self) -> KeyRotationResult<Vec<KeyMigrationTransaction>> {
        let pending = self
            .pending_migrations
            .read()
            .map_err(|_| KeyRotationError::LockPoisoned)?;
        Ok(pending.clone())
    }

    /// Clear confirmed migration
    pub fn confirm_migration(&self, tx_hash: &[u8; 32]) -> KeyRotationResult<()> {
        let mut pending = self
            .pending_migrations
            .write()
            .map_err(|_| KeyRotationError::LockPoisoned)?;

        pending.retain(|tx| &Self::compute_migration_hash(tx) != tx_hash);
        Ok(())
    }

    /// Get rotation events
    pub fn get_events(&self, limit: usize) -> KeyRotationResult<Vec<RotationEvent>> {
        let events = self
            .events
            .read()
            .map_err(|_| KeyRotationError::LockPoisoned)?;

        let start = events.len().saturating_sub(limit);
        Ok(events[start..].to_vec())
    }

    /// Get policy
    pub fn policy(&self) -> &RotationPolicy {
        &self.policy
    }

    // Helper functions

    fn compute_key_id(public_key: &[u8]) -> [u8; 32] {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(public_key);
        let result = hasher.finalize();
        let mut key_id = [0u8; 32];
        key_id.copy_from_slice(&result);
        key_id
    }

    fn compute_migration_hash(tx: &KeyMigrationTransaction) -> [u8; 32] {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(&tx.old_pubkey_hash);
        hasher.update(&tx.new_pubkey_hash);
        hasher.update(&tx.target_height.to_le_bytes());
        hasher.update(&tx.timestamp.to_le_bytes());
        let result = hasher.finalize();
        let mut hash = [0u8; 32];
        hash.copy_from_slice(&result);
        hash
    }

    fn sign_migration(
        &self,
        keypair: &QuantumKeyPair,
        old_key_id: &[u8; 32],
        new_key_id: &[u8; 32],
    ) -> KeyRotationResult<Vec<u8>> {
        let mut message = Vec::with_capacity(64);
        message.extend_from_slice(old_key_id);
        message.extend_from_slice(new_key_id);

        keypair
            .sign(&message)
            .map_err(|e| KeyRotationError::MigrationFailed(e.to_string()))
    }

    fn record_event(&self, event: RotationEvent) -> KeyRotationResult<()> {
        let mut events = self
            .events
            .write()
            .map_err(|_| KeyRotationError::LockPoisoned)?;

        events.push(event);

        // Keep only recent events
        if events.len() > 1000 {
            events.drain(0..100);
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_manager() -> KeyRotationManager {
        let mut policy = RotationPolicy::default();
        policy.interval_blocks = 10; // Short interval for testing
        policy.grace_period_blocks = 5;
        policy.require_migration_tx = false;
        KeyRotationManager::new(policy, QuantumScheme::Dilithium)
    }

    #[test]
    fn test_key_registration() {
        let manager = create_test_manager();
        let key_id = manager.generate_and_register("test_owner".to_string());
        assert!(key_id.is_ok());

        let key = manager.get_key(&key_id.unwrap());
        assert!(key.is_ok());
        assert!(key.unwrap().is_some());
    }

    #[test]
    fn test_rotation_not_needed_initially() {
        let manager = create_test_manager();
        let key_id = manager.generate_and_register("test".to_string()).unwrap();

        manager.update_height(5).unwrap();
        assert!(!manager.needs_rotation(&key_id).unwrap());
    }

    #[test]
    fn test_rotation_needed_after_interval() {
        let manager = create_test_manager();
        let key_id = manager.generate_and_register("test".to_string()).unwrap();

        manager.update_height(15).unwrap(); // Past interval of 10
        assert!(manager.needs_rotation(&key_id).unwrap());
    }

    #[test]
    fn test_key_rotation() {
        let manager = create_test_manager();
        let key_id = manager.generate_and_register("test".to_string()).unwrap();

        manager.update_height(15).unwrap();
        let result = manager.rotate_key(&key_id, RotationTrigger::BlockHeight);
        assert!(result.is_ok());

        let migration = result.unwrap();
        assert_eq!(migration.old_pubkey_hash, key_id);
        assert_ne!(migration.new_pubkey_hash, key_id);
    }

    #[test]
    fn test_grace_period() {
        let manager = create_test_manager();
        let key_id = manager.generate_and_register("test".to_string()).unwrap();

        manager.update_height(15).unwrap();
        let migration = manager.rotate_key(&key_id, RotationTrigger::Manual).unwrap();
        let new_key_id = migration.new_pubkey_hash;

        // Key should be in rotating state
        let key = manager.get_key(&new_key_id).unwrap().unwrap();
        assert_eq!(key.metadata.state, KeyState::Rotating);

        // Should have one previous key
        assert_eq!(key.previous_keys.len(), 1);
    }

    #[test]
    fn test_rotation_completion() {
        let manager = create_test_manager();
        let key_id = manager.generate_and_register("test".to_string()).unwrap();

        manager.update_height(15).unwrap();
        let migration = manager.rotate_key(&key_id, RotationTrigger::Manual).unwrap();
        let new_key_id = migration.new_pubkey_hash;

        // Advance past grace period
        manager.update_height(25).unwrap();
        manager.complete_rotation(&new_key_id).unwrap();

        let key = manager.get_key(&new_key_id).unwrap().unwrap();
        assert_eq!(key.metadata.state, KeyState::Active);
    }

    #[test]
    fn test_emergency_rotation() {
        let manager = create_test_manager();
        let key_id = manager.generate_and_register("test".to_string()).unwrap();

        let result = manager.emergency_rotate(&key_id, RotationTrigger::KeyCompromise);
        assert!(result.is_ok());

        let migration = result.unwrap();
        let new_key = manager.get_key(&migration.new_pubkey_hash).unwrap().unwrap();

        // Should be immediately active (no grace period)
        assert_eq!(new_key.metadata.state, KeyState::Active);

        // Old key should be revoked
        assert!(new_key.previous_keys.iter().all(|(_, m)| m.state == KeyState::Revoked));
    }

    #[test]
    fn test_events_recorded() {
        let manager = create_test_manager();
        let key_id = manager.generate_and_register("test".to_string()).unwrap();

        manager.update_height(15).unwrap();
        manager.rotate_key(&key_id, RotationTrigger::Manual).unwrap();

        let events = manager.get_events(10).unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].trigger, RotationTrigger::Manual);
    }

    #[test]
    fn test_keys_needing_rotation() {
        let manager = create_test_manager();
        let key1 = manager.generate_and_register("test1".to_string()).unwrap();
        let key2 = manager.generate_and_register("test2".to_string()).unwrap();

        manager.update_height(15).unwrap();

        let needing = manager.get_keys_needing_rotation().unwrap();
        assert_eq!(needing.len(), 2);
        assert!(needing.contains(&key1));
        assert!(needing.contains(&key2));
    }

    #[test]
    fn test_process_pending_rotations() {
        let manager = create_test_manager();
        manager.generate_and_register("test1".to_string()).unwrap();
        manager.generate_and_register("test2".to_string()).unwrap();

        manager.update_height(15).unwrap();

        let migrations = manager.process_pending_rotations().unwrap();
        assert_eq!(migrations.len(), 2);
    }
}
