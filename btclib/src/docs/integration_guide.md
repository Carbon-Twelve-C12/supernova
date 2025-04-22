# Integration Guide: Quantum Signatures and Confidential Transactions

This guide demonstrates how to integrate quantum-resistant cryptography and zero-knowledge proofs into your blockchain applications using the SuperNova library.

## Setting Up Your Environment

First, add the required dependencies to your `Cargo.toml` file:

```toml
[dependencies]
btclib = { path = "../path/to/btclib" }
rand = "0.8.5"
```

## Basic Setup

Start by initializing the cryptographic API with your desired configuration:

```rust
use btclib::api::{CryptoAPI, create_testnet_api};
use btclib::config::Config;
use rand::rngs::OsRng;

fn main() {
    // Create an API with testnet configuration (quantum and ZKP features enabled)
    let api = create_testnet_api();
    
    // Alternatively, create a custom configuration
    let custom_config = Config::default();
    let custom_api = CryptoAPI::new(custom_config);
    
    // Initialize a secure random number generator
    let mut rng = OsRng;
    
    // Now you can use these objects to interact with the advanced crypto features
}
```

## Working with Quantum-Resistant Signatures

### Generating Keys

```rust
use btclib::api::create_testnet_api;
use btclib::crypto::quantum::QuantumScheme;
use rand::rngs::OsRng;

fn generate_quantum_keys() {
    let api = create_testnet_api();
    let mut rng = OsRng;
    
    // Generate a quantum-resistant key pair using the default scheme (Dilithium)
    let keypair = api.generate_quantum_keypair(&mut rng).expect("Failed to generate keypair");
    
    println!("Generated quantum keypair:");
    println!("  Scheme: {:?}", keypair.parameters.scheme);
    println!("  Security level: {}", keypair.parameters.security_level);
    println!("  Public key size: {} bytes", keypair.public_key.len());
    
    // You can save the public key for later verification
    let public_key = keypair.public_key.clone();
    
    // Note: In a real application, you would securely store the private key
}
```

### Signing Transactions

```rust
use btclib::api::create_testnet_api;
use btclib::types::transaction::{Transaction, TransactionInput, TransactionOutput};
use rand::rngs::OsRng;

fn sign_transaction() {
    let api = create_testnet_api();
    let mut rng = OsRng;
    
    // Generate a key pair
    let keypair = api.generate_quantum_keypair(&mut rng).expect("Failed to generate keypair");
    
    // Create a simple transaction
    let tx = Transaction::new(
        1, // version
        vec![], // inputs
        vec![], // outputs
        0, // locktime
    );
    
    // Sign the transaction with the quantum key
    let signed_tx = api.sign_quantum_transaction(&tx, &keypair)
        .expect("Failed to sign transaction");
    
    // The signed transaction can now be broadcast to the network
    println!("Transaction signed with quantum-resistant signature");
}
```

### Handling Errors for Unsupported Schemes

> **Note:** Currently, only the Dilithium scheme is fully implemented. When using other schemes (Falcon, SPHINCS+, and Hybrid), you need to handle the `CryptoOperationFailed` error appropriately.

```rust
use btclib::api::create_testnet_api;
use btclib::crypto::quantum::{QuantumScheme, QuantumError, QuantumParameters};
use btclib::types::transaction::Transaction;
use rand::rngs::OsRng;

fn handle_quantum_errors() {
    let api = create_testnet_api();
    let mut rng = OsRng;
    
    // Try to use Falcon scheme (not fully implemented yet)
    let params = QuantumParameters {
        security_level: 3,
        scheme: QuantumScheme::Falcon,
        use_compression: false,
    };
    
    // Generate key pair
    let falcon_keypair = api.generate_quantum_keypair_with_params(&mut rng, params)
        .expect("Failed to generate keypair");
    
    // Create a transaction
    let tx = Transaction::new(1, vec![], vec![], 0);
    
    // Attempt to sign the transaction
    match api.sign_quantum_transaction(&tx, &falcon_keypair) {
        Ok(signed_tx) => {
            println!("Transaction signed successfully (unexpected)");
        },
        Err(QuantumError::CryptoOperationFailed(msg)) => {
            println!("Expected error: {}", msg);
            // In production code, you might:
            // 1. Fall back to Dilithium scheme
            // 2. Log the error
            // 3. Notify the user about feature limitations
        },
        Err(e) => {
            println!("Unexpected error: {}", e);
        }
    }
}
```

### Verifying Signatures

```rust
use btclib::api::create_testnet_api;
use btclib::types::extended_transaction::QuantumTransaction;
use rand::rngs::OsRng;

fn verify_signature() {
    let api = create_testnet_api();
    
    // Assume we received a transaction with a quantum signature
    let received_tx: QuantumTransaction = receive_transaction();
    
    // Assume we have the sender's public key
    let sender_public_key = get_sender_public_key();
    
    // Verify the signature
    match received_tx.verify_signature(&sender_public_key) {
        Ok(true) => {
            println!("Signature is valid");
            // Process the transaction
        },
        Ok(false) => {
            println!("Invalid signature - reject transaction");
        },
        Err(QuantumError::CryptoOperationFailed(msg)) => {
            println!("Unsupported scheme: {}", msg);
            // Handle the unsupported scheme appropriately
        },
        Err(e) => {
            println!("Verification error: {}", e);
        }
    }
}
```

