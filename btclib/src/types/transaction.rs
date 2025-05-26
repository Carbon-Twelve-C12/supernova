use serde::{Deserialize, Serialize};
use sha2::{Sha256, Digest};
use crate::environmental::emissions::{EmissionsError, EmissionsTracker, Emissions};
use crate::crypto::signature::{SignatureType, SignatureVerifier, SignatureError};
use crate::crypto::quantum::{QuantumParameters, QuantumScheme, QuantumKeyPair};
use std::fmt;
use chrono::{DateTime, Utc};
use crate::types::block::BlockHeader;
use crate::crypto::hash::{hash_to_hex, double_sha256};
use crate::crypto::signature::{SignatureParams};

/// Transaction validation and processing errors
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransactionError {
    /// Invalid transaction format
    InvalidFormat(String),
    /// Invalid signature
    InvalidSignature(String),
    /// Insufficient funds
    InsufficientFunds,
    /// Double spending attempt
    DoubleSpend,
    /// Invalid input reference
    InvalidInput(String),
    /// Invalid output
    InvalidOutput(String),
    /// Transaction too large
    TooLarge,
    /// Invalid fee
    InvalidFee,
    /// Signature verification failed
    SignatureVerificationFailed,
    /// Quantum signature error
    QuantumSignatureError(String),
    /// Environmental validation error
    EnvironmentalError(EmissionsError),
}

impl fmt::Display for TransactionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TransactionError::InvalidFormat(msg) => write!(f, "Invalid transaction format: {}", msg),
            TransactionError::InvalidSignature(msg) => write!(f, "Invalid signature: {}", msg),
            TransactionError::InsufficientFunds => write!(f, "Insufficient funds"),
            TransactionError::DoubleSpend => write!(f, "Double spending attempt"),
            TransactionError::InvalidInput(msg) => write!(f, "Invalid input: {}", msg),
            TransactionError::InvalidOutput(msg) => write!(f, "Invalid output: {}", msg),
            TransactionError::TooLarge => write!(f, "Transaction too large"),
            TransactionError::InvalidFee => write!(f, "Invalid fee"),
            TransactionError::SignatureVerificationFailed => write!(f, "Signature verification failed"),
            TransactionError::QuantumSignatureError(msg) => write!(f, "Quantum signature error: {}", msg),
            TransactionError::EnvironmentalError(err) => write!(f, "Environmental error: {}", err),
        }
    }
}

impl std::error::Error for TransactionError {}

impl From<EmissionsError> for TransactionError {
    fn from(err: EmissionsError) -> Self {
        TransactionError::EnvironmentalError(err)
    }
}

impl From<SignatureError> for TransactionError {
    fn from(err: SignatureError) -> Self {
        TransactionError::InvalidSignature(err.to_string())
    }
}

/// Reference to a transaction output
#[derive(Debug, Clone, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub struct OutPoint {
    /// Transaction ID (hash)
    pub txid: [u8; 32],
    /// Output index in the transaction
    pub vout: u32,
}

impl fmt::Display for OutPoint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", hex::encode(self.txid), self.vout)
    }
}

/// Represents a transaction input referencing a previous output
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionInput {
    /// Reference to the previous transaction's hash
    prev_tx_hash: [u8; 32],
    /// Index of the output in the previous transaction
    prev_output_index: u32,
    /// Signature script that satisfies the output's conditions
    signature_script: Vec<u8>,
    /// Sequence number for replacement/locktime
    sequence: u32,
}

/// Represents a transaction output with an amount and spending conditions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionOutput {
    /// Amount of coins in this output
    amount: u64,
    /// Public key script that must be satisfied to spend this output
    pub pub_key_script: Vec<u8>,
}

/// Type of signature scheme used in a transaction
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SignatureSchemeType {
    /// Legacy ECDSA with secp256k1 (original Bitcoin)
    Legacy,
    /// Ed25519 signatures
    Ed25519,
    /// Post-quantum Dilithium signatures
    Dilithium,
    /// Post-quantum Falcon signatures
    Falcon,
    /// Post-quantum SPHINCS+ signatures
    Sphincs,
    /// Hybrid scheme combining classical and quantum signatures
    Hybrid,
}

