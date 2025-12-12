//! Simple Quantum Signature Performance Test
//!
//! This example benchmarks the performance of Supernova's quantum signatures
//! without relying on complex module structures.

extern crate supernova_core as btclib;

use btclib::crypto::quantum::{
    sign_quantum, verify_quantum_signature, QuantumKeyPair, QuantumParameters, QuantumScheme,
};
use std::time::Instant;

fn main() {
    println!("╔═══════════════════════════════════════════════════════════════╗");
    println!("║                                                               ║");
    println!("║       SUPERNOVA QUANTUM SIGNATURE PERFORMANCE TEST            ║");
    println!("║                                                               ║");
    println!("╚═══════════════════════════════════════════════════════════════╝\n");

    // Test different batch sizes similar to the research paper
    let batch_sizes = vec![1, 10, 100, 1000, 5000, 10000];

    println!("Testing Dilithium signatures (NIST Level 3)...\n");

    for batch_size in batch_sizes {
        benchmark_batch(QuantumScheme::Dilithium, 3, batch_size);
    }

    println!("\n=== Comparison with LaBRADOR Research ===");
    println!("LaBRADOR (10k Falcon-512 signatures):");
    println!("  - Proof size: 74.07 KB");
    println!("  - Proving time: 5.95 seconds");
    println!("  - Verification time: 2.65 seconds");
    println!("\nNote: Supernova uses different algorithms (NIST-standardized)");
    println!("with different performance characteristics but similar security guarantees.");
}

fn benchmark_batch(scheme: QuantumScheme, security_level: u8, batch_size: usize) {
    let params = QuantumParameters {
        scheme,
        security_level,
    };

    // Generate keys and signatures
    let mut total_keygen_time = std::time::Duration::ZERO;
    let mut total_sign_time = std::time::Duration::ZERO;
    let mut total_verify_time = std::time::Duration::ZERO;
    let mut total_size = 0;

    let mut keypairs = Vec::new();
    let mut signatures = Vec::new();

    // Key generation phase
    let batch_start = Instant::now();
    for i in 0..batch_size {
        let start = Instant::now();
        let keypair = match QuantumKeyPair::generate(params) {
            Ok(kp) => kp,
            Err(e) => {
                println!("Error generating keypair: {:?}", e);
                return;
            }
        };
        total_keygen_time += start.elapsed();

        // Sign a unique message
        let message = format!("Test message {}", i);
        let start = Instant::now();
        match sign_quantum(&keypair, message.as_bytes()) {
            Ok(sig) => {
                total_size += sig.len();
                signatures.push((keypair.public_key.clone(), message, sig));
                keypairs.push(keypair);
            }
            Err(e) => {
                println!("Error signing: {:?}", e);
                return;
            }
        }
        total_sign_time += start.elapsed();
    }

    // Verification phase
    let verify_start = Instant::now();
    for (pubkey, message, signature) in &signatures {
        match verify_quantum_signature(pubkey, message.as_bytes(), signature, params) {
            Ok(valid) => {
                if !valid {
                    println!("WARNING: Invalid signature detected!");
                }
            }
            Err(e) => {
                println!("Error verifying: {:?}", e);
                return;
            }
        }
    }
    total_verify_time = verify_start.elapsed();

    let total_time = batch_start.elapsed();

    // Calculate aggregated size estimate (based on research)
    let aggregated_size = estimate_aggregated_size(scheme, batch_size);

    // Print results
    println!("Batch size: {} signatures", batch_size);
    println!("  Total time: {:.2}s", total_time.as_secs_f64());
    println!(
        "  Key generation: {:.2}s ({:.2}ms per key)",
        total_keygen_time.as_secs_f64(),
        total_keygen_time.as_secs_f64() * 1000.0 / batch_size as f64
    );
    println!(
        "  Signing: {:.2}s ({:.2}ms per signature)",
        total_sign_time.as_secs_f64(),
        total_sign_time.as_secs_f64() * 1000.0 / batch_size as f64
    );
    println!(
        "  Verification: {:.2}s ({:.2}ms per signature)",
        total_verify_time.as_secs_f64(),
        total_verify_time.as_secs_f64() * 1000.0 / batch_size as f64
    );
    println!(
        "  Average signature size: {} bytes",
        total_size / batch_size
    );
    println!(
        "  Estimated aggregated proof size: {:.2} KB",
        aggregated_size as f64 / 1024.0
    );
    println!(
        "  Throughput: {:.0} signatures/second\n",
        batch_size as f64 / total_time.as_secs_f64()
    );
}

fn estimate_aggregated_size(scheme: QuantumScheme, batch_size: usize) -> usize {
    // Based on LaBRADOR paper estimates
    match scheme {
        QuantumScheme::Dilithium => {
            // Dilithium has larger signatures than Falcon
            // Estimate ~100KB for 10k signatures
            let base_size = 90_000;
            let per_sig = 10;
            base_size + (batch_size * per_sig)
        }
        QuantumScheme::Falcon => {
            // Based on paper: ~74KB for 10k signatures
            let base_size = 70_000;
            let per_sig = 5;
            base_size + (batch_size * per_sig)
        }
        QuantumScheme::SphincsPlus => {
            // Hash-based, much larger
            let base_size = 150_000;
            let per_sig = 20;
            base_size + (batch_size * per_sig)
        }
        QuantumScheme::Hybrid(_) => {
            // Variable size
            let base_size = 100_000;
            let per_sig = 15;
            base_size + (batch_size * per_sig)
        }
    }
}
