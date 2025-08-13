//! Test utilities module for btclib
//! 
//! ⚠️ WARNING: This module is FOR TESTING ONLY ⚠️
//! 
//! This module contains mock implementations and test helpers that bypass critical
//! security checks. It MUST NEVER be used in production code. All items in this
//! module are gated behind #[cfg(test)] to prevent accidental production use.
//!
//! # Security Considerations
//! 
//! The utilities in this module intentionally weaken or bypass security mechanisms
//! to facilitate testing. This includes:
//! - Mock signature verification that always returns true
//! - Transaction builders that create unsigned transactions
//! - Simplified cryptographic operations
//! 
//! # Architecture
//! 
//! Following the principle of defense in depth, this module is structured to:
//! 1. Clearly mark all functions as test-only
//! 2. Use Result types for all operations to maintain error handling patterns
//! 3. Provide builders that follow Rust's ownership model
//! 4. Document security implications for each utility

#![cfg(test)]

use crate::crypto::signature::{Signature, SignatureVerifier, SignatureType};
use crate::types::transaction::{Transaction, TransactionInput, TransactionOutput};
use crate::types::block::{Block, BlockHeader};
use crate::error::SupernovaError;
use std::time::{SystemTime, UNIX_EPOCH};
use std::cmp::min;

/// Mock signature verification function for testing purposes only
/// 
/// ⚠️ SECURITY WARNING: This implementation ALWAYS returns true
/// and should NEVER be used in production code.
pub fn mock_signature_verify(
    _message: &[u8], 
    _signature: &[u8], 
    _public_key: &[u8]
) -> bool {
    // Always return true for testing
    true
}

/// Create a mock signature verifier that always returns true
/// 
/// ⚠️ SECURITY WARNING: This bypasses all cryptographic verification
pub fn create_mock_verifier() -> impl Fn(&[u8; 32], u32) -> Option<TransactionOutput> {
    |_tx_hash: &[u8; 32], _index: u32| -> Option<TransactionOutput> {
        // Return a mock UTXO for testing
        Some(TransactionOutput::new(100_000_000, vec![1, 2, 3, 4]))
    }
}

/// Builder for creating test transactions with proper error handling
/// 
/// This builder follows Rust's ownership model and builder pattern
/// to create transactions suitable for testing consensus mechanisms.
pub struct TestTransactionBuilder {
    version: u32,
    inputs: Vec<TransactionInput>,
    outputs: Vec<TransactionOutput>,
    lock_time: u32,
}

impl TestTransactionBuilder {
    /// Create a new test transaction builder
    pub fn new() -> Self {
        Self {
            version: 1,
            inputs: Vec::new(),
            outputs: Vec::new(),
            lock_time: 0,
        }
    }

    /// Set transaction version
    pub fn version(mut self, version: u32) -> Self {
        self.version = version;
        self
    }

    /// Add a coinbase input (doesn't require signatures)
    /// 
    /// # Security Note
    /// Coinbase transactions bypass signature verification, making them
    /// ideal for testing consensus logic without cryptographic complexity.
    pub fn add_coinbase_input(mut self, script_data: Vec<u8>) -> Self {
        let input = TransactionInput::new_coinbase(script_data);
        self.inputs.push(input);
        self
    }

    /// Add a regular input (for testing double-spend scenarios)
    pub fn add_input(mut self, prev_tx_hash: [u8; 32], prev_output_index: u32) -> Self {
        let input = TransactionInput::new(
            prev_tx_hash,
            prev_output_index,
            vec![], // Empty signature script for testing
            0xffffffff,
        );
        self.inputs.push(input);
        self
    }

    /// Add an output
    pub fn add_output(mut self, amount: u64, script_pubkey: Vec<u8>) -> Self {
        let output = TransactionOutput::new(amount, script_pubkey);
        self.outputs.push(output);
        self
    }

    /// Set lock time
    pub fn lock_time(mut self, lock_time: u32) -> Self {
        self.lock_time = lock_time;
        self
    }

