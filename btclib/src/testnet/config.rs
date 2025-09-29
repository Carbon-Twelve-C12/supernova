use serde::{Deserialize, Serialize};
use std::path::PathBuf;

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
    /// Transaction propagation configuration
    pub tx_propagation: TransactionPropagationConfig,
    /// Blockchain storage configuration
    pub storage: StorageConfig,
    /// Auto-mining configuration (for automated tests)
    pub auto_mining: Option<AutoMiningConfig>,
    /// Block explorer configuration
    pub block_explorer: Option<BlockExplorerConfig>,
    /// Fast sync options for test networks
    pub fast_sync: FastSyncConfig,
    /// Logging and metrics configuration
    pub logging: LoggingConfig,
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
    /// Pre-allocated test accounts with different balances
    pub test_accounts: Vec<TestAccount>,
}

/// Test account configuration for test networks
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestAccount {
    /// Account name (for reference)
    pub name: String,
    /// Account address
    pub address: String,
    /// Initial balance in millinova (1 NOVA = 1,000 millinova)
    pub balance: u64,
    /// Whether this account is a miner
    pub is_miner: bool,
    /// Optional private key (for automated tests)
    pub private_key: Option<String>,
}

/// Initial coin distribution entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoinDistribution {
    /// Recipient address
    pub address: String,
    /// Amount in millinova (1 NOVA = 1,000 millinova)
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
    /// Network jitter simulation (random latency variation)
    pub jitter_ms: u64,
    /// Network topology simulation
    pub topology: NetworkTopology,
    /// Periodic connectivity disruptions
    pub disruption_schedule: Option<DisruptionSchedule>,
}

impl Default for NetworkSimulationConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            latency_ms_mean: 100,
            latency_ms_std_dev: 50,
            packet_loss_percent: 0,
            bandwidth_limit_kbps: 0,
            simulate_clock_drift: false,
            max_clock_drift_ms: 500,
            jitter_ms: 20,
            topology: NetworkTopology::FullyConnected,
            disruption_schedule: None,
        }
    }
}

/// Network topology configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NetworkTopology {
    /// All nodes connected to all others
    FullyConnected,
    /// Ring topology where each node connects only to adjacent nodes
    Ring,
    /// Star topology with a central node
    Star { central_node: usize },
    /// Random topology with specified connection probability
    Random { connection_probability: f64 },
    /// Custom topology with explicit connections
    Custom { connections: Vec<(usize, usize)> },
}

/// Configuration for scheduled network disruptions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DisruptionSchedule {
    /// How often to cause disruptions (in seconds)
    pub frequency_secs: u64,
    /// Duration of each disruption (in seconds)
    pub duration_secs: u64,
    /// Percentage of nodes affected (0-100)
    pub affected_nodes_percent: u8,
    /// Type of disruption to simulate
    pub disruption_type: DisruptionType,
}

/// Type of network disruption to simulate
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DisruptionType {
    /// Complete disconnection
    Disconnection,
    /// High latency
    HighLatency { latency_ms: u64 },
    /// Packet loss
    PacketLoss { loss_percent: u8 },
    /// Limited bandwidth
    LimitedBandwidth { kbps: u64 },
}

/// Transaction propagation configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionPropagationConfig {
    /// Base delay for transaction propagation in milliseconds
    pub base_delay_ms: u64,
    /// Whether to prioritize transactions by fee
    pub prioritize_by_fee: bool,
    /// Maximum transactions to relay per round
    pub max_relay_count: usize,
    /// Whether to simulate transaction censorship by some nodes
    pub simulate_censorship: bool,
}

/// Storage configuration for test networks
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageConfig {
    /// Where to store blockchain data
    pub data_dir: PathBuf,
    /// Whether to use memory-only storage
    pub in_memory: bool,
    /// Flush interval in seconds (0 = flush immediately)
    pub flush_interval_secs: u64,
    /// Whether to compress stored blocks
    pub compress_blocks: bool,
    /// Maximum block height to store (for pruning)
    pub max_height: Option<u64>,
}

/// Automated mining configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutoMiningConfig {
    /// Whether to mine blocks automatically
    pub enabled: bool,
    /// Target time between blocks in seconds
    pub block_interval_secs: u64,
    /// Maximum transactions per block
    pub max_transactions_per_block: usize,
    /// Node IDs that will be mining
    pub mining_nodes: Vec<usize>,
}

