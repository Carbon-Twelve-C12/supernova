use super::*;
use super::reward::{calculate_mining_reward, EnvironmentalProfile};
use super::HALVING_INTERVAL;
use super::environmental_verification::{EnvironmentalVerifier, RECCertificate, EfficiencyAudit};
use crate::difficulty::{DifficultyAdjuster, DIFFICULTY_ADJUSTMENT_INTERVAL};

#[cfg(test)]
mod testnet_integration_tests {
    use super::*;
    use std::collections::HashMap;
    
    struct TestnetSimulator {
        current_height: u64,
        current_time: u64,
        difficulty_adjuster: DifficultyAdjuster,
        environmental_verifier: EnvironmentalVerifier,
        miner_profiles: HashMap<String, EnvironmentalProfile>,
        total_rewards_paid: u128,
    }
    
    impl TestnetSimulator {
        fn new() -> Self {
            Self {
                current_height: 0,
                current_time: 1_700_000_000, // Recent timestamp
                difficulty_adjuster: DifficultyAdjuster::new(0x1d00ffff),
                environmental_verifier: EnvironmentalVerifier::new(),
                miner_profiles: HashMap::new(),
                total_rewards_paid: 0,
            }
        }
        
        async fn setup_environmental_system(&mut self) {
            // Register trusted REC issuers
            self.environmental_verifier.register_trusted_issuer("Green-e".to_string()).await;
            self.environmental_verifier.register_trusted_issuer("I-REC".to_string()).await;
            self.environmental_verifier.register_trusted_issuer("TIGR".to_string()).await;
        }
        
        fn mine_block(&mut self, miner_id: &str) -> u64 {
            // Get miner's environmental profile
            let env_profile = self.miner_profiles.get(miner_id)
                .cloned()
                .unwrap_or_default();
            
            // Calculate reward
            let reward = calculate_mining_reward(self.current_height, &env_profile);
            
            // Update state
            self.current_height += 1;
            self.current_time += 150; // 2.5 minutes
            self.total_rewards_paid += reward.total_reward as u128;
            
            // Add timestamp to difficulty adjuster
            let _ = self.difficulty_adjuster.add_block_timestamp(self.current_time);
            
            reward.total_reward
        }
        
        async fn register_green_miner(&mut self, miner_id: &str, renewable_pct: f64, efficiency: f64) {
            // Create REC certificate
            let cert = RECCertificate {
                certificate_id: format!("REC-{}", miner_id),
                issuer: "Green-e".to_string(),
                coverage_mwh: renewable_pct * 150.0, // Scale to MWh
                valid_from: self.current_time - 3600,
                valid_until: self.current_time + 30 * 24 * 3600,
                verified: false,
            };
            
            self.environmental_verifier.register_rec_certificate(cert.clone()).await;
            
            // Create efficiency audit if needed
            let audit = if efficiency > 0.0 {
                Some(EfficiencyAudit {
                    auditor: "EnergyAuditor".to_string(),
                    hash_rate_per_watt: efficiency * 150.0,
                    cooling_efficiency: efficiency * 0.95,
                    overall_pue: 1.0 + (1.0 - efficiency) * 0.5,
                    audit_timestamp: self.current_time - 24 * 3600,
                })
            } else {
                None
            };
            
            // Verify and store profile
            let result = self.environmental_verifier.verify_miner_profile(
                miner_id.to_string(),
                EnvironmentalProfile::default(),
                vec![cert],
                audit,
            ).await;
            
            if let Ok(verified) = result {
                self.miner_profiles.insert(
                    miner_id.to_string(),
                    verified.environmental_profile,
                );
            }
        }
    }
    
    #[tokio::test]
    async fn test_testnet_launch_scenario() {
        let mut sim = TestnetSimulator::new();
        sim.setup_environmental_system().await;
        
        // Register different types of miners
        sim.register_green_miner("green_miner_1", 1.0, 0.8).await;  // 100% renewable, 80% efficient
        sim.register_green_miner("green_miner_2", 0.5, 0.5).await;  // 50% renewable, 50% efficient
        sim.register_green_miner("regular_miner", 0.0, 0.0).await;  // No environmental benefits
        
        // Simulate first day of mining (576 blocks)
        let mut rewards_by_miner = HashMap::new();
        let miners = vec!["green_miner_1", "green_miner_2", "regular_miner"];
        
        for i in 0..576 {
            let miner = miners[i % 3];
            let reward = sim.mine_block(miner);
            *rewards_by_miner.entry(miner).or_insert(0u64) += reward;
        }
        
        // Verify rewards follow expected pattern
        let green1_reward = rewards_by_miner["green_miner_1"];
        let green2_reward = rewards_by_miner["green_miner_2"];
        let regular_reward = rewards_by_miner["regular_miner"];
        
        // Green miner 1 should get ~35% more than regular
        assert!(green1_reward > regular_reward * 130 / 100, 
                "Full green miner should get significant bonus");
        
        // Green miner 2 should get moderate bonus
        assert!(green2_reward > regular_reward * 110 / 100,
                "Partial green miner should get moderate bonus");
        
        // Total daily emission should be close to expected
        let total_daily = sim.total_rewards_paid / 100_000_000;
        assert!(total_daily >= 28_000 && total_daily <= 32_000,
                "Daily emission should be ~28,800 NOVA + environmental bonuses");
    }
    
