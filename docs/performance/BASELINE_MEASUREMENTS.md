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
| Hardware | Apple M1 Pro, 10 cores (8 perf + 2 efficiency), 16 GiB unified memory, NVMe |
| OS / kernel | macOS (Darwin 23.5.0) |
| Rust toolchain | `rustc 1.90.0 (1159e78c4 2025-09-14)` |
| Build profile | `--release` |
| Date measured | 2026-04-26 |
| Commit | `88a6b2c5fcd243b6b648ba27aaf12a402a720c54` (`demo refund flow`) |

> **Note on dev-laptop measurement.** The numbers in §2.5 below were taken
> on a developer laptop, not the target server hardware. Use them as a
> sanity-check baseline (regression detection on the same machine) rather
> than as a production capacity claim. Re-run on production-class hardware
> before quoting peak throughput.

---

## 2. Measurement categories

### 2.1 Single-node signature verification (bench)

The hot path on one node is dominated by post-quantum signature
verification. This is the ceiling on single-node TPS: whatever number
`signature_verify/dilithium/5` produces, the node cannot exceed it on one
core, no matter how the rest of the stack is tuned.

| Operation | p50 latency | Throughput (ops/s/core) | Notes |
|---|---|---|---|
| `signature_verify/dilithium/2` | 28.9 µs | 34.6 K ops/s | Security level Low (ML-DSA-44) |
| `signature_verify/dilithium/3` | 43.2 µs | 23.2 K ops/s | Security level Medium (ML-DSA-65) — default |
| `signature_verify/dilithium/5` | 69.2 µs | 14.4 K ops/s | Security level High (ML-DSA-87) — wallet default |
| `signature_sign/dilithium/2` | TBD | TBD | Signer perspective |
| `signature_sign/dilithium/3` | TBD | TBD | Signer perspective |
| `signature_sign/dilithium/5` | TBD | TBD | Signer perspective |

> Numbers above were collected with `cargo bench -- --quick`, which uses
> a shorter convergence window than the full statistical-sample mode.
> Re-run without `--quick` before quoting figures with confidence intervals.

Source: `supernova-core/benches/tps_harness.rs::bench_signature_verify`.

### 2.1.1 Batch signature verification (bench)

Lattice-based PQ signatures do not admit algebraic batch verification the
way BLS or ECDSA-with-random-linear-combinations do. The only win from a
batch is parallel verification across cores. These rows make that
explicit: `sequential` should scale linearly with batch size at ~1× the
single-verify throughput; `parallel` should scale to physical-core count
before memory-bandwidth effects flatten the curve.

Target from the planning document: **10 000 ML-DSA signatures verified
in under 3 seconds**. That is only achievable with parallel verification
on a multi-core host.

| Operation | Total duration | Per-sig latency | Throughput (sigs/s) |
|---|---|---|---|
| `batch_sequential/dilithium_3/10` | 436 µs | 43.6 µs | 22.9 K |
| `batch_sequential/dilithium_3/100` | 5.46 ms | 54.6 µs | 18.3 K |
| `batch_sequential/dilithium_3/1000` | 43.5 ms | 43.5 µs | 23.0 K |
| `batch_sequential/dilithium_3/10000` | 435 ms | 43.5 µs | 23.0 K |
| `batch_parallel/dilithium_3/10` | 130 µs | 13.0 µs | 76.9 K |
| `batch_parallel/dilithium_3/100` | 770 µs | 7.70 µs | 130 K |
| `batch_parallel/dilithium_3/1000` | 6.05 ms | 6.05 µs | 165 K |
| `batch_parallel/dilithium_3/10000` | 56.8 ms | 5.68 µs | **176 K** |

**Plan target check:** the master plan requires *10 000 ML-DSA-3
signatures verified in under 3 seconds*. Measured: **56.8 ms** for 10 K
parallel verifies on this 10-core M1 Pro — ~50× faster than the target.
Single-core saturation flat-lines at ~23 K sigs/s; parallel scales
roughly with core count up to ~176 K sigs/s, the actual TPS ceiling for
sig-verify-bound workloads on this hardware.

Source: `supernova-core/benches/tps_harness.rs::bench_signature_verify_batch_*`.

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

**Per-bench numbers (M1 Pro, see §1):**

