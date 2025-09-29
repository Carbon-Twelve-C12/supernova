use anyhow::{Context, Result};
use dirs::home_dir;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// RPC endpoint URL
    pub rpc_url: String,

    /// Network (mainnet, testnet, devnet)
    pub network: String,

    /// Default wallet path
    pub wallet_path: Option<PathBuf>,

    /// Request timeout in seconds
    pub timeout: u64,

    /// Enable debug logging
    pub debug: bool,

    /// Output format (json, table, text)
    pub output_format: OutputFormat,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum OutputFormat {
    Json,
    Table,
    Text,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            rpc_url: "http://localhost:9332".to_string(),
            network: "testnet".to_string(),
            wallet_path: None,
            timeout: 30,
            debug: false,
            output_format: OutputFormat::Table,
        }
    }
}

impl Config {
    /// Load configuration from file or create default
    pub fn load() -> Result<Self> {
        let config_path = Self::config_path()?;

        if config_path.exists() {
            let contents =
                fs::read_to_string(&config_path).context("Failed to read config file")?;
            let config: Config =
                toml::from_str(&contents).context("Failed to parse config file")?;
            Ok(config)
        } else {
            // Create default config
            let config = Config::default();
            config.save()?;
            Ok(config)
        }
    }

    /// Save configuration to file
    pub fn save(&self) -> Result<()> {
        let config_path = Self::config_path()?;
        let config_dir = config_path.parent().unwrap();

        // Create directory if it doesn't exist
        fs::create_dir_all(config_dir).context("Failed to create config directory")?;

        let contents = toml::to_string_pretty(self).context("Failed to serialize config")?;

        fs::write(&config_path, contents).context("Failed to write config file")?;

        Ok(())
    }

    /// Get the configuration file path
    pub fn config_path() -> Result<PathBuf> {
        let home = home_dir().context("Failed to get home directory")?;
        Ok(home.join(".supernova").join("cli").join("config.toml"))
    }

    /// Get the wallet directory path
    pub fn wallet_dir() -> Result<PathBuf> {
        let home = home_dir().context("Failed to get home directory")?;
        Ok(home.join(".supernova").join("wallets"))
    }
}
