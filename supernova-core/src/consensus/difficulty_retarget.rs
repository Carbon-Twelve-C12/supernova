//! Deterministic integer difficulty retargeting (#2.2).
//!
//! A block's difficulty is a compact ("nBits") encoding of a 256-bit target; the
//! block is valid only if its hash is `<=` that target. This module is the
//! SINGLE, INTEGER-ONLY source of truth for "what `bits` does the chain require
//! at a given height" — shared by the validator (block accept) and the miner so
//! they can never disagree.
//!
//! There is **no floating point anywhere on this path**. `f64` rounds
//! differently across platforms (x87 vs SSE, fma contraction), which would fork
//! the chain; the legacy `f64` engines must not be reachable from consensus.
//!
//! The retarget is Bitcoin's rule: every `interval` blocks, scale the target by
//! the ratio of the time the period actually took to the time it should have
//! taken, clamped to a 4x move per period, and never easier than the network's
//! `pow_limit`. The scaling multiply (`old_target * timespan`) can exceed 256
//! bits when the old target is near the (large) `pow_limit`, so the arithmetic
//! is done in a 512-bit intermediate — Bitcoin Core gets away with 256 bits only
//! because its `powLimit` is `2^224` (ample headroom); ours is larger.

use uint::construct_uint;

construct_uint! {
    /// 256-bit difficulty target.
    pub struct Target256(4);
}
construct_uint! {
    /// 512-bit intermediate so `old_target * clamped_timespan` never overflows.
    struct Wide512(8);
}

/// Per-network retarget parameters — the one place block-time, interval, and the
/// minimum-difficulty floor are defined, so miner and validator share them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RetargetParams {
    /// Target seconds per block.
    pub target_block_time: u64,
    /// Blocks per difficulty period.
    pub interval: u64,
    /// Easiest permitted compact target (the maximum target / minimum difficulty).
    pub pow_limit_bits: u32,
}

impl RetargetParams {
    /// Mainnet: 150 s blocks, 2016-block periods, `0x1e0fffff` floor.
    pub const fn mainnet() -> Self {
        Self { target_block_time: 150, interval: 2016, pow_limit_bits: 0x1e0f_ffff }
    }
    /// Testnet: identical cadence/floor to mainnet today.
    pub const fn testnet() -> Self {
        Self { target_block_time: 150, interval: 2016, pow_limit_bits: 0x1e0f_ffff }
    }
    /// Regtest: fast 30 s blocks, short 144-block periods, easy `0x207fffff`
    /// floor (matches the storage tests / current genesis bits).
    pub const fn regtest() -> Self {
        Self { target_block_time: 30, interval: 144, pow_limit_bits: 0x207f_ffff }
    }

    /// Total target time for one period (`block_time * interval`).
    fn target_timespan(&self) -> u64 {
        self.target_block_time.saturating_mul(self.interval)
    }
}

/// Decode a compact `bits` field into its 256-bit target, using the SAME layout
/// as `BlockHeader::target()` / `meets_target` (`coefficient * 256^(exponent-3)`,
/// little-endian). Over-range exponents (`> 32`) decode to zero, exactly as the
/// header decoder does — a zero target is unmineable and is rejected upstream.
pub fn decode_target(bits: u32) -> Target256 {
    let exponent = (bits >> 24) & 0xff;
    let coefficient = Target256::from((bits & 0x00ff_ffff) as u64);
    if exponent <= 3 {
        coefficient >> ((8 * (3 - exponent)) as usize)
    } else if exponent <= 32 {
        coefficient << ((8 * (exponent - 3)) as usize)
    } else {
        Target256::zero()
    }
}

/// Encode a 256-bit target into its CANONICAL compact `bits` form (Bitcoin's
/// `GetCompact`): a 3-byte mantissa in `[0x008000, 0x7fffff]` with the
/// `0x00800000` sign bit forced clear by bumping the exponent. Being canonical
/// is what lets the equality gate `block.bits == required_bits` work — miner and
/// validator independently compute the identical compact for the same target.
pub fn target_to_compact(target: Target256) -> u32 {
    let nbits = target.bits();
    let mut size = ((nbits + 7) / 8) as u32;
    let mut compact: u32 = if size <= 3 {
        (target.low_u64() << (8 * (3 - size))) as u32
    } else {
        (target >> ((8 * (size as usize - 3)) as usize)).low_u64() as u32
    };
    // If the mantissa's high bit is set it would read as the sign bit; shift the
    // mantissa down a byte and widen the exponent to keep the value positive.
    if compact & 0x0080_0000 != 0 {
        compact >>= 8;
        size += 1;
    }
    compact | (size << 24)
}

