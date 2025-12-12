extern crate supernova_core as btclib;

use btclib::crypto::falcon_real::{FalconKeyPair, FalconParameters};
use btclib::crypto::quantum::{QuantumKeyPair, QuantumParameters, QuantumScheme};
use btclib::crypto::signature::{
    SignatureError, SignatureScheme, SignatureType, SignatureVerifier,
};
use btclib::validation::SecurityLevel;
use rand::rngs::OsRng;
use rayon::prelude::*;
use std::time::{Duration, Instant};

// Utility function to generate test data for batch verification
fn generate_test_data(
    scheme: QuantumScheme,
    security_level: u8,
    count: usize,
) -> (Vec<Vec<u8>>, Vec<Vec<u8>>, Vec<Vec<u8>>) {
    let mut public_keys = Vec::with_capacity(count);
    let mut messages = Vec::with_capacity(count);
    let mut signatures = Vec::with_capacity(count);

    println!("Generating {} test signatures for {:?}", count, scheme);

    for i in 0..count {
        let params = QuantumParameters {
            scheme,
            security_level,
        };

        let keypair =
            QuantumKeyPair::generate(scheme, Some(params)).expect("Failed to generate key pair");

        let message = format!("Test message {}", i).into_bytes();
        let signature = keypair.sign(&message).expect("Failed to sign message");

        public_keys.push(keypair.public_key);
        messages.push(message);
        signatures.push(signature);
    }

    (public_keys, messages, signatures)
}

/// Stress test batch verification with a large number of signatures
#[test]
#[ignore] // This test is resource-intensive, run it explicitly with `cargo test -- --ignored`
fn stress_test_batch_verification() {
    // Skip in CI environments
    if std::env::var("CI").is_ok() {
        println!("Skipping stress test in CI environment");
        return;
    }

    println!("\n====== BATCH VERIFICATION STRESS TEST ======");

    let security_level = SecurityLevel::Medium as u8;
    let batch_sizes = [10, 50, 100, 250];

    for batch_size in batch_sizes {
        // Generate test data
        let (public_keys, messages, signatures) =
            generate_test_data(QuantumScheme::Dilithium, security_level, batch_size);

        // Create references for batch verification
        let public_key_refs: Vec<&[u8]> = public_keys.iter().map(|k| k.as_slice()).collect();
        let message_refs: Vec<&[u8]> = messages.iter().map(|m| m.as_slice()).collect();
        let signature_refs: Vec<&[u8]> = signatures.iter().map(|s| s.as_slice()).collect();

        // Create verifier
        let verifier = btclib::crypto::signature::DilithiumScheme::new(security_level);

        // Measure time for batch verification
        let batch_start = Instant::now();
        let batch_result = verifier.batch_verify(&public_key_refs, &message_refs, &signature_refs);
        let batch_duration = batch_start.elapsed();

        // Measure time for individual verification
        let individual_start = Instant::now();
        let individual_results: Vec<Result<bool, SignatureError>> = public_keys
            .iter()
            .zip(messages.iter())
            .zip(signatures.iter())
            .map(|((pk, msg), sig)| verifier.verify(pk, msg, sig))
            .collect();
        let individual_duration = individual_start.elapsed();

        // Calculate verification rates
        let batch_rate = batch_size as f64 / batch_duration.as_secs_f64();
        let individual_rate = batch_size as f64 / individual_duration.as_secs_f64();

        println!("Batch size: {}", batch_size);
        println!(
            "  Batch verification time: {:?} ({:.2} verifications/sec)",
            batch_duration, batch_rate
        );
        println!(
            "  Individual verification time: {:?} ({:.2} verifications/sec)",
            individual_duration, individual_rate
        );
        println!(
            "  Speedup: {:.2}x",
            individual_duration.as_secs_f64() / batch_duration.as_secs_f64()
        );

        // Verify that batch verification gives the same result as individual verification
        let all_individual_valid = individual_results
            .iter()
            .all(|r| r.as_ref().map_or(false, |&v| v));
        assert!(
            batch_result.is_ok(),
            "Batch verification failed with error: {:?}",
            batch_result
        );

        if batch_result.is_ok() {
            assert_eq!(
                batch_result.unwrap(),
                all_individual_valid,
                "Batch verification result should match individual verification results"
            );
        }
    }
}

