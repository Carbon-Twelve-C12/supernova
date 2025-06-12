// Crypto module
// Contains cryptographic primitives and functions

// Public modules
pub mod hash;
pub mod signature;
pub mod quantum;
pub mod zkp;
pub mod falcon;
pub mod kem;

// Test modules
#[cfg(test)]
mod quantum_security_test;

// Re-export public types
// Note: Not exporting Hash to avoid conflicts with other Hash types
pub use quantum::{QuantumScheme, QuantumKeyPair, QuantumParameters, QuantumError, sign_quantum, verify_quantum_signature};
pub use zkp::{ZkpType, Commitment, ZeroKnowledgeProof, ZkpParams, generate_zkp, verify_zkp};
pub use signature::{SignatureScheme, SignatureVerifier, SignatureType, SignatureError, SignatureParams};
pub use falcon::{FalconKeyPair, FalconParameters, FalconError};
pub use kem::{KemKeyPair, KemError, encapsulate, decapsulate};

// Re-export hash functions from parent module
pub use crate::hash::hash256;
pub use hash::{hash256 as crypto_hash256, hash160};

// Module that provides cryptographic primitives for the blockchain