## Working with Confidential Transactions

### Creating Confidential Transactions

```rust
use btclib::api::create_testnet_api;
use btclib::types::transaction::TransactionInput;
use rand::rngs::OsRng;

fn create_confidential_transaction() {
    let api = create_testnet_api();
    let mut rng = OsRng;
    
    // Create a transaction with inputs and outputs
    let inputs = vec![TransactionInput::new(
        [1u8; 32], // Previous transaction hash
        0,         // Output index
        vec![],    // Signature script
        0xffffffff, // Sequence
    )];
    
    // Define outputs as (amount, pubkey script) pairs
    let outputs = vec![
        (50_000_000, vec![]), // Output 1: 0.5 NOVA
        (40_000_000, vec![]), // Output 2: 0.4 NOVA
    ];
    
    // Create a confidential transaction that hides the amounts
    // This returns both the transaction and the blinding factors
    let (conf_tx, blinding_factors) = api.create_confidential_transaction(inputs, outputs, &mut rng)
        .expect("Failed to create confidential transaction");
    
    println!("Created confidential transaction:");
    println!("  Number of outputs: {}", conf_tx.conf_outputs().len());
    
    // IMPORTANT: The blinding factors are critical secrets!
    // You must securely store them to later spend these outputs
    println!("  Received {} blinding factors to store securely", blinding_factors.len());
    
    // In a real wallet, you would securely store the binding factors along with
    // the transaction information, possibly encrypted:
    for (i, bf) in blinding_factors.iter().enumerate() {
        println!("  Output {}: Blinding factor length: {} bytes", i, bf.len());
        // store_securely(output_id, bf)
    }
    
    // The conf_tx can now be broadcast to the network
    // The amounts will be hidden, but the transaction can still be validated
}
```

> ⚠️ **CRITICAL SECURITY WARNING**: Blinding factors must be stored securely. Loss of a blinding 
> factor will make the corresponding output unspendable, resulting in permanent loss of funds.
> Treat blinding factors with the same level of security as private keys.

### Securely Storing Blinding Factors

For secure storage of blinding factors, consider the following approach:

```rust
use btclib::api::create_testnet_api;
use serde::{Serialize, Deserialize};
use std::collections::HashMap;
use std::fs::File;
use std::io::{Write, Read};

// Structure to store confidential transaction data
#[derive(Serialize, Deserialize)]
struct ConfidentialTransactionData {
    tx_id: String,
    outputs: Vec<ConfidentialOutputData>,
}

#[derive(Serialize, Deserialize)]
struct ConfidentialOutputData {
    output_index: usize,
    amount: u64,
    blinding_factor: Vec<u8>,
    script: Vec<u8>,
}

fn store_transaction_data(tx_id: &str, amounts: &[u64], blinding_factors: &[Vec<u8>], scripts: &[Vec<u8>]) {
    let mut outputs = Vec::new();
    
    for i in 0..amounts.len() {
        outputs.push(ConfidentialOutputData {
            output_index: i,
            amount: amounts[i],
            blinding_factor: blinding_factors[i].clone(),
            script: scripts[i].clone(),
        });
    }
    
    let tx_data = ConfidentialTransactionData {
        tx_id: tx_id.to_string(),
        outputs,
    };
    
    // In a real implementation, encrypt this data before writing to disk
    // using a password-derived key or hardware security module
    let serialized = serde_json::to_string(&tx_data).expect("Failed to serialize");
    
    // Write to secure storage
    let mut file = File::create(format!("{}.encrypted", tx_id)).expect("Failed to create file");
    file.write_all(serialized.as_bytes()).expect("Failed to write");
    
    println!("Securely stored transaction data for {}", tx_id);
}
```

### Verifying Confidential Transactions

```rust
use btclib::api::create_testnet_api;
use btclib::types::extended_transaction::ConfidentialTransaction;

fn verify_confidential_transaction(conf_tx: &ConfidentialTransaction) {
    let api = create_testnet_api();
    
    // Verify the confidential transaction
    let verification_result = api.verify_confidential_transaction(conf_tx)
        .expect("Failed to verify transaction");
    
    if verification_result {
        println!("Confidential transaction verified successfully!");
        println!("All range proofs are valid, meaning:");
        println!("  1. All amounts are positive");
        println!("  2. No integer overflow is possible");
    } else {
        println!("Confidential transaction verification failed!");
    }
}
```

## Advanced Usage: Custom Commitments and Proofs

For more advanced use cases, you can work directly with commitments and proofs:

