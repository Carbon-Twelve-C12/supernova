//! L4 environmental treasury — pure accounting (ADR-0002 §8, §9).
//!
//! **Redistributive, 42M-safe, no mint.** Every function here either moves
//! value that already exists inside the treasury balance *out* to miners, or
//! computes a *cut* of an already-existing block reward. No function increases
//! the total money supply. The core distribution invariant —
//! `sum(payouts) + remaining == balance` — is enforced by construction and
//! proven by the property tests in `tests/`.
//!
//! Nothing here is wired to coinbase, block validation, or consensus state;
//! these are pure functions over plain integers.

use serde::{Deserialize, Serialize};

use crate::error::GreenError;
use crate::types::MinerAddr;

/// Consensus-account treasury state (ADR-0002 §4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct EnvironmentalTreasury {
    /// Current spendable balance (real value, never a phantom ledger).
    pub balance: u64,
    /// Lifetime total rebated to verified-green miners.
    pub total_rebated: u64,
    /// Lifetime carbon offsets retired, in milli-tonnes CO2.
    pub total_offsets_retired_milli: u64,
}

impl EnvironmentalTreasury {
    /// Create a treasury with an initial balance.
    pub fn new(balance: u64) -> Self {
        Self {
            balance,
            total_rebated: 0,
            total_offsets_retired_milli: 0,
        }
    }

    /// Add a treasury cut from a block (subsidy/fee routing).
    ///
    /// This does not mint: the caller must have deducted `amount` from the
    /// miner's payout of an already-scheduled (<=42M) block reward. Saturating
    /// to avoid overflow panics; in the fixed-supply system the balance can
    /// never realistically approach `u64::MAX`.
    pub fn credit(&mut self, amount: u64) {
        self.balance = self.balance.saturating_add(amount);
    }

    /// Debit `amount` from the balance, or fail if it would underflow.
    ///
    /// Used to move value out to miners/offsets. Never goes negative, never mints.
    pub fn debit(&mut self, amount: u64) -> Result<(), GreenError> {
        self.balance = self
            .balance
            .checked_sub(amount)
            .ok_or(GreenError::TreasuryUnderflow)?;
        Ok(())
    }

    /// Apply a batch of rebate payouts computed by [`distribute_rebates`],
    /// debiting their sum from the balance and accumulating `total_rebated`.
    ///
    /// Returns [`GreenError::TreasuryUnderflow`] if the payouts exceed balance
    /// (which cannot happen for payouts produced against this balance, but the
    /// guard makes the invariant explicit).
    pub fn apply_rebates(&mut self, payouts: &[(MinerAddr, u64)]) -> Result<(), GreenError> {
        let total: u64 = payouts
            .iter()
            .try_fold(0u64, |acc, (_, amt)| acc.checked_add(*amt))
            .ok_or(GreenError::TreasuryUnderflow)?;
        self.debit(total)?;
        self.total_rebated = self.total_rebated.saturating_add(total);
        Ok(())
    }

    /// Record a retired carbon offset of `tonnes_co2_milli`, debiting `cost`.
    pub fn retire_offset(
        &mut self,
        cost: u64,
        tonnes_co2_milli: u64,
    ) -> Result<(), GreenError> {
        self.debit(cost)?;
        self.total_offsets_retired_milli = self
            .total_offsets_retired_milli
            .saturating_add(tonnes_co2_milli);
        Ok(())
    }
}

/// Treasury cut of a block reward (ADR-0002 §8).
///
/// Returns `X%` of `(subsidy + fees)`, saturating and clamping `x_percent` to
/// `[0, 100]`. The result is **guaranteed `<= subsidy + fees`**, so routing it
/// to the treasury and the remainder to the miner never creates value.
pub fn fund_from_block(subsidy: u64, fees: u64, x_percent: u8) -> u64 {
    let x = x_percent.min(100) as u128;
    let total = (subsidy as u128).saturating_add(fees as u128);
    let cut = total.saturating_mul(x) / 100;
    // cut <= total <= u64 range (since total is sum of two u64 cut by <=100%).
    cut.min(total).min(u64::MAX as u128) as u64
}

