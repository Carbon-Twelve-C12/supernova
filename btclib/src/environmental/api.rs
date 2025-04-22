use std::collections::HashMap;
use chrono::{DateTime, Utc};

use crate::blockchain::Block;
use crate::environmental::{
    emissions::{EmissionsError, EmissionsTracker, Region},
    miner_reporting::{MinerEnvironmentalInfo, VerificationStatus},
    treasury::{EnvironmentalTreasury, TreasuryError},
    types::HardwareType,
};

/// Main error type for the environmental API
#[derive(Debug, thiserror::Error)]
pub enum EnvironmentalApiError {
    #[error("Emissions error: {0}")]
    EmissionsError(#[from] EmissionsError),
    
    #[error("Treasury error: {0}")]
    TreasuryError(#[from] TreasuryError),
    
    #[error("Miner not found: {0}")]
    MinerNotFound(String),
    
    #[error("Invalid request: {0}")]
    InvalidRequest(String),
    
    #[error("Database error: {0}")]
    DatabaseError(String),
    
    #[error("Authorization error: {0}")]
    AuthorizationError(String),
}

/// Result type for Environmental API operations
pub type EnvironmentalResult<T> = Result<T, EnvironmentalApiError>;

/// Detailed emissions data for a miner
#[derive(Debug, Clone)]
pub struct MinerEmissionsData {
    pub miner_id: String,
    pub timestamp: DateTime<Utc>,
    pub daily_energy_kwh: f64,
    pub gross_emissions_kg: f64,
    pub net_emissions_kg: f64,
    pub reduction_percentage: f64,
    pub renewable_percentage: f64,
    pub rec_percentage: f64,
    pub offset_percentage: f64,
    pub impact_score: f64,
    pub classification: String,
    pub verification_status: VerificationStatus,
}

/// Network-wide emissions data
#[derive(Debug, Clone)]
pub struct NetworkEmissionsData {
    pub timestamp: DateTime<Utc>,
    pub total_energy_kwh: f64,
    pub total_gross_emissions_kg: f64,
    pub total_net_emissions_kg: f64,
    pub reduction_percentage: f64,
    pub average_impact_score: f64,
    pub miner_count: usize,
    pub green_miner_percentage: f64,
}

/// Treasury asset purchase record
#[derive(Debug, Clone)]
pub struct AssetPurchaseRecord {
    pub timestamp: DateTime<Utc>,
    pub rec_amount: u64,
    pub offset_amount: u64,
    pub rec_percentage: f64,
    pub impact_score: f64,
}

/// Environmental reporting options
#[derive(Debug, Clone)]
pub struct ReportingOptions {
    pub include_unverified_miners: bool,
    pub detailed_breakdown: bool,
    pub regional_analysis: bool,
    pub timeframe_days: u32,
}

impl Default for ReportingOptions {
    fn default() -> Self {
        Self {
            include_unverified_miners: false,
            detailed_breakdown: false,
            regional_analysis: false,
            timeframe_days: 30,
        }
    }
}

/// The main Environmental API that provides a unified interface to all environmental features
pub struct EnvironmentalApi {
    emissions_tracker: EmissionsTracker,
    treasury: EnvironmentalTreasury,
    miner_info: HashMap<String, MinerEnvironmentalInfo>,
    asset_purchase_history: Vec<AssetPurchaseRecord>,
}

impl EnvironmentalApi {
    /// Create a new Environmental API instance
    pub fn new() -> Self {
        Self {
            emissions_tracker: EmissionsTracker::new(),
            treasury: EnvironmentalTreasury::new(),
            miner_info: HashMap::new(),
            asset_purchase_history: Vec::new(),
        }
    }
    
    /// Register a new miner with environmental information
    pub fn register_miner(&mut self, id: &str, info: MinerEnvironmentalInfo) -> EnvironmentalResult<()> {
        if info.renewable_energy_percentage + info.rec_percentage + info.offset_percentage > 100.0 {
            return Err(EnvironmentalApiError::InvalidRequest(
                "Total environmental coverage cannot exceed 100%".to_string(),
            ));
        }
        
        self.miner_info.insert(id.to_string(), info);
        self.emissions_tracker.add_miner_data(id, &self.miner_info[id])?;
        Ok(())
    }
    
    /// Update a miner's environmental information
    pub fn update_miner(&mut self, id: &str, info: MinerEnvironmentalInfo) -> EnvironmentalResult<()> {
        if !self.miner_info.contains_key(id) {
            return Err(EnvironmentalApiError::MinerNotFound(id.to_string()));
        }
        
        if info.renewable_energy_percentage + info.rec_percentage + info.offset_percentage > 100.0 {
            return Err(EnvironmentalApiError::InvalidRequest(
                "Total environmental coverage cannot exceed 100%".to_string(),
            ));
        }
        
        self.miner_info.insert(id.to_string(), info);
        self.emissions_tracker.update_miner_data(id, &self.miner_info[id])?;
        Ok(())
    }
    
    /// Get a miner's environmental information
    pub fn get_miner_info(&self, id: &str) -> EnvironmentalResult<&MinerEnvironmentalInfo> {
        self.miner_info.get(id).ok_or_else(|| EnvironmentalApiError::MinerNotFound(id.to_string()))
    }
    
    /// Calculate emissions for a specific miner
    pub fn calculate_miner_emissions(&self, id: &str) -> EnvironmentalResult<MinerEmissionsData> {
        let miner = self.get_miner_info(id)?;
        let emissions = self.emissions_tracker.calculate_miner_emissions(id)?;
        
        // Determine miner classification based on environmental strategy
        let classification = if miner.renewable_energy_percentage >= 80.0 {
            "Green Miner"
        } else if miner.rec_percentage >= 50.0 {
            "REC-Backed Miner"
        } else if miner.offset_percentage >= 50.0 {
            "Offset Miner"
        } else {
            "Standard Miner"
        };
        
        // Calculate impact score with higher weights for RECs vs offsets
        let impact_score = match miner.verification_status {
            VerificationStatus::Verified => {
                let renewable_weight = 10.0;
                let rec_weight = 9.0;
                let offset_weight = 5.5;
                
                let renewable_score = miner.renewable_energy_percentage * renewable_weight;
                let rec_score = miner.rec_percentage * rec_weight;
                let offset_score = miner.offset_percentage * offset_weight;
                
                let total_coverage = miner.renewable_energy_percentage + miner.rec_percentage + miner.offset_percentage;
                if total_coverage <= 0.0 {
                    0.0
                } else {
                    (renewable_score + rec_score + offset_score) / total_coverage
                }
            },
            _ => 0.0, // Unverified miners get no impact score
        };
        
        Ok(MinerEmissionsData {
            miner_id: id.to_string(),
            timestamp: Utc::now(),
            daily_energy_kwh: emissions.daily_energy_kwh,
            gross_emissions_kg: emissions.gross_emissions_kg,
            net_emissions_kg: emissions.net_emissions_kg,
            reduction_percentage: emissions.reduction_percentage,
            renewable_percentage: miner.renewable_energy_percentage,
            rec_percentage: miner.rec_percentage,
            offset_percentage: miner.offset_percentage,
            impact_score,
            classification: classification.to_string(),
            verification_status: miner.verification_status.clone(),
        })
    }
    
    /// Calculate network-wide emissions data
    pub fn calculate_network_emissions(&self, options: &ReportingOptions) -> EnvironmentalResult<NetworkEmissionsData> {
        let mut total_energy_kwh = 0.0;
        let mut total_gross_emissions_kg = 0.0;
        let mut total_net_emissions_kg = 0.0;
        let mut total_impact_score = 0.0;
        let mut green_miner_count = 0;
        let mut included_miners = 0;
        
        for (id, miner) in &self.miner_info {
            // Skip unverified miners if specified
            if !options.include_unverified_miners && miner.verification_status != VerificationStatus::Verified {
                continue;
            }
            
            let emissions_data = self.calculate_miner_emissions(id)?;
            total_energy_kwh += emissions_data.daily_energy_kwh;
            total_gross_emissions_kg += emissions_data.gross_emissions_kg;
            total_net_emissions_kg += emissions_data.net_emissions_kg;
            total_impact_score += emissions_data.impact_score;
            
            if emissions_data.classification == "Green Miner" {
                green_miner_count += 1;
            }
            
            included_miners += 1;
        }
        
        let reduction_percentage = if total_gross_emissions_kg > 0.0 {
            ((total_gross_emissions_kg - total_net_emissions_kg) / total_gross_emissions_kg) * 100.0
        } else {
            0.0
        };
        
        let average_impact_score = if included_miners > 0 {
            total_impact_score / included_miners as f64
        } else {
            0.0
        };
        
        let green_miner_percentage = if included_miners > 0 {
            (green_miner_count as f64 / included_miners as f64) * 100.0
        } else {
            0.0
        };
        
        Ok(NetworkEmissionsData {
            timestamp: Utc::now(),
            total_energy_kwh,
            total_gross_emissions_kg,
            total_net_emissions_kg,
            reduction_percentage,
            average_impact_score,
            miner_count: included_miners,
            green_miner_percentage,
        })
    }
    
    /// Allocate funds to the environmental treasury from transaction fees
    pub fn process_block_allocation(&mut self, block: &Block) -> EnvironmentalResult<u64> {
        let allocation_amount = self.treasury.process_block_allocation(block)
            .map_err(EnvironmentalApiError::TreasuryError)?;
        Ok(allocation_amount)
    }
    
    /// Calculate fee discount for a miner based on their environmental commitments
    pub fn calculate_fee_discount(&self, miner_id: &str) -> EnvironmentalResult<f64> {
        let miner = self.get_miner_info(miner_id)?;
        
        match miner.verification_status {
            VerificationStatus::Verified => {
                // Calculate base discount from direct renewable
                let renewable_discount = miner.renewable_energy_percentage * 0.5;
                
                // REC discount (prioritized - higher multiplier than offsets)
                let rec_discount = miner.rec_percentage * 0.4;
                
                // Offset discount (lowest priority)
                let offset_discount = miner.offset_percentage * 0.2;
                
                // Cap at 50% max discount
                Ok((renewable_discount + rec_discount + offset_discount).min(50.0))
            },
            _ => Ok(0.0), // No discount for unverified miners
        }
    }
    
    /// Purchase environmental assets with the treasury balance
    pub fn purchase_environmental_assets(&mut self, rec_allocation_percentage: f64) -> EnvironmentalResult<AssetPurchaseRecord> {
        // Enforce minimum 60% REC allocation
        let rec_allocation = rec_allocation_percentage.max(60.0);
        
        let (rec_amount, offset_amount) = self.treasury.purchase_prioritized_assets(rec_allocation)
            .map_err(EnvironmentalApiError::TreasuryError)?;
        
        // Calculate impact score for this purchase
        let rec_weight = 9.0;
        let offset_weight = 5.5;
        
        let total_amount = rec_amount + offset_amount;
        let impact_score = if total_amount > 0 {
            let rec_score = (rec_amount as f64 / total_amount as f64) * rec_weight;
            let offset_score = (offset_amount as f64 / total_amount as f64) * offset_weight;
            rec_score + offset_score
        } else {
            0.0
        };
        
        let record = AssetPurchaseRecord {
            timestamp: Utc::now(),
            rec_amount,
            offset_amount,
            rec_percentage: rec_allocation,
            impact_score,
        };
        
        self.asset_purchase_history.push(record.clone());
        Ok(record)
    }
    
    /// Get the transaction fee for a miner considering environmental discounts
    pub fn get_transaction_fee(&self, base_fee: u64, miner_id: &str) -> EnvironmentalResult<u64> {
        let discount_percentage = self.calculate_fee_discount(miner_id)?;
        let discount_multiplier = 1.0 - (discount_percentage / 100.0);
        let fee = (base_fee as f64 * discount_multiplier) as u64;
        Ok(fee)
    }
    
    /// Get historical asset purchases
    pub fn get_asset_purchase_history(&self) -> &[AssetPurchaseRecord] {
        &self.asset_purchase_history
    }
    
    /// Update emissions factors for a region
    pub fn update_region_emissions_factor(&mut self, region: Region, emissions_factor: f64) -> EnvironmentalResult<()> {
        self.emissions_tracker.update_emissions_factor(region, emissions_factor)
            .map_err(EnvironmentalApiError::EmissionsError)
    }
    
    /// Get the average emissions factor across all regions
    pub fn get_average_emissions_factor(&self) -> EnvironmentalResult<f64> {
        self.emissions_tracker.get_average_emissions_factor()
            .map_err(EnvironmentalApiError::EmissionsError)
    }
    
    /// Get treasury balance
    pub fn get_treasury_balance(&self) -> u64 {
        self.treasury.get_balance()
    }
    
    /// Get regional emissions data
    pub fn get_regional_emissions(&self) -> EnvironmentalResult<HashMap<Region, f64>> {
        if self.miner_info.is_empty() {
            return Ok(HashMap::new());
        }
        
        let mut regional_emissions = HashMap::new();
        
        for (id, _) in &self.miner_info {
            let emissions_data = self.calculate_miner_emissions(id)?;
            let miner = self.get_miner_info(id)?;
            let region = miner.region.clone();
            
            *regional_emissions.entry(region).or_insert(0.0) += emissions_data.net_emissions_kg;
        }
        
        Ok(regional_emissions)
    }
    
    /// Calculate the emissions for a specific transaction
    pub fn calculate_transaction_emissions(&self, tx_size_bytes: usize) -> EnvironmentalResult<f64> {
        // Simplified calculation based on transaction size and network average
        // Real implementation would use a more complex model
        let avg_emissions_factor = self.get_average_emissions_factor()?;
        let energy_per_byte = 0.0000002; // kWh per byte (example value)
        let tx_energy = tx_size_bytes as f64 * energy_per_byte;
        let tx_emissions = tx_energy * avg_emissions_factor;
        
        Ok(tx_emissions)
    }
    
    /// Get miners by classification
    pub fn get_miners_by_classification(&self, classification: &str) -> Vec<String> {
        let mut result = Vec::new();
        
        for (id, _) in &self.miner_info {
            if let Ok(emissions_data) = self.calculate_miner_emissions(id) {
                if emissions_data.classification == classification {
                    result.push(id.clone());
                }
            }
        }
        
        result
    }
    
    /// Get verified hardware types in the network
    pub fn get_hardware_distribution(&self) -> HashMap<HardwareType, usize> {
        let mut distribution = HashMap::new();
        
        for (_, miner) in &self.miner_info {
            if miner.verification_status == VerificationStatus::Verified {
                *distribution.entry(miner.hardware_type.clone()).or_insert(0) += 1;
            }
        }
        
        distribution
    }
}

// Default implementation
impl Default for EnvironmentalApi {
    fn default() -> Self {
        Self::new()
    }
}

/// Example usage of the Environmental API
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_miner_registration_and_emissions() {
        let mut api = EnvironmentalApi::new();
        
        // Create a green miner
        let green_miner = MinerEnvironmentalInfo {
            region: Region::NorthAmerica,
            hardware_type: HardwareType::AntminerS19XP,
            units: 100,
            renewable_energy_percentage: 100.0,
            rec_percentage: 0.0,
            offset_percentage: 0.0,
            verification_status: VerificationStatus::Verified,
        };
        
        // Create a REC-backed miner
        let rec_miner = MinerEnvironmentalInfo {
            region: Region::Europe,
            hardware_type: HardwareType::AntminerS19,
            units: 200,
            renewable_energy_percentage: 30.0,
            rec_percentage: 70.0,
            offset_percentage: 0.0,
            verification_status: VerificationStatus::Verified,
        };
        
        // Register miners
        api.register_miner("green_miner", green_miner).unwrap();
        api.register_miner("rec_miner", rec_miner).unwrap();
        
        // Calculate emissions
        let green_emissions = api.calculate_miner_emissions("green_miner").unwrap();
        let rec_emissions = api.calculate_miner_emissions("rec_miner").unwrap();
        
        // Assert classifications
        assert_eq!(green_emissions.classification, "Green Miner");
        assert_eq!(rec_emissions.classification, "REC-Backed Miner");
        
        // Check that RECs are properly prioritized in impact scores
        assert!(green_emissions.impact_score > rec_emissions.impact_score);
        
        // Calculate network emissions
        let network = api.calculate_network_emissions(&ReportingOptions::default()).unwrap();
        assert!(network.reduction_percentage > 0.0);
        
        // Test fee discounts
        let green_discount = api.calculate_fee_discount("green_miner").unwrap();
        let rec_discount = api.calculate_fee_discount("rec_miner").unwrap();
        
        // Green miners should get higher discounts than REC-backed miners
        assert!(green_discount > rec_discount);
    }
}

/// Data structure containing network-wide emissions information
pub struct NetworkEmissionsData {
    pub total_emissions: f64,           // in kg CO2e
    pub emissions_per_transaction: f64, // in kg CO2e
    pub emissions_per_block: f64,       // in kg CO2e
    pub timestamp: DateTime<Utc>,
}

/// Data structure for individual miner emissions information
pub struct MinerEmissionsData {
    pub miner_id: String,
    pub hardware_type: HardwareType,
    pub region: String,
    pub renewable_percentage: f64,
    pub verification_status: VerificationStatus,
    pub emissions: f64, // in kg CO2e
}

/// Record of an environmental asset purchase
pub struct AssetPurchaseRecord {
    pub asset_type: String,      // "REC" or "Carbon Offset"
    pub value: f64,              // in USD
    pub purchase_date: String,   // ISO 8601 format
}

/// Trait defining methods for accessing environmental data
pub trait EnvironmentalApi {
    /// Get network-wide emissions data
    fn get_network_emissions(&self) -> Result<NetworkEmissionsData, String>;
    
    /// Get emissions data for all miners
    fn get_all_miners(&self) -> Result<Vec<MinerEmissionsData>, String>;
    
    /// Get recent environmental asset purchases
    fn get_recent_asset_purchases(&self, limit: usize) -> Result<Vec<AssetPurchaseRecord>, String>;
    
    /// Get the current treasury balance
    fn get_treasury_balance(&self) -> Result<f64, String>;
    
    /// Get historical emissions data for the specified number of days
    fn get_emissions_history(&self, days: usize) -> Result<Vec<(DateTime<Utc>, f64)>, String>;
    
    /// Get all asset purchases (for calculating distribution)
    fn get_all_asset_purchases(&self) -> Result<Vec<AssetPurchaseRecord>, String>;
} 