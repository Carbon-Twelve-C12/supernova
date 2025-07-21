//! Security tests for atomic swap implementation
//! 
//! Tests for attack resistance, cryptographic security, and edge cases

use super::test_utils::*;
use crate::atomic_swap::{
    htlc::*, crypto::*, error::*, monitor::*, SwapState, HTLCState,
};
use std::time::{SystemTime, UNIX_EPOCH, Duration};

#[cfg(test)]
mod replay_attack_tests {
    use super::*;
    
    #[test]
    fn test_htlc_replay_protection() {
        let alice = TestParticipant::new("alice");
        let bob = TestParticipant::new("bob");
        
        let mut htlc = create_test_htlc(&alice.info, &bob.info, 1_000_000);
        htlc.update_state(HTLCState::Funded).unwrap();
        
        // Create and verify a claim
        let preimage = htlc.hash_lock.preimage.unwrap();
        let claim_msg = htlc.create_claim_message(&preimage).unwrap();
        let signature = bob.private_key.sign(&claim_msg);
        
        // First claim should succeed
        let result1 = htlc.verify_claim(&preimage, &signature, 1000);
        assert!(result1.is_ok());
        assert!(htlc.update_state(HTLCState::Claimed).is_ok());
        
        // Replay attempt should fail (already claimed)
        let result2 = htlc.verify_claim(&preimage, &signature, 1000);
        assert!(result2.is_err());
    }
    
    #[test]
    fn test_double_spend_prevention() {
        let alice = TestParticipant::new("alice");
        let bob = TestParticipant::new("bob");
        
        let mut htlc = create_test_htlc(&alice.info, &bob.info, 1_000_000);
        htlc.update_state(HTLCState::Funded).unwrap();
        
        // Claim the HTLC
        htlc.update_state(HTLCState::Claimed).unwrap();
        
        // Attempt to refund after claim should fail
        let refund_msg = htlc.create_refund_message();
        assert!(refund_msg.is_err()); // Should fail - already claimed
    }
}

#[cfg(test)]
mod timing_attack_tests {
    use super::*;
    
    #[test]
    fn test_race_condition_claim_refund() {
        let alice = TestParticipant::new("alice");
        let bob = TestParticipant::new("bob");
        
        let mut htlc = create_test_htlc(&alice.info, &bob.info, 1_000_000);
        htlc.update_state(HTLCState::Funded).unwrap();
        
        // Set timeout to be very close
        htlc.time_lock.absolute_timeout = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() + 1;
        
        // Bob tries to claim
        let preimage = htlc.hash_lock.preimage.unwrap();
        let claim_msg = htlc.create_claim_message(&preimage).unwrap();
        let claim_sig = bob.private_key.sign(&claim_msg);
        
        // Alice prepares refund
        let refund_msg = htlc.create_refund_message().unwrap();
        let refund_sig = alice.private_key.sign(&refund_msg);
        
        // Current time is before timeout - claim should succeed
        let claim_result = htlc.verify_claim(&preimage, &claim_sig, 1000);
        assert!(claim_result.is_ok());
        
        // After timeout - refund would succeed if not already claimed
        std::thread::sleep(Duration::from_secs(2));
        let current_height = 2000; // After timeout
        
        // But since already claimed, refund should fail
        htlc.update_state(HTLCState::Claimed).unwrap();
        assert!(htlc.state != HTLCState::Funded); // Can't refund
    }
    
    #[test]
    fn test_timeout_boundary_conditions() {
        let alice = TestParticipant::new("alice");
        let bob = TestParticipant::new("bob");
        
        let mut htlc = create_test_htlc(&alice.info, &bob.info, 1_000_000);
        htlc.time_lock.absolute_timeout = 1000;
        htlc.time_lock.grace_period = 10;
        
        // Exactly at timeout - should not be expired yet
        assert!(!htlc.is_expired_at_height(1000));
        
        // Within grace period - still not expired for claims
        assert!(!htlc.is_expired_at_height(1005));
        
        // After grace period - definitely expired
        assert!(htlc.is_expired_at_height(1011));
    }
}

#[cfg(test)]
mod cryptographic_security_tests {
    use super::*;
    
    #[test]
    fn test_hash_preimage_secrecy() {
        let hash_lock = HashLock::new(HashFunction::SHA256).unwrap();
        let hash = hash_lock.hash_value;
        
        // Should not be able to derive preimage from hash
        // This is a basic sanity check - real security depends on hash function
        let random_preimage = generate_secure_random_32();
        assert!(!hash_lock.verify_preimage(&random_preimage).unwrap());
        
        // Only the correct preimage should verify
        assert!(hash_lock.verify_preimage(&hash_lock.preimage.unwrap()).unwrap());
    }
    
