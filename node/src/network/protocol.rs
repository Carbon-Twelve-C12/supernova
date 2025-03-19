use libp2p::{
    gossipsub::{self, MessageId, Gossipsub, GossipsubEvent, GossipsubMessage, MessageAuthenticity, ValidationMode, GossipsubConfigBuilder, IdentTopic, error},
    identity::Keypair, identity::PublicKey,
    PeerId,
};
use serde::{Serialize, Deserialize};
use std::error::Error;
use sha2::{Sha256, Digest};
use std::time::{Duration, SystemTime};
use bincode;
use std::fmt;
use std::error::Error as StdError;
use tracing::debug;

// Topic constants
const BLOCKS_TOPIC: &str = "blocks";
const TXS_TOPIC: &str = "transactions";
const HEADERS_TOPIC: &str = "headers";
const STATUS_TOPIC: &str = "status";
const MEMPOOL_TOPIC: &str = "mempool";

/// Message types for node-to-node communication
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Message {
    /// New block announcement
    Block {
        block: Vec<u8>, // Serialized block data
    },
    
    /// New block announcement with metadata
    NewBlock {
        block_data: Vec<u8>, // Serialized block data
        height: u64,
        total_difficulty: u64,
    },
    
    /// New transaction announcement
    Transaction {
        transaction: Vec<u8>, // Serialized transaction data
    },
    
    /// Broadcast a transaction directly
    BroadcastTransaction(Vec<u8>), // Serialized transaction directly
    
    /// Transaction announcement with hash only
    TransactionAnnouncement {
        tx_hash: [u8; 32],
        fee_rate: u64,
    },
    
    /// Request for blocks in a range
    GetBlocks {
        start_height: u64,
        end_height: u64,
    },
    
    /// Request for blocks by hash
    GetBlocksByHash {
        block_hashes: Vec<[u8; 32]>,
    },
    
    /// Request blocks by height range 
    GetBlocksByHeight {
        start_height: u64,
        end_height: u64,
    },
    
    /// Response with block batch
    Blocks {
        blocks: Vec<Vec<u8>>, // List of serialized blocks
    },
    
    /// Response with block batch with metadata
    BlockResponse {
        blocks: Vec<Vec<u8>>, // List of serialized blocks
        total_difficulty: u64,
    },
    
    /// Request for block headers
    GetHeaders {
        start_height: u64,
        end_height: u64,
    },
    
    /// Response with block headers
    Headers {
        headers: Vec<Vec<u8>>, // List of serialized headers
        total_difficulty: u64,
    },
    
    /// Node status information
    Status {
        version: u32,
        height: u64,
        best_hash: [u8; 32],
        total_difficulty: u64,
        head_timestamp: u64,
    },
    
    /// Request status from a peer 
    GetStatus,
    
    /// Request for mempool transactions
    GetMempool {
        max_tx_count: u32,
    },
    
    /// Response with mempool transactions
    Mempool {
        transactions: Vec<Vec<u8>>, // List of serialized transactions
    },
    
    /// Request for peer information
    GetPeers,
    
    /// Response with connected peers
    Peers {
        peers: Vec<String>, // List of multiaddresses
    },
    
    /// Request specific data by hash
    GetData {
        block_hashes: Vec<[u8; 32]>,
        tx_hashes: Vec<[u8; 32]>,
    },
    
    /// Request for checkpoint information
    GetCheckpoints {
        start_height: u64,
        end_height: u64,
    },
    
    /// Checkpoint information response
    Checkpoints {
        checkpoints: Vec<Checkpoint>,
    },
    
    /// Ping with timestamp for latency measurement
    Ping(u64),
    
    /// Pong response with original timestamp
    Pong(u64),
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
    gossipsub: gossipsub::Gossipsub,
    local_peer_id: PeerId,
}

