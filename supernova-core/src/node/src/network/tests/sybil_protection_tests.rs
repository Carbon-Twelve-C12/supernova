//! Tests for Sybil attack protection mechanisms

use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr};
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

use libp2p::PeerId;
use rand::Rng;

use crate::network::peer_manager::{PeerManager, PeerInfo, IpSubnet, PeerScore, PeerBehavior, ChallengeStatus};
use crate::network::p2p::{ConnectionManager, EclipsePreventionConfig, ConnectionDirection, NetworkDiversityTracker};

/// Test basic peer scoring functionality
#[tokio::test]
async fn test_peer_behavior_scoring() {
    // Create a peer with some behavior history
    let mut peer_behavior = PeerBehavior::new();

    // Initial score should be perfect
    assert_eq!(peer_behavior.reliability_score(), 1.0);

    // Record some good behavior
    peer_behavior.valid_blocks_announced = 10;
    peer_behavior.valid_txns_relayed = 20;

    // Score should still be perfect
    assert_eq!(peer_behavior.reliability_score(), 1.0);

    // Record some bad behavior
    peer_behavior.invalid_blocks_announced = 2;  // 2/12 = ~16.7% invalid
    peer_behavior.protocol_violations = 1;

    // Score should be reduced
    let score = peer_behavior.reliability_score();
    assert!(score < 1.0);
    assert!(score > 0.7); // Not too severe penalty

    // More severe violations
    peer_behavior.protocol_violations = 5;
    peer_behavior.unusual_patterns_detected = vec!["Spam".to_string(), "Invalid format".to_string()];

    // Score should be further reduced
    let new_score = peer_behavior.reliability_score();
    assert!(new_score < score);
}

/// Test challenge generation and verification
#[tokio::test]
async fn test_identity_challenge_verification() {
    let mut peer_manager = PeerManager::new();

    // Enable challenges
    peer_manager.enable_connection_challenges = true;
    peer_manager.challenge_difficulty = 8; // Lower for testing

    // Create a peer
    let peer_id = PeerId::random();
    let ip = IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1));

    // Register peer
    let result = peer_manager.try_add_connection(peer_id, ip, 8333);

    // Should require challenge
    assert!(matches!(result, Ok(false)));

    // Generate a challenge
    let challenge = peer_manager.issue_challenge(&peer_id).expect("Failed to issue challenge");

    // Create a valid response (simplified for test)
    let mut hasher = sha2::Sha256::new();
    hasher.update(&challenge);
    let mut nonce = 0u64;
    let mut solution = String::new();

    // Simple proof of work - find a nonce that produces required leading zeros
    loop {
        let nonce_bytes = nonce.to_le_bytes();
        let mut trial_hasher = hasher.clone();
        trial_hasher.update(&nonce_bytes);
        let hash = trial_hasher.finalize();

        // Check if it meets difficulty
        if hash[0] <= 1 << (8 - peer_manager.challenge_difficulty) {
            solution = format!("{}{}", hex::encode(hash), hex::encode(nonce_bytes));
            break;
        }

        nonce += 1;

        // Prevent infinite loop in test
        if nonce > 1_000_000 {
            panic!("Could not find solution in reasonable time");
        }
    }

    // Process the response
    let verification = peer_manager.process_challenge_response(&peer_id, &solution);
    assert!(verification, "Challenge verification failed");

    // Check peer status
    if let Some(peer) = peer_manager.get_peer_info(&peer_id) {
        assert!(matches!(peer.challenge_status, ChallengeStatus::Verified { .. }));
    } else {
        panic!("Peer not found after verification");
    }

    // Now connection should be accepted
    let result = peer_manager.try_add_connection(peer_id, ip, 8333);
    assert!(matches!(result, Ok(true)));
}

