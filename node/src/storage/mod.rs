// Storage module for supernova node
//
// This module handles persistence of blockchain data and related functionality

pub mod atomic_utxo_set;
pub mod backup;
pub mod checkpoint;
pub mod checksum;
pub mod corruption;
pub mod database;
pub mod database_shutdown;
pub mod integrity;
pub mod journal;
pub mod memory;
pub mod persistence;
pub mod traits;
pub mod transaction_index;
pub mod utxo_cache;
pub mod utxo_set;

#[cfg(test)]
pub mod database_shutdown_tests;

#[cfg(test)]
mod utxo_attack_tests;

pub use atomic_utxo_set::{AtomicUtxoSet, OutPoint, UnspentOutput, UtxoLockManager, UtxoTransaction};
pub use backup::{
    BackupError, BackupManager, BackupMode, BackupOperation, BackupState, RecoveryManager,
};
pub use checkpoint::{CheckpointConfig, CheckpointError, CheckpointManager, CheckpointType};
pub use checksum::{
    calculate_block_checksum, calculate_crc32, calculate_sha256, calculate_utxo_checksum,
    verify_block_checksum, verify_crc32, verify_sha256, verify_utxo_checksum, ChecksumError,
    ChecksummedData, StreamingChecksum,
};
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
pub use transaction_index::{
    BlockLocation, IndexStatistics, IndexedTransaction, TransactionIndexConfig, TransactionIndexer,
    TransactionIndexError,
};
pub use utxo_cache::{
    CacheEntry, CacheEntryState, CacheStatistics, PruningConfig, UtxoCache, UtxoCacheConfig,
    UtxoSnapshot, load_from_snapshot,
};
