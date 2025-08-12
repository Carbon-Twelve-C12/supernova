use std::collections::HashMap;
use serde::{Serialize, Deserialize};
use chrono::{DateTime, Utc, Duration};
use crate::environmental::emissions::{Emissions, EmissionsTracker, Region, EmissionFactor, EmissionsConfig, HashRate};
use crate::environmental::types::EmissionsFactorType;
use crate::environmental::treasury::{EnvironmentalTreasury, EnvironmentalAssetPurchase, EnvironmentalAssetType, TreasuryConfig};
use crate::environmental::miner_reporting::{MinerReportingManager, MinerEnvironmentalReport, MinerVerificationStatus, MinerEnvironmentalInfo};
use std::fmt;
use std::path::Path;
use crate::environmental::{
    api::{NetworkEmissionsData, AssetPurchaseRecord, EnvironmentalApiTrait, MinerEmissionsData},
    types::{EnergySource, HardwareType}
};

/// Time period for emissions calculations
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EmissionsTimePeriod {
    /// Last 24 hours
    Day,
    /// Last 7 days
    Week,
    /// Last 30 days
    Month,
    /// Last 365 days
    Year,
    /// All time
    AllTime,
    /// Custom time period
    Custom {
        /// Start time
        start: DateTime<Utc>,
        /// End time
        end: DateTime<Utc>,
    },
}

/// Report type for emissions
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EmissionsReportType {
    /// Location-based emissions (grid factors)
    LocationBased,
    /// Market-based emissions (with RECs)
    MarketBased,
    /// Marginal emissions impact
    MarginalImpact,
    /// Full detailed report
    Comprehensive,
}

/// Environmental performance metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvironmentalMetrics {
    /// Time period for these metrics
    pub period: EmissionsTimePeriod,
    /// Total emissions in tonnes CO2e
    pub total_emissions: f64,
    /// Energy consumption in kWh
    pub energy_consumption: f64,
    /// Renewable energy percentage
    pub renewable_percentage: Option<f64>,
    /// Emissions per transaction in kg CO2e
    pub emissions_per_transaction: f64,
    /// Number of transactions
    pub transaction_count: u64,
    /// Environmental assets purchased
    pub assets_purchased: Vec<EnvironmentalAssetPurchase>,
    /// Total environmental assets in tonnes CO2e
    pub total_assets: f64,
    /// Net emissions (emissions - offsets) in tonnes CO2e
    pub net_emissions: f64,
    /// Location-based emissions in tonnes CO2e
    pub location_based_emissions: Option<f64>,
    /// Market-based emissions in tonnes CO2e
    pub market_based_emissions: Option<f64>,
    /// Marginal emissions impact in tonnes CO2e
    pub marginal_emissions_impact: Option<f64>,
    /// REC coverage percentage
    pub rec_coverage_percentage: Option<f64>,
    /// Timestamp of calculation
    pub calculation_time: DateTime<Utc>,
    /// Confidence level of calculation (0-1)
    pub confidence_level: Option<f64>,
}

/// REC certificate summary
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RECSummary {
    /// Total MWh covered by RECs
    pub total_mwh: f64,
    /// Number of certificates
    pub certificate_count: usize,
    /// Percentage of network energy covered
    pub coverage_percentage: f64,
    /// Breakdown by energy type
    pub energy_type_breakdown: HashMap<String, f64>,
    /// Number of verified certificates
    pub verified_certificates: usize,
}

/// Carbon offset summary
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CarbonOffsetSummary {
    /// Total tonnes CO2e offset
    pub total_tonnes: f64,
    /// Number of offsets
    pub offset_count: usize,
    /// Percentage of emissions offset
    pub coverage_percentage: f64,
    /// Breakdown by project type
    pub project_type_breakdown: HashMap<String, f64>,
    /// Number of verified offsets
    pub verified_offsets: usize,
}

/// Summary of environmental assets
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetSummary {
    /// REC summary
    pub rec_summary: Option<RECSummary>,
    /// Carbon offset summary
    pub carbon_offset_summary: Option<CarbonOffsetSummary>,
    /// Total environmental impact in tonnes CO2e
    pub total_impact: f64,
    /// Prioritization info
    pub prioritization: Option<String>,
}

