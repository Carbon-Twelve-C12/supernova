//! Mempool DoS Protection Tests
//!
//! SECURITY TEST SUITE (P1-002): Comprehensive tests for mempool DoS protection
//!
//! Test Coverage:
//! - Fee-based eviction under full mempool conditions
//! - Ancestor chain limits (count and size)
//! - Descendant chain limits (count and size)
//! - RBF with insufficient fee bump rejection
//! - RBF success path
//! - RBF eviction count limits
//! - Transaction relay rate limiting
//! - Memory cap enforcement
//! - Transaction size limits

#[cfg(test)]
mod tests {
    use crate::mempool::chain_limits::{ChainLimitsConfig, ChainLimitsTracker};
    use crate::mempool::error::MempoolError;
    use crate::mempool::pool::{MempoolConfig, TransactionPool};
    use crate::mempool::rate_limiter::{MempoolDoSConfig, MempoolRateLimiter};
    use supernova_core::types::transaction::{Transaction, TransactionInput, TransactionOutput};
    use std::thread;
    use std::time::Duration;

    // =========================================================================
    // Helper Functions
    // =========================================================================

    fn create_test_transaction(prev_hash: [u8; 32], value: u64) -> Transaction {
        Transaction::new(
            1,
            vec![TransactionInput::new(prev_hash, 0, vec![], 0xffffffff)],
            vec![TransactionOutput::new(value, vec![0u8; 25])], // P2PKH-like output
            0,
        )
    }

    fn create_transaction_with_output_index(prev_hash: [u8; 32], output_index: u32, value: u64) -> Transaction {
        Transaction::new(
            1,
            vec![TransactionInput::new(prev_hash, output_index, vec![], 0xffffffff)],
            vec![TransactionOutput::new(value, vec![0u8; 25])],
            0,
        )
    }

    // =========================================================================
    // Fee-Based Eviction Tests
    // =========================================================================

    #[test]
    fn test_mempool_eviction_under_full_conditions() {
        let config = MempoolConfig {
            max_size: 5,
            min_fee_rate: 1,
            ..MempoolConfig::default()
        };
        let pool = TransactionPool::new(config);

        // Fill mempool with low-fee transactions
        for i in 0..5u8 {
            let tx = create_test_transaction([i; 32], 50_000);
            pool.add_transaction(tx, 1).unwrap();
        }

        assert_eq!(pool.size(), 5);

        // Add higher-fee transaction - should evict lowest fee
        let high_fee_tx = create_test_transaction([100u8; 32], 50_000);
        let result = pool.add_transaction(high_fee_tx.clone(), 5); // 5x higher fee

        // Should succeed and evict a low-fee transaction
        assert!(result.is_ok(), "High-fee transaction should be accepted");
        assert_eq!(pool.size(), 5);
        assert!(pool.get_transaction(&high_fee_tx.hash()).is_some());

        println!("✓ Mempool eviction under full conditions works correctly");
    }

    #[test]
    fn test_mempool_rejects_low_fee_when_full() {
        let config = MempoolConfig {
            max_size: 3,
            min_fee_rate: 1,
            ..MempoolConfig::default()
        };
        let pool = TransactionPool::new(config);

        // Fill mempool with medium-fee transactions
        for i in 0..3u8 {
            let tx = create_test_transaction([i; 32], 50_000);
            pool.add_transaction(tx, 10).unwrap();
        }

        // Try to add low-fee transaction - should fail
        let low_fee_tx = create_test_transaction([99u8; 32], 50_000);
        let result = pool.add_transaction(low_fee_tx, 1);

        assert!(result.is_err());
        match result {
            Err(MempoolError::MempoolFull { .. }) => (),
            _ => panic!("Expected MempoolFull error"),
        }

        println!("✓ Low-fee transactions rejected when mempool full");
    }

    // =========================================================================
    // Ancestor/Descendant Chain Limit Tests
    // =========================================================================

