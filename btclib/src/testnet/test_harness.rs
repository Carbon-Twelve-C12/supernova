use crate::config::BlockchainConfig;
use crate::testnet::{TestNetManager, config::TestNetConfig};
use crate::testnet::network_simulator::NetworkCondition;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::time::{Duration, sleep};
use tracing::{info, warn, error};

/// Represents a test node in the harness
#[derive(Clone)]
pub struct TestNode {
    /// Node ID within the test harness
    pub id: usize,
    /// Node type (full, miner, etc.)
    pub node_type: TestNodeType,
    /// Current block height
    pub height: u64,
    /// Current best block hash
    pub best_block_hash: String,
    /// Current mempool transaction count
    pub mempool_count: usize,
    /// Node connection information
    pub connections: Vec<usize>,
    /// Node status
    pub status: TestNodeStatus,
}

/// Type of test node
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TestNodeType {
    /// Full node with complete validation
    Full,
    /// Mining node
    Miner,
    /// Light client
    Light,
    /// Special purpose node (e.g., explorer)
    Special(String),
}

/// Status of a test node
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TestNodeStatus {
    /// Node is running normally
    Running,
    /// Node is currently syncing
    Syncing,
    /// Node is offline
    Offline,
    /// Node has an error
    Error(String),
}

/// Test scenario definition
#[derive(Clone)]
pub struct TestScenario {
    /// Name of the scenario
    pub name: String,
    /// Description of what the scenario tests
    pub description: String,
    /// Network configuration
    pub network_config: TestNetConfig,
    /// Initial node setup
    pub initial_nodes: Vec<TestNodeSetup>,
    /// Test steps to execute
    pub steps: Vec<TestStep>,
    /// Expected outcomes
    pub expected_outcomes: Vec<TestOutcome>,
}

/// Test node setup information
#[derive(Clone)]
pub struct TestNodeSetup {
    /// Node ID
    pub id: usize,
    /// Node type
    pub node_type: TestNodeType,
    /// Initial connections
    pub initial_connections: Vec<usize>,
    /// Custom configuration overrides
    pub config_overrides: Option<HashMap<String, String>>,
}

/// A test step in a scenario
#[derive(Clone)]
pub enum TestStep {
    /// Wait for a specified duration
    Wait(Duration),
    /// Mine blocks on specified nodes
    MineBlocks {
        /// Node IDs to mine on
        node_ids: Vec<usize>,
        /// Number of blocks to mine
        block_count: u64,
    },
    /// Send transactions between nodes
    SendTransactions {
        /// Source node ID
        from_node: usize,
        /// Destination node ID
        to_node: usize,
        /// Number of transactions
        tx_count: usize,
    },
    /// Modify network conditions
    SetNetworkCondition {
        /// Source node ID
        from_node: usize,
        /// Destination node ID
        to_node: usize,
        /// Latency in milliseconds
        latency_ms: Option<u64>,
        /// Packet loss percentage
        packet_loss_percent: Option<u8>,
        /// Bandwidth limit in kbps
        bandwidth_kbps: Option<u64>,
    },
    /// Create a network partition
    CreatePartition {
        /// First group of nodes
        group_a: Vec<usize>,
        /// Second group of nodes
        group_b: Vec<usize>,
    },
    /// Heal a network partition
    HealPartition {
        /// First group of nodes
        group_a: Vec<usize>,
        /// Second group of nodes
        group_b: Vec<usize>,
    },
    /// Change node status
    SetNodeStatus {
        /// Node ID
        node_id: usize,
        /// New status
        status: TestNodeStatus,
    },
    /// Simulate clock drift
    SetClockDrift {
        /// Node ID
        node_id: usize,
        /// Drift in milliseconds (can be negative)
        drift_ms: i64,
    },
    /// Create a custom event
    CustomEvent {
        /// Event name
        name: String,
        /// Event parameters
        params: HashMap<String, String>,
    },
}

