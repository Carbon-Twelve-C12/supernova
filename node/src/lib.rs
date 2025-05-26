pub mod network;
pub mod mempool;
pub mod storage;
pub mod metrics;
pub mod config;
pub mod node;
pub mod api;
pub mod environmental;
pub mod testnet;

#[cfg(test)]
mod tests;

pub use crate::config::NodeConfig;
pub use crate::node::{Node, NodeError};
pub use crate::network::{P2PNetwork, PeerInfo, NetworkCommand, NetworkEvent};
pub use crate::storage::{
    BackupManager, BackupOperation, BlockchainDB, ChainState, CheckpointConfig, 
    CheckpointManager, CheckpointType, RecoveryManager, StorageError, UtxoSet
};
pub use crate::testnet::{NodeTestnetManager, TestnetNodeConfig, TestnetStats, FaucetStatus, FaucetDistributionResult};
pub use btclib::validation::{BlockValidator, TransactionValidator};