/// Geographic emissions breakdown
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeographicEmissionsBreakdown {
    /// Emissions by country
    pub country_emissions: HashMap<String, f64>,
    /// Emissions by region
    pub region_emissions: HashMap<String, f64>,
    /// Energy by country
    pub country_energy: HashMap<String, f64>,
    /// Renewable percentage by country
    pub country_renewable: HashMap<String, f64>,
}

/// Dashboard display options
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashboardOptions {
    /// Whether to show regional data
    pub show_regional_data: bool,
    /// Whether to show miner-specific data
    pub show_miner_data: bool,
    /// Whether to show transaction-level details
    pub show_transaction_details: bool,
    /// Whether to show monetary values
    pub show_monetary_values: bool,
    /// Currency to display monetary values in
    pub currency: String,
    /// Emissions report type to display
    pub emissions_report_type: EmissionsReportType,
    /// Whether to prioritize RECs over offsets in display
    pub prioritize_recs: bool,
    /// Whether to show marginal emissions data
    pub show_marginal_data: bool,
    /// Whether to show confidence levels
    pub show_confidence_levels: bool,
}

impl Default for DashboardOptions {
    fn default() -> Self {
        Self {
            show_regional_data: true,
            show_miner_data: true,
            show_transaction_details: false,
            show_monetary_values: true,
            currency: "USD".to_string(),
            emissions_report_type: EmissionsReportType::Comprehensive,
            prioritize_recs: true,
            show_marginal_data: true,
            show_confidence_levels: true,
        }
    }
}

/// Represents dashboard data for environmental metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashboardData {
    network_emissions: NetworkEmissionsData,
    renewable_percentage: f64,
    total_miners: usize,
    green_miners: usize,
    hardware_distribution: HashMap<String, usize>,
    region_distribution: HashMap<String, usize>,
    recent_asset_purchases: Vec<AssetPurchaseRecord>,
    treasury_balance: f64,
    emissions_trend: Vec<EmissionsTrend>,
}

/// Represents a point in the emissions trend data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmissionsTrend {
    timestamp: DateTime<Utc>,
    emissions_value: f64,
}

/// Environmental performance dashboard
pub struct EnvironmentalDashboard {
    /// Emissions tracker reference
    emissions_tracker: EmissionsTracker,
    /// Treasury reference
    treasury: EnvironmentalTreasury,
    /// Miner reporting manager reference
    miner_reporting: Option<MinerReportingManager>,
    /// Historical metrics
    historical_metrics: HashMap<EmissionsTimePeriod, EnvironmentalMetrics>,
    /// Display options
    options: DashboardOptions,
    /// Geographic emissions breakdown
    geographic_breakdown: Option<GeographicEmissionsBreakdown>,
    /// Asset summary
    asset_summary: Option<AssetSummary>,
    /// API reference
    api: Box<dyn EnvironmentalApiTrait>,
}

impl EnvironmentalDashboard {
    /// Create a new environmental dashboard
    pub fn new(emissions_tracker: EmissionsTracker, treasury: EnvironmentalTreasury, api: Box<dyn EnvironmentalApiTrait>) -> Self {
        Self {
            emissions_tracker,
            treasury,
            miner_reporting: None,
            historical_metrics: HashMap::new(),
            options: DashboardOptions::default(),
            geographic_breakdown: None,
            asset_summary: None,
            api,
        }
    }
    
    /// Create a new environmental dashboard with miner reporting
    pub fn with_miner_reporting(
        emissions_tracker: EmissionsTracker, 
        treasury: EnvironmentalTreasury,
        miner_reporting: MinerReportingManager,
        api: Box<dyn EnvironmentalApiTrait>,
    ) -> Self {
        Self {
            emissions_tracker,
            treasury,
            miner_reporting: Some(miner_reporting),
            historical_metrics: HashMap::new(),
            options: DashboardOptions::default(),
            geographic_breakdown: None,
            asset_summary: None,
            api,
        }
    }
    
    /// Get current environmental metrics for a given time period
    pub fn get_metrics(&self, period: EmissionsTimePeriod) -> Option<&EnvironmentalMetrics> {
        self.historical_metrics.get(&period)
    }
    
