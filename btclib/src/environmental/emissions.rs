use std::collections::HashMap;
use thiserror::Error;
use serde::{Serialize, Deserialize};
use chrono::{DateTime, Utc, NaiveDateTime};
use crate::types::transaction::Transaction;
use crate::config::Config;
use crate::environmental::types::{EmissionsDataSource, EmissionsFactorType, Region as TypesRegion, EmissionFactor as TypesEmissionFactor};
use crate::environmental::oracle::EnvironmentalOracle;
use reqwest::Client;
use tokio::sync::RwLock;
use std::sync::Arc;
use url::Url;
use std::time::{Duration, SystemTime};
use crate::types::block::Block;

/// Environmental data announcement for network sharing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvironmentalDataAnnouncement {
    /// Announcing node ID
    pub node_id: String,
    /// Regional energy sources
    pub energy_sources: Vec<RegionalEnergySource>,
    /// Timestamp of announcement
    pub timestamp: DateTime<Utc>,
    /// Network hashrate information
    pub network_hashrate: f32,
    /// Total energy consumption in MWh
    pub total_energy_consumption_mwh: f32,
    /// Total carbon emissions in tonnes
    pub total_carbon_emissions_tonnes: f32,
    /// Global renewable percentage
    pub global_renewable_percentage: f32,
    /// Regional energy sources (legacy field for compatibility)
    pub regional_energy_sources: Vec<RegionalEnergySource>,
    /// Network data
    pub network_data: NetworkData,
    /// Total energy consumption (legacy field)
    pub energy_consumption: f64,
}

/// Regional energy source information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegionalEnergySource {
    /// Region identifier
    pub region_id: String,
    /// Energy source information (for compatibility)
    pub energy_info: EnergySourceInfo,
    /// Energy sources breakdown
    pub energy_sources: Vec<EnergySourceInfo>,
    /// Energy consumption for this region
    pub energy_consumption: f64,
    /// Region name
    pub name: String,
    /// Percentage of total hashrate
    pub hashrate_percentage: f64,
}

/// Energy source information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnergySourceInfo {
    /// Type of energy source
    pub source_type: EnergySourceType,
    /// Percentage of total energy
    pub percentage: f64,
    /// Whether this source is renewable
    pub is_renewable: bool,
}

/// Network-wide emissions data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkEmissions {
    /// Total energy consumption in MWh
    pub total_energy_mwh: f64,
    /// Total emissions in tonnes CO2e
    pub total_emissions_tons_co2e: f64,
    /// Renewable energy percentage
    pub renewable_percentage: f64,
    /// Emissions per transaction in kg CO2e
    pub emissions_per_tx: f64,
    /// Timestamp of calculation
    pub timestamp: u64,
}

/// Network emissions data (alias for NetworkEmissions for compatibility)
pub type NetworkEmissionsData = NetworkEmissions;

/// Error types for emissions tracking operations
#[derive(Error, Debug, Clone, PartialEq, Eq)]
pub enum EmissionsError {
    #[error("Invalid region code: {0}")]
    InvalidRegion(String),
    
    #[error("Missing emissions factor for region: {0}")]
    MissingEmissionsFactor(String),
    
    #[error("Invalid time range for calculation")]
    InvalidTimeRange,
    
    #[error("Configuration error: {0}")]
    ConfigError(String),
    
    #[error("Data source error: {0}")]
    DataSourceError(String),
    
    #[error("API error: {0}")]
    ApiError(String),
    
    #[error("Network error: {0}")]
    NetworkError(String),
    
    #[error("Verification error: {0}")]
    VerificationError(String),
}

/// Geographic region for emissions tracking
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Region {
    /// ISO country code
    pub country_code: String,
    /// Optional sub-region code (e.g., state, province)
    pub sub_region: Option<String>,
}

impl Region {
    /// Create a new Region with just a country code
    pub fn new(country_code: &str) -> Self {
        Self {
            country_code: country_code.to_string(),
            sub_region: None,
        }
    }

    /// Create a new Region with country code and sub-region
    pub fn with_sub_region(country_code: &str, sub_region: &str) -> Self {
        Self {
            country_code: country_code.to_string(),
            sub_region: Some(sub_region.to_string()),
        }
    }
}

/// Emissions factor for a specific region (gCO2e/kWh)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmissionFactor {
    /// Grid carbon intensity in grams CO2e per kWh
    pub grid_emissions_factor: f64,
    /// Year for this emissions factor
    pub year: Option<u16>,
    /// Source of the emissions factor data
    pub data_source: EmissionsDataSource,
    /// Region name
    pub region_name: String,
    /// Type of emission factor
    pub factor_type: EmissionsFactorType,
    /// Timestamp of the data
    pub timestamp: Option<DateTime<Utc>>,
    /// Confidence level (0-1) of the data
    pub confidence: Option<f64>,
}

/// Source of emissions factor data
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EmissionsFactorSource {
    /// International Energy Agency
    IEA,
    /// US Environmental Protection Agency
    EPA,
    /// European Environment Agency
    EEA,
    /// Other source (custom or user-provided)
    Other,
}

/// Mining hardware type
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum HardwareType {
    /// ASIC miners
    ASIC(String), // Model name
    /// GPU mining rigs
    GPU(String),  // Model name
    /// CPU mining
    CPU(String),  // Model name
    /// Other hardware types
    Other(String),
}

/// Energy efficiency of mining hardware (J/TH)
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Efficiency {
    /// Energy consumption in Joules per Terahash
    pub joules_per_terahash: f64,
    /// Optional typical power usage in watts
    pub typical_power_watts: Option<f64>,
}

/// Represents hashrate in terahashes per second
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct HashRate(pub f64);

/// Mining pool identifier
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PoolId(pub String);

/// Energy source with verification status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerifiedEnergySource {
    /// Type of energy source
    pub source_type: String,
    /// Percentage of total energy
    pub percentage: f64,
    /// Whether this source is renewable
    pub is_renewable: bool,
    /// Whether this source is zero-carbon
    pub is_zero_carbon: bool,
    /// Verification status (none, pending, verified)
    pub verification_status: VerificationStatus,
    /// Verification certificate URL or identifier
    pub verification_reference: Option<String>,
}

/// Mining pool energy source information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoolEnergyInfo {
    /// Percentage of renewable energy used (0-100)
    pub renewable_percentage: f64,
    /// Verified by third party
    pub verified: bool,
    /// Geographic regions where mining occurs
    pub regions: Vec<Region>,
    /// Last updated timestamp
    pub last_updated: DateTime<Utc>,
    /// Detailed breakdown of energy sources
    pub energy_sources: Vec<VerifiedEnergySource>,
    /// REC certificates information
    pub rec_certificates: Option<RECCertificateInfo>,
    /// Carbon offset information
    pub carbon_offsets: Option<CarbonOffsetInfo>,
}

/// Renewable Energy Certificate information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RECCertificateInfo {
    /// Certificate ID or reference
    pub certificate_id: String,
    /// Issuing organization
    pub issuer: String,
    /// Amount in MWh
    pub amount_mwh: f64,
    /// Generation period start
    pub generation_start: DateTime<Utc>,
    /// Generation period end
    pub generation_end: DateTime<Utc>,
    /// Generation location
    pub generation_location: Option<Region>,
    /// Verification status
    pub verification_status: VerificationStatus,
    /// Certificate URL
    pub certificate_url: Option<String>,
}

/// Carbon offset information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CarbonOffsetInfo {
    /// Offset ID or reference
    pub offset_id: String,
    /// Issuing organization
    pub issuer: String,
    /// Amount in tonnes CO2e
    pub amount_tonnes: f64,
    /// Project type
    pub project_type: String,
    /// Project location
    pub project_location: Option<Region>,
    /// Verification status
    pub verification_status: VerificationStatus,
    /// Certificate URL
    pub certificate_url: Option<String>,
}

