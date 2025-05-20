// SuperNova Blockchain Library
// Core implementation of the SuperNova blockchain

// Re-export public API
pub mod api;
pub mod config;
pub mod crypto;
pub mod environmental;
pub mod types;
pub mod validation;
pub mod testnet;
pub mod consensus_verification;
pub mod consensus;
#[cfg(feature = "lightning")]
pub mod lightning;
pub mod mempool;
pub mod util;

// Internal modules
mod transaction_processor;
mod storage;
mod mining;
mod network;
mod security_mitigation;
mod monitoring;

// Re-export common types for convenience
pub use types::transaction::{Transaction, TransactionInput, TransactionOutput};
pub use types::block::{Block, BlockHeader};
pub use types::units::{NovaUnit, TOTAL_NOVA_SUPPLY, format_as_nova};
pub use crypto::quantum::{QuantumKeyPair, QuantumParameters, QuantumScheme, ClassicalScheme};
pub use environmental::emissions::{EmissionsTracker, Emissions};
pub use environmental::treasury::{EnvironmentalTreasury, EnvironmentalAssetType};
pub use environmental::dashboard::{EnvironmentalDashboard, EmissionsTimePeriod};
pub use consensus_verification::{ConsensusVerificationFramework, VerificationReport, ConsensusProperty};
pub use consensus::{DifficultyAdjustment, DifficultyAdjustmentConfig};
pub use validation::{BlockValidator, BlockValidationConfig, TransactionValidator, ValidationResult};
pub use mempool::{TransactionPool, TransactionPoolConfig, MempoolError};
pub use util::merkle::{MerkleTree, MerkleProof, MerkleError};

// Re-export Lightning types when feature is enabled
#[cfg(feature = "lightning")]
pub use lightning::{
    LightningNetwork, 
    LightningConfig, 
    LightningNetworkError,
    channel::{Channel, ChannelId, ChannelState, ChannelConfig, ChannelInfo}
};

// Add the freeze module to the library

// Freeze feature allows parts of the code to be disabled during compilation
// This is useful for working around circular dependencies or other issues
// that prevent the codebase from building.
pub mod freeze;
pub use freeze::*;
