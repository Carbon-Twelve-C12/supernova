use btclib::crypto::quantum::{ClassicalScheme, QuantumKeyPair, QuantumParameters, QuantumScheme};
use btclib::validation::SecurityLevel;
use rand::rngs::OsRng;
use std::time::Instant;

fn main() {
    println!("Supernova Quantum-Resistant Signatures Demo");
    println!("===========================================\n");

    let mut rng = OsRng;

    // Test all signature schemes
    println!("1. CRYSTALS-Dilithium Signatures");
    println!("--------------------------------");
    benchmark_scheme(
        QuantumScheme::Dilithium,
        SecurityLevel::Medium as u8,
        &mut rng,
    );

    println!("\n2. SPHINCS+ Signatures");
    println!("---------------------");
    benchmark_scheme(QuantumScheme::Sphincs, SecurityLevel::Low as u8, &mut rng);

    println!("\n3. Hybrid Signatures (Secp256k1 + Dilithium)");
    println!("------------------------------------------");
    benchmark_scheme(
        QuantumScheme::Hybrid(ClassicalScheme::Secp256k1),
        SecurityLevel::Medium as u8,
        &mut rng,
    );

    println!("\n4. Hybrid Signatures (Ed25519 + Dilithium)");
    println!("----------------------------------------");
    benchmark_scheme(
        QuantumScheme::Hybrid(ClassicalScheme::Ed25519),
        SecurityLevel::Medium as u8,
        &mut rng,
    );
}

fn benchmark_scheme(scheme: QuantumScheme, security_level: u8, rng: &mut OsRng) {
    // Generate keypair
    let start = Instant::now();
    let parameters = QuantumParameters {
        scheme,
        security_level,
    };

    let keypair = match QuantumKeyPair::generate(rng, parameters) {
        Ok(kp) => kp,
        Err(e) => {
            println!("Failed to generate keypair: {}", e);
            return;
        }
    };
    let keygen_time = start.elapsed();

    println!("Key generation time: {:?}", keygen_time);
    println!("Public key size: {} bytes", keypair.public_key.len());

    // Sign a message
    let message = b"This is a test message for quantum-resistant signatures in Supernova";

    let start = Instant::now();
    let signature = match keypair.sign(message) {
        Ok(sig) => sig,
        Err(e) => {
            println!("Failed to sign message: {}", e);
            return;
        }
    };
    let sign_time = start.elapsed();

    println!("Signing time: {:?}", sign_time);
    println!("Signature size: {} bytes", signature.len());

    // Verify the signature
    let start = Instant::now();
    let valid = match keypair.verify(message, &signature) {
        Ok(v) => v,
        Err(e) => {
            println!("Failed to verify signature: {}", e);
            return;
        }
    };
    let verify_time = start.elapsed();

    println!("Verification time: {:?}", verify_time);
    println!("Signature valid: {}", valid);

    // Try an invalid message
    let modified_message = b"This is a MODIFIED message that should fail verification";

    let start = Instant::now();
    let invalid = match keypair.verify(modified_message, &signature) {
        Ok(v) => v,
        Err(e) => {
            println!("Failed to verify modified message: {}", e);
            return;
        }
    };
    let invalid_verify_time = start.elapsed();

    println!(
        "Invalid message verification time: {:?}",
        invalid_verify_time
    );
    println!(
        "Modified message verification result: {} (should be false)",
        invalid
    );

    // Security properties
    match scheme {
        QuantumScheme::Dilithium => {
            println!("\nDilithium is a lattice-based signature scheme selected by NIST");
            println!("Security based on Module Learning With Errors (M-LWE) problem");
            match security_level {
                1 | 2 => println!("Security Level: NIST Level 1 (128-bit security)"),
                3 | 4 => println!("Security Level: NIST Level 3 (192-bit security)"),
                5 => println!("Security Level: NIST Level 5 (256-bit security)"),
                _ => println!("Security Level: Unknown"),
            }
        }
        QuantumScheme::Sphincs => {
            println!("\nSPHINCS+ is a stateless hash-based signature scheme");
            println!("Security based solely on the security of the underlying hash function");
            println!("Most conservative choice with minimal security assumptions");
            println!("Trade-off: Larger signatures compared to lattice-based schemes");
        }
        QuantumScheme::Hybrid(classical) => {
            println!("\nHybrid signature combines classical and quantum-resistant signatures");
            println!("Classical scheme: {:?}", classical);
            println!("Quantum scheme: Dilithium");
            println!("Provides protection against both classical and quantum attacks");
            println!("Even if one scheme is broken, the other provides a security backstop");
        }
        _ => {}
    }
}
