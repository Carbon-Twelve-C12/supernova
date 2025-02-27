use libp2p::gossipsub::{
    self, IdentTopic as Topic, MessageAuthenticity, ValidationMode,
};
use libp2p::identity::Keypair;
use libp2p::PeerId;
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::time::Duration;
use btclib::types::{Block, BlockHeader, Transaction};
use sha2::{Sha256, Digest};

// Topic constants
const BLOCKS_TOPIC: &str = "supernova/blocks/1.0.0";
const TXS_TOPIC: &str = "supernova/transactions/1.0.0";
const HEADERS_TOPIC: &str = "supernova/headers/1.0.0";
const STATUS_TOPIC: &str = "supernova/status/1.0.0";

/// Network protocol messages for communication between nodes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Message {
    /// New block announcement with height and total difficulty
    NewBlock {
        block_data: Vec<u8>,
        height: u64,
        total_difficulty: u64,
    },
    
    /// New transaction announcement
    NewTransaction(Vec<u8>),
    
    /// Broadcast a complete transaction
    BroadcastTransaction(Transaction),
    
    /// Announce a transaction hash
    TransactionAnnouncement {
        tx_hash: [u8; 32],
        fee_rate: u64,
    },
    
    /// Request blocks by hash
    GetBlocks {
        block_hashes: Vec<[u8; 32]>,
    },
    
    /// Request blocks by height range
    GetBlocksByHeight {
        start_height: u64,
        end_height: u64,
    },
    
    /// Block response containing multiple blocks
    BlockResponse {
        blocks: Vec<Block>,
        total_difficulty: u64,
    },
    
    /// Request headers for a height range
    GetHeaders {
        start_height: u64,
        end_height: u64,
    },
    
    /// Headers response
    Headers {
        headers: Vec<BlockHeader>,
        total_difficulty: u64,
    },
    
    /// Request mempool contents
    GetMempool,
    
    /// Mempool response
    MempoolResponse {
        transaction_hashes: Vec<[u8; 32]>,
    },
    
    /// Node status announcement
    Status {
        version: u32,
        height: u64,
        best_hash: [u8; 32],
        total_difficulty: u64,
        head_timestamp: u64,
    },
    
    /// Request node status
    GetStatus,
    
    /// Ping to measure latency
    Ping(u64),
    
    /// Pong response with timestamp from ping
    Pong(u64),
    
    /// Request checkpoint information
    GetCheckpoints {
        since_timestamp: u64,
    },
    
    /// Checkpoint information response
    Checkpoints {
        checkpoints: Vec<Checkpoint>,
    },
}

/// Checkpoint information for validation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Checkpoint {
    pub height: u64,
    pub hash: [u8; 32],
    pub timestamp: u64,
    pub signature: Option<Vec<u8>>, // Optional trusted signature
}

/// Main protocol implementation
pub struct Protocol {
    gossipsub: gossipsub::Behaviour,
    local_peer_id: PeerId,
}

impl Protocol {
    /// Create a new protocol instance
    pub fn new(keypair: Keypair) -> Result<Self, Box<dyn Error>> {
        let local_peer_id = PeerId::from(keypair.public());
        
        // Configure gossipsub with appropriate parameters
        let gossipsub_config = gossipsub::ConfigBuilder::default()
            .heartbeat_interval(Duration::from_secs(10))
            .validation_mode(ValidationMode::Strict)
            .message_id_fn(message_id_from_content)
            .max_transmit_size(1024 * 1024 * 2) // 2MB max message size
            .duplicate_cache_time(Duration::from_secs(60))
            .do_px() // Enable peer exchange
            .build()
            .map_err(|e| format!("Failed to build gossipsub config: {}", e))?;
        
        // Create gossipsub behavior
        let gossipsub = gossipsub::Behaviour::new(
            MessageAuthenticity::Signed(keypair),
            gossipsub_config,
        )?;
        
        Ok(Self {
            gossipsub,
            local_peer_id,
        })
    }
    
    /// Subscribe to all protocol topics
    pub fn subscribe_to_topics(&mut self) -> Result<(), GossipsubError> {
        // Subscribe to all topics
        let topics = [
            Topic::new(BLOCKS_TOPIC),
            Topic::new(TXS_TOPIC),
            Topic::new(HEADERS_TOPIC),
            Topic::new(STATUS_TOPIC),
        ];
        
        for topic in &topics {
            self.gossipsub.subscribe(topic)?;
        }
        
        Ok(())
    }
    
    /// Get the underlying gossipsub behavior
    pub fn gossipsub(&mut self) -> &mut gossipsub::Behaviour {
        &mut self.gossipsub
    }
    
    /// Broadcast node status to the network
    pub fn broadcast_status(&mut self, 
                           version: u32,
                           height: u64, 
                           best_hash: [u8; 32], 
                           total_difficulty: u64,
                           head_timestamp: u64) -> Result<MessageId, PublishError> {
        let message = Message::Status {
            version,
            height,
            best_hash,
            total_difficulty,
            head_timestamp,
        };
        
        self.publish_to_topic(STATUS_TOPIC, message)
    }
    
