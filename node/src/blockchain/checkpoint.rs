//! Blockchain Checkpoints for Sync Optimization and Security
//!
//! This module provides hardcoded checkpoints for known good blocks.
//! Checkpoints accelerate initial sync and prevent deep chain reorganizations
//! below checkpoint heights.

use supernova_core::types::Block;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::RwLock;

/// Checkpoint enforcement level
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckpointEnforcement {
    /// Strict enforcement - reject any chain that doesn't match checkpoints
    Strict,
    /// Warn only - log mismatches but allow chain progression
    Warn,
    /// Disabled - no checkpoint enforcement
    Disabled,
}

/// Represents a checkpoint - a known good block that all nodes must agree on
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Checkpoint {
    /// Block height
    pub height: u64,
    /// Block hash
    pub hash: [u8; 32],
    /// Block timestamp
    pub timestamp: u64,
    /// Accumulated work at this checkpoint
    pub total_work: u128,
}

impl Checkpoint {
    /// Create a new checkpoint
    pub fn new(height: u64, hash: [u8; 32], timestamp: u64, total_work: u128) -> Self {
        Self {
            height,
            hash,
            timestamp,
            total_work,
        }
    }

    /// Validate a block against this checkpoint
    pub fn validate_block(&self, block: &Block) -> Result<(), CheckpointError> {
        if block.height() != self.height {
            return Err(CheckpointError::HeightMismatch {
                expected: self.height,
                actual: block.height(),
            });
        }

        let block_hash = block.hash();
        if block_hash != self.hash {
            return Err(CheckpointError::HashMismatch {
                expected: hex::encode(&self.hash),
                actual: hex::encode(&block_hash),
                height: self.height,
            });
        }

        Ok(())
    }
}

/// Checkpoint errors
#[derive(Debug, thiserror::Error)]
pub enum CheckpointError {
    #[error("Checkpoint height mismatch: expected {expected}, got {actual}")]
    HeightMismatch { expected: u64, actual: u64 },

    #[error("Checkpoint hash mismatch at height {height}: expected {expected}, got {actual}")]
    HashMismatch {
        expected: String,
        actual: String,
        height: u64,
    },

    #[error("Block below checkpoint height {checkpoint_height} cannot be reorganized")]
    ReorganizationBelowCheckpoint { checkpoint_height: u64 },

    #[error("Checkpoint not found for height {height}")]
    CheckpointNotFound { height: u64 },
}

/// Checkpoint manager for blockchain synchronization
pub struct CheckpointManager {
    /// Checkpoints indexed by height
    checkpoints: HashMap<u64, Checkpoint>,
    /// Enforcement level
    enforcement: CheckpointEnforcement,
    /// Highest checkpoint height
    max_checkpoint_height: u64,
}

impl CheckpointManager {
    /// Create a new checkpoint manager with default checkpoints
    pub fn new() -> Self {
        let mut checkpoints = HashMap::new();

        // Add genesis block as checkpoint (height 0)
        // Genesis hash from genesis.rs
        checkpoints.insert(
            0,
            Checkpoint::new(
                0,
                crate::blockchain::genesis::TESTNET_GENESIS_HASH,
                crate::blockchain::genesis::TESTNET_GENESIS_TIMESTAMP,
                0, // Genesis has no work
            ),
        );

        // Add additional testnet checkpoints as they become available
        // These would be populated from actual testnet blocks after deployment
        // Example format:
        // checkpoints.insert(
        //     1000,
        //     Checkpoint::new(
        //         1000,
        //         [0x...; 32], // Hash of block at height 1000
        //         1730044800 + (1000 * 600), // Estimated timestamp
        //         1000, // Accumulated work
        //     ),
        // );

        let max_checkpoint_height = checkpoints.keys().max().copied().unwrap_or(0);

        Self {
            checkpoints,
            enforcement: CheckpointEnforcement::Strict,
            max_checkpoint_height,
        }
    }

    /// Set the enforcement level
    pub fn set_enforcement(&mut self, level: CheckpointEnforcement) {
        self.enforcement = level;
    }

    /// Get the highest checkpoint height
    pub fn max_checkpoint_height(&self) -> u64 {
        self.max_checkpoint_height
    }

    /// Get the nearest checkpoint at or below the given height
    pub fn get_nearest_checkpoint(&self, height: u64) -> Option<&Checkpoint> {
        // Find the highest checkpoint at or below this height
        self.checkpoints
            .iter()
            .filter(|(checkpoint_height, _)| **checkpoint_height <= height)
            .max_by_key(|(checkpoint_height, _)| **checkpoint_height)
            .map(|(_, checkpoint)| checkpoint)
    }

