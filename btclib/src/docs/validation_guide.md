# Transaction Validation Guide

The SuperNova blockchain includes a comprehensive validation system for both standard and advanced transactions. This guide explains how to use the validation service for transaction validation and security auditing.

## Overview

The validation service provides:

1. Standard transaction validation
2. Quantum-resistant signature validation
3. Zero-knowledge proof validation for confidential transactions
4. Security scoring and reporting
5. Performance metrics

## Getting Started

To use the validation service, first create an instance with your desired configuration and security level:

```rust
use btclib::config::Config;
use btclib::validation::{ValidationService, SecurityLevel};

// Create a configuration
let config = Config::testnet(); // or your custom configuration

// Create a validation service with enhanced security
let validation_service = ValidationService::new(
    config,
    SecurityLevel::Enhanced // Choose from Standard, Enhanced, or Maximum
);
```

## Security Levels

The validation service supports three security levels:

1. **Standard**: Basic validation with minimal overhead
2. **Enhanced**: Additional checks for better security guarantees
3. **Maximum**: Thorough validation with highest security standards

## Validating Standard Transactions

```rust
use btclib::types::transaction::Transaction;

// Get or create a transaction
let transaction: Transaction = get_transaction();

// Validate the transaction
match validation_service.validate_transaction(&transaction) {
    Ok(result) => {
        if result.is_valid {
            println!("Transaction is valid with security score: {}", result.security_score);
        } else {
            println!("Transaction validation issues:");
            for issue in &result.issues {
                println!("  - {}", issue);
            }
        }
        
        // Performance metrics
        println!("Validation time: {}ms", result.metrics.validation_time_ms);
        println!("Transaction size: {} bytes", result.metrics.transaction_size);
    },
    Err(err) => {
        println!("Validation error: {}", err);
    }
}
```

## Validating Quantum-Resistant Transactions

```rust
use btclib::types::extended_transaction::QuantumTransaction;

// Get or create a quantum transaction
let quantum_tx: QuantumTransaction = get_quantum_transaction();

// Get the relevant public key
let public_key: Vec<u8> = get_public_key();

// Validate the quantum transaction
match validation_service.validate_quantum_transaction(&quantum_tx, &public_key) {
    Ok(result) => {
        if result.is_valid {
            println!("Quantum transaction is valid with security score: {}", result.security_score);
            println!("Signature scheme: {:?}", quantum_tx.scheme());
            println!("Security level: {}", quantum_tx.security_level());
        } else {
            println!("Quantum transaction validation issues:");
            for issue in &result.issues {
                println!("  - {}", issue);
            }
        }
    },
    Err(err) => {
        println!("Validation error: {}", err);
    }
}
```

## Validating Confidential Transactions

```rust
use btclib::types::extended_transaction::ConfidentialTransaction;

// Get or create a confidential transaction
let conf_tx: ConfidentialTransaction = get_confidential_transaction();

// Validate the confidential transaction
match validation_service.validate_confidential_transaction(&conf_tx) {
    Ok(result) => {
        if result.is_valid {
            println!("Confidential transaction is valid with security score: {}", result.security_score);
            println!("Number of outputs: {}", conf_tx.conf_outputs().len());
        } else {
            println!("Confidential transaction validation issues:");
            for issue in &result.issues {
                println!("  - {}", issue);
            }
        }
        
        // Performance metrics are particularly important for ZKP operations
        println!("Verification operations: {}", result.metrics.verification_ops);
        println!("Validation time: {}ms", result.metrics.validation_time_ms);
    },
    Err(err) => {
        println!("Validation error: {}", err);
    }
}
```

## Security Scoring

The validation service assigns a security score (0-100) to each transaction:

- **90-100**: Excellent - Transaction meets all security standards
- **70-89**: Good - Transaction is secure but has minor issues
- **50-69**: Fair - Transaction has multiple issues but is still valid
- **0-49**: Poor - Transaction has serious security concerns

You can use this scoring system to implement policies:

