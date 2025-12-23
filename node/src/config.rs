use crate::api::ApiConfig;
use config::{Config, ConfigError, Environment, File};
use notify::{self, RecommendedWatcher, RecursiveMode, Watcher};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::time::Duration;
use thiserror::Error;
use tracing::{error, info, warn};

#[derive(Debug, Error)]
pub enum NodeConfigValidationError {
    #[error("Invalid port: {0}")]
    InvalidPort(String),

    #[error("Port conflict: {0}")]
    PortConflict(String),

    #[error("Invalid value: {0}")]
    InvalidValue(String),

    #[error("Invalid path: {0}")]
    InvalidPath(String),

    #[error("Missing required field: {0}")]
    MissingField(String),
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct NodeConfig {
    pub network: NetworkConfig,
    pub storage: StorageConfig,
    pub mempool: MempoolConfig,
    pub backup: BackupConfig,
    pub node: GeneralConfig,
    pub checkpoint: CheckpointConfig,
    pub api: ApiConfig,
    pub testnet: TestnetConfig,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct NetworkConfig {
    pub listen_addr: String,
    pub max_peers: usize,
    pub bootstrap_nodes: Vec<String>,
    #[serde(with = "duration_serde")]
    pub peer_ping_interval: Duration,
    pub max_outbound_connections: usize,
    pub max_inbound_connections: usize,
    pub ban_threshold: u32,
    #[serde(with = "duration_serde")]
    pub ban_duration: Duration,

    // Added network configuration options
    pub key_path: Option<PathBuf>,
    pub network_id: String,
    pub enable_mdns: bool,
    pub enable_upnp: bool,
    pub enable_peer_exchange: bool,
    pub enable_nat_traversal: bool,
    #[serde(with = "duration_serde")]
    pub connection_timeout: Duration,
    #[serde(with = "duration_serde")]
    pub reconnect_interval: Duration,
    #[serde(with = "duration_serde")]
    pub status_broadcast_interval: Duration,
    pub trusted_peers: Vec<String>,
    pub min_outbound_connections: usize,
    pub peer_diversity: PeerDiversityConfig,
    pub pubsub_config: PubSubConfig,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct StorageConfig {
    pub db_path: PathBuf,
    pub enable_compression: bool,
    pub cache_size: usize,
    pub max_open_files: i32,
    pub block_cache_size: usize,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MempoolConfig {
    pub max_size: usize,
    #[serde(with = "duration_serde")]
    pub transaction_timeout: Duration,
    pub min_fee_rate: f64,
    pub max_per_address: usize,
    pub max_orphan_transactions: usize,
    pub enable_rbf: bool,
    pub min_rbf_fee_increase: f64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct BackupConfig {
    pub backup_dir: PathBuf,
    pub max_backups: usize,
    #[serde(with = "duration_serde")]
    pub backup_interval: Duration,
    pub enable_automated_backups: bool,
    pub compress_backups: bool,
    pub verify_on_startup: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GeneralConfig {
    pub chain_id: String,
    pub environment: NetworkEnvironment,
    pub metrics_enabled: bool,
    pub metrics_port: u16,
    pub log_level: String,
    pub network_name: String,
    pub enable_lightning: bool,
    pub enable_quantum_security: bool,
    pub enable_mining: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum NetworkEnvironment {
    Development,
    Testnet,
    Production,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CheckpointConfig {
    pub checkpoints_enabled: bool,
    #[serde(with = "duration_serde")]
    pub checkpoint_interval: Duration,
    pub checkpoint_type: String,
    pub data_dir: PathBuf,
    pub max_checkpoints: usize,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TestnetConfig {
    pub enabled: bool,
    pub network_id: String,
    pub enable_faucet: bool,
    pub faucet_amount: u64,
    pub faucet_cooldown: u64,
    pub faucet_max_balance: u64,
    pub enable_test_mining: bool,
    pub test_mining_difficulty: u64,
    pub enable_network_simulation: bool,
    pub simulated_latency_ms: u64,
    pub simulated_packet_loss: f64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PeerDiversityConfig {
    pub enabled: bool,
    pub min_diversity_score: f64,
    pub connection_strategy: String,
    #[serde(with = "duration_serde")]
    pub rotation_interval: Duration,
    pub max_peers_per_subnet: usize,
    pub max_peers_per_asn: usize,
    pub max_peers_per_region: usize,
    pub max_inbound_ratio: f64,
    pub max_connection_attempts_per_min: usize,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PubSubConfig {
    pub history_length: usize,
    pub history_gossip: usize,
    pub duplicate_cache_size: usize,
    #[serde(with = "duration_serde")]
    pub duplicate_cache_ttl: Duration,
    #[serde(with = "duration_serde")]
    pub heartbeat_interval: Duration,
    pub validation_mode: String,
    pub max_transmit_size: usize,
    pub explicit_relays: usize,
}

fn parse_libp2p_listen_port(listen_addr: &str) -> Result<u16, NodeConfigValidationError> {
    // Expected pattern contains "/tcp/<port>"
    let port_str = listen_addr
        .split("/tcp/")
        .nth(1)
        .and_then(|rest| rest.split('/').next())
        .ok_or_else(|| {
            NodeConfigValidationError::InvalidValue(format!(
                "network.listen_addr must contain '/tcp/<port>': got '{listen_addr}'"
            ))
        })?;

    let port: u16 = port_str.parse().map_err(|_| {
        NodeConfigValidationError::InvalidPort(format!(
            "network.listen_addr has invalid TCP port '{port_str}'"
        ))
    })?;

    if port == 0 {
        return Err(NodeConfigValidationError::InvalidPort(
            "network.listen_addr TCP port cannot be 0".to_string(),
        ));
    }

    Ok(port)
}

impl NetworkConfig {
    pub fn validate(&self) -> Result<(), NodeConfigValidationError> {
        let _listen_port = parse_libp2p_listen_port(&self.listen_addr)?;

        if self.max_peers < 8 {
            return Err(NodeConfigValidationError::InvalidValue(
                "network.max_peers must be >= 8".to_string(),
            ));
        }
        if self.max_inbound_connections > self.max_peers {
            return Err(NodeConfigValidationError::InvalidValue(
                "network.max_inbound_connections cannot exceed network.max_peers".to_string(),
            ));
        }
        if self.max_outbound_connections > self.max_peers {
            return Err(NodeConfigValidationError::InvalidValue(
                "network.max_outbound_connections cannot exceed network.max_peers".to_string(),
            ));
        }
        if self.max_inbound_connections == 0 {
            return Err(NodeConfigValidationError::InvalidValue(
                "network.max_inbound_connections must be > 0".to_string(),
            ));
        }
        if self.max_outbound_connections == 0 {
            return Err(NodeConfigValidationError::InvalidValue(
                "network.max_outbound_connections must be > 0".to_string(),
            ));
        }
        if self.min_outbound_connections < 8 {
            return Err(NodeConfigValidationError::InvalidValue(
                "network.min_outbound_connections must be >= 8".to_string(),
            ));
        }
        if self.min_outbound_connections > self.max_outbound_connections {
            return Err(NodeConfigValidationError::InvalidValue(
                "network.min_outbound_connections cannot exceed network.max_outbound_connections"
                    .to_string(),
            ));
        }
        if self.peer_ping_interval.as_secs() < 1 {
            return Err(NodeConfigValidationError::InvalidValue(
                "network.peer_ping_interval must be >= 1 second".to_string(),
            ));
        }
        if self.connection_timeout.as_secs() < 1 {
            return Err(NodeConfigValidationError::InvalidValue(
                "network.connection_timeout must be >= 1 second".to_string(),
            ));
        }
        if self.reconnect_interval.as_secs() < 1 {
            return Err(NodeConfigValidationError::InvalidValue(
                "network.reconnect_interval must be >= 1 second".to_string(),
            ));
        }
        if self.status_broadcast_interval.as_secs() < 1 {
            return Err(NodeConfigValidationError::InvalidValue(
                "network.status_broadcast_interval must be >= 1 second".to_string(),
            ));
        }
        if self.ban_threshold == 0 {
            return Err(NodeConfigValidationError::InvalidValue(
                "network.ban_threshold must be > 0".to_string(),
            ));
        }
        if self.ban_duration.as_secs() < 60 {
            return Err(NodeConfigValidationError::InvalidValue(
                "network.ban_duration must be >= 60 seconds".to_string(),
            ));
        }
        if self.network_id.trim().is_empty() {
            return Err(NodeConfigValidationError::MissingField(
                "network.network_id cannot be empty".to_string(),
            ));
        }

        self.peer_diversity.validate()?;
        self.pubsub_config.validate()?;
        Ok(())
    }
}

impl PeerDiversityConfig {
    pub fn validate(&self) -> Result<(), NodeConfigValidationError> {
        if !self.enabled {
            return Ok(());
        }
        if !(0.0..=1.0).contains(&self.min_diversity_score) {
            return Err(NodeConfigValidationError::InvalidValue(
                "network.peer_diversity.min_diversity_score must be between 0.0 and 1.0"
                    .to_string(),
            ));
        }
        if self.rotation_interval.as_secs() < 60 {
            return Err(NodeConfigValidationError::InvalidValue(
                "network.peer_diversity.rotation_interval must be >= 60 seconds".to_string(),
            ));
        }
        if self.max_peers_per_subnet == 0
            || self.max_peers_per_asn == 0
            || self.max_peers_per_region == 0
        {
            return Err(NodeConfigValidationError::InvalidValue(
                "network.peer_diversity max_peers_per_* values must be > 0".to_string(),
            ));
        }
        if self.max_inbound_ratio <= 0.0 {
            return Err(NodeConfigValidationError::InvalidValue(
                "network.peer_diversity.max_inbound_ratio must be > 0".to_string(),
            ));
        }
        if self.max_connection_attempts_per_min == 0 {
            return Err(NodeConfigValidationError::InvalidValue(
                "network.peer_diversity.max_connection_attempts_per_min must be > 0".to_string(),
            ));
        }
        Ok(())
    }
}

impl PubSubConfig {
    pub fn validate(&self) -> Result<(), NodeConfigValidationError> {
        if self.history_length == 0 {
            return Err(NodeConfigValidationError::InvalidValue(
                "network.pubsub_config.history_length must be > 0".to_string(),
            ));
        }
        if self.duplicate_cache_size == 0 {
            return Err(NodeConfigValidationError::InvalidValue(
                "network.pubsub_config.duplicate_cache_size must be > 0".to_string(),
            ));
        }
        if self.max_transmit_size == 0 {
            return Err(NodeConfigValidationError::InvalidValue(
                "network.pubsub_config.max_transmit_size must be > 0".to_string(),
            ));
        }
        if self.heartbeat_interval.as_secs() < 1 {
            return Err(NodeConfigValidationError::InvalidValue(
                "network.pubsub_config.heartbeat_interval must be >= 1 second".to_string(),
            ));
        }
        Ok(())
    }
}

impl StorageConfig {
    pub fn validate(&self) -> Result<(), NodeConfigValidationError> {
        if self.cache_size < 64 * 1024 * 1024 {
            return Err(NodeConfigValidationError::InvalidValue(
                "storage.cache_size must be >= 64MB".to_string(),
            ));
        }
        if self.max_open_files <= 0 {
            return Err(NodeConfigValidationError::InvalidValue(
                "storage.max_open_files must be > 0".to_string(),
            ));
        }
        if self.block_cache_size < 8 * 1024 * 1024 {
            return Err(NodeConfigValidationError::InvalidValue(
                "storage.block_cache_size must be >= 8MB".to_string(),
            ));
        }
        fs::create_dir_all(&self.db_path).map_err(|e| {
            NodeConfigValidationError::InvalidPath(format!(
                "Cannot create storage.db_path {:?}: {e}",
                self.db_path
            ))
        })?;
        Ok(())
    }
}

impl BackupConfig {
    pub fn validate(&self) -> Result<(), NodeConfigValidationError> {
        if self.max_backups == 0 {
            return Err(NodeConfigValidationError::InvalidValue(
                "backup.max_backups must be > 0".to_string(),
            ));
        }
        if self.backup_interval.as_secs() < 60 {
            return Err(NodeConfigValidationError::InvalidValue(
                "backup.backup_interval must be >= 60 seconds".to_string(),
            ));
        }
        fs::create_dir_all(&self.backup_dir).map_err(|e| {
            NodeConfigValidationError::InvalidPath(format!(
                "Cannot create backup.backup_dir {:?}: {e}",
                self.backup_dir
            ))
        })?;
        Ok(())
    }
}

impl MempoolConfig {
    pub fn validate(&self) -> Result<(), NodeConfigValidationError> {
        if self.max_size == 0 {
            return Err(NodeConfigValidationError::InvalidValue(
                "mempool.max_size must be > 0".to_string(),
            ));
        }
        if self.transaction_timeout.as_secs() < 60 {
            return Err(NodeConfigValidationError::InvalidValue(
                "mempool.transaction_timeout must be >= 60 seconds".to_string(),
            ));
        }
        if self.min_fee_rate < 0.0 {
            return Err(NodeConfigValidationError::InvalidValue(
                "mempool.min_fee_rate must be non-negative".to_string(),
            ));
        }
        if self.max_orphan_transactions == 0 {
            return Err(NodeConfigValidationError::InvalidValue(
                "mempool.max_orphan_transactions must be > 0".to_string(),
            ));
        }
        if self.enable_rbf && self.min_rbf_fee_increase < 0.0 {
            return Err(NodeConfigValidationError::InvalidValue(
                "mempool.min_rbf_fee_increase must be non-negative".to_string(),
            ));
        }
        Ok(())
    }
}

impl GeneralConfig {
    pub fn validate(&self) -> Result<(), NodeConfigValidationError> {
        if self.chain_id.trim().is_empty() {
            return Err(NodeConfigValidationError::MissingField(
                "node.chain_id cannot be empty".to_string(),
            ));
        }
        if self.network_name.trim().is_empty() {
            return Err(NodeConfigValidationError::MissingField(
                "node.network_name cannot be empty".to_string(),
            ));
        }
        if self.metrics_enabled && self.metrics_port == 0 {
            return Err(NodeConfigValidationError::InvalidPort(
                "node.metrics_port cannot be 0 when metrics are enabled".to_string(),
            ));
        }
        Ok(())
    }
}

impl CheckpointConfig {
    pub fn validate(&self) -> Result<(), NodeConfigValidationError> {
        if self.checkpoints_enabled {
            if self.checkpoint_interval.as_secs() < 60 {
                return Err(NodeConfigValidationError::InvalidValue(
                    "checkpoint.checkpoint_interval must be >= 60 seconds".to_string(),
                ));
            }
            if self.max_checkpoints == 0 {
                return Err(NodeConfigValidationError::InvalidValue(
                    "checkpoint.max_checkpoints must be > 0".to_string(),
                ));
            }
            fs::create_dir_all(&self.data_dir).map_err(|e| {
                NodeConfigValidationError::InvalidPath(format!(
                    "Cannot create checkpoint.data_dir {:?}: {e}",
                    self.data_dir
                ))
            })?;
        }
        Ok(())
    }
}

mod duration_serde {
    use serde::{Deserialize, Deserializer, Serializer};
    use std::time::Duration;

    pub fn serialize<S>(duration: &Duration, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_u64(duration.as_secs())
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Duration, D::Error>
    where
        D: Deserializer<'de>,
    {
        let secs = u64::deserialize(deserializer)?;
        Ok(Duration::from_secs(secs))
    }
}

impl Default for NetworkConfig {
    fn default() -> Self {
        Self {
            listen_addr: "/ip4/0.0.0.0/tcp/8000".to_string(),
            max_peers: 50,
            bootstrap_nodes: vec![],
            peer_ping_interval: Duration::from_secs(20), // Faster pings for 2.5-min blocks
            max_outbound_connections: 32,                // More connections for faster propagation
            max_inbound_connections: 128,
            ban_threshold: 100,
            ban_duration: Duration::from_secs(24 * 60 * 60),

            // New defaults
            key_path: None,
            network_id: "supernova-mainnet".to_string(),
            enable_mdns: true,
            enable_upnp: true,
            enable_peer_exchange: true,
            enable_nat_traversal: true,
            connection_timeout: Duration::from_secs(20), // Faster connection timeout
            reconnect_interval: Duration::from_secs(45), // Faster reconnection
            status_broadcast_interval: Duration::from_secs(120), // More frequent status updates
            trusted_peers: vec![],
            min_outbound_connections: 8,
            peer_diversity: PeerDiversityConfig::default(),
            pubsub_config: PubSubConfig::default(),
        }
    }
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            db_path: PathBuf::from("./data"),
            enable_compression: true,
            cache_size: 512 * 1024 * 1024,
            max_open_files: 1000,
            block_cache_size: 32 * 1024 * 1024,
        }
    }
}

impl Default for MempoolConfig {
    fn default() -> Self {
        Self {
            max_size: 5000,
            transaction_timeout: Duration::from_secs(1800), // 30 minutes (reduced for faster blocks)
            min_fee_rate: 1.0,
            max_per_address: 100,
            max_orphan_transactions: 100,
            enable_rbf: true,
            min_rbf_fee_increase: 10.0,
        }
    }
}

impl Default for BackupConfig {
    fn default() -> Self {
        Self {
            backup_dir: PathBuf::from("./backups"),
            max_backups: 5,
            backup_interval: Duration::from_secs(3600),
            enable_automated_backups: true,
            compress_backups: true,
            verify_on_startup: true,
        }
    }
}

impl Default for GeneralConfig {
    fn default() -> Self {
        Self {
            chain_id: "supernova-dev".to_string(),
            environment: NetworkEnvironment::Development,
            metrics_enabled: true,
            metrics_port: 9000,
            log_level: "info".to_string(),
            network_name: "Supernova".to_string(),
            enable_lightning: true,
            enable_quantum_security: true,
            enable_mining: true,
        }
    }
}

impl Default for CheckpointConfig {
    fn default() -> Self {
        Self {
            checkpoints_enabled: true,
            checkpoint_interval: Duration::from_secs(3600 * 24), // Daily checkpoints
            checkpoint_type: "Full".to_string(),
            data_dir: PathBuf::from("./checkpoints"),
            max_checkpoints: 7, // Keep a week of checkpoints
        }
    }
}

impl Default for TestnetConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            network_id: "testnet".to_string(),
            enable_faucet: false,
            faucet_amount: 1000000000000000000, // 1 trillion nova units
            faucet_cooldown: 60,                // 1 minute
            faucet_max_balance: 1000000000000000000, // 1 trillion nova units
            enable_test_mining: false,
            test_mining_difficulty: 1,
            enable_network_simulation: false,
            simulated_latency_ms: 0,
            simulated_packet_loss: 0.0,
        }
    }
}

impl Default for PeerDiversityConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            min_diversity_score: 0.7,
            connection_strategy: "BalancedDiversity".to_string(),
            rotation_interval: Duration::from_secs(3600 * 6), // 6 hours
            max_peers_per_subnet: 3,
            max_peers_per_asn: 5,
            max_peers_per_region: 10,
            max_inbound_ratio: 3.0,
            max_connection_attempts_per_min: 5,
        }
    }
}

impl Default for PubSubConfig {
    fn default() -> Self {
        Self {
            history_length: 5,
            history_gossip: 3,
            duplicate_cache_size: 1000,
            duplicate_cache_ttl: Duration::from_secs(120),
            heartbeat_interval: Duration::from_secs(10),
            validation_mode: "Strict".to_string(),
            max_transmit_size: 1024 * 1024 * 5, // 5 MB
            explicit_relays: 3,
        }
    }
}

impl NodeConfig {
    pub fn load() -> Result<Self, ConfigError> {
        let mut config = Config::builder();
        config = config.add_source(Config::try_from(&Self::default())?);

        // Try multiple config file locations
        let config_paths = vec![
            PathBuf::from("config.toml"),           // Root directory
            PathBuf::from("config/node.toml"),      // Legacy location
            PathBuf::from(".supernova/node.toml"),  // User directory
        ];
        
        let mut config_loaded = false;
        for config_path in &config_paths {
            if config_path.exists() {
                info!("Loading configuration from: {:?}", config_path);
                if let Some(config_str) = config_path.to_str() {
                    config = config.add_source(File::with_name(config_str.trim_end_matches(".toml")));
                    config_loaded = true;
                    break;
                }
            }
        }
        
        if !config_loaded {
            warn!("No configuration file found, using defaults");
            if let Err(e) = Self::create_default_config(&PathBuf::from("config.toml")) {
                warn!("Failed to create default config file: {}", e);
            }
        }

        config = config.add_source(
            Environment::with_prefix("SUPERNOVA")
                .separator("_")
                .try_parsing(true),
        );

        let config: NodeConfig = config.build()?.try_deserialize()?;
        Self::ensure_directories(&config)?;
        if let Err(e) = config.validate() {
            return Err(ConfigError::Message(format!("Configuration validation error: {e}")));
        }

        // Log loaded configuration for debugging
        info!("Configuration loaded:");
        info!("  Network listen_addr: {}", config.network.listen_addr);
        info!("  Bootstrap nodes: {} configured", config.network.bootstrap_nodes.len());
        for (i, node) in config.network.bootstrap_nodes.iter().enumerate() {
            info!("    [{}] {}", i, node);
        }

        Ok(config)
    }

    pub async fn reload(&mut self) -> Result<(), ConfigError> {
        info!("Reloading configuration");
        match Self::load() {
            Ok(new_config) => {
                if let Err(e) = new_config.validate() {
                    error!("Invalid configuration: {}", e);
                    return Err(ConfigError::Message(format!(
                        "Invalid configuration: {}",
                        e
                    )));
                }
                *self = new_config;
                info!("Configuration reloaded successfully");
                Ok(())
            }
            Err(e) => {
                error!("Failed to reload configuration: {}", e);
                Err(e)
            }
        }
    }

    pub async fn watch_config() -> Result<tokio::sync::mpsc::Receiver<()>, ConfigError> {
        let config_path = Self::config_path();
        let (tx, rx) = tokio::sync::mpsc::channel(1);

        let watcher_result = RecommendedWatcher::new(
            move |res: Result<notify::Event, notify::Error>| match res {
                Ok(event) => {
                    if event.kind.is_modify() {
                        if let Err(e) = tx.try_send(()) {
                            match e {
                                tokio::sync::mpsc::error::TrySendError::Full(_) => {
                                    error!("Config reload channel full");
                                }
                                tokio::sync::mpsc::error::TrySendError::Closed(_) => {
                                    error!("Config reload channel closed");
                                }
                            }
                        }
                    }
                }
                Err(e) => error!("Config watch error: {}", e),
            },
            notify::Config::default(),
        );

        let mut watcher = match watcher_result {
            Ok(w) => w,
            Err(e) => return Err(ConfigError::Foreign(Box::new(NotifyError(e)))),
        };

        let watch_result = watcher.watch(&config_path, RecursiveMode::NonRecursive);
        if let Err(e) = watch_result {
            return Err(ConfigError::Foreign(Box::new(NotifyError(e))));
        }

        Ok(rx)
    }

    fn create_default_config(path: &PathBuf) -> std::io::Result<()> {
        let default_config = Self::default();
        let toml = toml::to_string_pretty(&default_config)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;

        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        fs::write(path, toml)?;
        info!("Created default configuration file at {:?}", path);
        Ok(())
    }

    fn ensure_directories(config: &NodeConfig) -> Result<(), ConfigError> {
        // Create storage directory
        if let Err(e) = fs::create_dir_all(&config.storage.db_path) {
            return Err(ConfigError::Foreign(Box::new(IoError(e))));
        }
        info!(
            "Ensured storage directory exists at {:?}",
            config.storage.db_path
        );

        // Create backup directory
        if let Err(e) = fs::create_dir_all(&config.backup.backup_dir) {
            return Err(ConfigError::Foreign(Box::new(IoError(e))));
        }
        info!(
            "Ensured backup directory exists at {:?}",
            config.backup.backup_dir
        );

        Ok(())
    }

    pub fn validate(&self) -> Result<(), NodeConfigValidationError> {
        // Validate per-module config first
        self.network.validate()?;
        self.storage.validate()?;
        self.mempool.validate()?;
        self.backup.validate()?;
        self.node.validate()?;
        self.checkpoint.validate()?;

        // Cross-field validation
        let p2p_port = parse_libp2p_listen_port(&self.network.listen_addr)?;
        if self.api.port == 0 {
            return Err(NodeConfigValidationError::InvalidPort(
                "api.port cannot be 0".to_string(),
            ));
        }
        if p2p_port == self.api.port {
            return Err(NodeConfigValidationError::PortConflict(format!(
                "network.listen_addr TCP port ({p2p_port}) must differ from api.port ({})",
                self.api.port
            )));
        }

        // Security: refuse to start with default API key in non-development environments.
        // This prevents accidental deployment with insecure credentials.
        let is_non_dev = matches!(
            self.node.environment,
            NetworkEnvironment::Production | NetworkEnvironment::Testnet
        );
        let has_default_key = self
            .api
            .api_keys
            .as_ref()
            .map(|keys| keys.iter().any(|k| k.contains("CHANGE-ME")))
            .unwrap_or(true);

        if is_non_dev && self.api.enable_auth && has_default_key {
            let env_name = match self.node.environment {
                NetworkEnvironment::Production => "production",
                NetworkEnvironment::Testnet => "testnet",
                NetworkEnvironment::Development => "development",
            };
            return Err(NodeConfigValidationError::InvalidValue(format!(
                "api.enable_auth is true in {}, but api.api_keys is missing or contains \
                 the default 'CHANGE-ME' key. Generate a secure key with: \
                 supernova-node generate-api-key",
                env_name
            )));
        }

        Ok(())
    }

    /// Get the path to the configuration file
    fn config_path() -> PathBuf {
        PathBuf::from("config/node.toml")
    }
}

/// Custom error wrapper that converts to ConfigError
#[derive(Debug)]
pub struct IoError(std::io::Error);

impl From<std::io::Error> for IoError {
    fn from(err: std::io::Error) -> Self {
        IoError(err)
    }
}

impl From<IoError> for ConfigError {
    fn from(err: IoError) -> Self {
        ConfigError::NotFound(err.0.to_string())
    }
}

impl std::fmt::Display for IoError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "IO Error: {}", self.0)
    }
}

impl std::error::Error for IoError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.0)
    }
}

/// Custom error wrapper that converts to ConfigError
#[derive(Debug)]
pub struct NotifyError(notify::Error);

impl From<notify::Error> for NotifyError {
    fn from(err: notify::Error) -> Self {
        NotifyError(err)
    }
}

impl From<NotifyError> for ConfigError {
    fn from(err: NotifyError) -> Self {
        ConfigError::Foreign(Box::new(err.0))
    }
}

impl std::fmt::Display for NotifyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Notify Error: {}", self.0)
    }
}

impl std::error::Error for NotifyError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.0)
    }
}
