//! Weak Subjectivity for Long-Range Attack Protection
//!
//! This module implements weak subjectivity checks to protect against long-range attacks.
//! In Proof-of-Work systems, an attacker with sufficient historical hashpower could
//! potentially create an alternate chain from a point far in the past. Weak subjectivity
//! ensures that nodes joining the network must trust a recent checkpoint to prevent
//! accepting such fraudulent chains.
//!
//! # Security Model
//! - New nodes MUST obtain a weak subjectivity checkpoint from a trusted source
//! - Nodes cannot accept chains that diverge before their weak subjectivity checkpoint
//! - The weak subjectivity period defines how far back a trusted checkpoint can be
//!
//! # Integration Points
//! - Initial Block Download (IBD): Validates chain against WS checkpoint
//! - Peer validation: Rejects peers on incompatible chains
//! - Fork resolution: Prevents reorgs past WS boundary

use super::checkpoint::{Checkpoint, CheckpointError, CheckpointManager, CheckpointSource};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use thiserror::Error;
use tracing::{debug, error, info, warn};

/// Default weak subjectivity period in blocks
/// This represents approximately 2 weeks at 10-minute block times
pub const DEFAULT_WS_PERIOD_BLOCKS: u64 = 2016;

/// Default weak subjectivity period in seconds (2 weeks)
pub const DEFAULT_WS_PERIOD_SECONDS: u64 = 14 * 24 * 60 * 60;

/// Maximum age of a weak subjectivity checkpoint before requiring refresh
pub const MAX_WS_CHECKPOINT_AGE_SECONDS: u64 = 30 * 24 * 60 * 60; // 30 days

/// Minimum confirmations required before creating automatic WS checkpoint
pub const MIN_WS_CONFIRMATIONS: u64 = 100;

/// Errors that can occur during weak subjectivity operations
#[derive(Debug, Error, Clone)]
pub enum WeakSubjectivityError {
    #[error("No weak subjectivity checkpoint configured")]
    NoCheckpoint,

    #[error("Weak subjectivity checkpoint too old: age {0} seconds exceeds maximum {1}")]
    CheckpointTooOld(u64, u64),

    #[error("Chain diverges before weak subjectivity checkpoint at height {0}")]
    ChainDivergence(u64),

    #[error("Peer chain incompatible: diverges at height {0}, WS checkpoint at {1}")]
    IncompatiblePeer(u64, u64),

    #[error("Cannot verify chain: missing block at height {0}")]
    MissingBlock(u64),

    #[error("Invalid weak subjectivity checkpoint: {0}")]
    InvalidCheckpoint(String),

    #[error("Weak subjectivity period exceeded: current height {0}, checkpoint height {1}")]
    PeriodExceeded(u64, u64),

    #[error("Checkpoint source not trusted: {0:?}")]
    UntrustedSource(CheckpointSource),

    #[error("Checkpoint error: {0}")]
    CheckpointError(#[from] CheckpointError),

    #[error("Lock poisoned")]
    LockPoisoned,
}

/// Result type for weak subjectivity operations
pub type WeakSubjectivityResult<T> = Result<T, WeakSubjectivityError>;

/// Configuration for weak subjectivity
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WeakSubjectivityConfig {
    /// Period in blocks after which weak subjectivity applies
    pub period_blocks: u64,
    /// Period in seconds (alternative to blocks)
    pub period_seconds: u64,
    /// Whether to enforce weak subjectivity checks
    pub enforce: bool,
    /// Minimum trust level required for WS checkpoint source
    pub min_trust_level: u8,
    /// Whether to automatically create WS checkpoints
    pub auto_checkpoint: bool,
    /// Confirmations required for auto checkpoints
    pub auto_checkpoint_confirmations: u64,
    /// DNS seeds for fetching WS checkpoints
    pub dns_seeds: Vec<String>,
    /// Trusted checkpoint servers
    pub trusted_servers: Vec<String>,
    /// Whether to warn (vs error) on WS violations in testnet
    pub testnet_warn_only: bool,
}

impl Default for WeakSubjectivityConfig {
    fn default() -> Self {
        Self {
            period_blocks: DEFAULT_WS_PERIOD_BLOCKS,
            period_seconds: DEFAULT_WS_PERIOD_SECONDS,
            enforce: true,
            min_trust_level: 50,
            auto_checkpoint: true,
            auto_checkpoint_confirmations: MIN_WS_CONFIRMATIONS,
            dns_seeds: vec![
                "ws-checkpoint.supernovanetwork.xyz".to_string(),
                "ws-checkpoint-2.supernovanetwork.xyz".to_string(),
            ],
            trusted_servers: vec!["https://checkpoints.supernovanetwork.xyz".to_string()],
            testnet_warn_only: true,
        }
    }
}

/// State of weak subjectivity validation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WeakSubjectivityState {
    /// No WS checkpoint configured, node is vulnerable
    Unprotected,
    /// WS checkpoint is valid and recent
    Protected,
    /// WS checkpoint exists but is getting old, should refresh
    NeedsRefresh,
    /// WS checkpoint is too old, must refresh before accepting new blocks
    Expired,
}

