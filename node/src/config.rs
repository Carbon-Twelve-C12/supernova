use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::Duration;
use std::fs;
use tracing::{info, warn, error};
use config::{Config, ConfigError, Environment, File};
use notify::{self, Watcher, RecommendedWatcher, RecursiveMode};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct NodeConfig {
    pub network: NetworkConfig,
    pub storage: StorageConfig,
    pub mempool: MempoolConfig,
    pub backup: BackupConfig,
    pub node: GeneralConfig,
    pub checkpoint: CheckpointConfig,
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

impl Default for NodeConfig {
    fn default() -> Self {
        Self {
            network: NetworkConfig::default(),
            storage: StorageConfig::default(),
            mempool: MempoolConfig::default(),
            backup: BackupConfig::default(),
            node: GeneralConfig::default(),
            checkpoint: CheckpointConfig::default(),
        }
    }
}

impl Default for NetworkConfig {
    fn default() -> Self {
        Self {
            listen_addr: "/ip4/0.0.0.0/tcp/8000".to_string(),
            max_peers: 50,
            bootstrap_nodes: vec![],
            peer_ping_interval: Duration::from_secs(30),
            max_outbound_connections: 8,
            max_inbound_connections: 32,
            ban_threshold: 100,
            ban_duration: Duration::from_secs(24 * 60 * 60),
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
            transaction_timeout: Duration::from_secs(3600 * 2),
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

impl NodeConfig {
    pub fn load() -> Result<Self, ConfigError> {
        let mut config = Config::builder();
        config = config.add_source(Config::try_from(&Self::default())?);

        let config_path = PathBuf::from("config/node.toml");
        if config_path.exists() {
            config = config.add_source(File::with_name("config/node.toml"));
        } else {
            warn!("Configuration file not found at {:?}, using defaults", config_path);
            if let Err(e) = Self::create_default_config(&config_path) {
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

        Ok(config)
    }

    pub async fn reload(&mut self) -> Result<(), ConfigError> {
        info!("Reloading configuration");
        match Self::load() {
            Ok(new_config) => {
                if let Err(e) = new_config.validate() {
                    error!("Invalid configuration: {}", e);
                    return Err(ConfigError::Message(format!("Invalid configuration: {}", e)));
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
            move |res: Result<notify::Event, notify::Error>| {
                match res {
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
                }
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
        info!("Ensured storage directory exists at {:?}", config.storage.db_path);

        // Create backup directory
        if let Err(e) = fs::create_dir_all(&config.backup.backup_dir) {
            return Err(ConfigError::Foreign(Box::new(IoError(e))));
        }
        info!("Ensured backup directory exists at {:?}", config.backup.backup_dir);

        Ok(())
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.backup.max_backups == 0 {
            return Err("max_backups must be greater than 0".to_string());
        }
        if self.backup.backup_interval.as_secs() < 60 {
            return Err("backup_interval must be at least 60 seconds".to_string());
        }

        if self.network.max_peers == 0 {
            return Err("max_peers must be greater than 0".to_string());
        }
        if self.network.peer_ping_interval.as_secs() < 1 {
            return Err("peer_ping_interval must be at least 1 second".to_string());
        }
        if self.network.max_inbound_connections == 0 {
            return Err("max_inbound_connections must be greater than 0".to_string());
        }
        if self.network.max_outbound_connections == 0 {
            return Err("max_outbound_connections must be greater than 0".to_string());
        }

        if self.mempool.max_size == 0 {
            return Err("mempool max_size must be greater than 0".to_string());
        }
        if self.mempool.min_fee_rate < 0.0 {
            return Err("min_fee_rate must be non-negative".to_string());
        }
        if self.mempool.max_orphan_transactions == 0 {
            return Err("max_orphan_transactions must be greater than 0".to_string());
        }
        if self.mempool.min_rbf_fee_increase < 0.0 {
            return Err("min_rbf_fee_increase must be non-negative".to_string());
        }

        if self.storage.max_open_files < 100 {
            return Err("max_open_files must be at least 100".to_string());
        }
        if self.storage.block_cache_size == 0 {
            return Err("block_cache_size must be greater than 0".to_string());
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