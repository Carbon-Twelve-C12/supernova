/// Mining module for SuperNova
///
/// This module provides mining-related functionality.

use serde::{Serialize, Deserialize};
use thiserror::Error;

/// Mining error types
#[derive(Debug, Error)]
pub enum MiningError {
    #[error("Mining target error: {0}")]
    TargetError(String),

    #[error("Hash calculation error: {0}")]
    HashError(String),

    #[error("Nonce overflow error")]
    NonceOverflow,

    #[error("Mining interrupted: {0}")]
    Interrupted(String),
}

/// Mining configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MiningConfig {
    /// Maximum number of threads to use for mining
    pub max_threads: usize,
    
    /// Number of hashes to calculate before checking for stop signal
    pub batch_size: usize,

    /// Whether to enable environmental optimization
    pub environmental_optimization: bool,
}

impl Default for MiningConfig {
    fn default() -> Self {
        Self {
            max_threads: 4,
            batch_size: 1000,
            environmental_optimization: true,
        }
    }
}

// Re-export mining components that will be implemented later
// pub mod miner;
// pub use miner::Miner; 