/// Information about a peer's chain for WS validation
#[derive(Debug, Clone)]
pub struct PeerChainInfo {
    /// Peer's reported chain tip hash
    pub tip_hash: [u8; 32],
    /// Peer's reported chain height
    pub tip_height: u64,
    /// Block hash at WS checkpoint height (if peer has it)
    pub ws_checkpoint_hash: Option<[u8; 32]>,
    /// Peer's total accumulated work
    pub total_work: u128,
}

/// Manager for weak subjectivity operations
pub struct WeakSubjectivityManager {
    /// Configuration
    config: WeakSubjectivityConfig,
    /// Current weak subjectivity checkpoint
    ws_checkpoint: Arc<RwLock<Option<Checkpoint>>>,
    /// Checkpoint manager for integration
    checkpoint_manager: Arc<CheckpointManager>,
    /// Cache of validated peer chains (peer_id -> last validated height)
    validated_peers: Arc<RwLock<HashMap<String, u64>>>,
    /// Current state
    state: Arc<RwLock<WeakSubjectivityState>>,
}

impl WeakSubjectivityManager {
    /// Create a new weak subjectivity manager
    pub fn new(config: WeakSubjectivityConfig, checkpoint_manager: Arc<CheckpointManager>) -> Self {
        Self {
            config,
            ws_checkpoint: Arc::new(RwLock::new(None)),
            checkpoint_manager,
            validated_peers: Arc::new(RwLock::new(HashMap::new())),
            state: Arc::new(RwLock::new(WeakSubjectivityState::Unprotected)),
        }
    }

    /// Create with default configuration
    pub fn with_defaults(checkpoint_manager: Arc<CheckpointManager>) -> Self {
        Self::new(WeakSubjectivityConfig::default(), checkpoint_manager)
    }

    /// Initialize the manager, loading or fetching WS checkpoint
    pub fn initialize(&self) -> WeakSubjectivityResult<()> {
        info!("Initializing weak subjectivity manager");

        // First, try to get the latest finalized checkpoint from checkpoint manager
        if let Ok(Some(checkpoint)) = self.checkpoint_manager.get_latest_finalized() {
            if checkpoint.source.trust_level() >= self.config.min_trust_level {
                info!(
                    "Using checkpoint at height {} as WS checkpoint (source: {:?})",
                    checkpoint.height, checkpoint.source
                );
                self.set_ws_checkpoint(checkpoint)?;
                return Ok(());
            }
        }

        // If no suitable checkpoint, we're unprotected
        warn!("No suitable weak subjectivity checkpoint found - node is vulnerable to long-range attacks");
        self.update_state(WeakSubjectivityState::Unprotected)?;

        Ok(())
    }