    /// Generate metrics for the specified time period
    pub fn generate_metrics(&mut self, period: EmissionsTimePeriod, transaction_count: u64) -> Result<EnvironmentalMetrics, String> {
        let now = Utc::now();
        
        // Determine time range based on period
        let (start, end) = match period {
            EmissionsTimePeriod::Day => (now - Duration::days(1), now),
            EmissionsTimePeriod::Week => (now - Duration::weeks(1), now),
            EmissionsTimePeriod::Month => (now - Duration::days(30), now),
            EmissionsTimePeriod::Year => (now - Duration::days(365), now),
            EmissionsTimePeriod::AllTime => (now - Duration::days(3650), now), // ~10 years
            EmissionsTimePeriod::Custom { start, end } => (start, end),
        };
        
        // Calculate emissions for the period
        let emissions = match self.emissions_tracker.calculate_network_emissions(start, end) {
            Ok(e) => e,
            Err(e) => return Err(format!("Error calculating emissions: {:?}", e)),
        };
        
        // Get REC coverage percentage
        let rec_coverage_percentage = if let Some(miner_reporting) = &self.miner_reporting {
            let report = miner_reporting.generate_report_with_rec_priority();
            report.rec_coverage_percentage
        } else {
            None
        };
        
        // Get asset purchases from treasury
        let asset_purchases = self.treasury.get_asset_purchases(10);
        
        // Calculate total environmental assets
        let total_assets: f64 = asset_purchases.iter()
            .filter(|purchase| purchase.asset_type == EnvironmentalAssetType::REC || 
                   purchase.asset_type == EnvironmentalAssetType::CarbonOffset)
            .map(|purchase| purchase.amount)
            .sum();
        
        // Calculate net emissions (prioritizing RECs)
        let net_emissions = if let Some(market_based) = emissions.market_based_emissions {
            market_based // Market-based already accounts for RECs
        } else {
            emissions.tonnes_co2e - total_assets
        };
        
        // Generate the metrics
        let metrics = EnvironmentalMetrics {
            period,
            total_emissions: emissions.tonnes_co2e,
            energy_consumption: emissions.energy_kwh,
            renewable_percentage: emissions.renewable_percentage,
            emissions_per_transaction: if transaction_count > 0 {
                (emissions.tonnes_co2e * 1000.0) / transaction_count as f64
            } else {
                0.0
            },
            transaction_count,
            assets_purchased: asset_purchases,
            total_assets,
            net_emissions,
            location_based_emissions: emissions.location_based_emissions,
            market_based_emissions: emissions.market_based_emissions,
            marginal_emissions_impact: emissions.marginal_emissions_impact,
            rec_coverage_percentage,
            calculation_time: emissions.calculation_time,
            confidence_level: emissions.confidence_level,
        };
        
        // Generate geographic breakdown if enabled
        if self.options.show_regional_data {
            self.generate_geographic_breakdown(&metrics);
        }
        
        // Generate asset summary
        self.generate_asset_summary(&metrics);
        
        // Cache the metrics
        self.historical_metrics.insert(period, metrics.clone());
        
        Ok(metrics)
    }
    
    /// Generate geographic emissions breakdown
    fn generate_geographic_breakdown(&mut self, metrics: &EnvironmentalMetrics) {
        // This would use data from emissions_tracker to build detailed geographic insights
        // In a production system, this would create a detailed map of emissions by region
        
        let mut country_emissions = HashMap::new();
        let mut region_emissions = HashMap::new();
        let mut country_energy = HashMap::new();
        let mut country_renewable = HashMap::new();
        
        // Example data - in production would use actual regional data
        country_emissions.insert("US".to_string(), metrics.total_emissions * 0.3);
        country_emissions.insert("CN".to_string(), metrics.total_emissions * 0.25);
        country_emissions.insert("EU".to_string(), metrics.total_emissions * 0.2);
        country_emissions.insert("Other".to_string(), metrics.total_emissions * 0.25);
        
        region_emissions.insert("US-West".to_string(), metrics.total_emissions * 0.15);
        region_emissions.insert("US-East".to_string(), metrics.total_emissions * 0.15);
        region_emissions.insert("CN-North".to_string(), metrics.total_emissions * 0.15);
        region_emissions.insert("CN-South".to_string(), metrics.total_emissions * 0.1);
        region_emissions.insert("EU-Central".to_string(), metrics.total_emissions * 0.2);
        region_emissions.insert("Other".to_string(), metrics.total_emissions * 0.25);
        
        country_energy.insert("US".to_string(), metrics.energy_consumption * 0.3);
        country_energy.insert("CN".to_string(), metrics.energy_consumption * 0.25);
        country_energy.insert("EU".to_string(), metrics.energy_consumption * 0.2);
        country_energy.insert("Other".to_string(), metrics.energy_consumption * 0.25);
        
        country_renewable.insert("US".to_string(), 35.0);
        country_renewable.insert("CN".to_string(), 30.0);
        country_renewable.insert("EU".to_string(), 60.0);
        country_renewable.insert("Other".to_string(), 20.0);
        
        self.geographic_breakdown = Some(GeographicEmissionsBreakdown {
            country_emissions,
            region_emissions,
            country_energy,
            country_renewable,
        });
    }
    
