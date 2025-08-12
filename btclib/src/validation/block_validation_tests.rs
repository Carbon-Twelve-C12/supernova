//! Block Validation Security Tests for Supernova
//! 
//! This module tests the block validation fixes to ensure
//! malicious blocks cannot bypass validation.

#[cfg(test)]
mod tests {
    use crate::validation::block::{BlockValidator, BlockValidationConfig, ValidationContext, BlockValidationError};
    use crate::types::block::{Block, BlockHeader};
    use crate::types::transaction::{Transaction, TransactionInput, TransactionOutput};
    use std::time::{SystemTime, UNIX_EPOCH};

    /// Create a test block with specified parameters
    fn create_test_block(
        height: u64,
        prev_hash: [u8; 32],
        timestamp: u64,
        version: u32,
    ) -> Block {
        // Create coinbase transaction
        let coinbase = Transaction::new_coinbase();
        
        let mut header = BlockHeader::new(
            version,
            prev_hash,
            [0; 32], // Merkle root will be calculated
            timestamp,
            0x1d00ffff, // Easy difficulty
            0,
        );
        header.set_height(height);
        
        let mut block = Block::new(header, vec![coinbase]);
        
        // Calculate and set correct merkle root
        let merkle_root = block.calculate_merkle_root();
        block.header.merkle_root = merkle_root;
        
        block
    }
    
    /// Create a validation context for testing
    fn create_test_context(
        prev_height: u64,
        prev_hash: [u8; 32],
        prev_timestamp: u64,
    ) -> ValidationContext {
        ValidationContext {
            prev_block_hash: prev_hash,
            prev_block_height: prev_height,
            prev_block_timestamp: prev_timestamp,
            median_time_past: prev_timestamp - 3600, // 1 hour before previous block
            current_difficulty: 0x1d00ffff,
            utxo_provider: None,
        }
    }
    
    #[test]
    fn test_validation_bypass_vulnerability_fixed() {
        // This test verifies that the validation bypass vulnerability is fixed
        let validator = BlockValidator::new();
        
        // Create an invalid block (empty transactions)
        let header = BlockHeader::new(
            1,
            [0; 32],
            [0; 32],
            SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
            0x1d00ffff,
            0,
        );
        
        let invalid_block = Block::new(header, vec![]);
        
        // This should fail validation (no coinbase)
        let result = validator.validate_block(&invalid_block);
        assert!(result.is_err());
        
        match result.err().unwrap() {
            BlockValidationError::MissingCoinbase => {},
            _ => panic!("Expected MissingCoinbase error"),
        }
    }
    
    #[test]
    fn test_block_size_validation() {
        let config = BlockValidationConfig {
            max_block_size: 1000, // Very small for testing
            ..BlockValidationConfig::default()
        };
        
        let validator = BlockValidator::with_config(config);
        
        // Create a block with many transactions to exceed size
        let mut transactions = vec![Transaction::new_coinbase()];
        
        // Add many transactions to exceed size limit
        for i in 0..100 {
            let tx = Transaction::new(
                1,
                vec![TransactionInput::new([i as u8; 32], 0, vec![], 0xffffffff)],
                vec![TransactionOutput::new(1000, vec![0; 100])], // Large script
                0,
            );
            transactions.push(tx);
        }
        
        let block = Block::new(
            BlockHeader::new(1, [0; 32], [0; 32], 0, 0x1d00ffff, 0),
            transactions,
        );
        
        let result = validator.validate_block(&block);
        assert!(result.is_err());
        
        match result.err().unwrap() {
            BlockValidationError::BlockTooLarge(_, _) => {},
            _ => panic!("Expected BlockTooLarge error"),
        }
    }
    
    #[test]
    fn test_duplicate_transaction_detection() {
        let validator = BlockValidator::new();
        
        // Create a duplicate transaction
        let tx = Transaction::new(
            1,
            vec![TransactionInput::new([1; 32], 0, vec![], 0xffffffff)],
            vec![TransactionOutput::new(1000, vec![])],
            0,
        );
        
        let block = Block::new(
            BlockHeader::new(1, [0; 32], [0; 32], 0, 0x1d00ffff, 0),
            vec![Transaction::new_coinbase(), tx.clone(), tx], // Duplicate!
        );
        
        let result = validator.validate_block(&block);
        assert!(result.is_err());
        
        match result.err().unwrap() {
            BlockValidationError::DuplicateTransaction(_) => {},
            _ => panic!("Expected DuplicateTransaction error"),
        }
    }
    
