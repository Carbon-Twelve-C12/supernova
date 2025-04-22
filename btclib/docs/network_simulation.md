# SuperNova Network Simulation Guide

## Overview

The SuperNova blockchain includes a powerful network simulation framework designed to test blockchain behavior under various network conditions. This framework allows developers to:

- Test blockchain performance in realistic network environments
- Simulate adverse conditions like high latency, packet loss, and bandwidth constraints
- Create network partitions to validate consensus mechanisms
- Verify block and transaction propagation across different node types
- Simulate clock drift between nodes

## Test Harness Architecture

The test harness is composed of several main components:

### Core Components

- **TestHarness**: The main orchestrator that manages test networks and scenarios
- **TestScenario**: Defines a test case with network configuration, node setup, test steps, and expected outcomes
- **NetworkSimulator**: Controls network conditions between nodes
- **TestNetManager**: Manages the lifecycle of test nodes

### Supporting Types

- **TestNodeType**: Defines node roles (Miner, Full, Light)
- **TestNodeStatus**: Tracks node state (Starting, Running, Stopped, Failed)
- **TestStep**: Defines actions to take during a test (MineBlocks, SendTransactions, etc.)
- **TestOutcome**: Defines expected outcomes to verify after test completion

## Getting Started

### Basic Usage

To run network simulations, you'll need to:

1. Create a `TestNetConfig` with appropriate network simulation settings
2. Initialize a `TestHarness` with this configuration
3. Define node setups and connections
4. Create a `TestScenario` with test steps and expected outcomes
5. Run the scenario and analyze results

### Example: Basic Connectivity Test

```rust
// Create a test network config with network simulation enabled
let mut config = TestNetConfig::default();
config.network_name = "basic-connectivity-test".to_string();

// Enable network simulation with minimal interference
let mut sim_config = NetworkSimulationConfig::default();
sim_config.enabled = true;
sim_config.latency_ms_mean = 50;
sim_config.latency_ms_std_dev = 10;
config.network_simulation = Some(sim_config);

// Initialize test harness
let mut harness = TestHarness::new(config);

// Setup a network with 4 nodes
let node_setups = vec![
    TestNodeSetup {
        id: 0,
        node_type: TestNodeType::Miner,
        initial_connections: vec![1, 2, 3],
        config_overrides: None,
    },
    // ... other nodes
];

// Create and run a test scenario
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
        // ... other steps
    ],
    expected_outcomes: vec![
        // All nodes should have the same chain tip
        TestOutcome::AllNodesHaveSameChainTip,
    ],
};

// Run the scenario
let result = harness.run_scenario(scenario).await;
```

## Network Simulation Features

### Simulating Network Conditions

You can simulate various network conditions:

```rust
// Set specific network conditions between nodes
TestStep::SetNetworkCondition {
    from_node: 0,
    to_node: 1,
    latency_ms: Some(300),
    packet_loss_percent: Some(10),
    bandwidth_kbps: Some(250),
}
```

### Creating Network Partitions

Test network partitions and recovery:

```rust
// Create a network partition
TestStep::CreatePartition {
    group_a: vec![0, 1, 2],
    group_b: vec![3, 4, 5],
}

// Later, heal the partition
TestStep::HealPartition {
    group_a: vec![0, 1, 2],
    group_b: vec![3, 4, 5],
}
```

### Simulating Clock Drift

Test time synchronization issues:

```rust
// Set clock drift on a node
TestStep::SetClockDrift {
    node_id: 4,
    drift_ms: 2000, // 2 seconds ahead
}
```

## Common Test Scenarios

The network simulator can test several real-world scenarios:

### 1. Block Propagation Performance

Test how quickly blocks propagate across the network under different conditions.

### 2. Network Partitions and Chain Convergence

Test what happens when the network is split and different chains form, then verify the network converges when reconnected.

### 3. Transaction Propagation Under Constrained Bandwidth

Test transaction propagation when network bandwidth is limited.

### 4. Resistance to Eclipse Attacks

Test node behavior when isolated from honest nodes.

### 5. Consensus Under Clock Drift

Test how consensus mechanisms handle unsynchronized node clocks.

## Best Practices

- **Allow sufficient wait times**: Especially for high-latency or packet-loss scenarios
- **Test progressively**: Start with ideal conditions, then add constraints
- **Test both normal and adversarial conditions**: Don't just test the happy path
- **Use realistic network parameters**: Base simulation parameters on real-world measurements
- **Verify all node types**: Test miners, full nodes, and light clients

## Advanced Configuration

### Preset Configurations

The `presets` module provides common network configurations:

```rust
// Use the network simulation preset
let config = presets::create_simulation_testnet();
```

### Custom Network Topologies

You can create custom network topologies by specifying connections:

```rust
// Ring topology
let node_setups = vec![
    TestNodeSetup {
        id: 0,
        initial_connections: vec![1, 4], // Connect to neighbors
        // ...
    },
    TestNodeSetup {
        id: 1,
        initial_connections: vec![0, 2],
        // ...
    },
    // ... and so on
];
```

## Further Resources

For more details, see the comprehensive examples in:
- `btclib/examples/network_simulation.rs`

For implementation details, review:
- `btclib/src/testnet/test_harness.rs`
- `btclib/src/testnet/network_simulator.rs` 