    /// Build the transaction
    /// 
    /// # Returns
    /// Result<Transaction, SupernovaError> following proper error handling patterns
    pub fn build(self) -> Result<Transaction, SupernovaError> {
        use crate::error::TransactionError;
        
        if self.inputs.is_empty() {
            return Err(SupernovaError::Transaction(
                TransactionError::Invalid("Transaction must have at least one input".to_string())
            ));
        }
        
        if self.outputs.is_empty() {
            return Err(SupernovaError::Transaction(
                TransactionError::Invalid("Transaction must have at least one output".to_string())
            ));
        }

        Ok(Transaction::new(
            self.version,
            self.inputs,
            self.outputs,
            self.lock_time,
        ))
    }
}

/// Builder for creating test blocks with proper structure
/// 
/// This builder ensures blocks are created with valid structure
/// for testing consensus mechanisms.
pub struct TestBlockBuilder {
    version: u32,
    prev_block_hash: [u8; 32],
    transactions: Vec<Transaction>,
    target: u32,
    nonce: u32,
    timestamp: u64,
}

impl TestBlockBuilder {
    /// Create a new test block builder
    pub fn new() -> Self {
        Self {
            version: 1,
            prev_block_hash: [0u8; 32],
            transactions: Vec::new(),
            target: 0x1d00ffff, // Default difficulty
            nonce: 0,
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0),
        }
    }

    /// Set block version
    pub fn version(mut self, version: u32) -> Self {
        self.version = version;
        self
    }

    /// Set previous block hash
    pub fn prev_block_hash(mut self, hash: [u8; 32]) -> Self {
        self.prev_block_hash = hash;
        self
    }

    /// Add a transaction to the block
    pub fn add_transaction(mut self, tx: Transaction) -> Self {
        self.transactions.push(tx);
        self
    }

    /// Set difficulty target
    pub fn target(mut self, target: u32) -> Self {
        self.target = target;
        self
    }

    /// Set nonce (for pre-mined blocks in tests)
    pub fn nonce(mut self, nonce: u32) -> Self {
        self.nonce = nonce;
        self
    }

    /// Set timestamp
    pub fn timestamp(mut self, timestamp: u64) -> Self {
        self.timestamp = timestamp;
        self
    }

    /// Build the block with a coinbase transaction if needed
    /// 
    /// # Security Note
    /// This automatically adds a coinbase transaction if no transactions
    /// are present, ensuring block validity for consensus testing.
    pub fn build(mut self) -> Result<Block, SupernovaError> {
        // Ensure block has at least a coinbase transaction
        if self.transactions.is_empty() {
            let coinbase = TestTransactionBuilder::new()
                .add_coinbase_input(vec![1, 2, 3])
                .add_output(50_000_000_000, vec![1, 2, 3, 4])
                .build()?;
            self.transactions.push(coinbase);
        }

        // Ensure first transaction is coinbase
        if !self.transactions[0].is_coinbase() {
            use crate::error::TransactionError;
            return Err(SupernovaError::Transaction(
                TransactionError::Invalid("First transaction must be coinbase".to_string())
            ));
        }

        let mut block = Block::new_with_params(
            self.version,
            self.prev_block_hash,
            self.transactions,
            self.target,
        );

        // Set specific timestamp and nonce if provided
        block.header.timestamp = self.timestamp;
        block.header.nonce = self.nonce;

        Ok(block)
    }

    /// Build and mine the block (for tests requiring valid PoW)
    /// 
    /// ⚠️ WARNING: This can be computationally expensive for low targets
    pub fn build_and_mine(self) -> Result<Block, SupernovaError> {
        let mut block = self.build()?;
        
        // Mine the block (find valid nonce)
        while !block.header.meets_target() {
            block.header.increment_nonce();
            
            // Prevent infinite loop in tests
            if block.header.nonce > 1_000_000 {
                use crate::error::MiningError;
                return Err(SupernovaError::Mining(
                    MiningError::NonceExhausted
                ));
            }
        }

        Ok(block)
    }
}

