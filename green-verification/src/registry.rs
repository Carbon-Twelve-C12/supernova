//! L0 EAC registry anchor (ADR-0002 §12).
//!
//! **EXPERIMENTAL / UNAUDITED / TESTNET-ONLY.** Pure logic + tests; nothing here
//! performs real network I/O or touches consensus.
//!
//! This module resolves an *Energy Attribute Certificate* (EAC) retirement id to
//! a verified [`EacRecord`], modeled as a **W3C Verifiable-Credential-like**
//! envelope (ADR-0002 §12: "W3C Verifiable Credentials + DIDs, ML-DSA-signed").
//!
//! Two implementations of the [`EacRegistry`] trait are provided:
//!
//! - [`OriginRegistryAdapter`] — an INTERFACE stub for Energy Web Origin /
//!   I-REC (ERC-1188). The real network lookup is intentionally NOT implemented
//!   in this prototype; the method documents the integration point and returns
//!   [`RegistryError::NotImplemented`].
//! - [`MockRegistry`] — a deterministic in-memory registry used by the tests.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::error::GreenError;
use crate::types::{EnergyType, RegistryId};

/// Canonical `@context` entry for a base W3C Verifiable Credential.
pub const VC_CONTEXT_W3C: &str = "https://www.w3.org/2018/credentials/v1";
/// Supernova-specific context describing the EAC credential subject shape.
pub const VC_CONTEXT_EAC: &str = "https://supernovanetwork.xyz/contexts/eac/v1";
/// Base credential `type` tag required on every Verifiable Credential.
pub const VC_TYPE_BASE: &str = "VerifiableCredential";
/// Credential `type` tag identifying an Energy Attribute Certificate.
pub const VC_TYPE_EAC: &str = "EnergyAttributeCertificate";

/// Errors produced by an [`EacRegistry`] lookup.
///
/// A dedicated, self-contained error type (ADR-0002 §12) so the registry layer
/// does not depend on the rest of the crate's error surface. It converts into
/// the crate-wide [`GreenError`] via `From` for callers that want a single error
/// channel.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum RegistryError {
    /// No certificate is registered under the supplied retirement id.
    #[error("unknown EAC retirement id: 0x{0}")]
    UnknownRetirement(String),

    /// A certificate exists but is not in a `retired` state (still issued/live).
    ///
    /// Only *retired* certificates may back a green claim — otherwise the same
    /// certificate could be claimed and later re-sold.
    #[error("EAC 0x{0} found but not retired")]
    NotRetired(String),

    /// The registry record was malformed or failed structural validation.
    #[error("malformed EAC record: {0}")]
    Malformed(String),

    /// The backing registry integration is not wired in this prototype.
    #[error("registry backend not implemented: {0}")]
    NotImplemented(String),
}

impl From<RegistryError> for GreenError {
    fn from(e: RegistryError) -> Self {
        GreenError::RegistryError(e.to_string())
    }
}

/// The verifiable claims carried by an EAC credential (the `credentialSubject`).
///
/// This is the payload a green attestation ultimately binds to: how much clean
/// energy was certified, of what type, for which vintage, on which registry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EacCredentialSubject {
    /// 32-byte retirement id (the credential subject's stable identifier).
    pub eac_retirement_id: [u8; 32],
    /// Registry the certificate was retired on.
    pub registry_id: RegistryId,
    /// Certified clean energy, in milli-MWh (1 MWh = 1000).
    pub mwh_milli: u64,
    /// Vintage epoch of generation.
    pub vintage_epoch: u64,
    /// Category of generation.
    pub energy_type: EnergyType,
    /// Whether the certificate is confirmed *retired* (not merely issued).
    pub retired: bool,
    /// Optional human-readable generation location (grid zone / country code).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub generation_location: Option<String>,
}

