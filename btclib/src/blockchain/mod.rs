// Blockchain module - re-exports core blockchain types and functionality

// Re-export Block and BlockHeader from types
pub use crate::types::block::{Block, BlockHeader};
pub use crate::types::transaction::Transaction;

// Re-export hash functions
pub use crate::crypto::hash::Hash;

// Re-export validation functionality
pub use crate::validation::{
    ValidationError,
    ValidationResult,
    BlockValidator,
    TransactionValidator,
};

// Export Consensus functionality
pub use crate::consensus::{
    DifficultyAdjustment,
    DifficultyAdjustmentConfig,
};

// Export UTXO functionality
pub use crate::storage::utxo_set::UtxoSet;

// This module organizes the core blockchain components for easier imports 