/// Helper function to create a mock UTXO lookup function for testing
/// 
/// This creates a closure that returns predetermined outputs for specific
/// transaction hashes, useful for testing transaction validation.
pub fn create_mock_utxo_lookup(
    utxos: Vec<([u8; 32], u32, TransactionOutput)>
) -> impl Fn(&[u8; 32], u32) -> Option<TransactionOutput> {
    move |tx_hash: &[u8; 32], index: u32| {
        utxos.iter()
            .find(|(hash, idx, _)| hash == tx_hash && *idx == index)
            .map(|(_, _, output)| output.clone())
    }
}

/// Create a simple chain of blocks for testing consensus
/// 
/// # Security Note
/// This creates blocks with minimal PoW for testing efficiency.
/// Real consensus tests should verify behavior with proper difficulty.
pub fn create_test_chain(length: usize) -> Result<Vec<Block>, SupernovaError> {
    let mut chain = Vec::with_capacity(length);
    let mut prev_hash = [0u8; 32]; // Genesis

    for i in 0..length {
        let block = TestBlockBuilder::new()
            .prev_block_hash(prev_hash)
            .timestamp(1_600_000_000 + (i as u64 * 600)) // 10 minutes apart
            .target(0x207fffff) // Very easy difficulty for testing
            .build_and_mine()?;

        prev_hash = block.hash();
        chain.push(block);
    }

    Ok(chain)
}

/// Utilities for testing difficulty adjustments
pub mod difficulty {
    use super::*;
    
    /// Create a block with specific timestamp for difficulty testing
    pub fn create_block_with_timestamp(
        prev_hash: [u8; 32],
        timestamp: u64,
        target: u32,
    ) -> Result<Block, SupernovaError> {
        TestBlockBuilder::new()
            .prev_block_hash(prev_hash)
            .timestamp(timestamp)
            .target(target)
            .build()
    }
}

/// Utilities for testing fork scenarios
pub mod fork {
    use super::*;
    
    /// Create two competing chains from a common ancestor
    pub fn create_fork_scenario(
        common_length: usize,
        fork1_length: usize,
        fork2_length: usize,
    ) -> Result<(Vec<Block>, Vec<Block>, Vec<Block>), SupernovaError> {
        // Create common chain
        let common = create_test_chain(common_length)?;
        
        // Create fork 1
        let mut fork1 = common.clone();
        let mut prev_hash = common.last()
            .map(|b| b.hash())
            .unwrap_or([0u8; 32]);
            
        for i in 0..fork1_length {
            let block = TestBlockBuilder::new()
                .prev_block_hash(prev_hash)
                .timestamp(1_600_000_000 + ((common_length + i) as u64 * 600))
                .build_and_mine()?;
            prev_hash = block.hash();
            fork1.push(block);
        }
        
        // Create fork 2 (with slightly different transactions)
        let mut fork2 = common.clone();
        prev_hash = common.last()
            .map(|b| b.hash())
            .unwrap_or([0u8; 32]);
            
        for i in 0..fork2_length {
            let block = TestBlockBuilder::new()
                .prev_block_hash(prev_hash)
                .timestamp(1_600_000_000 + ((common_length + i) as u64 * 600))
                .add_transaction(
                    TestTransactionBuilder::new()
                        .add_coinbase_input(vec![99, 99, 99]) // Different coinbase
                        .add_output(50_000_000_000, vec![99, 99, 99, 99])
                        .build()?
                )
                .build_and_mine()?;
            prev_hash = block.hash();
            fork2.push(block);
        }
        
        Ok((common, fork1, fork2))
    }
}

/// Quantum cryptography test utilities
/// 
/// ⚠️ SECURITY WARNING: These implementations are FOR TESTING ONLY ⚠️
/// They provide deterministic, insecure mocks of quantum signatures.
/// NEVER use these in production code.
pub mod quantum {
    use super::*;
    use crate::crypto::quantum::{QuantumKeyPair, QuantumParameters, QuantumScheme, QuantumError};
    use crate::validation::SecurityLevel;
    
