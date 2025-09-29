/// Network module for supernova
///
/// This module provides networking capabilities for node-to-node communication.
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub mod quantum_p2p;

/// Network error types
#[derive(Debug, Error)]
pub enum NetworkError {
    #[error("Connection error: {0}")]
    ConnectionError(String),

    #[error("Peer error: {0}")]
    PeerError(String),

    #[error("Message error: {0}")]
    MessageError(String),

    #[error("Protocol error: {0}")]
    ProtocolError(String),

    #[error("IO error: {0}")]
    IoError(String),
}

/// Network configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkConfig {
    /// Maximum number of connections
    pub max_connections: usize,

    /// Maximum number of outbound connections
    pub max_outbound_connections: usize,

    /// Maximum number of inbound connections
    pub max_inbound_connections: usize,

    /// Connection timeout in seconds
    pub connection_timeout_secs: u64,

    /// Peers to connect to on startup
    pub bootstrap_peers: Vec<String>,
}

impl Default for NetworkConfig {
    fn default() -> Self {
        Self {
            max_connections: 125,
            max_outbound_connections: 8,
            max_inbound_connections: 117,
            connection_timeout_secs: 60,
            bootstrap_peers: vec![],
        }
    }
}

// Network protocol type stubs
pub mod protocol {
    // Stub for BlockHeader used in protocol
    #[derive(Debug, Clone)]
    pub struct BlockHeader {
        pub version: u32,
        pub prev_block_hash: [u8; 32],
        pub merkle_root: [u8; 32],
        pub timestamp: u64,
        pub bits: u32,
        pub nonce: u32,
    }
}

// P2P module stubs
pub mod p2p {
    // Stub for Block used in p2p module
    #[derive(Debug, Clone)]
    pub struct Block {
        // Empty implementation for now
    }

    impl Block {
        pub fn hash(&self) -> [u8; 32] {
            [0; 32] // Placeholder implementation
        }
    }
}

// Re-export node protocol stubs
pub use self::p2p::Block;
pub use self::protocol::BlockHeader;

// Re-export quantum P2P types
pub use quantum_p2p::{
    P2PError, QuantumHandshake, QuantumMessage, QuantumP2PConfig, QuantumPeerInfo,
    QuantumProtocolHandler,
};

// Re-export networking components that will be implemented later
// pub mod peer;
// pub mod message;
// pub mod protocol;
// pub use peer::Peer;
// pub use message::Message;
// pub use protocol::NetworkProtocol;
