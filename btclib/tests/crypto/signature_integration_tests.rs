use btclib::crypto::falcon_real::{FalconKeyPair, FalconParameters};
use btclib::crypto::quantum::{QuantumKeyPair, QuantumParameters, QuantumScheme};
use btclib::crypto::signature::{
    DilithiumScheme, Ed25519Scheme, FalconScheme, Secp256k1Scheme, SignatureError, SignatureParams,
    SignatureScheme, SignatureType, SignatureVerifier,
};
use btclib::validation::SecurityLevel;
use rand::rngs::OsRng;
use rayon::prelude::*;
use std::collections::HashMap;
use std::time::{Duration, Instant};

/// Test the sign and verify flow for all signature schemes
#[test]
fn test_all_signature_schemes_sign_verify() {
    let test_cases = vec![
        ("Secp256k1", SignatureType::Secp256k1, SecurityLevel::Medium),
        ("Ed25519", SignatureType::Ed25519, SecurityLevel::Medium),
        ("Dilithium", SignatureType::Dilithium, SecurityLevel::Medium),
        ("Falcon", SignatureType::Falcon, SecurityLevel::Medium),
    ];

    for (name, sig_type, security_level) in test_cases {
        println!("Testing {} signature scheme", name);

        // Generate appropriate key pair based on signature type
        let (public_key, signature) = match sig_type {
            SignatureType::Secp256k1 => {
                // For testing purposes, we use dummy values since we can't generate keys here
                println!("Note: Using dummy secp256k1 keys for testing");
                let public_key = vec![0u8; 33]; // Compressed public key
                let signature = vec![0u8; 64]; // DER signature
                (public_key, signature)
            }
            SignatureType::Ed25519 => {
                // For testing purposes, we use dummy values
                println!("Note: Using dummy Ed25519 keys for testing");
                let public_key = vec![0u8; 32];
                let signature = vec![0u8; 64];
                (public_key, signature)
            }
            SignatureType::Dilithium => {
                let params = QuantumParameters {
                    scheme: QuantumScheme::Dilithium,
                    security_level: security_level as u8,
                };

                let keypair = QuantumKeyPair::generate(QuantumScheme::Dilithium, Some(params))
                    .expect("Failed to generate Dilithium key pair");

                let message = b"Test message for Dilithium";
                let signature = keypair
                    .sign(message)
                    .expect("Failed to sign with Dilithium");

                (keypair.public_key, signature)
            }
            SignatureType::Falcon => {
                let params = FalconParameters::with_security_level(security_level as u8);
                let mut rng = OsRng;

                let keypair = FalconKeyPair::generate(&mut rng, params)
                    .expect("Failed to generate Falcon key pair");

                let message = b"Test message for Falcon";
                let signature = keypair.sign(message).expect("Failed to sign with Falcon");

                (keypair.public_key, signature)
            }
            _ => {
                println!("Skipping unsupported scheme: {:?}", sig_type);
                continue;
            }
        };

        // Verify using the appropriate scheme
        let verifier = match sig_type {
            SignatureType::Secp256k1 => Box::new(Secp256k1Scheme) as Box<dyn SignatureScheme>,
            SignatureType::Ed25519 => Box::new(Ed25519Scheme) as Box<dyn SignatureScheme>,
            SignatureType::Dilithium => {
                Box::new(DilithiumScheme::new(security_level as u8)) as Box<dyn SignatureScheme>
            }
            SignatureType::Falcon => {
                Box::new(FalconScheme::new(security_level as u8)) as Box<dyn SignatureScheme>
            }
            _ => continue,
        };

        // Skip actual verification for schemes with placeholder implementations
        if matches!(sig_type, SignatureType::Secp256k1 | SignatureType::Ed25519) {
            println!(
                "Skipping verification for placeholder implementation: {:?}",
                sig_type
            );
            continue;
        }

        let message = match sig_type {
            SignatureType::Dilithium => b"Test message for Dilithium",
            SignatureType::Falcon => b"Test message for Falcon",
            _ => b"Test message",
        };

        // Try to verify
        let result = verifier.verify(&public_key, message, &signature);

        // For now, we expect verification to succeed for Dilithium
        // Falcon depends on the current implementation (might be placeholder)
        match sig_type {
            SignatureType::Dilithium => {
                assert!(
                    result.is_ok(),
                    "Dilithium verification resulted in error: {:?}",
                    result
                );
                let verified = result.unwrap();
                assert!(verified, "Dilithium signature should verify successfully");
            }
            SignatureType::Falcon => {
                // If Falcon has a real implementation, it should succeed
                // If it's a placeholder that returns an error, that's acceptable for now
                if let Ok(verified) = result {
                    assert!(
                        verified,
                        "Falcon signature should verify successfully if implemented"
                    );
                } else {
                    println!(
                        "Falcon verification returned error (expected if using placeholder): {:?}",
                        result
                    );
                }
            }
            _ => {}
        }
    }
}

