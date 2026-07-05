//! L5 redistributive incentive gradient (ADR-0002 §9).
//!
//! Pure helpers that describe how a block reward splits between the miner and
//! the treasury, and how the treasury's redistribution makes verified-green
//! mining out-earn non-green mining — **without any new mint**. All value moved
//! is value that already existed (block subsidy schedule sums to 42M; treasury
//! rebates only move existing balance). Nothing here is wired to consensus.

use crate::treasury::{distribute_rebates, fund_from_block};
use crate::types::MinerAddr;

/// Split of a single block reward (ADR-0002 §9).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RewardSplit {
    /// Portion paid directly to the miner: `(100 - X)%` of `(subsidy + fees)`.
    pub miner_share: u64,
    /// Portion routed to the environmental treasury: `X%`.
    pub treasury_cut: u64,
}

/// Split `(subsidy + fees)` into miner share and treasury cut.
///
/// Guarantees `miner_share + treasury_cut == subsidy + fees` (saturating),
/// so the split never creates or destroys value — it only redirects part of an
/// already-scheduled reward.
pub fn split_block_reward(subsidy: u64, fees: u64, x_percent: u8) -> RewardSplit {
    let total = subsidy.saturating_add(fees);
    let treasury_cut = fund_from_block(subsidy, fees, x_percent);
    // treasury_cut <= total by construction, so this never underflows.
    let miner_share = total - treasury_cut;
    RewardSplit {
        miner_share,
        treasury_cut,
    }
}

/// Net position of a verified-green miner over one distribution epoch.
///
/// `direct` is what the miner earned directly (their [`RewardSplit::miner_share`]
/// summed over the epoch); `rebate` is their pro-rata treasury payout. The
/// gradient goal (ADR-0002 §9) is that a green miner recovers their treasury
/// contribution *plus* a share of non-green miners' contributions, so green
/// out-earns non-green. This is a reporting helper only.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MinerEpochOutcome {
    /// Direct reward retained across the epoch.
    pub direct: u64,
    /// Treasury rebate received this epoch.
    pub rebate: u64,
}

impl MinerEpochOutcome {
    /// Total earned = direct reward + treasury rebate.
    pub fn total(&self) -> u64 {
        self.direct.saturating_add(self.rebate)
    }
}

/// Compute per-miner rebates for an epoch from a treasury `balance` and each
/// miner's verified clean energy. Thin wrapper over
/// [`crate::treasury::distribute_rebates`]; preserves the no-mint invariant.
///
/// Returns `(payouts, remaining_in_treasury)`.
pub fn epoch_rebates(
    treasury_balance: u64,
    verified_claims: &[(MinerAddr, u64)],
) -> (Vec<(MinerAddr, u64)>, u64) {
    distribute_rebates(treasury_balance, verified_claims)
}

/// Clawback helper (ADR-0002 §9): reverse a previously granted `rebate` on a
/// successful fraud proof.
///
/// Pure: returns the amount to return to the treasury, capped at the original
/// rebate (never claws back more than was paid, never mints). The caller
/// credits the returned amount back to the treasury balance and records it.
pub fn clawback_amount(original_rebate: u64, proven_invalid: bool) -> u64 {
    if proven_invalid {
        original_rebate
    } else {
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        /// Conservation (ADR-0002 §9): for any subsidy, fees, and X%, the split
        /// neither creates nor destroys value —
        ///   `miner_share + treasury_cut == subsidy + fees` (saturating total),
        /// and the treasury cut never exceeds the whole reward.
        #[test]
        fn split_conserves_reward_for_all_inputs(
            subsidy in 0u64..=1_000_000_000_000u64,
            fees in 0u64..=1_000_000_000_000u64,
            x in 0u8..=255u8,
        ) {
            let split = split_block_reward(subsidy, fees, x);
            let total = subsidy.saturating_add(fees);
            // No coins created or destroyed: the two shares sum back to the reward.
            prop_assert_eq!(
                split.miner_share as u128 + split.treasury_cut as u128,
                total as u128
            );
            // The treasury never takes more than the whole reward (no mint).
            prop_assert!(split.treasury_cut <= total);
        }
    }

    #[test]
    fn split_conserves_reward() {
        let s = split_block_reward(1_000, 200, 20);
        assert_eq!(s.miner_share + s.treasury_cut, 1_200);
        assert_eq!(s.treasury_cut, 240);
        assert_eq!(s.miner_share, 960);
    }

    #[test]
    fn green_can_out_earn_via_rebate() {
        // Two miners contribute equally to the treasury, but only the green one
        // has verified MWh, so it recaptures the whole treasury pot.
        let split = split_block_reward(1_000, 0, 20); // cut = 200 each block
        let treasury = split.treasury_cut * 2; // both miners funded it
        let green = MinerAddr([1; 32]);
        let (payouts, remaining) = epoch_rebates(treasury, &[(green, 5_000)]);
        assert_eq!(remaining, 0);
        let green_outcome = MinerEpochOutcome {
            direct: split.miner_share,
            rebate: payouts[0].1,
        };
        // Green miner's total exceeds its own direct share (it clawed back the
        // non-green miner's treasury contribution too).
        assert!(green_outcome.total() > split.miner_share);
    }

    #[test]
    fn clawback_is_bounded_by_original() {
        assert_eq!(clawback_amount(500, true), 500);
        assert_eq!(clawback_amount(500, false), 0);
    }
}
