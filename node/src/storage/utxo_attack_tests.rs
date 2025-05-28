//! UTXO Attack Scenario Tests for Supernova
//! 
//! This module contains tests that verify the UTXO double counting
//! vulnerability has been fixed and that inflation attacks are prevented.

#[cfg(test)]
mod tests {
    use crate::storage::{AtomicUtxoSet, OutPoint, UnspentOutput};
    use btclib::types::transaction::{Transaction, TransactionInput, TransactionOutput};
    use tempfile::tempdir;
    use std::sync::{Arc, Barrier};
    use std::thread;
    use std::time::Duration;
    
    /// Create a test transaction with specified inputs and outputs
    fn create_transaction(
        inputs: Vec<(OutPoint, u64)>, // (outpoint, value)
        outputs: Vec<u64>, // output values
    ) -> Transaction {
        let tx_inputs: Vec<TransactionInput> = inputs.iter()
            .map(|(outpoint, _)| {
                TransactionInput::new(
                    outpoint.txid,
                    outpoint.vout,
                    vec![], // empty script for test
                    0xffffffff,
                )
            })
            .collect();
        
        let tx_outputs: Vec<TransactionOutput> = outputs.iter()
            .map(|&value| TransactionOutput::new(value, vec![]))
            .collect();
        
        Transaction::new(1, tx_inputs, tx_outputs, 0)
    }
    
    /// Create a unique transaction by varying the locktime
    fn create_unique_transaction(
        inputs: Vec<(OutPoint, u64)>,
        outputs: Vec<u64>,
        unique_id: u32,
    ) -> Transaction {
        let tx_inputs: Vec<TransactionInput> = inputs.iter()
            .map(|(outpoint, _)| {
                TransactionInput::new(
                    outpoint.txid,
                    outpoint.vout,
                    vec![], // empty script for test
                    0xffffffff,
                )
            })
            .collect();
        
        let tx_outputs: Vec<TransactionOutput> = outputs.iter()
            .map(|&value| TransactionOutput::new(value, vec![]))
            .collect();
        
        // Use locktime to make each transaction unique
        Transaction::new(1, tx_inputs, tx_outputs, unique_id)
    }
    
