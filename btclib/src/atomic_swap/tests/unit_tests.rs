//! Unit tests for atomic swap components

use super::test_utils::*;
use crate::atomic_swap::{
    htlc::*, crypto::*, bitcoin_adapter::*, monitor::*, api::*, error::*,
    websocket::*, SwapState, HTLCState,
};
use crate::crypto::{MLDSASignature, MLDSAPrivateKey};
use std::time::{SystemTime, UNIX_EPOCH, Duration};

#[cfg(test)]
mod htlc_tests {
    use super::*;
    
    #[test]
    fn test_htlc_creation_with_all_fields() {
        let alice = TestParticipant::new("alice");
        let bob = TestParticipant::new("bob");
        
        let htlc = create_test_htlc(&alice.info, &bob.info, 1_000_000);
        
        assert_eq!(htlc.initiator.address, "nova1alice");
        assert_eq!(htlc.participant.address, "nova1bob");
        assert_eq!(htlc.amount, 1_000_000);
        assert_eq!(htlc.state, HTLCState::Created);
        assert!(htlc.htlc_id != [0u8; 32]);
    }
    
    #[test]
    fn test_htlc_invalid_amount() {
        let alice = TestParticipant::new("alice");
        let bob = TestParticipant::new("bob");
        
        let hash_lock = HashLock::new(HashFunction::SHA256).unwrap();
        let time_lock = TimeLock {
            absolute_timeout: 1000,
            relative_timeout: 100,
            grace_period: 10,
        };
        
        let result = SupernovaHTLC::new(
            alice.info.clone(),
            bob.info.clone(),
            hash_lock,
            time_lock,
            0, // Invalid zero amount
            FeeStructure::default(),
        );
        
        assert!(result.is_err());
    }
    
    #[test]
    fn test_htlc_claim_verification() {
        let alice = TestParticipant::new("alice");
        let bob = TestParticipant::new("bob");
        
        let mut htlc = create_test_htlc(&alice.info, &bob.info, 1_000_000);
        htlc.update_state(HTLCState::Funded).unwrap();
        
        // Create claim message
        let preimage = htlc.hash_lock.preimage.unwrap();
        let claim_msg = htlc.create_claim_message(&preimage).unwrap();
        
        // Sign with participant's key
        let signature = bob.private_key.sign(&claim_msg);
        
        // Verify claim
        let result = htlc.verify_claim(&preimage, &signature, 1000);
        assert!(result.is_ok());
    }
    
    #[test]
    fn test_htlc_refund_after_timeout() {
        let alice = TestParticipant::new("alice");
        let bob = TestParticipant::new("bob");
        
        let mut htlc = create_test_htlc(&alice.info, &bob.info, 1_000_000);
        htlc.time_lock.absolute_timeout = 100; // Set to past
        htlc.update_state(HTLCState::Funded).unwrap();
        
        // Create refund message
        let refund_msg = htlc.create_refund_message().unwrap();
        
        // Sign with initiator's key
        let signature = alice.private_key.sign(&refund_msg);
        
        // Verify refund
        let result = htlc.verify_refund(&signature, 200); // Current height > timeout
        assert!(result.is_ok());
    }
    
    #[test]
    fn test_htlc_state_machine() {
        let alice = TestParticipant::new("alice");
        let bob = TestParticipant::new("bob");
        
        let mut htlc = create_test_htlc(&alice.info, &bob.info, 1_000_000);
        
        // Test valid transitions
        assert_eq!(htlc.state, HTLCState::Created);
        assert!(htlc.update_state(HTLCState::Funded).is_ok());
        assert_eq!(htlc.state, HTLCState::Funded);
        
        // Test invalid transition
        assert!(htlc.update_state(HTLCState::Created).is_err());
        
        // Can transition to Claimed or Refunded from Funded
        assert!(htlc.update_state(HTLCState::Claimed).is_ok());
        assert_eq!(htlc.state, HTLCState::Claimed);
    }
}

#[cfg(test)]
mod crypto_tests {
    use super::*;
    
    #[test]
    fn test_hash_lock_verification() {
        let hash_lock = HashLock::new(HashFunction::SHA256).unwrap();
        let preimage = hash_lock.preimage.unwrap();
        
        // Correct preimage should verify
        assert!(hash_lock.verify_preimage(&preimage).unwrap());
        
        // Wrong preimage should fail
        let wrong_preimage = generate_secure_random_32();
        assert!(!hash_lock.verify_preimage(&wrong_preimage).unwrap());
    }
    
    #[test]
    fn test_all_hash_functions() {
        let data = b"test data";
        
        // Test each hash function
        for hash_fn in &[HashFunction::SHA256, HashFunction::BLAKE3, HashFunction::SHA3_256] {
            let hash = compute_hash_with_type(data, hash_fn).unwrap();
            assert_eq!(hash.len(), 32);
            
            // Same data should produce same hash
            let hash2 = compute_hash_with_type(data, hash_fn).unwrap();
            assert_eq!(hash, hash2);
        }
    }
    