/// True iff `bits` decodes to a mineable target no easier than the network floor:
/// non-zero and `<= pow_limit` target. The accept path rejects anything else.
pub fn target_within_limit(bits: u32, params: &RetargetParams) -> bool {
    let target = decode_target(bits);
    !target.is_zero() && target <= decode_target(params.pow_limit_bits)
}

fn widen(x: &Target256) -> Wide512 {
    let mut le = [0u8; 64];
    x.to_little_endian(&mut le[..32]);
    Wide512::from_little_endian(&le)
}

fn narrow(x: &Wide512) -> Target256 {
    // Only called after flooring to `pow_limit`, so the value fits in 256 bits;
    // the high half is zero.
    let mut le = [0u8; 64];
    x.to_little_endian(&mut le);
    Target256::from_little_endian(&le[..32])
}

/// Compute the new compact target after a period, given the previous target's
/// `bits` and how long the period ACTUALLY took. Deterministic integer math:
/// clamp the timespan to `[target/4, target*4]`, scale the target by
/// `clamped / target_timespan` in 512-bit space, floor to `pow_limit`, re-encode.
fn retarget(prev_bits: u32, actual_timespan: u64, params: &RetargetParams) -> u32 {
    let target_timespan = params.target_timespan();
    if target_timespan == 0 {
        return prev_bits; // degenerate params — never on a real network
    }
    let clamped = actual_timespan.clamp(target_timespan / 4, target_timespan * 4);

    let old = decode_target(prev_bits);
    let pow_limit = decode_target(params.pow_limit_bits);

    // old * clamped can exceed 256 bits near pow_limit → do it in 512 bits.
    let mut scaled = widen(&old) * Wide512::from(clamped) / Wide512::from(target_timespan);
    let pow_limit_wide = widen(&pow_limit);
    if scaled > pow_limit_wide {
        scaled = pow_limit_wide; // never easier than the floor
    }
    target_to_compact(narrow(&scaled))
}

/// The compact `bits` the chain REQUIRES for the block at `height`, whose parent
/// has `prev_bits`.
///
/// Off a retarget boundary the difficulty is unchanged (`prev_bits`). At a
/// boundary (`height % interval == 0`, `height > 0`), `boundary_timestamps` must
/// be `Some((first_ts, last_ts))` — the timestamps of the period's FIRST block
/// (`height - interval`) and LAST block (`height - 1`), gathered by the caller by
/// walking parent links along the BLOCK'S OWN chain (never the canonical height
/// index, which would use the wrong chain for a fork).
///
/// `height` must be the trustworthy DERIVED height (parent-stamped), not the
/// wire value.
pub fn required_bits(
    height: u64,
    prev_bits: u32,
    boundary_timestamps: Option<(u64, u64)>,
    params: &RetargetParams,
) -> u32 {
    if height > 0 && params.interval > 0 && height % params.interval == 0 {
        if let Some((first_ts, last_ts)) = boundary_timestamps {
            return retarget(prev_bits, last_ts.saturating_sub(first_ts), params);
        }
    }
    prev_bits
}

#[cfg(test)]
mod tests {
    use super::*;

    // A mid-range difficulty harder than either floor, so retargets can move it
    // up or down without immediately hitting the floor.
    const MID_BITS: u32 = 0x1d00_ffff;

    #[test]
    fn compact_roundtrips_canonically() {
        for &bits in &[0x1d00_ffffu32, 0x1e0f_ffff, 0x207f_ffff, 0x1b04_8000, 0x1c00_8000] {
            let target = decode_target(bits);
            let canon = target_to_compact(target);
            // Re-decoding the canonical form yields the identical target, and the
            // canonical form is idempotent.
            assert_eq!(decode_target(canon), target, "bits {:#x} target round-trip", bits);
            assert_eq!(
                target_to_compact(decode_target(canon)),
                canon,
                "canonical form idempotent for {:#x}",
                bits
            );
        }
    }

