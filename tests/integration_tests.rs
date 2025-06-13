// Comprehensive Integration Tests for Supernova
// Meeting the Satoshi Standard: Full system integration testing

use btclib::{
    blockchain::{Blockchain, Block, Transaction},
    consensus::{ConsensusEngine, QuantumProofOfWork},
    crypto::{
        falcon_real::FalconKeyPair,
        kem::KyberKEM,
    },
    environmental::{
        tracker::EnvironmentalTracker,
        types::{Region, EnergySource},
    },
    lightning::{
        manager::LightningManager,
        config::LightningConfig,
    },
    mempool::Mempool,
    mining::miner::Miner,
    network::{
        p2p::P2PNetwork,
        config::NetworkConfig,
    },
    node::{
        Node, NodeConfig,
        api::ApiServer,
    },
    storage::{
        persistence::BlockchainDB,
        utxo_set::UtxoSet,
    },
    types::{
        transaction::{TxInput, TxOutput, OutPoint},
        block::BlockHeader,
    },
    wallet::{
        quantum_wallet::QuantumWallet,
        hdwallet::HDWallet,
    },
};
use std::{
    sync::{Arc, Mutex},
    time::Duration,
    thread,
};
use tokio::runtime::Runtime;

/// Test full node lifecycle
#[test]
fn test_node_lifecycle() {
    let rt = Runtime::new().unwrap();
    
    rt.block_on(async {
        // Create node configuration
        let config = NodeConfig {
            data_dir: "/tmp/supernova_test".to_string(),
            network: "testnet".to_string(),
            rpc_port: 18332,
            p2p_port: 18333,
            enable_mining: true,
            enable_lightning: true,
            quantum_security: true,
            environmental_tracking: true,
        };
        
        // Initialize node
        let node = Node::new(config).await.unwrap();
        
        // Start node
        assert!(node.start().await.is_ok());
        
        // Verify node is running
        assert!(node.is_running());
        
        // Get node info
        let info = node.get_info().await.unwrap();
        assert_eq!(info.network, "testnet");
        assert!(info.quantum_enabled);
        
        // Stop node
        assert!(node.stop().await.is_ok());
        assert!(!node.is_running());
    });
}

/// Test blockchain synchronization
#[test]
fn test_blockchain_sync() {
    let rt = Runtime::new().unwrap();
    
    rt.block_on(async {
        // Create two nodes
        let node1 = create_test_node(18334, 18335).await;
        let node2 = create_test_node(18336, 18337).await;
        
        // Start both nodes
        node1.start().await.unwrap();
        node2.start().await.unwrap();
        
        // Connect nodes
        node2.connect_peer(&format!("127.0.0.1:{}", 18335)).await.unwrap();
        
        // Mine blocks on node1
        for _ in 0..10 {
            node1.mine_block().await.unwrap();
        }
        
        // Wait for sync
        tokio::time::sleep(Duration::from_secs(5)).await;
        
        // Verify both nodes have same chain height
        let height1 = node1.get_blockchain_height().await.unwrap();
        let height2 = node2.get_blockchain_height().await.unwrap();
        assert_eq!(height1, height2);
        
        // Verify chain tips match
        let tip1 = node1.get_best_block_hash().await.unwrap();
        let tip2 = node2.get_best_block_hash().await.unwrap();
        assert_eq!(tip1, tip2);
        
        // Cleanup
        node1.stop().await.unwrap();
        node2.stop().await.unwrap();
    });
}

