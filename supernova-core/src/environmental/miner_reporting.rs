use crate::environmental::score_validation::EnvironmentalScoreValidator;
use crate::environmental::types::{
    EmissionFactor, EnergySource as TypesEnergySource, HardwareType as TypesHardwareType, Region,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use tracing::{info, warn};

/// Status of miner verification
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MinerVerificationStatus {
    /// Miner has been verified
    Verified,
    /// Miner verification is pending
    Pending,
    /// Miner verification is rejected
    Rejected,
    /// Miner is unverified
    Unverified,
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
    pub status: MinerVerificationStatus,
}

/// REC certificate information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RECCertificate {
    /// Certificate ID or reference number
    pub certificate_id: String,
    /// Issuing organization
    pub issuer: String,
    /// Amount of renewable energy in MWh
    pub amount_mwh: f64,
    /// Start date of generation period
    pub generation_start: DateTime<Utc>,
    /// End date of generation period
    pub generation_end: DateTime<Utc>,
    /// Location of renewable energy generation
    pub generation_location: Option<Region>,
    /// Type of renewable energy
    pub energy_type: TypesEnergySource,
    /// Verification status
    pub verification_status: MinerVerificationStatus,
    /// URL to certificate
    pub certificate_url: Option<String>,
    /// Last verification date
    pub last_verified: Option<DateTime<Utc>>,
    /// Transaction ID if recorded on blockchain
    pub blockchain_tx_id: Option<String>,
}

/// Carbon offset information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CarbonOffset {
    /// Offset ID or reference number
    pub offset_id: String,
    /// Issuing organization
    pub issuer: String,
    /// Amount of carbon offset in tonnes CO2e
    pub amount_tonnes: f64,
    /// Type of offset project
    pub project_type: String,
    /// Location of offset project
    pub project_location: Option<Region>,
    /// Start date of offset period
    pub offset_start: DateTime<Utc>,
    /// End date of offset period
    pub offset_end: DateTime<Utc>,
    /// Verification status
    pub verification_status: MinerVerificationStatus,
    /// URL to offset certificate
    pub certificate_url: Option<String>,
    /// Last verification date
    pub last_verified: Option<DateTime<Utc>>,
    /// Transaction ID if recorded on blockchain
    pub blockchain_tx_id: Option<String>,
}

/// Location verification method
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum LocationVerificationMethod {
    /// IP address geolocation
    IPGeolocation,
    /// Third-party audit
    Audit,
    /// Cryptographic proof (like synthetic location)
    CryptographicProof,
    /// Self-declaration (lowest confidence)
    SelfDeclared,
    /// Government registration
    GovernmentRegistry,
    /// Multi-factor verification (highest confidence)
    MultiFactor,
}

/// Location verification information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocationVerification {
    /// Verification method used
    pub method: LocationVerificationMethod,
    /// Verification timestamp
    pub timestamp: DateTime<Utc>,
    /// Confidence level (0-1)
    pub confidence: f64,
    /// Verifier name or ID
    pub verifier: Option<String>,
    /// Evidence reference
    pub evidence_reference: Option<String>,
    /// Verification status
    pub status: MinerVerificationStatus,
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
    /// Location verification information
    pub location_verification: Option<LocationVerification>,
    /// Types of hardware used by the miner
    pub hardware_types: Vec<TypesHardwareType>,
    /// Energy sources with percentage breakdown
    pub energy_sources: HashMap<TypesEnergySource, f64>,
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
    /// Detailed REC certificate information
    pub rec_certificates: Vec<RECCertificate>,
    /// Detailed carbon offset information
    pub carbon_offsets: Vec<CarbonOffset>,
    /// Environmental score (0-100)
    pub environmental_score: Option<f64>,
    /// Preferred renewable energy type
    pub preferred_energy_type: Option<TypesEnergySource>,
}

impl MinerEnvironmentalInfo {
    /// Create a new miner environmental info record
    pub fn new(miner_id: String, name: String, region: Region) -> Self {
        Self {
            miner_id,
            name,
            region,
            location_verification: None,
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
            rec_certificates: Vec::new(),
            carbon_offsets: Vec::new(),
            environmental_score: None,
            preferred_energy_type: None,
        }
    }

    /// Calculate carbon footprint based on energy mix and regional emission factors
    pub fn calculate_carbon_footprint(
        &mut self,
        emission_factors: &HashMap<Region, EmissionFactor>,
    ) -> Result<f64, String> {
        if self.energy_consumption_kwh_day <= 0.0 {
            return Err("Energy consumption must be greater than zero".to_string());
        }

        // Get emission factor for the region
        let emission_factor = match emission_factors.get(&self.region) {
            Some(factor) => factor,
            None => {
                return Err(format!(
                    "No emission factor available for region {:?}",
                    self.region
                ))
            }
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
            final_footprint =
                carbon_footprint * (1.0 - (self.renewable_percentage / 100.0).min(1.0).max(0.0));
        }

        // Carbon offsets directly reduce the final footprint
        if self.has_carbon_offsets {
            // This is a simplified model; a real implementation would track
            // exact offset quantities and verification status
            final_footprint *= 0.9; // Assume 10% reduction from offsets
        }

        // Update the carbon footprint field
        self.carbon_footprint_tonnes_year = Some(final_footprint);

        Ok(final_footprint)
    }

    /// Update the energy source mix
    pub fn update_energy_sources(
        &mut self,
        sources: HashMap<TypesEnergySource, f64>,
    ) -> Result<(), String> {
        // Validate that percentages sum to approximately 100%
        let total: f64 = sources.values().sum();
        if (total - 100.0).abs() > 1.0 {
            return Err(format!(
                "Energy source percentages should sum to approximately 100%, got {}",
                total
            ));
        }

        // Calculate renewable percentage
        let renewable_percentage = sources
            .iter()
            .filter(|(source, _)| source.is_renewable())
            .map(|(_, percentage)| percentage)
            .sum();

        self.energy_sources = sources;
        self.renewable_percentage = renewable_percentage;
        self.last_update = chrono::Utc::now();

        Ok(())
    }

