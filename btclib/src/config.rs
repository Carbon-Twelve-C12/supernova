use serde::{Deserialize, Serialize};
use crate::crypto::quantum::{QuantumScheme, ClassicalScheme};
use crate::crypto::zkp::ZkpType;

/// Configuration for advanced cryptographic features
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CryptoConfig {
    /// Configuration for quantum-resistant cryptography
    pub quantum: QuantumConfig,
    
    /// Configuration for zero-knowledge proofs
    pub zkp: ZkpConfig,
}

/// Configuration for quantum-resistant cryptography
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuantumConfig {
    /// Whether quantum-resistant signatures are enabled
    pub enabled: bool,
    
    /// The default quantum scheme to use
    pub default_scheme: QuantumScheme,
    
    /// The default security level (1-5)
    pub security_level: u8,
    
    /// Whether to allow hybrid schemes
    pub allow_hybrid: bool,
    
    /// For hybrid schemes, which classical scheme to use
    pub classical_scheme: ClassicalScheme,
}

/// Configuration for zero-knowledge proofs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZkpConfig {
    /// Whether confidential transactions are enabled
    pub enabled: bool,
    
    /// The default ZKP scheme to use
    pub default_scheme: ZkpType,
    
    /// The default security level
    pub security_level: u8,
    
    /// Maximum number of range proofs per transaction
    pub max_range_proofs: usize,
}

impl Default for CryptoConfig {
    fn default() -> Self {
        Self {
            quantum: QuantumConfig::default(),
            zkp: ZkpConfig::default(),
        }
    }
}

impl Default for QuantumConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            default_scheme: QuantumScheme::Dilithium,
            security_level: 3,
            allow_hybrid: true,
            classical_scheme: ClassicalScheme::Secp256k1,
        }
    }
}

impl Default for ZkpConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            default_scheme: ZkpType::Bulletproof,
            security_level: 128,
            max_range_proofs: 100,
        }
    }
}

/// Main blockchain configuration including cryptographic settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Network to connect to (mainnet, testnet, etc.)
    pub network: NetworkType,
    
    /// Cryptographic feature configuration
    pub crypto: CryptoConfig,
    
    /// Maximum transaction size in bytes
    pub max_tx_size: usize,
    
    /// Maximum block size in bytes
    pub max_block_size: usize,
    
    /// Maximum number of transactions per block
    pub max_tx_per_block: usize,
}

/// Network type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NetworkType {
    /// Main network
    Mainnet,
    
    /// Test network
    Testnet,
    
    /// Regression test network
    Regtest,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            network: NetworkType::Testnet,
            crypto: CryptoConfig::default(),
            max_tx_size: 1_000_000, // 1 MB
            max_block_size: 4_000_000, // 4 MB
            max_tx_per_block: 10_000,
        }
    }
}

impl Config {
    /// Load configuration from a TOML file
    pub fn load_from_file(path: &str) -> Result<Self, std::io::Error> {
        let config_str = std::fs::read_to_string(path)?;
        let config = toml::from_str(&config_str)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        Ok(config)
    }
    
    /// Save configuration to a TOML file
    pub fn save_to_file(&self, path: &str) -> Result<(), std::io::Error> {
        let config_str = toml::to_string_pretty(self)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        std::fs::write(path, config_str)
    }
    
    /// Create a testnet configuration
    pub fn testnet() -> Self {
        let mut config = Self::default();
        config.network = NetworkType::Testnet;
        
        // Enable quantum signatures and confidential transactions for testnet
        config.crypto.quantum.enabled = true;
        config.crypto.zkp.enabled = true;
        
        config
    }
    
    /// Create a regtest configuration
    pub fn regtest() -> Self {
        let mut config = Self::default();
        config.network = NetworkType::Regtest;
        
        // Enable all features for regtest
        config.crypto.quantum.enabled = true;
        config.crypto.quantum.allow_hybrid = true;
        
        config.crypto.zkp.enabled = true;
        config.crypto.zkp.max_range_proofs = 1000; // More permissive for testing
        
        config
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_default_config() {
        let config = Config::default();
        
        // Check default values
        assert_eq!(config.network, NetworkType::Testnet);
        assert!(!config.crypto.quantum.enabled);
        assert!(!config.crypto.zkp.enabled);
        
        assert_eq!(config.crypto.quantum.default_scheme, QuantumScheme::Dilithium);
        assert_eq!(config.crypto.quantum.security_level, 3);
        assert_eq!(config.crypto.zkp.default_scheme, ZkpType::Bulletproof);
    }
    
    #[test]
    fn test_testnet_config() {
        let config = Config::testnet();
        
        assert_eq!(config.network, NetworkType::Testnet);
        assert!(config.crypto.quantum.enabled);
        assert!(config.crypto.zkp.enabled);
    }
    
    #[test]
    fn test_regtest_config() {
        let config = Config::regtest();
        
        assert_eq!(config.network, NetworkType::Regtest);
        assert!(config.crypto.quantum.enabled);
        assert!(config.crypto.zkp.enabled);
        assert_eq!(config.crypto.zkp.max_range_proofs, 1000);
    }
    
    #[test]
    fn test_serialization() {
        let config = Config::testnet();
        
        // Serialize to TOML
        let toml_str = toml::to_string_pretty(&config).unwrap();
        
        // Deserialize from TOML
        let deserialized: Config = toml::from_str(&toml_str).unwrap();
        
        // Check that values match
        assert_eq!(deserialized.network, config.network);
        assert_eq!(deserialized.crypto.quantum.enabled, config.crypto.quantum.enabled);
        assert_eq!(deserialized.crypto.zkp.enabled, config.crypto.zkp.enabled);
    }
} 