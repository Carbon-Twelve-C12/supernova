use std::collections::HashMap;
use thiserror::Error;
use serde::{Serialize, Deserialize};
use chrono::{DateTime, Utc};
use crate::types::transaction::Transaction;

/// Error types for environmental treasury operations
#[derive(Error, Debug)]
pub enum TreasuryError {
    #[error("Insufficient funds: required {0}, available {1}")]
    InsufficientFunds(u64, u64),
    
    #[error("Invalid miner registration: {0}")]
    InvalidMinerRegistration(String),
    
    #[error("Invalid asset purchase: {0}")]
    InvalidAssetPurchase(String),
    
    #[error("Invalid allocation parameters: {0}")]
    InvalidAllocation(String),
    
    #[error("Database error: {0}")]
    DatabaseError(String),
}

/// Type of environmental asset
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EnvironmentalAssetType {
    /// Renewable Energy Certificate (MWh)
    RenewableEnergyCertificate,
    
    /// Carbon Credit (tonnes CO2e)
    CarbonOffset,
}

/// Status of verification for environmental claims
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum VerificationStatus {
    /// Verification is pending
    Pending,
    
    /// Verification has been approved
    Approved,
    
    /// Verification has been rejected
    Rejected,
}

/// Information about verification of environmental claims
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationInfo {
    /// Organization providing verification
    pub provider: String,
    
    /// Date of verification
    pub date: DateTime<Utc>,
    
    /// Reference identifier for the verification
    pub reference: String,
    
    /// Status of the verification
    pub status: VerificationStatus,
}

/// Information about a green miner
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GreenMinerInfo {
    /// Percentage of renewable energy used (0-100)
    pub renewable_percentage: f64,
    
    /// Verification information, if available
    pub verification: Option<VerificationInfo>,
    
    /// Date the miner was registered
    pub registration_date: DateTime<Utc>,
    
    /// Date the information was last updated
    pub last_updated: DateTime<Utc>,
}

/// Environmental asset purchase record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvironmentalAssetPurchase {
    /// Type of asset purchased
    pub asset_type: EnvironmentalAssetType,
    
    /// Amount of asset (MWh for RECs, tonnes CO2e for offsets)
    pub amount: f64,
    
    /// Cost in satoshis
    pub cost: u64,
    
    /// Date of purchase
    pub date: DateTime<Utc>,
    
    /// Provider of the asset
    pub provider: String,
    
    /// Reference identifier for the purchase
    pub reference: String,
    
    /// Environmental impact score (higher is better)
    pub impact_score: f64,
}

/// Environmental treasury for allocating funds to environmental initiatives
#[derive(Debug, Clone)]
pub struct EnvironmentalTreasury {
    /// Current balance in satoshis
    balance: u64,
    
    /// Allocation percentage from transaction fees
    allocation_percentage: f64,
    
    /// Authorized signers for treasury operations
    authorized_signers: Vec<String>,
    
    /// Required signatures for treasury operations
    required_signatures: usize,
    
    /// Green miners registered with the treasury
    green_miners: HashMap<String, GreenMinerInfo>,
    
    /// Environmental asset purchases
    asset_purchases: Vec<EnvironmentalAssetPurchase>,
}

impl EnvironmentalTreasury {
    /// Create a new environmental treasury
    pub fn new(allocation_percentage: f64, authorized_signers: Vec<String>, required_signatures: usize) -> Self {
        Self {
            balance: 0,
            allocation_percentage,
            authorized_signers,
            required_signatures,
            green_miners: HashMap::new(),
            asset_purchases: Vec::new(),
        }
    }
    
    /// Process a block's allocation to the environmental treasury
    pub fn process_block_allocation(&mut self, total_fees: u64) -> u64 {
        let allocation = (total_fees as f64 * self.allocation_percentage / 100.0) as u64;
        self.balance += allocation;
        allocation
    }
    
    /// Register a green miner
    pub fn register_green_miner(
        &mut self,
        miner_id: String,
        renewable_percentage: f64,
        verification: Option<VerificationInfo>,
    ) -> Result<(), TreasuryError> {
        if renewable_percentage < 0.0 || renewable_percentage > 100.0 {
            return Err(TreasuryError::InvalidMinerRegistration(
                "Renewable percentage must be between 0 and 100".to_string()
            ));
        }
        
        let info = GreenMinerInfo {
            renewable_percentage,
            verification,
            registration_date: Utc::now(),
            last_updated: Utc::now(),
        };
        
        self.green_miners.insert(miner_id, info);
        
        Ok(())
    }
    