/// A verified EAC retirement record, shaped like a W3C Verifiable Credential.
///
/// The envelope (`@context`, `type`, `issuer`, `issuance_epoch`) mirrors the VC
/// data model for Energy Web interop (ADR-0002 §12); the domain payload lives in
/// [`EacCredentialSubject`]. In the real system this credential is ML-DSA-signed
/// by the issuing registry / oracle quorum; the prototype carries the claims
/// only (signature verification lives in the `attestation` layer).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EacRecord {
    /// JSON-LD `@context` list (base VC context first).
    #[serde(rename = "@context")]
    pub context: Vec<String>,
    /// Credential `type` list (must include [`VC_TYPE_BASE`]).
    #[serde(rename = "type")]
    pub credential_type: Vec<String>,
    /// Credential issuer — a DID or registry identifier string.
    pub issuer: String,
    /// Issuance epoch (registry-clock epoch when the credential was minted).
    pub issuance_epoch: u64,
    /// The verifiable claims.
    pub credential_subject: EacCredentialSubject,
}

impl EacRecord {
    /// Wrap a credential subject in a default VC envelope for `issuer`.
    ///
    /// Fills the standard `@context` / `type` lists so callers (and tests) do not
    /// have to restate the boilerplate.
    pub fn new(
        credential_subject: EacCredentialSubject,
        issuer: impl Into<String>,
        issuance_epoch: u64,
    ) -> Self {
        Self {
            context: vec![VC_CONTEXT_W3C.to_string(), VC_CONTEXT_EAC.to_string()],
            credential_type: vec![VC_TYPE_BASE.to_string(), VC_TYPE_EAC.to_string()],
            issuer: issuer.into(),
            issuance_epoch,
            credential_subject,
        }
    }

    /// Structural check that this looks like a well-formed EAC credential.
    ///
    /// Verifies the required VC `@context` / `type` tags are present. Returns a
    /// [`RegistryError::Malformed`] describing the first problem found.
    pub fn validate_shape(&self) -> Result<(), RegistryError> {
        if !self.context.iter().any(|c| c == VC_CONTEXT_W3C) {
            return Err(RegistryError::Malformed(format!(
                "missing base @context {VC_CONTEXT_W3C}"
            )));
        }
        if !self.credential_type.iter().any(|t| t == VC_TYPE_BASE) {
            return Err(RegistryError::Malformed(format!(
                "missing base credential type {VC_TYPE_BASE}"
            )));
        }
        if !self.credential_type.iter().any(|t| t == VC_TYPE_EAC) {
            return Err(RegistryError::Malformed(format!(
                "missing credential type {VC_TYPE_EAC}"
            )));
        }
        Ok(())
    }

    /// 32-byte retirement id of the underlying certificate.
    pub fn retirement_id(&self) -> [u8; 32] {
        self.credential_subject.eac_retirement_id
    }

    /// Certified clean energy in milli-MWh.
    pub fn mwh_milli(&self) -> u64 {
        self.credential_subject.mwh_milli
    }

    /// Vintage epoch of the underlying generation.
    pub fn vintage_epoch(&self) -> u64 {
        self.credential_subject.vintage_epoch
    }

    /// Category of the underlying generation.
    pub fn energy_type(&self) -> EnergyType {
        self.credential_subject.energy_type
    }

    /// Registry the certificate was retired on.
    pub fn registry_id(&self) -> RegistryId {
        self.credential_subject.registry_id
    }

    /// Whether the certificate is confirmed retired.
    pub fn is_retired(&self) -> bool {
        self.credential_subject.retired
    }
}

/// Anchor abstraction: resolve an EAC retirement id to its verified record
/// (ADR-0002 §12).
pub trait EacRegistry {
    /// Look up and verify a retirement by its 32-byte id.
    ///
    /// Implementations MUST only return `Ok(record)` for a certificate that is
    /// confirmed **retired** on the backing registry, and the returned record
    /// SHOULD pass [`EacRecord::validate_shape`].
    fn verify_retirement(&self, id: [u8; 32]) -> Result<EacRecord, RegistryError>;
}

