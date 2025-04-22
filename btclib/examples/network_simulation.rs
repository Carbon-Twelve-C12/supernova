use btclib::testnet::{
    config::{TestNetConfig, NetworkSimulationConfig, presets},
    test_harness::{
        TestHarness, TestScenario, TestNodeSetup, TestStep, TestOutcome,
        TestNodeType, TestNodeStatus
    }
};
use std::collections::HashMap;
use std::time::Duration;
use tokio::time::sleep;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing for logging
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();
    
    println!("SuperNova Network Simulation Example");
    println!("===================================");
    
    // Run basic connectivity test
    println!("\n[1/3] Running basic connectivity test...");
    let basic_result = run_basic_connectivity_test().await?;
    report_test_result(&basic_result);
    
    // Run network partition test
    println!("\n[2/3] Running network partition test...");
    let partition_result = run_network_partition_test().await?;
    report_test_result(&partition_result);
    
    // Run adverse network conditions test
    println!("\n[3/3] Running adverse network conditions test...");
    let adverse_result = run_adverse_conditions_test().await?;
    report_test_result(&adverse_result);
    
    println!("\nAll network simulation tests completed!");
    
    Ok(())
}

/// Run a basic connectivity test to verify nodes can communicate
async fn run_basic_connectivity_test() -> Result<HashMap<String, bool>, Box<dyn std::error::Error>> {
    // Create a test network config with network simulation enabled
    let mut config = TestNetConfig::default();
    config.network_name = "basic-connectivity-test".to_string();
    
    // Enable network simulation with minimal interference
    let mut sim_config = NetworkSimulationConfig::default();
    sim_config.enabled = true;
    sim_config.latency_ms_mean = 50;
    sim_config.latency_ms_std_dev = 10;
    sim_config.packet_loss_percent = 0;
    config.network_simulation = Some(sim_config);
    
    // Initialize test harness
    let mut harness = TestHarness::new(config);
    
    // Setup a simple network with 4 nodes
    let node_setups = vec![
        TestNodeSetup {
            id: 0,
            node_type: TestNodeType::Miner,
            initial_connections: vec![1, 2, 3],
            config_overrides: None,
        },
        TestNodeSetup {
            id: 1,
            node_type: TestNodeType::Full,
            initial_connections: vec![0, 2, 3],
            config_overrides: None,
        },
        TestNodeSetup {
            id: 2,
            node_type: TestNodeType::Full,
            initial_connections: vec![0, 1, 3],
            config_overrides: None,
        },
        TestNodeSetup {
            id: 3,
            node_type: TestNodeType::Light,
            initial_connections: vec![0, 1, 2],
            config_overrides: None,
        },
    ];
    
    // Create the test scenario
    let scenario = TestScenario {
        name: "Basic Connectivity Test".to_string(),
        description: "Tests basic connectivity and block propagation".to_string(),
        network_config: config,
        initial_nodes: node_setups,
        steps: vec![
            // Mine 5 blocks on node 0
            TestStep::MineBlocks {
                node_ids: vec![0],
                block_count: 5,
            },
            // Wait for propagation
            TestStep::Wait(Duration::from_secs(2)),
            // Send transactions from node 1 to node 2
            TestStep::SendTransactions {
                from_node: 1,
                to_node: 2,
                tx_count: 10,
            },
            // Wait again for propagation
            TestStep::Wait(Duration::from_secs(2)),
            // Mine another block to include transactions
            TestStep::MineBlocks {
                node_ids: vec![0],
                block_count: 1,
            },
            // Wait for final propagation
            TestStep::Wait(Duration::from_secs(2)),
        ],
        expected_outcomes: vec![
            // All nodes should have the same chain tip
            TestOutcome::AllNodesHaveSameChainTip,
            // Node 0 should be at height 6
            TestOutcome::NodeAtHeight {
                node_id: 0,
                height: 6,
            },
            // Node 3 (light client) should also be at height 6
            TestOutcome::NodeAtHeight {
                node_id: 3,
                height: 6,
            },
        ],
    };
    
    // Run the scenario
    let result = harness.run_scenario(scenario).await;
    
    // Return results as a map
    let mut results = HashMap::new();
    results.insert("Basic Connectivity".to_string(), result.passed);
    results.insert("Block Propagation".to_string(), 
        !result.failed_outcomes.iter().any(|o| o.contains("NodeAtHeight")));
    results.insert("Transaction Propagation".to_string(), 
        !result.failed_outcomes.iter().any(|o| o.contains("NodeHasTransactions")));
    
    Ok(results)
}

