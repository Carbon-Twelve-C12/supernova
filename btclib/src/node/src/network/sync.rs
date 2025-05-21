use tokio::sync::mpsc;
use tokio::time::{Duration, sleep, timeout};
use std::collections::{HashMap, HashSet, VecDeque};
use std::error::Error;
use std::sync::{Arc, RwLock, Mutex};
use tracing::{debug, info, warn, error};
use std::time::Instant;

use super::protocol::{Message, BlockAnnouncement};
use super::p2p::{NetworkCommand, NetworkEvent, P2PNetwork};
use crate::blockchain::{BlockHeader, Block, BlockchainState};
use crate::storage::BlockStorage;
use crate::validation::{BlockValidator, ValidationError, ValidationResult};
use libp2p::PeerId;

// Constants for sync configuration
const HEADERS_BATCH_SIZE: u32 = 2000;
const MAX_HEADERS_REQUEST: u32 = 2000;
const BLOCKS_DOWNLOAD_BATCH_SIZE: u32 = 50;
const MAX_CONCURRENT_BLOCK_REQUESTS: usize = 10;
const BLOCK_REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
const RETRY_DELAY: Duration = Duration::from_secs(10);
const MAX_RETRIES: usize = 3;
const CHECKPOINT_INTERVAL: u64 = 10_000; // Create checkpoint every 10,000 blocks
const MIN_CHECKPOINT_PEERS: usize = 3; // Minimum peers to agree on checkpoint

/// Represents the current state of the sync process
#[derive(Debug, Clone, PartialEq)]
pub enum SyncState {
    /// Not currently syncing
    Idle,
    
    /// Downloading headers
    HeaderSync {
        /// Current sync height
        current_height: u64,
        /// Target height to sync to
        target_height: u64,
        /// Time when sync started
        start_time: Instant,
    },
    
    /// Downloading blocks
    BlockSync {
        /// Current sync height
        current_height: u64,
        /// Target height to sync to
        target_height: u64,
        /// Time when sync started
        start_time: Instant,
    },
    
    /// Waiting for IBD (Initial Block Download) to complete
    IBDWait {
        /// Target height
        target_height: u64,
    },
}

/// Blockchain synchronization manager
pub struct ChainSync {
    /// Current locally validated chain height
    pub height: u64,
    
    /// Best known height from peers
    pub best_known_height: u64,
    
    /// Headers we've downloaded but not yet processed
    pending_headers: VecDeque<BlockHeader>,
    
    /// Blocks we've requested but not yet received
    pending_block_requests: HashMap<[u8; 32], (PeerId, Instant, usize)>, // hash -> (peer, request_time, retry_count)
    
    /// Blocks we've received but not yet processed
    pending_blocks: HashMap<u64, Block>, // height -> block
    
    /// Currently downloading heights
    blocks_in_flight: HashSet<u64>,
    
    /// Sync state
    sync_state: SyncState,
    
    /// Command sender to network layer
    command_sender: mpsc::Sender<NetworkCommand>,
    
    /// Best peers for downloading
    best_peers: Vec<(PeerId, u64)>, // (peer_id, height)
    
    /// Block validator
    validator: Arc<BlockValidator>,
    
    /// Block storage
    storage: Arc<dyn BlockStorage>,
    
    /// Chain state
    chain_state: Arc<RwLock<BlockchainState>>,
    
    /// Last checkpoint height
    last_checkpoint_height: u64,
    
    /// Checkpoint proposals from peers
    checkpoint_proposals: HashMap<u64, HashMap<[u8; 32], HashSet<PeerId>>>, // height -> (hash -> peer_ids)
}

impl ChainSync {
    /// Create a new chain synchronization manager
    pub fn new(
        command_sender: mpsc::Sender<NetworkCommand>, 
        validator: Arc<BlockValidator>,
        storage: Arc<dyn BlockStorage>,
        chain_state: Arc<RwLock<BlockchainState>>,
    ) -> Self {
        // Get current height from chain state
        let height = chain_state.read().unwrap().get_height();
        let last_checkpoint_height = height - (height % CHECKPOINT_INTERVAL);
        
        Self {
            height,
            best_known_height: height,
            pending_headers: VecDeque::new(),
            pending_block_requests: HashMap::new(),
            pending_blocks: HashMap::new(),
            blocks_in_flight: HashSet::new(),
            sync_state: SyncState::Idle,
            command_sender,
            best_peers: Vec::new(),
            validator,
            storage,
            chain_state,
            last_checkpoint_height,
            checkpoint_proposals: HashMap::new(),
        }
    }

