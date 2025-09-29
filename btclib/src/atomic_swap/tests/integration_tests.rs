//! Integration tests for complete atomic swap flows

use super::test_utils::*;
use crate::atomic_swap::{
    AtomicSwapConfig, SwapSession, SwapState, HTLCState,
    monitor::*, api::*, websocket::*,
};
use std::sync::Arc;
use tokio::sync::mpsc;
use std::time::Duration;

#[cfg(test)]
mod full_swap_flow_tests {
    use super::*;

    #[tokio::test]
    async fn test_successful_atomic_swap_flow() {
        // Setup participants
        let alice = TestParticipant::new("alice");
        let bob = TestParticipant::new("bob");

        // Setup infrastructure
        let config = create_test_config();
        let monitor = create_test_monitor();
        let api = Arc::new(AtomicSwapRpcImpl::new(
            config.clone(),
            monitor.clone(),
            None,
        ));

        // Alice initiates swap (BTC for NOVA)
        let params = InitiateSwapParams {
            bitcoin_amount: 100_000, // 0.001 BTC
            nova_amount: 1_000_000_000, // 10 NOVA
            bitcoin_counterparty: "tb1qbob".to_string(),
            nova_counterparty: bob.info.address.clone(),
            timeout_minutes: 60,
            memo: Some("Alice trading BTC for NOVA".to_string()),
        };

        let swap_session = api.initiate_swap(params).await.unwrap();
        let swap_id = swap_session.setup.swap_id;
        let secret = swap_session.secret.unwrap();

        // Verify initial state
        let status = api.get_swap_status(swap_id).await.unwrap();
        assert_eq!(status.state, SwapState::Active);
        assert_eq!(status.bitcoin_amount, 100_000);
        assert_eq!(status.nova_amount, 1_000_000_000);

        // Simulate funding events
        monitor.handle_bitcoin_event(SwapEvent::BitcoinHTLCFunded {
            swap_id,
            txid: "btc_funding_tx".to_string(),
            amount: 100_000,
            confirmations: 6,
        }).await.unwrap();

        // Check state transition
        let status = api.get_swap_status(swap_id).await.unwrap();
        assert_eq!(status.state, SwapState::BothFunded);

        // Bob claims Bitcoin (reveals secret)
        monitor.handle_bitcoin_event(SwapEvent::BitcoinSecretRevealed {
            swap_id,
            secret,
            txid: "btc_claim_tx".to_string(),
        }).await.unwrap();

        // Monitor should automatically trigger Supernova claim
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Verify final state
        let status = api.get_swap_status(swap_id).await.unwrap();
        assert_eq!(status.state, SwapState::Completed);

        // Check events
        let events = api.get_swap_events(swap_id, 10).await.unwrap();
        assert!(events.len() >= 4); // Init, Fund, Reveal, Complete
    }

    #[tokio::test]
    async fn test_refund_on_timeout_flow() {
        let alice = TestParticipant::new("alice");
        let bob = TestParticipant::new("bob");

        let config = AtomicSwapConfig {
            timeout_delta: 10, // Very short timeout
            ..create_test_config()
        };

        let monitor = create_test_monitor();
        let api = Arc::new(AtomicSwapRpcImpl::new(
            config,
            monitor.clone(),
            None,
        ));

        // Initiate swap
        let params = InitiateSwapParams {
            bitcoin_amount: 50_000,
            nova_amount: 500_000_000,
            bitcoin_counterparty: "tb1qbob".to_string(),
            nova_counterparty: bob.info.address.clone(),
            timeout_minutes: 1, // Very short
            memo: Some("Test timeout scenario".to_string()),
        };

        let swap_session = api.initiate_swap(params).await.unwrap();
        let swap_id = swap_session.setup.swap_id;

        // Fund but don't claim
        monitor.handle_bitcoin_event(SwapEvent::BitcoinHTLCFunded {
            swap_id,
            txid: "btc_funding_tx".to_string(),
            amount: 50_000,
            confirmations: 6,
        }).await.unwrap();

        // Wait for timeout
        tokio::time::sleep(Duration::from_secs(65)).await;

        // Trigger refund
        let refund_result = api.refund_swap(swap_id).await;
        assert!(refund_result.is_ok());

        // Check final state
        let status = api.get_swap_status(swap_id).await.unwrap();
        assert_eq!(status.state, SwapState::Refunded);
    }
}

#[cfg(test)]
mod websocket_integration_tests {
    use super::*;
    use uuid::Uuid;