/// Energy Web Origin / I-REC adapter — INTERFACE ONLY in this prototype.
///
/// The real implementation performs an ERC-1188 retirement lookup against the
/// configured Origin / I-REC endpoint, verifies the returned credential's
/// registry signature, and maps it into an [`EacRecord`]. That network call is
/// deliberately absent here (no HTTP dependency) and is the Phase-2 integration
/// point requiring operator sign-off.
#[derive(Debug, Clone)]
pub struct OriginRegistryAdapter {
    /// Configured registry endpoint (e.g. an Origin GraphQL / REST URL).
    pub endpoint: String,
    /// Which registry this adapter speaks to.
    pub registry_id: RegistryId,
}

impl OriginRegistryAdapter {
    /// Construct an adapter bound to an endpoint and registry id.
    pub fn new(endpoint: impl Into<String>, registry_id: RegistryId) -> Self {
        Self {
            endpoint: endpoint.into(),
            registry_id,
        }
    }
}

impl EacRegistry for OriginRegistryAdapter {
    fn verify_retirement(&self, _id: [u8; 32]) -> Result<EacRecord, RegistryError> {
        // TODO(phase-2): Energy Web Origin / I-REC ERC-1188 retirement lookup.
        //
        // Real implementation outline (requires operator sign-off + a wired,
        // audited HTTP client — intentionally absent in the Phase-0 foundation):
        //   1. Issue a signed query for `id` to `self.endpoint`.
        //   2. Verify the registry's signature over the returned VC.
        //   3. Confirm `credentialSubject.retired == true`.
        //   4. Map the VC into an `EacRecord` and return it.
        Err(RegistryError::NotImplemented(format!(
            "Origin/I-REC network lookup for endpoint {} (Phase-2, operator sign-off required)",
            self.endpoint
        )))
    }
}

/// Deterministic in-memory registry for tests and local simulation.
#[derive(Debug, Clone, Default)]
pub struct MockRegistry {
    /// Pre-seeded retirement records keyed by EAC id.
    records: HashMap<[u8; 32], EacRecord>,
}

impl MockRegistry {
    /// Create an empty mock registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Insert (or overwrite) a record for the given EAC id.
    ///
    /// The record's own `eac_retirement_id` is treated as authoritative for the
    /// map key so lookups and record contents stay consistent.
    pub fn insert(&mut self, record: EacRecord) {
        self.records.insert(record.retirement_id(), record);
    }

    /// Convenience: seed a retired solar-agnostic record from raw claim fields.
    pub fn insert_retired(
        &mut self,
        id: [u8; 32],
        registry_id: RegistryId,
        mwh_milli: u64,
        vintage_epoch: u64,
        energy_type: EnergyType,
    ) {
        let subject = EacCredentialSubject {
            eac_retirement_id: id,
            registry_id,
            mwh_milli,
            vintage_epoch,
            energy_type,
            retired: true,
            generation_location: None,
        };
        self.insert(EacRecord::new(subject, "did:supernova:mock-registry", 0));
    }

    /// Number of seeded records.
    pub fn len(&self) -> usize {
        self.records.len()
    }

    /// Whether the registry has no seeded records.
    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }
}