/// Contains signature data for extended signature schemes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionSignatureData {
    /// Signature scheme used
    pub scheme: SignatureSchemeType,
    /// Security level (for quantum schemes)
    pub security_level: u8,
    /// Extended signature data (format depends on scheme)
    pub data: Vec<u8>,
    /// Public key associated with this signature
    pub public_key: Vec<u8>,
}

/// Main transaction structure containing inputs and outputs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transaction {
    /// Version number for protocol upgrades
    version: u32,
    /// List of inputs spending previous outputs
    inputs: Vec<TransactionInput>,
    /// List of new outputs created by this transaction
    outputs: Vec<TransactionOutput>,
    /// Lock time (block height or timestamp)
    lock_time: u32,
    /// Optional signature data for extended signature schemes
    signature_data: Option<TransactionSignatureData>,
}

impl TransactionInput {
    pub fn new(prev_tx_hash: [u8; 32], prev_output_index: u32, signature_script: Vec<u8>, sequence: u32) -> Self {
        Self {
            prev_tx_hash,
            prev_output_index,
            signature_script,
            sequence,
        }
    }

    pub fn prev_tx_hash(&self) -> [u8; 32] {
        self.prev_tx_hash
    }

    pub fn prev_output_index(&self) -> u32 {
        self.prev_output_index
    }

    pub fn signature_script(&self) -> &[u8] {
        &self.signature_script
    }
}

impl TransactionOutput {
    pub fn new(amount: u64, pub_key_script: Vec<u8>) -> Self {
        Self {
            amount,
            pub_key_script,
        }
    }

    pub fn amount(&self) -> u64 {
        self.amount
    }

    /// Get the value (amount) of this output - alias for amount()
    pub fn value(&self) -> u64 {
        self.amount
    }

    /// Get the script pubkey
    pub fn script_pubkey(&self) -> &[u8] {
        &self.pub_key_script
    }
}

impl Transaction {
    /// Create a new transaction
    pub fn new(version: u32, inputs: Vec<TransactionInput>, outputs: Vec<TransactionOutput>, lock_time: u32) -> Self {
        Self {
            version,
            inputs,
            outputs,
            lock_time,
            signature_data: None,
        }
    }

    /// Create a new transaction with extended signature data
    pub fn new_with_signature(
        version: u32, 
        inputs: Vec<TransactionInput>, 
        outputs: Vec<TransactionOutput>, 
        lock_time: u32,
        signature_data: TransactionSignatureData
    ) -> Self {
        Self {
            version,
            inputs,
            outputs,
            lock_time,
            signature_data: Some(signature_data),
        }
    }

    /// Get the transaction's signature data if present
    pub fn signature_data(&self) -> Option<&TransactionSignatureData> {
        self.signature_data.as_ref()
    }

    /// Set the transaction's signature data
    pub fn set_signature_data(&mut self, signature_data: TransactionSignatureData) {
        self.signature_data = Some(signature_data);
    }

    /// Clear the transaction's signature data
    pub fn clear_signature_data(&mut self) {
        self.signature_data = None;
    }

    /// Calculate the transaction hash
    pub fn hash(&self) -> [u8; 32] {
        if self.version >= 2 && self.signature_data.is_some() {
            // For v2+ transactions with extended signatures, calculate hash differently
            // to exclude the signature data for signing purposes
            let mut tx_copy = self.clone();
            tx_copy.signature_data = None;
            
            let serialized = bincode::serialize(&tx_copy).unwrap();
            let mut hasher = Sha256::new();
            hasher.update(&serialized);
            let result = hasher.finalize();
            let mut hash = [0u8; 32];
            hash.copy_from_slice(&result);
            hash
        } else {
            // For legacy transactions or those without extended signatures, use the original hash method
            let serialized = bincode::serialize(&self).unwrap();
            let mut hasher = Sha256::new();
            hasher.update(&serialized);
            let result = hasher.finalize();
            let mut hash = [0u8; 32];
            hash.copy_from_slice(&result);
            hash
        }
    }

