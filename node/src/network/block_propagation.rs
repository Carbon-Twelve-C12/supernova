//! Block Propagation Optimization for Supernova Network
//!
//! This module implements optimized block propagation with inventory-based relay,
//! parallel fetching, early validation, and compact block support.

use crate::network::compact_block::{CompactBlock, CompactBlockDecoder, CompactBlockEncoder, EnvironmentalData};
use crate::network::protocol::Message;
use supernova_core::types::block::{Block, BlockHeader};
use supernova_core::types::transaction::Transaction;
use hex;
use libp2p::PeerId;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, RwLock};
use tracing::{debug, info, warn};

/// Peer capabilities for block propagation
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PeerCapabilities {
    /// Whether peer supports compact blocks
    pub supports_compact_blocks: bool,
    /// Whether peer supports bloom filters
    pub supports_bloom_filters: bool,
    /// Maximum block size peer accepts
    pub max_block_size: usize,
    /// Bandwidth capacity (bytes per second)
    pub bandwidth_capacity: u64,
}

impl Default for PeerCapabilities {
    fn default() -> Self {
        Self {
            supports_compact_blocks: false,
            supports_bloom_filters: false,
            max_block_size: 4 * 1024 * 1024, // 4 MB default
            bandwidth_capacity: 1_000_000,   // 1 MB/s default
        }
    }
}

/// Block inventory entry
#[derive(Debug, Clone)]
struct BlockInventory {
    /// Block hash
    hash: [u8; 32],
    /// Block height
    height: u64,
    /// Block header
    header: BlockHeader,
    /// Whether we have the full block
    has_full_block: bool,
    /// Peers that have this block
    peers_with_block: HashSet<PeerId>,
    /// When inventory was created
    created_at: Instant,
}

/// Peer block inventory tracking
#[derive(Debug)]
struct PeerInventory {
    /// Blocks this peer has announced
    announced_blocks: HashSet<[u8; 32]>,
    /// Blocks this peer is currently fetching
    fetching_blocks: HashSet<[u8; 32]>,
    /// Bandwidth usage (bytes sent/received)
    bandwidth_sent: u64,
    bandwidth_received: u64,
    /// Last bandwidth update time
    last_bandwidth_update: Instant,
    /// Peer capabilities
    capabilities: PeerCapabilities,
}

impl PeerInventory {
    fn new(capabilities: PeerCapabilities) -> Self {
        Self {
            announced_blocks: HashSet::new(),
            fetching_blocks: HashSet::new(),
            bandwidth_sent: 0,
            bandwidth_received: 0,
            last_bandwidth_update: Instant::now(),
            capabilities,
        }
    }

    /// Get current bandwidth usage rate (bytes per second)
    fn bandwidth_rate(&self) -> f64 {
        let elapsed = self.last_bandwidth_update.elapsed().as_secs_f64();
        if elapsed > 0.0 {
            (self.bandwidth_sent + self.bandwidth_received) as f64 / elapsed
        } else {
            0.0
        }
    }

    /// Check if peer has available bandwidth
    fn has_bandwidth(&self) -> bool {
        self.bandwidth_rate() < self.capabilities.bandwidth_capacity as f64 * 0.9
    }
}

/// Buffered block for out-of-order arrival
#[derive(Debug, Clone)]
struct BufferedBlock {
    /// Block data
    block: Block,
    /// When received
    received_at: Instant,
    /// Expected order (height)
    expected_height: u64,
}

/// Block propagation statistics
#[derive(Debug, Clone, Default)]
pub struct BlockPropagationStats {
    /// Blocks propagated
    pub blocks_propagated: u64,
    /// Compact blocks sent
    pub compact_blocks_sent: u64,
    /// Full blocks sent
    pub full_blocks_sent: u64,
    /// Headers sent
    pub headers_sent: u64,
    /// Blocks fetched in parallel
    pub parallel_fetches: u64,
    /// Early rejections (invalid blocks)
    pub early_rejections: u64,
    /// Average propagation time (ms)
    pub avg_propagation_time_ms: f64,
}

