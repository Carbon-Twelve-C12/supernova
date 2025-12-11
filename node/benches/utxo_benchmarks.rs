//! UTXO Performance Benchmarks
//!
//! This benchmark suite validates UTXO cache performance meets production requirements.
//!
//! Target metrics:
//! - < 1ms p99 lookup latency with 1M UTXOs
//! - > 90% cache hit rate for typical usage
//! - Memory usage stays within configured limit

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use parking_lot::RwLock;
use rand::{rngs::StdRng, Rng, SeedableRng};
use std::sync::Arc;
use tempfile::tempdir;

// Import from the node crate
use node::storage::{
    utxo_cache::{UtxoCache, UtxoCacheConfig},
    utxo_set::{OutPoint, UnspentOutput, UtxoSet},
};

/// Create a test UTXO with random values
fn create_test_utxo(rng: &mut StdRng, height: u64) -> UnspentOutput {
    let mut txid = [0u8; 32];
    rng.fill(&mut txid);
    
    UnspentOutput {
        txid,
        vout: rng.gen_range(0..10),
        value: rng.gen_range(1_000..1_000_000_000), // 1k satoshis to 10 BTC
        script_pubkey: vec![0x76, 0xa9, 0x14], // Minimal P2PKH prefix
        height,
        is_coinbase: false,
    }
}

/// Create a random outpoint for lookups
fn random_outpoint(rng: &mut StdRng) -> OutPoint {
    let mut txid = [0u8; 32];
    rng.fill(&mut txid);
    OutPoint::new(txid, rng.gen_range(0..10))
}

/// Set up a populated UTXO cache for benchmarks
fn setup_cache(size: usize) -> (UtxoCache, Vec<OutPoint>) {
    let temp_dir = tempdir().expect("Failed to create temp dir");
    let db_path = temp_dir.path().join("utxo.db");
    let db = Arc::new(RwLock::new(
        UtxoSet::new(&db_path).expect("Failed to create UTXO set"),
    ));

    let config = UtxoCacheConfig {
        max_memory_bytes: 1024 * 1024 * 1024, // 1 GB for large tests
        flush_threshold: 50_000,
        write_back: true,
        max_flush_interval_secs: 300,
        target_hit_rate: 0.90,
        collect_stats: true,
    };

    let cache = UtxoCache::new(db, config);

    // Populate with UTXOs
    let mut rng = StdRng::seed_from_u64(42);
    let mut outpoints = Vec::with_capacity(size);

    for i in 0..size {
        let utxo = create_test_utxo(&mut rng, i as u64);
        let outpoint = OutPoint::new(utxo.txid, utxo.vout);
        outpoints.push(outpoint);
        cache.add(outpoint, utxo);
    }

    // Flush to ensure consistent state
    cache.flush().expect("Failed to flush cache");

    // Keep temp_dir alive by leaking it (benchmark only)
    std::mem::forget(temp_dir);

    (cache, outpoints)
}

/// Benchmark UTXO cache lookup performance
fn bench_utxo_lookup(c: &mut Criterion) {
    let mut group = c.benchmark_group("utxo_lookup");
    
    // Test with different UTXO set sizes
    let sizes = [10_000, 100_000, 500_000];
    
    for size in sizes {
        let (cache, outpoints) = setup_cache(size);
        let mut rng = StdRng::seed_from_u64(123);
        
        group.throughput(Throughput::Elements(1));
        group.bench_with_input(
            BenchmarkId::new("cached_lookup", size),
            &size,
            |b, _| {
                b.iter(|| {
                    // Random lookup from known outpoints (cache hit)
                    let idx = rng.gen_range(0..outpoints.len());
                    black_box(cache.get(&outpoints[idx]))
                })
            },
        );
        
        group.bench_with_input(
            BenchmarkId::new("miss_lookup", size),
            &size,
            |b, _| {
                b.iter(|| {
                    // Random lookup of non-existent UTXO (cache miss)
                    let outpoint = random_outpoint(&mut rng);
                    black_box(cache.get(&outpoint))
                })
            },
        );
    }
    
    group.finish();
}

/// Benchmark UTXO add performance
fn bench_utxo_add(c: &mut Criterion) {
    let mut group = c.benchmark_group("utxo_add");
    
    let temp_dir = tempdir().expect("Failed to create temp dir");
    let db_path = temp_dir.path().join("utxo.db");
    let db = Arc::new(RwLock::new(
        UtxoSet::new(&db_path).expect("Failed to create UTXO set"),
    ));

    let config = UtxoCacheConfig::default();
    let cache = UtxoCache::new(db, config);
    
    let mut rng = StdRng::seed_from_u64(456);
    
    group.throughput(Throughput::Elements(1));
    group.bench_function("single_add", |b| {
        b.iter(|| {
            let utxo = create_test_utxo(&mut rng, 1);
            let outpoint = OutPoint::new(utxo.txid, utxo.vout);
            cache.add(black_box(outpoint), black_box(utxo));
        })
    });
    
    group.finish();
}

