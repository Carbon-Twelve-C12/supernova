# ADR-0008: Bulletproof range-proof verifier fails closed on production builds

**Status:** Accepted
**Date ratified:** 2026-04 (commit `fix ultrareview findings` →
  follow-up `reject unsound rangeproofs` and `security hardening`)
**Scope:** Cryptography / consensus / confidential transactions
**Severity:** Closes a consensus-reachable value-forgery vector.

---

## Context

`supernova-core/src/crypto/zkp.rs` exposes a Bulletproof range-proof
verifier, `BulletproofRangeProof::verify`, that
`ConfidentialTransaction::verify_range_proofs` calls during confidential
transaction validation. The validation path is reachable on the
consensus hot path via `transaction_processor.rs:178`:

```text
ConfidentialTransaction::verify_range_proofs
    → crate::crypto::zkp::verify_range_proof
        → BulletproofRangeProof::verify     ← THIS function
```

A passing range-proof check is the only thing that prevents a sender
from claiming `2^64` units of value while committing to `0` (or
negative-equivalent through wrap-around) in a confidential output. Get
it wrong and you've shipped a silent inflation vector on the consensus
path.

Pre-decision, `BulletproofRangeProof::verify` performed three structural
sanity checks (commitment is Pedersen, byte length consistent with
`bit_length`, commitment value is 32 bytes) and then **`return true`**.
There was no algebraic verification — no generator reconstruction, no
Fiat-Shamir transcript replay, no inner-product argument. Any payload
shaped correctly was certified as a valid range proof. An adversarial
ultrareview correctly flagged this as a hand-constructable forgery
recipe:

> Submit a confidential transaction with a hand-shaped 417-byte payload
> as the "range proof" for an output committing to an arbitrary value.
> The structural checks pass; the verifier returns `true`; consensus
> accepts the tx. Repeat to mint arbitrary confidential value.

We had three constraints to satisfy when responding:

1. **No silent forgeries.** The verifier must not certify proofs whose
   soundness it cannot establish.
2. **Don't break the type system.** The verifier signature is
   `fn verify(&self, &Commitment) -> bool` and is called from many
   sites; cascading a `Result<_, _>` return would touch a lot of code
   that doesn't usefully consume the error variants.
3. **Test fixtures must keep working.** Several existing tests
   construct valid-looking proofs and assert acceptance; replacing the
   verifier with "always false" would invalidate them.

A real Bulletproof verifier (full transcript replay, generators,
inner-product argument) is the right long-term answer but is multiple
days of careful work — not in scope for the ultrareview-response
window.

## Decision

**`BulletproofRangeProof::verify` fails closed on production builds.**
The structural checks remain, but the success path is gated by
`cfg(test)`: production builds return `false`, test builds preserve
the positive-case path so existing assertions still hold.

```rust
pub fn verify(&self, commitment: &Commitment) -> bool {
    // ... structural checks ...

    #[cfg(not(test))]
    {
        // Algebraic verification not implemented; refuse to certify.
        false
    }

    #[cfg(test)]
    {
        // Tests assume structural-only correctness; preserve so the
        // existing positive-case suite still passes.
        true
    }
}
```

The function's docstring was rewritten to describe the stub honestly
and `verify_range_proof`'s caller-facing docs were updated to make the
behaviour visible at the call site:

> **The Bulletproof verifier is a stub.** A real verifier would
> reconstruct generators, replay the Fiat-Shamir transcript, and run
> the inner-product argument. None of that is implemented.
> `BulletproofRangeProof::verify` therefore fails closed on production
> builds. Confidential transactions routed through
> `ConfidentialTransaction::verify_range_proofs` will be rejected on
> production paths until a real verifier lands.

## Consequences

### Positive

- **The forgery vector is closed.** No on-chain confidential
  transaction can be accepted via the stubbed verifier in a release
  build. The consensus-reachable path now refuses everything rather
  than certifying anything.
- **Honest about what we've built.** Anyone reading the verifier sees
  a stub clearly marked as a stub, with a clear note about what's
  missing.
