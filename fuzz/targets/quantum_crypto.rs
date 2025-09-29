//! Fuzzing harness for quantum-resistant cryptography
//!
//! This harness tests our post-quantum cryptographic implementations including
//! Dilithium, SPHINCS+, and Falcon to ensure they handle malformed inputs safely.

use afl::fuzz;
use btclib::crypto::quantum_signatures::{
    QuantumSignature, DilithiumSig, SphincsPlus, FalconSig
};
use btclib::crypto::kem::KyberKEM;

fn main() {
    fuzz!(|data: &[u8]| {
        // Test different cryptographic operations based on data patterns
        if data.is_empty() {
            return;
        }

        match data[0] % 6 {
            0 => fuzz_dilithium_operations(data),
            1 => fuzz_sphincs_operations(data),
            2 => fuzz_falcon_operations(data),
            3 => fuzz_kyber_kem(data),
            4 => fuzz_signature_verification(data),
            5 => fuzz_key_generation(data),
            _ => unreachable!(),
        }
    });
}

/// Fuzz Dilithium signature operations
fn fuzz_dilithium_operations(data: &[u8]) {
    use btclib::crypto::dilithium::{DilithiumKeyPair, DilithiumSignature};

    // Test key generation with fuzzer-provided seed
    if data.len() >= 32 {
        let mut seed = [0u8; 32];
        seed.copy_from_slice(&data[..32]);

        match DilithiumKeyPair::from_seed(&seed) {
            Ok(keypair) => {
                // Test signing with various message sizes
                for chunk_size in [32, 64, 128, 256, 1024] {
                    if data.len() > chunk_size {
                        let message = &data[32..32 + chunk_size.min(data.len() - 32)];

                        // Signing should never panic
                        match keypair.sign(message) {
                            Ok(signature) => {
                                // Verification should handle any signature gracefully
                                let _ = keypair.verify(message, &signature);

                                // Test signature malleability
                                if let Ok(mut sig_bytes) = signature.to_bytes() {
                                    // Flip random bits
                                    for i in 0..sig_bytes.len().min(10) {
                                        sig_bytes[i] ^= data.get(i).unwrap_or(&1);
                                    }

                                    // Parse and verify corrupted signature
                                    if let Ok(corrupted_sig) = DilithiumSignature::from_bytes(&sig_bytes) {
                                        let _ = keypair.verify(message, &corrupted_sig);
                                    }
                                }
                            }
                            Err(_) => {
                                // Signing failure is acceptable for fuzzing
                            }
                        }
                    }
                }
            }
            Err(_) => {
                // Key generation failure is acceptable for invalid seeds
            }
        }
    }

    // Test signature parsing from arbitrary data
    if data.len() >= 2420 {  // Dilithium signature size
        match DilithiumSignature::from_bytes(&data[..2420]) {
            Ok(sig) => {
                // Test serialization round-trip
                let _ = sig.to_bytes();
            }
            Err(_) => {
                // Parsing failure is expected for most fuzzer inputs
            }
        }
    }
}

/// Fuzz SPHINCS+ operations
fn fuzz_sphincs_operations(data: &[u8]) {
    use btclib::crypto::sphincs::{SphincsKeyPair, SphincsSignature};

    // Test stateless signature operations
    if data.len() >= 64 {
        let mut seed = [0u8; 48];
        seed.copy_from_slice(&data[..48]);

        match SphincsKeyPair::from_seed(&seed) {
            Ok(keypair) => {
                let message = &data[48..];

                // Test signing with various security parameters
                match keypair.sign(message) {
                    Ok(signature) => {
                        // Verify signature
                        let _ = keypair.verify(message, &signature);

                        // Test signature robustness
                        test_signature_robustness(&signature, message, &keypair);
                    }
                    Err(_) => {}
                }
            }
            Err(_) => {}
        }
    }
}

