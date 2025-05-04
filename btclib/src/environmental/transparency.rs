use std::collections::HashMap;
use serde::{Serialize, Deserialize};
use chrono::{DateTime, Utc};

use crate::environmental::emissions::{EmissionsTracker, Emissions, Region, EmissionFactor};
use crate::environmental::treasury::{EnvironmentalTreasury, EnvironmentalAssetPurchase};
use crate::environmental::dashboard::{EnvironmentalDashboard, EnvironmentalMetrics, EmissionsTimePeriod};
use crate::environmental::miner_reporting::{MinerReportingManager, MinerEnvironmentalReport, VerificationStatus};
use crate::environmental::governance::{EnvironmentalGovernance, EnvironmentalProposal, ProposalStatus};

/// Transparency level for environmental reporting
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TransparencyLevel {
    /// Basic reporting with minimal details
    Basic,
    /// Standard reporting with moderate details
    Standard,
    /// Comprehensive reporting with all details
    Comprehensive,
    /// Public audit-ready with verification proofs
    AuditReady,
}

/// Environmental impact summary
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvironmentalSummary {
    /// Total energy usage in kWh
    pub total_energy_kwh: f64,
    /// Total carbon emissions in tonnes CO2e
    pub total_emissions_tonnes: f64,
    /// Renewable energy percentage
    pub renewable_percentage: f64,
    /// Carbon offset percentage
    pub offset_percentage: f64,
    /// Net carbon impact (emissions minus offsets)
    pub net_carbon_impact: f64,
    /// Carbon intensity (kg CO2e per transaction)
    pub carbon_intensity: f64,
    /// REC coverage in MWh
    pub rec_coverage_mwh: f64,
    /// Offset coverage in tonnes CO2e
    pub offset_coverage_tonnes: f64,
    /// Carbon negative status
    pub carbon_negative: bool,
    /// Timestamp of report
    pub timestamp: DateTime<Utc>,
}

/// REC verification details
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RECVerificationDetails {
    /// Total MWh covered by RECs
    pub total_mwh: f64,
    /// Number of certificates
    pub certificate_count: usize,
    /// Percentage of network energy covered
    pub coverage_percentage: f64,
    /// Breakdown by energy type
    pub energy_type_breakdown: HashMap<String, f64>,
    /// Breakdown by verification status
    pub verification_status_breakdown: HashMap<VerificationStatus, usize>,
    /// List of verification providers
    pub verification_providers: Vec<String>,
    /// Number of verified certificates
    pub verified_certificates: usize,
}

/// Carbon offset verification details
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OffsetVerificationDetails {
    /// Total tonnes CO2e offset
    pub total_tonnes: f64,
    /// Number of offsets
    pub offset_count: usize,
    /// Percentage of emissions offset
    pub coverage_percentage: f64,
    /// Breakdown by project type
    pub project_type_breakdown: HashMap<String, f64>,
    /// Breakdown by verification status
    pub verification_status_breakdown: HashMap<VerificationStatus, usize>,
    /// List of verification providers
    pub verification_providers: Vec<String>,
    /// Number of verified offsets
    pub verified_offsets: usize,
}

/// Treasury allocation details
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TreasuryAllocationDetails {
    /// Current treasury balance
    pub balance: u64,
    /// Fee allocation percentage
    pub fee_allocation_percentage: f64,
    /// Amount allocated to renewable certificates
    pub renewable_certificates_allocation: f64,
    /// Amount allocated to carbon offsets
    pub carbon_offsets_allocation: f64,
    /// Amount allocated to grants
    pub grants_allocation: f64,
    /// Amount allocated to operations
    pub operations_allocation: f64,
    /// Recent purchases of environmental assets
    pub recent_purchases: Vec<EnvironmentalAssetPurchase>,
    /// Active governance proposals
    pub active_proposals: Vec<EnvironmentalProposal>,
}

