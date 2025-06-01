// Deployment module for Supernova blockchain
// Manages testnet and mainnet deployment configurations

pub mod testnet_config;

// Re-export deployment types
pub use testnet_config::{
    TestnetConfiguration, TestnetDeploymentManager,
    deploy_supernova_testnet, TestnetDeploymentStatus,
    NetworkType, DeploymentError,
}; 