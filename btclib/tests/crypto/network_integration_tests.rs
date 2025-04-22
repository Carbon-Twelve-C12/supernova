use btclib::crypto::signature::{
    SignatureVerifier, SignatureScheme, SignatureType, SignatureError, SignatureParams
};
use btclib::validation::SecurityLevel;
use btclib::crypto::falcon::{FalconKeyPair, FalconParameters};
use btclib::crypto::quantum::{QuantumKeyPair, QuantumScheme, QuantumParameters};
use rand::rngs::OsRng;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Instant, Duration};

// Simulate a network node with signature verification capabilities
struct TestNode {
    id: String,
    verifier: SignatureVerifier,
    message_pool: Vec<TestNetworkMessage>,
    verified_messages: Vec<TestNetworkMessage>,
}

// Simulate a network message with signature information
#[derive(Clone)]
struct TestNetworkMessage {
    id: String,
    data: Vec<u8>,
    sig_type: SignatureType,
    public_key: Vec<u8>,
    signature: Vec<u8>,
    verified: bool,
}

impl TestNode {
    fn new(id: &str) -> Self {
        let mut verifier = SignatureVerifier::new();
        
        // Register Falcon with the verifier
        verifier.register(
            SignatureType::Falcon,
            Box::new(btclib::crypto::signature::FalconScheme::new(SecurityLevel::Medium as u8))
        );
        
        TestNode {
            id: id.to_string(),
            verifier,
            message_pool: Vec::new(),
            verified_messages: Vec::new(),
        }
    }
    
    fn receive_message(&mut self, message: TestNetworkMessage) {
        self.message_pool.push(message);
    }
    
    fn process_messages(&mut self) -> usize {
        let mut verified_count = 0;
        let mut new_verified = Vec::new();
        
        for message in &self.message_pool {
            let verification_result = self.verifier.verify(
                message.sig_type,
                &message.public_key,
                &message.data,
                &message.signature
            );
            
            if let Ok(is_valid) = verification_result {
                if is_valid {
                    let mut verified_message = message.clone();
                    verified_message.verified = true;
                    new_verified.push(verified_message);
                    verified_count += 1;
                }
            }
        }
        
        // Remove verified messages from pool and add to verified list
        for verified in &new_verified {
            self.message_pool.retain(|m| m.id != verified.id);
            self.verified_messages.push(verified.clone());
        }
        
        verified_count
    }
}

