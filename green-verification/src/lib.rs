//! # green-verification — PROTOTYPE FOUNDATION
//!
//! **EXPERIMENTAL / UNAUDITED / TESTNET-ONLY** prototype of
//! [ADR-0002 "Verifiable Green-Mining & Carbon-Negative Incentive System"].
//!
//! **NOT wired into consensus. Do not use on mainnet.**
//!
//! This crate contains only *pure functions* and their tests. Nothing here
//! touches live consensus, block validation, coinbase, or money-supply. All
//! incentive logic is **redistributive** — it moves value that already exists
//! inside the environmental treasury and **NEVER mints new NOVA**. Total supply
//! is fixed at 42,000,000 NOVA; the treasury/rebate helpers preserve that
//! invariant by construction (see [`treasury`] and its property tests).
//!
//! Consensus wiring, persistence of the nullifier set, real registry network
//! calls, and VRF committee sampling are explicitly deferred to later,
//! operator-sign-off phases and are marked with `TODO(phase-N)` where they are
//! stubbed.
//!
//! ## Cryptography
//!
//! - Signatures: **ML-DSA (Dilithium, Level 3 / ML-DSA-65)** reused verbatim
//!   from [`supernova_core::crypto`]. No new signature primitives are defined.
//! - Canonical digest: **SHA3-512** over a domain-separated field encoding
//!   (see [`attestation::GreenAttestation::canonical_digest`]).
//!
//! ## Module map (mirrors ADR-0002 §3 layers)
//!
//! - [`error`]    — crate error type ([`error::GreenError`]).
//! - [`types`]    — shared newtypes, enums, and protocol constants.
//! - [`attestation`] — L2 attestation object + L3 deterministic verification.
//! - [`oracle`]   — L1 permissionless staked oracle pool + committee sampling.
//! - [`nullifier`] — L2 in-memory nullifier set (double-claim rejection).
//! - [`registry`] — L0 EAC registry trait + Origin adapter stub + mock.
//! - [`treasury`] — L4 treasury pure accounting (redistributive, 42M-safe).
//! - [`incentive`] — L5 redistributive rebate gradient helpers.
//! - [`offset`]   — L6 carbon offset retirement + provable-negative accounting.

#![forbid(unsafe_code)]

pub mod attestation;
pub mod error;
pub mod incentive;
pub mod nullifier;
pub mod offset;
pub mod oracle;
pub mod registry;
pub mod treasury;
pub mod types;

// Convenience re-exports of the most commonly used items.
pub use attestation::{GreenAttestation, OracleSig, VerifyParams};
pub use error::GreenError;
pub use nullifier::NullifierSet;
pub use oracle::{Committee, OracleRegistry, OracleStake};
pub use registry::{
    EacCredentialSubject, EacRecord, EacRegistry, MockRegistry, OriginRegistryAdapter,
    RegistryError,
};
pub use treasury::EnvironmentalTreasury;
pub use types::{
    EnergyType, MinerAddr, OffsetStandard, OracleId, RegistryId, DOMAIN_TAG_V1,
    DEFAULT_COMMITTEE_N, DEFAULT_THRESHOLD_M,
};
