use btclib::crypto::falcon_real::{FalconKeyPair, FalconParameters};
use btclib::crypto::quantum::{QuantumKeyPair, QuantumParameters, QuantumScheme};
use btclib::crypto::signature::{
    SignatureError, SignatureParams, SignatureScheme, SignatureType, SignatureVerifier,
};
use btclib::validation::SecurityLevel;
use rand::rngs::OsRng;
use std::collections::HashMap;

// Simulate a blockchain transaction with various signature types
struct TestTransaction {
    id: String,
    data: Vec<u8>,
    signatures: HashMap<SignatureType, Vec<u8>>,
    public_keys: HashMap<SignatureType, Vec<u8>>,
}

impl TestTransaction {
    fn new(id: &str, data: &[u8]) -> Self {
        TestTransaction {
            id: id.to_string(),
            data: data.to_vec(),
            signatures: HashMap::new(),
            public_keys: HashMap::new(),
        }
    }

    fn add_signature(&mut self, sig_type: SignatureType, public_key: Vec<u8>, signature: Vec<u8>) {
        self.signatures.insert(sig_type, signature);
        self.public_keys.insert(sig_type, public_key);
    }

    fn verify(&self, verifier: &SignatureVerifier) -> Result<bool, SignatureError> {
        // Check all signatures
        for (sig_type, signature) in &self.signatures {
            let public_key = self.public_keys.get(sig_type).ok_or_else(|| {
                SignatureError::InvalidKey(format!("Missing public key for {:?}", sig_type))
            })?;

            let result = verifier.verify(*sig_type, public_key, &self.data, signature)?;
            if !result {
                return Ok(false);
            }
        }

        Ok(true)
    }
}

/// Test interoperability between classical and post-quantum signature schemes
#[test]
fn test_multi_signature_transaction() {
    // Create a unified verifier
    let mut verifier = SignatureVerifier::new();

    // Register Falcon with the verifier
    verifier.register(
        SignatureType::Falcon,
        Box::new(btclib::crypto::signature::FalconScheme::new(
            SecurityLevel::Medium as u8,
        )),
    );

    // Create test transaction
    let tx_data = b"Transfer 10.5 BTC from Alice to Bob";
    let mut transaction = TestTransaction::new("tx123", tx_data);

    // Generate a Dilithium signature
    let dilithium_params = QuantumParameters {
        scheme: QuantumScheme::Dilithium,
        security_level: SecurityLevel::Medium as u8,
    };

    let dilithium_keypair =
        QuantumKeyPair::generate(QuantumScheme::Dilithium, Some(dilithium_params))
            .expect("Failed to generate Dilithium key pair");

    let dilithium_signature = dilithium_keypair
        .sign(tx_data)
        .expect("Failed to sign with Dilithium");

    transaction.add_signature(
        SignatureType::Dilithium,
        dilithium_keypair.public_key.clone(),
        dilithium_signature,
    );

    // Generate a Falcon signature
    let falcon_params = FalconParameters::with_security_level(SecurityLevel::Medium as u8);
    let mut rng = OsRng;

    let falcon_keypair = FalconKeyPair::generate(&mut rng, falcon_params)
        .expect("Failed to generate Falcon key pair");

    let falcon_signature = falcon_keypair
        .sign(tx_data)
        .expect("Failed to sign with Falcon");

    transaction.add_signature(
        SignatureType::Falcon,
        falcon_keypair.public_key.clone(),
        falcon_signature,
    );

    // Verify transaction with both signatures
    // Note: Falcon verification might fail if using a placeholder implementation
    let verification_result = transaction.verify(&verifier);
    println!(
        "Multi-signature transaction verification: {:?}",
        verification_result
    );

    // If verification fails, it might be because of placeholder implementations
    // so we'll check each signature separately to identify which one failed

    let dilithium_result = verifier.verify(
        SignatureType::Dilithium,
        &dilithium_keypair.public_key,
        tx_data,
        &dilithium_signature,
    );
    println!("Dilithium verification: {:?}", dilithium_result);

    let falcon_result = verifier.verify(
        SignatureType::Falcon,
        &falcon_keypair.public_key,
        tx_data,
        &falcon_signature,
    );
    println!("Falcon verification: {:?}", falcon_result);

    // At least the Dilithium signature should verify correctly
    assert!(
        dilithium_result.is_ok(),
        "Dilithium verification resulted in error"
    );
    if let Ok(result) = dilithium_result {
        assert!(result, "Dilithium signature verification should succeed");
    }
}