    /// Validate a block against checkpoints
    pub fn validate_block(&self, block: &Block) -> Result<(), CheckpointError> {
        let block_height = block.height();

        // Check if this height has a checkpoint
        if let Some(checkpoint) = self.checkpoints.get(&block_height) {
            match checkpoint.validate_block(block) {
                Ok(()) => {
                    tracing::debug!("Block at height {} matches checkpoint", block_height);
                    Ok(())
                }
                Err(e) => {
                    match self.enforcement {
                        CheckpointEnforcement::Strict => {
                            tracing::error!(
                                "Checkpoint validation failed: {}",
                                e
                            );
                            Err(e)
                        }
                        CheckpointEnforcement::Warn => {
                            tracing::warn!("Checkpoint mismatch (warn only): {}", e);
                            Ok(())
                        }
                        CheckpointEnforcement::Disabled => Ok(()),
                    }
                }
            }
        } else {
            // No checkpoint at this height - validation passes
            Ok(())
        }
    }

    /// Check if a reorganization is allowed below checkpoint height
    pub fn can_reorganize_below(&self, fork_height: u64) -> Result<(), CheckpointError> {
        if self.enforcement == CheckpointEnforcement::Disabled {
            return Ok(());
        }

        // Find the highest checkpoint below the fork point
        if let Some(checkpoint) = self.get_nearest_checkpoint(fork_height) {
            if fork_height <= checkpoint.height {
                match self.enforcement {
                    CheckpointEnforcement::Strict => {
                        return Err(CheckpointError::ReorganizationBelowCheckpoint {
                            checkpoint_height: checkpoint.height,
                        });
                    }
                    CheckpointEnforcement::Warn => {
                        tracing::warn!(
                            "Reorganization below checkpoint at height {} (warn only)",
                            checkpoint.height
                        );
                    }
                    CheckpointEnforcement::Disabled => {}
                }
            }
        }

        Ok(())
    }

    /// Get checkpoint at a specific height
    pub fn get_checkpoint(&self, height: u64) -> Option<&Checkpoint> {
        self.checkpoints.get(&height)
    }

    /// Add a new checkpoint (for testing or future checkpoint updates)
    pub fn add_checkpoint(&mut self, checkpoint: Checkpoint) {
        self.checkpoints.insert(checkpoint.height, checkpoint);
        self.max_checkpoint_height = self.checkpoints.keys().max().copied().unwrap_or(0);
    }

    /// Get all checkpoints
    pub fn get_all_checkpoints(&self) -> Vec<&Checkpoint> {
        let mut checkpoints: Vec<&Checkpoint> = self.checkpoints.values().collect();
        checkpoints.sort_by_key(|c| c.height);
        checkpoints
    }
}

impl Default for CheckpointManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Global checkpoint manager instance
lazy_static::lazy_static! {
    static ref CHECKPOINT_MANAGER: Arc<RwLock<CheckpointManager>> = Arc::new(RwLock::new(CheckpointManager::new()));
}

/// Get the global checkpoint manager
pub fn get_checkpoint_manager() -> Arc<RwLock<CheckpointManager>> {
    Arc::clone(&CHECKPOINT_MANAGER)
}

/// Validate a block against checkpoints
pub fn validate_checkpoint(block: &Block) -> Result<(), CheckpointError> {
    let manager = get_checkpoint_manager();
    let manager = manager.read().unwrap();
    manager.validate_block(block)
}

/// Check if reorganization is allowed below checkpoint height
pub fn can_reorganize_below(fork_height: u64) -> Result<(), CheckpointError> {
    let manager = get_checkpoint_manager();
    let manager = manager.read().unwrap();
    manager.can_reorganize_below(fork_height)
}

#[cfg(test)]
mod tests {
    use super::*;
    use supernova_core::types::{Block, BlockHeader, Transaction};

    fn create_test_block(height: u64, prev_hash: [u8; 32]) -> Block {
        let header = BlockHeader::new(
            1,
            prev_hash,
            [0u8; 32],
            1000 + height * 600,
            0x1d00ffff,
            0,
        );
        let tx = Transaction::new(2, vec![], vec![], 0);
        Block::new(header, vec![tx])
    }

    #[test]
    fn test_checkpoint_validation() {
        let manager = CheckpointManager::new();

        // Get genesis checkpoint
        let genesis_checkpoint = manager.get_checkpoint(0).unwrap();

        // Create genesis block matching checkpoint
        let genesis = crate::blockchain::genesis::create_testnet_genesis_block().unwrap();
        assert!(genesis_checkpoint.validate_block(&genesis).is_ok());

        // Create invalid genesis block
        let invalid_genesis = create_test_block(0, [0u8; 32]);
        assert!(genesis_checkpoint.validate_block(&invalid_genesis).is_err());
    }