    /// Start the synchronization process
    pub async fn start_sync(&mut self, target_height: u64) -> Result<(), Box<dyn Error>> {
        if target_height <= self.height {
            debug!("Already at or beyond target height: {}", target_height);
            return Ok(());
        }

        // Reset sync state
        self.pending_headers.clear();
        self.pending_block_requests.clear();
        self.blocks_in_flight.clear();

        // Update sync state
        self.best_known_height = target_height;
        self.sync_state = SyncState::HeaderSync {
            current_height: self.height,
            target_height,
            start_time: Instant::now(),
        };
        
        info!("Starting sync from height {} to {}", self.height, target_height);
        
        // Start headers-first sync
        self.request_next_headers_batch().await?;

        Ok(())
    }

    /// Handles a new block announcement from the network
    pub async fn handle_block_announcement(&mut self, peer_id: PeerId, announcement: BlockAnnouncement) -> Result<(), Box<dyn Error>> {
        let height = announcement.height;
        
        // Update our knowledge of the peer's height
        self.update_peer_height(peer_id, height);
        
        // If this is higher than our known best, update sync target
        if height > self.best_known_height {
            self.best_known_height = height;
            
            // If we're idle, start syncing
            if matches!(self.sync_state, SyncState::Idle) {
                info!("New block announced at height {}, starting sync", height);
                self.start_sync(height).await?;
            } else {
                // Otherwise update target height if we're already syncing
                self.update_sync_target(height);
            }
        }
        
        // If this is just the next block we need, request it directly instead of full sync
        if height == self.height + 1 && 
           matches!(self.sync_state, SyncState::Idle) && 
           !self.blocks_in_flight.contains(&height) {
            info!("Requesting next block at height {}", height);
            
            let message = Message::GetBlock { hash: announcement.hash };
            self.command_sender
                .send(NetworkCommand::SendMessage(peer_id, message))
                .await?;
                
            self.pending_block_requests.insert(announcement.hash, (peer_id, Instant::now(), 0));
            self.blocks_in_flight.insert(height);
        }
        
        Ok(())
    }

    /// Handle received headers from peer
    pub async fn handle_headers(&mut self, peer_id: PeerId, protocol_headers: Vec<super::protocol::BlockHeader>, start_height: u64) -> Result<(), Box<dyn Error>> {
        if protocol_headers.is_empty() {
            debug!("Received empty headers response from {}", peer_id);
            return Ok(());
        }
        
        info!("Received {} headers from peer {} starting at height {}", protocol_headers.len(), peer_id, start_height);
        
        // Convert protocol headers to core headers
        let headers: Vec<BlockHeader> = protocol_headers.iter()
            .map(|h| BlockHeader::from_protocol_header(h))
            .collect();
        
        // Validate headers (check linking and proof of work)
        let mut prev_hash = if start_height > 0 && start_height == self.height + 1 {
            // If this is continuing from our current chain tip, use the last hash
            self.get_tip_hash()
        } else if !self.pending_headers.is_empty() && start_height == self.height + 1 + self.pending_headers.len() as u64 {
            // If this continues our pending headers, use the last pending header hash
            self.pending_headers.back().unwrap().hash()
        } else {
            // Otherwise, we need to validate this against our stored block at start_height - 1
            if start_height > 0 {
                match self.storage.get_block_hash_at_height(start_height - 1) {
                    Ok(Some(hash)) => hash,
                    _ => {
                        warn!("Cannot validate headers: missing previous block at height {}", start_height - 1);
                        return Ok(());
                    }
                }
            } else {
                // Genesis block has no previous hash
                [0; 32]
            }
        };
        
        // Validate each header
        let mut valid_headers = Vec::new();
        for (i, header) in headers.iter().enumerate() {
            // Check proper linking to previous block
            if header.prev_block_hash != prev_hash {
                warn!("Header at height {} has incorrect prev_hash, expected {}, got {}", 
                    start_height + i as u64, hex::encode(prev_hash), hex::encode(header.prev_block_hash));
                break;
            }
            
            // Check proof of work
            if !header.meets_target() {
                warn!("Header at height {} does not meet proof-of-work target", start_height + i as u64);
                break;
            }
            
            // Update prev_hash for the next iteration
            prev_hash = header.hash();
            valid_headers.push(header.clone());
        }
        
        if valid_headers.is_empty() {
            warn!("No valid headers found in batch from peer {}", peer_id);
            
            // Consider downgrading peer's reputation
            // TODO: Implement peer reputation system
            
            return Ok(());
        }
        
        // Add valid headers to pending queue
        for header in valid_headers {
            self.pending_headers.push_back(header);
        }
        
        if let SyncState::HeaderSync { current_height, target_height, start_time } = self.sync_state {
            // If we've got all headers up to target, start downloading blocks
            let new_height = current_height + valid_headers.len() as u64;
            if new_height >= target_height {
                info!("Headers sync complete, switching to block download. Got {} headers", 
                     self.pending_headers.len());
                self.sync_state = SyncState::BlockSync {
                    current_height: self.height,
                    target_height,
                    start_time: Instant::now(),
                };
                
                // Start downloading blocks
                self.request_blocks_parallel().await?;
            } else {
                // Update current height and request next batch
                self.sync_state = SyncState::HeaderSync {
                    current_height: new_height,
                    target_height,
                    start_time,
                };
                
                // Request next headers batch
                self.request_next_headers_batch().await?;
            }
        }
        
        Ok(())
    }