    #[test]
    fn test_timelock_hash_computation() {
        let timelock = 1000u32;
        let hash = timelock::compute_timelock_hash(timelock);
        assert_eq!(hash.len(), 32);
        
        // Same timelock should produce same hash
        let hash2 = timelock::compute_timelock_hash(timelock);
        assert_eq!(hash, hash2);
    }
    
    #[test]
    fn test_signature_adaptation() {
        use crate::crypto::ECDSASignature;
        
        let btc_sig = signature_adapter::BitcoinSignature {
            r: [1u8; 32],
            s: [2u8; 32],
            recovery_id: 0,
        };
        
        let nova_sig = signature_adapter::adapt_bitcoin_signature(&btc_sig).unwrap();
        
        // Should produce valid quantum signature placeholder
        match nova_sig {
            signature_adapter::SupernovaSignature::ECDSA(sig) => {
                // Verify it's a valid ECDSA signature format
                assert!(sig.0.len() > 0);
            }
            _ => panic!("Expected ECDSA signature"),
        }
    }
    
    #[test]
    fn test_merkle_proof_generation() {
        let leaves = vec![[1u8; 32], [2u8; 32], [3u8; 32], [4u8; 32]];
        let root = merkle::compute_htlc_merkle_root(&leaves).unwrap();
        
        // Generate proof for leaf at index 1
        let proof = merkle::generate_merkle_proof(&leaves, 1).unwrap();
        
        // Verify the proof (would need verification function)
        assert!(!proof.is_empty());
        assert_ne!(root, [0u8; 32]);
    }
}

#[cfg(test)]
mod bitcoin_adapter_tests {
    use super::*;
    use bitcoin::blockdata::script::Builder as ScriptBuilder;
    
    #[test]
    fn test_bitcoin_htlc_script_creation() {
        let receiver_pubkey = [3u8; 33]; // Compressed pubkey
        let sender_pubkey = [2u8; 33];
        let hash = [42u8; 32];
        let timeout = 1000u32;
        
        let htlc = BitcoinHTLC {
            receiver_pubkey,
            sender_pubkey,
            hash,
            timeout,
            script_type: HTLCScriptType::P2WSH,
        };
        
        let script = htlc.create_redeem_script();
        assert!(!script.is_empty());
        
        // Script should contain the hash
        let script_bytes = script.as_bytes();
        assert!(script_bytes.windows(32).any(|w| w == &hash));
    }
    
    #[test]
    fn test_bitcoin_script_address_generation() {
        let htlc = BitcoinHTLC {
            receiver_pubkey: [3u8; 33],
            sender_pubkey: [2u8; 33],
            hash: [42u8; 32],
            timeout: 1000,
            script_type: HTLCScriptType::P2WSH,
        };
        
        let address = htlc.create_address(bitcoin::Network::Testnet);
        assert!(address.is_ok());
        
        let addr = address.unwrap();
        assert!(addr.to_string().starts_with("tb1")); // Testnet bech32
    }
    
    #[test]
    fn test_secret_extraction_from_bitcoin_tx() {
        use bitcoin::Script;
        
        // Create a mock claim transaction
        let secret = [99u8; 32];
        let mut witness = bitcoin::blockdata::witness::Witness::new();
        witness.push(&secret);
        witness.push(vec![1u8; 64]); // Dummy signature
        witness.push(vec![2u8; 100]); // Dummy script
        
        let mut tx = create_mock_bitcoin_tx("test", 100000, vec![]);
        tx.input[0].witness = witness;
        
        let extracted = extract_secret_from_bitcoin_tx(&tx, 0);
        assert!(extracted.is_ok());
        assert_eq!(extracted.unwrap(), secret);
    }
    
    #[test]
    fn test_htlc_output_detection() {
        let script_bytes = vec![
            0x63, // OP_IF
            0x82, // OP_SIZE
            0x20, // Push 32 bytes
        ];
        
        let outputs = vec![
            bitcoin::TxOut {
                value: 100000,
                script_pubkey: Script::from(script_bytes.clone()).into(),
            },
            bitcoin::TxOut {
                value: 50000,
                script_pubkey: Script::from(vec![0x00, 0x14]).into(), // P2WPKH
            },
        ];
        
        let htlc_outputs = find_htlc_outputs(&outputs);
        assert_eq!(htlc_outputs.len(), 1);
        assert_eq!(htlc_outputs[0], 0);
    }
}

#[cfg(test)]
mod monitor_tests {
    use super::*;
    
