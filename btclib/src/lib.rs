// SuperNova Blockchain Library
// Core implementation of the SuperNova blockchain

// Public modules
pub mod api;
pub mod block;
pub mod blockchain;
pub mod config;
pub mod consensus;
pub mod consensus_verification;
pub mod crypto;
pub mod environmental;
pub mod error;
pub mod errors;
pub mod hash;
pub mod mempool;
pub mod monitoring;
pub mod network;
pub mod mining;
pub mod script;
pub mod security_mitigation;
pub mod storage;
pub mod testnet;
pub mod transaction_processor;
pub mod types;
pub mod util;
pub mod validation;
pub mod verification;

// Library version
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

// Important! Re-export ValidationResult first to ensure it's available
pub use crate::validation::transaction::ValidationResult;

// Re-export commonly used types
pub use crate::blockchain::{Block, BlockHeader, Transaction};
pub use crate::crypto::hash::Hash;
pub use crate::validation::ValidationError;

// Re-export public API
pub use crate::api::{Api, ApiConfig};
pub use crate::config::SuperNovaConfig;
pub use crate::environmental::{
    EmissionsTracker, 
    Emissions, 
    EnvironmentalTreasury, 
    EnvironmentalAssetType,
    EnvironmentalDashboard, 
    EmissionsTimePeriod
};
pub use crate::verification::{
    VerificationService,
    VerificationStatus,
};
pub use crate::consensus_verification::{
    ConsensusVerificationFramework, 
    VerificationReport, 
    ConsensusProperty
};
pub use crate::consensus::{DifficultyAdjustment, DifficultyAdjustmentConfig};
pub use crate::validation::{
    BlockValidator, 
    BlockValidationConfig, 
    TransactionValidator
};
pub use crate::mempool::{TransactionPool, TransactionPoolConfig, MempoolError};
pub use crate::util::merkle::{MerkleTree, MerkleProof, MerkleError};
pub use crate::errors::{SuperNovaError, SuperNovaResult};

// Re-export Lightning types when feature is enabled
#[cfg(feature = "lightning")]
pub mod lightning;

#[cfg(feature = "lightning")]
pub use lightning::{
    LightningNetwork,
    LightningConfig,
    LightningNetworkError,
    Channel,
    ChannelId,
    ChannelState,
    ChannelConfig,
    Invoice,
    PaymentHash,
    PaymentPreimage,
    Router,
    LightningWallet,
};

// Add the freeze module to the library
// Freeze feature allows parts of the code to be disabled during compilation
// This is useful for working around circular dependencies or other issues
pub mod freeze;
pub use freeze::*;

// Add this export near other testnet-related exports
pub use testnet::network_simulator::SimulationConfig;
