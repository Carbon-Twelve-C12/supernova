use std::collections::HashMap;
use thiserror::Error;
use serde::{Serialize, Deserialize};
use chrono::{DateTime, Utc};
use crate::types::transaction::Transaction;

/// Error types for environmental treasury operations
#[derive(Error, Debug)]
pub enum TreasuryError {
    #[error("Insufficient funds: requested {requested}, available {available}")]
    InsufficientFunds { requested: u64, available: u64 },
    
    #[error("Invalid transaction: {0}")]
    InvalidTransaction(String),
    
    #[error("Authorization error: {0}")]
    AuthorizationError(String),
    
    #[error("Governance error: {0}")]
    GovernanceError(String),
    
    #[error("Asset verification error: {0}")]
    AssetVerificationError(String),
}

/// Type of environmental asset
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EnvironmentalAssetType {
    /// Renewable energy certificates (prioritized)
    RenewableEnergyCertificate,
    /// Carbon offset credits (secondary)
    CarbonOffset,
    /// Energy efficiency credits
    EnergyEfficiency,
    /// Other environmental assets
    Other,
}

/// Environmental asset purchase record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvironmentalAssetPurchase {
    /// Unique purchase identifier
    pub id: String,
    /// Type of asset purchased
    pub asset_type: EnvironmentalAssetType,
    /// Amount of asset in tonnes CO2e or MWh
    pub amount: f64,
    /// Cost in blockchain's native currency
    pub cost: u64,
    /// Time of purchase
    pub timestamp: DateTime<Utc>,
    /// Verification information
    pub verification: Option<VerificationInfo>,
    /// Environmental impact score (higher is better)
    pub impact_score: f64,
}

/// Verification information for environmental assets
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationInfo {
    /// Verification provider
    pub provider: String,
    /// Verification date
    pub date: DateTime<Utc>,
    /// Reference code/ID
    pub reference: String,
    /// Verification status
    pub status: VerificationStatus,
}

/// Verification status for environmental assets
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum VerificationStatus {
    /// Pending verification
    Pending,
    /// Successfully verified
    Verified,
    /// Verification failed
    Failed,
    /// Verification expired
    Expired,
}

/// Environmental treasury proposal types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GovernanceProposal {
    /// Change allocation percentage
    AllocationPercentage {
        /// Proposed new percentage
        new_percentage: f64,
        /// Proposer's address
        proposer: String,
        /// Proposal expiration date
        expiration: DateTime<Utc>,
    },
    /// Change authorized signers
    AuthorizedSigners {
        /// Addresses to add as signers
        add: Vec<String>,
        /// Addresses to remove as signers
        remove: Vec<String>,
        /// Proposer's address
        proposer: String,
        /// Proposal expiration date
        expiration: DateTime<Utc>,
    },
    /// Purchase environmental assets
    PurchaseAssets {
        /// Type of assets to purchase
        asset_type: EnvironmentalAssetType,
        /// Amount to purchase
        amount: f64,
        /// Maximum cost
        max_cost: u64,
        /// Proposer's address
        proposer: String,
        /// Proposal expiration date
        expiration: DateTime<Utc>,
    },
}

/// Environmental treasury account for tracking funds
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TreasuryAccount {
    /// Current balance
    pub balance: u64,
    /// Total collected since inception
    pub total_collected: u64,
    /// Total spent on environmental assets
    pub total_spent: u64,
}

/// Environmentally friendly miner registration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GreenMinerRegistration {
    /// Miner identifier
    pub miner_id: String,
    /// Renewable energy percentage (0-100)
    pub renewable_percentage: f64,
    /// Registration timestamp
    pub registration_date: DateTime<Utc>,
    /// Verification information
    pub verification: Option<VerificationInfo>,
}

/// Structure representing the environmental treasury system
pub struct EnvironmentalTreasury {
    /// Treasury account
    account: TreasuryAccount,
    /// Current allocation percentage from transaction fees
    allocation_percentage: f64,
    /// Authorized signers (multi-sig governance)
    authorized_signers: Vec<String>,
    /// Required signatures for operations
    required_signatures: usize,
    /// Environmental asset purchases
    asset_purchases: Vec<EnvironmentalAssetPurchase>,
    /// Active governance proposals
    active_proposals: Vec<GovernanceProposal>,
    /// Green miner registrations
    green_miners: HashMap<String, GreenMinerRegistration>,
}

impl EnvironmentalTreasury {
    /// Create a new environmental treasury
    pub fn new(initial_allocation_percentage: f64, authorized_signers: Vec<String>, required_signatures: usize) -> Self {
        Self {
            account: TreasuryAccount {
                balance: 0,
                total_collected: 0,
                total_spent: 0,
            },
            allocation_percentage: initial_allocation_percentage,
            authorized_signers,
            required_signatures,
            asset_purchases: Vec::new(),
            active_proposals: Vec::new(),
            green_miners: HashMap::new(),
        }
    }
    
