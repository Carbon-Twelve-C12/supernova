use crate::api::types::{
    EnvironmentalImpact, EnergyUsage, CarbonFootprint, EnvironmentalSettings,
    ResourceUtilization, EmissionsSource, EnergySource, CarbonOffset, EnergyUsageHistory
};
use std::collections::HashMap;
use std::sync::{Arc, RwLock, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use sysinfo::{System, SystemExt, ProcessorExt, DiskExt};
use thiserror::Error;

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
    settings: RwLock<EnvironmentalSettings>,
    /// System information for resource monitoring
    system: Mutex<System>,
    /// Historical energy usage data
    energy_history: RwLock<Vec<EnergyUsageHistory>>,
    /// Emissions factors by region (g CO2e/kWh)
    emission_factors: HashMap<String, f64>,
    /// Energy mix data by region
    energy_mix: HashMap<String, HashMap<EnergySource, f64>>,
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
        let settings = EnvironmentalSettings {
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
        global_mix.insert(EnergySource::Coal, 38.0);
        global_mix.insert(EnergySource::NaturalGas, 23.0);
        global_mix.insert(EnergySource::Nuclear, 10.0);
        global_mix.insert(EnergySource::Hydro, 16.0);
        global_mix.insert(EnergySource::Wind, 5.0);
        global_mix.insert(EnergySource::Solar, 3.0);
        global_mix.insert(EnergySource::Other, 5.0);
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
            Some(energy_data.total_energy_kwh / transaction_count as f64)
        } else {
            None
        };
        
        let carbon_per_tx = if transaction_count > 0 {
            Some(carbon_data.total_emissions_g / transaction_count as f64)
        } else {
            None
        };
        
        // Calculate efficiency improvements
        let efficiency_improvement = if let Some(hist) = energy_data.history.as_ref() {
            if hist.len() > 1 {
                // Calculate improvement from earliest to latest
                let earliest = hist.first().unwrap();
                let latest = hist.last().unwrap();
                
                if earliest.usage > 0.0 {
                    Some((1.0 - (latest.usage / earliest.usage)) * 100.0)
                } else {
                    Some(0.0)
                }
            } else {
                None
            }
        } else {
            None
        };
        
        // Calculate renewable percentage
        let renewable_percentage = self.calculate_renewable_percentage();
        
        // Create impact report
        let impact = EnvironmentalImpact {
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            period,
            total_energy_kwh: energy_data.total_energy_kwh,
            total_carbon_g: carbon_data.total_emissions_g,
            renewable_percentage,
            transaction_count,
            energy_per_transaction_kwh: energy_per_tx,
            carbon_per_transaction_g: carbon_per_tx,
            energy_sources: self.get_energy_sources(),
            emissions_sources: self.get_emissions_sources(),
            offsets: carbon_data.offsets,
            resource_utilization: Some(resource_data),
            efficiency_improvement,
            energy_efficiency: energy_data.efficiency,
            carbon_intensity: carbon_data.intensity,
            detail_level: detail.to_string(),
        };
        
        Ok(impact)
    }
    
    /// Get energy usage data
    pub fn get_energy_usage(&self, period: u64, include_history: bool) -> Result<EnergyUsage, EnvironmentalError> {
        if period == 0 {
            return Err(EnvironmentalError::InvalidTimePeriod("Period must be greater than 0".to_string()));
        }
        
        // Calculate energy usage based on system resources
        let system = self.system.lock().unwrap();
        let cpu_usage = system.global_processor_info().cpu_usage() as f64 / 100.0;
        
        // Estimate energy usage based on CPU usage and a base consumption model
        // This is a simplified model and would be replaced with more accurate measurements
        let base_power_watts = 80.0; // Base power consumption when idle
        let max_power_watts = 200.0; // Maximum power consumption at full load
        let current_power_watts = base_power_watts + (max_power_watts - base_power_watts) * cpu_usage;
        
        // Convert watts to kWh for the specified period
        let hours = period as f64 / 3600.0;
        let total_energy_kwh = current_power_watts * hours / 1000.0;
        
        // Determine energy sources based on settings or defaults
        let energy_sources = self.get_energy_sources();
        
        // Calculate energy efficiency (kWh per resource utilization unit)
        let efficiency = if cpu_usage > 0.0 {
            total_energy_kwh / cpu_usage
        } else {
            0.0
        };
        
        // Get historical data if requested
        let history = if include_history {
            let energy_history = self.energy_history.read().unwrap();
            
            // Filter history to the requested period
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            
            let filtered_history: Vec<EnergyUsageHistory> = energy_history
                .iter()
                .filter(|entry| now - entry.timestamp < period)
                .cloned()
                .collect();
            
            if filtered_history.is_empty() {
                None
            } else {
                Some(filtered_history)
            }
        } else {
            None
        };
        
        let energy_data = EnergyUsage {
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            period,
            current_power_watts,
            total_energy_kwh,
            energy_sources,
            efficiency,
            history,
        };
        
        // Store this reading in history
        if let Ok(mut energy_history) = self.energy_history.write() {
            // Add current reading to history
            let history_entry = EnergyUsageHistory {
                timestamp: energy_data.timestamp,
                usage: total_energy_kwh,
                power: current_power_watts,
            };
            
            energy_history.push(history_entry);
            
            // Trim history based on retention settings
            let retention_seconds = self.settings.read().unwrap().data_retention_days * 86400;
            energy_history.retain(|entry| {
                energy_data.timestamp - entry.timestamp < retention_seconds
            });
        }
        
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
        let total_emissions_g = energy_data.total_energy_kwh * emission_factor;
        
        // Calculate carbon intensity (g CO2e per kWh)
        let intensity = emission_factor;
        
        // Get offsets if enabled and requested
        let offsets = if include_offsets && self.settings.read().unwrap().carbon_offset_enabled {
            Some(vec![
                CarbonOffset {
                    id: "mock-offset-1".to_string(),
                    quantity_g: total_emissions_g * 0.5, // 50% offset for example
                    provider: "Example Offset Provider".to_string(),
                    verification: Some("Gold Standard".to_string()),
                    timestamp: SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs(),
                }
            ])
        } else {
            None
        };
        
        // Calculate net emissions after offsets
        let net_emissions_g = if let Some(offset_list) = &offsets {
            let total_offset = offset_list.iter().map(|o| o.quantity_g).sum::<f64>();
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
        let cpu_usage = system.global_processor_info().cpu_usage() as f64;
        
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
        
        // Calculate network usage (simplified)
        let network_usage = 50.0; // Mock value, would be replaced with real monitoring
        
        // Calculate uptime
        let uptime_seconds = SystemTime::now()
            .duration_since(self.start_time)
            .unwrap_or_default()
            .as_secs();
        
        let resource_data = ResourceUtilization {
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            period,
            cpu_usage,
            memory_usage,
            disk_usage,
            network_usage,
            uptime_seconds,
        };
        
        Ok(resource_data)
    }
    
    /// Get current environmental settings
    pub fn get_settings(&self) -> Result<EnvironmentalSettings, EnvironmentalError> {
        Ok(self.settings.read().unwrap().clone())
    }
    
    /// Update environmental settings
    pub fn update_settings(&self, new_settings: EnvironmentalSettings) -> Result<EnvironmentalSettings, EnvironmentalError> {
        // Validate settings
        if let Some(loc) = &new_settings.location_code {
            if !self.emission_factors.contains_key(loc) {
                return Err(EnvironmentalError::InvalidRegion(loc.clone()));
            }
        }
        
        if let Some(renewable) = new_settings.renewable_energy_percentage {
            if renewable < 0.0 || renewable > 100.0 {
                return Err(EnvironmentalError::InvalidSetting(
                    "Renewable energy percentage must be between 0 and 100".to_string()
                ));
            }
        }
        
        // Update settings
        *self.settings.write().unwrap() = new_settings.clone();
        
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
            energy_mix.keys().cloned().collect()
        } else if let Some(global_mix) = self.energy_mix.get("global") {
            global_mix.keys().cloned().collect()
        } else {
            vec![]
        }
    }
    
    /// Helper method to get emissions sources
    fn get_emissions_sources(&self) -> Vec<EmissionsSource> {
        vec![
            EmissionsSource::Electricity,
            EmissionsSource::Cooling,
            EmissionsSource::Manufacturing,
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
                EnergySource::Hydro,
                EnergySource::Wind,
                EnergySource::Solar,
                EnergySource::Geothermal,
            ];
            
            renewable_sources.iter()
                .filter_map(|source| energy_mix.get(source))
                .sum()
        } else {
            // Default if no data available
            20.0
        }
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
        assert!(energy_data.total_energy_kwh > 0.0);
        assert!(energy_data.current_power_watts > 0.0);
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
        assert!(resource_data.cpu_usage >= 0.0);
        assert!(resource_data.cpu_usage <= 100.0);
        assert!(resource_data.memory_usage >= 0.0);
        assert!(resource_data.memory_usage <= 100.0);
    }
    
    #[test]
    fn test_settings_update() {
        let monitor = EnvironmentalMonitor::new();
        
        let mut new_settings = monitor.get_settings().unwrap();
        new_settings.renewable_energy_percentage = Some(75.0);
        new_settings.carbon_offset_enabled = true;
        
        let updated = monitor.update_settings(new_settings.clone()).unwrap();
        
        assert_eq!(updated.renewable_energy_percentage, Some(75.0));
        assert!(updated.carbon_offset_enabled);
    }
    
    #[test]
    fn test_invalid_settings() {
        let monitor = EnvironmentalMonitor::new();
        
        let mut invalid_settings = monitor.get_settings().unwrap();
        invalid_settings.renewable_energy_percentage = Some(150.0); // Invalid: > 100%
        
        let result = monitor.update_settings(invalid_settings);
        assert!(result.is_err());
    }
    
    #[test]
    fn test_environmental_impact() {
        let monitor = EnvironmentalMonitor::new();
        let impact = monitor.get_environmental_impact(86400, "standard").unwrap();
        
        // Ensure basic metrics are present
        assert!(impact.total_energy_kwh > 0.0);
        assert!(impact.total_carbon_g > 0.0);
        assert!(!impact.energy_sources.is_empty());
        assert!(!impact.emissions_sources.is_empty());
    }
} 