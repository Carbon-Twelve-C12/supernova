// Verification module for environmental claims

// Re-export verification types and services from environmental
pub use crate::environmental::verification::{
    CarbonOffset, RenewableCertificate, VerificationConfig, VerificationError,
    VerificationProvider, VerificationService,
};

// Import VerificationStatus directly from emissions since it's the original definition
pub use crate::environmental::emissions::VerificationStatus;

// Re-export miner verification status
pub use crate::environmental::miner_reporting::MinerVerificationStatus;

// This module centralizes all verification-related functionality for the blockchain
// and helps organize the environmental verification components.