/// Verification status for environmental claims
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum VerificationStatus {
    /// No verification
    None,
    /// Verification pending
    Pending,
    /// Verification successful
    Verified,
    /// Verification failed
    Failed,
    /// Verification expired
    Expired,
}

/// Emissions measurement for a timeframe
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Emissions {
    /// Total CO2 equivalent emissions in metric tons
    pub tonnes_co2e: f64,
    /// Energy consumption in kilowatt-hours
    pub energy_kwh: f64,
    /// Percentage from renewable sources (if known)
    pub renewable_percentage: Option<f64>,
    /// Location-based emissions (grid average)
    pub location_based_emissions: Option<f64>,
    /// Market-based emissions (with RECs)
    pub market_based_emissions: Option<f64>,
    /// Marginal emissions impact
    pub marginal_emissions_impact: Option<f64>,
    /// Timestamp of calculation
    pub calculation_time: DateTime<Utc>,
    /// Confidence level (0-1) of the calculation
    pub confidence_level: Option<f64>,
}

/// Configuration for emissions tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmissionsConfig {
    /// Whether emissions tracking is enabled
    pub enabled: bool,
    
    /// Default emission factor for unknown regions (gCO2e/kWh)
    pub default_emission_factor: f64,
    
    /// Emissions API endpoint
    pub emissions_api_endpoint: Option<String>,
    
    /// API key for emissions data service
    pub emissions_api_key: Option<String>,
    
    /// Preferred emissions data source
    pub preferred_data_source: Option<EmissionsDataSource>,
    
    /// Whether to use marginal emissions data when available
    pub use_marginal_emissions: bool,
    
    /// Percentage of network hashrate that is known/tracked (0-100)
    pub known_hashrate_percentage: f64,
    
    /// Default network efficiency in Joules per Terahash
    pub default_network_efficiency: f64,
    
    /// Update frequency for emissions data in hours
    pub data_update_frequency_hours: u32,
    
    /// Whether to cache emissions factors locally
    pub cache_emissions_factors: bool,
    
    /// Whether to verify miner location claims
    pub verify_miner_locations: bool,
    
    /// Whether to prioritize REC verification
    pub prioritize_rec_verification: bool,
    
    /// Mining power usage effectiveness factor
    pub mining_pue_factor: f64,
    
    /// Default carbon intensity if no regional data is available
    pub default_carbon_intensity: f64,
    
    /// Default renewable percentage if no regional data is available
    pub default_renewable_percentage: f64,
}

impl Default for EmissionsConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            default_emission_factor: 450.0,  // 450 gCO2e/kWh (global average)
            emissions_api_endpoint: None,
            emissions_api_key: None,
            preferred_data_source: Some(EmissionsDataSource::IEA),
            use_marginal_emissions: false,
            known_hashrate_percentage: 25.0, // Assume we track 25% of network by default
            default_network_efficiency: 50.0, // J/TH
            data_update_frequency_hours: 24,
            cache_emissions_factors: true,
            verify_miner_locations: true,
            prioritize_rec_verification: true,
            mining_pue_factor: 1.0,
            default_carbon_intensity: 475.0,
            default_renewable_percentage: 0.3,
        }
    }
}

/// Emissions tracker for the supernova network
#[derive(Clone)]
pub struct EmissionsTracker {
    /// Network hashrate by geographic region
    region_hashrates: HashMap<Region, HashRate>,
    /// Emissions factors by region (gCO2e/kWh)
    region_emission_factors: HashMap<Region, EmissionFactor>,
    /// Alternative emission factors (marginal, etc.)
    alt_emission_factors: HashMap<(Region, EmissionsFactorType), EmissionFactor>,
    /// Reported renewable energy percentage by mining pool
    pool_energy_info: HashMap<PoolId, PoolEnergyInfo>,
    /// Global configuration for the emissions tracker
    config: EmissionsConfig,
    /// HTTP client for API requests
    http_client: Option<Client>,
    /// Last data update timestamp
    last_update: Option<DateTime<Utc>>,
    /// Environmental oracle system for verification
    oracle_system: Option<Arc<EnvironmentalOracle>>,
}

impl EmissionsTracker {
    /// Create a new emissions tracker with the given configuration
    pub fn new(config: EmissionsConfig) -> Self {
        let http_client = if config.emissions_api_endpoint.is_some() {
            Some(Client::new())
        } else {
            None
        };
        
        Self {
            region_hashrates: HashMap::new(),
            region_emission_factors: HashMap::new(),
            alt_emission_factors: HashMap::new(),
            pool_energy_info: HashMap::new(),
            config,
            http_client,
            last_update: None,
            oracle_system: None,
        }
    }

    /// Create a new emissions tracker with default configuration
    pub fn default() -> Self {
        Self::new(EmissionsConfig::default())
    }
    
    /// Load predefined emissions factors for common regions
    pub fn load_default_emission_factors(&mut self) {
        let factors = EmissionFactor::default_factors();
        
        for factor in factors {
            let region_name = factor.region_name.clone();
            let region_parts: Vec<&str> = region_name.split('-').collect();
            
            let region = if region_parts.len() > 1 {
                Region::with_sub_region(region_parts[0], region_parts[1])
            } else {
                Region::new(&region_name)
            };
            
            self.region_emission_factors.insert(region, factor);
        }
    }
    
    /// Register a mining pool's energy information
    pub fn register_pool_energy_info(&mut self, pool_id: PoolId, info: PoolEnergyInfo) {
        self.pool_energy_info.insert(pool_id, info);
    }
    
    /// Update the hashrate distribution by region
    pub fn update_region_hashrate(&mut self, region: Region, hashrate: HashRate) {
        self.region_hashrates.insert(region, hashrate);
    }
    
    /// Fetch the latest emissions factors from API
    pub async fn fetch_latest_emissions_factors(&mut self) -> Result<(), EmissionsError> {
        if let Some(api_endpoint) = &self.config.emissions_api_endpoint {
            let client = match &self.http_client {
                Some(client) => client,
                None => {
                    self.http_client = Some(Client::new());
                    self.http_client.as_ref().unwrap()
                }
            };
            
            // Build API request URL
            let request_url = format!("{}/emissions-factors", api_endpoint);
            
            // Add API key if configured
            let request = if let Some(api_key) = &self.config.emissions_api_key {
                client.get(&request_url)
                    .header("Authorization", format!("Bearer {}", api_key))
            } else {
                client.get(&request_url)
            };
            
            // Make API request
            let response = match request.send().await {
                Ok(response) => response,
                Err(e) => return Err(EmissionsError::NetworkError(e.to_string())),
            };
            
            if !response.status().is_success() {
                return Err(EmissionsError::ApiError(format!(
                    "API returned error: {}", response.status()
                )));
            }
            
            // Parse response
            let factors: Vec<EmissionFactor> = match response.json().await {
                Ok(factors) => factors,
                Err(e) => return Err(EmissionsError::ApiError(format!(
                    "Failed to parse API response: {}", e
                ))),
            };
            
            // Update emission factors
            for factor in factors.iter() {
                let region_name = factor.region_name.clone();
                let region_parts: Vec<&str> = region_name.split('-').collect();
                
                let region = if region_parts.len() > 1 {
                    Region::with_sub_region(region_parts[0], region_parts[1])
                } else {
                    Region::new(&region_name)
                };
                
                match factor.factor_type {
                    EmissionsFactorType::GridAverage => {
                        self.region_emission_factors.insert(region.clone(), factor.clone());
                    },
                    _ => {
                        self.alt_emission_factors.insert((region, factor.factor_type), factor.clone());
                    }
                }
            }
            
            self.last_update = Some(Utc::now());
            
            Ok(())
        } else {
            Err(EmissionsError::ConfigError(
                "No emissions API endpoint configured".to_string()
            ))
        }
    }
    