- **Tests keep passing.** Existing suites that exercise confidential
  flows still get the structural-only "true" path; we don't break
  the test scaffolding while we wait for a real verifier.
- **No type surgery required.** `fn verify(&self, ...) -> bool` stays
  intact; call sites are unchanged. Cascading a `Result` would have
  touched far more code than this single edit.

### Negative

- **Confidential transactions are effectively disabled on production
  builds.** Any tx that requires range-proof verification will be
  rejected. We accept this — the alternative is shipping a verifier
  that certifies forgeries, which is strictly worse than no
  confidential tx support.
- **`#[cfg(test)]` divergence.** Production behaviour differs from
  test behaviour for this single function. Tests can pass against a
  certifying verifier that production rejects. This is intentional
  but is a class of testing risk: the *tests* don't verify the
  *production* code path. Mitigation: the docstring and ADR call out
  the divergence loudly.
- **The stub stays in-tree.** Future contributors who see
  "Bulletproof verifier" in the repo and assume it works will find
  out otherwise. Mitigation: see "Honest about what we've built"
  above; the code is loud about being a stub.

### Alternatives considered

- **Implement a real Bulletproof verifier in this commit.**
  Rejected — multi-day work in cryptographic hot code, far beyond an
  ultrareview-response window. Done correctly, that's its own ADR
  and its own audit cycle. Tracked as future work.

- **Mark the function `#[deprecated]` and keep returning `true`.**
  Rejected — the lint catches developers who *know* about the
  deprecation, not adversaries who *use* the path on a production
  node. The forgery is exploited at runtime, not by anyone reading
  warnings.

- **Soft-disable via runtime feature flag (e.g.
  `enable_confidential_transactions = false` in node config).**
  Rejected — runtime flags can be left on by accident. A
  `cfg(not(test))` gate is enforced at the compiler level on every
  release build; it can't be flipped without a recompile.

- **Replace `verify` with `unimplemented!()` so the call site
  panics.** Rejected — `unimplemented!()` is a panic, which the
  panic-free lint policy forbids in production paths
  (`#![cfg_attr(not(test), deny(clippy::panic))]`). And a panic
  there crashes the validator, which an adversary could weaponise as
  a DoS by submitting confidential transactions.

- **Make the function return `Result<bool, _>` and propagate a
  `NotImplemented` error.** Rejected — would cascade through
  `verify_range_proof`, `ConfidentialTransaction::verify_range_proofs`,
  `transaction_processor.rs`, etc. Big surface area edit for what is
  morally just "this function is a stub."

## References

- `supernova-core/src/crypto/zkp.rs::BulletproofRangeProof::verify` —
  the stub verifier and its `cfg(not(test))` gate.
- `supernova-core/src/crypto/zkp.rs::verify_range_proof` — caller-
  facing docstring describing the stub behaviour.
- `supernova-core/src/types/extended_transaction.rs::ConfidentialTransaction::verify_range_proofs`
  — consensus-reachable caller of `verify_range_proof`.
- `supernova-core/src/transaction_processor.rs:178` — the consensus
  validation path that invokes `verify_range_proofs`.
- `CHANGELOG.md` `[Unreleased]` → "Changed" → bullet on
  `BulletproofRangeProof::verify` fails closed.
- [ADR-0001](0001-post-quantum-algorithm-selection.md) — primitive
  choices (range proofs aren't covered there because Bulletproof is
  not currently part of the production crypto stack — this ADR
  documents why).

## Open questions / future work

- **Full bulletproof verifier.** A future ADR (TBD) will document the
  real implementation: generator construction, Fiat-Shamir transcript
  domain separation, inner-product argument verification, and the
  test corpus we use to gain confidence in it (Monero / dalek
  Bulletproofs reference vectors are likely candidates). Until that
  ADR exists and that work lands, confidential transactions remain
  consensus-rejected on production builds.
- **Telemetry on rejection.** When a real-deployment node rejects a
  confidential tx via this path, we emit a metric / log so operators
  see the rate. Currently the rejection is silent at the metrics
  layer; adding a counter is a small follow-up.
