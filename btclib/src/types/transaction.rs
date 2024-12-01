use serde::{Deserialize, Serialize};
use sha2::{Sha256, Digest};

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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_transaction_creation() {
        let inputs = vec![TransactionInput {
            prev_tx_hash: [0u8; 32],
            prev_output_index: 0,
            signature_script: vec![],
            sequence: 0xffffffff,
        }];

        let outputs = vec![TransactionOutput {
            amount: 50_000_000, // 0.5 BTC
            pub_key_script: vec![],
        }];

        let tx = Transaction::new(1, inputs, outputs, 0);
        assert_eq!(tx.version, 1);
        assert_eq!(tx.total_output(), 50_000_000);
    }

    #[test]
    fn test_transaction_validation() {
        let inputs = vec![TransactionInput {
            prev_tx_hash: [0u8; 32],
            prev_output_index: 0,
            signature_script: vec![],
            sequence: 0xffffffff,
        }];

        let outputs = vec![TransactionOutput {
            amount: 50_000_000,
            pub_key_script: vec![],
        }];

        let tx = Transaction::new(1, inputs, outputs, 0);

        // Mock function to provide previous output
        let get_output = |_hash: &[u8; 32], _index: u32| {
            Some(TransactionOutput {
                amount: 60_000_000, // Previous output has more value than current output
                pub_key_script: vec![],
            })
        };

        assert!(tx.validate(&get_output));
    }
}