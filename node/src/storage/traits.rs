use crate::network::peer::PeerInfo;
use libp2p::PeerId;
use std::error::Error;

/// Storage trait for persisting network and blockchain data
#[async_trait::async_trait]
pub trait Storage: Send + Sync {
    /// Save peer information
    async fn save_peer_info(&self, peer_id: &PeerId, info: &PeerInfo)
        -> Result<(), Box<dyn Error>>;

    /// Load peer information
    async fn load_peer_info(&self, peer_id: &PeerId) -> Result<Option<PeerInfo>, Box<dyn Error>>;

    /// Remove peer information
    async fn remove_peer_info(&self, peer_id: &PeerId) -> Result<(), Box<dyn Error>>;

    /// List all stored peers
    async fn list_all_peers(&self) -> Result<Vec<(PeerId, PeerInfo)>, Box<dyn Error>>;

    /// Save a trusted peer
    async fn save_trusted_peer(&self, peer_id: &PeerId) -> Result<(), Box<dyn Error>>;

    /// Remove a trusted peer
    async fn remove_trusted_peer(&self, peer_id: &PeerId) -> Result<(), Box<dyn Error>>;

    /// List all trusted peers
    async fn list_trusted_peers(&self) -> Result<Vec<PeerId>, Box<dyn Error>>;

    /// Save banned peer information
    async fn save_banned_peer(
        &self,
        peer_id: &PeerId,
        reason: &str,
        until: std::time::Instant,
    ) -> Result<(), Box<dyn Error>>;

    /// Check if a peer is banned
    async fn is_peer_banned(&self, peer_id: &PeerId) -> Result<bool, Box<dyn Error>>;

    /// Remove expired bans
    async fn cleanup_expired_bans(&self) -> Result<(), Box<dyn Error>>;
}
