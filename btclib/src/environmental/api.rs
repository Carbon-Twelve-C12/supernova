use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use chrono::{DateTime, Utc};
use thiserror::Error;

use crate::blockchain::Block;
use crate::environmental::{
    emissions::{EmissionsError, EmissionsTrackerAdapter, MinerEmissionsResults},
    emissions_factors::{EmissionsFactorDatabase, EmissionFactorSource},
    hardware_types::{HardwareDatabase, HardwareType},
    miner_reporting::{MinerEnvironmentalInfo, VerificationStatus},
    treasury::{EnvironmentalTreasury, TreasuryError},
    types::Region,
};

/// Main error type for the environmental API
#[derive(Debug, Error)]
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
    
    #[error("External API error: {0}")]
    ExternalApiError(String),
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

/// Regional emissions breakdown
#[derive(Debug, Clone)]
pub struct RegionalEmissionsData {
    pub region: Region,
    pub total_energy_kwh: f64,
    pub emissions_kg: f64,
    pub miner_count: usize,
    pub average_efficiency: f64,
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

/// Trait defining methods for accessing environmental data
pub trait EnvironmentalApi {
    /// Register a new miner with environmental information
    fn register_miner(&mut self, id: &str, info: MinerEnvironmentalInfo) -> EnvironmentalResult<()>;

    /// Update a miner's environmental information
    fn update_miner(&mut self, id: &str, info: MinerEnvironmentalInfo) -> EnvironmentalResult<()>;

    /// Get a miner's environmental information
    fn get_miner_info(&self, id: &str) -> EnvironmentalResult<&MinerEnvironmentalInfo>;

    /// Calculate emissions for a specific miner
    fn calculate_miner_emissions(&self, id: &str) -> EnvironmentalResult<MinerEmissionsData>;

    /// Calculate network-wide emissions data
    fn calculate_network_emissions(&self, options: &ReportingOptions) -> EnvironmentalResult<NetworkEmissionsData>;

    /// Get regional emissions breakdown
    fn get_regional_emissions(&self) -> EnvironmentalResult<HashMap<Region, RegionalEmissionsData>>;

    /// Allocate funds to the environmental treasury from transaction fees
    fn process_block_allocation(&mut self, block: &Block) -> EnvironmentalResult<u64>;

    /// Calculate fee discount for a miner based on their environmental commitments
    fn calculate_fee_discount(&self, miner_id: &str) -> EnvironmentalResult<f64>;

    /// Purchase environmental assets with the treasury balance
    fn purchase_environmental_assets(&mut self, rec_allocation_percentage: f64) -> EnvironmentalResult<AssetPurchaseRecord>;

    /// Get the transaction fee for a miner considering environmental discounts
    fn get_transaction_fee(&self, base_fee: u64, miner_id: &str) -> EnvironmentalResult<u64>;

    /// Get historical asset purchases
    fn get_asset_purchase_history(&self) -> &[AssetPurchaseRecord];

    /// Update emissions factors for a region
    fn update_region_emissions_factor(&mut self, region: Region, emissions_factor: f64) -> EnvironmentalResult<()>;

    /// Get the average emissions factor across all regions
    fn get_average_emissions_factor(&self) -> EnvironmentalResult<f64>;

    /// Get treasury balance
    fn get_treasury_balance(&self) -> u64;

    /// Calculate the emissions for a specific transaction
    fn calculate_transaction_emissions(&self, tx_size_bytes: usize) -> EnvironmentalResult<f64>;

    /// Get miners by classification
    fn get_miners_by_classification(&self, classification: &str) -> Vec<String>;

    /// Get verified hardware types in the network
    fn get_hardware_distribution(&self) -> HashMap<HardwareType, usize>;
    
    /// Get emissions history for a specified time period
    fn get_emissions_history(&self, days: usize) -> EnvironmentalResult<Vec<(DateTime<Utc>, f64)>>;
    
    /// Update hardware database
    fn update_hardware_specs(&mut self) -> EnvironmentalResult<()>;
    
    /// Update emissions factors database
    fn update_emissions_factors(&mut self) -> EnvironmentalResult<()>;
}

/// The main Environmental API implementation
pub struct StandardEnvironmentalApi {
    emissions_tracker: EmissionsTrackerAdapter,
    treasury: EnvironmentalTreasury,
    miner_info: HashMap<String, MinerEnvironmentalInfo>,
    asset_purchase_history: Vec<AssetPurchaseRecord>,
    hardware_db: HardwareDatabase,
    emissions_factor_db: EmissionsFactorDatabase,
    emissions_history: Vec<(DateTime<Utc>, f64)>,
}

impl StandardEnvironmentalApi {
    /// Create a new Environmental API instance
    pub fn new() -> Self {
        Self {
            emissions_tracker: EmissionsTrackerAdapter::new(),
            treasury: EnvironmentalTreasury::new(),
            miner_info: HashMap::new(),
            asset_purchase_history: Vec::new(),
            hardware_db: HardwareDatabase::new(),
            emissions_factor_db: EmissionsFactorDatabase::new(),
            emissions_history: Vec::new(),
        }
    }
    
