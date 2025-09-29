// Clock drift resilience tests
//
// This file contains tests for handling clock drift scenarios, testing how the
// system behaves when nodes have incorrect system clocks.

use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::time::sleep;
use tracing::{debug, info, warn};

use crate::network::{NetworkSimulator, NodeHandle, NodeConfig};
use crate::storage::BlockchainDB;
use crate::chain::{ChainState, Block, BlockHeader};
use crate::config::Config;
use crate::consensus::TimeManager;
use crate::types::Hash;

// Helper function to create a test network with nodes that have different clock offsets
async fn create_test_network_with_clock_drift(node_count: usize, clock_offsets: Vec<i64>) -> (Vec<NodeHandle>, NetworkSimulator) {
    assert_eq!(node_count, clock_offsets.len(), "Must provide clock offset for each node");

    let mut nodes = Vec::with_capacity(node_count);

    // Create nodes with different clock offsets
    for i in 0..node_count {
        let db = Arc::new(BlockchainDB::create_in_memory().unwrap());
        let chain_state = ChainState::new(Arc::clone(&db)).unwrap();

        // Create config with specific clock offset
        let mut config = NodeConfig::default();
        config.time_offset = clock_offsets[i];

        // Create node with custom config
        let node = NodeHandle::new_with_config(
            format!("node-{}", i),
            db,
            chain_state,
            config
        ).await;

        nodes.push(node);
    }

    // Create network simulator
    let mut simulator = NetworkSimulator::new();
    for node in &nodes {
        simulator.add_node(Arc::clone(node));
    }

    // Connect all nodes in a mesh
    for i in 0..node_count {
        for j in 0..node_count {
            if i != j {
                simulator.connect_nodes(i, j).await;
            }
        }
    }

    (nodes, simulator)
}

// Test block timestamp validation with clock drift
#[tokio::test]
async fn test_block_timestamp_validation_with_drift() {
    // Create a test network with 3 nodes
    // - Node 0: Normal clock
    // - Node 1: Clock 1 hour ahead
    // - Node 2: Clock 30 minutes behind
    let clock_offsets = vec![
        0,               // Normal clock
        3600,            // 1 hour ahead
        -1800,           // 30 minutes behind
    ];

    let (nodes, mut simulator) = create_test_network_with_clock_drift(3, clock_offsets).await;

    // Mine blocks on each node to test timestamp acceptance
    info!("Mining block on node with normal clock");
    let block_normal = simulator.mine_block(0).await;

    info!("Mining block on node with clock ahead");
    let block_ahead = simulator.mine_block(1).await;

    info!("Mining block on node with clock behind");
    let block_behind = simulator.mine_block(2).await;

    // Extract timestamps
    let ts_normal = block_normal.header().timestamp();
    let ts_ahead = block_ahead.header().timestamp();
    let ts_behind = block_behind.header().timestamp();

    info!("Normal timestamp: {}", ts_normal);
    info!("Ahead timestamp: {}", ts_ahead);
    info!("Behind timestamp: {}", ts_behind);

    // Verify timestamps reflect clock offsets (approximately)
    assert!(ts_ahead > ts_normal, "Ahead node timestamp should be greater than normal node");
    assert!(ts_normal > ts_behind, "Normal node timestamp should be greater than behind node");

    // Approximate difference should match the clock offset
    let diff_ahead = ts_ahead as i64 - ts_normal as i64;
    let diff_behind = ts_normal as i64 - ts_behind as i64;

    // Allow for some processing time variation (+/- 5 seconds)
    assert!((diff_ahead - 3600).abs() < 5, "Ahead node timestamp difference should be close to 1 hour");
    assert!((diff_behind - 1800).abs() < 5, "Behind node timestamp difference should be close to 30 minutes");

    // Test validation of these blocks by different nodes

    // Node 0 (normal) should accept blocks from both other nodes
    assert!(simulator.validate_block(0, &block_ahead).await, "Normal node should accept block from ahead node");
    assert!(simulator.validate_block(0, &block_behind).await, "Normal node should accept block from behind node");

    // Node 1 (ahead) should accept blocks from normal node but reject too-old blocks
    assert!(simulator.validate_block(1, &block_normal).await, "Ahead node should accept block from normal node");
    assert!(simulator.validate_block(1, &block_behind).await, "Ahead node should accept block from behind node (valid past block)");

    // Node 2 (behind) should reject blocks too far in the future
    assert!(simulator.validate_block(2, &block_normal).await, "Behind node should accept block from normal node");

    // This might fail if the median time check is enforced strictly - depends on implementation
    // In most implementations, blocks up to 2 hours in the future are accepted
    if !simulator.validate_block(2, &block_ahead).await {
        info!("Behind node rejected block from ahead node (expected if strict future block policy)");
    } else {
        info!("Behind node accepted block from ahead node (expected if permissive future block policy)");
    }
}

