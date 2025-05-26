// Chaos testing framework
//
// This file contains a framework for testing the node's resilience against
// various failure scenarios like node crashes, network issues, disk failures, etc.

use std::sync::Arc;
use std::time::Duration;
use std::collections::HashMap;
use tokio::time::sleep;
use rand::{thread_rng, Rng};
use tracing::{debug, info, warn, error};

use crate::network::{NetworkSimulator, NodeHandle, NetworkCondition};
use crate::storage::BlockchainDB;
use crate::chain::{ChainState, Block};
use crate::config::Config;
use crate::types::Hash;

// Chaos testing framework
pub struct ChaosTest {
    // Network simulator
    simulator: NetworkSimulator,
    // Node handles
    nodes: Vec<NodeHandle>,
    // Node status (true = running, false = stopped)
    node_status: Vec<bool>,
    // Network conditions between nodes
    network_conditions: HashMap<(usize, usize), NetworkCondition>,
    // Test duration
    test_duration: Duration,
    // RNG for randomized testing
    rng: rand::rngs::ThreadRng,
}

impl ChaosTest {
    // Create a new chaos test with the specified number of nodes
    pub async fn new(node_count: usize) -> Self {
        // Create nodes
        let mut nodes = Vec::with_capacity(node_count);
        let mut node_status = Vec::with_capacity(node_count);
        
        for i in 0..node_count {
            let db = Arc::new(BlockchainDB::create_in_memory().unwrap());
            let chain_state = ChainState::new(Arc::clone(&db)).unwrap();
            let node = NodeHandle::new(format!("node-{}", i), db, chain_state).await;
            
            nodes.push(node);
            node_status.push(true); // All nodes start as running
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
        
        // Initialize network conditions
        let mut network_conditions = HashMap::new();
        for i in 0..node_count {
            for j in 0..node_count {
                if i != j {
                    network_conditions.insert((i, j), NetworkCondition::default());
                }
            }
        }
        
        Self {
            simulator,
            nodes,
            node_status,
            network_conditions,
            test_duration: Duration::from_secs(30), // Default 30 seconds
            rng: thread_rng(),
        }
    }
    
    // Set test duration
    pub fn with_duration(mut self, duration: Duration) -> Self {
        self.test_duration = duration;
        self
    }
    
    // Run the chaos test
    pub async fn run(&mut self) -> ChaosTestResults {
        info!("Starting chaos test with {} nodes for {:?}", self.nodes.len(), self.test_duration);
        
        let start_time = std::time::Instant::now();
        let mut results = ChaosTestResults::new(self.nodes.len());
        
        // Mine some initial blocks to establish a baseline
        self.mine_initial_blocks(5).await;
        
        // Main chaos test loop
        while start_time.elapsed() < self.test_duration {
            // Choose a random chaos event
            let event_type = self.choose_chaos_event();
            
            match event_type {
                ChaosEventType::NodeFailure => self.simulate_node_failure().await,
                ChaosEventType::NodeRestart => self.simulate_node_restart().await,
                ChaosEventType::NetworkPartition => self.simulate_network_partition().await,
                ChaosEventType::NetworkLatency => self.simulate_network_latency().await,
                ChaosEventType::PacketLoss => self.simulate_packet_loss().await,
                ChaosEventType::DiskFailure => self.simulate_disk_failure().await,
                ChaosEventType::HighLoad => self.simulate_high_load().await,
                ChaosEventType::InvalidBlocks => self.simulate_invalid_blocks().await,
            }
            
            // Record the event
            results.record_event(event_type);
            
            // Progress the blockchain
            self.mine_blocks_on_running_nodes(2).await;
            
            // Wait a bit between events
            let wait_time = Duration::from_millis(self.rng.gen_range(100..500));
            sleep(wait_time).await;
            
            // Check node health periodically
            if self.rng.gen_bool(0.3) {
                self.check_node_health(&mut results).await;
            }
        }
        
        // Final health check
        self.check_node_health(&mut results).await;
        
        // Heal all issues before finishing
        self.heal_all_issues().await;
        
        // Wait for final sync
        sleep(Duration::from_millis(500)).await;
        
        // Final chain state check
        self.check_final_chain_state(&mut results).await;
        
        info!("Chaos test completed: {} events generated", results.total_events());
        
        results
    }
    
    // Choose a random chaos event type
    fn choose_chaos_event(&mut self) -> ChaosEventType {
        let event_types = [
            ChaosEventType::NodeFailure,
            ChaosEventType::NodeRestart,
            ChaosEventType::NetworkPartition,
            ChaosEventType::NetworkLatency,
            ChaosEventType::PacketLoss,
            ChaosEventType::DiskFailure,
            ChaosEventType::HighLoad,
            ChaosEventType::InvalidBlocks,
        ];
        
        // Different weights for different event types
        let weights = [
            0.15, // NodeFailure
            0.15, // NodeRestart
            0.15, // NetworkPartition
            0.15, // NetworkLatency
            0.15, // PacketLoss
            0.05, // DiskFailure (less common)
            0.10, // HighLoad
            0.10, // InvalidBlocks
        ];
        
        // Choose an event based on weights
        let random_val = self.rng.gen::<f64>();
        let mut cumulative = 0.0;
        
        for i in 0..event_types.len() {
            cumulative += weights[i];
            if random_val < cumulative {
                return event_types[i];
            }
        }
        
        // Fallback
        ChaosEventType::NodeFailure
    }
    
    // Mine initial blocks to establish a baseline
    async fn mine_initial_blocks(&mut self, count: u64) {
        info!("Mining {} initial blocks", count);
        
        for _ in 0..count {
            // Choose a random running node to mine
            let miner = self.choose_random_running_node();
            if let Some(node_id) = miner {
                self.simulator.mine_block(node_id).await;
            }
            
            // Short delay for block propagation
            sleep(Duration::from_millis(100)).await;
        }
    }
    
    // Mine blocks on currently running nodes
    async fn mine_blocks_on_running_nodes(&mut self, count: u64) {
        for _ in 0..count {
            // Choose a random running node to mine
            let miner = self.choose_random_running_node();
            if let Some(node_id) = miner {
                self.simulator.mine_block(node_id).await;
            }
            
            // Short delay for block propagation
            sleep(Duration::from_millis(100)).await;
        }
    }
    
    // Choose a random running node
    fn choose_random_running_node(&mut self) -> Option<usize> {
        let running_nodes: Vec<usize> = self.node_status.iter()
            .enumerate()
            .filter(|(_, &status)| status)
            .map(|(idx, _)| idx)
            .collect();
            
        if running_nodes.is_empty() {
            None
        } else {
            let idx = self.rng.gen_range(0..running_nodes.len());
            Some(running_nodes[idx])
        }
    }
    
    // Check health of all nodes
    async fn check_node_health(&mut self, results: &mut ChaosTestResults) {
        for i in 0..self.nodes.len() {
            if self.node_status[i] {
                // Check if node is responsive
                let is_responsive = self.simulator.is_node_responsive(i).await;
                
                // Check if node has a valid blockchain
                let has_valid_chain = self.simulator.has_valid_blockchain(i).await;
                
                // Get node's best block height
                let height = self.simulator.get_block_height(i).await;
                
                // Update results
                results.update_node_health(i, is_responsive, has_valid_chain, height);
            }
        }
    }
    
    // Simulate a node failure
    async fn simulate_node_failure(&mut self) {
        // Choose a random running node
        if let Some(node_id) = self.choose_random_running_node() {
            info!("Simulating failure of node {}", node_id);
            
            // Stop the node
            self.simulator.stop_node(node_id).await;
            self.node_status[node_id] = false;
        }
    }
    
    // Simulate a node restart
    async fn simulate_node_restart(&mut self) {
        // Choose a random stopped node
        let stopped_nodes: Vec<usize> = self.node_status.iter()
            .enumerate()
            .filter(|(_, &status)| !status)
            .map(|(idx, _)| idx)
            .collect();
            
        if !stopped_nodes.is_empty() {
            let idx = self.rng.gen_range(0..stopped_nodes.len());
            let node_id = stopped_nodes[idx];
            
            info!("Restarting node {}", node_id);
            
            // Restart the node
            self.simulator.restart_node(node_id).await;
            self.node_status[node_id] = true;
        }
    }
    
    // Simulate a network partition
    async fn simulate_network_partition(&mut self) {
        if self.nodes.len() < 2 {
            return;
        }
        
        // Create two groups by randomly assigning nodes
        let mut group_a = Vec::new();
        let mut group_b = Vec::new();
        
        for i in 0..self.nodes.len() {
            if self.node_status[i] {
                if self.rng.gen_bool(0.5) {
                    group_a.push(i);
                } else {
                    group_b.push(i);
                }
            }
        }
        
        // Ensure we have at least one node in each group
        if group_a.is_empty() && !group_b.is_empty() {
            let idx = self.rng.gen_range(0..group_b.len());
            group_a.push(group_b.remove(idx));
        } else if group_b.is_empty() && !group_a.is_empty() {
            let idx = self.rng.gen_range(0..group_a.len());
            group_b.push(group_a.remove(idx));
        }
        
        if !group_a.is_empty() && !group_b.is_empty() {
            info!("Creating network partition: {:?} vs {:?}", group_a, group_b);
            
            // Create partition by setting 100% packet loss between groups
            for &a in &group_a {
                for &b in &group_b {
                    let condition = NetworkCondition {
                        latency_ms: None,
                        packet_loss_percent: Some(100),
                        bandwidth_limit_kbps: None,
                    };
                    
                    self.network_conditions.insert((a, b), condition.clone());
                    self.network_conditions.insert((b, a), condition.clone());
                    
                    self.simulator.set_network_condition(a, b, condition.clone()).await;
                    self.simulator.set_network_condition(b, a, condition.clone()).await;
                }
            }
        }
    }
    
    // Simulate increased network latency
    async fn simulate_network_latency(&mut self) {
        if self.nodes.len() < 2 {
            return;
        }
        
        // Choose random source and target nodes
        let source = self.choose_random_running_node();
        let target = self.choose_random_running_node();
        
        if let (Some(src), Some(tgt)) = (source, target) {
            if src != tgt {
                // Generate random latency between 100ms and 2000ms
                let latency = self.rng.gen_range(100..2000);
                
                info!("Adding {}ms latency between nodes {} and {}", latency, src, tgt);
                
                let condition = NetworkCondition {
                    latency_ms: Some(latency),
                    packet_loss_percent: None,
                    bandwidth_limit_kbps: None,
                };
                
                self.network_conditions.insert((src, tgt), condition.clone());
                self.simulator.set_network_condition(src, tgt, condition).await;
            }
        }
    }
    
    // Simulate packet loss
    async fn simulate_packet_loss(&mut self) {
        if self.nodes.len() < 2 {
            return;
        }
        
        // Choose random source and target nodes
        let source = self.choose_random_running_node();
        let target = self.choose_random_running_node();
        
        if let (Some(src), Some(tgt)) = (source, target) {
            if src != tgt {
                // Generate random packet loss between 10% and 50%
                let packet_loss = self.rng.gen_range(10..50);
                
                info!("Adding {}% packet loss between nodes {} and {}", packet_loss, src, tgt);
                
                let condition = NetworkCondition {
                    latency_ms: None,
                    packet_loss_percent: Some(packet_loss),
                    bandwidth_limit_kbps: None,
                };
                
                self.network_conditions.insert((src, tgt), condition.clone());
                self.simulator.set_network_condition(src, tgt, condition).await;
            }
        }
    }
    
    // Simulate disk failure (corruption)
    async fn simulate_disk_failure(&mut self) {
        if let Some(node_id) = self.choose_random_running_node() {
            info!("Simulating disk corruption on node {}", node_id);
            
            // Choose what to corrupt
            let corruption_target = match self.rng.gen_range(0..3) {
                0 => "UTXO set",
                1 => "Block index",
                2 => "Chain state",
                _ => "UTXO set",
            };
            
            info!("Corrupting {} on node {}", corruption_target, node_id);
            
            // Simulate corruption
            self.simulator.corrupt_storage(node_id, corruption_target).await;
        }
    }
    
    // Simulate high load (many transactions)
    async fn simulate_high_load(&mut self) {
        if let Some(node_id) = self.choose_random_running_node() {
            // Generate a random number of transactions between 100 and 1000
            let tx_count = self.rng.gen_range(100..1000);
            
            info!("Simulating high load with {} transactions on node {}", tx_count, node_id);
            
            // Add transactions to node's mempool
            self.simulator.add_random_transactions(node_id, tx_count).await;
        }
    }
    
    // Simulate invalid blocks
    async fn simulate_invalid_blocks(&mut self) {
        if let Some(node_id) = self.choose_random_running_node() {
            info!("Sending invalid blocks to node {}", node_id);
            
            // Create an invalid block
            let best_hash = self.simulator.get_best_block_hash(node_id).await;
            let invalid_block = self.simulator.create_invalid_block(node_id, best_hash).await;
            
            // Send to random nodes
            let target_count = self.rng.gen_range(1..self.nodes.len().max(2));
            let mut targets = Vec::new();
            
            for _ in 0..target_count {
                if let Some(target) = self.choose_random_running_node() {
                    if !targets.contains(&target) {
                        targets.push(target);
                    }
                }
            }
            
            for target in targets {
                info!("Sending invalid block to node {}", target);
                self.simulator.send_block(target, &invalid_block).await;
            }
        }
    }
    
    // Heal all issues
    async fn heal_all_issues(&mut self) {
        info!("Healing all issues");
        
        // Restart all stopped nodes
        for i in 0..self.nodes.len() {
            if !self.node_status[i] {
                info!("Restarting node {}", i);
                self.simulator.restart_node(i).await;
                self.node_status[i] = true;
            }
        }
        
        // Reset all network conditions
        for i in 0..self.nodes.len() {
            for j in 0..self.nodes.len() {
                if i != j {
                    info!("Resetting network conditions between {} and {}", i, j);
                    let normal_condition = NetworkCondition::default();
                    self.network_conditions.insert((i, j), normal_condition.clone());
                    self.simulator.set_network_condition(i, j, normal_condition).await;
                }
            }
        }
        
        // Allow time for recovery
        sleep(Duration::from_millis(500)).await;
    }
    
    // Check final chain state
    async fn check_final_chain_state(&mut self, results: &mut ChaosTestResults) {
        info!("Checking final chain state");
        
        // Get heights from all nodes
        let mut heights = Vec::new();
        let mut hashes = Vec::new();
        
        for i in 0..self.nodes.len() {
            if self.node_status[i] {
                let height = self.simulator.get_block_height(i).await;
                let hash = self.simulator.get_best_block_hash(i).await;
                
                heights.push(height);
                hashes.push(hash);
            }
        }
        
        // Check if all nodes have converged on the same chain
        let all_same_height = heights.windows(2).all(|w| w[0] == w[1]);
        let all_same_hash = hashes.windows(2).all(|w| w[0] == w[1]);
        
        // Update results
        results.set_final_consistency(all_same_height && all_same_hash);
        
        if all_same_height && all_same_hash {
            info!("All nodes converged on the same chain: height={}, hash={:?}", 
                heights[0], hashes[0]);
        } else {
            warn!("Nodes did not converge on the same chain!");
            for i in 0..heights.len() {
                warn!("Node {}: height={}, hash={:?}", i, heights[i], hashes[i]);
            }
        }
    }
}

// Chaos event types
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ChaosEventType {
    NodeFailure,
    NodeRestart,
    NetworkPartition,
    NetworkLatency,
    PacketLoss,
    DiskFailure,
    HighLoad,
    InvalidBlocks,
}

// Results of a chaos test
pub struct ChaosTestResults {
    // Number of nodes
    node_count: usize,
    // Event counts
    event_counts: HashMap<ChaosEventType, usize>,
    // Node health status
    node_health: Vec<NodeHealth>,
    // Final consistency check
    final_consistency: bool,
}

// Health status of a node
#[derive(Debug, Clone)]
pub struct NodeHealth {
    // Number of times checked
    check_count: usize,
    // Number of times responsive
    responsive_count: usize,
    // Number of times with valid chain
    valid_chain_count: usize,
    // Latest block height
    latest_height: u64,
}

impl ChaosTestResults {
    // Create new test results
    fn new(node_count: usize) -> Self {
        let mut node_health = Vec::with_capacity(node_count);
        for _ in 0..node_count {
            node_health.push(NodeHealth {
                check_count: 0,
                responsive_count: 0,
                valid_chain_count: 0,
                latest_height: 0,
            });
        }
        
        Self {
            node_count,
            event_counts: HashMap::new(),
            node_health,
            final_consistency: false,
        }
    }
    
