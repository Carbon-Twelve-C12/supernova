# SuperNova Test Network

This document outlines the test network infrastructure provided by the SuperNova blockchain project. The test network features are designed to facilitate development, testing, and experimentation with various network conditions.

## Overview

The SuperNova Test Network provides:

1. **Fast Block Times**: Configurable block intervals as low as 5 seconds for rapid iteration
2. **Simplified Difficulty Adjustment**: Faster and more predictable difficulty changes for testing
3. **Test Faucet**: Easy distribution of test coins
4. **Network Simulation**: Tools for testing network conditions like latency, packet loss, and partitions
5. **Multiple Preset Configurations**: Pre-configured test environments for different testing scenarios

## Usage

The test network can be used in different ways:

### 1. Starting a Local Test Network

```rust
use btclib::testnet::{TestNetManager, config::presets};

// Initialize with a high-speed configuration
let mut testnet = TestNetManager::new(presets::create_high_speed_testnet());

// Start the network
println!("Test network started with difficulty: {}", testnet.get_current_difficulty());
```

### 2. Requesting Test Coins from Faucet

```rust
// Request test coins
match testnet.request_faucet_coins("test1q0jru4pue9zk83ljt4eqxstx2uv4d6sp98yvz06") {
    Ok(amount) => println!("Received {} satoshis", amount),
    Err(e) => println!("Faucet request failed: {}", e),
}
```

### 3. Simulating Network Conditions

```rust
// Create testnet with network simulation enabled
let mut testnet = TestNetManager::new(presets::create_simulation_testnet());

// Set latency between nodes
testnet.apply_network_conditions(0, 1, Some(200), None, None)
       .expect("Failed to apply network conditions");

// Simulate packet loss
testnet.apply_network_conditions(2, 3, None, Some(5), None)
       .expect("Failed to apply network conditions");
```

### 4. Testing Network Partitions

```rust
// Create a network partition
testnet.simulate_network_partition(&[0, 1, 2], &[3, 4, 5])
       .expect("Failed to create partition");

// Heal the partition later
testnet.heal_network_partition(&[0, 1, 2], &[3, 4, 5])
       .expect("Failed to heal partition");
```

## Configuration Options

### Network Configuration

The test network can be configured with various parameters:

```rust
use btclib::testnet::config::TestNetConfig;

let mut config = TestNetConfig::default();

// Set custom parameters
config.target_block_time_secs = 15;                // 15 seconds between blocks
config.initial_difficulty = 50_000;                // Lower initial difficulty
config.difficulty_adjustment_window = 10;          // Adjust every 10 blocks
config.max_difficulty_adjustment_factor = 2.0;     // Limit difficulty changes
config.enable_faucet = true;                       // Enable test faucet
config.faucet_distribution_amount = 50_000_000;    // 0.5 NOVA per request
```

### Network Simulation Settings

Network simulation allows testing of various network conditions:

```rust
// Enable network simulation
config.network_simulation = Some(ConfigNetworkSimulationConfig {
    enabled: true,
    latency_ms_mean: 150,                  // 150ms average latency
    latency_ms_std_dev: 50,                // With 50ms standard deviation
    packet_loss_percent: 2,                // 2% packet loss
    bandwidth_limit_kbps: 1000,            // 1Mbps bandwidth limit
    simulate_clock_drift: true,            // Enable clock drift simulation
    max_clock_drift_ms: 200,               // Up to 200ms drift
});
```

## Preset Configurations

Several preset configurations are available for common testing scenarios:

1. **High-Speed Testnet**: For rapid development and testing
   ```rust
   let config = presets::create_high_speed_testnet();
   ```

2. **Network Simulation Testnet**: For testing resilience to network conditions
   ```rust
   let config = presets::create_simulation_testnet();
   ```

3. **Performance Testing Testnet**: For benchmarking and performance testing
   ```rust
   let config = presets::create_performance_testnet();
   ```

## Testing Scenarios

The test network infrastructure enables various testing scenarios:

### 1. Fork Resolution

Test how the network resolves competing chains by creating a network partition:

1. Create a network partition between two groups of nodes
2. Mine different blocks on each partition
3. Heal the partition
4. Observe how the network reconciles the competing chains

### 2. Difficulty Adjustment

Test the difficulty adjustment algorithm:

1. Set up a high-speed testnet
2. Mine blocks at varying speeds
3. Observe how difficulty adjusts to maintain target block time

### 3. Network Resilience

Test how the network performs under adverse conditions:

1. Introduce high latency between some nodes
2. Add packet loss to simulate unreliable connections
3. Limit bandwidth to test performance under constrained conditions
4. Simulate clock drift to test timestamp validation

## Integration with Test Suites

The test network can be integrated with automated test suites:

```rust
#[test]
fn test_network_partition_recovery() {
    let mut testnet = TestNetManager::new(presets::create_simulation_testnet());
    
    // Create a partition
    testnet.simulate_network_partition(&[0, 1, 2], &[3, 4, 5]).unwrap();
    
    // Mine blocks on both partitions
    // ...
    
    // Heal the partition
    testnet.heal_network_partition(&[0, 1, 2], &[3, 4, 5]).unwrap();
    
    // Verify that the network converged on the expected chain
    // ...
}
```

## Future Enhancements

Planned enhancements to the test network infrastructure include:

1. **Reproducible Test Scenarios**: Save and replay specific network conditions
2. **GUI Test Dashboard**: Visual interface for monitoring test network status
3. **Distributed Test Networks**: Support for multi-machine test setups
4. **Automated Stress Testing**: Tools for generating high transaction volumes
5. **Custom Attack Simulations**: Simulate specific attack vectors for security testing 