    #[tokio::test]
    async fn test_monitor_swap_lifecycle() {
        let alice = TestParticipant::new("alice");
        let bob = TestParticipant::new("bob");
        
        let monitor = create_test_monitor();
        let swap = create_test_swap_session(&alice.info, &bob.info, 100000, 1000000);
        
        // Add swap
        monitor.add_swap(swap.clone()).await.unwrap();
        
        let active = monitor.get_active_swaps().await;
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].swap_id, swap.setup.swap_id);
        
        // Remove swap
        monitor.remove_swap(&swap.setup.swap_id).await.unwrap();
        let active = monitor.get_active_swaps().await;
        assert!(active.is_empty());
    }
    
    #[tokio::test]
    async fn test_monitor_event_detection() {
        let config = MonitorConfig {
            poll_interval: 1,
            min_confirmations: 1,
            max_reorg_depth: 6,
            event_retention: 3600,
        };
        
        let monitor = Arc::new(CrossChainMonitor::new(config, None));
        
        // Add test swap
        let alice = TestParticipant::new("alice");
        let bob = TestParticipant::new("bob");
        let swap = create_test_swap_session(&alice.info, &bob.info, 100000, 1000000);
        
        monitor.add_swap(swap.clone()).await.unwrap();
        
        // Simulate Bitcoin event
        let event = SwapEvent::BitcoinHTLCFunded {
            swap_id: swap.setup.swap_id,
            txid: "test_txid".to_string(),
            amount: 100000,
            confirmations: 1,
        };
        
        // Process event (would be done by monitoring loop)
        monitor.handle_bitcoin_event(event.clone()).await.unwrap();
        
        // Check state was updated
        let swaps = monitor.active_swaps.read().await;
        let updated_swap = swaps.get(&swap.setup.swap_id).unwrap();
        assert_eq!(updated_swap.state, SwapState::BothFunded);
    }
}

#[cfg(test)]
mod api_tests {
    use super::*;
    use crate::atomic_swap::api::*;
    
    #[tokio::test]
    async fn test_api_swap_initiation() {
        let config = create_test_config();
        let monitor = create_test_monitor();
        let api = AtomicSwapRpcImpl::new(config, monitor, None);
        
        let params = InitiateSwapParams {
            bitcoin_amount: 100000,
            nova_amount: 1000000,
            bitcoin_counterparty: "tb1qtest".to_string(),
            nova_counterparty: "nova1test".to_string(),
            timeout_minutes: 60,
            memo: Some("Test swap".to_string()),
        };
        
        let result = api.initiate_swap(params).await;
        assert!(result.is_ok());
        
        let session = result.unwrap();
        assert_eq!(session.state, SwapState::Active);
        assert_eq!(session.setup.bitcoin_amount, 100000);
    }
    
    #[tokio::test]
    async fn test_api_swap_claiming() {
        let config = create_test_config();
        let monitor = create_test_monitor();
        let api = AtomicSwapRpcImpl::new(config, monitor, None);
        
        // Create swap
        let params = InitiateSwapParams {
            bitcoin_amount: 100000,
            nova_amount: 1000000,
            bitcoin_counterparty: "tb1qtest".to_string(),
            nova_counterparty: "nova1test".to_string(),
            timeout_minutes: 60,
            memo: None,
        };
        
        let session = api.initiate_swap(params).await.unwrap();
        let secret = session.secret.unwrap();
        
        // Claim swap
        let result = api.claim_swap(session.setup.swap_id, secret).await;
        assert!(result.is_ok());
        
        // Check status
        let status = api.get_swap_status(session.setup.swap_id).await.unwrap();
        assert_eq!(status.state, SwapState::Claimed);
    }
    
    #[tokio::test]
    async fn test_api_fee_estimation() {
        let config = create_test_config();
        let monitor = create_test_monitor();
        let api = AtomicSwapRpcImpl::new(config, monitor, None);
        
        let params = InitiateSwapParams {
            bitcoin_amount: 100000,
            nova_amount: 1000000,
            bitcoin_counterparty: "tb1qtest".to_string(),
            nova_counterparty: "nova1test".to_string(),
            timeout_minutes: 60,
            memo: None,
        };
        
        let fees = api.estimate_swap_fees(params).await.unwrap();
        assert!(fees.bitcoin_network_fee > 0);
        assert!(fees.nova_network_fee > 0);
        assert!(fees.total_fee_btc > 0);
    }
}

#[cfg(test)]
mod websocket_tests {
    use super::*;
    use tokio::sync::mpsc;
    use uuid::Uuid;
    
    #[tokio::test]
    async fn test_websocket_subscription() {
        let (event_tx, event_rx) = mpsc::unbounded_channel();
        let mut manager = WsNotificationManager::new(event_rx);
        
        // Add client
        let client_id = Uuid::new_v4();
        let (client_tx, mut client_rx) = mpsc::unbounded_channel();
        
        manager.add_client(client_id, client_tx).await;
        
        // Subscribe to swap
        let swap_id = [1u8; 32];
        manager.handle_subscription(client_id, swap_id).await.unwrap();
        
        // Send event
        let event = SwapEvent::SwapInitiated {
            swap_id,
            initiator: "alice".to_string(),
            participant: "bob".to_string(),
            amounts: SwapAmounts {
                bitcoin_sats: 100000,
                nova_units: 1000000,
            },
        };
        
        event_tx.send(event).unwrap();
        
        // Start manager
        let handle = tokio::spawn(async move {
            manager.start().await;
        });
        
        // Check client received event
        if let Some(msg) = client_rx.recv().await {
            match msg {
                WsMessage::SwapEvent { event } => {
                    if let SwapEvent::SwapInitiated { swap_id: id, .. } = event {
                        assert_eq!(id, swap_id);
                    }
                }
                _ => panic!("Unexpected message type"),
            }
        }
        
        drop(event_tx);
        let _ = handle.await;
    }
} 