/// Expected test outcome
#[derive(Clone)]
pub enum TestOutcome {
    /// All nodes should have the same best block
    AllNodesHaveSameChainTip,
    /// Specified nodes should have the same best block
    NodesHaveSameChainTip(Vec<usize>),
    /// Nodes should have different best blocks
    NodesHaveDifferentChainTips {
        /// First group of nodes
        group_a: Vec<usize>,
        /// Second group of nodes
        group_b: Vec<usize>,
    },
    /// Node should be at specified height
    NodeAtHeight {
        /// Node ID
        node_id: usize,
        /// Expected height
        height: u64,
    },
    /// Node should have transactions in mempool
    NodeHasTransactions {
        /// Node ID
        node_id: usize,
        /// Minimum transaction count
        min_tx_count: usize,
    },
    /// Custom outcome with verification function
    Custom {
        /// Outcome description
        description: String,
        /// Function to verify the outcome (returns true if outcome is met)
        verify: fn(&TestHarness) -> bool,
    },
}

/// Result of a test run
#[derive(Clone)]
pub struct TestResult {
    /// Scenario name
    pub scenario_name: String,
    /// Whether the test passed
    pub passed: bool,
    /// Failed outcomes if any
    pub failed_outcomes: Vec<String>,
    /// Test execution time
    pub execution_time: Duration,
    /// Additional notes or information
    pub notes: Vec<String>,
}

/// Test harness for running network simulations
pub struct TestHarness {
    /// Test network manager
    testnet: TestNetManager,
    /// Test nodes
    nodes: HashMap<usize, TestNode>,
    /// Current running scenario if any
    current_scenario: Option<TestScenario>,
    /// Network conditions between nodes
    network_conditions: HashMap<(usize, usize), NetworkCondition>,
    /// Test results history
    test_results: Vec<TestResult>,
}

impl TestHarness {
    /// Create a new test harness
    pub fn new(config: TestNetConfig) -> Self {
        let testnet = TestNetManager::new(config);
        
        Self {
            testnet,
            nodes: HashMap::new(),
            current_scenario: None,
            network_conditions: HashMap::new(),
            test_results: Vec::new(),
        }
    }
    
    /// Initialize nodes for a test
    pub fn initialize_nodes(&mut self, node_setups: Vec<TestNodeSetup>) -> Result<(), String> {
        // Clear existing nodes
        self.nodes.clear();
        
        // Create new nodes
        for setup in node_setups {
            let node = TestNode {
                id: setup.id,
                node_type: setup.node_type,
                height: 0,
                best_block_hash: "0000000000000000000000000000000000000000000000000000000000000000".to_string(),
                mempool_count: 0,
                connections: setup.initial_connections,
                status: TestNodeStatus::Running,
            };
            
            self.nodes.insert(setup.id, node);
        }
        
        info!("Initialized {} test nodes", self.nodes.len());
        Ok(())
    }
    
    /// Run a complete test scenario
    pub async fn run_scenario(&mut self, scenario: TestScenario) -> TestResult {
        let start_time = std::time::Instant::now();
        
        info!("Starting test scenario: {}", scenario.name);
        info!("Description: {}", scenario.description);
        
        // Initialize the test network with the scenario configuration
        self.testnet = TestNetManager::new(scenario.network_config.clone());
        
        // Initialize nodes
        if let Err(e) = self.initialize_nodes(scenario.initial_nodes.clone()) {
            error!("Failed to initialize nodes: {}", e);
            return TestResult {
                scenario_name: scenario.name.clone(),
                passed: false,
                failed_outcomes: vec!["Node initialization failed".to_string()],
                execution_time: start_time.elapsed(),
                notes: vec![format!("Error: {}", e)],
            };
        }
        
        // Set current scenario
        self.current_scenario = Some(scenario.clone());
        
        // Execute test steps
        for (step_idx, step) in scenario.steps.iter().enumerate() {
            info!("Executing step {} of {}", step_idx + 1, scenario.steps.len());
            
            if let Err(e) = self.execute_step(step).await {
                error!("Step execution failed: {}", e);
                return TestResult {
                    scenario_name: scenario.name.clone(),
                    passed: false,
                    failed_outcomes: vec![format!("Step {} failed", step_idx + 1)],
                    execution_time: start_time.elapsed(),
                    notes: vec![format!("Error in step {}: {}", step_idx + 1, e)],
                };
            }
        }
        
        // Verify outcomes
        let mut failed_outcomes = Vec::new();
        for (outcome_idx, outcome) in scenario.expected_outcomes.iter().enumerate() {
            info!("Verifying outcome {} of {}", outcome_idx + 1, scenario.expected_outcomes.len());
            
            if !self.verify_outcome(outcome) {
                let outcome_desc = format!("Outcome {} failed", outcome_idx + 1);
                error!("{}", outcome_desc);
                failed_outcomes.push(outcome_desc);
            }
        }
        
        let passed = failed_outcomes.is_empty();
        let result = TestResult {
            scenario_name: scenario.name.clone(),
            passed,
            failed_outcomes,
            execution_time: start_time.elapsed(),
            notes: vec![],
        };
        
        // Record result
        self.test_results.push(result.clone());
        
        if passed {
            info!("Scenario '{}' passed in {:?}", scenario.name, result.execution_time);
        } else {
            warn!("Scenario '{}' failed in {:?}", scenario.name, result.execution_time);
        }
        
        result
    }
    
