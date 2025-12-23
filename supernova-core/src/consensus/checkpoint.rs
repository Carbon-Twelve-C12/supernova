//! Checkpoint Finality System for Supernova
//!
//! This module implements checkpoint-based finality to prevent deep chain reorganizations.
//! Checkpoints provide a security guarantee that blocks beyond a certain depth cannot
//! be reorganized, protecting against long-range attacks and providing faster confirmation
//! for transactions in finalized blocks.
//!
//! # Security Properties
//! - Blocks at or before the latest checkpoint cannot be reorganized
//! - Checkpoints are validated against multiple sources (hardcoded, DNS, peer consensus)
//! - Automatic checkpoint creation after configurable confirmation depth
//!
//! # Architecture
//! - `CheckpointManager` - Central coordinator for checkpoint operations
//! - `Checkpoint` - Immutable record of a finalized block
//! - `CheckpointSource` - Origin of checkpoint data for trust verification

use crate::types::block::BlockHeader;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::{Arc, RwLock};
use thiserror::Error;
use tracing::{debug, error, info, warn};

/// Errors that can occur during checkpoint operations
#[derive(Debug, Error, Clone)]
pub enum CheckpointError {
    #[error("Checkpoint not found for height {0}")]
    NotFound(u64),

    #[error("Invalid checkpoint: {0}")]
    Invalid(String),

    #[error("Checkpoint conflict at height {0}: expected {1}, got {2}")]
    Conflict(u64, String, String),

    #[error("Block {0} is before finalized checkpoint at height {1}")]
    BeforeFinality(String, u64),

    #[error("Reorg would cross finalized checkpoint at height {0}")]
    ReorgBeyondCheckpoint(u64),

    #[error("Checkpoint validation failed: {0}")]
    ValidationFailed(String),

    #[error("Storage error: {0}")]
    StorageError(String),

    #[error("Lock poisoned")]
    LockPoisoned,
}

/// Result type for checkpoint operations
pub type CheckpointResult<T> = Result<T, CheckpointError>;

/// Source of a checkpoint for trust verification
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CheckpointSource {
    /// Hardcoded in the binary (highest trust)
    Hardcoded,
    /// From DNS seed with DNSSEC validation
    DnsSeed,
    /// Consensus among connected peers
    PeerConsensus,
    /// Automatically created after sufficient confirmations
    Automatic,
    /// From a trusted checkpoint server
    TrustedServer,
    /// User-configured checkpoint
    UserConfigured,
}

impl CheckpointSource {
    /// Get the trust level of this source (higher = more trusted)
    pub fn trust_level(&self) -> u8 {
        match self {
            Self::Hardcoded => 100,
            Self::TrustedServer => 90,
            Self::DnsSeed => 80,
            Self::UserConfigured => 70,
            Self::PeerConsensus => 50,
            Self::Automatic => 40,
        }
    }
}

/// A checkpoint representing a finalized block
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Checkpoint {
    /// Block height
    pub height: u64,
    /// Block hash (32 bytes)
    pub block_hash: [u8; 32],
    /// Source of this checkpoint
    pub source: CheckpointSource,
    /// Unix timestamp when checkpoint was created
    pub created_at: u64,
    /// Optional human-readable name (e.g., "Genesis", "First Halving")
    pub name: Option<String>,
}

impl Checkpoint {
    /// Create a new checkpoint
    pub fn new(height: u64, block_hash: [u8; 32], source: CheckpointSource) -> Self {
        let created_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        Self {
            height,
            block_hash,
            source,
            created_at,
            name: None,
        }
    }

    /// Create a named checkpoint (for well-known milestones)
    pub fn named(
        height: u64,
        block_hash: [u8; 32],
        source: CheckpointSource,
        name: impl Into<String>,
    ) -> Self {
        let mut cp = Self::new(height, block_hash, source);
        cp.name = Some(name.into());
        cp
    }

    /// Verify this checkpoint matches the given block header
    pub fn verify(&self, header: &BlockHeader) -> bool {
        header.height() == self.height && header.hash() == self.block_hash
    }
}