    /// Mock quantum key pair for testing
    /// 
    /// ⚠️ WARNING: This generates INSECURE, DETERMINISTIC keys
    pub struct MockQuantumKeyPair;
    
    impl MockQuantumKeyPair {
        /// Generate a mock key pair with predictable sizes
        /// 
        /// This ensures tests don't fail due to key size mismatches
        pub fn generate(scheme: QuantumScheme, security_level: u8) -> Result<QuantumKeyPair, QuantumError> {
            let (pk_size, sk_size) = match scheme {
                QuantumScheme::Dilithium => {
                    match security_level {
                        1 => (1312, 2528),    // Dilithium2 (Low)
                        3 => (1952, 4000),    // Dilithium3 (Medium)
                        5 => (2592, 4864),    // Dilithium5 (High)
                        _ => (1952, 4000),    // Default to Dilithium3
                    }
                },
                QuantumScheme::Falcon => {
                    match security_level {
                        1 => (897, 1281),      // Falcon512 (Low)
                        3 => (1793, 2305),     // Falcon1024 (Medium)
                        5 => (1793, 2305),     // Falcon1024 (High)
                        _ => (1793, 2305),     // Default to Falcon1024
                    }
                },
                QuantumScheme::SphincsPlus => {
                    match security_level {
                        1 => (32, 64),         // SPHINCS+-128s (Low)
                        3 => (48, 96),         // SPHINCS+-192s (Medium)
                        5 => (64, 128),        // SPHINCS+-256s (High)
                        _ => (48, 96),         // Default to SPHINCS+-192s
                    }
                },
                QuantumScheme::Hybrid(_) => {
                    // For hybrid, combine classical + quantum sizes
                    let quantum_sizes = match security_level {
                        1 => (1312, 2528),     // Low
                        3 => (1952, 4000),     // Medium
                        5 => (2592, 4864),     // High
                        _ => (1952, 4000),     // Default to Medium
                    };
                    (quantum_sizes.0 + 32, quantum_sizes.1 + 32) // Add Ed25519 sizes
                },
            };
            
            // Generate deterministic but unique keys based on scheme and level
            let seed = format!("{:?}-{}", scheme, security_level);
            let mut public_key = vec![0u8; pk_size];
            let mut secret_key = vec![0u8; sk_size];
            
            // Fill with deterministic data
            for (i, byte) in public_key.iter_mut().enumerate() {
                *byte = ((i + seed.len()) % 256) as u8;
            }
            for (i, byte) in secret_key.iter_mut().enumerate() {
                *byte = ((i * 2 + seed.len()) % 256) as u8;
            }
            
            Ok(QuantumKeyPair {
                public_key,
                secret_key,
                parameters: QuantumParameters { scheme, security_level },
            })
        }
        