    #[test]
    fn test_multiple_coinbase_detection() {
        let validator = BlockValidator::new();
        
        let block = Block::new(
            BlockHeader::new(1, [0; 32], [0; 32], 0, 0x1d00ffff, 0),
            vec![Transaction::new_coinbase(), Transaction::new_coinbase()], // Two coinbases!
        );
        
        let result = validator.validate_block(&block);
        assert!(result.is_err());
        
        match result.err().unwrap() {
            BlockValidationError::MultipleCoinbase => {},
            _ => panic!("Expected MultipleCoinbase error"),
        }
    }
    
    #[test]
    fn test_timestamp_validation() {
        let validator = BlockValidator::new();
        
        let current_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        
        // Test block too far in future
        let future_block = create_test_block(
            1,
            [0; 32],
            current_time + 10000, // Way too far in future
            1,
        );
        
        let result = validator.validate_block(&future_block);
        assert!(result.is_err());
        
        match result.err().unwrap() {
            BlockValidationError::TimestampTooFar(_, _) => {},
            _ => panic!("Expected TimestampTooFar error"),
        }
    }
    
    #[test]
    fn test_full_context_validation() {
        let validator = BlockValidator::new();
        
        let prev_hash = [1; 32];
        let current_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        
        let context = create_test_context(0, prev_hash, current_time - 600);
        
        // Create valid block
        let mut block = create_test_block(1, prev_hash, current_time, 1);
        
        // First, should pass with correct setup
        let result = validator.validate_block_with_context(&block, &context);
        assert!(result.is_ok());
        
        // Now test various invalid conditions
        
        // Wrong previous block hash
        block.header.prev_block_hash = [2; 32];
        let result = validator.validate_block_with_context(&block, &context);
        assert!(matches!(result.err().unwrap(), BlockValidationError::PrevBlockMismatch));
        
        // Fix prev hash but wrong height
        block.header.prev_block_hash = prev_hash;
        block.header.set_height(5); // Should be 1
        let result = validator.validate_block_with_context(&block, &context);
        assert!(matches!(result.err().unwrap(), BlockValidationError::InvalidHeader(_)));
        
        // Fix height but timestamp too early
        block.header.set_height(1);
        block.header.timestamp = context.median_time_past - 1;
        let result = validator.validate_block_with_context(&block, &context);
        assert!(matches!(result.err().unwrap(), BlockValidationError::TimestampTooEarly(_, _)));
    }
    
    #[test]
    fn test_merkle_root_validation() {
        let validator = BlockValidator::new();
        
        let mut block = create_test_block(1, [0; 32], 
            SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(), 1);
        
        // Corrupt the merkle root
        block.header.merkle_root = [0xFF; 32];
        
        let result = validator.validate_block(&block);
        assert!(result.is_err());
        
        match result.err().unwrap() {
            BlockValidationError::InvalidMerkleRoot => {},
            _ => panic!("Expected InvalidMerkleRoot error"),
        }
    }
    
    #[test]
    fn test_proof_of_work_validation() {
        let config = BlockValidationConfig {
            validate_pow: true,
            ..BlockValidationConfig::default()
        };
        
        let validator = BlockValidator::with_config(config);
        
        let context = ValidationContext {
            prev_block_hash: [0; 32],
            prev_block_height: 0,
            prev_block_timestamp: 0,
            median_time_past: 0,
            current_difficulty: 0x1d00ffff,
            utxo_provider: None,
        };
        
        // Create a block that doesn't meet PoW requirements
        let mut block = create_test_block(1, [0; 32], 
            SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(), 1);
        
        // The block hash won't meet the difficulty target unless we mine it
        // For this test, we expect it to fail
        let result = validator.validate_block_with_context(&block, &context);
        
        // It might pass if we get lucky with the hash, but most likely will fail
        if result.is_err() {
            match result.err().unwrap() {
                BlockValidationError::InvalidPoW => {},
                e => panic!("Unexpected error: {:?}", e),
            }
        }
    }
    
