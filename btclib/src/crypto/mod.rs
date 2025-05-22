// Crypto module
// Contains cryptographic primitives and functions

// Public modules
pub mod hash;
pub mod signature;
pub mod quantum;
pub mod zkp;
pub mod falcon;

// Re-export public types
pub use hash::Hash;
pub use quantum::{QuantumScheme, QuantumKeyPair, QuantumParameters, QuantumError};
pub use zkp::{ZkpType, Commitment, ZeroKnowledgeProof, ZkpParams};
pub use signature::{SignatureScheme, SignatureVerifier, SignatureType, SignatureError, SignatureParams};
pub use falcon::{FalconKeyPair, FalconParameters, FalconError};

// Module that provides cryptographic primitives for the blockchain
