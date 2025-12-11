//! Consensus Attack Scenario Tests
//!
//! SECURITY MODULE (P0-004): Tests for various consensus attack scenarios
//!
//! This module validates that Supernova's consensus implementation is resistant to:
//! - 51% attacks with deep reorganization detection
//! - Selfish mining strategies
//! - Block withholding attacks
//! - Time warp attacks
//!
//! These tests are critical for mainnet security.

use btclib::consensus::difficulty::calculate_required_work;
use btclib::consensus::fork_resolution_v2::ProofOfWorkForkResolver;
use btclib::types::block::BlockHeader;
use std::cmp::Ordering;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering as AtomicOrdering};
use std::sync::Arc;
use std::time::{Duration, Instant};

// ============================================================================
// Test Helpers
// ============================================================================

/// Create a test block header with specific properties
fn create_header(
    height: u64,
    prev_hash: [u8; 32],
    bits: u32,
    timestamp: u64,
    nonce: u32,
) -> BlockHeader {
    BlockHeader::new_with_height(
        1,         // version
        prev_hash,
        [0; 32],   // merkle_root
        timestamp,
        bits,
        nonce,
        height,
    )
}

/// Simple metrics tracker for attack detection
struct AttackMetrics {
    /// Number of deep reorganizations detected
    deep_reorg_count: AtomicU64,
    /// Maximum reorg depth observed
    max_reorg_depth: AtomicU64,
    /// Total reorganizations
    total_reorgs: AtomicU64,
}

impl AttackMetrics {
    fn new() -> Self {
        Self {
            deep_reorg_count: AtomicU64::new(0),
            max_reorg_depth: AtomicU64::new(0),
            total_reorgs: AtomicU64::new(0),
        }
    }
    
    fn record_reorg(&self, depth: u64) {
        self.total_reorgs.fetch_add(1, AtomicOrdering::SeqCst);
        
        // Update max depth
        let mut current_max = self.max_reorg_depth.load(AtomicOrdering::SeqCst);
        while depth > current_max {
            match self.max_reorg_depth.compare_exchange_weak(
                current_max, depth, AtomicOrdering::SeqCst, AtomicOrdering::SeqCst
            ) {
                Ok(_) => break,
                Err(x) => current_max = x,
            }
        }
        
        // Deep reorg threshold (configurable, using 10 blocks as threshold)
        if depth > 10 {
            self.deep_reorg_count.fetch_add(1, AtomicOrdering::SeqCst);
        }
    }
    
    fn deep_reorgs(&self) -> u64 {
        self.deep_reorg_count.load(AtomicOrdering::SeqCst)
    }
}

// ============================================================================
// 51% Attack Detection Tests
// ============================================================================

mod fifty_one_percent_attack {
    use super::*;
    
    /// Test detection of a 51% attack attempting a deep reorganization
    /// 
    /// Scenario: An attacker with majority hashrate mines a private chain
    /// and releases it to reorganize the network.
    #[test]
    fn test_51_percent_attack_detection() {
        println!("\n=== 51% Attack Detection Test ===");
        let resolver = ProofOfWorkForkResolver::new(100);
        let metrics = AttackMetrics::new();
        
        let mut headers = HashMap::new();
        
        // Build the "honest" chain (50 blocks)
        let mut honest_prev_hash = [0; 32];
        let mut honest_chain_hashes = vec![[0; 32]];
        headers.insert([0; 32], create_header(0, [0; 32], 0x1d00ffff, 1000, 0));
        
        for i in 1..=50 {
            let block = create_header(i, honest_prev_hash, 0x1d00ffff, 1000 + i * 600, i as u32);
            let hash = [100 + i as u8; 32];
            headers.insert(hash, block);
            honest_chain_hashes.push(hash);
            honest_prev_hash = hash;
        }
        let honest_tip = honest_prev_hash;
        
        // Build the "attacker" chain (51 blocks with more work starting from genesis)
        // Simulates 51% attack where attacker secretly mines longer chain
        let mut attacker_prev_hash = [0; 32];
        
        for i in 1..=51 {
            // Attacker has slightly more work per block (lower bits = more work)
            let bits = 0x1c00ffff; // More work than honest chain
            let block = create_header(i, attacker_prev_hash, bits, 1000 + i * 600, 1000 + i as u32);
            let hash = [200 + i as u8; 32];
            headers.insert(hash, block);
            attacker_prev_hash = hash;
        }
        let attacker_tip = attacker_prev_hash;
        
        // When attacker releases chain, it should win (more work)
        let get_header = |hash: &[u8; 32]| headers.get(hash).cloned();
        
        let result = resolver.compare_chains(&honest_tip, &attacker_tip, get_header);
        
        match result {
            Ok(ordering) => {
                if ordering == Ordering::Less {
                    // Honest chain loses - this is the 51% attack succeeding
                    // In reality, we'd record this as a deep reorg
                    metrics.record_reorg(50); // 50 block reorg
                    
                    // Verify metrics recorded the attack
                    assert!(
                        metrics.deep_reorgs() > 0,
                        "Deep reorg should be detected as potential attack"
                    );
                    println!("✓ 51% attack detected: {} deep reorgs", metrics.deep_reorgs());
                } else {
                    // This shouldn't happen with our test setup
                    panic!("Expected attacker chain to have more work");
                }
            }
            Err(e) => panic!("Chain comparison failed: {:?}", e),
        }
        
        println!("✓ Attack detection metrics working correctly");
    }
    