    /// Execute a single test step
    async fn execute_step(&mut self, step: &TestStep) -> Result<(), String> {
        match step {
            TestStep::Wait(duration) => {
                info!("Waiting for {:?}", duration);
                sleep(*duration).await;
                Ok(())
            }
            
            TestStep::MineBlocks { node_ids, block_count } => {
                info!("Mining {} blocks on nodes {:?}", block_count, node_ids);
                self.mine_blocks(node_ids, *block_count).await
            }
            
            TestStep::SendTransactions { from_node, to_node, tx_count } => {
                info!("Sending {} transactions from node {} to {}", tx_count, from_node, to_node);
                self.send_transactions(*from_node, *to_node, *tx_count).await
            }
            
            TestStep::SetNetworkCondition { from_node, to_node, latency_ms, packet_loss_percent, bandwidth_kbps } => {
                info!(
                    "Setting network condition from node {} to {}: latency={:?}ms, loss={:?}%, bandwidth={:?}kbps",
                    from_node, to_node, latency_ms, packet_loss_percent, bandwidth_kbps
                );
                
                self.testnet.apply_network_conditions(
                    *from_node,
                    *to_node,
                    *latency_ms,
                    *packet_loss_percent,
                    *bandwidth_kbps,
                )
            }
            
            TestStep::CreatePartition { group_a, group_b } => {
                info!("Creating network partition between groups {:?} and {:?}", group_a, group_b);
                self.testnet.simulate_network_partition(group_a, group_b)
            }
            
            TestStep::HealPartition { group_a, group_b } => {
                info!("Healing network partition between groups {:?} and {:?}", group_a, group_b);
                self.testnet.heal_network_partition(group_a, group_b)
            }
            
            TestStep::SetNodeStatus { node_id, status } => {
                info!("Setting node {} status to {:?}", node_id, status);
                self.set_node_status(*node_id, status.clone())
            }
            
            TestStep::SetClockDrift { node_id, drift_ms } => {
                info!("Setting clock drift for node {} to {}ms", node_id, drift_ms);
                // Implementation would depend on how clock drift is simulated
                Ok(())
            }
            
            TestStep::CustomEvent { name, params } => {
                info!("Executing custom event '{}' with parameters: {:?}", name, params);
                self.handle_custom_event(name, params).await
            }
        }
    }
    
    /// Set status for a node
    fn set_node_status(&mut self, node_id: usize, status: TestNodeStatus) -> Result<(), String> {
        let node = self.nodes.get_mut(&node_id).ok_or_else(|| {
            format!("Node {} not found", node_id)
        })?;
        
        node.status = status;
        Ok(())
    }
    
    /// Mine blocks on specified nodes
    async fn mine_blocks(&mut self, node_ids: &[usize], block_count: u64) -> Result<(), String> {
        // This is a simplified implementation - in a real system this would interact
        // with actual node implementations
        for &node_id in node_ids {
            let node = self.nodes.get_mut(&node_id).ok_or_else(|| {
                format!("Node {} not found", node_id)
            })?;
            
            if node.status != TestNodeStatus::Running {
                return Err(format!("Node {} is not running", node_id));
            }
            
            // Simulate mining blocks
            for _ in 0..block_count {
                node.height += 1;
                
                // Generate a fake block hash for testing
                let hash = format!("block{}node{}", node.height, node_id);
                node.best_block_hash = hash;
                
                // Process block in testnet manager
                self.testnet.process_block(
                    node.height,
                    // Use current time as timestamp
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_secs(),
                    Some(format!("node{}", node_id)),
                );
                
                // Short delay to simulate block time
                sleep(Duration::from_millis(10)).await;
            }
            
            info!("Mined {} blocks on node {}, new height: {}", block_count, node_id, node.height);
        }
        
        // Propagate blocks to connected nodes
        self.propagate_blocks().await?;
        
        Ok(())
    }
    