    /// Get the best available emissions factor for a region
    fn get_best_emissions_factor(&self, region: &Region) -> Option<&EmissionFactor> {
        // First check if we have a direct match for the region
        if let Some(factor) = self.region_emission_factors.get(region) {
            return Some(factor);
        }
        
        // Try to find a match based on country and sub_region
        if let Some(sub_region) = &region.sub_region {
            // Try with just the country and sub_region
            let parent_region = Region::with_sub_region(&region.country_code, sub_region);
            if let Some(factor) = self.region_emission_factors.get(&parent_region) {
                return Some(factor);
            }
        }
        
        // Try with just the country
        let country_region = Region::new(&region.country_code);
        if let Some(factor) = self.region_emission_factors.get(&country_region) {
            return Some(factor);
        }
        
        // Use global average as last resort
        let global_region = Region::new("GLOBAL");
        self.region_emission_factors.get(&global_region)
    }
    
    /// Get the best available marginal emissions factor for a region
    fn get_marginal_emissions_factor(&self, region: &Region) -> Option<&EmissionFactor> {
        // First check if we have a direct match for the region with marginal type
        if let Some(factor) = self.alt_emission_factors.get(&(region.clone(), EmissionsFactorType::Marginal)) {
            return Some(factor);
        }
        
        // Try to find a match based on country and sub_region
        if let Some(sub_region) = &region.sub_region {
            // Try with just the country and sub_region
            let parent_region = Region::with_sub_region(&region.country_code, sub_region);
            if let Some(factor) = self.alt_emission_factors.get(&(parent_region, EmissionsFactorType::Marginal)) {
                return Some(factor);
            }
        }
        
        // Try with just the country
        let country_region = Region::new(&region.country_code);
        if let Some(factor) = self.alt_emission_factors.get(&(country_region, EmissionsFactorType::Marginal)) {
            return Some(factor);
        }
        
        // Fallback to grid average if no marginal data
        self.get_best_emissions_factor(region)
    }
    
    /// Calculate total network emissions for a given time period using Filecoin Green-inspired methodology
    pub fn calculate_network_emissions(&self, start_time: DateTime<Utc>, end_time: DateTime<Utc>) -> Result<Emissions, EmissionsError> {
        if start_time >= end_time {
            return Err(EmissionsError::InvalidTimeRange);
        }
        
        // Duration in hours
        let duration_hours = (end_time - start_time).num_seconds() as f64 / 3600.0;
        
        // Sum hashrate across all known regions
        let known_hashrate: f64 = self.region_hashrates.values().map(|hr| hr.0).sum();
        
        // Estimate total network hashrate based on known percentage
        let total_hashrate = if self.config.known_hashrate_percentage > 0.0 {
            known_hashrate / (self.config.known_hashrate_percentage / 100.0)
        } else {
            known_hashrate
        };
        
        // Calculate energy consumption using Cambridge methodology
        let energy_per_second = total_hashrate * 1e12 * self.config.default_network_efficiency / 1e9; // Convert J/s to kW
        let total_energy_kwh = energy_per_second * duration_hours;
        
        // Calculate emissions for known regions
        let mut location_based_emissions = 0.0;
        let mut market_based_emissions = 0.0;
        let mut marginal_emissions = 0.0;
        let mut known_energy = 0.0;
        let mut renewable_total = 0.0;
        let mut known_renewable_energy = 0.0;
        
        // Track confidence levels
        let mut confidence_sum = 0.0;
        let mut confidence_count = 0;
        
        for (region, hashrate) in &self.region_hashrates {
            let region_hashrate_percentage = hashrate.0 / known_hashrate;
            let region_energy = total_energy_kwh * region_hashrate_percentage;
            known_energy += region_energy;
            
            // Get emission factor for this region
            let emission_factor = match self.get_best_emissions_factor(region) {
                Some(factor) => factor.grid_emissions_factor * 1000.0, // Convert tonnes/MWh to kg/kWh
                None => self.config.default_emission_factor / 1000.0, // Convert g to kg
            };
            
            // Calculate location-based emissions (without RECs)
            let region_location_emissions = region_energy * emission_factor;
            location_based_emissions += region_location_emissions;
            
            // If enabled, get marginal emissions factor
            if self.config.use_marginal_emissions {
                let marginal_factor = match self.get_marginal_emissions_factor(region) {
                    Some(factor) => factor.grid_emissions_factor * 1000.0, // Convert tonnes/MWh to kg/kWh
                    None => emission_factor, // Fall back to average if no marginal data
                };
                
                let region_marginal_emissions = region_energy * marginal_factor;
                marginal_emissions += region_marginal_emissions;
            }
            
            // Check for confidence levels
            let confidence = 0.7; // Default confidence level
            confidence_sum += confidence;
            confidence_count += 1;
            
            // Check for renewable percentage data for mining pools in this region
            let region_pools: Vec<_> = self.pool_energy_info
                .iter()
                .filter(|(_, info)| info.regions.contains(region))
                .collect();
                
            if !region_pools.is_empty() {
                let avg_renewable = region_pools.iter()
                    .map(|(_, info)| info.renewable_percentage)
                    .sum::<f64>() / region_pools.len() as f64;
                    
                renewable_total += region_hashrate_percentage * avg_renewable;
                known_renewable_energy += region_energy * (avg_renewable / 100.0);
                
                // Calculate market-based emissions (with RECs)
                // For pools with verified RECs, we count renewable energy as zero emissions
                let mut region_market_emissions = region_location_emissions;
                
                for (_, info) in &region_pools {
                    if let Some(rec_info) = &info.rec_certificates {
                        if rec_info.verification_status == VerificationStatus::Verified {
                            // Reduce emissions based on verified REC percentage
                            let rec_percentage = (rec_info.amount_mwh * 1000.0 / region_energy).min(1.0);
                            region_market_emissions *= (1.0 - rec_percentage);
                        }
                    }
                }
                
                market_based_emissions += region_market_emissions;
            } else {
                // Without pools, market-based = location-based
                market_based_emissions += region_location_emissions;
            }
        }
        
        // Calculate unknown emissions using default factor
        let unknown_energy = total_energy_kwh - known_energy;
        let unknown_emissions = unknown_energy * (self.config.default_emission_factor / 1000.0);
        
        let total_location_emissions_tonnes = (location_based_emissions + unknown_emissions) / 1000.0;
        let total_market_emissions_tonnes = (market_based_emissions + unknown_emissions) / 1000.0;
        let total_marginal_emissions_tonnes = if self.config.use_marginal_emissions {
            (marginal_emissions + unknown_emissions) / 1000.0
        } else {
            total_location_emissions_tonnes
        };
        
        // Calculate overall renewable percentage if we have data
        let renewable_percentage = if known_energy > 0.0 {
            Some((known_renewable_energy / total_energy_kwh) * 100.0)
        } else {
            None
        };
        
        // Calculate confidence level
        let confidence_level = if confidence_count > 0 {
            Some(confidence_sum / confidence_count as f64)
        } else {
            None
        };
        
        Ok(Emissions {
            tonnes_co2e: total_location_emissions_tonnes,
            energy_kwh: total_energy_kwh,
            renewable_percentage,
            location_based_emissions: Some(total_location_emissions_tonnes),
            market_based_emissions: Some(total_market_emissions_tonnes),
            marginal_emissions_impact: if self.config.use_marginal_emissions {
                Some(total_marginal_emissions_tonnes)
            } else {
                None
            },
            calculation_time: Utc::now(),
            confidence_level,
        })
    }
    
