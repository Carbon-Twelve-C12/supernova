//! Safe transaction methods that prevent integer overflow
//!
//! This module provides overflow-safe versions of transaction methods
//! to prevent economic attacks through integer overflow vulnerabilities.

use super::transaction::{Transaction, TransactionOutput};
use super::safe_arithmetic::{safe_add, safe_sub, ArithmeticError};

/// Extension trait for Transaction to add safe arithmetic methods
pub trait TransactionSafe {
    /// Calculate total output with overflow protection
    fn total_output_safe(&self) -> Result<u64, ArithmeticError>;
    
    /// Calculate fee with overflow protection
    fn calculate_fee_safe(&self, get_output: impl Fn(&[u8; 32], u32) -> Option<TransactionOutput>) -> Result<u64, ArithmeticError>;
    
    /// Validate transaction with overflow checking
    fn validate_safe(&self, get_output: impl Fn(&[u8; 32], u32) -> Option<TransactionOutput>) -> bool;
}

impl TransactionSafe for Transaction {
    fn total_output_safe(&self) -> Result<u64, ArithmeticError> {
        self.outputs()
            .iter()
            .map(|output| output.amount())
            .try_fold(0u64, |acc, amount| safe_add(acc, amount))
    }
    
    fn calculate_fee_safe(&self, get_output: impl Fn(&[u8; 32], u32) -> Option<TransactionOutput>) -> Result<u64, ArithmeticError> {
        match self.total_input(&get_output) {
            Some(total_input) => {
                let total_output = self.total_output_safe()?;
                safe_sub(total_input, total_output)
            }
            None => Err(ArithmeticError::SubtractionOverflow), // No inputs found
        }
    }
    
    fn validate_safe(&self, get_output: impl Fn(&[u8; 32], u32) -> Option<TransactionOutput>) -> bool {
        // Check basic structure
        if self.inputs().is_empty() || self.outputs().is_empty() {
            return false;
        }
        
        // Check for output overflow
        let total_output = match self.total_output_safe() {
            Ok(total) => total,
            Err(_) => return false, // Overflow in outputs
        };
        
        // Check inputs vs outputs
        match self.total_input(&get_output) {
            Some(total_input) => {
                if total_input < total_output {
                    return false; // Outputs exceed inputs
                }
            }
            None => return false, // Missing inputs
        }
        
        // Verify signatures (delegates to existing method)
        for (i, input) in self.inputs().iter().enumerate() {
            let prev_output = match get_output(&input.prev_tx_hash(), input.prev_output_index()) {
                Some(output) => output,
                None => return false,
            };
            
            if !self.verify_signature(input.signature_script(), &prev_output.pub_key_script, i) {
                return false;
            }
        }
        
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::transaction::{Transaction, TransactionInput, TransactionOutput};

    fn create_test_transaction(outputs: Vec<u64>) -> Transaction {
        let inputs = vec![TransactionInput::new([0u8; 32], 0, vec![], 0xffffffff)];
        let tx_outputs = outputs.into_iter()
            .map(|amount| TransactionOutput::new(amount, vec![]))
            .collect();
        Transaction::new(1, inputs, tx_outputs, 0)
    }

    #[test]
    fn test_total_output_safe_normal() {
        let tx = create_test_transaction(vec![100_000, 200_000, 300_000]);
        assert_eq!(tx.total_output_safe(), Ok(600_000));
    }

    #[test]
    fn test_total_output_safe_overflow() {
        let tx = create_test_transaction(vec![u64::MAX, 1]);
        assert_eq!(tx.total_output_safe(), Err(ArithmeticError::AdditionOverflow));
    }

    #[test]
    fn test_total_output_safe_near_max() {
        let tx = create_test_transaction(vec![u64::MAX - 100, 50, 50]);
        assert_eq!(tx.total_output_safe(), Ok(u64::MAX));
    }

    #[test]
    fn test_calculate_fee_safe() {
        let tx = create_test_transaction(vec![50_000_000]);
        
        let get_output = |_: &[u8; 32], _: u32| {
            Some(TransactionOutput::new(60_000_000, vec![]))
        };
        
        assert_eq!(tx.calculate_fee_safe(&get_output), Ok(10_000_000));
    }

    #[test]
    fn test_validate_safe_with_overflow() {
        let tx = create_test_transaction(vec![u64::MAX, 1]);
        
        let get_output = |_: &[u8; 32], _: u32| {
            Some(TransactionOutput::new(100_000, vec![]))
        };
        
        // Should fail due to output overflow
        assert!(!tx.validate_safe(&get_output));
    }
} 