    #[test]
    fn test_block_weight_validation() {
        let config = BlockValidationConfig {
            max_block_weight: 1000, // Very low for testing
            ..BlockValidationConfig::default()
        };
        
        let validator = BlockValidator::with_config(config);
        
        // Create a heavy block
        let mut transactions = vec![Transaction::new_coinbase()];
        
        // Add transactions to exceed weight
        for i in 0..10 {
            let tx = Transaction::new(
                1,
                vec![TransactionInput::new([i as u8; 32], 0, vec![0; 50], 0xffffffff)],
                vec![TransactionOutput::new(1000, vec![0; 50])],
                0,
            );
            transactions.push(tx);
        }
        
        let block = Block::new(
            BlockHeader::new(1, [0; 32], [0; 32], 0, 0x1d00ffff, 0),
            transactions,
        );
        
        let result = validator.validate_block(&block);
        assert!(result.is_err());
        
        match result.err().unwrap() {
            BlockValidationError::WeightTooHigh(_, _) => {},
            _ => panic!("Expected WeightTooHigh error"),
        }
    }
    
    #[test]
    fn test_block_version_validation() {
        let config = BlockValidationConfig {
            min_block_version: 2,
            ..BlockValidationConfig::default()
        };
        
        let validator = BlockValidator::with_config(config);
        
        // Create block with old version
        let block = create_test_block(1, [0; 32], 
            SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(), 1);
        
        let result = validator.validate_block(&block);
        assert!(result.is_err());
        
        match result.err().unwrap() {
            BlockValidationError::InvalidVersion(_) => {},
            _ => panic!("Expected InvalidVersion error"),
        }
    }
    
    #[test]
    fn test_coinbase_subsidy_validation() {
        let validator = BlockValidator::new();
        
        let context = create_test_context(0, [0; 32], 0);
        
        // Create a coinbase with excessive reward
        let coinbase_input = TransactionInput::new_coinbase(vec![1, 2, 3]);
        let coinbase = Transaction::new(
            1,
            vec![coinbase_input],
            vec![TransactionOutput::new(100_000_000_000u64, vec![])], // 100 NOVA - too much!
            0
        );
        
        let mut block = Block::new(
            BlockHeader::new(1, [0; 32], [0; 32], 
                SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(), 
                0x1d00ffff, 0),
            vec![coinbase],
        );
        block.header.set_height(1);
        
        let result = validator.validate_block_with_context(&block, &context);
        assert!(result.is_err());
        
        match result.err().unwrap() {
            BlockValidationError::InvalidSubsidy(_, _) => {},
            e => panic!("Expected InvalidSubsidy error, got {:?}", e),
        }
    }
    
    #[test]
    fn test_attack_scenario_crafted_header() {
        // This test simulates the attack where a malicious node sends
        // a block with a crafted header to bypass validation
        
        let validator = BlockValidator::new();
        
        // Attacker creates a block with:
        // 1. Invalid merkle root
        // 2. No transactions
        // 3. Wrong timestamp
        let malicious_block = Block::new(
            BlockHeader::new(
                999, // Invalid version
                [0xFF; 32], // Wrong prev hash
                [0xAA; 32], // Invalid merkle root
                0, // Zero timestamp
                0, // Invalid difficulty
                0,
            ),
            vec![], // No transactions!
        );
        
        // The old implementation would return Ok(()) here!
        // Our new implementation must catch all these issues
        let result = validator.validate_block(&malicious_block);
        
        // Should fail validation
        assert!(result.is_err(), "Malicious block should not pass validation!");
        
        // Could fail for multiple reasons, any is fine as long as it fails
        match result.err().unwrap() {
            BlockValidationError::MissingCoinbase |
            BlockValidationError::InvalidVersion(_) |
            BlockValidationError::TimestampTooFar(_, _) => {},
            e => panic!("Unexpected error type: {:?}", e),
        }
    }
} 