    #[test]
    fn test_checkpoint_prevents_deep_reorg() {
        let mut manager = CheckpointManager::new();
        manager.set_enforcement(CheckpointEnforcement::Strict);

        // Add checkpoint at height 100
        let checkpoint = Checkpoint::new(
            100,
            [0x42; 32],
            1000 + 100 * 600,
            100,
        );
        manager.add_checkpoint(checkpoint);

        // Try to reorganize below checkpoint - should fail
        assert!(manager.can_reorganize_below(50).is_err());
        assert!(manager.can_reorganize_below(100).is_err());

        // Reorganize above checkpoint - should succeed
        assert!(manager.can_reorganize_below(101).is_ok());
    }

    #[test]
    fn test_checkpoint_sync_optimization() {
        let manager = CheckpointManager::new();

        // Should find genesis checkpoint
        let nearest = manager.get_nearest_checkpoint(1000);
        assert!(nearest.is_some());
        assert_eq!(nearest.unwrap().height, 0);

        // Add checkpoint at height 500
        let mut manager = CheckpointManager::new();
        let checkpoint = Checkpoint::new(
            500,
            [0x42; 32],
            1000 + 500 * 600,
            500,
        );
        manager.add_checkpoint(checkpoint);

        // Should find checkpoint at height 500
        let nearest = manager.get_nearest_checkpoint(1000);
        assert!(nearest.is_some());
        assert_eq!(nearest.unwrap().height, 500);

        // Should find checkpoint at height 500 even for height 501
        let nearest = manager.get_nearest_checkpoint(501);
        assert!(nearest.is_some());
        assert_eq!(nearest.unwrap().height, 500);
    }

    #[test]
    fn test_invalid_checkpoint_rejection() {
        let mut manager = CheckpointManager::new();
        manager.set_enforcement(CheckpointEnforcement::Strict);

        // Add checkpoint
        let checkpoint = Checkpoint::new(
            100,
            [0x42; 32],
            1000,
            100,
        );
        manager.add_checkpoint(checkpoint);

        // Create block with wrong hash
        let invalid_block = create_test_block(100, [0u8; 32]);
        assert!(manager.validate_block(&invalid_block).is_err());

        // Create block with wrong height
        let wrong_height_block = create_test_block(101, [0x42; 32]);
        assert!(manager.validate_block(&wrong_height_block).is_ok()); // No checkpoint at 101
    }

    #[test]
    fn test_checkpoint_enforcement_levels() {
        let mut manager = CheckpointManager::new();

        // Add checkpoint
        let checkpoint = Checkpoint::new(
            100,
            [0x42; 32],
            1000,
            100,
        );
        manager.add_checkpoint(checkpoint);

        // Test Strict enforcement
        manager.set_enforcement(CheckpointEnforcement::Strict);
        let invalid_block = create_test_block(100, [0u8; 32]);
        assert!(manager.validate_block(&invalid_block).is_err());

        // Test Warn enforcement
        manager.set_enforcement(CheckpointEnforcement::Warn);
        assert!(manager.validate_block(&invalid_block).is_ok()); // Warns but allows

        // Test Disabled enforcement
        manager.set_enforcement(CheckpointEnforcement::Disabled);
        assert!(manager.validate_block(&invalid_block).is_ok()); // No validation
        assert!(manager.can_reorganize_below(50).is_ok()); // Reorg allowed
    }

    #[test]
    fn test_multiple_checkpoints() {
        let mut manager = CheckpointManager::new();

        // Add multiple checkpoints
        manager.add_checkpoint(Checkpoint::new(100, [0x11; 32], 1000, 100));
        manager.add_checkpoint(Checkpoint::new(500, [0x22; 32], 5000, 500));
        manager.add_checkpoint(Checkpoint::new(1000, [0x33; 32], 10000, 1000));

        // Verify all checkpoints exist
        assert!(manager.get_checkpoint(100).is_some());
        assert!(manager.get_checkpoint(500).is_some());
        assert!(manager.get_checkpoint(1000).is_some());

        // Verify nearest checkpoint lookup
        assert_eq!(manager.get_nearest_checkpoint(250).unwrap().height, 100);
        assert_eq!(manager.get_nearest_checkpoint(750).unwrap().height, 500);
        assert_eq!(manager.get_nearest_checkpoint(1500).unwrap().height, 1000);
    }
}