    /// Generate asset summary
    fn generate_asset_summary(&mut self, metrics: &EnvironmentalMetrics) {
        // Count and summarize RECs and carbon offsets
        let mut rec_mwh = 0.0;
        let mut rec_count = 0;
        let mut rec_verified = 0;
        let mut rec_types: HashMap<String, f64> = HashMap::new();
        
        let mut offset_tonnes = 0.0;
        let mut offset_count = 0;
        let mut offset_verified = 0;
        let mut offset_types: HashMap<String, f64> = HashMap::new();
        
        for asset in &metrics.assets_purchased {
            match asset.asset_type {
                EnvironmentalAssetType::REC => {
                    rec_mwh += asset.amount;
                    rec_count += 1;
                    rec_verified += 1; // In a real system would check verification status
                    
                    // Add to energy type breakdown - simulated data
                    *rec_types.entry("Solar".to_string()).or_insert(0.0) += asset.amount * 0.4;
                    *rec_types.entry("Wind".to_string()).or_insert(0.0) += asset.amount * 0.3;
                    *rec_types.entry("Hydro".to_string()).or_insert(0.0) += asset.amount * 0.2;
                    *rec_types.entry("Other".to_string()).or_insert(0.0) += asset.amount * 0.1;
                },
                EnvironmentalAssetType::CarbonOffset => {
                    offset_tonnes += asset.amount;
                    offset_count += 1;
                    offset_verified += 1; // In a real system would check verification status
                    
                    // Add to project type breakdown - simulated data
                    *offset_types.entry("Forestry".to_string()).or_insert(0.0) += asset.amount * 0.4;
                    *offset_types.entry("Renewable Energy".to_string()).or_insert(0.0) += asset.amount * 0.3;
                    *offset_types.entry("Methane Capture".to_string()).or_insert(0.0) += asset.amount * 0.2;
                    *offset_types.entry("Other".to_string()).or_insert(0.0) += asset.amount * 0.1;
                },
                EnvironmentalAssetType::GreenInvestment => {
                    // Green investments don't directly contribute to offsets or RECs
                    // but could be tracked separately in a real implementation
                },
                EnvironmentalAssetType::ResearchGrant => {
                    // Research grants don't directly contribute to offsets or RECs
                    // but could be tracked separately in a real implementation
                },
            }
        }
        
        // Calculate REC coverage percentage
        let rec_coverage = if metrics.energy_consumption > 0.0 {
            (rec_mwh * 1000.0 / metrics.energy_consumption) * 100.0
        } else {
            0.0
        };
        
        // Calculate offset coverage percentage
        let offset_coverage = if metrics.total_emissions > 0.0 {
            (offset_tonnes / metrics.total_emissions) * 100.0
        } else {
            0.0
        };
        
        // Create summaries
        let rec_summary = if rec_count > 0 {
            Some(RECSummary {
                total_mwh: rec_mwh,
                certificate_count: rec_count,
                coverage_percentage: rec_coverage,
                energy_type_breakdown: rec_types,
                verified_certificates: rec_verified,
            })
        } else {
            None
        };
        
        let carbon_offset_summary = if offset_count > 0 {
            Some(CarbonOffsetSummary {
                total_tonnes: offset_tonnes,
                offset_count,
                coverage_percentage: offset_coverage,
                project_type_breakdown: offset_types,
                verified_offsets: offset_verified,
            })
        } else {
            None
        };
        
        // Create the asset summary
        let prioritization = if self.options.prioritize_recs {
            Some("RECs prioritized over carbon offsets for emissions reduction".to_string())
        } else {
            None
        };
        
        self.asset_summary = Some(AssetSummary {
            rec_summary,
            carbon_offset_summary,
            total_impact: metrics.total_assets,
            prioritization,
        });
    }
    
