//! Block Invalidation System
//!
//! Tracks invalid blocks and prevents re-processing, automatically marks descendants
//! as invalid, and cleans up orphaned blocks in the chain.

use supernova_core::types::block::Block;
use supernova_core::validation::block::BlockValidationError;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, RwLock};
use thiserror::Error;

/// Error types for block invalidation
#[derive(Debug, Error)]
pub enum InvalidationError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Serialization error: {0}")]
    Serialization(String),
    #[error("Block not found: {0}")]
    BlockNotFound(String),
}

/// Reason for block invalidation
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum InvalidationReason {
    /// Consensus rule violation
    ConsensusViolation(String),
    /// Invalid proof-of-work
    InvalidProofOfWork,
    /// Invalid signature
    InvalidSignature,
    /// Invalid Merkle root
    InvalidMerkleRoot,
    /// Invalid difficulty
    InvalidDifficulty(String),
    /// Timestamp violation
    TimestampViolation(String),
    /// Transaction validation failure
    TransactionValidation(String),
    /// Block structure invalid
    InvalidStructure(String),
    /// Checkpoint violation
    CheckpointViolation,
    /// Fork too deep
    ForkTooDeep,
    /// Parent block invalid
    ParentInvalid,
    /// Unknown reason
    Unknown(String),
}

impl From<&BlockValidationError> for InvalidationReason {
    fn from(err: &BlockValidationError) -> Self {
        match err {
            BlockValidationError::InvalidPoW => InvalidationReason::InvalidProofOfWork,
            BlockValidationError::InvalidMerkleRoot => InvalidationReason::InvalidMerkleRoot,
            BlockValidationError::InvalidDifficulty(msg) => {
                InvalidationReason::InvalidDifficulty(msg.clone())
            }
            BlockValidationError::TimestampTooFar(_, _) | BlockValidationError::TimestampTooEarly(_, _) => {
                InvalidationReason::TimestampViolation(err.to_string())
            }
            BlockValidationError::InvalidTransaction(msg) => {
                InvalidationReason::TransactionValidation(msg.clone())
            }
            BlockValidationError::BlockTooLarge(_, _) | BlockValidationError::WeightTooHigh(_, _) => {
                InvalidationReason::InvalidStructure(err.to_string())
            }
            BlockValidationError::PrevBlockNotFound(_) => InvalidationReason::ParentInvalid,
            _ => InvalidationReason::Unknown(err.to_string()),
        }
    }
}

/// Invalid block record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InvalidBlock {
    /// Block hash
    pub block_hash: [u8; 32],
    /// Reason for invalidation
    pub reason: InvalidationReason,
    /// Timestamp of invalidation
    pub invalidated_at: DateTime<Utc>,
    /// Number of validation attempts
    pub attempt_count: u32,
    /// Whether invalidation is permanent
    pub permanent: bool,
    /// Parent block hash (if known)
    pub parent_hash: Option<[u8; 32]>,
    /// Block height (if known)
    pub height: Option<u64>,
}

/// Configuration for invalid block tracker
#[derive(Debug, Clone)]
pub struct InvalidBlockTrackerConfig {
    /// Maximum number of invalid blocks to track
    pub max_tracked_blocks: usize,
    /// Number of attempts before permanent invalidation
    pub max_attempts: u32,
    /// Enable automatic descendant marking
    pub mark_descendants: bool,
    /// Enable orphan cleanup
    pub cleanup_orphans: bool,
    /// Timeout for temporary invalidations (seconds)
    pub temporary_timeout_seconds: u64,
}

impl Default for InvalidBlockTrackerConfig {
    fn default() -> Self {
        Self {
            max_tracked_blocks: 100_000,
            max_attempts: 3,
            mark_descendants: true,
            cleanup_orphans: true,
            temporary_timeout_seconds: 3600, // 1 hour
        }
    }
}

/// Invalid block tracker
pub struct InvalidBlockTracker {
    /// Configuration
    config: InvalidBlockTrackerConfig,
    /// Map from block hash to invalid block record
    invalid_blocks: Arc<RwLock<HashMap<[u8; 32], InvalidBlock>>>,
    /// Set of permanently invalid blocks (for fast lookup)
    permanent_invalid: Arc<RwLock<HashSet<[u8; 32]>>>,
    /// Map from parent hash to child hashes (for descendant tracking)
    parent_to_children: Arc<RwLock<HashMap<[u8; 32], Vec<[u8; 32]>>>>,
}

