//! Block Checkpoints for Initial Block Download (IBD)
//!
//! PERFORMANCE MODULE (P1-004): Skip full validation for known-good blocks.
//!
//! Checkpoints are hardcoded (block_height, block_hash) pairs representing
//! blocks that have been validated by the network over time. During IBD,
//! when a node is catching up to the current chain tip, it can skip
//! expensive script validation for checkpointed blocks.
//!
//! SECURITY CONSIDERATIONS:
//! - Checkpoints are only used during IBD (1000+ blocks behind tip)
//! - Header chain validation is still performed
//! - Checkpoints do NOT prevent reorganizations past the checkpoint
//! - New checkpoints should be added conservatively (deep confirmations)
//!
//! Adding new checkpoints:
//! 1. Select blocks with >10,000 confirmations
//! 2. Verify block hash from multiple independent sources
//! 3. Add to CHECKPOINTS array in height order
//! 4. Update LAST_CHECKPOINT_HEIGHT constant

use once_cell::sync::Lazy;
use std::collections::HashMap;
use tracing::debug;

// ============================================================================
// Checkpoint Data
// ============================================================================

/// Minimum confirmations required before a block can become a checkpoint
pub const MIN_CHECKPOINT_CONFIRMATIONS: u64 = 10_000;

/// Number of blocks behind tip to consider IBD mode
pub const IBD_THRESHOLD_BLOCKS: u64 = 1_000;

/// Height of the last checkpoint (for quick IBD detection)
pub const LAST_CHECKPOINT_HEIGHT: u64 = 0; // Only genesis for now

/// Hardcoded checkpoints: (height, block_hash)
///
/// These are blocks that have been validated by the network and can be
/// assumed valid during Initial Block Download.
///
/// SECURITY: Only add checkpoints for blocks with >10,000 confirmations.
/// Verify hashes from multiple independent sources (block explorers, etc.)
pub static CHECKPOINTS: Lazy<HashMap<u64, [u8; 32]>> = Lazy::new(|| {
    let mut m = HashMap::new();

    // Genesis block - height 0
    // The genesis block hash should be set based on the actual genesis
    // For testnet/development, this is a placeholder
    m.insert(0, hex_to_bytes32(
        "0000000000000000000000000000000000000000000000000000000000000000"
    ));

    // Add more checkpoints as the network matures
    // Example (commented out until mainnet has sufficient history):
    //
    // // Block 100,000
    // m.insert(100_000, hex_to_bytes32(
    //     "00000000000000000..."
    // ));
    //
    // // Block 200,000
    // m.insert(200_000, hex_to_bytes32(
    //     "00000000000000000..."
    // ));

    m
});

/// Convert a hex string to a 32-byte array
///
/// Panics if the hex string is invalid or not 64 characters.
/// This is acceptable since checkpoints are hardcoded constants.
fn hex_to_bytes32(hex: &str) -> [u8; 32] {
    let bytes = hex::decode(hex).expect("Invalid checkpoint hex");
    let mut arr = [0u8; 32];
    arr.copy_from_slice(&bytes);
    arr
}

// ============================================================================
// Checkpoint Functions
// ============================================================================

/// Check if a block is a known checkpoint
///
/// # Arguments
/// * `height` - Block height
/// * `hash` - Block hash
///
/// # Returns
/// `true` if the block matches a known checkpoint
pub fn is_checkpoint(height: u64, hash: &[u8; 32]) -> bool {
    CHECKPOINTS.get(&height).map(|h| h == hash).unwrap_or(false)
}

/// Check if a height has a checkpoint
pub fn has_checkpoint_at_height(height: u64) -> bool {
    CHECKPOINTS.contains_key(&height)
}

/// Get the checkpoint hash at a given height (if exists)
pub fn get_checkpoint_hash(height: u64) -> Option<&'static [u8; 32]> {
    CHECKPOINTS.get(&height)
}

/// Check if we should skip full validation for a block
///
/// Validation can be skipped if:
/// 1. The block matches a known checkpoint
/// 2. We're in IBD mode (far behind the current tip)
///
/// # Arguments
/// * `height` - Block height being validated
/// * `hash` - Block hash being validated
/// * `current_tip_height` - Current chain tip height
///
/// # Returns
/// `true` if full script validation can be skipped
pub fn should_skip_validation(height: u64, hash: &[u8; 32], current_tip_height: u64) -> bool {
    // Only skip during IBD
    if !is_initial_block_download(height, current_tip_height) {
        return false;
    }

    // Only skip for checkpointed blocks
    if !is_checkpoint(height, hash) {
        return false;
    }

    debug!(
        "Skipping validation for checkpointed block at height {} during IBD",
        height
    );
    true
}

/// Check if we're in Initial Block Download mode
///
/// IBD mode is when we're significantly behind the network tip.
/// During IBD, we can use checkpoints to speed up synchronization.
pub fn is_initial_block_download(block_height: u64, tip_height: u64) -> bool {
    // We're in IBD if we're more than IBD_THRESHOLD_BLOCKS behind
    tip_height > block_height + IBD_THRESHOLD_BLOCKS
}

