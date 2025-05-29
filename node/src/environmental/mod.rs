use sysinfo::System;
use std::collections::HashMap;
use std::sync::{Arc, RwLock, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::time::sleep;
use tracing::{debug, info, warn, error};
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use thiserror::Error;

use crate::api::types::environmental::{
    EnergyUsage, CarbonFootprint, ResourceUtilization, EnvironmentalSettings,
    EnvironmentalImpact, EnergySource, EmissionsSource
};

/// Internal settings for environmental monitoring with extended fields
#[derive(Debug, Clone, Serialize, Deserialize)]
struct EnvironmentalSettingsInternal {
    /// Whether monitoring is enabled
    monitoring_enabled: bool,
    /// Whether emission tracking is enabled
    emission_tracking_enabled: bool,
    /// Whether power saving mode is enabled
    power_saving_mode: bool,
    /// Renewable energy percentage (if known)
    renewable_energy_percentage: Option<f64>,
    /// Energy source type (e.g., "grid", "solar", "hybrid")
    energy_source_type: Option<String>,
    /// Whether carbon offset is enabled
    carbon_offset_enabled: bool,
    /// Data retention in days
    data_retention_days: u64,
    /// Energy efficiency target (kWh per transaction)
    energy_efficiency_target: Option<f64>,
    /// Location code for regional emissions calculation
    location_code: Option<String>,
}

/// Energy source types
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum EnergySourceType {
    Coal,
    NaturalGas,
    Nuclear,
    Hydro,
    Wind,
    Solar,
    Geothermal,
    Other,
}

#[derive(Error, Debug)]
pub enum EnvironmentalError {
    #[error("Invalid time period: {0}")]
    InvalidTimePeriod(String),
    #[error("Data not available: {0}")]
    DataNotAvailable(String),
    #[error("Invalid region code: {0}")]
    InvalidRegion(String),
    #[error("Invalid setting: {0}")]
    InvalidSetting(String),
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
}

/// Main structure for monitoring and tracking environmental impact
pub struct EnvironmentalMonitor {
    /// Current settings for environmental monitoring
    settings: RwLock<EnvironmentalSettingsInternal>,
    /// System information for resource monitoring
    system: Mutex<System>,
    /// Historical energy usage data
    energy_history: RwLock<Vec<EnergyUsageHistory>>,
    /// Emissions factors by region (g CO2e/kWh)
    emission_factors: HashMap<String, f64>,
    /// Energy mix data by region
    energy_mix: HashMap<String, HashMap<EnergySourceType, f64>>,
    /// Current node location
    node_location: String,
    /// Start time of the node for tracking uptime
    start_time: SystemTime,
}

impl EnvironmentalMonitor {
    /// Create a new environmental monitor
    pub fn new() -> Self {
        let mut system = System::new();
        system.refresh_all();
        
        // Default settings
        let settings = EnvironmentalSettingsInternal {
            monitoring_enabled: true,
            emission_tracking_enabled: true,
            power_saving_mode: false,
            renewable_energy_percentage: None,
            energy_source_type: Some("grid".to_string()),
            carbon_offset_enabled: false,
            data_retention_days: 30,
            energy_efficiency_target: Some(0.5),
            location_code: Some("global".to_string()),
        };
        
        // Initialize with some common emission factors (g CO2e/kWh)
        let mut emission_factors = HashMap::new();
        emission_factors.insert("us".to_string(), 417.0);
        emission_factors.insert("eu".to_string(), 295.0);
        emission_factors.insert("cn".to_string(), 609.0);
        emission_factors.insert("global".to_string(), 475.0);
        
        // Initialize energy mix data
        let mut energy_mix = HashMap::new();
        
        // Global average energy mix
        let mut global_mix = HashMap::new();
        global_mix.insert(EnergySourceType::Coal, 38.0);
        global_mix.insert(EnergySourceType::NaturalGas, 23.0);
        global_mix.insert(EnergySourceType::Nuclear, 10.0);
        global_mix.insert(EnergySourceType::Hydro, 16.0);
        global_mix.insert(EnergySourceType::Wind, 5.0);
        global_mix.insert(EnergySourceType::Solar, 3.0);
        global_mix.insert(EnergySourceType::Other, 5.0);
        energy_mix.insert("global".to_string(), global_mix);
        
        Self {
            settings: RwLock::new(settings),
            system: Mutex::new(system),
            energy_history: RwLock::new(Vec::new()),
            emission_factors,
            energy_mix,
            node_location: "global".to_string(),
            start_time: SystemTime::now(),
        }
    }
    
    /// Get comprehensive environmental impact data
    pub fn get_environmental_impact(&self, period: u64, detail: &str) -> Result<EnvironmentalImpact, EnvironmentalError> {
        if period == 0 {
            return Err(EnvironmentalError::InvalidTimePeriod("Period must be greater than 0".to_string()));
        }
        
        // Get energy and carbon data
        let energy_data = self.get_energy_usage(period, true)?;
        let carbon_data = self.get_carbon_footprint(period, true)?;
        let resource_data = self.get_resource_utilization(period)?;
        
        // Calculate additional metrics
        let transaction_count = self.estimate_transaction_count(period);
        let energy_per_tx = if transaction_count > 0 {
            Some(energy_data.total_consumption / transaction_count as f64)
        } else {
            None
        };
        
        let carbon_per_tx = if transaction_count > 0 {
            Some(carbon_data.total_emissions_g / transaction_count as f64)
        } else {
            None
        };
        
        // Calculate efficiency improvements
        let efficiency_improvement = None; // Simplified for now
        
        // Calculate renewable percentage
        let renewable_percentage = self.calculate_renewable_percentage();
        
        // Since the API EnvironmentalImpact type only has basic fields, we'll create a simple one
        Ok(EnvironmentalImpact {
            carbon_emissions_g_per_hour: carbon_data.total_emissions_g,
            renewable_percentage: self.calculate_renewable_percentage(),
            carbon_intensity: energy_data.carbon_intensity,
            carbon_offsets_tons: 0.0, // TODO: Implement carbon offset tracking
            net_emissions_g_per_hour: carbon_data.total_emissions_g,
            is_carbon_negative: false,
            environmental_score: self.calculate_environmental_score(&energy_data, &carbon_data),
            green_mining_bonus: self.calculate_green_mining_bonus(&energy_data),
            data_sources: self.get_emissions_sources(),
            calculated_at: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        })
    }
    
    /// Get energy usage data
    pub fn get_energy_usage(&self, period: u64, include_history: bool) -> Result<EnergyUsage, EnvironmentalError> {
        if period == 0 {
            return Err(EnvironmentalError::InvalidTimePeriod("Period must be greater than 0".to_string()));
        }
        
        // Calculate energy usage based on system resources
        let system = self.system.lock().unwrap();
        let cpu_usage = system.global_cpu_info().cpu_usage() as f64 / 100.0;
        
        // Estimate energy usage based on CPU usage and a base consumption model
        // This is a simplified model and would be replaced with more accurate measurements
        let base_power_watts = 80.0; // Base power consumption when idle
        let max_power_watts = 200.0; // Maximum power consumption at full load
        let current_power_watts = base_power_watts + (max_power_watts - base_power_watts) * cpu_usage;
        
        // Convert watts to kWh for the specified period
        let hours = period as f64 / 3600.0;
        let total_energy_kwh = current_power_watts * hours / 1000.0;
        
        // Calculate renewable percentage
        let renewable_percentage = self.calculate_renewable_percentage() / 100.0;
        
        // Calculate renewable and non-renewable consumption
        let renewable_consumption = total_energy_kwh * renewable_percentage;
        let non_renewable_consumption = total_energy_kwh * (1.0 - renewable_percentage);
        
        // Store this reading in history (internal tracking)
        if let Ok(mut energy_history) = self.energy_history.write() {
            // Add current reading to history
            let history_entry = EnergyUsageHistory {
                timestamp: SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs(),
                consumption: total_energy_kwh,
                renewable_percentage: renewable_percentage * 100.0,
            };
            
            energy_history.push(history_entry);
            
            // Trim history based on retention settings
            let retention_seconds = self.settings.read().unwrap().data_retention_days * 86400;
            let current_timestamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            energy_history.retain(|entry| {
                current_timestamp - entry.timestamp < retention_seconds
            });
        }
        
        let energy_data = EnergyUsage {
            total_consumption: total_energy_kwh,
            renewable_consumption,
            non_renewable_consumption,
        };
        
        Ok(energy_data)
    }
    
    /// Get carbon footprint data
    pub fn get_carbon_footprint(&self, period: u64, include_offsets: bool) -> Result<CarbonFootprint, EnvironmentalError> {
        if period == 0 {
            return Err(EnvironmentalError::InvalidTimePeriod("Period must be greater than 0".to_string()));
        }
        
        // Get energy usage
        let energy_data = self.get_energy_usage(period, false)?;
        
        // Get emissions factor for the current region
        let emission_factor = self.emission_factors
            .get(&self.node_location)
            .cloned()
            .unwrap_or(475.0); // Default global average if region not found
        
        // Calculate total emissions
        let total_emissions_g = energy_data.total_consumption * emission_factor;
        
        // Calculate carbon intensity (g CO2e per kWh)
        let intensity = emission_factor;
        
        // Get offsets if enabled and requested
        let offsets = if include_offsets && self.settings.read().unwrap().carbon_offset_enabled {
            Some(vec![
                CarbonOffset {
                    amount: total_emissions_g * 0.5 / 1000.0, // Convert to tons and apply 50% offset
                    offset_type: "Renewable Energy Certificate".to_string(),
                    verification_standard: "Gold Standard".to_string(),
                    purchase_date: chrono::Utc::now().format("%Y-%m-%d").to_string(),
                }
            ])
        } else {
            None
        };
        
        // Calculate net emissions after offsets
        let net_emissions_g = if let Some(offset_list) = &offsets {
            let total_offset = offset_list.iter().map(|o| o.amount * 1000.0).sum::<f64>(); // Convert tons back to grams
            total_emissions_g - total_offset
        } else {
            total_emissions_g
        };
        
        // Get emissions sources
        let emissions_sources = self.get_emissions_sources();
        
        let carbon_data = CarbonFootprint {
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            period,
            total_emissions_g,
            net_emissions_g,
            offsets,
            intensity,
            emissions_sources,
            renewable_percentage: self.calculate_renewable_percentage(),
        };
        
        Ok(carbon_data)
    }
    
    /// Get resource utilization data
    pub fn get_resource_utilization(&self, period: u64) -> Result<ResourceUtilization, EnvironmentalError> {
        if period == 0 {
            return Err(EnvironmentalError::InvalidTimePeriod("Period must be greater than 0".to_string()));
        }
        
        let mut system = self.system.lock().unwrap();
        system.refresh_all();
        
        // Calculate CPU usage
        let cpu_usage = system.global_cpu_info().cpu_usage() as f64;
        
        // Calculate memory usage
        let total_memory = system.total_memory() as f64;
        let used_memory = system.used_memory() as f64;
        let memory_usage = if total_memory > 0.0 {
            (used_memory / total_memory) * 100.0
        } else {
            0.0
        };
        
        // Calculate disk usage
        let mut total_disk = 0.0;
        let mut used_disk = 0.0;
        
        for disk in system.disks() {
            total_disk += disk.total_space() as f64;
            used_disk += (disk.total_space() - disk.available_space()) as f64;
        }
        
        let disk_usage = if total_disk > 0.0 {
            (used_disk / total_disk) * 100.0
        } else {
            0.0
        };
        
        let resource_data = ResourceUtilization {
            cpu_utilization: cpu_usage,
            memory_utilization: memory_usage,
            storage_utilization: disk_usage,
        };
        
        Ok(resource_data)
    }
    
    /// Get current environmental settings (converted to API type)
    pub fn get_settings(&self) -> Result<EnvironmentalSettings, EnvironmentalError> {
        let internal_settings = self.settings.read().unwrap();
        Ok(EnvironmentalSettings {
            carbon_offset_enabled: internal_settings.carbon_offset_enabled,
            renewable_tracking_enabled: internal_settings.emission_tracking_enabled,
            reporting_enabled: internal_settings.monitoring_enabled,
        })
    }
    
    /// Update environmental settings (partial update from API type)
    pub fn update_settings(&self, new_settings: EnvironmentalSettings) -> Result<EnvironmentalSettings, EnvironmentalError> {
        let mut internal_settings = self.settings.write().unwrap();
        
        // Update only the fields that exist in the API type
        internal_settings.carbon_offset_enabled = new_settings.carbon_offset_enabled;
        internal_settings.emission_tracking_enabled = new_settings.renewable_tracking_enabled;
        internal_settings.monitoring_enabled = new_settings.reporting_enabled;
        
        drop(internal_settings);
        
        Ok(new_settings)
    }
    
    /// Helper method to estimate transaction count for a period
    fn estimate_transaction_count(&self, period: u64) -> u64 {
        // This would typically come from the node's transaction processing metrics
        // For now, using a simplified model
        let tx_per_second = 5.0; // Estimated average transactions per second
        (tx_per_second * period as f64) as u64
    }
    
    /// Helper method to get energy sources based on configured location
    fn get_energy_sources(&self) -> Vec<EnergySource> {
        if let Some(energy_mix) = self.energy_mix.get(&self.node_location) {
            energy_mix.iter().map(|(source_type, percentage)| {
                let name = match source_type {
                    EnergySourceType::Coal => "Coal",
                    EnergySourceType::NaturalGas => "Natural Gas",
                    EnergySourceType::Nuclear => "Nuclear",
                    EnergySourceType::Hydro => "Hydro",
                    EnergySourceType::Wind => "Wind",
                    EnergySourceType::Solar => "Solar",
                    EnergySourceType::Geothermal => "Geothermal",
                    EnergySourceType::Other => "Other",
                };
                let renewable = matches!(source_type, 
                    EnergySourceType::Hydro | 
                    EnergySourceType::Wind | 
                    EnergySourceType::Solar | 
                    EnergySourceType::Geothermal
                );
                EnergySource {
                    name: name.to_string(),
                    energy: *percentage,
                    percentage: *percentage,
                    renewable,
                }
            }).collect()
        } else {
            vec![]
        }
    }
    
    /// Helper method to get emissions sources
    fn get_emissions_sources(&self) -> Vec<EmissionsSource> {
        vec![
            EmissionsSource {
                name: "Electricity".to_string(),
                emissions: 80.0,
                percentage: 80.0,
            },
            EmissionsSource {
                name: "Cooling".to_string(),
                emissions: 15.0,
                percentage: 15.0,
            },
            EmissionsSource {
                name: "Manufacturing".to_string(),
                emissions: 5.0,
                percentage: 5.0,
            },
        ]
    }
    
    /// Calculate renewable percentage based on location and settings
    fn calculate_renewable_percentage(&self) -> f64 {
        // First check if a value is manually set in settings
        if let Some(renewable) = self.settings.read().unwrap().renewable_energy_percentage {
            return renewable;
        }
        
        // Otherwise calculate based on energy mix of the region
        if let Some(energy_mix) = self.energy_mix.get(&self.node_location) {
            let renewable_sources = [
                EnergySourceType::Hydro,
                EnergySourceType::Wind,
                EnergySourceType::Solar,
                EnergySourceType::Geothermal,
            ];
            
            renewable_sources.iter()
                .filter_map(|source| energy_mix.get(source))
                .sum()
        } else {
            // Default if no data available
            20.0
        }
    }
    
    /// Calculate environmental score
    fn calculate_environmental_score(&self, energy_data: &EnergyUsage, carbon_data: &CarbonFootprint) -> f64 {
        // Simple scoring algorithm: higher renewable percentage = better score
        // Lower emissions = better score
        let renewable_score = energy_data.renewable_percentage;
        let emission_score = 100.0 - (carbon_data.total_emissions_g / 1000.0).min(100.0);
        (renewable_score + emission_score) / 2.0
    }
    
    /// Calculate green mining bonus
    fn calculate_green_mining_bonus(&self, energy_data: &EnergyUsage) -> f64 {
        // Bonus percentage based on renewable energy usage
        if energy_data.renewable_percentage >= 75.0 {
            10.0 // 10% bonus for >75% renewable
        } else if energy_data.renewable_percentage >= 50.0 {
            5.0 // 5% bonus for >50% renewable
        } else {
            0.0
        }
    }
    
    /// Get emissions sources
    fn get_emissions_sources(&self) -> Vec<String> {
        vec![
            "grid_electricity".to_string(),
            "cooling_systems".to_string(),
            "hardware_lifecycle".to_string(),
        ]
    }
}

impl Default for EnvironmentalMonitor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_monitor_creation() {
        let monitor = EnvironmentalMonitor::new();
        assert!(monitor.settings.read().unwrap().monitoring_enabled);
    }
    
    #[test]
    fn test_energy_usage_calculation() {
        let monitor = EnvironmentalMonitor::new();
        let energy_data = monitor.get_energy_usage(3600, false).unwrap();
        
        // Energy should be positive
        assert!(energy_data.total_consumption > 0.0);
        assert!(energy_data.renewable_consumption > 0.0);
        assert!(energy_data.non_renewable_consumption > 0.0);
    }
    
    #[test]
    fn test_carbon_footprint_calculation() {
        let monitor = EnvironmentalMonitor::new();
        let carbon_data = monitor.get_carbon_footprint(3600, true).unwrap();
        
        // Carbon emissions should be positive
        assert!(carbon_data.total_emissions_g > 0.0);
        
        // Renewable percentage should be between 0 and 100
        assert!(carbon_data.renewable_percentage >= 0.0);
        assert!(carbon_data.renewable_percentage <= 100.0);
    }
    
    #[test]
    fn test_resource_utilization() {
        let monitor = EnvironmentalMonitor::new();
        let resource_data = monitor.get_resource_utilization(300).unwrap();
        
        // Resource utilization should be between 0 and 100
        assert!(resource_data.cpu_utilization >= 0.0);
        assert!(resource_data.cpu_utilization <= 100.0);
        assert!(resource_data.memory_utilization >= 0.0);
        assert!(resource_data.memory_utilization <= 100.0);
    }
    
    #[test]
    fn test_settings_update() {
        let monitor = EnvironmentalMonitor::new();
        
        let new_settings = EnvironmentalSettings {
            carbon_offset_enabled: true,
            renewable_tracking_enabled: true,
            reporting_enabled: true,
        };
        
        let updated = monitor.update_settings(new_settings.clone()).unwrap();
        
        assert!(updated.carbon_offset_enabled);
        assert!(updated.renewable_tracking_enabled);
        assert!(updated.reporting_enabled);
    }
    
    #[test]
    fn test_environmental_impact() {
        let monitor = EnvironmentalMonitor::new();
        let impact = monitor.get_environmental_impact(86400, "standard").unwrap();
        
        // Ensure basic metrics are present
        assert!(impact.carbon_emissions_g_per_hour > 0.0);
        assert!(impact.renewable_percentage >= 0.0);
        assert!(impact.renewable_percentage <= 100.0);
    }
} 