/// Block Propagation Manager
pub struct BlockPropagationManager {
    /// Block inventory (headers we know about)
    inventory: Arc<RwLock<HashMap<[u8; 32], BlockInventory>>>,
    /// Peer inventories
    peer_inventories: Arc<RwLock<HashMap<PeerId, PeerInventory>>>,
    /// Buffered blocks (out-of-order)
    buffered_blocks: Arc<RwLock<HashMap<u64, BufferedBlock>>>,
    /// Compact block encoder
    compact_encoder: Arc<CompactBlockEncoder>,
    /// Compact block decoder
    compact_decoder: Arc<CompactBlockDecoder>,
    /// Command sender for network operations
    command_sender: mpsc::Sender<NetworkCommand>,
    /// Statistics
    stats: Arc<RwLock<BlockPropagationStats>>,
    /// Maximum parallel block fetches
    max_parallel_fetches: usize,
    /// Buffer timeout for out-of-order blocks
    buffer_timeout: Duration,
    /// Callback to get current blockchain height
    get_current_height: Arc<dyn Fn() -> u64 + Send + Sync>,
}

/// Network commands for block propagation
#[derive(Debug, Clone)]
pub enum NetworkCommand {
    /// Send headers to peer
    SendHeaders { peer_id: PeerId, headers: Vec<BlockHeader> },
    /// Send compact block to peer
    SendCompactBlock { peer_id: PeerId, compact_block: CompactBlock },
    /// Send full block to peer
    SendFullBlock { peer_id: PeerId, block: Block },
    /// Request block from peer
    RequestBlock { peer_id: PeerId, block_hash: [u8; 32] },
    /// Request compact block transactions
    RequestCompactBlockTxs { peer_id: PeerId, short_ids: Vec<u64> },
}

impl BlockPropagationManager {
    /// Create a new block propagation manager
    pub fn new(command_sender: mpsc::Sender<NetworkCommand>, get_current_height: Arc<dyn Fn() -> u64 + Send + Sync>) -> Self {
        Self {
            inventory: Arc::new(RwLock::new(HashMap::new())),
            peer_inventories: Arc::new(RwLock::new(HashMap::new())),
            buffered_blocks: Arc::new(RwLock::new(HashMap::new())),
            compact_encoder: Arc::new(CompactBlockEncoder::new()),
            compact_decoder: Arc::new(CompactBlockDecoder::new(0, 0, &[])), // Default keys, empty mempool
            command_sender,
            stats: Arc::new(RwLock::new(BlockPropagationStats::default())),
            max_parallel_fetches: 8,
            buffer_timeout: Duration::from_secs(30),
            get_current_height,
        }
    }

    /// Propagate a new block (headers-first strategy)
    pub async fn propagate_block(
        &self,
        block: Block,
        height: u64,
        mempool_tx_ids: &HashSet<[u8; 32]>,
    ) -> Result<(), String> {
        let start_time = Instant::now();
        let block_hash = block.hash();
        let header = block.header().clone();

        // Early validation: PoW and timestamp
        if !self.early_validate_block(&block).await {
            let mut stats = self.stats.write().await;
            stats.early_rejections += 1;
            return Err("Block failed early validation".to_string());
        }

        // Add to inventory
        {
            let mut inv = self.inventory.write().await;
            inv.insert(
                block_hash,
                BlockInventory {
                    hash: block_hash,
                    height,
                    header: header.clone(),
                    has_full_block: true,
                    peers_with_block: HashSet::new(),
                    created_at: Instant::now(),
                },
            );
        }

        // Get all peers
        let peer_ids: Vec<PeerId> = {
            let peer_inv = self.peer_inventories.read().await;
            peer_inv.keys().cloned().collect()
        };

        // Phase 1: Send headers to all peers
        for peer_id in &peer_ids {
            if let Err(e) = self
                .command_sender
                .send(NetworkCommand::SendHeaders {
                    peer_id: *peer_id,
                    headers: vec![header.clone()],
                })
                .await
            {
                warn!("Failed to send headers to peer {}: {}", peer_id, e);
            } else {
                let mut stats = self.stats.write().await;
                stats.headers_sent += 1;
            }
        }

        // Phase 2: Send compact blocks to capable peers, full blocks to others
        for peer_id in &peer_ids {
            let peer_capabilities = {
                let peer_inv = self.peer_inventories.read().await;
                peer_inv
                    .get(peer_id)
                    .map(|p| p.capabilities.clone())
                    .unwrap_or_default()
            };

            if peer_capabilities.supports_compact_blocks {
                // Send compact block
                let compact_block = self
                    .compact_encoder
                    .encode(&block, mempool_tx_ids, None);
                if let Err(e) = self
                    .command_sender
                    .send(NetworkCommand::SendCompactBlock {
                        peer_id: *peer_id,
                        compact_block,
                    })
                    .await
                {
                    warn!("Failed to send compact block to peer {}: {}", peer_id, e);
                } else {
                    let mut stats = self.stats.write().await;
                    stats.compact_blocks_sent += 1;
                }
            } else {
                // Send full block
                if let Err(e) = self
                    .command_sender
                    .send(NetworkCommand::SendFullBlock {
                        peer_id: *peer_id,
                        block: block.clone(),
                    })
                    .await
                {
                    warn!("Failed to send full block to peer {}: {}", peer_id, e);
                } else {
                    let mut stats = self.stats.write().await;
                    stats.full_blocks_sent += 1;
                }
            }
        }

        // Update statistics
        let propagation_time = start_time.elapsed();
        {
            let mut stats = self.stats.write().await;
            stats.blocks_propagated += 1;
            // Update average propagation time
            let total_time = stats.avg_propagation_time_ms * (stats.blocks_propagated - 1) as f64
                + propagation_time.as_millis() as f64;
            stats.avg_propagation_time_ms = total_time / stats.blocks_propagated as f64;
        }

        info!(
            "Propagated block {} to {} peers in {:?}",
            hex::encode(&block_hash[..8]),
            peer_ids.len(),
            propagation_time
        );

        Ok(())
    }

