/// Network module for SuperNova
///
/// This module provides networking capabilities for node-to-node communication.

use serde::{Serialize, Deserialize};
use thiserror::Error;
use std::net::{IpAddr, SocketAddr};

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

// Re-export networking components that will be implemented later
// pub mod peer;
// pub mod message;
// pub mod protocol;
// pub use peer::Peer;
// pub use message::Message;
// pub use protocol::NetworkProtocol; 