    #[test]
    fn test_ancestor_chain_count_limit() {
        let config = ChainLimitsConfig {
            max_ancestors: 25,
            ..Default::default()
        };
        let tracker = ChainLimitsTracker::new(config);

        // Create a chain of 25 transactions
        let mut prev_hash = [0u8; 32];
        for i in 0..25u8 {
            let tx_hash = [i + 1; 32];
            tracker.register_transaction(tx_hash, 250, &[prev_hash]);
            prev_hash = tx_hash;
        }

        // 26th transaction should fail (would have 25 ancestors)
        let tx26 = [26u8; 32];
        let result = tracker.check_chain_limits(tx26, 250, &[prev_hash]);

        assert!(matches!(result, Err(MempoolError::AncestorChainTooLong { count: 25, limit: 25 })));

        println!("✓ Ancestor chain count limit (25) correctly enforced");
    }

    #[test]
    fn test_descendant_chain_count_limit() {
        let config = ChainLimitsConfig {
            max_descendants: 25,
            ..Default::default()
        };
        let tracker = ChainLimitsTracker::new(config);

        // Create root transaction
        let root_hash = [0u8; 32];
        tracker.register_transaction(root_hash, 250, &[]);

        // Create 25 direct children of root
        for i in 1..=25u8 {
            let child_hash = [i; 32];
            tracker.check_chain_limits(child_hash, 250, &[root_hash]).unwrap();
            tracker.register_transaction(child_hash, 250, &[root_hash]);
        }

        // 26th child should fail
        let tx26 = [26u8; 32];
        let result = tracker.check_chain_limits(tx26, 250, &[root_hash]);

        assert!(matches!(result, Err(MempoolError::DescendantChainTooLong { .. })));

        println!("✓ Descendant chain count limit (25) correctly enforced");
    }

    #[test]
    fn test_ancestor_chain_size_limit() {
        let config = ChainLimitsConfig {
            max_ancestor_size_bytes: 101 * 1024, // 101KB
            max_ancestors: 100, // High count limit to test size
            ..Default::default()
        };
        let tracker = ChainLimitsTracker::new(config);

        // Create chain with large transactions (10KB each)
        let mut prev_hash = [0u8; 32];
        for i in 0..10u8 {
            let tx_hash = [i + 1; 32];
            tracker.register_transaction(tx_hash, 10 * 1024, &[prev_hash]); // 10KB each
            prev_hash = tx_hash;
        }

        // Another 10KB transaction would make total 110KB > 101KB limit
        let large_tx = [100u8; 32];
        let result = tracker.check_chain_limits(large_tx, 10 * 1024, &[prev_hash]);

        assert!(matches!(result, Err(MempoolError::AncestorSizeTooLarge { .. })));

        println!("✓ Ancestor chain size limit (101KB) correctly enforced");
    }

    #[test]
    fn test_descendant_chain_size_limit() {
        let config = ChainLimitsConfig {
            max_descendant_size_bytes: 101 * 1024, // 101KB
            max_descendants: 100, // High count limit to test size
            ..Default::default()
        };
        let tracker = ChainLimitsTracker::new(config);

        // Create root with large descendants
        let root_hash = [0u8; 32];
        tracker.register_transaction(root_hash, 10 * 1024, &[]);

        // Add descendants totaling 100KB
        for i in 1..=10u8 {
            let child_hash = [i; 32];
            tracker.check_chain_limits(child_hash, 10 * 1024, &[root_hash]).unwrap();
            tracker.register_transaction(child_hash, 10 * 1024, &[root_hash]);
        }

        // Another descendant would exceed 101KB limit
        let result = tracker.check_chain_limits([100u8; 32], 10 * 1024, &[root_hash]);

        assert!(matches!(result, Err(MempoolError::DescendantSizeTooLarge { .. })));

        println!("✓ Descendant chain size limit (101KB) correctly enforced");
    }

    // =========================================================================
    // RBF Tests
    // =========================================================================

