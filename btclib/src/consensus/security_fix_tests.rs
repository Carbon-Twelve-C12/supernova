//! Security Fix Verification Tests
//! 
//! These tests verify that our redesigned algorithms prevent the critical vulnerabilities

#[cfg(test)]
mod tests {
    use crate::consensus::fork_resolution_v2::ProofOfWorkForkResolver;
    use crate::consensus::time_warp_prevention::{TimeWarpPrevention, TimeWarpConfig, TimeValidationError};
    use crate::validation::unified_validation::{UnifiedBlockValidator, UnifiedValidationError, ValidationContext};
    use crate::types::block::{Block, BlockHeader};
    use crate::types::transaction::{Transaction, TransactionInput, TransactionOutput};
    use std::collections::HashMap;
    use std::cmp::Ordering;
    
    /// Test that fork resolution correctly chooses chain with more work
    #[test]
    fn test_fork_resolution_chooses_more_work() {
        let resolver = ProofOfWorkForkResolver::new(100);
        let mut headers = HashMap::new();
        
        // Genesis
        let genesis = BlockHeader::new(0, [0; 32], [0; 32], 0, 0x1f7fffff, 0);
        headers.insert([0; 32], genesis);
        
        // Chain A: 3 blocks with easy difficulty
        let mut prev = [0; 32];
        for i in 1..=3 {
            let hash = [i as u8; 32];
            let header = BlockHeader::new(i, prev, [0; 32], (i as u64) * 600, 0x1f7fffff, 0); // Easy
            headers.insert(hash, header);
            prev = hash;
        }
        let chain_a_tip = prev;
        
        // Chain B: 2 blocks with harder difficulty (more work despite fewer blocks)
        prev = [0; 32];
        for i in 1..=2 {
            let hash = [10 + i as u8; 32];
            let header = BlockHeader::new(i, prev, [0; 32], (i as u64) * 600, 0x1d00ffff, 0); // Harder
            headers.insert(hash, header);
            prev = hash;
        }
        let chain_b_tip = prev;
        
        let get_header = |hash: &[u8; 32]| headers.get(hash).cloned();
        
        // Chain B should win despite having fewer blocks (more total work)
        let result = resolver.compare_chains(&chain_a_tip, &chain_b_tip, get_header).unwrap();
        assert_eq!(result, Ordering::Less, "Chain B should have more work");
    }
    
    /// Test that classic time warp attack is prevented
    #[test]
    fn test_time_warp_attack_prevented() {
        let config = TimeWarpConfig::default();
        let mut prevention = TimeWarpPrevention::new(config);
        
        // Simulate classic time warp: alternating timestamps
        let previous_timestamps = vec![
            2000, // Block n-4
            1000, // Block n-3 (jump back)
            2100, // Block n-2 (jump forward)
            900,  // Block n-1 (jump back)
        ];
        
        // Try to continue the pattern
        let new_timestamp = 2200; // Would continue alternating pattern
        let header = BlockHeader::new(5, [0; 32], [0; 32], new_timestamp, 0x1d00ffff, 0);
        
        let result = prevention.validate_timestamp(&header, &previous_timestamps, Some(3000));
        
        // Should detect manipulation
        assert!(result.is_err());
        match result {
            Err(TimeValidationError::ManipulationDetected(msg)) => {
                assert!(msg.contains("Alternating timestamp pattern"));
            }
            _ => panic!("Expected ManipulationDetected error"),
        }
    }
    
    /// Test median time past enforcement
    #[test]
    fn test_median_time_past_enforcement() {
        let config = TimeWarpConfig::default();
        let mut prevention = TimeWarpPrevention::new(config);
        
        // Previous 11 blocks (newest first)
        let previous_timestamps = vec![
            1100, 1090, 1080, 1070, 1060, 1050, 1040, 1030, 1020, 1010, 1000
        ];
        
        // Median is 1050 (6th element when sorted)
        // New timestamp must be > 1050
        
        let header = BlockHeader::new(12, [0; 32], [0; 32], 1050, 0x1d00ffff, 0);
        let result = prevention.validate_timestamp(&header, &previous_timestamps, Some(2000));
        
        assert!(result.is_err());
        match result {
            Err(TimeValidationError::MedianTimePastViolation(ts, mtp)) => {
                assert_eq!(ts, 1050);
                assert_eq!(mtp, 1050);
            }
            _ => panic!("Expected MedianTimePastViolation"),
        }
    }
    
