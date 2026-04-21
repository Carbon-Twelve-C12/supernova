# ADR-0003: SHA3-512 Proof-of-Work hash

**Status:** Accepted
**Date ratified:** 2025-05-06 (0.9.9-FRC)
**Scope:** Consensus / mining

---

## Context

A Proof-of-Work chain must pick a hash for block-header PoW and for
every consensus-relevant Merkle / commitment hash in the protocol.
Candidates at design time:

- **SHA-256** (Bitcoin, double-SHA256 in `block_hash`).
- **Scrypt / Argon2** (memory-hard alternatives — Litecoin, Monero family).
- **SHA3-256 / SHA3-512** (Keccak-based, FIPS 202).
- **Blake3** — modern, fast, not a government standard.

Key constraints for Supernova:

1. **Grover resistance margin.** A quantum adversary running Grover's
   algorithm achieves quadratic speed-up on preimage search — an `n`-bit
   hash provides only `n/2` bits of quantum security. 256 bits collapses
   to 128 bits. 512 bits collapses to 256 — the conservative margin we
   want for a chain with a multi-decade horizon.
2. **Standardized.** We committed in [ADR-0001](0001-post-quantum-algorithm-selection.md)
   to NIST-standardized primitives in the cryptographic path.
3. **Deterministic, side-channel safe, hardware-friendly for both
   CPU miners and eventual ASIC.** Memory-hard functions were rejected
   because the ASIC-resistance argument is historically temporary, and
   the determinism cost on verification is real.
4. **Not pure SHA-256.** Deliberate departure from the Bitcoin lineage:
   reusing SHA-256 invites accidental compatibility surface with
   Bitcoin tooling that does not track Supernova's consensus rules,
   and Grover's 128-bit margin is too narrow for our horizon.

## Decision

- **PoW hash:** SHA3-512, evaluated on the canonicalised block header.
- **Merkle hash:** SHA3-512.
- **Generic content hash (`hash256`, `hash_data`):** SHA3-512.
- **Address / keyed hashes:** BLAKE3 and RIPEMD are used for wallet /
  address derivation where shorter digests are acceptable; consensus
  state never depends on these.

Implementation: `sha3` crate (`Sha3_512`). The PoW check is a direct
byte-compare of the hash output against the target in
`supernova-core/src/consensus/difficulty.rs`.

## Consequences

### Positive
- **256 bits of quantum preimage resistance** under Grover — matching
  the classical security level of SHA-256 even against a future
  CRQC (cryptographically relevant quantum computer).
- **Distinct from Bitcoin's SHA-256d.** Mining hardware for Bitcoin is
  not useful for Supernova out of the box; this is a feature from both
  a decentralisation standpoint (no instant hashrate dominance by
  rental markets) and a security standpoint (distinct attack surface).
- **FIPS 202 standard** — auditable, reviewed, and implemented in
  multiple independent libraries.
- Keccak's sponge construction composes well with the commitment
  patterns we use in Lightning and the script system.

### Negative
- **Raw CPU throughput is lower than SHA-256.** SHA3-512 is roughly
  2–3× slower per block on commodity CPUs. We accepted this because
  the cost is paid once per header, not per transaction.
- **Larger digest (64 bytes vs 32).** All `BlockHash`, Merkle node, and
  `hash256` values carry the larger footprint — a real cost on disk
  and wire. Mitigated by the fact that payload (transactions, signatures)
  dominates; header overhead is marginal.
- **ASIC ecosystem is thin.** Early mining will be CPU/GPU; ASICs for
  SHA3-512 exist but are not as commoditised as SHA-256. This is
  intentional for early-network decentralisation.

### Alternatives considered

- **SHA-256d.** Rejected on quantum margin and ecosystem collision.
- **Blake3.** Rejected because it is not a government standard; we are
  unwilling to bet consensus on an unstandardised primitive.
- **Memory-hard (Scrypt/Argon2/RandomX).** Rejected — the "ASIC
  resistance" property is historically temporary (Monero/RandomX has
  seen repeated ASIC pressure) and verification cost is higher than
  we want for a chain that must support PQ signature verification in
  the same block-processing budget.

## References

- FIPS 202 (SHA-3 standard).
- `supernova-core/src/consensus/difficulty.rs`, `hash.rs`.
- [ADR-0001](0001-post-quantum-algorithm-selection.md).
