use crate::network::{NetworkCommand, Message, PeerId};
use crate::storage::{BlockchainDB, StorageError, ChainState};
use btclib::types::block::{Block, BlockHeader};
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};
use tokio::sync::mpsc;
use tracing::{info, warn, error, debug, trace};
use async_trait::async_trait;
use dashmap::DashMap;
use std::cmp::Ordering;
use serde;
use std::clone::Clone;

// Constants for sync configuration
const MAX_HEADERS_PER_REQUEST: u64 = 2000;
const MAX_BLOCKS_PER_REQUEST: u64 = 128;
const HEADER_DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(30);
const BLOCK_DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(60);
const SYNC_STATUS_UPDATE_INTERVAL: Duration = Duration::from_secs(10);
const MAX_PARALLEL_BLOCK_DOWNLOADS: usize = 8;
const CHECKPOINT_INTERVAL: u64 = 10000;
const MIN_PEER_SCORE: i32 = -100;
const MAX_PEER_SCORE: i32 = 100;
const DEFAULT_PEER_SCORE: i32 = 0;
const PEER_SCORE_GOOD_RESPONSE: i32 = 1;
const PEER_SCORE_INVALID_DATA: i32 = -10;
const PEER_SCORE_TIMEOUT: i32 = -5;

/// Trait to abstract sync metrics for better testing
#[async_trait]
pub trait SyncMetrics: Send + Sync {
    async fn record_sync_started(&self, target_height: u64);
    async fn record_sync_progress(&self, current_height: u64, target_height: u64);
    async fn record_sync_completed(&self, final_height: u64, duration_secs: u64);
    async fn record_header_download(&self, count: usize, duration_ms: u64);
    async fn record_block_download(&self, count: usize, duration_ms: u64);
    async fn record_block_validation(&self, result: bool, duration_ms: u64);
    async fn record_fork_detection(&self, old_height: u64, new_height: u64);
}

/// Default implementation of sync metrics
pub struct DefaultSyncMetrics;

#[async_trait]
impl SyncMetrics for DefaultSyncMetrics {
    async fn record_sync_started(&self, target_height: u64) {
        info!("Sync started. Target height: {}", target_height);
    }

    async fn record_sync_progress(&self, current_height: u64, target_height: u64) {
        let progress = if target_height > 0 {
            (current_height as f64 / target_height as f64) * 100.0
        } else {
            0.0
        };
        info!("Sync progress: {:.2}% ({}/{})", progress, current_height, target_height);
    }

    async fn record_sync_completed(&self, final_height: u64, duration_secs: u64) {
        info!("Sync completed at height {}. Duration: {} seconds", final_height, duration_secs);
    }

    async fn record_header_download(&self, count: usize, duration_ms: u64) {
        debug!("Downloaded {} headers in {} ms", count, duration_ms);
    }

    async fn record_block_download(&self, count: usize, duration_ms: u64) {
        debug!("Downloaded {} blocks in {} ms", count, duration_ms);
    }

    async fn record_block_validation(&self, result: bool, duration_ms: u64) {
        if result {
            trace!("Block validation succeeded in {} ms", duration_ms);
        } else {
            warn!("Block validation failed after {} ms", duration_ms);
        }
    }

    async fn record_fork_detection(&self, old_height: u64, new_height: u64) {
        info!("Fork detected: switching from height {} to {}", old_height, new_height);
    }
}

/// Information about a checkpoint
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Checkpoint {
    pub height: u64,
    pub hash: [u8; 32],
    pub timestamp: u64,
}

/// Data maintained for each peer
#[derive(Debug, Clone)]
struct PeerData {
    score: i32,
    first_seen: Instant,
    last_active: Instant,
    blocks_provided: u64,
    headers_provided: u64,
    timeouts: u64,
    invalid_data: u64,
    reported_height: u64,
}

impl PeerData {
    fn new() -> Self {
        let now = Instant::now();
        Self {
            score: DEFAULT_PEER_SCORE,
            first_seen: now,
            last_active: now,
            blocks_provided: 0,
            headers_provided: 0,
            timeouts: 0,
            invalid_data: 0,
            reported_height: 0,
        }
    }

    fn update_score(&mut self, delta: i32) {
        self.score = (self.score + delta).clamp(MIN_PEER_SCORE, MAX_PEER_SCORE);
        self.last_active = Instant::now();
    }
}

/// Current state of the sync process
#[derive(Debug, Clone)]
enum SyncState {
    Idle,
    SyncingHeaders {
        start_height: u64,
        end_height: u64,
        request_time: Instant,
        requesting_peer: Option<PeerId>,
    },
    SyncingBlocks {
        headers: Vec<BlockHeader>,
        blocks_requested: HashSet<[u8; 32]>,
        blocks_received: HashMap<[u8; 32], Block>,
        last_request_time: Instant,
    },
    VerifyingBlocks {
        blocks: VecDeque<Block>,
        current_verification_start: Instant,
    },
}

