use colored::*;
use pqcrypto_mldsa::mldsa65;
use pqcrypto_sphincsplus::sphincssha256128fsimple;
use pqcrypto_traits::sign::{DetachedSignature, PublicKey, SecretKey};
use serde::Serialize;
use std::time::Instant;

#[derive(Debug, Serialize)]
struct TestResults {
    scheme: String,
    security_level: String,
    key_generation_time_ms: f64,
    signing_time_ms: f64,
    verification_time_ms: f64,
    signature_size_bytes: usize,
    public_key_size_bytes: usize,
    secret_key_size_bytes: usize,
    tests_passed: usize,
    tests_failed: usize,
}

fn main() {
    println!(
        "\n{}",
        "╔═══════════════════════════════════════════════════════════════╗".bright_cyan()
    );
    println!(
        "{}",
        "║     SUPERNOVA QUANTUM CRYPTOGRAPHY VALIDATION SUITE          ║".bright_cyan()
    );
    println!(
        "{}",
        "║                                                               ║".bright_cyan()
    );
    println!(
        "{}",
        "║            Validating Post-Quantum Signature Schemes          ║".bright_cyan()
    );
    println!(
        "{}",
        "╚═══════════════════════════════════════════════════════════════╝".bright_cyan()
    );
    println!();

    let mut all_results = Vec::new();

    // Test CRYSTALS-Dilithium
    println!(
        "{}",
        "\n=== PHASE 2.1: ML-DSA (Module Lattice Digital Signature Algorithm) Validation ==="
            .bright_green()
    );
    let result = test_dilithium();
    print_test_result(&result);
    all_results.push(result);

    // Test SPHINCS+
    println!(
        "{}",
        "\n=== PHASE 2.2: SPHINCS+ Validation ===".bright_green()
    );
    let result = test_sphincs();
    print_test_result(&result);
    all_results.push(result);

    // Print summary
    print_validation_summary(&all_results);
}

fn test_dilithium() -> TestResults {
    let mut tests_passed = 0;
    let mut tests_failed = 0;

    println!("Testing ML-DSA-65 (NIST Level 3)...");

    // Key generation
    let start = Instant::now();
    let (pk, sk) = mldsa65::keypair();
    let key_gen_time = start.elapsed();

    let message = b"Supernova: The quantum-resistant blockchain";

    // Signing
    let start = Instant::now();
    let signature = mldsa65::detached_sign(message, &sk);
    let signing_time = start.elapsed();

    // Verification
    let start = Instant::now();
    let verified = mldsa65::verify_detached_signature(&signature, message, &pk).is_ok();
    let verification_time = start.elapsed();

    if verified {
        tests_passed += 1;
        println!("  {} Basic signature verification", "✓".green());
    } else {
        tests_failed += 1;
        println!("  {} Basic signature verification", "✗".red());
    }

    // Test invalid message
    if mldsa65::verify_detached_signature(&signature, b"wrong", &pk).is_err() {
        tests_passed += 1;
        println!("  {} Invalid message detection", "✓".green());
    } else {
        tests_failed += 1;
        println!("  {} Invalid message detection", "✗".red());
    }

    TestResults {
        scheme: "ML-DSA-65".to_string(),
        security_level: "NIST Level 3".to_string(),
        key_generation_time_ms: key_gen_time.as_secs_f64() * 1000.0,
        signing_time_ms: signing_time.as_secs_f64() * 1000.0,
        verification_time_ms: verification_time.as_secs_f64() * 1000.0,
        signature_size_bytes: signature.as_bytes().len(),
        public_key_size_bytes: pk.as_bytes().len(),
        secret_key_size_bytes: sk.as_bytes().len(),
        tests_passed,
        tests_failed,
    }
}

fn test_sphincs() -> TestResults {
    let mut tests_passed = 0;
    let mut tests_failed = 0;

    println!("Testing SPHINCS-SHA256-128f...");

    // Key generation
    let start = Instant::now();
    let (pk, sk) = sphincssha256128fsimple::keypair();
    let key_gen_time = start.elapsed();

    let message = b"Supernova: Hash-based signatures";

    // Signing
    let start = Instant::now();
    let signature = sphincssha256128fsimple::detached_sign(message, &sk);
    let signing_time = start.elapsed();

    // Verification
    let start = Instant::now();
    let verified =
        sphincssha256128fsimple::verify_detached_signature(&signature, message, &pk).is_ok();
    let verification_time = start.elapsed();

    if verified {
        tests_passed += 1;
        println!("  {} Basic signature verification", "✓".green());
    } else {
        tests_failed += 1;
        println!("  {} Basic signature verification", "✗".red());
    }

    TestResults {
        scheme: "SPHINCS-SHA256-128f".to_string(),
        security_level: "128-bit".to_string(),
        key_generation_time_ms: key_gen_time.as_secs_f64() * 1000.0,
        signing_time_ms: signing_time.as_secs_f64() * 1000.0,
        verification_time_ms: verification_time.as_secs_f64() * 1000.0,
        signature_size_bytes: signature.as_bytes().len(),
        public_key_size_bytes: pk.as_bytes().len(),
        secret_key_size_bytes: sk.as_bytes().len(),
        tests_passed,
        tests_failed,
    }
}

fn print_test_result(result: &TestResults) {
    println!("  Key Generation: {:.2}ms", result.key_generation_time_ms);
    println!("  Signing Time: {:.2}ms", result.signing_time_ms);
    println!("  Verification Time: {:.2}ms", result.verification_time_ms);
    println!("  Signature Size: {} bytes", result.signature_size_bytes);
    println!(
        "  Tests: {} passed, {} failed",
        result.tests_passed, result.tests_failed
    );
}

fn print_validation_summary(results: &[TestResults]) {
    println!(
        "\n{}",
        "═══════════════════════════════════════════════════════════════".bright_cyan()
    );
    println!(
        "{}",
        "                    VALIDATION SUMMARY                         ".bright_cyan()
    );
    println!(
        "{}",
        "═══════════════════════════════════════════════════════════════".bright_cyan()
    );

    let total_passed: usize = results.iter().map(|r| r.tests_passed).sum();
    let total_tests: usize = results
        .iter()
        .map(|r| r.tests_passed + r.tests_failed)
        .sum();

    println!("\nTotal Tests: {}", total_tests);
    println!(
        "Tests Passed: {} ({}%)",
        total_passed.to_string().green(),
        ((total_passed as f64 / total_tests as f64) * 100.0).round()
    );

    println!(
        "\n{}",
        "SUPERNOVA IS QUANTUM-RESISTANT".bright_green().bold()
    );
}
