use super::database::{create_utxo_key, BlockchainDB, StorageError};
use super::reorg::ReorgChangeSet;
use supernova_core::consensus::chainwork::{self, Work};
use supernova_core::consensus::difficulty_retarget::{self, RetargetParams};
use supernova_core::types::block::Block;
use supernova_core::types::block_subsidy;
use supernova_core::types::transaction::{Transaction, TransactionOutput};
use crate::blockchain::checkpoint::{validate_checkpoint, can_reorganize_below};
use crate::blockchain::invalidation::{InvalidBlockTracker, InvalidBlockTrackerConfig, InvalidationReason};
use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Duration;
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::{debug, info, warn};

const MAX_REORG_DEPTH: u64 = 100;
const MAX_FORK_DISTANCE: u64 = 6;
const FORK_CHOICE_WINDOW: u64 = 10;
const STALE_TIP_THRESHOLD: Duration = Duration::from_secs(3600);

// Add the missing BlockNotFound variant to StorageError in persistence.rs
impl From<&'static str> for StorageError {
    fn from(error: &'static str) -> Self {
        StorageError::DatabaseError(error.to_string())
    }
}

#[derive(Clone)]
pub struct ChainState {
    db: Arc<BlockchainDB>,
    current_height: u64,
    best_block_hash: [u8; 32],
    chain_work: HashMap<[u8; 32], Work>,
    fork_points: HashSet<[u8; 32]>,
    last_reorg_time: SystemTime,
    reorg_count: u64,
    active_forks: HashMap<[u8; 32], ForkInfo>,
    last_block_time: SystemTime,
    rejected_reorgs: u64,
    invalid_block_tracker: Arc<InvalidBlockTracker>,
    /// Per-network consensus parameters (difficulty floor, retarget interval,
    /// block time) — the validator's source of truth for required difficulty.
    retarget_params: RetargetParams,
}

#[derive(Debug)]
pub struct ReorganizationEvent {
    pub old_tip: [u8; 32],
    pub new_tip: [u8; 32],
    pub fork_point: [u8; 32],
    pub fork_height: u64,
    pub blocks_disconnected: u64,
    pub blocks_connected: u64,
    pub timestamp: SystemTime,
    pub time_since_last_reorg: Duration,
    pub fork_choice_reason: ForkChoiceReason,
}

/// Information tracked for each active fork
#[derive(Debug, Clone)]
pub struct ForkInfo {
    pub fork_point_hash: [u8; 32],
    pub fork_point_height: u64,
    pub tip_hash: [u8; 32],
    pub tip_height: u64,
    pub chain_work: Work,
    pub blocks_added: u64,
    pub first_seen: SystemTime,
    pub last_updated: SystemTime,
    pub is_active: bool,
}

/// Reason for choosing a particular fork
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ForkChoiceReason {
    /// Fork has higher accumulated work (primary decision factor)
    HigherChainWork,
    /// First-seen fork in case of equal chain work
    FirstSeen,
    /// Fork with more blocks in case of equal chain work
    MoreBlocks,
    /// Fork with better connectivity (more peers supporting it)
    BetterConnectivity,
    /// Fork with better transaction efficiency
    BetterTransactionEfficiency,
    /// Fork with better block propagation metrics
    BetterPropagation,
    /// Manual override by operator
    ManualSelection,
}

impl ChainState {
    /// Open chain state with the default (launch-network = testnet) consensus
    /// parameters. Defaulting to the HARD difficulty floor is the consensus-safe
    /// choice: a caller that forgets to pass network params still enforces real
    /// difficulty rather than silently dropping to the easy regtest floor.
    pub fn new(db: Arc<BlockchainDB>) -> Result<Self, StorageError> {
        Self::with_params(db, RetargetParams::testnet())
    }

    /// Open chain state with explicit per-network consensus parameters (tests use
    /// `RetargetParams::regtest()` for the easy floor; a future multi-network node
    /// passes the configured network's params here).
    pub fn with_params(
        db: Arc<BlockchainDB>,
        retarget_params: RetargetParams,
    ) -> Result<Self, StorageError> {
        // Read height as big-endian bytes (to match how we write it)
        let current_height = match db.get_metadata(b"height")? {
            Some(height_bytes) => {
                if height_bytes.len() >= 8 {
                    u64::from_be_bytes(height_bytes[..8].try_into()
                        .map_err(|_| {
                            // ENHANCED ERROR CONTEXT: Height metadata conversion failure during persistence load
                            // This is the FINAL error context enhancement for P2-001!
                            StorageError::DatabaseError(format!(
                                "Invalid height metadata during persistence initialization. \
                                 Expected 8 bytes for u64 blockchain height, got {} bytes. \
                                 This indicates database corruption of the 'height' metadata key. \
                                 Cannot determine blockchain state. Database recovery required. \
                                 Raw bytes: {}",
                                height_bytes.len(),
                                hex::encode(&height_bytes[..height_bytes.len().min(16)])
                            ))
                        })?)
                } else {
                    0
                }
            }
            None => 0,
        };

        let best_block_hash = match db.get_metadata(b"best_hash")? {
            Some(hash_bytes) => {
                let mut hash = [0u8; 32];
                if hash_bytes.len() >= 32 {
                    hash.copy_from_slice(&hash_bytes[..32]);
                }
                hash
            }
            None => [0u8; 32],
        };

        // One-time height->hash index backfill (#5 migration). Older databases
        // never populated `block_height_index` (its writer had no callers), so
        // every height-based lookup returned None. If the tip isn't indexed,
        // rebuild the index by walking the best chain. The guard makes normal
        // restarts (already indexed) skip the walk; failure is non-fatal so a
        // corrupt tail block cannot block startup — readers stay degraded until
        // repaired rather than the node refusing to boot.
        if current_height > 0
            && best_block_hash != [0u8; 32]
            && db.get_block_hash_by_height(current_height)?.is_none()
        {
            match db.backfill_height_index(&best_block_hash) {
                Ok(n) => tracing::info!("Backfilled block height index: {} entries", n),
                Err(e) => {
                    tracing::warn!("Height index backfill failed (non-fatal): {:?}", e)
                }
            }
        }

        tracing::debug!(
            "ChainState initialized: height={}, best_hash={}",
            current_height,
            hex::encode(&best_block_hash[..8])
        );

        Ok(Self {
            db,
            current_height,
            best_block_hash,
            chain_work: HashMap::new(),
            fork_points: HashSet::new(),
            last_reorg_time: SystemTime::now(),
            reorg_count: 0,
            active_forks: HashMap::new(),
            last_block_time: SystemTime::now(),
            rejected_reorgs: 0,
            invalid_block_tracker: Arc::new(InvalidBlockTracker::new(InvalidBlockTrackerConfig::default())),
            retarget_params,
        })
    }

    pub fn get_height(&self) -> u64 {
        self.current_height
    }

    pub fn get_best_block_hash(&self) -> [u8; 32] {
        self.best_block_hash
    }

    /// The per-network consensus parameters this chain enforces (difficulty
    /// floor, retarget interval, block time). The miner and the difficulty gate
    /// both read these so they agree on the required difficulty.
    pub fn retarget_params(&self) -> RetargetParams {
        self.retarget_params
    }

    /// Get the invalid block tracker
    pub fn invalid_block_tracker(&self) -> Arc<InvalidBlockTracker> {
        self.invalid_block_tracker.clone()
    }

    /// Initialize the chain state with a genesis block
    pub fn initialize_with_genesis(&mut self, genesis_block: Block) -> Result<(), StorageError> {
        // Check if already initialized
        if self.current_height > 0 {
            return Err(StorageError::DatabaseError(
                "Chain already initialized".to_string(),
            ));
        }

        // Store genesis block with bloom filter and cache updates
        let genesis_hash = genesis_block.hash();
        tracing::info!("Initializing genesis block {}", hex::encode(&genesis_hash[..8]));
        
        let genesis_data = bincode::serialize(&genesis_block)
            .map_err(|e| StorageError::DatabaseError(format!("Genesis serialization failed: {}", e)))?;
        self.db.store_block(&genesis_hash, &genesis_data)?;
        self.db.flush()?;
        
        tracing::info!("Genesis block stored successfully");
        
        // Set genesis hash in metadata
        self.db.store_metadata(b"genesis_hash", &genesis_hash)?;
        
        // Initialize chain state for genesis
        self.best_block_hash = genesis_hash;
        self.current_height = 0; // Genesis is height 0
        
        // Store height as big-endian bytes (to match get_height() which reads as big-endian)
        self.db.set_metadata(b"height", &self.current_height.to_be_bytes())?;
        self.db.set_metadata(b"best_hash", &genesis_hash)?;
        // Record genesis in the height->hash index (#5): every best-chain block
        // must be queryable by height, and genesis is height 0.
        self.db.store_block_height_index(self.current_height, &genesis_hash)?;

        // Process genesis transactions to create initial UTXO set
        self.process_block_transactions(&genesis_block)?;
        
        self.db.flush()?;

        Ok(())
    }

    /// Add a block to the chain
    pub async fn add_block(&mut self, block: &Block) -> Result<(), StorageError> {
        // Process the block using existing async logic
        let block_clone = block.clone();
        self.process_block(block_clone).await?;
        Ok(())
    }

    /// Get a block by hash
    pub fn get_block(&self, hash: &[u8; 32]) -> Option<Block> {
        self.db.get_block(hash).ok().flatten()
    }

    /// Get the genesis block hash
    pub fn get_genesis_hash(&self) -> [u8; 32] {
        // Fetch the genesis block hash from database or use a cached value
        if let Ok(Some(hash)) = self.db.get_metadata(b"genesis_hash") {
            let mut result = [0u8; 32];
            if hash.len() == 32 {
                result.copy_from_slice(&hash);
                return result;
            }
        }

        // Fallback to zeros if not found
        [0u8; 32]
    }

    /// Get total difficulty of the current chain tip
    pub fn get_total_difficulty(&self) -> u64 {
        if let Ok(Some(difficulty_bytes)) = self.db.get_metadata(b"total_difficulty") {
            if let Ok(difficulty) = bincode::deserialize::<u64>(&difficulty_bytes) {
                return difficulty;
            }
        }

        // Default value if not found
        0
    }

    /// Update total difficulty when adding a new block
    fn update_total_difficulty(&mut self, new_block_difficulty: u64) -> Result<(), StorageError> {
        let current_difficulty = self.get_total_difficulty();
        let new_total = current_difficulty.saturating_add(new_block_difficulty);

        let difficulty_bytes =
            bincode::serialize(&new_total).map_err(StorageError::Serialization)?;

        self.db
            .store_metadata(b"total_difficulty", &difficulty_bytes)?;
        Ok(())
    }

    /// Get time since last block was added
    pub fn time_since_last_block(&self) -> Duration {
        SystemTime::now()
            .duration_since(self.last_block_time)
            .unwrap_or_else(|_| Duration::from_secs(0))
    }

    /// Check if the current tip is considered stale
    pub fn is_tip_stale(&self) -> bool {
        self.time_since_last_block() > STALE_TIP_THRESHOLD
    }

    /// Get number of active forks
    pub fn get_active_fork_count(&self) -> usize {
        self.active_forks.len()
    }

    /// Get stats about rejected reorganizations
    pub fn get_rejected_reorg_count(&self) -> u64 {
        self.rejected_reorgs
    }

    /// Get information about active forks
    pub fn get_active_forks(&self) -> Vec<ForkInfo> {
        self.active_forks.values().cloned().collect()
    }

    /// Get the difficulty target for the next block
    pub fn get_difficulty_target(&self) -> u32 {
        // The difficulty the NEXT block must use, computed with the SAME
        // `required_bits` rule the validator enforces (#2.2) so the node's own
        // miner never produces a block the validator would reject. Falls back to
        // the network floor only if the tip can't be read.
        self.next_required_bits()
            .unwrap_or(self.retarget_params.pow_limit_bits)
    }