    /// Verify renewable energy certificate claims through oracle consensus
    pub fn verify_rec_claim(&self, certificate: &RECCertificateInfo) -> VerificationStatus {
        // Use the oracle system for real verification
        if let Some(oracle) = &self.oracle_system {
            match oracle.verify_rec_certificate(certificate) {
                Ok(status) => status,
                Err(e) => {
                    log::error!("Oracle verification failed: {}", e);
                    VerificationStatus::Failed
                }
            }
        } else {
            // Fallback to basic validation if oracle system not available
            let now = Utc::now();
            if certificate.generation_end < now - chrono::Duration::days(365) {
                return VerificationStatus::Expired;
            }
            
            // Without oracle, can only do basic checks
            if certificate.certificate_url.is_some() && !certificate.certificate_id.is_empty() {
                VerificationStatus::Pending // Cannot verify without oracle
            } else {
                VerificationStatus::Failed
            }
        }
    }
    
    /// Estimate emissions for a single transaction
    pub fn estimate_transaction_emissions(&self, transaction: &Transaction) -> Result<Emissions, EmissionsError> {
        // Get the current network energy intensity per transaction
        let avg_tx_energy_kwh = self.estimate_transaction_energy(transaction)?;
        
        // Get the weighted emission factor based on hashrate distribution
        let (weighted_emission_factor, confidence_level) = self.calculate_weighted_emission_factor();
        
        let emissions_tonnes = avg_tx_energy_kwh * weighted_emission_factor / 1000000.0; // Convert g to tonnes
        
        // Calculate market-based emissions (considering RECs)
        let renewable_percentage = self.calculate_network_renewable_percentage();
        let market_based_emissions = emissions_tonnes * (1.0 - (renewable_percentage / 100.0));
        
        // Marginal emissions calculation if enabled
        let marginal_emissions = if self.config.use_marginal_emissions {
            let (weighted_marginal_factor, _) = self.calculate_weighted_marginal_factor();
            Some(avg_tx_energy_kwh * weighted_marginal_factor / 1000000.0)
        } else {
            None
        };
        
        Ok(Emissions {
            tonnes_co2e: emissions_tonnes,
            energy_kwh: avg_tx_energy_kwh,
            renewable_percentage: Some(renewable_percentage),
            location_based_emissions: Some(emissions_tonnes),
            market_based_emissions: Some(market_based_emissions),
            marginal_emissions_impact: marginal_emissions,
            calculation_time: Utc::now(),
            confidence_level: Some(confidence_level),
        })
    }
    
    /// Estimate energy consumption for a transaction
    fn estimate_transaction_energy(&self, transaction: &Transaction) -> Result<f64, EmissionsError> {
        // This is a simplified model for Phase 1
        // In a real implementation, would consider tx weight, fees, etc.
        
        // Basic transaction energy estimate
        let tx_size_bytes = 250.0; // Conservative average
        let network_hashrate: f64 = self.region_hashrates.values().map(|hr| hr.0).sum();
        
        // Energy proportional to transaction size and inversely to hashrate
        let energy = tx_size_bytes * self.config.default_network_efficiency / (1000.0 * network_hashrate.max(1.0));
        
        Ok(energy)
    }
    
    /// Calculate weighted emission factor based on hashrate distribution
    fn calculate_weighted_emission_factor(&self) -> (f64, f64) {
        let mut weighted_emission_factor = 0.0;
        let mut total_weight = 0.0;
        let mut confidence_sum = 0.0;
        let mut confidence_count = 0;
        
        for (region, hashrate) in &self.region_hashrates {
            if let Some(factor) = self.get_best_emissions_factor(region) {
                weighted_emission_factor += factor.grid_emissions_factor * 1000.0 * hashrate.0; // Convert to g/kWh
                total_weight += hashrate.0;
                
                let confidence = factor.confidence.unwrap_or(0.7); // Default confidence level
                confidence_sum += confidence;
                confidence_count += 1;
            }
        }
        
        let final_factor = if total_weight > 0.0 {
            weighted_emission_factor / total_weight
        } else {
            self.config.default_emission_factor
        };
        
        let confidence = if confidence_count > 0 {
            confidence_sum / confidence_count as f64
        } else {
            0.7 // Default confidence level
        };
        
        (final_factor, confidence)
    }
    
    /// Calculate weighted marginal emission factor based on hashrate distribution
    fn calculate_weighted_marginal_factor(&self) -> (f64, f64) {
        let mut weighted_emission_factor = 0.0;
        let mut total_weight = 0.0;
        let mut confidence_sum = 0.0;
        let mut confidence_count = 0;
        
        for (region, hashrate) in &self.region_hashrates {
            if let Some(factor) = self.get_marginal_emissions_factor(region) {
                weighted_emission_factor += factor.grid_emissions_factor * 1000.0 * hashrate.0; // Convert to g/kWh
                total_weight += hashrate.0;
                
                let confidence = factor.confidence.unwrap_or(0.6); // Lower confidence for marginal by default
                confidence_sum += confidence;
                confidence_count += 1;
            }
        }
        
        let final_factor = if total_weight > 0.0 {
            weighted_emission_factor / total_weight
        } else {
            self.config.default_emission_factor * 1.1 // Marginal typically higher than average
        };
        
        let confidence = if confidence_count > 0 {
            confidence_sum / confidence_count as f64
        } else {
            0.6 // Lower confidence for marginal by default
        };
        
        (final_factor, confidence)
    }
    
    /// Calculate network-wide renewable energy percentage
    pub fn calculate_network_renewable_percentage(&self) -> f64 {
        let mut total_renewable = 0.0;
        let mut total_hashrate = 0.0;
        
        for (pool_id, info) in &self.pool_energy_info {
            // Calculate weighted average based on regions and their hashrates
            let mut pool_hashrate = 0.0;
            
            for region in &info.regions {
                if let Some(hashrate) = self.region_hashrates.get(region) {
                    pool_hashrate += hashrate.0;
                }
            }
            
            // Default to equal distribution if no hashrate data
            if pool_hashrate == 0.0 {
                continue;
            }
            
            total_renewable += pool_hashrate * info.renewable_percentage;
            total_hashrate += pool_hashrate;
        }
        
        if total_hashrate > 0.0 {
            total_renewable / total_hashrate
        } else {
            0.0
        }
    }
    
    /// Update configuration
    pub fn update_config(&mut self, config: EmissionsConfig) {
        self.config = config;
    }

    /// Get network-wide carbon intensity
    pub fn get_network_carbon_intensity(&self) -> Result<f64, EmissionsError> {
        // Calculate weighted average carbon intensity across all regions
        let mut total_weighted_intensity = 0.0;
        let mut total_hashrate = 0.0;
        
        for (region, hashrate) in &self.region_hashrates {
            if let Some(emission_factor) = self.region_emission_factors.get(region) {
                total_weighted_intensity += emission_factor.grid_emissions_factor * hashrate.0;
                total_hashrate += hashrate.0;
            }
        }
        
        if total_hashrate > 0.0 {
            Ok(total_weighted_intensity / total_hashrate)
        } else {
            Ok(self.config.default_carbon_intensity)
        }
    }
    
