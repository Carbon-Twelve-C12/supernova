use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::environmental::{
    dashboard::EnvironmentalDashboard,
    emissions::{EmissionsError, EmissionsTracker, VerificationStatus},
    miner_reporting::{MinerEnvironmentalInfo, MinerReportingManager, MinerVerificationStatus},
    transparency::TransparencyDashboard,
    treasury::{EnvironmentalAssetType, EnvironmentalTreasury, TreasuryAccountType, TreasuryError},
};
use crate::types::block::Block;

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

/// Emissions data for a specific miner
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MinerEmissionsData {
    /// Miner ID
    pub miner_id: String,
    /// Miner name
    pub miner_name: String,
    /// Region where the miner is located
    pub region: String,
    /// Energy consumption in kWh per day
    pub energy_consumption_kwh_day: f64,
    /// Emissions in tonnes per year
    pub emissions_tonnes_year: f64,
    /// Hardware types used by the miner
    pub hardware_types: Vec<String>,
    /// Energy sources with percentage breakdown
    pub energy_sources: HashMap<String, f64>,
    /// Renewable energy percentage
    pub renewable_percentage: f64,
    /// Carbon offsets in tonnes
    pub offset_tonnes: f64,
    /// Verification status
    pub verification_status: String,
    /// Energy efficiency in J/TH
    pub energy_efficiency: Option<f64>,
    /// Net carbon impact (emissions minus offsets)
    pub net_carbon_impact: f64,
    /// Whether the miner data is verified
    pub is_verified: bool,
    /// Timestamp of the data
    pub timestamp: DateTime<Utc>,
}

/// Treasury asset purchase record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetPurchaseRecord {
    pub purchase_id: String,
    pub asset_type: String,
    pub amount: f64,
    pub unit: String,
    pub price: f64,
    pub purchase_date: DateTime<Utc>,
    pub issuer: String,
    pub is_verified: bool,
    pub certificate_url: Option<String>,
    pub timestamp: DateTime<Utc>,
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

/// Configuration for the environmental API
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvironmentalConfig {
    /// Whether to enable environmental features
    pub enabled: bool,
    /// Fee allocation percentage for environmental treasury
    pub fee_allocation_percentage: f64,
    /// Whether to enforce environmental standards
    pub enforce_standards: bool,
    /// Minimum renewable percentage for fee discounts
    pub min_renewable_percentage: f64,
    /// Maximum fee discount percentage
    pub max_fee_discount: f64,
    /// REC incentive multiplier
    pub rec_incentive_multiplier: f64,
    /// Offset incentive multiplier
    pub offset_incentive_multiplier: f64,
}

impl Default for EnvironmentalConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            fee_allocation_percentage: 2.0,
            enforce_standards: false,
            min_renewable_percentage: 25.0,
            max_fee_discount: 50.0,
            rec_incentive_multiplier: 2.0,
            offset_incentive_multiplier: 1.2,
        }
    }
}

/// Environmental asset type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvironmentalAsset {
    /// Asset type (REC or carbon offset)
    pub asset_type: String,
    /// Asset amount
    pub amount: f64,
    /// Asset unit
    pub unit: String,
    /// Asset issuer
    pub issuer: String,
    /// Asset verification status
    pub verified: bool,
    /// Asset timestamp
    pub timestamp: DateTime<Utc>,
    /// Asset certificate URL
    pub certificate_url: Option<String>,
}

/// The main Environmental API that provides a unified interface to all environmental features
pub struct EnvironmentalApi {
    /// Emissions tracker
    emissions_tracker: EmissionsTracker,
    /// Miner reporting manager
    miner_reporting: Option<MinerReportingManager>,
    /// Treasury
    treasury: EnvironmentalTreasury,
    /// Configuration
    config: EnvironmentalConfig,
    /// Transparency dashboard
    transparency: Option<TransparencyDashboard>,
    /// Miner environmental information by ID
    miner_info: HashMap<String, MinerEnvironmentalInfo>,
    /// Environmental dashboard
    dashboard: Option<EnvironmentalDashboard>,
    /// Asset purchase history
    pub asset_purchase_history: Vec<AssetPurchaseRecord>,
    /// RECs and carbon offsets
    energy_assets: Vec<EnvironmentalAsset>,
}

