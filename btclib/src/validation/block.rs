use thiserror::Error;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::types::block::{Block, BlockHeader};
use crate::types::transaction::Transaction;
use crate::validation::transaction::{TransactionValidator, ValidationResult, ValidationError};
use crate::util::merkle::MerkleTree;

/// Error types specific to block validation
#[derive(Debug, Error)]
pub enum BlockValidationError {
    #[error("Missing block header")]
    MissingHeader,
    
    #[error("Invalid proof of work: hash {0:?} does not meet target difficulty {1}")]
    InvalidProofOfWork([u8; 32], u32),
    
    #[error("Invalid merkle root: expected {0:?}, found {1:?}")]
    InvalidMerkleRoot([u8; 32], [u8; 32]),
    
    #[error("Block timestamp too far in future: {0} seconds")]
    TimestampTooFarInFuture(u64),
    
    #[error("Block timestamp before previous block: current {0}, previous {1}")]
    TimestampBeforePrevious(u64, u64),
    
    #[error("Invalid block size: {0} bytes exceeds maximum {1} bytes")]
    BlockTooLarge(usize, usize),
    
    #[error("No transactions in block")]
    NoTransactions,
    
    #[error("Invalid coinbase transaction")]
    InvalidCoinbase,
    
    #[error("Transaction validation failed: {0}")]
    TransactionValidation(#[from] ValidationError),
    
    #[error("Duplicate transaction: {0:?}")]
    DuplicateTransaction([u8; 32]),
    
    #[error("Previous block hash mismatch: expected {0:?}, found {1:?}")]
    PrevBlockMismatch([u8; 32], [u8; 32]),
    
    #[error("Block version too old: {0} (minimum: {1})")]
    VersionTooOld(u32, u32),
}

/// Result of block validation
pub type BlockValidationResult = Result<(), BlockValidationError>;

/// Configuration for block validation
#[derive(Debug, Clone)]
pub struct BlockValidationConfig {
    /// Maximum block size in bytes
    pub max_block_size: usize,
    
    /// Maximum timestamp offset in the future (seconds)
    pub max_future_time_offset: u64,
    
    /// Minimum required block version
    pub min_block_version: u32,
    
    /// Whether to perform full transaction validation
    pub full_transaction_validation: bool,
}

impl Default for BlockValidationConfig {
    fn default() -> Self {
        Self {
            max_block_size: 1_000_000, // 1MB
            max_future_time_offset: 7200, // 2 hours
            min_block_version: 1,
            full_transaction_validation: true,
        }
    }
}

/// Validates blocks according to SuperNova consensus rules
pub struct BlockValidator {
    config: BlockValidationConfig,
    transaction_validator: TransactionValidator,
}

impl BlockValidator {
    /// Create a new block validator with default configuration
    pub fn new() -> Self {
        Self {
            config: BlockValidationConfig::default(),
            transaction_validator: TransactionValidator::new(),
        }
    }
    
    /// Create a new block validator with custom configuration
    pub fn with_config(config: BlockValidationConfig) -> Self {
        Self {
            config,
            transaction_validator: TransactionValidator::new(),
        }
    }
    
    /// Validate a block
    pub fn validate_block(&self, block: &Block, prev_block_header: Option<&BlockHeader>) -> BlockValidationResult {
        // Check block size
        let serialized_size = bincode::serialize(block).map_err(|_| BlockValidationError::MissingHeader)?.len();
        if serialized_size > self.config.max_block_size {
            return Err(BlockValidationError::BlockTooLarge(serialized_size, self.config.max_block_size));
        }
        
        // Check block version
        if block.header().version() < self.config.min_block_version {
            return Err(BlockValidationError::VersionTooOld(
                block.header().version(),
                self.config.min_block_version
            ));
        }
        
        // Validate timestamp
        self.validate_timestamp(block, prev_block_header)?;
        
        // Validate proof of work
        self.validate_proof_of_work(block)?;
        
        // Validate merkle root
        self.validate_merkle_root(block)?;
        
        // Validate transactions
        self.validate_transactions(block)?;
        
        // Validate previous block hash
        if let Some(prev_header) = prev_block_header {
            let expected_prev_hash = prev_header.hash();
            let actual_prev_hash = block.header().prev_block_hash();
            
            if expected_prev_hash != actual_prev_hash {
                return Err(BlockValidationError::PrevBlockMismatch(expected_prev_hash, actual_prev_hash));
            }
        }
        
        Ok(())
    }
    
