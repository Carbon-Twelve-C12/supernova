use tracing::info;
use miner::mining::Miner;
use btclib::types::transaction::Transaction;
use std::sync::Arc;
use async_trait::async_trait;

// Mock implementation of MempoolInterface for the main program
struct EmptyMempool;

#[async_trait]
impl miner::mining::MempoolInterface for EmptyMempool {
    async fn get_transactions(&self, _max_size: usize) -> Vec<Transaction> {
        Vec::new() // Return empty transaction list
    }
}

#[tokio::main]
async fn main() {
    // Initialize logging
    tracing_subscriber::fmt::init();

    // Create mempool and reward address
    let mempool = Arc::new(EmptyMempool);
    let reward_address = vec![1, 2, 3, 4]; // Simple test address

    // Create miner with 4 threads and initial target
    let (miner, mut block_rx) = Miner::new(4, 0x1d00ffff, mempool, reward_address);

    // Start mining
    let _mining_task = tokio::spawn({
        let miner = miner.clone();
        async move {
            if let Err(e) = miner.start_mining(1, [0u8; 32], 0).await {
                eprintln!("Mining error: {}", e);
            }
        }
    });

    // Handle mined blocks in a separate task
    let _block_handle = tokio::spawn(async move {
        while let Some(block) = block_rx.recv().await {
            info!("New block mined! Hash: {:?}", block.hash());
        }
    });

    // Wait for ctrl-c
    tokio::signal::ctrl_c()
        .await
        .expect("Failed to listen for ctrl-c");

    info!("Shutting down miner...");

    // Wait for the block handling task to complete
    _block_handle.await.expect("Failed to join block handling task");
}