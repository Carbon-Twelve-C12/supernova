// Cryptographic primitives for SuperNova blockchain
// This module collects various cryptographic features used in the blockchain

// Export quantum-resistant cryptography module
pub mod quantum;

// Export zero-knowledge proof systems module
pub mod zkp;

// Export signature abstraction layer
pub mod signature;

// Export Falcon signature scheme
pub mod falcon;

// Re-export commonly used types for convenience
pub use quantum::{QuantumScheme, QuantumKeyPair, QuantumParameters, QuantumError};
pub use zkp::{ZkpType, Commitment, ZeroKnowledgeProof, ZkpParams};
pub use signature::{SignatureScheme, SignatureVerifier, SignatureType, SignatureError, SignatureParams};
pub use falcon::{FalconKeyPair, FalconParameters, FalconError};