    /// Validate the block's timestamp
    fn validate_timestamp(&self, block: &Block, prev_block_header: Option<&BlockHeader>) -> BlockValidationResult {
        let block_timestamp = block.header().timestamp();
        
        // Check if timestamp is too far in the future
        let current_timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| BlockValidationError::MissingHeader)?
            .as_secs();
            
        if block_timestamp > current_timestamp + self.config.max_future_time_offset {
            return Err(BlockValidationError::TimestampTooFarInFuture(
                block_timestamp - current_timestamp
            ));
        }
        
        // Check if timestamp is after previous block
        if let Some(prev_header) = prev_block_header {
            let prev_timestamp = prev_header.timestamp();
            
            if block_timestamp < prev_timestamp {
                return Err(BlockValidationError::TimestampBeforePrevious(
                    block_timestamp,
                    prev_timestamp
                ));
            }
        }
        
        Ok(())
    }
    
    /// Validate the block's proof of work
    fn validate_proof_of_work(&self, block: &Block) -> BlockValidationResult {
        let hash = block.hash();
        let target = block.header().target();
        
        // Simple validation: hash must be less than target
        // Note: In a full implementation, you'd need more sophisticated target handling
        let hash_value = u32::from_be_bytes([hash[0], hash[1], hash[2], hash[3]]);
        
        if hash_value > target {
            return Err(BlockValidationError::InvalidProofOfWork(hash, target));
        }
        
        Ok(())
    }
    
    /// Validate the block's merkle root
    fn validate_merkle_root(&self, block: &Block) -> BlockValidationResult {
        let transactions = block.transactions();
        
        if transactions.is_empty() {
            return Err(BlockValidationError::NoTransactions);
        }
        
        // Calculate merkle root from transactions
        let tx_bytes: Vec<Vec<u8>> = transactions
            .iter()
            .map(|tx| bincode::serialize(tx).unwrap_or_default())
            .collect();
            
        let tree = MerkleTree::new(&tx_bytes);
        let calculated_root = tree.root_hash().unwrap_or([0u8; 32]);
        let expected_root = block.header().merkle_root();
        
        if calculated_root != expected_root {
            return Err(BlockValidationError::InvalidMerkleRoot(expected_root, calculated_root));
        }
        
        Ok(())
    }
    
    /// Validate all transactions in the block
    fn validate_transactions(&self, block: &Block) -> BlockValidationResult {
        let transactions = block.transactions();
        
        if transactions.is_empty() {
            return Err(BlockValidationError::NoTransactions);
        }
        
        // First transaction must be coinbase
        self.validate_coinbase(&transactions[0])?;
        
        // Track txids to prevent duplicates
        let mut tx_ids = std::collections::HashSet::new();
        
        // Validate each transaction
        for (i, tx) in transactions.iter().enumerate() {
            // Skip detailed validation of coinbase
            if i > 0 || self.config.full_transaction_validation {
                match self.transaction_validator.validate(tx) {
                    ValidationResult::Valid => {},
                    ValidationResult::Invalid(err) => return Err(BlockValidationError::TransactionValidation(err)),
                    // Soft failures are allowed in the mempool but not in blocks
                    ValidationResult::SoftFail(err) => return Err(BlockValidationError::TransactionValidation(err)),
                }
            }
            
            // Check for duplicate transactions
            let tx_id = tx.hash();
            if !tx_ids.insert(tx_id) {
                return Err(BlockValidationError::DuplicateTransaction(tx_id));
            }
        }
        
        Ok(())
    }
    
