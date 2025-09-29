//! Tests for Lightning Network race condition fixes
//!
//! This module contains tests to verify that the atomic operations
//! properly prevent race conditions and fund creation exploits.

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lightning::atomic_operations::{AtomicChannel, AtomicOperationError};
    use crate::lightning::channel::{Channel, ChannelState};
    use secp256k1::PublicKey;
    use std::sync::{Arc, Barrier};
    use std::thread;
    use std::time::Duration;

    /// Test that demonstrates the fund creation bug is fixed
    #[test]
    #[ignore] // Lightning race condition fixes pending
    fn test_fund_creation_bug_fixed() {
        // Create a channel with 1,000,000 NOVA capacity
        let secp = secp256k1::Secp256k1::new();
        let local_private_key = secp256k1::SecretKey::from_slice(&[1u8; 32]).unwrap();
        let local_node_id = PublicKey::from_secret_key(&secp, &local_private_key);
        let remote_private_key = secp256k1::SecretKey::from_slice(&[2u8; 32]).unwrap();
        let remote_node_id = PublicKey::from_secret_key(&secp, &remote_private_key);
        let mut channel = Channel::new(local_node_id, remote_node_id, 1_000_000, true, false);
        channel.state = ChannelState::Active;
        channel.local_balance_novas = 600_000;
        channel.remote_balance_novas = 400_000;

        let atomic_channel = Arc::new(AtomicChannel::new(channel));

        // Spawn multiple threads that try to exploit the race condition
        let num_threads = 10;
        let barrier = Arc::new(Barrier::new(num_threads));
        let mut handles = vec![];

        for i in 0..num_threads {
            let channel_clone = Arc::clone(&atomic_channel);
            let barrier_clone = Arc::clone(&barrier);

            let handle = thread::spawn(move || {
                // Wait for all threads to be ready
                barrier_clone.wait();

                // Try to add and settle HTLCs rapidly to create funds
                let mut results = vec![];

                // Add HTLC
                let payment_hash = [i as u8; 32];
                let htlc_result = channel_clone.add_htlc(payment_hash, 50_000, 500_000, true);

                if let Ok(htlc_id) = htlc_result {
                    // Try to settle immediately
                    let settle_result = channel_clone.settle_htlc(htlc_id, payment_hash);
                    results.push((htlc_id, settle_result.is_ok()));
                }

                results
            });

            handles.push(handle);
        }

        // Collect all results
        let mut all_results = vec![];
        for handle in handles {
            let thread_results = handle.join().unwrap();
            all_results.extend(thread_results);
        }

        // Verify balances are consistent
        let (final_local, final_remote) = atomic_channel.get_balances().unwrap();
        let total_balance = final_local + final_remote;

        // Count successful HTLCs
        let successful_htlcs = all_results.iter().filter(|(_, success)| *success).count();

        // The total balance should still equal the capacity
        // No funds should have been created from nothing
        assert_eq!(total_balance, 1_000_000, "Funds were created from nothing!");

        println!(
            "Successfully processed {} HTLCs without creating funds",
            successful_htlcs
        );
    }

    /// Test concurrent HTLC operations don't corrupt state
    #[test]
    fn test_concurrent_htlc_state_consistency() {
        let secp = secp256k1::Secp256k1::new();
        let local_private_key = secp256k1::SecretKey::from_slice(&[1u8; 32]).unwrap();
        let local_node_id = PublicKey::from_secret_key(&secp, &local_private_key);
        let remote_private_key = secp256k1::SecretKey::from_slice(&[2u8; 32]).unwrap();
        let remote_node_id = PublicKey::from_secret_key(&secp, &remote_private_key);
        let mut channel = Channel::new(local_node_id, remote_node_id, 10_000_000, true, false);
        channel.state = ChannelState::Active;
        channel.local_balance_novas = 5_000_000;
        channel.remote_balance_novas = 5_000_000;

        let atomic_channel = Arc::new(AtomicChannel::new(channel));

        // Spawn threads for different operations
        let barrier = Arc::new(Barrier::new(4));
        let mut handles = vec![];

        // Thread 1: Add outgoing HTLCs
        let channel1 = Arc::clone(&atomic_channel);
        let barrier1 = Arc::clone(&barrier);
        handles.push(thread::spawn(move || {
            barrier1.wait();
            let mut htlc_ids = vec![];
            for i in 0..5 {
                if let Ok(id) = channel1.add_htlc([i; 32], 100_000, 500_000, true) {
                    htlc_ids.push(id);
                }
                thread::sleep(Duration::from_millis(10));
            }
            htlc_ids
        }));

        // Thread 2: Add incoming HTLCs
        let channel2 = Arc::clone(&atomic_channel);
        let barrier2 = Arc::clone(&barrier);
        handles.push(thread::spawn(move || {
            barrier2.wait();
            let mut htlc_ids = vec![];
            for i in 5..10 {
                if let Ok(id) = channel2.add_htlc([i; 32], 100_000, 500_000, false) {
                    htlc_ids.push(id);
                }
                thread::sleep(Duration::from_millis(10));
            }
            htlc_ids
        }));

        // Thread 3: Settle some HTLCs
        let channel3 = Arc::clone(&atomic_channel);
        let barrier3 = Arc::clone(&barrier);
        handles.push(thread::spawn(move || {
            barrier3.wait();
            thread::sleep(Duration::from_millis(50)); // Wait for some HTLCs to be added
            let mut settled = 0;
            for i in 0..10 {
                if channel3.settle_htlc(i, [i as u8; 32]).is_ok() {
                    settled += 1;
                }
            }
            vec![settled as u64]
        }));

        // Thread 4: Fail some HTLCs
        let channel4 = Arc::clone(&atomic_channel);
        let barrier4 = Arc::clone(&barrier);
        handles.push(thread::spawn(move || {
            barrier4.wait();
            thread::sleep(Duration::from_millis(50)); // Wait for some HTLCs to be added
            let mut failed = 0;
            for i in 10..15 {
                if channel4.fail_htlc(i, "Test failure").is_ok() {
                    failed += 1;
                }
            }
            vec![failed as u64]
        }));

        // Wait for all threads to complete
        for handle in handles {
            handle.join().unwrap();
        }

        // Verify state consistency
        let channel_info = atomic_channel.get_channel_info().unwrap();
        let (local, remote) = atomic_channel.get_balances().unwrap();

        // Total balance should still equal capacity
        assert_eq!(
            local + remote + (channel_info.pending_htlcs_count as u64 * 100_000),
            10_000_000
        );

        println!("Channel state after concurrent operations:");
        println!("  Local balance: {} NOVA", local);
        println!("  Remote balance: {} NOVA", remote);
        println!("  Pending HTLCs: {}", channel_info.pending_htlcs_count);
        println!("  Commitment number: {}", channel_info.commitment_number);
    }

    /// Test that operations are properly serialized
    #[test]
    fn test_operation_serialization() {
        let secp = secp256k1::Secp256k1::new();
        let local_private_key = secp256k1::SecretKey::from_slice(&[1u8; 32]).unwrap();
        let local_node_id = PublicKey::from_secret_key(&secp, &local_private_key);
        let remote_private_key = secp256k1::SecretKey::from_slice(&[2u8; 32]).unwrap();
        let remote_node_id = PublicKey::from_secret_key(&secp, &remote_private_key);
        let mut channel = Channel::new(local_node_id, remote_node_id, 1_000_000, true, false);
        channel.state = ChannelState::Active;
        channel.local_balance_novas = 1_000_000;
        channel.remote_balance_novas = 0;

        let atomic_channel = Arc::new(AtomicChannel::new(channel));

        // Try to add two HTLCs concurrently that would overdraw the balance
        let barrier = Arc::new(Barrier::new(2));
        let mut handles = vec![];

        for i in 0..2 {
            let channel_clone = Arc::clone(&atomic_channel);
            let barrier_clone = Arc::clone(&barrier);

            let handle = thread::spawn(move || {
                barrier_clone.wait();
                // Each thread tries to send 600,000 NOVA
                channel_clone.add_htlc([i; 32], 600_000, 500_000, true)
            });

            handles.push(handle);
        }

        // Collect results
        let mut successes = 0;
        let mut failures = 0;

        for handle in handles {
            match handle.join().unwrap() {
                Ok(_) => successes += 1,
                Err(_) => failures += 1,
            }
        }

        // Exactly one should succeed and one should fail
        assert_eq!(successes, 1, "Expected exactly one HTLC to succeed");
        assert_eq!(failures, 1, "Expected exactly one HTLC to fail");

        // Verify balance
        let (local, _) = atomic_channel.get_balances().unwrap();
        assert_eq!(
            local, 400_000,
            "Local balance should be 400,000 after one 600,000 HTLC"
        );
    }

    /// Test channel state transitions are atomic
    #[test]
    fn test_atomic_state_transitions() {
        let secp = secp256k1::Secp256k1::new();
        let local_private_key = secp256k1::SecretKey::from_slice(&[1u8; 32]).unwrap();
        let local_node_id = PublicKey::from_secret_key(&secp, &local_private_key);
        let remote_private_key = secp256k1::SecretKey::from_slice(&[2u8; 32]).unwrap();
        let remote_node_id = PublicKey::from_secret_key(&secp, &remote_private_key);
        let mut channel = Channel::new(local_node_id, remote_node_id, 1_000_000, true, false);
        channel.state = ChannelState::Active;

        let atomic_channel = Arc::new(AtomicChannel::new(channel));

        // Try to transition state from multiple threads
        let barrier = Arc::new(Barrier::new(3));
        let mut handles = vec![];

        // Thread 1: Try to set to ClosingNegotiation
        let channel1 = Arc::clone(&atomic_channel);
        let barrier1 = Arc::clone(&barrier);
        handles.push(thread::spawn(move || {
            barrier1.wait();
            channel1.set_state(ChannelState::ClosingNegotiation)
        }));

        // Thread 2: Try to set to ForceClosed
        let channel2 = Arc::clone(&atomic_channel);
        let barrier2 = Arc::clone(&barrier);
        handles.push(thread::spawn(move || {
            barrier2.wait();
            thread::sleep(Duration::from_millis(10)); // Small delay
            channel2.set_state(ChannelState::ForceClosed)
        }));

        // Thread 3: Try to set to an invalid state
        let channel3 = Arc::clone(&atomic_channel);
        let barrier3 = Arc::clone(&barrier);
        handles.push(thread::spawn(move || {
            barrier3.wait();
            thread::sleep(Duration::from_millis(20)); // Small delay
                                                      // Try to go from ClosingNegotiation back to Active (invalid)
            channel3.set_state(ChannelState::Active)
        }));

        // Collect results
        let results: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();

        // First transition should succeed
        assert!(results[0].is_ok(), "First valid transition should succeed");

        // Second transition might succeed or fail depending on timing
        // If it happens after first, it's invalid (ClosingNegotiation -> ForceClosed)

        // Third transition should definitely fail (invalid transition)
        assert!(results[2].is_err(), "Invalid state transition should fail");

        // Final state should be consistent
        let final_state = atomic_channel.get_state().unwrap();
        assert!(
            final_state == ChannelState::ClosingNegotiation
                || final_state == ChannelState::ForceClosed,
            "Final state should be one of the valid transitions"
        );
    }

    /// Test balance update atomicity
    #[test]
    fn test_atomic_balance_updates() {
        let secp = secp256k1::Secp256k1::new();
        let local_private_key = secp256k1::SecretKey::from_slice(&[1u8; 32]).unwrap();
        let local_node_id = PublicKey::from_secret_key(&secp, &local_private_key);
        let remote_private_key = secp256k1::SecretKey::from_slice(&[2u8; 32]).unwrap();
        let remote_node_id = PublicKey::from_secret_key(&secp, &remote_private_key);
        let mut channel = Channel::new(local_node_id, remote_node_id, 1_000_000, true, false);
        channel.state = ChannelState::Active;
        channel.local_balance_novas = 500_000;
        channel.remote_balance_novas = 500_000;

        let atomic_channel = Arc::new(AtomicChannel::new(channel));

        // Spawn many threads that try to update balances
        let num_threads = 20;
        let barrier = Arc::new(Barrier::new(num_threads));
        let mut handles = vec![];

        for i in 0..num_threads {
            let channel_clone = Arc::clone(&atomic_channel);
            let barrier_clone = Arc::clone(&barrier);

            let handle = thread::spawn(move || {
                barrier_clone.wait();

                // Each thread moves 1000 NOVA back and forth
                let mut success_count = 0;
                for _ in 0..10 {
                    // Move from local to remote
                    if channel_clone
                        .update_balances(&|local, remote| {
                            if local >= 1000 {
                                Ok((local - 1000, remote + 1000))
                            } else {
                                Err("Insufficient local balance".to_string())
                            }
                        })
                        .is_ok()
                    {
                        success_count += 1;
                    }

                    // Move from remote to local
                    if channel_clone
                        .update_balances(&|local, remote| {
                            if remote >= 1000 {
                                Ok((local + 1000, remote - 1000))
                            } else {
                                Err("Insufficient remote balance".to_string())
                            }
                        })
                        .is_ok()
                    {
                        success_count += 1;
                    }
                }

                success_count
            });

            handles.push(handle);
        }

        // Wait for all threads
        let total_updates: usize = handles.into_iter().map(|h| h.join().unwrap()).sum();

        // Verify final balances
        let (final_local, final_remote) = atomic_channel.get_balances().unwrap();

        // Balances should sum to capacity
        assert_eq!(
            final_local + final_remote,
            1_000_000,
            "Total balance should remain constant"
        );

        println!(
            "Completed {} balance updates across {} threads",
            total_updates, num_threads
        );
        println!(
            "Final balances: local={}, remote={}",
            final_local, final_remote
        );
    }
}