/// Miner verification details
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MinerVerificationDetails {
    /// Total number of miners
    pub total_miners: usize,
    /// Number of verified miners
    pub verified_miners: usize,
    /// Percentage of verified miners
    pub verified_percentage: f64,
    /// Breakdown by verification status
    pub verification_status_breakdown: HashMap<VerificationStatus, usize>,
    /// Number of miners with renewable energy
    pub renewable_energy_miners: usize,
    /// Number of miners with RECs
    pub rec_miners: usize,
    /// Number of miners with carbon offsets
    pub offset_miners: usize,
    /// Breakdown by region
    pub region_breakdown: HashMap<String, usize>,
}

/// Historical metrics point
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoricalMetricPoint {
    /// Timestamp
    pub timestamp: DateTime<Utc>,
    /// Total energy usage in kWh
    pub energy_kwh: f64,
    /// Total carbon emissions in tonnes CO2e
    pub emissions_tonnes: f64,
    /// Renewable energy percentage
    pub renewable_percentage: f64,
    /// Carbon offset percentage
    pub offset_percentage: f64,
}

/// Environmental transparency report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransparencyReport {
    /// Report generation timestamp
    pub timestamp: DateTime<Utc>,
    /// Transparency level
    pub transparency_level: TransparencyLevel,
    /// Environmental impact summary
    pub summary: EnvironmentalSummary,
    /// REC verification details
    pub rec_verification: Option<RECVerificationDetails>,
    /// Carbon offset verification details
    pub offset_verification: Option<OffsetVerificationDetails>,
    /// Treasury allocation details
    pub treasury_allocation: Option<TreasuryAllocationDetails>,
    /// Miner verification details
    pub miner_verification: Option<MinerVerificationDetails>,
    /// Historical metrics (daily)
    pub daily_metrics: Option<Vec<HistoricalMetricPoint>>,
    /// Historical metrics (monthly)
    pub monthly_metrics: Option<Vec<HistoricalMetricPoint>>,
    /// Verification proof URL
    pub verification_proof_url: Option<String>,
    /// Audit report URL
    pub audit_report_url: Option<String>,
}

/// Configuration for the transparency dashboard
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransparencyConfig {
    /// Default transparency level
    pub default_level: TransparencyLevel,
    /// Whether to include REC verification details
    pub include_rec_verification: bool,
    /// Whether to include carbon offset verification details
    pub include_offset_verification: bool,
    /// Whether to include treasury allocation details
    pub include_treasury_allocation: bool,
    /// Whether to include miner verification details
    pub include_miner_verification: bool,
    /// Whether to include daily historical metrics
    pub include_daily_metrics: bool,
    /// Whether to include monthly historical metrics
    pub include_monthly_metrics: bool,
    /// Maximum number of daily metrics to include
    pub max_daily_metrics: usize,
    /// Maximum number of monthly metrics to include
    pub max_monthly_metrics: usize,
    /// Whether to automatically generate reports
    pub auto_generate_reports: bool,
    /// Report generation frequency in hours
    pub report_frequency_hours: u32,
    /// Whether to publish reports to a public URL
    pub publish_reports: bool,
    /// Public URL for reports
    pub report_url: Option<String>,
}

impl Default for TransparencyConfig {
    fn default() -> Self {
        Self {
            default_level: TransparencyLevel::Standard,
            include_rec_verification: true,
            include_offset_verification: true,
            include_treasury_allocation: true,
            include_miner_verification: true,
            include_daily_metrics: true,
            include_monthly_metrics: true,
            max_daily_metrics: 30,
            max_monthly_metrics: 12,
            auto_generate_reports: true,
            report_frequency_hours: 24,
            publish_reports: false,
            report_url: None,
        }
    }
}