        /// Generate a mock signature
        /// 
        /// ⚠️ WARNING: This signature is NOT cryptographically secure
        pub fn mock_sign(
            scheme: QuantumScheme,
            security_level: u8,
            message: &[u8],
        ) -> Vec<u8> {
            let sig_size = match scheme {
                QuantumScheme::Dilithium => {
                    match security_level {
                        1 => 2420,    // Dilithium2 (Low)
                        3 => 3293,    // Dilithium3 (Medium)
                        5 => 4595,    // Dilithium5 (High)
                        _ => 3293,    // Default to Dilithium3
                    }
                },
                QuantumScheme::Falcon => {
                    match security_level {
                        1 => 666,     // Falcon512 (max)
                        3 => 1280,    // Falcon1024 (max)
                        5 => 1280,    // Falcon1024 (max)
                        _ => 1280,    // Default to Falcon1024
                    }
                },
                QuantumScheme::SphincsPlus => {
                    match security_level {
                        1 => 7856,    // SPHINCS+-128s
                        3 => 16224,   // SPHINCS+-192s
                        5 => 29792,   // SPHINCS+-256s
                        _ => 16224,   // Default to SPHINCS+-192s
                    }
                },
                QuantumScheme::Hybrid(_) => {
                    // Combine classical + quantum signature sizes
                    let quantum_size = match security_level {
                        1 => 2420,    // Low
                        3 => 3293,    // Medium
                        5 => 4595,    // High
                        _ => 3293,    // Default to Medium
                    };
                    quantum_size + 64 // Add Ed25519 signature size
                },
            };
            
            // Generate deterministic signature based on message
            let mut signature = vec![0u8; sig_size];
            use sha2::{Sha256, Digest};
            let mut hasher = Sha256::new();
            hasher.update(message);
            let msg_hash = hasher.finalize();
            
            for (i, byte) in signature.iter_mut().enumerate() {
                *byte = msg_hash[i % 32] ^ ((i / 32) as u8);
            }
            
            signature
        }
        
        /// Mock signature verification
        /// 
        /// ⚠️ WARNING: Always returns true for testing
        pub fn mock_verify(
            _public_key: &[u8],
            _message: &[u8],
            _signature: &[u8],
        ) -> bool {
            true
        }
    }
    
    /// Create a test quantum transaction builder
    pub struct TestQuantumTransactionBuilder {
        scheme: QuantumScheme,
        security_level: u8,
    }
    
    impl TestQuantumTransactionBuilder {
        pub fn new(scheme: QuantumScheme, security_level: u8) -> Self {
            Self { scheme, security_level }
        }
        
        /// Build a mock signed transaction
        /// 
        /// ⚠️ WARNING: Uses mock signatures, not secure
        pub fn build_signed_transaction(
            &self,
            inputs: Vec<TransactionInput>,
            outputs: Vec<TransactionOutput>,
        ) -> Result<Transaction, SupernovaError> {
            use crate::error::TransactionError;
            
            if inputs.is_empty() {
                return Err(SupernovaError::Transaction(
                    TransactionError::Invalid("Transaction must have inputs".to_string())
                ));
            }
            
            let tx = Transaction::new(1, inputs, outputs, 0);
            
            // Add mock quantum signature to witness data
            let _mock_sig = MockQuantumKeyPair::mock_sign(
                self.scheme,
                self.security_level,
                &tx.hash(),
            );
            
            // In a real implementation, this would properly set witness data
            // For testing, we just need the signature to be the right size
            
            Ok(tx)
        }
    }
    
    /// Helper to create quantum parameters for testing
    pub fn test_quantum_params(scheme: QuantumScheme, level: SecurityLevel) -> QuantumParameters {
        QuantumParameters {
            scheme,
            security_level: level as u8,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_transaction_builder() {
        let tx = TestTransactionBuilder::new()
            .add_coinbase_input(vec![1, 2, 3])
            .add_output(50_000_000_000, vec![4, 5, 6])
            .build()
            .expect("Failed to build test transaction");

        assert!(tx.is_coinbase());
        assert_eq!(tx.inputs().len(), 1);
        assert_eq!(tx.outputs().len(), 1);
    }

    #[test]
    fn test_block_builder() {
        let block = TestBlockBuilder::new()
            .prev_block_hash([1u8; 32])
            .build()
            .expect("Failed to build test block");

        assert_eq!(block.header.prev_block_hash, [1u8; 32]);
        assert_eq!(block.transactions.len(), 1);
        assert!(block.transactions[0].is_coinbase());
    }

    #[test]
    fn test_chain_creation() {
        let chain = create_test_chain(5)
            .expect("Failed to create test chain");

        assert_eq!(chain.len(), 5);
        
        // Verify chain linkage
        for i in 1..chain.len() {
            assert_eq!(
                chain[i].header.prev_block_hash,
                chain[i-1].hash()
            );
        }
    }
}
