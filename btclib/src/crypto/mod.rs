// Cryptographic primitives for SuperNova blockchain
// This module collects various cryptographic features used in the blockchain

// Export quantum-resistant cryptography module
pub mod quantum;

// Export zero-knowledge proof systems module
pub mod zkp;

// Re-export commonly used types for convenience
pub use quantum::{QuantumScheme, QuantumKeyPair, QuantumParameters, QuantumError};
pub use zkp::{ZkpType, Commitment, ZeroKnowledgeProof, ZkpParams};
