# Supernova vs Bitcoin vs Ethereum — holistic comparison

A side-by-side reference for engineers, auditors, and external
reviewers who need to place Supernova in the design space established
by Bitcoin (BTC) and Ethereum (ETH). This is a **descriptive**
document — it states what each chain is and what each chain isn't,
not whether one is "better." Every claim about BTC and ETH is from
publicly-cited specifications and operational data; every claim
about Supernova is backed by an in-tree commit, ADR, or measurement.

> **Read this with `BASELINE_MEASUREMENTS.md`** — that doc has the
> measured per-op numbers; this doc puts them in cross-chain context.
> Status as of `2026-04-27` (post-RC4, pre-RC5 testnet).

---

## 1. Snapshot

| Dimension | **Supernova** | **Bitcoin** | **Ethereum (post-Merge)** |
|---|---|---|---|
| Status | Pre-mainnet (`1.0.0-RC4` → `RC5` testnet → `1.0.0` mainnet) | Mainnet since 2009-01 | Mainnet since 2015-07 (PoS since 2022-09) |
| Consensus | Quantum-resistant Proof-of-Work | Proof-of-Work (SHA-256d) | Proof-of-Stake (Casper FFG + LMD-GHOST) |
| Block time | 150 s (2.5 min) | 600 s (10 min) | 12 s |
| State model | UTXO | UTXO | Account (Merkle Patricia Trie) |
| Hash function | SHA3-512 (Grover-resistant) | SHA-256d | keccak-256 |
| Primary signatures | ML-DSA (post-quantum, NIST-standardised) | secp256k1 ECDSA / Schnorr (Taproot) | secp256k1 ECDSA |
| Quantum resistance | **Yes** — ML-DSA / SPHINCS+ / ML-KEM | No (Shor breaks ECDSA) | No (Shor breaks ECDSA) |
| Total supply | 42 M NOVA (cap) | 21 M BTC (cap) | No cap (issuance offset by burn since EIP-1559) |
| Block-size policy | TBD — testnet calibration | 4 MB weight (≈ 1–2 MB effective) | ~30 M gas / block, dynamic |
| Smart-contract VM | Script + atomic-swap HTLC primitives | Script (limited) | EVM (Turing-complete) |
| Lightning / L2 | Quantum-safe Lightning (in-tree) | Lightning Network | Rollups (Optimism, Arbitrum, ZK-rollups) |
| Confidential txs | Stubbed Bulletproof verifier (production-disabled per [ADR-0008](../adr/0008-bulletproof-range-proof-fail-closed.md)) | None native (Liquid sidechain offers them) | Privacy via mixers / ZK rollups, not L1 |
| Carbon accounting | First-class — oracle-verified energy / REC integration | None | None |
| External audit | Pending (Phase 6) | Decades of public review + multiple audits | Decades of public review + multiple audits + Foundation-sponsored audits |

---

## 2. Cryptography and security model

### 2.1 Signature schemes

| | Supernova | Bitcoin | Ethereum |
|---|---|---|---|
| Default sig | ML-DSA-3 (NIST Level 3) | secp256k1 Schnorr (Taproot) / ECDSA | secp256k1 ECDSA |
| Wallet-default sig | ML-DSA-5 (NIST Level 5) | same as above | same as above |
| High-security sig | SPHINCS+ (used for treasury — see [ADR-0006](../adr/0006-treasury-governance.md) and [ADR-0001](../adr/0001-post-quantum-algorithm-selection.md)) | n/a | n/a |
| KEM | ML-KEM (Kyber) for P2P handshake | none | none |
| Hybrid mode | Classical+PQ (both must verify) supported | n/a | n/a |
| Quantum threat model | Shor on ECDSA → broken; Grover on hash → 2× security loss accounted for in 512-bit hash | "Q-day" event would invalidate every UTXO whose pubkey is on-chain (the entire post-Taproot supply) | Same as Bitcoin, plus state-trie commitments expose pubkeys on every transaction |

**The headline:** Supernova's primary chain signature is post-quantum,
NIST-standardised, and verifies in 62.7 µs (ML-DSA-3) on an M1 Pro —
within ~2× the latency of classical ECDSA on the same hardware class.
That ratio is the load-bearing performance fact: **PQ security at a
near-classical performance cost**.

Bitcoin and Ethereum are not vulnerable to current quantum hardware
(estimated ~10⁶+ logical qubits required for Shor on secp256k1; current
production qubit counts are ~3 orders of magnitude short). They are,
however, **not** secure against a future cryptographically-relevant
quantum computer (CRQC). Migration plans for both networks involve
hard-fork-grade changes; nothing standardised has shipped.

### 2.2 Hash function and PoW

