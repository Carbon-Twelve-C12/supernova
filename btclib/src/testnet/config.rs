use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Configuration for the test network
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestNetConfig {
    /// Network name
    pub network_name: String,
    /// Target time between blocks in seconds
    pub target_block_time_secs: u64,
    /// Initial mining difficulty (higher = more difficult)
    pub initial_difficulty: u64,
    /// Difficulty adjustment window (in blocks)
    pub difficulty_adjustment_window: u64,
    /// Maximum difficulty adjustment factor per window
    pub max_difficulty_adjustment_factor: f64,
    /// Genesis block configuration
    pub genesis_config: GenesisConfig,
    /// Whether to enable test faucet
    pub enable_faucet: bool,
    /// Maximum faucet distribution per request
    pub faucet_distribution_amount: u64,
    /// Faucet cooldown period in seconds
    pub faucet_cooldown_secs: u64,
    /// Test network listening port
    pub p2p_port: u16,
    /// RPC port for test network
    pub rpc_port: u16,
    /// Network simulation options
    pub network_simulation: Option<NetworkSimulationConfig>,
}

/// Genesis block configuration for test networks
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenesisConfig {
    /// Timestamp for genesis block
    pub timestamp: u64,
    /// Initial coin distribution
    pub initial_distribution: Vec<CoinDistribution>,
    /// Custom genesis message
    pub message: String,
}

/// Initial coin distribution entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoinDistribution {
    /// Recipient address
    pub address: String,
    /// Amount in satoshis
    pub amount: u64,
}

/// Network simulation configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkSimulationConfig {
    /// Whether to enable network simulation
    pub enabled: bool,
    /// Simulated latency in milliseconds (mean)
    pub latency_ms_mean: u64,
    /// Latency standard deviation in milliseconds
    pub latency_ms_std_dev: u64,
    /// Packet loss percentage (0-100)
    pub packet_loss_percent: u8,
    /// Bandwidth limit in kilobits per second (0 = unlimited)
    pub bandwidth_limit_kbps: u64,
    /// Whether to simulate clock drift
    pub simulate_clock_drift: bool,
    /// Maximum clock drift in milliseconds
    pub max_clock_drift_ms: u64,
}

impl Default for TestNetConfig {
    fn default() -> Self {
        Self {
            network_name: "supernova-testnet".to_string(),
            target_block_time_secs: 10, // 10 seconds between blocks (fast for testing)
            initial_difficulty: 100_000, // Low initial difficulty for easier mining
            difficulty_adjustment_window: 20, // Adjust every 20 blocks (faster adjustments)
            max_difficulty_adjustment_factor: 4.0, // Allow up to 4x difficulty change
            genesis_config: GenesisConfig {
                timestamp: 1672531200, // January 1, 2023
                initial_distribution: vec![
                    CoinDistribution {
                        address: "test1qqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqq".to_string(),
                        amount: 5_000_000_000, // 50 BTC equivalent for test faucet
                    },
                ],
                message: "SuperNova Test Network Genesis Block".to_string(),
            },
            enable_faucet: true,
            faucet_distribution_amount: 100_000_000, // 1 BTC equivalent
            faucet_cooldown_secs: 3600, // 1 hour
            p2p_port: 18444,
            rpc_port: 18443,
            network_simulation: Some(NetworkSimulationConfig {
                enabled: false, // Disabled by default
                latency_ms_mean: 100,
                latency_ms_std_dev: 50,
                packet_loss_percent: 0,
                bandwidth_limit_kbps: 0,
                simulate_clock_drift: false,
                max_clock_drift_ms: 500,
            }),
        }
    }
}

/// Difficulty calculation module for test networks
pub mod difficulty {
    use super::*;
    
    /// Calculate the next difficulty based on the time taken to mine the last window of blocks
    pub fn calculate_next_difficulty(
        config: &TestNetConfig,
        current_difficulty: u64,
        window_blocks: &[(u64, u64)], // (block_height, timestamp) pairs
    ) -> u64 {
        if window_blocks.len() < 2 {
            return current_difficulty; // Not enough blocks to adjust
        }
        
        // Sort blocks by height to ensure correct order
        let mut sorted_blocks = window_blocks.to_vec();
        sorted_blocks.sort_by_key(|&(height, _)| height);
        
        let window_size = sorted_blocks.len() as u64;
        let first_block = sorted_blocks.first().unwrap();
        let last_block = sorted_blocks.last().unwrap();
        
        // Calculate the actual time taken for this window
        let time_diff = last_block.1.saturating_sub(first_block.1);
        if time_diff == 0 {
            return current_difficulty; // Avoid division by zero
        }
        
        // Calculate the expected time for this window
        let expected_time = config.target_block_time_secs * (window_size - 1);
        
        // Calculate the adjustment factor
        let mut adjustment_factor = expected_time as f64 / time_diff as f64;
        
        // Limit the adjustment factor
        let max_factor = config.max_difficulty_adjustment_factor;
        adjustment_factor = adjustment_factor.max(1.0 / max_factor).min(max_factor);
        
        // Apply the adjustment
        let new_difficulty = (current_difficulty as f64 * adjustment_factor) as u64;
        
        // Ensure difficulty doesn't drop below the initial level
        new_difficulty.max(config.initial_difficulty)
    }
}

/// Testnet presets for different testing scenarios
pub mod presets {
    use super::*;
    
    /// Create a high-speed testnet configuration
    pub fn create_high_speed_testnet() -> TestNetConfig {
        let mut config = TestNetConfig::default();
        config.network_name = "supernova-highspeed".to_string();
        config.target_block_time_secs = 5; // 5 seconds between blocks
        config.difficulty_adjustment_window = 10; // Adjust every 10 blocks
        config
    }
    
    /// Create a network simulation testnet
    pub fn create_simulation_testnet() -> TestNetConfig {
        let mut config = TestNetConfig::default();
        config.network_name = "supernova-netsim".to_string();
        
        // Enable network simulation
        let mut sim_config = config.network_simulation.unwrap_or_default();
        sim_config.enabled = true;
        sim_config.latency_ms_mean = 200;
        sim_config.latency_ms_std_dev = 100;
        sim_config.packet_loss_percent = 2;
        sim_config.bandwidth_limit_kbps = 1000;
        config.network_simulation = Some(sim_config);
        
        config
    }
    
    /// Create a testnet preset for performance testing
    pub fn create_performance_testnet() -> TestNetConfig {
        let mut config = TestNetConfig::default();
        config.network_name = "supernova-perftest".to_string();
        config.target_block_time_secs = 30; // 30 seconds for more stable timing
        
        // Disable network simulation for optimal performance
        if let Some(sim_config) = config.network_simulation.as_mut() {
            sim_config.enabled = false;
        }
        
        config
    }
} 