    // Record a new event
    fn record_event(&mut self, event_type: ChaosEventType) {
        *self.event_counts.entry(event_type).or_insert(0) += 1;
    }
    
    // Update node health
    fn update_node_health(&mut self, node_id: usize, responsive: bool, valid_chain: bool, height: u64) {
        let health = &mut self.node_health[node_id];
        
        health.check_count += 1;
        if responsive {
            health.responsive_count += 1;
        }
        if valid_chain {
            health.valid_chain_count += 1;
        }
        health.latest_height = height;
    }
    
    // Set final consistency status
    fn set_final_consistency(&mut self, consistent: bool) {
        self.final_consistency = consistent;
    }
    
    // Get total number of events
    pub fn total_events(&self) -> usize {
        self.event_counts.values().sum()
    }
    
    // Get count for a specific event type
    pub fn event_count(&self, event_type: ChaosEventType) -> usize {
        *self.event_counts.get(&event_type).unwrap_or(&0)
    }
    
    // Check if all nodes converged on the same chain
    pub fn is_consistent(&self) -> bool {
        self.final_consistency
    }
    
    // Get node reliability percentage
    pub fn node_reliability(&self, node_id: usize) -> f64 {
        let health = &self.node_health[node_id];
        if health.check_count == 0 {
            return 0.0;
        }
        (health.responsive_count as f64) / (health.check_count as f64) * 100.0
    }
    
