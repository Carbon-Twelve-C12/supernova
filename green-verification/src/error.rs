//! Crate error type.
//!
//! Wraps [`supernova_core::crypto::quantum::QuantumError`] (ML-DSA failures)
//! via `#[from]` so signature-layer errors propagate cleanly.

use supernova_core::crypto::quantum::QuantumError;
use thiserror::Error;

/// Errors produced by the green-verification prototype.
///
/// EXPERIMENTAL / UNAUDITED — messages are intentionally descriptive for
/// testnet debugging and are not hardened against information leakage.
#[derive(Error, Debug, Clone, PartialEq, Eq)]
pub enum GreenError {
    /// Fewer than `need` distinct valid oracle signatures were present.
    #[error("quorum not met: have {have} distinct valid signatures, need {need}")]
    QuorumNotMet { have: usize, need: usize },

    /// A signature failed ML-DSA verification against the claimed key.
    #[error("invalid oracle signature")]
    InvalidSignature,

    /// The same oracle signed more than once (over-count / replay attempt).
    #[error("duplicate signer in attestation")]
    DuplicateSigner,

    /// The EAC retirement id has already been claimed (nullifier hit).
    #[error("EAC retirement id already used")]
    EacAlreadyUsed,

    /// An offset serial has already been retired (nullifier hit).
    #[error("offset serial already used")]
    OffsetSerialAlreadyUsed,

    /// The attestation expired at or before the current height.
    #[error("attestation expired: expiry_height {expiry} <= current height {height}")]
    ExpiredAttestation { expiry: u64, height: u64 },

    /// The EAC vintage epoch falls outside the allowed vintage window.
    #[error("vintage {vintage} outside allowed window [{lo}, {hi}]")]
    VintageOutOfWindow { vintage: u64, lo: u64, hi: u64 },

    /// Claimed MWh exceeds the conservative per-block upper bound.
    #[error("claimed mwh_milli {claimed} exceeds bound {bound}")]
    MwhExceedsBound { claimed: u64, bound: u64 },

    /// A signer's oracle id is not a registered member of the committee.
    #[error("signer is not a registered committee oracle key")]
    UnknownOracleKey,

    /// The backing registry rejected or could not verify the retirement.
    #[error("registry error: {0}")]
    RegistryError(String),

    /// A treasury operation would drive the balance negative.
    #[error("treasury underflow")]
    TreasuryUnderflow,

    /// The oracle is not currently bonded (cannot serve on a committee).
    #[error("oracle not bonded")]
    NotBonded,

    /// The oracle's bond is below the minimum required to participate.
    #[error("insufficient bond: have {have}, need {need}")]
    InsufficientBond { have: u64, need: u64 },

    /// An underlying ML-DSA / quantum crypto operation failed.
    #[error("quantum crypto error: {0}")]
    Quantum(#[from] QuantumError),
}
