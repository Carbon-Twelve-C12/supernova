use crate::network::{P2PNetwork, NetworkCommand, NetworkEvent};
use crate::storage::ChainState;
use tracing::{info, error};
use std::sync::Arc;
use tokio::sync::mpsc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    // Initialize storage
    let db = Arc::new(storage::BlockchainDB::new("./data")?);
    let chain_state = ChainState::new(db)?;

    // Initialize network
    let (mut network, command_tx, mut event_rx) = P2PNetwork::new().await?;

    // Initialize chain sync
    let mut sync = network::ChainSync::new(chain_state, command_tx.clone());

    // Start network event handling
    let event_handle = tokio::spawn(async move {
        while let Some(event) = event_rx.recv().await {
            match event {
                NetworkEvent::NewPeer(peer_id) => {
                    info!("New peer connected: {}", peer_id);
                }
                NetworkEvent::PeerLeft(peer_id) => {
                    info!("Peer disconnected: {}", peer_id);
                }
                NetworkEvent::NewBlock(block_data) => {
                    // Deserialize and handle new block
                    match bincode::deserialize(&block_data) {
                        Ok((block, height, total_difficulty)) => {
                            if let Err(e) = sync.handle_new_block(block, height, total_difficulty).await {
                                error!("Failed to process new block: {}", e);
                            }
                        }
                        Err(e) => error!("Failed to deserialize block: {}", e),
                    }
                }
                NetworkEvent::NewTransaction(tx_data) => {
                    // TODO: Handle new transaction
                    info!("Received new transaction");
                }
            }
        }
    });

    // Start network
    let network_handle = tokio::spawn(async move {
        if let Err(e) = network.run().await {
            error!("Network error: {}", e);
        }
    });

    // Start listening on default port
    command_tx.send(NetworkCommand::StartListening(
        "/ip4/0.0.0.0/tcp/8000".into()
    )).await?;

    // Wait for interrupt signal
    tokio::signal::ctrl_c().await?;
    info!("Shutting down...");

    // Wait for tasks to complete
    event_handle.await?;
    network_handle.await?;

    Ok(())
}