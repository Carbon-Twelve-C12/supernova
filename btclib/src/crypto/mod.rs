// Crypto module
// Contains cryptographic primitives and functions

// Public modules
pub mod hash;
pub mod signature;
pub mod quantum;
pub mod zkp;
pub mod falcon;

// Test modules
#[cfg(test)]
mod quantum_security_test;

// Re-export public types
pub use hash::Hash;
pub use quantum::{QuantumScheme, QuantumKeyPair, QuantumParameters, QuantumError};
pub use zkp::{ZkpType, Commitment, ZeroKnowledgeProof, ZkpParams};
pub use signature::{SignatureScheme, SignatureVerifier, SignatureType, SignatureError, SignatureParams};
pub use falcon::{FalconKeyPair, FalconParameters, FalconError};

// Re-export hash256 from parent module
pub use crate::hash::hash256;

// Module that provides cryptographic primitives for the blockchain
