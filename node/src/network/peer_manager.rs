use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::sync::RwLock;
use libp2p::PeerId;
use tracing::{info, warn, debug};
use crate::{
    network::peer::{PeerInfo, PeerState},
    storage::Storage,
};

/// Connection limits for peer management
#[derive(Debug, Clone)]
pub struct ConnectionLimits {
    pub max_peers: usize,
    pub max_inbound: usize,
    pub max_outbound: usize,
    pub reserved_slots: usize,
}

impl Default for ConnectionLimits {
    fn default() -> Self {
        Self {
            max_peers: 125,
            max_inbound: 80,
            max_outbound: 45,
            reserved_slots: 25,
        }
    }
}

/// Peer manager for handling peer connections and scoring
pub struct PeerManager {
    /// All known peers
    peers: Arc<RwLock<HashMap<PeerId, PeerInfo>>>,
    /// Trusted peers that get reserved slots
    trusted_peers: Arc<RwLock<HashSet<PeerId>>>,
    /// Peer reputation scores
    peer_scores: Arc<RwLock<HashMap<PeerId, f64>>>,
    /// Connection limits
    connection_limits: ConnectionLimits,
    /// Storage backend for persistence
    storage: Arc<dyn Storage>,
}

impl PeerManager {
    /// Create a new peer manager
    pub fn new(storage: Arc<dyn Storage>, limits: ConnectionLimits) -> Self {
        Self {
            peers: Arc::new(RwLock::new(HashMap::new())),
            trusted_peers: Arc::new(RwLock::new(HashSet::new())),
            peer_scores: Arc::new(RwLock::new(HashMap::new())),
            connection_limits: limits,
            storage,
        }
    }
    
    /// Add a new peer
    pub async fn add_peer(&self, peer_id: PeerId, info: PeerInfo) -> Result<(), Box<dyn std::error::Error>> {
        let mut peers = self.peers.write().await;
        
        // Check connection limits
        let current_count = peers.len();
        let trusted_count = self.trusted_peers.read().await.len();
        let available_slots = self.connection_limits.max_peers - self.connection_limits.reserved_slots;
        
        if current_count >= available_slots + trusted_count {
            return Err("Connection limit reached".into());
        }
        
        // Add peer
        peers.insert(peer_id, info.clone());
        self.peer_scores.write().await.insert(peer_id, 0.0);
        
        // Persist to storage
        self.storage.save_peer_info(&peer_id, &info).await?;
        
        info!("Added peer: {}", peer_id);
        Ok(())
    }
    
    /// Remove a peer
    pub async fn remove_peer(&self, peer_id: &PeerId) -> Result<(), Box<dyn std::error::Error>> {
        let removed = self.peers.write().await.remove(peer_id);
        self.peer_scores.write().await.remove(peer_id);
        
        if removed.is_some() {
            self.storage.remove_peer_info(peer_id).await?;
            info!("Removed peer: {}", peer_id);
        }
        
        Ok(())
    }
    
    /// Update peer score
    pub async fn update_peer_score(&self, peer_id: &PeerId, delta: f64) -> Result<(), Box<dyn std::error::Error>> {
        let mut scores = self.peer_scores.write().await;
        let score = scores.entry(*peer_id).or_insert(0.0);
        *score += delta;
        
        // Clamp score between -100 and 100
        *score = score.clamp(-100.0, 100.0);
        
        debug!("Updated peer {} score by {} to {}", peer_id, delta, score);
        Ok(())
    }
    
    /// Get the best peers by score
    pub async fn get_best_peers(&self, count: usize) -> Vec<PeerId> {
        let scores = self.peer_scores.read().await;
        let mut peer_scores: Vec<(PeerId, f64)> = scores.iter()
            .map(|(id, score)| (*id, *score))
            .collect();
        
        peer_scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        peer_scores.into_iter()
            .take(count)
            .map(|(id, _)| id)
            .collect()
    }
    
    /// Check if a peer is trusted
    pub async fn is_trusted(&self, peer_id: &PeerId) -> bool {
        self.trusted_peers.read().await.contains(peer_id)
    }
    
