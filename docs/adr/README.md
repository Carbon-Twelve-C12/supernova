# Architecture Decision Records

This directory captures the **load-bearing** architectural choices in
Supernova — the ones future contributors would otherwise have to
reverse-engineer from code archaeology. Format loosely follows
[Michael Nygard's template](https://cognitect.com/blog/2011/11/15/documenting-architecture-decisions).

Each record states:

- **Context** — the problem and the constraints we were under.
- **Decision** — what we actually chose.
- **Consequences** — what we accept, positive and negative.
- **Status** — Proposed / Accepted / Superseded (by ADR-00XX).

New decisions get the next sequential number. Supersessions never delete
prior records — we amend the status line and link forward so the history
stays auditable.

---

## Index

| # | Title | Status | Scope |
|---|---|---|---|
| [0001](0001-post-quantum-algorithm-selection.md) | Post-quantum algorithm selection | Accepted | Cryptography |
| [0002](0002-utxo-transaction-model.md) | UTXO transaction model | Accepted | Consensus / state |
| [0003](0003-sha3-512-proof-of-work.md) | SHA3-512 Proof-of-Work hash | Accepted | Consensus / mining |
| [0004](0004-block-time-150s.md) | 150-second (2.5-minute) block time | Accepted | Consensus |
| [0005](0005-fork-resolution-v2.md) | Fork resolution v2 | Accepted | Consensus |
| [0006](0006-treasury-governance.md) | Treasury governance scheme | Accepted | Governance / consensus |
| [0007](0007-refund-signer-broadcaster-traits.md) | Refund-flow trait abstraction for atomic-swap RPC | Accepted | Atomic-swap / cross-crate dependency design |
| [0008](0008-bulletproof-range-proof-fail-closed.md) | Bulletproof range-proof verifier fails closed on production builds | Accepted | Cryptography / consensus / confidential transactions |

---

## When to write a new ADR

Write one when a decision:

1. Shapes a public API or wire format.
2. Changes a consensus rule or touches a cryptographic primitive.
3. Picks one irreversible option among several viable ones.
4. Future-you or an auditor would ask "why *that* choice?" six months later.

Small refactors, local optimisations, and reversible style choices do **not**
need an ADR. When unsure, prefer writing one — they are cheap to draft and
invaluable during audit.
