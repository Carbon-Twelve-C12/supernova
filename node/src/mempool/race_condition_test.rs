//! Race condition tests for mempool
//!
//! This test suite demonstrates that the atomic mempool implementation
//! successfully prevents double-spending attacks through race conditions.

#[cfg(test)]
mod mempool_race_tests {
    use crate::mempool::{TransactionPool, SecureTransactionPool, MempoolConfig, MempoolError};
    use btclib::types::transaction::{Transaction, TransactionInput, TransactionOutput};
    use std::sync::{Arc, Barrier};
    use std::thread;
    use std::time::Duration;
    use std::collections::HashSet;

    fn create_transaction(inputs: Vec<([u8; 32], u32)>, output_value: u64) -> Transaction {
        let tx_inputs = inputs.into_iter().map(|(prev_hash, index)| {
            TransactionInput::new(prev_hash, index, vec![], 0xffffffff)
        }).collect();

        let outputs = vec![TransactionOutput::new(output_value, vec![])];
        Transaction::new(1, tx_inputs, outputs, 0)
    }

    #[test]
    fn test_old_pool_race_condition_vulnerability() {
        // This test demonstrates the race condition in the old pool
        let config = MempoolConfig::default();
        let pool = Arc::new(TransactionPool::new(config));
        
        // Create multiple threads trying to spend the same output
        let num_threads = 10;
        let barrier = Arc::new(Barrier::new(num_threads));
        
        let handles: Vec<_> = (0..num_threads).map(|i| {
            let pool_clone = Arc::clone(&pool);
            let barrier_clone = Arc::clone(&barrier);
            
            thread::spawn(move || {
                // Create transaction spending the same output but with different amounts
                let tx = create_transaction(vec![([1u8; 32], 0)], 50_000_000 - (i as u64 * 1000));
                
                // Wait for all threads to be ready
                barrier_clone.wait();
                
                // Try to add transaction - race condition here!
                pool_clone.add_transaction(tx, 2)
            })
        }).collect();
        
        // Collect results
        let results: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();
        
        // Count successes
        let successes = results.iter().filter(|r| r.is_ok()).count();
        
        // In a properly functioning mempool, only ONE should succeed
        // But due to race conditions, multiple might succeed
        println!("Old pool: {} transactions succeeded (should be 1)", successes);
        
        // This demonstrates the vulnerability - multiple transactions
        // spending the same output could be added
    }

    #[test]
    fn test_secure_pool_prevents_race_conditions() {
        // This test demonstrates the fix in the secure pool
        let config = MempoolConfig::default();
        let pool = Arc::new(SecureTransactionPool::new(config));
        
        // Create multiple threads trying to spend the same output
        let num_threads = 100; // Use more threads to increase race probability
        let barrier = Arc::new(Barrier::new(num_threads));
        
        let handles: Vec<_> = (0..num_threads).map(|i| {
            let pool_clone = Arc::clone(&pool);
            let barrier_clone = Arc::clone(&barrier);
            
            thread::spawn(move || {
                // Create transaction spending the same output
                let tx = create_transaction(vec![([1u8; 32], 0)], 50_000_000 - (i as u64 * 100));
                
                // Wait for all threads to be ready
                barrier_clone.wait();
                
                // Try to add transaction - atomic operation prevents races!
                let result = pool_clone.add_transaction(tx.clone(), 2);
                (result, tx.hash())
            })
        }).collect();
        
        // Collect results
        let results: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();
        
        // Count successes and track which transaction succeeded
        let mut success_count = 0;
        let mut success_hashes = HashSet::new();
        
        for (result, tx_hash) in results {
            if result.is_ok() {
                success_count += 1;
                success_hashes.insert(tx_hash);
            }
        }
        
        // Exactly ONE transaction should succeed
        assert_eq!(success_count, 1, "Exactly one transaction should succeed");
        assert_eq!(success_hashes.len(), 1, "Only one unique transaction should be in pool");
        
        // Verify pool contains exactly one transaction
        assert_eq!(pool.size(), 1);
    }

    #[test]
    fn test_concurrent_different_outputs_succeed() {
        // Test that non-conflicting transactions all succeed
        let config = MempoolConfig::default();
        let pool = Arc::new(SecureTransactionPool::new(config));
        
        let num_threads = 50;
        let barrier = Arc::new(Barrier::new(num_threads));
        
        let handles: Vec<_> = (0..num_threads).map(|i| {
            let pool_clone = Arc::clone(&pool);
            let barrier_clone = Arc::clone(&barrier);
            
            thread::spawn(move || {
                // Each transaction spends a different output
                let tx = create_transaction(vec![([i as u8; 32], i as u32)], 50_000_000);
                
                // Wait for all threads
                barrier_clone.wait();
                
                // Add transaction
                pool_clone.add_transaction(tx, 2)
            })
        }).collect();
        
        // All should succeed since they don't conflict
        let results: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();
        let successes = results.iter().filter(|r| r.is_ok()).count();
        
        assert_eq!(successes, num_threads, "All non-conflicting transactions should succeed");
        assert_eq!(pool.size(), num_threads);
    }

