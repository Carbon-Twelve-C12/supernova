//! Block Validation Module
//!
//! PERFORMANCE MODULE (P1-004): High-performance block and transaction validation.
//!
//! This module provides optimized validation infrastructure including:
//! - Parallel script validation using rayon
//! - Signature verification caching
//! - Validation timeouts for DoS prevention
//! - Checkpoint skipping during Initial Block Download
//!
//! ## Performance Optimizations
//!
//! 1. **Parallel Validation**: Transaction scripts are validated in parallel
//!    using rayon's work-stealing scheduler. This provides near-linear
//!    speedup on multi-core systems.
//!
//! 2. **Signature Cache**: Signature verification results are cached to avoid
//!    redundant cryptographic operations. Signatures are verified when:
//!    - Transaction enters mempool
//!    - Transaction is included in a received block
//!    - Block is received from multiple peers
//!
//! 3. **Checkpoint Skipping**: During IBD, blocks matching hardcoded
//!    checkpoints skip full script validation. Header chain validation
//!    is still performed.
//!
//! 4. **Validation Timeouts**: Each block and transaction has a maximum
//!    validation time to prevent DoS attacks via slow-to-validate blocks.
//!
//! ## Usage
//!
//! ```ignore
//! use node::validation::{ParallelBlockValidator, ParallelValidatorConfig};
//!
//! let config = ParallelValidatorConfig::default();
//! let validator = ParallelBlockValidator::new(config);
//!
//! // Validate a block
//! let result = validator.validate_block_scripts(&block, |txid, vout| {
//!     utxo_set.get_output(txid, vout)
//! });
//! ```
//!
//! ## Security Considerations
//!
//! - Signature cache must be cleared during chain reorganizations
//! - Checkpoints only apply during IBD (1000+ blocks behind tip)
//! - Validation timeouts prevent CPU exhaustion attacks
//! - Parallel validation maintains same security as sequential

pub mod checkpoints;
pub mod parallel_validator;
pub mod sig_cache;

// Re-export main types
pub use checkpoints::{
    is_checkpoint, should_skip_validation, verify_against_checkpoints, CheckpointManager,
    IBD_THRESHOLD_BLOCKS, LAST_CHECKPOINT_HEIGHT,
};

pub use parallel_validator::{
    BlockValidationResult, ParallelBlockValidator, ParallelValidationError,
    ParallelValidatorConfig, TxValidationResult, MAX_BLOCK_VALIDATION_TIME,
    MAX_TX_VALIDATION_TIME,
};

pub use sig_cache::{
    SignatureCache, SignatureCacheConfig, SignatureCacheKey, SignatureCacheStats,
    SignatureType, DEFAULT_CACHE_CAPACITY,
};

// ============================================================================
// Validation Statistics
// ============================================================================

/// Combined statistics for validation operations
#[derive(Debug, Clone, Default)]
pub struct ValidationStats {
    /// Total blocks validated
    pub blocks_validated: u64,
    /// Total transactions validated
    pub txs_validated: u64,
    /// Total signatures verified (not from cache)
    pub sigs_verified: u64,
    /// Signature cache hits
    pub cache_hits: u64,
    /// Signature cache hit rate
    pub cache_hit_rate: f64,
    /// Blocks skipped due to checkpoints
    pub checkpoint_skips: u64,
    /// Validation timeouts
    pub timeouts: u64,
    /// Average block validation time (ms)
    pub avg_block_time_ms: f64,
    /// Average transaction validation time (ms)
    pub avg_tx_time_ms: f64,
}

impl ValidationStats {
    /// Merge stats from a block validation result
    pub fn record_block_validation(&mut self, result: &BlockValidationResult) {
        self.blocks_validated += 1;
        self.txs_validated += result.tx_count as u64;
        self.sigs_verified += result.total_sigs_verified;
        self.cache_hits += result.total_cache_hits;

        if result.checkpoint_skipped {
            self.checkpoint_skips += 1;
        }

        // Update cache hit rate
        let total = self.cache_hits + self.sigs_verified;
        if total > 0 {
            self.cache_hit_rate = self.cache_hits as f64 / total as f64;
        }

        // Update average time (exponential moving average)
        let block_time_ms = result.validation_time.as_secs_f64() * 1000.0;
        self.avg_block_time_ms = self.avg_block_time_ms * 0.9 + block_time_ms * 0.1;
    }

    /// Record a timeout
    pub fn record_timeout(&mut self) {
        self.timeouts += 1;
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validation_stats() {
        let mut stats = ValidationStats::default();

        let result = BlockValidationResult {
            block_hash: [0u8; 32],
            height: 100,
            valid: true,
            validation_time: std::time::Duration::from_millis(50),
            tx_count: 10,
            total_cache_hits: 8,
            total_sigs_verified: 2,
            checkpoint_skipped: false,
        };

        stats.record_block_validation(&result);

        assert_eq!(stats.blocks_validated, 1);
        assert_eq!(stats.txs_validated, 10);
        assert_eq!(stats.cache_hits, 8);
        assert_eq!(stats.sigs_verified, 2);
        assert!(stats.cache_hit_rate > 0.7); // 8 out of 10
    }
}

