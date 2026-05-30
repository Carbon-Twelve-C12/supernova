//! Oracle stake bonds (carbon-negative Step 3).
//!
//! An environmental oracle's economic stake is a real UTXO — an *oracle bond* —
//! not a self-declared integer. A bond is an ordinary transaction output whose
//! `pub_key_script` is a tagged commitment that (a) marks the output as an
//! oracle bond and (b) binds it to the oracle's post-quantum public key, plus an
//! *unbonding height* before which the stake cannot be voluntarily withdrawn.
//!
//! This module defines the bond OUTPUT FORMAT only. Recognising bonds in the
//! UTXO set and deriving an oracle's live stake from them (registry, Step 4),
//! and the consensus spend rules — voluntary unbonding after the timelock and
//! slashing via fraud-proof transactions (Steps 4/7) — are layered on top.
//!
//! IMPORTANT: the script engine has no covenant execution (P2SH/P2WSH spends are
//! rejected fail-closed by `Transaction::verify_signature`), so a bond's spend
//! rules are enforced by **consensus transaction-validity rules**, never by an
//! in-script covenant. A bond output also does not match the standard key
//! commitment, so the ordinary signature path cannot spend it — exactly the
//! intended behaviour: a bond can only move under the dedicated bond-spend rules
//! that later steps add.

use crate::types::transaction::pubkey_commitment;

/// Domain tag (with embedded layout version) prefixing every oracle-bond output
/// script. Any change to the layout MUST bump the `_v1` suffix.
pub const ORACLE_BOND_TAG: &[u8] = b"SNOVA_ORACLE_BOND_v1";

/// Length of the oracle key commitment embedded in a bond (`SHA3-512(pk)[..32]`).
const COMMITMENT_LEN: usize = 32;
/// Length of the little-endian unbond-height suffix.
const HEIGHT_LEN: usize = 8;

/// Exact length of a well-formed oracle-bond output script.
pub const ORACLE_BOND_SCRIPT_LEN: usize = ORACLE_BOND_TAG.len() + COMMITMENT_LEN + HEIGHT_LEN;

/// Parsed terms of an oracle-bond output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OracleBondTerms {
    /// `SHA3-512(oracle_public_key)[..32]` — binds the bond to the oracle key
    /// (the same commitment scheme used by ordinary outputs).
    pub oracle_commitment: [u8; 32],
    /// Block height before which the bond may not be voluntarily withdrawn.
    pub unbond_height: u64,
}

/// Build the `pub_key_script` for an oracle bond locked to `oracle_public_key`
/// and withdrawable no earlier than `unbond_height`.
pub fn oracle_bond_script(oracle_public_key: &[u8], unbond_height: u64) -> Vec<u8> {
    let mut script = Vec::with_capacity(ORACLE_BOND_SCRIPT_LEN);
    script.extend_from_slice(ORACLE_BOND_TAG);
    script.extend_from_slice(&pubkey_commitment(oracle_public_key)); // 32 bytes
    script.extend_from_slice(&unbond_height.to_le_bytes());
    script
}

/// Parse an oracle-bond output script. Returns `None` for anything that is not a
/// well-formed bond (wrong tag or wrong length) — fail-closed.
pub fn parse_oracle_bond_script(script: &[u8]) -> Option<OracleBondTerms> {
    if script.len() != ORACLE_BOND_SCRIPT_LEN || !script.starts_with(ORACLE_BOND_TAG) {
        return None;
    }
    let body = &script[ORACLE_BOND_TAG.len()..];
    let mut oracle_commitment = [0u8; 32];
    oracle_commitment.copy_from_slice(&body[..COMMITMENT_LEN]);
    let mut height = [0u8; HEIGHT_LEN];
    height.copy_from_slice(&body[COMMITMENT_LEN..COMMITMENT_LEN + HEIGHT_LEN]);
    Some(OracleBondTerms {
        oracle_commitment,
        unbond_height: u64::from_le_bytes(height),
    })
}

/// True iff `script` is a well-formed oracle-bond output script.
pub fn is_oracle_bond_script(script: &[u8]) -> bool {
    parse_oracle_bond_script(script).is_some()
}

/// True iff the bond was created for `oracle_public_key`.
pub fn bond_belongs_to(terms: &OracleBondTerms, oracle_public_key: &[u8]) -> bool {
    terms.oracle_commitment.as_slice() == pubkey_commitment(oracle_public_key).as_slice()
}

/// Resolves an oracle stake bond from the live UTXO set (carbon-negative Step 4).
///
/// Implemented by the node over its UTXO database: a bond is simply an unspent
/// output whose `pub_key_script` parses as an [`OracleBondTerms`] committing to
/// the oracle's key. An oracle's economic weight is the value of its UNSPENT
/// bond, so a spent or voluntarily-withdrawn bond drops the oracle's stake to
/// zero — the on-chain link the previous self-declared `u64` stake lacked.
pub trait BondResolver {
    /// Value (in nova units) of the unspent bond at `(txid, vout)` IFF its
    /// output script is a well-formed oracle bond committing to
    /// `oracle_public_key`. Returns `None` if the bond is spent, missing,
    /// malformed, or bound to a different key (fail-closed).
    fn resolve_bond(&self, txid: &[u8; 32], vout: u32, oracle_public_key: &[u8]) -> Option<u64>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_parse_roundtrip() {
        let pk = vec![7u8; 100]; // bond format is key-agnostic; any bytes hash
        let script = oracle_bond_script(&pk, 123_456);
        assert_eq!(script.len(), ORACLE_BOND_SCRIPT_LEN);
        let terms = parse_oracle_bond_script(&script).expect("valid bond");
        assert_eq!(terms.unbond_height, 123_456);
        assert_eq!(
            terms.oracle_commitment.to_vec(),
            pubkey_commitment(&pk),
            "bond commits to SHA3-512(pubkey)[..32]"
        );
        assert!(is_oracle_bond_script(&script));
        assert!(bond_belongs_to(&terms, &pk));
    }

    #[test]
    fn different_keys_yield_different_bonds() {
        let a = oracle_bond_script(&[1u8; 64], 0);
        let b = oracle_bond_script(&[2u8; 64], 0);
        assert_ne!(a, b);
        let ta = parse_oracle_bond_script(&a).unwrap();
        assert!(!bond_belongs_to(&ta, &[2u8; 64]), "bond bound to its own key only");
    }

    #[test]
    fn malformed_scripts_are_rejected() {
        // Empty, short, wrong tag, and a normal 32-byte commitment must all fail.
        assert!(parse_oracle_bond_script(&[]).is_none());
        assert!(parse_oracle_bond_script(b"too short").is_none());
        assert!(parse_oracle_bond_script(&pubkey_commitment(&[9u8; 32])).is_none());

        let mut wrong_tag = oracle_bond_script(&[3u8; 64], 7);
        wrong_tag[0] ^= 0xff; // corrupt the tag
        assert!(parse_oracle_bond_script(&wrong_tag).is_none());

        let mut too_long = oracle_bond_script(&[3u8; 64], 7);
        too_long.push(0x00); // wrong length
        assert!(parse_oracle_bond_script(&too_long).is_none());
        assert!(!is_oracle_bond_script(&too_long));
    }
}
