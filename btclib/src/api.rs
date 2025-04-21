use std::sync::Arc;
use crate::config::{Config, QuantumConfig, ZkpConfig};
use crate::crypto::quantum::{QuantumScheme, QuantumKeyPair, QuantumParameters, QuantumError};
use crate::crypto::zkp::{ZkpParams, ZkpType, Commitment, ZeroKnowledgeProof};
use crate::types::transaction::{Transaction, TransactionInput, TransactionOutput};
use crate::types::extended_transaction::{
    QuantumTransaction, ConfidentialTransaction, 
    QuantumTransactionBuilder, ConfidentialTransactionBuilder
};
use crate::transaction_processor::{TransactionProcessor, TransactionType, TransactionProcessorError};

/// High-level API for working with quantum signatures and confidential transactions
pub struct CryptoAPI {
    /// Global configuration
    config: Config,
}

impl CryptoAPI {
    /// Create a new API instance with the given configuration
    pub fn new(config: Config) -> Self {
        Self {
            config,
        }
    }
    
    /// Generate a quantum-resistant key pair with the default settings
    pub fn generate_quantum_keypair<R: rand::CryptoRng + rand::RngCore>(
        &self,
        rng: &mut R,
    ) -> Result<QuantumKeyPair, QuantumError> {
        if !self.config.crypto.quantum.enabled {
            return Err(QuantumError::UnsupportedScheme);
        }
        
        let params = QuantumParameters {
            security_level: self.config.crypto.quantum.security_level,
            scheme: self.config.crypto.quantum.default_scheme,
            use_compression: false,
        };
        
        QuantumKeyPair::generate(self.config.crypto.quantum.default_scheme, Some(params))
    }
    
    /// Sign a transaction using a quantum-resistant signature
    pub fn sign_transaction_quantum(
        &self,
        transaction: Transaction,
        keypair: &QuantumKeyPair,
    ) -> Result<QuantumTransaction, QuantumError> {
        if !self.config.crypto.quantum.enabled {
            return Err(QuantumError::UnsupportedScheme);
        }
        
        // Get the transaction hash
        let tx_hash = transaction.hash();
        
        // Sign the transaction hash
        let signature = keypair.sign(&tx_hash)?;
        
        // Create the quantum transaction
        Ok(QuantumTransaction::new(
            transaction,
            keypair.parameters.scheme,
            keypair.parameters.security_level,
            signature,
        ))
    }
    
    /// Create a confidential transaction
    ///
    /// This method creates a confidential transaction that hides the output amounts,
    /// returning both the transaction and the blinding factors used.
    ///
    /// # Arguments
    /// * `inputs` - The transaction inputs
    /// * `outputs` - The transaction outputs as (amount, pub_key_script) pairs
    /// * `rng` - A cryptographically secure random number generator
    ///
    /// # Returns
    /// * `Result<(ConfidentialTransaction, Vec<Vec<u8>>), TransactionProcessorError>` - 
    ///   The confidential transaction and the blinding factors, or an error
    ///
    /// # Security considerations
    /// The returned blinding factors are critical secrets that must be stored securely.
    /// Loss of a blinding factor will prevent spending the corresponding output.
    pub fn create_confidential_transaction<R: rand::CryptoRng + rand::RngCore>(
        &self,
        inputs: Vec<TransactionInput>,
        outputs: Vec<(u64, Vec<u8>)>, // (amount, pub_key_script)
        rng: &mut R,
    ) -> Result<(ConfidentialTransaction, Vec<Vec<u8>>), TransactionProcessorError> {
        if !self.config.crypto.zkp.enabled {
            return Err(TransactionProcessorError::InvalidTransaction(
                "Confidential transactions are not enabled".to_string(),
            ));
        }
        
        if outputs.len() > self.config.crypto.zkp.max_range_proofs {
            return Err(TransactionProcessorError::InvalidTransaction(
                format!(
                    "Too many outputs: {} (max {})",
                    outputs.len(),
                    self.config.crypto.zkp.max_range_proofs
                ),
            ));
        }
        
        // Create ZKP parameters
        let zkp_params = ZkpParams {
            proof_type: self.config.crypto.zkp.default_scheme,
            security_level: self.config.crypto.zkp.security_level,
        };
        
        // Create a builder
        let builder = ConfidentialTransactionBuilder::new(zkp_params);
        
        // Create the transaction
        let result = builder.create_transaction(
            1, // version
            inputs,
            outputs,
            0, // lock_time
            rng,
        ).map_err(|e| TransactionProcessorError::InvalidTransaction(e.to_string()))?;
        
        Ok(result)
    }
    
    /// Verify a quantum transaction
    pub fn verify_quantum_transaction(
        &self,
        transaction: &QuantumTransaction,
        public_key: &[u8],
    ) -> Result<bool, QuantumError> {
        if !self.config.crypto.quantum.enabled {
            return Err(QuantumError::UnsupportedScheme);
        }
        
        transaction.verify_signature(public_key)
    }
    
    /// Verify a confidential transaction
    pub fn verify_confidential_transaction(
        &self,
        transaction: &ConfidentialTransaction,
    ) -> Result<bool, TransactionProcessorError> {
        if !self.config.crypto.zkp.enabled {
            return Err(TransactionProcessorError::InvalidTransaction(
                "Confidential transactions are not enabled".to_string(),
            ));
        }
        
        // Verify all range proofs
        if !transaction.verify_range_proofs() {
            return Err(TransactionProcessorError::InvalidRangeProof);
        }
        
        // Additional verification would be done here in a real implementation
        
        Ok(true)
    }
    
    /// Create a commitment to a value
    pub fn commit_to_value<R: rand::CryptoRng + rand::RngCore>(
        &self,
        value: u64,
        rng: &mut R,
    ) -> (Commitment, Vec<u8>) {
        if !self.config.crypto.zkp.enabled {
            // Return a dummy commitment if ZKP is disabled
            return (
                Commitment {
                    value: vec![0u8; 32],
                    commitment_type: crate::crypto::zkp::CommitmentType::Pedersen,
                },
                vec![0u8; 32],
            );
        }
        
        crate::crypto::zkp::commit_pedersen(value, rng)
    }
    
    /// Create a range proof for a value
    pub fn create_range_proof<R: rand::CryptoRng + rand::RngCore>(
        &self,
        value: u64,
        blinding_factor: &[u8],
        range_bits: u8,
        rng: &mut R,
    ) -> ZeroKnowledgeProof {
        if !self.config.crypto.zkp.enabled {
            // Return a dummy proof if ZKP is disabled
            return ZeroKnowledgeProof {
                proof_type: ZkpType::Bulletproof,
                proof: vec![0u8; 32],
                public_inputs: vec![],
            };
        }
        
        let params = ZkpParams {
            proof_type: self.config.crypto.zkp.default_scheme,
            security_level: self.config.crypto.zkp.security_level,
        };
        
        crate::crypto::zkp::create_range_proof(value, blinding_factor, range_bits, params, rng)
    }
}

/// Create a high-level API with default settings
pub fn create_default_api() -> CryptoAPI {
    CryptoAPI::new(Config::default())
}

/// Create a high-level API for testnet
pub fn create_testnet_api() -> CryptoAPI {
    CryptoAPI::new(Config::testnet())
}

/// Create a high-level API for regtest
pub fn create_regtest_api() -> CryptoAPI {
    CryptoAPI::new(Config::regtest())
} 