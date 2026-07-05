use crate::environmental::api::{AssetPurchaseRecord, EnvironmentalApiTrait, NetworkEmissionsData};
use crate::environmental::emissions::EmissionsTracker;
use crate::environmental::miner_reporting::{MinerReportingManager, MinerVerificationStatus};
use crate::environmental::treasury::{
    EnvironmentalAssetPurchase, EnvironmentalAssetType, EnvironmentalTreasury, VerificationStatus,
};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

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
    pub fn new(
        emissions_tracker: EmissionsTracker,
        treasury: EnvironmentalTreasury,
        api: Box<dyn EnvironmentalApiTrait>,
    ) -> Self {
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
    pub fn generate_metrics(
        &mut self,
        period: EmissionsTimePeriod,
        transaction_count: u64,
    ) -> Result<EnvironmentalMetrics, String> {
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
        let emissions = match self
            .emissions_tracker
            .calculate_network_emissions(start, end)
        {
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
        let total_assets: f64 = asset_purchases
            .iter()
            .filter(|purchase| {
                purchase.asset_type == EnvironmentalAssetType::REC
                    || purchase.asset_type == EnvironmentalAssetType::CarbonOffset
            })
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
    ///
    /// Derives per-region emissions, energy, and renewable percentages from the
    /// actual registered miners (their reported region, energy consumption, and
    /// renewable mix) rather than fabricating fixed country ratios. When no miner
    /// data is available, the breakdown is set to `None` so that
    /// `export_geographic_json` fails honestly instead of serving invented splits.
    fn generate_geographic_breakdown(&mut self, metrics: &EnvironmentalMetrics) {
        let miners = match self.api.get_all_miners() {
            Ok(m) => m,
            Err(_) => {
                self.geographic_breakdown = None;
                return;
            }
        };

        // Aggregate each region's actual share of network energy consumption from
        // miner-reported data. `region_renewable_weighted` accumulates
        // energy-weighted renewable percentages so we can compute a real
        // per-region average.
        let mut region_energy: HashMap<String, f64> = HashMap::new();
        let mut region_renewable_weighted: HashMap<String, f64> = HashMap::new();
        let mut total_energy = 0.0;

        for miner in &miners {
            let region = miner.region.to_string();
            let energy = miner.energy_consumption_kwh_day;
            *region_energy.entry(region.clone()).or_insert(0.0) += energy;
            *region_renewable_weighted.entry(region).or_insert(0.0) +=
                energy * miner.renewable_percentage;
            total_energy += energy;
        }

        // Without any reported energy we cannot derive real regional shares, so
        // report no breakdown rather than an invented one.
        if total_energy <= 0.0 {
            self.geographic_breakdown = None;
            return;
        }

        let mut region_emissions = HashMap::new();
        let mut country_energy = HashMap::new();
        let mut country_renewable = HashMap::new();

        for (region, energy) in &region_energy {
            let share = energy / total_energy;
            // Attribute the period's total emissions and energy to each region by
            // its real share of network energy consumption.
            region_emissions.insert(region.clone(), metrics.total_emissions * share);
            country_energy.insert(region.clone(), metrics.energy_consumption * share);
            // Energy-weighted average renewable percentage for the region.
            let weighted = region_renewable_weighted
                .get(region)
                .copied()
                .unwrap_or(0.0);
            country_renewable.insert(region.clone(), weighted / energy);
        }

        self.geographic_breakdown = Some(GeographicEmissionsBreakdown {
            // Region is the finest geographic granularity miners report, so the
            // country-keyed map is populated with real region-level data instead
            // of an invented country split.
            country_emissions: region_emissions.clone(),
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
                    // Only count assets whose verification actually succeeded, rather
                    // than assuming every purchased asset is verified.
                    if asset.verification_status == VerificationStatus::Verified {
                        rec_verified += 1;
                    }

                    // Attribute the full amount to the energy source recorded on the
                    // asset's metadata (populated at purchase time). Fall back to
                    // "Unspecified" instead of inventing a fixed source split.
                    let energy_type = asset
                        .metadata
                        .get("source")
                        .cloned()
                        .unwrap_or_else(|| "Unspecified".to_string());
                    *rec_types.entry(energy_type).or_insert(0.0) += asset.amount;
                }
                EnvironmentalAssetType::CarbonOffset => {
                    offset_tonnes += asset.amount;
                    offset_count += 1;
                    // Only count assets whose verification actually succeeded, rather
                    // than assuming every purchased asset is verified.
                    if asset.verification_status == VerificationStatus::Verified {
                        offset_verified += 1;
                    }

                    // Attribute the full amount to the project type recorded on the
                    // asset's metadata (populated at purchase time). Fall back to
                    // "Unspecified" instead of inventing a fixed project split.
                    let project_type = asset
                        .metadata
                        .get("project")
                        .cloned()
                        .unwrap_or_else(|| "Unspecified".to_string());
                    *offset_types.entry(project_type).or_insert(0.0) += asset.amount;
                }
                EnvironmentalAssetType::GreenInvestment => {
                    // Green investments don't directly contribute to offsets or RECs
                    // but could be tracked separately in a real implementation
                }
                EnvironmentalAssetType::ResearchGrant => {
                    // Research grants don't directly contribute to offsets or RECs
                    // but could be tracked separately in a real implementation
                }
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
            EmissionsTimePeriod::Custom { start, end } => format!(
                "Custom ({} to {})",
                start.format("%Y-%m-%d"),
                end.format("%Y-%m-%d")
            ),
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
            EmissionsReportType::LocationBased => metrics
                .location_based_emissions
                .unwrap_or(metrics.total_emissions),
            EmissionsReportType::MarketBased => metrics
                .market_based_emissions
                .unwrap_or(metrics.total_emissions),
            EmissionsReportType::MarginalImpact => metrics
                .marginal_emissions_impact
                .unwrap_or(metrics.total_emissions),
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
                    metrics
                        .location_based_emissions
                        .unwrap_or(metrics.total_emissions),
                    metrics
                        .market_based_emissions
                        .unwrap_or(metrics.total_emissions)
                ));

                if let Some(marginal) = metrics.marginal_emissions_impact {
                    report.push_str(&format!(
                        "Marginal Emissions Impact: {:.2} tonnes CO2e\n",
                        marginal
                    ));
                }
            }
            _ => {
                report.push_str(&format!(
                    "Total Emissions: {:.2} tonnes CO2e\n",
                    emissions_value
                ));
            }
        }

        // Add energy and renewable info
        report.push_str(&format!(
            "Energy Consumption: {:.2} kWh\n\
            Renewable Energy: {}\n\
            REC Coverage: {}\n",
            metrics.energy_consumption, renewable_str, rec_coverage_str
        ));

        // Add transaction emissions
        report.push_str(&format!(
            "Emissions per Transaction: {:.4} kg CO2e\n\
            Transactions Processed: {}\n",
            metrics.emissions_per_transaction, metrics.transaction_count
        ));

        // Add environmental assets section
        report.push_str("\nEnvironmental Assets:\n");

        if let Some(summary) = &self.asset_summary {
            if let Some(rec_summary) = &summary.rec_summary {
                report.push_str(&format!(
                    "Renewable Energy Certificates: {:.2} MWh ({:.1}% coverage)\n",
                    rec_summary.total_mwh, rec_summary.coverage_percentage
                ));
            }

            if let Some(offset_summary) = &summary.carbon_offset_summary {
                report.push_str(&format!(
                    "Carbon Offsets: {:.2} tonnes CO2e ({:.1}% of emissions)\n",
                    offset_summary.total_tonnes, offset_summary.coverage_percentage
                ));
            }

            if let Some(priority) = &summary.prioritization {
                report.push_str(&format!("Note: {}\n", priority));
            }
        }

        // Add net emissions
        report.push_str(&format!(
            "\nNet Emissions: {:.2} tonnes CO2e\n",
            metrics.net_emissions
        ));

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
        let green_miners = miners
            .iter()
            .filter(|m| m.renewable_percentage > 75.0)
            .count();

        // Calculate renewable percentage across network, energy-weighted so a
        // fleet of tiny 100%-renewable miners cannot mask one large fossil
        // miner. Uses sum(energy_i * renewable_i) / sum(energy_i), falling back
        // to 0.0 when no energy is reported (mirrors generate_geographic_breakdown).
        let total_energy_kwh: f64 = miners.iter().map(|m| m.energy_consumption_kwh_day).sum();
        let renewable_percentage = if total_energy_kwh > 0.0 {
            miners
                .iter()
                .map(|m| m.energy_consumption_kwh_day * m.renewable_percentage)
                .sum::<f64>()
                / total_energy_kwh
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
            Ok(history) => history
                .into_iter()
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
    pub fn get_verification_distribution(
        &self,
    ) -> Result<HashMap<MinerVerificationStatus, usize>, String> {
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
    use crate::test_common::*;

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

        fn get_recent_asset_purchases(
            &self,
            _limit: usize,
        ) -> Result<Vec<AssetPurchaseRecord>, String> {
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
            use crate::environmental::types::{EnergySource, HardwareType, Region};
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
                carbon_footprint_tonnes_year: Some(250.0),
                last_update: Utc::now(),
                has_rec_certificates: false,
                has_carbon_offsets: false,
                certificates_url: None,
                rec_certificates: vec![],
                carbon_offsets: vec![],
                environmental_score: Some(95.0),
                preferred_energy_type: Some(EnergySource::Solar),
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
    #[ignore] // Environmental dashboard implementation pending
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
            crate::environmental::emissions::Region {
                country_code: "US".to_string(),
                sub_region: None,
            },
            HashRate(100.0),
        );

        // Create treasury
        let mut min_purchase_amounts = HashMap::new();
        min_purchase_amounts.insert(EnvironmentalAssetType::REC, 1000.0);
        min_purchase_amounts.insert(EnvironmentalAssetType::CarbonOffset, 1000.0);

        let treasury = EnvironmentalTreasury::new(TreasuryConfig {
            enabled: true,
            fee_allocation_percentage: 2.0,
            allocation: TreasuryAllocation {
                rec_percentage: 50.0,
                offset_percentage: 30.0,
                investment_percentage: 15.0,
                research_percentage: 5.0,
            },
            min_purchase_amounts,
            verification_service_url: Some("https://verify.example.com".to_string()),
            require_verification: true,
            automatic_purchases: true,
            max_single_purchase_percentage: 50.0,
        });

        // Create dashboard
        let mut dashboard = EnvironmentalDashboard::new(
            emissions_tracker,
            treasury,
            Box::new(MockEnvironmentalApi::new()),
        );

        // Generate metrics for a day (with some transactions)
        let transaction_count = 100_000;
        let metrics_result =
            dashboard.generate_metrics(EmissionsTimePeriod::Day, transaction_count);

        assert!(
            metrics_result.is_ok(),
            "Should generate metrics successfully"
        );

        // Generate a text report
        let report_result = dashboard.generate_text_report(EmissionsTimePeriod::Day);

        assert!(report_result.is_ok(), "Should generate report successfully");
        let report = report_result.unwrap();

        // Basic checks on the report content
        assert!(
            report.contains("supernova Environmental Impact Report"),
            "Report should have title"
        );
        assert!(
            report.contains("Total Emissions"),
            "Report should have emissions data"
        );
        assert!(
            report.contains(&format!("Transactions Processed: {}", transaction_count)),
            "Report should have transaction count"
        );
    }

    // Mock API returning real miners across regions, used to verify the
    // geographic breakdown is derived from actual miner data.
    struct MockApiWithMiners {
        miners: Vec<MinerEnvironmentalInfo>,
    }

    impl EnvironmentalApiTrait for MockApiWithMiners {
        fn get_all_miners(&self) -> Result<Vec<MinerEnvironmentalInfo>, String> {
            Ok(self.miners.clone())
        }
        fn get_miner_by_id(&self, _miner_id: &str) -> Result<MinerEnvironmentalInfo, String> {
            Err("not needed".to_string())
        }
        fn get_network_emissions(&self) -> Result<NetworkEmissionsData, String> {
            Err("not needed".to_string())
        }
        fn get_miner_emissions(&self, _miner_id: &str) -> Result<MinerEmissionsData, String> {
            Err("not needed".to_string())
        }
        fn get_recent_asset_purchases(
            &self,
            _limit: usize,
        ) -> Result<Vec<AssetPurchaseRecord>, String> {
            Ok(vec![])
        }
        fn get_all_asset_purchases(&self) -> Result<Vec<AssetPurchaseRecord>, String> {
            Ok(vec![])
        }
        fn get_treasury_balance(&self) -> Result<f64, String> {
            Ok(0.0)
        }
        fn get_emissions_history(&self, _days: usize) -> Result<Vec<(DateTime<Utc>, f64)>, String> {
            Ok(vec![])
        }
    }

    fn make_miner(
        region: crate::environmental::types::Region,
        energy_kwh_day: f64,
        renewable_percentage: f64,
    ) -> MinerEnvironmentalInfo {
        let mut m = MinerEnvironmentalInfo::new(
            "m".to_string(),
            "Miner".to_string(),
            region,
        );
        m.energy_consumption_kwh_day = energy_kwh_day;
        m.renewable_percentage = renewable_percentage;
        m
    }

    fn make_dashboard(api: Box<dyn EnvironmentalApiTrait>) -> EnvironmentalDashboard {
        let emissions_tracker = EmissionsTracker::new(EmissionsConfig::default());
        let treasury = EnvironmentalTreasury::default();
        EnvironmentalDashboard::new(emissions_tracker, treasury, api)
    }

    fn metrics_with(total_emissions: f64, energy_consumption: f64) -> EnvironmentalMetrics {
        EnvironmentalMetrics {
            period: EmissionsTimePeriod::Day,
            total_emissions,
            energy_consumption,
            renewable_percentage: None,
            emissions_per_transaction: 0.0,
            transaction_count: 0,
            assets_purchased: vec![],
            total_assets: 0.0,
            net_emissions: total_emissions,
            location_based_emissions: None,
            market_based_emissions: None,
            marginal_emissions_impact: None,
            rec_coverage_percentage: None,
            calculation_time: Utc::now(),
            confidence_level: None,
        }
    }

    #[test]
    fn test_geographic_breakdown_derived_from_real_miners() {
        use crate::environmental::types::Region;

        // Two miners in North America (energy-weighted renewable 50%) and one in
        // Europe (80%). Regional shares must come from actual reported energy.
        let miners = vec![
            make_miner(Region::NorthAmerica, 100.0, 40.0),
            make_miner(Region::NorthAmerica, 100.0, 60.0),
            make_miner(Region::Europe, 200.0, 80.0),
        ];
        let mut dashboard = make_dashboard(Box::new(MockApiWithMiners { miners }));

        let metrics = metrics_with(1000.0, 800.0);
        dashboard.generate_geographic_breakdown(&metrics);

        let b = dashboard
            .geographic_breakdown
            .as_ref()
            .expect("breakdown should be derived from miner data");

        // Total energy = 400; NA share 0.5, EU share 0.5.
        let na_em = b.region_emissions.get("NA").copied().unwrap();
        let eu_em = b.region_emissions.get("EU").copied().unwrap();
        assert!((na_em - 500.0).abs() < 1e-6, "NA emissions from real share");
        assert!((eu_em - 500.0).abs() < 1e-6, "EU emissions from real share");
        assert!(
            (na_em + eu_em - metrics.total_emissions).abs() < 1e-6,
            "regional emissions must sum to the period total"
        );

        // Energy attributed by the same real shares.
        assert!((b.country_energy.get("NA").copied().unwrap() - 400.0).abs() < 1e-6);
        assert!((b.country_energy.get("EU").copied().unwrap() - 400.0).abs() < 1e-6);

        // Renewable percentages are energy-weighted averages, not hardcoded.
        assert!((b.country_renewable.get("NA").copied().unwrap() - 50.0).abs() < 1e-6);
        assert!((b.country_renewable.get("EU").copied().unwrap() - 80.0).abs() < 1e-6);

        // The old fabricated country/region keys must be gone.
        assert!(!b.country_emissions.contains_key("US"));
        assert!(!b.country_emissions.contains_key("CN"));
        assert!(!b.region_emissions.contains_key("US-West"));
    }

    #[test]
    fn test_geographic_breakdown_none_without_miners() {
        // No registered miners -> no invented breakdown, and the export fails
        // honestly instead of serving fabricated country splits.
        let mut dashboard = make_dashboard(Box::new(MockEnvironmentalApi::new()));
        dashboard.generate_geographic_breakdown(&metrics_with(1000.0, 800.0));
        assert!(dashboard.geographic_breakdown.is_none());
        assert!(dashboard.export_geographic_json().is_err());
    }

    fn make_asset(
        asset_type: EnvironmentalAssetType,
        amount: f64,
        status: VerificationStatus,
        metadata: &[(&str, &str)],
    ) -> EnvironmentalAssetPurchase {
        let mut md = HashMap::new();
        for (k, v) in metadata {
            md.insert(k.to_string(), v.to_string());
        }
        EnvironmentalAssetPurchase {
            purchase_id: "PUR-TEST".to_string(),
            asset_type,
            provider: "TestProvider".to_string(),
            amount,
            cost: 0,
            purchase_date: Utc::now(),
            verification_status: status,
            verification_reference: None,
            region: None,
            metadata: md,
        }
    }

    #[test]
    fn test_asset_summary_uses_real_verification_and_metadata() {
        let mut dashboard = make_dashboard(Box::new(MockEnvironmentalApi::new()));

        let mut metrics = metrics_with(100.0, 1_000_000.0);
        metrics.assets_purchased = vec![
            // Two RECs from wind, only one actually verified.
            make_asset(
                EnvironmentalAssetType::REC,
                1000.0,
                VerificationStatus::Verified,
                &[("source", "Wind Power")],
            ),
            make_asset(
                EnvironmentalAssetType::REC,
                500.0,
                VerificationStatus::Pending,
                &[("source", "Wind Power")],
            ),
            // A solar REC with no recorded source metadata -> "Unspecified".
            make_asset(
                EnvironmentalAssetType::REC,
                250.0,
                VerificationStatus::Verified,
                &[],
            ),
            // One offset, verified, from a reforestation project.
            make_asset(
                EnvironmentalAssetType::CarbonOffset,
                40.0,
                VerificationStatus::Verified,
                &[("project", "Reforestation")],
            ),
            // One offset, failed verification, no project metadata.
            make_asset(
                EnvironmentalAssetType::CarbonOffset,
                10.0,
                VerificationStatus::Failed,
                &[],
            ),
        ];

        dashboard.generate_asset_summary(&metrics);
        let summary = dashboard.asset_summary.expect("summary generated");

        let rec = summary.rec_summary.expect("rec summary present");
        assert_eq!(rec.certificate_count, 3, "counts every REC record");
        // Only the two Verified RECs count as verified, not all three.
        assert_eq!(rec.verified_certificates, 2, "verified count gated on status");
        // Energy-type breakdown reflects real metadata sources, not a fixed split.
        let wind = rec.energy_type_breakdown.get("Wind Power").copied().unwrap();
        assert!((wind - 1500.0).abs() < 1e-6, "full amount attributed to Wind Power");
        let unspecified = rec
            .energy_type_breakdown
            .get("Unspecified")
            .copied()
            .unwrap();
        assert!((unspecified - 250.0).abs() < 1e-6, "missing source -> Unspecified");
        // The fabricated fixed-split buckets must be gone.
        assert!(!rec.energy_type_breakdown.contains_key("Solar"));
        assert!(!rec.energy_type_breakdown.contains_key("Hydro"));

        let offset = summary
            .carbon_offset_summary
            .expect("offset summary present");
        assert_eq!(offset.offset_count, 2, "counts every offset record");
        assert_eq!(offset.verified_offsets, 1, "only the Verified offset counts");
        let forestry = offset
            .project_type_breakdown
            .get("Reforestation")
            .copied()
            .unwrap();
        assert!((forestry - 40.0).abs() < 1e-6, "full amount to real project");
        assert!(!offset.project_type_breakdown.contains_key("Forestry"));
        assert!(!offset.project_type_breakdown.contains_key("Methane Capture"));
    }
}
