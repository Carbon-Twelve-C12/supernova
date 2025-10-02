// Mining Infrastructure for Supernova Blockchain

pub mod merkle;
pub mod coinbase;
pub mod template;

#[cfg(feature = "testnet")]
pub mod test_miner;

pub use merkle::{calculate_merkle_root, build_merkle_tree, generate_merkle_proof, verify_merkle_proof};
pub use coinbase::build_coinbase_transaction;
pub use template::BlockTemplate;

#[cfg(feature = "testnet")]
pub use test_miner::mine_block_simple;