/// Test transaction flow from creation to confirmation
#[test]
fn test_transaction_flow() {
    let rt = Runtime::new().unwrap();
    
    rt.block_on(async {
        let node = create_test_node(18338, 18339).await;
        node.start().await.unwrap();
        
        // Create wallets
        let wallet1 = QuantumWallet::new().unwrap();
        let wallet2 = QuantumWallet::new().unwrap();
        
        // Fund wallet1 (coinbase)
        let coinbase_tx = create_coinbase_transaction(&wallet1.get_address().unwrap(), 50_000_000);
        node.submit_transaction(coinbase_tx).await.unwrap();
        
        // Mine block to confirm coinbase
        node.mine_block().await.unwrap();
        
        // Create transaction from wallet1 to wallet2
        let tx = wallet1.create_transaction(
            &wallet2.get_address().unwrap(),
            25_000_000,
            1000, // fee
        ).unwrap();
        
        // Submit transaction
        let txid = node.submit_transaction(tx.clone()).await.unwrap();
        
        // Verify transaction is in mempool
        assert!(node.get_mempool_transaction(&txid).await.unwrap().is_some());
        
        // Mine block
        node.mine_block().await.unwrap();
        
        // Verify transaction is confirmed
        let confirmed_tx = node.get_transaction(&txid).await.unwrap();
        assert!(confirmed_tx.is_some());
        assert!(confirmed_tx.unwrap().confirmations > 0);
        
        // Verify balances
        let balance1 = node.get_balance(&wallet1.get_address().unwrap()).await.unwrap();
        let balance2 = node.get_balance(&wallet2.get_address().unwrap()).await.unwrap();
        
        assert_eq!(balance1, 24_999_000); // 50M - 25M - 1000 fee
        assert_eq!(balance2, 25_000_000);
        
        node.stop().await.unwrap();
    });
}

/// Test Lightning Network channel lifecycle
#[test]
fn test_lightning_channel_lifecycle() {
    let rt = Runtime::new().unwrap();
    
    rt.block_on(async {
        // Create two Lightning nodes
        let ln_node1 = create_lightning_node(9735).await;
        let ln_node2 = create_lightning_node(9736).await;
        
        // Start nodes
        ln_node1.start().await.unwrap();
        ln_node2.start().await.unwrap();
        
        // Connect peers
        let node2_id = ln_node2.get_node_id();
        ln_node1.connect_peer(&node2_id, "127.0.0.1:9736").await.unwrap();
        
        // Open channel
        let channel_response = ln_node1.open_channel(
            &node2_id,
            1_000_000, // 1M novas
            100_000,   // 100k push amount
            false,     // public channel
        ).await.unwrap();
        
        // Mine blocks to confirm channel
        for _ in 0..6 {
            mine_test_block().await;
        }
        
        // Verify channel is active
        let channels = ln_node1.list_channels().await.unwrap();
        assert_eq!(channels.len(), 1);
        assert_eq!(channels[0].remote_pubkey, node2_id);
        assert!(channels[0].active);
        
        // Create invoice on node2
        let invoice = ln_node2.create_invoice(
            50_000,
            "Test payment",
            3600,
        ).await.unwrap();
        
        // Pay invoice from node1
        let payment = ln_node1.pay_invoice(&invoice.payment_request).await.unwrap();
        assert_eq!(payment.status, "SUCCEEDED");
        
        // Verify payment received
        let invoices = ln_node2.list_invoices().await.unwrap();
        assert_eq!(invoices.len(), 1);
        assert!(invoices[0].settled);
        
        // Close channel cooperatively
        ln_node1.close_channel(&channel_response.channel_id, false).await.unwrap();
        
        // Mine blocks to confirm closing
        for _ in 0..6 {
            mine_test_block().await;
        }
        
        // Verify channel is closed
        let channels = ln_node1.list_channels().await.unwrap();
        assert_eq!(channels.len(), 0);
        
        // Cleanup
        ln_node1.stop().await.unwrap();
        ln_node2.stop().await.unwrap();
    });
}

/// Test quantum security features
#[test]
fn test_quantum_security() {
    // Test Falcon signatures
    let keypair = FalconKeyPair::generate().unwrap();
    let message = b"Test message for quantum signatures";
    
    let signature = keypair.sign(message).unwrap();
    assert!(keypair.verify(message, &signature).unwrap());
    
    // Test invalid signature
    let mut invalid_sig = signature.clone();
    invalid_sig[0] ^= 0xFF;
    assert!(!keypair.verify(message, &invalid_sig).unwrap());
    
    // Test Kyber KEM
    let (pk, sk) = KyberKEM::generate_keypair().unwrap();
    let (ciphertext, shared_secret1) = KyberKEM::encapsulate(&pk).unwrap();
    let shared_secret2 = KyberKEM::decapsulate(&ciphertext, &sk).unwrap();
    
    assert_eq!(shared_secret1, shared_secret2);
}