| Operation | p50 latency (median) | Throughput |
|---|---|---|
| `utxo_lookup/cached/10 K` | 197 ns | 5.07 M ops/s |
| `utxo_lookup/cached/100 K` | 612 ns | 1.63 M ops/s |
| `utxo_lookup/cached/500 K` | 749 ns | 1.34 M ops/s |
| `utxo_lookup/miss/10 K` | 220 ns | 4.54 M ops/s |
| `utxo_lookup/miss/100 K` | 240 ns | 4.16 M ops/s |
| `utxo_lookup/miss/500 K` | 222 ns | 4.51 M ops/s |
| `utxo_add/single` | 34.9 µs | 28.7 K ops/s |
| `utxo_spend/single` | 1.55 ns | 644 M ops/s |
| `utxo_flush/batch=1 K` | 2.28 ms | 438 K ops/s (≈ 2.28 µs/UTXO) |
| `utxo_flush/batch=5 K` | 9.06 ms | 552 K ops/s (≈ 1.81 µs/UTXO) |
| `utxo_flush/batch=10 K` | 17.9 ms | 558 K ops/s (≈ 1.79 µs/UTXO) |
| `hit_rate_workload` (mixed) | 970 µs | 1.03 M ops/s |

**Reading the numbers:**

- **Cached-lookup latency grows with set size** (197 ns @ 10 K → 749 ns
  @ 500 K). The LRU eviction window starts dominating once the working
  set exceeds the cache capacity. Miss-lookup latency stays flat
  (~220 ns) across sizes — `DashMap` lookups don't depend on cache
  state.
- **`utxo_spend/single_spend` at 1.55 ns is implausibly fast for a real
  DB write**, suggesting the bench is measuring an in-memory bitset
  flip rather than the durable write path. Flag for next-pass
  refurbishment of the harness.
- **Flush amortises well**: per-UTXO cost drops from 2.28 µs (batch=1 K)
  to 1.79 µs (batch=10 K) as sync overhead is spread across more rows.
- **Mixed workload sustains ~1 M ops/s** at 80% reads / 10% adds /
  10% spends.

| Roll-up metric | Target | Measured (M1 Pro) |
|---|---|---|
| p99 lookup latency @ 500 K UTXOs | < 1 ms | well within (max p50 < 1 µs) |
| Cache hit rate, typical load | > 90% | TBD (workload-shape dependent; see harness) |
| Memory within configured limit | yes | TBD (re-measure under sustained load) |

### 2.5 Block propagation — per-hop cost (bench)

Propagation time across a topology is `Σ per_hop_cpu + Σ per_link_latency`.
This harness measures the per-hop-CPU half on a fixed machine, so a
regression (for example, an expensive `Serialize` impl) is visible. The
per-link-latency half is a network property and belongs to the
multi-node testnet run (§2.6).

| Operation | p50 latency (median) | Throughput |
|---|---|---|
| `block_header_hash/sha3_over_header` | 1.06 µs | 944 K ops/s |
| `block_verify_pow/header_only` | 974 ns | 1.03 M ops/s |
| `block_verify_merkle/tx_count=10` | 242 µs | 41.3 K tx/s |
| `block_verify_merkle/tx_count=100` | 2.58 ms | 38.8 K tx/s |
| `block_verify_merkle/tx_count=1000` | 21.8 ms | 45.8 K tx/s |
| `block_serialise/tx_count=10` | 30.7 µs | 1.45 GiB/s |
| `block_serialise/tx_count=100` | 308 µs | 1.44 GiB/s |
| `block_serialise/tx_count=1000` | 3.86 ms | 1.15 GiB/s |
| `block_deserialise/tx_count=10` | 62.3 µs | 732 MiB/s |
| `block_deserialise/tx_count=100` | 511 µs | 890 MiB/s |
| `block_deserialise/tx_count=1000` | 5.04 ms | 903 MiB/s |

Per-bench statistical confidence intervals (criterion p95): see
`target/criterion/<group>/<bench>/report/index.html` after a fresh run.
Outlier counts ranged from 5 to 18 per 100 samples; the medians above are
robust to those.

**Reading the numbers:**

- **Header hash + PoW check** are sub-microsecond — accept-for-forwarding
  decisions don't bottleneck propagation.
- **Merkle verification scales linearly in tx count** (10 → 100 → 1000
  txs ≈ 10× → 90× cost). Throughput holds ~40 K tx/s across scales,
  consistent with O(n) hash work dominating.
- **Serialisation throughput ~1.4 GiB/s** vastly exceeds typical link
  bandwidth (≤ 10 Gbit/s = 1.25 GiB/s); deserialisation at ~900 MiB/s
  is the lower-throughput half of the codec but still well above any
  real network pipe.
- These numbers are upper bounds on this M1 Pro machine; the multi-node
  testnet E2E numbers in §2.6 will be lower because they include link
  latency.

Source: `node/benches/propagation.rs`.

### 2.6 Block propagation — multi-node (deferred)

End-to-end propagation time across a real topology. Requires the
multi-node testnet harness that is not yet in-tree. The per-hop numbers
in §2.5 are a lower bound on each relay's contribution.