    /// Update dashboard options
    pub fn update_options(&mut self, options: DashboardOptions) {
        self.options = options;
    }
    
    /// Generate a simple text report of environmental metrics
    pub fn generate_text_report(&self, period: EmissionsTimePeriod) -> Result<String, String> {
        let metrics = match self.get_metrics(period) {
            Some(m) => m,
            None => return Err("No metrics available for the requested period".to_string()),
        };
        
        // Format the period as a string
        let period_str = match period {
            EmissionsTimePeriod::Day => "Daily (Last 24 hours)".to_string(),
            EmissionsTimePeriod::Week => "Weekly (Last 7 days)".to_string(),
            EmissionsTimePeriod::Month => "Monthly (Last 30 days)".to_string(),
            EmissionsTimePeriod::Year => "Yearly (Last 365 days)".to_string(),
            EmissionsTimePeriod::AllTime => "All Time".to_string(),
            EmissionsTimePeriod::Custom { start, end } => 
                format!("Custom ({} to {})", 
                    start.format("%Y-%m-%d"), 
                    end.format("%Y-%m-%d")),
        };
        
        // Format renewable percentage
        let renewable_str = match metrics.renewable_percentage {
            Some(pct) => format!("{:.1}%", pct),
            None => "Unknown".to_string(),
        };
        
        // Format confidence level
        let confidence_str = match metrics.confidence_level {
            Some(conf) => format!("{:.1}%", conf * 100.0),
            None => "Unknown".to_string(),
        };
        
        // Format REC coverage
        let rec_coverage_str = match metrics.rec_coverage_percentage {
            Some(pct) => format!("{:.1}%", pct),
            None => "Unknown".to_string(),
        };
        
        // Choose which emissions to show based on options
        let emissions_value = match self.options.emissions_report_type {
            EmissionsReportType::LocationBased => metrics.location_based_emissions.unwrap_or(metrics.total_emissions),
            EmissionsReportType::MarketBased => metrics.market_based_emissions.unwrap_or(metrics.total_emissions),
            EmissionsReportType::MarginalImpact => metrics.marginal_emissions_impact.unwrap_or(metrics.total_emissions),
            EmissionsReportType::Comprehensive => metrics.total_emissions,
        };
        
        // Build the report
        let mut report = format!(
            "supernova Environmental Impact Report: {}\n\
            --------------------------------------------\n",
            period_str
        );
        
        // Add emissions section based on report type
        match self.options.emissions_report_type {
            EmissionsReportType::Comprehensive => {
                report.push_str(&format!(
                    "Location-based Emissions: {:.2} tonnes CO2e\n\
                    Market-based Emissions: {:.2} tonnes CO2e\n",
                    metrics.location_based_emissions.unwrap_or(metrics.total_emissions),
                    metrics.market_based_emissions.unwrap_or(metrics.total_emissions)
                ));
                
                if let Some(marginal) = metrics.marginal_emissions_impact {
                    report.push_str(&format!("Marginal Emissions Impact: {:.2} tonnes CO2e\n", marginal));
                }
            },
            _ => {
                report.push_str(&format!("Total Emissions: {:.2} tonnes CO2e\n", emissions_value));
            }
        }
        
        // Add energy and renewable info
        report.push_str(&format!(
            "Energy Consumption: {:.2} kWh\n\
            Renewable Energy: {}\n\
            REC Coverage: {}\n",
            metrics.energy_consumption,
            renewable_str,
            rec_coverage_str
        ));
        
        // Add transaction emissions
        report.push_str(&format!(
            "Emissions per Transaction: {:.4} kg CO2e\n\
            Transactions Processed: {}\n",
            metrics.emissions_per_transaction,
            metrics.transaction_count
        ));
        
        // Add environmental assets section
        report.push_str("\nEnvironmental Assets:\n");
        
        if let Some(summary) = &self.asset_summary {
            if let Some(rec_summary) = &summary.rec_summary {
                report.push_str(&format!(
                    "Renewable Energy Certificates: {:.2} MWh ({:.1}% coverage)\n",
                    rec_summary.total_mwh,
                    rec_summary.coverage_percentage
                ));
            }
            
            if let Some(offset_summary) = &summary.carbon_offset_summary {
                report.push_str(&format!(
                    "Carbon Offsets: {:.2} tonnes CO2e ({:.1}% of emissions)\n",
                    offset_summary.total_tonnes,
                    offset_summary.coverage_percentage
                ));
            }
            
            if let Some(priority) = &summary.prioritization {
                report.push_str(&format!("Note: {}\n", priority));
            }
        }
        
        // Add net emissions
        report.push_str(&format!("\nNet Emissions: {:.2} tonnes CO2e\n", metrics.net_emissions));
        
        // Add confidence information if enabled
        if self.options.show_confidence_levels {
            report.push_str(&format!("\nCalculation Confidence: {}\n", confidence_str));
        }
        
        Ok(report)
    }
    
