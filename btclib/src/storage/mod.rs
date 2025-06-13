/// Storage subsystem for supernova blockchain
///
/// Provides persistent storage for blockchain data including blocks, transactions,
/// UTXO set, and associated metadata. Optimized for performance and data integrity.

pub mod utxo_set;
pub mod chain_state;
pub mod block_store;
pub mod backup;
pub mod bloom;

// Re-export key types
pub use utxo_set::{UtxoSet, UtxoEntry, UtxoCommitment, UtxoCacheStats};
pub use chain_state::{ChainState, ChainStateConfig, ForkResolutionPolicy};
pub use block_store::{BlockStore, BlockLocation, BlockStorageConfig}; 