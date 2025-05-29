//! Transaction validation for mempool

use crate::mempool::error::{MempoolError, MempoolResult};
use btclib::types::transaction::Transaction;

/// Transaction validator for mempool
pub struct TransactionValidator {
    /// Minimum fee rate required
    min_fee_rate: u64,
    /// Maximum transaction size
    max_tx_size: usize,
}

impl TransactionValidator {
    /// Create new validator
    pub fn new(min_fee_rate: u64, max_tx_size: usize) -> Self {
        Self { min_fee_rate, max_tx_size }
    }
    
    /// Validate a transaction for mempool inclusion
    pub fn validate(&self, tx: &Transaction) -> MempoolResult<()> {
        // Check transaction size
        let tx_size = tx.size();
        if tx_size > self.max_tx_size {
            return Err(MempoolError::InvalidTransaction(
                format!("Transaction size {} exceeds maximum {}", tx_size, self.max_tx_size)
            ));
        }
        
        // Check fee rate
        let fee = tx.fee().unwrap_or(0);
        let fee_rate = if tx_size > 0 { fee / tx_size as u64 } else { 0 };
        
        if fee_rate < self.min_fee_rate {
            return Err(MempoolError::FeeTooLow {
                required: self.min_fee_rate,
                provided: fee_rate,
            });
        }
        
        // Basic transaction checks
        if tx.inputs().is_empty() {
            return Err(MempoolError::InvalidTransaction(
                "Transaction has no inputs".to_string()
            ));
        }
        
        if tx.outputs().is_empty() {
            return Err(MempoolError::InvalidTransaction(
                "Transaction has no outputs".to_string()
            ));
        }
        
        Ok(())
    }
} 