    /// Get the transaction version
    pub fn version(&self) -> u32 {
        self.version
    }

    /// Get the lock time
    pub fn lock_time(&self) -> u32 {
        self.lock_time
    }

    /// Get reference to inputs
    pub fn inputs(&self) -> &[TransactionInput] {
        &self.inputs
    }

    /// Get reference to outputs
    pub fn outputs(&self) -> &[TransactionOutput] {
        &self.outputs
    }

    /// Calculate the total input amount (requires access to previous transactions)
    pub fn total_input(&self, get_output: impl Fn(&[u8; 32], u32) -> Option<TransactionOutput>) -> Option<u64> {
        self.inputs
            .iter()
            .map(|input| get_output(&input.prev_tx_hash, input.prev_output_index))
            .try_fold(0u64, |acc, output| {
                output.map(|o| acc.checked_add(o.amount)).flatten()
            })
    }

    /// Calculate the total output amount
    pub fn total_output(&self) -> u64 {
        self.outputs
            .iter()
            .map(|output| output.amount)
            .sum()
    }

    /// Basic validation of the transaction
    pub fn validate(&self, get_output: impl Fn(&[u8; 32], u32) -> Option<TransactionOutput>) -> bool {
        // Ensure transaction has at least one input and output
        if self.inputs.is_empty() || self.outputs.is_empty() {
            return false;
        }

        // Verify total input amount is greater than or equal to total output amount
        match self.total_input(&get_output) {
            Some(total_in) => {
                let total_out = self.total_output();
                if total_in < total_out {
                    return false;
                }
            }
            None => return false, // Couldn't find an input's previous output
        }

        // Verify signatures for each input
        for (i, input) in self.inputs.iter().enumerate() {
            // Get the previous output being spent
            let prev_output = match get_output(&input.prev_tx_hash, input.prev_output_index) {
                Some(output) => output,
                None => return false, // Previous output not found
            };
            
            // Verify the signature script against the previous output's public key script
            if !self.verify_signature(&input.signature_script, &prev_output.pub_key_script, i) {
                return false;
            }
        }

        true
    }
    
    /// Verify a signature for a specific input, handling multiple signature schemes
    pub fn verify_signature(&self, signature_script: &[u8], pub_key_script: &[u8], input_index: usize) -> bool {
        // Check if this is a transaction with extended signature data
        if self.version >= 2 && self.signature_data.is_some() {
            return self.verify_extended_signature(signature_script, pub_key_script, input_index);
        }
        
        // Use the legacy verification for standard transactions
        if let Some(script_type) = self.determine_script_type(pub_key_script) {
            match script_type {
                ScriptType::P2PKH => {
                    // Extract signature and public key from signature script
                    if signature_script.len() < 2 {
                        return false; // Invalid script format
                    }
                    
                    // In a real implementation, we would:
                    // 1. Parse the signature script to extract the signature and public key
                    // 2. Verify the signature against the transaction hash
                    // 3. Verify the public key hash matches the one in pub_key_script
                    
                    // For now, we'll implement a basic verification
                    let signature_offset = 1; // Skip the first byte (script len)
                    let signature_len = signature_script[signature_offset] as usize;
                    
                    if signature_script.len() < signature_offset + 1 + signature_len {
                        return false; // Invalid script format
                    }
                    
                    let signature = &signature_script[signature_offset + 1..signature_offset + 1 + signature_len];
                    
                    let pubkey_offset = signature_offset + 1 + signature_len;
                    if signature_script.len() <= pubkey_offset {
                        return false; // Invalid script format
                    }
                    
                    let pubkey_len = signature_script[pubkey_offset] as usize;
                    
                    if signature_script.len() < pubkey_offset + 1 + pubkey_len {
                        return false; // Invalid script format
                    }
                    
                    let pubkey = &signature_script[pubkey_offset + 1..pubkey_offset + 1 + pubkey_len];
                    
                    // Compute the hash of the transaction for this input (sighash)
                    let sighash = self.calculate_sighash(input_index, pub_key_script);
                    
                    // Verify the signature (simplified for this implementation)
                    self.verify_ecdsa_signature(signature, pubkey, &sighash)
                },
                ScriptType::P2SH => true,
                ScriptType::P2WPKH => true,
                ScriptType::P2WSH => true,
            }
        } else {
            // Unknown script type
            false
        }
    }
    
