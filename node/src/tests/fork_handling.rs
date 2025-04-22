use crate::storage::persistence::{ChainState, ForkChoiceReason};
use crate::storage::database::BlockchainDB;
use crate::network::sync::ChainSync;
use btclib::types::block::Block;
use tempfile::tempdir;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;

/// Create a test chain with a specific number of blocks
async fn create_test_chain(block_count: u64) -> (Arc<BlockchainDB>, ChainState, Vec<Block>) {
    let temp_dir = tempdir().unwrap();
    let db = Arc::new(BlockchainDB::new(temp_dir.path()).unwrap());
    let mut chain_state = ChainState::new(db.clone()).unwrap();
    
    // Create a genesis block with a known hash
    let genesis = Block::new(1, [0u8; 32], vec![], u32::MAX);
    chain_state.store_block(genesis.clone()).unwrap();
    
    // Update initial chain state with the genesis block
    chain_state.current_height = 1;
    chain_state.best_block_hash = genesis.hash();
    
    let mut blocks = vec![genesis];
    
    // Add blocks to the chain
    for i in 1..block_count {
        let prev_block = &blocks[i as usize - 1];
        let new_block = Block::new(
            (i + 1) as u32,
            prev_block.hash(),
            vec![],
            u32::MAX
        );
        
        // Store and process the block
        chain_state.process_block(new_block.clone()).await.unwrap();
        blocks.push(new_block);
    }
    
    (db, chain_state, blocks)
}

/// Create a competing fork at the specified height with a specified number of blocks
async fn create_fork(chain_state: &mut ChainState, main_chain: &[Block], fork_height: u64, blocks: u64, lower_difficulty: bool) -> Vec<Block> {
    let fork_base_idx = fork_height as usize - 1;
    let fork_base = &main_chain[fork_base_idx];
    
    let mut fork_blocks = vec![];
    let target = if lower_difficulty {
        u32::MAX  // Lower difficulty (easier target)
    } else {
        u32::MAX / 2  // Higher difficulty (harder target)
    };
    
    // Create first block in fork with different target
    let mut current = Block::new(
        (fork_height + 1) as u32,
        fork_base.hash(),
        vec![],
        target
    );
    
    chain_state.process_block(current.clone()).await.unwrap();
    fork_blocks.push(current.clone());
    
    // Add more blocks to the fork
    for i in 1..blocks {
        current = Block::new(
            (fork_height + 1 + i) as u32,
            current.hash(),
            vec![],
            target
        );
        
        chain_state.process_block(current.clone()).await.unwrap();
        fork_blocks.push(current.clone());
    }
    
    fork_blocks
}

#[tokio::test]
async fn test_basic_fork_handling() {
    // Create a chain with 10 blocks
    let (db, mut chain_state, main_chain) = create_test_chain(10).await;
    
    // Create a competing fork at height 5 with 3 blocks
    let fork_blocks = create_fork(&mut chain_state, &main_chain, 5, 3, false).await;
    
    // Since the fork has higher difficulty, it should be the main chain now
    assert_eq!(chain_state.get_best_block_hash(), fork_blocks.last().unwrap().hash());
    assert_eq!(chain_state.get_height(), 8);  // Height 5 + 3 new blocks
    
    // The main chain should be in active forks
    let fork_metrics = chain_state.calculate_fork_metrics();
    assert!(fork_metrics.get("active_forks").unwrap() > &0);
}

#[tokio::test]
async fn test_fork_choice_reasons() {
    // Create a chain with 10 blocks
    let (db, mut chain_state, main_chain) = create_test_chain(10).await;
    
    // Create a competing fork at height 5 with 3 blocks (higher difficulty)
    let fork_blocks = create_fork(&mut chain_state, &main_chain, 5, 3, false).await;
    
    // The fork should be selected due to higher chain work
    assert_eq!(chain_state.get_best_block_hash(), fork_blocks.last().unwrap().hash());
    
    // Create another fork with equal difficulty but different data to avoid hash collisions
    let current_height = chain_state.get_height();
    let prev_hash = chain_state.get_best_block_hash();
    
    // First block has different transactions to avoid hash collision
    let mut equal_work_block = Block::new(
        (current_height + 1) as u32,
        prev_hash,
        vec![],  // Empty tx list to make different hash
        u32::MAX / 2
    );
    
    // This should not trigger a reorg since it's equal work but our fork was seen first
    chain_state.process_block(equal_work_block.clone()).await.unwrap();
    
    // The original fork should still be selected
    assert_eq!(chain_state.get_best_block_hash(), fork_blocks.last().unwrap().hash());
    
    // Now create a higher difficulty fork
    let higher_work_block = Block::new(
        (current_height + 1) as u32,
        prev_hash,
        vec![],
        u32::MAX / 4  // Even higher difficulty
    );
    
    // This should trigger a reorg due to higher chain work
    chain_state.process_block(higher_work_block.clone()).await.unwrap();
    
    // The new block should be selected due to higher chain work
    assert_eq!(chain_state.get_best_block_hash(), higher_work_block.hash());
}

