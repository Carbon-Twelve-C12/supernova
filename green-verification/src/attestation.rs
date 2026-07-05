//! L2 attestation object and L3 deterministic verification (ADR-0002 §4, §7).
//!
//! Verification here is a **pure function**: it takes the attestation, the
//! committee, the current height, and bound parameters, and returns
//! `Ok(())` iff the attestation carries `>= M` distinct valid ML-DSA
//! signatures from registered committee keys and passes freshness / bound
//! checks. It does **not** mutate the nullifier set — nullifier insertion is
//! the caller's responsibility (see [`crate::nullifier`]) so that this
//! function stays side-effect free and easy to test.

use std::collections::HashSet;

use serde::{Deserialize, Serialize};
use sha3::{Digest, Sha3_512};
use supernova_core::crypto::MLDSASignature;

use crate::error::GreenError;
use crate::oracle::Committee;
use crate::types::{
    EnergyType, MeterSignature, MinerAddr, OracleId, RegistryId, TimeMatchProof, DOMAIN_TAG_V1,
};

/// A single oracle's signature over the attestation digest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OracleSig {
    /// Which committee oracle produced this signature.
    pub oracle_id: OracleId,
    /// The ML-DSA (Dilithium3) detached signature over the canonical digest.
    pub sig: MLDSASignature,
}

/// Bounds and windows applied during L3 verification (ADR-0002 §7).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VerifyParams {
    /// Inclusive lower bound for an acceptable vintage epoch.
    pub vintage_lo: u64,
    /// Inclusive upper bound for an acceptable vintage epoch.
    pub vintage_hi: u64,
    /// Conservative maximum claimable MWh (in milli-MWh) for this block,
    /// e.g. `min(attested_capacity, hashrate->energy_upper_bound)`.
    pub max_claimable_mwh_milli: u64,
}

/// A trust-minimized renewable-energy attestation (ADR-0002 §4).
///
/// `signatures` must carry `>= M` distinct valid ML-DSA signatures from the
/// `committee_epoch` committee for the attestation to be accepted.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GreenAttestation {
    /// Schema version (1 = attested-REC; 2 = time-matched + metered).
    pub version: u16,
    /// Retired EAC id — the nullifier key that prevents double-claims.
    pub eac_retirement_id: [u8; 32],
    /// Registry that issued/retired the certificate.
    pub registry_id: RegistryId,
    /// Verified clean energy, in milli-MWh (1 MWh = 1000).
    pub mwh_milli: u64,
    /// Vintage epoch of the generation.
    pub vintage_epoch: u64,
    /// Category of generation.
    pub energy_type: EnergyType,
    /// Miner payout address the rebate is bound to (== coinbase payout in L3).
    pub miner_pqc_addr: MinerAddr,
    /// Anti-replay nonce.
    pub nonce: [u8; 32],
    /// Height at/after which the attestation is no longer valid.
    pub expiry_height: u64,
    /// v2: optional 24/7 time-match proof (not verified in v1).
    pub time_match: Option<TimeMatchProof>,
    /// v2: optional signed meter reading (not verified in v1).
    pub meter_sig: Option<MeterSignature>,
    /// Epoch whose committee produced the signatures.
    pub committee_epoch: u64,
    /// `>= M` distinct ML-DSA oracle signatures over [`Self::canonical_digest`].
    pub signatures: Vec<OracleSig>,
}

impl GreenAttestation {
    /// Compute the canonical SHA3-512 digest signed by oracles (ADR-0002 §4).
    ///
    /// Domain-separated, fixed field order, little-endian integer encoding.
    /// The optional v2 fields are folded in with an explicit presence byte so
    /// that "absent" and "present-but-empty" never collide.
    pub fn canonical_digest(&self) -> [u8; 64] {
        let mut h = Sha3_512::new();
        h.update(DOMAIN_TAG_V1);
        h.update(self.version.to_le_bytes());
        h.update(self.eac_retirement_id);
        h.update(self.registry_id.0.to_le_bytes());
        h.update(self.mwh_milli.to_le_bytes());
        h.update(self.vintage_epoch.to_le_bytes());
        h.update([self.energy_type.as_u8()]);
        h.update(self.miner_pqc_addr.0);
        h.update(self.nonce);
        h.update(self.expiry_height.to_le_bytes());
        h.update(self.committee_epoch.to_le_bytes());

        // v2 upgrade fields: presence byte + bytes, so schema can bump without
        // ambiguity in the pre-image.
        match &self.time_match {
            Some(tm) => {
                h.update([1u8]);
                h.update((tm.proof.len() as u64).to_le_bytes());
                h.update(&tm.proof);
            }
            None => h.update([0u8]),
        }
        match &self.meter_sig {
            Some(ms) => {
                h.update([1u8]);
                h.update((ms.sig.len() as u64).to_le_bytes());
                h.update(&ms.sig);
            }
            None => h.update([0u8]),
        }

        h.finalize().into()
    }

    /// L3 deterministic verification (ADR-0002 §7), minus nullifier insertion.
    ///
    /// Accepts iff:
    /// 1. every signer is a registered member of `committee`,
    /// 2. every signature verifies (ML-DSA) over the canonical digest,
    /// 3. there are `>= committee.threshold_m` **distinct** valid signers,
    /// 4. `expiry_height > height` (not yet expired),
    /// 5. `vintage_epoch` is within `[params.vintage_lo, params.vintage_hi]`,
    /// 6. `mwh_milli <= params.max_claimable_mwh_milli`.
    ///
    /// PURE: no I/O, no global state, no mutation of the nullifier set. The
    /// caller must separately reject a reused `eac_retirement_id`.
    pub fn verify(
        &self,
        committee: &Committee,
        height: u64,
        params: &VerifyParams,
    ) -> Result<(), GreenError> {
        // Freshness (cheap checks first).
        if self.expiry_height <= height {
            return Err(GreenError::ExpiredAttestation {
                expiry: self.expiry_height,
                height,
            });
        }
        if self.vintage_epoch < params.vintage_lo || self.vintage_epoch > params.vintage_hi {
            return Err(GreenError::VintageOutOfWindow {
                vintage: self.vintage_epoch,
                lo: params.vintage_lo,
                hi: params.vintage_hi,
            });
        }
        if self.mwh_milli > params.max_claimable_mwh_milli {
            return Err(GreenError::MwhExceedsBound {
                claimed: self.mwh_milli,
                bound: params.max_claimable_mwh_milli,
            });
        }

        let digest = self.canonical_digest();

        // Count DISTINCT valid signers. A HashSet of oracle_ids defends against
        // the over-count attack where one oracle submits M copies.
        let mut valid_signers: HashSet<OracleId> = HashSet::new();
        for osig in &self.signatures {
            let pubkey = committee
                .public_key_for(&osig.oracle_id)
                .ok_or(GreenError::UnknownOracleKey)?;

            // ML-DSA verify: reject on Ok(false) or Err.
            match pubkey.verify(&digest, &osig.sig) {
                Ok(true) => {
                    valid_signers.insert(osig.oracle_id);
                }
                Ok(false) => return Err(GreenError::InvalidSignature),
                Err(e) => return Err(GreenError::Quantum(e)),
            }
        }

        let have = valid_signers.len();
        if have < committee.threshold_m {
            return Err(GreenError::QuorumNotMet {
                have,
                need: committee.threshold_m,
            });
        }

        Ok(())
    }
}
