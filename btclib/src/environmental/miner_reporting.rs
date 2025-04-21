use std::collections::HashMap;
use std::time::{Duration, SystemTime};
use serde::{Deserialize, Serialize};
use tracing::{debug, info, warn};
use chrono::{DateTime, Utc};

use crate::environmental::types::{EnergySource, EmissionFactor, HardwareType, Region};
use crate::environmental::treasury::VerificationStatus;

/// Status of an environmental claim verification
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum VerificationStatus {
    /// Claim has been submitted but not yet verified
    Pending,
    /// Claim has been verified by a trusted authority
    Verified,
    /// Claim verification has failed
    Failed,
    /// Claim has expired and needs renewal
    Expired,
    /// Claim is in dispute or under review
    Disputed,
}

/// Information about the verification of an environmental claim
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationInfo {
    /// Name of the verification provider
    pub provider: String,
    /// Date of verification
    pub date: chrono::DateTime<chrono::Utc>,
    /// Reference ID for the verification
    pub reference: String,
    /// Current status of the verification
    pub status: VerificationStatus,
}

/// Information about a miner's environmental claims
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MinerEnvironmentalInfo {
    /// Unique ID for this miner
    pub miner_id: String,
    /// Name of the mining operation
    pub name: String,
    /// Geographic region of the miner
    pub region: Region,
    /// Types of hardware used by the miner
    pub hardware_types: Vec<HardwareType>,
    /// Energy sources with percentage breakdown
    pub energy_sources: HashMap<EnergySource, f64>,
    /// Renewable energy percentage (0-100)
    pub renewable_percentage: f64,
    /// Optional verification information
    pub verification: Option<VerificationInfo>,
    /// Total hashrate in TH/s
    pub total_hashrate: f64,
    /// Energy consumption in kWh/day
    pub energy_consumption_kwh_day: f64,
    /// Carbon footprint in tonnes CO2e/year
    pub carbon_footprint_tonnes_year: Option<f64>,
    /// Date of the last update
    pub last_update: chrono::DateTime<chrono::Utc>,
    /// Whether this miner has REC certificates
    pub has_rec_certificates: bool,
    /// Whether this miner has carbon offsets
    pub has_carbon_offsets: bool,
    /// URL to environmental policy or certificates
    pub certificates_url: Option<String>,
}

impl MinerEnvironmentalInfo {
    /// Create a new miner environmental info record
    pub fn new(
        miner_id: String,
        name: String,
        region: Region,
    ) -> Self {
        Self {
            miner_id,
            name,
            region,
            hardware_types: Vec::new(),
            energy_sources: HashMap::new(),
            renewable_percentage: 0.0,
            verification: None,
            total_hashrate: 0.0,
            energy_consumption_kwh_day: 0.0,
            carbon_footprint_tonnes_year: None,
            last_update: chrono::Utc::now(),
            has_rec_certificates: false,
            has_carbon_offsets: false,
            certificates_url: None,
        }
    }

    /// Calculate carbon footprint based on energy mix and regional emission factors
    pub fn calculate_carbon_footprint(&mut self, emission_factors: &HashMap<Region, EmissionFactor>) -> Result<f64, String> {
        if self.energy_consumption_kwh_day <= 0.0 {
            return Err("Energy consumption must be greater than zero".to_string());
        }

        // Get emission factor for the region
        let emission_factor = match emission_factors.get(&self.region) {
            Some(factor) => factor,
            None => return Err(format!("No emission factor available for region {:?}", self.region)),
        };

        // Calculate annual energy consumption in MWh
        let annual_energy_mwh = self.energy_consumption_kwh_day * 365.0 / 1000.0;

        // Apply renewable percentage
        let non_renewable_percentage = 100.0 - self.renewable_percentage.min(100.0).max(0.0);
        let non_renewable_energy_mwh = annual_energy_mwh * (non_renewable_percentage / 100.0);

        // Calculate carbon footprint
        let carbon_footprint = non_renewable_energy_mwh * emission_factor.grid_emissions_factor;

        // Apply reductions for RECs and offsets
        let mut final_footprint = carbon_footprint;
        
        // RECs effectively reduce the non-renewable portion
        if self.has_rec_certificates {
            // This calculation assumes RECs cover the renewable percentage declared
            // A more complex implementation would track specific REC quantities
            final_footprint = carbon_footprint * 
                (1.0 - (self.renewable_percentage / 100.0).min(1.0).max(0.0));
        }
        
        // Carbon offsets directly reduce the final footprint
        if self.has_carbon_offsets {
            // This is a simplified model; a real implementation would track
            // exact offset quantities and verification status
            final_footprint = final_footprint * 0.9; // Assume 10% reduction from offsets
        }

        // Update the carbon footprint field
        self.carbon_footprint_tonnes_year = Some(final_footprint);

        Ok(final_footprint)
    }