| Metric | Target | Measured |
|---|---|---|
| Time-to-99%-peers, 10-node, 0–200 ms latency | < 2 s | TBD |

### 2.7 Memory characterisation

The mempool-admission hot path is profiled by
`supernova-core/examples/memory_profile.rs` with the `dhat` global
allocator. That gives per-transaction allocation numbers; the
per-process peak RSS rows for node/miner/wallet are still pending a
full-node run under `valgrind --tool=massif`.

First-principles budgets are documented in
`docs/operations/PERFORMANCE_TUNING.md` §Memory profiling and budgets
(Default 8 GiB profile). Record measured peaks below once collected.

| Role | Workload | Budget (PERFORMANCE_TUNING.md) | Measured peak RSS |
|---|---|---|---|
| Full node | Headers-first sync, 24 h | ~4.5 GiB | TBD |
| Full node | Mempool at configured cap (300 MiB) | ~4.5 GiB | TBD |
| Miner | Block assembly, sig cache warm | ~4.5 GiB | TBD |
| Wallet | ML-DSA signing, HD derivation | ~500 MiB | TBD |

Per-transaction allocation (dhat, `--tx-count 10000`):

| Metric | Measured |
|---|---|
| Total bytes allocated | TBD |
| Total allocation blocks | TBD |
| Peak heap live (`At t-gmax`) | TBD |

A regression is any >10% increase in `At t-gmax` at the same
`--tx-count` against the previous baseline.

### 2.8 Chaos / load

Strategy, invariants, and per-scenario run instructions live in
[`docs/testing/CHAOS_TESTING.md`](../testing/CHAOS_TESTING.md). This
section records the measured outcomes of running the scenarios.

| Scenario | Status | Result |
|---|---|---|
| §3.1 Partition-and-heal | In-tree scenario available | TBD |
| §3.2 Crash-and-restart under load | Primitive available, scenario pending | TBD |
| §3.3 Clock-drift skew | Primitive available, scenario pending | TBD |
| §3.4 Byzantine oracle | Unit-level covered, multi-node deferred | TBD |
| §3.5 24-hour 10-node mixed soak | Deferred — requires multi-node testnet | TBD |

A scenario is green when all invariants in `CHAOS_TESTING.md` §1 hold:
safety (no divergent tips), liveness (each running node advances),
bounded reorg depth, no silent fork retention, zero consensus-path
panics.

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

# Heap allocation profile for the mempool-admission hot path.
# Emits `dhat-heap.json` in cwd; open with
# https://nnethercote.github.io/dh_view/dh_view.html and copy the
# three peak numbers into §2.7.
cargo run --release --example memory_profile \
    -p supernova-core --features dhat-heap -- --tx-count 10000
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
- **`SignatureVerifier::batch_verify_transactions` is a stub**
  (`supernova-core/src/crypto/signature.rs`, near "This is a placeholder
  for batch transaction verification"). It groups transactions by scheme
  as documented, but then returns `Ok(true)` without actually calling
  the per-scheme verifier. Any caller relying on it as a security check
  will accept invalid signatures. Until that is wired up, the bench
  numbers in §2.1.1 describe the performance of the correct primitive
  (`verify_quantum_signature`), **not** of `batch_verify_transactions`.
  Resolution is tracked alongside the consensus-verification cleanup.
- **Multi-node testnet harness** (4-node cross-region) is not in-tree.
  The planning document describes the shape; implementation is deferred
  to track E1 of Phase 5.
- **Orphaned chaos test files.** `node/src/tests/chaos_testing.rs`,
  `clock_drift_tests.rs`, `network_partition_tests.rs`,
  `large_block_tests.rs`, and `fork_handling.rs` reference a
  `crate::network::{NetworkSimulator, NodeHandle, NetworkCondition,
  NodeConfig}` path that does not exist in the current architecture.
  The canonical simulator lives at
  `supernova-core/src/testnet/network_simulator.rs`. The `tests/mod.rs`
  stub is not imported from `node/src/lib.rs`, so these files
  contribute zero to the test run. They should either be rewritten
  against the real testnet API or deleted. See
  `docs/testing/CHAOS_TESTING.md` §5.
- **Memory profile under mempool-at-capacity** has not been captured.
  Sizing guidance in `docs/operations/PERFORMANCE_TUNING.md` is
  first-principles, not measured.

---

## Related

- `supernova-core/benches/tps_harness.rs` — TPS harness source
- `node/benches/utxo_benchmarks.rs` — UTXO cache harness
- [`../operations/PERFORMANCE_TUNING.md`](../operations/PERFORMANCE_TUNING.md)
  — operator-facing sizing guidance, to be updated from these numbers