/// Test environmental tracking
#[test]
fn test_environmental_tracking() {
    let rt = Runtime::new().unwrap();
    
    rt.block_on(async {
        let tracker = EnvironmentalTracker::new();
        
        // Set mining location
        tracker.set_location(Region::NorthAmerica).await.unwrap();
        
        // Update energy source
        tracker.update_energy_source(EnergySource::Solar, 0.8).await.unwrap();
        tracker.update_energy_source(EnergySource::Grid, 0.2).await.unwrap();
        
        // Calculate emissions for 1MW mining operation
        let emissions = tracker.calculate_emissions(1000.0).await.unwrap();
        assert!(emissions.co2_grams > 0.0);
        assert!(emissions.renewable_percentage == 80.0);
        
        // Purchase carbon credits
        let credits = tracker.purchase_carbon_credits(
            emissions.co2_grams / 1000.0, // Convert to kg
        ).await.unwrap();
        
        assert!(credits.verified);
        assert_eq!(credits.amount_kg, emissions.co2_grams / 1000.0);
        
        // Verify carbon neutral status
        let status = tracker.get_carbon_status().await.unwrap();
        assert!(status.is_carbon_neutral);
    });
}

/// Test mining with environmental bonuses
#[test]
fn test_environmental_mining() {
    let rt = Runtime::new().unwrap();
    
    rt.block_on(async {
        let node = create_test_node(18340, 18341).await;
        node.start().await.unwrap();
        
        // Configure environmental profile
        let env_profile = node.set_environmental_profile(
            1.0,  // 100% renewable
            0.9,  // 90% efficiency score
            true, // verified
            0.8,  // 80% REC coverage
        ).await.unwrap();
        
        // Mine block and check reward
        let block = node.mine_block().await.unwrap();
        let coinbase = &block.transactions[0];
        let reward = coinbase.outputs[0].value;
        
        // Verify environmental bonus applied
        let base_reward = 50_000_000; // 50 NOVA
        let expected_bonus = base_reward * 35 / 100; // 35% max bonus
        let expected_total = base_reward + expected_bonus;
        
        assert_eq!(reward, expected_total);
        
        node.stop().await.unwrap();
    });
}

/// Test consensus mechanism switching
#[test]
fn test_consensus_switching() {
    let rt = Runtime::new().unwrap();
    
    rt.block_on(async {
        let node = create_test_node(18342, 18343).await;
        node.start().await.unwrap();
        
        // Start with PoW
        assert_eq!(node.get_consensus_type().await.unwrap(), "ProofOfWork");
        
        // Mine some blocks
        for _ in 0..10 {
            node.mine_block().await.unwrap();
        }
        
        // Switch to Quantum PoW at height 10
        node.activate_quantum_pow(10).await.unwrap();
        
        // Mine more blocks
        for _ in 0..5 {
            let block = node.mine_block().await.unwrap();
            assert!(block.header.quantum_proof.is_some());
        }
        
        assert_eq!(node.get_consensus_type().await.unwrap(), "QuantumProofOfWork");
        
        node.stop().await.unwrap();
    });
}

/// Test API server functionality
#[test]
fn test_api_server() {
    let rt = Runtime::new().unwrap();
    
    rt.block_on(async {
        let node = create_test_node(18344, 18345).await;
        node.start().await.unwrap();
        
        // Start API server
        let api_server = ApiServer::new(node.clone(), 8080);
        let api_handle = tokio::spawn(async move {
            api_server.start().await.unwrap();
        });
        
        // Wait for server to start
        tokio::time::sleep(Duration::from_secs(1)).await;
        
        // Test API endpoints
        let client = reqwest::Client::new();
        
        // Get blockchain info
        let response = client
            .get("http://localhost:8080/api/v1/blockchain/info")
            .send()
            .await
            .unwrap();
        
        assert_eq!(response.status(), 200);
        let info: serde_json::Value = response.json().await.unwrap();
        assert!(info["height"].is_number());
        
        // Get mempool info
        let response = client
            .get("http://localhost:8080/api/v1/mempool/info")
            .send()
            .await
            .unwrap();
        
        assert_eq!(response.status(), 200);
        
        // Cleanup
        api_handle.abort();
        node.stop().await.unwrap();
    });
}