    // Get node chain validity percentage
    pub fn node_chain_validity(&self, node_id: usize) -> f64 {
        let health = &self.node_health[node_id];
        if health.check_count == 0 {
            return 0.0;
        }
        (health.valid_chain_count as f64) / (health.check_count as f64) * 100.0
    }
    
    // Print summary
    pub fn print_summary(&self) {
        println!("\n--- CHAOS TEST RESULTS ---");
        println!("Total events: {}", self.total_events());
        
        println!("\nEvent distribution:");
        for (event_type, count) in &self.event_counts {
            println!("  {:?}: {}", event_type, count);
        }
        
        println!("\nNode health:");
        for i in 0..self.node_count {
            let health = &self.node_health[i];
            println!("  Node {}: Reliability: {:.1}%, Chain validity: {:.1}%, Final height: {}", 
                i, self.node_reliability(i), self.node_chain_validity(i), health.latest_height);
        }
        
        println!("\nFinal consistency: {}", if self.final_consistency { "ACHIEVED ✓" } else { "FAILED ✗" });
        println!("------------------------\n");
    }
}

// Main chaos test function
#[tokio::test]
async fn test_chaos_resilience() {
    // Create test with 5 nodes
    let mut test = ChaosTest::new(5).await;
    
    // Run test for 30 seconds
    let results = test.with_duration(Duration::from_secs(30))
        .run()
        .await;
    
    // Print results
    results.print_summary();
    
    // Assert final consistency
    assert!(results.is_consistent(), "Nodes should converge on the same chain after chaos test");
    
    // Check minimum node reliability
    for i in 0..results.node_count {
        let reliability = results.node_reliability(i);
        assert!(reliability >= 50.0, "Node {} reliability should be at least 50%", i);
    }
}

// Test chaos resilience with even more extreme conditions
#[tokio::test]
async fn test_extreme_chaos_resilience() {
    // Create test with 10 nodes
    let mut test = ChaosTest::new(10).await;
    
    // Run test for 60 seconds
    let results = test.with_duration(Duration::from_secs(60))
        .run()
        .await;
    
    // Print results
    results.print_summary();
    
    // Assert final consistency
    assert!(results.is_consistent(), "Nodes should converge on the same chain after extreme chaos test");
} 