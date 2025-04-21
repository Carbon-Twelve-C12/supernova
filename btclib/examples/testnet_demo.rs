use btclib::testnet::{
    TestNetManager,
    config::{TestNetConfig, presets},
};
use std::time::{Duration, Instant};
use std::thread;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("SuperNova Testnet Demo");
    println!("=====================\n");
    
    // Create a high-speed testnet configuration
    let high_speed_config = presets::create_high_speed_testnet();
    println!("Created high-speed testnet with block time: {} seconds", high_speed_config.target_block_time_secs);
    
    // Initialize the testnet
    let mut testnet = TestNetManager::new(high_speed_config);
    println!("Testnet initialized with difficulty: {}\n", testnet.get_current_difficulty());
    
    // Simulate mining blocks
    println!("Simulating block mining...");
    simulate_block_mining(&mut testnet, 25)?;
    
    // Try faucet distribution
    println!("\nTesting faucet distribution...");
    let test_address = "test1q0jru4pue9zk83ljt4eqxstx2uv4d6sp98yvz06";
    match testnet.request_faucet_coins(test_address) {
        Ok(amount) => println!("Successfully distributed {} satoshis to {}", amount, test_address),
        Err(e) => println!("Faucet distribution failed: {}", e),
    }
    
    // Try immediate second distribution (should fail due to cooldown)
    match testnet.request_faucet_coins(test_address) {
        Ok(amount) => println!("Successfully distributed {} satoshis to {}", amount, test_address),
        Err(e) => println!("Second distribution attempt: {}", e),
    }
    
    // Create a testnet with network simulation
    println!("\nCreating a testnet with network simulation...");
    let sim_config = presets::create_simulation_testnet();
    let mut sim_testnet = TestNetManager::new(sim_config);
    
    // Simulate a network with 6 nodes
    println!("Simulating a network with 6 nodes");
    println!("Setting latency between nodes 0 and 1...");
    sim_testnet.apply_network_conditions(0, 1, Some(200), None, None)?;
    
    // Simulate a network partition
    println!("Creating a network partition between nodes [0,1,2] and [3,4,5]...");
    sim_testnet.simulate_network_partition(&[0, 1, 2], &[3, 4, 5])?;
    
    // Simulate mining on both partitions
    println!("Mining blocks on both partitions...");
    
    // Mine blocks on first partition (nodes 0-2)
    println!("Mining 5 blocks on first partition...");
    for i in 0..5 {
        // In a real scenario, these would be mined by different nodes
        // For simplicity, we're just simulating the arrival of blocks
        sim_testnet.process_block(i, current_timestamp(), Some("node0".to_string()));
        thread::sleep(Duration::from_millis(500));
    }
    
    // Mine blocks on second partition (nodes 3-5)
    println!("Mining 3 blocks on second partition...");
    for i in 0..3 {
        sim_testnet.process_block(i, current_timestamp(), Some("node3".to_string()));
        thread::sleep(Duration::from_millis(500));
    }
    
    // Heal the partition
    println!("Healing the network partition...");
    sim_testnet.heal_network_partition(&[0, 1, 2], &[3, 4, 5])?;
    
    println!("\nIn a real scenario, the network would now reconcile the fork");
    println!("and converge on the longest valid chain (the 5-block chain).");
    
    println!("\nTestnet demo completed successfully!");
    
    Ok(())
}

/// Simulate mining a series of blocks with realistic timestamps
fn simulate_block_mining(testnet: &mut TestNetManager, count: u64) -> Result<(), Box<dyn std::error::Error>> {
    let target_time = testnet.get_blockchain_config().consensus.target_block_time;
    let mut last_time = current_timestamp();
    
    for i in 0..count {
        // Simulate some randomness in block times (±20%)
        let random_factor = 0.8 + (i as f64 % 4.0) * 0.1; // Between 0.8 and 1.1
        let time_increment = (target_time as f64 * random_factor) as u64;
        
        // Calculate next block time
        let next_time = last_time + time_increment;
        
        // Process the block
        testnet.process_block(i, next_time, Some(format!("miner{}", i % 3)));
        
        // Update last time
        last_time = next_time;
        
        // Display difficulty adjustment if it happens
        if i > 0 && i % testnet.get_blockchain_config().consensus.difficulty_adjustment_window == 0 {
            println!("  Block {} - New difficulty: {}", i, testnet.get_current_difficulty());
        } else {
            println!("  Block {} - Mined at time {} ({}s after previous)", 
                    i, next_time, time_increment);
        }
        
        // Short sleep to make the output readable
        thread::sleep(Duration::from_millis(100));
    }
    
    Ok(())
}

/// Get current timestamp in seconds
fn current_timestamp() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards")
        .as_secs()
} 