    #[test]
    fn test_rbf_insufficient_fee_bump_rejected() {
        let config = MempoolConfig {
            enable_rbf: true,
            min_rbf_fee_increase: 10.0, // 10% minimum increase
            min_fee_rate: 1,
            ..MempoolConfig::default()
        };
        let pool = TransactionPool::new(config);

        // Add original transaction
        let tx1 = create_test_transaction([1u8; 32], 50_000);
        pool.add_transaction(tx1, 10).unwrap();

        // Try RBF with only 5% increase - should fail
        let tx2 = create_test_transaction([1u8; 32], 50_000); // Same input
        let result = pool.replace_transaction(tx2, 10); // Same fee rate

        assert!(matches!(result, Err(MempoolError::FeeTooLow { .. })));

        println!("✓ RBF with insufficient fee bump correctly rejected");
    }

    #[test]
    fn test_rbf_success_with_sufficient_fee() {
        let config = MempoolConfig {
            enable_rbf: true,
            min_rbf_fee_increase: 10.0,
            min_fee_rate: 1,
            ..MempoolConfig::default()
        };
        let pool = TransactionPool::new(config);

        // Add original transaction
        let tx1 = create_test_transaction([1u8; 32], 50_000);
        let tx1_hash = tx1.hash();
        pool.add_transaction(tx1, 10).unwrap();

        // RBF with 50% increase - should succeed
        let tx2 = create_test_transaction([1u8; 32], 49_000); // Different output, same input
        let tx2_hash = tx2.hash();
        let result = pool.replace_transaction(tx2, 15); // 50% higher fee rate

        assert!(result.is_ok());
        assert!(pool.get_transaction(&tx1_hash).is_none(), "Original tx should be removed");
        assert!(pool.get_transaction(&tx2_hash).is_some(), "Replacement tx should exist");

        println!("✓ RBF success with sufficient fee bump");
    }

    #[test]
    fn test_rbf_eviction_count_limit() {
        let config = ChainLimitsConfig {
            max_rbf_evictions: 5,
            ..Default::default()
        };
        let tracker = ChainLimitsTracker::new(config);

        // Create chain: tx0 -> tx1 -> tx2 -> tx3 -> tx4 -> tx5 -> tx6
        let tx0 = [0u8; 32];
        tracker.register_transaction(tx0, 100, &[]);

        for i in 1..=6u8 {
            let tx = [i; 32];
            let parent = [(i - 1); 32];
            tracker.register_transaction(tx, 100, &[parent]);
        }

        // RBF on tx0 would evict 7 transactions (tx0 + 6 descendants) > limit of 5
        let result = tracker.check_rbf_eviction_count(&[tx0]);
        assert!(matches!(result, Err(MempoolError::RbfTooManyEvictions { count: 7, limit: 5 })));

        // RBF on tx4 would evict 3 transactions (tx4, tx5, tx6) <= limit of 5
        let tx4 = [4u8; 32];
        assert!(tracker.check_rbf_eviction_count(&[tx4]).is_ok());

        println!("✓ RBF eviction count limit correctly enforced");
    }

    // =========================================================================
    // Transaction Relay Rate Limiting Tests
    // =========================================================================

    #[test]
    fn test_relay_rate_limit_per_second() {
        let limiter = MempoolRateLimiter::new();
        let peer_id = "test_peer_1";

        // Should allow MAX_TX_RELAY_PER_SECOND relays
        for i in 0..MempoolDoSConfig::MAX_TX_RELAY_PER_SECOND {
            let result = limiter.check_relay_rate_limit(peer_id);
            assert!(result.is_ok(), "Relay {} should be allowed", i);
        }

        // Next relay should be rejected
        let result = limiter.check_relay_rate_limit(peer_id);
        assert!(matches!(result, Err(MempoolError::RelayRateLimitExceeded { .. })));

        println!("✓ Transaction relay rate limit ({}/second) enforced", 
            MempoolDoSConfig::MAX_TX_RELAY_PER_SECOND);
    }