    /// Handle a received block from a peer
    pub async fn handle_block(&mut self, peer_id: PeerId, height: u64, block_data: Vec<u8>) -> Result<(), Box<dyn Error>> {
        debug!("Received block at height {} from peer {}", height, peer_id);
        
        // Deserialize the block
        let block = match Block::deserialize(&block_data) {
            Ok(block) => block,
            Err(e) => {
                warn!("Failed to deserialize block from peer {}: {}", peer_id, e);
                return Ok(());
            }
        };
        
        // Verify block hash matches the requested hash (if we have it)
        let block_hash = block.hash();
        for (hash, (request_peer, _, _)) in self.pending_block_requests.iter() {
            if *hash == block_hash && *request_peer == peer_id {
                // Remove from pending requests
                self.pending_block_requests.remove(hash);
                break;
            }
        }
        
        // Mark as no longer in flight
        self.blocks_in_flight.remove(&height);
        
        // Store in pending blocks
        self.pending_blocks.insert(height, block);
        
        // Try to process any available blocks
        self.process_pending_blocks().await?;
        
        // If we're in block sync and have capacity, request more blocks
        if matches!(self.sync_state, SyncState::BlockSync { .. }) && 
           self.blocks_in_flight.len() < MAX_CONCURRENT_BLOCK_REQUESTS {
            self.request_blocks_parallel().await?;
        }
        
        Ok(())
    }

    /// Process blocks that have been downloaded but not yet validated
    async fn process_pending_blocks(&mut self) -> Result<(), Box<dyn Error>> {
        let mut processed_heights = Vec::new();
        
        // Process blocks in order as long as we have the next one
        let mut current_height = self.height + 1;
        while let Some(block) = self.pending_blocks.get(&current_height) {
            // Validate block
            match self.validate_block(current_height, block) {
                Ok(_) => {
                    info!("Block at height {} validated and added to chain", current_height);
                    
                    // Mark height for removal
                    processed_heights.push(current_height);
                    
                    // Store block in persistent storage
                    if let Err(e) = self.storage.store_block(current_height, block) {
                        error!("Failed to store block at height {}: {}", current_height, e);
                        break;
                    }
                    
                    // Update chain state
                    {
                        let mut state = self.chain_state.write().unwrap();
                        if let Err(e) = state.apply_block(block) {
                            error!("Failed to apply block to chain state: {}", e);
                            break;
                        }
                    }
                    
                    // Check if this height is a checkpoint multiple
                    if current_height % CHECKPOINT_INTERVAL == 0 {
                        self.create_checkpoint(current_height, block.hash()).await?;
                    }
                    
                    // Move to next height
                    current_height += 1;
                    self.height = current_height - 1;
                },
                Err(e) => {
                    error!("Failed to validate block at height {}: {}", current_height, e);
                    
                    // Handle invalid block
                    // TODO: Apply penalties for peers that sent invalid blocks
                    
                    // For now, just remove the block and try to redownload
                    self.pending_blocks.remove(&current_height);
                    
                    // Request the block again
                    self.request_block_at_height(current_height).await?;
                    
                    break;
                }
            }
        }
        
        // Remove processed blocks
        for height in processed_heights {
            self.pending_blocks.remove(&height);
        }
        
        // Check if sync is complete
        if let SyncState::BlockSync { target_height, start_time, .. } = self.sync_state {
            if self.height >= target_height {
                let elapsed = start_time.elapsed();
                info!("Sync complete at height {}. Took {:?}", self.height, elapsed);
                self.sync_state = SyncState::Idle;
                
                // Clear any remaining sync data to free memory
                self.pending_headers.clear();
                self.blocks_in_flight.clear();
            } else {
                // Update sync state with new height
                self.sync_state = SyncState::BlockSync {
                    current_height: self.height,
                    target_height,
                    start_time,
                };
            }
        }
        
        Ok(())
    }

