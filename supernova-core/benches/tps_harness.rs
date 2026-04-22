//! TPS harness — single-node throughput benchmarks.
//!
//! Supernova's TPS ceiling on one node is dominated by three costs, in
//! descending order of weight:
//!
//!   1. Post-quantum signature verification (Dilithium5: ~2–5 ms/op).
//!   2. Full transaction validation (structural + signature).
//!   3. Wire-format encode/decode (bincode round-trip).
//!
//! A multi-node testnet benchmark sits on top of these numbers; if any of
//! them regresses, the testnet number drops proportionally. Running this
//! harness against a release build gives a deterministic, hardware-pinned
//! baseline that the testnet run can be regressed against.
//!
//! Run with:
//!
//! ```
//! cargo bench -p supernova-core --bench tps_harness
//! ```
//!
//! Criterion emits per-operation latency and derives throughput
//! (`Throughput::Elements(1)`), so the report reads directly as ops/sec.

use std::time::Duration;

use criterion::{
    black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput,
};
use rayon::prelude::*;

use supernova_core::crypto::quantum::{
    sign_quantum, verify_quantum_signature, QuantumKeyPair, QuantumParameters, QuantumScheme,
};
use supernova_core::types::transaction::{
    Transaction, TransactionInput, TransactionOutput,
};
use supernova_core::validation::validate_transaction;

// --------------------------------------------------------------------------
// Fixtures
// --------------------------------------------------------------------------

/// A realistic-shaped transaction (1 in, 2 out) plus its signing keypair.
fn build_fixture_tx() -> Transaction {
    let input = TransactionInput::new([0x11u8; 32], 0, vec![0u8; 64], 0xffffffff);
    let output_primary = TransactionOutput::new(1_000_000, vec![0u8; 32]);
    let output_change = TransactionOutput::new(900_000, vec![0u8; 32]);
    Transaction::new(1, vec![input], vec![output_primary, output_change], 0)
}

/// Dilithium keypair at the requested security level.
fn dilithium_keypair(level: u8) -> QuantumKeyPair {
    let params = QuantumParameters {
        scheme: QuantumScheme::Dilithium,
        security_level: level,
    };
    QuantumKeyPair::generate(params).expect("dilithium keygen")
}

// --------------------------------------------------------------------------
// 1. Signature verification throughput
// --------------------------------------------------------------------------
//
// This is the dominant cost on the hot path. We parametrise by Dilithium
// security level because wallets may be mixed. SPHINCS+ and Falcon are
// available via the same API; add entries below if a deployment pins those.

fn bench_signature_verify(c: &mut Criterion) {
    let mut group = c.benchmark_group("signature_verify");
    group.throughput(Throughput::Elements(1));

    let message = b"supernova tps harness fixed message";

    for level in [2u8, 3, 5] {
        let keypair = dilithium_keypair(level);
        let signature = sign_quantum(&keypair, message).expect("sign");

        group.bench_with_input(
            BenchmarkId::new("dilithium", level),
            &level,
            |b, _| {
                b.iter(|| {
                    let ok = verify_quantum_signature(
                        black_box(&keypair.public_key),
                        black_box(message),
                        black_box(&signature),
                        keypair.parameters,
                    )
                    .expect("verify");
                    assert!(ok);
                })
            },
        );
    }

    group.finish();
}

// --------------------------------------------------------------------------
// 2. Signing throughput
// --------------------------------------------------------------------------
//
// Wallets care about signing cost (user-perceived), nodes care about
// verifying cost (throughput). Both are tracked so regressions are visible
// on either side.

fn bench_signature_sign(c: &mut Criterion) {
    let mut group = c.benchmark_group("signature_sign");
    group.throughput(Throughput::Elements(1));

    let message = b"supernova tps harness fixed message";

    for level in [2u8, 3, 5] {
        let keypair = dilithium_keypair(level);

        group.bench_with_input(
            BenchmarkId::new("dilithium", level),
            &level,
            |b, _| {
                b.iter(|| {
                    let sig = sign_quantum(black_box(&keypair), black_box(message)).expect("sign");
                    black_box(sig);
                })
            },
        );
    }

    group.finish();
}

// --------------------------------------------------------------------------
// 2a. Batch signature verification
// --------------------------------------------------------------------------
//
// Lattice-based PQ signatures (Dilithium, Falcon) do not admit algebraic
// batch verification the way BLS or ECDSA-with-random-linear-combinations
// do. The only win from "batching" is parallelism: split the batch across
// cores and verify each signature independently.
//
// This section measures both paths so the amortisation claim is visible:
//
//   - `batch_sequential` — one-thread, serial loop. Throughput should be
//     N × single-sig throughput, modulo loop overhead.
//   - `batch_parallel` — rayon par_iter. Expect near-linear scaling to
//     physical-core count, then flattening from memory bandwidth and
//     allocator contention.
//
// Runtime tuning: 10k Dilithium5 verifies take ~30 s on a modern CPU, so
// the batch-10k bench uses `sample_size(10)` and a longer measurement
// window. Keygen and sig production happen once outside the timed region.