    #[test]
    fn test_relay_rate_limit_resets_after_window() {
        let limiter = MempoolRateLimiter::new();
        let peer_id = "test_peer_2";

        // Use up the limit
        for _ in 0..MempoolDoSConfig::MAX_TX_RELAY_PER_SECOND {
            limiter.check_relay_rate_limit(peer_id).unwrap();
        }

        // Should be rate limited
        assert!(limiter.check_relay_rate_limit(peer_id).is_err());

        // Wait for window to reset (1 second + small margin)
        thread::sleep(Duration::from_millis(1100));

        // Should be allowed again
        assert!(limiter.check_relay_rate_limit(peer_id).is_ok());

        println!("✓ Relay rate limit correctly resets after window");
    }

    // =========================================================================
    // Memory and Size Limit Tests
    // =========================================================================

    #[test]
    fn test_memory_cap_enforcement() {
        let limiter = MempoolRateLimiter::new();

        // Simulate adding transactions up to near the limit
        let near_limit_size = MempoolDoSConfig::MAX_MEMPOOL_BYTES - 1000;
        limiter.record_addition(near_limit_size);

        // Small transaction should still be allowed
        assert!(limiter.check_rate_limit(None, 500, 1000).is_ok());

        // Large transaction that would exceed limit should be rejected
        let result = limiter.check_rate_limit(None, 2000, 1000);
        assert!(matches!(result, Err(MempoolError::MemoryLimitExceeded { .. })));

        println!("✓ Memory cap ({}MB) correctly enforced", 
            MempoolDoSConfig::MAX_MEMPOOL_BYTES / 1024 / 1024);
    }

    #[test]
    fn test_transaction_size_limit() {
        let limiter = MempoolRateLimiter::new();

        // Transaction at the limit should be allowed
        assert!(limiter.check_rate_limit(None, MempoolDoSConfig::MAX_SINGLE_TX_SIZE, 1000).is_ok());

        // Transaction exceeding limit should be rejected
        let result = limiter.check_rate_limit(None, MempoolDoSConfig::MAX_SINGLE_TX_SIZE + 1, 1000);
        assert!(matches!(result, Err(MempoolError::TransactionTooLarge { .. })));

        println!("✓ Transaction size limit ({}MB) correctly enforced",
            MempoolDoSConfig::MAX_SINGLE_TX_SIZE / 1024 / 1024);
    }

    #[test]
    fn test_minimum_fee_rate() {
        let limiter = MempoolRateLimiter::new();

        // Transaction with fee below minimum should be rejected
        let result = limiter.check_rate_limit(None, 250, MempoolDoSConfig::MIN_FEE_RATE - 1);
        assert!(matches!(result, Err(MempoolError::FeeTooLow { .. })));

        // Transaction at minimum fee should be allowed
        assert!(limiter.check_rate_limit(None, 250, MempoolDoSConfig::MIN_FEE_RATE).is_ok());

        println!("✓ Minimum fee rate ({} novas/byte) correctly enforced",
            MempoolDoSConfig::MIN_FEE_RATE);
    }

    // =========================================================================
    // Per-Peer Rate Limit Tests
    // =========================================================================

    #[test]
    fn test_per_peer_rate_limit() {
        let limiter = MempoolRateLimiter::new();
        let peer_id = "test_peer_3";

        // Should allow MAX_TXS_PER_PEER_PER_MINUTE transactions
        for i in 0..MempoolDoSConfig::MAX_TXS_PER_PEER_PER_MINUTE {
            let result = limiter.check_rate_limit(Some(peer_id), 250, MempoolDoSConfig::MIN_FEE_RATE);
            assert!(result.is_ok(), "Transaction {} should be allowed", i);
        }

        // Next transaction should be rejected
        let result = limiter.check_rate_limit(Some(peer_id), 250, MempoolDoSConfig::MIN_FEE_RATE);
        assert!(matches!(result, Err(MempoolError::RateLimitExceeded { .. })));

        println!("✓ Per-peer rate limit ({}/minute) enforced",
            MempoolDoSConfig::MAX_TXS_PER_PEER_PER_MINUTE);
    }

