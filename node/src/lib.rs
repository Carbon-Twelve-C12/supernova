pub mod network;
pub mod mempool;
pub mod storage;
pub mod metrics;
pub mod config;
pub mod node;
pub mod api;
pub mod environmental;

#[cfg(test)]
mod tests;

pub use crate::config::NodeConfig;
pub use crate::node::{Node, NodeError};
pub use crate::network::{NetworkManager, PeerInfo};
pub use crate::storage::{
    BackupManager, BackupOperation, BlockchainDB, ChainState, CheckpointConfig, 
    CheckpointManager, CheckpointType, RecoveryManager, StorageError, UTXOSet
};
pub use btclib::validation::{BlockValidator, TransactionValidator};