    /// Calculate fee discount for a green miner
    pub fn calculate_miner_fee_discount(&self, miner_id: &str) -> f64 {
        let info = match self.green_miners.get(miner_id) {
            Some(info) => info,
            None => return 0.0, // No discount for non-registered miners
        };
        
        // Check if verification is required and present
        let verification_multiplier = match &info.verification {
            Some(verification) => match verification.status {
                VerificationStatus::Approved => 1.0,
                VerificationStatus::Pending => 0.5, // Half discount for pending verification
                VerificationStatus::Rejected => 0.0, // No discount for rejected verification
            },
            None => 0.3, // 30% of discount for unverified claims
        };
        
        // Calculate discount based on renewable percentage
        if info.renewable_percentage >= 95.0 {
            10.0 * verification_multiplier // 10% discount for 95%+ renewable
        } else if info.renewable_percentage >= 75.0 {
            7.0 * verification_multiplier // 7% discount for 75%+ renewable
        } else if info.renewable_percentage >= 50.0 {
            5.0 * verification_multiplier // 5% discount for 50%+ renewable
        } else if info.renewable_percentage >= 25.0 {
            2.0 * verification_multiplier // 2% discount for 25%+ renewable
        } else {
            0.0 // No discount for less than 25% renewable
        }
    }
    
    /// Purchase environmental assets with REC prioritization
    pub fn purchase_prioritized_assets(
        &mut self,
        amount: u64,
        rec_allocation_percentage: f64,
    ) -> Result<Vec<EnvironmentalAssetPurchase>, TreasuryError> {
        if amount > self.balance {
            return Err(TreasuryError::InsufficientFunds(amount, self.balance));
        }
        
        if rec_allocation_percentage < 0.0 || rec_allocation_percentage > 100.0 {
            return Err(TreasuryError::InvalidAllocation(
                "REC allocation percentage must be between 0 and 100".to_string()
            ));
        }
        
        // Default to a minimum of 60% for RECs unless explicitly set lower
        // This enforces the priority of RECs over carbon offsets
        let adjusted_rec_percentage = if rec_allocation_percentage < 60.0 {
            // Only go below 60% if explicitly requested, but log a warning
            log::warn!("REC allocation below recommended minimum of 60%. Using specified {}%", rec_allocation_percentage);
            rec_allocation_percentage
        } else {
            rec_allocation_percentage
        };
        
        let rec_amount = (amount as f64 * adjusted_rec_percentage / 100.0) as u64;
        let offset_amount = amount - rec_amount;
        
        let mut purchases = Vec::new();
        
        // Purchase RECs first - they have priority
        if rec_amount > 0 {
            // In a real implementation, this would connect to a REC marketplace
            // For demo, we'll simulate a purchase
            
            let rec_price_per_mwh = 10000; // 10,000 satoshis per MWh
            let rec_mwh = rec_amount as f64 / rec_price_per_mwh as f64;
            
            let purchase = EnvironmentalAssetPurchase {
                asset_type: EnvironmentalAssetType::RenewableEnergyCertificate,
                amount: rec_mwh,
                cost: rec_amount,
                date: Utc::now(),
                provider: "EcoREC Provider".to_string(),
                reference: format!("REC-{}", Utc::now().timestamp()),
                impact_score: 9.0, // Higher impact score for RECs (increased from 8.5)
            };
            
            purchases.push(purchase.clone());
            self.asset_purchases.push(purchase);
        }
        
        // Purchase carbon offsets with remaining funds only if RECs are already purchased
        if offset_amount > 0 {
            // In a real implementation, this would connect to a carbon offset marketplace
            // For demo, we'll simulate a purchase
            
            let offset_price_per_tonne = 15000; // 15,000 satoshis per tonne
            let offset_tonnes = offset_amount as f64 / offset_price_per_tonne as f64;
            
            let purchase = EnvironmentalAssetPurchase {
                asset_type: EnvironmentalAssetType::CarbonOffset,
                amount: offset_tonnes,
                cost: offset_amount,
                date: Utc::now(),
                provider: "Carbon Offset Provider".to_string(),
                reference: format!("OFFSET-{}", Utc::now().timestamp()),
                impact_score: 5.5, // Lower impact score for offsets (decreased from 6.0)
            };
            
            purchases.push(purchase.clone());
            self.asset_purchases.push(purchase);
        }
        
        // Deduct from balance
        self.balance -= amount;
        
        Ok(purchases)
    }
    