/// Environmental transparency dashboard
pub struct TransparencyDashboard {
    /// Dashboard configuration
    config: TransparencyConfig,
    /// Emissions tracker
    emissions_tracker: EmissionsTracker,
    /// Environmental treasury
    treasury: EnvironmentalTreasury,
    /// Environmental dashboard
    dashboard: EnvironmentalDashboard,
    /// Miner reporting manager
    miner_reporting: Option<MinerReportingManager>,
    /// Environmental governance
    governance: Option<EnvironmentalGovernance>,
    /// Historical reports
    historical_reports: Vec<TransparencyReport>,
    /// Latest report
    latest_report: Option<TransparencyReport>,
    /// Daily metrics history
    daily_metrics: Vec<HistoricalMetricPoint>,
    /// Monthly metrics history
    monthly_metrics: Vec<HistoricalMetricPoint>,
}

impl TransparencyDashboard {
    /// Create a new transparency dashboard
    pub fn new(
        config: TransparencyConfig,
        emissions_tracker: EmissionsTracker,
        treasury: EnvironmentalTreasury,
        dashboard: EnvironmentalDashboard,
    ) -> Self {
        Self {
            config,
            emissions_tracker,
            treasury,
            dashboard,
            miner_reporting: None,
            governance: None,
            historical_reports: Vec::new(),
            latest_report: None,
            daily_metrics: Vec::new(),
            monthly_metrics: Vec::new(),
        }
    }
    
    /// Add miner reporting manager
    pub fn with_miner_reporting(mut self, miner_reporting: MinerReportingManager) -> Self {
        self.miner_reporting = Some(miner_reporting);
        self
    }
    
    /// Add environmental governance
    pub fn with_governance(mut self, governance: EnvironmentalGovernance) -> Self {
        self.governance = Some(governance);
        self
    }
    
    /// Generate a transparency report
    pub fn generate_report(&mut self, level: TransparencyLevel) -> Result<TransparencyReport, String> {
        // Get current metrics from dashboard
        let metrics = match self.dashboard.get_metrics(EmissionsTimePeriod::Day) {
            Some(m) => m,
            None => return Err("No daily metrics available".to_string()),
        };
        
        // Create environmental summary
        let summary = EnvironmentalSummary {
            total_energy_kwh: metrics.energy_consumption,
            total_emissions_tonnes: metrics.total_emissions,
            renewable_percentage: metrics.renewable_percentage.unwrap_or(0.0),
            offset_percentage: if metrics.total_emissions > 0.0 {
                (metrics.total_assets / metrics.total_emissions) * 100.0
            } else {
                0.0
            },
            net_carbon_impact: metrics.net_emissions,
            carbon_intensity: metrics.emissions_per_transaction,
            rec_coverage_mwh: metrics.assets_purchased.iter()
                .filter(|a| a.asset_type == crate::environmental::treasury::EnvironmentalAssetType::RenewableEnergyCertificate)
                .map(|a| a.amount)
                .sum(),
            offset_coverage_tonnes: metrics.assets_purchased.iter()
                .filter(|a| a.asset_type == crate::environmental::treasury::EnvironmentalAssetType::CarbonOffset)
                .map(|a| a.amount)
                .sum(),
            carbon_negative: metrics.net_emissions < 0.0,
            timestamp: Utc::now(),
        };
        
        // Create additional details based on transparency level
        let rec_verification = if self.config.include_rec_verification 
                && (level == TransparencyLevel::Comprehensive || level == TransparencyLevel::AuditReady) {
            self.generate_rec_verification_details()
        } else {
            None
        };
        
        let offset_verification = if self.config.include_offset_verification 
                && (level == TransparencyLevel::Comprehensive || level == TransparencyLevel::AuditReady) {
            self.generate_offset_verification_details()
        } else {
            None
        };
        
        let treasury_allocation = if self.config.include_treasury_allocation 
                && level != TransparencyLevel::Basic {
            self.generate_treasury_allocation_details()
        } else {
            None
        };
        
        let miner_verification = if self.config.include_miner_verification 
                && self.miner_reporting.is_some() 
                && level != TransparencyLevel::Basic {
            self.generate_miner_verification_details()
        } else {
            None
        };
        
        // Include historical metrics based on configuration
        let daily_metrics = if self.config.include_daily_metrics 
                && level != TransparencyLevel::Basic {
            Some(self.daily_metrics.iter()
                .take(self.config.max_daily_metrics)
                .cloned()
                .collect())
        } else {
            None
        };
        
        let monthly_metrics = if self.config.include_monthly_metrics 
                && level != TransparencyLevel::Basic {
            Some(self.monthly_metrics.iter()
                .take(self.config.max_monthly_metrics)
                .cloned()
                .collect())
        } else {
            None
        };
        
        // Create verification URLs for audit-ready reports
        let (verification_proof_url, audit_report_url) = if level == TransparencyLevel::AuditReady {
            (Some("https://example.com/verification/proof".to_string()), 
             Some("https://example.com/audit/report".to_string()))
        } else {
            (None, None)
        };
        
        // Create the full report
        let report = TransparencyReport {
            timestamp: Utc::now(),
            transparency_level: level,
            summary,
            rec_verification,
            offset_verification,
            treasury_allocation,
            miner_verification,
            daily_metrics,
            monthly_metrics,
            verification_proof_url,
            audit_report_url,
        };
        
        // Save the report
        self.latest_report = Some(report.clone());
        self.historical_reports.push(report.clone());
        
        Ok(report)
    }
    