    /// Get current treasury balance
    pub fn balance(&self) -> u64 {
        self.account.balance
    }
    
    /// Get current allocation percentage
    pub fn allocation_percentage(&self) -> f64 {
        self.allocation_percentage
    }
    
    /// Calculate the environmental fee allocation from a transaction's fees
    pub fn calculate_allocation(&self, transaction_fee: u64) -> u64 {
        (transaction_fee as f64 * self.allocation_percentage / 100.0) as u64
    }
    
    /// Process a block's transaction fees, allocating the environmental portion
    pub fn process_block_allocation(&mut self, total_fees: u64) -> u64 {
        let allocation = self.calculate_allocation(total_fees);
        self.account.balance += allocation;
        self.account.total_collected += allocation;
        allocation
    }
    
    /// Create a proposal to adjust the allocation percentage
    pub fn create_allocation_proposal(&mut self, 
                                   new_percentage: f64,
                                   proposer: String,
                                   expiration: DateTime<Utc>) -> Result<(), TreasuryError> {
        // Basic validation
        if new_percentage < 0.0 || new_percentage > 20.0 {
            return Err(TreasuryError::GovernanceError(
                "Allocation percentage must be between 0% and 20%".to_string()
            ));
        }
        
        if !self.authorized_signers.contains(&proposer) {
            return Err(TreasuryError::AuthorizationError(
                "Proposer is not an authorized signer".to_string()
            ));
        }
        
        // Create the proposal
        self.active_proposals.push(GovernanceProposal::AllocationPercentage {
            new_percentage,
            proposer,
            expiration,
        });
        
        Ok(())
    }
    
    /// Register a green miner
    pub fn register_green_miner(&mut self, 
                             miner_id: String, 
                             renewable_percentage: f64,
                             verification: Option<VerificationInfo>) -> Result<(), TreasuryError> {
        // Basic validation
        if renewable_percentage < 0.0 || renewable_percentage > 100.0 {
            return Err(TreasuryError::AssetVerificationError(
                "Renewable percentage must be between 0% and 100%".to_string()
            ));
        }
        
        // Register the miner
        self.green_miners.insert(miner_id.clone(), GreenMinerRegistration {
            miner_id,
            renewable_percentage,
            registration_date: Utc::now(),
            verification,
        });
        
        Ok(())
    }
    
    /// Calculate fee discount for green miners based on renewable percentage
    pub fn calculate_miner_fee_discount(&self, miner_id: &str) -> f64 {
        match self.green_miners.get(miner_id) {
            Some(registration) => {
                // Apply tiered discount based on renewable percentage
                // This is a simplified implementation for Phase 1
                if registration.renewable_percentage >= 95.0 {
                    10.0 // 10% discount for >=95% renewable
                } else if registration.renewable_percentage >= 75.0 {
                    7.0 // 7% discount for >=75% renewable
                } else if registration.renewable_percentage >= 50.0 {
                    5.0 // 5% discount for >=50% renewable
                } else if registration.renewable_percentage >= 25.0 {
                    2.0 // 2% discount for >=25% renewable
                } else {
                    0.0 // No discount
                }
            },
            None => 0.0, // No registration, no discount
        }
    }

    /// Purchase environmental assets with prioritization for RECs
    pub fn purchase_prioritized_assets(&mut self, 
                                   available_funds: u64,
                                   rec_allocation_percentage: f64) -> Result<Vec<EnvironmentalAssetPurchase>, TreasuryError> {
        if available_funds == 0 || self.account.balance < available_funds {
            return Err(TreasuryError::InsufficientFunds { 
                requested: available_funds, 
                available: self.account.balance 
            });
        }
        
        // Calculate amount to spend on each asset type
        let rec_funds = (available_funds as f64 * rec_allocation_percentage / 100.0) as u64;
        let carbon_funds = available_funds - rec_funds;
        
        let mut purchases = Vec::new();
        
        // Purchase RECs first (prioritized)
        if rec_funds > 0 {
            let rec_purchase = self.execute_asset_purchase(
                EnvironmentalAssetType::RenewableEnergyCertificate,
                rec_funds,
                1.0, // Base impact score
            )?;
            
            purchases.push(rec_purchase);
        }
        
        // Purchase carbon offsets with remaining funds
        if carbon_funds > 0 {
            let carbon_purchase = self.execute_asset_purchase(
                EnvironmentalAssetType::CarbonOffset,
                carbon_funds,
                0.5, // Lower impact score than RECs
            )?;
            
            purchases.push(carbon_purchase);
        }
        
        // Update treasury balance
        self.account.balance -= available_funds;
        self.account.total_spent += available_funds;
        
        // Add purchases to history
        for purchase in &purchases {
            self.asset_purchases.push(purchase.clone());
        }
        
        Ok(purchases)
    }

