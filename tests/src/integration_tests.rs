use node::network::{P2PNetwork, NetworkCommand, NetworkEvent};
use node::storage::{ChainState, BlockchainDB};
use miner::mining::Miner;
use wallet::core::Wallet;
use btclib::types::{Block, Transaction};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::time::timeout;
use tempfile::tempdir;
use tracing::{info, warn, error};

/// Test complete workflow: mine blocks, create transactions, mine and verify them
#[tokio::test]
async fn test_full_workflow() -> Result<(), Box<dyn std::error::Error>> {
    // Set up logging
    tracing_subscriber::fmt::init();
    
    // Create temporary directories
    let node_dir = tempdir()?;
    let wallet_dir = tempdir()?;
    
    // Initialize node components
    let db = Arc::new(BlockchainDB::new(node_dir.path())?);
    let chain_state = ChainState::new(Arc::clone(&db))?;
    
    // Initialize mempool
    let mempool_config = node::mempool::MempoolConfig::default();
    let mempool = Arc::new(node::mempool::TransactionPool::new(mempool_config));
    
    // Initialize network
    let genesis_hash = chain_state.get_genesis_hash();
    let (network, command_tx, mut event_rx) = 
        P2PNetwork::new(None, genesis_hash, "supernova-test").await?;
    
    // Initialize miner
    let reward_address = vec![1, 2, 3, 4]; // Dummy reward address
    let (miner, mut block_rx) = Miner::new(
        4, // 4 threads
        u32::MAX, // Easy target for testing
        Arc::clone(&mempool),
        reward_address.clone(),
    );
    
    // Initialize wallet
    let wallet_path = wallet_dir.path().join("test_wallet.json");
    let wallet = Wallet::new(wallet_path)?;
    
    // Start network and mining tasks
    let network_handle = tokio::spawn(async move {
        if let Err(e) = network.run().await {
            error!("Network error: {}", e);
        }
    });
    
    let mining_handle = tokio::spawn(async move {
        if let Err(e) = miner.start_mining(1, [0u8; 32], 0).await {
            error!("Mining error: {}", e);
        }
    });
    
    // Handle mined blocks
    let chain_state_clone = chain_state.clone();
    let block_handle = tokio::spawn(async move {
        while let Some(block) = block_rx.recv().await {
            info!("New block mined! Hash: {:?}", block.hash());
            let result = chain_state_clone.process_block(block).await;
            if let Err(e) = result {
                error!("Error processing mined block: {}", e);
            }
        }
    });
    
    // Wait for some blocks to be mined
    tokio::time::sleep(Duration::from_secs(10)).await;
    
    // Create a transaction
    let recipient = hex::encode([5, 6, 7, 8]); // Dummy recipient
    let transaction = wallet.create_transaction(&recipient, 10, 1)?;
    
    // Broadcast transaction
    command_tx.send(NetworkCommand::AnnounceTransaction {
        transaction: transaction.clone(),
        fee_rate: 1,
    }).await?;
    
    // Verify transaction was added to mempool
    assert!(mempool.get_transaction(&transaction.hash()).is_some(), 
           "Transaction should be added to mempool");
    
    // Wait for transaction to be mined
    let timeout_duration = Duration::from_secs(30);
    let result = timeout(timeout_duration, async {
        loop {
            tokio::time::sleep(Duration::from_secs(1)).await;
            let height = chain_state.get_height();
            let block = chain_state.get_block_at_height(height)?;
            if block.transactions().iter().any(|tx| tx.hash() == transaction.hash()) {
                return Ok::<_, Box<dyn std::error::Error>>(true);
            }
        }
    }).await;
    
    assert!(result.is_ok() && result.unwrap()?, "Transaction should be mined within timeout");
    
    // Clean up
    mining_handle.abort();
    network_handle.abort();
    block_handle.abort();
    
    Ok(())
}