/// Test subnet diversity limits
#[tokio::test]
async fn test_subnet_diversity_limits() {
    let mut peer_manager = PeerManager::new();
    let mut peers = Vec::new();

    // Create peers from the same subnet
    let subnet_ip = Ipv4Addr::new(192, 168, 1, 0);
    let max_peers = peer_manager.diversity_manager.max_peers_per_subnet;

    // Should accept up to max_peers
    for i in 1..=max_peers+1 {
        let peer_id = PeerId::random();
        let ip = IpAddr::V4(Ipv4Addr::new(192, 168, 1, i as u8));

        // Disable challenges for this test
        peer_manager.enable_connection_challenges = false;

        let result = peer_manager.try_add_connection(peer_id, ip, 8333);

        if i <= max_peers {
            assert!(result.is_ok(), "Should accept peer {} from subnet", i);
            peers.push(peer_id);
        } else {
            assert!(result.is_err(), "Should reject peer {} from subnet", i);
        }
    }

    // Now create a peer from different subnet
    let peer_id = PeerId::random();
    let ip = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1));

    let result = peer_manager.try_add_connection(peer_id, ip, 8333);
    assert!(result.is_ok(), "Should accept peer from different subnet");
}

/// Test network diversity tracking
#[tokio::test]
async fn test_network_diversity_tracking() {
    let mut tracker = NetworkDiversityTracker::new();

    // Create diverse peers
    let mut peers = Vec::new();

    // Add peers from different subnets
    for i in 1..=5 {
        let peer_id = PeerId::random();
        let ip = IpAddr::V4(Ipv4Addr::new(10, i, 0, 1));
        let mut peer = PeerInfo::new(peer_id, ip, 8333);

        // Add ASN and region
        peer.asn = Some(64000 + i);
        peer.region = Some(format!("Region-{}", i % 3 + 1));

        tracker.add_peer(&peer);
        peers.push(peer);
    }

    // Calculate diversity score
    let score = tracker.calculate_diversity_score();

    // Should have good diversity
    assert!(score > 0.7, "Diversity score should be high for diverse network");

    // Now add many peers from the same subnet/ASN/region
    for i in 1..=10 {
        let peer_id = PeerId::random();
        let ip = IpAddr::V4(Ipv4Addr::new(192, 168, 1, i));
        let mut peer = PeerInfo::new(peer_id, ip, 8333);

        // Same ASN and region
        peer.asn = Some(64000);
        peer.region = Some("Region-1".to_string());

        tracker.add_peer(&peer);
    }

    // Calculate diversity score again
    let new_score = tracker.calculate_diversity_score();

    // Diversity should be reduced
    assert!(new_score < score, "Diversity score should decrease with less diverse peers");
}

/// Test eclipse attack prevention
#[tokio::test]
async fn test_eclipse_prevention() {
    let config = EclipsePreventionConfig {
        min_outbound_connections: 4,
        forced_rotation_interval: 1, // 1 second for testing
        enable_automatic_rotation: true,
        max_peers_per_subnet: 3,
        max_peers_per_asn: 5,
        max_peers_per_region: 7,
        max_inbound_ratio: 3.0,
    };

    let peer_manager_arc = Arc::new(RwLock::new(PeerManager::new()));
    let mut connection_manager = ConnectionManager::new(peer_manager_arc.clone(), config);

    // Add several outbound connections
    let mut outbound_peers = Vec::new();
    for i in 1..=8 {
        let peer_id = PeerId::random();
        let ip = IpAddr::V4(Ipv4Addr::new(10, i, 0, 1));
        let peer = PeerInfo::new(peer_id, ip, 8333);

        connection_manager.add_connection(peer_id, ConnectionDirection::Outbound, &peer)
            .expect("Failed to add outbound connection");

        outbound_peers.push((peer_id, peer));
    }

    // Verify connections
    let (inbound, outbound) = connection_manager.count_connections_by_direction();
    assert_eq!(inbound, 0);
    assert_eq!(outbound, 8);

    // Wait long enough for rotation interval
    tokio::time::sleep(Duration::from_secs(2)).await;

    // Check if rotation is needed
    assert!(connection_manager.check_rotation_needed(), "Should need rotation after interval");

    // Create rotation plan
    let rotation_plan = connection_manager.create_rotation_plan();
    assert!(rotation_plan.is_some(), "Should create rotation plan");

    // Verify rotation plan
    if let Some((to_disconnect, count)) = rotation_plan {
        assert!(count > 0, "Should have peers to disconnect");
        assert!(count < outbound_peers.len(), "Should not disconnect all peers");
    }

    // Perform rotation
    let rotated = connection_manager.perform_rotation();
    assert!(rotated.is_some(), "Should perform rotation");

    // Remove the rotated peers
    if let Some(to_disconnect) = rotated {
        for peer_id in to_disconnect {
            let peer = outbound_peers.iter()
                .find(|(id, _)| *id == peer_id)
                .map(|(_, p)| p.clone());

            if let Some(peer_info) = peer {
                connection_manager.remove_connection(&peer_id, &peer_info);
            }
        }
    }

    // Verify fewer outbound connections
    let (_, outbound) = connection_manager.count_connections_by_direction();
    assert!(outbound < 8, "Should have fewer outbound connections after rotation");
    assert!(outbound >= config.min_outbound_connections, "Should not go below minimum outbound connections");
}