impl Protocol {
    /// Create a new protocol instance
    pub fn new(keypair: Keypair) -> Result<Self, Box<dyn Error>> {
        let local_peer_id = PeerId::from(keypair.public());
        
        // Configure gossipsub with appropriate parameters
        let gossipsub_config = gossipsub::GossipsubConfigBuilder::default()
            .heartbeat_interval(Duration::from_secs(10))
            .validation_mode(gossipsub::ValidationMode::Strict)
            .message_id_fn(message_id_from_content)
            .build()
            .map_err(|e| format!("Failed to build gossipsub config: {}", e))?;
        
        // Create gossipsub behavior
        let gossipsub = gossipsub::Gossipsub::new(
            gossipsub::MessageAuthenticity::Signed(keypair),
            gossipsub_config,
        )?;
        
        Ok(Self {
            gossipsub,
            local_peer_id,
        })
    }
    
    /// Subscribe to all protocol topics
    pub fn subscribe_to_topics(&mut self) -> Result<(), GossipsubError> {
        // Define the topics we want to subscribe to
        let topics = [
            IdentTopic::new(BLOCKS_TOPIC),
            IdentTopic::new(TXS_TOPIC),
            IdentTopic::new(STATUS_TOPIC),
            IdentTopic::new(HEADERS_TOPIC),
            IdentTopic::new(MEMPOOL_TOPIC),
        ];
        
        // Subscribe to each topic with proper error conversion
        for topic in &topics {
            self.gossipsub.subscribe(topic).map_err(|e| GossipsubError::from(e))?;
        }
        
        Ok(())
    }
    
    /// Access to the underlying gossipsub behavior
    pub fn gossipsub(&mut self) -> &mut gossipsub::Gossipsub {
        &mut self.gossipsub
    }
    
    /// Create and publish a message
    pub fn publish_message(&mut self, topic: &str, message: Message) -> Result<MessageId, PublishError> {
        // Serialize message to binary
        let encoded = bincode::serialize(&message)?;
        
        // Publish to gossipsub network
        let message_id = self.publish_to_topic(topic, message)?;
        Ok(message_id)
    }
    
    /// Helper method to publish block announcements
    pub fn announce_block(&mut self, block: &[u8], height: u64, total_difficulty: u64) -> Result<MessageId, PublishError> {
        let message = Message::NewBlock {
            block_data: block.to_vec(),
            height,
            total_difficulty,
        };
        self.publish_message(BLOCKS_TOPIC, message)
    }
    
    /// Helper method to announce new transactions
    pub fn announce_transaction(&mut self, tx_data: &[u8], fee_rate: u64) -> Result<MessageId, PublishError> {
        // First try using the transaction itself
        let message = Message::Transaction {
            transaction: tx_data.to_vec(),
        };
        self.publish_message(TXS_TOPIC, message)
    }
    
    /// Helper method to publish status updates
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
        self.publish_message(STATUS_TOPIC, message)
    }
    
    /// Send a message to a specific peer
    pub fn send_to_peer(&mut self, peer_id: &PeerId, message: Message) -> Result<MessageId, PublishError> {
        // For now, we'll just publish to the regular topic and let the peer pick it up
        // This is less efficient but works with the current gossipsub implementation
        let topic_name = message_to_topic(&message);
        
        // Log that we're targeting a specific peer, though we're using broadcast
        debug!("Sending message to peer {} via topic {}", peer_id, topic_name);
        
        self.publish_message(topic_name, message)
    }
    
    /// Request blocks by hash
    pub fn request_blocks(&mut self, block_hashes: Vec<[u8; 32]>) -> Result<MessageId, PublishError> {
        let message = Message::GetBlocksByHash { block_hashes };
        self.publish_message(BLOCKS_TOPIC, message)
    }
    
    /// Request headers in a range
    pub fn request_headers(&mut self, start_height: u64, end_height: u64) -> Result<MessageId, PublishError> {
        let message = Message::GetHeaders {
            start_height,
            end_height,
        };
        self.publish_message(HEADERS_TOPIC, message)
    }
    
    /// Request blocks by height range
    pub fn request_blocks_by_height(&mut self, start_height: u64, end_height: u64) -> Result<MessageId, PublishError> {
        let message = Message::GetBlocksByHeight {
            start_height,
            end_height,
        };
        self.publish_message(BLOCKS_TOPIC, message)
    }
    
    /// Broadcast a message to all subscribers of a topic
    pub fn broadcast(&mut self, message: Message) -> Result<MessageId, PublishError> {
        let topic_name = message_to_topic(&message);
        self.publish_message(topic_name, message)
    }
    
    /// Generic method to publish to a topic
    fn publish_to_topic(&mut self, topic: &str, message: Message) -> Result<MessageId, PublishError> {
        let encoded = bincode::serialize(&message)?;
        let topic = IdentTopic::new(topic);
        match self.gossipsub.publish(topic, encoded) {
            Ok(id) => Ok(id),
            Err(err) => Err(PublishError::Gossipsub(err.to_string())),
        }
    }
}

