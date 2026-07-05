//! Property tests proving the redistribution invariant: rebate distribution
//! never exceeds the treasury balance and NEVER mints. Total supply is fixed
//! at 42,000,000 NOVA; these tests guard the "no new mint" guarantee.

use green_verification::treasury::{distribute_rebates, fund_from_block};
use green_verification::types::MinerAddr;
use proptest::prelude::*;

fn miner_addr(seed: u8) -> MinerAddr {
    MinerAddr([seed; 32])
}

proptest! {
    /// For any balance and any set of (miner, mwh) claims:
    ///   1. sum(payouts) <= balance          (never mints)
    ///   2. sum(payouts) + remaining == balance exactly (pure redistribution)
    #[test]
    fn distribute_never_mints_and_conserves(
        balance in 0u64..=1_000_000_000_000u64,
        claims in proptest::collection::vec((0u8..64u8, 0u64..1_000_000u64), 0..32),
    ) {
        let claim_vec: Vec<(MinerAddr, u64)> =
            claims.iter().map(|(s, mwh)| (miner_addr(*s), *mwh)).collect();

        let (payouts, remaining) = distribute_rebates(balance, &claim_vec);

        // Same number of payouts as claims, same order.
        prop_assert_eq!(payouts.len(), claim_vec.len());

        // Sum payouts using u128 to detect any overflow that would signal a mint.
        let sum: u128 = payouts.iter().map(|(_, a)| *a as u128).sum();

        // (1) never exceeds balance -> never mints.
        prop_assert!(sum <= balance as u128, "payouts {} exceeded balance {}", sum, balance);

        // (2) exact conservation: distributed + remaining == balance.
        prop_assert_eq!(sum + remaining as u128, balance as u128);
    }

    /// A miner with zero verified MWh must receive nothing.
    #[test]
    fn zero_mwh_claims_receive_nothing(
        balance in 0u64..=1_000_000_000u64,
        n in 1usize..16usize,
    ) {
        let claim_vec: Vec<(MinerAddr, u64)> =
            (0..n).map(|i| (miner_addr(i as u8), 0u64)).collect();
        let (payouts, remaining) = distribute_rebates(balance, &claim_vec);
        prop_assert!(payouts.iter().all(|(_, a)| *a == 0));
        prop_assert_eq!(remaining, balance);
    }

    /// fund_from_block never returns more than subsidy + fees (no value created).
    #[test]
    fn treasury_cut_never_exceeds_reward(
        subsidy in 0u64..=1_000_000_000_000u64,
        fees in 0u64..=1_000_000_000_000u64,
        x in 0u8..=255u8,
    ) {
        let cut = fund_from_block(subsidy, fees, x);
        let total = (subsidy as u128) + (fees as u128);
        prop_assert!(cut as u128 <= total, "cut {} exceeded reward {}", cut, total);
    }
}
