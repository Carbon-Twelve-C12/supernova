# ADR-0005: Fork resolution v2

**Status:** Accepted (supersedes v1 heuristic that shipped in 0.9.9-FRC)
**Date ratified:** 2025-09-29 (RC4 consistency)
**Scope:** Consensus

---

## Context

A PoW chain must choose a winner when two valid chains compete. The
naïve rule — "most accumulated work" — is correct but not sufficient in
practice:

- **Ties are possible.** Two chains may arrive with identical work.
- **Selfish-mining and withholding attacks** can craft chains that
  reveal stale state at adversarial timing.
- **Time-warp attacks** exploit timestamp manipulation to change how
  difficulty accumulates. A pure "more work" rule that trusts the
  reported difficulty is vulnerable.
- **Reorg economics.** Deep reorgs erode user trust; a fork-choice rule
  that permits them where a more conservative rule would not is a
  real cost even if mechanically "more work".

Supernova shipped a first-cut rule in 0.9.9-FRC that performed most-work
selection with a simple first-seen tiebreaker. The RC3/RC4 audit cycle
surfaced three concrete weaknesses:

1. Headers could be accepted before their full chain was validated,
   causing re-reorgs on validation failure — a race condition
   (tracked as RC4 P0, commit `fecf67d`).
2. No explicit check against timestamp manipulation at fork-point.
3. No quality score weighting — a chain with wildly variable block
   times could still win if work matched.

## Decision

Replace the v1 heuristic with **fork-resolution v2** implemented in
`supernova-core/src/consensus/fork_resolution_v2.rs` and
`secure_fork_resolution.rs`.

The rule, in priority order:

1. **Weak-subjectivity checkpoint compliance.** Any chain that diverges
   from a committed checkpoint is rejected outright. See
   `supernova-core/src/consensus/weak_subjectivity.rs`.
2. **Accumulated verified work.** Work is counted only for headers
   whose full block has been validated. This closes the race condition
   in v1.
3. **Timing quality score.** `SecureForkResolution` scores candidate
   chains on average block time, variance, and compliance with the
   `min_block_time` / `max_block_time` envelope (30 s and 3600 s
   respectively). Chains with adversarial timing are down-weighted.
4. **Median-time-past (MTP) enforcement.** Block timestamps must
   satisfy `MTP < timestamp < now + max_drift`. Time-warp windows are
   constrained by this envelope.
5. **First-seen tiebreak.** If accumulated work ties exactly after all
   of the above, the chain received first wins. This is deterministic
   given consistent peer behaviour and avoids network-wide oscillation.

Max reorg depth is enforced separately — reorgs deeper than the
weak-subjectivity checkpoint are not reorgs; they are a chain split
requiring operator action.

## Consequences

### Positive
- **Race-free.** Validation-gate on accumulated work prevents the v1
  "accept header, reorg, reject block, reorg again" flutter.
- **Selfish-mining / withholding cost goes up.** The timing-quality
  score penalises long silences followed by bursts, which is the
  shape of most withholding strategies.
- **Time-warp is bounded.** MTP + drift envelope + quality score
  combine to bound how much an adversary can move the difficulty
  target.
- **Deterministic tiebreak.** First-seen is a weak tiebreak but
  deterministic; stronger alternatives (e.g. cryptographic lottery)
  were considered and rejected as adding surface without closing
  meaningful attacks at 1.0.0.

### Negative
- **Complexity.** Two modules (`fork_resolution_v2.rs` and
  `secure_fork_resolution.rs`) implement overlapping logic; Phase 2
  remediation includes merging these. Tracked as a post-1.0.0 cleanup,
  not a consensus-relevant refactor.
- **Validation order matters.** Callers must ensure a block's
  predecessors are validated before its work is counted. This is
  enforced but is a footgun for future contributors touching the
  sync path.
- **Quality score parameters are consensus-critical.** Changing the
  scoring weights is a hard fork.

### Alternatives considered

- **Pure longest chain (first seen).** Rejected — ignores work, vulnerable
  to timestamp manipulation.
- **GHOST / uncle rule.** Rejected — inherits complexity without a
  matching gain for Supernova's block-time regime; relevant for much
  shorter block times.
- **GRANDPA-style finality gadget.** Rejected — out of scope for a PoW
  chain; would effectively add a second consensus layer.

## References

- `supernova-core/src/consensus/fork_resolution_v2.rs`
- `supernova-core/src/consensus/secure_fork_resolution.rs`
- `supernova-core/src/consensus/weak_subjectivity.rs`
- `supernova-core/src/consensus/timestamp_validation.rs`
- `supernova-core/src/consensus/fork_resolution_attack_tests.rs` — the
  attack scenarios this rule must resist.
- [ADR-0004](0004-block-time-150s.md) — block-time constants this rule
  depends on.