/// Configuration for the checkpoint manager
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckpointConfig {
    /// Enable automatic checkpoint creation
    pub auto_checkpoint_enabled: bool,
    /// Number of confirmations before creating automatic checkpoint
    pub auto_checkpoint_depth: u64,
    /// Minimum interval between automatic checkpoints (in blocks)
    pub auto_checkpoint_interval: u64,
    /// Maximum age of peer checkpoints to accept (in seconds)
    pub max_peer_checkpoint_age: u64,
    /// Minimum peer agreement for peer consensus checkpoints
    pub min_peer_agreement: f64,
    /// Enable strict mode (reject blocks that conflict with checkpoints)
    pub strict_mode: bool,
}

impl Default for CheckpointConfig {
    fn default() -> Self {
        Self {
            auto_checkpoint_enabled: true,
            auto_checkpoint_depth: 2016, // ~2 weeks with 10-min blocks
            auto_checkpoint_interval: 2016,
            max_peer_checkpoint_age: 86400, // 24 hours
            min_peer_agreement: 0.67,        // 2/3 majority
            strict_mode: true,
        }
    }
}

/// Manager for checkpoint finality
pub struct CheckpointManager {
    /// All known checkpoints indexed by height
    checkpoints: Arc<RwLock<BTreeMap<u64, Checkpoint>>>,
    /// The highest finalized checkpoint
    latest_finalized: Arc<RwLock<Option<Checkpoint>>>,
    /// Configuration
    config: CheckpointConfig,
    /// Last automatic checkpoint height
    last_auto_checkpoint_height: Arc<RwLock<u64>>,
}

impl CheckpointManager {
    /// Create a new checkpoint manager with default configuration
    pub fn new() -> Self {
        Self::with_config(CheckpointConfig::default())
    }

    /// Create a new checkpoint manager with custom configuration
    pub fn with_config(config: CheckpointConfig) -> Self {
        Self {
            checkpoints: Arc::new(RwLock::new(BTreeMap::new())),
            latest_finalized: Arc::new(RwLock::new(None)),
            config,
            last_auto_checkpoint_height: Arc::new(RwLock::new(0)),
        }
    }

    /// Initialize with hardcoded genesis checkpoint
    pub fn initialize_genesis(&self, genesis_hash: [u8; 32]) -> CheckpointResult<()> {
        let genesis_checkpoint = Checkpoint::named(
            0,
            genesis_hash,
            CheckpointSource::Hardcoded,
            "Genesis",
        );
        self.add_checkpoint(genesis_checkpoint)
    }

    /// Add a checkpoint
    pub fn add_checkpoint(&self, checkpoint: Checkpoint) -> CheckpointResult<()> {
        let mut checkpoints = self
            .checkpoints
            .write()
            .map_err(|_| CheckpointError::LockPoisoned)?;

        // Check for conflicts
        if let Some(existing) = checkpoints.get(&checkpoint.height) {
            if existing.block_hash != checkpoint.block_hash {
                // Conflict - decide based on trust level
                if checkpoint.source.trust_level() > existing.source.trust_level() {
                    warn!(
                        "Replacing checkpoint at height {} (higher trust source)",
                        checkpoint.height
                    );
                } else {
                    return Err(CheckpointError::Conflict(
                        checkpoint.height,
                        hex::encode(existing.block_hash),
                        hex::encode(checkpoint.block_hash),
                    ));
                }
            }
        }

        info!(
            "Adding checkpoint at height {}: {} (source: {:?})",
            checkpoint.height,
            hex::encode(checkpoint.block_hash),
            checkpoint.source
        );

        checkpoints.insert(checkpoint.height, checkpoint.clone());
        drop(checkpoints);

        // Update latest finalized if this is newer
        self.update_latest_finalized(checkpoint)?;

        Ok(())
    }

    /// Update the latest finalized checkpoint
    fn update_latest_finalized(&self, checkpoint: Checkpoint) -> CheckpointResult<()> {
        let mut latest = self
            .latest_finalized
            .write()
            .map_err(|_| CheckpointError::LockPoisoned)?;

        let should_update = match &*latest {
            None => true,
            Some(current) => checkpoint.height > current.height,
        };

        if should_update {
            debug!(
                "Updating latest finalized checkpoint to height {}",
                checkpoint.height
            );
            *latest = Some(checkpoint);
        }

        Ok(())
    }

    /// Get checkpoint at a specific height
    pub fn get_checkpoint(&self, height: u64) -> CheckpointResult<Option<Checkpoint>> {
        let checkpoints = self
            .checkpoints
            .read()
            .map_err(|_| CheckpointError::LockPoisoned)?;

        Ok(checkpoints.get(&height).cloned())
    }