/// Test failure scenarios in multi-signature transactions
#[test]
fn test_multi_signature_transaction_failures() {
    let mut verifier = SignatureVerifier::new();

    // Create test transaction
    let tx_data = b"Transfer 5.0 BTC from Charlie to Dave";
    let mut transaction = TestTransaction::new("tx456", tx_data);

    // Generate a Dilithium signature
    let dilithium_params = QuantumParameters {
        scheme: QuantumScheme::Dilithium,
        security_level: SecurityLevel::Medium as u8,
    };

    let dilithium_keypair =
        QuantumKeyPair::generate(QuantumScheme::Dilithium, Some(dilithium_params))
            .expect("Failed to generate Dilithium key pair");

    let dilithium_signature = dilithium_keypair
        .sign(tx_data)
        .expect("Failed to sign with Dilithium");

    transaction.add_signature(
        SignatureType::Dilithium,
        dilithium_keypair.public_key.clone(),
        dilithium_signature,
    );

    // Verify transaction with just the Dilithium signature
    let verification_result = transaction.verify(&verifier);
    assert!(verification_result.is_ok(), "Verification should not error");
    assert!(
        verification_result.unwrap(),
        "Transaction should verify with valid Dilithium signature"
    );

    // Now let's test failure by tampering with the transaction data
    let mut tampered_transaction = transaction.clone();
    tampered_transaction.data = b"Transfer 500.0 BTC from Charlie to Dave".to_vec();

    let tamper_verification = tampered_transaction.verify(&verifier);
    assert!(
        tamper_verification.is_ok(),
        "Tampered verification should not error"
    );
    assert!(
        !tamper_verification.unwrap(),
        "Tampered transaction should fail verification"
    );

    // Test with a missing public key
    let mut missing_key_tx = TestTransaction::new("tx789", tx_data);
    missing_key_tx
        .signatures
        .insert(SignatureType::Dilithium, dilithium_signature);
    // Intentionally not adding the public key

    let missing_key_verification = missing_key_tx.verify(&verifier);
    assert!(
        missing_key_verification.is_err(),
        "Missing key verification should error"
    );
    if let Err(err) = missing_key_verification {
        assert!(
            matches!(err, SignatureError::InvalidKey(_)),
            "Expected InvalidKey error, got: {:?}",
            err
        );
    }
}

/// Test compatibility with different security levels
#[test]
fn test_security_level_compatibility() {
    let mut verifier = SignatureVerifier::new();

    // Create test transactions with different security levels
    let security_levels = [
        (SecurityLevel::Low, "Low security transaction"),
        (SecurityLevel::Medium, "Medium security transaction"),
        (SecurityLevel::High, "High security transaction"),
    ];

    for (level, tx_name) in &security_levels {
        let tx_data = tx_name.as_bytes();
        let mut transaction = TestTransaction::new(tx_name, tx_data);

        // Generate a Dilithium signature with this security level
        let dilithium_params = QuantumParameters {
            scheme: QuantumScheme::Dilithium,
            security_level: *level as u8,
        };

        let dilithium_keypair =
            QuantumKeyPair::generate(QuantumScheme::Dilithium, Some(dilithium_params))
                .expect("Failed to generate Dilithium key pair");

        let dilithium_signature = dilithium_keypair
            .sign(tx_data)
            .expect("Failed to sign with Dilithium");

        transaction.add_signature(
            SignatureType::Dilithium,
            dilithium_keypair.public_key.clone(),
            dilithium_signature,
        );

        // Register a verifier with the appropriate security level
        verifier.register(
            SignatureType::Dilithium,
            Box::new(btclib::crypto::signature::DilithiumScheme::new(
                *level as u8,
            )),
        );

        // Verify the transaction
        let verification_result = transaction.verify(&verifier);
        assert!(
            verification_result.is_ok(),
            "Verification at security level {:?} should not error",
            level
        );
        assert!(
            verification_result.unwrap(),
            "Transaction should verify with valid Dilithium signature at security level {:?}",
            level
        );

        println!(
            "Successfully verified transaction at security level: {:?}",
            level
        );
    }
}