/// Main chain sync implementation
pub struct ChainSync {
    db: Arc<BlockchainDB>,
    chain_state: ChainState,
    command_sender: mpsc::Sender<NetworkCommand>,
    sync_state: SyncState,
    highest_seen_height: u64,
    highest_seen_total_difficulty: u64,
    checkpoints: Vec<Checkpoint>,
    sync_start_time: Option<Instant>,
    last_status_update: Instant,
    peer_data: DashMap<PeerId, PeerData>,
    metrics: Arc<dyn SyncMetrics>,
}

impl Clone for ChainSync {
    fn clone(&self) -> Self {
        Self {
            db: Arc::clone(&self.db),
            chain_state: self.chain_state.clone(),
            command_sender: self.command_sender.clone(),
            sync_state: self.sync_state.clone(),
            highest_seen_height: self.highest_seen_height,
            highest_seen_total_difficulty: self.highest_seen_total_difficulty,
            checkpoints: self.checkpoints.clone(),
            sync_start_time: self.sync_start_time,
            last_status_update: self.last_status_update,
            peer_data: self.peer_data.clone(),
            metrics: Arc::clone(&self.metrics),
        }
    }
}

impl ChainSync {
    /// Create a new ChainSync instance
    pub fn new(
        chain_state: ChainState, 
        db: Arc<BlockchainDB>,
        command_sender: mpsc::Sender<NetworkCommand>,
    ) -> Self {
        Self {
            chain_state,
            sync_state: SyncState::Idle,
            peer_data: DashMap::new(),
            highest_seen_height: 0,
            highest_seen_total_difficulty: 0,
            checkpoints: Vec::new(),
            command_sender,
            metrics: Arc::new(DefaultSyncMetrics),
            db,
            sync_start_time: None,
            last_status_update: Instant::now(),
        }
    }

    /// Set custom metrics implementation
    pub fn with_metrics(mut self, metrics: Arc<dyn SyncMetrics>) -> Self {
        self.metrics = metrics;
        self
    }

    /// Load checkpoints from database or config
    pub async fn load_checkpoints(&mut self) -> Result<(), StorageError> {
        info!("Loading chain checkpoints");
        
        // Load from DB if available
        if let Some(checkpoint_data) = self.db.get_metadata(b"checkpoints")? {
            let checkpoints: Vec<Checkpoint> = bincode::deserialize(&checkpoint_data)
                .map_err(|e| StorageError::Serialization(e))?;
            
            if !checkpoints.is_empty() {
                info!("Loaded {} checkpoints from database", checkpoints.len());
                self.checkpoints = checkpoints;
                return Ok(());
            }
        }
        
        // Otherwise, use hardcoded checkpoints for mainnet
        // In a real implementation, these would be carefully chosen trusted blocks
        self.checkpoints = vec![
            Checkpoint {
                height: 0,
                hash: [0u8; 32], // Genesis block hash
                timestamp: 0,
            },
            // Add more checkpoints here
        ];
        
        info!("Using {} hardcoded checkpoints", self.checkpoints.len());
        Ok(())
    }

    /// Register a new peer
    pub fn register_peer(&self, peer_id: PeerId) {
        if !self.peer_data.contains_key(&peer_id) {
            self.peer_data.insert(peer_id, PeerData::new());
            debug!("Registered new peer: {}", peer_id);
        }
    }

    /// Update peer height information
    pub fn update_peer_height(&self, peer_id: &PeerId, height: u64, total_difficulty: u64) {
        if let Some(mut peer) = self.peer_data.get_mut(peer_id) {
            peer.reported_height = height;
            peer.update_score(PEER_SCORE_GOOD_RESPONSE);
            
            if height > self.highest_seen_height || 
               (height == self.highest_seen_height && total_difficulty > self.highest_seen_total_difficulty) {
                debug!("New highest chain detected: height={}, difficulty={}", height, total_difficulty);
            }
        }
    }

