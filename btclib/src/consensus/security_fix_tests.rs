//! Security Fix Verification Tests
//!
//! These tests verify that our redesigned algorithms prevent the critical vulnerabilities

#[cfg(test)]
mod tests {
    use crate::consensus::fork_resolution_v2::ProofOfWorkForkResolver;
    use crate::consensus::time_warp_prevention::{
        TimeValidationError, TimeWarpConfig, TimeWarpPrevention,
    };
    use crate::types::block::{Block, BlockHeader};
    use crate::types::transaction::{Transaction, TransactionInput, TransactionOutput};
    use crate::validation::unified_validation::{
        UnifiedBlockValidator, UnifiedValidationError, ValidationContext,
    };
    use std::cmp::Ordering;
    use std::collections::HashMap;

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
        let result = resolver
            .compare_chains(&chain_a_tip, &chain_b_tip, get_header)
            .unwrap();
        assert_eq!(result, Ordering::Less, "Chain B should have more work");
    }

    /// Test that classic time warp attack is prevented
    #[test]
    fn test_time_warp_attack_prevented() {
        let mut config = TimeWarpConfig::default();
        // Disable anomaly detection threshold to focus on pattern detection
        config.enable_anomaly_detection = false;
        let mut prevention = TimeWarpPrevention::new(config);

        // Simulate classic time warp: alternating timestamps
        // Previous timestamps should be in reverse order (newest first)
        let previous_timestamps = vec![
            900,  // Block n-1 (most recent)
            2100, // Block n-2
            1000, // Block n-3
            2000, // Block n-4
            1100, // Block n-5
        ];

        // Try to continue the alternating pattern
        let new_timestamp = 2200; // Would continue alternating pattern
        let header = BlockHeader::new(5, [0; 32], [0; 32], new_timestamp, 0x1d00ffff, 0);

        let result = prevention.validate_timestamp(&header, &previous_timestamps, Some(3000));

        // Should detect the alternating pattern
        assert!(
            result.is_err(),
            "Should detect alternating timestamp pattern"
        );
        match result {
            Err(TimeValidationError::ManipulationDetected(msg)) => {
                assert!(
                    msg.contains("Alternating timestamp pattern"),
                    "Error should mention alternating pattern, got: {}",
                    msg
                );
            }
            Ok(_) => panic!("Should have detected time warp attack"),
            Err(e) => panic!("Wrong error type: {:?}", e),
        }
    }

    /// Test median time past enforcement
    #[test]
    fn test_median_time_past_enforcement() {
        let config = TimeWarpConfig::default();
        let mut prevention = TimeWarpPrevention::new(config);

        // Previous 11 blocks (newest first)
        let previous_timestamps = vec![
            1100, 1090, 1080, 1070, 1060, 1050, 1040, 1030, 1020, 1010, 1000,
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
        println!("Multiple coinbase validation result: {:?}", result);
        assert!(
            matches!(result, Err(UnifiedValidationError::MultipleCoinbase)),
            "Expected MultipleCoinbase error, got: {:?}",
            result
        );
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
        assert!(matches!(
            result,
            Err(UnifiedValidationError::InvalidMerkleRoot)
        ));
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
            is_coinbase_mature: None,
        };

        let result = validator.validate_block_secure(&block, Some(&context));
        assert!(matches!(
            result,
            Err(UnifiedValidationError::InvalidSubsidy {
                expected: 50_000_000_000,
                actual: 100_000_000_000
            })
        ));
    }

    /// Test halving schedule validation
    #[test]
    fn test_halving_schedule_validation() {
        let validator = UnifiedBlockValidator::new();

        // Test heights around halving boundaries
        let test_cases = vec![
            (0, 50_000_000_000),       // Genesis: 50 NOVA
            (1, 50_000_000_000),       // Block 1: 50 NOVA
            (209_999, 50_000_000_000), // Just before first halving: 50 NOVA
            (210_000, 25_000_000_000), // First halving: 25 NOVA
            (210_001, 25_000_000_000), // Just after first halving: 25 NOVA
            (419_999, 25_000_000_000), // Just before second halving: 25 NOVA
            (420_000, 12_500_000_000), // Second halving: 12.5 NOVA
            (630_000, 6_250_000_000),  // Third halving: 6.25 NOVA
            (840_000, 3_125_000_000),  // Fourth halving: 3.125 NOVA
            (13_440_000, 0),           // After 64 halvings: 0 NOVA
        ];

        for (height, expected_subsidy) in test_cases {
            let subsidy = validator.calculate_block_subsidy(height);
            assert_eq!(
                subsidy, expected_subsidy,
                "Incorrect subsidy at height {}: expected {}, got {}",
                height, expected_subsidy, subsidy
            );
        }
    }

    /// Test coinbase validation with fees
    #[test]
    fn test_coinbase_with_fees_validation() {
        let mut validator = UnifiedBlockValidator::new();

        // Create a regular transaction with fee
        let regular_tx = Transaction::new(
            1,
            vec![TransactionInput::new([1; 32], 0, vec![], 0xffffffff)], // References UTXO
            vec![TransactionOutput::new(40_000_000_000, vec![5, 6, 7, 8])], // 40 NOVA output
            0,
        );

        // Create coinbase that claims subsidy + fee (50 + 10 = 60 NOVA)
        let coinbase = Transaction::new(
            1,
            vec![TransactionInput::new_coinbase(vec![1, 2, 3])],
            vec![TransactionOutput::new(60_000_000_000, vec![1, 2, 3, 4])], // 60 NOVA
            0,
        );

        let block = Block::new_with_params(1, [0; 32], vec![coinbase, regular_tx], 0x207fffff);

        let context = ValidationContext {
            previous_headers: vec![],
            get_utxo: Box::new(|hash, index| {
                // Mock UTXO for the regular transaction input
                if hash == &[1; 32] && index == 0 {
                    Some(TransactionOutput::new(50_000_000_000, vec![9, 10, 11, 12]))
                // 50 NOVA input
                } else {
                    None
                }
            }),
            height: 1,
            current_time: None,
            is_coinbase_mature: None,
        };

        // Should pass: coinbase claims 60 NOVA (50 subsidy + 10 fee)
        let result = validator.validate_block_secure(&block, Some(&context));
        assert!(
            result.is_ok(),
            "Valid coinbase with fees should pass: {:?}",
            result
        );
    }

    /// Test coinbase trying to claim too much
    #[test]
    fn test_excessive_coinbase_with_fees() {
        let mut validator = UnifiedBlockValidator::new();

        // Create a regular transaction with fee
        let regular_tx = Transaction::new(
            1,
            vec![TransactionInput::new([1; 32], 0, vec![], 0xffffffff)],
            vec![TransactionOutput::new(45_000_000_000, vec![5, 6, 7, 8])], // 45 NOVA output
            0,
        );

        // Create coinbase that claims MORE than subsidy + fee
        let coinbase = Transaction::new(
            1,
            vec![TransactionInput::new_coinbase(vec![1, 2, 3])],
            vec![TransactionOutput::new(61_000_000_000, vec![1, 2, 3, 4])], // 61 NOVA (too much!)
            0,
        );

        let block = Block::new_with_params(1, [0; 32], vec![coinbase, regular_tx], 0x207fffff);

        let context = ValidationContext {
            previous_headers: vec![],
            get_utxo: Box::new(|hash, index| {
                if hash == &[1; 32] && index == 0 {
                    Some(TransactionOutput::new(50_000_000_000, vec![9, 10, 11, 12]))
                // 50 NOVA input
                } else {
                    None
                }
            }),
            height: 1,
            current_time: None,
            is_coinbase_mature: None,
        };

        // Should fail: coinbase claims 61 NOVA but can only claim 55 (50 subsidy + 5 fee)
        let result = validator.validate_block_secure(&block, Some(&context));
        assert!(
            matches!(result, Err(UnifiedValidationError::InvalidSubsidy { .. })),
            "Excessive coinbase should fail"
        );
    }

    /// Test coinbase maturity validation
    #[test]
    fn test_coinbase_maturity_validation() {
        let mut validator = UnifiedBlockValidator::new();

        // Create a transaction spending a coinbase output
        let spend_tx = Transaction::new(
            1,
            vec![TransactionInput::new([99; 32], 0, vec![], 0xffffffff)], // Tries to spend coinbase
            vec![TransactionOutput::new(50_000_000_000, vec![5, 6, 7, 8])],
            0,
        );

        // Create new coinbase
        let coinbase = Transaction::new(
            1,
            vec![TransactionInput::new_coinbase(vec![1, 2, 3])],
            vec![TransactionOutput::new(50_000_000_000, vec![1, 2, 3, 4])],
            0,
        );

        let block = Block::new_with_params(1, [0; 32], vec![coinbase, spend_tx], 0x207fffff);

        let context = ValidationContext {
            previous_headers: vec![],
            get_utxo: Box::new(|hash, index| {
                if hash == &[99; 32] && index == 0 {
                    Some(TransactionOutput::new(50_000_000_000, vec![9, 10, 11, 12]))
                } else {
                    None
                }
            }),
            height: 150, // Height 150, trying to spend coinbase from height 51
            current_time: None,
            is_coinbase_mature: Some(Box::new(|hash, _index, _maturity| {
                // Mock: coinbase from block 51 is only 99 blocks old (immature)
                if hash == &[99; 32] {
                    false // Not mature yet!
                } else {
                    true
                }
            })),
        };

        // Should fail: coinbase not mature
        let result = validator.validate_block_secure(&block, Some(&context));
        assert!(
            matches!(result, Err(UnifiedValidationError::InvalidTransaction(_))),
            "Spending immature coinbase should fail: {:?}",
            result
        );
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

        // Create block with proper timestamp (after previous blocks)
        // Use an extremely easy target (0x207fffff) - max target for testing
        let mut block = Block::new_with_params(1, [0; 32], vec![coinbase], 0x207fffff);
        // Set timestamp to be after the last previous block
        block.header.set_timestamp(7200); // After the 11th block at 6600

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
            is_coinbase_mature: None,
        };

        // Should pass all validations
        let result = validator.validate_block_secure(&block, Some(&context));
        if let Err(e) = &result {
            println!("Full validation error: {:?}", e);
        }
        assert!(
            result.is_ok(),
            "Valid block should pass all security checks: {:?}",
            result
        );
    }
}
