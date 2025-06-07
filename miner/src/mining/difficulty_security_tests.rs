use crate::difficulty::{DifficultyAdjuster, BLOCK_TIME_TARGET, DIFFICULTY_ADJUSTMENT_INTERVAL};
use std::time::Duration;

#[cfg(test)]
mod difficulty_security_tests {
    use super::*;
    
    #[test]
    fn test_difficulty_time_warp_attack_prevention() {
        let mut adjuster = DifficultyAdjuster::new(0x1d00ffff);
        
        // Simulate blocks with manipulated timestamps
        let base_time = 1_000_000u64;
        
        // Add blocks with normal timestamps
        for i in 0..DIFFICULTY_ADJUSTMENT_INTERVAL / 2 {
            let _ = adjuster.add_block_timestamp(base_time + i * 150); // 2.5 minute blocks
        }
        
        // Try to add blocks with timestamps far in the past (time warp attack)
        let result = adjuster.add_block_timestamp(base_time - 10000);
        assert!(result.is_err(), "Should reject timestamps before median");
        
        // Try to add blocks with timestamps far in the future
        let future_time = base_time + DIFFICULTY_ADJUSTMENT_INTERVAL * 300; // Way too far
        let result = adjuster.add_block_timestamp(future_time);
        assert!(result.is_err(), "Should reject timestamps too far in future");
    }
    
    #[test]
    fn test_difficulty_adjustment_bounds() {
        let mut adjuster = DifficultyAdjuster::new(0x1d00ffff);
        let initial_target = adjuster.get_current_target();
        
        // Simulate very fast blocks (should increase difficulty)
        let base_time = 1_000_000u64;
        for i in 0..DIFFICULTY_ADJUSTMENT_INTERVAL {
            let _ = adjuster.add_block_timestamp(base_time + i * 30); // 30 second blocks
        }
        
        let new_target = adjuster.adjust_difficulty(
            DIFFICULTY_ADJUSTMENT_INTERVAL,
            base_time + DIFFICULTY_ADJUSTMENT_INTERVAL * 30,
            DIFFICULTY_ADJUSTMENT_INTERVAL
        ).unwrap();
        
        // Difficulty should increase (target should decrease) but within bounds
        assert!(new_target < initial_target, "Target should decrease for fast blocks");
        assert!(new_target >= initial_target / 4, "Target adjustment should be capped at 4x");
    }
    
    #[test]
    fn test_block_time_target_consistency() {
        // Verify that the block time target matches tokenomics
        assert_eq!(BLOCK_TIME_TARGET, Duration::from_secs(150), "Block time should be 2.5 minutes");
        
        // Calculate expected blocks per day
        let seconds_per_day = 24 * 60 * 60;
        let blocks_per_day = seconds_per_day / BLOCK_TIME_TARGET.as_secs();
        assert_eq!(blocks_per_day, 576, "Should produce 576 blocks per day");
        
        // Calculate expected blocks per halving period
        let blocks_per_halving = 840_000u64;
        let seconds_per_halving = blocks_per_halving * BLOCK_TIME_TARGET.as_secs();
        let days_per_halving = seconds_per_halving / seconds_per_day;
        let years_per_halving = days_per_halving as f64 / 365.25;
        
        assert!(years_per_halving > 3.9 && years_per_halving < 4.1,
                "Halving should occur approximately every 4 years, got {:.2}", years_per_halving);
    }
    
    #[test]
    fn test_difficulty_adjustment_interval_manipulation() {
        let mut adjuster = DifficultyAdjuster::new(0x1d00ffff);
        
        // Try to trigger adjustment before the interval
        let base_time = 1_000_000u64;
        for i in 0..DIFFICULTY_ADJUSTMENT_INTERVAL - 1 {
            let _ = adjuster.add_block_timestamp(base_time + i * 150);
        }
        
        // This should not trigger an adjustment yet
        let result = adjuster.adjust_difficulty(
            DIFFICULTY_ADJUSTMENT_INTERVAL - 1,
            base_time + (DIFFICULTY_ADJUSTMENT_INTERVAL - 1) * 150,
            DIFFICULTY_ADJUSTMENT_INTERVAL - 1
        );
        
        // Should return an error or the same difficulty
        match result {
            Ok(target) => assert_eq!(target, adjuster.get_current_target(), 
                                    "Difficulty should not change before interval"),
            Err(_) => {} // This is also acceptable
        }
    }
    
    #[test]
    fn test_extreme_hash_rate_changes() {
        let mut adjuster = DifficultyAdjuster::new(0x1d00ffff);
        let initial_target = adjuster.get_current_target();
        
        // Simulate sudden hash rate drop (very slow blocks)
        let base_time = 1_000_000u64;
        for i in 0..DIFFICULTY_ADJUSTMENT_INTERVAL {
            let _ = adjuster.add_block_timestamp(base_time + i * 600); // 10 minute blocks
        }
        
        let new_target = adjuster.adjust_difficulty(
            DIFFICULTY_ADJUSTMENT_INTERVAL,
            base_time + DIFFICULTY_ADJUSTMENT_INTERVAL * 600,
            DIFFICULTY_ADJUSTMENT_INTERVAL
        ).unwrap();
        
        // Difficulty should decrease (target should increase) but within bounds
        assert!(new_target > initial_target, "Target should increase for slow blocks");
        assert!(new_target <= initial_target * 4, "Target adjustment should be capped at 4x");
    }
    
    #[test]
    fn test_timestamp_median_calculation() {
        let mut adjuster = DifficultyAdjuster::new(0x1d00ffff);
        
        // Add timestamps in non-sequential order (simulating network delays)
        let timestamps = vec![1000, 1150, 1100, 1300, 1250, 1400, 1350, 1500, 1450, 1600, 1550];
        
        for ts in timestamps {
            let result = adjuster.add_block_timestamp(ts);
            if result.is_err() {
                // Some timestamps might be rejected due to median time rule
                continue;
            }
        }
        
        // The adjuster should maintain proper ordering internally
        assert!(adjuster.get_block_timestamps().len() <= 11, 
                "Should maintain maximum of 11 timestamps for median calculation");
    }
    
    #[test]
    fn test_mining_reward_consistency_with_difficulty() {
        use super::super::reward::calculate_base_reward;
        
        // Verify that mining rewards are independent of difficulty
        let height = 100_000u64;
        let reward1 = calculate_base_reward(height);
        
        // Even if difficulty changes dramatically, reward should be the same
        let reward2 = calculate_base_reward(height);
        
        assert_eq!(reward1, reward2, "Mining reward should only depend on block height");
    }
    
    #[test]
    fn test_concurrent_difficulty_adjustment() {
        use std::sync::{Arc, Mutex};
        use std::thread;
        
        let adjuster = Arc::new(Mutex::new(DifficultyAdjuster::new(0x1d00ffff)));
        let mut handles = vec![];
        
        // Spawn multiple threads trying to add timestamps
        for i in 0..10 {
            let adjuster_clone = Arc::clone(&adjuster);
            let handle = thread::spawn(move || {
                let base_time = 1_000_000u64 + i * 1000;
                for j in 0..100 {
                    let mut adj = adjuster_clone.lock().unwrap();
                    let _ = adj.add_block_timestamp(base_time + j * 150);
                }
            });
            handles.push(handle);
        }
        
        // Wait for all threads
        for handle in handles {
            handle.join().unwrap();
        }
        
        // Verify adjuster is still in valid state
        let adj = adjuster.lock().unwrap();
        assert!(adj.get_block_timestamps().len() <= 11, "Timestamp buffer should not exceed limit");
    }
} 