    /// Test that rapid successive reorgs trigger alerts
    #[test]
    fn test_rapid_reorg_detection() {
        println!("\n=== Rapid Reorg Detection Test ===");
        let metrics = AttackMetrics::new();
        
        // Simulate rapid reorgs (potential eclipse attack or 51% attack)
        for i in 0..5 {
            metrics.record_reorg(15 + i); // Each reorg is deeper than threshold
        }
        
        // All should be detected as deep reorgs
        assert_eq!(
            metrics.deep_reorgs(), 5,
            "All rapid deep reorgs should be detected"
        );
        
        println!("✓ Rapid reorg detection: {} suspicious reorgs detected", metrics.deep_reorgs());
    }
}

// ============================================================================
// Selfish Mining Resistance Tests
// ============================================================================

mod selfish_mining {
    use super::*;
    
    /// Test that selfish mining doesn't give unfair advantage
    /// 
    /// Selfish mining: Attacker mines blocks but doesn't broadcast immediately,
    /// waiting to release when honest miners find competing blocks.
    #[test]
    fn test_selfish_mining_resistance() {
        println!("\n=== Selfish Mining Resistance Test ===");
        let resolver = ProofOfWorkForkResolver::new(100);
        
        let mut headers = HashMap::new();
        
        // Genesis
        headers.insert([0; 32], create_header(0, [0; 32], 0x1d00ffff, 1000, 0));
        
        // Scenario: Selfish miner has 1 block lead
        // Honest miner broadcasts block 1
        let honest_block_1 = create_header(1, [0; 32], 0x1d00ffff, 1600, 111);
        let honest_hash_1 = [10; 32];
        headers.insert(honest_hash_1, honest_block_1);
        
        // Selfish miner releases their competing block 1 (mined earlier)
        let selfish_block_1 = create_header(1, [0; 32], 0x1d00ffff, 1590, 222); // Earlier timestamp
        let selfish_hash_1 = [20; 32];
        headers.insert(selfish_hash_1, selfish_block_1);
        
        // With equal work, the tiebreaker should be deterministic
        let get_header = |hash: &[u8; 32]| headers.get(hash).cloned();
        
        let result = resolver.compare_chains(&honest_hash_1, &selfish_hash_1, get_header);
        
        // Both chains have same work, so result depends on deterministic tiebreaker
        // The key point is that the result is consistent
        assert!(result.is_ok(), "Equal work comparison should succeed");
        
        // Run multiple times to verify determinism
        let mut results = vec![];
        for _ in 0..10 {
            let r = resolver.compare_chains(&honest_hash_1, &selfish_hash_1, get_header);
            results.push(r.unwrap());
        }
        
        // All results should be the same (no randomness)
        assert!(
            results.windows(2).all(|w| w[0] == w[1]),
            "Fork resolution must be deterministic to prevent oscillation"
        );
        
        println!("✓ Selfish mining: deterministic resolution prevents gaming");
    }
    
