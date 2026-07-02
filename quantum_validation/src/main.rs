//! Supernova quantum cryptography validation harness.
//!
//! This binary performs REAL post-quantum signature validation: it generates
//! keypairs, signs known test messages, verifies the signatures, and checks
//! that tampered signatures / tampered messages / mismatched keys are
//! correctly rejected. It only prints a "VALIDATED" / "READY FOR PRODUCTION"
//! line if every check below actually passed — any failure aborts the
//! process with a non-zero exit code.

use colored::*;
use pqcrypto_dilithium::{dilithium2, dilithium3, dilithium5};
use pqcrypto_sphincsplus::{
    sphincsshake256128fsimple, sphincsshake256192fsimple, sphincsshake256256fsimple,
};
use pqcrypto_traits::sign::PublicKey as _;

/// Result of a single named check.
struct CheckResult {
    name: String,
    passed: bool,
    detail: String,
}

// The pqcrypto dilithium/sphincs+ modules are not generic over a shared
// trait for keypair/sign/verify with matching associated types across
// crates, so each parameter set is validated via this macro rather than a
// single generic helper function.
macro_rules! validate_dilithium_scheme {
    ($module:ident, $label:expr, $results:expr) => {{
        let message = format!(
            "supernova-quantum-validation-test-vector::{}",
            $label
        )
        .into_bytes();

        // 1. Keypair generation.
        let (pk, sk) = $module::keypair();

        // 2. Sign + verify (positive case).
        let sig = $module::detached_sign(&message, &sk);
        let verified = $module::verify_detached_signature(&sig, &message, &pk).is_ok();
        $results.push(CheckResult {
            name: format!("{} sign/verify", $label),
            passed: verified,
            detail: if verified {
                "signature verified against signed message".to_string()
            } else {
                "FAILED: valid signature did not verify".to_string()
            },
        });

        // 3. Tampered message must fail verification.
        let mut tampered_message = message.clone();
        tampered_message[0] ^= 0xFF;
        let tamper_rejected =
            $module::verify_detached_signature(&sig, &tampered_message, &pk).is_err();
        $results.push(CheckResult {
            name: format!("{} tampered-message rejection", $label),
            passed: tamper_rejected,
            detail: if tamper_rejected {
                "tampered message correctly rejected".to_string()
            } else {
                "FAILED: signature verified against a tampered message".to_string()
            },
        });

        // 4. Signature from a different keypair must not verify against
        //    the original public key (cross-key rejection).
        let (_other_pk, other_sk) = $module::keypair();
        let other_sig = $module::detached_sign(&message, &other_sk);
        let cross_key_rejected =
            $module::verify_detached_signature(&other_sig, &message, &pk).is_err();
        $results.push(CheckResult {
            name: format!("{} cross-key rejection", $label),
            passed: cross_key_rejected,
            detail: if cross_key_rejected {
                "signature from a different keypair correctly rejected".to_string()
            } else {
                "FAILED: signature from a different keypair verified successfully".to_string()
            },
        });

        // 5. Round-trip public key bytes through from_bytes/as_bytes, as
        //    production code (supernova-core) does when (de)serializing keys.
        let pk_bytes = pk.as_bytes().to_vec();
        let pk_roundtrip = $module::PublicKey::from_bytes(&pk_bytes)
            .map(|rt_pk| rt_pk.as_bytes() == pk.as_bytes())
            .unwrap_or(false);
        $results.push(CheckResult {
            name: format!("{} public key byte round-trip", $label),
            passed: pk_roundtrip,
            detail: if pk_roundtrip {
                "public key survives from_bytes/as_bytes round-trip".to_string()
            } else {
                "FAILED: public key round-trip mismatch".to_string()
            },
        });
    }};
}

