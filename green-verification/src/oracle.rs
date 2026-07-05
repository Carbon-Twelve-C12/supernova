//! L1 permissionless staked oracle pool + per-epoch committee (ADR-0002 §5).
//!
//! All state transitions here are pure (either return a new value or mutate a
//! passed-in `&mut self`). There is **no networking**: the real OCR round,
//! registry fetch, and VRF sampling are deferred to later phases and marked
//! `TODO(phase-N)`. The committee sampler is a *deterministic* stake-weighted
//! stub so the rest of the pipeline is testable end-to-end.

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};
use sha3::{Digest, Sha3_512};
use supernova_core::crypto::{MLDSAPrivateKey, MLDSAPublicKey};

use crate::attestation::OracleSig;
use crate::error::GreenError;
use crate::types::{Did, OracleId, DEFAULT_THRESHOLD_M};

/// Minimum bond required to enter the permissionless oracle pool
/// (ADR-0002 §5, §17: `MIN_ORACLE_BOND`, governance-tunable / sim-calibrated).
///
/// EXPERIMENTAL placeholder value for the testnet prototype — the production
/// value is economic-simulation calibrated (ADR-0002 §17) and set by governance.
pub const MIN_ORACLE_BOND: u64 = 10_000;

/// A staked oracle's on-pool record (ADR-0002 §4).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OracleStake {
    /// Stable oracle identity.
    pub oracle_id: OracleId,
    /// ML-DSA public key used to sign attestations.
    pub ml_dsa_pubkey: MLDSAPublicKey,
    /// Optional W3C DID for Verifiable-Credential interop.
    pub did: Option<Did>,
    /// Bonded stake at risk (slashable).
    pub bond: u64,
    /// Height at which the oracle bonded.
    pub bonded_at: u64,
    /// If unbonding, the height unbonding was initiated (must exceed fraud window).
    pub unbonding_at: Option<u64>,
    /// Cumulative slashed amount.
    pub slashed: u64,
}

impl OracleStake {
    /// An oracle is eligible to serve iff it is bonded, not unbonding, and its
    /// live bond meets the minimum.
    pub fn is_eligible(&self, min_bond: u64) -> bool {
        self.unbonding_at.is_none() && self.bond >= min_bond && self.bond > 0
    }
}

/// The permissionless oracle registry (ADR-0002 §5).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct OracleRegistry {
    /// All known oracle stakes, keyed by id.
    pub by_id: HashMap<OracleId, OracleStake>,
    /// Minimum bond to enter the pool (governance-tunable; `MIN_ORACLE_BOND`).
    pub min_bond: u64,
}

impl OracleRegistry {
    /// Create an empty registry with the given minimum bond.
    pub fn new(min_bond: u64) -> Self {
        Self {
            by_id: HashMap::new(),
            min_bond,
        }
    }

    /// Bond a new oracle into the pool.
    ///
    /// PURE state transition. Rejects a bond below the minimum.
    pub fn bond(&mut self, stake: OracleStake) -> Result<(), GreenError> {
        if stake.bond < self.min_bond {
            return Err(GreenError::InsufficientBond {
                have: stake.bond,
                need: self.min_bond,
            });
        }
        self.by_id.insert(stake.oracle_id, stake);
        Ok(())
    }

    /// Begin unbonding an oracle at `height`.
    ///
    /// NOTE: the unbonding *delay* (> fraud-proof window) is enforced at the
    /// consensus/withdrawal layer in a later phase; here we only record intent.
    pub fn begin_unbond(&mut self, id: &OracleId, height: u64) -> Result<(), GreenError> {
        let stake = self.by_id.get_mut(id).ok_or(GreenError::NotBonded)?;
        stake.unbonding_at = Some(height);
        Ok(())
    }

    /// Slash an oracle by `amount` (saturating; never negative).
    ///
    /// Redistribution of the slashed amount (bounty to fraud-prover, remainder
    /// to treasury) is handled by the treasury/incentive layer — this only
    /// reduces the bond and records the slash.
    pub fn slash(&mut self, id: &OracleId, amount: u64) -> Result<u64, GreenError> {
        let stake = self.by_id.get_mut(id).ok_or(GreenError::NotBonded)?;
        let actual = amount.min(stake.bond);
        stake.bond -= actual;
        stake.slashed = stake.slashed.saturating_add(actual);
        Ok(actual)
    }

