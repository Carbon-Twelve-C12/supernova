// Storage module for supernova node
//
// This module handles persistence of blockchain data and related functionality

pub mod atomic_utxo_set;
pub mod backup;
pub mod checkpoint;
pub mod corruption;
pub mod database;
pub mod database_shutdown;
pub mod integrity;
pub mod journal;
pub mod memory;
pub mod persistence;
pub mod traits;
pub mod utxo_set;

#[cfg(test)]
pub mod database_shutdown_tests;

#[cfg(test)]
mod utxo_attack_tests;

pub use atomic_utxo_set::{AtomicUtxoSet, OutPoint, UnspentOutput};
pub use backup::{
    BackupError, BackupManager, BackupMode, BackupOperation, BackupState, RecoveryManager,
};
pub use checkpoint::{CheckpointConfig, CheckpointError, CheckpointManager, CheckpointType};
pub use corruption::{
    CorruptionError, CorruptionHandler, CorruptionInfo, CorruptionType, IntegrityChecker,
    RepairPlan,
};
pub use database::{
    BlockchainDB, BlockchainDBConfig, IntegrityCheckLevel, IntegrityCheckResult, StorageError,
};
pub use database_shutdown::{DatabaseShutdownHandler, DatabaseStartupHandler, ShutdownConfig};
pub use journal::{JournalEntry, WalError, WriteAheadLog};
pub use memory::MemoryStorage;
pub use persistence::ChainState;
pub use traits::Storage;
pub use utxo_set::UtxoSet;