/// Benchmark UTXO spend performance
fn bench_utxo_spend(c: &mut Criterion) {
    let mut group = c.benchmark_group("utxo_spend");
    
    // Create a fresh cache for each benchmark iteration
    let (cache, mut outpoints) = setup_cache(50_000);
    let mut rng = StdRng::seed_from_u64(789);
    
    group.throughput(Throughput::Elements(1));
    group.bench_function("single_spend", |b| {
        b.iter(|| {
            if !outpoints.is_empty() {
                let idx = rng.gen_range(0..outpoints.len());
                let outpoint = outpoints.swap_remove(idx);
                black_box(cache.spend(&outpoint))
            } else {
                None
            }
        })
    });
    
    group.finish();
}

/// Benchmark batch flush performance
fn bench_utxo_flush(c: &mut Criterion) {
    let mut group = c.benchmark_group("utxo_flush");
    
    let batch_sizes = [1_000, 5_000, 10_000];
    
    for batch_size in batch_sizes {
        group.throughput(Throughput::Elements(batch_size as u64));
        group.bench_with_input(
            BenchmarkId::new("batch_flush", batch_size),
            &batch_size,
            |b, &size| {
                // Set up fresh cache for each iteration
                let temp_dir = tempdir().expect("Failed to create temp dir");
                let db_path = temp_dir.path().join("utxo.db");
                let db = Arc::new(RwLock::new(
                    UtxoSet::new(&db_path).expect("Failed to create UTXO set"),
                ));

                let config = UtxoCacheConfig {
                    max_memory_bytes: 512 * 1024 * 1024,
                    flush_threshold: size * 2, // Don't auto-flush
                    write_back: true,
                    max_flush_interval_secs: 3600,
                    target_hit_rate: 0.90,
                    collect_stats: false, // Disable stats for perf
                };

                let cache = UtxoCache::new(db, config);
                let mut rng = StdRng::seed_from_u64(999);
                
                b.iter(|| {
                    // Add batch of UTXOs
                    for i in 0..size {
                        let utxo = create_test_utxo(&mut rng, i as u64);
                        let outpoint = OutPoint::new(utxo.txid, utxo.vout);
                        cache.add(outpoint, utxo);
                    }
                    
                    // Flush them
                    black_box(cache.flush().expect("Flush failed"))
                })
            },
        );
    }
    
    group.finish();
}

/// Benchmark hit rate under realistic workload
fn bench_hit_rate_workload(c: &mut Criterion) {
    let mut group = c.benchmark_group("utxo_workload");
    
    // Simulate realistic workload: 80% reads, 10% adds, 10% spends
    let (cache, mut outpoints) = setup_cache(100_000);
    let mut rng = StdRng::seed_from_u64(111);
    
    group.throughput(Throughput::Elements(1000));
    group.bench_function("mixed_workload_1000_ops", |b| {
        b.iter(|| {
            for _ in 0..1000 {
                let op = rng.gen_range(0..100);
                
                if op < 80 && !outpoints.is_empty() {
                    // Read (80%)
                    let idx = rng.gen_range(0..outpoints.len());
                    black_box(cache.get(&outpoints[idx]));
                } else if op < 90 {
                    // Add (10%)
                    let utxo = create_test_utxo(&mut rng, rng.gen());
                    let outpoint = OutPoint::new(utxo.txid, utxo.vout);
                    outpoints.push(outpoint);
                    cache.add(outpoint, utxo);
                } else if !outpoints.is_empty() {
                    // Spend (10%)
                    let idx = rng.gen_range(0..outpoints.len());
                    let outpoint = outpoints.swap_remove(idx);
                    black_box(cache.spend(&outpoint));
                }
            }
        })
    });
    
    // Report hit rate
    let stats = cache.statistics();
    println!("\n=== UTXO Cache Statistics ===");
    println!("Hit rate: {:.2}%", stats.hit_rate() * 100.0);
    println!("Hits: {}, Misses: {}", stats.hits, stats.misses);
    println!("Memory usage: {} bytes", stats.memory_usage);
    println!("Entry count: {}", stats.entry_count);
    
    group.finish();
}

criterion_group!(
    benches,
    bench_utxo_lookup,
    bench_utxo_add,
    bench_utxo_spend,
    bench_utxo_flush,
    bench_hit_rate_workload,
);

criterion_main!(benches);