    /// Send transactions between nodes
    async fn send_transactions(&mut self, from_node: usize, to_node: usize, tx_count: usize) -> Result<(), String> {
        let from = self.nodes.get(&from_node).ok_or_else(|| {
            format!("Source node {} not found", from_node)
        })?;
        
        if from.status != TestNodeStatus::Running {
            return Err(format!("Source node {} is not running", from_node));
        }
        
        // Check if the node exists before getting a mutable reference
        if !self.nodes.contains_key(&to_node) {
            return Err(format!("Destination node {} not found", to_node));
        }
        
        let to_node_status = self.nodes.get(&to_node).unwrap().status.clone(); // Get status before mutable borrow
        if to_node_status != TestNodeStatus::Running {
            return Err(format!("Destination node {} is not running", to_node));
        }

        // Check network conditions
        let should_drop = self.check_network_condition(from_node, to_node);
        if should_drop {
            info!("Network conditions prevented transaction propagation from {} to {}", from_node, to_node);
            return Ok(());
        }
        
        // Now we can safely get a mutable reference
        let to = self.nodes.get_mut(&to_node).unwrap();
        
        // Add transactions to destination node's mempool
        to.mempool_count += tx_count;
        
        info!("Sent {} transactions from node {} to {}", tx_count, from_node, to_node);
        
        Ok(())
    }
    
    /// Propagate blocks to connected nodes
    async fn propagate_blocks(&mut self) -> Result<(), String> {
        // This is a simplified implementation - in a real system this would be more complex
        let mut propagations = Vec::new();
        
        // Create a copy of nodes to avoid borrow checker issues
        let nodes_copy = self.nodes.clone();
        
        // For each node, propagate its blocks to connected nodes
        for (node_id, node) in &nodes_copy {
            if node.status != TestNodeStatus::Running {
                continue;
            }
            
            for &conn_id in &node.connections {
                // First check if connection exists and is running without mutable borrow
                if let Some(conn_node) = self.nodes.get(&conn_id) {
                    if conn_node.status != TestNodeStatus::Running {
                        continue;
                    }
                    
                    // Check network conditions
                    let should_drop = self.check_network_condition(*node_id, conn_id);
                    if should_drop {
                        continue;
                    }
                    
                    // Only propagate if source has higher block height
                    if node.height > conn_node.height {
                        propagations.push((conn_id, node.height, node.best_block_hash.clone()));
                    }
                }
            }
        }
        
        // Apply propagations
        for (node_id, height, hash) in propagations {
            if let Some(node) = self.nodes.get_mut(&node_id) {
                node.height = height;
                node.best_block_hash = hash;
                info!("Node {} updated to height {}", node_id, height);
            }
        }
        
        Ok(())
    }
    
    /// Check if network conditions would prevent communication - modified to not require &mut self
    fn check_network_condition(&self, _from_node: usize, _to_node: usize) -> bool {
        if let Some(_network_simulator) = &self.testnet.get_network_simulator() {
            // This is a placeholder - actual implementation would use the simulator directly
            return false;
        }
        
        false
    }
    
    /// Handle custom test events
    async fn handle_custom_event(&mut self, name: &str, _params: &HashMap<String, String>) -> Result<(), String> {
        // Custom event handling can be implemented here
        info!("Handled custom event: {}", name);
        Ok(())
    }
    
