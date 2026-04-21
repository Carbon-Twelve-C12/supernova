# ADR-0001: Post-quantum algorithm selection

**Status:** Accepted
**Date first drafted:** 2025-04-21 (Phase 1 cryptographic enhancements commit)
**Date ratified:** 2025-05-06 (0.9.9-FRC)
**Scope:** Cryptography

---

## Context

Supernova targets a multi-decade operational lifetime as a Proof-of-Work
chain. Any chain that cannot sign, verify, and key-exchange under a
cryptographically relevant quantum adversary loses custodial guarantees
the moment such an adversary exists. NIST's post-quantum competition
culminated in a final set of standards in 2024 (FIPS 203 / 204 / 205);
the design window for picking PQ primitives without reinventing our own
was closed.

Constraints we accepted:

- **Must be NIST-standardized.** Non-standard PQ schemes (e.g. SIKE,
  Rainbow) have been broken post-finalist; we will not build on anything
  outside the four standardized families.
- **Must not rely exclusively on one hardness assumption.** Lattice,
  hash, and code-based assumptions are independent; at least two
  families must be present in our stack so a single future break does
  not take everything.
- **Must be implementable today with reviewed Rust libraries.** We will
  not ship hand-rolled reference code in consensus-critical paths.
- **Classical + PQ hybrid in the signature path.** For the transition
  period, transactions can carry both a classical (secp256k1/ed25519)
  and a PQ signature; verification requires **both** to pass. This
  hedges against implementation bugs in either family while interop and
  library maturity catch up.

## Decision

Adopt the following primitives in the following roles:

| Role | Algorithm | Standard | Why this role |
|---|---|---|---|
| Primary transaction signatures | **ML-DSA** (Dilithium) | FIPS 204 | Best size/speed tradeoff for the high-volume path |
| Stateless high-security signatures | **SPHINCS+** | FIPS 205 | Hash-based fallback; used for wallet recovery, treasury multisig, key rotation anchors |
| Key exchange (P2P handshake, HPKE-style enveloping) | **ML-KEM** (Kyber) | FIPS 203 | Only standardized KEM candidate |
| Collision-resistant hashing (PoW, Merkle, commitments) | **SHA3-512** | FIPS 202 | Higher Grover margin than SHA-256; see [ADR-0003](0003-sha3-512-proof-of-work.md) |
| Transition-era signatures | **Hybrid** (classical + PQ, both-must-pass) | — | Defence-in-depth against implementation bugs |

Implementation uses the `pqcrypto-*` crate family
(`pqcrypto-dilithium`, `pqcrypto-sphincsplus`, `pqcrypto-falcon`,
`pqcrypto-kyber`), integrated via the `pqcrypto-traits` type boundary.
Timing guarantees are inherited from the NIST reference implementations
rather than provided by the traits layer — see the "Negative" subsection
below for the residual side-channel assumption. Classical signatures
use `secp256k1` (transactions) and `ed25519-dalek` (node identity).

Algorithm-downgrade is prevented by binding the algorithm identifier
into the signed payload at serialisation time (commit `12bba02`, issue
closed in RC4 P0 list).

## Consequences

### Positive
- Independence between signature family (lattice: ML-DSA; hash: SPHINCS+)
  and KEM family (lattice: ML-KEM) means a single future cryptanalytic
  result is unlikely to sink both.
- Hybrid signatures let us ship a PQ-safe chain today without betting
  the farm on the library maturity of any single PQ implementation.
- Hash-based SPHINCS+ at the recovery anchors gives a conservative
  fallback: recovery works even if every lattice scheme fails.

### Negative
- **Signature size.** ML-DSA signatures are ~2.4–4.6 KB (vs ~72 bytes for
  secp256k1). SPHINCS+ is worse (~8–50 KB). This dominates block-size
  budget and drives the compression work in `supernova-core/src/crypto/
  signature_compression.rs`.
- **Verification cost.** ML-DSA verify is ~10–50× slower than
  secp256k1. We mitigate with a signature cache
  (`node/src/validation/sig_cache.rs`) and parallel verification
  (`node/src/validation/parallel_validator.rs`).
- **Library surface.** `pqcrypto-*` wraps the NIST reference C
  implementations. We do not rely on these for constant-time guarantees
  beyond what the reference codes assert; the threat model
  ([`docs/security/THREAT_MODEL.md`](../security/THREAT_MODEL.md))
  documents the residual side-channel assumption.
- **Hybrid format ossification.** Once mainnet launches, the hybrid
  wire format is consensus-frozen. ADR-XXXX will be required to remove
  either leg.

### Alternatives considered

- **Pure classical + delayed PQ migration.** Rejected — the migration
  would be a hard fork with the whole UTXO set at risk; we do not
  believe we can execute that cleanly under adversarial timing.
- **Pure PQ, no classical.** Rejected for RC — PQ library maturity
  was not yet sufficient to bet consensus on a single family. Revisit
  in a future ADR once ≥2 years of production signal accumulates.
- **Falcon as primary.** Rejected — floating-point operations in
  Falcon's signing path are hostile to deterministic consensus
  behaviour across heterogeneous hardware.

## References

- NIST FIPS 203, 204, 205 (2024).
- [`docs/security/THREAT_MODEL.md`](../security/THREAT_MODEL.md) §3
  (Cryptography) for STRIDE analysis of this stack.
- Repo locations: `supernova-core/src/crypto/{signature.rs, quantum.rs,
  kem.rs, key_rotation.rs}`, `quantum_validation/`.