    #[tokio::test]
    async fn test_realtime_swap_notifications() {
        // Setup WebSocket infrastructure
        let (event_tx, event_rx) = mpsc::unbounded_channel();
        let mut ws_manager = WsNotificationManager::new(event_rx);

        // Setup swap infrastructure
        let monitor_config = MonitorConfig {
            event_retention: 3600,
            ..Default::default()
        };
        let monitor = Arc::new(CrossChainMonitor::new_with_event_channel(
            monitor_config,
            None,
            event_tx.clone(),
        ));

        // Add WebSocket client
        let client_id = Uuid::new_v4();
        let (client_tx, mut client_rx) = mpsc::unbounded_channel();
        ws_manager.add_client(client_id, client_tx).await;

        // Start WebSocket manager
        let ws_handle = tokio::spawn(async move {
            ws_manager.start().await;
        });

        // Create and monitor swap
        let alice = TestParticipant::new("alice");
        let bob = TestParticipant::new("bob");
        let swap = create_test_swap_session(&alice.info, &bob.info, 100_000, 1_000_000);
        let swap_id = swap.setup.swap_id;

        // Subscribe client to this swap
        event_tx.send(SwapEvent::SwapInitiated {
            swap_id,
            initiator: alice.info.address.clone(),
            participant: bob.info.address.clone(),
            amounts: SwapAmounts {
                bitcoin_sats: 100_000,
                nova_units: 1_000_000,
            },
        }).unwrap();

        // Add swap to monitor
        monitor.add_swap(swap).await.unwrap();

        // Simulate Bitcoin funding
        monitor.handle_bitcoin_event(SwapEvent::BitcoinHTLCFunded {
            swap_id,
            txid: "test_tx".to_string(),
            amount: 100_000,
            confirmations: 6,
        }).await.unwrap();

        // Check client received notifications
        let mut received_events = vec![];
        while let Ok(Some(msg)) = tokio::time::timeout(
            Duration::from_millis(100),
            client_rx.recv()
        ).await {
            if let WsMessage::SwapEvent { event } = msg {
                received_events.push(event);
            }
        }

        // Should have received at least init and fund events
        assert!(received_events.len() >= 2);

        // Cleanup
        drop(event_tx);
        let _ = ws_handle.await;
    }
}

#[cfg(test)]
mod multi_swap_integration_tests {
    use super::*;

    #[tokio::test]
    async fn test_concurrent_swaps_handling() {
        let config = create_test_config();
        let monitor = create_test_monitor();
        let api = Arc::new(AtomicSwapRpcImpl::new(
            config,
            monitor.clone(),
            None,
        ));

        // Create multiple concurrent swaps
        let mut swap_ids = vec![];

        for i in 0..5 {
            let params = InitiateSwapParams {
                bitcoin_amount: 100_000 * (i + 1),
                nova_amount: 1_000_000_000 * (i + 1),
                bitcoin_counterparty: format!("tb1quser{}", i),
                nova_counterparty: format!("nova1user{}", i),
                timeout_minutes: 60,
                memo: Some(format!("Swap {}", i)),
            };

            let session = api.initiate_swap(params).await.unwrap();
            swap_ids.push(session.setup.swap_id);
        }

        // Verify all swaps are active
        let all_swaps = api.list_swaps(SwapFilter::default()).await.unwrap();
        assert_eq!(all_swaps.len(), 5);

        // Process events for different swaps
        for (i, &swap_id) in swap_ids.iter().enumerate() {
            monitor.handle_bitcoin_event(SwapEvent::BitcoinHTLCFunded {
                swap_id,
                txid: format!("tx_{}", i),
                amount: 100_000 * (i as u64 + 1),
                confirmations: 6,
            }).await.unwrap();
        }

        // Verify states
        for &swap_id in &swap_ids {
            let status = api.get_swap_status(swap_id).await.unwrap();
            assert_eq!(status.state, SwapState::BothFunded);
        }

        // Complete one swap
        let first_swap = monitor.active_swaps.read().await
            .get(&swap_ids[0])
            .cloned()
            .unwrap();

        if let Some(secret) = first_swap.secret {
            monitor.handle_bitcoin_event(SwapEvent::BitcoinSecretRevealed {
                swap_id: swap_ids[0],
                secret,
                txid: "claim_tx".to_string(),
            }).await.unwrap();
        }

        // Check only first swap is completed
        let status = api.get_swap_status(swap_ids[0]).await.unwrap();
        assert_eq!(status.state, SwapState::Claimed);

        // Others should still be active
        for &swap_id in &swap_ids[1..] {
            let status = api.get_swap_status(swap_id).await.unwrap();
            assert_eq!(status.state, SwapState::BothFunded);
        }
    }
}

#[cfg(test)]
mod error_recovery_tests {
    use super::*;

    #[tokio::test]
    async fn test_recovery_from_partial_failure() {
        let config = create_test_config();
        let monitor = create_test_monitor();
        let api = Arc::new(AtomicSwapRpcImpl::new(
            config,
            monitor.clone(),
            None,
        ));

        // Create swap
        let params = InitiateSwapParams {
            bitcoin_amount: 100_000,
            nova_amount: 1_000_000_000,
            bitcoin_counterparty: "tb1qtest".to_string(),
            nova_counterparty: "nova1test".to_string(),
            timeout_minutes: 60,
            memo: Some("Recovery test".to_string()),
        };

        let session = api.initiate_swap(params).await.unwrap();
        let swap_id = session.setup.swap_id;

        // Simulate partial funding (only Bitcoin side)
        monitor.handle_bitcoin_event(SwapEvent::BitcoinHTLCFunded {
            swap_id,
            txid: "btc_tx".to_string(),
            amount: 100_000,
            confirmations: 6,
        }).await.unwrap();

        // Simulate network issue - no Supernova funding
        // Wait for safety timeout
        tokio::time::sleep(Duration::from_secs(2)).await;

        // Should still be able to query status
        let status = api.get_swap_status(swap_id).await.unwrap();
        assert_eq!(status.state, SwapState::BothFunded);
        assert!(status.can_refund); // Should be refundable

        // Cancel the swap
        let cancelled = api.cancel_swap(swap_id).await.unwrap();
        assert!(cancelled);

        // Verify state
        let final_status = api.get_swap_status(swap_id).await.unwrap();
        assert!(matches!(final_status.state, SwapState::Failed(_)));
    }
}