/// Test network partition and recovery
#[test]
fn test_network_partition() {
    let rt = Runtime::new().unwrap();
    
    rt.block_on(async {
        // Create 3 nodes
        let node1 = create_test_node(18346, 18347).await;
        let node2 = create_test_node(18348, 18349).await;
        let node3 = create_test_node(18350, 18351).await;
        
        // Start all nodes
        node1.start().await.unwrap();
        node2.start().await.unwrap();
        node3.start().await.unwrap();
        
        // Connect in a line: node1 <-> node2 <-> node3
        node2.connect_peer(&format!("127.0.0.1:{}", 18347)).await.unwrap();
        node3.connect_peer(&format!("127.0.0.1:{}", 18349)).await.unwrap();
        
        // Mine initial blocks
        for _ in 0..5 {
            node1.mine_block().await.unwrap();
        }
        
        // Wait for propagation
        tokio::time::sleep(Duration::from_secs(3)).await;
        
        // Verify all nodes synced
        let height1 = node1.get_blockchain_height().await.unwrap();
        let height2 = node2.get_blockchain_height().await.unwrap();
        let height3 = node3.get_blockchain_height().await.unwrap();
        assert_eq!(height1, height2);
        assert_eq!(height2, height3);
        
        // Partition: disconnect node2 from node1
        node2.disconnect_peer(&node1.get_peer_id()).await.unwrap();
        
        // Mine on different partitions
        for _ in 0..3 {
            node1.mine_block().await.unwrap();
            node3.mine_block().await.unwrap();
        }
        
        // Node1 and node3 should have different chains
        let tip1 = node1.get_best_block_hash().await.unwrap();
        let tip3 = node3.get_best_block_hash().await.unwrap();
        assert_ne!(tip1, tip3);
        
        // Heal partition
        node2.connect_peer(&format!("127.0.0.1:{}", 18347)).await.unwrap();
        
        // Wait for reorg
        tokio::time::sleep(Duration::from_secs(5)).await;
        
        // All nodes should converge on longest chain
        let final_tip1 = node1.get_best_block_hash().await.unwrap();
        let final_tip2 = node2.get_best_block_hash().await.unwrap();
        let final_tip3 = node3.get_best_block_hash().await.unwrap();
        assert_eq!(final_tip1, final_tip2);
        assert_eq!(final_tip2, final_tip3);
        
        // Cleanup
        node1.stop().await.unwrap();
        node2.stop().await.unwrap();
        node3.stop().await.unwrap();
    });
}

// Helper functions

async fn create_test_node(rpc_port: u16, p2p_port: u16) -> Arc<Node> {
    let config = NodeConfig {
        data_dir: format!("/tmp/supernova_test_{}", rpc_port),
        network: "testnet".to_string(),
        rpc_port,
        p2p_port,
        enable_mining: true,
        enable_lightning: false,
        quantum_security: true,
        environmental_tracking: true,
    };
    
    Arc::new(Node::new(config).await.unwrap())
}

async fn create_lightning_node(port: u16) -> Arc<LightningManager> {
    let config = LightningConfig {
        network: "testnet".to_string(),
        listen_port: port,
        max_channels: 100,
        min_channel_size: 10000,
        max_channel_size: 10000000,
        default_fee_rate: 1,
        quantum_secure: true,
    };
    
    let wallet = btclib::lightning::wallet::LightningWallet::new().unwrap();
    let (manager, _) = LightningManager::new(config, wallet).unwrap();
    
    Arc::new(manager)
}

fn create_coinbase_transaction(address: &str, amount: u64) -> Transaction {
    Transaction {
        version: 1,
        inputs: vec![TxInput {
            previous_output: OutPoint {
                txid: "0".repeat(64),
                vout: 0xffffffff,
            },
            script_sig: vec![0; 100], // Coinbase script
            sequence: 0xffffffff,
        }],
        outputs: vec![TxOutput {
            value: amount,
            script_pubkey: address.as_bytes().to_vec(), // Simplified
        }],
        lock_time: 0,
    }
}

async fn mine_test_block() {
    // Simplified block mining for tests
    tokio::time::sleep(Duration::from_millis(100)).await;
} 