    /// Test unified validation catches multiple coinbase
    #[test]
    fn test_unified_validation_multiple_coinbase() {
        let mut validator = UnifiedBlockValidator::new();
        
        // Create two distinct coinbase transactions
        let coinbase1 = Transaction::new(
            1,
            vec![TransactionInput::new_coinbase(vec![1, 2, 3])],
            vec![TransactionOutput::new(25_000_000_000, vec![1])],
            0,
        );
        
        let coinbase2 = Transaction::new(
            1,
            vec![TransactionInput::new_coinbase(vec![4, 5, 6])],
            vec![TransactionOutput::new(25_000_000_000, vec![2])],
            0,
        );
        
        let block = Block::new_with_params(1, [0; 32], vec![coinbase1, coinbase2], 0x207fffff);
        
        let result = validator.validate_block_secure(&block, None);
        assert!(matches!(result, Err(UnifiedValidationError::MultipleCoinbase)));
    }
    
    /// Test unified validation catches invalid merkle root
    #[test]
    fn test_unified_validation_merkle_root() {
        let mut validator = UnifiedBlockValidator::new();
        
        let coinbase = Transaction::new(
            1,
            vec![TransactionInput::new_coinbase(vec![1, 2, 3])],
            vec![TransactionOutput::new(50_000_000_000, vec![1, 2, 3, 4])],
            0,
        );
        
        let mut block = Block::new_with_params(1, [0; 32], vec![coinbase], 0x207fffff);
        
        // Corrupt the merkle root
        block.header.merkle_root = [0xFF; 32];
        
        let result = validator.validate_block_secure(&block, None);
        assert!(matches!(result, Err(UnifiedValidationError::InvalidMerkleRoot)));
    }
    
    /// Test excessive coinbase subsidy is caught
    #[test]
    fn test_unified_validation_coinbase_subsidy() {
        let mut validator = UnifiedBlockValidator::new();
        
        // Create coinbase with 100 NOVA (should be 50)
        let coinbase = Transaction::new(
            1,
            vec![TransactionInput::new_coinbase(vec![1, 2, 3])],
            vec![TransactionOutput::new(100_000_000_000, vec![1, 2, 3, 4])], // Too much!
            0,
        );
        
        let block = Block::new_with_params(1, [0; 32], vec![coinbase], 0x207fffff);
        
        let context = ValidationContext {
            previous_headers: vec![],
            get_utxo: Box::new(|_, _| None),
            height: 1,
            current_time: None,
        };
        
        let result = validator.validate_block_secure(&block, Some(&context));
        assert!(matches!(result, Err(UnifiedValidationError::InvalidSubsidy { expected: 50_000_000_000, actual: 100_000_000_000 })));
    }
    
    /// Integration test: Full validation with all security checks
    #[test]
    fn test_full_secure_validation() {
        let mut validator = UnifiedBlockValidator::new();
        
        // Create a valid block
        let coinbase = Transaction::new(
            1,
            vec![TransactionInput::new_coinbase(vec![1, 2, 3])],
            vec![TransactionOutput::new(50_000_000_000, vec![1, 2, 3, 4])],
            0,
        );
        
        let block = Block::new_with_params(1, [0; 32], vec![coinbase], 0x207fffff);
        
        // Create previous headers for timestamp validation
        let mut previous_headers = Vec::new();
        for i in 0..11 {
            let header = BlockHeader::new(
                i,
                if i == 0 { [0; 32] } else { [i as u8; 32] },
                [0; 32],
                1000 + (i as u64) * 600, // 10 minutes apart
                0x207fffff,
                0,
            );
            previous_headers.push(header);
        }
        
        let context = ValidationContext {
            previous_headers,
            get_utxo: Box::new(|_, _| None),
            height: 12,
            current_time: Some(8000), // Current time
        };
        
        // Should pass all validations
        let result = validator.validate_block_secure(&block, Some(&context));
        assert!(result.is_ok(), "Valid block should pass all security checks");
    }
}
