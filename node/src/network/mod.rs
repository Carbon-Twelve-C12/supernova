pub mod connection;
pub mod message;
pub mod peer;
pub mod protocol;
pub mod sync;
pub mod peer_diversity;
pub mod p2p;
pub mod advanced;
pub mod discovery;

use connection::Connection;
use message::Message;
use peer::Peer;
use protocol::Protocol;
use sync::ChainSync;
use peer_diversity::PeerDiversityManager;
use p2p::P2PNetwork;
use discovery::PeerDiscovery;

use std::sync::Arc;
use tokio::sync::mpsc;
use libp2p::core::PeerId;
use tracing::{debug, info, warn, error};

// Re-export network types for external use
pub use connection::ConnectionState;
pub use message::NetworkMessage;
pub use peer::{PeerState, PeerInfo, PeerMetadata};
pub use protocol::{ProtocolError, Message as ProtocolMessage};
pub use p2p::{NetworkCommand, NetworkEvent, NetworkStats};
pub use discovery::DiscoveryEvent;

/// Maximum number of peers to connect to
pub const MAX_PEERS: usize = 50;

/// Maximum number of inbound connections allowed
pub const MAX_INBOUND_CONNECTIONS: usize = 128;

/// Maximum number of outbound connections to maintain
pub const MAX_OUTBOUND_CONNECTIONS: usize = 24;

/// Target number of outbound connections for diversity
pub const TARGET_OUTBOUND_CONNECTIONS: usize = 8;

/// Connection timeout in seconds
pub const CONNECTION_TIMEOUT_SECS: u64 = 30;

/// Initialize the network stack with proper configuration
pub async fn initialize_network(
    config: &crate::config::NetworkConfig,
    genesis_hash: [u8; 32],
) -> Result<(P2PNetwork, mpsc::Sender<NetworkCommand>, mpsc::Receiver<NetworkEvent>), Box<dyn std::error::Error>> {
    info!("Initializing P2P network stack");
    
    // Create keypair from config or generate a new one
    let keypair = if let Some(key_path) = &config.key_path {
        // Load keypair from file
        let key_bytes = std::fs::read(key_path)
            .map_err(|e| format!("Failed to read key file: {}", e))?;
        libp2p::identity::Keypair::from_protobuf_encoding(&key_bytes)
            .map_err(|e| format!("Invalid key format: {}", e))?
    } else {
        // Generate a new keypair
        libp2p::identity::Keypair::generate_ed25519()
    };
    
    // Initialize P2P network with the keypair
    let (network, command_tx, event_rx) = P2PNetwork::new(
        Some(keypair),
        genesis_hash,
        &config.network_id,
    ).await?;
    
    info!("P2P network initialized with peer ID: {}", network.local_peer_id());
    
    Ok((network, command_tx, event_rx))
}

// Implementation continues...