```rust
// Example policy implementation
fn process_transaction(validation_result: &ValidationResult, tx_value: u64) -> bool {
    match validation_result.security_score {
        90..=100 => true, // Accept all transactions
        70..=89 => tx_value < 1_000_000_000, // Only allow < 10 BTC for good security
        50..=69 => tx_value < 100_000_000, // Only allow < 1 BTC for fair security
        _ => false, // Reject transactions with poor security
    }
}
```

## Advanced Validation Examples

### Validating Hybrid Quantum Signatures

```rust
// For hybrid signatures, check the scheme type first
if let QuantumScheme::Hybrid(classical_scheme) = quantum_tx.scheme() {
    println!("Using hybrid scheme with: {:?}", classical_scheme);
    
    // Validate with maximum security for hybrid schemes
    let max_validation_service = ValidationService::new(
        config,
        SecurityLevel::Maximum
    );
    
    let result = max_validation_service.validate_quantum_transaction(
        &quantum_tx,
        &public_key
    )?;
    
    // Hybrid schemes should score higher
    assert!(result.security_score >= 90, "Hybrid schemes should have high security");
}
```

### Batch Transaction Validation

```rust
// For validating multiple transactions
fn validate_batch(
    transactions: &[Transaction],
    validation_service: &ValidationService
) -> Vec<ValidationResult> {
    // For production use, could use parallel processing
    transactions.iter()
        .map(|tx| validation_service.validate_transaction(tx).unwrap_or_else(|e| {
            // Create a failed result for errors
            ValidationResult {
                is_valid: false,
                issues: vec![format!("Validation error: {}", e)],
                security_score: 0,
                metrics: Default::default(),
            }
        }))
        .collect()
}
```

## Customizing the Validator

To create a custom validator with special validation rules:

```rust
use btclib::validation::{ValidationService, SecurityLevel, ValidationResult};
use btclib::config::Config;

struct EnhancedValidator {
    inner: ValidationService,
    custom_rules: Vec<Box<dyn Fn(&Transaction) -> Option<String>>>,
}

impl EnhancedValidator {
    fn new(config: Config) -> Self {
        Self {
            inner: ValidationService::new(config, SecurityLevel::Enhanced),
            custom_rules: Vec::new(),
        }
    }
    
    fn add_rule<F>(&mut self, rule: F)
    where
        F: Fn(&Transaction) -> Option<String> + 'static,
    {
        self.custom_rules.push(Box::new(rule));
    }
    
    fn validate(&self, tx: &Transaction) -> ValidationResult {
        // First use the standard validator
        let mut result = self.inner.validate_transaction(tx)
            .unwrap_or_else(|_| ValidationResult {
                is_valid: false,
                issues: vec!["Standard validation failed".to_string()],
                security_score: 0,
                metrics: Default::default(),
            });
        
        // Then apply custom rules
        for rule in &self.custom_rules {
            if let Some(issue) = rule(tx) {
                result.issues.push(issue);
                result.security_score = result.security_score.saturating_sub(5);
            }
        }
        
        // Update validity based on custom rules
        result.is_valid = result.is_valid && result.security_score >= 50;
        
        result
    }
}
```

## Security Best Practices

1. **Always use Enhanced or Maximum security** for high-value transactions
2. **Verify transactions in a controlled environment** before broadcasting to the network
3. **Implement minimum score thresholds** based on transaction value
4. **Log validation results** for security auditing
5. **Monitor validation metrics** to detect anomalies

## Performance Considerations

The validation service includes performance metrics that can help you monitor:

1. **Validation time**: For identifying slow operations
2. **Transaction size**: For resource allocation
3. **Verification operations**: For tracking computational complexity

These metrics are particularly important for zero-knowledge proofs and quantum signatures, which can be more computationally intensive than classical cryptography.

## When to Use Maximum Security

The Maximum security level performs additional checks that may impact performance. Consider using it for:

1. High-value transactions
2. Critical system operations
3. Transactions with unusual characteristics
4. Security auditing

For standard user transactions or high-throughput scenarios, Enhanced security provides a good balance of performance and safety. 