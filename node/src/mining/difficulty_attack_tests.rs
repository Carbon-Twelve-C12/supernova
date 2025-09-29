//! Mining Difficulty Attack Prevention Tests
//!
//! This module contains tests that verify the secure difficulty adjustment
//! implementation prevents various manipulation attacks.

#[cfg(test)]
mod tests {
    use crate::mining::{
        SecureDifficultyAdjuster, DifficultySecurityConfig, BlockInfo, SecureDifficultyError
    };
    use std::time::{SystemTime, UNIX_EPOCH};

    /// Helper to create a valid block hash that meets a given target
    fn create_valid_hash(target: u32) -> [u8; 32] {
        // For testing, create a hash that's just below the target
        let adjuster = SecureDifficultyAdjuster::new(DifficultySecurityConfig::default());
        let threshold = adjuster.target_to_threshold(target);

        let mut hash = threshold;
        // Make it slightly smaller to pass PoW check
        if hash[0] > 0 {
            hash[0] -= 1;
        }
        hash
    }

    #[test]
    fn test_prevent_difficulty_lowering_attack() {
        // Attack: Try to artificially lower difficulty by manipulating timestamps
        let config = DifficultySecurityConfig {
            adjustment_interval: 10,
            target_block_time: 60,
            absolute_minimum_difficulty: 1000,
            ..Default::default()
        };

        let mut adjuster = SecureDifficultyAdjuster::new(config);
        let initial_target = 0x1d00ffff;

        // Add legitimate blocks for setup
        for i in 0..5 {
            let block = BlockInfo {
                height: i,
                timestamp: 1000 + i * 60,
                target: initial_target,
                hash: create_valid_hash(initial_target),
                nonce: i,
            };
            adjuster.add_block(block).unwrap();
        }

        // Attacker tries to add blocks with manipulated timestamps to make it seem
        // like blocks are taking much longer than they are
        for i in 5..10 {
            let block = BlockInfo {
                height: i,
                timestamp: 1000 + i * 600, // 10x longer intervals
                target: initial_target,
                hash: create_valid_hash(initial_target),
                nonce: i,
            };
            let result = adjuster.add_block(block);
            // Should either fail or be dampened significantly
            assert!(result.is_ok());
        }

        // Calculate next target at adjustment boundary
        let new_target = adjuster.calculate_next_target(10).unwrap();

        // Verify difficulty didn't drop below minimum
        let new_difficulty = adjuster.target_to_difficulty(new_target);
        assert!(new_difficulty >= 1000, "Difficulty dropped below minimum: {}", new_difficulty);

        // Verify adjustment was limited
        let adjustment_ratio = new_target as f64 / initial_target as f64;
        assert!(adjustment_ratio <= 4.0, "Adjustment exceeded maximum factor: {}", adjustment_ratio);
    }

    #[test]
    fn test_prevent_time_warp_attack() {
        // Attack: Alternating timestamps to manipulate difficulty calculation
        let config = DifficultySecurityConfig {
            adjustment_interval: 20,
            target_block_time: 60,
            enable_anti_manipulation: true,
            ..Default::default()
        };

        let mut adjuster = SecureDifficultyAdjuster::new(config);

        // Try classic time-warp pattern: alternating fast/slow blocks
        let base_time = 1000;
        for i in 0..20 {
            let timestamp = if i % 2 == 0 {
                base_time + i * 30  // Fast blocks (30 seconds)
            } else {
                base_time + i * 90  // Slow blocks (90 seconds)
            };

            let block = BlockInfo {
                height: i,
                timestamp,
                target: 0x1d00ffff,
                hash: create_valid_hash(0x1d00ffff),
                nonce: i,
            };

            let result = adjuster.add_block(block);

            // Anti-manipulation should detect this pattern
            if i >= 10 && result.is_err() {
                assert!(result.unwrap_err().to_string().contains("manipulation"));
                return; // Test passed - attack detected
            }
        }

        // If we get here, check that adjustment was heavily dampened
        let result = adjuster.calculate_next_target(20);
        if let Ok(new_target) = result {
            let ratio = new_target as f64 / 0x1d00ffff as f64;
            assert!(ratio > 0.8 && ratio < 1.2, "Time-warp attack wasn't sufficiently dampened");
        }
    }

    #[test]
    fn test_prevent_51_percent_attack_preparation() {
        // Attack: Attacker tries to lower difficulty drastically to prepare for 51% attack
        let config = DifficultySecurityConfig {
            adjustment_interval: 100,
            target_block_time: 600,
            require_chainwork_progress: true,
            absolute_minimum_difficulty: 10000,
            ..Default::default()
        };

        let mut adjuster = SecureDifficultyAdjuster::new(config);

        // Simulate attacker controlling mining for an adjustment period
        // They mine blocks very slowly to trigger difficulty drop
        for i in 0..100 {
            let block = BlockInfo {
                height: i,
                timestamp: 1000 + i * 3600, // 1 hour per block (6x slower)
                target: 0x1c00ffff,
                hash: create_valid_hash(0x1c00ffff),
                nonce: i,
            };

            let _ = adjuster.add_block(block);
        }

        // Try to get new difficulty
        let result = adjuster.calculate_next_target(100);

        match result {
            Ok(new_target) => {
                // Verify difficulty didn't drop too much
                let old_difficulty = adjuster.target_to_difficulty(0x1c00ffff);
                let new_difficulty = adjuster.target_to_difficulty(new_target);

                assert!(new_difficulty >= old_difficulty / 4,
                    "Difficulty dropped too much: {} -> {}", old_difficulty, new_difficulty);
                assert!(new_difficulty >= 10000,
                    "Difficulty below absolute minimum: {}", new_difficulty);
            },
            Err(e) => {
                // Attack might have been detected
                assert!(e.to_string().contains("manipulation") ||
                        e.to_string().contains("chainwork"));
            }
        }
    }