    #[test]
    fn test_rbf_race_condition_prevention() {
        // Test RBF operations are atomic
        let config = MempoolConfig {
            enable_rbf: true,
            min_rbf_fee_increase: 10.0,
            ..MempoolConfig::default()
        };
        let pool = Arc::new(SecureTransactionPool::new(config));
        
        // Add initial transaction
        let tx1 = create_transaction(vec![([1u8; 32], 0)], 50_000_000);
        assert!(pool.add_transaction(tx1.clone(), 100).is_ok());
        
        // Multiple threads try to replace it
        let num_threads = 20;
        let barrier = Arc::new(Barrier::new(num_threads));
        
        let handles: Vec<_> = (0..num_threads).map(|i| {
            let pool_clone = Arc::clone(&pool);
            let barrier_clone = Arc::clone(&barrier);
            
            thread::spawn(move || {
                // Create replacement with higher fee
                let tx = create_transaction(vec![([1u8; 32], 0)], 49_000_000 - (i as u64 * 100));
                
                barrier_clone.wait();
                
                // Try to replace
                pool_clone.replace_transaction(tx, 120 + i as u64)
            })
        }).collect();
        
        // Collect results
        let results: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();
        let successes = results.iter().filter(|r| r.is_ok()).count();
        
        // Exactly one replacement should succeed
        assert_eq!(successes, 1, "Exactly one RBF should succeed");
        assert_eq!(pool.size(), 1, "Pool should still contain exactly one transaction");
        
        // Original transaction should be gone
        assert!(pool.get_transaction(&tx1.hash()).is_none());
    }

    #[test]
    fn test_concurrent_add_remove_consistency() {
        // Test that concurrent adds and removes maintain consistency
        let config = MempoolConfig::default();
        let pool = Arc::new(SecureTransactionPool::new(config));
        
        // Add some initial transactions
        let mut tx_hashes = Vec::new();
        for i in 0..20 {
            let tx = create_transaction(vec![([i; 32], 0)], 50_000_000);
            let hash = tx.hash();
            pool.add_transaction(tx, 2).unwrap();
            tx_hashes.push(hash);
        }
        
        let barrier = Arc::new(Barrier::new(40));
        
        // Half threads add, half remove
        let mut handles = Vec::new();
        
        // Adders
        for i in 20..40 {
            let pool_clone = Arc::clone(&pool);
            let barrier_clone = Arc::clone(&barrier);
            
            handles.push(thread::spawn(move || {
                let tx = create_transaction(vec![([i; 32], 0)], 50_000_000);
                barrier_clone.wait();
                pool_clone.add_transaction(tx, 2)
            }));
        }
        
        // Removers
        for i in 0..20 {
            let pool_clone = Arc::clone(&pool);
            let barrier_clone = Arc::clone(&barrier);
            let tx_hash = tx_hashes[i];
            
            handles.push(thread::spawn(move || {
                barrier_clone.wait();
                pool_clone.remove_transaction(&tx_hash)
            }));
        }
        
        // Wait for all operations
        for handle in handles {
            handle.join().unwrap();
        }
        
        // Pool should have exactly 20 transactions (20 removed, 20 added)
        assert_eq!(pool.size(), 20);
        
        // Verify no transaction appears twice
        let all_txs = pool.get_sorted_transactions();
        let unique_hashes: HashSet<_> = all_txs.iter().map(|tx| tx.hash()).collect();
        assert_eq!(unique_hashes.len(), 20, "No duplicate transactions");
    }

    #[test]
    fn test_stress_test_no_double_spends() {
        // Stress test with many concurrent operations
        let config = MempoolConfig::default();
        let pool = Arc::new(SecureTransactionPool::new(config));
        
        // Track which outputs have been spent
        let spent_outputs = Arc::new(std::sync::Mutex::new(HashSet::new()));
        
        let num_threads = 100;
        let ops_per_thread = 50;
        
        let handles: Vec<_> = (0..num_threads).map(|thread_id| {
            let pool_clone = Arc::clone(&pool);
            let spent_clone = Arc::clone(&spent_outputs);
            
            thread::spawn(move || {
                let mut local_spent = HashSet::new();
                let mut successes = 0;
                
                for op in 0..ops_per_thread {
                    // Create unique output reference
                    let output_ref = (thread_id as u8, op as u32);
                    
                    // Sometimes try to double-spend
                    let (prev_hash, index) = if op % 10 == 0 && !local_spent.is_empty() {
                        // Try to reuse a previous output (double-spend attempt)
                        let existing: Vec<_> = local_spent.iter().cloned().collect();
                        existing[op % existing.len()]
                    } else {
                        // Use new output
                        ([output_ref.0; 32], output_ref.1)
                    };
                    
                    let tx = create_transaction(vec![(prev_hash, index)], 50_000_000);
                    
                    match pool_clone.add_transaction(tx, 2) {
                        Ok(()) => {
                            local_spent.insert((prev_hash, index));
                            successes += 1;
                        }
                        Err(MempoolError::DoubleSpend(_)) => {
                            // Expected for double-spend attempts
                        }
                        Err(e) => panic!("Unexpected error: {:?}", e),
                    }
                }
                
                // Record spent outputs
                spent_clone.lock().unwrap().extend(local_spent);
                
                successes
            })
        }).collect();
        
        // Collect results
        let total_successes: usize = handles.into_iter()
            .map(|h| h.join().unwrap())
            .sum();
        
        // Verify pool size matches successful additions
        assert_eq!(pool.size(), total_successes);
        
        // Verify no double-spends in pool
        let pool_txs = pool.get_sorted_transactions();
        let mut pool_spent = HashSet::new();
        
        for tx in pool_txs {
            for input in tx.inputs() {
                let output_ref = (input.prev_tx_hash(), input.prev_output_index());
                assert!(
                    pool_spent.insert(output_ref),
                    "Double-spend detected in pool!"
                );
            }
        }
        
        println!("Stress test completed: {} transactions in pool", total_successes);
    }
} 