/// Format message_id from message content using a hash
pub fn message_id_from_content(message: &gossipsub::GossipsubMessage) -> gossipsub::MessageId {
    let mut hasher = Sha256::new();
    hasher.update(&message.data);
    if let Some(source) = &message.source {
        hasher.update(source.to_bytes());
    }
    
    let hash = hasher.finalize();
    let hash_bytes: &[u8] = hash.as_ref();
    gossipsub::MessageId::from(hash_bytes)
}

/// Error type for gossipsub publishing
#[derive(Debug, thiserror::Error)]
pub enum PublishError {
    #[error("Serialization error: {0}")]
    Serialization(#[from] bincode::Error),
    
    #[error("Gossipsub error: {0}")]
    Gossipsub(String),
}

/// A proper GossipsubError implementation
#[derive(Debug)]
pub struct GossipsubError {
    message: String,
}

impl GossipsubError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for GossipsubError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Gossipsub error: {}", self.message)
    }
}

impl StdError for GossipsubError {}

impl From<gossipsub::error::PublishError> for GossipsubError {
    fn from(err: gossipsub::error::PublishError) -> Self {
        GossipsubError::new(err.to_string())
    }
}

impl From<gossipsub::error::SubscriptionError> for GossipsubError {
    fn from(err: gossipsub::error::SubscriptionError) -> Self {
        GossipsubError::new(err.to_string())
    }
}

/// Helper function to determine the appropriate topic for a message
fn message_to_topic(message: &Message) -> &'static str {
    match message {
        Message::Block { .. } | 
        Message::NewBlock { .. } | 
        Message::GetBlocks { .. } |
        Message::GetBlocksByHash { .. } |
        Message::GetBlocksByHeight { .. } |
        Message::Blocks { .. } |
        Message::BlockResponse { .. } => BLOCKS_TOPIC,
        
        Message::Transaction { .. } |
        Message::BroadcastTransaction(_) |
        Message::TransactionAnnouncement { .. } => TXS_TOPIC,
        
        Message::GetHeaders { .. } |
        Message::Headers { .. } => HEADERS_TOPIC,
        
        Message::Status { .. } |
        Message::GetStatus => STATUS_TOPIC,
        
        Message::GetMempool { .. } |
        Message::Mempool { .. } => MEMPOOL_TOPIC,
        
        // Default for other messages
        _ => STATUS_TOPIC,
    }
}

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
        let block_message = Message::Block {
            block: vec![1, 2, 3, 4],
        };
        
        let encoded = bincode::serialize(&block_message).unwrap();
        let decoded: Message = bincode::deserialize(&encoded).unwrap();
        
        match decoded {
            Message::Block { block } => {
                assert_eq!(block, vec![1, 2, 3, 4]);
            }
            _ => panic!("Wrong message type after deserialization"),
        }
        
        // Test headers message
        let headers = vec![vec![0u8; 32], vec![1u8; 32]];
        let headers_message = Message::Headers {
            headers,
            total_difficulty: 100,
        };
        
        let encoded = bincode::serialize(&headers_message).unwrap();
        let decoded: Message = bincode::deserialize(&encoded).unwrap();
        
        match decoded {
            Message::Headers { headers, total_difficulty } => {
                assert_eq!(headers.len(), 2);
                assert_eq!(total_difficulty, 100);
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
        let result = protocol.broadcast_status(1, 100, [0u8; 32], 0, SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs());
        assert!(result.is_ok());
        
        // Test transaction announcement
        let transaction = vec![1, 2, 3, 4]; // Empty test transaction
        let result = protocol.announce_transaction(&transaction, 0);
        assert!(result.is_ok());
    }
}