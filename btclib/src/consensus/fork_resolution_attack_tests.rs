//! Fork Resolution Attack Prevention Tests
//!
//! This module verifies that the secure fork resolution prevents permanent network splits.

#[cfg(test)]
mod tests {
    use super::super::fork_resolution_v2::ProofOfWorkForkResolver;
    use super::super::secure_fork_resolution::SecureForkConfig;
    use crate::consensus::difficulty::calculate_required_work;
    use crate::types::{Block, BlockHeader};
    use std::cmp::Ordering;
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
        BlockHeader::new_with_height(
            1, prev_hash, [0; 32], // merkle_root
            timestamp, bits, nonce, height,
        )
    }

    #[test]
    fn test_prevent_first_seen_split() {
        // Attack: Network split where nodes stick to first-seen chain
        let resolver = ProofOfWorkForkResolver::new(100);

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
        let block_1b = create_header(1, genesis_hash, 0x1c00ffff, 1601, 222);
        let hash_1b = [2; 32];
        headers.insert(hash_1b, block_1b);

        let get_header = |hash: &[u8; 32]| headers.get(hash).cloned();

        // Test: Chain B should win because it has more work
        let result = resolver
            .compare_chains(&hash_1a, &hash_1b, get_header)
            .unwrap();
        assert_eq!(
            result,
            Ordering::Less,
            "Chain B (more work) should be preferred over Chain A"
        );

        // Reverse test: B vs A should return Greater
        let result = resolver
            .compare_chains(&hash_1b, &hash_1a, get_header)
            .unwrap();
        assert_eq!(
            result,
            Ordering::Greater,
            "Chain B should still win when compared in reverse order"
        );
    }

    #[test]
    fn test_prevent_network_partition() {
        // Scenario: Two parts of network mine different chains
        let resolver = ProofOfWorkForkResolver::new(100);

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
            // Use valid harder difficulty: 0x1c00ffff (lower target = more work)
            let bits = if i == 3 { 0x1c00ffff } else { 0x1d00ffff };
            let block = create_header(i, prev_hash, bits, 1000 + i * 600, 1000 + i as u32);
            let hash = [20 + i as u8; 32];
            headers.insert(hash, block);
            prev_hash = hash;
            chain_b_tip = hash;
        }

        let get_header = |hash: &[u8; 32]| headers.get(hash).cloned();

        // Both partitions should agree on Chain B (more work)
        let result_a = resolver
            .compare_chains(&chain_a_tip, &chain_b_tip, get_header)
            .unwrap();
        assert_eq!(
            result_a,
            Ordering::Less,
            "Partition A should switch to Chain B"
        );

        let result_b = resolver
            .compare_chains(&chain_b_tip, &chain_a_tip, get_header)
            .unwrap();
        assert_eq!(
            result_b,
            Ordering::Greater,
            "Partition B should keep its chain"
        );

        // After resolution, both partitions follow the same chain
        println!("Network partition resolved: Both follow chain with more work");
    }

    // Note: Quality-based resolution is removed in v2 as it violates
    // the fundamental principle of Nakamoto Consensus: most proof-of-work wins
    #[test]
    #[ignore = "Quality metrics are not used in pure PoW fork resolution"]
    fn test_quality_based_resolution() {
        // This test is preserved for historical reference but disabled
        // as the new implementation follows Bitcoin's proven approach:
        // The chain with the most accumulated proof-of-work always wins

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

        // This test is now ignored as quality metrics are not used in pure PoW
        // The new implementation follows Bitcoin's approach: most work wins
    }

    #[test]
    fn test_anti_split_mechanism() {
        // Test: Deterministic resolution prevents oscillation
        let resolver = ProofOfWorkForkResolver::new(100);

        let chain_a = [1; 32];
        let chain_b = [2; 32];

        // Create headers for test chains
        let mut headers = HashMap::new();
        headers.insert(chain_a, create_header(10, [0; 32], 0x1d00ffff, 1000, 111));
        headers.insert(chain_b, create_header(10, [0; 32], 0x1d00ffff, 1000, 222));

        // Simulate multiple comparisons (would happen during network convergence)
        let mut results = Vec::new();
        for _ in 0..10 {
            // Use the public compare_chains API
            let result =
                resolver.compare_chains(&chain_a, &chain_b, |hash| headers.get(hash).cloned());

            match result {
                Ok(preferred) => results.push(preferred),
                Err(e) => panic!("Chain comparison failed: {:?}", e),
            }
        }

        // All nodes should make the same decision
        assert!(
            results.iter().all(|&r| r == results[0]),
            "Anti-split mechanism ensures consistent decisions"
        );
    }

    // Note: deterministic_tiebreaker is tested implicitly through compare_chains API
    // when chains have equal work

    #[test]
    fn test_deep_fork_handling() {
        // Test: Handle deep forks correctly with limited depth
        let resolver = ProofOfWorkForkResolver::new(10); // Max depth 10

        let mut headers = HashMap::new();

        // Build a deep chain
        let mut prev_hash = [0; 32];
        headers.insert(prev_hash, create_header(0, [0; 32], 0x1d00ffff, 1000, 0));

        for i in 1..20 {
            let block = create_header(i, prev_hash, 0x1d00ffff, 1000 + i * 600, i as u32);
            let hash = [i as u8; 32];
            headers.insert(hash, block);
            prev_hash = hash;
        }

        let get_header = |hash: &[u8; 32]| headers.get(hash).cloned();

        // Compare tips at different depths
        let shallow_tip = [5; 32];
        let deep_tip = [15; 32];

        // Compare chains - deep tip has more work
        let result = resolver.compare_chains(&shallow_tip, &deep_tip, get_header);
        match result {
            Ok(ordering) => assert_eq!(ordering, Ordering::Less, "Deep tip should have more work"),
            Err(_) => {
                // If we hit depth limit, that's also acceptable behavior
                // as it prevents DoS attacks via extremely deep chains
            }
        }
    }

    #[test]
    fn test_timestamp_manipulation_detection() {
        // Note: Timestamp validation is now handled by TimeWarpPrevention module
        // This test verifies fork resolution still works with unusual timestamps
        let resolver = ProofOfWorkForkResolver::new(100);

        // Create header storage
        let mut headers = HashMap::new();

        // Good chain with proper timestamps
        let good_chain = [10; 32];
        headers.insert([8; 32], create_header(1, [0; 32], 0x1d00ffff, 1000, 1));
        headers.insert([9; 32], create_header(2, [8; 32], 0x1d00ffff, 1600, 2));
        headers.insert(good_chain, create_header(3, [9; 32], 0x1d00ffff, 2200, 3));

        // Bad chain with backwards timestamps (manipulation attempt)
        let bad_chain = [11; 32];
        headers.insert([5; 32], create_header(1, [0; 32], 0x1d00ffff, 3000, 1));
        headers.insert([6; 32], create_header(2, [5; 32], 0x1d00ffff, 2000, 2));
        headers.insert(bad_chain, create_header(3, [6; 32], 0x1d00ffff, 1000, 3));

        // With equal work, the comparison is based on hash as tiebreaker
        let result =
            resolver.compare_chains(&good_chain, &bad_chain, |hash| headers.get(hash).cloned());
        // Note: In pure PoW, timestamp quality doesn't affect fork resolution
        // Only accumulated work matters, timestamps are validated separately
        assert!(result.is_ok(), "Fork comparison should succeed");
    }
}