    /// Update the energy source mix
    pub fn update_energy_sources(&mut self, sources: HashMap<EnergySource, f64>) -> Result<(), String> {
        // Validate that percentages sum to approximately 100%
        let total: f64 = sources.values().sum();
        if (total - 100.0).abs() > 1.0 {
            return Err(format!("Energy source percentages should sum to approximately 100%, got {}", total));
        }

        // Calculate renewable percentage
        let renewable_percentage = sources.iter()
            .filter(|(source, _)| source.is_renewable())
            .map(|(_, percentage)| percentage)
            .sum();

        self.energy_sources = sources;
        self.renewable_percentage = renewable_percentage;
        self.last_update = chrono::Utc::now();

        Ok(())
    }

    /// Add hardware types used by the miner
    pub fn add_hardware_types(&mut self, hardware: Vec<HardwareType>) {
        for hw in hardware {
            if !self.hardware_types.contains(&hw) {
                self.hardware_types.push(hw);
            }
        }
        self.last_update = chrono::Utc::now();
    }

    /// Update hashrate and energy consumption
    pub fn update_performance_metrics(
        &mut self,
        hashrate: f64,
        energy_consumption: f64,
    ) -> Result<(), String> {
        if hashrate <= 0.0 {
            return Err("Hashrate must be greater than zero".to_string());
        }

        if energy_consumption <= 0.0 {
            return Err("Energy consumption must be greater than zero".to_string());
        }

        self.total_hashrate = hashrate;
        self.energy_consumption_kwh_day = energy_consumption;
        self.last_update = chrono::Utc::now();

        Ok(())
    }

    /// Add verification information
    pub fn add_verification(
        &mut self,
        provider: String,
        reference: String,
        status: VerificationStatus,
    ) {
        self.verification = Some(VerificationInfo {
            provider,
            date: chrono::Utc::now(),
            reference,
            status,
        });
        self.last_update = chrono::Utc::now();
    }

    /// Update REC certificate status
    pub fn update_rec_status(&mut self, has_certificates: bool, url: Option<String>) {
        self.has_rec_certificates = has_certificates;
        if has_certificates {
            self.certificates_url = url;
        }
        self.last_update = chrono::Utc::now();
    }

    /// Update carbon offset status
    pub fn update_offset_status(&mut self, has_offsets: bool, url: Option<String>) {
        self.has_carbon_offsets = has_offsets;
        if has_offsets && self.certificates_url.is_none() {
            self.certificates_url = url;
        }
        self.last_update = chrono::Utc::now();
    }

    /// Check if verification is still valid (not expired)
    pub fn is_verification_valid(&self) -> bool {
        if let Some(verification) = &self.verification {
            match verification.status {
                VerificationStatus::Verified => {
                    // Check if verification is less than 1 year old
                    let one_year_ago = chrono::Utc::now() - chrono::Duration::days(365);
                    verification.date > one_year_ago
                }
                _ => false,
            }
        } else {
            false
        }
    }

    /// Calculate energy efficiency in J/TH
    pub fn calculate_energy_efficiency(&self) -> Option<f64> {
        if self.total_hashrate <= 0.0 {
            return None;
        }

        // Convert kWh/day to J/TH
        // 1 kWh = 3.6e6 J
        // energy_consumption_kwh_day * 3.6e6 / (total_hashrate * 24 * 3600)
        Some(self.energy_consumption_kwh_day * 3.6e6 / (self.total_hashrate * 24.0 * 3600.0))
    }
}

/// Manager for miner environmental reporting
pub struct MinerReportingManager {
    /// Map of miner IDs to their environmental information
    miners: HashMap<String, MinerEnvironmentalInfo>,
    /// Regional emission factors
    emission_factors: HashMap<Region, EmissionFactor>,
    /// Standard hardware efficiency baselines
    hardware_baselines: HashMap<HardwareType, f64>,
    /// Reports by miner ID
    reports: HashMap<String, MinerEnvironmentalReport>,
}