    /// Deterministically sample an `n`-member committee for `epoch` using the
    /// default [`DeterministicVrfSampler`] (ADR-0002 §5).
    ///
    /// Convenience wrapper over [`CommitteeSampler::sample`]. See that trait
    /// and [`DeterministicVrfSampler`] for the (non-secure) sampling rule and
    /// the `TODO(phase-2)` real-VRF note.
    pub fn sample_committee(&self, epoch: u64, n: usize, threshold_m: usize) -> Committee {
        DeterministicVrfSampler.sample(self, epoch, n, threshold_m)
    }
}

/// VRF-by-stake committee sampler (ADR-0002 §5).
///
/// Abstracts the per-epoch committee draw so the deterministic test sampler can
/// be swapped for a real VRF-by-stake implementation (with a diversity cap) in a
/// later, operator-sign-off phase without touching the rest of the pipeline.
pub trait CommitteeSampler {
    /// Sample an `n`-member committee for `epoch` from `registry`, requiring
    /// `threshold_m` signatures. Only eligible (bonded, non-unbonding,
    /// at-or-above `min_bond`) oracles are considered.
    fn sample(
        &self,
        registry: &OracleRegistry,
        epoch: u64,
        n: usize,
        threshold_m: usize,
    ) -> Committee;
}

/// Deterministic, reproducible stake-weighted committee sampler.
///
/// TODO(phase-2): replace with a real VRF-by-stake sampler with a diversity
/// cap. This stub ranks eligible oracles by a stake-weighted score
/// `bond + draw`, where `draw = SHA3-512("...committee.v1" || epoch ||
/// oracle_id)[0..8]`, so the draw is reproducible across nodes and exercisable
/// in tests. It is **NOT** a secure VRF (grindable, no unpredictability) and
/// MUST NOT be used in consensus.
#[derive(Debug, Clone, Copy, Default)]
pub struct DeterministicVrfSampler;

impl CommitteeSampler for DeterministicVrfSampler {
    fn sample(
        &self,
        registry: &OracleRegistry,
        epoch: u64,
        n: usize,
        threshold_m: usize,
    ) -> Committee {
        let mut scored: Vec<(OracleId, MLDSAPublicKey, u128)> = registry
            .by_id
            .values()
            .filter(|s| s.is_eligible(registry.min_bond))
            .map(|s| {
                let mut h = Sha3_512::new();
                h.update(b"supernova.green.committee.v1");
                h.update(epoch.to_le_bytes());
                h.update(s.oracle_id.0);
                let out: [u8; 64] = h.finalize().into();
                // Take the first 8 bytes as a pseudo-random draw in [0, 2^64).
                let draw = u64::from_le_bytes(out[0..8].try_into().unwrap_or([0u8; 8]));
                // Stake-weighted score: higher bond and higher draw rank higher.
                // Deterministic and testable; NOT a secure VRF.
                let score = (s.bond as u128).saturating_add(draw as u128);
                (s.oracle_id, s.ml_dsa_pubkey.clone(), score)
            })
            .collect();

        // Highest score first; tie-break by oracle_id for full determinism.
        scored.sort_by(|a, b| b.2.cmp(&a.2).then_with(|| a.0.cmp(&b.0)));
        scored.truncate(n);

        let members = scored
            .into_iter()
            .map(|(id, pk, _)| (id, pk))
            .collect::<Vec<_>>();

        Committee {
            epoch,
            members,
            threshold_m,
        }
    }
}

/// A per-epoch committee of oracle keys (ADR-0002 §5).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Committee {
    /// The epoch this committee serves.
    pub epoch: u64,
    /// `(oracle_id, ml_dsa_pubkey)` members, sampled from the pool.
    pub members: Vec<(OracleId, MLDSAPublicKey)>,
    /// Signature threshold `M` required to accept an attestation.
    pub threshold_m: usize,
}

impl Committee {
    /// Construct a committee directly (useful for tests / fixtures).
    pub fn new(epoch: u64, members: Vec<(OracleId, MLDSAPublicKey)>, threshold_m: usize) -> Self {
        Self {
            epoch,
            members,
            threshold_m,
        }
    }

    /// Look up a member's ML-DSA public key by oracle id, if registered.
    pub fn public_key_for(&self, id: &OracleId) -> Option<&MLDSAPublicKey> {
        self.members
            .iter()
            .find(|(mid, _)| mid == id)
            .map(|(_, pk)| pk)
    }