impl EnvironmentalApi {
    /// Create a new Environmental API instance
    pub fn new() -> Self {
        Self {
            emissions_tracker: EmissionsTracker::default(),
            miner_reporting: None,
            treasury: EnvironmentalTreasury::default(),
            config: EnvironmentalConfig::default(),
            transparency: None,
            miner_info: HashMap::new(),
            dashboard: None,
            asset_purchase_history: Vec::new(),
            energy_assets: Vec::new(),
        }
    }

    /// Register a new miner with environmental information
    pub fn register_miner(
        &mut self,
        id: &str,
        info: MinerEnvironmentalInfo,
    ) -> EnvironmentalResult<()> {
        if info.renewable_percentage > 100.0 {
            return Err(EnvironmentalApiError::InvalidRequest(
                "Renewable percentage cannot exceed 100%".to_string(),
            ));
        }

        self.miner_info.insert(id.to_string(), info);
        Ok(())
    }

    /// Update a miner's environmental information
    pub fn update_miner(
        &mut self,
        id: &str,
        info: MinerEnvironmentalInfo,
    ) -> EnvironmentalResult<()> {
        if !self.miner_info.contains_key(id) {
            return Err(EnvironmentalApiError::MinerNotFound(id.to_string()));
        }

        if info.renewable_percentage > 100.0 {
            return Err(EnvironmentalApiError::InvalidRequest(
                "Renewable percentage cannot exceed 100%".to_string(),
            ));
        }

        self.miner_info.insert(id.to_string(), info);
        Ok(())
    }

    /// Get a miner's environmental information
    pub fn get_miner_info(&self, id: &str) -> EnvironmentalResult<&MinerEnvironmentalInfo> {
        self.miner_info
            .get(id)
            .ok_or_else(|| EnvironmentalApiError::MinerNotFound(id.to_string()))
    }

    /// Calculate emissions for a specific miner
    pub fn calculate_miner_emissions(&self, id: &str) -> EnvironmentalResult<MinerEmissionsData> {
        let miner = self.get_miner_info(id)?;

        // Calculate offsets based on certificates
        let offset_tonnes = if miner.has_carbon_offsets {
            miner
                .carbon_offsets
                .iter()
                .filter(|offset| offset.verification_status == MinerVerificationStatus::Verified)
                .map(|offset| offset.amount_tonnes)
                .sum()
        } else {
            0.0
        };

        // Calculate the gross emissions
        let gross_emissions = miner.carbon_footprint_tonnes_year.unwrap_or(0.0);

        // Calculate net carbon impact (emissions minus offsets)
        let net_carbon_impact = (gross_emissions - offset_tonnes).max(0.0);

        // Determine verification status as a string
        let verification_status = if let Some(verification) = &miner.verification {
            match verification.status {
                MinerVerificationStatus::Verified => "Verified".to_string(),
                MinerVerificationStatus::Pending => "Pending".to_string(),
                MinerVerificationStatus::Rejected => "Rejected".to_string(),
                MinerVerificationStatus::Unverified => "Unverified".to_string(),
            }
        } else {
            "Unverified".to_string()
        };

        // Convert energy sources from TypesEnergySource to String
        let energy_sources: HashMap<String, f64> = miner
            .energy_sources
            .iter()
            .map(|(source, percentage)| (format!("{:?}", source), *percentage))
            .collect();

        // Convert hardware types from TypesHardwareType to String
        let hardware_types: Vec<String> = miner
            .hardware_types
            .iter()
            .map(|hw| format!("{:?}", hw))
            .collect();

        let emissions = MinerEmissionsData {
            miner_id: id.to_string(),
            miner_name: miner.name.clone(),
            region: miner.region.to_string(),
            energy_consumption_kwh_day: miner.energy_consumption_kwh_day,
            emissions_tonnes_year: gross_emissions,
            renewable_percentage: miner.renewable_percentage,
            energy_sources,
            hardware_types,
            energy_efficiency: miner.calculate_energy_efficiency(),
            offset_tonnes,
            net_carbon_impact,
            is_verified: miner.is_verification_valid(),
            verification_status,
            timestamp: Utc::now(),
        };

        Ok(emissions)
    }

