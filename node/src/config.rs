use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::Duration;
use std::fs;
use tracing::{info, warn, error};
use config::{Config, ConfigError, Environment, File};
use notify::{self, Event, EventKind};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct NodeConfig {
    pub network: NetworkConfig,
    pub storage: StorageConfig,
    pub mempool: MempoolConfig,
    pub backup: BackupConfig,
    pub node: GeneralConfig,
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
    pub environment: Environment,
    pub metrics_enabled: bool,
    pub metrics_port: u16,
    pub log_level: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum Environment {
    Development,
    Testnet,
    Production,
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
            environment: Environment::Development,
            metrics_enabled: true,
            metrics_port: 9000,
            log_level: "info".to_string(),
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

    pub async fn watch_config() -> Result<tokio::sync::mpsc::Receiver<()>, std::io::Error> {
        let (tx, rx) = tokio::sync::mpsc::channel(1);
        let config_path = PathBuf::from("config/node.toml");
        
        let mut watcher = notify::recommended_watcher(move |res| {
            match res {
                Ok(event) => {
                    if let Event { kind: EventKind::Modify(_), .. } = event {
                        if let Err(e) = tx.blocking_send(()) {
                            error!("Failed to send config reload notification: {}", e);
                        }
                    }
                }
                Err(e) => error!("Config watch error: {}", e),
            }
        })?;

        watcher.watch(&config_path, notify::RecursiveMode::NonRecursive)?;
        
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

    fn ensure_directories(&self) -> std::io::Result<()> {
        fs::create_dir_all(&self.storage.db_path)?;
        info!("Ensured storage directory exists at {:?}", self.storage.db_path);

        fs::create_dir_all(&self.backup.backup_dir)?;
        info!("Ensured backup directory exists at {:?}", self.backup.backup_dir);

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

        if self.storage.max_open_files < 100 {
            return Err("max_open_files must be at least 100".to_string());
        }
        if self.storage.block_cache_size == 0 {
            return Err("block_cache_size must be greater than 0".to_string());
        }

        Ok(())
    }
}