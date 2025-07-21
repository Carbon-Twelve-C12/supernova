//! Edge case tests for atomic swap implementation

#[cfg(test)]
mod edge_cases {
    use super::super::test_utils::*;
    use crate::atomic_swap::{*, error::*};
    
    #[test]
    fn test_extreme_timeout_values() {
        let alice = TestParticipant::new("alice");
        let bob = TestParticipant::new("bob");
        
        // Test with very short timeout
        let mut htlc = create_test_htlc(&alice.info, &bob.info, 1_000_000);
        htlc.time_lock.absolute_timeout = 1;
        htlc.time_lock.relative_timeout = 1;
        
        assert!(htlc.is_expired());
        
        // Test with maximum timeout
        htlc.time_lock.absolute_timeout = u64::MAX;
        assert!(!htlc.is_expired());
    }
    
    #[test]
    fn test_maximum_fee_values() {
        let alice = TestParticipant::new("alice");
        let bob = TestParticipant::new("bob");
        
        let fee_structure = FeeStructure {
            claim_fee: u64::MAX / 4,
            refund_fee: u64::MAX / 4,
            service_fee: Some(u64::MAX / 4),
        };
        
        let hash_lock = crypto::HashLock::new(crypto::HashFunction::SHA256).unwrap();
        let time_lock = TimeLock {
            absolute_timeout: 1000,
            relative_timeout: 100,
            grace_period: 10,
        };
        
        // Should fail with overflow protection
        let result = SupernovaHTLC::new(
            alice.info,
            bob.info,
            hash_lock,
            time_lock,
            1000, // Small amount with huge fees
            fee_structure,
        );
        
        assert!(result.is_err());
    }
    
    #[test]
    fn test_unicode_address_handling() {
        let alice = TestParticipant::new("alice");
        let mut bob = TestParticipant::new("bob");
        
        // Test with unicode addresses
        bob.info.address = "nova1🚀🌙".to_string();
        bob.info.refund_address = Some("nova1退款地址".to_string());
        
        let htlc = create_test_htlc(&alice.info, &bob.info, 1_000_000);
        assert_eq!(htlc.participant.address, "nova1🚀🌙");
    }
    
    #[test]
    fn test_concurrent_state_transitions() {
        use std::sync::{Arc, Mutex};
        use std::thread;
        
        let alice = TestParticipant::new("alice");
        let bob = TestParticipant::new("bob");
        
        let htlc = Arc::new(Mutex::new(
            create_test_htlc(&alice.info, &bob.info, 1_000_000)
        ));
        
        // Try concurrent state updates
        let handles: Vec<_> = (0..10)
            .map(|_| {
                let htlc_clone = Arc::clone(&htlc);
                thread::spawn(move || {
                    if let Ok(mut htlc) = htlc_clone.lock() {
                        let _ = htlc.update_state(HTLCState::Funded);
                    }
                })
            })
            .collect();
        
        for handle in handles {
            handle.join().unwrap();
        }
        
        // Should be in Funded state
        let final_state = htlc.lock().unwrap().state.clone();
        assert_eq!(final_state, HTLCState::Funded);
    }
} 