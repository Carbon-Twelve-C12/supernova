use libp2p::{PeerId, Multiaddr};
use std::time::Instant;

/// Information about a connected peer
#[derive(Debug, Clone)]
pub struct PeerInfo {
    /// Peer ID
    pub peer_id: PeerId,
    /// Peer addresses
    pub addresses: Vec<Multiaddr>,
    /// Supported protocols
    pub protocols: Vec<String>,
    /// Agent version
    pub agent_version: Option<String>,
    /// Connection time
    pub connected_at: Instant,
    /// Last seen time
    pub last_seen: Instant,
    /// Bytes sent to this peer
    pub bytes_sent: u64,
    /// Bytes received from this peer
    pub bytes_received: u64,
    /// Latency in milliseconds
    pub latency_ms: Option<u32>,
}