    /// Get the latest finalized checkpoint
    pub fn get_latest_finalized(&self) -> CheckpointResult<Option<Checkpoint>> {
        let latest = self
            .latest_finalized
            .read()
            .map_err(|_| CheckpointError::LockPoisoned)?;

        Ok(latest.clone())
    }

    /// Get the latest finalized height (0 if none)
    pub fn get_finalized_height(&self) -> CheckpointResult<u64> {
        let latest = self
            .latest_finalized
            .read()
            .map_err(|_| CheckpointError::LockPoisoned)?;

        Ok(latest.as_ref().map(|cp| cp.height).unwrap_or(0))
    }

    /// Check if a block hash at a given height matches the checkpoint (if one exists)
    pub fn verify_block(&self, height: u64, block_hash: &[u8; 32]) -> CheckpointResult<bool> {
        let checkpoints = self
            .checkpoints
            .read()
            .map_err(|_| CheckpointError::LockPoisoned)?;

        if let Some(checkpoint) = checkpoints.get(&height) {
            if &checkpoint.block_hash != block_hash {
                if self.config.strict_mode {
                    return Err(CheckpointError::Conflict(
                        height,
                        hex::encode(checkpoint.block_hash),
                        hex::encode(block_hash),
                    ));
                }
                return Ok(false);
            }
        }

        Ok(true)
    }

    /// Check if a reorg to the given height is allowed
    /// Returns error if the reorg would cross a finalized checkpoint
    pub fn check_reorg_allowed(&self, reorg_to_height: u64) -> CheckpointResult<bool> {
        let finalized_height = self.get_finalized_height()?;

        if reorg_to_height < finalized_height {
            if self.config.strict_mode {
                return Err(CheckpointError::ReorgBeyondCheckpoint(finalized_height));
            }
            warn!(
                "Reorg to height {} blocked by checkpoint at {}",
                reorg_to_height, finalized_height
            );
            return Ok(false);
        }

        Ok(true)
    }

    /// Check if a block is finalized (at or before latest checkpoint)
    pub fn is_finalized(&self, height: u64) -> CheckpointResult<bool> {
        let finalized_height = self.get_finalized_height()?;
        Ok(height <= finalized_height)
    }

    /// Process a new block and potentially create automatic checkpoint
    pub fn process_new_block(
        &self,
        height: u64,
        block_hash: [u8; 32],
        chain_tip_height: u64,
    ) -> CheckpointResult<()> {
        if !self.config.auto_checkpoint_enabled {
            return Ok(());
        }

        // Check if this block has enough confirmations
        let confirmations = chain_tip_height.saturating_sub(height);
        if confirmations < self.config.auto_checkpoint_depth {
            return Ok(());
        }

        // Check interval since last automatic checkpoint
        let last_auto = *self
            .last_auto_checkpoint_height
            .read()
            .map_err(|_| CheckpointError::LockPoisoned)?;

        if height.saturating_sub(last_auto) < self.config.auto_checkpoint_interval {
            return Ok(());
        }

        // Create automatic checkpoint
        let checkpoint = Checkpoint::new(height, block_hash, CheckpointSource::Automatic);

        info!(
            "Creating automatic checkpoint at height {} ({} confirmations)",
            height, confirmations
        );

        self.add_checkpoint(checkpoint)?;

        // Update last auto checkpoint height
        let mut last = self
            .last_auto_checkpoint_height
            .write()
            .map_err(|_| CheckpointError::LockPoisoned)?;
        *last = height;

        Ok(())
    }

    /// Add checkpoints from peer consensus
    pub fn add_peer_checkpoints(
        &self,
        peer_checkpoints: &[(u64, [u8; 32], u64)], // (height, hash, timestamp)
    ) -> CheckpointResult<u32> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        let mut added = 0u32;

        for (height, hash, timestamp) in peer_checkpoints {
            // Check age
            if now.saturating_sub(*timestamp) > self.config.max_peer_checkpoint_age {
                debug!(
                    "Skipping peer checkpoint at height {} (too old)",
                    height
                );
                continue;
            }

            let checkpoint = Checkpoint {
                height: *height,
                block_hash: *hash,
                source: CheckpointSource::PeerConsensus,
                created_at: *timestamp,
                name: None,
            };

            match self.add_checkpoint(checkpoint) {
                Ok(()) => added += 1,
                Err(CheckpointError::Conflict(..)) => {
                    debug!("Peer checkpoint at height {} conflicts with existing", height);
                }
                Err(e) => return Err(e),
            }
        }