/// Test signature verification across multiple simulated network nodes
#[test]
fn test_network_signature_verification() {
    println!("\n====== NETWORK SIGNATURE VERIFICATION TEST ======");
    
    // Create a network of 3 nodes
    let mut nodes = vec![
        TestNode::new("node1"),
        TestNode::new("node2"),
        TestNode::new("node3"),
    ];
    
    // Create test messages with different signature schemes
    let messages_data = [
        "Transfer 1.5 BTC from Alice to Bob",
        "Transfer 2.0 BTC from Charlie to Dave",
        "Transfer 0.5 BTC from Eve to Frank",
    ];
    
    let mut messages = Vec::new();
    let security_level = SecurityLevel::Medium as u8;
    
    // Create a Dilithium-signed message
    {
        let params = QuantumParameters {
            scheme: QuantumScheme::Dilithium,
            security_level,
        };
        
        let keypair = QuantumKeyPair::generate(QuantumScheme::Dilithium, Some(params))
            .expect("Failed to generate Dilithium key pair");
        
        let data = messages_data[0].as_bytes();
        let signature = keypair.sign(data)
            .expect("Failed to sign with Dilithium");
        
        messages.push(TestNetworkMessage {
            id: "msg1".to_string(),
            data: data.to_vec(),
            sig_type: SignatureType::Dilithium,
            public_key: keypair.public_key,
            signature,
            verified: false,
        });
    }
    
    // Create a Falcon-signed message
    {
        let params = FalconParameters::with_security_level(security_level);
        let mut rng = OsRng;
        
        let keypair = FalconKeyPair::generate(&mut rng, params)
            .expect("Failed to generate Falcon key pair");
        
        let data = messages_data[1].as_bytes();
        let signature = keypair.sign(data)
            .expect("Failed to sign with Falcon");
        
        messages.push(TestNetworkMessage {
            id: "msg2".to_string(),
            data: data.to_vec(),
            sig_type: SignatureType::Falcon,
            public_key: keypair.public_key,
            signature,
            verified: false,
        });
    }
    
    // Create another Dilithium-signed message with higher security level
    {
        let params = QuantumParameters {
            scheme: QuantumScheme::Dilithium,
            security_level: SecurityLevel::High as u8,
        };
        
        let keypair = QuantumKeyPair::generate(QuantumScheme::Dilithium, Some(params))
            .expect("Failed to generate Dilithium key pair");
        
        let data = messages_data[2].as_bytes();
        let signature = keypair.sign(data)
            .expect("Failed to sign with Dilithium");
        
        messages.push(TestNetworkMessage {
            id: "msg3".to_string(),
            data: data.to_vec(),
            sig_type: SignatureType::Dilithium,
            public_key: keypair.public_key,
            signature,
            verified: false,
        });
    }
    
    // Distribute messages to nodes (simulate network propagation)
    for (i, message) in messages.iter().enumerate() {
        // Send each message to two nodes
        nodes[i % 3].receive_message(message.clone());
        nodes[(i + 1) % 3].receive_message(message.clone());
    }
    
    // Process messages on each node
    let mut total_verified = 0;
    for (i, node) in nodes.iter_mut().enumerate() {
        let verified_count = node.process_messages();
        println!("Node {} verified {} messages", i + 1, verified_count);
        total_verified += verified_count;
    }
    
    // Verify that all messages were processed by at least one node
    assert!(total_verified >= messages.len(), 
            "All messages should be verified by at least one node");
    
    // Check that messages properly propagated through the network
    for node in &nodes {
        println!("Node {} has {} verified messages", node.id, node.verified_messages.len());
        assert!(!node.verified_messages.is_empty(), 
                "Each node should have verified at least one message");
    }
}

/// Test parallel message verification on multiple network nodes
#[test]
fn test_parallel_network_message_processing() {
    println!("\n====== PARALLEL NETWORK MESSAGE PROCESSING TEST ======");
    
    // Create a larger number of test messages
    let message_count = 30;
    let security_level = SecurityLevel::Medium as u8;
    
    // Generate a set of messages signed with different schemes
    let mut messages = Vec::with_capacity(message_count);
    
    for i in 0..message_count {
        let data = format!("Message {}: Transfer {} BTC to Recipient {}", 
                           i, i as f32 / 10.0, i % 5).into_bytes();
        
        // Alternate between Dilithium and Falcon signatures
        if i % 2 == 0 {
            // Dilithium signature
            let params = QuantumParameters {
                scheme: QuantumScheme::Dilithium,
                security_level,
            };
            
            let keypair = QuantumKeyPair::generate(QuantumScheme::Dilithium, Some(params))
                .expect("Failed to generate Dilithium key pair");
            
            let signature = keypair.sign(&data)
                .expect("Failed to sign with Dilithium");
            
            messages.push(TestNetworkMessage {
                id: format!("msg{}", i),
                data,
                sig_type: SignatureType::Dilithium,
                public_key: keypair.public_key,
                signature,
                verified: false,
            });
        } else {
            // Falcon signature
            let params = FalconParameters::with_security_level(security_level);
            let mut rng = OsRng;
            
            let keypair = FalconKeyPair::generate(&mut rng, params)
                .expect("Failed to generate Falcon key pair");
            
            let signature = keypair.sign(&data)
                .expect("Failed to sign with Falcon");
            
            messages.push(TestNetworkMessage {
                id: format!("msg{}", i),
                data,
                sig_type: SignatureType::Falcon,
                public_key: keypair.public_key,
                signature,
                verified: false,
            });
        }
    }
    
    // Create multiple nodes and process messages in parallel
    let node_count = 5;
    let mut nodes = Vec::with_capacity(node_count);
    
    for i in 0..node_count {
        nodes.push(TestNode::new(&format!("node{}", i + 1)));
    }
    
    // Distribute messages to nodes (simulate network flooding)
    for message in &messages {
        // Send each message to all nodes
        for node in &mut nodes {
            node.receive_message(message.clone());
        }
    }
    
    // Create shared nodes for parallel processing
    let shared_nodes = Arc::new(Mutex::new(nodes));
    let mut handles = Vec::with_capacity(node_count);
    
    // Process messages on each node in parallel
    let start_time = Instant::now();
    
    for i in 0..node_count {
        let nodes_clone = Arc::clone(&shared_nodes);
        
        let handle = thread::spawn(move || {
            let mut nodes = nodes_clone.lock().unwrap();
            let verified_count = nodes[i].process_messages();
            println!("Node {} verified {} messages", i + 1, verified_count);
            verified_count
        });
        
        handles.push(handle);
    }
    
    // Collect results
    let mut total_verified = 0;
    for handle in handles {
        total_verified += handle.join().unwrap();
    }
    
    let duration = start_time.elapsed();
    
    // Calculate verification rate
    let verification_rate = (total_verified as f64) / duration.as_secs_f64();
    
    println!("Total verified messages: {}", total_verified);
    println!("Time taken: {:?}", duration);
    println!("Verification rate: {:.2} messages/second", verification_rate);
    
    // Verify that all messages were processed by at least one node
    assert!(total_verified >= messages.len(), 
            "All messages should be verified by at least one node");
}