    /// Get network-wide hashrate in TH/s
    pub fn get_network_hashrate(&self) -> Result<f64, EmissionsError> {
        let total_hashrate: f64 = self.region_hashrates.values()
            .map(|hashrate| hashrate.0)
            .sum();
        
        if total_hashrate > 0.0 {
            Ok(total_hashrate)
        } else {
            // Return estimated network hashrate if no regions registered
            Ok(200_000_000.0) // Approximate current network hashrate in TH/s
        }
    }

    /// Set the oracle system for verification
    pub fn set_oracle_system(&mut self, oracle: Arc<EnvironmentalOracle>) {
        self.oracle_system = Some(oracle);
    }
}

impl EmissionFactor {
    /// Create default emission factors for common regions (compatibility override)
    pub fn default_factors() -> Vec<Self> {
        vec![
            // Global average
            Self {
                grid_emissions_factor: 0.45, // tonnes CO2e per MWh
                region_name: "GLOBAL".to_string(),
                data_source: EmissionsDataSource::IEA,
                factor_type: EmissionsFactorType::GridAverage,
                year: Some(2023),
                timestamp: None,
                confidence: None,
            },
            // USA
            Self {
                grid_emissions_factor: 0.38,
                region_name: "US".to_string(),
                data_source: EmissionsDataSource::EPA,
                factor_type: EmissionsFactorType::GridAverage,
                year: Some(2023),
                timestamp: None,
                confidence: None,
            },
            // Europe
            Self {
                grid_emissions_factor: 0.275,
                region_name: "EU".to_string(),
                data_source: EmissionsDataSource::EEA,
                factor_type: EmissionsFactorType::GridAverage,
                year: Some(2023),
                timestamp: None,
                confidence: None,
            },
            // China
            Self {
                grid_emissions_factor: 0.55,
                region_name: "CN".to_string(),
                data_source: EmissionsDataSource::IEA,
                factor_type: EmissionsFactorType::GridAverage,
                year: Some(2023),
                timestamp: None,
                confidence: None,
            },
            // Canada
            Self {
                grid_emissions_factor: 0.12,
                region_name: "CA".to_string(),
                data_source: EmissionsDataSource::IEA,
                factor_type: EmissionsFactorType::GridAverage,
                year: Some(2023),
                timestamp: None,
                confidence: None,
            },
        ]
    }
}

/// Emissions calculator trait for estimating carbon emissions
pub trait EmissionCalculator {
    /// Calculate the carbon emissions for a time period
    fn calculate_emissions(&self, start_time: DateTime<Utc>, end_time: DateTime<Utc>) -> Result<Emissions, EmissionsError>;
    
    /// Add a region to track
    fn add_region(&mut self, region: Region, hashrate: HashRate);
    
    /// Get the total hashrate across all tracked regions
    fn total_hashrate(&self) -> HashRate;
}

// Implement EmissionCalculator for EmissionsTracker
impl EmissionCalculator for EmissionsTracker {
    fn calculate_emissions(&self, start_time: DateTime<Utc>, end_time: DateTime<Utc>) -> Result<Emissions, EmissionsError> {
        self.calculate_network_emissions(start_time, end_time)
    }
    
    fn add_region(&mut self, region: Region, hashrate: HashRate) {
        self.update_region_hashrate(region, hashrate);
    }
    
    fn total_hashrate(&self) -> HashRate {
        HashRate(self.region_hashrates.values().map(|hr| hr.0).sum())
    }
}

// Add Default implementation for EmissionsTracker
impl Default for EmissionsTracker {
    fn default() -> Self {
        Self::new(EmissionsConfig::default())
    }
}

/// Calculator for network-wide emissions
pub struct EmissionsCalculator {
    /// Current network hashrate in TH/s
    total_hashrate: f64,
    /// Carbon intensity in kgCO2e/kWh
    carbon_intensity: f64,
    /// Average energy efficiency in J/TH
    energy_efficiency: f64,
    /// Renewable energy percentage (0-100)
    renewable_percentage: f64,
}

impl EmissionsCalculator {
    /// Create a new emissions calculator
    pub fn new() -> Self {
        Self {
            total_hashrate: 100_000.0, // Default 100 EH/s (100,000 TH/s)
            carbon_intensity: 0.5,     // Default 500 gCO2e/kWh
            energy_efficiency: 50.0,   // Default 50 J/TH
            renewable_percentage: 30.0, // Default 30% renewable
        }
    }
    
    /// Set the network hashrate
    pub fn set_hashrate(&mut self, hashrate_th_s: f64) {
        self.total_hashrate = hashrate_th_s;
    }
    
    /// Set the carbon intensity
    pub fn set_carbon_intensity(&mut self, intensity_kg_kwh: f64) {
        self.carbon_intensity = intensity_kg_kwh;
    }
    
    /// Set the energy efficiency
    pub fn set_energy_efficiency(&mut self, efficiency_j_th: f64) {
        self.energy_efficiency = efficiency_j_th;
    }
    
    /// Set the renewable percentage
    pub fn set_renewable_percentage(&mut self, percentage: f64) {
        self.renewable_percentage = percentage.max(0.0).min(100.0);
    }
    
    /// Calculate network emissions for a time period
    pub fn calculate_network_emissions(&self) -> Result<NetworkEmissions, String> {
        // Calculate energy consumption
        // Power (W) = Hashrate (TH/s) * Energy Efficiency (J/TH)
        let power_watts = self.total_hashrate * self.energy_efficiency;
        
        // Energy over 24 hours (kWh) = Power (W) * 24 / 1000
        let daily_energy_kwh = power_watts * 24.0 / 1000.0;
        
        // Convert to MWh for the API
        let daily_energy_mwh = daily_energy_kwh / 1000.0;
        
        // Calculate emissions accounting for renewables
        // Non-renewable percentage = 100% - Renewable percentage
        let non_renewable_percentage = (100.0 - self.renewable_percentage) / 100.0;
        
        // Non-renewable energy (kWh) = Total energy (kWh) * Non-renewable percentage
        let non_renewable_energy_kwh = daily_energy_kwh * non_renewable_percentage;
        
        // Emissions (kg CO2e) = Non-renewable energy (kWh) * Carbon intensity (kg CO2e/kWh)
        let daily_emissions_kg = non_renewable_energy_kwh * self.carbon_intensity;
        
        // Convert to tonnes for the API
        let daily_emissions_tonnes = daily_emissions_kg / 1000.0;
        
        // Assume 1 million transactions per day for emissions per transaction
        let tx_per_day = 1_000_000.0;
        let emissions_per_tx = daily_emissions_kg / tx_per_day;
        
        Ok(NetworkEmissions {
            total_energy_mwh: daily_energy_mwh,
            total_emissions_tons_co2e: daily_emissions_tonnes,
            renewable_percentage: self.renewable_percentage,
            emissions_per_tx,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        })
    }
}

/// Energy source types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EnergySourceType {
    Solar,
    Wind,
    Hydro,
    Nuclear,
    Gas,
    Coal,
    Oil,
    Geothermal,
    Biomass,
    Other,
}

/// Carbon intensity constants (gCO2e/kWh)
pub const CARBON_INTENSITY_SOLAR: f64 = 41.0;
pub const CARBON_INTENSITY_WIND: f64 = 11.0;
pub const CARBON_INTENSITY_HYDRO: f64 = 24.0;
pub const CARBON_INTENSITY_NUCLEAR: f64 = 12.0;
pub const CARBON_INTENSITY_GAS: f64 = 490.0;
pub const CARBON_INTENSITY_COAL: f64 = 820.0;
pub const CARBON_INTENSITY_OIL: f64 = 650.0;
pub const CARBON_INTENSITY_GEOTHERMAL: f64 = 38.0;
pub const CARBON_INTENSITY_BIOMASS: f64 = 230.0;
pub const CARBON_INTENSITY_OTHER: f64 = 500.0;