    /// Validate the coinbase transaction
    fn validate_coinbase(&self, tx: &Transaction) -> BlockValidationResult {
        // Coinbase must have exactly one input
        if tx.inputs().len() != 1 {
            return Err(BlockValidationError::InvalidCoinbase);
        }
        
        // The input must reference a null previous transaction
        let coinbase_input = &tx.inputs()[0];
        if coinbase_input.prev_tx_hash() != [0u8; 32] {
            return Err(BlockValidationError::InvalidCoinbase);
        }
        
        // Must have at least one output
        if tx.outputs().is_empty() {
            return Err(BlockValidationError::InvalidCoinbase);
        }
        
        // Additional coinbase rules can be implemented here
        
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::transaction::{TransactionInput, TransactionOutput};
    
    // Helper function to create a valid coinbase transaction
    fn create_coinbase_tx(value: u64) -> Transaction {
        Transaction::new(
            1, 
            vec![TransactionInput::new([0u8; 32], 0, vec![], 0)],
            vec![TransactionOutput::new(value, vec![1, 2, 3])],
            0
        )
    }
    
    // Helper function to create a regular transaction
    fn create_regular_tx() -> Transaction {
        Transaction::new(
            1,
            vec![TransactionInput::new([1u8; 32], 0, vec![1, 2, 3], 0)],
            vec![TransactionOutput::new(100, vec![4, 5, 6])],
            0
        )
    }
    
    #[test]
    fn test_valid_block() {
        // Create a block with valid transactions
        let coinbase = create_coinbase_tx(50_000_000);
        let regular_tx = create_regular_tx();
        
        let transactions = vec![coinbase, regular_tx];
        let block = Block::new(1, [0u8; 32], transactions, u32::MAX);
        
        let validator = BlockValidator::new();
        let result = validator.validate_block(&block, None);
        
        assert!(result.is_ok());
    }
    
    #[test]
    fn test_invalid_merkle_root() {
        // Create valid transactions
        let coinbase = create_coinbase_tx(50_000_000);
        let transactions = vec![coinbase];
        
        // Create block with valid transactions
        let mut block = Block::new(1, [0u8; 32], transactions, u32::MAX);
        
        // Corrupt the merkle root
        let header = block.header_mut();
        let mut corrupt_root = header.merkle_root();
        corrupt_root[0] ^= 0xFF;
        header.set_merkle_root(corrupt_root);
        
        let validator = BlockValidator::new();
        let result = validator.validate_block(&block, None);
        
        assert!(matches!(result, Err(BlockValidationError::InvalidMerkleRoot(_, _))));
    }
    
    #[test]
    fn test_timestamp_too_far_future() {
        // Create valid transactions
        let coinbase = create_coinbase_tx(50_000_000);
        let transactions = vec![coinbase];
        
        // Create block with timestamp far in the future
        let mut block = Block::new(1, [0u8; 32], transactions, u32::MAX);
        
        // Set timestamp to 1 day in the future
        let future_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() + 86400; // 1 day
            
        block.header_mut().set_timestamp(future_time);
        
        let validator = BlockValidator::new();
        let result = validator.validate_block(&block, None);
        
        assert!(matches!(result, Err(BlockValidationError::TimestampTooFarInFuture(_))));
    }
    
    #[test]
    fn test_invalid_coinbase() {
        // Create an invalid coinbase (referencing a previous transaction)
        let invalid_coinbase = Transaction::new(
            1,
            vec![TransactionInput::new([1u8; 32], 0, vec![], 0)], // Should be zeros
            vec![TransactionOutput::new(50_000_000, vec![])],
            0
        );
        
        let transactions = vec![invalid_coinbase];
        let block = Block::new(1, [0u8; 32], transactions, u32::MAX);
        
        let validator = BlockValidator::new();
        let result = validator.validate_block(&block, None);
        
        assert!(matches!(result, Err(BlockValidationError::InvalidCoinbase)));
    }
    
    #[test]
    fn test_duplicate_transaction() {
        // Create valid transactions
        let coinbase = create_coinbase_tx(50_000_000);
        let tx = create_regular_tx();
        
        // Include the same transaction twice
        let transactions = vec![coinbase, tx.clone(), tx.clone()];
        let block = Block::new(1, [0u8; 32], transactions, u32::MAX);
        
        let validator = BlockValidator::new();
        let result = validator.validate_block(&block, None);
        
        assert!(matches!(result, Err(BlockValidationError::DuplicateTransaction(_))));
    }
} 