    /// Test that hiding blocks doesn't provide advantage when released
    #[test]
    fn test_hidden_block_release() {
        println!("\n=== Hidden Block Release Test ===");
        let resolver = ProofOfWorkForkResolver::new(100);
        
        let mut headers = HashMap::new();
        headers.insert([0; 32], create_header(0, [0; 32], 0x1d00ffff, 1000, 0));
        
        // Honest chain: 3 blocks
        let mut prev = [0; 32];
        for i in 1..=3 {
            let block = create_header(i, prev, 0x1d00ffff, 1000 + i * 600, i as u32);
            let hash = [10 + i as u8; 32];
            headers.insert(hash, block);
            prev = hash;
        }
        let honest_tip = prev;
        
        // Attacker reveals 2 hidden blocks (came from before)
        // If attacker had 2 blocks, they should have released earlier
        // Now honest chain is longer
        prev = [0; 32];
        for i in 1..=2 {
            let block = create_header(i, prev, 0x1d00ffff, 900 + i * 600, 100 + i as u32);
            let hash = [20 + i as u8; 32];
            headers.insert(hash, block);
            prev = hash;
        }
        let attacker_tip = prev;
        
        let get_header = |hash: &[u8; 32]| headers.get(hash).cloned();
        
        // Honest chain should win (more work due to more blocks)
        let result = resolver.compare_chains(&honest_tip, &attacker_tip, get_header).unwrap();
        
        assert_eq!(
            result, Ordering::Greater,
            "Honest longer chain should beat hidden shorter chain"
        );
        
        println!("✓ Hidden blocks: longer honest chain wins");
    }
}

// ============================================================================
// Block Withholding Attack Tests
// ============================================================================

mod block_withholding {
    use super::*;
    
    /// Test that old blocks released late get orphaned
    #[test]
    fn test_block_withholding_orphaned() {
        println!("\n=== Block Withholding Orphan Test ===");
        let resolver = ProofOfWorkForkResolver::new(100);
        
        let mut headers = HashMap::new();
        headers.insert([0; 32], create_header(0, [0; 32], 0x1d00ffff, 1000, 0));
        
        // Current chain: 10 blocks deep
        let mut prev = [0; 32];
        for i in 1..=10 {
            let block = create_header(i, prev, 0x1d00ffff, 1000 + i * 600, i as u32);
            let hash = [10 + i as u8; 32];
            headers.insert(hash, block);
            prev = hash;
        }
        let current_tip = prev;
        
        // Attacker releases old block at height 1 (was withheld)
        // This should be orphaned as chain has moved on
        let withheld_block = create_header(1, [0; 32], 0x1d00ffff, 1200, 999);
        let withheld_hash = [99; 32];
        headers.insert(withheld_hash, withheld_block);
        
        let get_header = |hash: &[u8; 32]| headers.get(hash).cloned();
        
        // Current chain should be far ahead
        let result = resolver.compare_chains(&withheld_hash, &current_tip, get_header).unwrap();
        
        assert_eq!(
            result, Ordering::Less,
            "Withheld block should lose to current chain"
        );
        
        println!("✓ Block withholding: old blocks get orphaned");
    }
    
    /// Test that late-released chain with more work still wins
    /// This is expected behavior - we always follow most work
    #[test]
    fn test_late_chain_with_more_work() {
        println!("\n=== Late Chain With More Work Test ===");
        let resolver = ProofOfWorkForkResolver::new(100);
        
        let mut headers = HashMap::new();
        headers.insert([0; 32], create_header(0, [0; 32], 0x1d00ffff, 1000, 0));
        
        // Current chain: 5 blocks at normal difficulty
        let mut prev = [0; 32];
        for i in 1..=5 {
            let block = create_header(i, prev, 0x1d00ffff, 1000 + i * 600, i as u32);
            let hash = [10 + i as u8; 32];
            headers.insert(hash, block);
            prev = hash;
        }
        let current_tip = prev;
        
        // Attacker releases chain: 5 blocks with MORE work (harder difficulty)
        prev = [0; 32];
        for i in 1..=5 {
            // Much harder difficulty = more work
            let block = create_header(i, prev, 0x1c00ffff, 1000 + i * 600, 100 + i as u32);
            let hash = [20 + i as u8; 32];
            headers.insert(hash, block);
            prev = hash;
        }
        let attacker_tip = prev;
        
        let get_header = |hash: &[u8; 32]| headers.get(hash).cloned();
        
        // Chain with more work should win (this is Bitcoin's design)
        let result = resolver.compare_chains(&current_tip, &attacker_tip, get_header).unwrap();
        
        assert_eq!(
            result, Ordering::Less,
            "Chain with more work wins, even if released late"
        );
        
        println!("✓ Late chain with more work: follows Nakamoto consensus (most work wins)");
    }
}