/// Block explorer configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockExplorerConfig {
    /// Whether to enable the block explorer
    pub enabled: bool,
    /// Port for the explorer web interface
    pub port: u16,
    /// How many blocks to display in the explorer
    pub max_blocks_to_display: usize,
    /// Whether to collect additional explorer metrics
    pub collect_metrics: bool,
}

/// Fast sync configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FastSyncConfig {
    /// Whether to enable fast sync for test networks
    pub enabled: bool,
    /// Checkpoint interval (in blocks)
    pub checkpoint_interval: u64,
    /// Number of blocks to validate during fast sync
    pub validation_sample_size: u64,
}

/// Logging and metrics configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    /// Log level for test network
    pub log_level: LogLevel,
    /// Whether to log to file
    pub log_to_file: bool,
    /// Log file path
    pub log_file: Option<PathBuf>,
    /// Whether to enable metrics collection
    pub collect_metrics: bool,
    /// Metrics export interval in seconds
    pub metrics_interval_secs: u64,
}

/// Log level enum
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LogLevel {
    /// Error logs only
    Error,
    /// Warning and error logs
    Warning,
    /// Info, warning, and error logs
    Info,
    /// Debug and above logs
    Debug,
    /// Trace and above logs
    Trace,
}

impl Default for TestNetConfig {
    fn default() -> Self {
        Self {
            network_name: "supernova-testnet".to_string(),
            target_block_time_secs: 150, // 2.5 minutes between blocks (matches mainnet)
            initial_difficulty: 100_000, // Low initial difficulty for easier mining
            difficulty_adjustment_window: 2016, // Adjust every 2016 blocks (~3.5 days)
            max_difficulty_adjustment_factor: 4.0, // Allow up to 4x difficulty change
            genesis_config: GenesisConfig {
                timestamp: 1672531200, // June 1, 2025
                initial_distribution: vec![CoinDistribution {
                    address: "test1qqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqq"
                        .to_string(),
                    amount: 420_000_000_000, // 4.2 million NOVA for test faucet
                }],
                message: "supernova Test Network Genesis Block".to_string(),
                test_accounts: vec![
                    TestAccount {
                        name: "Alice".to_string(),
                        address: "test1alice1111111111111111111111111111111111111111111111111"
                            .to_string(),
                        balance: 100_000_000_000, // 1000 NOVA
                        is_miner: true,
                        private_key: None,
                    },
                    TestAccount {
                        name: "Bob".to_string(),
                        address: "test1bob111111111111111111111111111111111111111111111111111"
                            .to_string(),
                        balance: 50_000_000_000, // 500 NOVA
                        is_miner: false,
                        private_key: None,
                    },
                ],
            },
            enable_faucet: true,
            faucet_distribution_amount: 10_000_000_000, // 100 NOVA equivalent
            faucet_cooldown_secs: 3600,                 // 1 hour
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
                jitter_ms: 20,
                topology: NetworkTopology::FullyConnected,
                disruption_schedule: None,
            }),
            tx_propagation: TransactionPropagationConfig {
                base_delay_ms: 100,
                prioritize_by_fee: true,
                max_relay_count: 1000,
                simulate_censorship: false,
            },
            storage: StorageConfig {
                data_dir: PathBuf::from("./testnet_data"),
                in_memory: true,
                flush_interval_secs: 10,
                compress_blocks: false,
                max_height: None,
            },
            auto_mining: Some(AutoMiningConfig {
                enabled: false,
                block_interval_secs: 150, // 2.5 minutes to match target block time
                max_transactions_per_block: 1000,
                mining_nodes: vec![0],
            }),
            block_explorer: Some(BlockExplorerConfig {
                enabled: false,
                port: 8080,
                max_blocks_to_display: 100,
                collect_metrics: true,
            }),
            fast_sync: FastSyncConfig {
                enabled: true,
                checkpoint_interval: 100,
                validation_sample_size: 10,
            },
            logging: LoggingConfig {
                log_level: LogLevel::Info,
                log_to_file: false,
                log_file: None,
                collect_metrics: true,
                metrics_interval_secs: 30,
            },
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