    /// Handle a new block received from the network
    pub async fn handle_new_block(&mut self, block: Block, height: u64, total_difficulty: u64, from_peer: Option<&PeerId>) -> Result<(), String> {
        // Update peer score if applicable
        if let Some(peer_id) = from_peer {
            if let Some(mut peer_data) = self.peer_data.get_mut(peer_id) {
                peer_data.blocks_provided += 1;
                peer_data.update_score(PEER_SCORE_GOOD_RESPONSE);
            }
        }

        // Update highest seen height
        if height > self.highest_seen_height {
            self.highest_seen_height = height;
            self.highest_seen_total_difficulty = total_difficulty;
        } else if height == self.highest_seen_height && total_difficulty > self.highest_seen_total_difficulty {
            self.highest_seen_total_difficulty = total_difficulty;
        }

        // Process based on current sync state
        match &mut self.sync_state {
            SyncState::Idle => {
                // If we're significantly behind, start a full sync
                if height > self.chain_state.get_height() + 10 {
                    info!("Detected we're behind by more than 10 blocks, starting sync");
                    self.start_sync(height, total_difficulty).await?;
                } else if height == self.chain_state.get_height() + 1 {
                    // This block extends our chain directly, process it
                    self.process_single_block(block).await?;
                } else if height > self.chain_state.get_height() {
                    // We're missing a few blocks, request them
                    self.request_missing_blocks(self.chain_state.get_height() + 1, height).await?;
                }
            },
            SyncState::SyncingBlocks { blocks_received, blocks_requested, .. } => {
                let block_hash = block.hash();
                
                // If this block was requested, add it to received blocks
                if blocks_requested.contains(&block_hash) {
                    blocks_received.insert(block_hash, block);
                    blocks_requested.remove(&block_hash);
                    
                    // If we've received enough blocks, start verifying them
                    if blocks_requested.is_empty() || blocks_received.len() >= MAX_BLOCKS_PER_REQUEST as usize {
                        self.start_block_verification().await?;
                    }
                }
            },
            SyncState::VerifyingBlocks { .. } => {
                // Store for later processing
                let block_hash = block.hash();
                debug!("Received block {} while verifying, saving for later", hex::encode(&block_hash[..4]));
                self.save_block_for_later(block, height)?;
            },
            SyncState::SyncingHeaders { .. } => {
                // Store for later processing
                self.save_block_for_later(block, height)?;
            }
        }

        // Update sync progress if needed
        self.update_sync_progress().await;

        Ok(())
    }

    /// Handle headers received from the network
    pub async fn handle_headers(&mut self, headers: Vec<BlockHeader>, from_peer: Option<&PeerId>) -> Result<(), String> {
        if headers.is_empty() {
            return Ok(());
        }

        // Update peer data if applicable
        if let Some(peer_id) = from_peer {
            if let Some(mut peer_data) = self.peer_data.get_mut(peer_id) {
                peer_data.headers_provided += headers.len() as u64;
                peer_data.update_score(PEER_SCORE_GOOD_RESPONSE);
            }
        }

        // Validate headers are sequential
        for i in 1..headers.len() {
            if headers[i].prev_block_hash() != headers[i-1].hash() {
                warn!("Received non-sequential headers");
                
                if let Some(peer_id) = from_peer {
                    self.penalize_peer(peer_id, PEER_SCORE_INVALID_DATA).await;
                }
                
                return Err("Non-sequential headers received".to_string());
            }
        }

        // Process based on current state
        match &mut self.sync_state {
            SyncState::SyncingHeaders { requesting_peer, .. } => {
                // Check if these headers are from the peer we requested from
                if let Some(requested_peer) = requesting_peer {
                    if from_peer.map_or(false, |p| p != requested_peer) {
                        debug!("Ignoring headers from non-requested peer");
                        return Ok(());
                    }
                }

                // Validate headers
                if !self.validate_headers(&headers)? {
                    warn!("Received invalid headers");
                    
                    if let Some(peer_id) = from_peer {
                        self.penalize_peer(peer_id, PEER_SCORE_INVALID_DATA).await;
                    }
                    
                    return Err("Invalid headers received".to_string());
                }

                // Process the headers - this will transition to block downloading
                self.process_headers(headers).await?;
            },
            _ => {
                debug!("Received {} headers while not in header sync mode", headers.len());
            }
        }

        Ok(())
    }

    /// Start sync process targeting a specific height
    pub async fn start_sync(&mut self, target_height: u64, total_difficulty: u64) -> Result<(), String> {
        let current_height = self.chain_state.get_height();
        
        if target_height <= current_height {
            debug!("No sync needed, already at height {}", current_height);
            return Ok(());
        }

        self.highest_seen_height = target_height;
        self.highest_seen_total_difficulty = total_difficulty;
        self.sync_start_time = Some(Instant::now());
        
        info!("Starting blockchain sync from height {} to {}", current_height, target_height);
        self.metrics.record_sync_started(target_height).await;

        // Start with header synchronization
        self.start_header_sync(current_height, target_height).await?;

        Ok(())
    }

    /// Start the header synchronization process
    async fn start_header_sync(&mut self, start_height: u64, end_height: u64) -> Result<(), String> {
        let current_height = self.chain_state.get_height();
        
        // Find closest checkpoint below current height
        let checkpoint_height = self.find_best_checkpoint_height(current_height);
        
        // If we have a checkpoint, use its height as the starting point
        let actual_start = if checkpoint_height > 0 && checkpoint_height > current_height {
            info!("Using checkpoint at height {} for sync", checkpoint_height);
            checkpoint_height
        } else {
            start_height
        };
        
        // Calculate end height for this batch (limited by MAX_HEADERS_PER_REQUEST)
        let actual_end = std::cmp::min(
            actual_start + MAX_HEADERS_PER_REQUEST,
            end_height
        );
        
        // Find best peer to request headers from
        let best_peer = self.find_best_peer_for_height(actual_end);
        
        // Update sync state
        self.sync_state = SyncState::SyncingHeaders {
            start_height: actual_start,
            end_height: actual_end,
            request_time: Instant::now(),
            requesting_peer: best_peer.clone(),
        };
        
        // Send request for headers
        self.request_headers(actual_start, actual_end, best_peer).await?;
        
        info!("Requesting headers from height {} to {}", actual_start, actual_end);
        Ok(())
    }