```rust
use btclib::api::create_testnet_api;
use rand::rngs::OsRng;

fn work_with_commitments_and_proofs() {
    let api = create_testnet_api();
    let mut rng = OsRng;
    
    // Create a commitment to a value (e.g., an amount)
    let value = 1_000_000; // 0.01 NOVA
    let (commitment, blinding_factor) = api.commit_to_value(value, &mut rng);
    
    println!("Created commitment to the value {}", value);
    println!("  Commitment: {} bytes", commitment.value.len());
    println!("  Blinding factor: {} bytes", blinding_factor.len());
    
    // Create a range proof that proves the value is positive without revealing it
    let range_proof = api.create_range_proof(value, &blinding_factor, 64, &mut rng);
    
    println!("Created range proof:");
    println!("  Proof type: {:?}", range_proof.proof_type);
    println!("  Proof size: {} bytes", range_proof.proof.len());
    
    // These primitives can be used to build more complex protocols
}
```

## Integrating with Transaction Processing

If you're building a full node or wallet application, you can use the transaction processor:

```rust
use std::sync::Arc;
use btclib::transaction_processor::TransactionProcessor;
use btclib::types::extended_transaction::{QuantumTransaction, ConfidentialTransaction};
use dashmap::DashMap;

fn process_transactions(
    quantum_tx: &QuantumTransaction,
    conf_tx: &ConfidentialTransaction,
    utxo_set: Arc<DashMap<([u8; 32], u32), TransactionOutput>>
) {
    // Create a transaction processor with quantum and confidential features enabled
    let processor = TransactionProcessor::new(utxo_set, true, true);
    
    // Process a quantum transaction
    match processor.process_quantum_transaction(quantum_tx) {
        Ok(_) => println!("Quantum transaction is valid!"),
        Err(e) => println!("Quantum transaction is invalid: {:?}", e),
    }
    
    // Process a confidential transaction
    match processor.process_confidential_transaction(conf_tx) {
        Ok(_) => println!("Confidential transaction is valid!"),
        Err(e) => println!("Confidential transaction is invalid: {:?}", e),
    }
}
```

## Configuration Management

For deployment in different environments, you can configure the features:

```rust
use btclib::config::{Config, NetworkType};
use btclib::api::CryptoAPI;
use btclib::crypto::quantum::QuantumScheme;
use btclib::crypto::zkp::ZkpType;

fn configure_features() {
    // Create a custom configuration
    let mut config = Config::default();
    
    // Configure network
    config.network = NetworkType::Mainnet;
    
    // Configure quantum features
    config.crypto.quantum.enabled = true;
    config.crypto.quantum.default_scheme = QuantumScheme::Dilithium;
    config.crypto.quantum.security_level = 5; // Highest security level
    
    // Configure ZKP features
    config.crypto.zkp.enabled = true;
    config.crypto.zkp.default_scheme = ZkpType::Bulletproof;
    config.crypto.zkp.security_level = 128;
    
    // Save the configuration
    config.save_to_file("config.toml").expect("Failed to save configuration");
    
    // Later, load the configuration
    let loaded_config = Config::load_from_file("config.toml").expect("Failed to load configuration");
    
    // Create an API with the loaded configuration
    let api = CryptoAPI::new(loaded_config);
}
```

## Performance Considerations

When implementing quantum-resistant signatures and zero-knowledge proofs, keep these performance considerations in mind:

1. **Signature sizes**: Quantum-resistant signatures are larger than classical signatures
   - Dilithium: ~2-3 KB 
   - Falcon: ~1-2 KB
   - SPHINCS+: ~8-30 KB

2. **Proof sizes**: Range proofs add overhead to transactions
   - Bulletproofs: ~700 bytes per output (logarithmic in the range size)

3. **Verification time**: Quantum signature and range proof verification is more CPU-intensive
   - Consider using batch verification when processing multiple signatures/proofs

4. **Generation time**: Creating range proofs is more expensive than normal transaction signing
   - Optimize by generating proofs in parallel when possible

5. **Storage requirements**: Confidential transaction data and quantum signatures require more storage

For optimal performance, consider enabling these features only when needed and using the most appropriate scheme for your security/performance requirements.

## Security Guidelines

1. **Key management**: Safeguard quantum private keys with the same care as classical keys
2. **Blinding factors**: Securely store blinding factors for your confidential transactions
3. **Security levels**: Choose appropriate security levels based on your threat model
4. **Hybrid schemes**: Consider using hybrid schemes for transition periods
5. **Testing**: Thoroughly test the cryptographic features in a controlled environment before deployment

## Implementation Status

> **Important:** Not all quantum signature schemes are fully implemented in the current version:

1. **Fully Implemented:**
   - Dilithium signature scheme (all security levels)

2. **Planned for Future Releases:**
   - Falcon signature scheme
   - SPHINCS+ signature scheme
   - Hybrid schemes (combining classical and quantum algorithms)

When attempting to use non-Dilithium schemes, your code must properly handle `QuantumError::CryptoOperationFailed` errors that will be returned by the `sign` and `verify` methods.

## Further Resources

- [SuperNova Documentation](https://supernova.docs)  
- [Quantum Cryptography Tutorial](https://quantum.tutorial)
- [Zero-Knowledge Proofs Explained](https://zkp.explained)
- [Post-Quantum Security Standards](https://pqsecurity.standards) 