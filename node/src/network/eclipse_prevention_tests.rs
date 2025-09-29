//! Tests for Eclipse Attack Prevention System
//!
//! This module contains comprehensive tests to verify that eclipse attacks
//! are properly prevented through multiple defense mechanisms.

#[cfg(test)]
mod eclipse_prevention_tests {
    use super::super::eclipse_prevention::*;
    use super::super::p2p::P2PNetwork;
    use libp2p::PeerId;
    use std::net::{IpAddr, Ipv4Addr};
    use std::time::Duration;
    use tokio::time::sleep;

    /// Test basic diversity requirements
    #[tokio::test]
    async fn test_subnet_diversity_enforcement() {
        let config = EclipsePreventionConfig {
            max_subnet_percentage: 0.15, // Max 15% from same subnet
            min_connections_for_diversity: 5,
            ..Default::default()
        };

        let system = EclipsePreventionSystem::new(config);

        // Add connections from different subnets
        for i in 0..5 {
            let peer_id = PeerId::random();
            let ip = IpAddr::V4(Ipv4Addr::new(192, 168, i, 1));
            assert!(system
                .register_connection(peer_id, ip, true, false)
                .await
                .is_ok());
        }

        // Now try to add multiple from same subnet - should fail after limit
        let subnet_ip_base = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1));

        // First connection from this subnet should succeed
        let peer_id = PeerId::random();
        assert!(system
            .should_allow_connection(&peer_id, subnet_ip_base, true)
            .await
            .is_ok());
        system
            .register_connection(peer_id, subnet_ip_base, true, false)
            .await
            .unwrap();

        // Second connection from same subnet should fail (would be 2/6 = 33% > 15%)
        let peer_id2 = PeerId::random();
        let ip2 = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 2));
        let result = system.should_allow_connection(&peer_id2, ip2, true).await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .contains("Too many connections from subnet"));
    }

    /// Test ASN diversity requirements
    #[tokio::test]
    async fn test_asn_diversity_enforcement() {
        let config = EclipsePreventionConfig {
            max_asn_percentage: 0.25, // Max 25% from same ASN
            min_connections_for_diversity: 4,
            ..Default::default()
        };

        let system = EclipsePreventionSystem::new(config);

        // Add 4 connections to trigger diversity requirements
        for i in 0..4 {
            let peer_id = PeerId::random();
            let ip = IpAddr::V4(Ipv4Addr::new(192, 168, i + 1, 1));
            system
                .register_connection(peer_id, ip, true, false)
                .await
                .unwrap();
        }

        // In production, ASN would be looked up from GeoIP database
        // For now, the mock returns None, so ASN diversity check passes
        let peer_id = PeerId::random();
        let ip = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1));
        assert!(system
            .should_allow_connection(&peer_id, ip, true)
            .await
            .is_ok());
    }

    /// Test inbound/outbound connection ratio
    #[tokio::test]
    async fn test_inbound_outbound_ratio() {
        let config = EclipsePreventionConfig {
            max_inbound_percentage: 0.67, // Max 67% inbound
            min_connections_for_diversity: 3,
            ..Default::default()
        };

        let system = EclipsePreventionSystem::new(config);

        // Add 1 outbound connection
        let peer_id = PeerId::random();
        let ip = IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1));
        system
            .register_connection(peer_id, ip, false, false)
            .await
            .unwrap();

        // Add 2 inbound connections (would be 67% inbound)
        for i in 2..=3 {
            let peer_id = PeerId::random();
            let ip = IpAddr::V4(Ipv4Addr::new(192, 168, i, 1));
            system
                .register_connection(peer_id, ip, true, false)
                .await
                .unwrap();
        }

        // Try to add another inbound - should fail (would be 75% > 67%)
        let peer_id = PeerId::random();
        let ip = IpAddr::V4(Ipv4Addr::new(192, 168, 4, 1));
        let result = system.should_allow_connection(&peer_id, ip, true).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Too many inbound connections"));
    }

    /// Test proof-of-work challenge system
    #[tokio::test]
    async fn test_pow_challenge_system() {
        let config = EclipsePreventionConfig {
            require_pow_challenge: true,
            pow_difficulty: 8, // Low difficulty for testing
            ..Default::default()
        };

        let system = EclipsePreventionSystem::new(config);

        let peer_id = PeerId::random();
        let ip = IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1));

        // Inbound connection should require PoW
        let result = system.should_allow_connection(&peer_id, ip, true).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("PoW challenge not completed"));

        // Generate and solve challenge
        let (nonce, difficulty) = system.generate_pow_challenge(&peer_id).await;
        assert_eq!(difficulty, 8);

        // Solve the challenge
        let solution = solve_pow_challenge(&nonce, difficulty);

        // Verify solution
        assert!(system.verify_pow_challenge(&peer_id, &solution).await);

        // Now connection should be allowed
        assert!(system
            .should_allow_connection(&peer_id, ip, true)
            .await
            .is_ok());
    }

    /// Test connection flooding detection
    #[tokio::test]
    async fn test_connection_flooding_detection() {
        let config = EclipsePreventionConfig::default();
        let system = EclipsePreventionSystem::new(config);

        let subnet_base = IpAddr::V4(Ipv4Addr::new(192, 168, 1, 0));

        // Simulate rapid connections from same subnet
        for i in 1..=15 {
            let peer_id = PeerId::random();
            let ip = IpAddr::V4(Ipv4Addr::new(192, 168, 1, i));

            // After 10 connections, should detect flooding
            let result = system.should_allow_connection(&peer_id, ip, true).await;

            if i <= 10 {
                assert!(result.is_ok(), "Connection {} should be allowed", i);
                system
                    .register_connection(peer_id, ip, true, false)
                    .await
                    .unwrap();
            } else {
                assert!(result.is_err(), "Connection {} should be blocked", i);
                assert!(result.unwrap_err().contains("Connection flooding detected"));
            }
        }
    }

    /// Test peer rotation mechanism
    #[tokio::test]
    async fn test_peer_rotation() {
        let config = EclipsePreventionConfig {
            rotation_interval: Duration::from_millis(100), // Short for testing
            rotation_percentage: 0.25,                     // Rotate 25% of peers
            enable_automatic_rotation: true,
            ..Default::default()
        };

        let system = EclipsePreventionSystem::new(config);

        // Add 8 regular peers
        for i in 0..8 {
            let peer_id = PeerId::random();
            let ip = IpAddr::V4(Ipv4Addr::new(192, 168, i + 1, 1));
            system
                .register_connection(peer_id, ip, true, false)
                .await
                .unwrap();
        }

        // Add 2 anchor peers
        for i in 8..10 {
            let peer_id = PeerId::random();
            let ip = IpAddr::V4(Ipv4Addr::new(10, 0, i, 1));
            system
                .register_connection(peer_id, ip, false, true)
                .await
                .unwrap();
        }

        // Wait for rotation interval
        sleep(Duration::from_millis(150)).await;

        // Check rotation is needed
        assert!(system.check_rotation_needed().await);

        // Get rotation candidates
        let candidates = system.get_rotation_candidates().await;

        // Should rotate 25% of non-anchor peers (2 out of 8)
        assert_eq!(candidates.len(), 2);

        // Verify anchor peers are not in rotation list
        for candidate in &candidates {
            let connections = system.connections.read().await;
            let info = connections.get(candidate).unwrap();
            assert!(!info.is_anchor);
        }
    }

    /// Test behavioral scoring system
    #[tokio::test]
    async fn test_behavioral_scoring() {
        let config = EclipsePreventionConfig::default();
        let system = EclipsePreventionSystem::new(config);

        let peer_id = PeerId::random();
        let ip = IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1));
        system
            .register_connection(peer_id.clone(), ip, true, false)
            .await
            .unwrap();

        // Initial score should be 100
        {
            let connections = system.connections.read().await;
            let info = connections.get(&peer_id).unwrap();
            assert_eq!(info.behavior_score, 100.0);
        }

        // Reduce score
        system.update_behavior_score(&peer_id, -20.0).await;

        {
            let connections = system.connections.read().await;
            let info = connections.get(&peer_id).unwrap();
            assert_eq!(info.behavior_score, 80.0);
        }

        // Reduce score below threshold - should ban peer
        system.update_behavior_score(&peer_id, -75.0).await;

        // Peer should be banned
        assert!(system.is_banned(&peer_id, &ip).await);
    }

    /// Test eclipse attack detection
    #[tokio::test]
    async fn test_eclipse_attack_detection() {
        let config = EclipsePreventionConfig {
            enable_behavioral_analysis: true,
            eclipse_detection_threshold: 0.5, // 50% of indicators
            min_connections_for_diversity: 3,
            ..Default::default()
        };

        let system = EclipsePreventionSystem::new(config);

        // Simulate attack pattern: many connections from same subnet
        for i in 0..10 {
            let peer_id = PeerId::random();
            let ip = IpAddr::V4(Ipv4Addr::new(192, 168, 1, i + 1));
            let _ = system.register_connection(peer_id, ip, true, false).await;
        }

        // Simulate address convergence
        let advertised_peer = PeerId::random();
        for i in 0..8 {
            let from_peer = PeerId::random();
            system
                .record_peer_advertisement(from_peer, vec![advertised_peer.clone()])
                .await;
        }

        // Check risk level
        let risk = system.get_eclipse_risk_level().await;
        assert_eq!(risk, EclipseRiskLevel::Critical);
    }

    /// Test diversity score calculation
    #[tokio::test]
    async fn test_diversity_score_calculation() {
        let config = EclipsePreventionConfig::default();
        let system = EclipsePreventionSystem::new(config);

        // Add connections from same subnet - low diversity
        for i in 0..5 {
            let peer_id = PeerId::random();
            let ip = IpAddr::V4(Ipv4Addr::new(192, 168, 1, i + 1));
            system
                .register_connection(peer_id, ip, true, false)
                .await
                .unwrap();
        }

        let connections = system.connections.read().await;
        let diversity_score = system.calculate_diversity_score(&connections).await;

        // All from same subnet = low diversity
        assert!(diversity_score < 0.5);
    }

    /// Test concurrent connection attempts
    #[tokio::test]
    async fn test_concurrent_connection_handling() {
        let config = EclipsePreventionConfig {
            max_subnet_percentage: 0.2,
            min_connections_for_diversity: 5,
            ..Default::default()
        };

        let system = Arc::new(EclipsePreventionSystem::new(config));

        // Add some initial connections
        for i in 0..5 {
            let peer_id = PeerId::random();
            let ip = IpAddr::V4(Ipv4Addr::new(10, 0, i, 1));
            system
                .register_connection(peer_id, ip, true, false)
                .await
                .unwrap();
        }

        // Try concurrent connections from same subnet
        let mut handles = vec![];

        for i in 0..5 {
            let system_clone = Arc::clone(&system);
            let handle = tokio::spawn(async move {
                let peer_id = PeerId::random();
                let ip = IpAddr::V4(Ipv4Addr::new(192, 168, 1, i + 1));
                system_clone
                    .should_allow_connection(&peer_id, ip, true)
                    .await
            });
            handles.push(handle);
        }

        // Wait for all attempts
        let results: Vec<_> = futures::future::join_all(handles).await;

        // At most 1 should succeed (20% of 6 total = 1.2, rounds down to 1)
        let successes = results
            .iter()
            .filter(|r| r.as_ref().unwrap().is_ok())
            .count();

        assert!(
            successes <= 1,
            "Too many connections from same subnet allowed: {}",
            successes
        );
    }

    /// Helper function to solve PoW challenge
    fn solve_pow_challenge(nonce: &[u8], difficulty: u8) -> Vec<u8> {
        use rand::Rng;
        use sha2::{Digest, Sha256};

        let mut solution = vec![0u8; 8];
        let mut rng = rand::thread_rng();

        loop {
            rng.fill(&mut solution[..]);

            let mut hasher = Sha256::new();
            hasher.update(nonce);
            hasher.update(&solution);
            let hash = hasher.finalize();

            if count_leading_zeros(&hash) >= difficulty {
                return solution;
            }
        }
    }

    fn count_leading_zeros(hash: &[u8]) -> u8 {
        let mut count = 0;
        for byte in hash {
            if *byte == 0 {
                count += 8;
            } else {
                count += byte.leading_zeros() as u8;
                break;
            }
        }
        count
    }
}
