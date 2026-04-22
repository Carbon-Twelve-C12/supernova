//! Per-hop block processing benchmarks.
//!
//! Block propagation time across a real peer topology is:
//!
//!   total ≈ Σ per_hop_cpu + Σ per_link_latency
//!
//! The per-link-latency half is a network property — measured on the live
//! testnet, not in a micro-benchmark. The per-hop-CPU half is what every
//! relay pays regardless of network conditions: decode, header-hash,
//! PoW check, merkle verify, encode before forwarding. This harness
//! measures that half on a fixed CPU so a regression (for example, an
//! expensive `Serialize` impl added to a transaction field) is caught.
//!
//! A 10-node topology propagation simulation is explicitly out of scope
//! for this harness; it belongs in the multi-node testnet run that the
//! planning document tracks under Phase 5 E1 follow-up.
//!
//! Run:
//!
//! ```
//! cargo bench -p supernova-node --bench propagation
//! ```

use criterion::{
    black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput,
};

use supernova_core::types::block::{Block, BlockHeader};
use supernova_core::types::transaction::{
    Transaction, TransactionInput, TransactionOutput,
};

// --------------------------------------------------------------------------
// Fixtures
// --------------------------------------------------------------------------

/// Build a transaction of the shape wallets typically produce: one input,
/// two outputs. The signature payload is the size of a Dilithium5
/// signature envelope so serialised size matches reality.
fn synthetic_tx(seed: u8) -> Transaction {
    let input = TransactionInput::new([seed; 32], 0, vec![seed; 4_595], 0xffffffff);
    let payout = TransactionOutput::new(1_000_000, vec![seed; 32]);
    let change = TransactionOutput::new(900_000, vec![seed; 32]);
    Transaction::new(1, vec![input], vec![payout, change], 0)
}

/// Build a block with `tx_count` synthetic transactions. Difficulty bits
/// are set to a value that will *not* satisfy the target — we do not rely
/// on `verify_proof_of_work` returning true, we just time the computation.
fn build_block(tx_count: usize) -> Block {
    let transactions: Vec<Transaction> = (0..tx_count)
        .map(|i| synthetic_tx((i % 255) as u8))
        .collect();

    // Bits value is arbitrary — we are timing, not validating.
    Block::new_with_params(1, [0u8; 32], transactions, 0x1d00ffff)
}

// --------------------------------------------------------------------------
// 1. Header hash
// --------------------------------------------------------------------------
//
// The smallest per-hop cost: SHA3 over the 80-byte serialised header.
// Called at minimum once per received block, twice if the relay does
// optimistic forwarding. Should be sub-microsecond on any modern CPU.

fn bench_header_hash(c: &mut Criterion) {
    let mut group = c.benchmark_group("block_header_hash");
    group.throughput(Throughput::Elements(1));

    let header = BlockHeader::new(1, [0u8; 32], [1u8; 32], 1_700_000_000, 0x1d00ffff, 42);

    group.bench_function("sha3_over_header", |b| {
        b.iter(|| {
            let h = header.hash();
            black_box(h);
        })
    });

    group.finish();
}

// --------------------------------------------------------------------------
// 2. Proof-of-work check
// --------------------------------------------------------------------------
//
// Header hash + bigint-style byte-wise target compare. Cost dominated by
// the hash. Measured separately because a regression here affects the
// fast-path decision at every relay (accept-for-forwarding gate).

fn bench_verify_pow(c: &mut Criterion) {
    let mut group = c.benchmark_group("block_verify_pow");
    group.throughput(Throughput::Elements(1));

    let block = build_block(0);

    group.bench_function("header_only", |b| {
        b.iter(|| {
            let ok = block.verify_proof_of_work();
            black_box(ok);
        })
    });

    group.finish();
}

// --------------------------------------------------------------------------
// 3. Merkle-root verification
// --------------------------------------------------------------------------
//
// Linear in the number of transactions (rebuilds the tree). Dominates
// per-hop cost on full-sized blocks. Parametrise over realistic block
// fills so the regression window is visible at each scale.

fn bench_verify_merkle(c: &mut Criterion) {
    let mut group = c.benchmark_group("block_verify_merkle");

    for tx_count in [10usize, 100, 1_000] {
        let block = build_block(tx_count);
        group.throughput(Throughput::Elements(tx_count as u64));

        group.bench_with_input(
            BenchmarkId::new("tx_count", tx_count),
            &tx_count,
            |b, _| {
                b.iter(|| {
                    let ok = block.verify_merkle_root();
                    black_box(ok);
                })
            },
        );
    }

    group.finish();
}

// --------------------------------------------------------------------------
// 4. Serialise / deserialise (wire format)
// --------------------------------------------------------------------------
//
// Every hop pays one deserialise (incoming) and one serialise (outgoing
// to N peers, but the buffer is produced once and reused). Throughput is
// reported in bytes/second so the result can be compared against typical
// link bandwidth.

fn bench_serialise(c: &mut Criterion) {
    let mut group = c.benchmark_group("block_serialise");

    for tx_count in [10usize, 100, 1_000] {
        let block = build_block(tx_count);
        let size = block.serialize().len();
        group.throughput(Throughput::Bytes(size as u64));

        group.bench_with_input(
            BenchmarkId::new("tx_count", tx_count),
            &tx_count,
            |b, _| {
                b.iter(|| {
                    let bytes = block.serialize();
                    black_box(bytes);
                })
            },
        );
    }

    group.finish();
}

fn bench_deserialise(c: &mut Criterion) {
    let mut group = c.benchmark_group("block_deserialise");

    for tx_count in [10usize, 100, 1_000] {
        let block = build_block(tx_count);
        let bytes = block.serialize();
        group.throughput(Throughput::Bytes(bytes.len() as u64));

        group.bench_with_input(
            BenchmarkId::new("tx_count", tx_count),
            &tx_count,
            |b, _| {
                b.iter(|| {
                    let decoded = Block::deserialize(black_box(&bytes)).expect("decode");
                    black_box(decoded);
                })
            },
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_header_hash,
    bench_verify_pow,
    bench_verify_merkle,
    bench_serialise,
    bench_deserialise
);
criterion_main!(benches);