    /// Verify a signature using the extended signature data
    fn verify_extended_signature(&self, _signature_script: &[u8], _pub_key_script: &[u8], _input_index: usize) -> bool {
        let signature_data = match &self.signature_data {
            Some(data) => data,
            None => return false, // No signature data available
        };
        
        // Calculate the transaction hash for signing (this excludes the signature data itself)
        let message_hash = self.hash();
        
        // Create a signature verifier
        let signature_verifier = SignatureVerifier::new();
        
        // Determine which verification logic to use based on the signature scheme
        match signature_data.scheme {
            SignatureSchemeType::Legacy => {
                match signature_verifier.verify(
                    SignatureType::Secp256k1,
                    &signature_data.public_key,
                    &message_hash,
                    &signature_data.data
                ) {
                    Ok(result) => result,
                    Err(_) => false,
                }
            },
            SignatureSchemeType::Ed25519 => {
                match signature_verifier.verify(
                    SignatureType::Ed25519,
                    &signature_data.public_key,
                    &message_hash,
                    &signature_data.data
                ) {
                    Ok(result) => result,
                    Err(_) => false,
                }
            },
            SignatureSchemeType::Dilithium => {
                let params = QuantumParameters {
                    scheme: QuantumScheme::Dilithium,
                    security_level: signature_data.security_level,
                };
                
                match crate::crypto::quantum::verify_quantum_signature(
                    &signature_data.public_key,
                    &message_hash,
                    &signature_data.data,
                    params
                ) {
                    Ok(result) => result,
                    Err(_) => false,
                }
            },
            SignatureSchemeType::Falcon => {
                let params = QuantumParameters {
                    scheme: QuantumScheme::Falcon,
                    security_level: signature_data.security_level,
                };
                
                match crate::crypto::quantum::verify_quantum_signature(
                    &signature_data.public_key,
                    &message_hash,
                    &signature_data.data,
                    params
                ) {
                    Ok(result) => result,
                    Err(_) => false,
                }
            },
            SignatureSchemeType::Sphincs => {
                let params = QuantumParameters {
                    scheme: QuantumScheme::Sphincs,
                    security_level: signature_data.security_level,
                };
                
                match crate::crypto::quantum::verify_quantum_signature(
                    &signature_data.public_key,
                    &message_hash,
                    &signature_data.data,
                    params
                ) {
                    Ok(result) => result,
                    Err(_) => false,
                }
            },
            SignatureSchemeType::Hybrid => {
                // Implementation for hybrid verification is more complex and would require
                // parsing the data to separate classical and quantum signatures
                // For now, return false to indicate it's not implemented
                false
            },
        }
    }

    /// Determine the type of script based on the public key script
    fn determine_script_type(&self, pub_key_script: &[u8]) -> Option<ScriptType> {
        if pub_key_script.len() >= 25 && pub_key_script[0] == 0x76 && pub_key_script[1] == 0xa9 {
            // P2PKH: OP_DUP OP_HASH160 <pubKeyHash> OP_EQUALVERIFY OP_CHECKSIG
            Some(ScriptType::P2PKH)
        } else if pub_key_script.len() >= 23 && pub_key_script[0] == 0xa9 {
            // P2SH: OP_HASH160 <scriptHash> OP_EQUAL
            Some(ScriptType::P2SH)
        } else if pub_key_script.len() >= 22 && pub_key_script[0] == 0x00 && pub_key_script[1] == 0x14 {
            // P2WPKH: OP_0 <20-byte-key-hash>
            Some(ScriptType::P2WPKH)
        } else if pub_key_script.len() >= 34 && pub_key_script[0] == 0x00 && pub_key_script[1] == 0x20 {
            // P2WSH: OP_0 <32-byte-script-hash>
            Some(ScriptType::P2WSH)
        } else {
            None
        }
    }
    