/// Default carbon intensity if no specific data is available (gCO2e/kWh)
pub const DEFAULT_CARBON_INTENSITY: f64 = 475.0;

/// Average energy consumption per hash calculation (J/hash)
pub const ENERGY_PER_HASH: f64 = 0.0000015; // 1.5 μJ/hash for modern ASIC miners

/// Network hashrate estimate (in hashes per second)
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct NetworkHashrate {
    /// Timestamp when this data was recorded
    pub timestamp: DateTime<Utc>,
    /// Estimated network hashrate (hashes/second)
    pub hashrate: f64,
    /// Moving average over last 24 hours
    pub moving_average_24h: f64,
    /// Confidence factor (0.0-1.0)
    pub confidence: f64,
}

/// Regional energy data for mining operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegionalEnergyData {
    /// Region identifier (ISO 3166-1 alpha-2 country code)
    pub region_id: String,
    /// Name of the region
    pub name: String,
    /// Estimated percentage of total hashrate
    pub hashrate_percentage: f64,
    /// Energy source breakdown for this region
    pub energy_sources: HashMap<EnergySourceType, f64>,
    /// Carbon intensity for this region (gCO2e/kWh)
    pub carbon_intensity: f64,
    /// Percentage of renewable energy
    pub renewable_percentage: f64,
    /// Last updated timestamp
    pub last_updated: DateTime<Utc>,
}

/// Environmental data for a block
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockEnvironmentalData {
    /// Block hash
    pub block_hash: [u8; 32],
    /// Block height
    pub height: u64,
    /// Timestamp when block was mined
    pub timestamp: DateTime<Utc>,
    /// Estimated energy consumption in kWh
    pub energy_consumption: f64,
    /// Estimated carbon emissions in gCO2e
    pub carbon_emissions: f64,
    /// Average network renewable percentage at mining time
    pub renewable_percentage: f64,
    /// Regional breakdown of energy consumption
    pub regional_breakdown: Option<Vec<RegionalContribution>>,
}

/// Regional contribution to block's environmental impact
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegionalContribution {
    /// Region identifier
    pub region_id: String,
    /// Probability this region mined the block
    pub probability: f64,
    /// Energy consumption if mined in this region
    pub energy_consumption: f64,
    /// Carbon emissions if mined in this region
    pub carbon_emissions: f64,
    /// Renewable percentage in this region
    pub renewable_percentage: f64,
}

/// Transaction environmental data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionEnvironmentalData {
    /// Transaction hash
    pub tx_hash: [u8; 32],
    /// Estimated energy consumption in kWh
    pub energy_consumption: f64,
    /// Estimated carbon emissions in gCO2e
    pub carbon_emissions: f64,
    /// Renewable energy percentage
    pub renewable_percentage: f64,
    /// Timestamp
    pub timestamp: DateTime<Utc>,
}

/// Emissions registry for tracking energy usage and carbon emissions
pub struct EmissionsRegistry {
    /// Configuration for emissions calculations
    config: EmissionsConfig,
    /// Current network hashrate data
    network_hashrate: Arc<RwLock<NetworkHashrate>>,
    /// Regional energy data
    regional_data: Arc<RwLock<HashMap<String, RegionalEnergyData>>>,
    /// Recent blocks environmental data
    block_data: Arc<RwLock<HashMap<[u8; 32], BlockEnvironmentalData>>>,
    /// Transaction environmental data
    transaction_data: Arc<RwLock<HashMap<[u8; 32], TransactionEnvironmentalData>>>,
    /// Global carbon intensity (gCO2e/kWh)
    global_carbon_intensity: Arc<RwLock<f64>>,
    /// Global renewable percentage
    global_renewable_percentage: Arc<RwLock<f64>>,
}

impl EmissionsRegistry {
    /// Create a new emissions registry
    pub fn new(config: EmissionsConfig) -> Self {
        // Initialize with default values
        let current_time = Utc::now();
        let default_hashrate = NetworkHashrate {
            timestamp: current_time,
            hashrate: 100.0e18, // 100 EH/s as a default value
            moving_average_24h: 100.0e18,
            confidence: 0.9,
        };
        
        Self {
            config,
            network_hashrate: Arc::new(RwLock::new(default_hashrate)),
            regional_data: Arc::new(RwLock::new(HashMap::new())),
            block_data: Arc::new(RwLock::new(HashMap::new())),
            transaction_data: Arc::new(RwLock::new(HashMap::new())),
            global_carbon_intensity: Arc::new(RwLock::new(DEFAULT_CARBON_INTENSITY)),
            global_renewable_percentage: Arc::new(RwLock::new(0.3)), // Assume 30% renewable by default
        }
    }
    
    /// Update the network hashrate estimate
    pub async fn update_network_hashrate(&self, hashrate: f64, confidence: f64) {
        let mut network_hashrate = self.network_hashrate.write().await;
        let current_time = Utc::now();
        
        // Calculate a simple exponential moving average for 24h
        let alpha = 0.1; // Smoothing factor
        let moving_average = network_hashrate.moving_average_24h * (1.0 - alpha) + hashrate * alpha;
        
        *network_hashrate = NetworkHashrate {
            timestamp: current_time,
            hashrate,
            moving_average_24h: moving_average,
            confidence,
        };
    }
    
    /// Update regional energy data
    pub async fn update_regional_data(&self, region_id: String, data: RegionalEnergyData) {
        let mut regional_data = self.regional_data.write().await;
        regional_data.insert(region_id, data);
        
        // Recalculate global values
        drop(regional_data); // Release lock before recalculating
        self.recalculate_global_values().await;
    }
    
    /// Process a new block announcement to calculate its environmental impact
    pub async fn process_block(&self, block: &Block, height: u64, difficulty: f64) -> BlockEnvironmentalData {
        let timestamp = match chrono::DateTime::from_timestamp(block.header.timestamp as i64, 0) {
            Some(dt) => dt,
            None => Utc::now(),
        };
        
        // Calculate energy consumption based on difficulty
        let energy_consumption = self.calculate_block_energy(difficulty).await;
        
        // Get global carbon intensity and renewable percentage
        let carbon_intensity = *self.global_carbon_intensity.read().await;
        let renewable_percentage = *self.global_renewable_percentage.read().await;
        
        // Calculate carbon emissions
        let carbon_emissions = energy_consumption * carbon_intensity;
        
        // Calculate regional breakdown if regional data is available
        let regional_breakdown = self.calculate_regional_breakdown(energy_consumption).await;
        
        let block_env_data = BlockEnvironmentalData {
            block_hash: block.hash(),
            height,
            timestamp,
            energy_consumption,
            carbon_emissions,
            renewable_percentage,
            regional_breakdown,
        };
        
        // Store the data
        let mut block_data = self.block_data.write().await;
        block_data.insert(block.hash(), block_env_data.clone());
        
        block_env_data
    }
    