    /// Add hardware types used by the miner
    pub fn add_hardware_types(&mut self, hardware: Vec<TypesHardwareType>) {
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
        status: MinerVerificationStatus,
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
                MinerVerificationStatus::Verified => {
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

    /// Add REC certificate
    pub fn add_rec_certificate(&mut self, certificate: RECCertificate) {
        self.rec_certificates.push(certificate);
        self.has_rec_certificates = true;
        self.last_update = chrono::Utc::now();
    }

    /// Add carbon offset
    pub fn add_carbon_offset(&mut self, offset: CarbonOffset) {
        self.carbon_offsets.push(offset);
        self.has_carbon_offsets = true;
        self.last_update = chrono::Utc::now();
    }

    /// Set location verification information
    pub fn set_location_verification(&mut self, verification: LocationVerification) {
        self.location_verification = Some(verification);
        self.last_update = chrono::Utc::now();
    }

    /// Calculate environmental score
    pub fn calculate_environmental_score(&mut self) -> f64 {
        // Base score starts with renewable percentage (0-50 points)
        let renewable_score = (self.renewable_percentage / 100.0) * 50.0;

        // Add points for REC certificates (0-20 points)
        let rec_score = if self.has_rec_certificates {
            let verified_recs = self
                .rec_certificates
                .iter()
                .filter(|rec| rec.verification_status == MinerVerificationStatus::Verified)
                .count();

            if verified_recs > 0 {
                20.0
            } else {
                10.0 // Some points for unverified RECs
            }
        } else {
            0.0
        };

        // Add points for carbon offsets (0-10 points)
        let offset_score = if self.has_carbon_offsets {
            let verified_offsets = self
                .carbon_offsets
                .iter()
                .filter(|offset| offset.verification_status == MinerVerificationStatus::Verified)
                .count();

            if verified_offsets > 0 {
                10.0
            } else {
                5.0 // Some points for unverified offsets
            }
        } else {
            0.0
        };

        // Add points for location verification (0-10 points)
        let location_score = if let Some(verification) = &self.location_verification {
            match verification.method {
                LocationVerificationMethod::MultiFactor => 10.0,
                LocationVerificationMethod::Audit => 8.0,
                LocationVerificationMethod::GovernmentRegistry => 7.0,
                LocationVerificationMethod::CryptographicProof => 6.0,
                LocationVerificationMethod::IPGeolocation => 3.0,
                LocationVerificationMethod::SelfDeclared => 1.0,
            }
        } else {
            0.0
        };

        // Add points for energy efficiency (0-10 points)
        let efficiency_score = if let Some(efficiency) = self.calculate_energy_efficiency() {
            // Lower J/TH is better

            match efficiency {
                e if e < 25.0 => 10.0, // Most efficient ASICs
                e if e < 35.0 => 8.0,  // Very efficient
                e if e < 50.0 => 6.0,  // Efficient
                e if e < 75.0 => 4.0,  // Moderate
                e if e < 100.0 => 2.0, // Below average
                _ => 0.0,              // Inefficient
            }
        } else {
            0.0
        };

        // Total score (0-100)
        let total_score =
            renewable_score + rec_score + offset_score + location_score + efficiency_score;

        // Guard the computed score against out-of-range / non-finite manipulation before
        // it can drive green-mining incentives. Inputs such as `renewable_percentage` are
        // self-reported, so a bad or manipulated report could otherwise yield a score
        // outside the valid 0-100 band. Reject (clamp) any such score.
        let validator = EnvironmentalScoreValidator::default();
        let validated_score = match validator.validate_score(total_score) {
            Ok(()) => total_score,
            Err(reason) => {
                warn!(
                    "Environmental score {} for miner {} failed validation: {}. Clamping to valid range.",
                    total_score, self.miner_id, reason
                );
                if total_score.is_finite() {
                    total_score.clamp(0.0, 100.0)
                } else {
                    0.0
                }
            }
        };

        // Update the score
        self.environmental_score = Some(validated_score);

        validated_score
    }

    /// Calculate carbon footprint with REC and offset prioritization
    pub fn calculate_carbon_footprint_with_prioritization(
        &mut self,
        emission_factors: &HashMap<Region, EmissionFactor>,
    ) -> Result<f64, String> {
        if self.energy_consumption_kwh_day <= 0.0 {
            return Err("Energy consumption must be greater than zero".to_string());
        }

        // Get emission factor for the region
        let emission_factor = match emission_factors.get(&self.region) {
            Some(factor) => factor,
            None => {
                return Err(format!(
                    "No emission factor available for region {:?}",
                    self.region
                ))
            }
        };

        // Calculate annual energy consumption in MWh
        let annual_energy_mwh = self.energy_consumption_kwh_day * 365.0 / 1000.0;

        // Calculate gross carbon footprint (without RECs or offsets)
        let gross_footprint = annual_energy_mwh * emission_factor.grid_emissions_factor;

        // Apply reductions based on verified RECs (given top priority)
        let mut remaining_footprint = gross_footprint;

        // First apply RECs (full reduction of covered portion)
        let rec_covered_mwh: f64 = self
            .rec_certificates
            .iter()
            .filter(|cert| cert.verification_status == MinerVerificationStatus::Verified)
            .map(|cert| cert.amount_mwh)
            .sum();

        let rec_coverage_ratio = (rec_covered_mwh / annual_energy_mwh).min(1.0);
        remaining_footprint = gross_footprint * (1.0 - rec_coverage_ratio);

        // Then apply carbon offsets to remaining footprint
        let offset_tonnes: f64 = self
            .carbon_offsets
            .iter()
            .filter(|offset| offset.verification_status == MinerVerificationStatus::Verified)
            .map(|offset| offset.amount_tonnes)
            .sum();

        // Directly subtract verified offsets from remaining footprint
        remaining_footprint = (remaining_footprint - offset_tonnes).max(0.0);

        // Update the carbon footprint field
        self.carbon_footprint_tonnes_year = Some(remaining_footprint);

        Ok(remaining_footprint)
    }

    /// Check if miner has verified RECs
    pub fn has_verified_recs(&self) -> bool {
        self.rec_certificates
            .iter()
            .any(|cert| cert.verification_status == MinerVerificationStatus::Verified)
    }

    /// Check if miner has verified carbon offsets
    pub fn has_verified_offsets(&self) -> bool {
        self.carbon_offsets
            .iter()
            .any(|offset| offset.verification_status == MinerVerificationStatus::Verified)
    }

    /// Get total verified REC amount in MWh
    pub fn total_verified_recs_mwh(&self) -> f64 {
        self.rec_certificates
            .iter()
            .filter(|cert| cert.verification_status == MinerVerificationStatus::Verified)
            .map(|cert| cert.amount_mwh)
            .sum()
    }

    /// Get total verified offset amount in tonnes CO2e
    pub fn total_verified_offsets_tonnes(&self) -> f64 {
        self.carbon_offsets
            .iter()
            .filter(|offset| offset.verification_status == MinerVerificationStatus::Verified)
            .map(|offset| offset.amount_tonnes)
            .sum()
    }
}

/// Manager for miner environmental reporting
pub struct MinerReportingManager {
    /// Map of miner IDs to their environmental information
    miners: HashMap<String, MinerEnvironmentalInfo>,
    /// Regional emission factors
    emission_factors: HashMap<Region, EmissionFactor>,
    /// Standard hardware efficiency baselines
    hardware_baselines: HashMap<TypesHardwareType, f64>,
    /// Reports by miner ID
    reports: HashMap<String, MinerEnvironmentalReport>,
    /// Trusted REC issuers (normalized to UPPERCASE). Only certificates issued by
    /// one of these organizations may be marked `Verified`.
    trusted_rec_issuers: HashSet<String>,
    /// Maps a REC certificate ID to the miner that has already claimed it, preventing
    /// the same certificate (generation period) from being double-claimed by two miners.
    consumed_rec_certificates: HashMap<String, String>,
}

impl Default for MinerReportingManager {
    fn default() -> Self {
        Self::new()
    }
}

impl MinerReportingManager {
    /// Create a new miner reporting manager
    pub fn new() -> Self {
        Self {
            miners: HashMap::new(),
            emission_factors: HashMap::new(),
            hardware_baselines: HashMap::new(),
            reports: HashMap::new(),
            // Default trusted REC registries. Operators can extend this set via
            // `register_trusted_rec_issuer`. Matching is case-insensitive.
            trusted_rec_issuers: HashSet::from([
                "US-EPA".to_string(),
                "EU-ETS".to_string(),
                "GREEN-E".to_string(),
            ]),
            consumed_rec_certificates: HashMap::new(),
        }
    }

    /// Register a trusted REC issuer. Certificates are only eligible to be marked
    /// `Verified` when their issuer matches one of the registered organizations.
    /// Issuer names are matched case-insensitively (leading/trailing whitespace ignored).
    pub fn register_trusted_rec_issuer(&mut self, issuer: &str) {
        self.trusted_rec_issuers
            .insert(issuer.trim().to_ascii_uppercase());
    }

    /// Register a new miner
    pub fn register_miner(&mut self, info: MinerEnvironmentalInfo) -> Result<(), String> {
        if self.miners.contains_key(&info.miner_id) {
            return Err(format!(
                "Miner with ID {} is already registered",
                info.miner_id
            ));
        }

        let miner_id = info.miner_id.clone();
        self.miners.insert(miner_id.clone(), info);
        info!("Registered miner: {}", miner_id);

        Ok(())
    }

    /// Update an existing miner's information
    pub fn update_miner(&mut self, info: MinerEnvironmentalInfo) -> Result<(), String> {
        if !self.miners.contains_key(&info.miner_id) {
            return Err(format!("Miner with ID {} is not registered", info.miner_id));
        }

        let miner_id = info.miner_id.clone();
        self.miners.insert(miner_id.clone(), info);
        info!("Updated miner: {}", miner_id);

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
    pub fn set_hardware_baselines(&mut self, baselines: HashMap<TypesHardwareType, f64>) {
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
        self.miners
            .values()
            .filter(|info| {
                info.is_verification_valid()
                    && info.renewable_percentage >= 50.0
                    && info.has_rec_certificates
            })
            .collect()
    }

    /// Get miners with carbon offset claims
    pub fn get_offset_miners(&self) -> Vec<&MinerEnvironmentalInfo> {
        self.miners
            .values()
            .filter(|info| info.has_carbon_offsets && info.is_verification_valid())
            .collect()
    }

    /// Calculate average efficiency of all miners
    pub fn calculate_average_efficiency(&self) -> Option<f64> {
        let efficiencies: Vec<f64> = self
            .miners
            .values()
            .filter_map(|info| info.calculate_energy_efficiency())
            .collect();

        if efficiencies.is_empty() {
            return None;
        }

        Some(efficiencies.iter().sum::<f64>() / efficiencies.len() as f64)
    }

    /// Compare a miner's efficiency to the hardware baseline
    pub fn compare_to_baseline(&self, miner_id: &str) -> Result<f64, String> {
        let info = self
            .get_miner(miner_id)
            .ok_or_else(|| format!("Miner with ID {} not found", miner_id))?;

        let efficiency = info
            .calculate_energy_efficiency()
            .ok_or_else(|| "Cannot calculate efficiency without hashrate".to_string())?;

        // Find the most efficient hardware type as a baseline
        let baseline = info
            .hardware_types
            .iter()
            .filter_map(|hw| self.hardware_baselines.get(hw))
            .min_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
            .ok_or_else(|| "No baseline available for miner's hardware types".to_string())?;

        // Calculate ratio (lower is better)
        Ok(efficiency / baseline)
    }

    /// Generate a network-wide environmental report
    pub fn generate_report(&self) -> MinerEnvironmentalReport {
        let miners = self.list_miners();

        // Count verified miners
        let verified_miners = miners
            .iter()
            .filter(|miner| miner.is_verification_valid())
            .count();

        // Calculate average renewable percentage
        let average_renewable_percentage = if !miners.is_empty() {
            miners
                .iter()
                .map(|miner| miner.renewable_percentage)
                .sum::<f64>()
                / miners.len() as f64
        } else {
            0.0
        };

        // Calculate total hashrate
        let total_hashrate = miners.iter().map(|miner| miner.total_hashrate).sum();

        // Calculate total energy consumption
        let total_energy_consumption = miners
            .iter()
            .map(|miner| miner.energy_consumption_kwh_day)
            .sum();

        // Count miners with verified RECs
        let green_miners = miners
            .iter()
            .filter(|miner| miner.has_verified_recs())
            .count();

        // Count miners with offsets
        let offset_miners = miners
            .iter()
            .filter(|miner| miner.has_verified_offsets())
            .count();

        // Calculate average efficiency
        let efficiency_values: Vec<f64> = miners
            .iter()
            .filter_map(|miner| miner.calculate_energy_efficiency())
            .collect();

        let average_efficiency = if !efficiency_values.is_empty() {
            Some(efficiency_values.iter().sum::<f64>() / efficiency_values.len() as f64)
        } else {
            None
        };

        // Calculate REC coverage percentage
        let rec_coverage_percentage = if total_energy_consumption > 0.0 {
            let total_verified_recs_mwh: f64 = miners
                .iter()
                .map(|miner| miner.total_verified_recs_mwh())
                .sum();

            let annual_energy_mwh = total_energy_consumption * 365.0 / 1000.0;
            Some(f64::min(
                total_verified_recs_mwh / annual_energy_mwh * 100.0,
                100.0,
            ))
        } else {
            None
        };

        MinerEnvironmentalReport {
            timestamp: chrono::Utc::now(),
            total_miners: miners.len(),
            verified_miners,
            average_renewable_percentage,
            total_hashrate,
            total_energy_consumption_kwh_day: total_energy_consumption,
            green_miners,
            offset_miners,
            average_efficiency,
            rec_coverage_percentage,
        }
    }

    /// Submit a new environmental report
    pub fn submit_report(&mut self, report: MinerEnvironmentalReport) {
        // Generate a report ID from the timestamp to use as key
        let report_id = format!("report-{}", report.timestamp.timestamp());
        self.reports.insert(report_id, report);
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
        if let Some(_report) = self.reports.get_mut(miner_id) {
            // Access original MinerEnvironmentalInfo methods that exist
            if let Some(miner_info) = self.miners.get_mut(miner_id) {
                miner_info.add_verification(
                    verification.provider.clone(),
                    verification.reference.clone(),
                    verification.status,
                );
            }

            // Determine report status based on verification status
            match verification.status {
                MinerVerificationStatus::Verified => {
                    // Approve the report
                    self.update_miner_status(miner_id, true);
                    true
                }
                MinerVerificationStatus::Rejected => {
                    // Reject the report
                    self.update_miner_status(miner_id, false);
                    true
                }
                _ => false,
            }
        } else {
            false
        }
    }

    /// Update miner status after verification
    fn update_miner_status(&mut self, miner_id: &str, verified: bool) {
        // Implementation would update status in a real system
        info!("Updated miner {} status: verified={}", miner_id, verified);
    }

    /// Calculate network-wide renewable energy percentage
    pub fn calculate_network_renewable_percentage(&self) -> f64 {
        let mut total_renewable = 0.0;
        let mut total_miners = 0;

        // Use miners information directly instead of reports
        for miner_info in self.miners.values() {
            if miner_info.is_verification_valid() {
                total_renewable += miner_info.renewable_percentage;
                total_miners += 1;
            }
        }

        if total_miners > 0 {
            total_renewable / total_miners as f64
        } else {
            0.0
        }
    }

    /// Record a miner location verification claim.
    ///
    /// The resulting [`LocationVerification::status`] is derived from the method and
    /// the supporting evidence; it is NEVER unconditionally
    /// [`MinerVerificationStatus::Verified`]. A [`MinerVerificationStatus::Verified`]
    /// status is what unlocks the location fee discount (see
    /// [`Self::calculate_fee_discount_with_rec_priority`], `location_bonus`), so
    /// treating an unproven claim as `Verified` would let a miner earn renewable
    /// incentives with no evidence.
    ///
    /// Status rules:
    /// 1. [`LocationVerificationMethod::SelfDeclared`] is a bare claim and can never be
    ///    self-verified — it is always recorded as [`MinerVerificationStatus::Pending`]
    ///    awaiting external verification, regardless of any `evidence` string.
    /// 2. Evidence-backed methods (Audit, GovernmentRegistry, MultiFactor,
    ///    CryptographicProof, IPGeolocation) are recorded as
    ///    [`MinerVerificationStatus::Verified`] only when a non-empty `evidence`
    ///    reference is supplied; otherwise they stay [`MinerVerificationStatus::Pending`].
    ///
    /// This function records the claim and its evidence reference but does not itself
    /// cryptographically validate the evidence or identify an independent verifier, so
    /// no method is auto-verified without at least a supporting evidence reference.
    pub fn verify_miner_location(
        &mut self,
        miner_id: &str,
        method: LocationVerificationMethod,
        evidence: Option<String>,
    ) -> Result<(), String> {
        let miner = match self.miners.get_mut(miner_id) {
            Some(miner) => miner,
            None => return Err(format!("Miner with ID {} not found", miner_id)),
        };

        // Determine confidence level based on verification method
        let confidence = match method {
            LocationVerificationMethod::MultiFactor => 0.95,
            LocationVerificationMethod::Audit => 0.9,
            LocationVerificationMethod::GovernmentRegistry => 0.85,
            LocationVerificationMethod::CryptographicProof => 0.8,
            LocationVerificationMethod::IPGeolocation => 0.6,
            LocationVerificationMethod::SelfDeclared => 0.3,
        };

        // A non-empty evidence reference is the minimum objective basis this function
        // can attest to.
        let has_evidence = evidence
            .as_ref()
            .map(|e| !e.trim().is_empty())
            .unwrap_or(false);

        // Derive status honestly instead of rubber-stamping every claim as Verified.
        let status = match method {
            // Self-declaration is never itself proof — always pending external review.
            LocationVerificationMethod::SelfDeclared => MinerVerificationStatus::Pending,
            // All other methods require a supporting evidence reference to count as
            // verified; without one the claim remains pending.
            _ => {
                if has_evidence {
                    MinerVerificationStatus::Verified
                } else {
                    MinerVerificationStatus::Pending
                }
            }
        };

        // Create verification record
        let verification = LocationVerification {
            method,
            timestamp: Utc::now(),
            confidence,
            verifier: None,
            evidence_reference: evidence,
            status,
        };

        // Update miner record
        miner.set_location_verification(verification);

        Ok(())
    }

    /// Verify a REC certificate against the trusted-issuer registry and double-claim
    /// tracking, then record the result on the miner's certificate.
    ///
    /// A certificate is only marked [`MinerVerificationStatus::Verified`] when ALL of the
    /// following hold:
    /// 1. Its issuer is on the trusted-issuer list (see [`Self::register_trusted_rec_issuer`]).
    /// 2. Its generation period is well-formed (`start < end`) and not in the future.
    /// 3. Its claimed amount is positive.
    /// 4. The certificate has not already been claimed by a different miner.
    ///
    /// If any check fails the certificate is marked [`MinerVerificationStatus::Rejected`]
    /// (never silently `Verified`) and the failure reason is returned as an error. This
    /// prevents a caller from obtaining renewable-energy incentives (environmental score,
    /// fee discounts, carbon-footprint reductions) for unproven or double-claimed RECs.
    pub fn verify_rec_certificate(
        &mut self,
        miner_id: &str,
        certificate_id: &str,
    ) -> Result<(), String> {
        // Extract the details we need to validate without holding a mutable borrow.
        let (cert_index, issuer, generation_start, generation_end, amount_mwh) = {
            let miner = self
                .miners
                .get(miner_id)
                .ok_or_else(|| format!("Miner with ID {} not found", miner_id))?;

            let idx = miner
                .rec_certificates
                .iter()
                .position(|cert| cert.certificate_id == certificate_id)
                .ok_or_else(|| format!("Certificate with ID {} not found", certificate_id))?;

            let cert = &miner.rec_certificates[idx];
            (
                idx,
                cert.issuer.clone(),
                cert.generation_start,
                cert.generation_end,
                cert.amount_mwh,
            )
        };

        // Run all validation checks. Any Err means the certificate is Rejected.
        let validation: Result<(), String> = (|| {
            // 1. Issuer must be on the trusted list.
            let issuer_norm = issuer.trim().to_ascii_uppercase();
            if !self.trusted_rec_issuers.contains(&issuer_norm) {
                return Err(format!(
                    "REC issuer '{}' is not a trusted issuer",
                    issuer
                ));
            }

            // 2. Generation period must be well-formed and not claimed for the future.
            if generation_start >= generation_end {
                return Err("REC generation period is invalid (start >= end)".to_string());
            }
            if generation_end > Utc::now() {
                return Err("REC generation period ends in the future".to_string());
            }

            // 3. Claimed amount must be a positive, finite value (rejects 0, negatives, NaN, infinity).
            if !amount_mwh.is_finite() || amount_mwh <= 0.0 {
                return Err("REC amount must be greater than zero".to_string());
            }

            // 4. Certificate must not already be claimed by a different miner.
            if let Some(existing) = self.consumed_rec_certificates.get(certificate_id) {
                if existing != miner_id {
                    return Err(format!(
                        "REC certificate {} has already been claimed by miner {}",
                        certificate_id, existing
                    ));
                }
            }

            Ok(())
        })();

        let miner = self
            .miners
            .get_mut(miner_id)
            .ok_or_else(|| format!("Miner with ID {} not found", miner_id))?;

        match validation {
            Ok(()) => {
                miner.rec_certificates[cert_index].verification_status =
                    MinerVerificationStatus::Verified;
                miner.rec_certificates[cert_index].last_verified = Some(Utc::now());
                // Only reflect "has RECs" when at least one certificate is verified.
                miner.has_rec_certificates = true;
                // Record the claim so no other miner can double-claim this certificate.
                self.consumed_rec_certificates
                    .insert(certificate_id.to_string(), miner_id.to_string());
                Ok(())
            }
            Err(reason) => {
                miner.rec_certificates[cert_index].verification_status =
                    MinerVerificationStatus::Rejected;
                miner.rec_certificates[cert_index].last_verified = Some(Utc::now());
                // Recompute the flag: true only if some other certificate is verified.
                miner.has_rec_certificates = miner
                    .rec_certificates
                    .iter()
                    .any(|c| c.verification_status == MinerVerificationStatus::Verified);
                Err(reason)
            }
        }
    }

    /// Calculate fee discount with REC prioritization
    pub fn calculate_fee_discount_with_rec_priority(&self, miner_id: &str) -> f64 {
        let info = match self.miners.get(miner_id) {
            Some(info) => info,
            None => return 0.0, // No discount for non-registered miners
        };

        // Base discount from renewable percentage.
        //
        // `renewable_percentage` is a pure self-report (set by the miner via
        // `update_energy_sources` with no supporting evidence), so it must NOT grant a
        // fee discount on its own. Gate the base tier on verified evidence: the miner
        // must both hold a valid external verification (`is_verification_valid`) AND back
        // the renewable claim with at least one verified REC certificate
        // (`has_verified_recs`, which round-3 hardening ties to trusted issuers and
        // no double-claiming). Without that evidence the self-reported percentage yields
        // no base discount — mirroring how the REC/offset/location bonuses below are
        // already gated on `Verified` status.
        let base_discount = if info.is_verification_valid() && info.has_verified_recs() {
            if info.renewable_percentage >= 95.0 {
                10.0 // 10% discount for 95%+ renewable
            } else if info.renewable_percentage >= 75.0 {
                7.0 // 7% discount for 75%+ renewable
            } else if info.renewable_percentage >= 50.0 {
                5.0 // 5% discount for 50%+ renewable
            } else if info.renewable_percentage >= 25.0 {
                2.0 // 2% discount for 25%+ renewable
            } else {
                0.0 // No discount for less than 25% renewable
            }
        } else {
            0.0 // Unverified self-reported renewable claim earns no base discount
        };

        // REC bonus - prioritize RECs over everything else
        let rec_bonus = if info.has_verified_recs() {
            // Calculate REC coverage percentage relative to energy consumption
            let annual_energy_mwh = info.energy_consumption_kwh_day * 365.0 / 1000.0;
            let rec_coverage = (info.total_verified_recs_mwh() / annual_energy_mwh).min(1.0);

            // Bonus based on REC coverage
            rec_coverage * 5.0 // Up to 5% additional discount
        } else {
            0.0
        };

        // Offset bonus - smaller bonus for offsets
        let offset_bonus = if info.has_verified_offsets() {
            2.0 // 2% additional discount for verified offsets
        } else {
            0.0
        };

        // Location verification bonus
        let location_bonus = if let Some(verification) = &info.location_verification {
            if verification.status == MinerVerificationStatus::Verified {
                verification.confidence * 3.0 // Up to 3% additional discount
            } else {
                0.0
            }
        } else {
            0.0
        };

        // Total discount
        base_discount + rec_bonus + offset_bonus + location_bonus
    }

    /// Get miners with verified REC certificates (prioritize over offsets)
    pub fn get_verified_rec_miners(&self) -> Vec<&MinerEnvironmentalInfo> {
        self.miners
            .values()
            .filter(|info| info.has_verified_recs())
            .collect()
    }

    /// Generate a network-wide environmental report with REC prioritization
    pub fn generate_report_with_rec_priority(&self) -> MinerEnvironmentalReport {
        let miners = self.list_miners();

        // Count verified miners
        let verified_miners = miners
            .iter()
            .filter(|miner| miner.is_verification_valid())
            .count();

        // Calculate average renewable percentage
        let average_renewable_percentage = if !miners.is_empty() {
            miners
                .iter()
                .map(|miner| miner.renewable_percentage)
                .sum::<f64>()
                / miners.len() as f64
        } else {
            0.0
        };

        // Calculate total hashrate
        let total_hashrate = miners.iter().map(|miner| miner.total_hashrate).sum();

        // Calculate total energy consumption
        let total_energy_consumption = miners
            .iter()
            .map(|miner| miner.energy_consumption_kwh_day)
            .sum();

        // Count miners with verified RECs
        let green_miners = miners
            .iter()
            .filter(|miner| miner.has_verified_recs())
            .count();

        // Count miners with offsets
        let offset_miners = miners
            .iter()
            .filter(|miner| miner.has_verified_offsets())
            .count();

        // Calculate average efficiency
        let efficiency_values: Vec<f64> = miners
            .iter()
            .filter_map(|miner| miner.calculate_energy_efficiency())
            .collect();

        let average_efficiency = if !efficiency_values.is_empty() {
            Some(efficiency_values.iter().sum::<f64>() / efficiency_values.len() as f64)
        } else {
            None
        };

        // Calculate REC coverage percentage with priority to verified RECs
        let rec_coverage_percentage = if total_energy_consumption > 0.0 {
            let total_verified_recs_mwh: f64 = miners
                .iter()
                .map(|miner| miner.total_verified_recs_mwh())
                .sum();

            let annual_energy_mwh = total_energy_consumption * 365.0 / 1000.0;
            Some(f64::min(
                total_verified_recs_mwh / annual_energy_mwh * 100.0,
                100.0,
            ))
        } else {
            None
        };

        MinerEnvironmentalReport {
            timestamp: chrono::Utc::now(),
            total_miners: miners.len(),
            verified_miners,
            average_renewable_percentage,
            total_hashrate,
            total_energy_consumption_kwh_day: total_energy_consumption,
            green_miners,
            offset_miners,
            average_efficiency,
            rec_coverage_percentage,
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
    /// Percentage of network energy covered by RECs
    pub rec_coverage_percentage: Option<f64>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_common::*;

    #[test]
    fn test_miner_carbon_footprint_calculation() {
        // Create emission factors
        let mut emission_factors = HashMap::new();
        emission_factors.insert(
            Region::NorthAmerica,
            EmissionFactor {
                grid_emissions_factor: 0.4, // 0.4 tonnes CO2e per MWh
                region_name: "North America".to_string(),
                data_source: EmissionsDataSource::IEA,
                factor_type: EmissionsFactorType::GridAverage,
                year: Some(2024),
                timestamp: None,
                confidence: Some(0.95),
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
        sources.insert(TypesEnergySource::Solar, 30.0);
        sources.insert(TypesEnergySource::Wind, 20.0);
        sources.insert(TypesEnergySource::Coal, 50.0);
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
    fn test_environmental_score_manipulation_is_rejected() {
        // A manipulated self-reported renewable percentage well above 100% would,
        // without validation, push the environmental score outside the valid 0-100
        // band. The wired-in EnvironmentalScoreValidator must clamp it back.
        let mut miner = MinerEnvironmentalInfo::new(
            "attacker".to_string(),
            "Manipulated Miner".to_string(),
            Region::NorthAmerica,
        );

        // renewable_score = (renewable_percentage / 100) * 50 => 150 with 300%.
        miner.renewable_percentage = 300.0;

        let score = miner.calculate_environmental_score();

        // The out-of-range score must be rejected/clamped to the valid maximum.
        assert!(
            (0.0..=100.0).contains(&score),
            "score {} escaped the valid 0-100 range",
            score
        );
        assert_eq!(score, 100.0);
        assert_eq!(miner.environmental_score, Some(100.0));
    }

    #[test]
    fn test_environmental_score_valid_input_unchanged() {
        // A legitimate in-range score must pass through validation unmodified.
        let mut miner = MinerEnvironmentalInfo::new(
            "honest".to_string(),
            "Honest Miner".to_string(),
            Region::NorthAmerica,
        );
        miner.renewable_percentage = 50.0; // renewable_score = 25.0, no other components

        let score = miner.calculate_environmental_score();
        assert_eq!(score, 25.0);
        assert_eq!(miner.environmental_score, Some(25.0));
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
                data_source: EmissionsDataSource::EEA,
                factor_type: EmissionsFactorType::GridAverage,
                year: Some(2024),
                timestamp: None,
                confidence: Some(0.95),
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
        sources.insert(TypesEnergySource::Hydro, 50.0);
        sources.insert(TypesEnergySource::Wind, 30.0);
        sources.insert(TypesEnergySource::NaturalGas, 20.0);
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

    fn sample_rec(certificate_id: &str, issuer: &str) -> RECCertificate {
        RECCertificate {
            certificate_id: certificate_id.to_string(),
            issuer: issuer.to_string(),
            amount_mwh: 100.0,
            generation_start: Utc::now() - chrono::Duration::days(60),
            generation_end: Utc::now() - chrono::Duration::days(30),
            generation_location: Some(Region::NorthAmerica),
            energy_type: TypesEnergySource::Solar,
            verification_status: MinerVerificationStatus::Pending,
            certificate_url: None,
            last_verified: None,
            blockchain_tx_id: None,
        }
    }

    fn register_miner_with_cert(
        manager: &mut MinerReportingManager,
        miner_id: &str,
        cert: RECCertificate,
    ) {
        let mut miner = MinerEnvironmentalInfo::new(
            miner_id.to_string(),
            miner_id.to_string(),
            Region::NorthAmerica,
        );
        miner.add_rec_certificate(cert);
        manager.register_miner(miner).unwrap();
    }

    #[test]
    fn test_verify_rec_certificate_trusted_issuer_is_verified() {
        let mut manager = MinerReportingManager::new();
        register_miner_with_cert(&mut manager, "miner1", sample_rec("REC-1", "US-EPA"));

        assert!(manager.verify_rec_certificate("miner1", "REC-1").is_ok());

        let miner = manager.get_miner("miner1").unwrap();
        assert_eq!(
            miner.rec_certificates[0].verification_status,
            MinerVerificationStatus::Verified
        );
        assert!(miner.has_verified_recs());
    }

    #[test]
    fn test_verify_rec_certificate_untrusted_issuer_is_rejected() {
        let mut manager = MinerReportingManager::new();
        // "Totally Fake Registry" is not on the trusted list.
        register_miner_with_cert(
            &mut manager,
            "miner1",
            sample_rec("REC-1", "Totally Fake Registry"),
        );

        let result = manager.verify_rec_certificate("miner1", "REC-1");
        assert!(result.is_err());

        let miner = manager.get_miner("miner1").unwrap();
        assert_eq!(
            miner.rec_certificates[0].verification_status,
            MinerVerificationStatus::Rejected
        );
        // A rejected certificate must NOT drive incentives.
        assert!(!miner.has_verified_recs());
        assert!(!miner.has_rec_certificates);
    }

    #[test]
    fn test_verify_rec_certificate_future_period_is_rejected() {
        let mut manager = MinerReportingManager::new();
        let mut cert = sample_rec("REC-1", "US-EPA");
        // Generation period ends in the future.
        cert.generation_start = Utc::now() + chrono::Duration::days(1);
        cert.generation_end = Utc::now() + chrono::Duration::days(30);
        register_miner_with_cert(&mut manager, "miner1", cert);

        assert!(manager.verify_rec_certificate("miner1", "REC-1").is_err());
        assert_eq!(
            manager.get_miner("miner1").unwrap().rec_certificates[0].verification_status,
            MinerVerificationStatus::Rejected
        );
    }

    #[test]
    fn test_verify_rec_certificate_double_claim_is_rejected() {
        let mut manager = MinerReportingManager::new();
        register_miner_with_cert(&mut manager, "miner1", sample_rec("REC-DUP", "GREEN-E"));
        register_miner_with_cert(&mut manager, "miner2", sample_rec("REC-DUP", "GREEN-E"));

        // First miner legitimately claims the certificate.
        assert!(manager.verify_rec_certificate("miner1", "REC-DUP").is_ok());
        // Second miner attempts to claim the SAME certificate ID -> rejected.
        assert!(manager.verify_rec_certificate("miner2", "REC-DUP").is_err());

        assert_eq!(
            manager.get_miner("miner2").unwrap().rec_certificates[0].verification_status,
            MinerVerificationStatus::Rejected
        );
        assert!(manager.get_miner("miner1").unwrap().has_verified_recs());
    }

    #[test]
    fn test_verify_rec_certificate_infinite_amount_is_rejected() {
        let mut manager = MinerReportingManager::new();
        // Trusted issuer but a non-finite (infinite) claimed amount.
        let mut cert = sample_rec("REC-INF", "US-EPA");
        cert.amount_mwh = f64::INFINITY;
        register_miner_with_cert(&mut manager, "miner1", cert);

        // An infinite amount would poison total_verified_recs_mwh() and rec_coverage,
        // so it must be rejected just like NaN, zero, and negatives.
        assert!(manager.verify_rec_certificate("miner1", "REC-INF").is_err());
        let miner = manager.get_miner("miner1").unwrap();
        assert_eq!(
            miner.rec_certificates[0].verification_status,
            MinerVerificationStatus::Rejected
        );
        assert!(!miner.has_verified_recs());
    }

    #[test]
    fn test_register_trusted_rec_issuer_enables_verification() {
        let mut manager = MinerReportingManager::new();
        register_miner_with_cert(&mut manager, "miner1", sample_rec("REC-1", "Green-e Custom"));

        // Not trusted yet -> rejected.
        assert!(manager.verify_rec_certificate("miner1", "REC-1").is_err());

        // Register the issuer (case-insensitive) then re-verify -> accepted.
        manager.register_trusted_rec_issuer("green-e custom");
        assert!(manager.verify_rec_certificate("miner1", "REC-1").is_ok());
        assert_eq!(
            manager.get_miner("miner1").unwrap().rec_certificates[0].verification_status,
            MinerVerificationStatus::Verified
        );
    }

    fn register_bare_miner(manager: &mut MinerReportingManager, miner_id: &str) {
        let miner = MinerEnvironmentalInfo::new(
            miner_id.to_string(),
            miner_id.to_string(),
            Region::NorthAmerica,
        );
        manager.register_miner(miner).unwrap();
    }

    #[test]
    fn test_verify_miner_location_self_declared_is_pending() {
        let mut manager = MinerReportingManager::new();
        register_bare_miner(&mut manager, "miner1");

        // Self-declaration, even with an evidence string, must never be Verified.
        assert!(manager
            .verify_miner_location(
                "miner1",
                LocationVerificationMethod::SelfDeclared,
                Some("i-swear-im-in-iceland".to_string()),
            )
            .is_ok());

        let v = manager
            .get_miner("miner1")
            .unwrap()
            .location_verification
            .clone()
            .unwrap();
        assert_eq!(v.status, MinerVerificationStatus::Pending);

        // A pending self-declaration must not unlock the location fee discount.
        assert_eq!(manager.calculate_fee_discount_with_rec_priority("miner1"), 0.0);
    }

    #[test]
    fn test_verify_miner_location_no_evidence_is_pending() {
        let mut manager = MinerReportingManager::new();
        register_bare_miner(&mut manager, "miner1");

        // A high-trust method with no supporting evidence stays Pending.
        assert!(manager
            .verify_miner_location("miner1", LocationVerificationMethod::Audit, None)
            .is_ok());

        let v = manager
            .get_miner("miner1")
            .unwrap()
            .location_verification
            .clone()
            .unwrap();
        assert_eq!(v.status, MinerVerificationStatus::Pending);
        assert_eq!(manager.calculate_fee_discount_with_rec_priority("miner1"), 0.0);
    }

    #[test]
    fn test_verify_miner_location_with_evidence_is_verified() {
        let mut manager = MinerReportingManager::new();
        register_bare_miner(&mut manager, "miner1");

        // Evidence-backed audit -> Verified, and unlocks the location fee bonus.
        assert!(manager
            .verify_miner_location(
                "miner1",
                LocationVerificationMethod::Audit,
                Some("audit-report-2026-Q1".to_string()),
            )
            .is_ok());

        let v = manager
            .get_miner("miner1")
            .unwrap()
            .location_verification
            .clone()
            .unwrap();
        assert_eq!(v.status, MinerVerificationStatus::Verified);
        // location_bonus = confidence(0.9) * 3.0 = 2.7 (miner has 0% renewable so no base).
        assert!((manager.calculate_fee_discount_with_rec_priority("miner1") - 2.7).abs() < 1e-9);
    }

    #[test]
    fn test_base_discount_requires_verified_evidence() {
        // A miner self-reports 100% renewable energy with NO verified RECs and NO
        // external verification. The self-report alone must NOT unlock any base discount.
        let mut manager = MinerReportingManager::new();
        let mut miner = MinerEnvironmentalInfo::new(
            "miner1".to_string(),
            "miner1".to_string(),
            Region::NorthAmerica,
        );
        miner.update_performance_metrics(100.0, 2400.0).unwrap();
        let mut sources = HashMap::new();
        sources.insert(TypesEnergySource::Solar, 100.0);
        miner.update_energy_sources(sources).unwrap();
        assert_eq!(miner.renewable_percentage, 100.0);
        manager.register_miner(miner).unwrap();

        // Pure self-report earns nothing: base tier is gated on verified evidence.
        assert_eq!(
            manager.calculate_fee_discount_with_rec_priority("miner1"),
            0.0
        );
    }

    #[test]
    fn test_base_discount_unlocked_by_verified_rec_and_verification() {
        // Same 100% self-reported renewable claim, but now backed by a verified REC
        // certificate AND a valid external verification. The base tier now applies.
        let mut manager = MinerReportingManager::new();
        let mut miner = MinerEnvironmentalInfo::new(
            "miner1".to_string(),
            "miner1".to_string(),
            Region::NorthAmerica,
        );
        miner.update_performance_metrics(100.0, 2400.0).unwrap();
        let mut sources = HashMap::new();
        sources.insert(TypesEnergySource::Solar, 100.0);
        miner.update_energy_sources(sources).unwrap();

        // Attach a verified REC certificate (evidence backing the renewable claim).
        let mut cert = sample_rec("REC-1", "US-EPA");
        cert.verification_status = MinerVerificationStatus::Verified;
        miner.add_rec_certificate(cert);
        assert!(miner.has_verified_recs());

        // Attach a valid external verification.
        miner.add_verification(
            "US-EPA".to_string(),
            "attestation-2026".to_string(),
            MinerVerificationStatus::Verified,
        );
        assert!(miner.is_verification_valid());

        manager.register_miner(miner).unwrap();

        // With verified evidence, the 95%+ base tier (10%) applies on top of the REC
        // bonus, so the total discount must be at least the base tier.
        assert!(manager.calculate_fee_discount_with_rec_priority("miner1") >= 10.0);
    }
}
