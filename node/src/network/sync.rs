use crate::storage::ChainState;
use btclib::types::Block;
use std::collections::HashMap;
use std::time::{Duration, SystemTime};
use tokio::sync::mpsc;
use tracing::{info, warn};

const MAX_BLOCKS_PER_REQUEST: u64 = 500;
const SYNC_TIMEOUT: Duration = Duration::from_secs(30);

pub struct ChainSync {
    chain_state: ChainState,
    sync_state: SyncState,
    pending_blocks: HashMap<u64, Block>,
    highest_seen: u64,
    command_sender: mpsc::Sender<super::NetworkCommand>,
}

enum SyncState {
    Idle,
    Syncing {
        start_height: u64,
        end_height: u64,
        request_time: SystemTime,
    },
}

impl ChainSync {
    pub fn new(chain_state: ChainState, command_sender: mpsc::Sender<super::NetworkCommand>) -> Self {
        Self {
            chain_state,
            sync_state: SyncState::Idle,
            pending_blocks: HashMap::new(),
            highest_seen: 0,
            command_sender,
        }
    }

    pub async fn handle_new_block(&mut self, block: Block, height: u64, total_difficulty: u64) -> Result<(), String> {
        // Update highest seen block
        if height > self.highest_seen {
            self.highest_seen = height;
        }

        // If we're syncing, add to pending blocks
        match self.sync_state {
            SyncState::Syncing { start_height, end_height, .. } => {
                if height >= start_height && height <= end_height {
                    self.pending_blocks.insert(height, block);
                    self.try_process_pending_blocks().await?;
                }
            }
            SyncState::Idle => {
                // If not syncing, process block immediately
                if height == self.chain_state.get_height() + 1 {
                    self.chain_state.store_block(block)?;
                } else if height > self.chain_state.get_height() + 1 {
                    // We're behind, start syncing
                    self.start_sync(height).await?;
                }
            }
        }

        Ok(())
    }

    pub async fn start_sync(&mut self, target_height: u64) -> Result<(), String> {
        let current_height = self.chain_state.get_height();
        if target_height <= current_height {
            return Ok(());
        }

        let start_height = current_height + 1;
        let end_height = std::cmp::min(
            start_height + MAX_BLOCKS_PER_REQUEST - 1,
            target_height
        );

        info!("Starting sync from {} to {}", start_height, end_height);

        self.sync_state = SyncState::Syncing {
            start_height,
            end_height,
            request_time: SystemTime::now(),
        };

        // Request blocks
        self.request_blocks(start_height, end_height).await?;

        Ok(())
    }

    async fn request_blocks(&mut self, start_height: u64, end_height: u64) -> Result<(), String> {
        self.command_sender
            .send(super::NetworkCommand::RequestBlocks {
                start_height,
                end_height,
            })
            .await
            .map_err(|e| format!("Failed to send block request: {}", e))?;

        Ok(())
    }

    async fn try_process_pending_blocks(&mut self) -> Result<(), String> {
        let current_height = self.chain_state.get_height();

        while let Some(block) = self.pending_blocks.remove(&(current_height + 1)) {
            self.chain_state.store_block(block)?;
        }

        // Check if we've completed current sync range
        if let SyncState::Syncing { end_height, .. } = self.sync_state {
            if self.chain_state.get_height() >= end_height {
                // If we've reached our target, go idle
                if self.chain_state.get_height() >= self.highest_seen {
                    self.sync_state = SyncState::Idle;
                    info!("Sync completed at height {}", self.chain_state.get_height());
                } else {
                    // Otherwise, start next sync range
                    self.start_sync(self.highest_seen).await?;
                }
            }
        }

        Ok(())
    }

    pub fn check_timeouts(&mut self) {
        if let SyncState::Syncing { start_height, end_height, request_time } = self.sync_state {
            if SystemTime::now().duration_since(request_time).unwrap() > SYNC_TIMEOUT {
                warn!("Sync request timed out for range {} to {}", start_height, end_height);
                // Reset sync state and try again
                self.sync_state = SyncState::Idle;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[tokio::test]
    async fn test_sync_flow() {
        // Create test dependencies
        let db = Arc::new(BlockchainDB::new(tempfile::tempdir().unwrap().path()).unwrap());
        let chain_state = ChainState::new(db).unwrap();
        let (tx, _rx) = mpsc::channel(32);
        
        let mut sync = ChainSync::new(chain_state, tx);

        // Test sync initiation
        sync.start_sync(1000).await.unwrap();
        
        match sync.sync_state {
            SyncState::Syncing { start_height, end_height, .. } => {
                assert_eq!(start_height, 1);
                assert_eq!(end_height, 500); // MAX_BLOCKS_PER_REQUEST
            }
            _ => panic!("Expected Syncing state"),
        }
    }

    #[tokio::test]
    async fn test_block_processing() {
        // Create test dependencies
        let db = Arc::new(BlockchainDB::new(tempfile::tempdir().unwrap().path()).unwrap());
        let chain_state = ChainState::new(db).unwrap();
        let (tx, _rx) = mpsc::channel(32);
        
        let mut sync = ChainSync::new(chain_state, tx);

        // Test processing a new block
        let block = Block::new(1, [0u8; 32], Vec::new(), 0);
        sync.handle_new_block(block, 1, 100).await.unwrap();
        
        assert_eq!(sync.highest_seen, 1);
    }
}