/// Test inbound connection ratio limiting
#[tokio::test]
async fn test_inbound_ratio_limiting() {
    let config = EclipsePreventionConfig {
        min_outbound_connections: 4,
        forced_rotation_interval: 60,
        enable_automatic_rotation: true,
        max_peers_per_subnet: 3,
        max_peers_per_asn: 5,
        max_peers_per_region: 7,
        max_inbound_ratio: 2.0, // At most 2 inbound per outbound
    };

    let peer_manager_arc = Arc::new(RwLock::new(PeerManager::new()));
    let mut connection_manager = ConnectionManager::new(peer_manager_arc.clone(), config);

    // Add some outbound connections
    let mut outbound_peers = Vec::new();
    for i in 1..=5 {
        let peer_id = PeerId::random();
        let ip = IpAddr::V4(Ipv4Addr::new(10, i, 0, 1));
        let peer = PeerInfo::new(peer_id, ip, 8333);

        connection_manager.add_connection(peer_id, ConnectionDirection::Outbound, &peer)
            .expect("Failed to add outbound connection");

        outbound_peers.push((peer_id, peer));
    }

    // Should allow up to 10 inbound connections (2x5)
    let mut inbound_peers = Vec::new();
    for i in 1..=11 {
        let peer_id = PeerId::random();
        let ip = IpAddr::V4(Ipv4Addr::new(192, 168, i, 1));
        let peer = PeerInfo::new(peer_id, ip, 8333);

        let result = connection_manager.add_connection(peer_id, ConnectionDirection::Inbound, &peer);

        if i <= 10 {
            assert!(result.is_ok(), "Should accept inbound connection {}", i);
            inbound_peers.push((peer_id, peer));
        } else {
            assert!(result.is_err(), "Should reject inbound connection {}", i);
        }
    }

    // Verify correct counts
    let (inbound, outbound) = connection_manager.count_connections_by_direction();
    assert_eq!(inbound, 10);
    assert_eq!(outbound, 5);

    // Verify inbound ratio
    let ratio = connection_manager.inbound_ratio();
    assert_eq!(ratio, 2.0);
}

