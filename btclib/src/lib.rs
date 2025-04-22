// SuperNova Blockchain Library
// Core implementation of the SuperNova blockchain

// Re-export public API
pub mod api;
pub mod config;
pub mod crypto;
pub mod environmental;
pub mod types;
pub mod validation;
pub mod testnet;
pub mod consensus_verification;

// Internal modules
mod transaction_processor;
mod storage;
mod mempool;
mod mining;
mod network;
mod security_mitigation;
mod monitoring;

// Re-export common types for convenience
pub use types::transaction::{Transaction, TransactionInput, TransactionOutput};
pub use types::block::{Block, BlockHeader};
pub use types::units::{NovaUnit, TOTAL_NOVA_SUPPLY, format_as_nova};
pub use crypto::quantum::{QuantumKeyPair, QuantumParameters, QuantumScheme, ClassicalScheme};
pub use environmental::emissions::{EmissionsTracker, Emissions};
pub use environmental::treasury::{EnvironmentalTreasury, EnvironmentalAssetType};
pub use environmental::dashboard::{EnvironmentalDashboard, EmissionsTimePeriod};
pub use consensus_verification::{ConsensusVerificationFramework, VerificationReport, ConsensusProperty};