/// Run a network partition test to see how the network recovers
async fn run_network_partition_test() -> Result<HashMap<String, bool>, Box<dyn std::error::Error>> {
    // Use the network simulation preset
    let config = presets::create_simulation_testnet();
    
    // Initialize test harness
    let mut harness = TestHarness::new(config.clone());
    
    // Setup a network with 6 nodes in a fully connected topology
    let node_setups = vec![
        TestNodeSetup {
            id: 0,
            node_type: TestNodeType::Miner,
            initial_connections: vec![1, 2, 3, 4, 5],
            config_overrides: None,
        },
        TestNodeSetup {
            id: 1,
            node_type: TestNodeType::Miner,
            initial_connections: vec![0, 2, 3, 4, 5],
            config_overrides: None,
        },
        TestNodeSetup {
            id: 2,
            node_type: TestNodeType::Full,
            initial_connections: vec![0, 1, 3, 4, 5],
            config_overrides: None,
        },
        TestNodeSetup {
            id: 3,
            node_type: TestNodeType::Full,
            initial_connections: vec![0, 1, 2, 4, 5],
            config_overrides: None,
        },
        TestNodeSetup {
            id: 4,
            node_type: TestNodeType::Full,
            initial_connections: vec![0, 1, 2, 3, 5],
            config_overrides: None,
        },
        TestNodeSetup {
            id: 5,
            node_type: TestNodeType::Light,
            initial_connections: vec![0, 1, 2, 3, 4],
            config_overrides: None,
        },
    ];
    
    // Create the test scenario
    let scenario = TestScenario {
        name: "Network Partition Test".to_string(),
        description: "Tests network recovery after partition".to_string(),
        network_config: config,
        initial_nodes: node_setups,
        steps: vec![
            // Mine initial blocks to establish blockchain
            TestStep::MineBlocks {
                node_ids: vec![0],
                block_count: 3,
            },
            // Wait for propagation
            TestStep::Wait(Duration::from_secs(2)),
            
            // Create a network partition: Group A (0,1,2) and Group B (3,4,5)
            TestStep::CreatePartition {
                group_a: vec![0, 1, 2],
                group_b: vec![3, 4, 5],
            },
            
            // Mine different blocks in each partition
            TestStep::MineBlocks {
                node_ids: vec![0],
                block_count: 2, // Mine 2 blocks in Group A
            },
            TestStep::MineBlocks {
                node_ids: vec![1],
                block_count: 1, // Mine 1 more block in Group A
            },
            TestStep::MineBlocks {
                node_ids: vec![3],
                block_count: 2, // Mine 2 blocks in Group B (will create a fork)
            },
            
            // Verify the partitions have different chain tips
            TestStep::Wait(Duration::from_secs(2)),
            
            // Heal the partition
            TestStep::HealPartition {
                group_a: vec![0, 1, 2],
                group_b: vec![3, 4, 5],
            },
            
            // Wait for network to converge
            TestStep::Wait(Duration::from_secs(5)),
            
            // Mine one more block to finalize the winning chain
            TestStep::MineBlocks {
                node_ids: vec![0],
                block_count: 1,
            },
            
            // Wait for final propagation
            TestStep::Wait(Duration::from_secs(3)),
        ],
        expected_outcomes: vec![
            // After healing, all nodes should have the same chain tip
            TestOutcome::AllNodesHaveSameChainTip,
            
            // All nodes should be at the same height (either 6 or 7 depending on which chain won)
            // We're not asserting exact height since it depends on which chain wins
        ],
    };
    
    // Run the scenario
    let result = harness.run_scenario(scenario).await;
    
    // Return results as a map
    let mut results = HashMap::new();
    results.insert("Network Partition Recovery".to_string(), result.passed);
    results.insert("Chain Convergence".to_string(), 
        !result.failed_outcomes.iter().any(|o| o.contains("AllNodesHaveSameChainTip")));
    
    Ok(results)
}

