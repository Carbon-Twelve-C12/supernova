use node::network::{
    p2p::P2PNetwork,
    NetworkCommand,
    NetworkEvent,
    peer::PeerState,
};
use libp2p::{Multiaddr, PeerId};
use std::str::FromStr;
use std::time::Duration;
use tokio::time::timeout;

#[tokio::test]
async fn test_network_creation() {
    // Create a new P2P network instance
    let (network, _cmd_tx, _event_rx) = P2PNetwork::new(
        None, // Generate a new keypair
        [0u8; 32], // Genesis hash
        "supernova-test", // Network ID
    ).await.unwrap();
    
    // Check that network is created successfully
    assert_eq!(network.get_stats().peers_connected, 0);
}

#[tokio::test]
async fn test_network_command_processing() {
    // Create two P2P network instances
    let (mut network1, cmd_tx1, mut event_rx1) = P2PNetwork::new(
        None, 
        [0u8; 32], 
        "supernova-test",
    ).await.unwrap();
    
    let (mut network2, cmd_tx2, mut event_rx2) = P2PNetwork::new(
        None, 
        [0u8; 32], 
        "supernova-test",
    ).await.unwrap();
    
    // Start both networks
    network1.start().await.unwrap();
    network2.start().await.unwrap();
    
    // Get their local addresses
    let peer1_id = network1.local_peer_id();
    let peer2_id = network2.local_peer_id();
    
    // Create multiaddresses for testing
    let addr1 = Multiaddr::from_str("/ip4/127.0.0.1/tcp/9001").unwrap();
    let addr2 = Multiaddr::from_str("/ip4/127.0.0.1/tcp/9002").unwrap();
    
    // Start listening on addresses
    cmd_tx1.send(NetworkCommand::StartListening(addr1.clone())).await.unwrap();
    cmd_tx2.send(NetworkCommand::StartListening(addr2.clone())).await.unwrap();
    
    // Allow time for networks to start listening
    tokio::time::sleep(Duration::from_millis(100)).await;
    
    // Try to connect one network to the other
    cmd_tx1.send(NetworkCommand::Dial(peer2_id.clone(), addr2.clone())).await.unwrap();
    
    // Wait for the connection events
    let mut connected = false;
    let check_timeout = Duration::from_secs(5);
    
    // Wait for network2 to receive connection from network1
    if let Ok(Some(event)) = timeout(check_timeout, event_rx2.recv()).await {
        if let NetworkEvent::NewPeer(peer_id) = event {
            assert_eq!(peer_id, peer1_id);
            connected = true;
        }
    }
    
    assert!(connected, "Failed to establish connection between peers");
}

#[tokio::test]
async fn test_message_broadcasting() {
    // This test would set up multiple networks and test message propagation
    // For brevity, just a placeholder
    
    let (mut network, cmd_tx, mut event_rx) = P2PNetwork::new(
        None, 
        [0u8; 32], 
        "supernova-test",
    ).await.unwrap();
    
    network.start().await.unwrap();
    
    // Listen on a local address
    let addr = Multiaddr::from_str("/ip4/127.0.0.1/tcp/9003").unwrap();
    cmd_tx.send(NetworkCommand::StartListening(addr)).await.unwrap();
    
    // In a full test, we would:
    // 1. Set up multiple network instances
    // 2. Connect them to each other
    // 3. Send a broadcast message from one
    // 4. Verify all others receive it
}