    /// Calculate network-wide emissions data
    pub fn calculate_network_emissions(
        &self,
        options: &ReportingOptions,
    ) -> EnvironmentalResult<NetworkEmissionsData> {
        let mut total_energy_kwh = 0.0;
        let mut total_emissions_tonnes = 0.0;
        let mut total_renewable_percentage = 0.0;
        let mut total_energy_sources = HashMap::new();
        let mut total_offset_tonnes = 0.0;
        let mut total_net_carbon_impact = 0.0;
        let mut included_miners = 0;

        for (id, miner) in &self.miner_info {
            if !options.include_unverified_miners && !miner.is_verification_valid() {
                continue;
            }

            let emissions_data = self.calculate_miner_emissions(id)?;
            total_energy_kwh += emissions_data.energy_consumption_kwh_day;
            total_emissions_tonnes += emissions_data.emissions_tonnes_year;
            total_renewable_percentage += emissions_data.renewable_percentage;
            for (source, amount) in &emissions_data.energy_sources {
                *total_energy_sources.entry(source.clone()).or_insert(0.0) += amount;
            }
            total_offset_tonnes += emissions_data.offset_tonnes;
            total_net_carbon_impact += emissions_data.net_carbon_impact;

            included_miners += 1;
        }

        let renewable_percentage = if total_energy_kwh > 0.0 {
            total_renewable_percentage / included_miners as f64
        } else {
            0.0
        };

        let reduction_percentage = if total_emissions_tonnes > 0.0 {
            ((total_emissions_tonnes - total_offset_tonnes) / total_emissions_tonnes) * 100.0
        } else {
            0.0
        };

        let average_net_carbon_impact = if included_miners > 0 {
            total_net_carbon_impact / included_miners as f64
        } else {
            0.0
        };

        let carbon_intensity = if total_energy_kwh > 0.0 {
            total_emissions_tonnes * 1000.0 / total_energy_kwh
        } else {
            0.0
        };

        let data = NetworkEmissionsData {
            total_energy_mwh: total_energy_kwh / 1000.0, // Convert kWh to MWh
            total_emissions_tons_co2e: total_emissions_tonnes,
            renewable_percentage,
            emissions_per_tx: if !self.miner_info.is_empty() {
                total_emissions_tonnes / self.miner_info.len() as f64
            } else {
                0.0
            },
            timestamp: Utc::now().timestamp() as u64,
        };

        Ok(data)
    }

    /// Allocate funds to the environmental treasury from transaction fees
    pub fn process_block_allocation(&mut self, block: &Block) -> EnvironmentalResult<u64> {
        // Extract the total fees from the block
        let total_fees = block.calculate_total_fees();

        // Call the treasury method with the total fees
        let allocation = self.treasury.process_block_allocation(total_fees);

        // Return the allocation amount
        Ok(allocation)
    }

    /// Calculate fee discount for a miner based on their environmental commitments
    pub fn calculate_fee_discount(&self, miner_id: &str) -> EnvironmentalResult<f64> {
        let miner = self.get_miner_info(miner_id)?;

        if miner.is_verification_valid() {
            let renewable_discount = miner.renewable_percentage * 0.5;

            let rec_bonus = if miner.has_rec_certificates { 5.0 } else { 0.0 };

            let offset_bonus = if miner.has_carbon_offsets { 2.0 } else { 0.0 };

            Ok((renewable_discount + rec_bonus + offset_bonus).min(50.0))
        } else {
            Ok(0.0)
        }
    }