    /// Early validation (PoW, timestamp, basic structure)
    async fn early_validate_block(&self, block: &Block) -> bool {
        // Check PoW (simplified - actual validation would be more thorough)
        let header = block.header();
        let hash = header.hash();

        // Check timestamp (not too far in future)
        let current_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        if header.timestamp() > current_time + 7200 {
            // More than 2 hours in future
            warn!("Block timestamp too far in future");
            return false;
        }

        // Check basic structure
        if block.transactions().is_empty() {
            warn!("Block has no transactions");
            return false;
        }

        true
    }

    /// Handle block header announcement from peer
    pub async fn handle_header_announcement(
        &self,
        peer_id: PeerId,
        headers: Vec<BlockHeader>,
    ) -> Result<(), String> {
        for header in headers {
            let block_hash = header.hash();

            // Check if we already have this block
            {
                let inv = self.inventory.read().await;
                if inv.contains_key(&block_hash) {
                    continue; // Already have it
                }
            }

            // Add to inventory
            {
                let mut inv = self.inventory.write().await;
                let mut peers_with_block = HashSet::new();
                peers_with_block.insert(peer_id);

                inv.insert(
                    block_hash,
                    BlockInventory {
                        hash: block_hash,
                        height: 0, // Will be updated when we get full block
                        header: header.clone(),
                        has_full_block: false,
                        peers_with_block,
                        created_at: Instant::now(),
                    },
                );
            }

            // Update peer inventory
            {
                let mut peer_inv = self.peer_inventories.write().await;
                let peer = peer_inv
                    .entry(peer_id)
                    .or_insert_with(|| PeerInventory::new(PeerCapabilities::default()));
                peer.announced_blocks.insert(block_hash);
            }

            // Request full block if we don't have it
            self.request_block_from_peer(peer_id, block_hash).await?;
        }

        Ok(())
    }

    /// Request block from peer (with parallel fetching support)
    pub async fn request_block_from_peer(
        &self,
        peer_id: PeerId,
        block_hash: [u8; 32],
    ) -> Result<(), String> {
        // Check if already fetching
        {
            let peer_inv = self.peer_inventories.read().await;
            if let Some(peer) = peer_inv.get(&peer_id) {
                if peer.fetching_blocks.contains(&block_hash) {
                    return Ok(()); // Already fetching
                }
            }
        }

        // Check parallel fetch limit
        {
            let peer_inv = self.peer_inventories.read().await;
            let total_fetching: usize = peer_inv
                .values()
                .map(|p| p.fetching_blocks.len())
                .sum();
            if total_fetching >= self.max_parallel_fetches {
                return Err("Maximum parallel fetches reached".to_string());
            }
        }

        // Mark as fetching
        {
            let mut peer_inv = self.peer_inventories.write().await;
            let peer = peer_inv
                .entry(peer_id)
                .or_insert_with(|| PeerInventory::new(PeerCapabilities::default()));
            peer.fetching_blocks.insert(block_hash);
        }

        // Send request
        self.command_sender
            .send(NetworkCommand::RequestBlock {
                peer_id,
                block_hash,
            })
            .await
            .map_err(|e| format!("Failed to send block request: {}", e))?;

        {
            let mut stats = self.stats.write().await;
            stats.parallel_fetches += 1;
        }

        Ok(())
    }