    #[test]
    fn retarget_raises_difficulty_when_blocks_came_fast() {
        let p = RetargetParams::regtest();
        let half = p.target_timespan() / 2; // within [t/4, t*4]
        let new_bits = retarget(MID_BITS, half, &p);
        assert!(
            decode_target(new_bits) < decode_target(MID_BITS),
            "a fast period must lower the target (raise difficulty)"
        );
    }

    #[test]
    fn retarget_lowers_difficulty_when_blocks_came_slow() {
        let p = RetargetParams::regtest();
        let double = p.target_timespan() * 2; // within clamp, still below floor
        let new_bits = retarget(MID_BITS, double, &p);
        let nt = decode_target(new_bits);
        assert!(nt > decode_target(MID_BITS), "a slow period must raise the target");
        assert!(nt <= decode_target(p.pow_limit_bits), "but never past the floor");
    }

    #[test]
    fn retarget_clamps_to_four_x() {
        let p = RetargetParams::regtest();
        // Absurdly slow -> clamp to 4x -> target ~= old * 4.
        let way_slow = p.target_timespan() * 1000;
        let slow_bits = retarget(MID_BITS, way_slow, &p);
        let four_x = decode_target(MID_BITS) * Target256::from(4u64);
        assert_eq!(decode_target(slow_bits), decode_target(target_to_compact(four_x)));

        // Absurdly fast -> clamp to /4 -> target ~= old / 4.
        let fast_bits = retarget(MID_BITS, 0, &p);
        let quarter = decode_target(MID_BITS) / Target256::from(4u64);
        assert_eq!(decode_target(fast_bits), decode_target(target_to_compact(quarter)));
    }

    #[test]
    fn retarget_floors_at_pow_limit_without_overflow_at_testnet_params() {
        // Start AT the testnet floor and run a maximally slow period. old_target
        // is near pow_limit (bitlen ~236) and clamped is ~4*150*2016 (~2^21), so
        // old*clamped exceeds 256 bits — this exercises the 512-bit path. The 4x
        // easing would pass the floor, so the result must be floored back to it.
        let p = RetargetParams::testnet();
        let slow = p.target_timespan() * 4;
        let new_bits = retarget(p.pow_limit_bits, slow, &p);
        assert_eq!(
            new_bits, p.pow_limit_bits,
            "easing past the floor must clamp to pow_limit (and must not overflow/panic)"
        );
    }

    #[test]
    fn required_bits_unchanged_off_boundary() {
        let p = RetargetParams::regtest();
        // height 1 is not a multiple of interval -> difficulty stays at prev.
        assert_eq!(required_bits(1, MID_BITS, Some((0, 999_999)), &p), MID_BITS);
        assert_eq!(required_bits(p.interval - 1, MID_BITS, None, &p), MID_BITS);
    }

    #[test]
    fn required_bits_retargets_on_boundary() {
        let p = RetargetParams::regtest();
        // A boundary height with a fast period must change (raise) difficulty.
        let fast = p.target_timespan() / 2;
        let got = required_bits(p.interval, MID_BITS, Some((0, fast)), &p);
        assert_ne!(got, MID_BITS, "a boundary height must retarget");
        assert!(decode_target(got) < decode_target(MID_BITS), "fast period raises difficulty");
        // Genesis (height 0) is exempt.
        assert_eq!(required_bits(0, MID_BITS, Some((0, fast)), &p), MID_BITS);
    }

    #[test]
    fn floor_predicate_rejects_easier_and_zero_targets() {
        let p = RetargetParams::testnet(); // floor 0x1e0fffff
        assert!(target_within_limit(0x1e0f_ffff, &p), "the floor itself is valid");
        assert!(target_within_limit(0x1d00_ffff, &p), "harder than floor is valid");
        assert!(!target_within_limit(0x207f_ffff, &p), "easier than floor is rejected");
        assert!(!target_within_limit(0x2100_0000, &p), "over-range (zero target) is rejected");
    }
}