/// Test verification for corrupted network messages
#[test]
fn test_corrupted_network_messages() {
    println!("\n====== CORRUPTED NETWORK MESSAGES TEST ======");
    
    // Create a network of 2 nodes
    let mut nodes = vec![
        TestNode::new("honest_node"),
        TestNode::new("malicious_node"),
    ];
    
    // Create test message with Dilithium signature
    let security_level = SecurityLevel::Medium as u8;
    let params = QuantumParameters {
        scheme: QuantumScheme::Dilithium,
        security_level,
    };
    
    let keypair = QuantumKeyPair::generate(QuantumScheme::Dilithium, Some(params))
        .expect("Failed to generate Dilithium key pair");
    
    let original_data = b"Transfer 0.1 BTC from Alice to Bob";
    let signature = keypair.sign(original_data)
        .expect("Failed to sign with Dilithium");
    
    // Create honest message
    let honest_message = TestNetworkMessage {
        id: "honest_msg".to_string(),
        data: original_data.to_vec(),
        sig_type: SignatureType::Dilithium,
        public_key: keypair.public_key.clone(),
        signature: signature.clone(),
        verified: false,
    };
    
    // Create corrupted message (modified data but same signature)
    let corrupted_message = TestNetworkMessage {
        id: "corrupted_msg".to_string(),
        data: b"Transfer 100.0 BTC from Alice to Bob".to_vec(),  // Changed amount
        sig_type: SignatureType::Dilithium,
        public_key: keypair.public_key.clone(),
        signature: signature.clone(),
        verified: false,
    };
    
    // Send honest message to honest node
    nodes[0].receive_message(honest_message);
    
    // Send corrupted message to malicious node
    nodes[1].receive_message(corrupted_message);
    
    // Process messages on both nodes
    for (i, node) in nodes.iter_mut().enumerate() {
        let verified_count = node.process_messages();
        println!("Node {} verified {} messages", i + 1, verified_count);
        
        if i == 0 {
            // Honest node should verify the honest message
            assert_eq!(verified_count, 1, "Honest message should be verified");
            assert_eq!(node.verified_messages.len(), 1, "Honest node should have 1 verified message");
        } else {
            // Malicious node should not verify the corrupted message
            assert_eq!(verified_count, 0, "Corrupted message should not be verified");
            assert!(node.verified_messages.is_empty(), "Malicious node should have 0 verified messages");
        }
    }
    
    println!("Signature verification correctly rejected corrupted message");
} 