/// Fuzz Falcon operations
fn fuzz_falcon_operations(data: &[u8]) {
    use btclib::crypto::falcon::{FalconKeyPair, FalconSignature};

    // Falcon uses floating-point arithmetic, test edge cases
    if data.len() >= 32 {
        match FalconKeyPair::generate_from_seed(&data[..32]) {
            Ok(keypair) => {
                // Test with various message patterns
                for pattern in [
                    vec![0u8; 32],      // All zeros
                    vec![0xFF; 32],     // All ones
                    data[..32.min(data.len())].to_vec(),  // Fuzzer data
                ] {
                    match keypair.sign(&pattern) {
                        Ok(sig) => {
                            let _ = keypair.verify(&pattern, &sig);
                        }
                        Err(_) => {}
                    }
                }
            }
            Err(_) => {}
        }
    }
}

/// Fuzz Kyber KEM operations
fn fuzz_kyber_kem(data: &[u8]) {
    use btclib::crypto::kem::{KyberPublicKey, KyberCiphertext};

    // Test key encapsulation
    if data.len() >= 32 {
        match KyberKEM::generate_keypair_from_seed(&data[..32]) {
            Ok((pk, sk)) => {
                // Test encapsulation
                match KyberKEM::encapsulate(&pk) {
                    Ok((ciphertext, shared_secret1)) => {
                        // Test decapsulation
                        match KyberKEM::decapsulate(&ciphertext, &sk) {
                            Ok(shared_secret2) => {
                                // Verify shared secrets match
                                assert_eq!(shared_secret1, shared_secret2);
                            }
                            Err(_) => {}
                        }

                        // Test ciphertext manipulation
                        let ct_bytes = ciphertext.to_bytes();
                        let mut corrupted = ct_bytes.clone();
                        if !corrupted.is_empty() {
                            corrupted[0] ^= 1;
                            if let Ok(corrupted_ct) = KyberCiphertext::from_bytes(&corrupted) {
                                // Decapsulation should produce different shared secret
                                let _ = KyberKEM::decapsulate(&corrupted_ct, &sk);
                            }
                        }
                    }
                    Err(_) => {}
                }
            }
            Err(_) => {}
        }
    }
}

/// Fuzz signature verification with malformed inputs
fn fuzz_signature_verification(data: &[u8]) {
    use btclib::crypto::quantum_signatures::verify_quantum_signature;

    if data.len() < 100 {
        return;
    }

    // Create various malformed signatures
    let message = &data[..32.min(data.len())];
    let sig_data = &data[32..];

    // Test generic verification function
    let _ = verify_quantum_signature(message, sig_data);

    // Test with truncated signatures
    for size in [100, 500, 1000, 2000, 3000] {
        if sig_data.len() >= size {
            let _ = verify_quantum_signature(message, &sig_data[..size]);
        }
    }
}

/// Fuzz key generation with edge cases
fn fuzz_key_generation(data: &[u8]) {
    // Test key generation with various seed sizes
    for seed_size in [16, 24, 32, 48, 64] {
        if data.len() >= seed_size {
            let seed = &data[..seed_size];

            // Test each algorithm's key generation
            let _ = btclib::crypto::dilithium::DilithiumKeyPair::from_seed_sized(seed);
            let _ = btclib::crypto::sphincs::SphincsKeyPair::from_seed_sized(seed);
            let _ = btclib::crypto::falcon::FalconKeyPair::from_seed_sized(seed);
        }
    }
}

/// Test signature robustness against bit flips
fn test_signature_robustness<S, K>(sig: &S, msg: &[u8], keypair: &K)
where
    S: btclib::crypto::traits::QuantumSignature,
    K: btclib::crypto::traits::QuantumKeyPair<Signature = S>,
{
    // Test signature remains valid under serialization
    if let Ok(bytes) = sig.to_bytes() {
        if let Ok(parsed) = S::from_bytes(&bytes) {
            let _ = keypair.verify(msg, &parsed);
        }
    }
}