    /// Announce a new block to the network
    pub fn announce_block(&mut self, 
                         block: &Block, 
                         height: u64, 
                         total_difficulty: u64) -> Result<MessageId, PublishError> {
        let block_data = bincode::serialize(block)?;
        let message = Message::NewBlock {
            block_data,
            height,
            total_difficulty,
        };
        
        self.publish_to_topic(BLOCKS_TOPIC, message)
    }
    
    /// Announce a new transaction to the network
    pub fn announce_transaction(&mut self, tx: &Transaction, fee_rate: u64) -> Result<MessageId, PublishError> {
        let tx_hash = tx.hash();
        let message = Message::TransactionAnnouncement {
            tx_hash,
            fee_rate,
        };
        
        self.publish_to_topic(TXS_TOPIC, message)
    }
    
    /// Broadcast a full transaction to the network
    pub fn broadcast_transaction(&mut self, tx: Transaction) -> Result<MessageId, PublishError> {
        let message = Message::BroadcastTransaction(tx);
        self.publish_to_topic(TXS_TOPIC, message)
    }
    
    /// Request headers for a height range
    pub fn request_headers(&mut self, 
                          start_height: u64, 
                          end_height: u64) -> Result<MessageId, PublishError> {
        let message = Message::GetHeaders {
            start_height,
            end_height,
        };
        
        self.publish_to_topic(HEADERS_TOPIC, message)
    }
    
    /// Send headers in response to a request
    pub fn send_headers(&mut self, 
                       headers: Vec<BlockHeader>, 
                       total_difficulty: u64) -> Result<MessageId, PublishError> {
        let message = Message::Headers {
            headers,
            total_difficulty,
        };
        
        self.publish_to_topic(HEADERS_TOPIC, message)
    }
    
    /// Request blocks by hash
    pub fn request_blocks(&mut self, block_hashes: Vec<[u8; 32]>) -> Result<MessageId, PublishError> {
        let message = Message::GetBlocks { block_hashes };
        self.publish_to_topic(BLOCKS_TOPIC, message)
    }
    
    /// Request blocks by height range
    pub fn request_blocks_by_height(&mut self, 
                                   start_height: u64, 
                                   end_height: u64) -> Result<MessageId, PublishError> {
        let message = Message::GetBlocksByHeight {
            start_height,
            end_height,
        };
        
        self.publish_to_topic(BLOCKS_TOPIC, message)
    }
    
    /// Send blocks in response to a request
    pub fn send_blocks(&mut self, 
                      blocks: Vec<Block>, 
                      total_difficulty: u64) -> Result<MessageId, PublishError> {
        let message = Message::BlockResponse {
            blocks,
            total_difficulty,
        };
        
        self.publish_to_topic(BLOCKS_TOPIC, message)
    }
    
    /// Request mempool contents
    pub fn request_mempool(&mut self) -> Result<MessageId, PublishError> {
        let message = Message::GetMempool;
        self.publish_to_topic(TXS_TOPIC, message)
    }
    
    /// Send mempool contents in response to a request
    pub fn send_mempool(&mut self, transaction_hashes: Vec<[u8; 32]>) -> Result<MessageId, PublishError> {
        let message = Message::MempoolResponse { transaction_hashes };
        self.publish_to_topic(TXS_TOPIC, message)
    }
    
    /// Send a ping to measure latency
    pub fn send_ping(&mut self, timestamp: u64) -> Result<MessageId, PublishError> {
        let message = Message::Ping(timestamp);
        self.publish_to_topic(STATUS_TOPIC, message)
    }
    
    /// Respond to a ping
    pub fn send_pong(&mut self, timestamp: u64) -> Result<MessageId, PublishError> {
        let message = Message::Pong(timestamp);
        self.publish_to_topic(STATUS_TOPIC, message)
    }
    
    /// Request checkpoint information
    pub fn request_checkpoints(&mut self, since_timestamp: u64) -> Result<MessageId, PublishError> {
        let message = Message::GetCheckpoints { since_timestamp };
        self.publish_to_topic(STATUS_TOPIC, message)
    }
    
    /// Send checkpoint information
    pub fn send_checkpoints(&mut self, checkpoints: Vec<Checkpoint>) -> Result<MessageId, PublishError> {
        let message = Message::Checkpoints { checkpoints };
        self.publish_to_topic(STATUS_TOPIC, message)
    }
    
    /// Send a message directly to a specific peer
    pub fn send_to_peer(&mut self, 
                       peer_id: &PeerId, 
                       message: Message) -> Result<MessageId, PublishError> {
        // In gossipsub, we don't have direct peer-to-peer messaging
        // We'll publish to a topic and rely on peer filtering at a higher level
        
        // Determine the appropriate topic based on message type
        let topic = match &message {
            Message::NewBlock { .. } | Message::GetBlocks { .. } | 
            Message::GetBlocksByHeight { .. } | Message::BlockResponse { .. } => BLOCKS_TOPIC,
            
            Message::NewTransaction(..) | Message::BroadcastTransaction(..) | 
            Message::TransactionAnnouncement { .. } | Message::GetMempool | 
            Message::MempoolResponse { .. } => TXS_TOPIC,
            
            Message::GetHeaders { .. } | Message::Headers { .. } => HEADERS_TOPIC,
            
            _ => STATUS_TOPIC,
        };
        
        // Publish to the topic
        let encoded = bincode::serialize(&message)?;
        self.gossipsub.publish(Topic::new(topic), encoded)
    }
    
