use dashmap::DashMap;
use std::sync::Arc;
use thiserror::Error;

use crate::crypto::quantum::{QuantumError, QuantumScheme};
use crate::crypto::zkp::ZkpParams;
use crate::types::extended_transaction::{
    ConfidentialTransaction, ConfidentialTransactionBuilder, QuantumTransaction,
    QuantumTransactionBuilder,
};
use crate::types::transaction::{Transaction, TransactionInput, TransactionOutput};

/// Error types for transaction validation and processing
#[derive(Debug, Error)]
pub enum TransactionProcessorError {
    #[error("Invalid transaction: {0}")]
    InvalidTransaction(String),

    #[error("Double spend detected")]
    DoubleSpend,

    #[error("Insufficient funds")]
    InsufficientFunds,

    #[error("Invalid signature: {0}")]
    InvalidSignature(String),

    #[error("Range proof verification failed")]
    InvalidRangeProof,

    #[error("Quantum cryptography error: {0}")]
    QuantumError(#[from] QuantumError),

    #[error("Missing UTXO: {0:?}:{1}")]
    MissingUtxo([u8; 32], u32),
}

/// Type of transaction to process
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransactionType {
    /// Standard transaction
    Standard,

    /// Transaction with quantum-resistant signature
    Quantum(QuantumScheme),

    /// Transaction with confidential amounts
    Confidential,
}

/// Transaction processor for handling different transaction types
pub struct TransactionProcessor {
    /// UTXO set (transaction hash, output index) -> output
    utxo_set: Arc<DashMap<([u8; 32], u32), TransactionOutput>>,

    /// Quantum signature verification parameters
    quantum_verification_enabled: bool,

    /// Confidential transaction verification parameters
    confidential_verification_enabled: bool,
}

impl TransactionProcessor {
    pub fn new(
        utxo_set: Arc<DashMap<([u8; 32], u32), TransactionOutput>>,
        quantum_verification_enabled: bool,
        confidential_verification_enabled: bool,
    ) -> Self {
        Self {
            utxo_set,
            quantum_verification_enabled,
            confidential_verification_enabled,
        }
    }

    /// Process a standard transaction
    pub fn process_transaction(
        &self,
        transaction: &Transaction,
    ) -> Result<(), TransactionProcessorError> {
        // Basic validation
        if transaction.inputs().is_empty() || transaction.outputs().is_empty() {
            return Err(TransactionProcessorError::InvalidTransaction(
                "Transaction must have at least one input and one output".to_string(),
            ));
        }

        // Check for double spends (inputs already spent)
        for input in transaction.inputs() {
            let outpoint = (input.prev_tx_hash(), input.prev_output_index());
            if !self.utxo_set.contains_key(&outpoint) {
                return Err(TransactionProcessorError::MissingUtxo(
                    input.prev_tx_hash(),
                    input.prev_output_index(),
                ));
            }
        }

        // Verify inputs have sufficient funds
        let mut total_input = 0;
        for input in transaction.inputs() {
            let outpoint = (input.prev_tx_hash(), input.prev_output_index());
            if let Some(output) = self.utxo_set.get(&outpoint) {
                total_input += output.amount();
            } else {
                return Err(TransactionProcessorError::MissingUtxo(
                    input.prev_tx_hash(),
                    input.prev_output_index(),
                ));
            }
        }

        let total_output = transaction.total_output().ok_or_else(|| {
            TransactionProcessorError::InvalidTransaction("Output amount overflow".to_string())
        })?;

        if total_input < total_output {
            return Err(TransactionProcessorError::InsufficientFunds);
        }

        // In a real implementation, we would verify the signatures here

        Ok(())
    }

    /// Process a quantum transaction
    pub fn process_quantum_transaction(
        &self,
        transaction: &QuantumTransaction,
    ) -> Result<(), TransactionProcessorError> {
        // Skip quantum verification if disabled
        if !self.quantum_verification_enabled {
            return self.process_transaction(transaction.transaction());
        }

        // First validate the underlying transaction
        self.process_transaction(transaction.transaction())?;

        // Now verify the quantum signature
        // In a real implementation, we would extract the public key from the transaction
        // and verify the signature against it
        let public_key = vec![0u8; 32]; // Placeholder

        match transaction.verify_signature(&public_key) {
            Ok(true) => Ok(()),
            Ok(false) => Err(TransactionProcessorError::InvalidSignature(
                "Quantum signature verification failed".to_string(),
            )),
            Err(e) => Err(TransactionProcessorError::QuantumError(e)),
        }
    }