    /// Export metrics as JSON
    pub fn export_metrics_json(&self, period: EmissionsTimePeriod) -> Result<String, String> {
        let metrics = match self.get_metrics(period) {
            Some(m) => m,
            None => return Err("No metrics available for the requested period".to_string()),
        };
        
        match serde_json::to_string_pretty(metrics) {
            Ok(json) => Ok(json),
            Err(e) => Err(format!("Error serializing metrics to JSON: {}", e)),
        }
    }
    
    /// Export geographic breakdown as JSON
    pub fn export_geographic_json(&self) -> Result<String, String> {
        let breakdown = match &self.geographic_breakdown {
            Some(b) => b,
            None => return Err("No geographic breakdown available".to_string()),
        };
        
        match serde_json::to_string_pretty(breakdown) {
            Ok(json) => Ok(json),
            Err(e) => Err(format!("Error serializing geographic data to JSON: {}", e)),
        }
    }
    
    /// Export asset summary as JSON
    pub fn export_asset_summary_json(&self) -> Result<String, String> {
        let summary = match &self.asset_summary {
            Some(s) => s,
            None => return Err("No asset summary available".to_string()),
        };
        
        match serde_json::to_string_pretty(summary) {
            Ok(json) => Ok(json),
            Err(e) => Err(format!("Error serializing asset summary to JSON: {}", e)),
        }
    }

    /// Generate dashboard data from current network state
    pub fn generate_dashboard_data(&self) -> Result<DashboardData, String> {
        let network_emissions = match self.api.get_network_emissions() {
            Ok(data) => data,
            Err(e) => return Err(format!("Failed to get network emissions: {}", e)),
        };

        let miners = match self.api.get_all_miners() {
            Ok(m) => m,
            Err(e) => return Err(format!("Failed to get miners: {}", e)),
        };

        let total_miners = miners.len();
        let green_miners = miners.iter()
            .filter(|m| m.renewable_percentage > 75.0)
            .count();

        // Calculate renewable percentage across network
        let renewable_percentage = if total_miners > 0 {
            miners.iter().map(|m| m.renewable_percentage).sum::<f64>() / total_miners as f64
        } else {
            0.0
        };

        // Count hardware distribution
        let mut hardware_distribution = HashMap::new();
        for miner in &miners {
            // For each hardware type in the miner's hardware types
            for hardware in &miner.hardware_types {
                // Convert TypesHardwareType to String
                let hw_str = format!("{:?}", hardware);
                *hardware_distribution.entry(hw_str).or_insert(0) += 1;
            }
        }
        
        // Region distribution
        let mut region_distribution = HashMap::new();
        for miner in miners {
            // Convert Region to String
            let region_str = miner.region.to_string();
            *region_distribution.entry(region_str).or_insert(0) += 1;
        }

        // Get recent asset purchases
        let recent_asset_purchases = match self.api.get_recent_asset_purchases(10) {
            Ok(purchases) => purchases,
            Err(e) => return Err(format!("Failed to get recent purchases: {}", e)),
        };

        // Get treasury balance
        let treasury_balance = match self.api.get_treasury_balance() {
            Ok(balance) => balance,
            Err(e) => return Err(format!("Failed to get treasury balance: {}", e)),
        };

        // Get emissions trend (simplified implementation)
        let emissions_trend = match self.api.get_emissions_history(30) {
            Ok(history) => history.into_iter()
                .map(|(timestamp, value)| EmissionsTrend {
                    timestamp,
                    emissions_value: value,
                })
                .collect(),
            Err(e) => return Err(format!("Failed to get emissions history: {}", e)),
        };

        Ok(DashboardData {
            network_emissions,
            renewable_percentage,
            total_miners,
            green_miners,
            hardware_distribution,
            region_distribution,
            recent_asset_purchases,
            treasury_balance,
            emissions_trend,
        })
    }