/// Test batch verification with mixed valid and invalid signatures
#[test]
fn test_batch_verification_with_invalid_signatures() {
    println!("\n====== BATCH VERIFICATION WITH INVALID SIGNATURES ======");

    let security_level = SecurityLevel::Medium as u8;
    let batch_size = 20;

    // Generate test data
    let (mut public_keys, mut messages, mut signatures) =
        generate_test_data(QuantumScheme::Dilithium, security_level, batch_size);

    // Corrupt some signatures (every 5th one)
    for i in (0..batch_size).step_by(5) {
        if i < signatures.len() && !signatures[i].is_empty() {
            // Tamper with the signature
            signatures[i][0] ^= 0xff;
            println!("Corrupted signature at index {}", i);
        }
    }

    // Create references for batch verification
    let public_key_refs: Vec<&[u8]> = public_keys.iter().map(|k| k.as_slice()).collect();
    let message_refs: Vec<&[u8]> = messages.iter().map(|m| m.as_slice()).collect();
    let signature_refs: Vec<&[u8]> = signatures.iter().map(|s| s.as_slice()).collect();

    // Create verifier
    let verifier = btclib::crypto::signature::DilithiumScheme::new(security_level);

    // Batch verification should fail due to the corrupted signatures
    let batch_result = verifier.batch_verify(&public_key_refs, &message_refs, &signature_refs);

    assert!(
        batch_result.is_ok(),
        "Batch verification should not error: {:?}",
        batch_result
    );
    assert!(
        !batch_result.unwrap(),
        "Batch verification should fail with corrupted signatures"
    );

    // Check individual signatures to verify which ones fail
    let individual_results: Vec<Result<bool, SignatureError>> = public_keys
        .iter()
        .zip(messages.iter())
        .zip(signatures.iter())
        .map(|((pk, msg), sig)| verifier.verify(pk, msg, sig))
        .collect();

    // Count how many signatures are valid
    let valid_count = individual_results
        .iter()
        .filter(|r| r.as_ref().map_or(false, |&v| v))
        .count();

    println!("Valid signatures: {}/{}", valid_count, batch_size);
    assert!(
        valid_count < batch_size,
        "Some signatures should be invalid"
    );
    assert_eq!(
        valid_count,
        batch_size - (batch_size / 5),
        "Every 5th signature should be invalid"
    );
}

/// Test parallel verification against sequential verification
#[test]
fn test_parallel_vs_sequential_verification() {
    println!("\n====== PARALLEL VS SEQUENTIAL VERIFICATION ======");

    let security_level = SecurityLevel::Medium as u8;
    let batch_size = 50;

    // Generate test data
    let (public_keys, messages, signatures) =
        generate_test_data(QuantumScheme::Dilithium, security_level, batch_size);

    // Create references for verification
    let public_key_refs: Vec<&[u8]> = public_keys.iter().map(|k| k.as_slice()).collect();
    let message_refs: Vec<&[u8]> = messages.iter().map(|m| m.as_slice()).collect();
    let signature_refs: Vec<&[u8]> = signatures.iter().map(|s| s.as_slice()).collect();

    // Create verifier
    let verifier = btclib::crypto::signature::DilithiumScheme::new(security_level);

    // Create tuples for verification
    let verification_tuples: Vec<_> = public_keys.iter().zip(&messages).zip(&signatures).collect();

    // Test parallel verification (using rayon)
    let parallel_start = Instant::now();
    let parallel_results: Vec<Result<bool, SignatureError>> = verification_tuples
        .par_iter() // Using par_iter on the tuples
        .map(|((pk, msg), sig)| verifier.verify(pk, msg, sig))
        .collect();
    let parallel_duration = parallel_start.elapsed();

    // Test sequential verification
    let sequential_start = Instant::now();
    let sequential_results: Vec<Result<bool, SignatureError>> = verification_tuples
        .iter()
        .map(|((pk, msg), sig)| verifier.verify(pk, msg, sig))
        .collect();
    let sequential_duration = sequential_start.elapsed();

    // Calculate speedup
    let speedup = sequential_duration.as_secs_f64() / parallel_duration.as_secs_f64();

    println!("Sequential verification time: {:?}", sequential_duration);
    println!("Parallel verification time: {:?}", parallel_duration);
    println!("Parallel speedup: {:.2}x", speedup);

    // Results should be identical
    for i in 0..batch_size {
        assert_eq!(
            parallel_results[i].is_ok(),
            sequential_results[i].is_ok(),
            "Parallel and sequential verification should have the same error status"
        );

        if parallel_results[i].is_ok() && sequential_results[i].is_ok() {
            assert_eq!(
                parallel_results[i].as_ref().unwrap(),
                sequential_results[i].as_ref().unwrap(),
                "Parallel and sequential verification should give the same result"
            );
        }
    }

    // On multi-core systems, parallel should be faster
    let num_cpus = num_cpus::get();
    println!("Number of CPU cores: {}", num_cpus);

    if num_cpus > 1 {
        assert!(
            speedup > 1.0,
            "Parallel verification should be faster on multi-core systems"
        );
    }
}
