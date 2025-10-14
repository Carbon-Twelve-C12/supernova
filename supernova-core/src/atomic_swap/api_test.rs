//! Integration tests for atomic swap RPC API

#[cfg(test)]
mod tests {
    use super::super::*;
    use crate::atomic_swap::monitor::{CrossChainMonitor, MonitorConfig};
    use std::sync::Arc;

    fn create_test_api() -> AtomicSwapRpcImpl {
        let config = crate::atomic_swap::AtomicSwapConfig::default();
        let monitor_config = MonitorConfig::default();
        let monitor = Arc::new(CrossChainMonitor::new(monitor_config, None));

        AtomicSwapRpcImpl::new(
            config,
            monitor,
            #[cfg(feature = "atomic-swap")]
            None,
        )
    }

    #[tokio::test]
    async fn test_initiate_swap() {
        let api = create_test_api();

        let params = InitiateSwapParams {
            bitcoin_amount: 100000, // 0.001 BTC
            nova_amount: 1000000000, // 10 NOVA
            bitcoin_counterparty: "bc1qtest".to_string(),
            nova_counterparty: "nova1test".to_string(),
            timeout_minutes: 60,
        };

        let result = api.initiate_swap(params).await;
        assert!(result.is_ok());

        let session = result.unwrap();
        assert_eq!(session.state, SwapState::Active);
        assert_eq!(session.setup.bitcoin_amount, 100000);
        assert_eq!(session.setup.nova_amount, 1000000000);
    }

    #[tokio::test]
    async fn test_get_swap_status() {
        let api = create_test_api();

        // First create a swap
        let params = InitiateSwapParams {
            bitcoin_amount: 100000,
            nova_amount: 1000000000,
            bitcoin_counterparty: "bc1qtest".to_string(),
            nova_counterparty: "nova1test".to_string(),
            timeout_minutes: 60,
        };

        let session = api.initiate_swap(params).await.unwrap();
        let swap_id = session.setup.swap_id;

        // Get status
        let status = api.get_swap_status(swap_id).await.unwrap();
        assert_eq!(status.swap_id, hex::encode(&swap_id));
        assert_eq!(status.state, SwapState::Active);
        assert_eq!(status.bitcoin_amount, 100000);
        assert_eq!(status.nova_amount, 1000000000);
    }

    #[tokio::test]
    async fn test_claim_swap() {
        let api = create_test_api();

        // Create a swap
        let params = InitiateSwapParams {
            bitcoin_amount: 100000,
            nova_amount: 1000000000,
            bitcoin_counterparty: "bc1qtest".to_string(),
            nova_counterparty: "nova1test".to_string(),
            timeout_minutes: 60,
        };

        let session = api.initiate_swap(params).await.unwrap();
        let swap_id = session.setup.swap_id;

        // Get the correct secret
        let secret = session.nova_htlc.hash_lock.preimage.expect("Should have preimage");

        // Claim the swap
        let result = api.claim_swap(swap_id, secret).await;
        assert!(result.is_ok());

        let tx_id = result.unwrap();
        assert!(tx_id.txid.starts_with("nova_claim_"));

        // Verify state changed
        let status = api.get_swap_status(swap_id).await.unwrap();
        assert_eq!(status.state, SwapState::Claimed);
    }

    #[tokio::test]
    async fn test_invalid_secret_claim() {
        let api = create_test_api();

        // Create a swap
        let params = InitiateSwapParams {
            bitcoin_amount: 100000,
            nova_amount: 1000000000,
            bitcoin_counterparty: "bc1qtest".to_string(),
            nova_counterparty: "nova1test".to_string(),
            timeout_minutes: 60,
        };

        let session = api.initiate_swap(params).await.unwrap();
        let swap_id = session.setup.swap_id;

        // Try to claim with wrong secret
        let wrong_secret = [99u8; 32];
        let result = api.claim_swap(swap_id, wrong_secret).await;
        assert!(result.is_err());

        // Verify state is still Active
        let status = api.get_swap_status(swap_id).await.unwrap();
        assert_eq!(status.state, SwapState::Active);
    }

    #[tokio::test]
    async fn test_list_swaps() {
        let api = create_test_api();

        // Create multiple swaps
        for i in 0..3 {
            let params = InitiateSwapParams {
                bitcoin_amount: 100000 * (i + 1),
                nova_amount: 1000000000 * (i + 1),
                bitcoin_counterparty: format!("bc1qtest{}", i),
                nova_counterparty: format!("nova1test{}", i),
                timeout_minutes: 60,
            };

            api.initiate_swap(params).await.unwrap();
        }

        // List all swaps
        let filter = SwapFilter {
            state: None,
            from_timestamp: None,
            to_timestamp: None,
            counterparty: None,
        };

        let swaps = api.list_swaps(filter).await.unwrap();
        assert_eq!(swaps.len(), 3);

        // List only active swaps
        let filter = SwapFilter {
            state: Some(SwapState::Active),
            from_timestamp: None,
            to_timestamp: None,
            counterparty: None,
        };

        let active_swaps = api.list_swaps(filter).await.unwrap();
        assert_eq!(active_swaps.len(), 3);
        assert!(active_swaps.iter().all(|s| s.state == SwapState::Active));
    }

    #[tokio::test]
    async fn test_get_swap_events() {
        let api = create_test_api();

        // Create a swap
        let params = InitiateSwapParams {
            bitcoin_amount: 100000,
            nova_amount: 1000000000,
            bitcoin_counterparty: "bc1qtest".to_string(),
            nova_counterparty: "nova1test".to_string(),
            timeout_minutes: 60,
        };

        let session = api.initiate_swap(params).await.unwrap();
        let swap_id = session.setup.swap_id;

        // Get events
        let events = api.get_swap_events(swap_id).await.unwrap();
        assert!(!events.is_empty());

        // Should have at least the initiation event
        let has_init_event = events.iter().any(|e| {
            matches!(e, crate::atomic_swap::monitor::SwapEvent::SwapInitiated { .. })
        });
        assert!(has_init_event);
    }
}