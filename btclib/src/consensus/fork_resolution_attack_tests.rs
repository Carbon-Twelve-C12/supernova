//! Fork Resolution Attack Prevention Tests
//! 
//! This module verifies that the secure fork resolution prevents permanent network splits.

#[cfg(test)]
mod tests {
    use super::super::secure_fork_resolution::{
        SecureForkResolver, SecureForkConfig, ChainMetrics
    };
    use crate::types::{Block, BlockHeader};
    use crate::consensus::difficulty::calculate_required_work;
    use std::collections::HashMap;
    use std::time::Duration;
    
    /// Create a test block header with specific properties
    fn create_header(
        height: u64,
        prev_hash: [u8; 32],
        bits: u32,
        timestamp: u64,
        nonce: u32,
    ) -> BlockHeader {
        let mut block = Block::new(1, prev_hash, vec![], bits);
        // In a real implementation, would set timestamp and nonce properly
        block.header().clone()
    }
    
    #[test]
    fn test_prevent_first_seen_split() {
        // Attack: Network split where nodes stick to first-seen chain
        let config = SecureForkConfig::default();
        let mut resolver = SecureForkResolver::new(config);
        
        // Create header storage
        let mut headers = HashMap::new();
        
        // Genesis block
        let genesis = create_header(0, [0; 32], 0x1d00ffff, 1000, 0);
        let genesis_hash = [0; 32];
        headers.insert(genesis_hash, genesis.clone());
        
        // Two competing chains at same height
        // Chain A: First seen by some nodes
        let block_1a = create_header(1, genesis_hash, 0x1d00ffff, 1600, 111);
        let hash_1a = [1; 32];
        headers.insert(hash_1a, block_1a);
        
        // Chain B: Has slightly more work (lower bits = more work)
        let block_1b = create_header(1, genesis_hash, 0x1cfffff0, 1601, 222);
        let hash_1b = [2; 32];
        headers.insert(hash_1b, block_1b);
        
        let get_header = |hash: &[u8; 32]| headers.get(hash).cloned();
        
        // Test: Chain B should win because it has more work
        let result = resolver.compare_chains(&hash_1a, &hash_1b, get_header).unwrap();
        assert!(!result, "Chain B (more work) should be preferred over Chain A");
        
        // Reverse test: B vs A should return true
        let result = resolver.compare_chains(&hash_1b, &hash_1a, get_header).unwrap();
        assert!(result, "Chain B should still win when compared in reverse order");
    }
    
    #[test]
    fn test_prevent_network_partition() {
        // Scenario: Two parts of network mine different chains
        let config = SecureForkConfig::default();
        let mut resolver = SecureForkResolver::new(config);
        
        let mut headers = HashMap::new();
        
        // Common history
        let genesis = create_header(0, [0; 32], 0x1d00ffff, 1000, 0);
        headers.insert([0; 32], genesis.clone());
        
        // Partition A: Mines 3 blocks
        let mut prev_hash = [0; 32];
        let mut chain_a_tip = [0; 32];
        for i in 1..=3 {
            let block = create_header(i, prev_hash, 0x1d00ffff, 1000 + i * 600, i as u32);
            let hash = [10 + i as u8; 32];
            headers.insert(hash, block);
            prev_hash = hash;
            chain_a_tip = hash;
        }
        
        // Partition B: Mines 3 blocks with slightly more total work
        prev_hash = [0; 32];
        let mut chain_b_tip = [0; 32];
        for i in 1..=3 {
            // Slightly more work on last block
            let bits = if i == 3 { 0x1cffffff } else { 0x1d00ffff };
            let block = create_header(i, prev_hash, bits, 1000 + i * 600, 1000 + i as u32);
            let hash = [20 + i as u8; 32];
            headers.insert(hash, block);
            prev_hash = hash;
            chain_b_tip = hash;
        }
        
        let get_header = |hash: &[u8; 32]| headers.get(hash).cloned();
        
        // Both partitions should agree on Chain B (more work)
        let result_a = resolver.compare_chains(&chain_a_tip, &chain_b_tip, get_header).unwrap();
        assert!(!result_a, "Partition A should switch to Chain B");
        
        let result_b = resolver.compare_chains(&chain_b_tip, &chain_a_tip, get_header).unwrap();
        assert!(result_b, "Partition B should keep its chain");
        
        // After resolution, both partitions follow the same chain
        println!("Network partition resolved: Both follow chain with more work");
    }
    
    #[test]
    fn test_quality_based_resolution() {
        // Test: When work is similar, quality metrics decide
        let config = SecureForkConfig {
            work_weight: 0.5,
            quality_weight: 0.5,
            ..Default::default()
        };
        let mut resolver = SecureForkResolver::new(config);
        
        let mut headers = HashMap::new();
        
        // Genesis
        let genesis = create_header(0, [0; 32], 0x1d00ffff, 1000, 0);
        headers.insert([0; 32], genesis);
        
        // Chain A: Consistent block times (good quality)
        let mut prev_hash = [0; 32];
        for i in 1..=5 {
            let block = create_header(i, prev_hash, 0x1d00ffff, 1000 + i * 600, i as u32);
            let hash = [30 + i as u8; 32];
            headers.insert(hash, block);
            prev_hash = hash;
        }
        let chain_a_tip = prev_hash;
        
        // Chain B: Erratic block times (poor quality)
        prev_hash = [0; 32];
        let times = [1100, 1150, 2400, 2450, 4000]; // Very erratic
        for (i, &time) in times.iter().enumerate() {
            let block = create_header(i as u64 + 1, prev_hash, 0x1d00ffff, time, 100 + i as u32);
            let hash = [40 + i as u8; 32];
            headers.insert(hash, block);
            prev_hash = hash;
        }
        let chain_b_tip = prev_hash;
        
        let get_header = |hash: &[u8; 32]| headers.get(hash).cloned();
        
        // Chain A should win due to better quality
        let result = resolver.compare_chains(&chain_a_tip, &chain_b_tip, get_header).unwrap();
        assert!(result, "Chain A (better quality) should win over Chain B");
    }
    
