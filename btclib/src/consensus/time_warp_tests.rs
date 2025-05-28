//! Tests for time-warp attack prevention
//! 
//! This module contains comprehensive tests to verify that the consensus
//! rules properly prevent time-warp attacks.

#[cfg(test)]
mod tests {
    use crate::consensus::difficulty::{DifficultyAdjustment, DifficultyAdjustmentConfig};
    use crate::consensus::timestamp_validation::{TimestampValidator, TimestampValidationConfig, MAX_FUTURE_TIME};
    use std::time::{SystemTime, UNIX_EPOCH};
    
    /// Helper to get current Unix timestamp
    fn current_time() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()
    }
    
    #[test]
    fn test_classic_time_warp_attack_prevention() {
        // Classic time-warp attack: manipulate timestamps to reduce difficulty
        let mut config = DifficultyAdjustmentConfig::default();
        config.adjustment_interval = 10; // Small interval for testing
        config.validate_timestamps = true;
        
        let adjuster = DifficultyAdjustment::with_config(config);
        let initial_target = 0x1d00ffff;
        
        // Scenario: Attacker tries to make blocks appear to take longer than they did
        let base_time = 1000000;
        let mut timestamps = vec![];
        let mut heights = vec![];
        
        // First 5 blocks with normal timestamps (60 seconds apart)
        for i in 0..5 {
            timestamps.push(base_time + i * 60);
            heights.push(i);
        }
        
        // Attacker tries to set next 5 blocks far in the future to trigger easier difficulty
        for i in 5..10 {
            // Try to set timestamps 1 hour apart instead of 1 minute
            timestamps.push(base_time + 300 + (i - 5) * 3600);
            heights.push(i);
        }
        
        // This should be detected and prevented
        let result = adjuster.calculate_next_target(initial_target, &timestamps, &heights);
        
        // Should fail validation due to suspicious timestamp pattern
        assert!(result.is_err());
    }
    
    #[test]
    fn test_timestamp_manipulation_at_adjustment_boundary() {
        // Test manipulation right at difficulty adjustment boundary
        let validator = TimestampValidator::new();
        let base_time = current_time() - 86400; // 1 day ago
        
        // Create a history of blocks
        let mut timestamps = vec![];
        for i in 0..2015 {
            timestamps.push(base_time + i * 600); // 10 minutes per block
        }
        
        // Try to manipulate the last block before adjustment
        let manipulated_timestamp = base_time + 2015 * 600 + 7200; // 2 hours extra
        
        // This should be caught by future time validation
        let result = validator.validate_timestamp(
            manipulated_timestamp,
            &timestamps,
            Some(current_time()),
        );
        
        // Should pass only if within allowed future time
        if manipulated_timestamp > current_time() + MAX_FUTURE_TIME {
            assert!(result.is_err());
        }
    }
    
    #[test]
    fn test_median_time_past_protection() {
        // Test that median time past prevents timestamp rollback attacks
        let validator = TimestampValidator::new();
        
        // Create a block history
        let timestamps = vec![100, 200, 300, 400, 500, 600, 700, 800, 900, 1000, 1100];
        
        // Try to create a block with timestamp before median
        let median = validator.calculate_median_past_time(&timestamps).unwrap();
        let attack_timestamp = median - 100;
        
        let result = validator.validate_timestamp(
            attack_timestamp,
            &timestamps,
            None,
        );
        
        // Should fail - timestamp before median time past
        assert!(result.is_err());
    }
    
    #[test]
    fn test_timestamp_clustering_attack() {
        // Test prevention of timestamp clustering to game difficulty
        let validator = TimestampValidator::new();
        
        // Create timestamps with suspicious clustering
        let mut timestamps = vec![];
        let mut heights = vec![];
        
        // Normal blocks
        for i in 0..10 {
            timestamps.push(1000 + i * 600);
            heights.push(i);
        }
        
        // Clustered blocks - many with same timestamp
        for i in 10..17 {
            timestamps.push(7000); // 7 blocks with same timestamp
            heights.push(i);
        }
        
        let result = validator.validate_difficulty_timestamps(&timestamps, &heights, 2016);
        
        // Should fail - too many consecutive blocks with same timestamp
        assert!(result.is_err());
    }
    
    #[test]
    fn test_gradual_timestamp_drift_attack() {
        // Test prevention of gradual timestamp drift to slowly reduce difficulty
        let mut config = DifficultyAdjustmentConfig::default();
        config.adjustment_interval = 20;
        config.validate_timestamps = true;
        
        let adjuster = DifficultyAdjustment::with_config(config);
        let initial_target = 0x1d00ffff;
        
        let mut timestamps = vec![];
        let mut heights = vec![];
        let base_time = 1000000;
        
        // Attacker gradually increases timestamps to make blocks appear slower
        for i in 0..20 {
            // Each block adds an extra minute beyond the target
            let drift = i * 60; // Cumulative drift
            timestamps.push(base_time + i * 600 + drift);
            heights.push(i);
        }
        
        let result = adjuster.calculate_next_target(initial_target, &timestamps, &heights);
        
        // Should succeed but with bounded adjustment
        assert!(result.is_ok());
        let new_target = result.unwrap();
        
        // Verify the adjustment is bounded despite the drift
        let max_allowed_increase = (initial_target as f64 * 4.0) as u32;
        assert!(new_target <= max_allowed_increase);
    }
    
    #[test]
    fn test_backward_time_travel_attack() {
        // Test prevention of blocks with timestamps going backwards
        let validator = TimestampValidator::new();
        
        let timestamps = vec![1000, 2000, 3000, 4000];
        
        // Try to add a block that goes back in time
        let backward_timestamp = 3500; // Between blocks 3 and 4
        
        let result = validator.validate_timestamp(
            backward_timestamp,
            &timestamps,
            None,
        );
        
        // Should fail - timestamp not greater than median past time
        assert!(result.is_err());
    }
    
    #[test]
    fn test_rapid_block_timestamp_attack() {
        // Test prevention of unrealistically fast block times
        let validator = TimestampValidator::new();
        
        let mut timestamps = vec![];
        let mut heights = vec![];
        
        // Create blocks with timestamps only 5 seconds apart
        for i in 0..100 {
            timestamps.push(1000 + i * 5);
            heights.push(i);
        }
        
        let result = validator.validate_difficulty_timestamps(&timestamps, &heights, 2016);
        
        // Should fail - average block time too low
        assert!(result.is_err());
    }
    
    #[test]
    fn test_combined_attack_scenario() {
        // Test a sophisticated attack combining multiple techniques
        let mut config = DifficultyAdjustmentConfig::default();
        config.adjustment_interval = 20;
        config.validate_timestamps = true;
        
        let adjuster = DifficultyAdjustment::with_config(config);
        let initial_target = 0x1d00ffff;
        
        let mut timestamps = vec![];
        let mut heights = vec![];
        let base_time = current_time() - 12000;
        
        // Phase 1: Normal mining for first 10 blocks
        for i in 0..10 {
            timestamps.push(base_time + i * 600);
            heights.push(i);
        }
        
        // Phase 2: Try to manipulate next 10 blocks
        // - Some timestamps in future
        // - Some clustered together
        // - Overall trying to increase apparent block time
        
        // Clustered blocks
        for i in 10..13 {
            timestamps.push(base_time + 6000);
            heights.push(i);
        }
        
        // Future timestamps
        for i in 13..20 {
            let future_offset = (i - 13) * 1800; // 30 minutes per block
            timestamps.push(base_time + 6000 + future_offset);
            heights.push(i);
        }
        
        // Try to calculate new difficulty
        let result = adjuster.calculate_next_target(initial_target, &timestamps, &heights);
        
        // The sophisticated validation should catch this pattern
        if result.is_ok() {
            let new_target = result.unwrap();
            // Even if it passes, adjustment should be bounded
            assert!(new_target <= initial_target * 4);
        }
    }
    
    #[test]
    fn test_legitimate_network_time_variance() {
        // Test that legitimate timestamp variance is allowed
        let validator = TimestampValidator::new();
        
        let mut timestamps = vec![];
        let base_time = 1000000;
        
        // Create blocks with realistic variance (±2 minutes)
        let variance = vec![0, 60, -30, 45, -60, 90, -45, 30, -90, 120];
        
        for (i, &var) in variance.iter().enumerate() {
            let timestamp = base_time + (i as i64 * 600 + var) as u64;
            timestamps.push(timestamp);
        }
        
        // Sort to ensure median calculation works
        let mut sorted_timestamps = timestamps.clone();
        sorted_timestamps.sort_unstable();
        
        // Validate each timestamp
        for i in 1..timestamps.len() {
            let prior_timestamps = &sorted_timestamps[0..i];
            let result = validator.validate_timestamp(
                timestamps[i],
                prior_timestamps,
                None,
            );
            
            // Legitimate variance should be accepted
            if timestamps[i] > timestamps[i-1] {
                assert!(result.is_ok(), "Legitimate timestamp {} rejected", timestamps[i]);
            }
        }
    }
    
    #[test]
    fn test_recovery_from_time_warp_attempt() {
        // Test that the network can recover after a failed time-warp attempt
        let mut config = DifficultyAdjustmentConfig::default();
        config.adjustment_interval = 10;
        config.validate_timestamps = true;
        config.use_weighted_timespan = true;
        
        let adjuster = DifficultyAdjustment::with_config(config);
        let initial_target = 0x1d00ffff;
        
        let mut timestamps = vec![];
        let mut heights = vec![];
        let base_time = 1000000;
        
        // First adjustment period - include some outliers that will be filtered
        for i in 0..10 {
            if i == 5 {
                // Outlier - will be filtered by weighted calculation
                timestamps.push(base_time + i * 600 + 3600);
            } else {
                timestamps.push(base_time + i * 600);
            }
            heights.push(i);
        }
        
        let result = adjuster.calculate_next_target(initial_target, &timestamps, &heights);
        
        // Should succeed with weighted timespan filtering outliers
        assert!(result.is_ok());
        let new_target = result.unwrap();
        
        // Target adjustment should be modest despite outlier
        let expected_range = (initial_target as f64 * 0.9) as u32..(initial_target as f64 * 1.1) as u32;
        assert!(expected_range.contains(&new_target));
    }
} 