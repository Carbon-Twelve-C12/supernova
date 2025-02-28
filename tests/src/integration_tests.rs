use node::network::{P2PNetwork, NetworkCommand, NetworkEvent};
use node::storage::{ChainState, BlockchainDB};
use miner::mining::{Miner, MempoolInterface};
use wallet::core::Wallet;
use btclib::types::{Block, Transaction, TransactionInput, TransactionOutput};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::mpsc;
use tokio::time::timeout;
use tempfile::tempdir;
use tracing::{info, warn, error, debug};
use std::collections::HashSet;

/// Test complete workflow: mine blocks, create transactions, mine and verify them
#[tokio::test]
async fn test_full_workflow() -> Result<(), Box<dyn std::error::Error>> {
    // Set up detailed logging for integration tests
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .init();
    
    info!("Starting full blockchain workflow test");
    
    // Create temporary directories with meaningful names
    let node_dir = tempdir()?;
    let wallet_dir = tempdir()?;
    
    debug!("Test directories created at: {:?} and {:?}", node_dir.path(), wallet_dir.path());
    
    // Initialize node components with production-like configuration
    let db = Arc::new(BlockchainDB::new(node_dir.path())?);
    debug!("Database initialized at {:?}", node_dir.path());
    
    let chain_state = ChainState::new(Arc::clone(&db))?;
    info!("Chain state initialized at height {}", chain_state.get_height());
    
    // Initialize mempool with realistic settings
    let mut mempool_config = node::mempool::MempoolConfig::default();
    mempool_config.max_size = 5000;
    mempool_config.min_fee_rate = 1.0;
    let mempool = Arc::new(node::mempool::TransactionPool::new(mempool_config));
    debug!("Mempool initialized with max size {}", mempool_config.max_size);
    
    // Initialize network with specific test network identifier
    let genesis_hash = chain_state.get_genesis_hash();
    let (network, command_tx, mut event_rx) = 
        P2PNetwork::new(None, genesis_hash, "supernova-integration-test").await?;
    
    debug!("P2P network initialized with peer ID: {}", network.local_peer_id);
    
    // Initialize miner with realistic but quick-to-mine settings
    let reward_address = vec![1, 2, 3, 4]; // Dummy reward address for testing
    let (miner, mut block_rx) = Miner::new(
        2, // Use fewer threads for testing
        u32::MAX / 100, // Moderate difficulty for faster test completion
        Arc::clone(&mempool),
        reward_address.clone(),
    );
    
    debug!("Miner initialized with {} threads and target difficulty {}", 
          2, u32::MAX / 100);
    
    // Initialize wallet
    let wallet_path = wallet_dir.path().join("test_wallet.json");
    let mut wallet = Wallet::new(wallet_path.clone())?;
    info!("Wallet created at {:?} with address {}", wallet_path, wallet.get_address());
    
    // Add some initial balance to the wallet for testing
    let initial_utxo = wallet::core::UTXO {
        tx_hash: [1u8; 32],
        output_index: 0,
        amount: 1000_000, // 1 million units
        script_pubkey: hex::decode(wallet.get_address()).unwrap(),
    };
    wallet.add_utxo(initial_utxo);
    wallet.save()?;
    info!("Added initial balance of {} to wallet", 1000_000);
    
    // Record test start time
    let test_start_time = Instant::now();
    
    // Start network task
    let network_handle = tokio::spawn(async move {
        info!("Starting network event loop");
        if let Err(e) = network.run().await {
            error!("Network error: {}", e);
        }
    });
    
    // Start mining task
    let mining_handle = tokio::spawn(async move {
        info!("Starting mining process");
        if let Err(e) = miner.start_mining(1, [0u8; 32], 0).await {
            error!("Mining error: {}", e);
        }
    });
    
    // Handle mined blocks in separate task
    let chain_state_clone = chain_state.clone();
    let block_handle = tokio::spawn(async move {
        info!("Starting block processing task");
        let mut blocks_processed = 0;
        
        while let Some(block) = block_rx.recv().await {
            blocks_processed += 1;
            let block_hash = hex::encode(&block.hash()[0..4]); // First 4 bytes for readability
            info!("New block mined! Hash: {}..., height: {}", 
                 block_hash, blocks_processed);
            
            match chain_state_clone.process_block(block).await {
                Ok(true) => debug!("Block {} successfully added to chain", block_hash),
                Ok(false) => warn!("Block {} was not added to main chain", block_hash),
                Err(e) => error!("Error processing mined block {}: {}", block_hash, e),
            }
        }
    });
    
    // Wait for some blocks to be mined before proceeding
    info!("Waiting for initial blocks to be mined");
    let blocks_to_wait = 3;
    let timeout_duration = Duration::from_secs(30);
    
    let blocks_mined = timeout(timeout_duration, async {
        let mut count = 0;
        while count < blocks_to_wait {
            if chain_state.get_height() > count as u64 {
                count = chain_state.get_height() as usize;
                info!("Chain height now: {}", count);
            }
            tokio::time::sleep(Duration::from_millis(500)).await;
        }
        Ok::<_, Box<dyn std::error::Error>>(count)
    }).await??;
    
    info!("Initial mining complete, {} blocks created in {:.2} seconds", 
         blocks_mined, test_start_time.elapsed().as_secs_f64());
    
    // Create multiple transactions with different characteristics
    let test_transactions = create_test_transactions(5, &wallet)?;
    
    // Record all transaction hashes for later verification
    let tx_hashes: HashSet<_> = test_transactions.iter()
        .map(|tx| tx.hash())
        .collect();
    
    info!("Created {} test transactions", test_transactions.len());
    
    // Broadcast transactions
    for (i, tx) in test_transactions.iter().enumerate() {
        let tx_hash = hex::encode(&tx.hash()[0..4]);
        info!("Broadcasting transaction {}/{}: {}...", i+1, test_transactions.len(), tx_hash);
        
        command_tx.send(NetworkCommand::AnnounceTransaction {
            transaction: tx.clone(),
            fee_rate: 1 + (i as u64), // Increasing fee rates
        }).await?;
        
        // Small delay between transactions
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    
    // Verify transactions were added to mempool
    for tx in &test_transactions {
        let tx_hash = hex::encode(&tx.hash()[0..4]);
        if mempool.get_transaction(&tx.hash()).is_some() {
            debug!("Transaction {}... added to mempool", tx_hash);
        } else {
            error!("Transaction {}... not found in mempool", tx_hash);
            return Err("Transaction not added to mempool".into());
        }
    }
    
    info!("All transactions successfully added to mempool");
    
    // Wait for transactions to be mined
    info!("Waiting for transactions to be mined");
    let tx_mining_timeout = Duration::from_secs(60);
    let tx_mining_result = timeout(tx_mining_timeout, async {
        let mut mined_tx_count = 0;
        let start_time = Instant::now();
        
        while mined_tx_count < tx_hashes.len() {
            // Wait for next block
            tokio::time::sleep(Duration::from_secs(1)).await;
            
            // Check if height increased
            let height = chain_state.get_height();
            
            // Check for our transactions in recent blocks
            for h in (blocks_mined as u64 + 1)..=height {
                let block = chain_state.get_block_at_height(h)?;
                let found_txs: Vec<_> = block.transactions()
                    .iter()
                    .filter(|tx| tx_hashes.contains(&tx.hash()))
                    .collect();
                
                if !found_txs.is_empty() {
                    for tx in &found_txs {
                        let tx_hash = hex::encode(&tx.hash()[0..4]);
                        info!("Transaction {}... mined in block at height {}", tx_hash, h);
                    }
                    mined_tx_count += found_txs.len();
                }
            }
            
            // Report progress
            if mined_tx_count > 0 {
                debug!("Mining progress: {}/{} transactions mined", 
                     mined_tx_count, tx_hashes.len());
            }
        }
        
        let mining_duration = start_time.elapsed();
        info!("All transactions mined in {:.2} seconds", mining_duration.as_secs_f64());
        
        Ok::<_, Box<dyn std::error::Error>>(true)
    }).await;
    
    // Verify all transactions were mined
    match tx_mining_result {
        Ok(Ok(_)) => {
            info!("All transactions successfully mined and verified!");
        },
        Ok(Err(e)) => {
            error!("Error during transaction mining verification: {}", e);
            return Err(e);
        },
        Err(_) => {
            error!("Timeout waiting for transactions to be mined");
            return Err("Transaction mining timeout".into());
        }
    }
    
    // Test chain reorganization
    info!("Testing chain reorganization");
    
    // Create a fork from an earlier block
    let fork_height = chain_state.get_height() / 2;
    let fork_block = chain_state.get_block_at_height(fork_height)?;
    let fork_hash = fork_block.hash();
    
    // Create a competing chain with higher difficulty
    info!("Creating competing chain from height {}", fork_height);
    let competing_block = Block::new(
        1,
        fork_hash,
        create_test_transactions(2, &wallet)?,
        (u32::MAX / 200), // Higher difficulty than main chain
    );
    
    // Manually process the fork block
    match chain_state.process_block(competing_block.clone()).await {
        Ok(true) => info!("Fork block added to chain, reorganization successful"),
        Ok(false) => warn!("Fork block not added, reorganization failed"),
        Err(e) => error!("Error processing fork block: {}", e),
    }
    
    // Clean up
    info!("Test completed, shutting down");
    mining_handle.abort();
    network_handle.abort();
    block_handle.abort();
    
    // Report test duration
    let test_duration = test_start_time.elapsed();
    info!("Full blockchain integration test completed in {:.2} seconds", 
         test_duration.as_secs_f64());
    
    Ok(())
}

/// Helper to create realistic transactions for testing
fn create_test_transactions(count: usize, wallet: &Wallet) -> Result<Vec<Transaction>, Box<dyn std::error::Error>> {
    let mut transactions = Vec::with_capacity(count);
    
    for i in 0..count {
        // Create unique recipient addresses
        let mut recipient = [0u8; 32];
        recipient[0] = 0x02; // Start with 0x02 to make it look like a compressed public key
        recipient[1] = (i >> 8) as u8;
        recipient[2] = i as u8;
        
        let recipient_address = hex::encode(recipient);
        
        // Create transaction with varying amounts and fees
        let amount = 1000 + (i * 500) as u64;
        let fee = 10 + i as u64;
        
        let transaction = wallet.create_transaction(&recipient_address, amount, fee)?;
        transactions.push(transaction);
    }
    
    Ok(transactions)
}

/// Test multi-node network with transaction propagation
#[tokio::test]
async fn test_multinode_network() -> Result<(), Box<dyn std::error::Error>> {
    // Set up logging
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();
    
    info!("Starting multi-node network test");
    
    // Create multiple independent nodes
    const NODE_COUNT: usize = 3;
    let mut nodes = Vec::with_capacity(NODE_COUNT);
    let mut command_txs = Vec::with_capacity(NODE_COUNT);
    let mut event_rxs = Vec::with_capacity(NODE_COUNT);
    
    for i in 0..NODE_COUNT {
        // Create temporary directory for this node
        let node_dir = tempdir()?;
        
        // Initialize node components
        let db = Arc::new(BlockchainDB::new(node_dir.path())?);
        let chain_state = ChainState::new(Arc::clone(&db))?;
        
        // Initialize mempool
        let mempool_config = node::mempool::MempoolConfig::default();
        let mempool = Arc::new(node::mempool::TransactionPool::new(mempool_config));
        
        // Initialize network
        let genesis_hash = chain_state.get_genesis_hash();
        let (network, command_tx, event_rx) = 
            P2PNetwork::new(None, genesis_hash, &format!("supernova-test-node-{}", i)).await?;
        
        info!("Created node {} with peer ID: {}", i, network.local_peer_id);
        
        nodes.push((network, chain_state, mempool, db));
        command_txs.push(command_tx);
        event_rxs.push(event_rx);
    }
    
    // Start network tasks for each node
    let mut network_handles = Vec::new();
    for (i, (network, _, _, _)) in nodes.iter_mut().enumerate() {
        let network = std::mem::replace(network, P2PNetwork::default());
        network_handles.push(tokio::spawn(async move {
            info!("Starting network event loop for node {}", i);
            if let Err(e) = network.run().await {
                error!("Network error on node {}: {}", i, e);
            }
        }));
    }
    
    // Connect nodes to each other
    info!("Connecting nodes to form a network");
    for i in 0..NODE_COUNT {
        for j in 0..NODE_COUNT {
            if i != j {
                // Connect node i to node j
                let node_j = &nodes[j].0;
                let addr = format!("/ip4/127.0.0.1/tcp/{}/p2p/{}", 
                                  8000 + j as u16, 
                                  node_j.local_peer_id);
                
                debug!("Node {} connecting to Node {} at {}", i, j, addr);
                command_txs[i].send(NetworkCommand::Dial(addr)).await?;
            }
        }
    }
    
    // Wait for connections to establish
    tokio::time::sleep(Duration::from_secs(2)).await;
    
    // Create a transaction on node 0
    info!("Creating and broadcasting a test transaction");
    let wallet_dir = tempdir()?;
    let wallet_path = wallet_dir.path().join("test_wallet.json");
    let mut wallet = Wallet::new(wallet_path)?;
    
    // Add some initial balance
    let initial_utxo = wallet::core::UTXO {
        tx_hash: [1u8; 32],
        output_index: 0,
        amount: 1000_000,
        script_pubkey: vec![1, 2, 3, 4],
    };
    wallet.add_utxo(initial_utxo);
    
    // Create a transaction
    let recipient = hex::encode([5u8; 32]);
    let tx = wallet.create_transaction(&recipient, 1000, 10)?;
    
    // Broadcast from node 0 to the network
    command_txs[0].send(NetworkCommand::AnnounceTransaction {
        transaction: tx.clone(),
        fee_rate: 1,
    }).await?;
    
    info!("Transaction broadcasted from node 0, hash: {}", hex::encode(&tx.hash()[0..4]));
    
    // Wait for transaction propagation
    let propagation_timeout = Duration::from_secs(5);
    let mut propagation_success = true;
    
    info!("Waiting for transaction to propagate to all nodes");
    for i in 1..NODE_COUNT {
        let tx_received = timeout(propagation_timeout, async {
            let mut event_rx = &mut event_rxs[i];
            
            while let Some(event) = event_rx.recv().await {
                if let NetworkEvent::NewTransaction { transaction, .. } = event {
                    if transaction.hash() == tx.hash() {
                        info!("Node {} received the transaction", i);
                        return true;
                    }
                }
            }
            
            false
        }).await;
        
        if tx_received.is_err() || !tx_received.unwrap() {
            error!("Node {} did not receive the transaction within timeout", i);
            propagation_success = false;
        }
    }
    
    assert!(propagation_success, "Transaction should propagate to all nodes");
    
    // Check mempools to verify transaction was added
    info!("Verifying transaction presence in all mempools");
    for i in 0..NODE_COUNT {
        let mempool = &nodes[i].2;
        if mempool.get_transaction(&tx.hash()).is_some() {
            info!("Transaction found in node {}'s mempool", i);
        } else {
            error!("Transaction not found in node {}'s mempool", i);
            propagation_success = false;
        }
    }
    
    assert!(propagation_success, "Transaction should be in all nodes' mempools");
    
    // Clean up
    info!("Test completed, shutting down nodes");
    for handle in network_handles {
        handle.abort();
    }
    
    info!("Multi-node network test completed successfully");
    Ok(())
}

/// Test sync process between nodes at different heights
#[tokio::test]
async fn test_blockchain_synchronization() -> Result<(), Box<dyn std::error::Error>> {
    // Set up logging
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();
    
    info!("Starting blockchain synchronization test");
    
    // Create two nodes: a "seed" node with blocks and a "new" node to sync
    let seed_node_dir = tempdir()?;
    let new_node_dir = tempdir()?;
    
    // Initialize seed node components
    let seed_db = Arc::new(BlockchainDB::new(seed_node_dir.path())?);
    let mut seed_chain = ChainState::new(Arc::clone(&seed_db))?;
    
    // Initialize seed node network
    let genesis_hash = seed_chain.get_genesis_hash();
    let (seed_network, seed_command_tx, mut seed_event_rx) = 
        P2PNetwork::new(None, genesis_hash, "supernova-test-seed-node").await?;
    
    // Create some blocks on the seed node
    info!("Creating blockchain history on seed node");
    let block_count = 10;
    let mut prev_hash = genesis_hash;
    
    for i in 1..=block_count {
        let block = Block::new(
            1,
            prev_hash,
            Vec::new(), // Empty transactions for simplicity
            u32::MAX / 2, // Moderate difficulty
        );
        
        // Process block
        match seed_chain.process_block(block.clone()).await {
            Ok(true) => {
                info!("Added block {} to seed node at height {}", hex::encode(&block.hash()[0..4]), i);
                prev_hash = block.hash();
            },
            Ok(false) => {
                warn!("Block not added to main chain on seed node");
            },
            Err(e) => {
                error!("Error processing block on seed node: {}", e);
                return Err(e.into());
            }
        }
    }
    
    info!("Seed node blockchain created with {} blocks", block_count);
    
    // Initialize new node that needs to sync
    let new_db = Arc::new(BlockchainDB::new(new_node_dir.path())?);
    let new_chain = ChainState::new(Arc::clone(&new_db))?;
    assert_eq!(new_chain.get_height(), 0, "New node should start with empty blockchain");
    
    // Initialize new node network
    let (new_network, new_command_tx, mut new_event_rx) = 
        P2PNetwork::new(None, genesis_hash, "supernova-test-new-node").await?;
    
    // Start network tasks
    let seed_network_handle = tokio::spawn(async move {
        info!("Starting seed node network");
        if let Err(e) = seed_network.run().await {
            error!("Seed node network error: {}", e);
        }
    });
    
    let new_network_handle = tokio::spawn(async move {
        info!("Starting new node network");
        if let Err(e) = new_network.run().await {
            error!("New node network error: {}", e);
        }
    });
    
    // Connect new node to seed node
    info!("Connecting new node to seed node");
    let seed_addr = format!("/ip4/127.0.0.1/tcp/8000/p2p/{}", seed_network.local_peer_id);
    new_command_tx.send(NetworkCommand::Dial(seed_addr)).await?;
    
    // Wait for connection to establish
    tokio::time::sleep(Duration::from_secs(1)).await;
    
    // Announce seed node status to trigger sync
    info!("Announcing seed node status to trigger synchronization");
    seed_command_tx.send(NetworkCommand::AnnounceStatus {
        version: 1,
        height: block_count,
        best_hash: prev_hash,
        total_difficulty: block_count * 100, // Example difficulty calculation
    }).await?;
    
    // Wait for synchronization to complete
    info!("Waiting for new node to synchronize blocks");
    let sync_timeout = Duration::from_secs(30);
    let sync_result = timeout(sync_timeout, async {
        let sync_start = Instant::now();
        let mut last_height = 0;
        let mut no_progress_count = 0;
        
        loop {
            // Check current height of new node
            let current_height = new_chain.get_height();
            
            if current_height == block_count {
                // Sync completed
                info!("Synchronization completed! New node height: {}", current_height);
                let duration = sync_start.elapsed();
                info!("Sync completed in {:.2} seconds", duration.as_secs_f64());
                return Ok::<_, Box<dyn std::error::Error>>(true);
            }
            
            if current_height > last_height {
                // Making progress
                info!("Sync progress: {} of {} blocks", current_height, block_count);
                last_height = current_height;
                no_progress_count = 0;
            } else {
                no_progress_count += 1;
                if no_progress_count > 10 {
                    // Too many iterations without progress, something might be wrong
                    return Err("Sync stalled - no progress for too long".into());
                }
            }
            
            // Wait before checking again
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
    }).await;
    
    // Verify sync result
    match sync_result {
        Ok(Ok(_)) => {
            info!("Blockchain synchronization test passed!");
            
            // Verify last block hash matches
            let new_best_hash = new_chain.get_best_block_hash();
            if new_best_hash == prev_hash {
                info!("Block hashes match! New node has synchronized correctly");
            } else {
                error!("Block hashes don't match after sync");
                return Err("Blockchain verification failed".into());
            }
        },
        Ok(Err(e)) => {
            error!("Sync error: {}", e);
            return Err(e);
        },
        Err(_) => {
            error!("Synchronization timed out");
            return Err("Sync timeout".into());
        }
    }
    
    // Clean up
    seed_network_handle.abort();
    new_network_handle.abort();
    
    info!("Blockchain synchronization test completed successfully");
    Ok(())
}