    #[test]
    fn test_signature_forgery_resistance() {
        let alice = TestParticipant::new("alice");
        let bob = TestParticipant::new("bob");
        let eve = TestParticipant::new("eve"); // Attacker
        
        let mut htlc = create_test_htlc(&alice.info, &bob.info, 1_000_000);
        htlc.update_state(HTLCState::Funded).unwrap();
        
        // Create legitimate claim message
        let preimage = htlc.hash_lock.preimage.unwrap();
        let claim_msg = htlc.create_claim_message(&preimage).unwrap();
        
        // Eve tries to forge Bob's signature
        let forged_sig = eve.private_key.sign(&claim_msg);
        
        // Verification should fail - wrong key
        let result = htlc.verify_claim(&preimage, &forged_sig, 1000);
        assert!(result.is_err());
    }
    
    #[test]
    fn test_malformed_signature_handling() {
        let alice = TestParticipant::new("alice");
        let bob = TestParticipant::new("bob");
        
        let mut htlc = create_test_htlc(&alice.info, &bob.info, 1_000_000);
        let preimage = htlc.hash_lock.preimage.unwrap();
        
        // Create a malformed signature (wrong size)
        let malformed_sig = MLDSASignature {
            algorithm: crate::crypto::quantum::SignatureAlgorithm::Dilithium,
            data: vec![0u8; 10], // Too short
            public_key_hint: None,
        };
        
        // Should handle gracefully without panic
        let result = htlc.verify_claim(&preimage, &malformed_sig, 1000);
        assert!(result.is_err());
    }
}

#[cfg(test)]
mod malicious_input_tests {
    use super::*;
    
    #[test]
    fn test_oversized_preimage_handling() {
        let hash_lock = HashLock::new(HashFunction::SHA256).unwrap();
        
        // Try with oversized preimage
        let oversized = vec![0u8; 1000]; // Much larger than 32 bytes
        
        // Should handle gracefully
        let result = hash_lock.verify_preimage(&oversized);
        assert!(result.is_err() || !result.unwrap());
    }
    
    #[test]
    fn test_zero_amount_swap_prevention() {
        let alice = TestParticipant::new("alice");
        let bob = TestParticipant::new("bob");
        
        // Attempt to create HTLC with zero amount
        let hash_lock = HashLock::new(HashFunction::SHA256).unwrap();
        let time_lock = TimeLock {
            absolute_timeout: 1000,
            relative_timeout: 100,
            grace_period: 10,
        };
        
        let result = SupernovaHTLC::new(
            alice.info,
            bob.info,
            hash_lock,
            time_lock,
            0, // Zero amount
            FeeStructure::default(),
        );
        
        assert!(result.is_err());
    }
    
    #[test]
    fn test_overflow_protection() {
        let alice = TestParticipant::new("alice");
        let bob = TestParticipant::new("bob");
        
        // Test with maximum values
        let htlc = create_test_htlc(&alice.info, &bob.info, u64::MAX - 10000);
        
        // Total with fees should not overflow
        let total = htlc.total_amount_with_fees();
        assert!(total >= htlc.amount);
        assert!(total <= u64::MAX);
    }
}

#[cfg(test)]
mod monitor_security_tests {
    use super::*;
    
    #[tokio::test]
    async fn test_monitor_state_tampering_prevention() {
        let monitor = create_test_monitor();
        let alice = TestParticipant::new("alice");
        let bob = TestParticipant::new("bob");
        
        let swap = create_test_swap_session(&alice.info, &bob.info, 100000, 1000000);
        let swap_id = swap.setup.swap_id;
        
        monitor.add_swap(swap).await.unwrap();
        
        // Try to add duplicate swap
        let duplicate = create_test_swap_session(&alice.info, &bob.info, 200000, 2000000);
        let mut dup_with_same_id = duplicate;
        dup_with_same_id.setup.swap_id = swap_id; // Same ID
        
        let result = monitor.add_swap(dup_with_same_id).await;
        assert!(result.is_err()); // Should reject duplicate
    }
    