    /// Execute a single asset purchase
    fn execute_asset_purchase(&self, 
                            asset_type: EnvironmentalAssetType,
                            funds: u64,
                            base_impact_score: f64) -> Result<EnvironmentalAssetPurchase, TreasuryError> {
        // In a real implementation, this would interact with external markets
        // to purchase actual environmental assets
        
        // For now, use a simplified model:
        // - RECs: Assume 1 MWh costs approximately 5000 satoshis
        // - Carbon offsets: Assume 1 tonne CO2e costs approximately 8000 satoshis
        
        let (amount, impact_score) = match asset_type {
            EnvironmentalAssetType::RenewableEnergyCertificate => {
                let mwh = funds as f64 / 5000.0;
                // RECs have higher impact score because they directly support renewable energy
                (mwh, base_impact_score * 1.0)
            },
            EnvironmentalAssetType::CarbonOffset => {
                let tonnes = funds as f64 / 8000.0;
                (tonnes, base_impact_score * 1.0)
            },
            EnvironmentalAssetType::EnergyEfficiency => {
                let units = funds as f64 / 10000.0;
                (units, base_impact_score * 0.8)
            },
            EnvironmentalAssetType::Other => {
                let units = funds as f64 / 12000.0;
                (units, base_impact_score * 0.7)
            },
        };
        
        // Create the purchase record
        let purchase = EnvironmentalAssetPurchase {
            id: format!("ASSET-{}-{}", 
                       asset_type.to_string(), 
                       Utc::now().timestamp()),
            asset_type,
            amount,
            cost: funds,
            timestamp: Utc::now(),
            verification: Some(VerificationInfo {
                provider: "SuperNova Treasury".to_string(),
                date: Utc::now(),
                reference: format!("TR-{}", Utc::now().timestamp()),
                status: VerificationStatus::Verified,
            }),
            impact_score,
        };
        
        Ok(purchase)
    }
}

impl EnvironmentalAssetType {
    fn to_string(&self) -> String {
        match self {
            Self::RenewableEnergyCertificate => "REC".to_string(),
            Self::CarbonOffset => "CARBON".to_string(),
            Self::EnergyEfficiency => "EFFICIENCY".to_string(),
            Self::Other => "OTHER".to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_allocation_calculation() {
        let treasury = EnvironmentalTreasury::new(
            2.0, // 2% allocation
            vec!["signer1".to_string(), "signer2".to_string()],
            1,
        );
        
        let fee = 1000; // 1000 satoshis/wei/etc.
        let allocation = treasury.calculate_allocation(fee);
        
        // 2% of 1000 = 20
        assert_eq!(allocation, 20);
        
        // Test with fractional result
        let fee = 33;
        let allocation = treasury.calculate_allocation(fee);
        
        // 2% of 33 = 0.66, which should round down to 0 as u64
        assert_eq!(allocation, 0);
        
        // Test with larger values
        let fee = 1_000_000;
        let allocation = treasury.calculate_allocation(fee);
        
        // 2% of 1,000,000 = 20,000
        assert_eq!(allocation, 20_000);
    }
    
    #[test]
    fn test_green_miner_discount() {
        let mut treasury = EnvironmentalTreasury::new(
            2.0,
            vec!["signer1".to_string()],
            1,
        );
        
        // Register miners with different renewable percentages
        treasury.register_green_miner(
            "miner1".to_string(),
            100.0, // 100% renewable
            None,
        ).unwrap();
        
        treasury.register_green_miner(
            "miner2".to_string(),
            60.0, // 60% renewable
            None,
        ).unwrap();
        
        treasury.register_green_miner(
            "miner3".to_string(),
            30.0, // 30% renewable
            None,
        ).unwrap();
        
        treasury.register_green_miner(
            "miner4".to_string(),
            10.0, // 10% renewable
            None,
        ).unwrap();
        
        // Test discounts
        assert_eq!(treasury.calculate_miner_fee_discount("miner1"), 10.0); // 10% discount
        assert_eq!(treasury.calculate_miner_fee_discount("miner2"), 5.0);  // 5% discount
        assert_eq!(treasury.calculate_miner_fee_discount("miner3"), 2.0);  // 2% discount
        assert_eq!(treasury.calculate_miner_fee_discount("miner4"), 0.0);  // No discount
        assert_eq!(treasury.calculate_miner_fee_discount("nonexistent"), 0.0); // Nonexistent miner
    }
} 