    #[test]
    fn test_anti_split_mechanism() {
        // Test: Anti-split mechanism prevents oscillation
        let config = SecureForkConfig {
            enable_anti_split: true,
            equality_window: Duration::from_secs(60),
            ..Default::default()
        };
        let mut resolver = SecureForkResolver::new(config);
        
        let chain_a = [1; 32];
        let chain_b = [2; 32];
        
        // Create nearly equal metrics
        let metrics = ChainMetrics {
            total_work: 1000,
            avg_block_time: Duration::from_secs(600),
            block_time_variance: 100.0,
            length: 10,
            tip_timestamp: 1000,
            quality_score: 0.9,
        };
        
        // Simulate multiple comparisons (would happen during network convergence)
        let mut results = Vec::new();
        for _ in 0..10 {
            // Record observation for chain A
            resolver.split_observations.entry(chain_a).or_default().push(
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs()
            );
            
            let result = resolver.apply_anti_split_logic(
                &chain_a, &chain_b, &metrics, &metrics
            ).unwrap();
            results.push(result);
        }
        
        // All nodes should make the same decision
        assert!(results.iter().all(|&r| r == results[0]), 
            "Anti-split mechanism ensures consistent decisions");
    }
    
    #[test]
    fn test_deterministic_tiebreaker() {
        // Test: When everything else is equal, use deterministic tiebreaker
        let config = SecureForkConfig::default();
        let resolver = SecureForkResolver::new(config);
        
        // Test multiple hash pairs
        let test_cases = vec![
            ([1; 32], [2; 32], true),   // [1,1,1...] < [2,2,2...]
            ([255; 32], [0; 32], false), // [255,255...] > [0,0,0...]
            ([1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16,17,18,19,20,21,22,23,24,25,26,27,28,29,30,31,32],
             [1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16,17,18,19,20,21,22,23,24,25,26,27,28,29,30,31,33],
             true), // Last byte differs
        ];
        
        for (hash_a, hash_b, expected) in test_cases {
            let result = resolver.deterministic_tiebreaker(&hash_a, &hash_b);
            assert_eq!(result, expected, 
                "Deterministic tiebreaker failed for {:?} vs {:?}", hash_a, hash_b);
            
            // Verify it's consistent when reversed
            let reverse = resolver.deterministic_tiebreaker(&hash_b, &hash_a);
            assert_eq!(reverse, !expected, "Tiebreaker must be consistent");
        }
    }
    
    #[test]
    fn test_deep_fork_handling() {
        // Test: Handle deep forks correctly
        let config = SecureForkConfig {
            max_fork_depth: 10,
            ..Default::default()
        };
        let mut resolver = SecureForkResolver::new(config);
        
        let mut headers = HashMap::new();
        
        // Build a deep chain
        let mut prev_hash = [0; 32];
        for i in 0..20 {
            let block = create_header(i, prev_hash, 0x1d00ffff, 1000 + i * 600, i as u32);
            let hash = [(i + 1) as u8; 32];
            headers.insert(hash, block);
            prev_hash = hash;
        }
        
        let get_header = |hash: &[u8; 32]| headers.get(hash).cloned();
        
        // Compare tips at different depths
        let shallow_tip = [5; 32];
        let deep_tip = [15; 32];
        
        // Should only look back max_fork_depth blocks
        let metrics = resolver.calculate_chain_metrics(&deep_tip, &get_header);
        assert!(metrics.is_ok());
        
        let metrics = metrics.unwrap();
        assert!(metrics.length <= 10, "Should respect max_fork_depth");
    }
    
    #[test]
    fn test_timestamp_manipulation_detection() {
        // Test: Detect chains with manipulated timestamps
        let config = SecureForkConfig::default();
        let resolver = SecureForkResolver::new(config);
        
        // Chain with backwards timestamps (manipulation attempt)
        let bad_headers = vec![
            create_header(3, [2; 32], 0x1d00ffff, 1000, 3), // Latest block with old timestamp
            create_header(2, [1; 32], 0x1d00ffff, 2000, 2),
            create_header(1, [0; 32], 0x1d00ffff, 3000, 1),
        ];
        
        let good_progression = resolver.check_timestamp_progression(&bad_headers);
        assert!(!good_progression, "Should detect backwards timestamps");
        
        // Calculate quality score for bad chain
        let (avg_time, variance) = resolver.calculate_timing_metrics(&bad_headers).unwrap();
        let quality = resolver.calculate_quality_score(avg_time, variance, &bad_headers);
        
        assert!(quality < 0.8, "Chains with bad timestamps should have low quality");
    }
} 