extern crate supernova_core as btclib;

use btclib::storage::utxo_set::{UtxoEntry, UtxoSet};
use btclib::types::transaction::{OutPoint, TxOutput};
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use std::time::Instant;

const NUM_UTXOS: usize = 10_000;
const CACHE_SIZE: usize = 5_000;
const BLOCK_HEIGHT: u32 = 600_000;

fn main() {
    println!("supernova UTXO Set Optimization Demo");
    println!("====================================\n");

    // Create an in-memory UTXO set
    println!(
        "Creating in-memory UTXO set with cache capacity of {} entries...",
        CACHE_SIZE
    );
    let utxo_set = UtxoSet::new_in_memory(CACHE_SIZE);

    // Generate random UTXOs
    println!("Generating {} random UTXOs...", NUM_UTXOS);
    let utxos = generate_random_utxos(NUM_UTXOS);

    // Add UTXOs to the set
    println!("Adding UTXOs to the set...");
    let start = Instant::now();

    for utxo in &utxos {
        utxo_set.add(utxo.clone()).unwrap();
    }

    let add_time = start.elapsed();
    println!(
        "Added {} UTXOs in {:?} ({} UTXOs/sec)",
        NUM_UTXOS,
        add_time,
        NUM_UTXOS as f64 / add_time.as_secs_f64()
    );

    // Update UTXO commitment
    println!("\nUpdating UTXO commitment...");
    let start = Instant::now();
    let commitment = utxo_set.update_commitment(BLOCK_HEIGHT).unwrap();
    let commit_time = start.elapsed();

    println!("Updated commitment in {:?}", commit_time);
    println!("UTXO count: {}", commitment.utxo_count);
    println!("Total value: {} satoshis", commitment.total_value);
    println!("Root hash: {:?}", hex_encode(&commitment.root_hash));

    // Benchmark random access
    println!("\nBenchmarking random access...");
    let num_lookups = 1000;
    let mut rng = StdRng::seed_from_u64(42); // Use a fixed seed for reproducibility
    let mut hits = 0;

    let start = Instant::now();

    for _ in 0..num_lookups {
        let idx = rng.gen_range(0..utxos.len());
        let outpoint = &utxos[idx].outpoint;

        if let Ok(Some(_)) = utxo_set.get(outpoint) {
            hits += 1;
        }
    }

    let lookup_time = start.elapsed();
    println!(
        "Performed {} random lookups in {:?} ({} lookups/sec, hit rate: {:.1}%)",
        num_lookups,
        lookup_time,
        num_lookups as f64 / lookup_time.as_secs_f64(),
        hits as f64 / num_lookups as f64 * 100.0
    );

    // Benchmark spending (removing) UTXOs
    println!("\nBenchmarking UTXO spending...");
    let num_spends = 1000;
    let mut spent_count = 0;

    let start = Instant::now();

    for i in 0..num_spends {
        let idx = i % utxos.len();
        let outpoint = &utxos[idx].outpoint;

        if let Ok(Some(_)) = utxo_set.remove(outpoint) {
            spent_count += 1;
        }
    }

    let spend_time = start.elapsed();
    println!(
        "Spent {} UTXOs in {:?} ({} spends/sec)",
        spent_count,
        spend_time,
        spent_count as f64 / spend_time.as_secs_f64()
    );

    // Get cache statistics
    let stats = utxo_set.get_stats().unwrap();
    println!("\nUTXO Cache Statistics:");
    println!("  Cache entries: {}", stats.entries);
    println!("  Cache hits: {}", stats.hits);
    println!("  Cache misses: {}", stats.misses);
    println!(
        "  Hit rate: {:.2}%",
        stats.hits as f64 / (stats.hits + stats.misses) as f64 * 100.0
    );
    println!("  Total operation time: {:?}", stats.operation_time);

    // Update commitment after spending
    println!("\nUpdating UTXO commitment after spending...");
    let start = Instant::now();
    let new_commitment = utxo_set.update_commitment(BLOCK_HEIGHT + 1).unwrap();
    let new_commit_time = start.elapsed();

    println!("Updated commitment in {:?}", new_commit_time);
    println!("New UTXO count: {}", new_commitment.utxo_count);
    println!("New total value: {} satoshis", new_commitment.total_value);
    println!("New root hash: {:?}", hex_encode(&new_commitment.root_hash));

    // Demonstrate persistent storage (optional, depending on platform support)
    if cfg!(target_os = "linux") || cfg!(target_os = "macos") {
        println!("\nDemonstrating persistent storage with memory mapping...");

        let temp_path = std::env::temp_dir().join("supernova_utxo_demo.db");
        let path_str = temp_path.to_str().unwrap();

        println!("Creating persistent UTXO set at {}", path_str);

        match UtxoSet::new_persistent(path_str, CACHE_SIZE, true) {
            Ok(persistent_set) => {
                // Add some UTXOs
                for i in 0..100 {
                    let utxo = create_test_utxo(&format!("persistent{}", i), 0, 1000 * i as u64);
                    if let Err(e) = persistent_set.add(utxo) {
                        println!("Error adding UTXO: {}", e);
                    }
                }

                // Flush to disk
                if let Err(e) = persistent_set.flush() {
                    println!("Error flushing to disk: {}", e);
                } else {
                    println!("Successfully created persistent UTXO set with memory mapping");
                }

                // Clean up
                let _ = std::fs::remove_file(temp_path);
            }
            Err(e) => {
                println!("Failed to create persistent UTXO set: {}", e);
            }
        }
    }

    println!("\nUTXO optimization demo completed!");
}

// Generate random UTXOs for testing
fn generate_random_utxos(count: usize) -> Vec<UtxoEntry> {
    let mut rng = StdRng::seed_from_u64(42); // Use a fixed seed for reproducibility
    let mut utxos = Vec::with_capacity(count);

    for i in 0..count {
        let txid = format!("tx{:08x}", i);
        let vout = rng.gen_range(0..5);
        let value = rng.gen_range(1000..1_000_000);
        let script_len = rng.gen_range(20..100);
        let mut script = Vec::with_capacity(script_len);

        for _ in 0..script_len {
            script.push(rng.gen::<u8>());
        }

        let utxo = UtxoEntry {
            outpoint: OutPoint { txid, vout },
            output: TxOutput {
                value,
                script_pubkey: script,
            },
            height: rng.gen_range(1..BLOCK_HEIGHT),
            is_coinbase: rng.gen_bool(0.01), // 1% chance of being coinbase
            is_confirmed: true,
        };

        utxos.push(utxo);
    }

    utxos
}

// Helper to create a test UTXO
fn create_test_utxo(txid: &str, vout: u32, value: u64) -> UtxoEntry {
    UtxoEntry {
        outpoint: OutPoint {
            txid: txid.to_string(),
            vout,
        },
        output: TxOutput {
            value,
            script_pubkey: vec![0, 1, 2, 3],
        },
        height: 1,
        is_coinbase: false,
        is_confirmed: true,
    }
}

// Convert bytes to hex string
fn hex_encode(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|b| format!("{:02x}", b))
        .collect::<Vec<String>>()
        .join("")
}
