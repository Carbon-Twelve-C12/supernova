use super::reward::{calculate_base_reward, calculate_total_issued, get_subsidy_era};
use super::HALVING_INTERVAL;

#[cfg(test)]
mod halving_tests {
    use super::*;

    #[test]
    fn test_halving_schedule_matches_tokenomics() {
        // Test the halving schedule from the tokenomics document
        let test_cases = vec![
            (0, 50.0),                              // Year 1-4: 50 NOVA
            (HALVING_INTERVAL, 25.0),               // Year 5-8: 25 NOVA
            (HALVING_INTERVAL * 2, 12.5),           // Year 9-12: 12.5 NOVA
            (HALVING_INTERVAL * 3, 6.25),           // Year 13-16: 6.25 NOVA
            (HALVING_INTERVAL * 4, 3.125),          // Year 17-20: 3.125 NOVA
            (HALVING_INTERVAL * 5, 1.5625),         // Year 21-24: 1.5625 NOVA
            (HALVING_INTERVAL * 6, 0.78125),        // Year 25-28: 0.78125 NOVA
        ];
        
        for (block_height, expected_nova) in test_cases {
            let reward_satoshis = calculate_base_reward(block_height);
            let reward_nova = reward_satoshis as f64 / 100_000_000.0;
            assert_eq!(
                reward_nova, expected_nova,
                "Block {} should have reward {} NOVA, got {} NOVA",
                block_height, expected_nova, reward_nova
            );
        }
    }
    
    #[test]
    fn test_total_supply_distribution() {
        // Test that total mining rewards match tokenomics (21,000,000 NOVA)
        let blocks_per_era = HALVING_INTERVAL;
        let mut total_nova = 0.0;
        
        for era in 0..10 {
            let block_at_era = era * blocks_per_era;
            let reward_satoshis = calculate_base_reward(block_at_era);
            let reward_nova = reward_satoshis as f64 / 100_000_000.0;
            let era_total = reward_nova * blocks_per_era as f64;
            total_nova += era_total;
            
            println!("Era {}: {} NOVA per block, {} NOVA total for era", 
                     era, reward_nova, era_total);
        }
        
        // Should be close to 21,000,000 NOVA (within rounding)
        assert!(total_nova > 20_000_000.0 && total_nova < 21_000_000.0,
                "Total mining rewards should be ~21M NOVA, got {}", total_nova);
    }
    
    #[test]
    fn test_subsidy_era_calculation() {
        assert_eq!(get_subsidy_era(0), 0);
        assert_eq!(get_subsidy_era(839_999), 0);
        assert_eq!(get_subsidy_era(840_000), 1);
        assert_eq!(get_subsidy_era(1_679_999), 1);
        assert_eq!(get_subsidy_era(1_680_000), 2);
    }
    
    #[test]
    fn test_block_time_calculations() {
        // With 2.5 minute blocks and 840,000 blocks per halving
        let minutes_per_halving = 840_000 * 2.5;
        let hours_per_halving = minutes_per_halving / 60.0;
        let days_per_halving = hours_per_halving / 24.0;
        let years_per_halving = days_per_halving / 365.25;
        
        println!("Halving interval: {} blocks", HALVING_INTERVAL);
        println!("Minutes per halving: {}", minutes_per_halving);
        println!("Days per halving: {:.2}", days_per_halving);
        println!("Years per halving: {:.2}", years_per_halving);
        
        // Should be approximately 4 years
        assert!(years_per_halving > 3.9 && years_per_halving < 4.1,
                "Halving should occur approximately every 4 years, got {:.2} years", 
                years_per_halving);
    }
    
    #[test]
    fn test_daily_emission_rate() {
        // Test daily emission rates from tokenomics
        let blocks_per_day = 24 * 60 / 2.5; // 576 blocks per day with 2.5 minute blocks
        
        let test_cases = vec![
            (0, 28_800.0),           // Year 1-4: ~28,800 NOVA/day
            (HALVING_INTERVAL, 14_400.0),     // Year 5-8: ~14,400 NOVA/day
            (HALVING_INTERVAL * 2, 7_200.0), // Year 9-12: ~7,200 NOVA/day
            (HALVING_INTERVAL * 3, 3_600.0), // Year 13-16: ~3,600 NOVA/day
            (HALVING_INTERVAL * 4, 1_800.0), // Year 17-20: ~1,800 NOVA/day
        ];
        
        for (block_height, expected_daily) in test_cases {
            let reward_per_block = calculate_base_reward(block_height) as f64 / 100_000_000.0;
            let daily_emission = reward_per_block * blocks_per_day;
            
            // Allow 1% tolerance for rounding
            let tolerance = expected_daily * 0.01;
            assert!(
                (daily_emission - expected_daily).abs() < tolerance,
                "At block {}, daily emission should be ~{} NOVA, got {:.2} NOVA",
                block_height, expected_daily, daily_emission
            );
        }
    }
    
    #[test]
    fn test_cumulative_supply_over_time() {
        // Test cumulative supply matches tokenomics projections
        let blocks_per_year = (365.25 * 24 * 60 / 2.5) as u64;
        
        let test_cases = vec![
            (blocks_per_year * 4, 8_400_000),      // After 4 years
            (blocks_per_year * 8, 12_600_000),     // After 8 years
            (blocks_per_year * 12, 14_700_000),    // After 12 years
            (blocks_per_year * 16, 15_750_000),    // After 16 years
        ];
        
        for (block_height, expected_supply_nova) in test_cases {
            let total_satoshis = calculate_total_issued(block_height);
            let total_nova = total_satoshis / 100_000_000;
            
            // Allow 5% tolerance due to block time variations
            let tolerance = expected_supply_nova / 20;
            assert!(
                total_nova >= expected_supply_nova - tolerance && 
                total_nova <= expected_supply_nova + tolerance,
                "At block {} (year {:.1}), total supply should be ~{} NOVA, got {} NOVA",
                block_height, 
                block_height as f64 / blocks_per_year as f64,
                expected_supply_nova, 
                total_nova
            );
        }
    }
} 