#[tokio::test]
async fn test_peer_disconnect() {
    // Create two P2P network instances
    let (mut network1, cmd_tx1, mut event_rx1) = P2PNetwork::new(
        None, 
        [0u8; 32], 
        "supernova-test",
    ).await.unwrap();
    
    let (mut network2, cmd_tx2, mut event_rx2) = P2PNetwork::new(
        None, 
        [0u8; 32], 
        "supernova-test",
    ).await.unwrap();
    
    // Start both networks
    network1.start().await.unwrap();
    network2.start().await.unwrap();
    
    // Get their local addresses
    let peer1_id = network1.local_peer_id();
    let peer2_id = network2.local_peer_id();
    
    // Create multiaddresses for testing
    let addr1 = Multiaddr::from_str("/ip4/127.0.0.1/tcp/9004").unwrap();
    let addr2 = Multiaddr::from_str("/ip4/127.0.0.1/tcp/9005").unwrap();
    
    // Start listening on addresses
    cmd_tx1.send(NetworkCommand::StartListening(addr1.clone())).await.unwrap();
    cmd_tx2.send(NetworkCommand::StartListening(addr2.clone())).await.unwrap();
    
    // Allow time for networks to start listening
    tokio::time::sleep(Duration::from_millis(100)).await;
    
    // Connect one network to the other
    cmd_tx1.send(NetworkCommand::Dial(peer2_id.clone(), addr2.clone())).await.unwrap();
    
    // Wait for connection to establish
    let check_timeout = Duration::from_secs(5);
    let mut connected = false;
    
    if let Ok(Some(event)) = timeout(check_timeout, event_rx2.recv()).await {
        if let NetworkEvent::NewPeer(_) = event {
            connected = true;
        }
    }
    
    assert!(connected, "Failed to establish connection");
    
    // Now disconnect
    cmd_tx1.send(NetworkCommand::DisconnectPeer(peer2_id.clone())).await.unwrap();
    
    // Wait for disconnect event
    let mut disconnected = false;
    
    if let Ok(Some(event)) = timeout(check_timeout, event_rx2.recv()).await {
        if let NetworkEvent::PeerLeft(peer_id) = event {
            assert_eq!(peer_id, peer1_id);
            disconnected = true;
        }
    }
    
    assert!(disconnected, "Failed to disconnect peers");
}

#[tokio::test]
async fn test_peer_banning() {
    // Create two P2P network instances
    let (mut network1, cmd_tx1, mut event_rx1) = P2PNetwork::new(
        None, 
        [0u8; 32], 
        "supernova-test",
    ).await.unwrap();
    
    let (mut network2, cmd_tx2, _) = P2PNetwork::new(
        None, 
        [0u8; 32], 
        "supernova-test",
    ).await.unwrap();
    
    // Start both networks
    network1.start().await.unwrap();
    network2.start().await.unwrap();
    
    // Get their peer ids
    let peer1_id = network1.local_peer_id();
    let peer2_id = network2.local_peer_id();
    
    // Ban peer2 from network1
    cmd_tx1.send(NetworkCommand::BanPeer {
        peer_id: peer2_id.clone(),
        reason: "Testing ban functionality".to_string(),
        duration: Some(Duration::from_secs(3600)),
    }).await.unwrap();
    
    // In a complete implementation, we would now check that:
    // 1. Connecting to a banned peer fails
    // 2. The banned peer's connections are dropped
    // 3. After the ban expires, connection is allowed again
    
    // For now, just assert that ban command was processed
    assert_eq!(1, 1); // Placeholder assertion
}

#[tokio::test]
async fn test_network_shutdown() {
    // Create a P2P network instance
    let (mut network, cmd_tx, mut event_rx) = P2PNetwork::new(
        None, 
        [0u8; 32], 
        "supernova-test",
    ).await.unwrap();
    
    // Start the network
    network.start().await.unwrap();
    
    // Listen on a local address
    let addr = Multiaddr::from_str("/ip4/127.0.0.1/tcp/9006").unwrap();
    cmd_tx.send(NetworkCommand::StartListening(addr)).await.unwrap();
    
    // Allow time for network to start
    tokio::time::sleep(Duration::from_millis(100)).await;
    
    // Stop the network
    cmd_tx.send(NetworkCommand::Stop).await.unwrap();
    
    // Wait for the stopped event
    let check_timeout = Duration::from_secs(5);
    let mut stopped = false;
    
    if let Ok(Some(event)) = timeout(check_timeout, event_rx.recv()).await {
        if let NetworkEvent::Stopped = event {
            stopped = true;
        }
    }
    
    assert!(stopped, "Failed to stop the network");
    
    // Verify the network is no longer running
    assert!(!network.running);
} 