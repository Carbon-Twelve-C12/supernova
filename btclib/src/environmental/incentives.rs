use crate::environmental::types::{MinerEnvironmentalInfo, VerificationStatus};
use crate::environmental::emissions::{EmissionsTracker, EmissionsError};
use crate::environmental::treasury::{EnvironmentalTreasury, TreasuryError, TreasuryAccountType};
use crate::types::transaction::Transaction;
use crate::types::block::Block;
use serde::{Serialize, Deserialize};
use thiserror::Error;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use chrono::{DateTime, Utc};

/// Error types for incentive operations
#[derive(Error, Debug)]
pub enum IncentiveError {
    #[error("Invalid miner information: {0}")]
    InvalidMinerInfo(String),
    
    #[error("Verification error: {0}")]
    VerificationError(String),
    
    #[error("Treasury error: {0}")]
    TreasuryError(#[from] TreasuryError),
    
    #[error("Emissions calculation error: {0}")]
    EmissionsError(#[from] EmissionsError),
    
    #[error("Configuration error: {0}")]
    ConfigError(String),
}

/// Result type for incentive operations
pub type IncentiveResult<T> = Result<T, IncentiveError>;

/// Green mining incentive tier
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum IncentiveTier {
    /// Standard tier (no special incentives)
    Standard,
    /// Bronze tier (low renewable percentage)
    Bronze,
    /// Silver tier (medium renewable percentage)
    Silver,
    /// Gold tier (high renewable percentage)
    Gold,
    /// Platinum tier (very high renewable percentage)
    Platinum,
}

/// Configuration for green mining incentives
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IncentiveConfig {
    /// Whether green mining incentives are enabled
    pub enabled: bool,
    
    /// Minimum renewable percentage for Bronze tier
    pub bronze_threshold: f64,
    
    /// Minimum renewable percentage for Silver tier
    pub silver_threshold: f64,
    
    /// Minimum renewable percentage for Gold tier
    pub gold_threshold: f64,
    
    /// Minimum renewable percentage for Platinum tier
    pub platinum_threshold: f64,
    
    /// Fee discount percentage for Bronze tier
    pub bronze_fee_discount: f64,
    
    /// Fee discount percentage for Silver tier
    pub silver_fee_discount: f64,
    
    /// Fee discount percentage for Gold tier
    pub gold_fee_discount: f64,
    
    /// Fee discount percentage for Platinum tier
    pub platinum_fee_discount: f64,
    
    /// Reward multiplier for Bronze tier (as percentage above base reward)
    pub bronze_reward_multiplier: f64,
    
    /// Reward multiplier for Silver tier
    pub silver_reward_multiplier: f64,
    
    /// Reward multiplier for Gold tier
    pub gold_reward_multiplier: f64,
    
    /// Reward multiplier for Platinum tier
    pub platinum_reward_multiplier: f64,
    
    /// Percentage of additional rewards funded by treasury
    pub treasury_funding_percentage: f64,
    
    /// Whether verification is required for incentives
    pub require_verification: bool,
    
    /// Maximum total additional rewards per block (as percentage of base reward)
    pub max_additional_rewards_percentage: f64,
}

impl Default for IncentiveConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            bronze_threshold: 30.0,
            silver_threshold: 50.0,
            gold_threshold: 75.0,
            platinum_threshold: 95.0,
            bronze_fee_discount: 5.0,
            silver_fee_discount: 10.0,
            gold_fee_discount: 15.0,
            platinum_fee_discount: 20.0,
            bronze_reward_multiplier: 1.0,
            silver_reward_multiplier: 2.0,
            gold_reward_multiplier: 3.0,
            platinum_reward_multiplier: 5.0,
            treasury_funding_percentage: 50.0,
            require_verification: true,
            max_additional_rewards_percentage: 10.0,
        }
    }
}

/// Miner's incentive status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MinerIncentiveStatus {
    /// Miner's ID
    pub miner_id: String,
    
    /// Incentive tier
    pub tier: IncentiveTier,
    
    /// Renewable energy percentage
    pub renewable_percentage: f64,
    
    /// Verification status
    pub verification_status: VerificationStatus,
    
    /// Fee discount percentage
    pub fee_discount: f64,
    
    /// Reward multiplier
    pub reward_multiplier: f64,
    
    /// Last updated timestamp
    pub last_updated: DateTime<Utc>,
}

/// Green mining incentive manager
pub struct GreenIncentiveManager {
    /// Configuration
    config: IncentiveConfig,
    