impl InvalidBlockTracker {
    /// Create a new invalid block tracker
    pub fn new(config: InvalidBlockTrackerConfig) -> Self {
        Self {
            config,
            invalid_blocks: Arc::new(RwLock::new(HashMap::new())),
            permanent_invalid: Arc::new(RwLock::new(HashSet::new())),
            parent_to_children: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Mark a block as invalid
    pub fn mark_invalid(
        &self,
        block_hash: [u8; 32],
        reason: InvalidationReason,
        parent_hash: Option<[u8; 32]>,
        height: Option<u64>,
    ) -> Result<(), InvalidationError> {
        let mut invalid_blocks = self.invalid_blocks.write().unwrap();
        let mut permanent_invalid = self.permanent_invalid.write().unwrap();

        // Check if already tracked
        let existing = invalid_blocks.get(&block_hash);
        let attempt_count = existing.map(|b| b.attempt_count + 1).unwrap_or(1);
        let permanent = attempt_count >= self.config.max_attempts;

        let invalid_block = InvalidBlock {
            block_hash,
            reason: reason.clone(),
            invalidated_at: Utc::now(),
            attempt_count,
            permanent,
            parent_hash,
            height,
        };

        invalid_blocks.insert(block_hash, invalid_block.clone());

        if permanent {
            permanent_invalid.insert(block_hash);
        }

        // Track parent-child relationship
        if let Some(parent) = parent_hash {
            let mut parent_map = self.parent_to_children.write().unwrap();
            parent_map
                .entry(parent)
                .or_insert_with(Vec::new)
                .push(block_hash);
        }

        // Mark descendants as invalid if enabled
        if self.config.mark_descendants {
            self.mark_descendants_invalid(block_hash, &reason)?;
        }

        // Cleanup if needed
        if invalid_blocks.len() > self.config.max_tracked_blocks {
            self.cleanup_old_entries()?;
        }

        Ok(())
    }

    /// Check if a block is invalid
    pub fn is_invalid(&self, block_hash: &[u8; 32]) -> bool {
        let invalid_blocks = self.invalid_blocks.read().unwrap();
        invalid_blocks.contains_key(block_hash)
    }

    /// Check if a block is permanently invalid
    pub fn is_permanently_invalid(&self, block_hash: &[u8; 32]) -> bool {
        let permanent_invalid = self.permanent_invalid.read().unwrap();
        permanent_invalid.contains(block_hash)
    }

    /// Get invalid block record
    pub fn get_invalid_block(&self, block_hash: &[u8; 32]) -> Option<InvalidBlock> {
        let invalid_blocks = self.invalid_blocks.read().unwrap();
        invalid_blocks.get(block_hash).cloned()
    }

    /// Mark all descendants of an invalid block as invalid
    fn mark_descendants_invalid(
        &self,
        parent_hash: [u8; 32],
        reason: &InvalidationReason,
    ) -> Result<(), InvalidationError> {
        let parent_to_children = self.parent_to_children.read().unwrap();
        
        if let Some(children) = parent_to_children.get(&parent_hash) {
            let mut invalid_blocks = self.invalid_blocks.write().unwrap();
            let mut permanent_invalid = self.permanent_invalid.write().unwrap();

            for &child_hash in children {
                // Only mark if not already marked
                if !invalid_blocks.contains_key(&child_hash) {
                    let invalid_block = InvalidBlock {
                        block_hash: child_hash,
                        reason: InvalidationReason::ParentInvalid,
                        invalidated_at: Utc::now(),
                        attempt_count: 0,
                        permanent: true, // Descendants of invalid blocks are permanently invalid
                        parent_hash: Some(parent_hash),
                        height: None,
                    };

                    invalid_blocks.insert(child_hash, invalid_block.clone());
                    permanent_invalid.insert(child_hash);
                }
            }
        }

        Ok(())
    }

    /// Clean up old invalid block entries
    fn cleanup_old_entries(&self) -> Result<(), InvalidationError> {
        let mut invalid_blocks = self.invalid_blocks.write().unwrap();
        let mut permanent_invalid = self.permanent_invalid.write().unwrap();
        let now = Utc::now();

        // Remove temporary invalidations older than timeout
        let timeout = chrono::Duration::seconds(self.config.temporary_timeout_seconds as i64);
        let mut to_remove = Vec::new();

        for (hash, block) in invalid_blocks.iter() {
            if !block.permanent {
                let age = now.signed_duration_since(block.invalidated_at);
                if age > timeout {
                    to_remove.push(*hash);
                }
            }
        }

        for hash in &to_remove {
            invalid_blocks.remove(hash);
        }

        // If still over limit, remove oldest permanent entries
        if invalid_blocks.len() > self.config.max_tracked_blocks {
            let mut entries: Vec<([u8; 32], DateTime<Utc>)> = invalid_blocks
                .iter()
                .map(|(hash, block)| (*hash, block.invalidated_at))
                .collect();
            
            entries.sort_by_key(|(_, time)| *time);
            
            let to_remove_count = invalid_blocks.len() - self.config.max_tracked_blocks;
            for (hash, _) in entries.iter().take(to_remove_count) {
                invalid_blocks.remove(hash);
                permanent_invalid.remove(hash);
            }
        }

        Ok(())
    }

    /// Clean up orphaned blocks (blocks whose parents are invalid)
    pub fn cleanup_orphans(&self, chain_blocks: &HashSet<[u8; 32]>) -> Result<Vec<[u8; 32]>, InvalidationError> {
        if !self.config.cleanup_orphans {
            return Ok(Vec::new());
        }

        let invalid_blocks = self.invalid_blocks.read().unwrap();
        let mut orphaned = Vec::new();

        // Find blocks in chain whose parents are invalid
        for block_hash in chain_blocks {
            if let Some(invalid_block) = invalid_blocks.get(block_hash) {
                if let Some(parent) = invalid_block.parent_hash {
                    if invalid_blocks.contains_key(&parent) {
                        orphaned.push(*block_hash);
                    }
                }
            }
        }

        Ok(orphaned)
    }

    /// Remove a block from invalid tracking (if it becomes valid)
    pub fn remove_invalid(&self, block_hash: &[u8; 32]) -> Result<(), InvalidationError> {
        let mut invalid_blocks = self.invalid_blocks.write().unwrap();
        let mut permanent_invalid = self.permanent_invalid.write().unwrap();

        invalid_blocks.remove(block_hash);
        permanent_invalid.remove(block_hash);

        Ok(())
    }

    /// Get statistics about invalid blocks
    pub fn get_statistics(&self) -> InvalidationStatistics {
        let invalid_blocks = self.invalid_blocks.read().unwrap();
        let permanent_invalid = self.permanent_invalid.read().unwrap();

        let mut reason_counts: HashMap<String, u32> = HashMap::new();
        for block in invalid_blocks.values() {
            let reason_str = format!("{:?}", block.reason);
            *reason_counts.entry(reason_str).or_insert(0) += 1;
        }

        InvalidationStatistics {
            total_invalid_blocks: invalid_blocks.len(),
            permanent_invalid_count: permanent_invalid.len(),
            temporary_invalid_count: invalid_blocks.len() - permanent_invalid.len(),
            reason_counts,
        }
    }
}

/// Statistics about invalid blocks
#[derive(Debug, Clone)]
pub struct InvalidationStatistics {
    pub total_invalid_blocks: usize,
    pub permanent_invalid_count: usize,
    pub temporary_invalid_count: usize,
    pub reason_counts: HashMap<String, u32>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_block() -> Block {
        use supernova_core::types::block::BlockHeader;
        use supernova_core::types::transaction::{Transaction, TransactionInput, TransactionOutput};
        
        let header = BlockHeader::new(
            1,
            [0u8; 32],
            [0u8; 32],
            1000,
            0x1d00ffff,
            0,
        );
        let coinbase = Transaction::new(
            1,
            vec![TransactionInput::new_coinbase(vec![])],
            vec![TransactionOutput::new(50_000_000_00, vec![])],
            0,
        );
        Block::new(header, vec![coinbase])
    }

    #[test]
    fn test_block_invalidation_basic() {
        let tracker = InvalidBlockTracker::new(InvalidBlockTrackerConfig::default());
        let block_hash = [1u8; 32];

        tracker
            .mark_invalid(
                block_hash,
                InvalidationReason::InvalidProofOfWork,
                None,
                Some(100),
            )
            .unwrap();

        assert!(tracker.is_invalid(&block_hash));
        assert!(!tracker.is_permanently_invalid(&block_hash)); // First attempt
    }

    #[test]
    fn test_descendant_invalidation() {
        let tracker = InvalidBlockTracker::new(InvalidBlockTrackerConfig::default());
        let parent_hash = [1u8; 32];
        let child_hash = [2u8; 32];

        // Track parent-child relationship
        {
            let mut parent_map = tracker.parent_to_children.write().unwrap();
            parent_map
                .entry(parent_hash)
                .or_insert_with(Vec::new)
                .push(child_hash);
        }

        // Mark parent as invalid
        tracker
            .mark_invalid(
                parent_hash,
                InvalidationReason::InvalidProofOfWork,
                None,
                Some(100),
            )
            .unwrap();

        // Child should be marked as invalid
        assert!(tracker.is_invalid(&child_hash));
        assert!(tracker.is_permanently_invalid(&child_hash)); // Descendants are permanent
    }

    #[test]
    fn test_orphan_cleanup() {
        let tracker = InvalidBlockTracker::new(InvalidBlockTrackerConfig::default());
        let parent_hash = [1u8; 32];
        let child_hash = [2u8; 32];

        // Mark parent as invalid
        tracker
            .mark_invalid(
                parent_hash,
                InvalidationReason::InvalidProofOfWork,
                None,
                Some(100),
            )
            .unwrap();

        // Mark child with parent reference
        tracker
            .mark_invalid(
                child_hash,
                InvalidationReason::ParentInvalid,
                Some(parent_hash),
                Some(101),
            )
            .unwrap();

        let chain_blocks: HashSet<[u8; 32]> = [child_hash].iter().copied().collect();
        let orphans = tracker.cleanup_orphans(&chain_blocks).unwrap();
        assert!(orphans.contains(&child_hash));
    }

    #[test]
    fn test_invalidation_persistence() {
        let tracker = InvalidBlockTracker::new(InvalidBlockTrackerConfig::default());
        let block_hash = [1u8; 32];

        // Mark as invalid multiple times
        for _ in 0..3 {
            tracker
                .mark_invalid(
                    block_hash,
                    InvalidationReason::InvalidProofOfWork,
                    None,
                    Some(100),
                )
                .unwrap();
        }

        assert!(tracker.is_permanently_invalid(&block_hash));
        let invalid_block = tracker.get_invalid_block(&block_hash).unwrap();
        assert_eq!(invalid_block.attempt_count, 3);
    }

    #[test]
    fn test_temporary_vs_permanent_invalidation() {
        let mut config = InvalidBlockTrackerConfig::default();
        config.max_attempts = 2;
        let tracker = InvalidBlockTracker::new(config);
        let block_hash = [1u8; 32];

        // First attempt - temporary
        tracker
            .mark_invalid(
                block_hash,
                InvalidationReason::InvalidProofOfWork,
                None,
                Some(100),
            )
            .unwrap();
        assert!(!tracker.is_permanently_invalid(&block_hash));

        // Second attempt - permanent
        tracker
            .mark_invalid(
                block_hash,
                InvalidationReason::InvalidProofOfWork,
                None,
                Some(100),
            )
            .unwrap();
        assert!(tracker.is_permanently_invalid(&block_hash));
    }

    #[test]
    fn test_peer_notification() {
        let tracker = InvalidBlockTracker::new(InvalidBlockTrackerConfig::default());
        let block_hash = [1u8; 32];

        tracker
            .mark_invalid(
                block_hash,
                InvalidationReason::InvalidProofOfWork,
                None,
                Some(100),
            )
            .unwrap();

        // In a real implementation, this would trigger peer notification
        // For now, we just verify the block is tracked
        assert!(tracker.is_invalid(&block_hash));
        let stats = tracker.get_statistics();
        assert_eq!(stats.total_invalid_blocks, 1);
    }
}