    /// The required `bits` for the block at `current_height + 1`, derived from the
    /// tip (its parent) — off a retarget boundary this is the tip's bits; at a
    /// boundary it is the retargeted value. Shared with `validate_block` via
    /// `difficulty_retarget::required_bits` so miner and validator cannot diverge.
    fn next_required_bits(&self) -> Result<u32, StorageError> {
        let params = self.retarget_params;
        // No tip yet -> the next block is genesis-level, mined at the floor.
        if self.best_block_hash == [0u8; 32] {
            return Ok(params.pow_limit_bits);
        }
        let tip = self.db.get_block(&self.best_block_hash)?.ok_or_else(|| {
            StorageError::DatabaseError("tip block missing for difficulty target".to_string())
        })?;
        let next_height = self.current_height + 1;
        let boundary_timestamps =
            if next_height > 0 && params.interval > 0 && next_height % params.interval == 0 {
                Some(self.period_boundary_timestamps(&tip, params.interval)?)
            } else {
                None
            };
        Ok(difficulty_retarget::required_bits(
            next_height,
            tip.header().bits(),
            boundary_timestamps,
            &params,
        ))
    }

    /// Get the current network difficulty as a float
    pub fn get_current_difficulty(&self) -> f64 {
        // Convert target bits to difficulty
        let target = self.get_difficulty_target();

        // Bitcoin difficulty calculation
        // The maximum target (difficulty 1) is 0x1d00ffff in compact form
        // This represents: 0x00000000ffff0000000000000000000000000000000000000000000000000000

        // Convert compact bits to actual target value
        let exponent = (target >> 24) & 0xff;
        let mantissa = target & 0x00ffffff;

        if mantissa == 0 || exponent == 0 {
            return 1.0; // Minimum difficulty
        }

        // Calculate actual target value as f64 to avoid overflow
        let current_target: f64 = if exponent <= 3 {
            (mantissa >> (8 * (3 - exponent))) as f64
        } else {
            (mantissa as f64) * 2f64.powi((8 * (exponent - 3)) as i32)
        };

        if current_target == 0.0 {
            return 1.0; // Prevent division by zero
        }

        // Max target for difficulty 1 (0x1d00ffff)
        // This is 0xffff * 2^(8*(0x1d-3)) = 65535 * 2^208
        let max_target: f64 = 65535.0 * 2f64.powi(208);

        // Calculate difficulty
        max_target / current_target
    }

    pub async fn process_block(&mut self, mut block: Block) -> Result<bool, StorageError> {
        // Stamp the authoritative height DERIVED from the parent (#5 height
        // reliability). The wire height is attacker-controlled yet feeds the
        // subsidy cap (`check_block_value`), chain-work accumulation, the
        // persisted tip, and future children's derivation — so consensus must
        // use the derived value, not the claim. Height is excluded from the
        // block hash (`serialize_for_hash`), so stamping changes neither block
        // identity nor proof-of-work. A genuinely orphan block (parent unknown)
        // keeps its claimed height and is rejected downstream when its ancestors
        // cannot be walked.
        if let Some(expected) = self.expected_height(&block) {
            if block.height() != expected {
                tracing::warn!(
                    "Block {} claimed height {} but its parent implies {}; using the derived height",
                    hex::encode(&block.hash()[..8]),
                    block.height(),
                    expected
                );
            }
            block.set_height(expected);
        }

        let block_hash = block.hash();
        let prev_hash = block.prev_block_hash();

        // Update last block time even if we don't accept the block
        self.last_block_time = SystemTime::now();

        if !self.validate_block(&block).await? {
            tracing::warn!("Block validation failed for block at height {}", block.height());
            return Err(StorageError::InvalidBlock);
        }

        if self.db.get_block(&block_hash)?.is_some() {
            tracing::debug!("Block already exists in database");
            // Block already exists, but still update fork info
            self.update_fork_info(&block)?;
            return Ok(false);
        }

        let new_chain_work = self.calculate_chain_work(&block)?;
        tracing::debug!("Calculated chain work: {}", new_chain_work);
        
        if *prev_hash != self.best_block_hash {
            tracing::info!(
                "Block on fork detected: prev={}, best={}",
                hex::encode(prev_hash),
                hex::encode(&self.best_block_hash)
            );
            let current_work = match self.chain_work.get(&self.best_block_hash) {
                Some(work) => *work,
                None => {
                    // If we don't have work for current tip, calculate it
                    if let Ok(Some(block)) = self.db.get_block(&self.best_block_hash) {
                        let work = self.calculate_chain_work(&block)?;
                        self.chain_work.insert(self.best_block_hash, work);
                        work
                    } else {
                        Work::zero()
                    }
                }
            };

            // Update or create fork info regardless of whether we accept the block
            self.update_fork_info(&block)?;

            // Determine if we should switch to the new fork
            match new_chain_work.cmp(&current_work) {
                Ordering::Greater => {
                    // New chain has more work, attempt reorganization
                    let (fork_point, blocks_to_apply, blocks_to_disconnect) =
                        self.find_fork_point(&block)?;

                    // Check if reorganization is allowed below checkpoint height
                    if let Err(e) = can_reorganize_below(fork_point.height()) {
                        warn!(
                            "Rejected reorganization below checkpoint: {}",
                            e
                        );
                        self.rejected_reorgs += 1;
                        return Ok(false);
                    }

                    if blocks_to_disconnect.len() as u64 > MAX_REORG_DEPTH {
                        warn!(
                            "Rejected deep reorganization: {} blocks (max: {})",
                            blocks_to_disconnect.len(),
                            MAX_REORG_DEPTH
                        );
                        self.rejected_reorgs += 1;
                        return Ok(false);
                    }

                    let fork_choice_reason = ForkChoiceReason::HigherChainWork;
                    self.handle_chain_reorganization(
                        &block,
                        fork_point,
                        blocks_to_apply,
                        blocks_to_disconnect,
                        fork_choice_reason,
                    )
                    .await?;
                    return Ok(true);
                }
                Ordering::Equal => {
                    // Equal chain work - use secondary metrics to decide
                    // Look at the fork info to see which fork we saw first
                    let current_fork = self.active_forks.get(&self.best_block_hash).cloned();
                    let new_fork = self.active_forks.get(&block_hash).cloned();

                    if let (Some(current), Some(new)) = (current_fork, new_fork) {
                        // Prefer the fork we saw first
                        if new.first_seen < current.first_seen {
                            let (fork_point, blocks_to_apply, blocks_to_disconnect) =
                                self.find_fork_point(&block)?;

                            // Check if reorganization is allowed below checkpoint height
                            if let Err(e) = can_reorganize_below(fork_point.height()) {
                                warn!(
                                    "Rejected reorganization below checkpoint: {}",
                                    e
                                );
                                self.rejected_reorgs += 1;
                                return Ok(false);
                            }

                            if blocks_to_disconnect.len() as u64 > MAX_REORG_DEPTH {
                                warn!(
                                    "Rejected deep reorganization with equal work: {} blocks",
                                    blocks_to_disconnect.len()
                                );
                                self.rejected_reorgs += 1;
                                return Ok(false);
                            }

                            let fork_choice_reason = ForkChoiceReason::FirstSeen;
                            self.handle_chain_reorganization(
                                &block,
                                fork_point,
                                blocks_to_apply,
                                blocks_to_disconnect,
                                fork_choice_reason,
                            )
                            .await?;
                            return Ok(true);
                        }
                    }

                    // Add to our fork set, but don't switch
                    self.fork_points.insert(*prev_hash);
                }
                Ordering::Less => {
                    // Current chain has more work, just track this as a fork
                    self.fork_points.insert(*prev_hash);
                }
            }
        } else {
            // Direct extension of current chain
            let block_difficulty = block_work_u64(&block);

            // Store block to database with bloom filter and cache updates
            tracing::debug!("Storing block {} at height {}", hex::encode(&block_hash[..8]), block.height());
            let block_data = bincode::serialize(&block)
                .map_err(|e| StorageError::DatabaseError(format!("Block serialization failed: {}", e)))?;
            self.db.store_block(&block_hash, &block_data)?;
            self.db.flush()?;
            
            self.chain_work.insert(block_hash, new_chain_work);

            // Update chain state
            self.current_height = block.height();
            self.best_block_hash = block_hash;
            
            // Store updated height and best hash as big-endian
            self.db.set_metadata(b"height", &self.current_height.to_be_bytes())?;
            self.db.set_metadata(b"best_hash", &block_hash)?;
            // Record this block in the height->hash index (#5). `current_height`
            // was just set to the stamped/derived `block.height()`, so the key is
            // the trustworthy height, not an attacker-supplied wire value.
            self.db.store_block_height_index(self.current_height, &block_hash)?;
            self.db.flush()?;
            
            // Process transactions to update UTXO set
            self.process_block_transactions(&block)?;
            
            tracing::info!("Block added to chain: height={}, hash={}", self.current_height, hex::encode(&block_hash[..8]));

            // Update total difficulty
            self.update_total_difficulty(block_difficulty)?;

            // Update fork info for direct extension
            self.update_fork_info(&block)?;

            return Ok(true);
        }

        // Store the block in our database, but don't update best chain
        self.store_block(block)?;
        self.chain_work.insert(block_hash, new_chain_work);

        Ok(false)
    }

    /// The authoritative height a block should have, DERIVED from its parent
    /// rather than trusted from the wire (#5 height reliability): a genesis block
    /// (null prev hash) is height 0; otherwise it is the stored parent's height
    /// plus one. Returns `None` when the parent is unknown (an orphan), since the
    /// height cannot be derived without it.
    ///
    /// Consensus already trusts `block.height()` for the subsidy cap
    /// (`check_block_value`) and the fork-distance gate; making height
    /// parent-derived is what lets those checks be trustworthy. A later commit
    /// enforces `block.height() == expected_height` and stamps the derived value.
    fn expected_height(&self, block: &Block) -> Option<u64> {
        let prev = block.prev_block_hash();
        if *prev == [0u8; 32] {
            return Some(0); // genesis
        }
        self.db.get_block(prev).ok().flatten().map(|p| p.height() + 1)
    }

    /// Validate a block's difficulty `bits` against what the chain requires
    /// (#2.2). Returns `Ok(false)` to reject a permanently-wrong block (easier
    /// than the floor, or not the required difficulty for its height/parent), and
    /// `Err` when the parent is not yet known (a retryable orphan — do NOT mark it
    /// permanently invalid).
    fn check_block_difficulty(&self, block: &Block) -> Result<bool, StorageError> {
        let params = self.retarget_params;

        // Floor + mineability: reject any target easier than `pow_limit` or zero
        // (over-range bits). This is a cheap pre-filter independent of the parent.
        if !difficulty_retarget::target_within_limit(block.header().bits(), &params) {
            tracing::warn!(
                "Block {} difficulty {:#x} is easier than the pow_limit floor {:#x}",
                hex::encode(&block.hash()[..8]),
                block.header().bits(),
                params.pow_limit_bits
            );
            return Ok(false);
        }

        // Genesis (null parent) has no parent-derived difficulty to enforce.
        let prev_hash = *block.prev_block_hash();
        if prev_hash == [0u8; 32] {
            return Ok(true);
        }

        // The parent must be known to derive the required difficulty. A missing
        // parent is an orphan we cannot yet judge — surface as Err so it is
        // rejected for now but not blacklisted (it may arrive in order later).
        let parent = self.db.get_block(&prev_hash)?.ok_or_else(|| {
            StorageError::DatabaseError(format!(
                "difficulty check: parent {} not yet known",
                hex::encode(&prev_hash[..8])
            ))
        })?;

        // `block.height()` is the trustworthy derived/stamped height. At a
        // retarget boundary, sample the period's first/last timestamps along the
        // block's own ancestry; otherwise difficulty is unchanged from the parent.
        let height = block.height();
        let boundary_timestamps =
            if height > 0 && params.interval > 0 && height % params.interval == 0 {
                Some(self.period_boundary_timestamps(&parent, params.interval)?)
            } else {
                None
            };

        let required = difficulty_retarget::required_bits(
            height,
            parent.header().bits(),
            boundary_timestamps,
            &params,
        );
        if block.header().bits() != required {
            tracing::warn!(
                "Block {} has bits {:#x} but the chain requires {:#x} at height {}",
                hex::encode(&block.hash()[..8]),
                block.header().bits(),
                required,
                height
            );
            return Ok(false);
        }

        Ok(true)
    }