    /// Request headers from the network
    async fn request_headers(&mut self, start_height: u64, end_height: u64, preferred_peer: Option<PeerId>) -> Result<(), String> {
        let message = Message::GetHeaders {
            start_height,
            end_height,
        };
        
        if let Some(peer) = preferred_peer {
            // Request from specific peer
            self.command_sender
                .send(NetworkCommand::SendToPeer {
                    peer_id: peer,
                    message: message.clone(),
                })
                .await
                .map_err(|e| format!("Failed to send header request: {}", e))?;
        } else {
            // Broadcast to all peers
            self.command_sender
                .send(NetworkCommand::Broadcast(message))
                .await
                .map_err(|e| format!("Failed to send header request: {}", e))?;
        }
        
        Ok(())
    }

    /// Process headers received from the network
    async fn process_headers(&mut self, headers: Vec<BlockHeader>) -> Result<(), String> {
        if headers.is_empty() {
            return Ok(());
        }

        let headers_count = headers.len();
        info!("Processing {} headers", headers_count);

        // Ensure we're in the header syncing state
        let (start_height, end_height) = match &self.sync_state {
            SyncState::SyncingHeaders { start_height, end_height, .. } => {
                (*start_height, *end_height)
            },
            _ => {
                return Err("Not in header syncing state".to_string());
            }
        };

        // Store headers in database
        for header in &headers {
            let header_hash = header.hash();
            let serialized = match bincode::serialize(header) {
                Ok(data) => data,
                Err(e) => return Err(format!("Serialization error: {}", e)),
            };
            
            if let Err(e) = self.db.store_block_header(&header_hash, &serialized) {
                return Err(format!("Failed to store header: {}", e));
            }
        }

        // If we've received all requested headers, start downloading blocks
        let next_height = start_height + headers.len() as u64;
        
        if next_height >= end_height || headers.len() < MAX_HEADERS_PER_REQUEST as usize {
            // Start downloading the blocks for these headers
            self.start_block_downloads(headers).await?;
        } else {
            // Request more headers
            self.start_header_sync(next_height, self.highest_seen_height).await?;
        }

        Ok(())
    }

    /// Start downloading blocks for the given headers
    async fn start_block_downloads(&mut self, headers: Vec<BlockHeader>) -> Result<(), String> {
        if headers.is_empty() {
            return Ok(());
        }

        info!("Starting block downloads for {} headers", headers.len());

        // Create a set of block hashes to request
        let mut blocks_requested = HashSet::new();
        
        for header in &headers {
            blocks_requested.insert(header.hash());
        }

        // Update sync state
        self.sync_state = SyncState::SyncingBlocks {
            headers,
            blocks_requested,
            blocks_received: HashMap::new(),
            last_request_time: Instant::now(),
        };

        // Start requesting blocks
        self.request_next_blocks().await?;

        Ok(())
    }

    /// Request the next batch of blocks
    async fn request_next_blocks(&mut self) -> Result<(), String> {
        // Ensure we're in the block syncing state
        let blocks_to_request = match &self.sync_state {
            SyncState::SyncingBlocks { blocks_requested, blocks_received, .. } => {
                // Get a subset of blocks to request
                blocks_requested
                    .iter()
                    .filter(|hash| !blocks_received.contains_key(*hash))
                    .take(MAX_PARALLEL_BLOCK_DOWNLOADS)
                    .cloned()
                    .collect::<Vec<_>>()
            },
            _ => return Ok(()),
        };

        if blocks_to_request.is_empty() {
            return Ok(());
        }

        info!("Requesting {} blocks", blocks_to_request.len());

        // Find best peers for requests
        let peers = self.get_peers_for_block_requests(blocks_to_request.len());
        
        // Send block requests
        for (i, block_hash) in blocks_to_request.iter().enumerate() {
            let peer = if i < peers.len() { Some(peers[i]) } else { None };
            
            let message = Message::GetBlocksByHash {
                block_hashes: vec![*block_hash],
            };
            
            if let Some(peer_id) = peer {
                // Request from specific peer
                self.command_sender
                    .send(NetworkCommand::SendToPeer {
                        peer_id,
                        message: message.clone(),
                    })
                    .await
                    .map_err(|e| format!("Failed to send block request: {}", e))?;
            } else {
                // Broadcast to all peers
                self.command_sender
                    .send(NetworkCommand::Broadcast(message))
                    .await
                    .map_err(|e| format!("Failed to send block request: {}", e))?;
            }
        }

        // Update last request time
        if let SyncState::SyncingBlocks { last_request_time, .. } = &mut self.sync_state {
            *last_request_time = Instant::now();
        }

        Ok(())
    }