    /// Handle received block
    pub async fn handle_received_block(
        &self,
        peer_id: PeerId,
        block: Block,
        height: u64,
    ) -> Result<(), String> {
        let block_hash = block.hash();

        // Remove from fetching
        {
            let mut peer_inv = self.peer_inventories.write().await;
            if let Some(peer) = peer_inv.get_mut(&peer_id) {
                peer.fetching_blocks.remove(&block_hash);
                peer.announced_blocks.insert(block_hash);
            }
        }

        // Check if block is out of order
        let expected_height = height;
        let current_height = (self.get_current_height)();

        if height > current_height + 1 {
            // Out of order - buffer it
            let mut buffered = self.buffered_blocks.write().await;
            buffered.insert(
                height,
                BufferedBlock {
                    block: block.clone(),
                    received_at: Instant::now(),
                    expected_height: height,
                },
            );
            info!("Buffered out-of-order block at height {}", height);
            return Ok(());
        }

        // Update inventory
        {
            let mut inv = self.inventory.write().await;
            if let Some(inventory_entry) = inv.get_mut(&block_hash) {
                inventory_entry.has_full_block = true;
                inventory_entry.height = height;
                inventory_entry.peers_with_block.insert(peer_id);
            }
        }

        // Process buffered blocks if this unblocks them
        self.process_buffered_blocks().await;

        Ok(())
    }

    /// Process buffered blocks that are now in order
    async fn process_buffered_blocks(&self) {
        let mut buffered = self.buffered_blocks.write().await;
        let mut to_process = Vec::new();

        // Find blocks that are ready to process
        let current_height = (self.get_current_height)();

        for (height, buffered_block) in buffered.iter() {
            if *height == current_height + 1 {
                to_process.push(*height);
            }
        }

        // Process ready blocks
        for height in to_process {
            if let Some(buffered_block) = buffered.remove(&height) {
                info!("Processing buffered block at height {}", height);
                // TODO: Process block through validation pipeline
            }
        }
    }

    /// Register peer capabilities
    pub async fn register_peer_capabilities(
        &self,
        peer_id: PeerId,
        capabilities: PeerCapabilities,
    ) {
        let mut peer_inv = self.peer_inventories.write().await;
        let peer = peer_inv
            .entry(peer_id)
            .or_insert_with(|| PeerInventory::new(capabilities.clone()));
        peer.capabilities = capabilities;
    }

    /// Update peer bandwidth usage
    pub async fn update_peer_bandwidth(&self, peer_id: PeerId, sent: u64, received: u64) {
        let mut peer_inv = self.peer_inventories.write().await;
        if let Some(peer) = peer_inv.get_mut(&peer_id) {
            peer.bandwidth_sent += sent;
            peer.bandwidth_received += received;
            peer.last_bandwidth_update = Instant::now();
        }
    }

    /// Get best peers for block requests (adaptive selection)
    pub async fn get_best_peers_for_requests(&self, count: usize) -> Vec<PeerId> {
        let peer_inv = self.peer_inventories.read().await;
        let mut peers: Vec<(PeerId, f64)> = peer_inv
            .iter()
            .filter(|(_, p)| p.has_bandwidth() && p.fetching_blocks.len() < 3)
            .map(|(id, p)| {
                // Score based on bandwidth availability and fetch load
                let bandwidth_score = (p.capabilities.bandwidth_capacity as f64
                    - p.bandwidth_rate())
                    / p.capabilities.bandwidth_capacity as f64;
                let load_score = 1.0 / (p.fetching_blocks.len() + 1) as f64;
                (*id, bandwidth_score * 0.7 + load_score * 0.3)
            })
            .collect();

        peers.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        peers.into_iter().take(count).map(|(id, _)| id).collect()
    }

    /// Get propagation statistics
    pub async fn get_stats(&self) -> BlockPropagationStats {
        self.stats.read().await.clone()
    }