    /// Purchase environmental assets with the treasury balance
    pub fn purchase_environmental_assets(
        &mut self,
        rec_allocation_percentage: f64,
    ) -> EnvironmentalResult<AssetPurchaseRecord> {
        if !self.config.enabled {
            return Err(EnvironmentalApiError::InvalidRequest(
                "Environmental features are disabled".to_string(),
            ));
        }

        let current_balance = self.treasury.get_balance(None);
        if current_balance == 0 {
            return Err(EnvironmentalApiError::TreasuryError(
                TreasuryError::InsufficientFunds(0, 0),
            ));
        }

        // Calculate allocation for REC vs carbon offsets
        let carbon_allocation_percentage = 100.0 - rec_allocation_percentage;

        if !(0.0..=100.0).contains(&rec_allocation_percentage) {
            return Err(EnvironmentalApiError::InvalidRequest(
                "Invalid allocation percentage".to_string(),
            ));
        }

        let rec_amount = (current_balance as f64 * (rec_allocation_percentage / 100.0)) as u64;

        // Purchase an asset
        if rec_amount > 0 {
            let provider = "Green Energy Provider";
            let amount_kwh = (rec_amount as f64) * 10.0; // 10 kWh per unit cost

            let purchase_result = self
                .treasury
                .purchase_renewable_certificates(provider, amount_kwh, rec_amount);

            match purchase_result {
                Ok(purchase) => {
                    // Create a record for the purchase
                    let unit = match purchase.asset_type {
                        EnvironmentalAssetType::REC => "kWh",
                        EnvironmentalAssetType::CarbonOffset => "tonnes CO2e",
                        _ => "units",
                    };

                    let asset_record = AssetPurchaseRecord {
                        purchase_id: purchase.purchase_id,
                        asset_type: purchase.asset_type.to_string(),
                        amount: purchase.amount,
                        unit: unit.to_string(),
                        price: purchase.cost as f64,
                        purchase_date: purchase.purchase_date,
                        issuer: purchase.provider.clone(),
                        is_verified: purchase.verification_status == VerificationStatus::Verified,
                        certificate_url: purchase.verification_reference.clone(),
                        timestamp: Utc::now(),
                    };

                    self.asset_purchase_history.push(asset_record.clone());

                    Ok(asset_record)
                }
                Err(e) => Err(EnvironmentalApiError::TreasuryError(e)),
            }
        } else {
            Err(EnvironmentalApiError::InvalidRequest(
                "No assets purchased".to_string(),
            ))
        }
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

    /// Get treasury balance
    pub fn get_treasury_balance(&self) -> u64 {
        self.treasury.get_balance(Some(TreasuryAccountType::Main))
    }

    /// Get regional emissions data
    pub fn get_regional_emissions(&self) -> EnvironmentalResult<HashMap<String, f64>> {
        if self.miner_info.is_empty() {
            return Ok(HashMap::new());
        }

        let mut regional_emissions = HashMap::new();

        for (id, miner) in &self.miner_info {
            let emissions_data = self.calculate_miner_emissions(id)?;
            let region = miner.region.to_string();

            *regional_emissions.entry(region).or_insert(0.0) += emissions_data.net_carbon_impact;
        }

        Ok(regional_emissions)
    }

    /// Calculate the emissions for a specific transaction
    pub fn calculate_transaction_emissions(
        &self,
        tx_size_bytes: usize,
    ) -> EnvironmentalResult<f64> {
        let avg_emissions_factor = 0.5;
        let energy_per_byte = 0.0000002;
        let tx_energy = tx_size_bytes as f64 * energy_per_byte;
        let tx_emissions = tx_energy * avg_emissions_factor;

        Ok(tx_emissions)
    }

    /// Get miners by classification
    pub fn get_miners_by_classification(&self, _classification: &str) -> Vec<String> {
        let mut result = Vec::new();

        for (id, miner) in &self.miner_info {
            if let Ok(emissions_data) = self.calculate_miner_emissions(id) {
                if emissions_data.is_verified {
                    result.push(id.clone());
                }
            }
        }

        result
    }

    /// Get verified hardware types in the network
    pub fn get_hardware_distribution(&self) -> HashMap<String, usize> {
        let mut distribution = HashMap::new();

        for miner in self.miner_info.values() {
            if miner.is_verification_valid() {
                for hardware_type in &miner.hardware_types {
                    let hw_type_str = format!("{:?}", hardware_type);
                    *distribution.entry(hw_type_str).or_insert(0) += 1;
                }
            }
        }

        distribution
    }

    /// Get all miners
    fn get_all_miners(&self) -> Result<Vec<MinerEnvironmentalInfo>, String> {
        Ok(self.get_all_miners_internal())
    }

    /// Internal method to get all miners
    pub fn get_all_miners_internal(&self) -> Vec<MinerEnvironmentalInfo> {
        self.miner_info.values().cloned().collect()
    }

    /// Get all asset purchases
    pub fn get_all_asset_purchases(&self) -> Result<Vec<AssetPurchaseRecord>, String> {
        // Return the asset purchase history directly
        Ok(self.asset_purchase_history.clone())
    }

    /// Get recent asset purchases
    pub fn get_recent_asset_purchases(
        &self,
        limit: usize,
    ) -> Result<Vec<AssetPurchaseRecord>, String> {
        // Return the recent asset purchases directly
        Ok(self.get_recent_asset_purchases_internal(limit))
    }

    /// Internal method to get recent asset purchases
    pub fn get_recent_asset_purchases_internal(&self, limit: usize) -> Vec<AssetPurchaseRecord> {
        let mut purchases = self.asset_purchase_history.clone();
        purchases.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
        purchases.truncate(limit);
        purchases
    }
}

// Default implementation
impl Default for EnvironmentalApi {
    fn default() -> Self {
        Self::new()
    }
}

/// Environmental API trait
pub trait EnvironmentalApiTrait {
    /// Get all miners
    fn get_all_miners(&self) -> Result<Vec<MinerEnvironmentalInfo>, String>;