    /// Set the weak subjectivity checkpoint
    pub fn set_ws_checkpoint(&self, checkpoint: Checkpoint) -> WeakSubjectivityResult<()> {
        // Validate checkpoint source trust level
        if checkpoint.source.trust_level() < self.config.min_trust_level {
            return Err(WeakSubjectivityError::UntrustedSource(checkpoint.source));
        }

        let mut ws_cp = self
            .ws_checkpoint
            .write()
            .map_err(|_| WeakSubjectivityError::LockPoisoned)?;

        // If we already have a checkpoint, only allow updating to a higher one
        if let Some(existing) = ws_cp.as_ref() {
            if checkpoint.height < existing.height {
                return Err(WeakSubjectivityError::InvalidCheckpoint(format!(
                    "Cannot downgrade WS checkpoint from height {} to {}",
                    existing.height, checkpoint.height
                )));
            }
        }

        info!(
            "Setting weak subjectivity checkpoint: height={}, hash={}, source={:?}",
            checkpoint.height,
            hex::encode(&checkpoint.block_hash[..8]),
            checkpoint.source
        );

        *ws_cp = Some(checkpoint);
        drop(ws_cp);

        // Update state based on checkpoint age
        self.refresh_state()?;

        Ok(())
    }

    /// Get the current WS checkpoint
    pub fn get_ws_checkpoint(&self) -> WeakSubjectivityResult<Option<Checkpoint>> {
        let ws_cp = self
            .ws_checkpoint
            .read()
            .map_err(|_| WeakSubjectivityError::LockPoisoned)?;
        Ok(ws_cp.clone())
    }

    /// Get current protection state
    pub fn get_state(&self) -> WeakSubjectivityResult<WeakSubjectivityState> {
        let state = self
            .state
            .read()
            .map_err(|_| WeakSubjectivityError::LockPoisoned)?;
        Ok(*state)
    }

    /// Check if a block height is within the weak subjectivity period
    pub fn is_within_ws_period(&self, block_height: u64) -> WeakSubjectivityResult<bool> {
        let ws_cp = self
            .ws_checkpoint
            .read()
            .map_err(|_| WeakSubjectivityError::LockPoisoned)?;

        match ws_cp.as_ref() {
            Some(checkpoint) => Ok(block_height >= checkpoint.height),
            None => Ok(true), // No checkpoint means all heights are valid (unprotected)
        }
    }

    /// Validate that a chain is compatible with our WS checkpoint
    ///
    /// This is the core security check - ensures the provided chain includes
    /// our WS checkpoint block at the correct height.
    pub fn validate_chain<F>(
        &self,
        tip_height: u64,
        get_block_hash: F,
    ) -> WeakSubjectivityResult<bool>
    where
        F: Fn(u64) -> Option<[u8; 32]>,
    {
        let ws_cp = self
            .ws_checkpoint
            .read()
            .map_err(|_| WeakSubjectivityError::LockPoisoned)?;

        let checkpoint = match ws_cp.as_ref() {
            Some(cp) => cp,
            None => {
                if self.config.enforce {
                    warn!("No WS checkpoint configured, chain validation skipped");
                }
                return Ok(true);
            }
        };

        // If chain tip is below our checkpoint, it's incomplete
        if tip_height < checkpoint.height {
            debug!(
                "Chain tip {} below WS checkpoint {}, cannot validate yet",
                tip_height, checkpoint.height
            );
            return Ok(true); // Allow sync to continue
        }

        // Get the block hash at our checkpoint height
        let chain_hash = get_block_hash(checkpoint.height).ok_or_else(|| {
            WeakSubjectivityError::MissingBlock(checkpoint.height)
        })?;

        // Compare with our checkpoint
        if chain_hash != checkpoint.block_hash {
            error!(
                "Chain diverges from WS checkpoint! Height: {}, Expected: {}, Got: {}",
                checkpoint.height,
                hex::encode(&checkpoint.block_hash[..8]),
                hex::encode(&chain_hash[..8])
            );
            return Err(WeakSubjectivityError::ChainDivergence(checkpoint.height));
        }

        debug!(
            "Chain validated against WS checkpoint at height {}",
            checkpoint.height
        );
        Ok(true)
    }