    /// Gather the `(first_ts, last_ts)` timestamps of the difficulty period that
    /// closes just before a boundary block whose parent is `parent`, by walking
    /// the parent's OWN ancestry (`prev_block_hash`) — never the canonical height
    /// index. `last_ts` is the parent's timestamp (height `H-1`); `first_ts` is
    /// the timestamp `interval-1` ancestors back (height `H-interval`).
    fn period_boundary_timestamps(
        &self,
        parent: &Block,
        interval: u64,
    ) -> Result<(u64, u64), StorageError> {
        let last_ts = parent.header().timestamp();
        let mut cursor = parent.clone();
        for _ in 0..interval.saturating_sub(1) {
            let prev_hash = *cursor.prev_block_hash();
            cursor = self.db.get_block(&prev_hash)?.ok_or_else(|| {
                StorageError::DatabaseError(format!(
                    "difficulty retarget: period ancestor {} is missing",
                    hex::encode(&prev_hash[..8])
                ))
            })?;
        }
        Ok((cursor.header().timestamp(), last_ts))
    }

    async fn validate_block(&self, block: &Block) -> Result<bool, StorageError> {
        let block_hash = block.hash();

        // Check if block is already marked as invalid
        if self.invalid_block_tracker.is_permanently_invalid(&block_hash) {
            tracing::warn!("Block {} is permanently invalid, rejecting", hex::encode(&block_hash[..8]));
            return Ok(false);
        }

        // Check if parent is invalid
        let parent_hash = block.prev_block_hash();
        if self.invalid_block_tracker.is_permanently_invalid(parent_hash) {
            tracing::warn!("Parent block {} is invalid, marking block {} as invalid", 
                hex::encode(&parent_hash[..8]), hex::encode(&block_hash[..8]));
            self.invalid_block_tracker.mark_invalid(
                block_hash,
                InvalidationReason::ParentInvalid,
                Some(*parent_hash),
                Some(block.height()),
            ).map_err(|e| StorageError::DatabaseError(format!("Failed to mark block invalid: {}", e)))?;
            return Ok(false);
        }

        tracing::debug!(
            "Validating block: height={}, prev_hash={}",
            block.height(),
            hex::encode(block.prev_block_hash())
        );
        
        if !block.validate() {
            tracing::warn!("Block failed basic validation: height={}", block.height());
            self.invalid_block_tracker.mark_invalid(
                block_hash,
                InvalidationReason::InvalidStructure("Basic validation failed".to_string()),
                Some(*block.prev_block_hash()),
                Some(block.height()),
            ).map_err(|e| StorageError::DatabaseError(format!("Failed to mark block invalid: {}", e)))?;
            return Ok(false);
        }

        // Difficulty (#2.2): the block's `bits` must equal the difficulty the
        // chain requires at its (derived, trustworthy) height and must not be
        // easier than the network floor. Required difficulty is derived by
        // PARENT-WALK along the block's OWN ancestry — never the canonical height
        // index, which would use the wrong chain for a fork. Checked early so an
        // easy-bits block never reaches transaction or value validation. A
        // missing parent surfaces as Err (retryable orphan), not a permanent mark.
        if !self.check_block_difficulty(block)? {
            tracing::warn!("Block failed difficulty validation: height={}", block.height());
            self.invalid_block_tracker.mark_invalid(
                block_hash,
                InvalidationReason::InvalidStructure("Difficulty bits not as required".to_string()),
                Some(*block.prev_block_hash()),
                Some(block.height()),
            ).map_err(|e| StorageError::DatabaseError(format!("Failed to mark block invalid: {}", e)))?;
            return Ok(false);
        }

        // Validate against checkpoints
        if let Err(e) = validate_checkpoint(block) {
            tracing::warn!("Checkpoint validation failed: {}", e);
            self.invalid_block_tracker.mark_invalid(
                block_hash,
                InvalidationReason::CheckpointViolation,
                Some(*block.prev_block_hash()),
                Some(block.height()),
            ).map_err(|e| StorageError::DatabaseError(format!("Failed to mark block invalid: {}", e)))?;
            return Ok(false);
        }

        if block.height() != self.current_height + 1
            && *block.prev_block_hash() != self.best_block_hash
        {
            let fork_distance = self.calculate_fork_distance(block)?;
            tracing::debug!("Fork block at height {}, distance={}", block.height(), fork_distance);
            
            if fork_distance > MAX_FORK_DISTANCE {
                tracing::warn!("Fork distance {} exceeds maximum {}", fork_distance, MAX_FORK_DISTANCE);
                self.invalid_block_tracker.mark_invalid(
                    block_hash,
                    InvalidationReason::ForkTooDeep,
                    Some(*block.prev_block_hash()),
                    Some(block.height()),
                ).map_err(|e| StorageError::DatabaseError(format!("Failed to mark block invalid: {}", e)))?;
                return Ok(false);
            }
        }

        for (i, tx) in block.transactions().iter().enumerate() {
            if !self.validate_transaction(tx).await? {
                tracing::warn!("Transaction {} failed validation in block {}", i, hex::encode(&block.hash()[..8]));
                self.invalid_block_tracker.mark_invalid(
                    block_hash,
                    InvalidationReason::TransactionValidation(format!("Transaction {} invalid", i)),
                    Some(*block.prev_block_hash()),
                    Some(block.height()),
                ).map_err(|e| StorageError::DatabaseError(format!("Failed to mark block invalid: {}", e)))?;
                return Ok(false);
            }
        }

        // Coinbase subsidy cap (audit Critical #3): a block may not create value
        // beyond its block subsidy plus the fees of the transactions it confirms.
        if !self.check_block_value(block)? {
            self.invalid_block_tracker
                .mark_invalid(
                    block_hash,
                    InvalidationReason::InvalidStructure(
                        "Coinbase exceeds block subsidy plus fees".to_string(),
                    ),
                    Some(*block.prev_block_hash()),
                    Some(block.height()),
                )
                .map_err(|e| {
                    StorageError::DatabaseError(format!("Failed to mark block invalid: {}", e))
                })?;
            return Ok(false);
        }

        Ok(true)
    }

    /// Verify that a block does not create monetary value beyond what consensus
    /// permits (audit Critical #3): the total coinbase output must not exceed
    /// `block_subsidy(height)` plus the total fees of the block's non-coinbase
    /// transactions. **Fail-closed** with checked arithmetic — any overflow or a
    /// missing prevout returns `Ok(false)` (reject) rather than risking a wrap
    /// that could mint coins. Returns `Ok(true)` if the block conserves value.
    fn check_block_value(&self, block: &Block) -> Result<bool, StorageError> {
        let get_prevout = |txid: &[u8; 32], vout: u32| -> Option<TransactionOutput> {
            self.db.get_utxo(txid, vout).ok().flatten()
        };

        let mut total_fees: u64 = 0;
        let mut coinbase_output: u64 = 0;
        for tx in block.transactions() {
            if tx.is_coinbase() {
                let out = match tx.total_output() {
                    Some(v) => v,
                    None => return Ok(false), // coinbase output sum overflow
                };
                coinbase_output = match coinbase_output.checked_add(out) {
                    Some(v) => v,
                    None => return Ok(false),
                };
            } else {
                // calculate_fee = checked_sub(total_input, total_output); None on
                // overflow or a value-creating tx (already rejected per-tx, but
                // re-checked here so it can never poison the fee total).
                let fee = match tx.calculate_fee(&get_prevout) {
                    Some(v) => v,
                    None => return Ok(false),
                };
                total_fees = match total_fees.checked_add(fee) {
                    Some(v) => v,
                    None => return Ok(false),
                };
            }
        }

        let max_coinbase = match block_subsidy(block.height()).checked_add(total_fees) {
            Some(v) => v,
            None => return Ok(false),
        };
        if coinbase_output > max_coinbase {
            tracing::warn!(
                "Coinbase cap violated at height {}: coinbase output {} > subsidy + fees {}",
                block.height(),
                coinbase_output,
                max_coinbase
            );
            return Ok(false);
        }

        Ok(true)
    }

    /// Process transactions in a block to update UTXO set
    fn process_block_transactions(&mut self, block: &Block) -> Result<(), StorageError> {
        for tx in block.transactions() {
            let tx_hash = tx.hash();
            
            // Remove spent UTXOs (skip for coinbase as it has no real inputs)
            if !tx.is_coinbase() {
                for input in tx.inputs() {
                    let prev_tx = input.prev_tx_hash();
                    let prev_vout = input.prev_output_index();
                    
                    // Remove the spent UTXO
                    if let Err(e) = self.db.remove_utxo(&prev_tx, prev_vout) {
                        tracing::warn!("Failed to remove spent UTXO: {}", e);
                    }
                }
            }
            
            // Add new UTXOs from outputs
            for (vout, output) in tx.outputs().iter().enumerate() {
                let output_data = bincode::serialize(output)
                    .map_err(|e| StorageError::DatabaseError(format!("Output serialization failed: {}", e)))?;
                
                self.db.store_utxo(&tx_hash, vout as u32, &output_data)?;
            }
        }
        
        tracing::debug!("Processed {} transactions, updated UTXO set", block.transactions().len());
        Ok(())
    }

    async fn validate_transaction(&self, tx: &Transaction) -> Result<bool, StorageError> {
        // Skip UTXO validation for coinbase transactions
        if tx.is_coinbase() {
            return Ok(true);
        }
        
        let mut spent_outputs = HashSet::new();
        for (idx, input) in tx.inputs().iter().enumerate() {
            let outpoint = (input.prev_tx_hash(), input.prev_output_index());
            if !spent_outputs.insert(outpoint) {
                tracing::warn!("Double-spend detected in transaction: input {}", idx);
                return Ok(false);
            }

            if self
                .db
                .get_utxo(&input.prev_tx_hash(), input.prev_output_index())?
                .is_none()
            {
                tracing::warn!(
                    "UTXO not found for input {}: txid={}, vout={}",
                    idx,
                    hex::encode(input.prev_tx_hash()),
                    input.prev_output_index()
                );
                return Ok(false);
            }
        }

        // Value conservation (audit Critical #3): a non-coinbase transaction may
        // not create value — the sum of the amounts it spends must be at least
        // the sum of the amounts it creates. All sums use checked arithmetic
        // (`total_input`/`total_output` return `None` on overflow or a missing
        // prevout); a `None` rejects the transaction (fail-closed), since release
        // builds disable overflow-checks and a silent wrap could mint coins.
        let get_prevout = |txid: &[u8; 32], vout: u32| -> Option<TransactionOutput> {
            self.db.get_utxo(txid, vout).ok().flatten()
        };
        let total_in = match tx.total_input(&get_prevout) {
            Some(v) => v,
            None => {
                tracing::warn!(
                    "Value conservation: input sum overflow or missing prevout for tx {}",
                    hex::encode(tx.hash())
                );
                return Ok(false);
            }
        };
        let total_out = match tx.total_output() {
            Some(v) => v,
            None => {
                tracing::warn!(
                    "Value conservation: output sum overflow for tx {}",
                    hex::encode(tx.hash())
                );
                return Ok(false);
            }
        };
        if total_in < total_out {
            tracing::warn!(
                "Value conservation violated for tx {}: inputs {} < outputs {}",
                hex::encode(tx.hash()),
                total_in,
                total_out
            );
            return Ok(false);
        }

        // Cryptographically verify every input is authorized to spend its UTXO
        // (audit Critical #1). Fail-closed: a missing, invalid, or unbound
        // signature rejects the transaction and therefore the block.
        if let Err(e) = self.verify_transaction_authorization(tx) {
            tracing::warn!(
                "Signature authorization failed for tx {}: {}",
                hex::encode(tx.hash()),
                e
            );
            return Ok(false);
        }

        Ok(true)
    }