    /// Verify a test outcome
    fn verify_outcome(&self, outcome: &TestOutcome) -> bool {
        match outcome {
            TestOutcome::AllNodesHaveSameChainTip => {
                if self.nodes.is_empty() {
                    return false;
                }
                
                // Get the first running node's hash
                let mut reference_hash = None;
                for node in self.nodes.values() {
                    if node.status == TestNodeStatus::Running {
                        reference_hash = Some(node.best_block_hash.clone());
                        break;
                    }
                }
                
                let Some(reference_hash) = reference_hash else {
                    return false; // No running nodes
                };
                
                // Check all running nodes have the same hash
                for node in self.nodes.values() {
                    if node.status == TestNodeStatus::Running && node.best_block_hash != reference_hash {
                        return false;
                    }
                }
                
                true
            }
            
            TestOutcome::NodesHaveSameChainTip(node_ids) => {
                if node_ids.is_empty() {
                    return true;
                }
                
                // Get the first node's hash
                let first_id = node_ids[0];
                let first_node = match self.nodes.get(&first_id) {
                    Some(node) => node,
                    None => return false,
                };
                
                let reference_hash = first_node.best_block_hash.clone();
                
                // Check all specified nodes have the same hash
                for &id in node_ids.iter().skip(1) {
                    if let Some(node) = self.nodes.get(&id) {
                        if node.best_block_hash != reference_hash {
                            return false;
                        }
                    } else {
                        return false; // Node not found
                    }
                }
                
                true
            }
            
            TestOutcome::NodesHaveDifferentChainTips { group_a, group_b } => {
                if group_a.is_empty() || group_b.is_empty() {
                    return false;
                }
                
                // Get hash from first node in group A
                let node_a = match self.nodes.get(&group_a[0]) {
                    Some(node) => node,
                    None => return false,
                };
                let hash_a = node_a.best_block_hash.clone();
                
                // Get hash from first node in group B
                let node_b = match self.nodes.get(&group_b[0]) {
                    Some(node) => node,
                    None => return false,
                };
                let hash_b = node_b.best_block_hash.clone();
                
                // Check hashes are different
                if hash_a == hash_b {
                    return false;
                }
                
                // Check all nodes in group A have hash_a
                for &id in group_a.iter().skip(1) {
                    if let Some(node) = self.nodes.get(&id) {
                        if node.best_block_hash != hash_a {
                            return false;
                        }
                    } else {
                        return false; // Node not found
                    }
                }
                
                // Check all nodes in group B have hash_b
                for &id in group_b.iter().skip(1) {
                    if let Some(node) = self.nodes.get(&id) {
                        if node.best_block_hash != hash_b {
                            return false;
                        }
                    } else {
                        return false; // Node not found
                    }
                }
                
                true
            }
            
            TestOutcome::NodeAtHeight { node_id, height } => {
                match self.nodes.get(node_id) {
                    Some(node) => node.height == *height,
                    None => false,
                }
            }
            
            TestOutcome::NodeHasTransactions { node_id, min_tx_count } => {
                match self.nodes.get(node_id) {
                    Some(node) => node.mempool_count >= *min_tx_count,
                    None => false,
                }
            }
            
            TestOutcome::Custom { description: _, verify } => {
                verify(self)
            }
        }
    }
    
    /// Get a reference to the current test node
    pub fn get_node(&self, node_id: usize) -> Option<&TestNode> {
        self.nodes.get(&node_id)
    }
    
    /// Get all test nodes
    pub fn get_all_nodes(&self) -> &HashMap<usize, TestNode> {
        &self.nodes
    }
    
    /// Get test results history
    pub fn get_test_results(&self) -> &[TestResult] {
        &self.test_results
    }
}

/// Example test scenarios
pub mod example_scenarios {
    use super::*;
    use std::collections::HashMap;
    