    /// Calculate impact score based on environmental commitments
    fn calculate_impact_score(&self, miner_info: &MinerEnvironmentalInfo) -> f64 {
        match miner_info.verification_status {
            VerificationStatus::Verified => {
                let renewable_weight = 10.0;
                let rec_weight = 9.0;
                let offset_weight = 5.5;
                
                let renewable_score = miner_info.renewable_energy_percentage * renewable_weight;
                let rec_score = miner_info.rec_percentage * rec_weight;
                let offset_score = miner_info.offset_percentage * offset_weight;
                
                let total_coverage = miner_info.renewable_energy_percentage + miner_info.rec_percentage + miner_info.offset_percentage;
                if total_coverage <= 0.0 {
                    0.0
                } else {
                    (renewable_score + rec_score + offset_score) / total_coverage
                }
            },
            _ => 0.0, // Unverified miners get no impact score
        }
    }
    
    /// Determine miner classification based on environmental strategy
    fn determine_miner_classification(&self, miner_info: &MinerEnvironmentalInfo) -> String {
        if miner_info.renewable_energy_percentage >= 80.0 {
            "Green Miner".to_string()
        } else if miner_info.rec_percentage >= 50.0 {
            "REC-Backed Miner".to_string()
        } else if miner_info.offset_percentage >= 50.0 {
            "Offset Miner".to_string()
        } else {
            "Standard Miner".to_string()
        }
    }
}

impl EnvironmentalApi for StandardEnvironmentalApi {
    fn register_miner(&mut self, id: &str, info: MinerEnvironmentalInfo) -> EnvironmentalResult<()> {
        if info.renewable_energy_percentage + info.rec_percentage + info.offset_percentage > 100.0 {
            return Err(EnvironmentalApiError::InvalidRequest(
                "Total environmental coverage cannot exceed 100%".to_string(),
            ));
        }
        
        self.miner_info.insert(id.to_string(), info.clone());
        self.emissions_tracker.add_miner_data(id, &info)
            .map_err(EnvironmentalApiError::EmissionsError)?;
        Ok(())
    }
    
    fn update_miner(&mut self, id: &str, info: MinerEnvironmentalInfo) -> EnvironmentalResult<()> {
        if !self.miner_info.contains_key(id) {
            return Err(EnvironmentalApiError::MinerNotFound(id.to_string()));
        }
        
        if info.renewable_energy_percentage + info.rec_percentage + info.offset_percentage > 100.0 {
            return Err(EnvironmentalApiError::InvalidRequest(
                "Total environmental coverage cannot exceed 100%".to_string(),
            ));
        }
        
        self.miner_info.insert(id.to_string(), info.clone());
        self.emissions_tracker.update_miner_data(id, &info)
            .map_err(EnvironmentalApiError::EmissionsError)?;
        Ok(())
    }
    
    fn get_miner_info(&self, id: &str) -> EnvironmentalResult<&MinerEnvironmentalInfo> {
        self.miner_info.get(id).ok_or_else(|| EnvironmentalApiError::MinerNotFound(id.to_string()))
    }
    
