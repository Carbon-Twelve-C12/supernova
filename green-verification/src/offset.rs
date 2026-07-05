//! L6 carbon-offset retirement + provable-negative accounting (ADR-0002 §10).
//!
//! Mirrors the attestation quorum pattern for offset retirements: the treasury
//! buys and retires offsets (Gold Standard / Verra), an oracle quorum attests
//! the retirement, and the serial is nullified so it cannot be re-counted.
//! "Carbon-negative" then becomes a provable inequality:
//! `total_offsets_retired_milli >= emissions_estimate_milli`.
//!
//! All functions are pure; nothing is wired to consensus.

use std::collections::HashSet;

use serde::{Deserialize, Serialize};
use sha3::{Digest, Sha3_512};

use crate::attestation::OracleSig;
use crate::error::GreenError;
use crate::nullifier::NullifierSet;
use crate::oracle::Committee;
use crate::types::{OffsetStandard, OracleId, DOMAIN_TAG_OFFSET_V1};

/// An oracle-attested carbon-offset retirement (ADR-0002 §4, §10).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OffsetRetirement {
    /// Methodology standard (Gold Standard / Verra).
    pub standard: OffsetStandard,
    /// Unique retirement serial (nullifier key).
    pub serial: [u8; 32],
    /// Retired CO2, in milli-tonnes.
    pub tonnes_co2_milli: u64,
    /// `>= M` distinct ML-DSA oracle signatures over [`Self::digest`].
    pub quorum_sig: Vec<OracleSig>,
    /// Height at which the retirement was recorded.
    pub height: u64,
}

impl OffsetRetirement {
    /// Canonical SHA3-512 digest of the retirement, signed by the quorum.
    pub fn digest(&self) -> [u8; 64] {
        let mut h = Sha3_512::new();
        h.update(DOMAIN_TAG_OFFSET_V1);
        h.update([self.standard.as_u8()]);
        h.update(self.serial);
        h.update(self.tonnes_co2_milli.to_le_bytes());
        h.update(self.height.to_le_bytes());
        h.finalize().into()
    }

    /// Verify the offset retirement: `>= M` distinct valid committee signatures
    /// over the digest, and the serial has not already been retired.
    ///
    /// PURE with respect to consensus state except that, on success, the serial
    /// is inserted into `nullifier` (matching the attestation pattern where the
    /// caller owns nullifier mutation — here it is folded in because an
    /// unverified serial must never be nullified). On any failure the nullifier
    /// is left untouched.
    pub fn verify(
        &self,
        committee: &Committee,
        nullifier: &mut NullifierSet,
    ) -> Result<(), GreenError> {
        // Reject an already-retired serial before doing crypto work.
        if nullifier.contains_offset(&self.serial) {
            return Err(GreenError::OffsetSerialAlreadyUsed);
        }

        let digest = self.digest();
        let mut valid_signers: HashSet<OracleId> = HashSet::new();
        for osig in &self.quorum_sig {
            let pubkey = committee
                .public_key_for(&osig.oracle_id)
                .ok_or(GreenError::UnknownOracleKey)?;
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

        // Only nullify a fully verified retirement.
        nullifier.check_and_insert_offset(self.serial)?;
        Ok(())
    }
}

/// Net carbon position of the network (ADR-0002 §10).
///
/// Computed as `retired offsets − estimated emissions`. A non-negative
/// [`Self::net_milli`] means the network has retired at least as much carbon as
/// it is estimated to have emitted, i.e. it is provably carbon-negative.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct NetCarbonPosition {
    /// Cumulative retired offsets, in milli-tonnes CO2.
    pub total_offsets_retired_milli: u64,
    /// Estimated cumulative emissions, in milli-tonnes CO2.
    pub emissions_estimate_milli: u64,
    /// Signed net position `retired − emitted`, in milli-tonnes CO2.
    ///
    /// `i128` is used so the full `u64` range subtracts without overflow or
    /// wrap-around: positive ⇒ net carbon removed, negative ⇒ net emitted.
    pub net_milli: i128,
    /// `true` iff `net_milli >= 0` (retired ≥ emitted).
    pub is_carbon_negative: bool,
}