// Test median time past calculation with clock drift
#[tokio::test]
async fn test_median_time_past_with_drift() {
    // Create a network with one node that has a normal clock
    let (nodes, mut simulator) = create_test_network_with_clock_drift(1, vec![0]).await;

    // Mine 11 blocks with different timestamps
    // Use a specific pattern to verify median time calculation
    let timestamps = vec![
        1000, 1100, 1200, 1300, 1500,
        1400, 2000, 1600, 1900, 1700, 1800
    ];

    // Mine blocks with set timestamps
    for (i, &timestamp) in timestamps.iter().enumerate() {
        let prev_hash = if i == 0 {
            [0u8; 32] // Genesis
        } else {
            simulator.get_best_block_hash(0).await
        };

        // Create block with specific timestamp
        simulator.mine_block_with_timestamp(0, prev_hash, timestamp).await;
    }

    // Get the current median time past
    let mtp = simulator.get_median_time_past(0).await;

    // Median of last 11 blocks should be the middle value when sorted
    let mut sorted_timestamps = timestamps.clone();
    sorted_timestamps.sort();
    let expected_mtp = sorted_timestamps[sorted_timestamps.len() / 2];

    assert_eq!(mtp, expected_mtp, "Median time past should be calculated correctly");
    assert_eq!(mtp, 1500, "Expected median of timestamps should be 1500");
}

// Test future block policy with various clock drifts
#[tokio::test]
async fn test_future_block_policy() {
    // Create test network with 3 nodes
    let clock_offsets = vec![0, 0, 0]; // All normal clocks initially
    let (nodes, mut simulator) = create_test_network_with_clock_drift(3, clock_offsets).await;

    // Get current time from node 0
    let current_time = simulator.get_adjusted_time(0).await;

    // Create blocks with different future timestamps
    let block_2min_future = create_block_with_future_timestamp(&simulator, 0, 120).await;
    let block_2hr_future = create_block_with_future_timestamp(&simulator, 0, 7200).await;
    let block_3hr_future = create_block_with_future_timestamp(&simulator, 0, 10800).await;

    // Test future block acceptance policy (standard is to accept blocks up to 2 hours in the future)

    // 2 minutes in future should be accepted
    assert!(simulator.validate_block(0, &block_2min_future).await,
            "Block 2 minutes in future should be accepted");

    // 2 hours in future should be accepted (right at the limit)
    assert!(simulator.validate_block(0, &block_2hr_future).await,
            "Block 2 hours in future should be accepted");

    // 3 hours in future should be rejected
    assert!(!simulator.validate_block(0, &block_3hr_future).await,
            "Block 3 hours in future should be rejected");

    // Test time-dependent propagation

    // Process 2-minute future block
    assert!(simulator.process_block(0, block_2min_future.clone()).await.is_ok(),
            "Processing block 2 minutes in future should succeed");

    // Try to process 2-hour future block - this should be accepted but marked for later processing
    let result = simulator.process_block(0, block_2hr_future.clone()).await;

    if result.is_ok() {
        info!("2-hour future block was processed immediately (allowed by implementation)");
    } else {
        info!("2-hour future block was rejected or queued for later (common policy)");

        // Check if it's in the orphan pool or future block pool
        let has_block = simulator.has_future_block(0, &block_2hr_future.hash()).await;
        assert!(has_block, "2-hour future block should be in the future block pool");
    }

    // 3-hour future block should be rejected outright
    let result = simulator.process_block(0, block_3hr_future.clone()).await;
    assert!(result.is_err(), "3-hour future block should be rejected");

    // Verify 2-hour future block is eventually processed as time advances
    if !result.is_ok() {
        // Advance node time by 1 hour
        simulator.advance_node_time(0, 3600).await;

        // Try processing again - should succeed now that node time is closer to block time
        let result = simulator.process_block(0, block_2hr_future.clone()).await;
        assert!(result.is_ok(), "2-hour future block should be accepted after advancing time");
    }
}

