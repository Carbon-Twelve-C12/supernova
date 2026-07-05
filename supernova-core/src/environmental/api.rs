use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::environmental::{
    dashboard::EnvironmentalDashboard,
    emissions::{EmissionsError, EmissionsTracker},
    miner_reporting::{MinerEnvironmentalInfo, MinerReportingManager, MinerVerificationStatus},
    transparency::TransparencyDashboard,
    treasury::{EnvironmentalTreasury, TreasuryAccountType, TreasuryError},
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

    #[error("Methodology not available: {0}")]
    MethodologyNotAvailable(String),
}

/// Result type for Environmental API operations
pub type EnvironmentalResult<T> = Result<T, EnvironmentalApiError>;

/// Global-average grid carbon intensity, in tonnes CO2e per MWh.
///
/// This mirrors the emissions tracker's `default_emission_factor` of
/// 450 gCO2e/kWh (== 0.45 tonnes/MWh) used for regions with no measured
/// factor. It is used to derive a conservative emissions estimate for
/// miners that report energy consumption but have not reported a carbon
/// footprint, so such miners are never silently counted as zero-emission.
const GLOBAL_AVG_GRID_FACTOR_TONNES_PER_MWH: f64 = 0.45;

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
    /// Recorded network emissions snapshots, in chronological order.
    /// This is the source of truth for historical emissions reporting.
    emissions_history: Vec<(DateTime<Utc>, NetworkEmissionsData)>,
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
            emissions_history: Vec::new(),
        }
    }

    /// Record a snapshot of current network-wide emissions into the historical
    /// series so that [`EnvironmentalApiTrait::get_emissions_history`] can return
    /// real observed data instead of a fabricated trend.
    pub fn record_emissions_snapshot(&mut self) -> EnvironmentalResult<()> {
        let data = self.calculate_network_emissions(&ReportingOptions::default())?;
        self.emissions_history.push((Utc::now(), data));
        Ok(())
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

        // Calculate the gross emissions.
        //
        // A miner that has not reported a carbon footprint must NOT be treated
        // as zero-emission: doing so silently understates both its own figure
        // and the network-wide aggregate (see `calculate_network_emissions`),
        // which is unacceptable for a chain that claims carbon-negativity.
        // Instead, derive a conservative estimate from the reported energy
        // consumption using the global-average grid factor, crediting only the
        // renewable share the miner has actually reported. Miners with no
        // reported energy at all yield 0.0 because there is nothing to estimate
        // from.
        let gross_emissions = match miner.carbon_footprint_tonnes_year {
            Some(footprint) => footprint,
            None => {
                let annual_energy_mwh = miner.energy_consumption_kwh_day * 365.0 / 1000.0;
                let non_renewable_fraction =
                    1.0 - (miner.renewable_percentage.clamp(0.0, 100.0) / 100.0);
                annual_energy_mwh
                    * non_renewable_fraction
                    * GLOBAL_AVG_GRID_FACTOR_TONNES_PER_MWH
            }
        };

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
            // Energy-weight the renewable percentage so a fleet of tiny
            // 100%-renewable miners cannot mask one large fossil-fueled miner.
            // Accumulate sum(energy_i * renewable_i); divided by total energy
            // below, mirroring generate_geographic_breakdown.
            total_renewable_percentage +=
                emissions_data.energy_consumption_kwh_day * emissions_data.renewable_percentage;
            for (source, amount) in &emissions_data.energy_sources {
                *total_energy_sources.entry(source.clone()).or_insert(0.0) += amount;
            }
            total_offset_tonnes += emissions_data.offset_tonnes;
            total_net_carbon_impact += emissions_data.net_carbon_impact;

            included_miners += 1;
        }

        // Energy-weighted network renewable percentage:
        // sum(energy_i * renewable_i) / sum(energy_i). Falls back to 0.0 when
        // no energy is reported, matching generate_geographic_breakdown.
        let renewable_percentage = if total_energy_kwh > 0.0 {
            total_renewable_percentage / total_energy_kwh
        } else {
            0.0
        };

        let _reduction_percentage = if total_emissions_tonnes > 0.0 {
            ((total_emissions_tonnes - total_offset_tonnes) / total_emissions_tonnes) * 100.0
        } else {
            0.0
        };

        let _average_net_carbon_impact = if included_miners > 0 {
            total_net_carbon_impact / included_miners as f64
        } else {
            0.0
        };

        let _carbon_intensity = if total_energy_kwh > 0.0 {
            total_emissions_tonnes * 1000.0 / total_energy_kwh
        } else {
            0.0
        };

        let data = NetworkEmissionsData {
            total_energy_mwh: total_energy_kwh / 1000.0, // Convert kWh to MWh
            total_emissions_tons_co2e: total_emissions_tonnes,
            renewable_percentage,
            // Per-transaction emissions require a real transaction count
            // (see emissions.rs: daily_emissions_kg / tx_per_day). This
            // computation has no access to network throughput, so report 0.0
            // rather than dividing total emissions by the miner count, which
            // would publish a per-miner figure under a per-transaction label.
            // TODO: plumb transaction throughput through and compute honestly.
            emissions_per_tx: 0.0,
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
        let _carbon_allocation_percentage = 100.0 - rec_allocation_percentage;

        if !(0.0..=100.0).contains(&rec_allocation_percentage) {
            return Err(EnvironmentalApiError::InvalidRequest(
                "Invalid allocation percentage".to_string(),
            ));
        }

        // SECURITY FIX [R5-45]: Do NOT fabricate purchase details.
        //
        // The previous implementation invented a provider ("Green Energy
        // Provider") and synthesized a REC quantity from the treasury cost
        // (`amount_kwh = rec_amount * 10.0`, "10 kWh per unit cost"). Those
        // fabricated kWh were then added to `total_recs_kwh`, inflating the
        // renewable-energy totals that back Supernova's carbon-negative claims
        // for a purchase that never occurred on any external market — while a
        // real treasury balance was deducted.
        //
        // Fail closed: until a real renewable-energy market integration exists
        // to supply a verified provider, unit price, and delivered kWh, refuse
        // to record a REC purchase rather than mint synthetic quantities into
        // the treasury totals.
        Err(EnvironmentalApiError::MethodologyNotAvailable(
            "REC purchasing requires a verified external market integration \
             (provider, unit price, and delivered kWh). None is configured, so \
             the environmental treasury refuses to record a purchase with \
             fabricated quantities."
                .to_string(),
        ))
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

    /// Calculate the emissions for a specific transaction.
    ///
    /// A per-transaction emissions figure can only be reported honestly if it is
    /// apportioned from the network's measured energy consumption and emissions
    /// (see [`Self::calculate_network_emissions`]) against the network's measured
    /// transaction/byte throughput. The `EnvironmentalApi` does not currently track
    /// measured throughput, so there is no sourced basis to convert transaction size
    /// into energy or emissions. Returning a hardcoded per-byte factor would fabricate
    /// carbon data, so this method fails closed until a measured methodology is wired.
    pub fn calculate_transaction_emissions(
        &self,
        _tx_size_bytes: usize,
    ) -> EnvironmentalResult<f64> {
        Err(EnvironmentalApiError::MethodologyNotAvailable(
            "per-transaction emissions require measured network throughput, which is not \
             yet tracked; no sourced methodology is wired to convert transaction size \
             into emissions"
                .to_string(),
        ))
    }

    /// Get miners matching an environmental classification.
    ///
    /// The `classification` argument is honored: each verified miner's measured
    /// renewable share and net carbon impact are checked against the requested
    /// class, so a caller asking for e.g. `"green"` miners receives only miners
    /// that actually meet the green threshold — not the full verified set.
    ///
    /// Only verified miners are ever eligible, because an unverified self-report
    /// cannot substantiate any classification claim. An unrecognized
    /// classification string yields an empty list rather than silently returning
    /// every miner. Matching is case-insensitive and tolerates `-`/`_`/space
    /// separators.
    ///
    /// Recognized classifications:
    /// - `"green"` / `"renewable"`: renewable share ≥ 50%.
    /// - `"fully-renewable"`: renewable share ≥ 95%.
    /// - `"carbon-negative"`: net carbon impact (emissions minus offsets) ≤ 0.
    /// - `"verified"` / `"all"`: any verified miner, regardless of energy mix.
    pub fn get_miners_by_classification(&self, classification: &str) -> Vec<String> {
        // Normalize the requested class so callers can pass any of the common
        // spellings (e.g. "carbon-negative", "carbon_negative", "Carbon Negative").
        let normalized = classification
            .trim()
            .to_ascii_lowercase()
            .replace(['-', ' '], "_");

        let mut result = Vec::new();

        for id in self.miner_info.keys() {
            let emissions_data = match self.calculate_miner_emissions(id) {
                Ok(data) => data,
                Err(_) => continue,
            };

            // An unverified self-report cannot substantiate any environmental
            // classification, so such miners never match.
            if !emissions_data.is_verified {
                continue;
            }

            let matches = match normalized.as_str() {
                "green" | "renewable" => emissions_data.renewable_percentage >= 50.0,
                "fully_renewable" => emissions_data.renewable_percentage >= 95.0,
                "carbon_negative" => emissions_data.net_carbon_impact <= 0.0,
                "verified" | "all" => true,
                // Unknown classification: assert nothing rather than returning
                // the full verified set under a misleading label.
                _ => false,
            };

            if matches {
                result.push(id.clone());
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
        // Delegate to the real data-driven aggregation over registered miners
        // instead of returning hardcoded placeholder constants, so the dashboard
        // reflects actual network state. Propagate errors rather than masking them.
        self.calculate_network_emissions(&ReportingOptions::default())
            .map_err(|e| e.to_string())
    }

    fn get_miner_emissions(&self, miner_id: &str) -> Result<MinerEmissionsData, String> {
        // Delegate to the real data-driven calculation over the miner's registered
        // environmental info instead of fabricating fixed constants. Unknown miners
        // return MinerNotFound rather than an invented "verified" profile.
        self.calculate_miner_emissions(miner_id)
            .map_err(|e| e.to_string())
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
        // Return actual persisted network emissions samples within the requested
        // window rather than a fabricated trend. If no history has been recorded,
        // return an empty series instead of manufacturing a declining curve.
        let cutoff = Utc::now() - chrono::Duration::days(days as i64);
        let history = self
            .emissions_history
            .iter()
            .filter(|(timestamp, _)| *timestamp >= cutoff)
            .map(|(timestamp, data)| (*timestamp, data.total_emissions_tons_co2e))
            .collect();

        Ok(history)
    }
}

// Re-export types for convenience
pub use crate::environmental::emissions::NetworkEmissionsData;

/// Example usage of the Environmental API
#[cfg(test)]
mod tests {
    use super::*;
    use crate::environmental::{EnergySource, HardwareType, Region};
    use crate::environmental::miner_reporting::VerificationInfo;

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

    #[test]
    fn test_trait_network_emissions_reflects_real_data_not_constants() {
        // With no miners registered, the trait impl must return zeros derived
        // from real aggregation, NOT the old hardcoded placeholder constants
        // (100 MWh / 50 t / 30% / 0.1).
        let mut api = EnvironmentalApi::new();
        let empty = EnvironmentalApiTrait::get_network_emissions(&api).unwrap();
        assert_eq!(empty.total_energy_mwh, 0.0);
        assert_eq!(empty.total_emissions_tons_co2e, 0.0);
        assert_eq!(empty.renewable_percentage, 0.0);
        assert_eq!(empty.emissions_per_tx, 0.0);

        // Register a real miner and confirm the trait output now reflects it.
        let miner = MinerEnvironmentalInfo {
            miner_id: "m1".to_string(),
            name: "Miner One".to_string(),
            region: Region::NorthAmerica,
            location_verification: None,
            hardware_types: vec![HardwareType::Asic],
            energy_sources: {
                let mut sources = HashMap::new();
                sources.insert(EnergySource::Coal, 100.0);
                sources
            },
            renewable_percentage: 0.0,
            verification: Some(VerificationInfo {
                provider: "Verifier".to_string(),
                date: Utc::now(),
                reference: "REF-1".to_string(),
                status: MinerVerificationStatus::Verified,
            }),
            total_hashrate: 100.0,
            energy_consumption_kwh_day: 10000.0,
            carbon_footprint_tonnes_year: Some(50.0),
            last_update: Utc::now(),
            has_rec_certificates: false,
            has_carbon_offsets: false,
            certificates_url: None,
            rec_certificates: Vec::new(),
            carbon_offsets: Vec::new(),
            environmental_score: Some(10.0),
            preferred_energy_type: Some(EnergySource::Coal),
        };
        api.register_miner("m1", miner).unwrap();

        let data = EnvironmentalApiTrait::get_network_emissions(&api).unwrap();
        // 10000 kWh/day -> 10 MWh, driven by the registered miner, not the
        // stale 100.0 placeholder.
        assert!((data.total_energy_mwh - 10.0).abs() < 1e-9);
        assert_ne!(data.total_energy_mwh, 100.0);
        assert!(data.total_emissions_tons_co2e > 0.0);
        // emissions_per_tx must NOT be a per-miner figure (total emissions
        // divided by miner count). With no transaction throughput plumbed in,
        // it must stay 0.0 rather than leaking a per-miner value under a
        // per-transaction label.
        assert_eq!(data.emissions_per_tx, 0.0);
        assert_ne!(data.emissions_per_tx, data.total_emissions_tons_co2e);
        // Must equal the inherent data-driven method exactly.
        let inherent = api
            .calculate_network_emissions(&ReportingOptions::default())
            .unwrap();
        assert_eq!(data.total_energy_mwh, inherent.total_energy_mwh);
        assert_eq!(
            data.total_emissions_tons_co2e,
            inherent.total_emissions_tons_co2e
        );
    }

    #[test]
    fn test_trait_miner_emissions_reflects_real_data_not_constants() {
        // Unknown miners must error (MinerNotFound), not return a fabricated
        // "verified" constant profile.
        let mut api = EnvironmentalApi::new();
        assert!(EnvironmentalApiTrait::get_miner_emissions(&api, "does_not_exist").is_err());

        // Register a coal-heavy, low-renewable miner and confirm the trait output
        // reflects the registered data rather than the old placeholder constants
        // (Solar 25 / Wind 15 / Coal 60, 5000 kWh/day, 2.5 t/yr, 40% renewable,
        // is_verified:true).
        let miner = MinerEnvironmentalInfo {
            miner_id: "m1".to_string(),
            name: "Miner One".to_string(),
            region: Region::NorthAmerica,
            location_verification: None,
            hardware_types: vec![HardwareType::Asic],
            energy_sources: {
                let mut sources = HashMap::new();
                sources.insert(EnergySource::Coal, 100.0);
                sources
            },
            renewable_percentage: 0.0,
            verification: None,
            total_hashrate: 100.0,
            energy_consumption_kwh_day: 12345.0,
            carbon_footprint_tonnes_year: Some(77.0),
            last_update: Utc::now(),
            has_rec_certificates: false,
            has_carbon_offsets: false,
            certificates_url: None,
            rec_certificates: Vec::new(),
            carbon_offsets: Vec::new(),
            environmental_score: Some(10.0),
            preferred_energy_type: Some(EnergySource::Coal),
        };
        api.register_miner("m1", miner).unwrap();

        let data = EnvironmentalApiTrait::get_miner_emissions(&api, "m1").unwrap();

        // Values come from the registered miner, not the old constants.
        assert_eq!(data.miner_id, "m1");
        assert_eq!(data.miner_name, "Miner One");
        assert_eq!(data.energy_consumption_kwh_day, 12345.0);
        assert_ne!(data.energy_consumption_kwh_day, 5000.0);
        assert_eq!(data.emissions_tonnes_year, 77.0);
        assert_ne!(data.emissions_tonnes_year, 2.5);
        assert_eq!(data.renewable_percentage, 0.0);
        assert_ne!(data.renewable_percentage, 40.0);
        // Unverified miner must NOT be reported as verified.
        assert!(!data.is_verified);

        // Must equal the inherent data-driven method exactly.
        let inherent = api.calculate_miner_emissions("m1").unwrap();
        assert_eq!(data.energy_consumption_kwh_day, inherent.energy_consumption_kwh_day);
        assert_eq!(data.emissions_tonnes_year, inherent.emissions_tonnes_year);
        assert_eq!(data.is_verified, inherent.is_verified);
    }

    #[test]
    fn test_unreported_footprint_is_conservatively_estimated_not_zero() {
        // A miner that reports energy consumption but never reports a carbon
        // footprint must NOT be counted as zero-emission: that would silently
        // understate both the per-miner figure and the network aggregate.
        let mut api = EnvironmentalApi::new();

        let miner = MinerEnvironmentalInfo {
            miner_id: "no_report".to_string(),
            name: "Unreported Miner".to_string(),
            region: Region::NorthAmerica,
            location_verification: None,
            hardware_types: vec![HardwareType::Asic],
            energy_sources: HashMap::new(),
            renewable_percentage: 0.0,
            verification: None,
            total_hashrate: 100.0,
            energy_consumption_kwh_day: 10000.0,
            // The defect: no reported footprint.
            carbon_footprint_tonnes_year: None,
            last_update: Utc::now(),
            has_rec_certificates: false,
            has_carbon_offsets: false,
            certificates_url: None,
            rec_certificates: Vec::new(),
            carbon_offsets: Vec::new(),
            environmental_score: None,
            preferred_energy_type: None,
        };
        api.register_miner("no_report", miner).unwrap();

        let data = api.calculate_miner_emissions("no_report").unwrap();

        // Conservative estimate: 10000 kWh/day * 365 / 1000 = 3650 MWh/yr,
        // 0% renewable, * 0.45 t/MWh = 1642.5 t/yr. Must be strictly positive,
        // never the old silent zero.
        let expected = 10000.0 * 365.0 / 1000.0 * 1.0 * GLOBAL_AVG_GRID_FACTOR_TONNES_PER_MWH;
        assert!(data.emissions_tonnes_year > 0.0);
        assert!((data.emissions_tonnes_year - expected).abs() < 1e-6);
        assert_ne!(data.emissions_tonnes_year, 0.0);

        // The unreported miner must flow into the network aggregate as a
        // non-zero contribution too.
        let opts = ReportingOptions {
            include_unverified_miners: true,
            ..ReportingOptions::default()
        };
        let network = api.calculate_network_emissions(&opts).unwrap();
        assert!(network.total_emissions_tons_co2e > 0.0);

        // Reported renewable share is still credited: a fully-renewable
        // unreported miner estimates to zero, which is honest.
        let green = MinerEnvironmentalInfo {
            miner_id: "green_no_report".to_string(),
            name: "Green Unreported".to_string(),
            region: Region::NorthAmerica,
            location_verification: None,
            hardware_types: vec![HardwareType::Asic],
            energy_sources: HashMap::new(),
            renewable_percentage: 100.0,
            verification: None,
            total_hashrate: 100.0,
            energy_consumption_kwh_day: 10000.0,
            carbon_footprint_tonnes_year: None,
            last_update: Utc::now(),
            has_rec_certificates: false,
            has_carbon_offsets: false,
            certificates_url: None,
            rec_certificates: Vec::new(),
            carbon_offsets: Vec::new(),
            environmental_score: None,
            preferred_energy_type: None,
        };
        api.register_miner("green_no_report", green).unwrap();
        let green_data = api.calculate_miner_emissions("green_no_report").unwrap();
        assert_eq!(green_data.emissions_tonnes_year, 0.0);
    }

    #[test]
    fn test_emissions_history_is_real_not_fabricated_trend() {
        let mut api = EnvironmentalApi::new();

        // With nothing recorded, history must be empty rather than a manufactured
        // declining curve (the old mock returned `100.0 - i * 1.5`).
        let empty = EnvironmentalApiTrait::get_emissions_history(&api, 30).unwrap();
        assert!(
            empty.is_empty(),
            "expected empty history with no recorded snapshots, got {} points",
            empty.len()
        );

        // Register a coal miner with known emissions, then record two snapshots.
        let miner = MinerEnvironmentalInfo {
            miner_id: "hist_miner".to_string(),
            name: "History Miner".to_string(),
            region: Region::NorthAmerica,
            location_verification: None,
            hardware_types: vec![HardwareType::Asic],
            energy_sources: {
                let mut sources = HashMap::new();
                sources.insert(EnergySource::Coal, 100.0);
                sources
            },
            renewable_percentage: 0.0,
            verification: None,
            total_hashrate: 100.0,
            energy_consumption_kwh_day: 5000.0,
            carbon_footprint_tonnes_year: Some(42.0),
            last_update: Utc::now(),
            has_rec_certificates: false,
            has_carbon_offsets: false,
            certificates_url: None,
            rec_certificates: Vec::new(),
            carbon_offsets: Vec::new(),
            environmental_score: Some(10.0),
            preferred_energy_type: Some(EnergySource::Coal),
        };
        api.register_miner("hist_miner", miner).unwrap();

        api.record_emissions_snapshot().unwrap();
        api.record_emissions_snapshot().unwrap();

        let history = EnvironmentalApiTrait::get_emissions_history(&api, 30).unwrap();
        assert_eq!(history.len(), 2, "should return exactly the recorded snapshots");

        // Recorded values reflect the real network aggregate (unverified miner is
        // excluded by default options -> 0.0 emissions), never the old fabricated
        // 100.0 starting value or a monotonic 1.5/day decline.
        let expected = api
            .calculate_network_emissions(&ReportingOptions::default())
            .unwrap()
            .total_emissions_tons_co2e;
        for (_, value) in &history {
            assert_eq!(*value, expected);
            assert_ne!(*value, 100.0);
        }
    }

    #[test]
    fn test_transaction_emissions_fails_closed_not_fabricated() {
        let api = EnvironmentalApi::new();

        // The old implementation returned tx_size * 0.0000002 * 0.5 from two
        // unsourced constants. It must now fail closed rather than fabricate a figure.
        let result = api.calculate_transaction_emissions(250);
        assert!(matches!(
            result,
            Err(EnvironmentalApiError::MethodologyNotAvailable(_))
        ));

        // The fabricated value for 250 bytes would have been 0.000025 - never returned.
        assert!(api.calculate_transaction_emissions(250).is_err());
    }

    #[test]
    fn test_purchase_environmental_assets_fails_closed_not_fabricated() {
        let mut api = EnvironmentalApi::new();

        // Fund the environmental treasury with a real balance (2% of 1,000,000).
        let allocated = api.treasury.process_block_allocation(1_000_000);
        assert!(
            allocated > 0,
            "treasury should have a positive balance so the purchase path is reached"
        );
        let balance_before = api.treasury.get_balance(None);
        assert!(balance_before > 0);

        // Renewable totals that back the carbon-negative claims start at zero.
        assert_eq!(api.treasury.get_total_recs_kwh(), 0.0);

        // The old implementation fabricated a provider ("Green Energy Provider")
        // and synthesized `amount_kwh = rec_amount * 10.0`, deducting real balance
        // and inflating total_recs_kwh for a purchase that never occurred on any
        // external market. It must now fail closed instead of minting synthetic
        // REC quantities.
        let result = api.purchase_environmental_assets(50.0);
        assert!(matches!(
            result,
            Err(EnvironmentalApiError::MethodologyNotAvailable(_))
        ));

        // No fabricated REC quantity may be added to the totals that feed the
        // renewable-energy / carbon-negative accounting.
        assert_eq!(api.treasury.get_total_recs_kwh(), 0.0);
        // Real treasury balance must NOT be deducted for a purchase that did not
        // happen.
        assert_eq!(api.treasury.get_balance(None), balance_before);
        // No synthetic purchase record may be recorded either.
        assert!(api.get_asset_purchase_history().is_empty());
    }

    #[test]
    fn test_network_renewable_percentage_is_energy_weighted() {
        // Ten tiny 100%-renewable miners must NOT mask one large fossil miner.
        // Unweighted mean would report ~91%; energy-weighted reports ~0.1%.
        let mut api = EnvironmentalApi::new();

        let make_miner = |id: &str, energy: f64, renewable: f64| MinerEnvironmentalInfo {
            miner_id: id.to_string(),
            name: id.to_string(),
            region: Region::NorthAmerica,
            location_verification: None,
            hardware_types: vec![HardwareType::Asic],
            energy_sources: HashMap::new(),
            renewable_percentage: renewable,
            verification: Some(VerificationInfo {
                provider: "Verifier".to_string(),
                date: Utc::now(),
                reference: format!("REF-{}", id),
                status: MinerVerificationStatus::Verified,
            }),
            total_hashrate: 100.0,
            energy_consumption_kwh_day: energy,
            carbon_footprint_tonnes_year: Some(0.0),
            last_update: Utc::now(),
            has_rec_certificates: false,
            has_carbon_offsets: false,
            certificates_url: None,
            rec_certificates: Vec::new(),
            carbon_offsets: Vec::new(),
            environmental_score: Some(50.0),
            preferred_energy_type: None,
        };

        for i in 0..10 {
            let id = format!("tiny_{}", i);
            api.register_miner(&id, make_miner(&id, 1.0, 100.0)).unwrap();
        }
        api.register_miner("coal", make_miner("coal", 10_000.0, 0.0))
            .unwrap();

        let network = api
            .calculate_network_emissions(&ReportingOptions::default())
            .unwrap();

        // Energy-weighted: (10 * 1 * 100 + 10000 * 0) / (10 + 10000)
        //               = 1000 / 10010 ≈ 0.0999%
        let expected = (10.0 * 1.0 * 100.0) / (10.0 + 10_000.0);
        assert!(
            (network.renewable_percentage - expected).abs() < 1e-6,
            "expected energy-weighted {:.6}, got {:.6}",
            expected,
            network.renewable_percentage
        );
        // The large fossil miner must dominate: nowhere near the ~91% unweighted mean.
        assert!(network.renewable_percentage < 1.0);
    }

    #[test]
    fn test_get_miners_by_classification_actually_filters() {
        let mut api = EnvironmentalApi::new();

        let make_verified = |id: &str, renewable: f64| MinerEnvironmentalInfo {
            miner_id: id.to_string(),
            name: id.to_string(),
            region: Region::NorthAmerica,
            location_verification: None,
            hardware_types: vec![HardwareType::Asic],
            energy_sources: HashMap::new(),
            renewable_percentage: renewable,
            verification: Some(VerificationInfo {
                provider: "Verifier".to_string(),
                date: Utc::now(),
                reference: format!("REF-{id}"),
                status: MinerVerificationStatus::Verified,
            }),
            total_hashrate: 100.0,
            energy_consumption_kwh_day: 1000.0,
            carbon_footprint_tonnes_year: Some(10.0),
            last_update: Utc::now(),
            has_rec_certificates: false,
            has_carbon_offsets: false,
            certificates_url: None,
            rec_certificates: Vec::new(),
            carbon_offsets: Vec::new(),
            environmental_score: None,
            preferred_energy_type: None,
        };

        // A predominantly-renewable verified miner.
        api.register_miner("green", make_verified("green", 100.0))
            .unwrap();
        // A low-renewable (fossil-heavy) verified miner.
        api.register_miner("brown", make_verified("brown", 30.0))
            .unwrap();

        // An UNVERIFIED miner must never be returned for any classification.
        let mut unverified = make_verified("unverified", 100.0);
        unverified.verification = None;
        api.register_miner("unverified", unverified).unwrap();

        // "green" must filter to the >=50% renewable miner only — not the full
        // verified set, and never the unverified miner.
        let green = api.get_miners_by_classification("green");
        assert_eq!(green, vec!["green".to_string()]);

        // Case/separator tolerance and the "renewable" synonym behave identically.
        assert_eq!(api.get_miners_by_classification("Renewable"), green);

        // "fully-renewable" requires >=95%: only the 100% miner qualifies.
        assert_eq!(
            api.get_miners_by_classification("fully-renewable"),
            vec!["green".to_string()]
        );

        // "verified" returns exactly the two verified miners (order-independent).
        let mut verified = api.get_miners_by_classification("verified");
        verified.sort();
        assert_eq!(verified, vec!["brown".to_string(), "green".to_string()]);

        // An unknown classification must NOT silently return everyone — the core
        // bug being fixed. It yields an empty list.
        assert!(api.get_miners_by_classification("totally-bogus").is_empty());
    }
}
