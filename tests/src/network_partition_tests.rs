use crate::test_framework::{
    create_test_network, 
    simulate_network_partition,
    heal_network_partition,
    mine_blocks_on_partition,
    get_best_block_hash,
    TestNode
};

use btclib::types::transaction::{Transaction, TransactionInput, TransactionOutput};
use std::time::Duration;
use tracing::{info, debug};

#[tokio::test]
async fn test_basic_network_partition() -> Result<(), Box<dyn std::error::Error>> {
    // Create a test network with 6 nodes
    let (nodes, _network) = create_test_network(6).await?;
    
    // Create two partitions
    let partition_a = &nodes[0..3];
    let partition_b = &nodes[3..6];
    
    // Simulate network partition
    simulate_network_partition(partition_a, partition_b).await;
    
    // Mine blocks on both partitions
    mine_blocks_on_partition(partition_a, 3).await?;
    mine_blocks_on_partition(partition_b, 2).await?;
    
    // Verify partitions have different chain tips
    let tip_a = get_best_block_hash(&partition_a[0]);
    let tip_b = get_best_block_hash(&partition_b[0]);
    assert_ne!(tip_a, tip_b, "Partitions should have different chain tips");
    
    // Verify all nodes in partition A have the same tip
    for node in partition_a {
        assert_eq!(get_best_block_hash(node), tip_a, "All nodes in partition A should have the same tip");
    }
    
    // Verify all nodes in partition B have the same tip
    for node in partition_b {
        assert_eq!(get_best_block_hash(node), tip_b, "All nodes in partition B should have the same tip");
    }
    
    // Heal partition
    heal_network_partition(partition_a, partition_b).await;
    
    // Mine one more block on partition A to trigger sync
    mine_blocks_on_partition(partition_a, 1).await?;
    
    // Allow time for synchronization
    tokio::time::sleep(Duration::from_millis(500)).await;
    
    // Verify all nodes have converged to the same chain tip
    let final_tip = get_best_block_hash(&nodes[0]);
    for node in &nodes[1..] {
        assert_eq!(get_best_block_hash(node), final_tip, "All nodes should converge to the same tip after healing");
    }
    
    Ok(())
}

#[tokio::test]
async fn test_network_partition_with_transaction_propagation() -> Result<(), Box<dyn std::error::Error>> {
    // Create a test network with 6 nodes
    let (nodes, _network) = create_test_network(6).await?;
    
    // Create partitions
    let partition_a = &nodes[0..3];
    let partition_b = &nodes[3..6];
    
    // Create a test transaction
    let tx = Transaction::new(
        1,
        vec![TransactionInput::new([0u8; 32], 0, vec![], 0xffffffff)],
        vec![TransactionOutput::new(1_000_000, vec![1, 2, 3, 4, 5])],
        0,
    );
    
    // Add transaction to a node in partition A
    partition_a[0].add_transaction(tx.clone()).await?;
    
    // Mine a block in partition A to include the transaction
    mine_blocks_on_partition(partition_a, 1).await?;
    
    // Simulate network partition
    simulate_network_partition(partition_a, partition_b).await;
    
    // Mine blocks on both partitions
    mine_blocks_on_partition(partition_a, 2).await?;
    mine_blocks_on_partition(partition_b, 4).await?; // Mine more blocks on B to create a longer chain
    
    // Verify partitions have different chain tips
    let tip_a = get_best_block_hash(&partition_a[0]);
    let tip_b = get_best_block_hash(&partition_b[0]);
    assert_ne!(tip_a, tip_b, "Partitions should have different chain tips");
    
    // Verify partition B has a higher height (longer chain)
    assert!(partition_b[0].get_height() > partition_a[0].get_height(), 
        "Partition B should have a higher height");
    
    // Heal partition
    heal_network_partition(partition_a, partition_b).await;
    
    // Allow time for synchronization
    tokio::time::sleep(Duration::from_millis(500)).await;
    
    // Verify all nodes have converged to partition B's chain tip (longest chain)
    let final_tip = get_best_block_hash(&partition_b[0]);
    for node in &nodes {
        assert_eq!(get_best_block_hash(node), final_tip, 
            "All nodes should converge to the longest chain after healing");
    }
    
    Ok(())
}