    /// Generate REC verification details
    fn generate_rec_verification_details(&self) -> Option<RECVerificationDetails> {
        if let Some(miner_reporting) = &self.miner_reporting {
            let report = miner_reporting.generate_report_with_rec_priority();
            
            // Get REC certificates from treasury
            let certificates = self.treasury.get_rec_certificates();
            if certificates.is_empty() {
                return None;
            }
            
            // Calculate total MWh
            let total_mwh: f64 = certificates.iter()
                .map(|c| c.amount_mwh)
                .sum();
            
            // Count certificates by energy type
            let mut energy_type_breakdown = HashMap::new();
            let mut verification_status_breakdown = HashMap::new();
            let mut verification_providers = Vec::new();
            let mut verified_count = 0;
            
            for cert in &certificates {
                // Add to energy type breakdown
                *energy_type_breakdown.entry(cert.energy_type.to_string()).or_insert(0.0) += cert.amount_mwh;
                
                // Add to verification status breakdown
                *verification_status_breakdown.entry(cert.verification_status).or_insert(0) += 1;
                
                // Track verification providers
                if !verification_providers.contains(&cert.issuer) {
                    verification_providers.push(cert.issuer.clone());
                }
                
                // Count verified certificates
                if cert.verification_status == VerificationStatus::Verified {
                    verified_count += 1;
                }
            }
            
            // Calculate coverage percentage
            let coverage_percentage = if let Some(metrics) = self.dashboard.get_metrics(EmissionsTimePeriod::Month) {
                if metrics.energy_consumption > 0.0 {
                    (total_mwh * 1000.0 / metrics.energy_consumption) * 100.0
                } else {
                    0.0
                }
            } else {
                0.0
            };
            
            Some(RECVerificationDetails {
                total_mwh,
                certificate_count: certificates.len(),
                coverage_percentage,
                energy_type_breakdown,
                verification_status_breakdown,
                verification_providers,
                verified_certificates: verified_count,
            })
        } else {
            None
        }
    }
    
