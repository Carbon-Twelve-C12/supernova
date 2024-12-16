use crate::network::{P2PNetwork, NetworkCommand, NetworkEvent};
use crate::storage::ChainState;
use crate::mempool::{TransactionPool, MempoolConfig, TransactionPrioritizer, PrioritizationConfig};
use tracing::{info, error};
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};
use bincode;

struct Node {
    mempool: Arc<TransactionPool>,
    prioritizer: Arc<Mutex<TransactionPrioritizer>>,
    network: P2PNetwork,
    chain_state: ChainState,
}

impl Node {
    async fn new() -> Result<Self, Box<dyn std::error::Error>> {
        // Initialize mempool
        let mempool_config = MempoolConfig::default();
        let prioritization_config = PrioritizationConfig::default();
        let mempool = Arc::new(TransactionPool::new(mempool_config));
        let prioritizer = Arc::new(Mutex::new(TransactionPrioritizer::new(prioritization_config)));

        // Initialize storage
        let db = Arc::new(storage::BlockchainDB::new("./data")?);
        let chain_state = ChainState::new(db)?;

        // Initialize network
        let (network, _, _) = P2PNetwork::new().await?;

        Ok(Self {
            mempool,
            prioritizer,
            network,
            chain_state,
        })
    }

    async fn handle_new_transaction(&self, transaction: Transaction) -> Result<(), Box<dyn std::error::Error>> {
        // Check if transaction is already in mempool
        let tx_hash = transaction.hash();
        if self.mempool.get_transaction(&tx_hash).is_some() {
            return Ok(());
        }

        // Check for double spends
        if self.mempool.check_double_spend(&transaction) {
            return Err("Double spend detected".into());
        }

        // Calculate fee rate
        let tx_size = bincode::serialize(&transaction)?.len();
        let fee_rate = transaction.calculate_fee_rate()?;

        // Add to mempool and prioritizer
        self.mempool.add_transaction(transaction.clone(), fee_rate)?;
        
        let mut prioritizer = self.prioritizer.lock().await;
        prioritizer.add_transaction(transaction, fee_rate, tx_size);

        Ok(())
    }

    async fn get_transactions_for_block(&self, max_size: usize) -> Vec<Transaction> {
        let prioritizer = self.prioritizer.lock().await;
        let prioritized = prioritizer.get_prioritized_transactions();
        
        let mut selected = Vec::new();
        let mut total_size = 0;

        for tx in prioritized {
            let tx_size = bincode::serialize(tx).unwrap().len();
            if total_size + tx_size > max_size {
                break;
            }
            selected.push(tx.clone());
            total_size += tx_size;
        }

        selected
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    // Initialize node
    let node = Arc::new(Node::new().await?);
    let node_clone = Arc::clone(&node);

    // Initialize network
    let (mut network, command_tx, mut event_rx) = P2PNetwork::new().await?;

    // Initialize chain sync
    let mut sync = network::ChainSync::new(node.chain_state.clone(), command_tx.clone());

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
                    // Handle new transaction
                    match bincode::deserialize(&tx_data) {
                        Ok(transaction) => {
                            if let Err(e) = node_clone.handle_new_transaction(transaction).await {
                                error!("Failed to process transaction: {}", e);
                            }
                        }
                        Err(e) => error!("Failed to deserialize transaction: {}", e),
                    }
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

    // Periodic mempool maintenance
    let node_clone = Arc::clone(&node);
    let maintenance_handle = tokio::spawn(async move {
        let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(300));
        loop {
            interval.tick().await;
            // Clear expired transactions
            let removed = node_clone.mempool.clear_expired();
            if removed > 0 {
                info!("Cleared {} expired transactions from mempool", removed);
            }
        }
    });

    // Wait for interrupt signal
    tokio::signal::ctrl_c().await?;
    info!("Shutting down...");

    // Wait for tasks to complete
    event_handle.await?;
    network_handle.await?;
    maintenance_handle.await?;

    Ok(())
}