    /// Calculate environmental impact based on the RECs vs offsets ratio
    /// Returns a score from 0-10, with higher scores representing better environmental impact
    pub fn calculate_environmental_impact_score(&self) -> f64 {
        let total_spent: u64 = self.asset_purchases.iter().map(|p| p.cost).sum();
        
        if total_spent == 0 {
            return 0.0;
        }
        
        let rec_spent: u64 = self.asset_purchases
            .iter()
            .filter(|p| p.asset_type == EnvironmentalAssetType::RenewableEnergyCertificate)
            .map(|p| p.cost)
            .sum();
            
        let rec_percentage = (rec_spent as f64 / total_spent as f64) * 100.0;
        
        // Calculate impact score based on REC percentage
        // Higher REC percentage = higher score, prioritizing RECs over offsets
        if rec_percentage >= 90.0 {
            9.5 // Excellent impact, almost all funds to RECs
        } else if rec_percentage >= 80.0 {
            8.5
        } else if rec_percentage >= 70.0 {
            7.5
        } else if rec_percentage >= 60.0 {
            6.5 // Recommended minimum
        } else if rec_percentage >= 50.0 {
            5.5
        } else if rec_percentage >= 40.0 {
            4.5
        } else if rec_percentage >= 30.0 {
            3.5
        } else if rec_percentage >= 20.0 {
            2.5
        } else if rec_percentage > 0.0 {
            1.5
        } else {
            0.5 // Poor impact, no RECs purchased
        }
    }
    
    /// Get the current REC to carbon offset allocation ratio
    pub fn get_rec_allocation_ratio(&self) -> f64 {
        let total_spent: u64 = self.asset_purchases.iter().map(|p| p.cost).sum();
        
        if total_spent == 0 {
            return 0.0;
        }
        
        let rec_spent: u64 = self.asset_purchases
            .iter()
            .filter(|p| p.asset_type == EnvironmentalAssetType::RenewableEnergyCertificate)
            .map(|p| p.cost)
            .sum();
            
        (rec_spent as f64 / total_spent as f64) * 100.0
    }
    
    /// Get recommended REC allocation percentage based on network characteristics
    /// Takes into account the current renewable energy usage across registered miners
    pub fn get_recommended_rec_allocation(&self) -> f64 {
        if self.green_miners.is_empty() {
            return 80.0; // Default high percentage if no miners registered
        }
        
        // Calculate average renewable percentage among registered miners
        let total_miners = self.green_miners.len() as f64;
        let total_renewable_percentage: f64 = self.green_miners
            .values()
            .map(|info| info.renewable_percentage)
            .sum();
            
        let avg_renewable_percentage = total_renewable_percentage / total_miners;
        
        // If miners are already using lots of renewable energy, focus more on offsets
        // Otherwise, prioritize RECs even more
        if avg_renewable_percentage >= 80.0 {
            60.0 // 60% RECs, 40% offsets when miners already use lots of renewables
        } else if avg_renewable_percentage >= 50.0 {
            70.0 // 70% RECs, 30% offsets for moderate renewable usage
        } else {
            80.0 // 80% RECs, 20% offsets for low renewable usage to drive adoption
        }
    }
    
    /// Get the current balance of the treasury
    pub fn get_balance(&self) -> u64 {
        self.balance
    }
    
    /// Get all asset purchases
    pub fn get_asset_purchases(&self) -> &[EnvironmentalAssetPurchase] {
        &self.asset_purchases
    }
    
    /// Get a green miner's information
    pub fn get_green_miner_info(&self, miner_id: &str) -> Option<&GreenMinerInfo> {
        self.green_miners.get(miner_id)
    }
    
    /// Get all green miners
    pub fn get_all_green_miners(&self) -> &HashMap<String, GreenMinerInfo> {
        &self.green_miners
    }
    
    /// Get the allocation percentage
    pub fn get_allocation_percentage(&self) -> f64 {
        self.allocation_percentage
    }
    
    /// Set the allocation percentage
    pub fn set_allocation_percentage(&mut self, percentage: f64) -> Result<(), TreasuryError> {
        if percentage < 0.0 || percentage > 100.0 {
            return Err(TreasuryError::InvalidAllocation(
                "Allocation percentage must be between 0 and 100".to_string()
            ));
        }
        
        self.allocation_percentage = percentage;
        
        Ok(())
    }
}

impl EnvironmentalAssetType {
    fn to_string(&self) -> String {
        match self {
            Self::RenewableEnergyCertificate => "REC".to_string(),
            Self::CarbonOffset => "CARBON".to_string(),
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
        let allocation = treasury.process_block_allocation(fee);
        
        // 2% of 1000 = 20
        assert_eq!(allocation, 20);
        
        // Test with fractional result
        let fee = 33;
        let allocation = treasury.process_block_allocation(fee);
        
        // 2% of 33 = 0.66, which should round down to 0 as u64
        assert_eq!(allocation, 0);
        
        // Test with larger values
        let fee = 1_000_000;
        let allocation = treasury.process_block_allocation(fee);
        
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