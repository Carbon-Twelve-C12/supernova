use libp2p::gossipsub::{
    self, IdentTopic as Topic, MessageAuthenticity, ValidationMode,
};
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::fmt;

const BLOCKS_TOPIC: &str = "supernova/blocks/1.0.0";
const TXS_TOPIC: &str = "supernova/transactions/1.0.0";

/// Protocol message types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Message {
    /// Handshake message
    Handshake(HandshakeData),
    /// Transaction announcement
    Transaction(TransactionAnnouncement),
    /// Block announcement
    Block(BlockAnnouncement),
    /// Peer discovery message
    PeerDiscovery(PeerDiscoveryMessage),
    /// Challenge message for identity verification
    Challenge(ChallengeMessage),
    /// Ping message
    Ping(u64),
    /// Pong response
    Pong(u64),
}

impl fmt::Display for Message {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Message::Handshake(_) => write!(f, "Handshake"),
            Message::Transaction(_) => write!(f, "Transaction"),
            Message::Block(_) => write!(f, "Block"),
            Message::PeerDiscovery(_) => write!(f, "PeerDiscovery"),
            Message::Challenge(_) => write!(f, "Challenge"),
            Message::Ping(_) => write!(f, "Ping"),
            Message::Pong(_) => write!(f, "Pong"),
        }
    }
}

/// Handshake data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HandshakeData {
    /// Protocol version
    pub version: u32,
    /// Client name and version
    pub user_agent: String,
    /// Supported features bitflag
    pub features: u64,
    /// Current height of the node
    pub height: u64,
}

/// Transaction announcement
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionAnnouncement {
    /// Transaction hash
    pub hash: [u8; 32],
    /// Transaction size in bytes
    pub size: u32,
    /// Fee rate in satoshis per byte
    pub fee_rate: u64,
}

/// Block announcement
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockAnnouncement {
    /// Block hash
    pub hash: [u8; 32],
    /// Block height
    pub height: u64,
    /// Block size in bytes
    pub size: u32,
}

/// Peer discovery message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PeerDiscoveryMessage {
    /// Request for peers
    GetPeers,
    /// Response with peer addresses
    Peers(Vec<PeerInfo>),
}

/// Peer information for discovery
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerInfo {
    /// Address in host:port format
    pub address: String,
    /// Last seen time (seconds since Unix epoch)
    pub last_seen: u64,
    /// Node features bitflag
    pub features: u64,
}

/// Challenge message for identity verification to prevent Sybil attacks
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ChallengeMessage {
    /// Challenge request sent to peer
    Request { 
        /// Random challenge string
        challenge: String,
        /// Difficulty required (leading zeros)
        difficulty: u8,
        /// Timestamp when challenge was issued
        timestamp: u64,
    },
    /// Challenge response from peer
    Response {
        /// Original challenge string
        challenge: String,
        /// Solution to the challenge
        solution: String,
        /// Timestamp when solution was generated
        timestamp: u64,
        /// Additional proof of work data if needed
        nonce: u64,
    },
    /// Challenge verification result
    Result {
        /// Whether challenge was verified successfully
        success: bool,
        /// Error message if verification failed
        error: Option<String>,
    },
}

/// Protocol features bitflag definitions
pub mod features {
    /// Supports compact blocks
    pub const COMPACT_BLOCKS: u64 = 0x01;
    /// Supports Segregated Witness
    pub const SEGREGATED_WITNESS: u64 = 0x02;
    /// Supports transaction bloom filtering
    pub const BLOOM_FILTER: u64 = 0x04;
    /// Supports peer address filtering
    pub const ADDRESS_FILTER: u64 = 0x08;
    /// Supports challenge-response verification
    pub const CHALLENGE_RESPONSE: u64 = 0x10;
    /// Supports node network state queries
    pub const NETWORK_STATE: u64 = 0x20;
    /// Supports advanced peer scoring
    pub const PEER_SCORING: u64 = 0x40;
    /// Supports transaction fee estimation
    pub const FEE_ESTIMATION: u64 = 0x80;
}

/// Protocol version
pub const PROTOCOL_VERSION: u32 = 1;

pub struct Protocol {
    gossipsub: gossipsub::Behaviour,
}

impl Protocol {
    pub fn new() -> Result<Self, Box<dyn Error>> {
        let gossipsub_config = gossipsub::ConfigBuilder::default()
            .heartbeat_interval(Duration::from_secs(1))
            .validation_mode(ValidationMode::Strict)
            .build()
            .map_err(|e| format!("Failed to build gossipsub config: {}", e))?;

        let gossipsub = gossipsub::Behaviour::new(
            MessageAuthenticity::Signed(id_keys),
            gossipsub_config,
        )?;

        Ok(Self { gossipsub })
    }

    pub fn subscribe_to_topics(&mut self) {
        let blocks_topic = Topic::new(BLOCKS_TOPIC);
        let txs_topic = Topic::new(TXS_TOPIC);
        
        self.gossipsub.subscribe(&blocks_topic)?;
        self.gossipsub.subscribe(&txs_topic)?;
    }

    pub fn publish_block(&mut self, block_data: Vec<u8>) -> Result<MessageId, PublishError> {
        let message = Message::Block(BlockAnnouncement {
            hash: [0; 32],
            height: 0,
            size: 0,
        });
        let encoded = bincode::serialize(&message)?;
        self.gossipsub.publish(Topic::new(BLOCKS_TOPIC), encoded)
    }

    pub fn publish_transaction(&mut self, tx_data: Vec<u8>) -> Result<MessageId, PublishError> {
        let message = Message::Transaction(TransactionAnnouncement {
            hash: [0; 32],
            size: 0,
            fee_rate: 0,
        });
        let encoded = bincode::serialize(&message)?;
        self.gossipsub.publish(Topic::new(TXS_TOPIC), encoded)
    }
}