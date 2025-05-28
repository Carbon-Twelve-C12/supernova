// Storage module for SuperNova node
// 
// This module handles persistence of blockchain data and related functionality

pub mod database;
pub mod persistence;
pub mod backup;
pub mod checkpoint;
pub mod corruption;
pub mod integrity;
pub mod utxo_set;
pub mod atomic_utxo_set;
pub mod database_shutdown;
pub mod journal;

#[cfg(test)]
pub mod database_shutdown_tests;

#[cfg(test)]
mod utxo_attack_tests;

pub use persistence::ChainState;
pub use database::{BlockchainDB, BlockchainDBConfig, StorageError, IntegrityCheckLevel, IntegrityCheckResult};
pub use backup::{BackupManager, BackupMode, BackupState, BackupError, BackupOperation, RecoveryManager};
pub use checkpoint::{CheckpointManager, CheckpointType, CheckpointConfig, CheckpointError};
pub use corruption::{CorruptionHandler, CorruptionError, IntegrityChecker, CorruptionInfo, CorruptionType, RepairPlan};
pub use database_shutdown::{DatabaseShutdownHandler, DatabaseStartupHandler, ShutdownConfig};
pub use journal::{WriteAheadLog, JournalEntry, WalError};
pub use utxo_set::UtxoSet;
pub use atomic_utxo_set::{AtomicUtxoSet, UnspentOutput, OutPoint};