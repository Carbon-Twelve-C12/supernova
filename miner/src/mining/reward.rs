use super::{NOVA_BLOCK_REWARD, HALVING_INTERVAL, MAX_HALVINGS, ENV_BONUS_RENEWABLE, ENV_BONUS_EFFICIENCY, ENV_BONUS_MAX_TOTAL};

#[derive(Debug, Clone, PartialEq)]
pub struct EnvironmentalProfile {
    pub renewable_percentage: f64,  // 0.0 to 1.0
    pub efficiency_score: f64,      // 0.0 to 1.0
    pub verified: bool,
    pub rec_coverage: f64,          // Renewable Energy Certificate coverage
}

impl Default for EnvironmentalProfile {
    fn default() -> Self {
        Self {
            renewable_percentage: 0.0,
            efficiency_score: 0.0,
            verified: false,
            rec_coverage: 0.0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct MiningReward {
    pub base_reward: u64,
    pub environmental_bonus: u64,
    pub total_reward: u64,
}

/// Calculate the base block reward considering halvings
pub fn calculate_base_reward(block_height: u64) -> u64 {
    // Calculate how many halvings have occurred
    let halvings = (block_height / HALVING_INTERVAL) as u32;
    
    // Cap at maximum halvings to prevent underflow
    if halvings >= MAX_HALVINGS {
        return 0;
    }
    
    // Calculate reward: initial_reward / 2^halvings
    // Convert to satoshis (1 NOVA = 100_000_000 satoshis)
    let initial_reward_satoshis = NOVA_BLOCK_REWARD * 100_000_000;
    initial_reward_satoshis >> halvings
}

/// Calculate environmental bonus based on miner's environmental profile
pub fn calculate_environmental_bonus(base_reward: u64, profile: &EnvironmentalProfile) -> u64 {
    if !profile.verified {
        return 0;
    }
    
    let mut bonus_multiplier = 0.0;
    
    // Renewable energy bonus (up to 20%)
    if profile.renewable_percentage > 0.0 {
        bonus_multiplier += ENV_BONUS_RENEWABLE * profile.renewable_percentage;
    }
    
    // Efficiency bonus (up to 10%)
    if profile.efficiency_score > 0.0 {
        bonus_multiplier += ENV_BONUS_EFFICIENCY * profile.efficiency_score;
    }
    
    // Additional bonus for REC coverage
    if profile.rec_coverage > 0.0 {
        bonus_multiplier += 0.05 * profile.rec_coverage; // Up to 5% for full REC coverage
    }
    
    // Cap total bonus at maximum
    bonus_multiplier = bonus_multiplier.min(ENV_BONUS_MAX_TOTAL);
    
    (base_reward as f64 * bonus_multiplier) as u64
}

/// Calculate total mining reward including environmental bonuses
pub fn calculate_mining_reward(block_height: u64, environmental_profile: &EnvironmentalProfile) -> MiningReward {
    let base_reward = calculate_base_reward(block_height);
    let environmental_bonus = calculate_environmental_bonus(base_reward, environmental_profile);
    
    MiningReward {
        base_reward,
        environmental_bonus,
        total_reward: base_reward + environmental_bonus,
    }
}

/// Get the current subsidy era (which halving period we're in)
pub fn get_subsidy_era(block_height: u64) -> u32 {
    (block_height / HALVING_INTERVAL) as u32
}

/// Calculate total NOVA issued up to a given block height
pub fn calculate_total_issued(block_height: u64) -> u64 {
    let mut total = 0u64;
    let mut current_height = 0u64;
    let mut era = 0u32;
    
    while current_height < block_height && era < MAX_HALVINGS {
        let era_end = ((era + 1) as u64) * HALVING_INTERVAL;
        let blocks_in_era = if block_height < era_end {
            block_height - current_height
        } else {
            era_end - current_height
        };
        
        let reward_per_block = calculate_base_reward(current_height);
        total += blocks_in_era * reward_per_block;
        
        current_height = era_end;
        era += 1;
    }
    
    total
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_halving_schedule() {
        // Test initial reward
        assert_eq!(calculate_base_reward(0), 50 * 100_000_000);
        
        // Test first halving
        assert_eq!(calculate_base_reward(HALVING_INTERVAL), 25 * 100_000_000);
        
        // Test second halving
        assert_eq!(calculate_base_reward(HALVING_INTERVAL * 2), 12_50000000); // 12.5 NOVA
        
        // Test third halving
        assert_eq!(calculate_base_reward(HALVING_INTERVAL * 3), 6_25000000); // 6.25 NOVA
        
        // Test fourth halving
        assert_eq!(calculate_base_reward(HALVING_INTERVAL * 4), 3_12500000); // 3.125 NOVA
    }
    
    #[test]
    fn test_environmental_bonus() {
        let base_reward = 50 * 100_000_000; // 50 NOVA in satoshis
        
        // Test no bonus for unverified profile
        let unverified_profile = EnvironmentalProfile {
            renewable_percentage: 1.0,
            efficiency_score: 1.0,
            verified: false,
            rec_coverage: 1.0,
        };
        assert_eq!(calculate_environmental_bonus(base_reward, &unverified_profile), 0);
        
        // Test 20% renewable bonus
        let renewable_profile = EnvironmentalProfile {
            renewable_percentage: 1.0,
            efficiency_score: 0.0,
            verified: true,
            rec_coverage: 0.0,
        };
        assert_eq!(calculate_environmental_bonus(base_reward, &renewable_profile), 10 * 100_000_000); // 10 NOVA
        
        // Test combined bonuses
        let combined_profile = EnvironmentalProfile {
            renewable_percentage: 1.0,  // 20% bonus
            efficiency_score: 1.0,      // 10% bonus
            verified: true,
            rec_coverage: 1.0,          // 5% bonus
        };
        assert_eq!(calculate_environmental_bonus(base_reward, &combined_profile), 17_50000000); // 17.5 NOVA (35% bonus)
        
        // Test partial bonuses
        let partial_profile = EnvironmentalProfile {
            renewable_percentage: 0.5,  // 10% bonus
            efficiency_score: 0.5,      // 5% bonus
            verified: true,
            rec_coverage: 0.0,
        };
        assert_eq!(calculate_environmental_bonus(base_reward, &partial_profile), 7_50000000); // 7.5 NOVA (15% bonus)
    }
    
    #[test]
    fn test_total_mining_reward() {
        // Test at block 0 with full environmental bonus
        let env_profile = EnvironmentalProfile {
            renewable_percentage: 1.0,
            efficiency_score: 1.0,
            verified: true,
            rec_coverage: 1.0,
        };
        
        let reward = calculate_mining_reward(0, &env_profile);
        assert_eq!(reward.base_reward, 50 * 100_000_000);
        assert_eq!(reward.environmental_bonus, 17_50000000); // 35% bonus
        assert_eq!(reward.total_reward, 67_50000000); // 67.5 NOVA total
        
        // Test after first halving
        let reward_halved = calculate_mining_reward(HALVING_INTERVAL, &env_profile);
        assert_eq!(reward_halved.base_reward, 25 * 100_000_000);
        assert_eq!(reward_halved.environmental_bonus, 8_75000000); // 35% of 25 NOVA
        assert_eq!(reward_halved.total_reward, 33_75000000); // 33.75 NOVA total
    }
    
    #[test]
    fn test_subsidy_era() {
        assert_eq!(get_subsidy_era(0), 0);
        assert_eq!(get_subsidy_era(HALVING_INTERVAL - 1), 0);
        assert_eq!(get_subsidy_era(HALVING_INTERVAL), 1);
        assert_eq!(get_subsidy_era(HALVING_INTERVAL * 2), 2);
        assert_eq!(get_subsidy_era(HALVING_INTERVAL * 3), 3);
    }
    
    #[test]
    fn test_total_issued() {
        // Test first era
        let first_era_total = HALVING_INTERVAL * 50 * 100_000_000;
        assert_eq!(calculate_total_issued(HALVING_INTERVAL), first_era_total);
        
        // Test through second era
        let second_era_total = first_era_total + (HALVING_INTERVAL * 25 * 100_000_000);
        assert_eq!(calculate_total_issued(HALVING_INTERVAL * 2), second_era_total);
    }
} 