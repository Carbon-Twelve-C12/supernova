use std::sync::Arc;
use dashmap::DashMap;
use rand::rngs::OsRng;

use btclib::api::{CryptoAPI, create_testnet_api};
use btclib::config::{Config, NetworkType};
use btclib::crypto::quantum::{QuantumScheme, QuantumKeyPair, QuantumParameters};
use btclib::crypto::zkp::{ZkpType, ZkpParams};
use btclib::transaction_processor::{TransactionProcessor, TransactionProcessorError};
use btclib::types::transaction::{Transaction, TransactionInput, TransactionOutput};
use btclib::types::extended_transaction::{QuantumTransaction, ConfidentialTransaction};

#[test]
fn test_quantum_signature_flow() {
    // Create a test API with quantum features enabled
    let api = create_testnet_api();
    
    // Generate a quantum key pair
    let mut rng = OsRng;
    let keypair = api.generate_quantum_keypair(&mut rng).expect("Failed to generate keypair");
    
    // Create a simple transaction
    let inputs = vec![TransactionInput::new(
        [1u8; 32],
        0,
        vec![],
        0xffffffff,
    )];
    
    let outputs = vec![TransactionOutput::new(
        90_000_000,
        vec![],
    )];
    
    let tx = Transaction::new(1, inputs, outputs, 0);
    
    // Sign the transaction with the quantum key
    let quantum_tx = api.sign_transaction_quantum(tx, &keypair).expect("Failed to sign transaction");
    
    // Verify the transaction
    let verification_result = api.verify_quantum_transaction(&quantum_tx, &keypair.public_key)
        .expect("Failed to verify transaction");
    
    assert!(verification_result, "Transaction signature verification failed");
}

#[test]
fn test_confidential_transaction_flow() {
    // Create a test API with confidential transaction features enabled
    let api = create_testnet_api();
    
    // Create a transaction with inputs and outputs
    let inputs = vec![TransactionInput::new(
        [1u8; 32],
        0,
        vec![],
        0xffffffff,
    )];
    
    let outputs = vec![
        (50_000_000, vec![]), // 0.5 NOVA
        (40_000_000, vec![]), // 0.4 NOVA
    ];
    
    // Create a confidential transaction
    let mut rng = OsRng;
    let (conf_tx, blinding_factors) = api.create_confidential_transaction(inputs, outputs, &mut rng)
        .expect("Failed to create confidential transaction");
    
    // Verify we have blinding factors for each output
    assert_eq!(blinding_factors.len(), 2);
    assert!(!blinding_factors[0].is_empty());
    assert!(!blinding_factors[1].is_empty());
    
    // Verify the transaction
    let verification_result = api.verify_confidential_transaction(&conf_tx)
        .expect("Failed to verify transaction");
    
    assert!(verification_result, "Confidential transaction verification failed");
}

#[test]
fn test_transaction_processing() {
    // Create a UTXO set
    let utxo_set = Arc::new(DashMap::new());
    
    // Add some UTXOs
    let prev_tx_hash = [1u8; 32];
    utxo_set.insert((prev_tx_hash, 0), TransactionOutput::new(100_000_000, vec![]));
    
    // Create a transaction processor with quantum and ZKP features enabled
    let processor = TransactionProcessor::new(utxo_set, true, true);
    
    // Create a quantum transaction
    let api = create_testnet_api();
    let mut rng = OsRng;
    
    // Generate a quantum key pair
    let keypair = api.generate_quantum_keypair(&mut rng).expect("Failed to generate keypair");
    
    // Create a transaction
    let inputs = vec![TransactionInput::new(
        prev_tx_hash,
        0,
        vec![],
        0xffffffff,
    )];
    
    let outputs = vec![TransactionOutput::new(
        90_000_000, // 0.9 NOVA
        vec![],
    )];
    
    let tx = Transaction::new(1, inputs, outputs, 0);
    
    // Sign the transaction
    let quantum_tx = api.sign_transaction_quantum(tx, &keypair).expect("Failed to sign transaction");
    
    // Process the transaction
    let result = processor.process_quantum_transaction(&quantum_tx);
    
    assert!(result.is_ok(), "Failed to process quantum transaction: {:?}", result);
}

#[test]
fn test_confidential_transaction_processing() {
    // Create a UTXO set
    let utxo_set = Arc::new(DashMap::new());
    
    // Add some UTXOs
    let prev_tx_hash = [1u8; 32];
    utxo_set.insert((prev_tx_hash, 0), TransactionOutput::new(100_000_000, vec![]));
    
    // Create a transaction processor with quantum and ZKP features enabled
    let processor = TransactionProcessor::new(utxo_set, true, true);
    
    // Create a quantum transaction
    let api = create_testnet_api();
    let mut rng = OsRng;
    
    // Create a transaction
    let inputs = vec![TransactionInput::new(
        prev_tx_hash,
        0,
        vec![],
        0xffffffff,
    )];
    
    let outputs = vec![
        (90_000_000, vec![]), // 0.9 NOVA
    ];
    
    // Create a confidential transaction
    let (conf_tx, blinding_factors) = api.create_confidential_transaction(inputs, outputs, &mut rng)
        .expect("Failed to create confidential transaction");
    
    // Ensure blinding factor was generated
    assert_eq!(blinding_factors.len(), 1);
    assert!(!blinding_factors[0].is_empty());
    
    // Process the transaction
    let result = processor.process_confidential_transaction(&conf_tx);
    
    assert!(result.is_ok(), "Failed to process confidential transaction: {:?}", result);
}

#[test]
fn test_confidential_transaction_with_invalid_amounts() {
    let api = create_testnet_api();
    let mut rng = OsRng;
    
    // Create inputs
    let inputs = vec![TransactionInput::new(
        [1u8; 32],
        0,
        vec![],
        0xffffffff,
    )];
    
    // Test with zero amount
    let zero_outputs = vec![
        (0, vec![]), // Zero NOVA - should be rejected
    ];
    
    let zero_result = api.create_confidential_transaction(inputs.clone(), zero_outputs, &mut rng);
    assert!(zero_result.is_err(), "Should reject zero amount outputs");
    
    // Test with very large amount
    let large_outputs = vec![
        (u64::MAX, vec![]), // Maximum possible value - should be rejected
    ];
    
    let large_result = api.create_confidential_transaction(inputs.clone(), large_outputs, &mut rng);
    assert!(large_result.is_err(), "Should reject extremely large amounts");
}

#[test]
fn test_disabled_features() {
    // Create a config with features disabled
    let config = Config::default();
    assert!(!config.crypto.quantum.enabled);
    assert!(!config.crypto.zkp.enabled);
    
    // Create an API with features disabled
    let api = CryptoAPI::new(config);
    
    // Try to generate a quantum key pair
    let mut rng = OsRng;
    let keypair_result = api.generate_quantum_keypair(&mut rng);
    
    // This should fail because quantum features are disabled
    assert!(keypair_result.is_err());
    
    // Create a transaction
    let inputs = vec![TransactionInput::new(
        [1u8; 32],
        0,
        vec![],
        0xffffffff,
    )];
    
    let outputs = vec![
        (90_000_000, vec![]), // 0.9 NOVA
    ];
    
    // Try to create a confidential transaction
    let conf_tx_result = api.create_confidential_transaction(inputs, outputs, &mut rng);
    
    // This should fail because ZKP features are disabled
    assert!(conf_tx_result.is_err());
} 