    /// Calculate the signature hash (sighash) for a specific input
    fn calculate_sighash(&self, input_index: usize, prev_script_pubkey: &[u8]) -> [u8; 32] {
        // This is a simplified implementation
        // In a real implementation, we would:
        // 1. Create a copy of the transaction
        // 2. Zero out all input scripts
        // 3. Set the script of the input being signed to the previous output's script pubkey
        // 4. Append the hash type (SIGHASH_ALL, etc.)
        // 5. Double SHA256 the result
        
        // For now, just hash the transaction data
        self.hash()
    }
    
    /// Verify an ECDSA signature
    fn verify_ecdsa_signature(&self, signature: &[u8], pubkey: &[u8], message_hash: &[u8; 32]) -> bool {
        // In a real implementation, we would:
        // 1. Parse the signature (DER format) + sighash flag
        // 2. Parse the public key
        // 3. Verify the signature against the message hash
        
        // For now, assume the signature is valid
        // This would be replaced with actual crypto verification in production
        true
    }

    /// Calculate the fee rate in satoshis per byte
    pub fn calculate_fee_rate(&self, get_output: impl Fn(&[u8; 32], u32) -> Option<TransactionOutput>) -> Option<u64> {
        // Calculate the transaction size
        let tx_size = self.calculate_size();
        
        // Calculate fee (inputs - outputs)
        if let Some(fee) = self.calculate_fee(get_output) {
            if tx_size > 0 {
                return Some(fee / tx_size as u64);
            }
        }
        
        None
    }
    
    /// Calculate the transaction size in bytes
    pub fn calculate_size(&self) -> usize {
        // Version (4 bytes) + locktime (4 bytes)
        let mut size = 8;
        
        // Add input sizes
        for input in &self.inputs {
            // Previous tx hash (32) + output index (4) + sequence (4) + script length (1-9)
            size += 40 + input.signature_script.len();
        }
        
        // Add output sizes
        for output in &self.outputs {
            // Amount (8) + script length (1-9)
            size += 9 + output.pub_key_script.len();
        }
        
        // Add variable length encoding for input and output counts
        size += varint_size(self.inputs.len() as u64);
        size += varint_size(self.outputs.len() as u64);
        
        size
    }

    /// Calculate the carbon intensity of this transaction (gCO2e per byte)
    pub fn carbon_intensity(&self, tracker: &EmissionsTracker) -> Result<f64, EmissionsError> {
        let emissions = self.estimate_emissions(tracker)?;
        let size = self.calculate_size() as f64;
        
        if size > 0.0 {
            // Convert tonnes to grams and divide by size
            Ok((emissions.tonnes_co2e * 1_000_000.0) / size)
        } else {
            Err(EmissionsError::InvalidTimeRange) // Reuse existing error type
        }
    }
    
    /// Check if this transaction is carbon neutral (has offsetting certificates)
    pub fn is_carbon_neutral(&self, tracker: &EmissionsTracker) -> Result<bool, EmissionsError> {
        let emissions = self.calculate_emissions(tracker)?;
        
        // If market-based emissions available and close to zero
        if let Some(market_emissions) = emissions.market_based_emissions {
            Ok(market_emissions < 0.001) // Threshold for "carbon neutral"
        } else {
            // Fall back to comparing renewable percentage
            if let Some(renewable_pct) = emissions.renewable_percentage {
                Ok(renewable_pct >= 99.0) // 99% or more renewable
            } else {
                Ok(false)
            }
        }
    }
    
    /// Get estimated energy consumption of this transaction in kWh
    pub fn energy_consumption(&self, tracker: &EmissionsTracker) -> Result<f64, EmissionsError> {
        let emissions = self.estimate_emissions(tracker)?;
        Ok(emissions.energy_kwh)
    }

