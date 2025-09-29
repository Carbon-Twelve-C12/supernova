use crate::network::peer::PeerInfo;
use libp2p::PeerId;
use std::{collections::HashMap, error::Error, sync::Arc};
use tokio::sync::RwLock;

/// Simple in-memory storage implementation
pub struct MemoryStorage {
    peers: Arc<RwLock<HashMap<PeerId, PeerInfo>>>,
    trusted_peers: Arc<RwLock<Vec<PeerId>>>,
    banned_peers: Arc<RwLock<HashMap<PeerId, (String, std::time::Instant)>>>,
}

impl Default for MemoryStorage {
    fn default() -> Self {
        Self::new()
    }
}

impl MemoryStorage {
    pub fn new() -> Self {
        Self {
            peers: Arc::new(RwLock::new(HashMap::new())),
            trusted_peers: Arc::new(RwLock::new(Vec::new())),
            banned_peers: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

#[async_trait::async_trait]
impl crate::storage::Storage for MemoryStorage {
    async fn save_peer_info(
        &self,
        peer_id: &PeerId,
        info: &PeerInfo,
    ) -> Result<(), Box<dyn Error>> {
        self.peers.write().await.insert(*peer_id, info.clone());
        Ok(())
    }

    async fn load_peer_info(&self, peer_id: &PeerId) -> Result<Option<PeerInfo>, Box<dyn Error>> {
        Ok(self.peers.read().await.get(peer_id).cloned())
    }

    async fn remove_peer_info(&self, peer_id: &PeerId) -> Result<(), Box<dyn Error>> {
        self.peers.write().await.remove(peer_id);
        Ok(())
    }

    async fn list_all_peers(&self) -> Result<Vec<(PeerId, PeerInfo)>, Box<dyn Error>> {
        Ok(self
            .peers
            .read()
            .await
            .iter()
            .map(|(k, v)| (*k, v.clone()))
            .collect())
    }

    async fn save_trusted_peer(&self, peer_id: &PeerId) -> Result<(), Box<dyn Error>> {
        let mut trusted = self.trusted_peers.write().await;
        if !trusted.contains(peer_id) {
            trusted.push(*peer_id);
        }
        Ok(())
    }

    async fn remove_trusted_peer(&self, peer_id: &PeerId) -> Result<(), Box<dyn Error>> {
        self.trusted_peers.write().await.retain(|p| p != peer_id);
        Ok(())
    }

    async fn list_trusted_peers(&self) -> Result<Vec<PeerId>, Box<dyn Error>> {
        Ok(self.trusted_peers.read().await.clone())
    }

    async fn save_banned_peer(
        &self,
        peer_id: &PeerId,
        reason: &str,
        until: std::time::Instant,
    ) -> Result<(), Box<dyn Error>> {
        self.banned_peers
            .write()
            .await
            .insert(*peer_id, (reason.to_string(), until));
        Ok(())
    }

    async fn is_peer_banned(&self, peer_id: &PeerId) -> Result<bool, Box<dyn Error>> {
        if let Some((_, until)) = self.banned_peers.read().await.get(peer_id) {
            Ok(std::time::Instant::now() < *until)
        } else {
            Ok(false)
        }
    }

    async fn cleanup_expired_bans(&self) -> Result<(), Box<dyn Error>> {
        let now = std::time::Instant::now();
        self.banned_peers
            .write()
            .await
            .retain(|_, (_, until)| now < *until);
        Ok(())
    }
}
