# ADR-0004: 150-second (2.5-minute) block time

**Status:** Accepted
**Date ratified:** 2024-12-01 (initial consensus config)
**Scope:** Consensus

---

## Context

Block time trades off four independent concerns:

1. **Confirmation latency.** Users expect a first confirmation soon
   enough that a payment at a point-of-sale feels closed.
2. **Orphan / stale rate.** If the block time approaches the global
   propagation budget, stale rate rises and mining incentives
   centralise (well-connected pools win disproportionately).
3. **Storage growth.** At a fixed transactions-per-block, doubling the
   rate doubles the storage burden on every full node.
4. **Reorg probability.** Shorter block times make 1-block reorgs
   common and force confirmation counts up to recover the same
   effective finality.

Reference points:

| Chain | Block time | Notes |
|---|---|---|
| Bitcoin | 600 s | Conservative; stale rate very low |
| Litecoin | 150 s | Balanced; long production history |
| Ethereum | ~12 s | Short; relies on GHOST/uncle protocol to control stale cost |
| Monero | 120 s | Shorter; privacy rather than latency |

Supernova-specific considerations:

- **PQ signatures are large and slow to verify** (see
  [ADR-0001](0001-post-quantum-algorithm-selection.md)). A too-short
  block time compresses the verification window, pushing nodes toward
  parallelism that is already stressed.
- **Global propagation target: < 2 seconds to 99 % of peers.** This
  sets the floor on "safe" block time — we want block time comfortably
  above propagation, with headroom.
- **Lightning deployment** is the planned path for sub-second UX. We
  do not need to optimise base-layer block time to the floor because
  the retail UX path is off-base.

## Decision

**Block time target = 150 seconds (2.5 minutes).**

Defined at `supernova-core/src/consensus/difficulty.rs`:

```rust
pub const MAINNET_BLOCK_TIME_TARGET: u64 = 150;
pub const TESTNET_BLOCK_TIME_TARGET: u64 = 150;
```

Difficulty adjustment targets this interval over a moving window; see
`DifficultyAdjustment::calculate_next_target`.

## Consequences

### Positive
- **Expected propagation / block time ratio is roughly 2/150 ≈ 1.3 %**,
  keeping stale rate low and incentives well-aligned.
- **Confirmation UX** is better than Bitcoin's 10-minute default —
  first confirmation in ~2.5 min, six confirmations in ~15 min.
- **Verification budget** is generous enough that PQ signatures and
  signature-cache maintenance do not starve block processing even under
  load.
- **Well-understood operating point.** Litecoin has run at this cadence
  for 13+ years; operational knowledge transfers.

### Negative
- **Retail confirmation latency is still not "instant".** Users
  expecting Ethereum-class latency will find 2.5 minutes slow; this
  is the explicit Lightning-first tradeoff.
- **4× storage growth vs Bitcoin's cadence** at equal transactions
  per block. Addressed by the pruning story (`node/src/storage/
  pruning.rs`, not ADR-scope here).
- **Changing this is a hard fork.** Every constant that scales with
  block time (difficulty window, MTP window, weak-subjectivity
  checkpoint cadence) would move with it.

### Alternatives considered

- **600 s (Bitcoin).** Rejected — confirmation UX gap vs modern
  expectations is too wide.
- **30–60 s.** Rejected — PQ verification budget and global
  propagation headroom make this unsafe at 1.0.0 maturity.
- **Variable block time.** Explicitly rejected; opens surface for
  timing-manipulation attacks and complicates every downstream
  consensus rule.

## References

- `supernova-core/src/consensus/difficulty.rs:6-26`.
- [ADR-0005](0005-fork-resolution-v2.md) — fork-choice interacts
  with block-time via accumulated work and timing heuristics.
- `docs/security/THREAT_MODEL.md` §4 (Consensus) — time-warp and
  timestamp-manipulation concerns that bind this constant.
