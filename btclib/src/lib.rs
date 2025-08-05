// supernova Blockchain Library
// Core implementation of the supernova blockchain

// Enforce panic-free code in production
#![cfg_attr(not(test), warn(clippy::unwrap_used))]
#![cfg_attr(not(test), warn(clippy::expect_used))]
#![cfg_attr(not(test), warn(clippy::panic))]
#![cfg_attr(not(test), warn(clippy::unimplemented))]
#![cfg_attr(not(test), warn(clippy::todo))]
#![cfg_attr(not(test), warn(clippy::unreachable))]
#![cfg_attr(not(test), warn(clippy::indexing_slicing))]

// Public modules
pub mod api;
pub mod block;
pub mod blockchain;
pub mod cli;
pub mod config;
pub mod consensus;
pub mod consensus_verification;
pub mod crypto;
pub mod deployment;
pub mod environmental;
pub mod error;
pub mod errors;
pub mod freeze;
pub mod hash;
pub mod journal;
pub mod lightning;
pub mod mempool;
pub mod mining;
pub mod monitoring;
pub mod network;
pub mod p2p;
pub mod rpc;
pub mod script;
pub mod security;
pub mod security_mitigation;
pub mod state;
pub mod storage;
pub mod testnet;
pub mod transaction;
pub mod transaction_processor;
pub mod types;
pub mod util;
pub mod validation;
pub mod verification;
pub mod wallet;

// Feature-gated modules
#[cfg(feature = "atomic-swap")]
pub mod atomic_swap;

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
pub use crate::config::supernovaConfig;
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
pub use crate::errors::{supernovaError, supernovaResult};

// Re-export Lightning types when feature is enabled
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

// Re-export security audit types
pub use crate::security::{
    QuantumSecurityAuditReport,
    EnvironmentalSystemAuditReport,
    prepare_quantum_security_audit,
    prepare_environmental_system_audit,
};

// Re-export deployment types
pub use crate::deployment::{
    TestnetConfiguration,
    deploy_supernova_testnet,
    TestnetDeploymentStatus,
};

// Add the freeze module to the library
// Freeze feature allows parts of the code to be disabled during compilation
// This is useful for working around circular dependencies or other issues
pub use freeze::*;

// Add this export near other testnet-related exports
pub use testnet::network_simulator::SimulationConfig;
