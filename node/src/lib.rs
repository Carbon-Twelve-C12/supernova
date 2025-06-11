pub mod api;
pub mod blockchain;
pub mod config;
pub mod environmental;
pub mod logging;
pub mod mempool;
// pub mod miner; // TODO: Implement
pub mod network;
pub mod node;
// pub mod rpc; // TODO: Implement
pub mod storage;
// pub mod wallet; // TODO: Implement
pub mod metrics;
// pub mod utils; // TODO: Implement
pub mod adapters; // Architectural bridge adapters
pub mod testnet;
pub mod thread_safety_fix;
pub mod thread_safety_test;

// Re-exports for convenience
pub use crate::config::NodeConfig;
pub use crate::node::{Node, NodeError};
pub use crate::network::{P2PNetwork, PeerInfo, NetworkCommand, NetworkEvent};
pub use crate::storage::{
    BackupManager, BackupOperation, BlockchainDB, ChainState, CheckpointConfig, 
    CheckpointManager, CheckpointType, RecoveryManager, StorageError, UtxoSet
};
// pub use crate::mining::{SecureDifficultyAdjuster, DifficultySecurityConfig}; // TODO: Implement mining module
pub use crate::testnet::{NodeTestnetManager, TestnetNodeConfig, TestnetStats, FaucetStatus, FaucetDistributionResult};
pub use btclib::validation::{BlockValidator, TransactionValidator};