#[tokio::test]
async fn test_multi_partition_scenario() -> Result<(), Box<dyn std::error::Error>> {
    // Create a test network with 9 nodes
    let (nodes, _network) = create_test_network(9).await?;
    
    // Create three partitions
    let partition_a = &nodes[0..3];
    let partition_b = &nodes[3..6];
    let partition_c = &nodes[6..9];
    
    // Simulate network partition between all three groups
    simulate_network_partition(partition_a, partition_b).await;
    simulate_network_partition(partition_a, partition_c).await;
    simulate_network_partition(partition_b, partition_c).await;
    
    // Mine different number of blocks on each partition
    mine_blocks_on_partition(partition_a, 2).await?;
    mine_blocks_on_partition(partition_b, 3).await?;
    mine_blocks_on_partition(partition_c, 5).await?; // C has the longest chain
    
    // Verify partitions have different chain tips
    let tip_a = get_best_block_hash(&partition_a[0]);
    let tip_b = get_best_block_hash(&partition_b[0]);
    let tip_c = get_best_block_hash(&partition_c[0]);
    
    assert_ne!(tip_a, tip_b);
    assert_ne!(tip_a, tip_c);
    assert_ne!(tip_b, tip_c);
    
    // Verify partition C has the highest height
    assert!(partition_c[0].get_height() > partition_b[0].get_height());
    assert!(partition_c[0].get_height() > partition_a[0].get_height());
    
    // Heal partitions one at a time
    heal_network_partition(partition_a, partition_b).await;
    
    // Allow time for synchronization
    tokio::time::sleep(Duration::from_millis(500)).await;
    
    // Verify A and B have the same tip now (should be B's tip as it's longer)
    let ab_tip = get_best_block_hash(&partition_b[0]);
    for node in partition_a.iter().chain(partition_b.iter()) {
        assert_eq!(get_best_block_hash(node), ab_tip, 
            "All nodes in merged partition A+B should have the same tip");
    }
    
    // Now heal with partition C
    heal_network_partition(partition_b, partition_c).await;
    
    // Allow time for synchronization
    tokio::time::sleep(Duration::from_millis(500)).await;
    
    // Verify all nodes have converged to partition C's chain tip (longest chain)
    let final_tip = get_best_block_hash(&partition_c[0]);
    for node in &nodes {
        assert_eq!(get_best_block_hash(node), final_tip, 
            "All nodes should converge to partition C's chain (longest) after healing");
    }
    
    Ok(())
}

#[tokio::test]
async fn test_partition_with_reorg() -> Result<(), Box<dyn std::error::Error>> {
    // Create a test network with 6 nodes
    let (nodes, _network) = create_test_network(6).await?;
    
    // Create initial blocks that all nodes agree on
    mine_blocks_on_partition(&nodes, 2).await?;
    
    // Create two partitions
    let partition_a = &nodes[0..3];
    let partition_b = &nodes[3..6];
    
    // Simulate network partition
    simulate_network_partition(partition_a, partition_b).await;
    
    // Create conflicting transactions
    let tx1 = Transaction::new(
        1,
        vec![TransactionInput::new([1u8; 32], 0, vec![], 0xffffffff)],
        vec![TransactionOutput::new(1_000_000, vec![1, 2, 3])],
        0,
    );
    
    let tx2 = Transaction::new(
        1,
        vec![TransactionInput::new([1u8; 32], 0, vec![], 0xffffffff)], // Same input as tx1
        vec![TransactionOutput::new(1_000_000, vec![4, 5, 6])], // Different output
        0,
    );
    
    // Add transactions to different partitions
    partition_a[0].add_transaction(tx1.clone()).await?;
    partition_b[0].add_transaction(tx2.clone()).await?;
    
    // Mine blocks on both partitions
    mine_blocks_on_partition(partition_a, 1).await?; // Include tx1
    mine_blocks_on_partition(partition_b, 3).await?; // Include tx2, make this chain longer
    
    // Verify tx1 is in partition A's chain
    let blocks_a = partition_a[0].chain_state.get_blocks(partition_a[0].get_height(), 1)?;
    let contains_tx1 = blocks_a[0].transactions().iter().any(|tx| tx.hash() == tx1.hash());
    assert!(contains_tx1, "Transaction 1 should be included in partition A's chain");
    
    // Verify tx2 is in partition B's chain
    let blocks_b = partition_b[0].chain_state.get_blocks(partition_b[0].get_height() - 2, 1)?;
    let contains_tx2 = blocks_b[0].transactions().iter().any(|tx| tx.hash() == tx2.hash());
    assert!(contains_tx2, "Transaction 2 should be included in partition B's chain");
    
    // Heal partition
    heal_network_partition(partition_a, partition_b).await;
    
    // Allow time for synchronization
    tokio::time::sleep(Duration::from_millis(500)).await;
    
    // Verify all nodes converged to partition B's chain (longest)
    for node in &nodes {
        assert_eq!(node.get_height(), partition_b[0].get_height());
    }
    
    // Verify tx1 is no longer in the chain (reorg) and tx2 is present
    for node in &nodes {
        // Get all blocks
        let blocks = node.chain_state.get_blocks(1, node.get_height() as usize)?;
        
        // Check that tx1 is not in any block
        let tx1_present = blocks.iter().any(|block| {
            block.transactions().iter().any(|tx| tx.hash() == tx1.hash())
        });
        
        // Check that tx2 is in some block
        let tx2_present = blocks.iter().any(|block| {
            block.transactions().iter().any(|tx| tx.hash() == tx2.hash())
        });
        
        assert!(!tx1_present, "Transaction 1 should not be present after reorg");
        assert!(tx2_present, "Transaction 2 should be present after reorg");
    }
    
    Ok(())
}