    #[test]
    fn test_prevent_mining_bypass() {
        // Attack: Try to mine with artificially low difficulty
        let config = DifficultySecurityConfig::default();
        let mut adjuster = SecureDifficultyAdjuster::new(config);

        // Add some legitimate blocks
        for i in 0..10 {
            let block = BlockInfo {
                height: i,
                timestamp: 1000 + i * 600,
                target: 0x1d00ffff,
                hash: create_valid_hash(0x1d00ffff),
                nonce: i,
            };
            adjuster.add_block(block).unwrap();
        }

        // Attacker tries to add a block with easier target
        let easy_target = 0x1f00ffff; // Much easier than current
        let attack_block = BlockInfo {
            height: 10,
            timestamp: 1000 + 10 * 600,
            target: easy_target,
            hash: create_valid_hash(easy_target),
            nonce: 999999,
        };

        let result = adjuster.add_block(attack_block);

        // Should be rejected for wrong target
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Target exceeds maximum"));
    }

    #[test]
    fn test_chainwork_validation() {
        // Verify that chainwork must progress appropriately
        let config = DifficultySecurityConfig {
            require_chainwork_progress: true,
            adjustment_interval: 10,
            ..Default::default()
        };

        let mut adjuster = SecureDifficultyAdjuster::new(config);

        // Add blocks with proper work
        for i in 0..20 {
            let block = BlockInfo {
                height: i,
                timestamp: 1000 + i * 600,
                target: 0x1d00ffff,
                hash: create_valid_hash(0x1d00ffff),
                nonce: i * 12345,
            };
            adjuster.add_block(block).unwrap();
        }

        // Get statistics
        let stats = adjuster.get_statistics();

        // Verify chainwork accumulated
        assert!(stats.total_chainwork > 0);
        assert_eq!(stats.blocks_in_history, 20);

        // Verify average block time is reasonable
        assert!(stats.average_block_time >= 590 && stats.average_block_time <= 610);
    }

    #[test]
    fn test_timestamp_boundaries() {
        let config = DifficultySecurityConfig::default();
        let mut adjuster = SecureDifficultyAdjuster::new(config);

        let current_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        // Test future timestamp rejection
        let future_block = BlockInfo {
            height: 1,
            timestamp: current_time + 10000, // Way in future
            target: 0x1d00ffff,
            hash: create_valid_hash(0x1d00ffff),
            nonce: 1,
        };

        let result = adjuster.add_block(future_block);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("future"));

        // Test backward timestamp rejection
        let block1 = BlockInfo {
            height: 1,
            timestamp: current_time - 1000,
            target: 0x1d00ffff,
            hash: create_valid_hash(0x1d00ffff),
            nonce: 1,
        };
        adjuster.add_block(block1).unwrap();

        let backward_block = BlockInfo {
            height: 2,
            timestamp: current_time - 2000, // Before previous
            target: 0x1d00ffff,
            hash: create_valid_hash(0x1d00ffff),
            nonce: 2,
        };

        let result = adjuster.add_block(backward_block);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not greater than previous"));
    }

    #[test]
    fn test_rapid_block_attack() {
        // Attack: Try to generate blocks too quickly
        let config = DifficultySecurityConfig::default();
        let mut adjuster = SecureDifficultyAdjuster::new(config);

        let base_time = 1000;

        // First block
        let block1 = BlockInfo {
            height: 1,
            timestamp: base_time,
            target: 0x1d00ffff,
            hash: create_valid_hash(0x1d00ffff),
            nonce: 1,
        };
        adjuster.add_block(block1).unwrap();

        // Try to add block with same timestamp
        let block2 = BlockInfo {
            height: 2,
            timestamp: base_time, // Same time!
            target: 0x1d00ffff,
            hash: create_valid_hash(0x1d00ffff),
            nonce: 2,
        };

        let result = adjuster.add_block(block2);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not greater than previous"));
    }

    #[test]
    fn test_oscillation_attack_prevention() {
        // Attack: Try to create difficulty oscillations
        let config = DifficultySecurityConfig {
            adjustment_interval: 10,
            target_block_time: 60,
            ..Default::default()
        };

        let mut adjuster = SecureDifficultyAdjuster::new(config.clone());

        // First period: very fast blocks
        for i in 0..10 {
            let block = BlockInfo {
                height: i,
                timestamp: 1000 + i * 10, // 10 second blocks
                target: 0x1d00ffff,
                hash: create_valid_hash(0x1d00ffff),
                nonce: i,
            };
            adjuster.add_block(block).unwrap();
        }

        // Get harder difficulty
        let harder_target = adjuster.calculate_next_target(10).unwrap();
        assert!(harder_target < 0x1d00ffff); // Lower target = harder

        // Reset for second test
        let mut adjuster2 = SecureDifficultyAdjuster::new(config);
        adjuster2.last_adjustment.adjustment_ratio = 0.25; // Simulate previous hard adjustment

        // Now very slow blocks
        for i in 0..10 {
            let block = BlockInfo {
                height: i,
                timestamp: 1000 + i * 600, // 10 minute blocks
                target: harder_target,
                hash: create_valid_hash(harder_target),
                nonce: i,
            };
            adjuster2.add_block(block).unwrap();
        }

        // Try to get easier difficulty - should detect oscillation
        let result = adjuster2.calculate_next_target(10);

        match result {
            Ok(new_target) => {
                // Should be dampened
                let ratio = new_target as f64 / harder_target as f64;
                assert!(ratio < 2.0, "Oscillation not properly dampened");
            },
            Err(e) => {
                assert!(e.to_string().contains("oscillation"));
            }
        }
    }
}