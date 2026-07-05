//! Shared newtypes, enums, and protocol constants (ADR-0002 §4, §17).

use serde::{Deserialize, Serialize};

/// Domain separation tag for the v1 attestation digest (SHA3-512 input prefix).
///
/// Binding the tag into the hash prevents cross-protocol signature reuse.
pub const DOMAIN_TAG_V1: &[u8] = b"supernova.green.attestation.v1";

/// Domain separation tag for offset-retirement quorum digests.
pub const DOMAIN_TAG_OFFSET_V1: &[u8] = b"supernova.green.offset.v1";

/// Default per-epoch committee size (ADR-0002 §5, §17): N = 21.
pub const DEFAULT_COMMITTEE_N: usize = 21;

/// Default signature threshold (ADR-0002 §5, §17): M = 15 = ceil(2N/3)+1,
/// tolerating up to 6 faulty/malicious members.
pub const DEFAULT_THRESHOLD_M: usize = 15;

/// Registry identifier (which EAC registry issued/retired the certificate).
///
/// Small integer id keyed to a governance-registered registry (Origin, I-REC, ...).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RegistryId(pub u16);

/// Stable oracle identity (e.g. hash of the oracle's genesis pubkey).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct OracleId(pub [u8; 32]);

/// Miner's post-quantum payout address (must equal coinbase payout in L3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MinerAddr(pub [u8; 32]);

/// Optional W3C DID string for Verifiable-Credential interop (ADR-0002 §12).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Did(pub String);

/// Category of the underlying clean-energy generation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(u8)]
pub enum EnergyType {
    Solar = 0,
    Wind = 1,
    Hydro = 2,
    Geothermal = 3,
    Nuclear = 4,
    Other = 255,
}

impl EnergyType {
    /// Stable single-byte encoding used inside the canonical digest.
    pub fn as_u8(self) -> u8 {
        self as u8
    }
}

/// Carbon-offset methodology standard (ADR-0002 §10).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(u8)]
pub enum OffsetStandard {
    GoldStandard = 0,
    Verra = 1,
}

impl OffsetStandard {
    /// Stable single-byte encoding used inside the offset digest.
    pub fn as_u8(self) -> u8 {
        self as u8
    }
}

/// v2 upgrade placeholder: 24/7 time+location-matched CFE proof (ADR-0002 §13).
///
/// Present as an `Option` field on the attestation so the schema can bump to v2
/// without redesign. Not verified in the v1 prototype.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TimeMatchProof {
    /// Opaque, methodology-specific proof bytes (Green Proofs 24/7 CFE).
    pub proof: Vec<u8>,
}

/// v2 upgrade placeholder: signed hardware-meter reading (ADR-0002 §13, §15).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MeterSignature {
    /// Opaque meter-attestation bytes.
    pub sig: Vec<u8>,
}