    /// Validate a block
    fn validate_block(&self, height: u64, block: &Block) -> Result<(), ValidationError> {
        // First, check if we have the correct header for this height in our pending headers
        if !self.pending_headers.is_empty() {
            let header_index = (height - self.height - 1) as usize;
            if header_index < self.pending_headers.len() {
                let expected_header = &self.pending_headers[header_index];
                if expected_header.hash() != block.header.hash() {
                    return Err(ValidationError::HeaderMismatch);
                }
            }
        }
        
        // Basic validation
        if !block.validate() {
            return Err(ValidationError::InvalidBlock);
        }
        
        // Use block validator for full validation
        match self.validator.validate_block(block, height) {
            ValidationResult::Valid => Ok(()),
            ValidationResult::Invalid(error) => Err(error),
            ValidationResult::Deferred => {
                // For simplicity, we'll treat deferred as an error for now
                Err(ValidationError::ValidationDeferred)
            }
        }
    }

    /// Request the next batch of headers during headers-first sync
    async fn request_next_headers_batch(&mut self) -> Result<(), Box<dyn Error>> {
        if let SyncState::HeaderSync { current_height, .. } = self.sync_state {
            // Find best peer to request from
            if let Some((peer_id, _)) = self.get_best_peer() {
                let start_height = current_height + 1;
                
                info!("Requesting headers from height {} (batch size: {})", 
                     start_height, HEADERS_BATCH_SIZE);
                
                // Send get headers request to peer
                let message = Message::GetHeaders {
                    start_height,
                    count: HEADERS_BATCH_SIZE,
                };
                
                self.command_sender
                    .send(NetworkCommand::SendMessage(peer_id, message))
                    .await?;
            } else {
                warn!("No suitable peers available for headers sync");
                // Retry after delay
                sleep(RETRY_DELAY).await;
                self.request_next_headers_batch().await?;
            }
        }
        
        Ok(())
    }

    /// Request blocks in parallel during block download phase
    async fn request_blocks_parallel(&mut self) -> Result<(), Box<dyn Error>> {
        // If we're not in block sync mode, do nothing
        if !matches!(self.sync_state, SyncState::BlockSync { .. }) {
            return Ok(());
        }
        
        let mut requested = 0;
        let mut next_height = self.height + 1;
        
        // Request blocks until we hit the target concurrent request limit
        while self.blocks_in_flight.len() < MAX_CONCURRENT_BLOCK_REQUESTS && 
              requested < BLOCKS_DOWNLOAD_BATCH_SIZE {
            
            // Skip if we already have this block pending or in flight
            if self.pending_blocks.contains_key(&next_height) || 
               self.blocks_in_flight.contains(&next_height) {
                next_height += 1;
                continue;
            }
            
            // Request this block
            if let Err(e) = self.request_block_at_height(next_height).await {
                warn!("Failed to request block at height {}: {}", next_height, e);
                break;
            }
            
            requested += 1;
            next_height += 1;
        }
        
        if requested > 0 {
            info!("Requested {} blocks in parallel", requested);
        }
        
        Ok(())
    }
    