/// Compute the network's net-carbon position (ADR-0002 §10).
///
/// Pure comparison over consensus-tracked totals; performs the subtraction in
/// `i128` to avoid any overflow/underflow across the full `u64` input range.
pub fn net_carbon_position(
    total_offsets_retired_milli: u64,
    emissions_estimate_milli: u64,
) -> NetCarbonPosition {
    let net_milli = i128::from(total_offsets_retired_milli) - i128::from(emissions_estimate_milli);
    NetCarbonPosition {
        total_offsets_retired_milli,
        emissions_estimate_milli,
        net_milli,
        is_carbon_negative: net_milli >= 0,
    }
}

/// Provable carbon-negative accounting (ADR-0002 §10).
///
/// Returns `true` iff cumulative retired offsets meet or exceed the estimated
/// emissions. Thin boolean wrapper over [`net_carbon_position`].
pub fn is_provably_carbon_negative(
    total_offsets_retired_milli: u64,
    emissions_estimate_milli: u64,
) -> bool {
    net_carbon_position(total_offsets_retired_milli, emissions_estimate_milli).is_carbon_negative
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn carbon_negative_inequality() {
        assert!(is_provably_carbon_negative(1_000, 900));
        assert!(is_provably_carbon_negative(1_000, 1_000));
        assert!(!is_provably_carbon_negative(900, 1_000));
    }

    #[test]
    fn net_position_signed_delta() {
        // Retired exceeds emitted: positive net, carbon-negative.
        let pos = net_carbon_position(1_000, 900);
        assert_eq!(pos.net_milli, 100);
        assert!(pos.is_carbon_negative);

        // Exact break-even: zero net, still carbon-negative (>=).
        let even = net_carbon_position(1_000, 1_000);
        assert_eq!(even.net_milli, 0);
        assert!(even.is_carbon_negative);

        // Emitted exceeds retired: negative net, not carbon-negative.
        let neg = net_carbon_position(900, 1_000);
        assert_eq!(neg.net_milli, -100);
        assert!(!neg.is_carbon_negative);
    }

    #[test]
    fn net_position_no_overflow_at_u64_extremes() {
        // Full-range subtraction must not overflow/wrap in i128.
        let max_emit = net_carbon_position(0, u64::MAX);
        assert_eq!(max_emit.net_milli, -i128::from(u64::MAX));
        assert!(!max_emit.is_carbon_negative);

        let max_retire = net_carbon_position(u64::MAX, 0);
        assert_eq!(max_retire.net_milli, i128::from(u64::MAX));
        assert!(max_retire.is_carbon_negative);
    }

    #[test]
    fn net_position_agrees_with_bool_helper() {
        for (retired, emitted) in [(0, 0), (5, 4), (4, 5), (u64::MAX, u64::MAX)] {
            assert_eq!(
                net_carbon_position(retired, emitted).is_carbon_negative,
                is_provably_carbon_negative(retired, emitted)
            );
        }
    }

    #[test]
    fn empty_quorum_fails_below_threshold() {
        let committee = Committee::new(0, Vec::new(), 15);
        let mut nullifier = NullifierSet::new();
        let retirement = OffsetRetirement {
            standard: OffsetStandard::Verra,
            serial: [3u8; 32],
            tonnes_co2_milli: 100,
            quorum_sig: Vec::new(),
            height: 10,
        };
        assert_eq!(
            retirement.verify(&committee, &mut nullifier),
            Err(GreenError::QuorumNotMet { have: 0, need: 15 })
        );
        // Failed verification must not nullify the serial.
        assert!(!nullifier.contains_offset(&[3u8; 32]));
    }
}
