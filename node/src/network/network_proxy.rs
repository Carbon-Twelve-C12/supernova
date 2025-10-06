//! Thread-safe network proxy for API access
//!
//! This module provides a thread-safe wrapper around P2PNetwork that can be safely
//! shared across threads in the API server, avoiding libp2p's non-Sync types.

use libp2p::PeerId;
use std::error::Error;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, oneshot, RwLock};

use super::{NetworkCommand, P2PNetwork, P2PNetworkStats as NetworkStats};
use crate::api::types::{BandwidthUsage, ConnectionCount, NetworkInfo, PeerAddResponse, PeerInfo};

/// Thread-safe network proxy that can be shared across threads
#[derive(Clone)]
pub struct NetworkProxy {
    /// Command sender for network operations
    command_tx: mpsc::Sender<NetworkCommand>,
    /// Channel for making async requests to the network thread
    request_tx: mpsc::Sender<ProxyRequest>,
    /// Cached network stats (updated periodically)
    cached_stats: Arc<RwLock<NetworkStats>>,
    /// Local peer ID
    local_peer_id: PeerId,
    /// Network ID
    network_id: String,
}

/// Internal request types for proxy operations
#[derive(Debug)]
pub enum ProxyRequest {
    GetNetworkInfo(oneshot::Sender<Result<NetworkInfo, Box<dyn Error + Send>>>),
    GetConnectionCount(oneshot::Sender<Result<ConnectionCount, Box<dyn Error + Send>>>),
    GetPeers(oneshot::Sender<Result<Vec<PeerInfo>, Box<dyn Error + Send>>>),
    GetPeer(
        String,
        oneshot::Sender<Result<Option<PeerInfo>, Box<dyn Error + Send>>>,
    ),
    AddPeer(
        String,
        bool,
        oneshot::Sender<Result<PeerAddResponse, Box<dyn Error + Send>>>,
    ),
    RemovePeer(String, oneshot::Sender<Result<bool, Box<dyn Error + Send>>>),
    GetBandwidthUsage(
        u64,
        oneshot::Sender<Result<BandwidthUsage, Box<dyn Error + Send>>>,
    ),
    GetStats(oneshot::Sender<NetworkStats>),
    PeerCount(oneshot::Sender<usize>),
    IsSyncing(oneshot::Sender<bool>),
    UpdateStats(NetworkStats),
}

impl NetworkProxy {
    /// Create a new network proxy
    /// Note: The actual network handling should be done in a separate task that owns P2PNetwork
    pub fn new(
        local_peer_id: PeerId,
        network_id: String,
        command_tx: mpsc::Sender<NetworkCommand>,
    ) -> (
        Self,
        mpsc::Receiver<ProxyRequest>,
        Arc<RwLock<NetworkStats>>,
    ) {
        // Create request channel
        let (request_tx, request_rx) = mpsc::channel::<ProxyRequest>(100);

        // Create cached stats
        let cached_stats = Arc::new(RwLock::new(NetworkStats::default()));

        let proxy = Self {
            command_tx,
            request_tx,
            cached_stats: cached_stats.clone(),
            local_peer_id,
            network_id,
        };

        (proxy, request_rx, cached_stats)
    }

