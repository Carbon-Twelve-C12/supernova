pub mod coordinator;
pub mod environmental_verification;
pub mod fraud_detection;
pub mod reward;
pub mod template;
pub mod worker;

#[cfg(test)]
mod environmental_security_tests;
#[cfg(test)]
mod halving_test;
#[cfg(test)]
mod security_tests;
#[cfg(test)]
mod testnet_integration_tests;

pub use coordinator::Miner;
pub use environmental_verification::{EfficiencyAudit, EnvironmentalVerifier, RECCertificate};
pub use reward::{
    calculate_base_reward, calculate_mining_reward, EnvironmentalProfile, MiningReward,
};
pub use template::{BlockTemplate, MempoolInterface};
pub use worker::MiningWorker;

// Note: with the current schedule (50 NOVA initial reward, halving every 840,000 blocks),
// total issuance over all halvings converges to ~84,000,000 NOVA (geometric series).
pub const NOVA_TOTAL_SUPPLY: u64 = 84_000_000;
pub const NOVA_BLOCK_REWARD: u64 = 50; // Initial block reward in NOVA
pub const HALVING_INTERVAL: u64 = 840_000; // Halving every 840,000 blocks (~4 years)
pub const MAX_HALVINGS: u32 = 64; // Maximum number of halvings (same as Bitcoin)

// Environmental bonus constants
pub const ENV_BONUS_RENEWABLE: f64 = 0.20; // 20% bonus for verified renewable energy
pub const ENV_BONUS_EFFICIENCY: f64 = 0.10; // 10% bonus for exceptional efficiency
pub const ENV_BONUS_MAX_TOTAL: f64 = 0.75; // Maximum 75% total bonus