/// Run a test with adverse network conditions
async fn run_adverse_conditions_test() -> Result<HashMap<String, bool>, Box<dyn std::error::Error>> {
    // Create a test network config with challenging network conditions
    let mut config = TestNetConfig::default();
    config.network_name = "adverse-conditions-test".to_string();
    
    // Enable network simulation with challenging conditions
    let mut sim_config = NetworkSimulationConfig::default();
    sim_config.enabled = true;
    sim_config.latency_ms_mean = 200;
    sim_config.latency_ms_std_dev = 100;
    sim_config.packet_loss_percent = 5;
    sim_config.bandwidth_limit_kbps = 500;
    sim_config.simulate_clock_drift = true;
    sim_config.max_clock_drift_ms = 500;
    config.network_simulation = Some(sim_config);
    
    // Initialize test harness
    let mut harness = TestHarness::new(config.clone());
    
    // Setup a network with 5 nodes
    let node_setups = vec![
        TestNodeSetup {
            id: 0,
            node_type: TestNodeType::Miner,
            initial_connections: vec![1, 2, 3, 4],
            config_overrides: None,
        },
        TestNodeSetup {
            id: 1,
            node_type: TestNodeType::Miner,
            initial_connections: vec![0, 2, 3, 4],
            config_overrides: None,
        },
        TestNodeSetup {
            id: 2,
            node_type: TestNodeType::Full,
            initial_connections: vec![0, 1, 3, 4],
            config_overrides: None,
        },
        TestNodeSetup {
            id: 3,
            node_type: TestNodeType::Full,
            initial_connections: vec![0, 1, 2, 4],
            config_overrides: None,
        },
        TestNodeSetup {
            id: 4,
            node_type: TestNodeType::Light,
            initial_connections: vec![0, 1, 2, 3],
            config_overrides: None,
        },
    ];
    
    // Create the test scenario
    let scenario = TestScenario {
        name: "Adverse Network Conditions Test".to_string(),
        description: "Tests blockchain performance under poor network conditions".to_string(),
        network_config: config,
        initial_nodes: node_setups,
        steps: vec![
            // Set specific network conditions between nodes
            TestStep::SetNetworkCondition {
                from_node: 0,
                to_node: 1,
                latency_ms: Some(300),
                packet_loss_percent: Some(10),
                bandwidth_kbps: Some(250),
            },
            TestStep::SetNetworkCondition {
                from_node: 2,
                to_node: 3,
                latency_ms: Some(500),
                packet_loss_percent: Some(15),
                bandwidth_kbps: Some(200),
            },
            
            // Set extreme clock drift on one node
            TestStep::SetClockDrift {
                node_id: 4,
                drift_ms: 2000, // 2 seconds ahead
            },
            
            // Mine initial blocks
            TestStep::MineBlocks {
                node_ids: vec![0],
                block_count: 3,
            },
            
            // Wait longer for propagation due to poor conditions
            TestStep::Wait(Duration::from_secs(5)),
            
            // Mine blocks from different nodes
            TestStep::MineBlocks {
                node_ids: vec![1],
                block_count: 2,
            },
            
            // Send transactions under adverse conditions
            TestStep::SendTransactions {
                from_node: 0,
                to_node: 2,
                tx_count: 15,
            },
            TestStep::SendTransactions {
                from_node: 1,
                to_node: 3,
                tx_count: 10,
            },
            
            // Wait longer for propagation
            TestStep::Wait(Duration::from_secs(8)),
            
            // Mine a block to include transactions
            TestStep::MineBlocks {
                node_ids: vec![0],
                block_count: 1,
            },
            
            // Wait for final propagation
            TestStep::Wait(Duration::from_secs(10)),
        ],
        expected_outcomes: vec![
            // Despite adverse conditions, nodes should eventually converge
            TestOutcome::AllNodesHaveSameChainTip,
            
            // Check specific nodes are at expected heights
            TestOutcome::NodeAtHeight {
                node_id: 0,
                height: 6,
            },
            TestOutcome::NodeAtHeight {
                node_id: 4,
                height: 6,
            },
        ],
    };
    
    // Run the scenario
    let result = harness.run_scenario(scenario).await;
    
    // Return results as a map
    let mut results = HashMap::new();
    results.insert("Adverse Conditions Convergence".to_string(), result.passed);
    results.insert("Block Propagation under Latency".to_string(), 
        !result.failed_outcomes.iter().any(|o| o.contains("NodeAtHeight")));
    results.insert("Clock Drift Handling".to_string(), 
        !result.failed_outcomes.iter().any(|o| o.contains("node_id: 4")));
    
    Ok(results)
}

/// Report the test results in a user-friendly format
fn report_test_result(results: &HashMap<String, bool>) {
    let total = results.len();
    let passed = results.values().filter(|&&v| v).count();
    
    println!("\nTest Results: {}/{} passed", passed, total);
    println!("----------------------------");
    
    for (test, passed) in results {
        let status = if *passed { "PASSED" } else { "FAILED" };
        println!("  {} ... {}", test, status);
    }
} 