    /// Sign this transaction using the specified signature scheme
    pub fn sign(
        &mut self,
        private_key: &[u8],
        public_key: &[u8],
        scheme: SignatureSchemeType,
        security_level: u8
    ) -> Result<(), SignatureError> {
        // Calculate the hash of the transaction (without existing signature data)
        let tx_hash = self.hash();
        
        // Generate the signature based on the specified scheme
        let signature = match scheme {
            SignatureSchemeType::Legacy => {
                // Sign with secp256k1
                // This is a placeholder - in a real implementation, we would use the
                // appropriate crypto library to generate a real signature
                vec![0u8; 64] // Placeholder
            },
            SignatureSchemeType::Ed25519 => {
                // Sign with Ed25519
                // This is a placeholder
                vec![0u8; 64] // Placeholder
            },
            SignatureSchemeType::Dilithium => {
                // Create a quantum keypair for signing
                let mut quantum_keypair = QuantumKeyPair {
                    public_key: public_key.to_vec(),
                    secret_key: private_key.to_vec(),
                    parameters: QuantumParameters {
                        scheme: QuantumScheme::Dilithium,
                        security_level,
                    },
                };
                
                // Sign with Dilithium
                quantum_keypair.sign(&tx_hash)
                    .map_err(|e| SignatureError::CryptoOperationFailed(format!("Dilithium signing failed: {}", e)))?
            },
            SignatureSchemeType::Falcon => {
                // Create a quantum keypair for signing
                let mut quantum_keypair = QuantumKeyPair {
                    public_key: public_key.to_vec(),
                    secret_key: private_key.to_vec(),
                    parameters: QuantumParameters {
                        scheme: QuantumScheme::Falcon,
                        security_level,
                    },
                };
                
                // Sign with Falcon
                quantum_keypair.sign(&tx_hash)
                    .map_err(|e| SignatureError::CryptoOperationFailed(format!("Falcon signing failed: {}", e)))?
            },
            SignatureSchemeType::Sphincs => {
                // This is a placeholder - SPHINCS+ is not yet fully implemented
                return Err(SignatureError::UnsupportedScheme("SPHINCS+ not yet implemented".to_string()));
            },
            SignatureSchemeType::Hybrid => {
                // This is a placeholder - hybrid schemes are not yet fully implemented
                return Err(SignatureError::UnsupportedScheme("Hybrid signatures not yet implemented".to_string()));
            },
        };
        
        // Set the signature data
        self.signature_data = Some(TransactionSignatureData {
            scheme,
            security_level,
            data: signature,
            public_key: public_key.to_vec(),
        });
        
        Ok(())
    }

    /// Calculate the transaction fee (inputs - outputs)
    pub fn calculate_fee(&self, get_output: impl Fn(&[u8; 32], u32) -> Option<TransactionOutput>) -> Option<u64> {
        if let Some(total_input) = self.total_input(&get_output) {
            let total_output = self.total_output();
            
            if total_input > total_output {
                return Some(total_input - total_output);
            }
        }
        
        None
    }

    /// Get the priority score of this transaction based on fee rate and other factors
    pub fn get_priority_score(&self, get_output: impl Fn(&[u8; 32], u32) -> Option<TransactionOutput>) -> u64 {
        // Base priority is the fee rate
        let base_priority = self.calculate_fee_rate(&get_output).unwrap_or(0);
        
        // Apply environmental and quantum bonus factors if applicable
        let mut priority = base_priority;
        
        // Apply a bonus for transactions using quantum-resistant signatures
        if let Some(sig_data) = &self.signature_data {
            match sig_data.scheme {
                SignatureSchemeType::Dilithium | 
                SignatureSchemeType::Falcon | 
                SignatureSchemeType::Sphincs | 
                SignatureSchemeType::Hybrid => {
                    // 10% bonus for quantum-resistant transactions
                    priority = priority.saturating_add(priority / 10);
                },
                _ => {}
            }
        }
        
        // Future: Add environmental bonus based on carbon neutrality
        
        priority
    }
    
    /// Compare two transactions for ordering in mempool based on priority
    pub fn compare_by_priority(&self, other: &Self, get_output: impl Fn(&[u8; 32], u32) -> Option<TransactionOutput>) -> std::cmp::Ordering {
        let self_priority = self.get_priority_score(&get_output);
        let other_priority = other.get_priority_score(&get_output);
        
        // Higher priority comes first
        other_priority.cmp(&self_priority)
    }

