// Network partition resilience tests
//
// This file contains tests for network partition scenarios, testing how the network
// recovers when partitioned and how fork resolution works in such scenarios.

use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;
use tracing::{debug, info, warn};

use crate::network::{NetworkSimulator, NetworkCondition, NodeHandle};
use crate::storage::BlockchainDB;
use crate::chain::{ChainState, Block, Transaction};
use crate::mining::Miner;
use crate::types::Hash;

// Helper function to create a test network with multiple nodes
async fn create_test_network(node_count: usize) -> (Vec<NodeHandle>, NetworkSimulator) {
    let mut nodes = Vec::with_capacity(node_count);

    // Create nodes
    for i in 0..node_count {
        let db = Arc::new(BlockchainDB::create_in_memory().unwrap());
        let chain_state = ChainState::new(Arc::clone(&db)).unwrap();
        let node = NodeHandle::new(format!("node-{}", i), db, chain_state).await;
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

// Test network partition recovery
#[tokio::test]
async fn test_network_partition_recovery() {
    // Create a test network with 6 nodes
    let (nodes, mut simulator) = create_test_network(6).await;

    // Mine some initial blocks to establish a common chain
    mine_blocks_on_network(&simulator, 5).await;

    // Wait for all nodes to sync
    wait_for_sync(&simulator).await;

    // Create two partitions: nodes 0-2 and nodes 3-5
    let partition_a = vec![0, 1, 2];
    let partition_b = vec![3, 4, 5];

    info!("Creating network partition between groups");
    simulator.create_partition(partition_a.as_slice(), partition_b.as_slice()).await;

    // Mine blocks on both partitions
    info!("Mining blocks on partition A");
    mine_blocks_on_partition(&simulator, &partition_a, 5).await;

    info!("Mining blocks on partition B");
    mine_blocks_on_partition(&simulator, &partition_b, 3).await;

    // Verify partitions have different chain tips
    let tip_a = get_best_block_hash(&simulator, partition_a[0]).await;
    let tip_b = get_best_block_hash(&simulator, partition_b[0]).await;

    assert_ne!(tip_a, tip_b, "Partitions should have different chain tips");

    // Heal partition
    info!("Healing network partition");
    simulator.heal_partition(partition_a.as_slice(), partition_b.as_slice()).await;

    // Wait for sync
    wait_for_sync(&simulator).await;

    // Verify all nodes converged on same chain tip (should be the longer chain)
    let expected_tip = get_best_block_hash(&simulator, 0).await;
    for i in 1..nodes.len() {
        let node_tip = get_best_block_hash(&simulator, i).await;
        assert_eq!(node_tip, expected_tip, "Node {} did not converge on expected chain tip", i);
    }

    // Verify the winning chain is the longer one (partition A's chain)
    let heights = simulator.get_node_heights().await;
    assert_eq!(heights[0], 10, "Expected chain height after recovery should be 10");
}

// Test network partition with equal length chains
#[tokio::test]
async fn test_network_partition_equal_chains() {
    // Create a test network with 6 nodes
    let (nodes, mut simulator) = create_test_network(6).await;

    // Mine some initial blocks to establish a common chain
    mine_blocks_on_network(&simulator, 5).await;

    // Wait for all nodes to sync
    wait_for_sync(&simulator).await;

    // Create two partitions: nodes 0-2 and nodes 3-5
    let partition_a = vec![0, 1, 2];
    let partition_b = vec![3, 4, 5];

    info!("Creating network partition between groups");
    simulator.create_partition(partition_a.as_slice(), partition_b.as_slice()).await;

    // Mine equal number of blocks on both partitions
    info!("Mining blocks on partition A");
    mine_blocks_on_partition(&simulator, &partition_a, 4).await;

    info!("Mining blocks on partition B");
    mine_blocks_on_partition(&simulator, &partition_b, 4).await;

    // Verify partitions have different chain tips
    let tip_a = get_best_block_hash(&simulator, partition_a[0]).await;
    let tip_b = get_best_block_hash(&simulator, partition_b[0]).await;

    assert_ne!(tip_a, tip_b, "Partitions should have different chain tips");

    // Heal partition
    info!("Healing network partition");
    simulator.heal_partition(partition_a.as_slice(), partition_b.as_slice()).await;

    // Wait for sync
    wait_for_sync(&simulator).await;

    // Verify all nodes converged on same chain tip (should be based on cumulative difficulty)
    let tip_after_healing = get_best_block_hash(&simulator, 0).await;
    for i in 1..nodes.len() {
        let node_tip = get_best_block_hash(&simulator, i).await;
        assert_eq!(node_tip, tip_after_healing, "Node {} did not converge on expected chain tip", i);
    }

    // All nodes should have height 9
    let heights = simulator.get_node_heights().await;
    assert_eq!(heights[0], 9, "Expected chain height after recovery should be 9");
}

// Test three-way network partition and recovery
#[tokio::test]
async fn test_three_way_network_partition() {
    // Create a test network with 9 nodes
    let (nodes, mut simulator) = create_test_network(9).await;

    // Mine some initial blocks to establish a common chain
    mine_blocks_on_network(&simulator, 5).await;

    // Wait for all nodes to sync
    wait_for_sync(&simulator).await;

    // Create three partitions
    let partition_a = vec![0, 1, 2];
    let partition_b = vec![3, 4, 5];
    let partition_c = vec![6, 7, 8];

    info!("Creating three-way network partition");
    simulator.create_partition(partition_a.as_slice(), partition_b.as_slice()).await;
    simulator.create_partition(partition_a.as_slice(), partition_c.as_slice()).await;
    simulator.create_partition(partition_b.as_slice(), partition_c.as_slice()).await;

    // Mine different numbers of blocks on each partition
    info!("Mining blocks on partition A (6 blocks)");
    mine_blocks_on_partition(&simulator, &partition_a, 6).await;

    info!("Mining blocks on partition B (4 blocks)");
    mine_blocks_on_partition(&simulator, &partition_b, 4).await;

    info!("Mining blocks on partition C (2 blocks)");
    mine_blocks_on_partition(&simulator, &partition_c, 2).await;

    // Verify each partition has a different tip
    let tip_a = get_best_block_hash(&simulator, partition_a[0]).await;
    let tip_b = get_best_block_hash(&simulator, partition_b[0]).await;
    let tip_c = get_best_block_hash(&simulator, partition_c[0]).await;

    assert_ne!(tip_a, tip_b, "Partitions A and B should have different chain tips");
    assert_ne!(tip_b, tip_c, "Partitions B and C should have different chain tips");
    assert_ne!(tip_a, tip_c, "Partitions A and C should have different chain tips");

    // Heal all partitions
    info!("Healing all network partitions");
    simulator.heal_partition(partition_a.as_slice(), partition_b.as_slice()).await;
    simulator.heal_partition(partition_a.as_slice(), partition_c.as_slice()).await;
    simulator.heal_partition(partition_b.as_slice(), partition_c.as_slice()).await;

    // Wait for sync
    wait_for_sync(&simulator).await;

    // Verify all nodes converged on partition A's chain (the longest)
    let expected_tip = get_best_block_hash(&simulator, partition_a[0]).await;
    for i in 0..nodes.len() {
        let node_tip = get_best_block_hash(&simulator, i).await;
        assert_eq!(node_tip, expected_tip, "Node {} did not converge on expected chain tip", i);
    }

    // Verify the height is consistent with partition A's chain
    let heights = simulator.get_node_heights().await;
    assert_eq!(heights[0], 11, "Expected chain height after recovery should be 11");
}

// Helper functions

async fn wait_for_sync(simulator: &NetworkSimulator) {
    // Give nodes time to sync
    sleep(Duration::from_millis(500)).await;

    // Check if all nodes have the same chain height
    for _ in 0..10 {
        let heights = simulator.get_node_heights().await;
        let first_height = heights[0];

        let all_synced = heights.iter().all(|&h| h == first_height);
        if all_synced {
            return;
        }

        // Wait a bit longer
        sleep(Duration::from_millis(500)).await;
    }

    // Just return - test assertions will catch any sync issues
}

async fn mine_blocks_on_network(simulator: &NetworkSimulator, count: u64) {
    // Mine blocks on node 0
    mine_blocks_on_partition(simulator, &[0], count).await;
}

async fn mine_blocks_on_partition(simulator: &NetworkSimulator, nodes: &[usize], count: u64) {
    let node_id = nodes[0]; // Use first node in partition for mining

    for _ in 0..count {
        simulator.mine_block(node_id).await;

        // Give some time for block propagation within partition
        sleep(Duration::from_millis(100)).await;
    }
}

async fn get_best_block_hash(simulator: &NetworkSimulator, node_id: usize) -> Hash {
    simulator.get_best_block_hash(node_id).await
}