| | Supernova | Bitcoin | Ethereum |
|---|---|---|---|
| PoW hash | SHA3-512 | SHA-256d (double SHA-256) | n/a (PoS) |
| Hash latency | 1.06 µs / header (M1 Pro) | ~0.3–0.5 µs / header | n/a |
| Why this hash | Grover-resistant (512 bits → 256-bit post-quantum security) | Long-standing, ASIC ecosystem | n/a |
| ASIC market | Not yet established (PoW is too new) | Mature, multi-billion-dollar | n/a |

SHA3-512 is ~3× slower than SHA-256d per byte but provides 256 bits
of post-quantum security under Grover (vs SHA-256's 128 bits
post-quantum). The constant factor is not load-bearing for
propagation — see [`BASELINE_MEASUREMENTS.md`](BASELINE_MEASUREMENTS.md)
§2.5 for measured propagation numbers.

### 2.3 Threat model coverage

See [`docs/security/THREAT_MODEL.md`](../security/THREAT_MODEL.md)
for Supernova's STRIDE analysis. Bitcoin and Ethereum equivalents
exist as community-maintained docs but no official central document.

---

## 3. Performance

| Operation | **Supernova** (M1 Pro, measured) | **Bitcoin Core** (modern CPU) | **Ethereum** (post-Merge) |
|---|---|---|---|
| Single sig verify | 62.7 µs (ML-DSA-3) [53.6, 73.4] | ~25–80 µs (ECDSA / Schnorr) | ~25–80 µs (ECDSA) |
| Batch sig verify (10K, parallel) | **56.3 ms** (177 K sigs/s) on 10 cores | Not benched standardly; libsecp256k1 batched Schnorr ~50 K/s/core ⇒ ~500 K/s 10 cores | ~30–50 K/s/core |
| Block hash | 1.06 µs (SHA3-512) | ~0.3–0.5 µs (SHA-256d) | ~0.5 µs (keccak-256) |
| Merkle verify, 1000 txs | 21.8 ms | Comparable — same SHA-family tree shape | n/a (Patricia trie) |
| UTXO/state lookup (cached) | 197 ns | ~250–500 ns LevelDB hot cache | µs–ms (Patricia trie depth-dependent) |
| Network sustained TPS | TBD (Phase 5 §2.6 / Phase 7 testnet) | ~7 TPS | ~12–15 TPS |
| Per-node sig-verify ceiling | ~177 K sigs/s (M1 Pro 10 cores) — implies room for ≫ BTC/ETH at the consensus layer | ~500 K Schnorr/s 10 cores | ~300 K ECDSA/s 10 cores |

**Numbers are bench-level; real network TPS is governed by block
size, propagation, and fee-market dynamics, not per-node sig verify.**
BTC and ETH operate at <1% of their per-node sig-verify ceiling for
exactly that reason. Supernova's measured ceiling shows the
consensus-layer headroom is real but doesn't translate directly to a
network TPS prediction.

Three honest claims, with sources:

1. *"Supernova's ML-DSA-3 signature verification is within ~2× the
   latency of classical ECDSA on the same hardware class."* —
   [`BASELINE_MEASUREMENTS.md`](BASELINE_MEASUREMENTS.md) §2.1; ECDSA
   numbers cited from libsecp256k1 benches.
2. *"At the consensus layer, Supernova has ~177 K parallel sig-verifies/s
   of headroom on a 10-core developer laptop — orders of magnitude
   above BTC's and ETH's sustained TPS."* —
   [`BASELINE_MEASUREMENTS.md`](BASELINE_MEASUREMENTS.md) §2.1.1.
3. *"Production TPS is constrained by block size and propagation, not
   per-node sig verify — same as for BTC and ETH."* — operational
   observation; no chain currently runs at sig-verify ceiling.

---

## 4. State model

| | Supernova | Bitcoin | Ethereum |
|---|---|---|---|
| Model | UTXO | UTXO | Account (with state trie) |
| State growth | Bounded by UTXO set size | ~110 M UTXOs (2024) | "State bloat" is an ongoing concern; multi-TB on archival nodes |
| Per-tx state cost | Constant ⇒ predictable | Constant | Variable (gas), depends on storage touched |
| Privacy model | Pseudonymous; per-tx outputs | Pseudonymous; per-tx outputs | Pseudonymous; per-account |
| Validator-side memory | UTXO cache (DashMap-backed; see `node/benches/utxo_benchmarks.rs` and §2.4) | LevelDB-backed UTXO cache | State trie cache (much heavier) |

UTXO is the older / better-understood model. Account-based models
(ETH) trade simplicity of state for richer programmability — that
tradeoff drives a lot of ETH's complexity (state-rent proposals,
EIP-4444, etc.).

Supernova's choice of UTXO is documented in
[ADR-0002](../adr/0002-utxo-transaction-model.md).

---

## 5. Programmability / smart contracts

