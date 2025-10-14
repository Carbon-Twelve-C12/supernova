// Crypto module
// Contains cryptographic primitives and functions

// Public modules
pub mod falcon_real; // Using Falcon implementation
pub mod hash;
pub mod kem;
pub mod quantum;
pub mod signature;
pub mod zkp;

// Falcon module removed - use falcon_real for actual cryptographic implementation

// Test modules
#[cfg(test)]
mod quantum_security_test;

// Re-export public types
// Note: Not exporting Hash to avoid conflicts with other Hash types
pub use quantum::{
    sign_quantum, verify_quantum_signature, QuantumError, QuantumKeyPair, QuantumParameters,
    QuantumScheme,
};
pub use quantum::{ECDSASignature, FalconSignature, SPHINCSSignature};
pub use quantum::{MLDSAPrivateKey, MLDSAPublicKey, MLDSASecurityLevel, MLDSASignature};
pub use signature::{
    SignatureError, SignatureParams, SignatureScheme, SignatureType, SignatureVerifier,
};
pub use zkp::{generate_zkp, verify_zkp, Commitment, ZeroKnowledgeProof, ZkpParams, ZkpType};

// Export REAL Falcon implementation
pub use falcon_real::{
    falcon_sign, falcon_verify, FalconError as RealFalconError, FalconKeyPair as RealFalconKeyPair,
    FalconParameters as RealFalconParameters, FalconSecurityLevel,
};

// Legacy falcon exports removed - use RealFalcon* types instead

pub use kem::{decapsulate, encapsulate, KemError, KemKeyPair};

// Re-export hash functions from parent module
pub use crate::hash::{hash256, Hash256};
pub use hash::{hash160, hash256 as crypto_hash256};

// Module that provides cryptographic primitives for the blockchain
