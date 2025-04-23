use std::collections::HashMap;
use thiserror::Error;
use serde::{Serialize, Deserialize};
use chrono::{DateTime, Utc, NaiveDateTime};
use crate::types::transaction::Transaction;
use crate::config::Config;
use crate::environmental::types::{EmissionsDataSource, EmissionsFactorType, Region, EmissionFactor};
use reqwest::Client;
use tokio::sync::RwLock;
use std::sync::Arc;
use url::Url;
use std::time::{Duration, SystemTime};

/// Error types for emissions tracking operations
#[derive(Error, Debug)]
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

/// Emissions factor for a specific region (gCO2e/kWh)
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct EmissionFactor {
    /// CO2 equivalent emissions per kWh in grams
    pub g_co2e_per_kwh: f64,
    /// Year the factor was measured/published
    pub year: u16,
    /// Source of the emissions factor data
    pub source: EmissionsFactorSource,
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
        }
    }
}

/// Emissions tracker for the SuperNova network
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
            for factor in factors {
                let region_name = factor.region_name.clone();
                let region_parts: Vec<&str> = region_name.split('-').collect();
                
                let region = if region_parts.len() > 1 {
                    Region::with_sub_region(region_parts[0], region_parts[1])
                } else {
                    Region::new(&region_name)
                };
                
                match factor.factor_type {
                    EmissionsFactorType::GridAverage => {
                        self.region_emission_factors.insert(region.clone(), factor);
                    },
                    _ => {
                        self.alt_emission_factors.insert((region, factor.factor_type), factor);
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
                Some(factor) => factor.grid_emissions_factor,
                None => self.config.default_emission_factor / 1000.0, // Convert g to kg
            };
            
            // Calculate location-based emissions (without RECs)
            let region_location_emissions = region_energy * emission_factor;
            location_based_emissions += region_location_emissions;
            
            // If enabled, get marginal emissions factor
            if self.config.use_marginal_emissions {
                let marginal_factor = match self.get_marginal_emissions_factor(region) {
                    Some(factor) => factor.grid_emissions_factor,
                    None => emission_factor, // Fall back to average if no marginal data
                };
                
                let region_marginal_emissions = region_energy * marginal_factor;
                marginal_emissions += region_marginal_emissions;
            }
            
            // Check for confidence levels
            if let Some(factor) = self.get_best_emissions_factor(region) {
                if let Some(confidence) = factor.confidence {
                    confidence_sum += confidence;
                    confidence_count += 1;
                }
            }
            
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
    
    /// Verify renewable energy certificate claims
    pub fn verify_rec_claim(&self, certificate: &RECCertificateInfo) -> VerificationStatus {
        // This is a placeholder implementation
        // In a production system, this would connect to a verification service
        
        // Check if the certificate has expired
        let now = Utc::now();
        if certificate.generation_end < now - chrono::Duration::days(365) {
            return VerificationStatus::Expired;
        }
        
        // For demo purposes, simulate verification
        if certificate.certificate_url.is_some() && !certificate.certificate_id.is_empty() {
            VerificationStatus::Verified
        } else {
            VerificationStatus::Pending
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
                
                if let Some(confidence) = factor.confidence {
                    confidence_sum += confidence;
                    confidence_count += 1;
                }
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
                
                if let Some(confidence) = factor.confidence {
                    confidence_sum += confidence;
                    confidence_count += 1;
                }
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
}

/// Adapter to provide compatibility between the new API and existing EmissionsTracker
pub struct EmissionsTrackerAdapter {
    tracker: EmissionsTracker,
}

impl EmissionsTrackerAdapter {
    /// Create a new adapter with default configuration
    pub fn new() -> Self {
        Self {
            tracker: EmissionsTracker::default(),
        }
    }
    
    /// Create a new adapter with custom configuration
    pub fn with_config(config: EmissionsConfig) -> Self {
        Self {
            tracker: EmissionsTracker::new(config),
        }
    }
    
    /// Get the inner tracker
    pub fn inner(&self) -> &EmissionsTracker {
        &self.tracker
    }
    
    /// Get mutable reference to the inner tracker
    pub fn inner_mut(&mut self) -> &mut EmissionsTracker {
        &mut self.tracker
    }
    
    /// Add miner data to the emissions tracker
    pub fn add_miner_data(&mut self, id: &str, info: &crate::environmental::miner_reporting::MinerEnvironmentalInfo) -> Result<(), EmissionsError> {
        // Create a pool ID from the miner ID
        let pool_id = PoolId(id.to_string());
        
        // Convert MinerEnvironmentalInfo to PoolEnergyInfo
        let pool_info = PoolEnergyInfo {
            renewable_percentage: info.renewable_energy_percentage,
            verified: info.verification_status == crate::environmental::miner_reporting::VerificationStatus::Verified,
            regions: vec![info.region.clone()],
            last_updated: Utc::now(),
            energy_sources: Vec::new(), // Would need more detailed info from miner
            rec_certificates: if info.rec_percentage > 0.0 {
                Some(RECCertificateInfo {
                    certificate_id: format!("REC-{}", id),
                    issuer: "".to_string(),
                    amount_mwh: 0.0, // Would need more info
                    generation_start: Utc::now() - chrono::Duration::days(365),
                    generation_end: Utc::now(),
                    generation_location: None,
                    verification_status: match info.verification_status {
                        crate::environmental::miner_reporting::VerificationStatus::Verified => VerificationStatus::Verified,
                        crate::environmental::miner_reporting::VerificationStatus::Pending => VerificationStatus::Pending,
                        _ => VerificationStatus::None,
                    },
                    certificate_url: None,
                })
            } else {
                None
            },
            carbon_offsets: if info.offset_percentage > 0.0 {
                Some(CarbonOffsetInfo {
                    offset_id: format!("OFFSET-{}", id),
                    issuer: "".to_string(),
                    amount_tonnes: 0.0, // Would need more info
                    project_type: "".to_string(),
                    project_location: None,
                    verification_status: match info.verification_status {
                        crate::environmental::miner_reporting::VerificationStatus::Verified => VerificationStatus::Verified,
                        crate::environmental::miner_reporting::VerificationStatus::Pending => VerificationStatus::Pending,
                        _ => VerificationStatus::None,
                    },
                    certificate_url: None,
                })
            } else {
                None
            },
        };
        
        // Register the pool in the emissions tracker
        self.tracker.register_pool_energy_info(pool_id, pool_info);
        
        // Estimate hashrate based on hardware type and units
        let hashrate = match info.hardware_type {
            crate::environmental::hardware_types::HardwareType::AntminerS19XP => 140.0 * info.units as f64,
            crate::environmental::hardware_types::HardwareType::AntminerS19Pro => 110.0 * info.units as f64,
            crate::environmental::hardware_types::HardwareType::AntminerS19jPro => 104.0 * info.units as f64,
            crate::environmental::hardware_types::HardwareType::AntminerS19 => 95.0 * info.units as f64,
            crate::environmental::hardware_types::HardwareType::WhatsminerM30SPlusPlus => 112.0 * info.units as f64,
            crate::environmental::hardware_types::HardwareType::WhatsminerM30SPlus => 100.0 * info.units as f64,
            crate::environmental::hardware_types::HardwareType::WhatsminerM30S => 88.0 * info.units as f64,
            crate::environmental::hardware_types::HardwareType::AvalonA1246 => 90.0 * info.units as f64,
            _ => 80.0 * info.units as f64, // Default to 80 TH/s per unit for unknown types
        };
        
        // Update the region hashrate
        self.tracker.update_region_hashrate(info.region.clone(), HashRate(hashrate));
        
        Ok(())
    }
    
    /// Update miner data in the emissions tracker
    pub fn update_miner_data(&mut self, id: &str, info: &crate::environmental::miner_reporting::MinerEnvironmentalInfo) -> Result<(), EmissionsError> {
        // Implementation is the same as add_miner_data for now
        self.add_miner_data(id, info)
    }
    
    /// Calculate emissions for a specific miner
    pub fn calculate_miner_emissions(&self, id: &str) -> Result<MinerEmissionsResults, EmissionsError> {
        let pool_id = PoolId(id.to_string());
        
        // Check if we have information for this miner
        let pool_info = match self.tracker.pool_energy_info.get(&pool_id) {
            Some(info) => info,
            None => return Err(EmissionsError::InvalidRegion(format!("No data for miner {}", id))),
        };
        
        // Take the first region for this miner
        if pool_info.regions.is_empty() {
            return Err(EmissionsError::InvalidRegion(format!("No region for miner {}", id)));
        }
        
        let region = &pool_info.regions[0];
        
        // Get the hashrate for this miner
        let hashrate = match self.tracker.region_hashrates.get(region) {
            Some(hr) => hr.0,
            None => return Err(EmissionsError::InvalidRegion(format!("No hashrate for region {}", region.country_code))),
        };
        
        // Get the emissions factor for this region
        let emissions_factor = match self.tracker.get_best_emissions_factor(region) {
            Some(factor) => factor.grid_emissions_factor,
            None => self.tracker.config.default_emission_factor / 1000.0, // Convert g/kWh to kg/kWh
        };
        
        // Calculate energy consumption (kWh per day)
        let efficiency = self.tracker.config.default_network_efficiency; // J/TH
        let daily_energy_kwh = (hashrate * 1e12 * efficiency * 86400.0) / 3.6e9; // TH/s to kWh/day
        
        // Calculate gross emissions (kg CO2e)
        let gross_emissions_kg = daily_energy_kwh * emissions_factor;
        
        // Apply renewable percentage reduction
        let renewable_percentage = pool_info.renewable_percentage;
        let rec_percentage = pool_info.rec_certificates.as_ref().map_or(0.0, |_| 0.0); // Need more info
        let offset_percentage = pool_info.carbon_offsets.as_ref().map_or(0.0, |_| 0.0); // Need more info
        
        // Calculate net emissions after applying renewable percentage and RECs
        let reduction_percentage = (renewable_percentage + rec_percentage).min(100.0);
        let net_emissions_kg = gross_emissions_kg * (1.0 - reduction_percentage / 100.0);
        
        Ok(MinerEmissionsResults {
            daily_energy_kwh,
            gross_emissions_kg,
            net_emissions_kg,
            reduction_percentage,
        })
    }
    
    /// Update emissions factor for a region
    pub fn update_emissions_factor(&mut self, region: crate::environmental::types::Region, emissions_factor: f64) -> Result<(), EmissionsError> {
        // Convert from types::Region to emissions::Region
        let emissions_region = Region {
            country_code: region.country_code.clone(),
            sub_region: region.sub_region.clone(),
        };
        
        // Convert to kg/kWh for internal storage
        let factor_kg_per_kwh = emissions_factor / 1000.0;
        
        // Create an EmissionFactor
        let emission_factor = EmissionFactor {
            grid_emissions_factor: factor_kg_per_kwh,
            region_name: emissions_region.to_string(),
            data_source: EmissionsDataSource::Custom,
            factor_type: EmissionsFactorType::GridAverage,
            year: Some(chrono::Utc::now().year() as u16),
            timestamp: Some(chrono::Utc::now()),
            confidence: None,
        };
        
        // Update the emissions factor
        self.tracker.region_emission_factors.insert(emissions_region.clone(), emission_factor);
        
        Ok(())
    }
    
    /// Get the average emissions factor across all regions
    pub fn get_average_emissions_factor(&self) -> Result<f64, EmissionsError> {
        let (avg_factor, weight) = self.tracker.calculate_weighted_emission_factor();
        
        if weight <= 0.0 {
            return Err(EmissionsError::DataSourceError("No valid emissions factors found".to_string()));
        }
        
        // Convert back to g/kWh for API
        Ok(avg_factor * 1000.0)
    }
}

/// Results from miner emissions calculations
pub struct MinerEmissionsResults {
    pub daily_energy_kwh: f64,
    pub gross_emissions_kg: f64,
    pub net_emissions_kg: f64,
    pub reduction_percentage: f64,
}

impl Default for EmissionsTrackerAdapter {
    fn default() -> Self {
        Self::new()
    }
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