    /// Check if this is a coinbase transaction
    pub fn is_coinbase(&self) -> bool {
        if self.inputs.len() != 1 {
            return false;
        }
        
        let input = &self.inputs[0];
        
        // Check for null previous tx hash (all zeros)
        let is_zero_hash = input.prev_tx_hash.iter().all(|&byte| byte == 0);
        
        // Check for special 0xFFFFFFFF or 0 index
        let is_special_index = input.prev_output_index == 0xFFFFFFFF || input.prev_output_index == 0;
        
        is_zero_hash && is_special_index
    }
    
    /// Estimate emissions for this transaction
    pub fn estimate_emissions(&self, tracker: &EmissionsTracker) -> Result<Emissions, EmissionsError> {
        // Get byte size as a proxy for energy consumption
        let tx_size = self.calculate_size();
        
        // Use a dummy size-based estimation method instead of passing self
        let avg_emissions_factor = 0.5;
        let energy_per_byte = 0.0000002;
        let tx_energy = tx_size as f64 * energy_per_byte;
        let tx_emissions = tx_energy * avg_emissions_factor;
        
        // Create a simplified emissions struct
        Ok(Emissions {
            tonnes_co2e: tx_emissions / 1000.0, // Convert kg to tonnes
            energy_kwh: tx_energy,
            renewable_percentage: None,
            location_based_emissions: None,
            market_based_emissions: None,
            marginal_emissions_impact: None,
            calculation_time: chrono::Utc::now(),
            confidence_level: Some(0.5),
        })
    }

    /// Calculate emissions associated with this transaction
    pub fn calculate_emissions(&self, _tracker: &EmissionsTracker) -> Result<Emissions, EmissionsError> {
        // Use our size-based estimation instead of calling tracker.estimate_transaction_emissions
        self.estimate_emissions(_tracker)
    }

    /// Create a new coinbase transaction with an empty input and a reward output
    pub fn new_coinbase() -> Self {
        // Create an empty input that represents "coins from nowhere"
        let input = TransactionInput::new(
            [0; 32],  // Previous TX hash is all zeros for coinbase
            0xFFFFFFFF, // Special index value for coinbase
            vec![0],  // Empty script
            0,        // Sequence
        );
        
        // Create a reward output with block subsidy (simplified)
        let output = TransactionOutput::new(
            5000000000, // 50 BTC subsidy (simplified)
            vec![0x76, 0xa9, 0x14, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x88, 0xac], // P2PKH placeholder
        );
        
        Self {
            version: 1,
            inputs: vec![input],
            outputs: vec![output],
            lock_time: 0,
            signature_data: None,
        }
    }
    
    /// Basic validation of the transaction structure without checking inputs
    pub fn validate_basic(&self) -> bool {
        // Ensure transaction has at least one input and output
        if self.inputs.is_empty() || self.outputs.is_empty() {
            return false;
        }
        
        // Check for negative or zero outputs 
        for output in &self.outputs {
            if output.amount == 0 {
                return false;
            }
        }
        
        // For coinbase transactions, check they have exactly one input
        if self.is_coinbase() && self.inputs.len() != 1 {
            return false;
        }
        
        // Make sure transaction size isn't too large
        if self.calculate_size() > 1_000_000 { // 1MB limit (simplified)
            return false;
        }
        
        true
    }
}