/// Test peer behavior and reputation scoring
#[tokio::test]
async fn test_peer_reputation_scoring() {
    let mut peer_manager = PeerManager::new();

    // Create a peer
    let peer_id = PeerId::random();
    let ip = IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1));

    // Disable challenges for this test
    peer_manager.enable_connection_challenges = false;

    // Add the peer
    peer_manager.try_add_connection(peer_id, ip, 8333).expect("Failed to add peer");

    // Record different behaviors
    peer_manager.record_block_announcement(&peer_id, true);
    peer_manager.record_block_announcement(&peer_id, true);
    peer_manager.record_block_announcement(&peer_id, false); // 1 invalid

    peer_manager.record_transaction_relay(&peer_id, true);
    peer_manager.record_transaction_relay(&peer_id, true);
    peer_manager.record_transaction_relay(&peer_id, true);
    peer_manager.record_transaction_relay(&peer_id, false); // 1 invalid

    // Record response times
    peer_manager.record_response_time(&peer_id, 50);  // 50ms
    peer_manager.record_response_time(&peer_id, 150); // 150ms
    peer_manager.record_response_time(&peer_id, 80);  // 80ms

    // Record some protocol violations
    peer_manager.record_protocol_violation(&peer_id, "Invalid message format");

    // Get the peer score
    let peer_info = peer_manager.get_peer_info(&peer_id).expect("Peer should exist");

    // Verify behavior tracking
    assert_eq!(peer_info.behavior_patterns.valid_blocks_announced, 2);
    assert_eq!(peer_info.behavior_patterns.invalid_blocks_announced, 1);
    assert_eq!(peer_info.behavior_patterns.valid_txns_relayed, 3);
    assert_eq!(peer_info.behavior_patterns.invalid_txns_relayed, 1);
    assert_eq!(peer_info.behavior_patterns.protocol_violations, 1);

    // Verify average response time
    let avg_time = peer_info.behavior_patterns.average_response_time();
    assert!(avg_time.is_some());
    assert!(avg_time.unwrap() > 90.0 && avg_time.unwrap() < 95.0); // ~93.3

    // Verify overall score components
    assert!(peer_info.score.behavior_score > 0.0);
    assert!(peer_info.score.latency_score > 0.0);
    assert!(peer_info.score.stability_score > 0.0);
}

/// Test multiple security mechanisms working together
#[tokio::test]
async fn test_integrated_sybil_protection() {
    let mut peer_manager = PeerManager::new();

    // Enable security features
    peer_manager.enable_connection_challenges = true;
    peer_manager.challenge_difficulty = 8; // Lower for testing

    // Create a peer with suspicious behavior
    let suspicious_peer_id = PeerId::random();
    let ip = IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1));

    // 1. First, verify it needs challenge
    let result = peer_manager.try_add_connection(suspicious_peer_id, ip, 8333);
    assert!(matches!(result, Ok(false)));

    // 2. Pass the challenge
    let challenge = peer_manager.issue_challenge(&suspicious_peer_id).expect("Failed to issue challenge");

    // Create a valid response (simplified for test)
    let mut hasher = sha2::Sha256::new();
    hasher.update(&challenge);
    let mut nonce = 0u64;
    let mut solution = String::new();

    // Simple proof of work - find a nonce that produces required leading zeros
    loop {
        let nonce_bytes = nonce.to_le_bytes();
        let mut trial_hasher = hasher.clone();
        trial_hasher.update(&nonce_bytes);
        let hash = trial_hasher.finalize();

        // Check if it meets difficulty
        if hash[0] <= 1 << (8 - peer_manager.challenge_difficulty) {
            solution = format!("{}{}", hex::encode(hash), hex::encode(nonce_bytes));
            break;
        }

        nonce += 1;
    }

    peer_manager.process_challenge_response(&suspicious_peer_id, &solution);

    // 3. Now connection should be accepted
    let result = peer_manager.try_add_connection(suspicious_peer_id, ip, 8333);
    assert!(matches!(result, Ok(true)));

    // 4. Record suspicious behavior
    peer_manager.record_block_announcement(&suspicious_peer_id, false);
    peer_manager.record_block_announcement(&suspicious_peer_id, false);
    peer_manager.record_protocol_violation(&suspicious_peer_id, "Malformed message");
    peer_manager.record_protocol_violation(&suspicious_peer_id, "Spam");

    // 5. The peer should have a low score now
    let peer_info = peer_manager.get_peer_info(&suspicious_peer_id).expect("Peer should exist");
    assert!(peer_info.score.behavior_score < 2.0); // Low behavior score

    // 6. Record serious protocol violations to trigger ban
    for _ in 0..4 {
        peer_manager.record_protocol_violation(&suspicious_peer_id, "Severe violation");
    }

    // 7. The IP should now be banned
    let rate_limit = peer_manager.rate_limits.get(&ip).expect("Should have rate limit entry");
    assert!(rate_limit.banned_until.is_some());

    // 8. Attempt to connect again should fail
    let result = peer_manager.try_add_connection(PeerId::random(), ip, 8333);
    assert!(result.is_err());
}