/// Pre-generate `count` (pubkey, message, signature) triples for a given
/// Dilithium security level. Expensive; done once per bench group.
fn prebuild_signatures(level: u8, count: usize) -> Vec<(Vec<u8>, Vec<u8>, Vec<u8>)> {
    let keypair = dilithium_keypair(level);
    (0..count)
        .map(|i| {
            let message = format!("supernova batch bench #{i}").into_bytes();
            let signature = sign_quantum(&keypair, &message).expect("sign");
            (keypair.public_key.clone(), message, signature)
        })
        .collect()
}

fn bench_signature_verify_batch_sequential(c: &mut Criterion) {
    let mut group = c.benchmark_group("signature_verify_batch_sequential");

    // 10k verifies at ~3 ms/op = 30 s/iter; trim sample count so the
    // whole group finishes in a few minutes.
    group.sample_size(10).measurement_time(Duration::from_secs(120));

    // Single security level; the scaling pattern is the same at other levels.
    let level: u8 = 3;
    let params = QuantumParameters {
        scheme: QuantumScheme::Dilithium,
        security_level: level,
    };
    let max_batch = 10_000usize;
    let fixtures = prebuild_signatures(level, max_batch);

    for batch in [10usize, 100, 1_000, 10_000] {
        let slice = &fixtures[..batch];
        group.throughput(Throughput::Elements(batch as u64));

        group.bench_with_input(
            BenchmarkId::new("dilithium_3", batch),
            &batch,
            |b, _| {
                b.iter(|| {
                    for (pk, msg, sig) in slice.iter() {
                        let ok = verify_quantum_signature(
                            black_box(pk),
                            black_box(msg),
                            black_box(sig),
                            params,
                        )
                        .expect("verify");
                        assert!(ok);
                    }
                })
            },
        );
    }

    group.finish();
}

fn bench_signature_verify_batch_parallel(c: &mut Criterion) {
    let mut group = c.benchmark_group("signature_verify_batch_parallel");
    group.sample_size(10).measurement_time(Duration::from_secs(120));

    let level: u8 = 3;
    let params = QuantumParameters {
        scheme: QuantumScheme::Dilithium,
        security_level: level,
    };
    let max_batch = 10_000usize;
    let fixtures = prebuild_signatures(level, max_batch);

    for batch in [10usize, 100, 1_000, 10_000] {
        let slice = &fixtures[..batch];
        group.throughput(Throughput::Elements(batch as u64));

        group.bench_with_input(
            BenchmarkId::new("dilithium_3", batch),
            &batch,
            |b, _| {
                b.iter(|| {
                    let all_ok = slice
                        .par_iter()
                        .all(|(pk, msg, sig)| {
                            verify_quantum_signature(pk, msg, sig, params)
                                .ok()
                                .unwrap_or(false)
                        });
                    assert!(all_ok);
                })
            },
        );
    }

    group.finish();
}

// --------------------------------------------------------------------------
// 3. Full transaction validation
// --------------------------------------------------------------------------
//
// `validate_transaction` exercises the structural checks the mempool runs
// before admission — shape, overflow, dust, duplicate-input detection. It
// does NOT re-run signature verification against the UTXO set (that happens
// at block validation time, with access to the spent outputs). This bench
// therefore measures the "cheap half" of the admission path — it is a floor
// on throughput, not the full mempool picture.

fn bench_transaction_validate(c: &mut Criterion) {
    let mut group = c.benchmark_group("transaction_validate");
    group.throughput(Throughput::Elements(1));

    let tx = build_fixture_tx();

    group.bench_function("one_in_two_out", |b| {
        b.iter(|| {
            let _ = validate_transaction(black_box(&tx));
        })
    });

    group.finish();
}

// --------------------------------------------------------------------------
// 4. Wire-format round-trip
// --------------------------------------------------------------------------
//
// Bincode encode + decode is what network handlers and storage paths do
// per transaction. It is usually negligible next to sig verify, but a
// regression here (for example, adding an expensive `Serialize` impl)
// would silently halve throughput. Tracking it catches that.

fn bench_transaction_roundtrip(c: &mut Criterion) {
    let mut group = c.benchmark_group("transaction_roundtrip");
    group.throughput(Throughput::Elements(1));

    let tx = build_fixture_tx();
    let encoded = bincode::serialize(&tx).expect("encode");

    group.bench_function("encode", |b| {
        b.iter(|| {
            let bytes = bincode::serialize(black_box(&tx)).expect("encode");
            black_box(bytes);
        })
    });

    group.bench_function("decode", |b| {
        b.iter(|| {
            let tx: Transaction = bincode::deserialize(black_box(&encoded)).expect("decode");
            black_box(tx);
        })
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_signature_verify,
    bench_signature_sign,
    bench_signature_verify_batch_sequential,
    bench_signature_verify_batch_parallel,
    bench_transaction_validate,
    bench_transaction_roundtrip
);
criterion_main!(benches);