impl MinerReportingManager {
    /// Create a new miner reporting manager
    pub fn new() -> Self {
        Self {
            miners: HashMap::new(),
            emission_factors: HashMap::new(),
            hardware_baselines: HashMap::new(),
            reports: HashMap::new(),
        }
    }

    /// Register a new miner
    pub fn register_miner(&mut self, info: MinerEnvironmentalInfo) -> Result<(), String> {
        if self.miners.contains_key(&info.miner_id) {
            return Err(format!("Miner with ID {} is already registered", info.miner_id));
        }

        self.miners.insert(info.miner_id.clone(), info);
        info!("Registered miner: {}", info.miner_id);

        Ok(())
    }

    /// Update an existing miner's information
    pub fn update_miner(&mut self, info: MinerEnvironmentalInfo) -> Result<(), String> {
        if !self.miners.contains_key(&info.miner_id) {
            return Err(format!("Miner with ID {} is not registered", info.miner_id));
        }

        self.miners.insert(info.miner_id.clone(), info);
        info!("Updated miner: {}", info.miner_id);

        Ok(())
    }

    /// Get a miner's information by ID
    pub fn get_miner(&self, miner_id: &str) -> Option<&MinerEnvironmentalInfo> {
        self.miners.get(miner_id)
    }

    /// List all registered miners
    pub fn list_miners(&self) -> Vec<&MinerEnvironmentalInfo> {
        self.miners.values().collect()
    }

    /// Set emission factors for regions
    pub fn set_emission_factors(&mut self, factors: HashMap<Region, EmissionFactor>) {
        self.emission_factors = factors;
    }

    /// Set hardware efficiency baselines
    pub fn set_hardware_baselines(&mut self, baselines: HashMap<HardwareType, f64>) {
        self.hardware_baselines = baselines;
    }

    /// Calculate carbon footprints for all miners
    pub fn calculate_carbon_footprints(&mut self) -> Vec<(String, Result<f64, String>)> {
        let mut results = Vec::new();

        for (miner_id, info) in &mut self.miners {
            let result = info.calculate_carbon_footprint(&self.emission_factors);
            results.push((miner_id.clone(), result));
        }

        results
    }

    /// Get miners with verified renewable energy claims
    pub fn get_verified_green_miners(&self) -> Vec<&MinerEnvironmentalInfo> {
        self.miners.values()
            .filter(|info| {
                info.is_verification_valid() && 
                info.renewable_percentage >= 50.0 &&
                info.has_rec_certificates
            })
            .collect()
    }

    /// Get miners with carbon offset claims
    pub fn get_offset_miners(&self) -> Vec<&MinerEnvironmentalInfo> {
        self.miners.values()
            .filter(|info| {
                info.has_carbon_offsets &&
                info.is_verification_valid()
            })
            .collect()
    }

    /// Calculate average efficiency of all miners
    pub fn calculate_average_efficiency(&self) -> Option<f64> {
        let efficiencies: Vec<f64> = self.miners.values()
            .filter_map(|info| info.calculate_energy_efficiency())
            .collect();

        if efficiencies.is_empty() {
            return None;
        }

        Some(efficiencies.iter().sum::<f64>() / efficiencies.len() as f64)
    }

    /// Compare a miner's efficiency to the hardware baseline
    pub fn compare_to_baseline(&self, miner_id: &str) -> Result<f64, String> {
        let info = self.get_miner(miner_id)
            .ok_or_else(|| format!("Miner with ID {} not found", miner_id))?;

        let efficiency = info.calculate_energy_efficiency()
            .ok_or_else(|| "Cannot calculate efficiency without hashrate".to_string())?;

        // Find the most efficient hardware type as a baseline
        let baseline = info.hardware_types.iter()
            .filter_map(|hw| self.hardware_baselines.get(hw))
            .min_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
            .ok_or_else(|| "No baseline available for miner's hardware types".to_string())?;

        // Calculate ratio (lower is better)
        Ok(efficiency / baseline)
    }