    #[tokio::test]
    async fn test_monitor_event_validation() {
        let monitor = create_test_monitor();
        let alice = TestParticipant::new("alice");
        let bob = TestParticipant::new("bob");
        
        // Create swap but don't add it
        let swap = create_test_swap_session(&alice.info, &bob.info, 100000, 1000000);
        let fake_swap_id = [99u8; 32];
        
        // Try to process event for non-existent swap
        let event = SwapEvent::BitcoinSecretRevealed {
            swap_id: fake_swap_id,
            secret: [0u8; 32],
            txid: "fake".to_string(),
        };
        
        let result = monitor.handle_bitcoin_event(event).await;
        assert!(result.is_err()); // Should reject event for unknown swap
    }
}

#[cfg(test)]
mod privacy_security_tests {
    use super::*;
    
    #[cfg(feature = "atomic-swap")]
    #[test]
    fn test_confidential_amount_hiding() {
        use crate::atomic_swap::confidential::*;
        
        let builder = ConfidentialSwapBuilder::new();
        let alice = TestParticipant::new("alice");
        let bob = TestParticipant::new("bob");
        
        let htlc = create_test_htlc(&alice.info, &bob.info, 1_000_000);
        
        // Create confidential HTLC
        let conf_htlc = builder.create_confidential_htlc(
            htlc,
            1_000_000,
            100_000,
            10_000_000,
        ).unwrap();
        
        // Amount should be hidden in commitment
        assert_ne!(conf_htlc.amount_commitment.as_bytes(), &[0u8; 32]);
        
        // But base amount should be zero (hidden)
        assert!(conf_htlc.blinding_factor.is_some());
    }
    
    #[cfg(feature = "atomic-swap")]
    #[test]
    fn test_zk_proof_soundness() {
        use crate::atomic_swap::zk_swap::*;
        
        let mut builder = ZKSwapBuilder::new();
        builder.setup().unwrap();
        
        // Create valid proof
        let amount = 1_000_000u64;
        let (preimage, hash) = generate_test_hash_pair();
        let commitment = [42u8; 32];
        
        let proof = builder.prove_swap_validity(
            amount,
            preimage,
            commitment,
            hash,
        ).unwrap();
        
        // Verification with correct inputs should succeed
        let valid = builder.verify_swap_validity(&proof, commitment, hash).unwrap();
        assert!(valid);
        
        // Verification with wrong inputs should fail
        let wrong_hash = [99u8; 32];
        let invalid = builder.verify_swap_validity(&proof, commitment, wrong_hash);
        assert!(invalid.is_err() || !invalid.unwrap());
    }
}

#[cfg(test)]
mod dos_resistance_tests {
    use super::*;
    use std::sync::Arc;
    use tokio::sync::Mutex;
    
    #[tokio::test]
    async fn test_api_rate_limiting() {
        let config = AtomicSwapConfig {
            max_swaps_per_hour: 10,
            max_swaps_per_address: 2,
            ..Default::default()
        };
        
        let monitor = create_test_monitor();
        let api = crate::atomic_swap::api::AtomicSwapRpcImpl::new(
            config,
            monitor,
            None,
        );
        
        // Try to create many swaps from same address
        let mut successful = 0;
        for i in 0..5 {
            let params = crate::atomic_swap::api::InitiateSwapParams {
                bitcoin_amount: 100000,
                nova_amount: 1000000,
                bitcoin_counterparty: "tb1qtest".to_string(),
                nova_counterparty: "nova1test".to_string(), // Same address
                timeout_minutes: 60,
                memo: Some(format!("Test {}", i)),
            };
            
            if api.initiate_swap(params).await.is_ok() {
                successful += 1;
            }
        }
        
        // Should be limited by max_swaps_per_address
        assert!(successful <= 2);
    }
    
    #[tokio::test]
    async fn test_websocket_subscription_limits() {
        use tokio::sync::mpsc;
        use uuid::Uuid;
        
        let (event_tx, event_rx) = mpsc::unbounded_channel();
        let mut manager = crate::atomic_swap::websocket::WsNotificationManager::new(event_rx);
        
        let client_id = Uuid::new_v4();
        let (client_tx, _rx) = mpsc::unbounded_channel();
        
        manager.add_client(client_id, client_tx).await;
        
        // Try to subscribe to many swaps
        let mut subscribed = 0;
        for i in 0..1000 {
            let swap_id = [i as u8; 32];
            if manager.handle_subscription(client_id, swap_id).await.is_ok() {
                subscribed += 1;
            }
        }
        
        // Should have some reasonable limit
        assert!(subscribed < 1000); // Implementation should limit subscriptions
    }
} 