    /// Process network proxy requests
    /// This should be called from within the network thread that already owns P2PNetwork
    pub async fn process_requests(
        p2p_network: &P2PNetwork,
        request_rx: &mut mpsc::Receiver<ProxyRequest>,
        cached_stats: &Arc<RwLock<NetworkStats>>,
        timeout: Duration,
    ) -> bool {
        // Process one request with timeout
        match tokio::time::timeout(timeout, request_rx.recv()).await {
            Ok(Some(request)) => {
                match request {
                    ProxyRequest::GetNetworkInfo(tx) => {
                        let result = p2p_network.get_network_info().await.map_err(
                            |e| -> Box<dyn Error + Send> {
                                Box::new(std::io::Error::new(
                                    std::io::ErrorKind::Other,
                                    e.to_string(),
                                ))
                            },
                        );
                        let _ = tx.send(result);
                    }
                    ProxyRequest::GetConnectionCount(tx) => {
                        let result = p2p_network.get_connection_count().await.map_err(
                            |e| -> Box<dyn Error + Send> {
                                Box::new(std::io::Error::new(
                                    std::io::ErrorKind::Other,
                                    e.to_string(),
                                ))
                            },
                        );
                        let _ = tx.send(result);
                    }
                    ProxyRequest::GetPeers(tx) => {
                        let result =
                            p2p_network
                                .get_peers()
                                .await
                                .map_err(|e| -> Box<dyn Error + Send> {
                                    Box::new(std::io::Error::new(
                                        std::io::ErrorKind::Other,
                                        e.to_string(),
                                    ))
                                });
                        let _ = tx.send(result);
                    }
                    ProxyRequest::GetPeer(peer_id, tx) => {
                        let result = p2p_network.get_peer(&peer_id).await.map_err(
                            |e| -> Box<dyn Error + Send> {
                                Box::new(std::io::Error::new(
                                    std::io::ErrorKind::Other,
                                    e.to_string(),
                                ))
                            },
                        );
                        let _ = tx.send(result);
                    }
                    ProxyRequest::AddPeer(address, permanent, tx) => {
                        let result = p2p_network.add_peer(&address, permanent).await.map_err(
                            |e| -> Box<dyn Error + Send> {
                                Box::new(std::io::Error::new(
                                    std::io::ErrorKind::Other,
                                    e.to_string(),
                                ))
                            },
                        );
                        let _ = tx.send(result);
                    }
                    ProxyRequest::RemovePeer(peer_id, tx) => {
                        let result = p2p_network.remove_peer(&peer_id).await.map_err(
                            |e| -> Box<dyn Error + Send> {
                                Box::new(std::io::Error::new(
                                    std::io::ErrorKind::Other,
                                    e.to_string(),
                                ))
                            },
                        );
                        let _ = tx.send(result);
                    }
                    ProxyRequest::GetBandwidthUsage(period, tx) => {
                        let result = p2p_network.get_bandwidth_usage(period).await.map_err(
                            |e| -> Box<dyn Error + Send> {
                                Box::new(std::io::Error::new(
                                    std::io::ErrorKind::Other,
                                    e.to_string(),
                                ))
                            },
                        );
                        let _ = tx.send(result);
                    }
                    ProxyRequest::GetStats(tx) => {
                        let stats = cached_stats.read().await.clone();
                        let _ = tx.send(stats);
                    }
                    ProxyRequest::PeerCount(tx) => {
                        let count = p2p_network.peer_count().await;
                        let _ = tx.send(count);
                    }
                    ProxyRequest::IsSyncing(tx) => {
                        let syncing = p2p_network.is_syncing();
                        let _ = tx.send(syncing);
                    }
                    ProxyRequest::UpdateStats(stats) => {
                        *cached_stats.write().await = stats;
                    }
                }
                true // Continue processing
            }
            Ok(None) => false, // Channel closed
            Err(_) => true,    // Timeout, continue processing
        }
    }

    /// Update cached stats
    pub async fn update_cached_stats(
        p2p_network: &P2PNetwork,
        cached_stats: &Arc<RwLock<NetworkStats>>,
    ) {
        let stats = p2p_network.get_stats().await;
        *cached_stats.write().await = stats;
    }