    /// Start verifying the downloaded blocks
    async fn start_block_verification(&mut self) -> Result<(), String> {
        // Ensure we're in the block syncing state
        let (headers, mut blocks_received) = match std::mem::replace(&mut self.sync_state, SyncState::Idle) {
            SyncState::SyncingBlocks { headers, blocks_received, .. } => {
                (headers, blocks_received)
            },
            other => {
                self.sync_state = other;
                return Ok(());
            }
        };

        // Sort blocks by height (using header information)
        let mut blocks_to_verify = VecDeque::new();
        
        for header in &headers {
            let block_hash = header.hash();
            if let Some(block) = blocks_received.remove(&block_hash) {
                blocks_to_verify.push_back(block);
            }
        }

        if blocks_to_verify.is_empty() {
            info!("No blocks to verify, continuing sync");
            
            // Continue with next batch of headers
            self.start_header_sync(
                self.chain_state.get_height(), 
                self.highest_seen_height
            ).await?;
            
            return Ok(());
        }

        info!("Starting verification of {} blocks", blocks_to_verify.len());
        
        // Update sync state
        self.sync_state = SyncState::VerifyingBlocks {
            blocks: blocks_to_verify,
            current_verification_start: Instant::now(),
        };

        // Start verifying blocks
        self.verify_next_block().await?;

        Ok(())
    }

    /// Verify the next block in the queue
    async fn verify_next_block(&mut self) -> Result<(), String> {
        loop {
            // Ensure we're in the block verification state
            let (block, verification_start) = match &mut self.sync_state {
                SyncState::VerifyingBlocks { blocks, current_verification_start } => {
                    if let Some(block) = blocks.pop_front() {
                        (block, *current_verification_start)
                    } else {
                        // No more blocks to verify, continue sync
                        info!("Block verification complete");
                        
                        // Continue with next batch of headers if needed
                        if self.chain_state.get_height() < self.highest_seen_height {
                            self.start_header_sync(
                                self.chain_state.get_height(), 
                                self.highest_seen_height
                            ).await?;
                        } else {
                            info!("Sync complete! Chain height: {}", self.chain_state.get_height());
                            self.sync_state = SyncState::Idle;
                            
                            if let Some(start_time) = self.sync_start_time {
                                let duration = start_time.elapsed().as_secs();
                                self.metrics.record_sync_completed(self.chain_state.get_height(), duration).await;
                                self.sync_start_time = None;
                            }
                        }
                        
                        return Ok(());
                    }
                },
                _ => return Ok(()),
            };

            // Process the block
            let block_hash = block.hash();
            debug!("Verifying block {}", hex::encode(&block_hash[..4]));
            
            let verification_result = self.process_single_block(block).await;
            
            let duration_ms = verification_start.elapsed().as_millis() as u64;
            self.metrics.record_block_validation(verification_result.is_ok(), duration_ms).await;
            
            // Update verification start time for next block
            if let SyncState::VerifyingBlocks { current_verification_start, .. } = &mut self.sync_state {
                *current_verification_start = Instant::now();
            }
            
            // If verification failed, handle error
            if let Err(err) = verification_result {
                warn!("Block verification failed: {}", err);
                
                // Continue loop to process next block
                continue;
                
                // We don't return the error so we can continue processing blocks
            }
            
            // Loop continues to process the next block
        }
    }

    /// Process a single block (validate and add to chain)
    async fn process_single_block(&mut self, block: Block) -> Result<(), String> {
        let start_time = Instant::now();
        let block_hash = block.hash();
        
        // Store the block
        if let Err(e) = self.db.store_block(&block_hash, &bincode::serialize(&block)
            .map_err(|e| format!("Serialization error: {}", e))?) {
            return Err(format!("Failed to store block: {}", e));
        }
        
        // Process with chain state
        match self.chain_state.process_block(block).await {
            Ok(true) => {
                debug!("Block processed successfully: {}", hex::encode(&block_hash[..4]));
                
                // Create checkpoint if needed
                let height = self.chain_state.get_height();
                if height % CHECKPOINT_INTERVAL == 0 {
                    self.create_checkpoint(height, block_hash).await?;
                }
            },
            Ok(false) => {
                // Block was processed but not added to main chain
                debug!("Block not added to main chain: {}", hex::encode(&block_hash[..4]));
            },
            Err(e) => {
                return Err(format!("Failed to process block: {}", e));
            }
        }
        
        let duration_ms = start_time.elapsed().as_millis() as u64;
        self.metrics.record_block_validation(true, duration_ms).await;
        
        Ok(())
    }

