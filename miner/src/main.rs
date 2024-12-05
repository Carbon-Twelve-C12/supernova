use tracing::info;
use crate::mining::coordinator::Miner;

#[tokio::main]
async fn main() {
    // Initialize logging
    tracing_subscriber::fmt::init();

    // Create miner with 4 threads and initial target
    let (mut miner, mut block_rx) = Miner::new(4, 0x1d00ffff);

    // Handle mined blocks in a separate task
    let handle = tokio::spawn(async move {
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
    handle.await.expect("Failed to join block handling task");
}