// Supernova Node Library
// Node implementation for the Supernova blockchain

// Enforce panic-free code in production
#![cfg_attr(not(test), warn(clippy::unwrap_used))]
#![cfg_attr(not(test), warn(clippy::expect_used))]
#![cfg_attr(not(test), warn(clippy::panic))]
#![cfg_attr(not(test), warn(clippy::unimplemented))]
#![cfg_attr(not(test), warn(clippy::todo))]
#![cfg_attr(not(test), warn(clippy::unreachable))]
// Allow certain warnings for pragmatic reasons
#![allow(dead_code)] // Many functions are exposed as library API
#![allow(clippy::too_many_arguments)] // Complex blockchain functions need many params
#![allow(clippy::large_enum_variant)] // Blockchain data structures can be large
#![allow(clippy::type_complexity)] // Complex types are sometimes necessary
#![allow(clippy::indexing_slicing)] // We check bounds before indexing
#![allow(clippy::arc_with_non_send_sync)] // Necessary for certain async patterns

// Test-specific allows
#![cfg_attr(test, allow(clippy::unwrap_used))]
#![cfg_attr(test, allow(clippy::expect_used))]
#![cfg_attr(test, allow(clippy::panic))]

pub mod api;
pub mod blockchain;
pub mod config;
pub mod environmental;
pub mod logging;
pub mod mempool;
pub mod miner;
pub mod network;
pub mod node;
// pub mod rpc; // TODO: Implement
pub mod storage;
// pub mod wallet; // TODO: Implement
pub mod metrics;
// pub mod utils; // TODO: Implement
pub mod adapters; // Architectural bridge adapters
pub mod api_facade;
pub mod testnet;
pub mod thread_safety_fix;
pub mod thread_safety_test;

// Re-exports for convenience
pub use crate::config::NodeConfig;
pub use crate::miner::{BlockProducer, ProofOfWork};
pub use crate::network::{NetworkCommand, NetworkEvent, P2PNetwork, PeerInfo};
pub use crate::node::{Node, NodeError};
pub use crate::storage::{
    BackupManager, BackupOperation, BlockchainDB, ChainState, CheckpointConfig, CheckpointManager,
    CheckpointType, RecoveryManager, StorageError, UtxoSet,
};
pub use crate::testnet::{
    FaucetDistributionResult, FaucetStatus, NodeTestnetManager, TestnetNodeConfig, TestnetStats,
};
pub use btclib::validation::{BlockValidator, TransactionValidator};