    /// Request a specific block by height
    async fn request_block_at_height(&mut self, height: u64) -> Result<(), Box<dyn Error>> {
        // If we don't have the header for this height, we can't request the block
        if let Some(header) = self.get_header_at_height(height) {
            // Find a suitable peer
            if let Some((peer_id, _)) = self.get_best_peer() {
                let block_hash = header.hash();
                
                debug!("Requesting block at height {} (hash {}) from peer {}", 
                      height, hex::encode(&block_hash), peer_id);
                
                // Send request to peer
                let message = Message::GetBlock { hash: block_hash };
                self.command_sender
                    .send(NetworkCommand::SendMessage(peer_id, message))
                    .await?;
                
                // Track request
                self.pending_block_requests.insert(block_hash, (peer_id, Instant::now(), 0));
                self.blocks_in_flight.insert(height);
                
                Ok(())
            } else {
                Err("No suitable peers available for block download".into())
            }
        } else {
            Err(format!("Missing header for height {}, cannot request block", height).into())
        }
    }

    /// Check for timed out requests and retry if needed
    pub async fn check_timeouts(&mut self) -> Result<(), Box<dyn Error>> {
        let now = Instant::now();
        let mut timed_out = Vec::new();
        
        // Check for timed out requests
        for (hash, (peer_id, request_time, retry_count)) in &self.pending_block_requests {
            if now.duration_since(*request_time) > BLOCK_REQUEST_TIMEOUT {
                timed_out.push((*hash, *peer_id, *retry_count));
            }
        }
        
        // Handle timed out requests
        for (hash, peer_id, retry_count) in timed_out {
            self.pending_block_requests.remove(&hash);
            
            // Find the height for this hash
            let height = self.find_height_by_hash(&hash);
            
            if let Some(height) = height {
                self.blocks_in_flight.remove(&height);
                
                if retry_count < MAX_RETRIES {
                    warn!("Block request timed out for height {}, retrying (attempt {})", 
                         height, retry_count + 1);
                    
                    // Get a different peer
                    if let Some((new_peer_id, _)) = self.get_best_peer_excluding(&[peer_id]) {
                        // Retry with new peer
                        let message = Message::GetBlock { hash };
                        self.command_sender
                            .send(NetworkCommand::SendMessage(new_peer_id, message))
                            .await?;
                        
                        // Update request tracking
                        self.pending_block_requests.insert(hash, (new_peer_id, Instant::now(), retry_count + 1));
                        self.blocks_in_flight.insert(height);
                    } else {
                        warn!("No alternative peers available for retry");
                    }
                } else {
                    error!("Block request for height {} failed after {} retries", height, MAX_RETRIES);
                    
                    // If we've retried too many times, consider this peer unreliable
                    // TODO: Implement peer reliability scoring
                    
                    // For now, just try again with a longer timeout to recover
                    sleep(RETRY_DELAY * 2).await;
                    if let Some((new_peer_id, _)) = self.get_best_peer_excluding(&[peer_id]) {
                        // Try once more with a new peer after a delay
                        let message = Message::GetBlock { hash };
                        self.command_sender
                            .send(NetworkCommand::SendMessage(new_peer_id, message))
                            .await?;
                        
                        // Reset retry count
                        self.pending_block_requests.insert(hash, (new_peer_id, Instant::now(), 0));
                        self.blocks_in_flight.insert(height);
                    }
                }
            }
        }
        
        Ok(())
    }

    /// Create and broadcast a checkpoint
    async fn create_checkpoint(&mut self, height: u64, hash: [u8; 32]) -> Result<(), Box<dyn Error>> {
        if height <= self.last_checkpoint_height {
            return Ok(());
        }
        
        info!("Creating new checkpoint at height {}", height);
        
        // Store the checkpoint
        if let Err(e) = self.storage.store_checkpoint(height, hash) {
            error!("Failed to store checkpoint: {}", e);
            return Ok(());
        }
        
        // Update last checkpoint height
        self.last_checkpoint_height = height;
        
        // Broadcast the checkpoint to peers
        let checkpoint_msg = Message::CheckpointAnnouncement {
            height,
            hash,
        };
        
        self.command_sender
            .send(NetworkCommand::BroadcastMessage(checkpoint_msg))
            .await?;
            
        info!("Checkpoint at height {} broadcast to peers", height);
        
        Ok(())
    }
    