    /// Miner incentive status by ID
    miner_status: Arc<RwLock<HashMap<String, MinerIncentiveStatus>>>,
    
    /// Emissions tracker reference
    emissions_tracker: Arc<EmissionsTracker>,
    
    /// Treasury reference
    treasury: Arc<EnvironmentalTreasury>,
}

impl GreenIncentiveManager {
    /// Create a new incentive manager
    pub fn new(
        config: IncentiveConfig,
        emissions_tracker: Arc<EmissionsTracker>,
        treasury: Arc<EnvironmentalTreasury>,
    ) -> Self {
        Self {
            config,
            miner_status: Arc::new(RwLock::new(HashMap::new())),
            emissions_tracker,
            treasury,
        }
    }
    
    /// Update the incentive configuration
    pub fn update_config(&mut self, config: IncentiveConfig) {
        self.config = config;
    }
    
    /// Register or update a miner's environmental information
    pub async fn register_miner(&self, miner_id: &str, info: &MinerEnvironmentalInfo) -> IncentiveResult<MinerIncentiveStatus> {
        if !self.config.enabled {
            return Err(IncentiveError::ConfigError("Green incentives are not enabled".to_string()));
        }
        
        // Calculate the tier based on renewable percentage
        let tier = self.calculate_tier(info.renewable_percentage);
        
        // Calculate fee discount and reward multiplier based on tier
        let (fee_discount, reward_multiplier) = self.calculate_incentives(tier);
        
        // Check verification if required
        if self.config.require_verification && 
           info.verification_status != VerificationStatus::Verified &&
           tier != IncentiveTier::Standard {
            return Err(IncentiveError::VerificationError(
                "Verification required for incentives".to_string()
            ));
        }
        
        // Create status
        let status = MinerIncentiveStatus {
            miner_id: miner_id.to_string(),
            tier,
            renewable_percentage: info.renewable_percentage,
            verification_status: info.verification_status,
            fee_discount,
            reward_multiplier,
            last_updated: Utc::now(),
        };
        
        // Store status
        let mut miner_status = self.miner_status.write().await;
        miner_status.insert(miner_id.to_string(), status.clone());
        
        Ok(status)
    }
    
    /// Calculate the appropriate tier for a given renewable percentage
    fn calculate_tier(&self, renewable_percentage: f64) -> IncentiveTier {
        if renewable_percentage >= self.config.platinum_threshold {
            IncentiveTier::Platinum
        } else if renewable_percentage >= self.config.gold_threshold {
            IncentiveTier::Gold
        } else if renewable_percentage >= self.config.silver_threshold {
            IncentiveTier::Silver
        } else if renewable_percentage >= self.config.bronze_threshold {
            IncentiveTier::Bronze
        } else {
            IncentiveTier::Standard
        }
    }
    
    /// Calculate fee discount and reward multiplier based on tier
    fn calculate_incentives(&self, tier: IncentiveTier) -> (f64, f64) {
        match tier {
            IncentiveTier::Platinum => (
                self.config.platinum_fee_discount,
                self.config.platinum_reward_multiplier,
            ),
            IncentiveTier::Gold => (
                self.config.gold_fee_discount,
                self.config.gold_reward_multiplier,
            ),
            IncentiveTier::Silver => (
                self.config.silver_fee_discount,
                self.config.silver_reward_multiplier,
            ),
            IncentiveTier::Bronze => (
                self.config.bronze_fee_discount,
                self.config.bronze_reward_multiplier,
            ),
            IncentiveTier::Standard => (0.0, 0.0),
        }
    }
    
    /// Get a miner's incentive status
    pub async fn get_miner_status(&self, miner_id: &str) -> Option<MinerIncentiveStatus> {
        let miner_status = self.miner_status.read().await;
        miner_status.get(miner_id).cloned()
    }
    