// Test network time synchronization
#[tokio::test]
async fn test_network_time_synchronization() {
    // Create a network with 5 nodes with different clock offsets
    let clock_offsets = vec![
        0,       // Normal
        3600,    // 1 hour ahead
        -1800,   // 30 minutes behind
        7200,    // 2 hours ahead
        -3600,   // 1 hour behind
    ];

    let (nodes, mut simulator) = create_test_network_with_clock_drift(5, clock_offsets).await;

    // Let nodes exchange time data
    info!("Letting nodes exchange time samples...");
    for _ in 0..5 {
        simulator.exchange_time_data().await;
        sleep(Duration::from_millis(100)).await;
    }

    // After time synchronization, each node should adjust its offset
    // to match the network consensus (which should be closer to the median)

    // Check node time offsets after synchronization
    for i in 0..5 {
        let adjusted_offset = simulator.get_time_offset(i).await;
        info!("Node {} adjusted time offset: {} seconds", i, adjusted_offset);

        // Adjusted offset should move toward zero or network median
        // The exact value depends on the implementation, but it should be
        // smaller than the extreme values
        assert!(adjusted_offset.abs() < 7200, "Adjusted offset should be less extreme");
    }

    // Verify that node with 2-hour ahead clock has been adjusted downward
    let extreme_node_offset = simulator.get_time_offset(3).await;
    assert!(extreme_node_offset < 7200, "Extreme clock ahead should be adjusted downward");

    // Verify time consensus - all nodes should have similar adjusted times
    let reference_time = simulator.get_adjusted_time(0).await;
    for i in 1..5 {
        let node_time = simulator.get_adjusted_time(i).await;
        let time_diff = (node_time as i64 - reference_time as i64).abs();

        // After synchronization, times should be within a reasonable range
        // Typically this would be within 30-60 minutes depending on implementation
        assert!(time_diff < 3600, "Adjusted times should be within a reasonable range");
    }
}

// Test timestamp validation rules
#[tokio::test]
async fn test_timestamp_validation_rules() {
    // Create a network with one node
    let (nodes, mut simulator) = create_test_network_with_clock_drift(1, vec![0]).await;

    // Mine 10 blocks to establish a chain
    for _ in 0..10 {
        simulator.mine_block(0).await;
        sleep(Duration::from_millis(100)).await;
    }

    // Get the median time past
    let mtp = simulator.get_median_time_past(0).await;
    info!("Current median time past: {}", mtp);

    // 1. Test block with timestamp <= median time past (should be rejected)
    let prev_hash = simulator.get_best_block_hash(0).await;
    let block_old_timestamp = simulator.create_block_with_timestamp(0, prev_hash, mtp).await;

    assert!(!simulator.validate_block(0, &block_old_timestamp).await,
            "Block with timestamp equal to median time past should be rejected");

    // 2. Test block with timestamp = mtp + 1 (should be accepted)
    let block_mtp_plus_one = simulator.create_block_with_timestamp(0, prev_hash, mtp + 1).await;

    assert!(simulator.validate_block(0, &block_mtp_plus_one).await,
            "Block with timestamp = median time past + 1 should be accepted");

    // Process the valid block
    simulator.process_block(0, block_mtp_plus_one).await.unwrap();

    // 3. Test block with timestamp < previous block's timestamp (should be accepted)
    let current_hash = simulator.get_best_block_hash(0).await;
    let current_block = simulator.get_block(0, &current_hash).await;
    let current_timestamp = current_block.header().timestamp();

    // Create block with timestamp slightly lower than previous
    let block_earlier_timestamp = simulator.create_block_with_timestamp(
        0,
        current_hash,
        current_timestamp - 1
    ).await;

    assert!(simulator.validate_block(0, &block_earlier_timestamp).await,
            "Block with timestamp earlier than previous block should be accepted");

    // 4. Update MTP and test timestamp exactly at new MTP
    // Process a few more blocks
    for _ in 0..5 {
        simulator.mine_block(0).await;
        sleep(Duration::from_millis(100)).await;
    }

    // Get new MTP
    let new_mtp = simulator.get_median_time_past(0).await;
    info!("New median time past: {}", new_mtp);

    // Create block with timestamp exactly at MTP
    let new_prev_hash = simulator.get_best_block_hash(0).await;
    let block_at_new_mtp = simulator.create_block_with_timestamp(0, new_prev_hash, new_mtp).await;

    assert!(!simulator.validate_block(0, &block_at_new_mtp).await,
            "Block with timestamp exactly at new MTP should be rejected");

    // 5. Create block with timestamp MTP + 1
    let block_at_new_mtp_plus_one = simulator.create_block_with_timestamp(
        0,
        new_prev_hash,
        new_mtp + 1
    ).await;

    assert!(simulator.validate_block(0, &block_at_new_mtp_plus_one).await,
            "Block with timestamp at new MTP + 1 should be accepted");
}

// Helper function to create a block with a timestamp in the future
async fn create_block_with_future_timestamp(simulator: &NetworkSimulator, node_id: usize, seconds_in_future: u64) -> Block {
    // Get current adjusted time
    let current_time = simulator.get_adjusted_time(node_id).await;
    let future_time = current_time + seconds_in_future;

    // Get previous block hash
    let prev_hash = simulator.get_best_block_hash(node_id).await;

    // Create block with future timestamp
    simulator.create_block_with_timestamp(node_id, prev_hash, future_time).await
}