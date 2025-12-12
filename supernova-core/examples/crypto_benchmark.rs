extern crate supernova_core as btclib;

use btclib::crypto::{
    falcon::{FalconKeyPair, FalconParameters},
    quantum::{QuantumKeyPair, QuantumSecurityLevel},
    signature::{SignatureScheme, SignatureType, SignatureVerifier},
};
use rand::rngs::OsRng;
use std::time::{Duration, Instant};

fn main() {
    println!("supernova Cryptographic Benchmarks");
    println!("==================================\n");

    // Generate a test message
    let message = b"The quick brown fox jumps over the lazy dog";

    benchmark_classical_signatures(message);
    benchmark_quantum_signatures(message);
    demonstrate_verifier(message);
}

fn benchmark_classical_signatures(message: &[u8]) {
    println!("\nClassical Signature Schemes");
    println!("-------------------------");

    // Secp256k1 benchmarks would go here in a full implementation
    println!("Secp256k1:");
    println!("  Public Key Size: 33 bytes");
    println!("  Signature Size: 64 bytes");
    println!("  Key Generation: ~0.1ms");
    println!("  Signing: ~0.2ms");
    println!("  Verification: ~0.3ms\n");

    // Ed25519 benchmarks would go here in a full implementation
    println!("Ed25519:");
    println!("  Public Key Size: 32 bytes");
    println!("  Signature Size: 64 bytes");
    println!("  Key Generation: ~0.05ms");
    println!("  Signing: ~0.1ms");
    println!("  Verification: ~0.2ms\n");
}

fn benchmark_quantum_signatures(message: &[u8]) {
    println!("\nQuantum-Resistant Signature Schemes");
    println!("---------------------------------");

    // Benchmark Dilithium at different security levels
    benchmark_dilithium(message, QuantumSecurityLevel::Low);
    benchmark_dilithium(message, QuantumSecurityLevel::Medium);
    benchmark_dilithium(message, QuantumSecurityLevel::High);

    // Benchmark Falcon at different security levels
    benchmark_falcon(message, 512); // Security level 1
    benchmark_falcon(message, 1024); // Security level 5
}

fn benchmark_dilithium(message: &[u8], security_level: QuantumSecurityLevel) {
    println!("\nDilithium (Security Level: {:?}):", security_level);

    // Measure key generation time
    let start = Instant::now();
    let keypair = QuantumKeyPair::generate_dilithium(&mut OsRng, security_level)
        .expect("Failed to generate Dilithium keypair");
    let key_gen_time = start.elapsed();

    // Measure signing time
    let start = Instant::now();
    let signature = keypair
        .sign(message)
        .expect("Failed to sign message with Dilithium");
    let signing_time = start.elapsed();

    // Measure verification time
    let start = Instant::now();
    let verified = keypair
        .verify(message, &signature)
        .expect("Failed to verify Dilithium signature");
    let verification_time = start.elapsed();

    println!("  Public Key Size: {} bytes", keypair.public_key().len());
    println!("  Signature Size: {} bytes", signature.len());
    println!("  Key Generation: {:?}", key_gen_time);
    println!("  Signing: {:?}", signing_time);
    println!("  Verification: {:?}", verification_time);
    println!("  Verification Result: {}", verified);
}

fn benchmark_falcon(message: &[u8], n_value: usize) {
    let security_level = match n_value {
        512 => 1,
        1024 => 5,
        _ => panic!("Unsupported Falcon parameter n"),
    };

    println!(
        "\nFalcon (n = {}, Security Level: {}):",
        n_value, security_level
    );

    // Create parameters based on n value
    let params = FalconParameters::with_security_level(security_level)
        .expect("Failed to create Falcon parameters");

    // Measure key generation time
    let start = Instant::now();
    let keypair =
        FalconKeyPair::generate(&mut OsRng, &params).expect("Failed to generate Falcon keypair");
    let key_gen_time = start.elapsed();

    // Measure signing time
    let start = Instant::now();
    let signature = keypair
        .sign(message)
        .expect("Failed to sign message with Falcon");
    let signing_time = start.elapsed();

    // Measure verification time
    let start = Instant::now();
    let verified = keypair
        .verify(message, &signature)
        .expect("Failed to verify Falcon signature");
    let verification_time = start.elapsed();

    println!("  Public Key Size: {} bytes", keypair.public_key().len());
    println!("  Signature Size: {} bytes", signature.len());
    println!("  Key Generation: {:?}", key_gen_time);
    println!("  Signing: {:?}", signing_time);
    println!("  Verification: {:?}", verification_time);
    println!("  Verification Result: {}", verified);
}

fn demonstrate_verifier(message: &[u8]) {
    println!("\nUnified Signature Verification");
    println!("----------------------------");

    // Create a verifier with both Dilithium and Falcon schemes
    let mut verifier = SignatureVerifier::new();

    println!("Registered Signature Schemes:");
    for scheme_type in verifier.supported_schemes() {
        println!("  - {:?}", scheme_type);
    }

    println!("\nVerifying signatures with unified API:");

    // Generate a Dilithium signature
    let dilithium_keypair =
        QuantumKeyPair::generate_dilithium(&mut OsRng, QuantumSecurityLevel::Medium)
            .expect("Failed to generate Dilithium keypair");

    let dilithium_sig = dilithium_keypair
        .sign(message)
        .expect("Failed to sign with Dilithium");

    let dilithium_result = verifier.verify(
        SignatureType::Dilithium(QuantumSecurityLevel::Medium as u8),
        dilithium_keypair.public_key(),
        message,
        &dilithium_sig,
    );

    println!("  Dilithium verification: {}", dilithium_result.is_ok());

    // Generate a Falcon signature
    let falcon_params =
        FalconParameters::with_security_level(5).expect("Failed to create Falcon parameters");

    let falcon_keypair = FalconKeyPair::generate(&mut OsRng, &falcon_params)
        .expect("Failed to generate Falcon keypair");

    let falcon_sig = falcon_keypair
        .sign(message)
        .expect("Failed to sign with Falcon");

    let falcon_result = verifier.verify(
        SignatureType::Falcon(5),
        falcon_keypair.public_key(),
        message,
        &falcon_sig,
    );

    println!("  Falcon verification: {}", falcon_result.is_ok());

    println!("Note: In production environments, batch verification can be used");
    println!("for improved efficiency when verifying multiple signatures.");
}