    /// Broadcast a message to all peers
    pub fn broadcast(&mut self, message: Message) -> Result<MessageId, PublishError> {
        // Determine the appropriate topic based on message type
        let topic = match &message {
            Message::NewBlock { .. } | Message::GetBlocks { .. } | 
            Message::GetBlocksByHeight { .. } | Message::BlockResponse { .. } => BLOCKS_TOPIC,
            
            Message::NewTransaction(..) | Message::BroadcastTransaction(..) | 
            Message::TransactionAnnouncement { .. } | Message::GetMempool | 
            Message::MempoolResponse { .. } => TXS_TOPIC,
            
            Message::GetHeaders { .. } | Message::Headers { .. } => HEADERS_TOPIC,
            
            _ => STATUS_TOPIC,
        };
        
        self.publish_to_topic(topic, message)
    }
    
    /// Helper method to publish a message to a specific topic
    fn publish_to_topic(&mut self, topic: &str, message: Message) -> Result<MessageId, PublishError> {
        let encoded = bincode::serialize(&message)?;
        self.gossipsub.publish(Topic::new(topic), encoded)
    }
}

/// Custom message ID generation function for gossipsub
fn message_id_from_content(message: &gossipsub::Message) -> gossipsub::MessageId {
    let mut hasher = Sha256::new();
    hasher.update(&message.data);
    // Include source peer to differentiate same message from different peers
    if let Some(source) = &message.source {
        hasher.update(source.to_bytes());
    }
    gossipsub::MessageId::from(hasher.finalize().to_vec())
}

/// Error type for publishing messages
#[derive(Debug, thiserror::Error)]
pub enum PublishError {
    #[error("Serialization error: {0}")]
    Serialization(#[from] bincode::Error),
    
    #[error("Gossipsub error: {0}")]
    Gossipsub(#[from] gossipsub::error::PublishError),
}

/// Re-export gossipsub message ID
pub type MessageId = gossipsub::MessageId;

/// Error type for gossipsub operations
pub type GossipsubError = gossipsub::error::GossipsubError;

#[cfg(test)]
mod tests {
    use super::*;
    use libp2p::identity;
    
    #[test]
    fn test_protocol_creation() {
        let keypair = identity::Keypair::generate_ed25519();
        let protocol = Protocol::new(keypair).unwrap();
        
        assert_eq!(protocol.gossipsub.topics().count(), 0);
    }
    
    #[test]
    fn test_topic_subscription() {
        let keypair = identity::Keypair::generate_ed25519();
        let mut protocol = Protocol::new(keypair).unwrap();
        
        protocol.subscribe_to_topics().unwrap();
        assert_eq!(protocol.gossipsub.topics().count(), 4); // 4 topics now
    }
    
    #[test]
    fn test_message_serialization() {
        // Test block announcement message
        let block_message = Message::NewBlock {
            block_data: vec![1, 2, 3, 4],
            height: 1,
            total_difficulty: 100,
        };
        
        let encoded = bincode::serialize(&block_message).unwrap();
        let decoded: Message = bincode::deserialize(&encoded).unwrap();
        
        match decoded {
            Message::NewBlock { block_data, height, total_difficulty } => {
                assert_eq!(block_data, vec![1, 2, 3, 4]);
                assert_eq!(height, 1);
                assert_eq!(total_difficulty, 100);
            }
            _ => panic!("Wrong message type after deserialization"),
        }
        
        // Test headers message
        let headers = vec![BlockHeader::new(1, [0u8; 32], [0u8; 32], 100)];
        let headers_message = Message::Headers {
            headers: headers.clone(),
            total_difficulty: 200,
        };
        
        let encoded = bincode::serialize(&headers_message).unwrap();
        let decoded: Message = bincode::deserialize(&encoded).unwrap();
        
        match decoded {
            Message::Headers { headers: decoded_headers, total_difficulty } => {
                assert_eq!(decoded_headers.len(), headers.len());
                assert_eq!(total_difficulty, 200);
            }
            _ => panic!("Wrong message type after deserialization"),
        }
    }
    
    #[test]
    fn test_message_publishing() {
        let keypair = identity::Keypair::generate_ed25519();
        let mut protocol = Protocol::new(keypair).unwrap();
        
        protocol.subscribe_to_topics().unwrap();
        
        // Test status broadcast
        let result = protocol.broadcast_status(1, 100, [0u8; 32], 1000, 12345);
        assert!(result.is_ok());
        
        // Test transaction announcement
        let transaction = Transaction::new(1, vec![], vec![], 0); // Empty test transaction
        let result = protocol.announce_transaction(&transaction, 2);
        assert!(result.is_ok());
    }
}