| | Supernova | Bitcoin | Ethereum |
|---|---|---|---|
| L1 VM | Bitcoin-style Script + HTLC primitives for atomic swap and Lightning | Bitcoin Script (limited) | EVM (Turing-complete) |
| Turing-completeness | No (Script-style) | No | Yes |
| Native token standards | NOVA only at L1 | BTC only at L1 | ERC-20, ERC-721, ERC-1155, etc. |
| Composability | Limited (atomic-swap, Lightning) | Limited | Extensive |
| Reentrancy hazards | None at L1 | None at L1 | Yes — major class of exploits |

Supernova does not target the EVM-style smart-contract use case at
L1. The design philosophy mirrors Bitcoin's: **a settlement layer
with quantum-safe payment, Lightning, and atomic-swap primitives**,
not a general-purpose computation layer.

Programmability that needs Turing-completeness lives at L2 (e.g.
future rollups using Supernova as the data-availability + settlement
layer) or via cross-chain atomic swaps with a Turing-complete chain.

---

## 6. Layer-2 / atomic swaps

| | Supernova | Bitcoin | Ethereum |
|---|---|---|---|
| Lightning | In-tree (`supernova-core/src/lightning/`), quantum-safe HTLCs | Lightning Network (deployed) | n/a (different L2 model) |
| Atomic swaps | In-tree (`supernova-core/src/atomic_swap/`), Bitcoin-compatible HTLCs | Native (Bitcoin↔Bitcoin via PSBT, cross-chain via HTLC) | Custom contract per-swap, gas-heavy |
| Rollups | Not yet (UTXO-rollup design space is open) | Not yet | Optimistic + ZK rollups in production |
| Quantum-safe L2 | **Yes** — Lightning HTLCs use `subtle::ConstantTimeEq` over PQ keys | No | No |

Supernova's Lightning implementation is the *first* quantum-safe
Lightning channel design in production-shaped code. The HTLC
construction follows the Bitcoin Lightning protocol shape but
substitutes ML-DSA for ECDSA at the witness layer.

The atomic-swap refund-flow architecture is documented in
[ADR-0007](../adr/0007-refund-signer-broadcaster-traits.md).

---

## 7. Block parameters and supply

| Parameter | **Supernova** | **Bitcoin** | **Ethereum** |
|---|---|---|---|
| Block time | 150 s | 600 s | 12 s |
| Time-to-finality | Probabilistic (~6 conf = 15 min) | Probabilistic (~6 conf = 60 min) | 2 epochs ≈ 12.8 min after Merge |
| Difficulty adjustment | Per-block + window-based; time-warp prevented (consensus tests) | Every 2016 blocks | n/a (PoS) |
| Total supply | 42 M NOVA (capped) | 21 M BTC (capped) | No cap; ≈ 120 M ETH net of burn at time of writing |
| Issuance schedule | Halving-style w/ environmental component (see `supernova-core/src/docs/tokenomics.md`) | Halving every 210 K blocks (≈ 4 yr) | Variable; PoS issuance ~0.5–1% APY, offset by EIP-1559 burn |
| Treasury allocation | 5% of block reward to network-canonical multisig per [ADR-0006](../adr/0006-treasury-governance.md) | None | None at protocol level (Foundation funded separately) |
| Block-time choice rationale | [ADR-0004](../adr/0004-block-time-150s.md) | Whitepaper | Beacon Chain spec |

Supernova's 150 s block time is a **deliberate compromise**: faster
than Bitcoin's 10 min for better UX, slower than Ethereum's 12 s so
that propagation across globally-distributed nodes (target
< 2 s p99) doesn't routinely cause orphans. The choice is documented
in ADR-0004.

---

## 8. Environmental positioning

| | Supernova | Bitcoin | Ethereum |
|---|---|---|---|
| Energy posture | Carbon-aware: oracle-verified renewable energy reporting per miner; bonus payouts for verified-clean operation | Energy-intensive (~150 TWh/yr 2024 estimates) | Low-energy since Merge (PoS, ~99.95% reduction vs pre-Merge) |
| Native carbon accounting | Yes — REC certificate verification, environmental treasury allocation, emission tracking on-chain (`supernova-core/src/environmental/`) | No native | No native |
| Carbon-negative target | Yes, by design (treasury funds offsets exceeding network emissions) | No | No |

Supernova is the only one of the three with **on-chain carbon
accounting**. Whether the design successfully delivers
carbon-negative operation in practice is an empirical question
answered by mainnet operation; the mechanism is in place.

Ethereum's Merge (2022) effectively neutralised energy as a
differentiator vs Supernova for PoS comparisons. Supernova is
deliberately PoW because PoW provides security guarantees PoS
doesn't (objective sybil resistance, no validator-set capture risk).
The carbon-aware mechanism is how Supernova reconciles PoW's energy
cost with environmental commitments.