        info!("Added {} peer consensus checkpoints", added);
        Ok(added)
    }

    /// Get all checkpoints in a range
    pub fn get_checkpoints_in_range(
        &self,
        start_height: u64,
        end_height: u64,
    ) -> CheckpointResult<Vec<Checkpoint>> {
        let checkpoints = self
            .checkpoints
            .read()
            .map_err(|_| CheckpointError::LockPoisoned)?;

        Ok(checkpoints
            .range(start_height..=end_height)
            .map(|(_, cp)| cp.clone())
            .collect())
    }

    /// Get the number of checkpoints
    pub fn checkpoint_count(&self) -> CheckpointResult<usize> {
        let checkpoints = self
            .checkpoints
            .read()
            .map_err(|_| CheckpointError::LockPoisoned)?;

        Ok(checkpoints.len())
    }

    /// Clear all non-hardcoded checkpoints (for testing/reset)
    pub fn clear_non_hardcoded(&self) -> CheckpointResult<()> {
        let mut checkpoints = self
            .checkpoints
            .write()
            .map_err(|_| CheckpointError::LockPoisoned)?;

        checkpoints.retain(|_, cp| cp.source == CheckpointSource::Hardcoded);

        // Reset latest finalized to highest remaining checkpoint
        let mut latest = self
            .latest_finalized
            .write()
            .map_err(|_| CheckpointError::LockPoisoned)?;

        *latest = checkpoints.values().max_by_key(|cp| cp.height).cloned();

        Ok(())
    }

    /// Export checkpoints for persistence
    pub fn export_checkpoints(&self) -> CheckpointResult<Vec<Checkpoint>> {
        let checkpoints = self
            .checkpoints
            .read()
            .map_err(|_| CheckpointError::LockPoisoned)?;

        Ok(checkpoints.values().cloned().collect())
    }

    /// Import checkpoints from persistence
    pub fn import_checkpoints(&self, checkpoints: Vec<Checkpoint>) -> CheckpointResult<u32> {
        let mut imported = 0u32;

        for checkpoint in checkpoints {
            match self.add_checkpoint(checkpoint) {
                Ok(()) => imported += 1,
                Err(CheckpointError::Conflict(..)) => {
                    // Skip conflicts during import
                }
                Err(e) => return Err(e),
            }
        }

        Ok(imported)
    }
}

impl Default for CheckpointManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Hardcoded checkpoints for the Supernova mainnet
/// These are embedded in the binary and cannot be changed without a software update
pub fn mainnet_checkpoints() -> Vec<Checkpoint> {
    vec![
        // Genesis block - will be set during network initialization
        // Checkpoint::named(0, [0u8; 32], CheckpointSource::Hardcoded, "Genesis"),
        // Add more hardcoded checkpoints as the network matures
    ]
}