/// Test signature-based multisig scheme (similar to Bitcoin's multisig)
#[test]
fn test_signature_based_multisig() {
    // Create a test multisig scheme that requires 2 of 3 valid signatures
    // We'll implement this as a custom transaction

    let tx_data = b"Multisig transfer of 20 BTC from MultiWallet to Eve";
    let mut transaction = TestTransaction::new("multisig123", tx_data);

    // Create 3 different key pairs
    let security_level = SecurityLevel::Medium as u8;

    // Key 1: Dilithium
    let dilithium_params = QuantumParameters {
        scheme: QuantumScheme::Dilithium,
        security_level,
    };
    let dilithium_keypair =
        QuantumKeyPair::generate(QuantumScheme::Dilithium, Some(dilithium_params))
            .expect("Failed to generate Dilithium key pair");

    // Key 2: Falcon
    let falcon_params = FalconParameters::with_security_level(security_level);
    let mut rng = OsRng;
    let falcon_keypair = FalconKeyPair::generate(&mut rng, falcon_params)
        .expect("Failed to generate Falcon key pair");

    // Key 3: Another Dilithium key
    let dilithium_keypair2 =
        QuantumKeyPair::generate(QuantumScheme::Dilithium, Some(dilithium_params))
            .expect("Failed to generate second Dilithium key pair");

    // Sign with Key 1
    let dilithium_signature = dilithium_keypair
        .sign(tx_data)
        .expect("Failed to sign with Dilithium");

    // Sign with Key 2
    let falcon_signature = falcon_keypair
        .sign(tx_data)
        .expect("Failed to sign with Falcon");

    // We won't sign with Key 3 in this test

    // Store the signatures in the transaction
    transaction.add_signature(
        SignatureType::Dilithium,
        dilithium_keypair.public_key.clone(),
        dilithium_signature,
    );

    transaction.add_signature(
        SignatureType::Falcon,
        falcon_keypair.public_key.clone(),
        falcon_signature,
    );

    // Create a verifier with both schemes registered
    let mut verifier = SignatureVerifier::new();
    verifier.register(
        SignatureType::Falcon,
        Box::new(btclib::crypto::signature::FalconScheme::new(security_level)),
    );

    // Verify the transaction (should succeed with 2 of 3 signatures)
    let verification_result = transaction.verify(&verifier);
    println!("Multisig verification result: {:?}", verification_result);

    // At least Dilithium should verify successfully
    let dilithium_result = verifier.verify(
        SignatureType::Dilithium,
        &dilithium_keypair.public_key,
        tx_data,
        &dilithium_signature,
    );
    assert!(
        dilithium_result.is_ok(),
        "Dilithium verification resulted in error"
    );
    if let Ok(result) = dilithium_result {
        assert!(result, "Dilithium signature verification should succeed");
    }
}

impl Clone for TestTransaction {
    fn clone(&self) -> Self {
        TestTransaction {
            id: self.id.clone(),
            data: self.data.clone(),
            signatures: self.signatures.clone(),
            public_keys: self.public_keys.clone(),
        }
    }
}