    /// Get miner by ID
    fn get_miner_by_id(&self, miner_id: &str) -> Result<MinerEnvironmentalInfo, String>;

    /// Get network emissions data
    fn get_network_emissions(&self) -> Result<NetworkEmissionsData, String>;

    /// Get emissions data for a specific miner
    fn get_miner_emissions(&self, miner_id: &str) -> Result<MinerEmissionsData, String>;

    /// Get recent asset purchases
    fn get_recent_asset_purchases(&self, limit: usize) -> Result<Vec<AssetPurchaseRecord>, String>;

    /// Get all asset purchases
    fn get_all_asset_purchases(&self) -> Result<Vec<AssetPurchaseRecord>, String>;

    /// Get treasury balance
    fn get_treasury_balance(&self) -> Result<f64, String>;

    /// Get emissions history
    fn get_emissions_history(&self, days: usize) -> Result<Vec<(DateTime<Utc>, f64)>, String>;
}

// Implement the EnvironmentalApiTrait for our struct
impl EnvironmentalApiTrait for crate::environmental::api::EnvironmentalApi {
    fn get_all_miners(&self) -> Result<Vec<MinerEnvironmentalInfo>, String> {
        Ok(self.get_all_miners_internal())
    }

    fn get_miner_by_id(&self, miner_id: &str) -> Result<MinerEnvironmentalInfo, String> {
        self.get_miner_info(miner_id)
            .cloned()
            .map_err(|e| e.to_string())
    }