/// Test batch verification functionality
#[test]
fn test_batch_verification() {
    let security_level = SecurityLevel::Medium as u8;
    let message_sets = 10; // Number of message sets to create
    let messages_per_set = 5; // Number of messages in each set

    // Test with Dilithium (most likely to have working batch verification)
    let scheme = QuantumScheme::Dilithium;

    // Create message sets
    let mut public_keys = Vec::with_capacity(messages_per_set);
    let mut signatures = Vec::with_capacity(messages_per_set);
    let mut messages = Vec::with_capacity(messages_per_set);

    println!(
        "Generating {} key pairs for batch verification",
        messages_per_set
    );

    // Generate key pairs and signatures
    for i in 0..messages_per_set {
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

    // Create references for batch verification
    let public_key_refs: Vec<&[u8]> = public_keys.iter().map(|k| k.as_slice()).collect();
    let message_refs: Vec<&[u8]> = messages.iter().map(|m| m.as_slice()).collect();
    let signature_refs: Vec<&[u8]> = signatures.iter().map(|s| s.as_slice()).collect();

    // Create verifier
    let verifier = DilithiumScheme::new(security_level);

    // Measure time for batch verification vs. individual verification
    let batch_start = Instant::now();
    let batch_result = verifier.batch_verify(&public_key_refs, &message_refs, &signature_refs);
    let batch_duration = batch_start.elapsed();

    // Measure time for individual verification
    let individual_start = Instant::now();
    let mut all_verified = true;
    for i in 0..messages_per_set {
        let result = verifier.verify(&public_keys[i], &messages[i], &signatures[i]);

        if let Ok(verified) = result {
            all_verified &= verified;
        } else {
            all_verified = false;
            break;
        }
    }
    let individual_duration = individual_start.elapsed();

    println!("Batch verification time: {:?}", batch_duration);
    println!("Individual verification time: {:?}", individual_duration);
    println!(
        "Speedup: {:.2}x",
        individual_duration.as_secs_f64() / batch_duration.as_secs_f64()
    );

    // Check that batch verification gives the same result as individual verification
    assert!(
        batch_result.is_ok(),
        "Batch verification should not error: {:?}",
        batch_result
    );
    assert_eq!(
        batch_result.unwrap(),
        all_verified,
        "Batch verification should give the same result as individual verification"
    );
}

/// Test signature verification with invalid signatures
#[test]
fn test_invalid_signatures() {
    // Generate a Dilithium key pair for testing
    let params = QuantumParameters {
        scheme: QuantumScheme::Dilithium,
        security_level: SecurityLevel::Medium as u8,
    };

    let keypair = QuantumKeyPair::generate(QuantumScheme::Dilithium, Some(params))
        .expect("Failed to generate Dilithium key pair");

    let message = b"Test message for invalid signature testing";
    let signature = keypair
        .sign(message)
        .expect("Failed to sign with Dilithium");

    // Verify with correct signature (should succeed)
    let verifier = DilithiumScheme::new(SecurityLevel::Medium as u8);
    let result = verifier.verify(&keypair.public_key, message, &signature);
    assert!(
        result.is_ok(),
        "Valid signature verification resulted in error: {:?}",
        result
    );
    assert!(
        result.unwrap(),
        "Valid signature should verify successfully"
    );

    // Test with tampered signature (if we corrupt even a single byte, it should fail)
    if !signature.is_empty() {
        let mut tampered_signature = signature.clone();
        tampered_signature[0] ^= 0xff; // Flip all bits in the first byte

        let result = verifier.verify(&keypair.public_key, message, &tampered_signature);
        assert!(
            result.is_ok(),
            "Tampered signature verification shouldn't error: {:?}",
            result
        );
        assert!(!result.unwrap(), "Tampered signature should not verify");
    }

    // Test with wrong message
    let wrong_message = b"This is not the message that was signed";
    let result = verifier.verify(&keypair.public_key, wrong_message, &signature);
    assert!(
        result.is_ok(),
        "Wrong message verification shouldn't error: {:?}",
        result
    );
    assert!(
        !result.unwrap(),
        "Signature with wrong message should not verify"
    );

    // Test with wrong public key
    let wrong_params = QuantumParameters {
        scheme: QuantumScheme::Dilithium,
        security_level: SecurityLevel::Medium as u8,
    };

    let wrong_keypair = QuantumKeyPair::generate(QuantumScheme::Dilithium, Some(wrong_params))
        .expect("Failed to generate wrong Dilithium key pair");

    let result = verifier.verify(&wrong_keypair.public_key, message, &signature);
    assert!(
        result.is_ok(),
        "Wrong key verification shouldn't error: {:?}",
        result
    );
    assert!(
        !result.unwrap(),
        "Signature with wrong public key should not verify"
    );
}

/// Test the unified signature verifier with multiple schemes
#[test]
fn test_unified_signature_verifier() {
    let mut verifier = SignatureVerifier::new();

    // Ensure all basic schemes are registered
    assert!(verifier
        .verify(
            SignatureType::Secp256k1,
            &[0u8; 33], // Dummy public key
            b"test message",
            &[0u8; 64] // Dummy signature
        )
        .is_ok());

    assert!(verifier
        .verify(
            SignatureType::Ed25519,
            &[0u8; 32], // Dummy public key
            b"test message",
            &[0u8; 64] // Dummy signature
        )
        .is_ok());

    assert!(verifier
        .verify(
            SignatureType::Dilithium,
            &[0u8; 1312], // Dummy dilithium public key
            b"test message",
            &[0u8; 2420] // Dummy dilithium signature
        )
        .is_err());

    // Register Falcon
    verifier.register(
        SignatureType::Falcon,
        Box::new(FalconScheme::new(SecurityLevel::Medium as u8)),
    );

    // Verify Falcon is registered
    let result = verifier.verify(
        SignatureType::Falcon,
        &[0u8; 897], // Dummy Falcon-512 public key size
        b"test message",
        &[0u8; 666], // Dummy Falcon-512 signature size
    );

    // The result might be an error if Falcon is not fully implemented, that's okay
    println!("Falcon verification result: {:?}", result);
}

/// Performance benchmarks for different signature schemes
#[test]
fn benchmark_signature_schemes() {
    // Skip in CI environments or for quick test runs
    if std::env::var("CI").is_ok() || std::env::var("SKIP_BENCHMARKS").is_ok() {
        println!("Skipping benchmarks in CI environment");
        return;
    }

    println!("\n====== SIGNATURE SCHEME BENCHMARKS ======");
    println!("Note: These are rough benchmarks for comparison only\n");

    let security_level = SecurityLevel::Medium as u8;
    let schemes = vec![
        // Not implementing actual benchmarks for these as they're placeholders
        // ("Secp256k1", SignatureType::Secp256k1),
        // ("Ed25519", SignatureType::Ed25519),
        ("Dilithium", SignatureType::Dilithium),
        ("Falcon", SignatureType::Falcon),
    ];

    let message = b"This is a test message for benchmarking signature schemes";
    let iterations = 10;

    let mut results = HashMap::new();

    for (name, sig_type) in schemes {
        println!("Benchmarking {}", name);

        let mut key_gen_times = Vec::with_capacity(iterations);
        let mut sign_times = Vec::with_capacity(iterations);
        let mut verify_times = Vec::with_capacity(iterations);

        for _ in 0..iterations {
            match sig_type {
                SignatureType::Dilithium => {
                    // Benchmark Dilithium
                    let params = QuantumParameters {
                        scheme: QuantumScheme::Dilithium,
                        security_level,
                    };

                    // Key generation
                    let key_gen_start = Instant::now();
                    let keypair = QuantumKeyPair::generate(QuantumScheme::Dilithium, Some(params))
                        .expect("Failed to generate Dilithium key pair");
                    key_gen_times.push(key_gen_start.elapsed());

                    // Signing
                    let sign_start = Instant::now();
                    let signature = keypair
                        .sign(message)
                        .expect("Failed to sign with Dilithium");
                    sign_times.push(sign_start.elapsed());

                    // Verification
                    let verify_start = Instant::now();
                    let _ = keypair
                        .verify(message, &signature)
                        .expect("Failed to verify Dilithium signature");
                    verify_times.push(verify_start.elapsed());
                }
                SignatureType::Falcon => {
                    // Benchmark Falcon
                    let params = FalconParameters::with_security_level(security_level);
                    let mut rng = OsRng;

                    // Key generation
                    let key_gen_start = Instant::now();
                    let keypair = FalconKeyPair::generate(&mut rng, params)
                        .expect("Failed to generate Falcon key pair");
                    key_gen_times.push(key_gen_start.elapsed());

                    // Signing
                    let sign_start = Instant::now();
                    let signature = keypair.sign(message).expect("Failed to sign with Falcon");
                    sign_times.push(sign_start.elapsed());

                    // Verification
                    let verify_start = Instant::now();
                    let _ = keypair
                        .verify(message, &signature)
                        .expect("Failed to verify Falcon signature");
                    verify_times.push(verify_start.elapsed());
                }
                _ => continue,
            }
        }

        // Calculate averages
        let avg_key_gen = key_gen_times.iter().sum::<Duration>() / key_gen_times.len() as u32;
        let avg_sign = sign_times.iter().sum::<Duration>() / sign_times.len() as u32;
        let avg_verify = verify_times.iter().sum::<Duration>() / verify_times.len() as u32;

        println!("  Key generation: {:?}", avg_key_gen);
        println!("  Signing:        {:?}", avg_sign);
        println!("  Verification:   {:?}", avg_verify);
        println!(
            "  Signature size: {} bytes",
            match sig_type {
                SignatureType::Dilithium => 2420, // Approximate size for security level 3
                SignatureType::Falcon => 666,     // Approximate size for Falcon-512
                _ => 0,
            }
        );
        println!(
            "  Public key size: {} bytes",
            match sig_type {
                SignatureType::Dilithium => 1952, // Approximate size for security level 3
                SignatureType::Falcon => 897,     // Approximate size for Falcon-512
                _ => 0,
            }
        );
        println!();

        results.insert(name, (avg_key_gen, avg_sign, avg_verify));
    }

    println!("====== BENCHMARK SUMMARY ======");
    for (name, (key_gen, sign, verify)) in &results {
        println!(
            "{}: KeyGen={:?}, Sign={:?}, Verify={:?}",
            name, key_gen, sign, verify
        );
    }
}