    /// Calculate additional reward for a green miner
    pub async fn calculate_additional_reward(&self, miner_id: &str, base_reward: u64) -> IncentiveResult<u64> {
        if !self.config.enabled {
            return Ok(0);
        }
        
        // Get miner status
        let status = match self.get_miner_status(miner_id).await {
            Some(status) => status,
            None => return Ok(0), // No additional reward for unregistered miners
        };
        
        // Calculate additional reward
        let additional_percentage = status.reward_multiplier;
        if additional_percentage <= 0.0 {
            return Ok(0);
        }
        
        // Calculate additional reward amount
        let additional_reward = (base_reward as f64 * additional_percentage / 100.0) as u64;
        
        // Apply maximum cap
        let max_additional = (base_reward as f64 * self.config.max_additional_rewards_percentage / 100.0) as u64;
        let capped_reward = additional_reward.min(max_additional);
        
        // Determine how much comes from the treasury
        let treasury_contribution = (capped_reward as f64 * self.config.treasury_funding_percentage / 100.0) as u64;
        
        // Transfer from treasury if needed
        if treasury_contribution > 0 {
            self.treasury.transfer(
                TreasuryAccountType::GreenIncentives,
                TreasuryAccountType::RewardPool,
                treasury_contribution as f64
            ).await.map_err(IncentiveError::TreasuryError)?;
        }
        
        Ok(capped_reward)
    }
    
    /// Calculate fee discount for a transaction based on miner status
    pub async fn calculate_fee_discount(&self, miner_id: &str, base_fee: u64) -> IncentiveResult<u64> {
        if !self.config.enabled {
            return Ok(0);
        }
        
        // Get miner status
        let status = match self.get_miner_status(miner_id).await {
            Some(status) => status,
            None => return Ok(0), // No discount for unregistered miners
        };
        
        // Calculate discount amount
        let discount = (base_fee as f64 * status.fee_discount / 100.0) as u64;
        
        Ok(discount)
    }
    
    /// Process a block mined by a green miner
    pub async fn process_green_block(&self, block: &Block, miner_id: &str, base_reward: u64) -> IncentiveResult<u64> {
        if !self.config.enabled {
            return Ok(0);
        }
        
        // Calculate additional reward
        let additional_reward = self.calculate_additional_reward(miner_id, base_reward).await?;
        
        // Record environmental impact
        if additional_reward > 0 {
            // This would integrate with emissions tracking to record the positive impact
            // of incentivizing green mining
        }
        
        Ok(additional_reward)
    }
    
    /// Get all miner incentive statuses
    pub async fn get_all_miner_statuses(&self) -> HashMap<String, MinerIncentiveStatus> {
        self.miner_status.read().await.clone()
    }
    
    /// Get miners by tier
    pub async fn get_miners_by_tier(&self, tier: IncentiveTier) -> Vec<MinerIncentiveStatus> {
        let miner_status = self.miner_status.read().await;
        
        miner_status.values()
            .filter(|status| status.tier == tier)
            .cloned()
            .collect()
    }
    
    /// Get tier distribution statistics
    pub async fn get_tier_distribution(&self) -> HashMap<IncentiveTier, usize> {
        let miner_status = self.miner_status.read().await;
        let mut distribution = HashMap::new();
        
        // Initialize all tiers with 0
        distribution.insert(IncentiveTier::Standard, 0);
        distribution.insert(IncentiveTier::Bronze, 0);
        distribution.insert(IncentiveTier::Silver, 0);
        distribution.insert(IncentiveTier::Gold, 0);
        distribution.insert(IncentiveTier::Platinum, 0);
        
        // Count miners in each tier
        for status in miner_status.values() {
            *distribution.entry(status.tier).or_insert(0) += 1;
        }
        
        distribution
    }
    
    /// Get total fees discounted
    pub async fn get_total_discounted_fees(&self) -> f64 {
        // In a real implementation, this would track and return actual discounted fees
        0.0
    }
    
    /// Get total additional rewards paid
    pub async fn get_total_additional_rewards(&self) -> f64 {
        // In a real implementation, this would track and return actual additional rewards
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    // Helper to create a test miner info
    fn create_test_miner_info(renewable_percentage: f64, verified: bool) -> MinerEnvironmentalInfo {
        MinerEnvironmentalInfo {
            renewable_percentage,
            verification_status: if verified {
                VerificationStatus::Verified
            } else {
                VerificationStatus::Pending
            },
            // Add other required fields based on your MinerEnvironmentalInfo structure
            region: "us-west".to_string(),
            energy_sources: HashMap::new(),
            hardware_types: vec![],
            carbon_offset_tokens: 0,
            rec_certificates: None,
            total_hashrate_th: 0.0,
            energy_consumption_kwh: 0.0,
        }
    }
    
    #[tokio::test]
    async fn test_tier_calculation() {
        // Test would go here
    }
    
    #[tokio::test]
    async fn test_incentive_calculation() {
        // Test would go here
    }
    
    #[tokio::test]
    async fn test_verification_requirement() {
        // Test would go here
    }
} 