impl EacRegistry for MockRegistry {
    fn verify_retirement(&self, id: [u8; 32]) -> Result<EacRecord, RegistryError> {
        let id_hex = hex::encode(id);
        match self.records.get(&id) {
            Some(rec) => {
                rec.validate_shape()?;
                if rec.is_retired() {
                    Ok(rec.clone())
                } else {
                    Err(RegistryError::NotRetired(id_hex))
                }
            }
            None => Err(RegistryError::UnknownRetirement(id_hex)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn retired_subject(id: [u8; 32]) -> EacCredentialSubject {
        EacCredentialSubject {
            eac_retirement_id: id,
            registry_id: RegistryId(1),
            mwh_milli: 5_000,
            vintage_epoch: 100,
            energy_type: EnergyType::Solar,
            retired: true,
            generation_location: Some("US-CAISO".to_string()),
        }
    }

    #[test]
    fn mock_registry_returns_retired_record() {
        let mut reg = MockRegistry::new();
        let id = [1u8; 32];
        let rec = EacRecord::new(retired_subject(id), "did:supernova:test", 42);
        reg.insert(rec.clone());

        let got = reg.verify_retirement(id).expect("retired record resolves");
        assert_eq!(got, rec);
        assert_eq!(got.mwh_milli(), 5_000);
        assert_eq!(got.vintage_epoch(), 100);
        assert_eq!(got.energy_type(), EnergyType::Solar);
        assert_eq!(got.registry_id(), RegistryId(1));
        assert!(got.is_retired());
    }

    #[test]
    fn record_has_w3c_vc_envelope() {
        let rec = EacRecord::new(retired_subject([2u8; 32]), "did:supernova:test", 1);
        assert!(rec.context.iter().any(|c| c == VC_CONTEXT_W3C));
        assert!(rec.credential_type.iter().any(|t| t == VC_TYPE_BASE));
        assert!(rec.credential_type.iter().any(|t| t == VC_TYPE_EAC));
        rec.validate_shape().expect("default envelope is well-formed");
    }

    #[test]
    fn unknown_id_is_rejected() {
        let reg = MockRegistry::new();
        let err = reg.verify_retirement([9u8; 32]).unwrap_err();
        assert!(matches!(err, RegistryError::UnknownRetirement(_)));
    }

    #[test]
    fn unretired_certificate_is_rejected() {
        let mut reg = MockRegistry::new();
        let id = [3u8; 32];
        let mut subject = retired_subject(id);
        subject.retired = false;
        reg.insert(EacRecord::new(subject, "did:supernova:test", 7));

        let err = reg.verify_retirement(id).unwrap_err();
        assert!(matches!(err, RegistryError::NotRetired(_)));
    }

    #[test]
    fn insert_retired_helper_resolves() {
        let mut reg = MockRegistry::new();
        let id = [4u8; 32];
        reg.insert_retired(id, RegistryId(2), 1_000, 55, EnergyType::Wind);
        let got = reg.verify_retirement(id).expect("seeded record resolves");
        assert_eq!(got.mwh_milli(), 1_000);
        assert_eq!(got.energy_type(), EnergyType::Wind);
        assert!(!reg.is_empty());
        assert_eq!(reg.len(), 1);
    }

    #[test]
    fn origin_adapter_is_unimplemented_stub() {
        let adapter = OriginRegistryAdapter::new("https://origin.example", RegistryId(2));
        let err = adapter.verify_retirement([0u8; 32]).unwrap_err();
        assert!(matches!(err, RegistryError::NotImplemented(_)));
    }

    #[test]
    fn registry_error_converts_into_green_error() {
        let err = RegistryError::UnknownRetirement("dead".to_string());
        let green: GreenError = err.into();
        assert!(matches!(green, GreenError::RegistryError(_)));
    }

    #[test]
    fn malformed_record_fails_shape_check() {
        let mut rec = EacRecord::new(retired_subject([5u8; 32]), "did:supernova:test", 1);
        rec.credential_type = vec![VC_TYPE_BASE.to_string()]; // drop EAC type
        let err = rec.validate_shape().unwrap_err();
        assert!(matches!(err, RegistryError::Malformed(_)));
    }

    #[test]
    fn record_json_roundtrip_uses_vc_field_names() {
        let rec = EacRecord::new(retired_subject([6u8; 32]), "did:supernova:test", 3);
        let json = serde_json::to_string(&rec).expect("serialize");
        assert!(json.contains("@context"));
        assert!(json.contains("credential_subject"));
        let back: EacRecord = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back, rec);
    }
}