/// Pro-rata rebate distribution (ADR-0002 §9) — the redistribution primitive.
///
/// Given a treasury `balance` and per-miner verified clean energy `claims`
/// (`(addr, mwh_milli)`), distribute the balance pro-rata to verified MWh.
///
/// # Invariants (proven in `tests/`)
///
/// - `sum(payouts) <= balance` (never mints).
/// - `sum(payouts) + remaining == balance` exactly (pure redistribution; floor
///   dust stays in the treasury as `remaining`).
/// - Deterministic: payout order matches input order.
///
/// Edge cases: zero balance or zero total MWh ⇒ every payout is 0 and
/// `remaining == balance`. Claims with `mwh_milli == 0` receive 0.
pub fn distribute_rebates(
    balance: u64,
    claims: &[(MinerAddr, u64)],
) -> (Vec<(MinerAddr, u64)>, u64) {
    // Total verified MWh across all claims (u128 to avoid overflow).
    let total_mwh: u128 = claims.iter().map(|(_, mwh)| *mwh as u128).sum();

    // No balance to give or no verified energy ⇒ nothing is distributed.
    if balance == 0 || total_mwh == 0 {
        let payouts = claims.iter().map(|(a, _)| (*a, 0u64)).collect();
        return (payouts, balance);
    }

    let bal = balance as u128;
    let mut distributed: u128 = 0;
    let mut payouts = Vec::with_capacity(claims.len());

    for (addr, mwh) in claims {
        // Floor division: payout_i = balance * mwh_i / total_mwh.
        // Sum of floors <= balance because sum(mwh_i) == total_mwh, so the
        // exact (non-floored) shares sum to exactly `balance`.
        let share = bal.saturating_mul(*mwh as u128) / total_mwh;
        distributed = distributed.saturating_add(share);
        // share <= balance <= u64::MAX, safe to narrow.
        payouts.push((*addr, share as u64));
    }

    // Floor dust is retained in the treasury (never minted, never lost).
    let remaining = (bal - distributed) as u64;
    (payouts, remaining)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn addr(b: u8) -> MinerAddr {
        MinerAddr([b; 32])
    }

    #[test]
    fn distribution_conserves_balance() {
        let claims = vec![(addr(1), 1_000u64), (addr(2), 3_000u64)];
        let (payouts, remaining) = distribute_rebates(100, &claims);
        let sum: u64 = payouts.iter().map(|(_, a)| *a).sum();
        assert_eq!(sum + remaining, 100);
        assert!(sum <= 100);
        // 1000/4000 * 100 = 25, 3000/4000 * 100 = 75, dust 0.
        assert_eq!(payouts[0].1, 25);
        assert_eq!(payouts[1].1, 75);
        assert_eq!(remaining, 0);
    }

    #[test]
    fn zero_balance_pays_nothing() {
        let claims = vec![(addr(1), 500u64)];
        let (payouts, remaining) = distribute_rebates(0, &claims);
        assert_eq!(payouts[0].1, 0);
        assert_eq!(remaining, 0);
    }

    #[test]
    fn zero_energy_retains_balance() {
        let claims = vec![(addr(1), 0u64), (addr(2), 0u64)];
        let (payouts, remaining) = distribute_rebates(1_000, &claims);
        assert!(payouts.iter().all(|(_, a)| *a == 0));
        assert_eq!(remaining, 1_000);
    }

    #[test]
    fn rebate_against_empty_treasury_does_not_panic() {
        let mut t = EnvironmentalTreasury::new(0);
        let claims = vec![(addr(1), 10u64)];
        let (payouts, _rem) = distribute_rebates(t.balance, &claims);
        assert!(t.apply_rebates(&payouts).is_ok());
        assert_eq!(t.balance, 0);
        assert_eq!(t.total_rebated, 0);
    }

    #[test]
    fn fund_from_block_never_exceeds_reward() {
        assert_eq!(fund_from_block(100, 0, 20), 20);
        assert_eq!(fund_from_block(0, 50, 20), 10);
        assert!(fund_from_block(u64::MAX, u64::MAX, 100) <= u64::MAX);
        // Over-100% is clamped.
        assert_eq!(fund_from_block(100, 0, 200), 100);
    }

    #[test]
    fn debit_underflow_is_rejected() {
        let mut t = EnvironmentalTreasury::new(5);
        assert_eq!(t.debit(10), Err(GreenError::TreasuryUnderflow));
        assert_eq!(t.balance, 5);
    }
}