#[tokio::test]
async fn test_deep_reorganization_limits() {
    // Create a chain with 150 blocks
    let (db, mut chain_state, main_chain) = create_test_chain(150).await;
    
    // Try to create a fork at height 10 with 150 blocks
    // This should exceed the MAX_REORG_DEPTH limit
    let fork_blocks = create_fork(&mut chain_state, &main_chain, 10, 150, false).await;
    
    // Despite higher difficulty, the reorg should be rejected due to depth
    assert_ne!(chain_state.get_best_block_hash(), fork_blocks.last().unwrap().hash());
    
    // The original chain should still be the best chain
    assert_eq!(chain_state.get_best_block_hash(), main_chain.last().unwrap().hash());
    
    // Check if the rejected_reorgs metric increased
    let fork_metrics = chain_state.calculate_fork_metrics();
    assert!(fork_metrics.get("rejected_reorgs").unwrap() > &0);
}

#[tokio::test]
async fn test_fork_synchronization() {
    // Create a chain with 10 blocks
    let (db, mut chain_state, main_chain) = create_test_chain(10).await;
    
    // Create a command channel for the ChainSync
    let (tx, _rx) = mpsc::channel(10);
    
    // Create ChainSync instance
    let mut sync = ChainSync::new(chain_state.clone(), db.clone(), tx);
    
    // Create a competing fork at height 5 with 6 blocks
    let fork_blocks = create_fork(&mut chain_state, &main_chain, 5, 6, false).await;
    
    // Process blocks from the fork with the sync system
    for block in &fork_blocks {
        sync.handle_new_block(block.clone(), block.height(), 10000, None)
            .await
            .unwrap();
    }
    
    // Check if sync detected and adjusted to the fork
    let sync_stats = sync.get_stats();
    assert_eq!(sync_stats.current_height, 11); // Height 5 + 6 fork blocks
    
    // Check fork metrics
    let fork_stats = sync.get_fork_stats();
    assert!(fork_stats.get("active_forks").unwrap() > &0);
    assert!(fork_stats.get("max_fork_length").unwrap() > &0);
}

#[tokio::test]
async fn test_stale_tip_detection() {
    // Create a chain with 5 blocks
    let (db, mut chain_state, _main_chain) = create_test_chain(5).await;
    
    // Create a command channel for the ChainSync
    let (tx, _rx) = mpsc::channel(10);
    
    // Create ChainSync instance
    let sync = ChainSync::new(chain_state.clone(), db.clone(), tx);
    
    // No new blocks created, so tip shouldn't be stale yet
    assert!(!sync.check_for_stale_tip());
    
    // However, we can manually check the time since last block
    // (this will be very small since we just created the chain)
    let time_since_last = sync.time_since_last_block();
    assert!(time_since_last < Duration::from_secs(10));
}

#[tokio::test]
async fn test_metrics_tracking() {
    // Create a chain with 5 blocks
    let (db, mut chain_state, main_chain) = create_test_chain(5).await;
    
    // Create a command channel for the ChainSync
    let (tx, _rx) = mpsc::channel(10);
    
    // Create ChainSync instance
    let sync = ChainSync::new(chain_state.clone(), db.clone(), tx);
    
    // Initial height
    let initial_height = chain_state.get_height();
    
    // Create a competing fork
    let _fork_blocks = create_fork(&mut chain_state, &main_chain, 3, 3, false).await;
    
    // Get metrics
    let metrics = sync.get_fork_stats();
    
    // Check various metrics
    assert!(metrics.contains_key("main_chain_height"));
    assert!(metrics.contains_key("active_forks"));
    assert!(metrics.contains_key("reorg_count"));
    
    // Assert that the height has changed after adding the fork
    assert!(metrics.get("main_chain_height").unwrap() > &initial_height);
} 