/// Verify that a block doesn't violate checkpoint rules
///
/// A block is invalid if:
/// - It claims to be at a checkpoint height but has a different hash
///
/// # Arguments
/// * `height` - Block height
/// * `hash` - Block hash
///
/// # Returns
/// * `Ok(())` - Block doesn't violate checkpoints
/// * `Err(reason)` - Block violates checkpoint rules
pub fn verify_against_checkpoints(height: u64, hash: &[u8; 32]) -> Result<(), String> {
    if let Some(checkpoint_hash) = CHECKPOINTS.get(&height) {
        if hash != checkpoint_hash {
            return Err(format!(
                "Block at height {} has hash {:02x?}... but checkpoint requires {:02x?}...",
                height,
                &hash[..4],
                &checkpoint_hash[..4]
            ));
        }
    }
    Ok(())
}

/// Get all checkpoint heights (sorted)
pub fn get_checkpoint_heights() -> Vec<u64> {
    let mut heights: Vec<_> = CHECKPOINTS.keys().cloned().collect();
    heights.sort();
    heights
}

/// Get the highest checkpoint height at or below the given height
pub fn get_highest_checkpoint_at_or_below(height: u64) -> Option<u64> {
    CHECKPOINTS
        .keys()
        .filter(|&h| *h <= height)
        .max()
        .cloned()
}

/// Count total number of checkpoints
pub fn checkpoint_count() -> usize {
    CHECKPOINTS.len()
}

// ============================================================================
// Checkpoint Manager
// ============================================================================

/// Manager for runtime checkpoint operations
///
/// While the main checkpoints are hardcoded, this manager can be used for:
/// - Tracking which checkpoints have been verified
/// - Providing checkpoint statistics
/// - Future: soft checkpoints for additional security
pub struct CheckpointManager {
    /// Track which checkpoints have been verified during this session
    verified: std::collections::HashSet<u64>,
    /// Current known tip height
    tip_height: u64,
}

impl CheckpointManager {
    /// Create a new checkpoint manager
    pub fn new() -> Self {
        Self {
            verified: std::collections::HashSet::new(),
            tip_height: 0,
        }
    }

    /// Update the known tip height
    pub fn update_tip(&mut self, height: u64) {
        self.tip_height = height;
    }

    /// Mark a checkpoint as verified
    pub fn mark_verified(&mut self, height: u64) {
        if CHECKPOINTS.contains_key(&height) {
            self.verified.insert(height);
        }
    }

    /// Check if we're in IBD mode
    pub fn is_ibd(&self, current_height: u64) -> bool {
        is_initial_block_download(current_height, self.tip_height)
    }

    /// Get verification progress
    pub fn verification_progress(&self) -> f64 {
        let total = CHECKPOINTS.len();
        if total == 0 {
            return 1.0;
        }
        self.verified.len() as f64 / total as f64
    }

    /// Get unverified checkpoints
    pub fn unverified_checkpoints(&self) -> Vec<u64> {
        CHECKPOINTS
            .keys()
            .filter(|h| !self.verified.contains(h))
            .cloned()
            .collect()
    }
}

impl Default for CheckpointManager {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_genesis_checkpoint() {
        // Genesis should be a checkpoint
        assert!(has_checkpoint_at_height(0));
    }

    #[test]
    fn test_checkpoint_lookup() {
        let heights = get_checkpoint_heights();
        assert!(!heights.is_empty());
        assert!(heights.contains(&0));
    }

    #[test]
    fn test_is_ibd() {
        // Far behind tip = IBD
        assert!(is_initial_block_download(100, 2000));

        // Close to tip = not IBD
        assert!(!is_initial_block_download(1900, 2000));

        // At tip = not IBD
        assert!(!is_initial_block_download(2000, 2000));
    }

    #[test]
    fn test_skip_validation() {
        // Get genesis checkpoint hash
        let genesis_hash = get_checkpoint_hash(0).expect("Genesis checkpoint should exist");

        // Should skip during IBD with matching checkpoint
        assert!(should_skip_validation(0, genesis_hash, 10000));

        // Should NOT skip when not in IBD
        assert!(!should_skip_validation(0, genesis_hash, 100));

        // Should NOT skip with wrong hash
        let wrong_hash = [1u8; 32];
        assert!(!should_skip_validation(0, &wrong_hash, 10000));
    }

    #[test]
    fn test_verify_against_checkpoints() {
        let genesis_hash = get_checkpoint_hash(0).expect("Genesis checkpoint should exist");

        // Correct hash should pass
        assert!(verify_against_checkpoints(0, genesis_hash).is_ok());

        // Wrong hash should fail
        let wrong_hash = [1u8; 32];
        assert!(verify_against_checkpoints(0, &wrong_hash).is_err());

        // Non-checkpoint height should pass with any hash
        assert!(verify_against_checkpoints(12345, &wrong_hash).is_ok());
    }

    #[test]
    fn test_checkpoint_manager() {
        let mut manager = CheckpointManager::new();

        // Update tip
        manager.update_tip(10000);

        // Should be in IBD when at height 0
        assert!(manager.is_ibd(0));

        // Should not be in IBD near tip
        assert!(!manager.is_ibd(9500));

        // Mark checkpoint verified
        manager.mark_verified(0);
        assert!(manager.verified.contains(&0));
    }

    #[test]
    fn test_highest_checkpoint_at_or_below() {
        // Genesis exists
        assert_eq!(get_highest_checkpoint_at_or_below(100), Some(0));
        
        // At genesis
        assert_eq!(get_highest_checkpoint_at_or_below(0), Some(0));
    }
}