    fn get_network_emissions(&self) -> Result<NetworkEmissionsData, String> {
        let data = NetworkEmissionsData {
            total_energy_mwh: 100.0,         // Example value converted to MWh
            total_emissions_tons_co2e: 50.0, // Example value
            renewable_percentage: 30.0,      // Example value
            emissions_per_tx: 0.1,           // Example emissions per transaction
            timestamp: Utc::now().timestamp() as u64,
        };

        Ok(data)
    }

    fn get_miner_emissions(&self, miner_id: &str) -> Result<MinerEmissionsData, String> {
        // Create a new MinerEmissionsData directly
        let mut energy_sources = HashMap::new();
        energy_sources.insert("Solar".to_string(), 25.0);
        energy_sources.insert("Wind".to_string(), 15.0);
        energy_sources.insert("Coal".to_string(), 60.0);

        let data = MinerEmissionsData {
            miner_id: miner_id.to_string(),
            miner_name: format!("Miner {}", miner_id),
            region: "North America".to_string(),
            energy_consumption_kwh_day: 5000.0, // Example value
            emissions_tonnes_year: 2.5,         // Example value
            hardware_types: vec!["ASIC".to_string(), "GPU".to_string()],
            energy_sources,
            renewable_percentage: 40.0, // Example value
            offset_tonnes: 1.0,         // Example value
            verification_status: "Verified".to_string(),
            energy_efficiency: Some(35.0), // J/TH
            net_carbon_impact: 1.5,        // emissions minus offsets
            is_verified: true,
            timestamp: Utc::now(),
        };

        Ok(data)
    }

    fn get_recent_asset_purchases(&self, limit: usize) -> Result<Vec<AssetPurchaseRecord>, String> {
        // Convert internal AssetPurchaseRecord to the trait's AssetPurchaseRecord
        let internal_records = self.get_recent_asset_purchases_internal(limit);
        let mut converted_records = Vec::new();

        for record in internal_records {
            converted_records.push(AssetPurchaseRecord {
                purchase_id: record.purchase_id.clone(),
                asset_type: record.asset_type.clone(),
                amount: record.amount,
                unit: record.unit.clone(),
                price: record.price,
                purchase_date: record.purchase_date,
                issuer: record.issuer.clone(),
                is_verified: record.is_verified,
                certificate_url: record.certificate_url.clone(),
                timestamp: record.timestamp,
            });
        }

        Ok(converted_records)
    }

    fn get_all_asset_purchases(&self) -> Result<Vec<AssetPurchaseRecord>, String> {
        // Convert internal AssetPurchaseRecord to the trait's AssetPurchaseRecord
        let internal_records = self.asset_purchase_history.clone();
        let mut converted_records = Vec::new();

        for record in internal_records {
            converted_records.push(AssetPurchaseRecord {
                purchase_id: record.purchase_id.clone(),
                asset_type: record.asset_type.clone(),
                amount: record.amount,
                unit: record.unit.clone(),
                price: record.price,
                purchase_date: record.purchase_date,
                issuer: record.issuer.clone(),
                is_verified: record.is_verified,
                certificate_url: record.certificate_url.clone(),
                timestamp: record.timestamp,
            });
        }

        Ok(converted_records)
    }

    fn get_treasury_balance(&self) -> Result<f64, String> {
        Ok(self.get_treasury_balance() as f64)
    }

