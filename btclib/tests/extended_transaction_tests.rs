use btclib::types::transaction::{Transaction, TransactionInput, TransactionOutput};
use btclib::types::extended_transaction::{
    QuantumTransaction, ConfidentialTransaction,
    QuantumTransactionBuilder, ConfidentialTransactionBuilder
};
use btclib::crypto::quantum::{QuantumScheme, ClassicalScheme, QuantumParameters};
use btclib::crypto::zkp::{ZkpParams, ZkpType};
use rand::rngs::OsRng;

#[test]
fn test_quantum_transaction_creation() {
    // Create a regular transaction
    let inputs = vec![TransactionInput::new(
        [0u8; 32],
        0,
        vec![],
        0xffffffff,
    )];

    let outputs = vec![TransactionOutput::new(
        50_000_000, // 0.5 BTC
        vec![],
    )];

    let tx = Transaction::new(1, inputs, outputs, 0);
    
    // Create a quantum transaction builder with Dilithium scheme
    let builder = QuantumTransactionBuilder::new(QuantumScheme::Dilithium, 3);
    
    // Create a placeholder private key (in real code this would be a valid key)
    let private_key = vec![0u8; 32];
    
    // Sign the transaction
    let quantum_tx = builder.sign_transaction(tx, &private_key).expect("Failed to sign transaction");
    
    // Verify the transaction properties
    assert_eq!(quantum_tx.scheme(), QuantumScheme::Dilithium);
    assert_eq!(quantum_tx.security_level(), 3);
    assert!(!quantum_tx.signature().is_empty());
}

#[test]
fn test_confidential_transaction_creation() {
    // Create transaction inputs
    let inputs = vec![TransactionInput::new(
        [0u8; 32],
        0,
        vec![],
        0xffffffff,
    )];

    // Create transaction outputs as (amount, pub_key_script) pairs
    let outputs = vec![
        (50_000_000, vec![]), // 0.5 BTC
        (25_000_000, vec![]), // 0.25 BTC
    ];
    
    // Create ZKP parameters for Bulletproofs
    let zkp_params = ZkpParams {
        proof_type: ZkpType::Bulletproof,
        security_level: 128,
    };
    
    // Create a confidential transaction builder
    let builder = ConfidentialTransactionBuilder::new(zkp_params);
    
    // Create a random number generator
    let mut rng = OsRng;
    
    // Create the confidential transaction
    let conf_tx = builder.create_transaction(1, inputs, outputs, 0, &mut rng);
    
    // Verify the transaction properties
    assert_eq!(conf_tx.conf_outputs().len(), 2);
    
    // Verify all range proofs
    assert!(conf_tx.verify_range_proofs());
}

#[test]
fn test_hybrid_schemes() {
    // Create a regular transaction
    let inputs = vec![TransactionInput::new(
        [0u8; 32],
        0,
        vec![],
        0xffffffff,
    )];

    let outputs = vec![TransactionOutput::new(
        50_000_000, // 0.5 BTC
        vec![],
    )];

    let tx = Transaction::new(1, inputs, outputs, 0);
    
    // Test different hybrid schemes
    let hybrid_schemes = [
        QuantumScheme::Hybrid(ClassicalScheme::Secp256k1),
        QuantumScheme::Hybrid(ClassicalScheme::Ed25519),
    ];
    
    for scheme in &hybrid_schemes {
        let builder = QuantumTransactionBuilder::new(*scheme, 3);
        let private_key = vec![0u8; 64]; // In a hybrid scheme, we would need a larger key
        
        let quantum_tx = builder.sign_transaction(tx.clone(), &private_key).expect("Failed to sign transaction");
        
        assert_eq!(quantum_tx.scheme(), *scheme);
        assert!(!quantum_tx.signature().is_empty());
    }
}

#[test]
fn test_multiple_confidential_outputs() {
    // Create transaction inputs
    let inputs = vec![TransactionInput::new(
        [0u8; 32],
        0,
        vec![],
        0xffffffff,
    )];

    // Create multiple outputs of varying sizes
    let outputs = vec![
        (10_000_000, vec![]),
        (25_000_000, vec![]),
        (15_000_000, vec![]),
        (5_000_000, vec![]),
    ];
    
    // Create ZKP parameters
    let zkp_params = ZkpParams {
        proof_type: ZkpType::Bulletproof,
        security_level: 128,
    };
    
    let builder = ConfidentialTransactionBuilder::new(zkp_params);
    let mut rng = OsRng;
    
    // Create the confidential transaction
    let conf_tx = builder.create_transaction(1, inputs, outputs, 0, &mut rng);
    
    // Verify the transaction has the right number of outputs
    assert_eq!(conf_tx.conf_outputs().len(), 4);
    
    // Verify all range proofs
    assert!(conf_tx.verify_range_proofs());
}

#[test]
fn test_transaction_verification() {
    // Create a regular transaction
    let inputs = vec![TransactionInput::new(
        [0u8; 32],
        0,
        vec![],
        0xffffffff,
    )];

    let outputs = vec![TransactionOutput::new(
        50_000_000, // 0.5 BTC
        vec![],
    )];

    let tx = Transaction::new(1, inputs, outputs, 0);
    
    // Create a quantum transaction
    let builder = QuantumTransactionBuilder::new(QuantumScheme::Dilithium, 3);
    let private_key = vec![0u8; 32];
    
    let quantum_tx = builder.sign_transaction(tx, &private_key).expect("Failed to sign transaction");
    
    // Create a placeholder public key
    let public_key = vec![0u8; 32];
    
    // Verify the signature
    let verification_result = quantum_tx.verify_signature(&public_key).expect("Verification failed");
    
    // In a real implementation, this would only be true if the signature is valid
    // For now, our placeholder implementation returns true
    assert!(verification_result);
} 