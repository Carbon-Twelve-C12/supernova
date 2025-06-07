use super::reward::{calculate_base_reward, calculate_environmental_bonus, calculate_mining_reward, EnvironmentalProfile};
use super::{HALVING_INTERVAL, MAX_HALVINGS, ENV_BONUS_MAX_TOTAL};

#[cfg(test)]
mod security_tests {
    use super::*;

    // Test for integer overflow vulnerabilities
    #[test]
    fn test_reward_calculation_overflow_protection() {
        // Test at maximum block height
        let max_height = u64::MAX;
        let reward = calculate_base_reward(max_height);
        assert_eq!(reward, 0, "Reward should be 0 at maximum block height");
        
        // Test near halving boundaries
        for i in 0..10 {
            let height = HALVING_INTERVAL * i - 1;
            let reward1 = calculate_base_reward(height);
            let reward2 = calculate_base_reward(height + 1);
            assert!(reward2 <= reward1, "Reward should decrease or stay same after halving");
            assert!(reward1 > 0 || i >= MAX_HALVINGS as u64, "Reward should be positive before max halvings");
        }
    }

    #[test]
    fn test_environmental_bonus_manipulation() {
        let base_reward = 50 * 100_000_000; // 50 NOVA
        
        // Test maximum possible bonus
        let max_profile = EnvironmentalProfile {
            renewable_percentage: 1.0,
            efficiency_score: 1.0,
            verified: true,
            rec_coverage: 1.0,
        };
        let max_bonus = calculate_environmental_bonus(base_reward, &max_profile);
        let max_expected = (base_reward as f64 * 0.35) as u64; // 35% max bonus
        assert_eq!(max_bonus, max_expected, "Maximum bonus should be capped at 35%");
        
        // Test bonus cannot exceed cap even with manipulated values
        let exploit_profile = EnvironmentalProfile {
            renewable_percentage: 10.0, // Attempting to exploit with high values
            efficiency_score: 10.0,
            verified: true,
            rec_coverage: 10.0,
        };
        let exploit_bonus = calculate_environmental_bonus(base_reward, &exploit_profile);
        assert!(exploit_bonus <= (base_reward as f64 * ENV_BONUS_MAX_TOTAL) as u64,
                "Bonus should be capped even with exploited values");
    }

    #[test]
    fn test_unverified_profile_gets_no_bonus() {
        let base_reward = 50 * 100_000_000;
        
        // Even with perfect scores, unverified profile gets no bonus
        let unverified = EnvironmentalProfile {
            renewable_percentage: 1.0,
            efficiency_score: 1.0,
            verified: false,
            rec_coverage: 1.0,
        };
        let bonus = calculate_environmental_bonus(base_reward, &unverified);
        assert_eq!(bonus, 0, "Unverified profiles should receive no bonus");
    }

    #[test]
    fn test_negative_environmental_values() {
        let base_reward = 50 * 100_000_000;
        
        // Test negative values don't cause issues
        let negative_profile = EnvironmentalProfile {
            renewable_percentage: -1.0,
            efficiency_score: -1.0,
            verified: true,
            rec_coverage: -1.0,
        };
        let bonus = calculate_environmental_bonus(base_reward, &negative_profile);
        assert_eq!(bonus, 0, "Negative values should result in 0 bonus");
    }

    #[test]
    fn test_halving_boundary_conditions() {
        // Test exact halving boundaries
        let boundaries = vec![
            (HALVING_INTERVAL - 1, 50 * 100_000_000),
            (HALVING_INTERVAL, 25 * 100_000_000),
            (HALVING_INTERVAL * 2 - 1, 25 * 100_000_000),
            (HALVING_INTERVAL * 2, 12_50000000),
        ];
        
        for (height, expected) in boundaries {
            let reward = calculate_base_reward(height);
            assert_eq!(reward, expected, 
                      "Reward at height {} should be {}", height, expected);
        }
    }