    fn get_emissions_history(&self, days: usize) -> Result<Vec<(DateTime<Utc>, f64)>, String> {
        // Simplified implementation that returns mock historical data
        let now = Utc::now();
        let mut history = Vec::new();

        for i in 0..days {
            let date = now - chrono::Duration::days(i as i64);
            // Mock emissions value that decreases over time
            let emissions = 100.0 - (i as f64 * 1.5);
            history.push((date, emissions.max(0.0)));
        }

        Ok(history)
    }
}

// Re-export types for convenience
pub use crate::environmental::emissions::NetworkEmissionsData;

/// Example usage of the Environmental API
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_miner_registration_and_emissions() {
        let mut api = EnvironmentalApi::new();

        // Create a green miner
        let green_miner = MinerEnvironmentalInfo {
            miner_id: "green_miner".to_string(),
            name: "Green Miner".to_string(),
            region: Region::NorthAmerica,
            location_verification: None,
            hardware_types: vec![HardwareType::Asic],
            energy_sources: {
                let mut sources = HashMap::new();
                sources.insert(EnergySource::Solar, 80.0);
                sources.insert(EnergySource::Wind, 20.0);
                sources
            },
            renewable_percentage: 100.0,
            verification: Some(VerificationInfo {
                provider: "Green Energy Verifier".to_string(),
                date: Utc::now(),
                reference: "GEV-12345".to_string(),
                status: MinerVerificationStatus::Verified,
            }),
            total_hashrate: 100.0,
            energy_consumption_kwh_day: 2400.0,
            carbon_footprint_tonnes_year: Some(0.0),
            last_update: Utc::now(),
            has_rec_certificates: true,
            has_carbon_offsets: false,
            certificates_url: Some("https://example.com/certificates/green".to_string()),
            rec_certificates: Vec::new(),
            carbon_offsets: Vec::new(),
            environmental_score: Some(95.0),
            preferred_energy_type: Some(EnergySource::Solar),
        };

        // Create a REC-backed miner
        let rec_miner = MinerEnvironmentalInfo {
            miner_id: "rec_miner".to_string(),
            name: "REC-Backed Miner".to_string(),
            region: Region::Europe,
            location_verification: None,
            hardware_types: vec![HardwareType::Asic],
            energy_sources: {
                let mut sources = HashMap::new();
                sources.insert(EnergySource::Coal, 70.0);
                sources.insert(EnergySource::Solar, 30.0);
                sources
            },
            renewable_percentage: 30.0,
            verification: Some(VerificationInfo {
                provider: "REC Verifier".to_string(),
                date: Utc::now(),
                reference: "REC-67890".to_string(),
                status: MinerVerificationStatus::Verified,
            }),
            total_hashrate: 200.0,
            energy_consumption_kwh_day: 5000.0,
            carbon_footprint_tonnes_year: Some(100.0),
            last_update: Utc::now(),
            has_rec_certificates: true,
            has_carbon_offsets: false,
            certificates_url: Some("https://example.com/certificates/rec".to_string()),
            rec_certificates: Vec::new(),
            carbon_offsets: Vec::new(),
            environmental_score: Some(70.0),
            preferred_energy_type: Some(EnergySource::Wind),
        };

        // Register miners
        api.register_miner("green_miner", green_miner).unwrap();
        api.register_miner("rec_miner", rec_miner).unwrap();

        // Calculate emissions
        let green_emissions = api.calculate_miner_emissions("green_miner").unwrap();
        let rec_emissions = api.calculate_miner_emissions("rec_miner").unwrap();

        // Assert classifications
        assert_eq!(green_emissions.miner_name, "Green Miner");
        assert_eq!(rec_emissions.miner_name, "REC-Backed Miner");

        // Check that RECs are properly prioritized in impact scores
        assert!(green_emissions.net_carbon_impact <= rec_emissions.net_carbon_impact);

        // Calculate network emissions
        let network = api
            .calculate_network_emissions(&ReportingOptions::default())
            .unwrap();
        assert!(network.renewable_percentage > 0.0);

        // Test fee discounts
        let green_discount = api.calculate_fee_discount("green_miner").unwrap();
        let rec_discount = api.calculate_fee_discount("rec_miner").unwrap();

        // Green miners should get higher discounts than REC-backed miners
        assert!(green_discount > rec_discount);
    }
}