    /// Process a confidential transaction
    pub fn process_confidential_transaction(
        &self,
        transaction: &ConfidentialTransaction,
    ) -> Result<(), TransactionProcessorError> {
        // Skip confidential verification if disabled
        if !self.confidential_verification_enabled {
            // We can't fallback to standard processing because amounts are hidden
            return Err(TransactionProcessorError::InvalidTransaction(
                "Confidential transactions require confidential verification".to_string(),
            ));
        }

        // Check for double spends (inputs already spent)
        for input in transaction.inputs() {
            let outpoint = (input.prev_tx_hash(), input.prev_output_index());
            if !self.utxo_set.contains_key(&outpoint) {
                return Err(TransactionProcessorError::MissingUtxo(
                    input.prev_tx_hash(),
                    input.prev_output_index(),
                ));
            }
        }

        // Verify range proofs
        if !transaction.verify_range_proofs() {
            return Err(TransactionProcessorError::InvalidRangeProof);
        }

        // In a real implementation, we would verify:
        // 1. That the sum of inputs - outputs = 0 (value conservation)
        // 2. That all range proofs are valid
        // 3. That the transaction is properly signed

        Ok(())
    }
}

/// Builder for creating and processing different transaction types
pub struct TransactionBuilder {
    /// Quantum transaction builder
    quantum_builder: Option<QuantumTransactionBuilder>,

    /// Confidential transaction builder
    confidential_builder: Option<ConfidentialTransactionBuilder>,
}

impl Default for TransactionBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl TransactionBuilder {
    pub fn new() -> Self {
        Self {
            quantum_builder: None,
            confidential_builder: None,
        }
    }

    /// Enable quantum signatures with the specified scheme
    pub fn with_quantum_signatures(mut self, scheme: QuantumScheme, security_level: u8) -> Self {
        self.quantum_builder = Some(QuantumTransactionBuilder::new(scheme, security_level));
        self
    }

    /// Enable confidential transactions with the specified parameters
    pub fn with_confidential_transactions(mut self, zkp_params: ZkpParams) -> Self {
        self.confidential_builder = Some(ConfidentialTransactionBuilder::new(zkp_params));
        self
    }

    /// Create a transaction with the configured features
    pub fn create_transaction<R: rand::CryptoRng + rand::RngCore>(
        &self,
        version: u32,
        inputs: Vec<TransactionInput>,
        amounts_and_scripts: Vec<(u64, Vec<u8>)>, // (amount, pub_key_script)
        lock_time: u32,
        private_key: Option<&[u8]>,
        rng: &mut R,
    ) -> Result<TransactionType, TransactionProcessorError> {
        // If confidential transactions are enabled, create a confidential transaction
        if let Some(ref builder) = self.confidential_builder {
            let conf_tx =
                builder.create_transaction(version, inputs, amounts_and_scripts, lock_time, rng);

            return Ok(TransactionType::Confidential);
        }

        // Create regular outputs
        let outputs = amounts_and_scripts
            .into_iter()
            .map(|(amount, script)| TransactionOutput::new(amount, script))
            .collect();

        // Create a standard transaction
        let tx = Transaction::new(version, inputs, outputs, lock_time);

        // If quantum signatures are enabled, sign the transaction
        if let Some(ref builder) = self.quantum_builder {
            if let Some(key) = private_key {
                let quantum_tx = builder.sign_transaction(tx, key)?;
                return Ok(TransactionType::Quantum(quantum_tx.scheme()));
            } else {
                return Err(TransactionProcessorError::InvalidTransaction(
                    "Private key required for quantum signature".to_string(),
                ));
            }
        }

        // Return standard transaction
        Ok(TransactionType::Standard)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::rngs::OsRng;

    #[test]
    fn test_transaction_processor_standard() {
        // Create a UTXO set
        let utxo_set = Arc::new(DashMap::new());

        // Add some UTXOs
        let prev_tx_hash = [1u8; 32];
        utxo_set.insert(
            (prev_tx_hash, 0),
            TransactionOutput::new(100_000_000, vec![]),
        );

        // Create a transaction processor
        let processor = TransactionProcessor::new(utxo_set, false, false);

        // Create a transaction spending the UTXO
        let inputs = vec![TransactionInput::new(prev_tx_hash, 0, vec![], 0xffffffff)];

        let outputs = vec![TransactionOutput::new(
            90_000_000, // Spending 0.9 NOVA, implicit fee of 0.1 NOVA
            vec![],
        )];

        let tx = Transaction::new(1, inputs, outputs, 0);

        // Process the transaction
        let result = processor.process_transaction(&tx);
        assert!(result.is_ok());
    }

    #[test]
    fn test_transaction_processor_insufficient_funds() {
        // Create a UTXO set
        let utxo_set = Arc::new(DashMap::new());

        // Add some UTXOs
        let prev_tx_hash = [1u8; 32];
        utxo_set.insert(
            (prev_tx_hash, 0),
            TransactionOutput::new(100_000_000, vec![]),
        );

        // Create a transaction processor
        let processor = TransactionProcessor::new(utxo_set, false, false);

        // Create a transaction spending more than the UTXO
        let inputs = vec![TransactionInput::new(prev_tx_hash, 0, vec![], 0xffffffff)];

        let outputs = vec![TransactionOutput::new(
            110_000_000, // Trying to spend 1.1 NOVA when we only have 1.0 NOVA
            vec![],
        )];

        let tx = Transaction::new(1, inputs, outputs, 0);

        // Process the transaction
        let result = processor.process_transaction(&tx);
        assert!(result.is_err());

        match result {
            Err(TransactionProcessorError::InsufficientFunds) => (), // Expected
            Err(e) => panic!("Unexpected error: {:?}", e),
            Ok(_) => panic!("Expected error but got Ok"),
        }
    }
}