    /// Process a transaction to calculate its environmental impact
    pub async fn process_transaction(&self, tx: &Transaction, block_env_data: Option<&BlockEnvironmentalData>) -> TransactionEnvironmentalData {
        let current_time = Utc::now();
        
        // If we have block environmental data, use that as a base for calculations
        if let Some(block_data) = block_env_data {
            // Calculate transaction's share of the block's environmental impact
            // This is a simplified approach; a more sophisticated model would account for tx size, fees, etc.
            let tx_count = 1.max(tx.calculate_size() as u64); // Avoid division by zero
            let energy_per_tx = block_data.energy_consumption / tx_count as f64;
            
            let tx_env_data = TransactionEnvironmentalData {
                tx_hash: tx.hash(),
                energy_consumption: energy_per_tx,
                carbon_emissions: energy_per_tx * (*self.global_carbon_intensity.read().await),
                renewable_percentage: block_data.renewable_percentage,
                timestamp: block_data.timestamp,
            };
            
            // Store the data
            let mut tx_data = self.transaction_data.write().await;
            tx_data.insert(tx.hash(), tx_env_data.clone());
            
            tx_env_data
        } else {
            // If no block data, estimate based on global averages
            let tx_size = tx.calculate_size() as f64;
            let avg_tx_size = 250.0; // Assume average tx size of 250 bytes
            let base_energy = 0.0001; // Base energy in kWh for an average tx
            
            let energy_consumption = base_energy * (tx_size / avg_tx_size);
            let carbon_intensity = *self.global_carbon_intensity.read().await;
            let renewable_percentage = *self.global_renewable_percentage.read().await;
            
            let tx_env_data = TransactionEnvironmentalData {
                tx_hash: tx.hash(),
                energy_consumption,
                carbon_emissions: energy_consumption * carbon_intensity,
                renewable_percentage,
                timestamp: current_time,
            };
            
            // Store the data
            let mut tx_data = self.transaction_data.write().await;
            tx_data.insert(tx.hash(), tx_env_data.clone());
            
            tx_env_data
        }
    }
    
    /// Get environmental data for a block
    pub async fn get_block_environmental_data(&self, block_hash: &[u8; 32]) -> Option<BlockEnvironmentalData> {
        let block_data = self.block_data.read().await;
        block_data.get(block_hash).cloned()
    }
    
    /// Get environmental data for a transaction
    pub async fn get_transaction_environmental_data(&self, tx_hash: &[u8; 32]) -> Option<TransactionEnvironmentalData> {
        let tx_data = self.transaction_data.read().await;
        tx_data.get(tx_hash).cloned()
    }
    
    /// Calculate energy consumption for a block based on its difficulty
    async fn calculate_block_energy(&self, difficulty: f64) -> f64 {
        // Get current network hashrate
        let _network_hashrate = self.network_hashrate.read().await;
        
        // Calculate expected hashes to find a block with this difficulty
        let expected_hashes = difficulty * 2.0f64.powi(32);
        
        // Calculate energy in joules: hashes * energy per hash
        let energy_joules = expected_hashes * ENERGY_PER_HASH;
        
        // Convert to kWh (1 kWh = 3.6 million joules)
        let energy_kwh = energy_joules / 3.6e6;
        
        // Apply PUE (Power Usage Effectiveness) factor to account for cooling/infrastructure
        let pue_factor = self.config.mining_pue_factor;
        
        energy_kwh * pue_factor
    }
    
    /// Calculate regional energy breakdown for a block
    async fn calculate_regional_breakdown(&self, total_energy: f64) -> Option<Vec<RegionalContribution>> {
        let regional_data = self.regional_data.read().await;
        
        if regional_data.is_empty() {
            return None;
        }
        
        let mut contributions = Vec::new();
        
        for (region_id, data) in regional_data.iter() {
            // Calculate energy consumption for this region
            let energy = total_energy * data.hashrate_percentage;
            
            // Calculate carbon emissions
            let emissions = energy * data.carbon_intensity;
            
            contributions.push(RegionalContribution {
                region_id: region_id.clone(),
                probability: data.hashrate_percentage,
                energy_consumption: energy,
                carbon_emissions: emissions,
                renewable_percentage: data.renewable_percentage,
            });
        }
        
        Some(contributions)
    }
    
    /// Import regional energy data from a network announcement
    pub async fn import_regional_data(&self, announcement: &EnvironmentalDataAnnouncement) {
        let mut regional_data = self.regional_data.write().await;
        
        for region in &announcement.regional_energy_sources {
            // Convert energy source info to our internal format
            let mut energy_sources = HashMap::new();
            for source in &region.energy_sources {
                energy_sources.insert(source.source_type, source.percentage as f64);
            }
            
            // Calculate carbon intensity for this region
            let carbon_intensity = self.calculate_carbon_intensity(&energy_sources);
            
            // Calculate renewable percentage
            let renewable_percentage = self.calculate_renewable_percentage(&energy_sources);
            
            let data = RegionalEnergyData {
                region_id: region.region_id.clone(),
                name: region.region_id.clone(), // Use code as name for now
                hashrate_percentage: region.energy_consumption / announcement.energy_consumption,
                energy_sources,
                carbon_intensity,
                renewable_percentage,
                last_updated: Utc::now(),
            };
            
            regional_data.insert(region.region_id.clone(), data);
        }
        
        // Recalculate global values after updating regional data
        drop(regional_data); // Release lock before recalculating
        self.recalculate_global_values().await;
    }
    
    /// Calculate carbon intensity for a given energy source mix
    fn calculate_carbon_intensity(&self, energy_sources: &HashMap<EnergySourceType, f64>) -> f64 {
        let mut total_intensity = 0.0;
        let mut total_percentage = 0.0;
        
        for (source_type, percentage) in energy_sources {
            let intensity = match source_type {
                EnergySourceType::Solar => CARBON_INTENSITY_SOLAR,
                EnergySourceType::Wind => CARBON_INTENSITY_WIND,
                EnergySourceType::Hydro => CARBON_INTENSITY_HYDRO,
                EnergySourceType::Nuclear => CARBON_INTENSITY_NUCLEAR,
                EnergySourceType::Gas => CARBON_INTENSITY_GAS,
                EnergySourceType::Coal => CARBON_INTENSITY_COAL,
                EnergySourceType::Oil => CARBON_INTENSITY_OIL,
                EnergySourceType::Geothermal => CARBON_INTENSITY_GEOTHERMAL,
                EnergySourceType::Biomass => CARBON_INTENSITY_BIOMASS,
                EnergySourceType::Other => CARBON_INTENSITY_OTHER,
            };
            
            total_intensity += intensity * percentage;
            total_percentage += percentage;
        }
        
        if total_percentage > 0.0 {
            total_intensity / total_percentage
        } else {
            DEFAULT_CARBON_INTENSITY
        }
    }
    
    /// Calculate renewable percentage for a given energy source mix
    fn calculate_renewable_percentage(&self, energy_sources: &HashMap<EnergySourceType, f64>) -> f64 {
        let mut renewable_percentage = 0.0;
        let mut total_percentage = 0.0;
        
        for (source_type, percentage) in energy_sources {
            match source_type {
                EnergySourceType::Solar |
                EnergySourceType::Wind |
                EnergySourceType::Hydro |
                EnergySourceType::Geothermal => {
                    renewable_percentage += percentage;
                }
                _ => {} // Non-renewable sources
            };
            
            total_percentage += percentage;
        }
        
        if total_percentage > 0.0 {
            renewable_percentage / total_percentage
        } else {
            0.0
        }
    }
    
    /// Recalculate global carbon intensity and renewable percentage
    async fn recalculate_global_values(&self) {
        let regional_data = self.regional_data.read().await;
        
        if regional_data.is_empty() {
            return;
        }
        
        let mut total_hashrate_percentage = 0.0;
        let mut weighted_carbon_intensity = 0.0;
        let mut weighted_renewable_percentage = 0.0;
        
        for data in regional_data.values() {
            weighted_carbon_intensity += data.carbon_intensity * data.hashrate_percentage;
            weighted_renewable_percentage += data.renewable_percentage * data.hashrate_percentage;
            total_hashrate_percentage += data.hashrate_percentage;
        }
        
        if total_hashrate_percentage > 0.0 {
            let global_carbon_intensity = weighted_carbon_intensity / total_hashrate_percentage;
            let global_renewable = weighted_renewable_percentage / total_hashrate_percentage;
            
            *self.global_carbon_intensity.write().await = global_carbon_intensity;
            *self.global_renewable_percentage.write().await = global_renewable;
        }
    }
    
