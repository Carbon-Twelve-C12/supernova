//! Proof-of-work chain-work accounting (audit Critical #2).
//!
//! Nakamoto fork choice must follow *cumulative expended proof-of-work*, not a
//! function of block-hash bytes. The previous implementation derived a block's
//! "work" from the first four bytes of its own hash, letting an attacker grind a
//! tiny hash prefix to manufacture astronomically more "work" than an honestly
//! mined block — a complete break of consensus.
//!
//! This module provides a correct work primitive over a *vetted* 256-bit
//! integer (the `uint` crate, used by Parity/Substrate), instead of home-rolled
//! arithmetic. A block's work is Bitcoin's `GetBlockProof`:
//!
//! ```text
//! work(target) = 2^256 / (target + 1)
//! ```
//!
//! Lower target ⇒ more work, monotonically. Because a block that exists had to
//! find a hash ≤ target, its expected hash count is ≈ `work(target)`, so work is
//! now tied to real proof-of-work and is independent of the resulting hash bytes.

use uint::construct_uint;

construct_uint! {
    /// 256-bit unsigned integer for cumulative proof-of-work ("chain work").
    pub struct Work(4);
}

/// Proof-of-work contributed by a block whose target is `target_le` — 32 bytes,
/// **little-endian** (exactly the layout `BlockHeader::target()` produces and
/// `BlockHeader::meets_target` compares the hash against). Computing work from
/// the same target the PoW was checked against keeps work consistent with the
/// hashing actually performed.
///
/// Returns `2^256 / (target + 1)`, evaluated as `(!target / (target + 1)) + 1`
/// to avoid 257-bit overflow (this is Bitcoin Core's exact `GetBlockProof`).
///
/// Returns `None` when `target_le` is zero — a malformed/over-range `bits` field
/// decodes to a zero target, which is unmineable; treating it as *infinite* work
/// would hand an attacker the chain, so the caller MUST reject such a block
/// rather than award it work.
pub fn work_from_target(target_le: &[u8; 32]) -> Option<Work> {
    let target = Work::from_little_endian(target_le);
    if target.is_zero() {
        return None;
    }
    let not_target = !target; // 2^256 - 1 - target
    if not_target.is_zero() {
        // target == 2^256 - 1, so work == 2^256 / 2^256 == 1.
        return Some(Work::one());
    }
    // target < 2^256 - 1 here, so `target + 1` cannot overflow, and the quotient
    // (< 2^256 - 1) plus one cannot overflow either.
    Some(not_target / (target + Work::one()) + Work::one())
}

/// Saturating projection of 256-bit work into the legacy u64 `total_difficulty`
/// metric.
///
/// NON-AUTHORITATIVE: fork choice compares the full [`Work`] value via the
/// `chain_work` map; this u64 exists only for display/metadata and must never
/// gate consensus.
pub fn work_to_u64_saturating(work: Work) -> u64 {
    if work > Work::from(u64::MAX) {
        u64::MAX
    } else {
        work.low_u64()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a 32-byte little-endian target from a small u128 value.
    fn target_le(value: u128) -> [u8; 32] {
        let mut out = [0u8; 32];
        out[..16].copy_from_slice(&value.to_le_bytes());
        out
    }

    #[test]
    fn zero_target_is_invalid() {
        assert_eq!(work_from_target(&[0u8; 32]), None);
    }

    #[test]
    fn max_target_is_minimal_work() {
        assert_eq!(work_from_target(&[0xffu8; 32]), Some(Work::one()));
    }

    #[test]
    fn lower_target_yields_more_work() {
        // target 1 (hardest representable here) must out-work target 2, 3, ...
        let w1 = work_from_target(&target_le(1)).unwrap();
        let w2 = work_from_target(&target_le(2)).unwrap();
        let w3 = work_from_target(&target_le(0xffff)).unwrap();
        assert!(w1 > w2, "target 1 must be more work than target 2");
        assert!(w2 > w3, "target 2 must be more work than target 0xffff");
    }

    #[test]
    fn work_matches_get_block_proof_formula() {
        // 2^256 / (1 + 1) == 2^255.
        let w = work_from_target(&target_le(1)).unwrap();
        let expected = Work::one() << 255;
        assert_eq!(w, expected);
    }

    #[test]
    fn cumulative_work_adds_and_orders_correctly() {
        // Two hard blocks must outweigh three easy blocks here.
        let hard = work_from_target(&target_le(4)).unwrap();
        let easy = work_from_target(&target_le(0x0010_0000)).unwrap();
        let two_hard = hard.saturating_add(hard);
        let three_easy = easy.saturating_add(easy).saturating_add(easy);
        assert!(two_hard > three_easy);
    }

    #[test]
    fn u64_projection_saturates() {
        // 2^255 work is far beyond u64 and must clamp.
        let big = work_from_target(&target_le(1)).unwrap();
        assert_eq!(work_to_u64_saturating(big), u64::MAX);
        // A target near 2^256 gives tiny work that fits in u64.
        let small = work_from_target(&[0xffu8; 32]).unwrap();
        assert_eq!(work_to_u64_saturating(small), 1);
    }
}
