pub mod config;
pub mod faucet;
pub mod network_simulator;
pub mod regression_testing;
pub mod test_harness;

// Re-export TestNetConfig for public use
pub use self::config::TestNetConfig;

// Use consistent imports
use self::network_simulator::{NetworkSimulator, SimulationConfig};
use tracing::info;

/// Main testnet manager for handling test network operations
pub struct TestNetManager {
    /// Test network configuration
    config: TestNetConfig,
    /// Network simulator if enabled
    network_simulator: Option<NetworkSimulator>,
    /// Current mining difficulty
    current_difficulty: u64,
    /// Recent blocks for difficulty adjustment
    recent_blocks: Vec<(u64, u64)>, // (height, timestamp)
    /// Faucet manager if enabled
    faucet: Option<faucet::Faucet>,
    /// Blockchain configuration for the test network
    blockchain_config: TestNetConfig,
}

impl TestNetManager {
    /// Create a new testnet manager with the specified configuration
    pub fn new(config: TestNetConfig) -> Self {
        let blockchain_config = convert_to_blockchain_config(&config);
        let current_difficulty = config.initial_difficulty;

        // Initialize network simulator if enabled
        let network_simulator = if let Some(sim_config) = &config.network_simulation {
            if sim_config.enabled {
                // Create our SimulationConfig from the NetworkSimulationConfig
                let simulator_config = SimulationConfig {
                    enabled: sim_config.enabled,
                    latency_ms_mean: sim_config.latency_ms_mean,
                    latency_ms_std_dev: sim_config.latency_ms_std_dev,
                    packet_loss_percent: sim_config.packet_loss_percent,
                    bandwidth_limit_kbps: sim_config.bandwidth_limit_kbps,
                    simulate_clock_drift: sim_config.simulate_clock_drift,
                    max_clock_drift_ms: sim_config.max_clock_drift_ms,
                    jitter_ms: sim_config.jitter_ms,
                };
                Some(NetworkSimulator::new(simulator_config))
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
            self.recent_blocks
                .drain(0..(self.recent_blocks.len() - window * 2));
        }

        // Check if we need to adjust difficulty (every window blocks)
        if height % self.config.difficulty_adjustment_window == 0 && height > 0 {
            self.adjust_difficulty();
        }

        info!(
            "Testnet block {} processed, current difficulty: {}",
            height, self.current_difficulty
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
            self.current_difficulty, new_difficulty, adjustment_factor
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
            faucet
                .distribute_coins(recipient)
                .map_err(|e| e.to_string())
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
    pub fn get_blockchain_config(&self) -> &TestNetConfig {
        &self.blockchain_config
    }

    /// Get the network simulator if enabled
    pub fn get_network_simulator(&self) -> Option<&NetworkSimulator> {
        self.network_simulator.as_ref()
    }
}

/// Convert testnet configuration to blockchain configuration
fn convert_to_blockchain_config(testnet_config: &TestNetConfig) -> TestNetConfig {
    // Instead of trying to modify config like before, create a new TestNetConfig
    // with the correct values copied over
    TestNetConfig {
        network_name: testnet_config.network_name.clone(),
        target_block_time_secs: testnet_config.target_block_time_secs,
        initial_difficulty: testnet_config.initial_difficulty,
        difficulty_adjustment_window: testnet_config.difficulty_adjustment_window,
        max_difficulty_adjustment_factor: testnet_config.max_difficulty_adjustment_factor,
        p2p_port: testnet_config.p2p_port,
        rpc_port: testnet_config.rpc_port,
        // Set reasonable defaults for other fields
        genesis_config: testnet_config.genesis_config.clone(),
        enable_faucet: testnet_config.enable_faucet,
        faucet_distribution_amount: testnet_config.faucet_distribution_amount,
        faucet_cooldown_secs: testnet_config.faucet_cooldown_secs,
        network_simulation: testnet_config.network_simulation.clone(),
        tx_propagation: testnet_config.tx_propagation.clone(),
        storage: testnet_config.storage.clone(),
        auto_mining: testnet_config.auto_mining.clone(),
        block_explorer: testnet_config.block_explorer.clone(),
        fast_sync: testnet_config.fast_sync.clone(),
        logging: testnet_config.logging.clone(),
    }
}