    /// Get network information
    pub async fn get_network_info(&self) -> Result<NetworkInfo, Box<dyn Error>> {
        let (tx, rx) = oneshot::channel();
        self.request_tx
            .send(ProxyRequest::GetNetworkInfo(tx))
            .await
            .map_err(|_| -> Box<dyn Error> {
                Box::new(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "Network proxy channel closed",
                ))
            })?;
        rx.await
            .map_err(|_| -> Box<dyn Error> {
                Box::new(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "Network request cancelled",
                ))
            })?
            .map_err(|e| -> Box<dyn Error> {
                Box::new(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    e.to_string(),
                ))
            })
    }

    /// Get connection count
    pub async fn get_connection_count(&self) -> Result<ConnectionCount, Box<dyn Error>> {
        let (tx, rx) = oneshot::channel();
        self.request_tx
            .send(ProxyRequest::GetConnectionCount(tx))
            .await
            .map_err(|_| -> Box<dyn Error> {
                Box::new(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "Network proxy channel closed",
                ))
            })?;
        rx.await
            .map_err(|_| -> Box<dyn Error> {
                Box::new(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "Network request cancelled",
                ))
            })?
            .map_err(|e| -> Box<dyn Error> {
                Box::new(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    e.to_string(),
                ))
            })
    }

    /// Get list of peers
    pub async fn get_peers(&self) -> Result<Vec<PeerInfo>, Box<dyn Error>> {
        let (tx, rx) = oneshot::channel();
        self.request_tx
            .send(ProxyRequest::GetPeers(tx))
            .await
            .map_err(|_| -> Box<dyn Error> {
                Box::new(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "Network proxy channel closed",
                ))
            })?;
        rx.await
            .map_err(|_| -> Box<dyn Error> {
                Box::new(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "Network request cancelled",
                ))
            })?
            .map_err(|e| -> Box<dyn Error> {
                Box::new(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    e.to_string(),
                ))
            })
    }

    /// Get information about a specific peer
    pub async fn get_peer(&self, peer_id: &str) -> Result<Option<PeerInfo>, Box<dyn Error>> {
        let (tx, rx) = oneshot::channel();
        self.request_tx
            .send(ProxyRequest::GetPeer(peer_id.to_string(), tx))
            .await
            .map_err(|_| -> Box<dyn Error> {
                Box::new(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "Network proxy channel closed",
                ))
            })?;
        rx.await
            .map_err(|_| -> Box<dyn Error> {
                Box::new(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "Network request cancelled",
                ))
            })?
            .map_err(|e| -> Box<dyn Error> {
                Box::new(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    e.to_string(),
                ))
            })
    }

    /// Add a new peer
    pub async fn add_peer(
        &self,
        address: &str,
        permanent: bool,
    ) -> Result<PeerAddResponse, Box<dyn Error>> {
        let (tx, rx) = oneshot::channel();
        self.request_tx
            .send(ProxyRequest::AddPeer(address.to_string(), permanent, tx))
            .await
            .map_err(|_| -> Box<dyn Error> {
                Box::new(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "Network proxy channel closed",
                ))
            })?;
        rx.await
            .map_err(|_| -> Box<dyn Error> {
                Box::new(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "Network request cancelled",
                ))
            })?
            .map_err(|e| -> Box<dyn Error> {
                Box::new(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    e.to_string(),
                ))
            })
    }

    /// Remove a peer
    pub async fn remove_peer(&self, peer_id: &str) -> Result<bool, Box<dyn Error>> {
        let (tx, rx) = oneshot::channel();
        self.request_tx
            .send(ProxyRequest::RemovePeer(peer_id.to_string(), tx))
            .await
            .map_err(|_| -> Box<dyn Error> {
                Box::new(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "Network proxy channel closed",
                ))
            })?;
        rx.await
            .map_err(|_| -> Box<dyn Error> {
                Box::new(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "Network request cancelled",
                ))
            })?
            .map_err(|e| -> Box<dyn Error> {
                Box::new(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    e.to_string(),
                ))
            })
    }

    /// Get bandwidth usage
    pub async fn get_bandwidth_usage(&self, period: u64) -> Result<BandwidthUsage, Box<dyn Error>> {
        let (tx, rx) = oneshot::channel();
        self.request_tx
            .send(ProxyRequest::GetBandwidthUsage(period, tx))
            .await
            .map_err(|_| -> Box<dyn Error> {
                Box::new(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "Network proxy channel closed",
                ))
            })?;
        rx.await
            .map_err(|_| -> Box<dyn Error> {
                Box::new(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "Network request cancelled",
                ))
            })?
            .map_err(|e| -> Box<dyn Error> {
                Box::new(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    e.to_string(),
                ))
            })
    }

    /// Get network statistics (cached)
    pub async fn get_stats(&self) -> NetworkStats {
        let (tx, rx) = oneshot::channel();
        let _ = self.request_tx.send(ProxyRequest::GetStats(tx)).await;
        rx.await.unwrap_or_default()
    }

    /// Get peer count
    pub async fn peer_count(&self) -> usize {
        let (tx, rx) = oneshot::channel();
        let _ = self.request_tx.send(ProxyRequest::PeerCount(tx)).await;
        rx.await.unwrap_or(0)
    }

    /// Get peer count synchronously (returns cached value)
    pub fn peer_count_sync(&self) -> usize {
        // Try non-blocking read of cached stats
        // If lock is busy, return 0 as it's just a stat query
        match self.cached_stats.try_read() {
            Ok(stats) => stats.peers_connected,
            Err(_) => 0, // Lock busy, return default
        }
    }
    
    /// Get local peer ID
    pub fn local_peer_id(&self) -> PeerId {
        self.local_peer_id
    }

    /// Check if syncing
    pub fn is_syncing(&self) -> bool {
        // For now, return false as syncing logic is not implemented
        false
    }

    /// Get stats synchronously (returns cached value)
    pub fn get_stats_sync(&self) -> NetworkStats {
        // Try non-blocking read of cached stats
        match self.cached_stats.try_read() {
            Ok(stats) => stats.clone(),
            Err(_) => NetworkStats::default(), // Lock busy, return default
        }
    }

    /// Broadcast a transaction
    pub fn broadcast_transaction(&self, tx: &btclib::types::transaction::Transaction) {
        let tx = tx.clone();
        let command_tx = self.command_tx.clone();

        // Fire and forget
        tokio::spawn(async move {
            let _ = command_tx
                .send(NetworkCommand::AnnounceTransaction {
                    transaction: tx,
                    fee_rate: 1000, // Default fee rate
                })
                .await;
        });
    }

    /// Broadcast a block
    pub fn broadcast_block(&self, block: &btclib::types::block::Block) {
        let block = block.clone();
        let command_tx = self.command_tx.clone();

        // Fire and forget
        tokio::spawn(async move {
            let _ = command_tx
                .send(NetworkCommand::AnnounceBlock {
                    block: block.clone(),
                    height: block.height(),
                    total_difficulty: 1, // This should be calculated properly
                })
                .await;
        });
    }
    
    /// Dial a peer manually using multiaddr string
    pub async fn dial_peer_str(&self, multiaddr_str: &str) -> Result<(), String> {
        let command_tx = self.command_tx.clone();
        
        command_tx
            .send(NetworkCommand::ConnectToPeer(multiaddr_str.to_string()))
            .await
            .map_err(|e| format!("Failed to send connect command: {}", e))?;
        
        Ok(())
    }
}
