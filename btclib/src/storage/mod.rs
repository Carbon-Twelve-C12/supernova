pub mod backup;
pub mod block_store;
pub mod bloom;
pub mod chain_state;
/// Storage subsystem for supernova blockchain
///
/// Provides persistent storage for blockchain data including blocks, transactions,
/// UTXO set, and associated metadata. Optimized for performance and data integrity.
pub mod utxo_set;

// Re-export key types
pub use block_store::{BlockLocation, BlockStorageConfig, BlockStore};
pub use chain_state::{ChainState, ChainStateConfig, ForkResolutionPolicy};
pub use utxo_set::{UtxoCacheStats, UtxoCommitment, UtxoEntry, UtxoSet};