    #[test]
    fn test_different_peers_have_separate_limits() {
        let limiter = MempoolRateLimiter::new();

        // Use up peer1's limit
        for _ in 0..MempoolDoSConfig::MAX_TXS_PER_PEER_PER_MINUTE {
            limiter.check_rate_limit(Some("peer1"), 250, MempoolDoSConfig::MIN_FEE_RATE).unwrap();
        }

        // peer1 should be rate limited
        assert!(limiter.check_rate_limit(Some("peer1"), 250, MempoolDoSConfig::MIN_FEE_RATE).is_err());

        // peer2 should still have full quota
        assert!(limiter.check_rate_limit(Some("peer2"), 250, MempoolDoSConfig::MIN_FEE_RATE).is_ok());

        println!("✓ Different peers have separate rate limits");
    }

    // =========================================================================
    // Statistics Tests
    // =========================================================================

    #[test]
    fn test_dos_stats_tracking() {
        let limiter = MempoolRateLimiter::new();

        // Generate some rejections
        let _ = limiter.check_rate_limit(None, MempoolDoSConfig::MAX_SINGLE_TX_SIZE + 1, 1000);
        let _ = limiter.check_rate_limit(None, 250, MempoolDoSConfig::MIN_FEE_RATE - 1);

        // Use up a peer's relay limit
        for _ in 0..=MempoolDoSConfig::MAX_TX_RELAY_PER_SECOND {
            let _ = limiter.check_relay_rate_limit("peer_stats");
        }

        let stats = limiter.get_stats();
        assert!(stats.rejected_by_size >= 1, "Should track size rejections");
        assert!(stats.rejected_by_relay_limit >= 1, "Should track relay rejections");

        println!("✓ DoS protection statistics correctly tracked");
        println!("  - Rejected by size: {}", stats.rejected_by_size);
        println!("  - Rejected by relay limit: {}", stats.rejected_by_relay_limit);
        println!("  - Active peer limits: {}", stats.active_peer_limits);
    }

    // =========================================================================
    // Integration Test
    // =========================================================================

    #[test]
    fn test_full_dos_protection_integration() {
        println!("\n=== Mempool DoS Protection Integration Test ===\n");

        let config = MempoolConfig {
            max_size: 100,
            min_fee_rate: 1,
            enable_rbf: true,
            min_rbf_fee_increase: 10.0,
            ..MempoolConfig::default()
        };
        let pool = TransactionPool::new(config);

        // 1. Add various transactions
        for i in 0..50u8 {
            let tx = create_test_transaction([i; 32], 50_000);
            pool.add_transaction(tx, (i as u64 % 10) + 1).unwrap();
        }
        println!("  ✓ Added 50 transactions with varying fees");

        // 2. Test fee-based sorting
        let sorted = pool.get_sorted_transactions();
        assert!(!sorted.is_empty());
        println!("  ✓ Transactions sortable by fee");

        // 3. Test info retrieval
        let info = pool.get_info();
        assert_eq!(info.transaction_count, 50);
        println!("  ✓ Mempool info: {} txs, {} bytes", info.transaction_count, info.total_size);

        // 4. Test double-spend detection
        let double_spend = create_test_transaction([0u8; 32], 60_000);
        assert!(pool.check_double_spend(&double_spend));
        println!("  ✓ Double-spend detection working");

        // 5. Test RBF
        let replacement = create_test_transaction([0u8; 32], 45_000);
        let result = pool.replace_transaction(replacement, 20);
        assert!(result.is_ok());
        println!("  ✓ RBF working with sufficient fee bump");

        // 6. Test expiry clearing
        let cleared = pool.clear_expired();
        println!("  ✓ Cleared {} expired transactions", cleared);

        println!("\n=== All DoS Protection Tests Passed ===\n");
    }
}

