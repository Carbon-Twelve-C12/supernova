use libp2p::gossipsub::{
    self, IdentTopic as Topic, MessageAuthenticity, ValidationMode,
};
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::time::Duration;
use btclib::types::Transaction;

const BLOCKS_TOPIC: &str = "supernova/blocks/1.0.0";
const TXS_TOPIC: &str = "supernova/transactions/1.0.0";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Message {
    NewBlock {
        block_data: Vec<u8>,
        height: u64,
        total_difficulty: u64,
    },
    NewTransaction(Vec<u8>),
    BroadcastTransaction(Transaction),
    TransactionAnnouncement(Vec<u8>), // Transaction hash
    GetBlocks {
        start_height: u64,
        end_height: u64,
    },
    BlockResponse {
        blocks: Vec<Vec<u8>>,
        total_difficulty: u64,
    },
    GetMempool,
    MempoolResponse(Vec<Vec<u8>>),
}

pub struct Protocol {
    gossipsub: gossipsub::Behaviour,
}

impl Protocol {
    pub fn new(keypair: identity::Keypair) -> Result<Self, Box<dyn Error>> {
        // Configure gossipsub
        let gossipsub_config = gossipsub::ConfigBuilder::default()
            .heartbeat_interval(Duration::from_secs(1))
            .validation_mode(ValidationMode::Strict)
            .message_id_fn(|message: &gossipsub::Message| {
                // Custom message ID function to prevent duplicate messages
                let mut hasher = sha2::Sha256::new();
                hasher.update(&message.data);
                gossipsub::MessageId::from(hasher.finalize().to_vec())
            })
            .build()
            .map_err(|e| format!("Failed to build gossipsub config: {}", e))?;

        // Create gossipsub behavior
        let gossipsub = gossipsub::Behaviour::new(
            MessageAuthenticity::Signed(keypair),
            gossipsub_config,
        )?;

        Ok(Self { gossipsub })
    }

    pub fn subscribe_to_topics(&mut self) -> Result<(), Box<dyn Error>> {
        let blocks_topic = Topic::new(BLOCKS_TOPIC);
        let txs_topic = Topic::new(TXS_TOPIC);
        
        self.gossipsub.subscribe(&blocks_topic)?;
        self.gossipsub.subscribe(&txs_topic)?;
        Ok(())
    }

    pub fn publish_block(&mut self, block_data: Vec<u8>, height: u64, total_difficulty: u64) 
        -> Result<MessageId, PublishError> {
        let message = Message::NewBlock {
            block_data,
            height,
            total_difficulty,
        };
        let encoded = bincode::serialize(&message)?;
        self.gossipsub.publish(Topic::new(BLOCKS_TOPIC), encoded)
    }

    pub fn publish_transaction(&mut self, tx_data: Vec<u8>) -> Result<MessageId, PublishError> {
        let message = Message::NewTransaction(tx_data);
        let encoded = bincode::serialize(&message)?;
        self.gossipsub.publish(Topic::new(TXS_TOPIC), encoded)
    }

    pub fn broadcast_transaction(&mut self, transaction: Transaction) -> Result<MessageId, PublishError> {
        let message = Message::BroadcastTransaction(transaction);
        let encoded = bincode::serialize(&message)?;
        self.gossipsub.publish(Topic::new(TXS_TOPIC), encoded)
    }

    pub fn announce_transaction(&mut self, tx_hash: Vec<u8>) -> Result<MessageId, PublishError> {
        let message = Message::TransactionAnnouncement(tx_hash);
        let encoded = bincode::serialize(&message)?;
        self.gossipsub.publish(Topic::new(TXS_TOPIC), encoded)
    }

    pub fn request_blocks(&mut self, start_height: u64, end_height: u64) 
        -> Result<MessageId, PublishError> {
        let message = Message::GetBlocks {
            start_height,
            end_height,
        };
        let encoded = bincode::serialize(&message)?;
        self.gossipsub.publish(Topic::new(BLOCKS_TOPIC), encoded)
    }

    pub fn send_blocks(&mut self, blocks: Vec<Vec<u8>>, total_difficulty: u64) 
        -> Result<MessageId, PublishError> {
        let message = Message::BlockResponse {
            blocks,
            total_difficulty,
        };
        let encoded = bincode::serialize(&message)?;
        self.gossipsub.publish(Topic::new(BLOCKS_TOPIC), encoded)
    }

    pub fn request_mempool(&mut self) -> Result<MessageId, PublishError> {
        let message = Message::GetMempool;
        let encoded = bincode::serialize(&message)?;
        self.gossipsub.publish(Topic::new(TXS_TOPIC), encoded)
    }

    pub fn send_mempool(&mut self, transactions: Vec<Vec<u8>>) -> Result<MessageId, PublishError> {
        let message = Message::MempoolResponse(transactions);
        let encoded = bincode::serialize(&message)?;
        self.gossipsub.publish(Topic::new(TXS_TOPIC), encoded)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum PublishError {
    #[error("Serialization error: {0}")]
    Serialization(#[from] bincode::Error),
    #[error("Gossipsub error: {0}")]
    Gossipsub(#[from] gossipsub::error::PublishError),
}

#[cfg(test)]
mod tests {
    use super::*;
    use libp2p::identity;

    #[test]
    fn test_protocol_creation() {
        let keypair = identity::Keypair::generate_ed25519();
        let protocol = Protocol::new(keypair).unwrap();
        
        assert!(protocol.gossipsub.topics().count() == 0);
    }

    #[test]
    fn test_topic_subscription() {
        let keypair = identity::Keypair::generate_ed25519();
        let mut protocol = Protocol::new(keypair).unwrap();
        
        protocol.subscribe_to_topics().unwrap();
        assert!(protocol.gossipsub.topics().count() == 2);
    }

    #[test]
    fn test_message_serialization() {
        let block_data = vec![1, 2, 3, 4];
        let message = Message::NewBlock {
            block_data: block_data.clone(),
            height: 1,
            total_difficulty: 100,
        };
        
        let encoded = bincode::serialize(&message).unwrap();
        let decoded: Message = bincode::deserialize(&encoded).unwrap();
        
        match decoded {
            Message::NewBlock { block_data: decoded_data, height, total_difficulty } => {
                assert_eq!(decoded_data, block_data);
                assert_eq!(height, 1);
                assert_eq!(total_difficulty, 100);
            }
            _ => panic!("Wrong message type after deserialization"),
        }
    }

    #[test]
    fn test_transaction_broadcast() {
        let keypair = identity::Keypair::generate_ed25519();
        let mut protocol = Protocol::new(keypair).unwrap();
        
        let transaction = Transaction::new(1, vec![], vec![], 0); // Empty test transaction
        let result = protocol.broadcast_transaction(transaction);
        assert!(result.is_ok());
    }
}