// Supernova Blockchain Library
// Core implementation of the Supernova blockchain

// Enforce panic-free code in production
#![cfg_attr(not(test), warn(clippy::unwrap_used))]
#![cfg_attr(not(test), warn(clippy::expect_used))]
#![cfg_attr(not(test), warn(clippy::panic))]
#![cfg_attr(not(test), warn(clippy::unimplemented))]
#![cfg_attr(not(test), warn(clippy::todo))]
#![cfg_attr(not(test), warn(clippy::unreachable))]
// Allow certain warnings in the entire crate for pragmatic reasons
#![allow(dead_code)] // Many functions are exposed as library API
#![allow(clippy::too_many_arguments)] // Complex blockchain functions need many params
#![allow(clippy::large_enum_variant)] // Blockchain data structures can be large
#![allow(clippy::type_complexity)] // Complex types are sometimes necessary
#![allow(clippy::indexing_slicing)] // We check bounds before indexing
#![allow(clippy::manual_clamp)] // More readable than clamp in some cases

// Test-specific allows
#![cfg_attr(test, allow(clippy::unwrap_used))]
#![cfg_attr(test, allow(clippy::expect_used))]
#![cfg_attr(test, allow(clippy::panic))]

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
pub use crate::consensus::{DifficultyAdjustment, DifficultyAdjustmentConfig};
pub use crate::consensus_verification::{
    ConsensusProperty, ConsensusVerificationFramework, VerificationReport,
};
pub use crate::environmental::{
    Emissions, EmissionsTimePeriod, EmissionsTracker, EnvironmentalAssetType,
    EnvironmentalDashboard, EnvironmentalTreasury,
};
pub use crate::errors::{supernovaError, supernovaResult};
pub use crate::mempool::{MempoolError, TransactionPool, TransactionPoolConfig};
pub use crate::util::merkle::{MerkleError, MerkleProof, MerkleTree};
pub use crate::validation::{BlockValidationConfig, BlockValidator, TransactionValidator};
pub use crate::verification::{VerificationService, VerificationStatus};

// Re-export Lightning types when feature is enabled
#[cfg(feature = "lightning")]
pub use lightning::{
    Channel, ChannelConfig, ChannelId, ChannelState, Invoice, LightningConfig, LightningNetwork,
    LightningNetworkError, LightningWallet, PaymentHash, PaymentPreimage, Router,
};

// Re-export security audit types
pub use crate::security::{
    prepare_environmental_system_audit, prepare_quantum_security_audit,
    EnvironmentalSystemAuditReport, QuantumSecurityAuditReport,
};

// Re-export deployment types
pub use crate::deployment::{
    deploy_supernova_testnet, TestnetConfiguration, TestnetDeploymentStatus,
};

// Add the freeze module to the library
// Freeze feature allows parts of the code to be disabled during compilation
// This is useful for working around circular dependencies or other issues
pub use freeze::*;

// Add this export near other testnet-related exports
pub use testnet::network_simulator::SimulationConfig;

// Test utilities module - only available in test builds
#[cfg(test)]
pub mod test_utils;

#[cfg(test)]
pub mod test_common;