    /// Generate JSON representation of dashboard data
    pub fn generate_json(&self) -> Result<String, String> {
        match self.generate_dashboard_data() {
            Ok(data) => match serde_json::to_string_pretty(&data) {
                Ok(json) => Ok(json),
                Err(e) => Err(format!("Failed to serialize dashboard data: {}", e)),
            },
            Err(e) => Err(e),
        }
    }

    /// Get miner verification status distribution
    pub fn get_verification_distribution(&self) -> Result<HashMap<MinerVerificationStatus, usize>, String> {
        let miners = match self.api.get_all_miners() {
            Ok(m) => m,
            Err(e) => return Err(format!("Failed to get miners: {}", e)),
        };

        let mut distribution = HashMap::new();
        for miner in miners {
            // Extract status from verification field if present
            let status = if let Some(verification) = &miner.verification {
                verification.status
            } else {
                MinerVerificationStatus::Unverified
            };
            
            *distribution.entry(status).or_insert(0) += 1;
        }

        Ok(distribution)
    }

    /// Get REC vs carbon offset distribution
    pub fn get_asset_type_distribution(&self) -> Result<(f64, f64), String> {
        let purchases = match self.api.get_all_asset_purchases() {
            Ok(p) => p,
            Err(e) => return Err(format!("Failed to get asset purchases: {}", e)),
        };

        let mut rec_value = 0.0;
        let mut offset_value = 0.0;

        for purchase in purchases {
            match purchase.asset_type.as_str() {
                "REC" => rec_value += purchase.amount,
                "Carbon Offset" => offset_value += purchase.amount,
                _ => {}
            }
        }

        Ok((rec_value, offset_value))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::environmental::emissions::{EmissionsConfig, EmissionsTracker, HashRate};
    
    // Mock implementation for testing
    struct MockEnvironmentalApi;
    
    impl MockEnvironmentalApi {
        fn new() -> Self {
            Self
        }
    }
    
    impl EnvironmentalApiTrait for MockEnvironmentalApi {
        fn get_network_emissions(&self) -> Result<NetworkEmissionsData, String> {
            Ok(NetworkEmissionsData {
                total_energy_mwh: 1000.0,
                total_emissions_tons_co2e: 500.0,
                renewable_percentage: 50.0,
                emissions_per_tx: 0.005,
                timestamp: Utc::now().timestamp() as u64,
            })
        }
        
        fn get_all_miners(&self) -> Result<Vec<MinerEnvironmentalInfo>, String> {
            Ok(vec![])
        }
        
        fn get_recent_asset_purchases(&self, _limit: usize) -> Result<Vec<AssetPurchaseRecord>, String> {
            Ok(vec![])
        }
        
        fn get_treasury_balance(&self) -> Result<f64, String> {
            Ok(50000.0)
        }
        
        fn get_emissions_history(&self, _days: usize) -> Result<Vec<(DateTime<Utc>, f64)>, String> {
            Ok(vec![])
        }
        
        fn get_all_asset_purchases(&self) -> Result<Vec<AssetPurchaseRecord>, String> {
            Ok(vec![])
        }
        
        fn get_miner_by_id(&self, _miner_id: &str) -> Result<MinerEnvironmentalInfo, String> {
            use crate::environmental::types::{Region, EnergySource, HardwareType};
            use std::collections::HashMap;
            
            let mut energy_sources = HashMap::new();
            energy_sources.insert(EnergySource::Solar, 41.5);
            energy_sources.insert(EnergySource::Wind, 27.5);
            energy_sources.insert(EnergySource::Grid, 31.0);
            
            Ok(MinerEnvironmentalInfo {
                miner_id: "test_miner".to_string(),
                name: "Test Miner".to_string(),
                region: Region::NorthAmerica,
                location_verification: None,
                hardware_types: vec![HardwareType::Asic],
                energy_sources,
                renewable_percentage: 75.0,
                verification: None,
                total_hashrate: 1000.0,
                energy_consumption_kwh_day: 24000.0,
                carbon_footprint_tons_year: 250.0,
                recs: vec![],
                carbon_offsets: vec![],
                environmental_impact: 0.3,
                compliance_score: 95.0,
                last_report_timestamp: Utc::now(),
                status: MinerVerificationStatus::Verified,
            })
        }
        
        fn get_miner_emissions(&self, _miner_id: &str) -> Result<MinerEmissionsData, String> {
            Ok(MinerEmissionsData {
                miner_id: "test_miner".to_string(),
                miner_name: "Test Miner".to_string(),
                region: "US".to_string(),
                energy_consumption_kwh_day: 1000.0,
                emissions_tonnes_year: 100.0,
                hardware_types: vec!["ASIC".to_string()],
                energy_sources: {
                    let mut sources = HashMap::new();
                    sources.insert("renewable".to_string(), 75.0);
                    sources.insert("coal".to_string(), 25.0);
                    sources
                },
                renewable_percentage: 75.0,
                offset_tonnes: 10.0,
                verification_status: "verified".to_string(),
                energy_efficiency: Some(25.0),
                net_carbon_impact: 90.0,
                is_verified: true,
                timestamp: Utc::now(),
            })
        }
    }
    
    #[test]
    fn test_dashboard_basic_functionality() {
        // Create emissions tracker
        let mut emissions_tracker = EmissionsTracker::new(EmissionsConfig {
            enabled: true,
            default_emission_factor: 400.0,
            emissions_api_endpoint: None,
            preferred_data_source: None,
            use_marginal_emissions: false,
            known_hashrate_percentage: 100.0,
            default_network_efficiency: 50.0,
            data_update_frequency_hours: 24,
            cache_emissions_factors: true,
            verify_miner_locations: true,
            prioritize_rec_verification: true,
            emissions_api_key: None,
            default_carbon_intensity: 475.0,
            default_renewable_percentage: 0.3,
            mining_pue_factor: 1.2,
        });
        
        // Add some test data
        emissions_tracker.load_default_emission_factors();
        emissions_tracker.update_region_hashrate(
            Region::new("US"),
            HashRate(100.0),
        );
        
        // Create treasury
        let treasury = EnvironmentalTreasury::new(TreasuryConfig {
            fee_allocation_percentage: 2.0,
            required_signatures: 1,
            signers: vec!["signer1".to_string()],
            min_purchase_amount: 1000.0,
            max_purchase_amount: 100000.0,
            auto_purchase_threshold: 10000.0,
            enable_auto_purchase: true,
            preferred_asset_type: EnvironmentalAssetType::REC,
            backup_asset_type: Some(EnvironmentalAssetType::CarbonOffset),
            verification_required: true,
            max_asset_age_days: 365,
            diversification_enabled: true,
            max_single_asset_percentage: 50.0,
        });
        
        // Create dashboard
        let mut dashboard = EnvironmentalDashboard::new(emissions_tracker, treasury, Box::new(MockEnvironmentalApi::new()));
        
        // Generate metrics for a day (with some transactions)
        let transaction_count = 100_000;
        let metrics_result = dashboard.generate_metrics(EmissionsTimePeriod::Day, transaction_count);
        
        assert!(metrics_result.is_ok(), "Should generate metrics successfully");
        
        // Generate a text report
        let report_result = dashboard.generate_text_report(EmissionsTimePeriod::Day);
        
        assert!(report_result.is_ok(), "Should generate report successfully");
        let report = report_result.unwrap();
        
        // Basic checks on the report content
        assert!(report.contains("supernova Environmental Impact Report"), "Report should have title");
        assert!(report.contains("Total Emissions"), "Report should have emissions data");
        assert!(report.contains(&format!("Transactions Processed: {}", transaction_count)), 
                "Report should have transaction count");
    }
} 