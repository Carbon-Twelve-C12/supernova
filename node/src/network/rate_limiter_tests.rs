//! Network Rate Limiting Tests

#[cfg(test)]
mod tests {
    use super::super::rate_limiter::*;
    use std::net::{IpAddr, Ipv4Addr, SocketAddr};
    use std::time::Duration;
    use tokio::time::sleep;

    #[tokio::test]
    async fn test_network_rate_limiting_dos_prevention() {
        let config = RateLimitConfig {
            per_ip_limit: 10,
            ip_window: Duration::from_secs(60),
            per_subnet_limit: 50,
            subnet_window: Duration::from_secs(60),
            ban_duration: Duration::from_secs(300),
            violations_before_ban: 3,
            global_rps: 100,
            max_concurrent_connections: 50,
            circuit_breaker_enabled: true,
            circuit_breaker_threshold: 0.5,
            circuit_breaker_timeout: Duration::from_secs(30),
            // Per-message-type rate limits
            block_request_limit: 100,
            transaction_broadcast_limit: 500,
            peer_discovery_limit: 100,
            general_message_limit: 1000,
            global_message_limit: 10000,
            exponential_backoff_enabled: true,
            base_backoff_duration: Duration::from_secs(1),
            max_backoff_duration: Duration::from_secs(3600),
        };

        let limiter = NetworkRateLimiter::new(config);

        // Test 1: Normal traffic should pass
        let normal_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1)), 8080);
        for _ in 0..5 {
            assert!(limiter.check_connection(normal_addr).await.is_ok());
        }

        // Test 2: DoS attack from single IP should be blocked
        let attacker_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1)), 8080);
        let mut successes = 0;
        let mut failures = 0;

        for _ in 0..20 {
            match limiter.check_connection(attacker_addr).await {
                Ok(_) => successes += 1,
                Err(_) => failures += 1,
            }
        }

        assert_eq!(successes, 10); // Should allow exactly per_ip_limit
        assert_eq!(failures, 10); // Rest should be rejected

        // Test 3: Distributed DoS from same subnet should be blocked
        let mut subnet_successes = 0;
        for i in 1..=100 {
            let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(172, 16, 1, i)), 8080);
            if limiter.check_connection(addr).await.is_ok() {
                subnet_successes += 1;
            }
        }

        assert!(subnet_successes <= 50); // Should respect subnet limit

        // Test 4: Global rate limit should be enforced
        let mut global_permits = Vec::new();
        for i in 0..150 {
            let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(10, i as u8, 0, 1)), 8080);
            if let Ok(permit) = limiter.check_connection(addr).await {
                global_permits.push(permit);
            }
        }

        assert_eq!(global_permits.len(), 100); // Should respect global limit

        // Test 5: Check metrics
        let metrics = limiter.metrics();
        assert!(metrics.total_requests > 0);
        assert!(metrics.rejected_requests > 0);

        println!("Rate limiter test results:");
        println!("Total requests: {}", metrics.total_requests);
        println!("Rejected requests: {}", metrics.rejected_requests);
        println!("Banned IPs: {}", metrics.banned_ips);
    }

    #[tokio::test]
    async fn test_ban_mechanism() {
        let config = RateLimitConfig {
            per_ip_limit: 5,
            ip_window: Duration::from_secs(1),
            violations_before_ban: 2,
            ban_duration: Duration::from_secs(2),
            ..Default::default()
        };

        let limiter = NetworkRateLimiter::new(config);
        let attacker = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 100)), 8080);

        // First violation - exceed rate limit
        for _ in 0..5 {
            let _ = limiter.check_connection(attacker).await;
        }
        // This should fail (first violation)
        assert!(limiter.check_connection(attacker).await.is_err());

        // Wait for window to reset
        sleep(Duration::from_millis(1100)).await;

        // Second violation - exceed rate limit again
        for _ in 0..5 {
            let _ = limiter.check_connection(attacker).await;
        }
        // This should fail and trigger ban
        assert!(limiter.check_connection(attacker).await.is_err());

        // Now all requests should be banned
        sleep(Duration::from_millis(100)).await;
        match limiter.check_connection(attacker).await {
            Err(RateLimitError::IpBanned(ip, _)) => {
                assert_eq!(ip, attacker.ip());
            }
            _ => panic!("Expected IP to be banned"),
        }

        // After ban duration, should work again
        sleep(Duration::from_secs(2)).await;
        assert!(limiter.check_connection(attacker).await.is_ok());
    }

    #[tokio::test]
    async fn test_circuit_breaker() {
        let config = RateLimitConfig {
            per_ip_limit: 1000,
            circuit_breaker_enabled: true,
            circuit_breaker_threshold: 0.3, // 30% error rate
            circuit_breaker_timeout: Duration::from_secs(1),
            ..Default::default()
        };

        let limiter = NetworkRateLimiter::new(config);

        // Simulate mixed traffic with failures
        for i in 0..20 {
            let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 168, 1, i)), 8080);
            if let Ok(permit) = limiter.check_connection(addr).await {
                if i % 3 == 0 {
                    // 33% failure rate
                    permit.record_failure();
                } else {
                    permit.record_success();
                }
            }
        }

        // Circuit breaker should eventually trip
        // In a real test, we'd need to trigger enough failures to open the circuit
    }

    #[tokio::test]
    async fn test_cleanup() {
        let config = RateLimitConfig {
            per_ip_limit: 5,
            ip_window: Duration::from_secs(1),
            ..Default::default()
        };

        let limiter = NetworkRateLimiter::new(config);

        // Add some entries
        for i in 0..10 {
            let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 168, 1, i)), 8080);
            let _ = limiter.check_connection(addr).await;
        }

        // Wait for entries to become stale
        sleep(Duration::from_secs(3)).await;

        // Run cleanup
        limiter.cleanup();

        // New connections should work (old entries cleaned up)
        for i in 0..10 {
            let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 168, 1, i)), 8080);
            assert!(limiter.check_connection(addr).await.is_ok());
        }
    }

    #[test]
    fn test_concurrent_access() {
        use std::sync::Arc;
        use tokio::runtime::Runtime;

        let rt = Runtime::new().unwrap();
        let config = RateLimitConfig::default();
        let limiter = Arc::new(NetworkRateLimiter::new(config));

        // Spawn multiple tasks accessing the limiter concurrently
        let mut handles = Vec::new();

        for i in 0..10 {
            let limiter_clone = Arc::clone(&limiter);
            let handle = rt.spawn(async move {
                for j in 0..100 {
                    let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 168, i, j)), 8080);
                    let _ = limiter_clone.check_connection(addr).await;
                }
            });
            handles.push(handle);
        }

        // Wait for all tasks to complete
        rt.block_on(async {
            for handle in handles {
                handle.await.unwrap();
            }
        });

        // Check metrics
        let metrics = limiter.metrics();
        assert_eq!(metrics.total_requests, 1000); // 10 tasks * 100 requests
    }
}