    /// Number of committee members.
    pub fn size(&self) -> usize {
        self.members.len()
    }
}

impl Default for Committee {
    fn default() -> Self {
        Self {
            epoch: 0,
            members: Vec::new(),
            threshold_m: DEFAULT_THRESHOLD_M,
        }
    }
}

// ---------------------------------------------------------------------------
// OCR-style report assembly (ADR-0002 §5)
// ---------------------------------------------------------------------------

/// Produce one oracle's ML-DSA signature over a canonical attestation digest.
///
/// Models one committee member's step in an OCR-style round: after fetching and
/// validating the EAC retirement (network I/O is deferred — `TODO(phase-2)`),
/// the member signs the digest with its ML-DSA key. Signing is delegated
/// verbatim to [`supernova_core::crypto`] — no signature primitive is defined
/// here.
pub fn oracle_sign(
    oracle_id: OracleId,
    signing_key: &MLDSAPrivateKey,
    digest: &[u8; 64],
) -> Result<OracleSig, GreenError> {
    let sig = signing_key.sign(digest)?;
    Ok(OracleSig { oracle_id, sig })
}

/// A finalized OCR report: `>= M` distinct valid oracle signatures over one
/// digest, ready to be attached to a [`crate::attestation::GreenAttestation`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OracleReport {
    /// Epoch whose committee produced the report.
    pub committee_epoch: u64,
    /// The canonical digest every signature covers.
    pub digest: [u8; 64],
    /// The collected distinct, committee-verified signatures (`>= M`).
    pub signatures: Vec<OracleSig>,
}

/// Accumulates oracle signatures for a fixed digest, enforcing that every
/// contribution is from a *distinct* registered committee member with a valid
/// ML-DSA signature, and that the finalized report carries `>= M` of them
/// (ADR-0002 §5).
///
/// PURE: holds only the in-progress signature set; no I/O or global state.
#[derive(Debug, Clone)]
pub struct ReportBuilder {
    committee: Committee,
    digest: [u8; 64],
    sigs: Vec<OracleSig>,
    seen: HashSet<OracleId>,
}

impl ReportBuilder {
    /// Begin assembling a report for `committee` over `digest`.
    pub fn new(committee: Committee, digest: [u8; 64]) -> Self {
        Self {
            committee,
            digest,
            sigs: Vec::new(),
            seen: HashSet::new(),
        }
    }

    /// Number of distinct valid signatures collected so far.
    pub fn collected(&self) -> usize {
        self.sigs.len()
    }

    /// Whether the quorum threshold `M` has been reached.
    pub fn quorum_reached(&self) -> bool {
        self.sigs.len() >= self.committee.threshold_m
    }

    /// Add a pre-computed oracle signature.
    ///
    /// Rejects a signer that is not a registered committee member
    /// ([`GreenError::UnknownOracleKey`]), a duplicate signer
    /// ([`GreenError::DuplicateSigner`]), or a signature that fails ML-DSA
    /// verification over the digest ([`GreenError::InvalidSignature`]).
    pub fn add_signature(&mut self, osig: OracleSig) -> Result<(), GreenError> {
        let pubkey = self
            .committee
            .public_key_for(&osig.oracle_id)
            .ok_or(GreenError::UnknownOracleKey)?;

        if self.seen.contains(&osig.oracle_id) {
            return Err(GreenError::DuplicateSigner);
        }

        match pubkey.verify(&self.digest, &osig.sig) {
            Ok(true) => {}
            Ok(false) => return Err(GreenError::InvalidSignature),
            Err(e) => return Err(GreenError::Quantum(e)),
        }

        self.seen.insert(osig.oracle_id);
        self.sigs.push(osig);
        Ok(())
    }

    /// Sign the digest with a committee member's key and add the signature.
    ///
    /// Convenience over [`oracle_sign`] + [`Self::add_signature`].
    pub fn sign_and_add(
        &mut self,
        oracle_id: OracleId,
        signing_key: &MLDSAPrivateKey,
    ) -> Result<(), GreenError> {
        let osig = oracle_sign(oracle_id, signing_key, &self.digest)?;
        self.add_signature(osig)
    }