/// Calculate the size of a variable-length integer
fn varint_size(value: u64) -> usize {
    if value < 0xfd {
        1
    } else if value <= 0xffff {
        3
    } else if value <= 0xffffffff {
        5
    } else {
        9
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_transaction_creation() {
        let inputs = vec![TransactionInput::new(
            [0u8; 32],
            0,
            vec![],
            0xffffffff,
        )];

        let outputs = vec![TransactionOutput::new(
            50_000_000, // 0.5 NOVA
            vec![],
        )];

        let tx = Transaction::new(1, inputs, outputs, 0);
        assert_eq!(tx.version, 1);
        assert_eq!(tx.total_output(), 50_000_000);
    }

    #[test]
    fn test_transaction_validation() {
        let inputs = vec![TransactionInput::new(
            [0u8; 32],
            0,
            vec![],
            0xffffffff,
        )];

        let outputs = vec![TransactionOutput::new(
            50_000_000,
            vec![],
        )];

        let tx = Transaction::new(1, inputs, outputs, 0);

        // Mock function to provide previous output
        let get_output = |_hash: &[u8; 32], _index: u32| {
            Some(TransactionOutput::new(
                60_000_000, // Previous output has more value than current output
                vec![],
            ))
        };

        assert!(tx.validate(&get_output));
    }

    #[test]
    fn test_transaction_with_signature_data() {
        let inputs = vec![TransactionInput::new(
            [0u8; 32],
            0,
            vec![],
            0xffffffff,
        )];

        let outputs = vec![TransactionOutput::new(
            50_000_000,
            vec![],
        )];

        let signature_data = TransactionSignatureData {
            scheme: SignatureSchemeType::Dilithium,
            security_level: 3,
            data: vec![1, 2, 3, 4], // Example data
            public_key: vec![5, 6, 7, 8], // Example public key
        };

        let tx = Transaction::new_with_signature(2, inputs, outputs, 0, signature_data);
        
        assert_eq!(tx.version, 2);
        assert!(tx.signature_data().is_some());
        if let Some(sig_data) = tx.signature_data() {
            assert_eq!(sig_data.scheme, SignatureSchemeType::Dilithium);
            assert_eq!(sig_data.security_level, 3);
        }
    }

    #[test]
    fn test_transaction_fee_calculation() {
        let inputs = vec![TransactionInput::new(
            [0u8; 32],
            0,
            vec![0; 10], // Add some data to make size non-zero
            0xffffffff,
        )];

        let outputs = vec![TransactionOutput::new(
            50_000_000, // 0.5 NOVA
            vec![0; 10], // Add some data to make size non-zero
        )];

        let tx = Transaction::new(1, inputs, outputs, 0);
        
        // Mock function to provide previous output with higher value
        let get_output = |_hash: &[u8; 32], _index: u32| {
            Some(TransactionOutput::new(
                60_000_000, // 0.6 NOVA
                vec![],
            ))
        };
        
        // Fee should be (60_000_000 - 50_000_000) = 10_000_000
        assert_eq!(tx.calculate_fee(&get_output), Some(10_000_000));
        
        // Fee rate should be fee divided by size
        let size = tx.calculate_size();
        assert!(size > 0);
        let expected_fee_rate = 10_000_000 / size as u64;
        assert_eq!(tx.calculate_fee_rate(&get_output), Some(expected_fee_rate));
    }
    
    #[test]
    fn test_transaction_priority() {
        let inputs = vec![TransactionInput::new(
            [0u8; 32],
            0,
            vec![0; 10],
            0xffffffff,
        )];

        let outputs = vec![TransactionOutput::new(
            50_000_000,
            vec![0; 10],
        )];
        
        // Create a standard transaction
        let tx1 = Transaction::new(1, inputs.clone(), outputs.clone(), 0);
        
        // Create a quantum-resistant transaction
        let signature_data = TransactionSignatureData {
            scheme: SignatureSchemeType::Dilithium,
            security_level: 3,
            data: vec![1, 2, 3, 4],
            public_key: vec![5, 6, 7, 8],
        };
        let tx2 = Transaction::new_with_signature(2, inputs, outputs, 0, signature_data);
        
        // Mock function to provide previous output
        let get_output = |_hash: &[u8; 32], _index: u32| {
            Some(TransactionOutput::new(
                60_000_000,
                vec![],
            ))
        };
        
        // The quantum transaction should have a higher priority
        let priority1 = tx1.get_priority_score(&get_output);
        let priority2 = tx2.get_priority_score(&get_output);
        
        assert!(priority2 > priority1);
    }
}

/// Enum representing different types of transaction scripts
#[derive(Debug, Clone, PartialEq)]
pub enum ScriptType {
    /// Pay to Public Key Hash
    P2PKH,
    /// Pay to Script Hash
    P2SH,
    /// Pay to Witness Public Key Hash
    P2WPKH,
    /// Pay to Witness Script Hash
    P2WSH,
}