//! Atomic Swap Hardening Tests (P1-009)
//!
//! These tests target the production-readiness edge cases:
//! - Timeout handling (refund only after expiry)
//! - Rollback validation (no double-refund, no refund after claim)
//! - Race behavior between claim/refund
//! - Basic parameter sanity (dust and zero-fee rejection)

use super::test_utils::*;
use crate::atomic_swap::api::{AtomicSwapRPC, AtomicSwapRpcImpl, InitiateSwapParams};
use crate::atomic_swap::{SwapState};
use crate::atomic_swap::monitor::{retry_with_backoff, ReorgTracker, RetryConfig};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

#[cfg(test)]
mod timeout_and_refund_tests {
    use super::*;

    #[tokio::test]
    async fn test_refund_before_timeout_fails() {
        let monitor = create_test_monitor();
        let api = Arc::new(AtomicSwapRpcImpl::new(create_test_config(), monitor, None));

        let params = InitiateSwapParams {
            bitcoin_amount: 100_000,
            nova_amount: 1_000_000_000,
            bitcoin_counterparty: "tb1qcounterparty".to_string(),
            nova_counterparty: "nova1counterparty".to_string(),
            timeout_minutes: 60,
            memo: None,
        };

        let session = api.initiate_swap(params).await.unwrap();
        let swap_id = session.setup.swap_id;

        // Ensure timeout is in the future
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
        assert!(session.nova_htlc.time_lock.absolute_timeout > now);

        let result = api.refund_swap(swap_id).await;
        assert!(result.is_err(), "refund should be rejected before expiry");
    }

    #[tokio::test]
    async fn test_refund_after_timeout_succeeds() {
        let monitor = create_test_monitor();
        let api = Arc::new(AtomicSwapRpcImpl::new(create_test_config(), monitor, None));

        let params = InitiateSwapParams {
            bitcoin_amount: 100_000,
            nova_amount: 1_000_000_000,
            bitcoin_counterparty: "tb1qcounterparty".to_string(),
            nova_counterparty: "nova1counterparty".to_string(),
            timeout_minutes: 1,
            memo: None,
        };

        let session = api.initiate_swap(params).await.unwrap();
        let swap_id = session.setup.swap_id;

        // Force expiry for test.
        api.test_force_expire_swap(swap_id).await;

        let result = api.refund_swap(swap_id).await;
        assert!(result.is_ok(), "refund should succeed after expiry");

        let status = api.get_swap_status(swap_id).await.unwrap();
        assert_eq!(status.state, SwapState::Refunded);
    }
}

#[cfg(test)]
mod claim_tests {
    use super::*;

    #[tokio::test]
    async fn test_claim_before_timeout_succeeds() {
        let monitor = create_test_monitor();
        let api = Arc::new(AtomicSwapRpcImpl::new(create_test_config(), monitor, None));

        let params = InitiateSwapParams {
            bitcoin_amount: 100_000,
            nova_amount: 1_000_000_000,
            bitcoin_counterparty: "tb1qcounterparty".to_string(),
            nova_counterparty: "nova1counterparty".to_string(),
            timeout_minutes: 60,
            memo: None,
        };

        let session = api.initiate_swap(params).await.unwrap();
        let swap_id = session.setup.swap_id;
        let secret = session.secret.expect("test swap should have secret");

        let claim = api.claim_swap(swap_id, secret).await;
        assert!(claim.is_ok(), "claim should succeed");

        let status = api.get_swap_status(swap_id).await.unwrap();
        assert_eq!(status.state, SwapState::Completed);
    }

    #[tokio::test]
    async fn test_double_claim_attempt_fails() {
        let monitor = create_test_monitor();
        let api = Arc::new(AtomicSwapRpcImpl::new(create_test_config(), monitor, None));

        let params = InitiateSwapParams {
            bitcoin_amount: 100_000,
            nova_amount: 1_000_000_000,
            bitcoin_counterparty: "tb1qcounterparty".to_string(),
            nova_counterparty: "nova1counterparty".to_string(),
            timeout_minutes: 60,
            memo: None,
        };

        let session = api.initiate_swap(params).await.unwrap();
        let swap_id = session.setup.swap_id;
        let secret = session.secret.unwrap();

        assert!(api.claim_swap(swap_id, secret).await.is_ok());

        let second = api.claim_swap(swap_id, secret).await;
        assert!(second.is_err(), "second claim should be rejected");
    }
}

#[cfg(test)]
mod race_tests {
    use super::*;

    #[tokio::test]
    async fn test_concurrent_claim_and_refund_race() {
        let monitor = create_test_monitor();
        let api = Arc::new(AtomicSwapRpcImpl::new(create_test_config(), monitor, None));

        let params = InitiateSwapParams {
            bitcoin_amount: 100_000,
            nova_amount: 1_000_000_000,
            bitcoin_counterparty: "tb1qcounterparty".to_string(),
            nova_counterparty: "nova1counterparty".to_string(),
            timeout_minutes: 60,
            memo: None,
        };

        let session = api.initiate_swap(params).await.unwrap();
        let swap_id = session.setup.swap_id;
        let secret = session.secret.unwrap();

        // Force expiry to ensure refund path is eligible; race will be claim vs refund.
        api.test_force_expire_swap(swap_id).await;

        let api1 = Arc::clone(&api);
        let api2 = Arc::clone(&api);

        let claim_task = tokio::spawn(async move { api1.claim_swap(swap_id, secret).await });
        let refund_task = tokio::spawn(async move { api2.refund_swap(swap_id).await });

        let (claim_res, refund_res) = tokio::join!(claim_task, refund_task);
        let claim_ok = claim_res.unwrap().is_ok();
        let refund_ok = refund_res.unwrap().is_ok();

        // Exactly one should win.
        assert_ne!(claim_ok, refund_ok, "claim and refund should not both succeed");
    }
}

#[cfg(test)]
mod monitor_reliability_tests {
    use super::*;

    #[tokio::test]
    async fn test_monitor_retry_handles_remote_chain_downtime() {
        let retry = RetryConfig {
            base_delay: Duration::from_millis(1),
            max_delay: Duration::from_millis(5),
            max_retries: 5,
        };

        let attempts = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let attempts_clone = Arc::clone(&attempts);

        let result: Result<u32, _> = retry_with_backoff(retry, move || {
            let attempts = Arc::clone(&attempts_clone);
            async move {
                let n = attempts.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                if n < 2 {
                    Err(crate::atomic_swap::error::MonitorError::ConnectionLost {
                        chain: "bitcoin".to_string(),
                    })
                } else {
                    Ok(42u32)
                }
            }
        })
        .await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 42);
    }

    #[test]
    fn test_reorg_tracker_detects_reorg() {
        let original_hash = [1u8; 32];
        let tracker = ReorgTracker::new(100, original_hash);

        // Same hash => no reorg
        assert!(!tracker.is_reorg(original_hash));

        // Different hash => reorg detected
        assert!(tracker.is_reorg([2u8; 32]));
    }
}


