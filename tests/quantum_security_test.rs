use btclib::crypto::quantum::{
    QuantumScheme,
    QuantumKeyPair,
    QuantumParameters,
    ClassicalScheme
};
use btclib::validation::SecurityLevel;
use node::network::{
    peer_diversity::{PeerDiversityManager, ConnectionStrategy, SuspiciousBehavior},
    p2p::{P2PNetwork, count_leading_zero_bits, solve_pow_challenge},
};
use libp2p::{PeerId, Multiaddr};
use std::str::FromStr;
use rand::rngs::OsRng;
use std::time::Duration;

/// Test the Dilithium signature scheme
#[test]
fn test_dilithium_signatures() {
    let mut rng = OsRng;

    // Test with different security levels
    for security_level in &[
        SecurityLevel::Low as u8,
        SecurityLevel::Medium as u8,
        SecurityLevel::High as u8,
    ] {
        let parameters = QuantumParameters {
            scheme: QuantumScheme::Dilithium,
            security_level: *security_level,
        };

        // Generate keypair
        let keypair = QuantumKeyPair::generate(parameters)
            .expect("Failed to generate Dilithium keypair");

        // Test signing and verification
        let message = b"Test message for Dilithium signature";

        // Sign the message
        let signature = keypair.sign(message)
            .expect("Failed to sign with Dilithium");

        // Verify the signature
        let result = keypair.verify(message, &signature)
            .expect("Failed to verify Dilithium signature");

        assert!(result, "Dilithium signature verification should succeed");

        // Test with modified message
        let modified_message = b"Modified test message";
        let modified_result = keypair.verify(modified_message, &signature)
            .expect("Failed to run verification");

        assert!(!modified_result, "Verification should fail with modified message");
    }
}

/// Test the SPHINCS+ signature scheme
#[test]
fn test_sphincs_signatures() {
    let mut rng = OsRng;

    // Use low security for faster test
    let parameters = QuantumParameters {
        scheme: QuantumScheme::Sphincs,
        security_level: SecurityLevel::Low as u8,
    };

    // Generate keypair
    let keypair = QuantumKeyPair::generate(parameters)
        .expect("Failed to generate SPHINCS+ keypair");

    // Test signing and verification
    let message = b"Test message for SPHINCS+ signature";

    // Sign the message
    let signature = keypair.sign(message)
        .expect("Failed to sign with SPHINCS+");

    // Verify the signature
    let result = keypair.verify(message, &signature)
        .expect("Failed to verify SPHINCS+ signature");

    assert!(result, "SPHINCS+ signature verification should succeed");

    // Test with modified message
    let modified_message = b"Modified test message";
    let modified_result = keypair.verify(modified_message, &signature)
        .expect("Failed to run verification");

    assert!(!modified_result, "Verification should fail with modified message");
}

/// Test the hybrid signature scheme
#[test]
fn test_hybrid_signatures() {
    let mut rng = OsRng;

    // Test both classical schemes
    for classical_scheme in &[
        ClassicalScheme::Secp256k1,
        ClassicalScheme::Ed25519,
    ] {
        let parameters = QuantumParameters {
            scheme: QuantumScheme::Hybrid(*classical_scheme),
            security_level: SecurityLevel::Medium as u8,
        };

        // Generate keypair
        let keypair = QuantumKeyPair::generate(parameters)
            .expect("Failed to generate hybrid keypair");

        // Test signing and verification
        let message = b"Test message for hybrid signature";

        // Sign the message
        let signature = keypair.sign(message)
            .expect("Failed to sign with hybrid scheme");

        // Verify the signature
        let result = keypair.verify(message, &signature)
            .expect("Failed to verify hybrid signature");

        assert!(result, "Hybrid signature verification should succeed");

        // Test with modified message
        let modified_message = b"Modified test message";
        let modified_result = keypair.verify(modified_message, &signature)
            .expect("Failed to run verification");

        assert!(!modified_result, "Verification should fail with modified message");
    }
}

/// Test peer diversity for eclipse attack prevention
#[test]
fn test_peer_diversity_manager() {
    let mut diversity_manager = PeerDiversityManager::with_config(
        0.6, // Minimum diversity score
        ConnectionStrategy::BalancedDiversity,
        10, // Max connections per minute
    );

    // Register peers from different subnets
    for i in 1..10 {
        let peer_id = PeerId::random();
        let ip_addr = format!("/ip4/192.168.{}.1/tcp/8000", i);
        let multiaddr = Multiaddr::from_str(&ip_addr).unwrap();

        // Alternate between inbound and outbound
        let is_inbound = i % 2 == 0;

        diversity_manager.register_peer(peer_id, &multiaddr, is_inbound);
    }

    // Test diversity score calculation
    let score = diversity_manager.evaluate_diversity();
    assert!(score > 0.5, "Diversity score should be over 0.5 with multiple subnets");

    // Test suspicious behavior flagging
    let peer_id = PeerId::random();
    let addr = Multiaddr::from_str("/ip4/10.0.0.1/tcp/8000").unwrap();
    diversity_manager.register_peer(peer_id.clone(), &addr, true);

    // Flag some suspicious behaviors
    diversity_manager.flag_suspicious_behavior(&peer_id, SuspiciousBehavior::AddressFlooding);
    diversity_manager.flag_suspicious_behavior(&peer_id, SuspiciousBehavior::RoutingPoisoning);

    // After suspicious behavior, peer rotation should be needed
    assert!(diversity_manager.check_rotation_needed(),
           "Rotation should be needed after suspicious behavior");

    // Test rotation plan
    let (peers_to_disconnect, count) = diversity_manager.create_rotation_plan();
    assert!(count > 0, "Should have peers to disconnect in rotation plan");
}

/// Test proof-of-work challenge for Sybil protection
#[test]
fn test_pow_challenge() {
    // Generate a challenge
    let challenge = b"test challenge for Sybil protection";

    // Try different difficulties
    for difficulty in &[8, 12, 16] {
        // Solve the challenge
        let solution = solve_pow_challenge(challenge, *difficulty);

        // Verify the solution
        let mut hasher = sha2::Sha256::new();
        hasher.update(challenge);
        hasher.update(&solution);
        let hash = hasher.finalize();

        let leading_zeros = count_leading_zero_bits(&hash);
        assert!(leading_zeros >= *difficulty,
               "Solution doesn't meet difficulty: got {} bits, required {}",
               leading_zeros, *difficulty);
    }
}

/// Test the entire P2P network with security features
#[tokio::test]
async fn test_secure_p2p_network() {
    // Create a P2P network with security features
    let (mut network, cmd_tx, mut event_rx) = P2PNetwork::new(
        None, // Generate keypair
        [0u8; 32], // Genesis hash
        "test-network", // Network ID
    ).await.unwrap();

    // Configure security settings
    network.set_challenge_difficulty(8); // Lower for tests
    network.set_require_verification(true);

    // Start the network
    network.start().await.unwrap();

    // In a full integration test, we would:
    // 1. Connect multiple peers
    // 2. Test verification challenges
    // 3. Verify Sybil attack protection
    // 4. Test eclipse attack prevention

    // For this test, just verify the network starts properly
    assert!(network.running, "Network should be running");
}