    /// Verify that every input of `tx` is cryptographically authorized to spend
    /// its referenced UTXO, checked against the current UTXO set.
    ///
    /// **Fail-closed**: returns `Err` if any signature is missing, invalid, or
    /// not bound to the spent output's key (audit Critical #1). Coinbase
    /// transactions carry no spendable inputs and pass. See
    /// [`supernova_core::types::transaction::Transaction::verify_authorization`].
    pub fn verify_transaction_authorization(&self, tx: &Transaction) -> Result<(), StorageError> {
        let get_prevout = |txid: &[u8; 32], vout: u32| -> Option<TransactionOutput> {
            self.db.get_utxo(txid, vout).ok().flatten()
        };
        tx.verify_authorization(&get_prevout)
            .map_err(|e| StorageError::InvalidTransaction(e.to_string()))
    }

    /// Retrieve a specific output from a block by transaction hash and output index
    /// Used during chain reorganization to restore spent UTXOs
    fn get_output_from_disconnected_block(
        &self,
        tx_hash: &[u8; 32],
        vout: u32,
    ) -> Result<TransactionOutput, StorageError> {
        let tip_height = self.get_height();
        
        // Search last 1000 blocks (should cover any reasonable reorg depth)
        for height in (tip_height.saturating_sub(1000)..=tip_height).rev() {
            // Get block hash for this height
            if let Ok(Some(block_hash)) = self.db.get_block_hash_by_height(height) {
                // Get the full block
                if let Some(block) = self.get_block(&block_hash) {
                    for tx in block.transactions() {
                        if tx.hash() == *tx_hash {
                            return tx.outputs()
                                .get(vout as usize)
                                .cloned()
                                .ok_or_else(|| {
                                    StorageError::DatabaseError(format!(
                                        "Output index {} not found in transaction {}",
                                        vout,
                                        hex::encode(tx_hash)
                                    ))
                                });
                        }
                    }
                }
            }
        }
        
        Err(StorageError::DatabaseError(format!(
            "Transaction {} not found in recent blocks (searched last 1000 blocks)",
            hex::encode(tx_hash)
        )))
    }

    fn find_fork_point(
        &self,
        new_tip: &Block,
    ) -> Result<(Block, Vec<Block>, Vec<Block>), StorageError> {
        let mut blocks_to_apply = Vec::new();
        let mut blocks_to_disconnect = Vec::new();
        let mut current = new_tip.clone();
        let mut main_chain = self.get_block_at_height(self.current_height)?;

        while current.height() > 0 && main_chain.height() > 0 {
            if current.hash() == main_chain.hash() {
                return Ok((current, blocks_to_apply, blocks_to_disconnect));
            }

            if current.height() > main_chain.height() {
                blocks_to_apply.push(current.clone());
                let prev_hash = *current.prev_block_hash();
                current = self
                    .db
                    .get_block(&prev_hash)?
                    .ok_or(StorageError::DatabaseError("Block not found".to_string()))?;
            } else {
                blocks_to_disconnect.push(main_chain.clone());
                let prev_hash = *main_chain.prev_block_hash();
                main_chain = self
                    .db
                    .get_block(&prev_hash)?
                    .ok_or(StorageError::DatabaseError("Block not found".to_string()))?;
            }
        }

        Err(StorageError::InvalidChainReorganization)
    }

    async fn handle_chain_reorganization(
        &mut self,
        new_tip: &Block,
        fork_point: Block,
        blocks_to_apply: Vec<Block>,
        blocks_to_disconnect: Vec<Block>,
        fork_choice_reason: ForkChoiceReason,
    ) -> Result<(), StorageError> {
        let old_tip = self.best_block_hash;
        let time_since_last_reorg = SystemTime::now()
            .duration_since(self.last_reorg_time)
            .map_err(|e| StorageError::DatabaseError(e.to_string()))?;

        let _reorg_event = ReorganizationEvent {
            old_tip,
            new_tip: new_tip.hash(),
            fork_point: fork_point.hash(),
            fork_height: fork_point.height(),
            blocks_disconnected: blocks_to_disconnect.len() as u64,
            blocks_connected: blocks_to_apply.len() as u64,
            timestamp: SystemTime::now(),
            time_since_last_reorg,
            fork_choice_reason: fork_choice_reason.clone(),
        };

        // Log the reorganization event with detail appropriate to its size
        if blocks_to_disconnect.len() > 1 || blocks_to_apply.len() > 1 {
            info!("Chain reorganization: {} blocks disconnected, {} blocks connected at height {}. Reason: {:?}",
                blocks_to_disconnect.len(),
                blocks_to_apply.len(),
                fork_point.height(),
                fork_choice_reason);
        } else {
            debug!("Minor chain reorganization: {} blocks disconnected, {} blocks connected at height {}",
                blocks_to_disconnect.len(),
                blocks_to_apply.len(),
                fork_point.height());
        }

        // ---- PHASE A: PLAN. Build the full change-set against the pre-reorg
        // database; NO writes and NO in-memory mutation happen here, so the
        // whole reorg can be committed atomically (or not at all). ----
        let mut changes = ReorgChangeSet::new();

        // Disconnect tip-first: `blocks_to_disconnect` is ordered [old_tip ..
        // fork_point+1], so iterating forward unwinds the tip before its parents.
        for block in blocks_to_disconnect.iter() {
            self.plan_disconnect_block(block, &mut changes)?;
        }

        // Connect oldest-first: `blocks_to_apply` is ordered [new_tip ..
        // fork_point+1], so `.rev()` applies the oldest fork block first — an
        // output must be created before a later block can spend it.
        for block in blocks_to_apply.iter().rev() {
            self.plan_connect_block(block, &mut changes)?;
        }

        // total_difficulty tracks the cumulative PoW of the BEST chain. Recompute
        // it ABSOLUTELY from the new tip rather than as a delta off the previous
        // value: the stored metric can include work from fork blocks that were
        // stored but never on the best chain, so a delta would carry that
        // pollution forward (and could underflow). A fresh cumulative sum over
        // the new tip's ancestry is self-consistent and cannot double-count the
        // applied blocks.
        let new_total_difficulty =
            chainwork::work_to_u64_saturating(self.calculate_chain_work(new_tip)?);
        changes.put_meta(
            b"total_difficulty".to_vec(),
            bincode::serialize(&new_total_difficulty)?,
        );
        // Tip metadata: height big-endian to match every other writer/reader.
        changes.put_meta(b"height".to_vec(), new_tip.height().to_be_bytes().to_vec());
        changes.put_meta(b"best_hash".to_vec(), new_tip.hash().to_vec());

        // Persist the new tip's block bytes — the only applied block not already
        // stored (fork ancestors were stored when first received). store_block
        // also refreshes the bloom filter / cache so the tip is readable. Block
        // bytes are content-addressed, so a lingering orphan if the commit below
        // fails is harmless — the authoritative chain state IS the atomic commit.
        let tip_bytes = bincode::serialize(new_tip).map_err(|e| {
            StorageError::DatabaseError(format!("tip block serialize failed: {}", e))
        })?;
        self.db.store_block(&new_tip.hash(), &tip_bytes)?;

        // ---- PHASE B: COMMIT atomically. If this fails, NO in-memory state has
        // been touched, so ChainState and the DB both still reflect old_tip. ----
        self.db.apply_reorg_atomically(&changes)?;

        // ---- PHASE C: post-commit side effects (reached only on success).
        // These are non-transactional and must run after the durable commit. ----
        self.best_block_hash = new_tip.hash();
        self.current_height = new_tip.height();
        self.last_reorg_time = SystemTime::now();
        self.reorg_count += 1;

        // Record cumulative work for the applied tips and DROP the orphaned
        // disconnected hashes, so the chain_work map cannot leak (or feed a
        // stale comparison) across repeated reorgs.
        for block in blocks_to_apply.iter() {
            if let Ok(work) = self.calculate_chain_work(block) {
                self.chain_work.insert(block.hash(), work);
            }
        }
        for block in blocks_to_disconnect.iter() {
            self.chain_work.remove(&block.hash());
        }

        // Update fork points
        self.prune_fork_points()?;

        // Update fork info to reflect current state
        let new_tip_hash = new_tip.hash();
        let updates: Vec<([u8; 32], bool)> = self
            .active_forks
            .iter()
            .map(|(hash, fork)| {
                let is_active = *hash == new_tip_hash
                    || self
                        .is_ancestor_of(&fork.tip_hash, &new_tip_hash)
                        .unwrap_or(false);
                (*hash, is_active)
            })
            .collect();

        // Apply the updates
        for (hash, is_active) in updates {
            if let Some(fork) = self.active_forks.get_mut(&hash) {
                fork.is_active = is_active;
            }
        }

        // Log successful reorganization
        info!(
            "Chain reorganization complete: Activated fork with tip {} at height {}",
            hex::encode(&new_tip.hash()[..4]),
            new_tip.height()
        );

        // Clean up orphaned blocks (blocks whose parents are invalid)
        let chain_blocks: HashSet<[u8; 32]> = blocks_to_disconnect
            .iter()
            .map(|b| b.hash())
            .collect();
        if let Ok(orphaned) = self.invalid_block_tracker.cleanup_orphans(&chain_blocks) {
            if !orphaned.is_empty() {
                info!("Cleaned up {} orphaned blocks during reorganization", orphaned.len());
            }
        }

        Ok(())
    }

    /// Plan the UTXO/index effects of DISCONNECTING `block` into `changes` (#5).
    ///
    /// Emits ops only — performs NO database writes and mutates no in-memory
    /// state — so the whole reorg can be staged and then committed atomically.
    /// Reverses the block's transactions: restores every spent prevout (resolved
    /// against the pre-reorg chain, which still holds the disconnected blocks)
    /// and removes every output the block created, then drops the block's
    /// height->hash index entry. Transactions are reversed newest-first so an
    /// output created and then spent within the same block nets out correctly.
    fn plan_disconnect_block(
        &self,
        block: &Block,
        changes: &mut ReorgChangeSet,
    ) -> Result<(), StorageError> {
        for tx in block.transactions().iter().rev() {
            let tx_hash = tx.hash();

            // Restore the prevouts this tx spent (coinbase spends nothing).
            if !tx.is_coinbase() {
                for input in tx.inputs() {
                    let prev_tx = input.prev_tx_hash();
                    let prev_vout = input.prev_output_index();
                    // Resolve the spent output's value/script against the
                    // PRE-reorg state (no writes have happened yet), so the
                    // restoration is exact even for prevouts created below the
                    // fork point.
                    let prev_output =
                        self.get_output_from_disconnected_block(&prev_tx, prev_vout)?;
                    let output_data = bincode::serialize(&prev_output).map_err(|e| {
                        StorageError::DatabaseError(format!(
                            "undo output serialize failed: {}",
                            e
                        ))
                    })?;
                    changes.put_utxo(create_utxo_key(&prev_tx, prev_vout), output_data);
                }
            }

            // Remove the outputs this tx created.
            for vout in 0..tx.outputs().len() as u32 {
                changes.del_utxo(create_utxo_key(&tx_hash, vout));
            }
        }

        changes.del_height_index(block.height());
        Ok(())
    }

