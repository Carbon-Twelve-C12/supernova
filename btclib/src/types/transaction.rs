use serde::{Deserialize, Serialize};
use sha2::{Sha256, Digest};
use crate::environmental::emissions::{EmissionsError, EmissionsTracker, Emissions};

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
    pub_key_script: Vec<u8>,
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
}

impl Transaction {
    /// Create a new transaction
    pub fn new(version: u32, inputs: Vec<TransactionInput>, outputs: Vec<TransactionOutput>, lock_time: u32) -> Self {
        Self {
            version,
            inputs,
            outputs,
            lock_time,
        }
    }

    /// Calculate the transaction hash
    pub fn hash(&self) -> [u8; 32] {
        let serialized = bincode::serialize(&self).unwrap();
        let mut hasher = Sha256::new();
        hasher.update(&serialized);
        let result = hasher.finalize();
        let mut hash = [0u8; 32];
        hash.copy_from_slice(&result);
        hash
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

        // TODO: Verify signatures
        // This would involve checking that each input's signature_script
        // properly satisfies its referenced output's pub_key_script

        true
    }

    /// Calculate the fee rate in satoshis per byte
    pub fn calculate_fee_rate(&self, get_output: impl Fn(&[u8; 32], u32) -> Option<TransactionOutput>) -> Option<u64> {
        // Calculate the transaction size
        let tx_size = self.calculate_size();
        
        // Calculate fee (inputs - outputs)
        if let Some(total_input) = self.total_input(get_output) {
            let total_output = self.total_output();
            
            if total_input > total_output && tx_size > 0 {
                let fee = total_input - total_output;
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

    /// Calculate emissions associated with this transaction
    pub fn calculate_emissions(&self, tracker: &EmissionsTracker) -> Result<Emissions, EmissionsError> {
        tracker.estimate_transaction_emissions(self)
    }
    
    /// Calculate the carbon intensity of this transaction (gCO2e per byte)
    pub fn carbon_intensity(&self, tracker: &EmissionsTracker) -> Result<f64, EmissionsError> {
        let emissions = self.calculate_emissions(tracker)?;
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
        let emissions = self.calculate_emissions(tracker)?;
        Ok(emissions.energy_kwh)
    }
}

// Helper function to calculate variable integer size
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
}