    /// Finalize into an [`OracleReport`] iff `>= M` distinct valid signatures
    /// were collected; otherwise [`GreenError::QuorumNotMet`].
    pub fn finalize(self) -> Result<OracleReport, GreenError> {
        let have = self.sigs.len();
        let need = self.committee.threshold_m;
        if have < need {
            return Err(GreenError::QuorumNotMet { have, need });
        }
        Ok(OracleReport {
            committee_epoch: self.committee.epoch,
            digest: self.digest,
            signatures: self.sigs,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use supernova_core::crypto::MLDSASecurityLevel;

    /// Deterministic oracle id from a small index (test fixture only).
    fn oid(i: u8) -> OracleId {
        let mut b = [0u8; 32];
        b[0] = i;
        b[31] = 0xAA;
        OracleId(b)
    }

    /// Generate an ML-DSA-65 (Level 3) keypair for tests.
    fn gen_key() -> MLDSAPrivateKey {
        let mut rng = rand::rngs::OsRng;
        MLDSAPrivateKey::generate(&mut rng, MLDSASecurityLevel::Level3)
            .expect("ML-DSA keygen should succeed")
    }

    /// Build a registry of `count` bonded oracles, returning the registry and
    /// each oracle's `(id, signing_key)`.
    fn bonded_pool(count: u8, bond: u64) -> (OracleRegistry, Vec<(OracleId, MLDSAPrivateKey)>) {
        let mut reg = OracleRegistry::new(MIN_ORACLE_BOND);
        let mut keys = Vec::new();
        for i in 0..count {
            let sk = gen_key();
            let id = oid(i);
            reg.bond(OracleStake {
                oracle_id: id,
                ml_dsa_pubkey: sk.public_key(),
                did: None,
                bond,
                bonded_at: 0,
                unbonding_at: None,
                slashed: 0,
            })
            .expect("bond above minimum should succeed");
            keys.push((id, sk));
        }
        (reg, keys)
    }

    #[test]
    fn bond_below_minimum_rejected() {
        let mut reg = OracleRegistry::new(MIN_ORACLE_BOND);
        let sk = gen_key();
        let err = reg
            .bond(OracleStake {
                oracle_id: oid(1),
                ml_dsa_pubkey: sk.public_key(),
                did: None,
                bond: MIN_ORACLE_BOND - 1,
                bonded_at: 0,
                unbonding_at: None,
                slashed: 0,
            })
            .unwrap_err();
        assert_eq!(
            err,
            GreenError::InsufficientBond {
                have: MIN_ORACLE_BOND - 1,
                need: MIN_ORACLE_BOND,
            }
        );
    }

    #[test]
    fn committee_sampling_n21_m15() {
        // 30 bonded oracles; sample the canonical N=21 / M=15 committee.
        let (reg, _keys) = bonded_pool(30, MIN_ORACLE_BOND * 2);
        let committee = reg.sample_committee(7, 21, 15);
        assert_eq!(committee.size(), 21);
        assert_eq!(committee.threshold_m, 15);
        assert_eq!(committee.epoch, 7);

        // Deterministic: same inputs => identical committee.
        let again = reg.sample_committee(7, 21, 15);
        assert_eq!(committee, again);

        // All sampled members are distinct.
        let mut ids: Vec<_> = committee.members.iter().map(|(id, _)| *id).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), 21);
    }

    #[test]
    fn deterministic_sampler_trait_matches_wrapper() {
        let (reg, _keys) = bonded_pool(25, MIN_ORACLE_BOND);
        let via_trait = DeterministicVrfSampler.sample(&reg, 3, 21, 15);
        let via_wrapper = reg.sample_committee(3, 21, 15);
        assert_eq!(via_trait, via_wrapper);
    }

    #[test]
    fn unbonding_oracle_excluded_from_committee() {
        let (mut reg, keys) = bonded_pool(21, MIN_ORACLE_BOND);
        // Unbond one; committee can then only reach 20 members.
        reg.begin_unbond(&keys[0].0, 100).unwrap();
        let committee = reg.sample_committee(1, 21, 15);
        assert_eq!(committee.size(), 20);
        assert!(committee.public_key_for(&keys[0].0).is_none());
    }

    #[test]
    fn slashing_reduces_bond_and_eligibility() {
        let (mut reg, keys) = bonded_pool(1, MIN_ORACLE_BOND + 500);
        let id = keys[0].0;

        // Slash part of the bond.
        let slashed = reg.slash(&id, 500).unwrap();
        assert_eq!(slashed, 500);
        let s = &reg.by_id[&id];
        assert_eq!(s.bond, MIN_ORACLE_BOND);
        assert_eq!(s.slashed, 500);
        assert!(s.is_eligible(MIN_ORACLE_BOND));

        // Slash below the minimum: bond drops, oracle becomes ineligible.
        let slashed2 = reg.slash(&id, 1).unwrap();
        assert_eq!(slashed2, 1);
        let s = &reg.by_id[&id];
        assert_eq!(s.bond, MIN_ORACLE_BOND - 1);
        assert_eq!(s.slashed, 501);
        assert!(!s.is_eligible(MIN_ORACLE_BOND));

        // Slash is saturating: cannot go negative.
        let over = reg.slash(&id, u64::MAX).unwrap();
        assert_eq!(over, MIN_ORACLE_BOND - 1);
        assert_eq!(reg.by_id[&id].bond, 0);
    }

    #[test]
    fn report_needs_at_least_m_signatures() {
        let (reg, keys) = bonded_pool(21, MIN_ORACLE_BOND);
        let committee = reg.sample_committee(1, 21, 15);
        let digest = [0x11u8; 64];

        // Only committee members can sign; sign with the first 14 members.
        let key_for = |id: &OracleId| {
            keys.iter()
                .find(|(kid, _)| kid == id)
                .map(|(_, sk)| sk)
                .unwrap()
        };
        let member_ids: Vec<OracleId> = committee.members.iter().map(|(id, _)| *id).collect();

        // 14 signatures — one short of M=15.
        let mut b = ReportBuilder::new(committee.clone(), digest);
        for id in member_ids.iter().take(14) {
            b.sign_and_add(*id, key_for(id)).unwrap();
        }
        assert_eq!(b.collected(), 14);
        assert!(!b.quorum_reached());
        let err = b.finalize().unwrap_err();
        assert_eq!(err, GreenError::QuorumNotMet { have: 14, need: 15 });

        // 15 signatures — quorum reached, finalize succeeds.
        let mut b = ReportBuilder::new(committee.clone(), digest);
        for id in member_ids.iter().take(15) {
            b.sign_and_add(*id, key_for(id)).unwrap();
        }
        assert!(b.quorum_reached());
        let report = b.finalize().unwrap();
        assert_eq!(report.signatures.len(), 15);
        assert_eq!(report.committee_epoch, committee.epoch);
        assert_eq!(report.digest, digest);
    }

    #[test]
    fn report_rejects_duplicate_and_unknown_signers() {
        let (reg, keys) = bonded_pool(21, MIN_ORACLE_BOND);
        let committee = reg.sample_committee(1, 21, 15);
        let digest = [0x22u8; 64];
        let member = committee.members[0].0;
        let member_key = keys.iter().find(|(id, _)| *id == member).unwrap().1.clone();

        let mut b = ReportBuilder::new(committee.clone(), digest);
        b.sign_and_add(member, &member_key).unwrap();

        // Same oracle signing twice => duplicate rejected.
        let err = b.sign_and_add(member, &member_key).unwrap_err();
        assert_eq!(err, GreenError::DuplicateSigner);

        // A non-committee oracle => unknown key rejected.
        let outsider = gen_key();
        let outsider_id = oid(200);
        let osig = oracle_sign(outsider_id, &outsider, &digest).unwrap();
        let err = b.add_signature(osig).unwrap_err();
        assert_eq!(err, GreenError::UnknownOracleKey);
    }

    #[test]
    fn report_rejects_wrong_digest_signature() {
        let (reg, keys) = bonded_pool(21, MIN_ORACLE_BOND);
        let committee = reg.sample_committee(1, 21, 15);
        let member = committee.members[0].0;
        let member_key = keys.iter().find(|(id, _)| *id == member).unwrap().1.clone();

        // Sign a different digest than the builder is collecting for.
        let signed_digest = [0x33u8; 64];
        let builder_digest = [0x44u8; 64];
        let osig = oracle_sign(member, &member_key, &signed_digest).unwrap();

        let mut b = ReportBuilder::new(committee, builder_digest);
        let err = b.add_signature(osig).unwrap_err();
        assert_eq!(err, GreenError::InvalidSignature);
    }
}