    #[test]
    fn test_total_supply_never_exceeds_limit() {
        // Calculate total possible supply
        let mut total_supply = 0u128; // Use u128 to avoid overflow
        let mut height = 0u64;
        
        while height < HALVING_INTERVAL * MAX_HALVINGS as u64 {
            let reward = calculate_base_reward(height) as u128;
            total_supply += reward;
            height += 1;
            
            // Early exit if reward becomes 0
            if reward == 0 {
                break;
            }
        }
        
        let total_nova = total_supply / 100_000_000;
        assert!(total_nova <= 21_000_000, 
                "Total supply {} NOVA should never exceed 21,000,000 NOVA", total_nova);
    }

    #[test]
    fn test_block_time_manipulation_resistance() {
        // Verify that rewards are based on block height, not time
        let height = 1000;
        let reward1 = calculate_base_reward(height);
        
        // Simulate time passing without blocks
        std::thread::sleep(std::time::Duration::from_millis(100));
        
        let reward2 = calculate_base_reward(height);
        assert_eq!(reward1, reward2, "Rewards should be deterministic based on height");
    }

    #[test]
    fn test_environmental_bonus_precision() {
        let base_reward = 50 * 100_000_000;
        
        // Test various percentage combinations
        let test_cases = vec![
            (0.5, 0.5, 0.5, 7_50000000 + 2_50000000 + 1_25000000), // 15% + 2.5% = 17.5%
            (0.33, 0.0, 0.0, 3_30000000), // 6.6%
            (0.0, 0.75, 0.0, 3_75000000), // 7.5%
            (0.1, 0.1, 0.1, 2_00000000 + 50000000 + 25000000), // 2.75%
        ];
        
        for (renewable, efficiency, rec, expected) in test_cases {
            let profile = EnvironmentalProfile {
                renewable_percentage: renewable,
                efficiency_score: efficiency,
                verified: true,
                rec_coverage: rec,
            };
            let bonus = calculate_environmental_bonus(base_reward, &profile);
            // Allow small rounding differences
            assert!((bonus as i64 - expected as i64).abs() < 100000, 
                    "Bonus calculation precision error: got {}, expected {}", bonus, expected);
        }
    }

    #[test]
    fn test_concurrent_reward_calculation() {
        use std::sync::Arc;
        use std::thread;
        
        let height = 1000;
        let profile = Arc::new(EnvironmentalProfile {
            renewable_percentage: 0.5,
            efficiency_score: 0.5,
            verified: true,
            rec_coverage: 0.5,
        });
        
        let mut handles = vec![];
        
        // Spawn multiple threads calculating rewards simultaneously
        for _ in 0..10 {
            let profile_clone = Arc::clone(&profile);
            let handle = thread::spawn(move || {
                calculate_mining_reward(height, &profile_clone)
            });
            handles.push(handle);
        }
        
        // Collect results
        let mut results = vec![];
        for handle in handles {
            results.push(handle.join().unwrap());
        }
        
        // All results should be identical
        let first = &results[0];
        for result in &results {
            assert_eq!(result.total_reward, first.total_reward, 
                      "Concurrent calculations should yield identical results");
        }
    }

    #[test]
    fn test_reward_after_all_halvings() {
        // Test behavior after all halvings are exhausted
        let very_high_block = HALVING_INTERVAL * (MAX_HALVINGS as u64 + 10);
        let reward = calculate_base_reward(very_high_block);
        assert_eq!(reward, 0, "Reward should be 0 after all halvings");
        
        // Ensure environmental bonuses don't apply to 0 reward
        let profile = EnvironmentalProfile {
            renewable_percentage: 1.0,
            efficiency_score: 1.0,
            verified: true,
            rec_coverage: 1.0,
        };
        let mining_reward = calculate_mining_reward(very_high_block, &profile);
        assert_eq!(mining_reward.total_reward, 0, "No rewards should be given after all halvings");
    }
} 