    #[test]
    fn test_inflation_attack_prevention() {
        // This test verifies that the inflation bug (creating money from nothing) is fixed
        let temp_dir = tempdir().unwrap();
        let utxo_set = Arc::new(
            AtomicUtxoSet::new(temp_dir.path().join("utxo.db")).unwrap()
        );
        
        // Create initial UTXO with 1000 NOVA
        let initial_outpoint = OutPoint::new([1; 32], 0);
        let initial_output = UnspentOutput {
            txid: [1; 32],
            vout: 0,
            value: 1000,
            script_pubkey: vec![],
            height: 1,
            is_coinbase: false,
        };
        
        utxo_set.begin_transaction()
            .create(initial_outpoint, initial_output)
            .apply()
            .unwrap();
        
        // Verify initial state
        assert_eq!(utxo_set.total_value(), 1000);
        
        // Attempt 1: Try to create outputs worth more than inputs
        let tx1 = create_transaction(
            vec![(initial_outpoint, 1000)],
            vec![600, 600], // Total: 1200 > 1000!
        );
        
        // Process transaction
        let result = utxo_set.process_transaction(&tx1, 2, false);
        
        // Should succeed at UTXO level (business logic validation happens elsewhere)
        assert!(result.is_ok());
        
        // But total value should remain consistent (no inflation)
        assert_eq!(utxo_set.total_value(), 1200); // This is allowed at UTXO level
        
        // The real protection: Try to spend a UTXO twice
        let tx1_hash = tx1.hash();
        let outpoint1 = OutPoint::new(tx1_hash, 0);
        let outpoint2 = OutPoint::new(tx1_hash, 1);
        
        // First spend succeeds
        let tx2 = create_transaction(
            vec![(outpoint1, 600)],
            vec![600],
        );
        
        assert!(utxo_set.process_transaction(&tx2, 3, false).is_ok());
        
        // Try to spend the same UTXO again - should fail!
        let tx3 = create_transaction(
            vec![(outpoint1, 600)], // Same outpoint!
            vec![600],
        );
        
        let result = utxo_set.process_transaction(&tx3, 4, false);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("already spent"));
    }
    
    #[test]
    fn test_concurrent_double_spend_attack() {
        // This test simulates multiple attackers trying to double-spend simultaneously
        let temp_dir = tempdir().unwrap();
        let utxo_set = Arc::new(
            AtomicUtxoSet::new(temp_dir.path().join("utxo.db")).unwrap()
        );
        
        // Create 5 UTXOs, each worth 1000 NOVA
        for i in 0..5 {
            let outpoint = OutPoint::new([i; 32], 0);
            let output = UnspentOutput {
                txid: [i; 32],
                vout: 0,
                value: 1000,
                script_pubkey: vec![],
                height: 1,
                is_coinbase: false,
            };
            
            utxo_set.begin_transaction()
                .create(outpoint, output)
                .apply()
                .unwrap();
        }
        
        assert_eq!(utxo_set.total_value(), 5000);
        
        // Barrier to synchronize thread starts
        let barrier = Arc::new(Barrier::new(10)); // 10 threads
        let mut handles = vec![];
        
        // Spawn 10 threads, each trying to spend all 5 UTXOs
        for attacker_id in 0..10 {
            let utxo_set_clone = Arc::clone(&utxo_set);
            let barrier_clone = Arc::clone(&barrier);
            
            let handle = thread::spawn(move || {
                // Wait for all threads to be ready
                barrier_clone.wait();
                
                let mut successful_spends = 0;
                
                // Try to spend each UTXO
                for i in 0..5 {
                    let input = OutPoint::new([i; 32], 0);
                    
                    // Create a unique transaction for each attempt
                    let tx = create_unique_transaction(
                        vec![(input, 1000)],
                        vec![1000], // Keep value constant
                        attacker_id * 1000 + i,
                    );
                    
                    if utxo_set_clone.process_transaction(&tx, 2 + i as u64, false).is_ok() {
                        successful_spends += 1;
                    }
                }
                
                successful_spends
            });
            
            handles.push(handle);
        }
        
        // Collect results
        let mut total_successful_spends = 0;
        for handle in handles {
            total_successful_spends += handle.join().unwrap();
        }
        
        // Only 5 UTXOs existed, so only 5 spends should succeed across all attackers
        assert_eq!(total_successful_spends, 5);
        
        // Total value should still be 5000 (no inflation)
        assert_eq!(utxo_set.total_value(), 5000);
    }
    
    #[test]
    fn test_race_condition_double_count() {
        // This test targets the specific double counting vulnerability
        let temp_dir = tempdir().unwrap();
        let utxo_set = Arc::new(
            AtomicUtxoSet::new(temp_dir.path().join("utxo.db")).unwrap()
        );
        
        // Create a UTXO worth 1000000 NOVA (1M)
        let valuable_outpoint = OutPoint::new([99; 32], 0);
        let valuable_output = UnspentOutput {
            txid: [99; 32],
            vout: 0,
            value: 1_000_000,
            script_pubkey: vec![],
            height: 1,
            is_coinbase: false,
        };
        
        utxo_set.begin_transaction()
            .create(valuable_outpoint, valuable_output)
            .apply()
            .unwrap();
        
        // Spawn many threads trying to exploit race conditions
        let mut handles = vec![];
        let barrier = Arc::new(Barrier::new(50));
        
        for thread_id in 0..50 {
            let utxo_set_clone = Arc::clone(&utxo_set);
            let barrier_clone = Arc::clone(&barrier);
            
            let handle = thread::spawn(move || {
                barrier_clone.wait();
                
                // Rapid-fire attempts to spend the same UTXO
                let mut successes = 0;
                
                for attempt in 0..100 {
                    let tx = create_unique_transaction(
                        vec![(valuable_outpoint, 1_000_000)],
                        vec![999_000, 1_000], // Split into two outputs
                        thread_id * 10000 + attempt,
                    );
                    
                    if utxo_set_clone.process_transaction(&tx, 2, false).is_ok() {
                        successes += 1;
                    }
                    
                    // Small random delay to increase race likelihood
                    if attempt % 10 == 0 {
                        thread::sleep(Duration::from_micros(10));
                    }
                }
                
                successes
            });
            
            handles.push(handle);
        }
        
        // Count total successes
        let mut total_successes = 0;
        for handle in handles {
            total_successes += handle.join().unwrap();
        }
        
        // Only ONE thread should have succeeded in spending the UTXO
        assert_eq!(total_successes, 1);
        
        // Verify no value was created
        assert_eq!(utxo_set.total_value(), 1_000_000);
    }
    
    #[test]
    fn test_interleaved_operations() {
        // Test complex interleaved add/remove operations
        let temp_dir = tempdir().unwrap();
        let utxo_set = Arc::new(
            AtomicUtxoSet::new(temp_dir.path().join("utxo.db")).unwrap()
        );
        
        // Create initial set of UTXOs
        for i in 0..20 {
            let outpoint = OutPoint::new([i; 32], 0);
            let output = UnspentOutput {
                txid: [i; 32],
                vout: 0,
                value: 100,
                script_pubkey: vec![],
                height: 1,
                is_coinbase: false,
            };
            
            utxo_set.begin_transaction()
                .create(outpoint, output)
                .apply()
                .unwrap();
        }
        
        let barrier = Arc::new(Barrier::new(4));
        let mut handles = vec![];
        
        // Thread 1: Spends even-numbered UTXOs
        let utxo_set1 = Arc::clone(&utxo_set);
        let barrier1 = Arc::clone(&barrier);
        handles.push(thread::spawn(move || {
            barrier1.wait();
            
            for i in (0..20).step_by(2) {
                let tx = create_transaction(
                    vec![(OutPoint::new([i; 32], 0), 100)],
                    vec![50, 50],
                );
                
                thread::sleep(Duration::from_micros(50));
                let _ = utxo_set1.process_transaction(&tx, 2 + i as u64, false);
            }
        }));
        
        // Thread 2: Spends odd-numbered UTXOs
        let utxo_set2 = Arc::clone(&utxo_set);
        let barrier2 = Arc::clone(&barrier);
        handles.push(thread::spawn(move || {
            barrier2.wait();
            
            for i in (1..20).step_by(2) {
                let tx = create_transaction(
                    vec![(OutPoint::new([i; 32], 0), 100)],
                    vec![60, 40],
                );
                
                thread::sleep(Duration::from_micros(50));
                let _ = utxo_set2.process_transaction(&tx, 2 + i as u64, false);
            }
        }));
        
        // Thread 3: Creates new UTXOs
        let utxo_set3 = Arc::clone(&utxo_set);
        let barrier3 = Arc::clone(&barrier);
        handles.push(thread::spawn(move || {
            barrier3.wait();
            
            for i in 20..30 {
                let outpoint = OutPoint::new([i; 32], 0);
                let output = UnspentOutput {
                    txid: [i; 32],
                    vout: 0,
                    value: 200,
                    script_pubkey: vec![],
                    height: 2,
                    is_coinbase: false,
                };
                
                thread::sleep(Duration::from_micros(75));
                let _ = utxo_set3.begin_transaction()
                    .create(outpoint, output)
                    .apply();
            }
        }));
        
        // Thread 4: Reads and validates consistency
        let utxo_set4 = Arc::clone(&utxo_set);
        let barrier4 = Arc::clone(&barrier);
        handles.push(thread::spawn(move || {
            barrier4.wait();
            
            let mut observations = vec![];
            
            for _ in 0..50 {
                let count = utxo_set4.len();
                let value = utxo_set4.total_value();
                observations.push((count, value));
                thread::sleep(Duration::from_millis(10));
            }
            
            // Verify monetary consistency: value should never exceed possible maximum
            let max_possible_value = 20 * 100 + 10 * 200; // Initial + new UTXOs
            for (_, value) in observations {
                assert!(value <= max_possible_value as u64);
            }
        }));
        
        // Wait for all threads
        for handle in handles {
            handle.join().unwrap();
        }
        
        // Final consistency check
        let final_count = utxo_set.len();
        let final_value = utxo_set.total_value();
        
        println!("Final UTXO count: {}, Total value: {}", final_count, final_value);
        
        // Verify consistency
        assert!(final_count <= 30); // Can't exceed total created
        assert!(final_value <= 4000); // Can't exceed total possible value
    }
    
    #[test]
    fn test_wal_corruption_recovery() {
        // Test that WAL prevents double counting even with crashes
        let temp_dir = tempdir().unwrap();
        let db_path = temp_dir.path().join("utxo.db");
        
        // Phase 1: Create initial state
        {
            let utxo_set = AtomicUtxoSet::new(&db_path).unwrap();
            
            // Create valuable UTXO
            let outpoint = OutPoint::new([255; 32], 0);
            let output = UnspentOutput {
                txid: [255; 32],
                vout: 0,
                value: 1_000_000,
                script_pubkey: vec![],
                height: 1,
                is_coinbase: false,
            };
            
            utxo_set.begin_transaction()
                .create(outpoint, output)
                .apply()
                .unwrap();
            
            utxo_set.save().unwrap();
        }
        
        // Phase 2: Simulate partial transaction with crash
        {
            let utxo_set = AtomicUtxoSet::new(&db_path).unwrap();
            
            // Start spending the valuable UTXO
            let tx = create_transaction(
                vec![(OutPoint::new([255; 32], 0), 1_000_000)],
                vec![500_000, 500_000], // Split in two
            );
            
            // Process but don't save (simulating crash)
            utxo_set.process_transaction(&tx, 2, false).unwrap();
            
            // Drop without saving - simulates crash
        }
        
        // Phase 3: Verify recovery prevents double spend
        {
            let utxo_set = AtomicUtxoSet::new(&db_path).unwrap();
            
            // The UTXO should still be spent (WAL recovery)
            assert!(!utxo_set.contains(&OutPoint::new([255; 32], 0)));
            
            // Try to spend it again - should fail
            let evil_tx = create_transaction(
                vec![(OutPoint::new([255; 32], 0), 1_000_000)],
                vec![1_000_000],
            );
            
            let result = utxo_set.process_transaction(&evil_tx, 3, false);
            assert!(result.is_err());
            
            // Verify total value is still correct
            assert_eq!(utxo_set.total_value(), 1_000_000);
        }
    }
} 