    /// Generate a report of all miners' environmental status
    pub fn generate_report(&self) -> MinerEnvironmentalReport {
        let total_miners = self.miners.len();
        let verified_miners = self.miners.values()
            .filter(|info| info.is_verification_valid())
            .count();

        let renewable_percentage = if total_miners > 0 {
            self.miners.values()
                .map(|info| info.renewable_percentage)
                .sum::<f64>() / total_miners as f64
        } else {
            0.0
        };

        let total_hashrate = self.miners.values()
            .map(|info| info.total_hashrate)
            .sum();

        let total_energy = self.miners.values()
            .map(|info| info.energy_consumption_kwh_day)
            .sum();

        let green_miners = self.get_verified_green_miners().len();
        let offset_miners = self.get_offset_miners().len();

        MinerEnvironmentalReport {
            timestamp: chrono::Utc::now(),
            total_miners,
            verified_miners,
            average_renewable_percentage: renewable_percentage,
            total_hashrate,
            total_energy_consumption_kwh_day: total_energy,
            green_miners,
            offset_miners,
            average_efficiency: self.calculate_average_efficiency(),
        }
    }

    /// Submit a new environmental report
    pub fn submit_report(&mut self, report: MinerEnvironmentalReport) {
        self.reports.insert(report.miner_id.clone(), report);
    }

    /// Get a report by miner ID
    pub fn get_report(&self, miner_id: &str) -> Option<&MinerEnvironmentalReport> {
        self.reports.get(miner_id)
    }

    /// Get all reports
    pub fn get_all_reports(&self) -> &HashMap<String, MinerEnvironmentalReport> {
        &self.reports
    }

    /// Verify a report
    pub fn verify_report(&mut self, miner_id: &str, verification: VerificationInfo) -> bool {
        if let Some(report) = self.reports.get_mut(miner_id) {
            report.info.verification = Some(verification.clone());
            report.status = match verification.status {
                VerificationStatus::Approved => ReportStatus::Verified,
                VerificationStatus::Rejected => ReportStatus::Rejected,
                VerificationStatus::Pending => ReportStatus::Submitted,
            };
            true
        } else {
            false
        }
    }

    /// Calculate network-wide renewable energy percentage
    pub fn calculate_network_renewable_percentage(&self) -> f64 {
        let mut total_renewable = 0.0;
        let mut total_miners = 0;
        
        for report in self.reports.values() {
            if report.status == ReportStatus::Verified {
                total_renewable += report.info.renewable_percentage;
                total_miners += 1;
            }
        }
        
        if total_miners > 0 {
            total_renewable / total_miners as f64
        } else {
            0.0
        }
    }
}

/// Report of miners' environmental status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MinerEnvironmentalReport {
    /// Timestamp of the report
    pub timestamp: chrono::DateTime<chrono::Utc>,
    /// Total number of registered miners
    pub total_miners: usize,
    /// Number of miners with verified claims
    pub verified_miners: usize,
    /// Average renewable percentage across all miners
    pub average_renewable_percentage: f64,
    /// Total hashrate of all miners in TH/s
    pub total_hashrate: f64,
    /// Total energy consumption in kWh/day
    pub total_energy_consumption_kwh_day: f64,
    /// Number of miners with verified renewable energy certificates
    pub green_miners: usize,
    /// Number of miners with carbon offsets
    pub offset_miners: usize,
    /// Average energy efficiency in J/TH
    pub average_efficiency: Option<f64>,
}

/// Energy source for mining operations
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EnergySource {
    /// Solar power
    Solar,
    /// Wind power
    Wind,
    /// Hydroelectric power
    Hydro,
    /// Geothermal power
    Geothermal,
    /// Nuclear power
    Nuclear,
    /// Natural gas
    NaturalGas,
    /// Coal
    Coal,
    /// Oil
    Oil,
    /// Biomass
    Biomass,
    /// Unknown source
    Unknown,
}

/// Hardware type used for mining
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum HardwareType {
    /// ASIC miner
    Asic,
    /// GPU miner
    Gpu,
    /// CPU miner
    Cpu,
    /// Other hardware type
    Other,
}

/// Environmental information for a miner
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MinerEnvironmentalInfo {
    /// Miner identifier
    pub miner_id: String,
    
    /// Primary energy sources
    pub energy_sources: Vec<(EnergySource, f64)>,
    
    /// Percentage of renewable energy (0-100)
    pub renewable_percentage: f64,
    
    /// Mining hardware types
    pub hardware_types: HashMap<HardwareType, usize>,
    
    /// Energy efficiency in J/TH
    pub energy_efficiency: f64,
    
    /// Regions where mining operations are located
    pub regions: Vec<Region>,
    
    /// Carbon offset programs participation
    pub carbon_offsets: bool,
    
    /// Verification information for environmental claims
    pub verification: Option<VerificationInfo>,
}