    /// Create a basic network partition test scenario
    pub fn network_partition_scenario() -> TestScenario {
        // Create test network configuration
        let mut network_config = TestNetConfig::default();
        network_config.network_name = "partition-test".to_string();
        network_config.enable_faucet = true;
        network_config.target_block_time_secs = 1; // Fast blocks for testing
        
        // Configure network simulation
        network_config.network_simulation = Some(crate::testnet::config::NetworkSimulationConfig {
            enabled: true,
            latency_ms_mean: 100,
            latency_ms_std_dev: 20,
            packet_loss_percent: 0,
            bandwidth_limit_kbps: 1000,
            simulate_clock_drift: false,
            max_clock_drift_ms: 0,
            jitter_ms: 20,
            topology: crate::testnet::config::NetworkTopology::FullyConnected,
            disruption_schedule: None,
        });
        
        // Define initial node setup
        let initial_nodes = vec![
            TestNodeSetup {
                id: 0,
                node_type: TestNodeType::Miner,
                initial_connections: vec![1, 2, 3],
                config_overrides: None,
            },
            TestNodeSetup {
                id: 1,
                node_type: TestNodeType::Miner,
                initial_connections: vec![0, 2, 3],
                config_overrides: None,
            },
            TestNodeSetup {
                id: 2,
                node_type: TestNodeType::Full,
                initial_connections: vec![0, 1, 4, 5],
                config_overrides: None,
            },
            TestNodeSetup {
                id: 3,
                node_type: TestNodeType::Full,
                initial_connections: vec![0, 1, 4, 5],
                config_overrides: None,
            },
            TestNodeSetup {
                id: 4,
                node_type: TestNodeType::Miner,
                initial_connections: vec![2, 3, 5],
                config_overrides: None,
            },
            TestNodeSetup {
                id: 5,
                node_type: TestNodeType::Full,
                initial_connections: vec![2, 3, 4],
                config_overrides: None,
            },
        ];
        
        // Define test steps
        let steps = vec![
            // Initial mining to establish a common chain
            TestStep::MineBlocks {
                node_ids: vec![0],
                block_count: 5,
            },
            // Allow time for propagation
            TestStep::Wait(Duration::from_millis(500)),
            // Create a network partition
            TestStep::CreatePartition {
                group_a: vec![0, 1, 2],
                group_b: vec![3, 4, 5],
            },
            // Mine blocks on both sides of the partition
            TestStep::MineBlocks {
                node_ids: vec![0],
                block_count: 3,
            },
            TestStep::MineBlocks {
                node_ids: vec![4],
                block_count: 2,
            },
            // Allow time for propagation within partitions
            TestStep::Wait(Duration::from_millis(500)),
            // Heal the partition
            TestStep::HealPartition {
                group_a: vec![0, 1, 2],
                group_b: vec![3, 4, 5],
            },
            // Allow time for reconciliation
            TestStep::Wait(Duration::from_secs(1)),
            // Mine one more block to ensure convergence
            TestStep::MineBlocks {
                node_ids: vec![0],
                block_count: 1,
            },
            // Allow final propagation
            TestStep::Wait(Duration::from_secs(1)),
        ];
        
        // Define expected outcomes
        let expected_outcomes = vec![
            // All nodes should converge on the same chain
            TestOutcome::AllNodesHaveSameChainTip,
            // Node 0 should be at height 9 (5 + 3 + 1)
            TestOutcome::NodeAtHeight {
                node_id: 0,
                height: 9,
            },
        ];
        
        TestScenario {
            name: "Network Partition Test".to_string(),
            description: "Tests chain convergence after a temporary network partition".to_string(),
            network_config,
            initial_nodes,
            steps,
            expected_outcomes,
        }
    }
}

// Add unit tests for the test harness
#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_harness_basic_functionality() {
        let config = TestNetConfig::default();
        let mut harness = TestHarness::new(config);
        
        // Set up some test nodes
        let node_setups = vec![
            TestNodeSetup {
                id: 0,
                node_type: TestNodeType::Miner,
                initial_connections: vec![1],
                config_overrides: None,
            },
            TestNodeSetup {
                id: 1,
                node_type: TestNodeType::Full,
                initial_connections: vec![0],
                config_overrides: None,
            },
        ];
        
        assert!(harness.initialize_nodes(node_setups).is_ok());
        assert_eq!(harness.nodes.len(), 2);
        
        // Test mining blocks
        assert!(harness.mine_blocks(&[0], 5).await.is_ok());
        
        let node0 = harness.nodes.get(&0).unwrap();
        assert_eq!(node0.height, 5);
        
        // Test block propagation
        assert!(harness.propagate_blocks().await.is_ok());
        
        let node1 = harness.nodes.get(&1).unwrap();
        assert_eq!(node1.height, 5);
        
        // Test transaction sending
        assert!(harness.send_transactions(0, 1, 10).await.is_ok());
        
        let node1_after = harness.nodes.get(&1).unwrap();
        assert_eq!(node1_after.mempool_count, 10);
    }
} 