#[tokio::test]
async fn test_temporary_partition_recovery() -> Result<(), Box<dyn std::error::Error>> {
    // Create a test network with 4 nodes
    let (nodes, _network) = create_test_network(4).await?;
    
    // Create initial blocks that all nodes agree on
    mine_blocks_on_partition(&nodes, 1).await?;
    
    // Create two partitions
    let partition_a = &nodes[0..2];
    let partition_b = &nodes[2..4];
    
    // Simulate network partition
    simulate_network_partition(partition_a, partition_b).await;
    
    // Mine blocks on partition A only
    mine_blocks_on_partition(partition_a, 2).await?;
    
    // Verify heights differ
    assert_eq!(partition_a[0].get_height(), 3);
    assert_eq!(partition_b[0].get_height(), 1);
    
    // Heal partition
    heal_network_partition(partition_a, partition_b).await;
    
    // Allow time for synchronization
    tokio::time::sleep(Duration::from_millis(500)).await;
    
    // Verify all nodes have synced to the higher height
    for node in &nodes {
        assert_eq!(node.get_height(), 3);
    }
    
    // Create a temporary partition again
    simulate_network_partition(partition_a, partition_b).await;
    
    // Mine more blocks on partition B this time
    mine_blocks_on_partition(partition_b, 4).await?;
    
    // Heal partition again
    heal_network_partition(partition_a, partition_b).await;
    
    // Allow time for synchronization
    tokio::time::sleep(Duration::from_millis(500)).await;
    
    // Verify all nodes have converged to partition B's height now
    let expected_height = partition_b[0].get_height();
    for node in &nodes {
        assert_eq!(node.get_height(), expected_height);
    }
    
    Ok(())
}

// Function to calculate network convergence time
async fn measure_convergence_time(nodes: &[TestNode], partitions: &[&[TestNode]]) -> Duration {
    // Create partitions
    for i in 0..(partitions.len() - 1) {
        for j in (i+1)..partitions.len() {
            simulate_network_partition(partitions[i], partitions[j]).await;
        }
    }
    
    // Mine different blocks on each partition
    for (i, partition) in partitions.iter().enumerate() {
        // Mine i+1 blocks on partition i
        mine_blocks_on_partition(partition, i + 1).await.unwrap();
    }
    
    // Verify partitions have different chain tips
    let mut tips = Vec::new();
    for partition in partitions {
        tips.push(get_best_block_hash(&partition[0]));
    }
    
    // Heal all partitions
    for i in 0..(partitions.len() - 1) {
        for j in (i+1)..partitions.len() {
            heal_network_partition(partitions[i], partitions[j]).await;
        }
    }
    
    // Measure convergence time
    let start = std::time::Instant::now();
    
    // Check every 50ms if network has converged
    let check_interval = Duration::from_millis(50);
    let max_wait = Duration::from_secs(10);
    
    // Wait until all nodes have the same tip or timeout
    let result = tokio::time::timeout(max_wait, async {
        loop {
            let current_tips: Vec<_> = nodes.iter().map(|n| get_best_block_hash(n)).collect();
            
            let all_same = current_tips.windows(2).all(|w| w[0] == w[1]);
            if all_same {
                return start.elapsed();
            }
            
            tokio::time::sleep(check_interval).await;
        }
    }).await;
    
    match result {
        Ok(duration) => duration,
        Err(_) => max_wait,
    }
}

#[tokio::test]
async fn test_network_convergence_performance() -> Result<(), Box<dyn std::error::Error>> {
    // Create a test network with 9 nodes
    let (nodes, _network) = create_test_network(9).await?;
    
    // Create three partitions
    let partition_a = &nodes[0..3];
    let partition_b = &nodes[3..6];
    let partition_c = &nodes[6..9];
    
    let partitions = [partition_a, partition_b, partition_c];
    
    // Measure convergence time
    let duration = measure_convergence_time(&nodes, &partitions).await;
    
    // Report convergence time
    info!("Network convergence time: {:?}", duration);
    
    // Verify network did converge (all nodes have same tip)
    let final_tip = get_best_block_hash(&nodes[0]);
    for node in &nodes[1..] {
        assert_eq!(get_best_block_hash(node), final_tip,
            "All nodes should have converged to the same tip");
    }
    
    // We're not asserting on specific duration since it's environment-dependent,
    // but we could add benchmarks here in the future
    
    Ok(())
} 