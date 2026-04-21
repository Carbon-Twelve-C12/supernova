# ADR-0006: Treasury governance scheme

**Status:** Accepted
**Date ratified:** 2026-04 (Phase 1 A1, commit `9f7fc10`)
**Scope:** Governance / consensus

---

## Context

Supernova's block reward splits between:

- **Miner coinbase** — incentive for Proof-of-Work.
- **Treasury allocation** — a protocol-level fraction that funds
  environmental carbon offsets, core-protocol maintenance, and
  ecosystem grants.

The 0.9.9-FRC / early-RC builds carried a **`TREASURY_ADDRESS_PLACEHOLDER`**
in `miner/src/mining/template.rs`. Shipping a placeholder to mainnet
was rightly flagged as a P0 blocker: either an adversary controls
whatever that placeholder resolves to, or the treasury fraction is
silently burnt, and in both cases the protocol's stated economics
diverge from on-chain reality.

Design constraints we had to satisfy:

1. **Single-signer custody is unacceptable.** Any single key
   (hardware, cloud, or human) compromises both the treasury and the
   protocol's credibility if lost or suborned.
2. **PQ-safe.** Treasury custody must survive cryptographically
   relevant quantum computers; classical multisig alone is not enough.
3. **Network-aware.** Testnet and mainnet treasuries must be distinct
   so a testnet compromise cannot move real value.
4. **Verifiable by every validator.** The treasury policy is consensus —
   blocks paying the wrong address to the treasury output are invalid.
5. **No mutable state.** The treasury address is a chain constant, not
   a governance register. Changing it is a hard fork.

## Decision

Treasury custody is an **`m`-of-`n` SPHINCS+ multisig** anchored at a
compile-time constant per network.

- Implementation: `supernova-core/src/governance/treasury.rs`
  - `TreasuryConfig::MAINNET_ADDRESS: &'static [u8]` — `m`-of-`n`
    SPHINCS+ multisig, `n ≥ 5`, `m` chosen per the launch ceremony.
  - `TreasuryConfig::TESTNET_ADDRESS` — separate multisig for testnet.
  - `treasury_address(network: Network) -> &[u8]` — network-aware
    accessor used by coinbase construction and block validation.
- **SPHINCS+** (not ML-DSA) is chosen because the treasury is the
  single point in the protocol where we privilege conservatism over
  signature size; hash-based signatures are the safest long-term bet
  available today (see [ADR-0001](0001-post-quantum-algorithm-selection.md)).
- Coinbase validation (`miner/src/mining/template.rs` and
  `supernova-core/src/validation/block.rs`) rejects any block whose
  treasury output does not pay to the configured address for its
  network.
- Signer set and `m`/`n` for mainnet are established at the genesis
  ceremony and documented in the launch post-mortem; signers sit with
  independent operators on independent hardware.

## Consequences

### Positive
- **No single point of failure.** Losing fewer than `n - m` signers
  does not compromise the treasury; compromising fewer than `m`
  signers does not steal it.
- **PQ-safe.** SPHINCS+ custody survives Grover- and Shor-class
  adversaries.
- **Network-indistinguishable testnet.** Testnet treasury cannot
  accidentally receive mainnet value and vice versa.
- **Consensus-enforced.** Miners cannot redirect the treasury
  allocation; the output's destination is validated the same way
  the block subsidy amount is.

### Negative
- **Signature size.** SPHINCS+ is the largest signature family in the
  stack. Treasury-spend transactions carry this cost. We accept it:
  treasury spends are rare.
- **Operational overhead.** Signing a treasury transaction requires
  coordinating `m` signers; practical cadence is measured in months,
  not minutes.
- **Rotating signers is a hard fork.** Any change to the set requires
  a consensus upgrade. This is deliberate — the alternative is a
  governance surface that an attacker can exploit.

### Alternatives considered

- **Single-key treasury.** Rejected — custody risk.
- **Classical multisig only.** Rejected — not PQ-safe.
- **On-chain governance register.** Rejected — adds a mutable
  consensus surface we do not want at 1.0.0. A future ADR may
  introduce delegated governance, but that is post-mainnet.
- **Burn the treasury fraction entirely.** Rejected — environmental
  and ecosystem commitments need an actual funding address.

## References

- `supernova-core/src/governance/treasury.rs`
- `miner/src/mining/template.rs` — coinbase construction.
- `supernova-core/src/validation/block.rs` — treasury-output validation.
- `miner/tests/treasury_validation_tests.rs` — rejects coinbase with
  wrong treasury address.
- [ADR-0001](0001-post-quantum-algorithm-selection.md) — primitive
  choice (SPHINCS+).
- `docs/security/THREAT_MODEL.md` §13 (Governance).
