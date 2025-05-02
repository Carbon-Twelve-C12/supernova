/// Consensus subsystem for SuperNova blockchain
/// 
/// Provides algorithms and rules governing the blockchain consensus,
/// including difficulty adjustment, block validation, and fork resolution.

pub mod difficulty;

// Re-export key types
pub use difficulty::{
    DifficultyAdjustment,
    DifficultyAdjustmentConfig,
    DifficultyAdjustmentError,
}; 