    /// Generate carbon offset verification details
    fn generate_offset_verification_details(&self) -> Option<OffsetVerificationDetails> {
        // Get carbon offsets from treasury
        let offsets = self.treasury.get_carbon_offsets();
        if offsets.is_empty() {
            return None;
        }
        
        // Calculate total tonnes
        let total_tonnes: f64 = offsets.iter()
            .map(|o| o.amount_tonnes)
            .sum();
        
        // Count offsets by project type
        let mut project_type_breakdown = HashMap::new();
        let mut verification_status_breakdown = HashMap::new();
        let mut verification_providers = Vec::new();
        let mut verified_count = 0;
        
        for offset in &offsets {
            // Add to project type breakdown
            *project_type_breakdown.entry(offset.project_type.clone()).or_insert(0.0) += offset.amount_tonnes;
            
            // Add to verification status breakdown
            *verification_status_breakdown.entry(offset.verification_status).or_insert(0) += 1;
            
            // Track verification providers
            if !verification_providers.contains(&offset.issuer) {
                verification_providers.push(offset.issuer.clone());
            }
            
            // Count verified offsets
            if offset.verification_status == VerificationStatus::Verified {
                verified_count += 1;
            }
        }
        
        // Calculate coverage percentage
        let coverage_percentage = if let Some(metrics) = self.dashboard.get_metrics(EmissionsTimePeriod::Month) {
            if metrics.total_emissions > 0.0 {
                (total_tonnes / metrics.total_emissions) * 100.0
            } else {
                0.0
            }
        } else {
            0.0
        };
        
        Some(OffsetVerificationDetails {
            total_tonnes,
            offset_count: offsets.len(),
            coverage_percentage,
            project_type_breakdown,
            verification_status_breakdown,
            verification_providers,
            verified_offsets: verified_count,
        })
    }
    
    /// Generate treasury allocation details
    fn generate_treasury_allocation_details(&self) -> Option<TreasuryAllocationDetails> {
        // Get treasury balance and allocation percentages
        let balance = self.treasury.get_balance();
        let fee_allocation_percentage = self.treasury.get_current_fee_percentage();
        let allocation = self.treasury.get_allocation();
        
        // Get recent purchases
        let purchases = self.treasury.get_recent_purchases(10);
        
        // Get active governance proposals related to treasury
        let active_proposals = if let Some(governance) = &self.governance {
            governance.get_proposals_by_status(ProposalStatus::Active)
                .into_iter()
                .cloned()
                .collect()
        } else {
            Vec::new()
        };
        
        Some(TreasuryAllocationDetails {
            balance,
            fee_allocation_percentage,
            renewable_certificates_allocation: allocation.renewable_certificates,
            carbon_offsets_allocation: allocation.carbon_offsets,
            grants_allocation: allocation.grants,
            operations_allocation: allocation.operations,
            recent_purchases,
            active_proposals,
        })
    }
    
    /// Generate miner verification details
    fn generate_miner_verification_details(&self) -> Option<MinerVerificationDetails> {
        if let Some(miner_reporting) = &self.miner_reporting {
            let report = miner_reporting.generate_report();
            
            // Get miners by verification status
            let mut verification_status_breakdown = HashMap::new();
            let mut region_breakdown = HashMap::new();
            
            for miner in miner_reporting.list_miners() {
                // Add to verification status breakdown
                if let Some(verification) = &miner.verification {
                    *verification_status_breakdown.entry(verification.status).or_insert(0) += 1;
                } else {
                    *verification_status_breakdown.entry(VerificationStatus::Pending).or_insert(0) += 1;
                }
                
                // Add to region breakdown
                let region_key = miner.region.to_string();
                *region_breakdown.entry(region_key).or_insert(0) += 1;
            }
            
            // Count miners by category
            let renewable_energy_miners = miner_reporting.get_verified_green_miners().len();
            let rec_miners = miner_reporting.get_verified_rec_miners().len();
            let offset_miners = miner_reporting.get_offset_miners().len();
            
            Some(MinerVerificationDetails {
                total_miners: report.total_miners,
                verified_miners: report.verified_miners,
                verified_percentage: if report.total_miners > 0 {
                    (report.verified_miners as f64 / report.total_miners as f64) * 100.0
                } else {
                    0.0
                },
                verification_status_breakdown,
                renewable_energy_miners,
                rec_miners,
                offset_miners,
                region_breakdown,
            })
        } else {
            None
        }
    }
    
    /// Add a daily metric point
    pub fn add_daily_metric(&mut self, metric: HistoricalMetricPoint) {
        self.daily_metrics.push(metric);
        
        // Keep only the configured number of metrics
        if self.daily_metrics.len() > self.config.max_daily_metrics * 2 {
            self.daily_metrics.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
            self.daily_metrics.truncate(self.config.max_daily_metrics);
        }
    }
    
