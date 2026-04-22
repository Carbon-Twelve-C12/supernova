# Baseline performance measurements

This document is the canonical home for Supernova's measured performance
numbers. It is populated by running the benchmark harnesses in this
repository on a specific hardware target and recording the results below.

It is **not** a promise sheet. Numbers drift with release, hardware, and
workload shape — the goal is to make regressions visible, not to advertise
peak throughput.

> Entries with `TBD` have not yet been measured on this hardware. Run the
> commands in [§3](#3-how-to-populate) to fill them in.

---

## 1. Target hardware and configuration

All numbers below must be re-measured and re-recorded when the target
hardware changes. The release profile is mandatory (`--release`); debug
numbers are not meaningful.

| Field | Value |
|---|---|
| Hardware | TBD (example: Hetzner AX52, AMD Ryzen 7 7700, 64 GiB ECC, NVMe) |
| OS / kernel | TBD |
| Rust toolchain | TBD (`rustc --version`) |
| Build profile | `--release` |
| Date measured | TBD (ISO-8601) |
| Commit | TBD (`git rev-parse HEAD`) |

---

## 2. Measurement categories

### 2.1 Single-node signature verification (bench)

The hot path on one node is dominated by post-quantum signature
verification. This is the ceiling on single-node TPS: whatever number
`signature_verify/dilithium/5` produces, the node cannot exceed it on one
core, no matter how the rest of the stack is tuned.

| Operation | p50 latency | Throughput (ops/s/core) | Notes |
|---|---|---|---|
| `signature_verify/dilithium/2` | TBD | TBD | Security level Low (ML-DSA-44) |
| `signature_verify/dilithium/3` | TBD | TBD | Security level Medium (ML-DSA-65) — default |
| `signature_verify/dilithium/5` | TBD | TBD | Security level High (ML-DSA-87) — wallet default |
| `signature_sign/dilithium/2` | TBD | TBD | Signer perspective |
| `signature_sign/dilithium/3` | TBD | TBD | Signer perspective |
| `signature_sign/dilithium/5` | TBD | TBD | Signer perspective |

Source: `supernova-core/benches/tps_harness.rs::bench_signature_verify`.

### 2.2 Transaction validation (bench)

The structural half of mempool admission — shape, overflow, dust checks,
duplicate-input detection. Signature verification is separate (measured
above) because it is re-run against the UTXO set at block-validation time.

| Operation | p50 latency | Throughput (ops/s/core) |
|---|---|---|
| `transaction_validate/one_in_two_out` | TBD | TBD |

Source: `supernova-core/benches/tps_harness.rs::bench_transaction_validate`.

### 2.3 Wire-format round-trip (bench)

Bincode encode / decode per transaction. Should be small next to sig
verify; a regression here silently halves P2P throughput.

| Operation | p50 latency | Throughput (ops/s/core) |
|---|---|---|
| `transaction_roundtrip/encode` | TBD | TBD |
| `transaction_roundtrip/decode` | TBD | TBD |

Source: `supernova-core/benches/tps_harness.rs::bench_transaction_roundtrip`.

### 2.4 UTXO cache (bench)

Already measured in a separate harness:
`node/benches/utxo_benchmarks.rs`. Target metrics live in that file's
header comment; record the measured values here once the harness is run.

| Metric | Target | Measured |
|---|---|---|
| p99 lookup latency @ 1M UTXOs | < 1 ms | TBD |
| Cache hit rate, typical load | > 90% | TBD |
| Memory within configured limit | yes | TBD |

### 2.5 Block propagation — per-hop cost (bench)

Propagation time across a topology is `Σ per_hop_cpu + Σ per_link_latency`.
This harness measures the per-hop-CPU half on a fixed machine, so a
regression (for example, an expensive `Serialize` impl) is visible. The
per-link-latency half is a network property and belongs to the
multi-node testnet run (§2.6).

| Operation | p50 latency | Throughput |
|---|---|---|
| `block_header_hash/sha3_over_header` | TBD | TBD ops/s |
| `block_verify_pow/header_only` | TBD | TBD ops/s |
| `block_verify_merkle/tx_count=10` | TBD | TBD tx/s |
| `block_verify_merkle/tx_count=100` | TBD | TBD tx/s |
| `block_verify_merkle/tx_count=1000` | TBD | TBD tx/s |
| `block_serialise/tx_count=10` | TBD | TBD MiB/s |
| `block_serialise/tx_count=100` | TBD | TBD MiB/s |
| `block_serialise/tx_count=1000` | TBD | TBD MiB/s |
| `block_deserialise/tx_count=10` | TBD | TBD MiB/s |
| `block_deserialise/tx_count=100` | TBD | TBD MiB/s |
| `block_deserialise/tx_count=1000` | TBD | TBD MiB/s |

Source: `node/benches/propagation.rs`.

### 2.6 Block propagation — multi-node (deferred)

End-to-end propagation time across a real topology. Requires the
multi-node testnet harness that is not yet in-tree. The per-hop numbers
in §2.5 are a lower bound on each relay's contribution.

| Metric | Target | Measured |
|---|---|---|
| Time-to-99%-peers, 10-node, 0–200 ms latency | < 2 s | TBD |

### 2.7 Memory characterisation (deferred)

Requires `dhat` or `massif` integration. Placeholder until track E4 lands.

| Role | Peak RSS under load | Measured |
|---|---|---|
| Full node, sync | TBD | TBD |
| Miner, block assembly | TBD | TBD |
| Wallet, signing | TBD | TBD |

### 2.8 Chaos / load (deferred)

24-hour 10-node run with injected faults (crashes, partitions, clock
drift). Owned by track E5.

---

## 3. How to populate

Run in order, recording output into the tables above.

### 3.1 Single-node benches

```bash
# TPS harness (signatures, validation, wire format)
cargo bench -p supernova-core --bench tps_harness

# UTXO cache benches
cargo bench -p supernova-node --bench utxo_benchmarks

# Block propagation (per-hop cost)
cargo bench -p supernova-node --bench propagation
```

Criterion writes HTML reports to `target/criterion/`; the p50/p99 numbers
and derived throughput are in the `report/` subdirectory per bench.

### 3.2 Deterministic environment

To keep numbers comparable across runs:

- Pin the CPU governor to `performance` (`cpupower frequency-set -g performance`).
- Disable SMT/turbo for stability, or note that they are enabled.
- Run with `taskset -c 0-3` to isolate noise from other cores.
- Record the `rustc` version and toolchain channel.

### 3.3 Recording a baseline

1. Run the benches.
2. Copy the p50 from each criterion report into the tables above.
3. Update §1 with the measurement hardware, date, and commit.
4. Commit the updated document alongside any code change that affected
   the numbers. A performance regression is a meaningful PR event.

---

## 4. How to interpret these numbers

### A single-node number is a ceiling, not a promise

`signature_verify/dilithium/5` running at *N* ops/sec on one core means a
single-core mempool-admission loop cannot exceed *N* TPS. Real-world TPS
at the chain level depends on:

- How many cores the verifier uses (the validator uses Rayon; scaling is
  sub-linear above 4–8 cores due to memory bandwidth).
- Block size and interval (2.5 minutes).
- Signature-scheme mix — wallets signing Dilithium3 contribute faster
  than those signing Dilithium5.
- Network bandwidth and propagation latency across peers.
- Mempool eviction policy under sustained load.

The single-node number gates the *potential*; the multi-node testnet
number (track E1 of Phase 5) reports the actual sustained rate.

### A "TPS claim" needs a scheme

"Supernova does 1000 TPS" is not a meaningful statement. "Supernova's
mempool admits 1000 Dilithium3 single-input transactions per second per
core on $HARDWARE at $COMMIT" is. Keep the qualifiers in the tables
above; drop them only in marketing copy, never in technical documents.

---

## 5. Known regressions and open questions

- **Comprehensive benchmark at `supernova-core/benches/comprehensive_benchmarks.rs`**
  references the legacy `btclib::` crate path and will not compile
  against the current tree. It is kept for historical continuity;
  `tps_harness.rs` is the active harness. Retiring or porting that file
  is tracked as separate cleanup.
- **Multi-node testnet harness** (4-node cross-region) is not in-tree.
  The planning document describes the shape; implementation is deferred
  to track E1 of Phase 5.
- **Memory profile under mempool-at-capacity** has not been captured.
  Sizing guidance in `docs/operations/PERFORMANCE_TUNING.md` is
  first-principles, not measured.

---

## Related

- `supernova-core/benches/tps_harness.rs` — TPS harness source
- `node/benches/utxo_benchmarks.rs` — UTXO cache harness
- [`../operations/PERFORMANCE_TUNING.md`](../operations/PERFORMANCE_TUNING.md)
  — operator-facing sizing guidance, to be updated from these numbers