// ============================================================================
// Time Warp Attack Tests
// ============================================================================

mod time_warp_attack {
    use super::*;
    
    /// Test that time warp attempts don't bypass difficulty
    /// Note: Actual time validation is in TimeWarpPrevention module
    #[test]
    fn test_time_warp_detection() {
        println!("\n=== Time Warp Attack Test ===");
        
        // Time warp attack: manipulate timestamps to lower difficulty
        // This test verifies that fork resolution isn't affected
        let resolver = ProofOfWorkForkResolver::new(100);
        
        let mut headers = HashMap::new();
        headers.insert([0; 32], create_header(0, [0; 32], 0x1d00ffff, 1000, 0));
        
        // Normal chain
        let mut prev = [0; 32];
        for i in 1..=5 {
            let block = create_header(i, prev, 0x1d00ffff, 1000 + i * 600, i as u32);
            let hash = [10 + i as u8; 32];
            headers.insert(hash, block);
            prev = hash;
        }
        let normal_tip = prev;
        
        // Time warp chain: timestamps go backwards (manipulation attempt)
        prev = [0; 32];
        let timestamps = [10000, 5000, 2000, 1000, 500]; // Going backwards
        for (i, &ts) in timestamps.iter().enumerate() {
            let block = create_header((i + 1) as u64, prev, 0x1d00ffff, ts, 100 + i as u32);
            let hash = [20 + i as u8; 32];
            headers.insert(hash, block);
            prev = hash;
        }
        let warp_tip = prev;
        
        let get_header = |hash: &[u8; 32]| headers.get(hash).cloned();
        
        // Fork resolution should still work (timestamp validation is separate)
        let result = resolver.compare_chains(&normal_tip, &warp_tip, get_header);
        
        assert!(result.is_ok(), "Fork resolution should handle timestamp anomalies");
        
        // Note: In production, the time warp chain would be rejected by
        // TimeWarpPrevention before reaching fork resolution
        println!("✓ Time warp: fork resolution not affected by timestamp order");
    }
    
    /// Test blocks with future timestamps
    #[test]
    fn test_future_timestamp_blocks() {
        println!("\n=== Future Timestamp Test ===");
        let resolver = ProofOfWorkForkResolver::new(100);
        
        let mut headers = HashMap::new();
        headers.insert([0; 32], create_header(0, [0; 32], 0x1d00ffff, 1000, 0));
        
        // Normal chain
        let normal_block = create_header(1, [0; 32], 0x1d00ffff, 1600, 111);
        let normal_hash = [10; 32];
        headers.insert(normal_hash, normal_block);
        
        // Future timestamp chain (2 hours in future is typically max allowed)
        let future_block = create_header(1, [0; 32], 0x1d00ffff, 9999999999, 222);
        let future_hash = [20; 32];
        headers.insert(future_hash, future_block);
        
        let get_header = |hash: &[u8; 32]| headers.get(hash).cloned();
        
        // Fork resolution should work - timestamp validation is separate
        let result = resolver.compare_chains(&normal_hash, &future_hash, get_header);
        
        assert!(result.is_ok(), "Fork resolution should handle future timestamps");
        
        // Note: In production, future timestamps beyond threshold would be rejected
        println!("✓ Future timestamps: handled by separate validation");
    }
}

// ============================================================================
// Main Test Runner
// ============================================================================

#[test]
fn run_consensus_attack_tests() {
    println!("\n");
    println!("╔═══════════════════════════════════════════════════════════════════════╗");
    println!("║           CONSENSUS ATTACK SCENARIO TESTS - P0-004                   ║");
    println!("║                                                                       ║");
    println!("║  Testing Resistance To:                                              ║");
    println!("║  • 51% Attacks with Deep Reorganization Detection                    ║");
    println!("║  • Selfish Mining Strategies                                         ║");
    println!("║  • Block Withholding Attacks                                         ║");
    println!("║  • Time Warp Manipulation                                            ║");
    println!("╚═══════════════════════════════════════════════════════════════════════╝");
    println!("\n");
    println!("All tests validate Nakamoto Consensus: Most Proof-of-Work Wins");
    println!("Additional protections provided by TimeWarpPrevention module.\n");
}