    /// Handle a checkpoint announcement from a peer
    pub async fn handle_checkpoint_announcement(&mut self, peer_id: PeerId, height: u64, hash: [u8; 32]) -> Result<(), Box<dyn Error>> {
        // If this is a checkpoint we already have, ignore it
        if height <= self.last_checkpoint_height {
            return Ok(());
        }
        
        // Add to checkpoint proposals
        let peer_entry = self.checkpoint_proposals
            .entry(height)
            .or_insert_with(HashMap::new)
            .entry(hash)
            .or_insert_with(HashSet::new);
            
        peer_entry.insert(peer_id);
        
        // Check if we have enough peers agreeing on this checkpoint
        if peer_entry.len() >= MIN_CHECKPOINT_PEERS {
            info!("Checkpoint at height {} confirmed by {} peers", height, peer_entry.len());
            
            // Store the checkpoint
            if let Err(e) = self.storage.store_checkpoint(height, hash) {
                error!("Failed to store checkpoint: {}", e);
                return Ok(());
            }
            
            // Update our last checkpoint height
            if height > self.last_checkpoint_height {
                self.last_checkpoint_height = height;
            }
            
            // Clean up proposals for this height
            self.checkpoint_proposals.remove(&height);
        }
        
        Ok(())
    }

    /// Update the sync target height if we discover a higher chain tip
    fn update_sync_target(&mut self, new_target: u64) {
        match &mut self.sync_state {
            SyncState::HeaderSync { target_height, .. } => {
                if new_target > *target_height {
                    info!("Updating sync target height from {} to {}", *target_height, new_target);
                    *target_height = new_target;
                }
            },
            SyncState::BlockSync { target_height, .. } => {
                if new_target > *target_height {
                    info!("Updating sync target height from {} to {}", *target_height, new_target);
                    *target_height = new_target;
                }
            },
            SyncState::IBDWait { target_height } => {
                if new_target > *target_height {
                    *target_height = new_target;
                }
            },
            _ => {}
        }
    }

    /// Update our knowledge of peer heights
    fn update_peer_height(&mut self, peer_id: PeerId, height: u64) {
        // Check if we already have this peer
        for (existing_peer, existing_height) in self.best_peers.iter_mut() {
            if *existing_peer == peer_id {
                if height > *existing_height {
                    *existing_height = height;
                }
                return;
            }
        }
        
        // Add new peer
        self.best_peers.push((peer_id, height));
        
        // Sort peers by height (descending)
        self.best_peers.sort_by(|a, b| b.1.cmp(&a.1));
    }

    /// Get the best peer for requesting data (highest height)
    fn get_best_peer(&self) -> Option<(PeerId, u64)> {
        self.best_peers.first().copied()
    }

    /// Get the best peer excluding specified peers
    fn get_best_peer_excluding(&self, exclude: &[PeerId]) -> Option<(PeerId, u64)> {
        self.best_peers.iter()
            .filter(|(peer_id, _)| !exclude.contains(peer_id))
            .next()
            .copied()
    }

    /// Get header at specific height (from pending headers)
    fn get_header_at_height(&self, height: u64) -> Option<BlockHeader> {
        if height <= self.height {
            // Get from storage for blocks we already have
            match self.storage.get_block_header_at_height(height) {
                Ok(Some(header)) => Some(header),
                _ => None,
            }
        } else {
            let index = (height - self.height - 1) as usize;
            self.pending_headers.get(index).cloned()
        }
    }

    /// Find block height by hash
    fn find_height_by_hash(&self, hash: &[u8; 32]) -> Option<u64> {
        // Check if it's in our pending headers
        for (i, header) in self.pending_headers.iter().enumerate() {
            if &header.hash() == hash {
                return Some(self.height + 1 + i as u64);
            }
        }
        
        // Check in storage
        if let Ok(Some(height)) = self.storage.get_block_height_by_hash(hash) {
            return Some(height);
        }
        
        None
    }
    
    /// Get the hash of our current chain tip
    fn get_tip_hash(&self) -> [u8; 32] {
        match self.storage.get_block_hash_at_height(self.height) {
            Ok(Some(hash)) => hash,
            _ => [0; 32], // Should never happen for a properly initialized chain
        }
    }

    /// Get sync progress percentage
    pub fn get_sync_progress(&self) -> f64 {
        match self.sync_state {
            SyncState::Idle => 100.0,
            SyncState::HeaderSync { current_height, target_height, .. } => {
                if target_height <= self.height {
                    100.0
                } else {
                    let progress = (current_height - self.height) as f64 / 
                                   (target_height - self.height) as f64;
                    (progress * 50.0).min(50.0) // Headers sync is first 50%
                }
            },
            SyncState::BlockSync { current_height, target_height, .. } => {
                if target_height <= self.height {
                    100.0
                } else {
                    let progress = (current_height - self.height) as f64 / 
                                   (target_height - self.height) as f64;
                    50.0 + (progress * 50.0).min(50.0) // Block sync is second 50%
                }
            },
            SyncState::IBDWait { .. } => 0.0,
        }
    }
    
