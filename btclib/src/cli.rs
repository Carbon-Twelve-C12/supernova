// CLI module - functionality moved to separate cli crate
// This module is kept for backwards compatibility

use crate::errors::supernovaError;

pub struct CliConfig {
    pub rpc_url: String,
    pub network: String,
}

impl Default for CliConfig {
    fn default() -> Self {
        Self {
            rpc_url: "http://localhost:8332".to_string(),
            network: "mainnet".to_string(),
        }
    }
} 