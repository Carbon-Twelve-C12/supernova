pub mod config;
pub mod faucet;
pub mod network_simulator;
pub mod test_harness;
pub mod regression_testing;

use crate::config::BlockchainConfig;
use config::TestNetConfig;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, warn};

/// Main testnet manager for handling test network operations
pub struct TestNetManager {
    /// Test network configuration
    config: TestNetConfig,
    /// Network simulator if enabled
    network_simulator: Option<network_simulator::NetworkSimulator>,
    /// Current mining difficulty
    current_difficulty: u64,
    /// Recent blocks for difficulty adjustment
    recent_blocks: Vec<(u64, u64)>, // (height, timestamp)
    /// Faucet manager if enabled
    faucet: Option<faucet::Faucet>,
    /// Blockchain configuration for the test network
    blockchain_config: BlockchainConfig,
}

impl TestNetManager {
    /// Create a new testnet manager with the specified configuration
    pub fn new(config: TestNetConfig) -> Self {
        let blockchain_config = convert_to_blockchain_config(&config);
        let current_difficulty = config.initial_difficulty;
        
        // Initialize network simulator if enabled
        let network_simulator = if let Some(sim_config) = &config.network_simulation {
            if sim_config.enabled {
                Some(network_simulator::NetworkSimulator::new(sim_config.clone()))
            } else {
                None
            }
        } else {
            None
        };
        
        // Initialize faucet if enabled
        let faucet = if config.enable_faucet {
            Some(faucet::Faucet::new(
                config.faucet_distribution_amount,
                config.faucet_cooldown_secs,
            ))
        } else {
            None
        };
        
        info!("Initializing test network: {}", config.network_name);
        if network_simulator.is_some() {
            info!("Network simulation enabled");
        }
        if faucet.is_some() {
            info!("Test faucet enabled");
        }
        
        Self {
            config,
            network_simulator,
            current_difficulty,
            recent_blocks: Vec::new(),
            faucet,
            blockchain_config,
        }
    }
    
    /// Create a new testnet manager with default configuration
    pub fn default() -> Self {
        Self::new(TestNetConfig::default())
    }
    
    /// Process a new block on the test network
    pub fn process_block(&mut self, height: u64, timestamp: u64, miner: Option<String>) {
        // Add to recent blocks
        self.recent_blocks.push((height, timestamp));
        
        // Keep only the blocks needed for difficulty adjustment
        let window = self.config.difficulty_adjustment_window as usize;
        if self.recent_blocks.len() > window * 2 {
            self.recent_blocks.drain(0..(self.recent_blocks.len() - window * 2));
        }
        
        // Check if we need to adjust difficulty (every window blocks)
        if height % self.config.difficulty_adjustment_window == 0 && height > 0 {
            self.adjust_difficulty();
        }
        
        info!(
            "Testnet block {} processed, current difficulty: {}", 
            height, 
            self.current_difficulty
        );
        
        if let Some(miner_address) = miner {
            info!("Block mined by: {}", miner_address);
        }
    }
    
    /// Adjust mining difficulty based on recent blocks
    fn adjust_difficulty(&mut self) {
        let window = self.config.difficulty_adjustment_window as usize;
        if self.recent_blocks.len() < window {
            return; // Not enough blocks to adjust difficulty
        }
        
        // Get the last window of blocks
        let window_blocks = &self.recent_blocks[self.recent_blocks.len() - window..];
        
        // Calculate new difficulty
        let new_difficulty = config::difficulty::calculate_next_difficulty(
            &self.config,
            self.current_difficulty,
            window_blocks,
        );
        
        // Log adjustment
        let adjustment_factor = new_difficulty as f64 / self.current_difficulty as f64;
        info!(
            "Difficulty adjusted: {} -> {} (factor: {:.2})",
            self.current_difficulty,
            new_difficulty,
            adjustment_factor
        );
        
        // Update current difficulty
        self.current_difficulty = new_difficulty;
    }
    
    /// Get the current mining difficulty
    pub fn get_current_difficulty(&self) -> u64 {
        self.current_difficulty
    }
    
    /// Request coins from the test faucet
    pub fn request_faucet_coins(&mut self, recipient: &str) -> Result<u64, String> {
        if let Some(faucet) = &mut self.faucet {
            faucet.distribute_coins(recipient)
        } else {
            Err("Faucet is not enabled for this test network".to_string())
        }
    }
    
    /// Apply network conditions to a connection between nodes
    pub fn apply_network_conditions(
        &mut self, 
        from_node: usize, 
        to_node: usize, 
        latency_ms: Option<u64>,
        packet_loss_percent: Option<u8>,
        bandwidth_kbps: Option<u64>,
    ) -> Result<(), String> {
        if let Some(simulator) = &mut self.network_simulator {
            simulator.set_connection_condition(
                from_node, 
                to_node, 
                latency_ms, 
                packet_loss_percent, 
                bandwidth_kbps,
            )
        } else {
            Err("Network simulation is not enabled for this test network".to_string())
        }
    }
    
    /// Simulate a network partition between two groups of nodes
    pub fn simulate_network_partition(
        &mut self, 
        group_a: &[usize], 
        group_b: &[usize],
    ) -> Result<(), String> {
        if let Some(simulator) = &mut self.network_simulator {
            simulator.create_partition(group_a, group_b)
        } else {
            Err("Network simulation is not enabled for this test network".to_string())
        }
    }
    
    /// Heal a network partition between previously separated groups
    pub fn heal_network_partition(
        &mut self, 
        group_a: &[usize], 
        group_b: &[usize],
    ) -> Result<(), String> {
        if let Some(simulator) = &mut self.network_simulator {
            simulator.heal_partition(group_a, group_b)
        } else {
            Err("Network simulation is not enabled for this test network".to_string())
        }
    }
    
    /// Get the blockchain configuration for this test network
    pub fn get_blockchain_config(&self) -> &BlockchainConfig {
        &self.blockchain_config
    }
}

/// Convert testnet configuration to blockchain configuration
fn convert_to_blockchain_config(testnet_config: &TestNetConfig) -> BlockchainConfig {
    let mut config = BlockchainConfig::default();
    
    // Modify the configuration for testnet use
    config.network.network_name = testnet_config.network_name.clone();
    config.consensus.target_block_time = testnet_config.target_block_time_secs;
    config.consensus.initial_difficulty = testnet_config.initial_difficulty;
    config.consensus.difficulty_adjustment_window = testnet_config.difficulty_adjustment_window;
    
    // Set testnet-specific settings
    config.network.p2p_port = testnet_config.p2p_port;
    config.network.rpc_port = testnet_config.rpc_port;
    config.network.dns_seeds = Vec::new(); // No DNS seeds for test networks
    config.network.allow_private_addresses = true; // Allow private IPs for testing
    
    // Set to test mode
    config.network.is_testnet = true;
    
    config
} 