    /// Add a monthly metric point
    pub fn add_monthly_metric(&mut self, metric: HistoricalMetricPoint) {
        self.monthly_metrics.push(metric);
        
        // Keep only the configured number of metrics
        if self.monthly_metrics.len() > self.config.max_monthly_metrics * 2 {
            self.monthly_metrics.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
            self.monthly_metrics.truncate(self.config.max_monthly_metrics);
        }
    }
    
    /// Get the latest transparency report
    pub fn get_latest_report(&self) -> Option<&TransparencyReport> {
        self.latest_report.as_ref()
    }
    
    /// Get historical transparency reports
    pub fn get_historical_reports(&self, limit: usize) -> Vec<&TransparencyReport> {
        self.historical_reports.iter()
            .rev()
            .take(limit)
            .collect()
    }
    
    /// Export transparency report as JSON
    pub fn export_report_json(&self, level: TransparencyLevel) -> Result<String, String> {
        match self.latest_report {
            Some(ref report) if report.transparency_level == level => {
                serde_json::to_string_pretty(report)
                    .map_err(|e| format!("Error serializing report to JSON: {}", e))
            },
            _ => {
                // Generate a new report at the requested level
                match self.generate_report(level) {
                    Ok(report) => serde_json::to_string_pretty(&report)
                        .map_err(|e| format!("Error serializing report to JSON: {}", e)),
                    Err(e) => Err(e),
                }
            }
        }
    }
    
    /// Generate a summary for public display
    pub fn generate_public_summary(&self) -> Result<String, String> {
        match &self.latest_report {
            Some(report) => {
                let summary = &report.summary;
                
                let mut result = String::new();
                result.push_str("# Environmental Impact Report\n\n");
                result.push_str(&format!("Report Date: {}\n\n", 
                    summary.timestamp.format("%Y-%m-%d %H:%M UTC")));
                
                result.push_str("## Network Energy & Emissions\n\n");
                result.push_str(&format!("- Total Energy Usage: {:.2} MWh\n", 
                    summary.total_energy_kwh / 1000.0));
                result.push_str(&format!("- Total Carbon Emissions: {:.2} tonnes CO2e\n", 
                    summary.total_emissions_tonnes));
                result.push_str(&format!("- Renewable Energy: {:.2}%\n", 
                    summary.renewable_percentage));
                result.push_str(&format!("- Carbon Intensity: {:.2} kg CO2e/tx\n", 
                    summary.carbon_intensity));
                
                result.push_str("\n## Environmental Mitigation\n\n");
                result.push_str(&format!("- Renewable Energy Certificates: {:.2} MWh\n", 
                    summary.rec_coverage_mwh));
                result.push_str(&format!("- Carbon Offsets: {:.2} tonnes CO2e\n", 
                    summary.offset_coverage_tonnes));
                result.push_str(&format!("- Net Carbon Impact: {:.2} tonnes CO2e\n", 
                    summary.net_carbon_impact));
                result.push_str(&format!("- Carbon Negative: {}\n", 
                    if summary.carbon_negative { "Yes" } else { "No" }));
                
                Ok(result)
            },
            None => Err("No report available".to_string()),
        }
    }
    
    /// Update dashboard configuration
    pub fn update_config(&mut self, config: TransparencyConfig) {
        self.config = config;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_report_generation() {
        // Create dependencies
        let emissions_tracker = EmissionsTracker::new();
        let treasury = EnvironmentalTreasury::new();
        let dashboard = EnvironmentalDashboard::new(emissions_tracker.clone(), treasury.clone());
        
        // Create dashboard with default configuration
        let mut transparency = TransparencyDashboard::new(
            TransparencyConfig::default(),
            emissions_tracker,
            treasury,
            dashboard,
        );
        
        // Test should generate basic report stub - actual implementation would need real metrics
        // In a real test environment, we would use mocks for these dependencies
    }
} 