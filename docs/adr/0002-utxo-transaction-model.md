# ADR-0002: UTXO transaction model

**Status:** Accepted
**Date ratified:** 2024-12-01 (initial transaction structure)
**Scope:** Consensus / state

---

## Context

Two transaction models dominate modern blockchains:

- **UTXO** (Unspent Transaction Output) — each transaction consumes
  prior outputs and produces new ones; state is the set of unspent
  outputs. Used by Bitcoin, Litecoin, Cardano.
- **Account** — state is a map from address to balance; transactions
  mutate that map. Used by Ethereum, Solana, most smart-contract chains.

Supernova's design priorities:

1. **Parallelism.** We must be able to verify unrelated transactions in
   parallel. Bottlenecks in sequential state-mutation become fatal once
   PQ signatures (10–50× slower verification) are in the path.
2. **Privacy enclaves are tractable.** Even without shipping a privacy
   feature on day one, we wanted the option open — UTXOs compose with
   mixing, confidential transactions, and bulletproof-style range
   proofs in ways an account model does not.
3. **Lightning compatibility.** Lightning channel construction relies on
   UTXO-shaped outputs and commitment transactions. An account model
   would have required a fundamentally different channel design.
4. **Operational familiarity.** Supernova draws heavily on the
   Bitcoin lineage; tools, auditors, and users are conversant with the
   UTXO model. That reduces operational risk at launch.

## Decision

Adopt the UTXO model. Concretely:

- `Transaction` = `{inputs: Vec<TransactionInput>, outputs:
  Vec<TransactionOutput>, witness, lock_time}` (`supernova-core/src/
  types/transaction.rs`).
- `TransactionInput` references a prior `(txid, output_index)` plus a
  witness script that proves authorisation.
- Authoritative state = the UTXO set (`node/src/storage/
  atomic_utxo_set.rs`, `supernova-core/src/storage/utxo_set.rs`) —
  backed by `DashMap`/`DashSet` for lock-free concurrent reads.
- Script/witness model uses a stack-based opcode interpreter patterned
  after Bitcoin Script, with PQ signature opcodes replacing
  `OP_CHECKSIG` family.

## Consequences

### Positive
- Independent transactions are **embarrassingly parallel**: the
  signature cache, parallel validator, and block-propagation pipeline
  all exploit this.
- UTXO set is trivially snapshot-able at arbitrary heights; reorgs
  undo by replaying a diff rather than reconstructing state.
- Lightning channels and HTLC scripts map cleanly; the atomic-swap
  module piggybacks on the same primitives.
- Historical audit trail: every coin's provenance is chain-complete.

### Negative
- **No native "account balance".** Wallets must scan UTXOs and
  aggregate; we ship this in `wallet/` but it is a real cost for
  third-party integrators.
- **No general-purpose smart contracts.** Supernova's scripting is
  deliberately constrained; Turing-complete contracts are a
  non-goal for 1.0.0 and would require a substantial follow-on.
- **Transaction size grows with input count.** PQ signatures on each
  input compound this (see [ADR-0001](0001-post-quantum-algorithm-selection.md)).
  We address with signature compression and per-witness caching, but the
  baseline remains larger than an account-model equivalent.

### Alternatives considered

- **Account model.** Rejected primarily on parallelism and Lightning
  grounds.
- **Hybrid (UTXO + account-like overlays).** Rejected for 1.0.0 as a
  premature abstraction; may be revisited post-mainnet via sidechain
  or L2 mechanisms.

## References

- `supernova-core/src/types/transaction.rs`, `supernova-core/src/
  types/block.rs` — data model.
- `supernova-core/src/storage/utxo_set.rs`, `node/src/storage/
  atomic_utxo_set.rs` — state backing.
- `supernova-core/src/script/` — opcode interpreter.
- `docs/supernova_whitepaper_v1.md` — broader motivation.
