use std::sync::Arc;
use rand::rngs::OsRng;

use btclib::api::create_testnet_api;
use btclib::config::{Config, NetworkType};
use btclib::validation::{ValidationService, SecurityLevel, ValidationError};
use btclib::types::transaction::{Transaction, TransactionInput, TransactionOutput};

#[test]
fn test_standard_transaction_validation() {
    // Create a config for testing
    let config = Config::testnet();
    
    // Create a validation service
    let validation_service = ValidationService::new(config, SecurityLevel::Standard);
    
    // Create a simple valid transaction
    let inputs = vec![TransactionInput::new(
        [1u8; 32],
        0,
        vec![],
        0xffffffff,
    )];
    
    let outputs = vec![TransactionOutput::new(
        90_000_000, // 0.9 NOVA
        vec![],
    )];
    
    let tx = Transaction::new(1, inputs, outputs, 0);
    
    // Validate the transaction
    let result = validation_service.validate_transaction(&tx).expect("Validation failed");
    
    // Check results
    assert!(result.is_valid, "Transaction should be valid");
    assert!(result.issues.is_empty(), "There should be no issues");
    assert_eq!(result.security_score, 100, "Security score should be 100");
    
    // Check metrics
    assert!(result.metrics.validation_time_ms > 0, "Validation time should be measured");
    assert!(result.metrics.transaction_size > 0, "Transaction size should be measured");
    assert_eq!(result.metrics.verification_ops, 1, "Should be one verification operation");
}

#[test]
fn test_invalid_transaction_validation() {
    // Create a config for testing
    let config = Config::testnet();
    
    // Create a validation service
    let validation_service = ValidationService::new(config, SecurityLevel::Enhanced);
    
    // Create a transaction with no inputs
    let inputs = vec![];
    
    let outputs = vec![TransactionOutput::new(
        90_000_000, // 0.9 NOVA
        vec![],
    )];
    
    let tx = Transaction::new(1, inputs, outputs, 0);
    
    // Validate the transaction
    let result = validation_service.validate_transaction(&tx).expect("Validation failed");
    
    // Check results
    assert!(result.is_valid == false, "Transaction should be invalid");
    assert!(!result.issues.is_empty(), "There should be issues");
    assert!(result.security_score < 100, "Security score should be reduced");
}

#[test]
fn test_quantum_transaction_validation() {
    // Create API with quantum features enabled
    let api = create_testnet_api();
    
    // Create a validation service with maximum security
    let validation_service = ValidationService::new(
        Config::testnet(), 
        SecurityLevel::Maximum
    );
    
    // Create a transaction
    let inputs = vec![TransactionInput::new(
        [1u8; 32],
        0,
        vec![],
        0xffffffff,
    )];
    
    let outputs = vec![TransactionOutput::new(
        90_000_000, // 0.9 NOVA
        vec![],
    )];
    
    let tx = Transaction::new(1, inputs, outputs, 0);
    
    // Generate a quantum keypair
    let mut rng = OsRng;
    let keypair = api.generate_quantum_keypair(&mut rng).expect("Failed to generate keypair");
    
    // Sign the transaction
    let quantum_tx = api.sign_transaction_quantum(tx, &keypair).expect("Failed to sign transaction");
    
    // Validate the quantum transaction
    let result = validation_service.validate_quantum_transaction(
        &quantum_tx, 
        &keypair.public_key
    ).expect("Validation failed");
    
    // Check results
    assert!(result.is_valid, "Transaction should be valid");
    
    // Validate using enhanced security
    let enhanced_service = ValidationService::new(
        Config::testnet(), 
        SecurityLevel::Enhanced
    );
    
    let enhanced_result = enhanced_service.validate_quantum_transaction(
        &quantum_tx, 
        &keypair.public_key
    ).expect("Validation failed");
    
    assert!(enhanced_result.is_valid, "Transaction should be valid with enhanced security");
}

#[test]
fn test_confidential_transaction_validation() {
    // Create API with ZKP features enabled
    let api = create_testnet_api();
    
    // Create a validation service
    let validation_service = ValidationService::new(
        Config::testnet(), 
        SecurityLevel::Enhanced
    );
    
    // Create a transaction
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
    let (conf_tx, _) = api.create_confidential_transaction(inputs, outputs, &mut rng)
        .expect("Failed to create confidential transaction");
    
    // Validate the confidential transaction
    let result = validation_service.validate_confidential_transaction(&conf_tx)
        .expect("Validation failed");
    
    // Check results
    assert!(result.is_valid, "Transaction should be valid");
    assert!(result.metrics.verification_ops > 1, "Multiple verification operations expected");
}

#[test]
fn test_validation_with_disabled_features() {
    // Create a config with features disabled
    let mut config = Config::default();
    config.crypto.quantum.enabled = false;
    config.crypto.zkp.enabled = false;
    
    // Create a validation service
    let validation_service = ValidationService::new(
        config, 
        SecurityLevel::Standard
    );
    
    // Create a quantum transaction
    let api = create_testnet_api(); // This has features enabled
    let mut rng = OsRng;
    
    // Generate a keypair and sign a transaction
    let keypair = api.generate_quantum_keypair(&mut rng).expect("Failed to generate keypair");
    
    let tx = Transaction::new(
        1,
        vec![TransactionInput::new([1u8; 32], 0, vec![], 0xffffffff)],
        vec![TransactionOutput::new(90_000_000, vec![])],
        0
    );
    
    let quantum_tx = api.sign_transaction_quantum(tx, &keypair).expect("Failed to sign transaction");
    
    // Try to validate with quantum features disabled
    let result = validation_service.validate_quantum_transaction(&quantum_tx, &keypair.public_key);
    assert!(result.is_err(), "Should error when quantum features are disabled");
    
    match result {
        Err(ValidationError::ConfigError(_)) => {
            // Expected error
        }
        _ => panic!("Unexpected result: {:?}", result),
    }
} 