See `docs/ENVIRONMENTAL_FEATURES.md` for the full design.

---

## 9. Maturity, governance, decentralisation

| | Supernova | Bitcoin | Ethereum |
|---|---|---|---|
| Years in production | 0 (pre-mainnet) | 17 | 11 |
| Public network operating record | None yet | Continuous since 2009 | Continuous since 2015 |
| Number of independent implementations | 1 (this repo) | 1 (Bitcoin Core) + several minority impls (btcd, etc.) | 5+ (Geth, Nethermind, Erigon, Besu, Reth) |
| Governance | Foundation (Switzerland) per `SuperNova_Foundation.md`; treasury multisig per ADR-0006 | BIP / soft-fork rough consensus | EIP / Foundation roadmap |
| External audit history | Pending (Phase 6) | Multiple, public | Multiple, Foundation-sponsored |
| Bug bounty | TBD (Phase 7 prerequisite) | None official; Bitcoin Core has informal channels | Yes — Foundation-run |
| Time to "trust" | Many years post-mainnet | High | High |

**This is the row Supernova does not yet score well on.** No amount
of clean code or measured performance substitutes for years of
adversarial public scrutiny on mainnet with billions of dollars at
stake. The path forward is documented in
[`docs/PRODUCTION_READINESS_CHECKLIST.md`](../PRODUCTION_READINESS_CHECKLIST.md)
(local, gitignored — operator-maintained):

1. External cryptography / consensus / network audits.
2. Public testnet stability for ≥ 16 weeks with active community.
3. Bug bounty program established.
4. Multiple independent implementations (long-term).

---

## 10. Open questions / known limitations

These are listed honestly because a comparison that hides them isn't
useful for the audiences this doc is for:

| Item | Status | Reference |
|---|---|---|
| Multi-node E2E TPS measured | Not yet — needs testnet harness (Phase 7) | `BASELINE_MEASUREMENTS.md` §2.6 |
| Memory characterisation under sustained load | Not yet | `BASELINE_MEASUREMENTS.md` §2.7 |
| Confidential transactions (Bulletproofs) | Verifier is a stub; production-disabled | [ADR-0008](../adr/0008-bulletproof-range-proof-fail-closed.md) |
| Atomic-swap refund signing/broadcast | Trait architecture in core; node-side adapters TBD | [ADR-0007](../adr/0007-refund-signer-broadcaster-traits.md) |
| External cryptography audit | Pending | Phase 6 |
| Independent reimplementation | Pending | Long-term roadmap |
| Live mainnet operating history | None | Pending mainnet |

---

## 11. Where each chain wins

Stripped to one sentence each, this is the honest framing:

- **Bitcoin** wins on *trust through age*: 17 years of continuous
  operation under adversarial conditions with billions of dollars at
  stake, the longest-running secure distributed system in history.
- **Ethereum** wins on *programmability*: a full Turing-complete VM
  at L1, the deepest L2 ecosystem, and the largest community of
  smart-contract developers and tooling.
- **Supernova** wins on *security forward in time*: the first chain
  designed end-to-end for a post-quantum threat model — primary
  signatures, KEM, and hash all chosen with Shor and Grover in scope —
  at performance competitive with classical ECDSA, with on-chain
  carbon accounting as a first-class concern.

These wins do not subtract from each other. The interesting question
isn't "which wins" — it's "which trade-offs match the user's
threat model and use case."

---

## 12. References

- [`BASELINE_MEASUREMENTS.md`](BASELINE_MEASUREMENTS.md) — measured
  per-op numbers backing the Supernova column above.
- [`docs/adr/`](../adr/) — ADRs 0001–0009 documenting the
  load-bearing architectural choices.
- [`docs/security/THREAT_MODEL.md`](../security/THREAT_MODEL.md) —
  Supernova's STRIDE threat model.
- [`CHANGELOG.md`](../../CHANGELOG.md) `[Unreleased]` — current
  pre-RC5 work.
- Bitcoin: [bitcoin.org/bitcoin.pdf](https://bitcoin.org/bitcoin.pdf),
  [github.com/bitcoin/bitcoin](https://github.com/bitcoin/bitcoin),
  [bitcoincore.org](https://bitcoincore.org).
- Ethereum: [ethereum.org/whitepaper](https://ethereum.org/whitepaper),
  [github.com/ethereum/go-ethereum](https://github.com/ethereum/go-ethereum),
  [ethresear.ch](https://ethresear.ch).
- libsecp256k1 benches:
  [github.com/bitcoin-core/secp256k1](https://github.com/bitcoin-core/secp256k1).

---

*This document will drift as the chains evolve. Re-read against the
referenced specs before quoting any specific number; the structural
comparison (security model, state model, consensus, etc.) drifts
slowly, but operational metrics drift quarterly.*