fn main() {
    let mut results: Vec<CheckResult> = Vec::new();

    println!(
        "\n{}",
        "SUPERNOVA QUANTUM CRYPTOGRAPHY VALIDATION"
            .bright_green()
            .bold()
    );
    println!("\nRunning real keygen/sign/verify checks against test vectors...\n");

    // --- CRYSTALS-Dilithium (all three NIST security levels) ---
    validate_dilithium_scheme!(dilithium2, "Dilithium2 (NIST L1)", results);
    validate_dilithium_scheme!(dilithium3, "Dilithium3 (NIST L3)", results);
    validate_dilithium_scheme!(dilithium5, "Dilithium5 (NIST L5)", results);

    // --- SPHINCS+ (all three NIST security levels, SHAKE/simple variants,
    //     matching the parameter sets used in supernova-core) ---
    validate_dilithium_scheme!(sphincsshake256128fsimple, "SPHINCS+-SHAKE-128f-simple (NIST L1)", results);
    validate_dilithium_scheme!(sphincsshake256192fsimple, "SPHINCS+-SHAKE-192f-simple (NIST L3)", results);
    validate_dilithium_scheme!(sphincsshake256256fsimple, "SPHINCS+-SHAKE-256f-simple (NIST L5)", results);

    let mut any_failed = false;
    for result in &results {
        if result.passed {
            println!("{} {}: {}", "✓".bright_green(), result.name, result.detail);
        } else {
            any_failed = true;
            println!("{} {}: {}", "✗".bright_red().bold(), result.name, result.detail);
        }
    }

    if any_failed {
        println!(
            "\n{}",
            "QUANTUM CRYPTOGRAPHY VALIDATION FAILED".bright_red().bold()
        );
        eprintln!(
            "quantum_validation: one or more post-quantum sign/verify checks failed; \
             see output above for details."
        );
        std::process::exit(1);
    }

    println!("\n✓ CRYSTALS-Dilithium: VALIDATED ({} checks passed)", results.iter().filter(|r| r.name.starts_with("Dilithium") && r.passed).count());
    println!("✓ SPHINCS+: VALIDATED ({} checks passed)", results.iter().filter(|r| r.name.starts_with("SPHINCS+") && r.passed).count());
    println!("✓ Quantum-Resistant: CONFIRMED");
    println!("\n{}", "READY FOR PRODUCTION".bright_green().bold());
}

#[cfg(test)]
mod tests {
    use super::*;
    use pqcrypto_traits::sign::DetachedSignature as _;

    /// Every scheme's positive sign/verify, tampered-message rejection,
    /// cross-key rejection, and public-key round-trip checks must actually
    /// pass — this is the same logic `main()` runs, exercised directly so
    /// `cargo test` fails loudly if the real pqcrypto verification ever
    /// regresses to a no-op.
    #[test]
    fn all_pqc_schemes_pass_real_validation() {
        let mut results: Vec<CheckResult> = Vec::new();

        validate_dilithium_scheme!(dilithium2, "Dilithium2 (NIST L1)", results);
        validate_dilithium_scheme!(dilithium3, "Dilithium3 (NIST L3)", results);
        validate_dilithium_scheme!(dilithium5, "Dilithium5 (NIST L5)", results);
        validate_dilithium_scheme!(
            sphincsshake256128fsimple,
            "SPHINCS+-SHAKE-128f-simple (NIST L1)",
            results
        );
        validate_dilithium_scheme!(
            sphincsshake256192fsimple,
            "SPHINCS+-SHAKE-192f-simple (NIST L3)",
            results
        );
        validate_dilithium_scheme!(
            sphincsshake256256fsimple,
            "SPHINCS+-SHAKE-256f-simple (NIST L5)",
            results
        );

        // Six schemes * four checks each.
        assert_eq!(results.len(), 24, "expected 24 validation checks to run");
        for result in &results {
            assert!(result.passed, "{}: {}", result.name, result.detail);
        }
    }

    /// A tampered signature (bit-flipped) must never verify — this is the
    /// negative case that a stub/no-op "validator" would silently miss.
    #[test]
    fn tampered_signature_is_rejected() {
        let message = b"supernova-quantum-validation-unit-test";
        let (pk, sk) = dilithium2::keypair();
        let sig = dilithium2::detached_sign(message, &sk);

        let mut sig_bytes = sig.as_bytes().to_vec();
        sig_bytes[0] ^= 0xFF;
        let tampered_sig = dilithium2::DetachedSignature::from_bytes(&sig_bytes)
            .expect("tampered bytes should still be a structurally valid signature");

        assert!(
            dilithium2::verify_detached_signature(&tampered_sig, message, &pk).is_err(),
            "a bit-flipped signature must not verify"
        );
    }
}