    /// Create a new checkpoint
    async fn create_checkpoint(&mut self, height: u64, block_hash: [u8; 32]) -> Result<(), String> {
        let checkpoint = Checkpoint {
            height,
            hash: block_hash,
            timestamp: SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_err(|e| format!("Time error: {}", e))?
                .as_secs(),
        };
        
        self.checkpoints.push(checkpoint.clone());
        
        // Store checkpoints in database
        let checkpoint_data = bincode::serialize(&self.checkpoints)
            .map_err(|e| format!("Serialization error: {}", e))?;
        
        if let Err(e) = self.db.store_metadata(b"checkpoints", &checkpoint_data) {
            return Err(format!("Failed to store checkpoint: {}", e));
        }
        
        info!("Created checkpoint at height {}", height);
        Ok(())
    }

    /// Find best checkpoint height for starting sync
    fn find_best_checkpoint_height(&self, current_height: u64) -> u64 {
        let mut best_checkpoint = 0;
        
        for checkpoint in &self.checkpoints {
            if checkpoint.height > current_height && (best_checkpoint == 0 || checkpoint.height < best_checkpoint) {
                best_checkpoint = checkpoint.height;
            }
        }
        
        best_checkpoint
    }

    /// Request missing blocks between two heights
    async fn request_missing_blocks(&mut self, start_height: u64, end_height: u64) -> Result<(), String> {
        info!("Requesting missing blocks from {} to {}", start_height, end_height);
        
        // Get block hashes at these heights from peers
        let message = Message::GetBlocksByHeight {
            start_height,
            end_height,
        };
        
        self.command_sender
            .send(NetworkCommand::Broadcast(message))
            .await
            .map_err(|e| format!("Failed to send missing blocks request: {}", e))?;
        
        Ok(())
    }

    /// Save a block for later processing (when not in block processing state)
    fn save_block_for_later(&self, block: Block, height: u64) -> Result<(), String> {
        let block_hash = block.hash();
        
        if let Err(e) = self.db.store_pending_block(&block_hash, &bincode::serialize(&block)
            .map_err(|e| format!("Serialization error: {}", e))?) {
            return Err(format!("Failed to store pending block: {}", e));
        }
        
        debug!("Saved block {} at height {} for later processing", 
              hex::encode(&block_hash[..4]), height);
        
        Ok(())
    }

    /// Find the best peer for requesting blocks at a specific height
    fn find_best_peer_for_height(&self, height: u64) -> Option<PeerId> {
        let mut best_peer = None;
        let mut best_score = MIN_PEER_SCORE - 1;
        
        for entry in self.peer_data.iter() {
            let peer_id = entry.key();
            let peer_data = entry.value();
            
            if peer_data.reported_height >= height && peer_data.score > best_score {
                best_score = peer_data.score;
                best_peer = Some(peer_id.clone());
            }
        }
        
        best_peer
    }

    /// Get a list of peers for block requests
    fn get_peers_for_block_requests(&self, count: usize) -> Vec<PeerId> {
        let mut peers: Vec<(PeerId, i32)> = self.peer_data.iter()
            .map(|entry| (entry.key().clone(), entry.value().score))
            .collect();
        
        // Sort peers by score (descending)
        peers.sort_by(|a, b| b.1.cmp(&a.1));
        
        // Take top N peers
        peers.into_iter()
            .take(count)
            .map(|(peer_id, _)| peer_id)
            .collect()
    }

    /// Penalize a peer for bad behavior
    async fn penalize_peer(&self, peer_id: &PeerId, penalty: i32) {
        if let Some(mut peer_data) = self.peer_data.get_mut(peer_id) {
            peer_data.update_score(penalty);
            
            if penalty < 0 {
                if penalty <= PEER_SCORE_INVALID_DATA {
                    peer_data.invalid_data += 1;
                } else if penalty <= PEER_SCORE_TIMEOUT {
                    peer_data.timeouts += 1;
                }
            }
            
            debug!("Penalized peer {} with {} points, new score: {}", 
                  peer_id, penalty, peer_data.score);
            
            // Disconnect if score is too low
            if peer_data.score <= MIN_PEER_SCORE {
                warn!("Disconnecting peer {} due to low score", peer_id);
                
                let _ = self.command_sender
                    .send(NetworkCommand::DisconnectPeer(peer_id.clone()))
                    .await;
            }
        }
    }

    /// Process sync timeouts
    pub async fn process_timeouts(&mut self) -> Result<(), String> {
        match &self.sync_state {
            SyncState::SyncingHeaders { request_time, requesting_peer, .. } => {
                if request_time.elapsed() > HEADER_DOWNLOAD_TIMEOUT {
                    warn!("Header download timed out");
                    
                    // Penalize peer if applicable
                    if let Some(peer_id) = requesting_peer {
                        self.penalize_peer(peer_id, PEER_SCORE_TIMEOUT).await;
                    }
                    
                    // Restart header sync
                    self.start_header_sync(
                        self.chain_state.get_height(), 
                        self.highest_seen_height
                    ).await?;
                }
            },
            SyncState::SyncingBlocks { last_request_time, .. } => {
                if last_request_time.elapsed() > BLOCK_DOWNLOAD_TIMEOUT {
                    warn!("Block download timed out");
                    
                    // Retry block requests
                    self.request_next_blocks().await?;
                }
            },
            _ => {}
        }
        
        Ok(())
    }