    /// Validate a peer's chain info against our WS checkpoint
    pub fn validate_peer_chain(&self, peer_id: &str, chain_info: &PeerChainInfo) -> WeakSubjectivityResult<bool> {
        let ws_cp = self
            .ws_checkpoint
            .read()
            .map_err(|_| WeakSubjectivityError::LockPoisoned)?;

        let checkpoint = match ws_cp.as_ref() {
            Some(cp) => cp,
            None => return Ok(true), // No checkpoint, accept all peers
        };

        // If peer's chain is shorter than our checkpoint, that's okay during sync
        if chain_info.tip_height < checkpoint.height {
            debug!(
                "Peer {} chain height {} below WS checkpoint {}, allowing sync",
                peer_id, chain_info.tip_height, checkpoint.height
            );
            return Ok(true);
        }

        // If peer provided hash at our checkpoint height, validate it
        if let Some(peer_ws_hash) = chain_info.ws_checkpoint_hash {
            if peer_ws_hash != checkpoint.block_hash {
                warn!(
                    "Peer {} on incompatible chain: diverges at WS checkpoint height {}",
                    peer_id, checkpoint.height
                );
                return Err(WeakSubjectivityError::IncompatiblePeer(
                    checkpoint.height,
                    checkpoint.height,
                ));
            }

            // Cache this peer as validated
            let mut validated = self
                .validated_peers
                .write()
                .map_err(|_| WeakSubjectivityError::LockPoisoned)?;
            validated.insert(peer_id.to_string(), chain_info.tip_height);

            debug!(
                "Peer {} validated against WS checkpoint at height {}",
                peer_id, checkpoint.height
            );
        }

        Ok(true)
    }

    /// Check if a peer has been validated
    pub fn is_peer_validated(&self, peer_id: &str) -> WeakSubjectivityResult<bool> {
        let validated = self
            .validated_peers
            .read()
            .map_err(|_| WeakSubjectivityError::LockPoisoned)?;
        Ok(validated.contains_key(peer_id))
    }

    /// Check if a reorg is allowed (doesn't cross WS boundary)
    pub fn can_reorg_to_height(&self, target_height: u64) -> WeakSubjectivityResult<bool> {
        let ws_cp = self
            .ws_checkpoint
            .read()
            .map_err(|_| WeakSubjectivityError::LockPoisoned)?;

        match ws_cp.as_ref() {
            Some(checkpoint) => {
                if target_height < checkpoint.height {
                    if self.config.enforce {
                        error!(
                            "Reorg to height {} blocked: would cross WS checkpoint at {}",
                            target_height, checkpoint.height
                        );
                        return Err(WeakSubjectivityError::ChainDivergence(checkpoint.height));
                    } else {
                        warn!(
                            "Reorg to height {} would cross WS checkpoint at {} (not enforced)",
                            target_height, checkpoint.height
                        );
                    }
                }
                Ok(true)
            }
            None => Ok(true),
        }
    }

    /// Update WS checkpoint based on current chain state
    ///
    /// Called after processing new blocks to potentially create automatic checkpoints
    pub fn update_from_chain(
        &self,
        current_height: u64,
        get_block_header: impl Fn(u64) -> Option<([u8; 32], u64)>,
    ) -> WeakSubjectivityResult<()> {
        if !self.config.auto_checkpoint {
            return Ok(());
        }

        let ws_cp = self
            .ws_checkpoint
            .read()
            .map_err(|_| WeakSubjectivityError::LockPoisoned)?;

        let current_checkpoint_height = ws_cp.as_ref().map(|cp| cp.height).unwrap_or(0);
        drop(ws_cp);

        // Calculate candidate checkpoint height (current - confirmations)
        let candidate_height = current_height.saturating_sub(self.config.auto_checkpoint_confirmations);

        // Only update if significantly newer than current
        if candidate_height > current_checkpoint_height + self.config.period_blocks / 2 {
            if let Some((block_hash, timestamp)) = get_block_header(candidate_height) {
                let new_checkpoint = Checkpoint {
                    height: candidate_height,
                    block_hash,
                    source: CheckpointSource::Automatic,
                    created_at: timestamp,
                    name: None,
                };

                info!(
                    "Creating automatic WS checkpoint at height {} (current: {})",
                    candidate_height, current_height
                );

                // Also add to checkpoint manager
                if let Err(e) = self.checkpoint_manager.add_checkpoint(new_checkpoint.clone()) {
                    warn!("Failed to add checkpoint to manager: {}", e);
                }

                self.set_ws_checkpoint(new_checkpoint)?;
            }
        }

        Ok(())
    }