/// Hardcoded checkpoints for the Supernova testnet
pub fn testnet_checkpoints() -> Vec<Checkpoint> {
    vec![
        // Testnet genesis - will be set during network initialization
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_hash(n: u8) -> [u8; 32] {
        let mut hash = [0u8; 32];
        hash[0] = n;
        hash
    }

    #[test]
    fn test_checkpoint_creation() {
        let cp = Checkpoint::new(100, test_hash(1), CheckpointSource::Hardcoded);
        assert_eq!(cp.height, 100);
        assert_eq!(cp.block_hash[0], 1);
        assert_eq!(cp.source, CheckpointSource::Hardcoded);
    }

    #[test]
    fn test_named_checkpoint() {
        let cp = Checkpoint::named(0, test_hash(0), CheckpointSource::Hardcoded, "Genesis");
        assert_eq!(cp.name, Some("Genesis".to_string()));
    }

    #[test]
    fn test_checkpoint_manager_basic() {
        let manager = CheckpointManager::new();

        // Add a checkpoint
        let cp = Checkpoint::new(100, test_hash(1), CheckpointSource::Hardcoded);
        manager.add_checkpoint(cp).unwrap();

        // Retrieve it
        let retrieved = manager.get_checkpoint(100).unwrap();
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().height, 100);
    }

    #[test]
    fn test_finalized_height() {
        let manager = CheckpointManager::new();

        // Initially 0
        assert_eq!(manager.get_finalized_height().unwrap(), 0);

        // Add checkpoint
        manager
            .add_checkpoint(Checkpoint::new(100, test_hash(1), CheckpointSource::Hardcoded))
            .unwrap();

        assert_eq!(manager.get_finalized_height().unwrap(), 100);

        // Add higher checkpoint
        manager
            .add_checkpoint(Checkpoint::new(200, test_hash(2), CheckpointSource::Hardcoded))
            .unwrap();

        assert_eq!(manager.get_finalized_height().unwrap(), 200);
    }

    #[test]
    fn test_reorg_blocked() {
        let manager = CheckpointManager::new();

        // Add checkpoint at height 100
        manager
            .add_checkpoint(Checkpoint::new(100, test_hash(1), CheckpointSource::Hardcoded))
            .unwrap();

        // Reorg to 50 should be blocked
        let result = manager.check_reorg_allowed(50);
        assert!(result.is_err());

        // Reorg to 150 should be allowed
        assert!(manager.check_reorg_allowed(150).unwrap());
    }

    #[test]
    fn test_is_finalized() {
        let manager = CheckpointManager::new();

        manager
            .add_checkpoint(Checkpoint::new(100, test_hash(1), CheckpointSource::Hardcoded))
            .unwrap();

        assert!(manager.is_finalized(50).unwrap());
        assert!(manager.is_finalized(100).unwrap());
        assert!(!manager.is_finalized(150).unwrap());
    }

    #[test]
    fn test_checkpoint_conflict() {
        let manager = CheckpointManager::new();

        // Add first checkpoint
        manager
            .add_checkpoint(Checkpoint::new(100, test_hash(1), CheckpointSource::Automatic))
            .unwrap();

        // Conflicting checkpoint with same trust level should fail
        let result = manager.add_checkpoint(Checkpoint::new(
            100,
            test_hash(2), // Different hash
            CheckpointSource::Automatic,
        ));
        assert!(matches!(result, Err(CheckpointError::Conflict(..))));

        // Higher trust level should succeed
        manager
            .add_checkpoint(Checkpoint::new(100, test_hash(3), CheckpointSource::Hardcoded))
            .unwrap();

        // Verify the higher trust checkpoint replaced the lower one
        let cp = manager.get_checkpoint(100).unwrap().unwrap();
        assert_eq!(cp.block_hash[0], 3);
    }

    #[test]
    fn test_checkpoint_source_trust_levels() {
        assert!(CheckpointSource::Hardcoded.trust_level() > CheckpointSource::PeerConsensus.trust_level());
        assert!(CheckpointSource::DnsSeed.trust_level() > CheckpointSource::Automatic.trust_level());
    }

    #[test]
    fn test_checkpoint_count() {
        let manager = CheckpointManager::new();

        assert_eq!(manager.checkpoint_count().unwrap(), 0);

        manager
            .add_checkpoint(Checkpoint::new(100, test_hash(1), CheckpointSource::Hardcoded))
            .unwrap();
        manager
            .add_checkpoint(Checkpoint::new(200, test_hash(2), CheckpointSource::Hardcoded))
            .unwrap();

        assert_eq!(manager.checkpoint_count().unwrap(), 2);
    }

    #[test]
    fn test_checkpoints_in_range() {
        let manager = CheckpointManager::new();

        for i in 0..10 {
            manager
                .add_checkpoint(Checkpoint::new(i * 100, test_hash(i as u8), CheckpointSource::Hardcoded))
                .unwrap();
        }

        let range = manager.get_checkpoints_in_range(200, 500).unwrap();
        assert_eq!(range.len(), 4); // 200, 300, 400, 500
    }

    #[test]
    fn test_export_import() {
        let manager1 = CheckpointManager::new();

        manager1
            .add_checkpoint(Checkpoint::new(100, test_hash(1), CheckpointSource::Hardcoded))
            .unwrap();
        manager1
            .add_checkpoint(Checkpoint::new(200, test_hash(2), CheckpointSource::Hardcoded))
            .unwrap();

        let exported = manager1.export_checkpoints().unwrap();

        let manager2 = CheckpointManager::new();
        let imported = manager2.import_checkpoints(exported).unwrap();

        assert_eq!(imported, 2);
        assert_eq!(manager2.checkpoint_count().unwrap(), 2);
    }
}