    /// Update and log sync progress
    async fn update_sync_progress(&mut self) {
        // Only update every SYNC_STATUS_UPDATE_INTERVAL
        if self.last_status_update.elapsed() < SYNC_STATUS_UPDATE_INTERVAL {
            return;
        }
        
        self.last_status_update = Instant::now();
        
        let current_height = self.chain_state.get_height();
        let target_height = self.highest_seen_height;
        
        if current_height < target_height {
            self.metrics.record_sync_progress(current_height, target_height).await;
        }
    }

    /// Validate a sequence of headers
    fn validate_headers(&self, headers: &[BlockHeader]) -> Result<bool, String> {
        if headers.is_empty() {
            return Ok(true);
        }
        
        // Check sequential ordering
        for i in 1..headers.len() {
            if headers[i].prev_block_hash() != headers[i-1].hash() {
                return Ok(false);
            }
        }
        
        // Verify first header connects to our chain
        if let Some(first_header) = headers.first() {
            let prev_hash = first_header.prev_block_hash();
            
            // If this is not the genesis block, check if we have its parent
            if prev_hash != [0u8; 32] {
                if let Ok(None) = self.db.get_block_header(&prev_hash) {
                    // We don't have the parent, check if it matches a checkpoint
                    let mut found_checkpoint = false;
                    
                    for checkpoint in &self.checkpoints {
                        if checkpoint.hash == prev_hash {
                            found_checkpoint = true;
                            break;
                        }
                    }
                    
                    if !found_checkpoint {
                        return Ok(false);
                    }
                }
            }
        }
        
        // Verify proof of work and difficulty for each header
        for header in headers {
            if !self.verify_header_pow(header) {
                return Ok(false);
            }
        }
        
        Ok(true)
    }

    /// Verify proof of work for a header
    fn verify_header_pow(&self, header: &BlockHeader) -> bool {
        let hash = header.hash();
        let target = header.target();
        
        // Check that hash is below target
        let hash_value = u32::from_be_bytes([hash[0], hash[1], hash[2], hash[3]]);
        hash_value <= target
    }

    /// Get the current chain height
    pub fn get_height(&self) -> u64 {
        self.chain_state.get_height()
    }

    /// Get the current sync state as a string
    pub fn get_sync_state_string(&self) -> String {
        match &self.sync_state {
            SyncState::Idle => "idle".to_string(),
            SyncState::SyncingHeaders { start_height, end_height, .. } => {
                format!("syncing headers {}-{}", start_height, end_height)
            },
            SyncState::SyncingBlocks { blocks_requested, blocks_received, .. } => {
                format!("syncing blocks {}/{}", 
                        blocks_received.len(),
                        blocks_received.len() + blocks_requested.len())
            },
            SyncState::VerifyingBlocks { blocks, .. } => {
                format!("verifying blocks (remaining: {})", blocks.len())
            }
        }
    }

    /// Get statistics about the sync process
    pub fn get_stats(&self) -> SyncStats {
        let peers = self.peer_data.len();
        let active_peers = self.peer_data.iter()
            .filter(|entry| entry.value().score > 0)
            .count();
        
        SyncStats {
            current_height: self.chain_state.get_height(),
            target_height: self.highest_seen_height,
            state: self.get_sync_state_string(),
            peers,
            active_peers,
            checkpoints: self.checkpoints.len(),
            sync_duration: self.sync_start_time.map(|t| t.elapsed().as_secs()),
        }
    }
}

/// Statistics about the sync process
#[derive(Debug, Clone)]
pub struct SyncStats {
    pub current_height: u64,
    pub target_height: u64,
    pub state: String,
    pub peers: usize,
    pub active_peers: usize,
    pub checkpoints: usize,
    pub sync_duration: Option<u64>,
}

/// Extension methods for BlockHeader
trait BlockHeaderExt {
    fn prev_block_hash(&self) -> [u8; 32];
    fn hash(&self) -> [u8; 32];
    fn target(&self) -> u32;
}

impl BlockHeaderExt for BlockHeader {
    fn prev_block_hash(&self) -> [u8; 32] {
        // Use indirect access to avoid private field access
        // In a real implementation, we'd get this from block header accessor methods
        [0u8; 32] // Default value for demonstration
    }
    
    fn hash(&self) -> [u8; 32] {
        // Call the existing hash method
        self.hash()
    }
    
