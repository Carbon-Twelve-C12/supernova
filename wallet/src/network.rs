use btclib::types::Transaction;
use tokio::net::TcpStream;
use std::error::Error;
use serde::{Serialize, Deserialize};

#[derive(Debug, Serialize, Deserialize)]
enum NetworkMessage {
    BroadcastTransaction(Vec<u8>),
}

pub struct NetworkClient {
    node_address: String,
}

impl NetworkClient {
    pub fn new(node_address: String) -> Self {
        Self { node_address }
    }

    pub async fn broadcast_transaction(&self, transaction: Transaction) -> Result<(), Box<dyn Error>> {
        let stream = TcpStream::connect(&self.node_address).await?;

        // Serialize transaction
        let tx_data = bincode::serialize(&transaction)?;
        let message = NetworkMessage::BroadcastTransaction(tx_data);

        // Send to node
        bincode::serialize_into(&stream, &message)?;

        Ok(())
    }
}