    /// Get the current sync state
    pub fn get_sync_state(&self) -> SyncState {
        self.sync_state.clone()
    }
    
    /// Check if we're currently syncing
    pub fn is_syncing(&self) -> bool {
        !matches!(self.sync_state, SyncState::Idle)
    }
    
    /// Get statistics for current sync session
    pub fn get_sync_stats(&self) -> SyncStats {
        SyncStats {
            current_height: self.height,
            target_height: match self.sync_state {
                SyncState::Idle => self.height,
                SyncState::HeaderSync { target_height, .. } => target_height,
                SyncState::BlockSync { target_height, .. } => target_height,
                SyncState::IBDWait { target_height } => target_height,
            },
            download_rate: self.calculate_download_rate(),
            estimated_remaining_time: self.estimate_remaining_time(),
            pending_headers: self.pending_headers.len(),
            blocks_in_flight: self.blocks_in_flight.len(),
            pending_blocks: self.pending_blocks.len(),
            peer_count: self.best_peers.len(),
        }
    }
    
    /// Calculate the current download rate (blocks per second)
    fn calculate_download_rate(&self) -> f64 {
        match &self.sync_state {
            SyncState::Idle => 0.0,
            SyncState::HeaderSync { start_time, current_height, .. } => {
                let elapsed = start_time.elapsed().as_secs_f64();
                if elapsed > 0.0 {
                    (current_height - self.height) as f64 / elapsed
                } else {
                    0.0
                }
            },
            SyncState::BlockSync { start_time, current_height, .. } => {
                let elapsed = start_time.elapsed().as_secs_f64();
                if elapsed > 0.0 {
                    (current_height - self.height) as f64 / elapsed
                } else {
                    0.0
                }
            },
            SyncState::IBDWait { .. } => 0.0,
        }
    }
    
    /// Estimate remaining sync time in seconds
    fn estimate_remaining_time(&self) -> u64 {
        match &self.sync_state {
            SyncState::Idle => 0,
            SyncState::HeaderSync { current_height, target_height, .. } |
            SyncState::BlockSync { current_height, target_height, .. } => {
                let download_rate = self.calculate_download_rate();
                if download_rate > 0.0 {
                    let blocks_remaining = target_height - current_height;
                    (blocks_remaining as f64 / download_rate) as u64
                } else {
                    0
                }
            },
            SyncState::IBDWait { .. } => 0,
        }
    }

    /// Run the main sync process loop
    pub async fn run(&mut self) {
        let mut interval = tokio::time::interval(Duration::from_secs(5));
        
        loop {
            tokio::select! {
                _ = interval.tick() => {
                    // Check for timed out requests
                    if let Err(e) = self.check_timeouts().await {
                        error!("Error checking timeouts: {}", e);
                    }
                    
                    // Log sync progress
                    match self.sync_state {
                        SyncState::Idle => {},
                        _ => {
                            let progress = self.get_sync_progress();
                            let stats = self.get_sync_stats();
                            info!("Sync progress: {:.2}% - Height: {}/{} - Rate: {:.2} blocks/s - ETA: {} seconds",
                                progress, stats.current_height, stats.target_height, 
                                stats.download_rate, stats.estimated_remaining_time);
                        }
                    }
                }
            }
        }
    }
}

/// Statistics about the current sync session
#[derive(Debug, Clone)]
pub struct SyncStats {
    /// Current blockchain height
    pub current_height: u64,
    /// Target height we're syncing to
    pub target_height: u64,
    /// Download rate in blocks per second
    pub download_rate: f64,
    /// Estimated remaining time in seconds
    pub estimated_remaining_time: u64,
    /// Number of headers we've downloaded but not processed
    pub pending_headers: usize,
    /// Number of blocks currently being downloaded
    pub blocks_in_flight: usize,
    /// Number of blocks downloaded but not yet validated
    pub pending_blocks: usize,
    /// Number of peers available for sync
    pub peer_count: usize,
}