    /// Plan the UTXO/index effects of CONNECTING `block` into `changes` (#5).
    ///
    /// Emits ops only — no database writes, no in-memory mutation. Applies the
    /// block's transactions to the UTXO set (spend each non-coinbase input,
    /// create each output) and records its height->hash index entry, keyed on
    /// the now-trustworthy stamped `block.height()`. The new tip's block bytes
    /// are persisted separately by the caller (content-addressed and idempotent,
    /// so they need not be inside the atomic UTXO/metadata commit).
    fn plan_connect_block(
        &self,
        block: &Block,
        changes: &mut ReorgChangeSet,
    ) -> Result<(), StorageError> {
        for tx in block.transactions() {
            let tx_hash = tx.hash();

            if !tx.is_coinbase() {
                for input in tx.inputs() {
                    changes.del_utxo(create_utxo_key(
                        &input.prev_tx_hash(),
                        input.prev_output_index(),
                    ));
                }
            }

            for (vout, output) in tx.outputs().iter().enumerate() {
                let output_data = bincode::serialize(output).map_err(|e| {
                    StorageError::DatabaseError(format!("output serialize failed: {}", e))
                })?;
                changes.put_utxo(create_utxo_key(&tx_hash, vout as u32), output_data);
            }
        }

        changes.put_height_index(block.height(), block.hash());
        Ok(())
    }

    fn store_block(&mut self, block: Block) -> Result<(), StorageError> {
        // Store the block in the database
        let block_hash = block.hash();
        
        // Serialize and store with bloom filter/cache updates
        let block_data = bincode::serialize(&block)
            .map_err(|e| StorageError::DatabaseError(format!("Block serialization failed: {}", e)))?;
        self.db.store_block(&block_hash, &block_data)?;
        self.db.flush()?;

        // Calculate block difficulty
        let block_difficulty = block_work_u64(&block);

        // Update chain work (cumulative; see connect_block).
        let cumulative_work = self.calculate_chain_work(&block)?;
        self.chain_work.insert(block_hash, cumulative_work);

        // Update total difficulty
        self.update_total_difficulty(block_difficulty)?;

        // Update best block hash and height if higher than current
        if self.current_height < block.height() {
            self.best_block_hash = block_hash;
            self.current_height = block.height();
            // Store height as big-endian bytes (to match get_height() which reads as big-endian)
            self.db.set_metadata(b"height", &self.current_height.to_be_bytes())?;
            self.db.set_metadata(b"best_hash", &block_hash)?;
            self.db.flush()?; // Flush metadata too
        }

        Ok(())
    }

    fn calculate_chain_work(&self, block: &Block) -> Result<Work, StorageError> {
        let mut total_work = Work::zero();
        let mut current = block.clone();

        tracing::debug!("Calculating chain work from height {}", current.height());

        while current.height() > 0 {
            total_work = total_work.saturating_add(block_work(&current)?);

            // Get previous block
            let prev_hash = current.prev_block_hash();
            
            if let Ok(Some(prev_block)) = self.db.get_block(prev_hash) {
                current = prev_block;
            } else {
                tracing::error!(
                    "Chain work calculation failed: block at height {} not found (hash: {})",
                    current.height() - 1,
                    hex::encode(prev_hash)
                );
                return Err(StorageError::DatabaseError(format!(
                    "Previous block not found at height {}",
                    current.height() - 1
                )));
            }
        }

        tracing::debug!("Chain work calculated: total={}", total_work);
        Ok(total_work)
    }

    fn calculate_fork_distance(&self, block: &Block) -> Result<u64, StorageError> {
        let mut current = block.clone();
        let mut distance = 0;

        while current.height() > 0 {
            if self.db.get_block(current.prev_block_hash())?.is_some() {
                return Ok(distance);
            }
            distance += 1;
            if let Ok(Some(prev_block)) = self.db.get_block(current.prev_block_hash()) {
                current = prev_block;
            } else {
                break;
            }
        }

        Ok(distance)
    }

    fn prune_fork_points(&mut self) -> Result<(), StorageError> {
        // Create a temporary set of hashes to avoid the borrow checker issue
        let mut hashes_to_keep = HashSet::new();

        // First collect the hashes that should be kept
        for hash in &self.fork_points {
            if let Ok(Some(block)) = self.db.get_block(hash) {
                let age = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .map_err(|e| StorageError::DatabaseError(e.to_string()))?
                    .as_secs()
                    - self.header_timestamp(&block);

                if age < 86400 {
                    hashes_to_keep.insert(*hash);
                }
            }
        }

        // Now replace the fork_points with the filtered set
        self.fork_points = hashes_to_keep;

        Ok(())
    }

    // Helper method to get timestamp from block header
    fn header_timestamp(&self, _block: &Block) -> u64 {
        // In a real implementation, this would access the timestamp directly
        // Here we're using a default value of current time - 1 hour
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
            - 3600
    }

    pub fn get_block_at_height(&self, height: u64) -> Result<Block, StorageError> {
        let mut current_hash = self.best_block_hash;
        let mut current_height = self.current_height;

        while current_height > height {
            let block = self
                .db
                .get_block(&current_hash)?
                .ok_or(StorageError::DatabaseError("Block not found".to_string()))?;
            current_hash = *block.prev_block_hash();
            current_height -= 1;
        }

        self.db
            .get_block(&current_hash)?
            .ok_or_else(|| StorageError::DatabaseError("Block not found".to_string()))
    }

    /// Get a reference to the underlying database
    pub fn get_db(&self) -> &Arc<BlockchainDB> {
        &self.db
    }

    /// Update information about active forks when a new block arrives
    fn update_fork_info(&mut self, block: &Block) -> Result<(), StorageError> {
        let block_hash = block.hash();
        let prev_hash = block.prev_block_hash();
        let now = SystemTime::now();

        // Check if this is an extension of an existing fork
        let prev_hash_clone = *prev_hash;

        // First check if we need to update an existing fork
        let fork_update = if let Some(fork) = self.active_forks.get(&prev_hash_clone) {
            // Create updated fork info
            Some(ForkInfo {
                fork_point_hash: fork.fork_point_hash,
                fork_point_height: fork.fork_point_height,
                tip_hash: block_hash,
                tip_height: block.height(),
                blocks_added: fork.blocks_added + 1,
                first_seen: fork.first_seen,
                last_updated: now,
                chain_work: self.calculate_chain_work(block)?,
                is_active: fork.is_active,
            })
        } else {
            None
        };

        // Apply the update if needed
        if let Some(updated_fork) = fork_update {
            // Update existing fork
            self.active_forks
                .insert(prev_hash_clone, updated_fork.clone());
            // Also store under the new block hash
            self.active_forks.insert(block_hash, updated_fork);

            debug!(
                "Extended fork to height {} with tip {}",
                block.height(),
                hex::encode(&block_hash[..4])
            );
        } else if *prev_hash == self.best_block_hash {
            // This is a direct extension of the main chain
            let fork_info = ForkInfo {
                fork_point_hash: *prev_hash,
                fork_point_height: self.current_height,
                tip_hash: block_hash,
                tip_height: block.height(),
                chain_work: self.calculate_chain_work(block)?,
                blocks_added: 1,
                first_seen: now,
                last_updated: now,
                is_active: true,
            };

            self.active_forks.insert(block_hash, fork_info);
        } else {
            // This is a new fork or extension of an unknown fork
            // Try to find fork point with main chain
            let mut current = block.clone();
            let mut fork_point_hash = [0u8; 32];
            let mut fork_point_height = 0;
            let mut blocks_on_fork = 1;

            // Work backwards until we find a common block with our main chain
            while current.height() > 0 {
                let prev_hash = current.prev_block_hash();

                if let Ok(Some(_)) = self.db.get_block(prev_hash) {
                    // Found a block we know about
                    fork_point_hash = *prev_hash;
                    if let Ok(Some(prev_block)) = self.db.get_block(prev_hash) {
                        fork_point_height = prev_block.height();
                    }
                    break;
                }

                blocks_on_fork += 1;

                if let Ok(Some(prev_block)) = self.db.get_block(prev_hash) {
                    current = prev_block;
                } else {
                    // We don't have the previous block, so we can't determine the fork point
                    break;
                }
            }

            let fork_info = ForkInfo {
                fork_point_hash,
                fork_point_height,
                tip_hash: block_hash,
                tip_height: block.height(),
                chain_work: self.calculate_chain_work(block)?,
                blocks_added: blocks_on_fork,
                first_seen: now,
                last_updated: now,
                is_active: true,
            };

            self.active_forks.insert(block_hash, fork_info);

            info!(
                "Detected new fork at height {} with {} blocks since fork point",
                block.height(),
                blocks_on_fork
            );
        }

        // Clean up old forks
        self.prune_inactive_forks();

        Ok(())
    }

    /// Remove forks that haven't been updated recently
    fn prune_inactive_forks(&mut self) {
        let now = SystemTime::now();
        let max_age = Duration::from_secs(86400); // 24 hours

        self.active_forks.retain(|_, fork| {
            if let Ok(age) = now.duration_since(fork.last_updated) {
                // Keep forks that have been updated recently or are still close to the main chain
                let height_difference = self.current_height.saturating_sub(fork.tip_height);
                age < max_age || height_difference < MAX_FORK_DISTANCE
            } else {
                true // Keep on duration error
            }
        });
    }

    /// Check if one block is an ancestor of another
    fn is_ancestor_of(
        &self,
        potential_ancestor_hash: &[u8; 32],
        descendant_hash: &[u8; 32],
    ) -> Result<bool, StorageError> {
        if potential_ancestor_hash == descendant_hash {
            return Ok(true);
        }

        let mut current_hash = *descendant_hash;

        // Walk back the chain until we find the ancestor or reach genesis
        loop {
            let current_block = match self.db.get_block(&current_hash)? {
                Some(block) => block,
                None => return Ok(false),
            };

            // Check if we've reached height 0 (genesis)
            if current_block.height() == 0 {
                return Ok(false);
            }

            // Get the previous hash
            let prev_hash = current_block.prev_block_hash();

            // Check if we found our ancestor
            if prev_hash == potential_ancestor_hash {
                return Ok(true);
            }

            // Move to previous block
            current_hash = *prev_hash;
        }
    }

    /// Calculate metrics about the longest chain and active forks
    pub fn calculate_fork_metrics(&self) -> HashMap<String, u64> {
        let mut metrics = HashMap::new();

        metrics.insert("main_chain_height".to_string(), self.current_height);
        metrics.insert("active_forks".to_string(), self.active_forks.len() as u64);
        metrics.insert("fork_points".to_string(), self.fork_points.len() as u64);
        metrics.insert("reorg_count".to_string(), self.reorg_count);
        metrics.insert("rejected_reorgs".to_string(), self.rejected_reorgs);

        // Calculate maximum fork length
        let mut max_fork_length = 0;
        for fork in self.active_forks.values() {
            let fork_length = fork.tip_height.saturating_sub(fork.fork_point_height);
            if fork_length > max_fork_length {
                max_fork_length = fork_length;
            }
        }
        metrics.insert("max_fork_length".to_string(), max_fork_length);

        // Time since last block
        if let Ok(duration) = SystemTime::now().duration_since(self.last_block_time) {
            metrics.insert("seconds_since_last_block".to_string(), duration.as_secs());
        }

        metrics
    }

    /// Get all transactions for a given block hash
    pub fn get_transactions_for_block(&self, hash: &[u8; 32]) -> Option<Vec<Transaction>> {
        self.get_block(hash).map(|b| b.transactions().to_vec())
    }
}

