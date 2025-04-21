use std::collections::HashMap;
use thiserror::Error;
use serde::{Serialize, Deserialize};
use chrono::{DateTime, Utc};
use crate::types::transaction::Transaction;
use crate::config::Config;

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
}

/// Emissions measurement for a timeframe
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Emissions {
    /// Total CO2 equivalent emissions in metric tons
    pub tonnes_co2e: f64,
    /// Energy consumption in kilowatt-hours
    pub energy_kwh: f64,
    /// Percentage from renewable sources (if known)
    pub renewable_percentage: Option<f64>,
}

/// Emissions tracker for the SuperNova network
pub struct EmissionsTracker {
    /// Network hashrate by geographic region
    region_hashrates: HashMap<Region, HashRate>,
    /// Emissions factors by region (gCO2e/kWh)
    region_emission_factors: HashMap<Region, EmissionFactor>,
    /// Energy efficiency of mining hardware over time
    hardware_efficiency: HashMap<HardwareType, Efficiency>,
    /// Reported renewable energy percentage by mining pool
    pool_energy_info: HashMap<PoolId, PoolEnergyInfo>,
    /// Global configuration for the emissions tracker
    config: EmissionsConfig,
}

/// Configuration for emissions tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmissionsConfig {
    /// Whether emissions tracking is enabled
    pub enabled: bool,
    /// Default emissions factor to use when region is unknown
    pub default_emission_factor: f64,
    /// API endpoint for emissions factor data
    pub emissions_api_endpoint: Option<String>,
    /// Default network efficiency (J/TH)
    pub default_network_efficiency: f64,
    /// Percentage of hashrate for which location is known
    pub known_hashrate_percentage: f64,
}

impl Default for EmissionsConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            default_emission_factor: 475.0, // Global average grid emission factor in gCO2e/kWh
            emissions_api_endpoint: None,
            default_network_efficiency: 50.0, // J/TH, modern ASIC average
            known_hashrate_percentage: 30.0, // Assume 30% of hashrate has known location
        }
    }
}

impl EmissionsTracker {
    /// Create a new emissions tracker with the given configuration
    pub fn new(config: EmissionsConfig) -> Self {
        Self {
            region_hashrates: HashMap::new(),
            region_emission_factors: HashMap::new(),
            hardware_efficiency: HashMap::new(),
            pool_energy_info: HashMap::new(),
            config,
        }
    }

    /// Create a new emissions tracker with default configuration
    pub fn default() -> Self {
        Self::new(EmissionsConfig::default())
    }
    
    /// Load predefined emissions factors for common regions
    pub fn load_default_emission_factors(&mut self) {
        // Add some common region emission factors as baseline data
        let regions = vec![
            (
                Region { country_code: "US".to_string(), sub_region: None },
                EmissionFactor {
                    g_co2e_per_kwh: 417.0,
                    year: 2022,
                    source: EmissionsFactorSource::EPA,
                }
            ),
            (
                Region { country_code: "CN".to_string(), sub_region: None },
                EmissionFactor {
                    g_co2e_per_kwh: 580.0,
                    year: 2022,
                    source: EmissionsFactorSource::IEA,
                }
            ),
            (
                Region { country_code: "EU".to_string(), sub_region: None },
                EmissionFactor {
                    g_co2e_per_kwh: 275.0,
                    year: 2022,
                    source: EmissionsFactorSource::EEA,
                }
            ),
            (
                Region { country_code: "IS".to_string(), sub_region: None },
                EmissionFactor {
                    g_co2e_per_kwh: 28.0, // Iceland's grid is mostly geothermal and hydro
                    year: 2022,
                    source: EmissionsFactorSource::IEA,
                }
            ),
        ];
        
        for (region, factor) in regions {
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
    
    /// Calculate total network emissions for a given time period using CBECI methodology
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
        let total_energy_kwh = energy_per_second * duration_hours / 1000.0;
        
        // Calculate emissions for known regions
        let mut known_emissions = 0.0;
        let mut known_energy = 0.0;
        let mut renewable_total = 0.0;
        let mut known_renewable_energy = 0.0;
        
        for (region, hashrate) in &self.region_hashrates {
            let region_hashrate_percentage = hashrate.0 / known_hashrate;
            let region_energy = total_energy_kwh * region_hashrate_percentage;
            known_energy += region_energy;
            
            // Get emission factor for this region
            let emission_factor = match self.region_emission_factors.get(region) {
                Some(factor) => factor.g_co2e_per_kwh,
                None => self.config.default_emission_factor,
            };
            
            known_emissions += region_energy * emission_factor;
            
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
            }
        }
        
        // Calculate unknown emissions using default factor
        let unknown_energy = total_energy_kwh - known_energy;
        let unknown_emissions = unknown_energy * self.config.default_emission_factor;
        
        let total_emissions_kg = (known_emissions + unknown_emissions) / 1000.0; // Convert to kg
        let total_emissions_tonnes = total_emissions_kg / 1000.0; // Convert to tonnes
        
        // Calculate overall renewable percentage if we have data
        let renewable_percentage = if known_energy > 0.0 {
            Some((known_renewable_energy / total_energy_kwh) * 100.0)
        } else {
            None
        };
        
        Ok(Emissions {
            tonnes_co2e: total_emissions_tonnes,
            energy_kwh: total_energy_kwh,
            renewable_percentage,
        })
    }
    
    /// Estimate emissions for a single transaction
    pub fn estimate_transaction_emissions(&self, transaction: &Transaction) -> Result<Emissions, EmissionsError> {
        // This is a basic implementation for Phase 1
        // A more sophisticated model would account for transaction size, computational complexity, etc.
        
        // For now, use a simple average-based approach
        // For Phase 1, we'll assume a fixed energy cost per transaction
        
        // Assuming the average block has 2000 transactions and consumes X energy
        let avg_tx_energy_kwh = 0.0002; // Simplified value for Phase 1
        
        // Get the current average emission factor based on known hashrate distribution
        let mut weighted_emission_factor = 0.0;
        let mut total_weight = 0.0;
        
        for (region, hashrate) in &self.region_hashrates {
            if let Some(factor) = self.region_emission_factors.get(region) {
                weighted_emission_factor += factor.g_co2e_per_kwh * hashrate.0;
                total_weight += hashrate.0;
            }
        }
        
        let emission_factor = if total_weight > 0.0 {
            weighted_emission_factor / total_weight
        } else {
            self.config.default_emission_factor
        };
        
        let emissions_kg = avg_tx_energy_kwh * emission_factor / 1000.0;
        let emissions_tonnes = emissions_kg / 1000.0;
        
        Ok(Emissions {
            tonnes_co2e: emissions_tonnes,
            energy_kwh: avg_tx_energy_kwh,
            renewable_percentage: None, // Not calculated at transaction level in Phase 1
        })
    }
    
    /// Update configuration
    pub fn update_config(&mut self, config: EmissionsConfig) {
        self.config = config;
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
            Region { country_code: "US".to_string(), sub_region: None },
            HashRate(10.0), // 10 TH/s
        );
        
        tracker.update_region_hashrate(
            Region { country_code: "CN".to_string(), sub_region: None },
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