    /// Add a trusted peer
    pub async fn add_trusted_peer(&self, peer_id: PeerId) -> Result<(), Box<dyn std::error::Error>> {
        self.trusted_peers.write().await.insert(peer_id);
        self.storage.save_trusted_peer(&peer_id).await?;
        info!("Added trusted peer: {}", peer_id);
        Ok(())
    }
    
    /// Remove a trusted peer
    pub async fn remove_trusted_peer(&self, peer_id: &PeerId) -> Result<(), Box<dyn std::error::Error>> {
        self.trusted_peers.write().await.remove(peer_id);
        self.storage.remove_trusted_peer(peer_id).await?;
        info!("Removed trusted peer: {}", peer_id);
        Ok(())
    }
    
    /// Get all connected peers
    pub async fn get_connected_peers(&self) -> Vec<PeerInfo> {
        self.peers.read().await.values().cloned().collect()
    }
    
    /// Get peer info
    pub async fn get_peer_info(&self, peer_id: &PeerId) -> Option<PeerInfo> {
        self.peers.read().await.get(peer_id).cloned()
    }
    
    /// Update peer info
    pub async fn update_peer_info(&self, peer_id: &PeerId, update_fn: impl FnOnce(&mut PeerInfo)) -> Result<(), Box<dyn std::error::Error>> {
        let mut peers = self.peers.write().await;
        if let Some(info) = peers.get_mut(peer_id) {
            update_fn(info);
            self.storage.save_peer_info(peer_id, info).await?;
        }
        Ok(())
    }
    
    /// Persist all peer data
    pub async fn persist_peers(&self) -> Result<(), Box<dyn std::error::Error>> {
        let peers = self.peers.read().await;
        for (peer_id, info) in peers.iter() {
            self.storage.save_peer_info(peer_id, info).await?;
        }
        
        let trusted = self.trusted_peers.read().await;
        for peer_id in trusted.iter() {
            self.storage.save_trusted_peer(peer_id).await?;
        }
        
        info!("Persisted {} peers to storage", peers.len());
        Ok(())
    }
    
    /// Load peers from storage
    pub async fn load_from_storage(&self) -> Result<(), Box<dyn std::error::Error>> {
        // Load peer info
        let stored_peers = self.storage.list_all_peers().await?;
        let mut peers = self.peers.write().await;
        let mut scores = self.peer_scores.write().await;
        
        for (peer_id, info) in stored_peers {
            peers.insert(peer_id, info);
            scores.insert(peer_id, 0.0);
        }
        
        // Load trusted peers
        let trusted_list = self.storage.list_trusted_peers().await?;
        let mut trusted = self.trusted_peers.write().await;
        for peer_id in trusted_list {
            trusted.insert(peer_id);
        }
        
        info!("Loaded {} peers from storage", peers.len());
        Ok(())
    }
    
    /// Get connection counts
    pub async fn get_connection_counts(&self) -> (usize, usize, usize) {
        let peers = self.peers.read().await;
        let total = peers.len();
        let inbound = peers.values().filter(|p| p.is_inbound).count();
        let outbound = total - inbound;
        (total, inbound, outbound)
    }
    
    /// Check if we can accept more connections
    pub async fn can_accept_connection(&self, is_inbound: bool) -> bool {
        let (total, inbound, outbound) = self.get_connection_counts().await;
        
        if total >= self.connection_limits.max_peers {
            return false;
        }
        
        if is_inbound && inbound >= self.connection_limits.max_inbound {
            return false;
        }
        
        if !is_inbound && outbound >= self.connection_limits.max_outbound {
            return false;
        }
        
        true
    }
    
    /// Get peers that should be disconnected (lowest scores)
    pub async fn get_peers_to_disconnect(&self, count: usize) -> Vec<PeerId> {
        let scores = self.peer_scores.read().await;
        let trusted = self.trusted_peers.read().await;
        
        let mut peer_scores: Vec<(PeerId, f64)> = scores.iter()
            .filter(|(id, _)| !trusted.contains(id))
            .map(|(id, score)| (*id, *score))
            .collect();
        
        peer_scores.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
        peer_scores.into_iter()
            .take(count)
            .map(|(id, _)| id)
            .collect()
    }
} 