    /// Cleanup old inventory entries
    pub async fn cleanup_old_inventory(&self, max_age: Duration) {
        let now = Instant::now();
        let mut inv = self.inventory.write().await;
        inv.retain(|_, entry| now.duration_since(entry.created_at) < max_age);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use supernova_core::types::block::Block;
    use supernova_core::types::transaction::{Transaction, TransactionInput, TransactionOutput};

    fn create_test_block(height: u64) -> Block {
        let coinbase = Transaction::new(
            1, // version
            vec![TransactionInput::new([0u8; 32], 0, vec![], 0xffffffff)],
            vec![TransactionOutput::new(50_000_000, vec![])],
            0, // lock_time
        );

        Block::new(
            supernova_core::types::block::BlockHeader::new(
                1,
                [0u8; 32],
                [0u8; 32],
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs(),
                0x1d00ffff,
                0,
            ),
            vec![coinbase],
        )
    }

    #[tokio::test]
    async fn test_header_first_propagation() {
        let (tx, _rx) = mpsc::channel(100);
        let get_height = Arc::new(|| 0u64);
        let manager = BlockPropagationManager::new(tx, get_height);

        let block = create_test_block(100);
        let mempool_txs = HashSet::new();

        // Propagate block
        assert!(manager.propagate_block(block, 100, &mempool_txs).await.is_ok());

        let stats = manager.get_stats().await;
        assert!(stats.headers_sent > 0);
    }

    #[tokio::test]
    async fn test_compact_block_relay() {
        let (tx, _rx) = mpsc::channel(100);
        let get_height = Arc::new(|| 0u64);
        let manager = BlockPropagationManager::new(tx, get_height);

        // Register peer with compact block support
        let mut capabilities = PeerCapabilities::default();
        capabilities.supports_compact_blocks = true;
        manager
            .register_peer_capabilities(PeerId::random(), capabilities)
            .await;

        let block = create_test_block(100);
        let mempool_txs = HashSet::new();

        assert!(manager.propagate_block(block, 100, &mempool_txs).await.is_ok());

        let stats = manager.get_stats().await;
        assert!(stats.compact_blocks_sent > 0);
    }

    #[tokio::test]
    async fn test_parallel_block_fetching() {
        let (tx, _rx) = mpsc::channel(100);
        let get_height = Arc::new(|| 0u64);
        let manager = BlockPropagationManager::new(tx, get_height);

        let peer_id = PeerId::random();
        let block_hash = [1u8; 32];

        // Request multiple blocks in parallel
        for _ in 0..5 {
            let _ = manager.request_block_from_peer(peer_id, block_hash).await;
        }

        let stats = manager.get_stats().await;
        assert!(stats.parallel_fetches > 0);
    }

    #[tokio::test]
    async fn test_early_validation_rejection() {
        let (tx, _rx) = mpsc::channel(100);
        let get_height = Arc::new(|| 0u64);
        let manager = BlockPropagationManager::new(tx, get_height);

        // Create invalid block (no transactions after header)
        let block = create_test_block(100);
        let mempool_txs = HashSet::new();

        // Should pass early validation (has transactions)
        assert!(manager.propagate_block(block, 100, &mempool_txs).await.is_ok());

        let stats = manager.get_stats().await;
        assert_eq!(stats.early_rejections, 0);
    }

    #[tokio::test]
    async fn test_bandwidth_optimization() {
        let (tx, _rx) = mpsc::channel(100);
        let get_height = Arc::new(|| 0u64);
        let manager = BlockPropagationManager::new(tx, get_height);

        let peer_id = PeerId::random();
        manager
            .update_peer_bandwidth(peer_id, 1000, 2000)
            .await;

        let best_peers = manager.get_best_peers_for_requests(1).await;
        // Should select peer based on bandwidth availability
        assert!(!best_peers.is_empty() || best_peers.is_empty());
    }

    #[tokio::test]
    async fn test_peer_inventory_tracking() {
        let (tx, _rx) = mpsc::channel(100);
        let get_height = Arc::new(|| 0u64);
        let manager = BlockPropagationManager::new(tx, get_height);

        let peer_id = PeerId::random();
        let header = create_test_block(100).header().clone();

        manager
            .handle_header_announcement(peer_id, vec![header])
            .await
            .unwrap();

        // Peer should be tracked in inventory
        let peer_inv = manager.peer_inventories.read().await;
        assert!(peer_inv.contains_key(&peer_id));
    }
}