    /// Refresh the protection state based on checkpoint age
    fn refresh_state(&self) -> WeakSubjectivityResult<()> {
        let ws_cp = self
            .ws_checkpoint
            .read()
            .map_err(|_| WeakSubjectivityError::LockPoisoned)?;

        let new_state = match ws_cp.as_ref() {
            None => WeakSubjectivityState::Unprotected,
            Some(checkpoint) => {
                let now = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or(Duration::ZERO)
                    .as_secs();

                let age = now.saturating_sub(checkpoint.created_at);

                if age > MAX_WS_CHECKPOINT_AGE_SECONDS {
                    WeakSubjectivityState::Expired
                } else if age > MAX_WS_CHECKPOINT_AGE_SECONDS * 2 / 3 {
                    WeakSubjectivityState::NeedsRefresh
                } else {
                    WeakSubjectivityState::Protected
                }
            }
        };

        drop(ws_cp);
        self.update_state(new_state)
    }

    /// Update the protection state
    fn update_state(&self, new_state: WeakSubjectivityState) -> WeakSubjectivityResult<()> {
        let mut state = self
            .state
            .write()
            .map_err(|_| WeakSubjectivityError::LockPoisoned)?;

        if *state != new_state {
            info!("Weak subjectivity state changed: {:?} -> {:?}", *state, new_state);
            *state = new_state;
        }

        Ok(())
    }

    /// Get configuration
    pub fn config(&self) -> &WeakSubjectivityConfig {
        &self.config
    }

    /// Check if enforcement is enabled
    pub fn is_enforcing(&self) -> bool {
        self.config.enforce
    }

