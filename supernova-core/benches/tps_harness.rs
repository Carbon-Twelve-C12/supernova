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

use criterion::{
    black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput,
};

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
    bench_transaction_validate,
    bench_transaction_roundtrip
);
criterion_main!(benches);