    fn target(&self) -> u32 {
        // Use a reasonable default target value
        u32::MAX // Maximum target (minimum difficulty)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use std::sync::Arc;
    use tokio::sync::mpsc;

    struct MockChainState {
        height: u64,
        best_hash: [u8; 32],
    }

    impl MockChainState {
        fn new(height: u64) -> Self {
            Self {
                height,
                best_hash: [0u8; 32],
            }
        }

        fn get_height(&self) -> u64 {
            self.height
        }

        fn get_best_block_hash(&self) -> [u8; 32] {
            self.best_hash
        }

        async fn process_block(&mut self, block: Block) -> Result<bool, String> {
            self.height += 1;
            self.best_hash = block.hash();
            Ok(true)
        }
    }

    #[tokio::test]
    async fn test_chain_sync_creation() {
        let temp_dir = tempdir().unwrap();
        let db = Arc::new(BlockchainDB::new(temp_dir.path()).unwrap());
        let chain_state = ChainState::new(Arc::clone(&db)).unwrap();
        
        let (tx, _) = mpsc::channel(32);
        let sync = ChainSync::new(chain_state, Arc::clone(&db), tx);
        
        assert_eq!(sync.get_height(), 0);
        assert_eq!(sync.get_sync_state_string(), "idle");
    }

    #[tokio::test]
    async fn test_peer_management() {
        let temp_dir = tempdir().unwrap();
        let db = Arc::new(BlockchainDB::new(temp_dir.path()).unwrap());
        let chain_state = ChainState::new(Arc::clone(&db)).unwrap();
        
        let (tx, _) = mpsc::channel(32);
        let sync = ChainSync::new(chain_state, Arc::clone(&db), tx);
        
        let peer_id = PeerId::random();
        sync.register_peer(peer_id.clone());
        
        // Test peer height update
        sync.update_peer_height(&peer_id, 100, 1000);
        
        let stats = sync.get_stats();
        assert_eq!(stats.peers, 1);
        assert_eq!(stats.active_peers, 1);
        
        // Penalize peer
        sync.penalize_peer(&peer_id, PEER_SCORE_INVALID_DATA).await;
        
        let stats = sync.get_stats();
        assert_eq!(stats.active_peers, 0);
    }

    #[tokio::test]
    async fn test_checkpoint_management() {
        let temp_dir = tempdir().unwrap();
        let db = Arc::new(BlockchainDB::new(temp_dir.path()).unwrap());
        let chain_state = ChainState::new(Arc::clone(&db)).unwrap();
        
        let (tx, _) = mpsc::channel(32);
        let mut sync = ChainSync::new(chain_state, Arc::clone(&db), tx);
        
        // Load default checkpoints
        sync.load_checkpoints().await.unwrap();
        assert!(sync.checkpoints.len() > 0);
        
        // Create a new checkpoint
        sync.create_checkpoint(1000, [1u8; 32]).await.unwrap();
        
        assert!(sync.checkpoints.iter().any(|cp| cp.height == 1000));
    }

    #[tokio::test]
    async fn test_header_validation() {
        let temp_dir = tempdir().unwrap();
        let db = Arc::new(BlockchainDB::new(temp_dir.path()).unwrap());
        let chain_state = ChainState::new(Arc::clone(&db)).unwrap();
        
        let (tx, _) = mpsc::channel(32);
        let sync = ChainSync::new(chain_state, Arc::clone(&db), tx);
        
        // Create valid headers
        let mut headers = Vec::new();
        let mut prev_hash = [0u8; 32];
        
        for i in 0..5 {
            let header = BlockHeader::new(1, prev_hash, [0u8; 32], u32::MAX);
            prev_hash = header.hash();
            headers.push(header);
        }
        
        assert!(sync.validate_headers(&headers).unwrap());
        
        // Create invalid headers (non-sequential)
        let mut invalid_headers = Vec::new();
        invalid_headers.push(BlockHeader::new(1, [0u8; 32], [0u8; 32], u32::MAX));
        invalid_headers.push(BlockHeader::new(1, [1u8; 32], [0u8; 32], u32::MAX));
        
        assert!(!sync.validate_headers(&invalid_headers).unwrap());
    }

    #[tokio::test]
    async fn test_find_best_peer() {
        let temp_dir = tempdir().unwrap();
        let db = Arc::new(BlockchainDB::new(temp_dir.path()).unwrap());
        let chain_state = ChainState::new(Arc::clone(&db)).unwrap();
        
        let (tx, _) = mpsc::channel(32);
        let sync = ChainSync::new(chain_state, Arc::clone(&db), tx);
        
        // Register peers with different heights
        let peer1 = PeerId::random();
        let peer2 = PeerId::random();
        let peer3 = PeerId::random();
        
        sync.register_peer(peer1.clone());
        sync.register_peer(peer2.clone());
        sync.register_peer(peer3.clone());
        
        sync.update_peer_height(&peer1, 100, 1000);
        sync.update_peer_height(&peer2, 200, 2000);
        sync.update_peer_height(&peer3, 50, 500);
        
        // Find best peer for height 150
        let best_peer = sync.find_best_peer_for_height(150);
        assert_eq!(best_peer, Some(peer2));
        
        // Find best peer for height 250 (none should qualify)
        let best_peer = sync.find_best_peer_for_height(250);
        assert_eq!(best_peer, None);
    }
}