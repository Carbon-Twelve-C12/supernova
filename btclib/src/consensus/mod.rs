/// Consensus subsystem for supernova blockchain
///
/// Provides algorithms and rules governing the blockchain consensus,
/// including difficulty adjustment, block validation, and fork resolution.
pub mod difficulty;
pub mod fork_resolution_v2;
pub mod secure_fork_resolution;
pub mod time_warp_prevention;
pub mod timestamp_validation;

#[cfg(test)]
pub mod time_warp_tests;

#[cfg(test)]
mod fork_resolution_attack_tests;

#[cfg(test)]
mod security_fix_tests;

// Re-export key types
pub use difficulty::{
    DifficultyAdjuster, DifficultyAdjustment, DifficultyAdjustmentConfig,
    DifficultyAdjustmentError, BLOCK_TIME_TARGET,
};

// Import validation functions from the main validation module
pub use crate::validation::{validate_block, validate_transaction};
// Export the new fork resolution v2 as the primary implementation
pub use fork_resolution_v2::{
    ForkResolutionError, ForkResolutionResult, ProofOfWorkForkResolver as SecureForkResolver,
};

// Keep the old types for backward compatibility during transition
pub use secure_fork_resolution::{ChainMetrics, SecureForkConfig};

pub use timestamp_validation::{
    TimestampValidationConfig, TimestampValidationError, TimestampValidator, MAX_FUTURE_TIME,
    MEDIAN_TIME_BLOCKS,
};