/// Verification information for environmental claims
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationInfo {
    /// Organization providing verification
    pub provider: String,
    
    /// Date of verification
    pub date: DateTime<Utc>,
    
    /// Reference identifier for the verification
    pub reference: String,
    
    /// Status of verification
    pub status: VerificationStatus,
}

/// Environmental report for a miner
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MinerEnvironmentalReport {
    /// Miner identifier
    pub miner_id: String,
    
    /// Environmental information
    pub info: MinerEnvironmentalInfo,
    
    /// Report timestamp
    pub timestamp: DateTime<Utc>,
    
    /// Report status
    pub status: ReportStatus,
}

/// Status of an environmental report
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReportStatus {
    /// Draft report, not yet submitted
    Draft,
    
    /// Submitted report, awaiting verification
    Submitted,
    
    /// Verified report
    Verified,
    
    /// Rejected report
    Rejected,
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_miner_carbon_footprint_calculation() {
        // Create emission factors
        let mut emission_factors = HashMap::new();
        emission_factors.insert(
            Region::NorthAmerica,
            EmissionFactor {
                grid_emissions_factor: 0.4, // 0.4 tonnes CO2e per MWh
                region_name: "North America".to_string(),
            },
        );
        
        // Create miner info
        let mut miner = MinerEnvironmentalInfo::new(
            "miner1".to_string(),
            "Test Miner".to_string(),
            Region::NorthAmerica,
        );
        
        // Update energy consumption
        miner.update_performance_metrics(100.0, 2400.0).unwrap(); // 100 TH/s, 2400 kWh/day
        
        // Update energy sources (50% renewable)
        let mut sources = HashMap::new();
        sources.insert(EnergySource::Solar, 30.0);
        sources.insert(EnergySource::Wind, 20.0);
        sources.insert(EnergySource::Coal, 50.0);
        miner.update_energy_sources(sources).unwrap();
        
        // Calculate carbon footprint
        let footprint = miner.calculate_carbon_footprint(&emission_factors).unwrap();
        
        // Expected calculation:
        // Annual energy = 2400 kWh/day * 365 days / 1000 = 876 MWh
        // Non-renewable = 876 MWh * 0.5 = 438 MWh
        // Footprint = 438 MWh * 0.4 tonnes/MWh = 175.2 tonnes CO2e
        
        // Allow for small floating-point differences
        assert!((footprint - 175.2).abs() < 0.1);
    }
    
    #[test]
    fn test_rec_and_offset_impact() {
        // Create emission factors
        let mut emission_factors = HashMap::new();
        emission_factors.insert(
            Region::Europe,
            EmissionFactor {
                grid_emissions_factor: 0.3, // 0.3 tonnes CO2e per MWh
                region_name: "Europe".to_string(),
            },
        );
        
        // Create miner info
        let mut miner = MinerEnvironmentalInfo::new(
            "miner2".to_string(),
            "Green Miner".to_string(),
            Region::Europe,
        );
        
        // Set metrics
        miner.update_performance_metrics(200.0, 4800.0).unwrap(); // 200 TH/s, 4800 kWh/day
        
        // 80% renewable energy
        let mut sources = HashMap::new();
        sources.insert(EnergySource::Hydro, 50.0);
        sources.insert(EnergySource::Wind, 30.0);
        sources.insert(EnergySource::NaturalGas, 20.0);
        miner.update_energy_sources(sources).unwrap();
        
        // Calculate baseline footprint (without RECs or offsets)
        let baseline = miner.calculate_carbon_footprint(&emission_factors).unwrap();
        
        // Add RECs
        miner.update_rec_status(true, Some("https://recs.example.com".to_string()));
        let with_recs = miner.calculate_carbon_footprint(&emission_factors).unwrap();
        
        // RECs should reduce footprint substantially
        assert!(with_recs < baseline);
        
        // Add offsets
        miner.update_offset_status(true, Some("https://offsets.example.com".to_string()));
        let with_both = miner.calculate_carbon_footprint(&emission_factors).unwrap();
        
        // Adding offsets should reduce further
        assert!(with_both < with_recs);
    }
} 