    #[tokio::test]
    async fn test_halving_transition() {
        let mut sim = TestnetSimulator::new();
        
        // Jump to just before first halving
        sim.current_height = HALVING_INTERVAL - 10;
        
        // Mine blocks across the halving boundary
        let mut pre_halving_rewards = Vec::new();
        let mut post_halving_rewards = Vec::new();
        
        for _ in 0..20 {
            let reward = sim.mine_block("miner");
            if sim.current_height <= HALVING_INTERVAL {
                pre_halving_rewards.push(reward);
            } else {
                post_halving_rewards.push(reward);
            }
        }
        
        // Verify halving occurred
        let avg_pre = pre_halving_rewards.iter().sum::<u64>() / pre_halving_rewards.len() as u64;
        let avg_post = post_halving_rewards.iter().sum::<u64>() / post_halving_rewards.len() as u64;
        
        assert_eq!(avg_pre / 100_000_000, 50, "Pre-halving should be 50 NOVA");
        assert_eq!(avg_post / 100_000_000, 25, "Post-halving should be 25 NOVA");
    }
    
    #[tokio::test]
    async fn test_environmental_certificate_expiry() {
        let mut sim = TestnetSimulator::new();
        sim.setup_environmental_system().await;
        
        // Register a green miner
        sim.register_green_miner("temp_green", 1.0, 0.0).await;
        
        // Mine some blocks with bonus
        let initial_reward = sim.mine_block("temp_green");
        assert!(initial_reward > 50 * 100_000_000, "Should get environmental bonus");
        
        // Simulate 31 days passing (certificate expires after 30 days)
        sim.current_time += 31 * 24 * 3600;
        
        // Get fresh profile (should be expired)
        let expired_profile = sim.environmental_verifier
            .get_verified_profile("temp_green")
            .await
            .unwrap_or_default();
        
        // Calculate reward with expired profile
        let expired_reward = calculate_mining_reward(sim.current_height, &expired_profile);
        assert_eq!(expired_reward.environmental_bonus, 0, "Expired profile should get no bonus");
    }
    
    #[tokio::test]
    async fn test_difficulty_adjustment_during_testnet() {
        let mut sim = TestnetSimulator::new();
        let initial_target = sim.difficulty_adjuster.get_current_target();
        
        // Simulate fast block production (1 minute blocks instead of 2.5)
        for _ in 0..DIFFICULTY_ADJUSTMENT_INTERVAL {
            sim.mine_block("fast_miner");
            sim.current_time -= 90; // Adjust time to simulate faster blocks
        }
        
        // Trigger difficulty adjustment
        let new_target = sim.difficulty_adjuster.adjust_difficulty(
            sim.current_height,
            sim.current_time,
            DIFFICULTY_ADJUSTMENT_INTERVAL
        ).unwrap();
        
        assert!(new_target < initial_target, "Difficulty should increase for fast blocks");
        assert!(new_target >= initial_target / 4, "Adjustment should be capped");
    }
    
    #[tokio::test]
    async fn test_stress_test_many_miners() {
        let mut sim = TestnetSimulator::new();
        sim.setup_environmental_system().await;
        
        // Register 100 miners with varying environmental profiles
        for i in 0..100 {
            let renewable = (i as f64 % 11.0) / 10.0; // 0.0 to 1.0
            let efficiency = ((i + 5) as f64 % 11.0) / 10.0; // 0.0 to 1.0
            sim.register_green_miner(&format!("miner_{}", i), renewable, efficiency).await;
        }
        
        // Mine 1000 blocks with random miner selection
        use rand::Rng;
        let mut rng = rand::thread_rng();
        
        for _ in 0..1000 {
            let miner_id = format!("miner_{}", rng.gen_range(0..100));
            sim.mine_block(&miner_id);
        }
        
        // Verify system stability
        assert_eq!(sim.current_height, 1000, "All blocks should be mined");
        assert!(sim.total_rewards_paid > 0, "Rewards should be paid");
        
        // Check reward distribution is reasonable
        let avg_reward_per_block = sim.total_rewards_paid / 1000;
        let expected_avg = 50 * 100_000_000 * 115 / 100; // ~15% average bonus
        assert!(avg_reward_per_block < expected_avg * 120 / 100,
                "Average reward should be reasonable");
    }
    
    #[tokio::test]
    async fn test_long_term_supply_projection() {
        let mut sim = TestnetSimulator::new();
        
        // Simulate 10 years of mining
        let blocks_per_year = (365.25 * 24.0 * 60.0 / 2.5) as u64;
        let target_blocks = blocks_per_year * 10;
        
        // Mine in chunks to speed up test
        let chunk_size = 10_000;
        for _ in 0..(target_blocks / chunk_size) {
            sim.current_height += chunk_size;
            let reward = calculate_mining_reward(sim.current_height, &EnvironmentalProfile::default());
            sim.total_rewards_paid += reward.total_reward as u128 * chunk_size as u128;
        }
        
        // Verify supply matches expectations
        let total_nova = sim.total_rewards_paid / 100_000_000;
        
        // After 10 years, should have mined significant portion of supply
        assert!(total_nova > 10_000_000, "Should have mined over 10M NOVA");
        assert!(total_nova < 15_000_000, "Should not exceed expected supply curve");
        
        // Verify we're in the correct halving era
        let era = (sim.current_height / HALVING_INTERVAL) as u32;
        assert_eq!(era, 2, "Should be in third era after ~10 years");
    }
} 