    /// Clear validated peers cache (e.g., on network reconnect)
    pub fn clear_peer_cache(&self) -> WeakSubjectivityResult<()> {
        let mut validated = self
            .validated_peers
            .write()
            .map_err(|_| WeakSubjectivityError::LockPoisoned)?;
        validated.clear();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::consensus::checkpoint::CheckpointConfig;

    fn create_test_checkpoint_manager() -> Arc<CheckpointManager> {
        Arc::new(CheckpointManager::new(CheckpointConfig::default()))
    }

    fn create_test_checkpoint(height: u64) -> Checkpoint {
        let mut hash = [0u8; 32];
        hash[0..8].copy_from_slice(&height.to_le_bytes());
        Checkpoint {
            height,
            block_hash: hash,
            source: CheckpointSource::Hardcoded,
            created_at: 1703318400, // Fixed timestamp
            name: Some(format!("Test checkpoint {}", height)),
        }
    }

    #[test]
    fn test_ws_manager_creation() {
        let checkpoint_manager = create_test_checkpoint_manager();
        let ws_manager = WeakSubjectivityManager::with_defaults(checkpoint_manager);

        let state = ws_manager.get_state().unwrap();
        assert_eq!(state, WeakSubjectivityState::Unprotected);
    }

    #[test]
    fn test_set_ws_checkpoint() {
        let checkpoint_manager = create_test_checkpoint_manager();
        let ws_manager = WeakSubjectivityManager::with_defaults(checkpoint_manager);

        let checkpoint = create_test_checkpoint(1000);
        ws_manager.set_ws_checkpoint(checkpoint.clone()).unwrap();

        let retrieved = ws_manager.get_ws_checkpoint().unwrap().unwrap();
        assert_eq!(retrieved.height, 1000);
    }

    #[test]
    fn test_cannot_downgrade_checkpoint() {
        let checkpoint_manager = create_test_checkpoint_manager();
        let ws_manager = WeakSubjectivityManager::with_defaults(checkpoint_manager);

        let high_checkpoint = create_test_checkpoint(2000);
        ws_manager.set_ws_checkpoint(high_checkpoint).unwrap();

        let low_checkpoint = create_test_checkpoint(1000);
        let result = ws_manager.set_ws_checkpoint(low_checkpoint);

        assert!(matches!(result, Err(WeakSubjectivityError::InvalidCheckpoint(_))));
    }

    #[test]
    fn test_untrusted_source_rejected() {
        let checkpoint_manager = create_test_checkpoint_manager();
        let mut config = WeakSubjectivityConfig::default();
        config.min_trust_level = 80; // Require high trust
        let ws_manager = WeakSubjectivityManager::new(config, checkpoint_manager);

        let mut checkpoint = create_test_checkpoint(1000);
        checkpoint.source = CheckpointSource::Automatic; // Trust level 40

        let result = ws_manager.set_ws_checkpoint(checkpoint);
        assert!(matches!(result, Err(WeakSubjectivityError::UntrustedSource(_))));
    }

    #[test]
    fn test_chain_validation_success() {
        let checkpoint_manager = create_test_checkpoint_manager();
        let ws_manager = WeakSubjectivityManager::with_defaults(checkpoint_manager);

        let checkpoint = create_test_checkpoint(1000);
        ws_manager.set_ws_checkpoint(checkpoint.clone()).unwrap();

        // Provide matching hash at checkpoint height
        let get_block_hash = |height: u64| -> Option<[u8; 32]> {
            if height == 1000 {
                Some(checkpoint.block_hash)
            } else {
                let mut hash = [0u8; 32];
                hash[0..8].copy_from_slice(&height.to_le_bytes());
                Some(hash)
            }
        };

        let result = ws_manager.validate_chain(1500, get_block_hash);
        assert!(result.is_ok());
        assert!(result.unwrap());
    }

    #[test]
    fn test_chain_validation_failure_divergence() {
        let checkpoint_manager = create_test_checkpoint_manager();
        let ws_manager = WeakSubjectivityManager::with_defaults(checkpoint_manager);

        let checkpoint = create_test_checkpoint(1000);
        ws_manager.set_ws_checkpoint(checkpoint).unwrap();

        // Provide different hash at checkpoint height (simulating alternate chain)
        let get_block_hash = |height: u64| -> Option<[u8; 32]> {
            let mut hash = [0xFFu8; 32]; // Different hash
            hash[0..8].copy_from_slice(&height.to_le_bytes());
            Some(hash)
        };

        let result = ws_manager.validate_chain(1500, get_block_hash);
        assert!(matches!(result, Err(WeakSubjectivityError::ChainDivergence(1000))));
    }

    #[test]
    fn test_peer_validation() {
        let checkpoint_manager = create_test_checkpoint_manager();
        let ws_manager = WeakSubjectivityManager::with_defaults(checkpoint_manager);

        let checkpoint = create_test_checkpoint(1000);
        ws_manager.set_ws_checkpoint(checkpoint.clone()).unwrap();

        // Peer on same chain
        let peer_info = PeerChainInfo {
            tip_hash: [1u8; 32],
            tip_height: 1500,
            ws_checkpoint_hash: Some(checkpoint.block_hash),
            total_work: 1000000,
        };

        let result = ws_manager.validate_peer_chain("peer1", &peer_info);
        assert!(result.is_ok());
        assert!(ws_manager.is_peer_validated("peer1").unwrap());
    }

    #[test]
    fn test_peer_validation_incompatible() {
        let checkpoint_manager = create_test_checkpoint_manager();
        let ws_manager = WeakSubjectivityManager::with_defaults(checkpoint_manager);

        let checkpoint = create_test_checkpoint(1000);
        ws_manager.set_ws_checkpoint(checkpoint).unwrap();

        // Peer on different chain
        let peer_info = PeerChainInfo {
            tip_hash: [1u8; 32],
            tip_height: 1500,
            ws_checkpoint_hash: Some([0xFFu8; 32]), // Different hash
            total_work: 1000000,
        };

        let result = ws_manager.validate_peer_chain("peer2", &peer_info);
        assert!(matches!(result, Err(WeakSubjectivityError::IncompatiblePeer(_, _))));
    }

    #[test]
    fn test_reorg_protection() {
        let checkpoint_manager = create_test_checkpoint_manager();
        let ws_manager = WeakSubjectivityManager::with_defaults(checkpoint_manager);

        let checkpoint = create_test_checkpoint(1000);
        ws_manager.set_ws_checkpoint(checkpoint).unwrap();

        // Reorg above checkpoint is allowed
        assert!(ws_manager.can_reorg_to_height(1001).is_ok());

        // Reorg below checkpoint is blocked
        let result = ws_manager.can_reorg_to_height(999);
        assert!(matches!(result, Err(WeakSubjectivityError::ChainDivergence(1000))));
    }

    #[test]
    fn test_within_ws_period() {
        let checkpoint_manager = create_test_checkpoint_manager();
        let ws_manager = WeakSubjectivityManager::with_defaults(checkpoint_manager);

        let checkpoint = create_test_checkpoint(1000);
        ws_manager.set_ws_checkpoint(checkpoint).unwrap();

        assert!(!ws_manager.is_within_ws_period(500).unwrap());
        assert!(ws_manager.is_within_ws_period(1000).unwrap());
        assert!(ws_manager.is_within_ws_period(1500).unwrap());
    }

    #[test]
    fn test_auto_checkpoint_creation() {
        let checkpoint_manager = create_test_checkpoint_manager();
        let mut config = WeakSubjectivityConfig::default();
        config.auto_checkpoint = true;
        config.auto_checkpoint_confirmations = 10;
        config.period_blocks = 100;
        config.min_trust_level = 40; // Allow automatic source
        let ws_manager = WeakSubjectivityManager::new(config, checkpoint_manager);

        // Initial checkpoint
        let checkpoint = create_test_checkpoint(100);
        ws_manager.set_ws_checkpoint(checkpoint).unwrap();

        // Simulate chain at height 300
        let get_header = |height: u64| -> Option<([u8; 32], u64)> {
            let mut hash = [0u8; 32];
            hash[0..8].copy_from_slice(&height.to_le_bytes());
            Some((hash, 1703318400))
        };

        ws_manager.update_from_chain(300, get_header).unwrap();

        // Should have created new checkpoint around height 290
        let current_cp = ws_manager.get_ws_checkpoint().unwrap().unwrap();
        assert!(current_cp.height > 100);
    }

    #[test]
    fn test_protection_state_transitions() {
        let checkpoint_manager = create_test_checkpoint_manager();
        let ws_manager = WeakSubjectivityManager::with_defaults(checkpoint_manager);

        // Initially unprotected
        assert_eq!(ws_manager.get_state().unwrap(), WeakSubjectivityState::Unprotected);

        // After setting checkpoint, should be protected
        let checkpoint = create_test_checkpoint(1000);
        ws_manager.set_ws_checkpoint(checkpoint).unwrap();

        // State depends on timestamp - since our test checkpoint has recent timestamp
        let state = ws_manager.get_state().unwrap();
        assert!(matches!(state, WeakSubjectivityState::Protected | WeakSubjectivityState::NeedsRefresh | WeakSubjectivityState::Expired));
    }

    #[test]
    fn test_clear_peer_cache() {
        let checkpoint_manager = create_test_checkpoint_manager();
        let ws_manager = WeakSubjectivityManager::with_defaults(checkpoint_manager);

        let checkpoint = create_test_checkpoint(1000);
        ws_manager.set_ws_checkpoint(checkpoint.clone()).unwrap();

        // Add a validated peer
        let peer_info = PeerChainInfo {
            tip_hash: [1u8; 32],
            tip_height: 1500,
            ws_checkpoint_hash: Some(checkpoint.block_hash),
            total_work: 1000000,
        };
        ws_manager.validate_peer_chain("peer1", &peer_info).unwrap();
        assert!(ws_manager.is_peer_validated("peer1").unwrap());

        // Clear cache
        ws_manager.clear_peer_cache().unwrap();
        assert!(!ws_manager.is_peer_validated("peer1").unwrap());
    }
}