    /// Create an environmental data announcement for network broadcast
    pub async fn create_environmental_announcement(&self) -> EnvironmentalDataAnnouncement {
        let regional_data = self.regional_data.read().await;
        let network_hashrate = self.network_hashrate.read().await;
        
        // Calculate total energy consumption over the last day
        // This is a simplified estimate: hashrate * energy_per_hash * seconds_in_day
        let seconds_in_day = 24.0 * 60.0 * 60.0;
        let total_energy = network_hashrate.moving_average_24h * ENERGY_PER_HASH * seconds_in_day / 3.6e6; // Convert to kWh
        
        // Convert regional data to announcement format
        let mut regions = Vec::new();
        for (region_id, data) in regional_data.iter() {
            let mut energy_sources = Vec::new();
            for (source_type, percentage) in &data.energy_sources {
                energy_sources.push(EnergySourceInfo {
                    source_type: *source_type,
                    percentage: *percentage,
                    is_renewable: match source_type {
                        EnergySourceType::Solar | EnergySourceType::Wind | 
                        EnergySourceType::Hydro | EnergySourceType::Geothermal => true,
                        _ => false,
                    },
                });
            }
            
            regions.push(RegionalEnergySource {
                region_id: region_id.clone(),
                energy_info: EnergySourceInfo {
                    source_type: EnergySourceType::Other,
                    percentage: 1.0,
                    is_renewable: false,
                },
                energy_sources,
                energy_consumption: data.hashrate_percentage * total_energy,
                name: region_id.clone(),
                hashrate_percentage: data.hashrate_percentage,
            });
        }
        
        let carbon_emissions = total_energy * (*self.global_carbon_intensity.read().await);
        
        EnvironmentalDataAnnouncement {
            node_id: "local".to_string(),
            energy_sources: regions.clone(),
            timestamp: Utc::now(),
            network_hashrate: network_hashrate.hashrate as f32,
            total_energy_consumption_mwh: total_energy as f32,
            total_carbon_emissions_tonnes: carbon_emissions as f32,
            global_renewable_percentage: (*self.global_renewable_percentage.read().await) as f32,
            regional_energy_sources: regions,
            network_data: NetworkData {
                total_power_mw: network_hashrate.hashrate / 1_000_000.0, // Convert to MW
                efficiency_j_th: 50.0, // Default efficiency
                last_updated: Utc::now(),
            },
            energy_consumption: total_energy,
        }
    }
    
    /// Get global statistics
    pub async fn get_global_stats(&self) -> (f64, f64, NetworkHashrate) {
        (
            *self.global_carbon_intensity.read().await,
            *self.global_renewable_percentage.read().await,
            self.network_hashrate.read().await.clone()
        )
    }

    /// Get the current network summary
    pub async fn get_network_summary(&self) -> NetworkSummary {
        // Get latest data
        let regional_data = self.regional_data.read().await;
        let network_hashrate = self.network_hashrate.read().await;
        
        // Calculate global values
        let global_carbon_intensity = *self.global_carbon_intensity.read().await;
        let global_renewable_percentage = *self.global_renewable_percentage.read().await;
        
        let summary = NetworkSummary {
            timestamp: Utc::now(),
            total_hashrate: network_hashrate.hashrate,
            total_energy_consumption_mwh: network_hashrate.moving_average_24h * ENERGY_PER_HASH * 24.0,
            total_carbon_emissions_tonnes: network_hashrate.hashrate * global_carbon_intensity,
            renewable_percentage: global_renewable_percentage as f32,
            carbon_intensity: global_carbon_intensity,
            regional_breakdown: regional_data.iter().map(|(region_id, data)| {
                RegionalSummary {
                    region_id: region_id.clone(),
                    hashrate_percentage: data.hashrate_percentage as f32,
                    renewable_percentage: data.renewable_percentage as f32,
                    carbon_intensity: data.carbon_intensity,
                    energy_consumption_mwh: data.hashrate_percentage * network_hashrate.moving_average_24h * ENERGY_PER_HASH * 24.0,
                }
            }).collect(),
            network_hashrate: network_hashrate.clone()
        };
        
        summary
    }

    /// Process a network announcement
    pub async fn process_network_announcement(&self, announcement: &EnvironmentalDataAnnouncement) -> Result<(), EmissionsError> {
        // Process regional data
        for region in &announcement.regional_energy_sources {
            let mut regional_energy_sources = HashMap::new();
            
            for source in &region.energy_sources {
                regional_energy_sources.insert(source.source_type, source.percentage as f64);
            }
            
            let carbon_intensity = self.calculate_carbon_intensity(&regional_energy_sources);
            let renewable_percentage = self.calculate_renewable_percentage(&regional_energy_sources);
            
            self.update_regional_data(
                region.region_id.clone(),
                RegionalEnergyData {
                    region_id: region.region_id.clone(),
                    name: region.name.clone(),
                    hashrate_percentage: region.hashrate_percentage as f64,
                    energy_sources: regional_energy_sources,
                    carbon_intensity,
                    renewable_percentage,
                    last_updated: Utc::now(),
                }
            ).await;
        }
        
        // Update network hashrate if provided
        let _network_hashrate = self.network_hashrate.read().await;
        // Update with the announcement data if needed
        // in a real implementation, this might merge the data with existing values
        
        Ok(())
    }
}

/// Network-specific data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkData {
    /// Total network power consumption in MW
    pub total_power_mw: f64,
    /// Network efficiency in J/TH
    pub efficiency_j_th: f64,
    /// Last update timestamp
    pub last_updated: DateTime<Utc>,
}

/// Regional summary for network reporting
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegionalSummary {
    /// Region identifier
    pub region_id: String,
    /// Percentage of total hashrate
    pub hashrate_percentage: f32,
    /// Renewable energy percentage
    pub renewable_percentage: f32,
    /// Carbon intensity
    pub carbon_intensity: f64,
    /// Energy consumption in MWh
    pub energy_consumption_mwh: f64,
}

/// Network summary
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkSummary {
    /// Timestamp
    pub timestamp: DateTime<Utc>,
    /// Total hashrate
    pub total_hashrate: f64,
    /// Total energy consumption in MWh
    pub total_energy_consumption_mwh: f64,
    /// Total carbon emissions in tonnes
    pub total_carbon_emissions_tonnes: f64,
    /// Renewable percentage
    pub renewable_percentage: f32,
    /// Carbon intensity
    pub carbon_intensity: f64,
    /// Regional breakdown
    pub regional_breakdown: Vec<RegionalSummary>,
    /// Network hashrate data
    pub network_hashrate: NetworkHashrate,
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;
    
    #[test]
    fn test_basic_emissions_calculation() {
        let mut tracker = EmissionsTracker::default();
        tracker.load_default_emission_factors();
        
        // Add some hashrate data
        tracker.update_region_hashrate(
            Region::new("US"),
            HashRate(10.0), // 10 TH/s
        );
        
        tracker.update_region_hashrate(
            Region::new("CN"),
            HashRate(15.0), // 15 TH/s
        );
        
        // Calculate for a 24-hour period
        let now = Utc::now();
        let yesterday = now - Duration::days(1);
        
        let result = tracker.calculate_network_emissions(yesterday, now).unwrap();
        
        // Basic verification
        assert!(result.energy_kwh > 0.0, "Energy consumption should be positive");
        assert!(result.tonnes_co2e > 0.0, "Emissions should be positive");
    }
} 