        // Enable auto-mining for continuous block generation
        if let Some(auto_mining) = config.auto_mining.as_mut() {
            auto_mining.enabled = true;
            auto_mining.block_interval_secs = 5;
        }

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
        sim_config.jitter_ms = 50;

        // Add periodic network disruptions
        sim_config.disruption_schedule = Some(DisruptionSchedule {
            frequency_secs: 600, // Every 10 minutes
            duration_secs: 60,   // 1 minute disruption
            affected_nodes_percent: 30,
            disruption_type: DisruptionType::HighLatency { latency_ms: 2000 },
        });

        config.network_simulation = Some(sim_config);

        config
    }

    /// Create a testnet preset for performance testing
    pub fn create_performance_testnet() -> TestNetConfig {
        let mut config = TestNetConfig::default();
        config.network_name = "supernova-perftest".to_string();
        config.target_block_time_secs = 150; // 2.5 minutes for stable timing matching mainnet

        // Disable network simulation for optimal performance
        if let Some(sim_config) = config.network_simulation.as_mut() {
            sim_config.enabled = false;
        }

        // Use memory storage for faster performance
        config.storage.in_memory = true;

        config
    }

    /// Create a testnet for regression testing
    pub fn create_regression_testnet() -> TestNetConfig {
        let mut config = TestNetConfig::default();
        config.network_name = "supernova-regression".to_string();

        // Make network deterministic by disabling random elements
        if let Some(sim_config) = config.network_simulation.as_mut() {
            sim_config.enabled = true;
            sim_config.latency_ms_std_dev = 0; // Fixed latency
            sim_config.jitter_ms = 0; // No jitter
            sim_config.simulate_clock_drift = false;
            sim_config.disruption_schedule = None;
        }

        // Use fixed difficulty for predictable block times
        config.target_block_time_secs = 15;
        config.max_difficulty_adjustment_factor = 1.0; // No adjustment

        config
    }

    /// Create a testnet with clock drift for consensus testing
    pub fn create_clock_drift_testnet() -> TestNetConfig {
        let mut config = TestNetConfig::default();
        config.network_name = "supernova-clockdrift".to_string();

        // Enable clock drift simulation
        if let Some(sim_config) = config.network_simulation.as_mut() {
            sim_config.enabled = true;
            sim_config.simulate_clock_drift = true;
            sim_config.max_clock_drift_ms = 5000; // 5 seconds drift
        }

        config
    }

    /// Create a testnet for stress testing with high transaction volume
    pub fn create_stress_test_testnet() -> TestNetConfig {
        let mut config = TestNetConfig::default();
        config.network_name = "supernova-stress".to_string();

        // Fast blocks
        config.target_block_time_secs = 5;

        // Configure for high transaction throughput
        config.tx_propagation.max_relay_count = 10000;
        config.tx_propagation.base_delay_ms = 50; // Faster propagation

        // Auto-mining with large blocks
        if let Some(auto_mining) = config.auto_mining.as_mut() {
            auto_mining.enabled = true;
            auto_mining.max_transactions_per_block = 5000;
        }

        config
    }

    /// Create a testnet for fork resolution testing
    pub fn create_fork_testnet() -> TestNetConfig {
        let mut config = TestNetConfig::default();
        config.network_name = "supernova-fork".to_string();

        // Enable network simulation with partition capability
        if let Some(sim_config) = config.network_simulation.as_mut() {
            sim_config.enabled = true;
            sim_config.topology = NetworkTopology::Custom {
                connections: vec![
                    // Two groups of nodes with minimal connections between them
                    (0, 1),
                    (1, 2),
                    (2, 3),
                    (3, 0), // Group A
                    (4, 5),
                    (5, 6),
                    (6, 7),
                    (7, 4), // Group B
                    (0, 4), // Single connection between groups
                ],
            };

            // Schedule periodic partitioning
            sim_config.disruption_schedule = Some(DisruptionSchedule {
                frequency_secs: 300, // Every 5 minutes
                duration_secs: 120,  // 2 minutes of partition
                affected_nodes_percent: 100,
                disruption_type: DisruptionType::Disconnection,
            });
        }

        config
    }
}
