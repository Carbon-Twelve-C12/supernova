use std::collections::HashMap;
use serde::{Serialize, Deserialize};
use chrono::{DateTime, Utc, Duration};
use crate::environmental::emissions::{Emissions, EmissionsTracker, Region, EmissionFactor};
use crate::environmental::treasury::{EnvironmentalTreasury, EnvironmentalAssetPurchase, EnvironmentalAssetType};

/// Time period for emissions data
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EmissionsTimePeriod {
    /// Daily emissions data
    Daily,
    /// Weekly emissions data
    Weekly,
    /// Monthly emissions data
    Monthly,
    /// Yearly emissions data
    Yearly,
    /// All-time emissions data
    AllTime,
    /// Custom time period
    Custom {
        /// Start time
        start: DateTime<Utc>,
        /// End time
        end: DateTime<Utc>,
    },
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
}

impl Default for DashboardOptions {
    fn default() -> Self {
        Self {
            show_regional_data: true,
            show_miner_data: true,
            show_transaction_details: false,
            show_monetary_values: true,
            currency: "USD".to_string(),
        }
    }
}

/// Environmental performance dashboard
pub struct EnvironmentalDashboard {
    /// Emissions tracker reference
    emissions_tracker: EmissionsTracker,
    /// Treasury reference
    treasury: EnvironmentalTreasury,
    /// Historical metrics
    historical_metrics: HashMap<EmissionsTimePeriod, EnvironmentalMetrics>,
    /// Display options
    options: DashboardOptions,
}

impl EnvironmentalDashboard {
    /// Create a new environmental dashboard
    pub fn new(emissions_tracker: EmissionsTracker, treasury: EnvironmentalTreasury) -> Self {
        Self {
            emissions_tracker,
            treasury,
            historical_metrics: HashMap::new(),
            options: DashboardOptions::default(),
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
            EmissionsTimePeriod::Daily => (now - Duration::days(1), now),
            EmissionsTimePeriod::Weekly => (now - Duration::weeks(1), now),
            EmissionsTimePeriod::Monthly => (now - Duration::days(30), now),
            EmissionsTimePeriod::Yearly => (now - Duration::days(365), now),
            EmissionsTimePeriod::AllTime => (now - Duration::days(3650), now), // ~10 years
            EmissionsTimePeriod::Custom { start, end } => (start, end),
        };
        
        // Calculate emissions for the period
        let emissions = match self.emissions_tracker.calculate_network_emissions(start, end) {
            Ok(e) => e,
            Err(e) => return Err(format!("Error calculating emissions: {:?}", e)),
        };
        
        // Use a placeholder for assets (to be replaced with actual data in production)
        let assets: Vec<EnvironmentalAssetPurchase> = Vec::new();
        let total_assets: f64 = 0.0;
        
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
            assets_purchased: assets,
            total_assets,
            net_emissions: emissions.tonnes_co2e - total_assets,
        };
        
        // Cache the metrics
        self.historical_metrics.insert(period, metrics.clone());
        
        Ok(metrics)
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
            EmissionsTimePeriod::Daily => "Daily (Last 24 hours)".to_string(),
            EmissionsTimePeriod::Weekly => "Weekly (Last 7 days)".to_string(),
            EmissionsTimePeriod::Monthly => "Monthly (Last 30 days)".to_string(),
            EmissionsTimePeriod::Yearly => "Yearly (Last 365 days)".to_string(),
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
        
        // Build the report
        let report = format!(
            "SuperNova Environmental Impact Report: {}\n\
            --------------------------------------------\n\
            Total Emissions: {:.2} tonnes CO2e\n\
            Energy Consumption: {:.2} kWh\n\
            Renewable Energy: {}\n\
            Emissions per Transaction: {:.4} kg CO2e\n\
            Transactions Processed: {:,}\n\
            \n\
            Environmental Assets:\n\
            Total Carbon Offsets: {:.2} tonnes CO2e\n\
            Net Emissions: {:.2} tonnes CO2e\n",
            period_str,
            metrics.total_emissions,
            metrics.energy_consumption,
            renewable_str,
            metrics.emissions_per_transaction,
            metrics.transaction_count,
            metrics.total_assets,
            metrics.net_emissions
        );
        
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::environmental::emissions::{EmissionsConfig, EmissionsTracker, HashRate};
    
    #[test]
    fn test_dashboard_basic_functionality() {
        // Create emissions tracker
        let mut emissions_tracker = EmissionsTracker::new(EmissionsConfig {
            enabled: true,
            default_emission_factor: 400.0,
            emissions_api_endpoint: None,
            default_network_efficiency: 50.0,
            known_hashrate_percentage: 100.0,
        });
        
        // Add some test data
        emissions_tracker.load_default_emission_factors();
        emissions_tracker.update_region_hashrate(
            Region { country_code: "US".to_string(), sub_region: None },
            HashRate(100.0),
        );
        
        // Create treasury
        let treasury = EnvironmentalTreasury::new(
            2.0,
            vec!["signer1".to_string()],
            1,
        );
        
        // Create dashboard
        let mut dashboard = EnvironmentalDashboard::new(emissions_tracker, treasury);
        
        // Generate metrics for a day (with some transactions)
        let transaction_count = 100_000;
        let metrics_result = dashboard.generate_metrics(EmissionsTimePeriod::Daily, transaction_count);
        
        assert!(metrics_result.is_ok(), "Should generate metrics successfully");
        
        // Generate a text report
        let report_result = dashboard.generate_text_report(EmissionsTimePeriod::Daily);
        
        assert!(report_result.is_ok(), "Should generate report successfully");
        let report = report_result.unwrap();
        
        // Basic checks on the report content
        assert!(report.contains("SuperNova Environmental Impact Report"), "Report should have title");
        assert!(report.contains("Total Emissions"), "Report should have emissions data");
        assert!(report.contains(&format!("Transactions Processed: {:,}", transaction_count)), 
                "Report should have transaction count");
    }
} 