    fn calculate_miner_emissions(&self, id: &str) -> EnvironmentalResult<MinerEmissionsData> {
        let miner = self.get_miner_info(id)?;
        let emissions = self.emissions_tracker.calculate_miner_emissions(id)
            .map_err(EnvironmentalApiError::EmissionsError)?;
        
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
            impact_score: self.calculate_impact_score(miner),
            classification: self.determine_miner_classification(miner),
            verification_status: miner.verification_status.clone(),
        })
    }
    
    fn calculate_network_emissions(&self, options: &ReportingOptions) -> EnvironmentalResult<NetworkEmissionsData> {
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
        
        // Record in history
        self.emissions_history.push((Utc::now(), total_net_emissions_kg));
        
        // Limit history size
        if self.emissions_history.len() > 365 {
            self.emissions_history.remove(0);
        }
        
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
    
    fn get_regional_emissions(&self) -> EnvironmentalResult<HashMap<Region, RegionalEmissionsData>> {
        if self.miner_info.is_empty() {
            return Ok(HashMap::new());
        }
        
        let mut regional_data = HashMap::new();
        
        for (id, miner) in &self.miner_info {
            let emissions_data = self.calculate_miner_emissions(id)?;
            let region = miner.region.clone();
            
            let entry = regional_data.entry(region.clone()).or_insert_with(|| RegionalEmissionsData {
                region: region.clone(),
                total_energy_kwh: 0.0,
                emissions_kg: 0.0,
                miner_count: 0,
                average_efficiency: 0.0,
            });
            
            entry.total_energy_kwh += emissions_data.daily_energy_kwh;
            entry.emissions_kg += emissions_data.net_emissions_kg;
            entry.miner_count += 1;
            
            // Get hardware efficiency
            if let Some(hw_spec) = self.hardware_db.get_spec(miner.hardware_type) {
                entry.average_efficiency += hw_spec.efficiency;
            }
        }
        
        // Calculate average efficiency
        for data in regional_data.values_mut() {
            if data.miner_count > 0 {
                data.average_efficiency /= data.miner_count as f64;
            }
        }
        
        Ok(regional_data)
    }
    
    fn process_block_allocation(&mut self, block: &Block) -> EnvironmentalResult<u64> {
        let allocation_amount = self.treasury.process_block_allocation(block)
            .map_err(EnvironmentalApiError::TreasuryError)?;
        Ok(allocation_amount)
    }
    
    fn calculate_fee_discount(&self, miner_id: &str) -> EnvironmentalResult<f64> {
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
    
    fn purchase_environmental_assets(&mut self, rec_allocation_percentage: f64) -> EnvironmentalResult<AssetPurchaseRecord> {
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
    
    fn get_transaction_fee(&self, base_fee: u64, miner_id: &str) -> EnvironmentalResult<u64> {
        let discount_percentage = self.calculate_fee_discount(miner_id)?;
        let discount_multiplier = 1.0 - (discount_percentage / 100.0);
        let fee = (base_fee as f64 * discount_multiplier) as u64;
        Ok(fee)
    }
    
    fn get_asset_purchase_history(&self) -> &[AssetPurchaseRecord] {
        &self.asset_purchase_history
    }
    
    fn update_region_emissions_factor(&mut self, region: Region, emissions_factor: f64) -> EnvironmentalResult<()> {
        self.emissions_tracker.update_emissions_factor(region, emissions_factor)
            .map_err(EnvironmentalApiError::EmissionsError)
    }
    
    fn get_average_emissions_factor(&self) -> EnvironmentalResult<f64> {
        self.emissions_tracker.get_average_emissions_factor()
            .map_err(EnvironmentalApiError::EmissionsError)
    }
    
    fn get_treasury_balance(&self) -> u64 {
        self.treasury.get_balance()
    }
    
    fn calculate_transaction_emissions(&self, tx_size_bytes: usize) -> EnvironmentalResult<f64> {
        // Improved calculation based on transaction size and network average
        let avg_emissions_factor = self.get_average_emissions_factor()?;
        let energy_per_byte = 0.0000002; // kWh per byte (example value)
        let tx_energy = tx_size_bytes as f64 * energy_per_byte;
        let tx_emissions = tx_energy * avg_emissions_factor;
        
        Ok(tx_emissions)
    }
    
    fn get_miners_by_classification(&self, classification: &str) -> Vec<String> {
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
    
    fn get_hardware_distribution(&self) -> HashMap<HardwareType, usize> {
        let mut distribution = HashMap::new();
        
        for (_, miner) in &self.miner_info {
            if miner.verification_status == VerificationStatus::Verified {
                *distribution.entry(miner.hardware_type.clone()).or_insert(0) += 1;
            }
        }
        
        distribution
    }
    
    fn get_emissions_history(&self, days: usize) -> EnvironmentalResult<Vec<(DateTime<Utc>, f64)>> {
        let limit = days.min(self.emissions_history.len());
        let start_idx = self.emissions_history.len().saturating_sub(limit);
        
        Ok(self.emissions_history[start_idx..].to_vec())
    }
    
    fn update_hardware_specs(&mut self) -> EnvironmentalResult<()> {
        // In a real implementation, this would fetch data from an external API
        // For now, just update with mock data to simulate an update
        self.hardware_db = HardwareDatabase::new();
        
        Ok(())
    }
    
    fn update_emissions_factors(&mut self) -> EnvironmentalResult<()> {
        // In a real implementation, this would fetch data from an external API
        let result = self.emissions_factor_db.update_from_api()
            .map_err(|e| EnvironmentalApiError::ExternalApiError(e))?;
            
        // Update the emissions tracker with the new factors
        for (code, country) in &self.emissions_factor_db.countries {
            if let Some(factor) = country.factors.get(&EmissionFactorSource::WattTimeMOER) {
                let region = Region::new(code);
                self.emissions_tracker.update_emissions_factor(region, *factor)?;
            }
        }
        
        Ok(())
    }
}

// Default implementation
impl Default for StandardEnvironmentalApi {
    fn default() -> Self {
        Self::new()
    }
}

/// Thread-safe version of the Environmental API for use in concurrent contexts
pub struct ThreadSafeEnvironmentalApi {
    api: Arc<Mutex<StandardEnvironmentalApi>>,
}

impl ThreadSafeEnvironmentalApi {
    /// Create a new thread-safe Environmental API instance
    pub fn new() -> Self {
        Self {
            api: Arc::new(Mutex::new(StandardEnvironmentalApi::new())),
        }
    }
    
    /// Get a clone of the internal API reference
    pub fn clone_api(&self) -> Arc<Mutex<StandardEnvironmentalApi>> {
        self.api.clone()
    }
}

impl Default for ThreadSafeEnvironmentalApi {
    fn default() -> Self {
        Self::new()
    }
}

/// EmissionsApiClient provides a simplified interface for external systems
pub struct EmissionsApiClient {
    api: Arc<Mutex<dyn EnvironmentalApi + Send>>,
}

impl EmissionsApiClient {
    /// Create a new client with a thread-safe API implementation
    pub fn new(api: Arc<Mutex<dyn EnvironmentalApi + Send>>) -> Self {
        Self { api }
    }
    
    /// Get network-wide emissions summary
    pub fn get_network_emissions_summary(&self) -> EnvironmentalResult<NetworkEmissionsData> {
        let api = self.api.lock().map_err(|_| {
            EnvironmentalApiError::DatabaseError("Failed to acquire API lock".to_string())
        })?;
        
        api.calculate_network_emissions(&ReportingOptions::default())
    }
    
    /// Get regional emissions breakdown
    pub fn get_regional_breakdown(&self) -> EnvironmentalResult<Vec<RegionalEmissionsData>> {
        let api = self.api.lock().map_err(|_| {
            EnvironmentalApiError::DatabaseError("Failed to acquire API lock".to_string())
        })?;
        
        let regions = api.get_regional_emissions()?;
        Ok(regions.into_values().collect())
    }
    
    /// Get the current treasury balance
    pub fn get_treasury_balance(&self) -> EnvironmentalResult<u64> {
        let api = self.api.lock().map_err(|_| {
            EnvironmentalApiError::DatabaseError("Failed to acquire API lock".to_string())
        })?;
        
        Ok(api.get_treasury_balance())
    }
    
    /// Purchase environmental assets with the specified REC allocation percentage
    pub fn purchase_assets(&self, rec_percentage: f64) -> EnvironmentalResult<AssetPurchaseRecord> {
        let mut api = self.api.lock().map_err(|_| {
            EnvironmentalApiError::DatabaseError("Failed to acquire API lock".to_string())
        })?;
        
        api.purchase_environmental_assets(rec_percentage)
    }
    
    /// Get emissions history for the specified number of days
    pub fn get_emissions_history(&self, days: usize) -> EnvironmentalResult<Vec<(DateTime<Utc>, f64)>> {
        let api = self.api.lock().map_err(|_| {
            EnvironmentalApiError::DatabaseError("Failed to acquire API lock".to_string())
        })?;
        
        api.get_emissions_history(days)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::environmental::types::{Region as TypesRegion};
    
    #[test]
    fn test_miner_registration_and_emissions() {
        let mut api = StandardEnvironmentalApi::new();
        
        // Create a green miner
        let green_miner = MinerEnvironmentalInfo {
            region: TypesRegion::new("US").into(),
            hardware_type: HardwareType::AntminerS19XP,
            units: 100,
            renewable_energy_percentage: 100.0,
            rec_percentage: 0.0,
            offset_percentage: 0.0,
            verification_status: VerificationStatus::Verified,
        };
        
        // Create a REC-backed miner
        let rec_miner = MinerEnvironmentalInfo {
            region: TypesRegion::new("DE").into(),
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
    
    #[test]
    fn test_thread_safe_api() {
        let api = ThreadSafeEnvironmentalApi::new();
        
        // Test that we can clone the API reference
        let api_clone = api.clone_api();
        
        // Verify that we can lock and use the API
        let mut locked_api = api_clone.lock().unwrap();
        
        // Create a test miner
        let test_miner = MinerEnvironmentalInfo {
            region: TypesRegion::new("US").into(),
            hardware_type: HardwareType::AntminerS19XP,
            units: 100,
            renewable_energy_percentage: 100.0,
            rec_percentage: 0.0,
            offset_percentage: 0.0,
            verification_status: VerificationStatus::Verified,
        };
        
        // Register the miner
        locked_api.register_miner("test_miner", test_miner).unwrap();
        
        // Verify the miner was registered
        let info = locked_api.get_miner_info("test_miner").unwrap();
        assert_eq!(info.units, 100);
    }
} 