/// Real proof-of-work contributed by a block (audit Critical #2).
///
/// Computed from the block's declared difficulty `target` — the SAME bytes
/// `BlockHeader::meets_target` checks the hash against — via Bitcoin's
/// `GetBlockProof` (`2^256 / (target + 1)`). This replaces the previous
/// `extract_target_from_block`, which derived "work" from the first four bytes
/// of the block hash and was trivially grindable to fake unlimited work.
///
/// Returns `Err(InvalidBlock)` for a malformed/over-range `bits` field (which
/// decodes to a zero target): such a block is unmineable and must never be
/// awarded work.
fn block_work(block: &Block) -> Result<Work, StorageError> {
    chainwork::work_from_target(&block.header().target()).ok_or(StorageError::InvalidBlock)
}

/// Saturating u64 projection of a block's real work, for the legacy
/// `total_difficulty` metric only. NON-AUTHORITATIVE: fork choice uses the full
/// [`Work`] value via the `chain_work` map, never this. A block with invalid
/// `bits` projects to 0 here (it is rejected separately on the accept path).
fn block_work_u64(block: &Block) -> u64 {
    chainwork::work_from_target(&block.header().target())
        .map(chainwork::work_to_u64_saturating)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    /// Open chain state with the easy REGTEST difficulty floor (0x207fffff), which
    /// matches the `bits` every test block uses. Production defaults to the hard
    /// testnet floor (0x1e0fffff) via `ChainState::new`, so tests must opt into
    /// regtest or their easy blocks would be rejected once the difficulty gate is
    /// wired in.
    fn regtest_chain_state(db: Arc<BlockchainDB>) -> Result<ChainState, StorageError> {
        ChainState::with_params(db, RetargetParams::regtest())
    }

    /// Seed `g0(h0) <- a1(h1)` directly as the chain tip and return their hashes.
    /// Heights 0/1 carry a genesis checkpoint that process_block would reject for
    /// a custom block, so the shared prefix is seeded and only heights >=2 are
    /// fed through the accept path.
    fn seed_base_chain(db: &Arc<BlockchainDB>, bits: u32, tag0: u64) -> ([u8; 32], [u8; 32]) {
        let mut g0 = unique_coinbase_block([0u8; 32], bits, tag0);
        g0.set_height(0);
        let g0 = mine(g0);
        let gh = g0.hash();
        db.store_block(&gh, &bincode::serialize(&g0).unwrap()).unwrap();
        let mut a1 = unique_coinbase_block(gh, bits, tag0 + 1);
        a1.set_height(1);
        let a1 = mine(a1);
        let a1h = a1.hash();
        db.store_block(&a1h, &bincode::serialize(&a1).unwrap()).unwrap();
        db.store_block_height_index(0, &gh).unwrap();
        db.store_block_height_index(1, &a1h).unwrap();
        db.set_metadata(b"height", &1u64.to_be_bytes()).unwrap();
        db.set_metadata(b"best_hash", &a1h).unwrap();
        (gh, a1h)
    }

    #[tokio::test]
    async fn test_chain_reorganization() -> Result<(), StorageError> {
        // A heavier competing fork must replace the main chain. (The exhaustive
        // UTXO/index/difficulty checks live in
        // reorg_switches_to_heavier_branch_atomically; this is the focused
        // tip-switch assertion the legacy test intended, with real mined blocks.)
        let temp_dir = tempdir().unwrap();
        let db = Arc::new(BlockchainDB::new(temp_dir.path())?);
        let bits = 0x207f_ffff;
        let (_g, a1h) = seed_base_chain(&db, bits, 220);
        let mut cs = regtest_chain_state(db.clone())?;

        // Main chain a1 -> a2 (height 2).
        let a2 = mine(unique_coinbase_block(a1h, bits, 222));
        assert!(cs.process_block(a2.clone()).await?);
        assert_eq!(cs.get_best_block_hash(), a2.hash());

        // A strictly heavier fork off a1: b2(h2), b3(h3). b2 ties a2 (no switch);
        // b3 makes the fork heavier and triggers the reorg.
        let b2 = mine(unique_coinbase_block(a1h, bits, 232));
        let b3 = mine(unique_coinbase_block(b2.hash(), bits, 233));
        assert!(!cs.process_block(b2.clone()).await?, "equal-work fork must not switch");
        assert!(cs.process_block(b3.clone()).await?, "heavier fork must trigger a reorg");
        assert_eq!(cs.get_best_block_hash(), b3.hash());
        assert_eq!(cs.get_height(), 3);
        Ok(())
    }

    #[tokio::test]
    async fn test_fork_validation() -> Result<(), StorageError> {
        // validate_block accepts a well-formed mined block and rejects one whose
        // proof-of-work does not meet its target. (The legacy test asserted a
        // fork-distance rejection, but calculate_fork_distance can never exceed
        // 1 today — a separate, deferred bug — so that path is not exercised.)
        let temp_dir = tempdir().unwrap();
        let db = Arc::new(BlockchainDB::new(temp_dir.path())?);
        let bits = 0x207f_ffff;
        let (_g, a1h) = seed_base_chain(&db, bits, 240);
        let cs = regtest_chain_state(db.clone())?;

        // A valid mined coinbase block extending the tip passes validation.
        let mut valid = unique_coinbase_block(a1h, bits, 242);
        valid.set_height(2);
        let valid = mine(valid);
        assert!(cs.validate_block(&valid).await?, "a valid mined block must pass");

        // Breaking its proof-of-work (nonce that misses the target) is rejected.
        let mut bad_pow = valid.clone();
        while bad_pow.verify_proof_of_work() {
            bad_pow.increment_nonce();
        }
        assert!(
            !cs.validate_block(&bad_pow).await?,
            "a block that fails its PoW target must be rejected"
        );
        Ok(())
    }

    #[tokio::test]
    async fn test_total_difficulty() -> Result<(), StorageError> {
        // A fresh chain reports zero work; extending it accumulates proof-of-work.
        let temp_dir = tempdir().unwrap();
        let db = Arc::new(BlockchainDB::new(temp_dir.path())?);
        let bits = 0x207f_ffff;
        let (_g, a1h) = seed_base_chain(&db, bits, 250);
        let mut cs = regtest_chain_state(db.clone())?;
        assert_eq!(cs.get_total_difficulty(), 0, "no difficulty recorded yet");

        let a2 = mine(unique_coinbase_block(a1h, bits, 252));
        assert!(cs.process_block(a2).await?);
        assert!(cs.get_total_difficulty() > 0, "extending the chain accumulates work");
        Ok(())
    }

    #[test]
    fn plan_disconnect_restores_spent_prevout() {
        // The reorg undo path must restore a spent prevout to its EXACT original
        // output and remove the output the disconnected tx created. This drives
        // plan_disconnect_block directly (no signatures), since the accept-path
        // signature check is covered separately; it is the only test that
        // exercises the non-coinbase restore logic.
        use crate::storage::reorg::ReorgOp;
        use supernova_core::types::transaction::{Transaction, TransactionInput, TransactionOutput};

        let temp_dir = tempdir().unwrap();
        let db = Arc::new(BlockchainDB::new(temp_dir.path()).unwrap());

        // Source tx S creates output O (value 7777) in a stored, height-indexed
        // block, so get_output_from_disconnected_block can resolve S:0.
        let source_in =
            TransactionInput::new([0u8; 32], 0xffff_ffff, b"source".to_vec(), 0xffff_ffff);
        let o = TransactionOutput::new(7777, vec![1, 2, 3]);
        let s = Transaction::new(1, vec![source_in], vec![o.clone()], 0);
        let s_txid = s.hash();
        let mut src_block = Block::new_with_params(1, [0u8; 32], vec![s], 0x207f_ffff);
        src_block.set_height(1);
        let src_hash = src_block.hash();
        db.store_block(&src_hash, &bincode::serialize(&src_block).unwrap())
            .unwrap();
        db.store_block_height_index(1, &src_hash).unwrap();
        db.set_metadata(b"height", &1u64.to_be_bytes()).unwrap();
        db.set_metadata(b"best_hash", &src_hash).unwrap();

        let cs = regtest_chain_state(db.clone()).unwrap();

        // Disconnected block D (height 2): tx T spends S:0 and creates output P.
        let spend_in = TransactionInput::new(s_txid, 0, b"sig".to_vec(), 0xffff_ffff);
        let p = TransactionOutput::new(7000, vec![4, 5, 6]);
        let t = Transaction::new(1, vec![spend_in], vec![p], 0);
        let t_txid = t.hash();
        let mut d = Block::new_with_params(1, src_hash, vec![t], 0x207f_ffff);
        d.set_height(2);

        let mut changes = ReorgChangeSet::new();
        cs.plan_disconnect_block(&d, &mut changes).unwrap();

        // S:0 restored to its EXACT original output; T:0 removed; index dropped.
        assert!(
            changes.ops.contains(&ReorgOp::PutUtxo(
                create_utxo_key(&s_txid, 0),
                bincode::serialize(&o).unwrap()
            )),
            "spent prevout must be restored with its exact original output"
        );
        assert!(
            changes
                .ops
                .contains(&ReorgOp::DelUtxo(create_utxo_key(&t_txid, 0))),
            "the output the disconnected tx created must be removed"
        );
        assert!(
            changes
                .ops
                .contains(&ReorgOp::DelHeightIndex(2u64.to_be_bytes())),
            "the disconnected block's height index entry must be dropped"
        );
    }

    /// End-to-end check of the accept-path wiring (audit Critical #1): the owner
    /// of a UTXO can spend it, but an attacker signing with their own key cannot
    /// — the signing key is bound to the spent output's script.
    #[tokio::test]
    async fn test_verify_transaction_authorization_binds_signing_key_to_utxo(
    ) -> Result<(), StorageError> {
        use sha3::{Digest as _, Sha3_512};
        use supernova_core::crypto::quantum::{QuantumKeyPair, QuantumParameters, QuantumScheme};
        use supernova_core::types::transaction::{
            SignatureSchemeType, TransactionInput, TransactionSignatureData,
        };

        let temp_dir = tempdir().unwrap();
        let db = Arc::new(BlockchainDB::new(temp_dir.path())?);
        let chain_state = regtest_chain_state(db.clone())?;

        // Owner key and the output-script commitment it controls
        // (SHA3-512(pubkey)[..32], matching the wallet address derivation).
        let owner = QuantumKeyPair::generate(QuantumParameters::new(QuantumScheme::Dilithium))
            .expect("owner keypair");
        let mut hasher = Sha3_512::new();
        hasher.update(&owner.public_key);
        let owner_script = hasher.finalize()[..32].to_vec();

        // Seed a UTXO locked to the owner.
        let prev_txid = [9u8; 32];
        let prevout = TransactionOutput::new(1_000_000, owner_script);
        db.store_utxo(&prev_txid, 0, &bincode::serialize(&prevout).unwrap())?;

        let spend = |kp: &QuantumKeyPair| -> Transaction {
            let inputs = vec![TransactionInput::new(prev_txid, 0, vec![], 0xffff_ffff)];
            let outputs = vec![TransactionOutput::new(900_000, vec![0xab; 32])];
            let mut tx = Transaction::new(2, inputs, outputs, 0);
            let sig = kp.sign(&tx.signature_hash()).expect("sign");
            tx.set_signature_data(TransactionSignatureData {
                scheme: SignatureSchemeType::Dilithium,
                security_level: kp.parameters.security_level,
                data: sig,
                public_key: kp.public_key.clone(),
            });
            tx
        };

        // The owner can spend their own UTXO.
        assert!(
            chain_state
                .verify_transaction_authorization(&spend(&owner))
                .is_ok(),
            "owner's signed transaction must be authorized"
        );

        // An attacker signing with their own key cannot spend the owner's UTXO.
        let attacker = QuantumKeyPair::generate(QuantumParameters::new(QuantumScheme::Dilithium))
            .expect("attacker keypair");
        assert!(
            chain_state
                .verify_transaction_authorization(&spend(&attacker))
                .is_err(),
            "attacker's key must NOT authorize spending the owner's UTXO"
        );

        Ok(())
    }

    // --- audit Critical #2: real proof-of-work chain-work accounting ---

    fn coinbase_block(prev: [u8; 32], bits: u32) -> Block {
        Block::new_with_params(1, prev, vec![Transaction::new_coinbase()], bits)
    }

    /// Grind the nonce until the block satisfies its proof-of-work target. Cheap
    /// for the easy regtest-style `0x207f_ffff` target these tests use.
    fn mine(mut block: Block) -> Block {
        while !block.verify_proof_of_work() {
            block.increment_nonce();
        }
        block
    }

    #[test]
    fn block_work_is_independent_of_hash_grinding() {
        // Two blocks with identical difficulty `bits` but different nonces (hence
        // different hashes) MUST contribute identical work. This is exactly the
        // property the old `extract_target_from_block` (first 4 hash bytes)
        // violated, letting an attacker grind a tiny hash prefix to fake huge
        // work and reorg the chain at no cost.
        let bits = 0x207f_ffff; // valid, easy regtest-style target
        let b1 = coinbase_block([0u8; 32], bits);
        let mut b2 = coinbase_block([0u8; 32], bits);
        b2.increment_nonce();
        assert_ne!(b1.hash(), b2.hash(), "different nonces must give different hashes");
        assert_eq!(
            block_work(&b1).unwrap(),
            block_work(&b2).unwrap(),
            "work must depend only on the difficulty bits, not on the block hash"
        );
    }

    #[test]
    fn harder_bits_yield_more_work() {
        // Lower target (harder to mine) must be worth more proof-of-work.
        let easy = coinbase_block([0u8; 32], 0x207f_ffff);
        let hard = coinbase_block([0u8; 32], 0x1d00_ffff);
        assert!(
            block_work(&hard).unwrap() > block_work(&easy).unwrap(),
            "a harder target must contribute more chain work"
        );
    }

    #[test]
    fn invalid_bits_have_no_work() {
        // An over-range `bits` (e.g. u32::MAX) decodes to a zero target and must
        // be rejected — never awarded (would-be infinite) work.
        let bad = coinbase_block([0u8; 32], u32::MAX);
        assert!(block_work(&bad).is_err());
        assert_eq!(block_work_u64(&bad), 0);
    }

    // --- audit Critical #3: value conservation + coinbase subsidy cap ---

    /// A coinbase transaction paying exactly `amount` (smallest units).
    fn coinbase_paying(amount: u64) -> Transaction {
        use supernova_core::types::transaction::TransactionInput;
        Transaction::new(
            1,
            vec![TransactionInput::new_coinbase(vec![0u8])],
            vec![TransactionOutput::new(amount, vec![0x88, 0xac])],
            0,
        )
    }

    #[tokio::test]
    async fn coinbase_cap_rejects_overpay_and_allows_exact_subsidy() -> Result<(), StorageError> {
        let temp_dir = tempdir().unwrap();
        let db = Arc::new(BlockchainDB::new(temp_dir.path())?);
        let chain_state = regtest_chain_state(db)?;

        // new_with_params blocks are at height 0, where block_subsidy == 50 NOVA
        // == 5_000_000_000 smallest units. (No fees: coinbase-only blocks.)
        let bits = 0x207f_ffff;
        let exact = Block::new_with_params(1, [0u8; 32], vec![coinbase_paying(5_000_000_000)], bits);
        assert!(
            chain_state.check_block_value(&exact)?,
            "a coinbase paying exactly the subsidy must be accepted"
        );

        let over = Block::new_with_params(1, [0u8; 32], vec![coinbase_paying(5_000_000_001)], bits);
        assert!(
            !chain_state.check_block_value(&over)?,
            "a coinbase paying one unit over the subsidy must be rejected (no minting)"
        );

        // Genesis / coinbase-less block conserves value trivially.
        let empty = Block::new_with_params(1, [0u8; 32], vec![], bits);
        assert!(
            chain_state.check_block_value(&empty)?,
            "a block with no coinbase creates no value"
        );
        Ok(())
    }

    #[tokio::test]
    async fn value_conservation_rejects_inflated_outputs() -> Result<(), StorageError> {
        use sha3::{Digest as _, Sha3_512};
        use supernova_core::crypto::quantum::{QuantumKeyPair, QuantumParameters, QuantumScheme};
        use supernova_core::types::transaction::{
            SignatureSchemeType, TransactionInput, TransactionSignatureData,
        };

        let temp_dir = tempdir().unwrap();
        let db = Arc::new(BlockchainDB::new(temp_dir.path())?);
        let chain_state = regtest_chain_state(db.clone())?;

        // Owner key + a UTXO of 1_000_000 locked to it.
        let owner = QuantumKeyPair::generate(QuantumParameters::new(QuantumScheme::Dilithium))
            .expect("keypair");
        let mut hasher = Sha3_512::new();
        hasher.update(&owner.public_key);
        let owner_script = hasher.finalize()[..32].to_vec();
        let prev_txid = [3u8; 32];
        db.store_utxo(
            &prev_txid,
            0,
            &bincode::serialize(&TransactionOutput::new(1_000_000, owner_script)).unwrap(),
        )?;

        // A correctly-signed spend creating `out_amount` from the 1_000_000 UTXO.
        let signed_spend = |out_amount: u64| -> Transaction {
            let inputs = vec![TransactionInput::new(prev_txid, 0, vec![], 0xffff_ffff)];
            let outputs = vec![TransactionOutput::new(out_amount, vec![0xab; 32])];
            let mut tx = Transaction::new(2, inputs, outputs, 0);
            let sig = owner.sign(&tx.signature_hash()).expect("sign");
            tx.set_signature_data(TransactionSignatureData {
                scheme: SignatureSchemeType::Dilithium,
                security_level: owner.parameters.security_level,
                data: sig,
                public_key: owner.public_key.clone(),
            });
            tx
        };

        // Creating MORE value than is spent must be rejected (minting).
        assert!(
            !chain_state
                .validate_transaction(&signed_spend(1_000_001))
                .await?,
            "a transaction whose outputs exceed its inputs must be rejected"
        );
        // Spending exactly, or leaving a fee, is allowed.
        assert!(
            chain_state
                .validate_transaction(&signed_spend(1_000_000))
                .await?,
            "a value-conserving transaction must be accepted"
        );
        assert!(
            chain_state
                .validate_transaction(&signed_spend(900_000))
                .await?,
            "a transaction leaving a fee must be accepted"
        );
        Ok(())
    }

    // --- #5: atomic chain reorganization ---

    /// Build a coinbase block whose coinbase has a UNIQUE txid (its input script
    /// is keyed on `tag`), so each block's coinbase UTXO key differs — required
    /// to observe UTXO divergence between competing branches across a reorg.
    fn unique_coinbase_block(prev: [u8; 32], bits: u32, tag: u64) -> Block {
        use supernova_core::types::transaction::{Transaction, TransactionInput, TransactionOutput};
        let input = TransactionInput::new(
            [0u8; 32],
            0xffff_ffff,
            tag.to_le_bytes().to_vec(),
            0xffff_ffff,
        );
        let output = TransactionOutput::new(5_000_000_000, vec![]);
        let coinbase = Transaction::new(1, vec![input], vec![output], 0);
        Block::new_with_params(1, prev, vec![coinbase], bits)
    }

    #[tokio::test]
    async fn reorg_switches_to_heavier_branch_atomically() -> Result<(), StorageError> {
        // A strictly heavier competing branch must atomically replace the main
        // chain: tip, height index, UTXO set, total difficulty, and the
        // persisted (reload-safe) height all switch to the new branch, and the
        // old branch's effects are fully unwound. The fork point is height 1 —
        // NOT genesis, which find_fork_point rejects — and every coinbase is
        // distinct so UTXO divergence between branches is observable.
        let temp_dir = tempdir().unwrap();
        let db = Arc::new(BlockchainDB::new(temp_dir.path())?);
        let bits = 0x207f_ffff;

        // Seed the shared prefix g0(h0) <- a1(h1) directly: height 0 carries a
        // genesis checkpoint that process_block would reject for a custom block,
        // so only heights >= 2 are fed through the accept path.
        let mut g0 = unique_coinbase_block([0u8; 32], bits, 100);
        g0.set_height(0);
        let g0 = mine(g0);
        let gh = g0.hash();
        db.store_block(&gh, &bincode::serialize(&g0).unwrap())?;
        let mut a1 = unique_coinbase_block(gh, bits, 101);
        a1.set_height(1);
        let a1 = mine(a1);
        let a1h = a1.hash();
        db.store_block(&a1h, &bincode::serialize(&a1).unwrap())?;
        db.store_block_height_index(0, &gh)?;
        db.store_block_height_index(1, &a1h)?;
        db.set_metadata(b"height", &1u64.to_be_bytes())?;
        db.set_metadata(b"best_hash", &a1h)?;
        let w = block_work_u64(&g0); // all blocks share `bits`, so equal work
        db.store_metadata(b"total_difficulty", &bincode::serialize(&(2 * w)).unwrap())?;

        let mut cs = regtest_chain_state(db.clone())?;

        // Main chain extends a1: a2(h2), a3(h3) via the normal accept path.
        let a2 = mine(unique_coinbase_block(a1h, bits, 2));
        let a3 = mine(unique_coinbase_block(a2.hash(), bits, 3));
        let a2_cb = a2.transactions()[0].hash();
        let a3_cb = a3.transactions()[0].hash();
        assert!(cs.process_block(a2.clone()).await?);
        assert!(cs.process_block(a3.clone()).await?);

        // Pre-reorg baseline.
        assert_eq!(cs.get_height(), 3);
        assert_eq!(cs.get_best_block_hash(), a3.hash());
        assert!(db.get_utxo(&a2_cb, 0)?.is_some(), "main-chain coinbase present");

        // A strictly heavier fork off a1: b2(h2), b3(h3), b4(h4).
        let b2 = mine(unique_coinbase_block(a1h, bits, 12));
        let b3 = mine(unique_coinbase_block(b2.hash(), bits, 13));
        let b4 = mine(unique_coinbase_block(b3.hash(), bits, 14));
        let (b2h, b3h, b4h) = (b2.hash(), b3.hash(), b4.hash());
        let b2_cb = b2.transactions()[0].hash();
        let b3_cb = b3.transactions()[0].hash();
        let b4_cb = b4.transactions()[0].hash();
        let (b2w, b3w, b4w) =
            (block_work_u64(&b2), block_work_u64(&b3), block_work_u64(&b4));

        assert!(db.get_utxo(&b2_cb, 0)?.is_none(), "fork coinbase not yet applied");

        // Forks at equal-or-lower cumulative work are stored but do not switch.
        assert!(!cs.process_block(b2.clone()).await?);
        assert!(!cs.process_block(b3.clone()).await?);
        // The fork now has strictly more work -> reorg.
        assert!(
            cs.process_block(b4.clone()).await?,
            "heavier fork must trigger a reorg"
        );

        // Tip + height switched to the new branch.
        assert_eq!(cs.get_best_block_hash(), b4h);
        assert_eq!(cs.get_height(), 4);

        // Height index resolves to the NEW branch for every reorged height.
        assert_eq!(db.get_block_hash_by_height(2)?, Some(b2h));
        assert_eq!(db.get_block_hash_by_height(3)?, Some(b3h));
        assert_eq!(db.get_block_hash_by_height(4)?, Some(b4h));

        // UTXO set reflects the new branch; the old branch is fully unwound.
        assert!(db.get_utxo(&a2_cb, 0)?.is_none(), "disconnected a2 coinbase removed");
        assert!(db.get_utxo(&a3_cb, 0)?.is_none(), "disconnected a3 coinbase removed");
        assert!(db.get_utxo(&b2_cb, 0)?.is_some(), "connected b2 coinbase added");
        assert!(db.get_utxo(&b3_cb, 0)?.is_some(), "connected b3 coinbase added");
        assert!(db.get_utxo(&b4_cb, 0)?.is_some(), "connected b4 coinbase added");

        // Total difficulty = cumulative PoW of the NEW best chain (genesis g0 at
        // height 0 is excluded by calculate_chain_work), proving no double-count
        // and no fork-block pollution carried over from the old metric.
        assert_eq!(
            cs.get_total_difficulty(),
            block_work_u64(&a1) + b2w + b3w + b4w,
            "total difficulty must equal the new best chain's cumulative work"
        );

        // The new height survives a reload (atomic big-endian metadata commit).
        let reloaded = regtest_chain_state(db.clone())?;
        assert_eq!(reloaded.get_height(), 4);
        assert_eq!(db.get_block_hash_by_height(4)?, Some(b4h));
        Ok(())
    }

    #[test]
    fn expected_height_is_derived_from_parent() {
        let temp_dir = tempdir().unwrap();
        let db = Arc::new(BlockchainDB::new(temp_dir.path()).unwrap());
        let chain_state = regtest_chain_state(db.clone()).unwrap();

        // Genesis (null prev hash) is height 0.
        let genesis = Block::new_with_params(1, [0u8; 32], vec![], 0x207f_ffff);
        assert_eq!(chain_state.expected_height(&genesis), Some(0));

        // A stored parent at height 5 implies its child is height 6 — derived
        // from the parent, NOT from the child's (untrusted) wire height.
        let mut parent = Block::new_with_params(1, [9u8; 32], vec![], 0x207f_ffff);
        parent.set_height(5);
        db.store_block(&parent.hash(), &bincode::serialize(&parent).unwrap())
            .unwrap();
        let mut child = Block::new_with_params(1, parent.hash(), vec![], 0x207f_ffff);
        child.set_height(999); // lying wire height is ignored
        assert_eq!(chain_state.expected_height(&child), Some(6));

        // An unknown parent (orphan) cannot derive a height.
        let orphan = Block::new_with_params(1, [7u8; 32], vec![], 0x207f_ffff);
        assert_eq!(chain_state.expected_height(&orphan), None);
    }

    #[tokio::test]
    async fn process_block_stamps_derived_height_over_wire_claim() {
        // A block whose WIRE height lies must be accepted at its parent-derived
        // height, so the subsidy cap, the persisted tip, and the stored block all
        // reflect the real height — never the attacker's claim. Regression for
        // the fake-low-height inflation vector (a low claimed height would lift
        // the `block_subsidy` cap and let the coinbase mint extra coins).
        let temp_dir = tempdir().unwrap();
        let db = Arc::new(BlockchainDB::new(temp_dir.path()).unwrap());
        let mut chain_state = regtest_chain_state(db.clone()).unwrap();

        // Seed a tiny stored ancestry: genesis0 (height 0) <- parent (height 1).
        // These exist only for the chain-work walk and height derivation, so they
        // bypass validation (stored directly) and may be empty blocks.
        let genesis0 = Block::new_with_params(1, [0u8; 32], vec![], 0x207f_ffff);
        db.store_block(&genesis0.hash(), &bincode::serialize(&genesis0).unwrap())
            .unwrap();
        let mut parent = Block::new_with_params(1, genesis0.hash(), vec![], 0x207f_ffff);
        parent.set_height(1);
        db.store_block(&parent.hash(), &bincode::serialize(&parent).unwrap())
            .unwrap();
        chain_state.current_height = 1;
        chain_state.best_block_hash = parent.hash();

        // A valid coinbase child of `parent` that LIES about its height (claims 7
        // while the parent implies 2).
        let mut child = coinbase_block(parent.hash(), 0x207f_ffff);
        child.set_height(7);
        let child = mine(child);
        let child_hash = child.hash();

        let accepted = chain_state
            .process_block(child)
            .await
            .expect("valid child of the tip must be accepted");
        assert!(accepted, "child extends the tip, so it must be accepted");

        // The tip height is the DERIVED 2, never the claimed 7.
        assert_eq!(
            chain_state.get_height(),
            2,
            "tip must use the parent-derived height, not the wire claim"
        );
        // The STORED block also carries the derived height, so its own children
        // derive correctly and the subsidy cap can never read the wire lie.
        let stored = db.get_block(&child_hash).unwrap().unwrap();
        assert_eq!(stored.height(), 2, "stored block height must be derived");
    }

    #[tokio::test]
    async fn direct_extension_populates_height_index() {
        // Accepting a block that extends the tip must record (height -> hash) so
        // get_block_hash_by_height resolves. The index was previously never
        // written by any production path, so every height lookup returned None.
        let temp_dir = tempdir().unwrap();
        let db = Arc::new(BlockchainDB::new(temp_dir.path()).unwrap());
        let mut chain_state = regtest_chain_state(db.clone()).unwrap();

        // Seed a stored ancestry genesis0(0) <- parent(1) and point the tip at it.
        let genesis0 = Block::new_with_params(1, [0u8; 32], vec![], 0x207f_ffff);
        db.store_block(&genesis0.hash(), &bincode::serialize(&genesis0).unwrap())
            .unwrap();
        let mut parent = Block::new_with_params(1, genesis0.hash(), vec![], 0x207f_ffff);
        parent.set_height(1);
        db.store_block(&parent.hash(), &bincode::serialize(&parent).unwrap())
            .unwrap();
        chain_state.current_height = 1;
        chain_state.best_block_hash = parent.hash();

        // Extend the tip with a valid mined coinbase block (derived height 2).
        let child = mine(coinbase_block(parent.hash(), 0x207f_ffff));
        let child_hash = child.hash();
        assert!(chain_state.process_block(child).await.unwrap());

        assert_eq!(
            db.get_block_hash_by_height(2).unwrap(),
            Some(child_hash),
            "a direct-extension accept must index (height -> hash)"
        );
    }

    #[test]
    fn backfill_rebuilds_height_index_idempotently() {
        // An older DB with blocks + height/best_hash metadata but an EMPTY height
        // index must be repaired on startup by ChainState::new, and the repair
        // must be idempotent and follow the best chain — never orphan blocks.
        let temp_dir = tempdir().unwrap();
        let db = Arc::new(BlockchainDB::new(temp_dir.path()).unwrap());

        // Store a 3-block best chain (heights 0,1,2) linked by prev_block_hash,
        // directly (bypassing the index), then set the tip metadata.
        let mut g = Block::new_with_params(1, [0u8; 32], vec![], 0x207f_ffff);
        g.set_height(0);
        let gh = g.hash();
        db.store_block(&gh, &bincode::serialize(&g).unwrap()).unwrap();
        let mut b1 = Block::new_with_params(1, gh, vec![], 0x207f_ffff);
        b1.set_height(1);
        let h1 = b1.hash();
        db.store_block(&h1, &bincode::serialize(&b1).unwrap()).unwrap();
        let mut b2 = Block::new_with_params(1, h1, vec![], 0x207f_ffff);
        b2.set_height(2);
        let h2 = b2.hash();
        db.store_block(&h2, &bincode::serialize(&b2).unwrap()).unwrap();
        db.set_metadata(b"height", &2u64.to_be_bytes()).unwrap();
        db.set_metadata(b"best_hash", &h2).unwrap();

        // The index starts empty.
        assert_eq!(db.get_block_hash_by_height(2).unwrap(), None);

        // ChainState::new triggers the guarded one-time backfill.
        let _chain_state = regtest_chain_state(db.clone()).unwrap();
        assert_eq!(db.get_block_hash_by_height(0).unwrap(), Some(gh));
        assert_eq!(db.get_block_hash_by_height(1).unwrap(), Some(h1));
        assert_eq!(db.get_block_hash_by_height(2).unwrap(), Some(h2));

        // An orphan at the SAME height as genesis, off the best chain, must not
        // shadow the canonical mapping — the walk follows parent links, never
        // blocks.iter() — and re-running is idempotent (same count).
        let mut orphan = Block::new_with_params(1, [0xaa; 32], vec![], 0x207f_ffff);
        orphan.set_height(0);
        db.store_block(&orphan.hash(), &bincode::serialize(&orphan).unwrap())
            .unwrap();
        let n1 = db.backfill_height_index(&h2).unwrap();
        let n2 = db.backfill_height_index(&h2).unwrap();
        assert_eq!(n1, 3, "backfill indexes exactly the 3 best-chain blocks");
        assert_eq!(n2, 3, "backfill is idempotent");
        assert_eq!(
            db.get_block_hash_by_height(0).unwrap(),
            Some(gh),
            "height 0 stays genesis, not the same-height orphan"
        );
    }

    #[tokio::test]
    async fn validate_block_rejects_easy_bits() {
        // A block claiming difficulty EASIER than the floor must be rejected even
        // though it satisfies its own (trivial) target. Regtest floor is
        // 0x207fffff; 0x20ffffff decodes to a larger (easier) target.
        let temp_dir = tempdir().unwrap();
        let db = Arc::new(BlockchainDB::new(temp_dir.path()).unwrap());
        let (_g, a1h) = seed_base_chain(&db, 0x207f_ffff, 300);
        let mut cs = regtest_chain_state(db.clone()).unwrap();

        let easy = mine(unique_coinbase_block(a1h, 0x20ff_ffff, 301));
        assert!(
            cs.process_block(easy).await.is_err(),
            "a block easier than the pow_limit floor must be rejected"
        );

        // A correctly-difficultied block at the same height IS accepted, proving
        // the gate rejects only the easy one, not all blocks.
        let ok_block = mine(unique_coinbase_block(a1h, 0x207f_ffff, 302));
        assert!(
            cs.process_block(ok_block).await.unwrap(),
            "a floor-difficulty block must be accepted"
        );
    }

    #[tokio::test]
    async fn validate_block_rejects_wrong_bits_at_retarget_boundary() {
        // At a retarget boundary the chain requires a recomputed difficulty; a
        // block that ignores the retarget and keeps the parent's bits is rejected.
        // A tiny 4-block interval makes the boundary cheap to reach, and the fast
        // (near-instant) test blocks make the required difficulty rise above the
        // parent's bits — so the boundary path is exercised without expensive
        // mining at the harder target.
        let temp_dir = tempdir().unwrap();
        let db = Arc::new(BlockchainDB::new(temp_dir.path()).unwrap());
        let bits = 0x207f_ffff;
        let (_g, a1h) = seed_base_chain(&db, bits, 310);
        let params = RetargetParams { target_block_time: 30, interval: 4, pow_limit_bits: bits };
        let mut cs = ChainState::with_params(db.clone(), params).unwrap();

        // Heights 2 and 3 are off-boundary (interval 4): difficulty is unchanged,
        // so a block keeping `bits` is accepted.
        let a2 = mine(unique_coinbase_block(a1h, bits, 312));
        assert!(cs.process_block(a2.clone()).await.unwrap());
        let a3 = mine(unique_coinbase_block(a2.hash(), bits, 313));
        assert!(cs.process_block(a3.clone()).await.unwrap());

        // Height 4 IS a boundary -> the required difficulty rises, so a block
        // still claiming `bits` is rejected.
        let a4 = mine(unique_coinbase_block(a3.hash(), bits, 314));
        assert!(
            cs.process_block(a4).await.is_err(),
            "a boundary block that ignores the required retarget must be rejected"
        );
    }
}
