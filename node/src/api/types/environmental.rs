use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Energy source types
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema)]
pub enum EnergySource {
    /// Coal power generation
    Coal,
    /// Natural gas power generation
    NaturalGas,
    /// Nuclear power generation
    Nuclear,
    /// Hydroelectric power generation
    Hydro,
    /// Wind power generation
    Wind,
    /// Solar power generation
    Solar,
    /// Geothermal power generation
    Geothermal,
    /// Other power generation sources
    Other,
}

/// Emissions source categories
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema)]
pub enum EmissionsSource {
    /// Direct electricity consumption
    Electricity,
    /// Cooling systems for hardware
    Cooling,
    /// Hardware manufacturing emissions
    Manufacturing,
    /// Network infrastructure
    Network,
    /// Maintenance operations
    Maintenance,
}

/// Carbon offset information
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CarbonOffset {
    /// Unique identifier for the offset
    pub id: String,
    /// Quantity of CO2e offset in grams
    pub quantity_g: f64,
    /// Provider of the carbon offset
    pub provider: String,
    /// Verification standard (if any)
    pub verification: Option<String>,
    /// Timestamp when the offset was applied
    pub timestamp: u64,
}

/// Historical energy usage data point
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct EnergyUsageHistory {
    /// Timestamp of the data point
    pub timestamp: u64,
    /// Energy usage in kWh
    pub usage: f64,
    /// Power consumption in watts
    pub power: f64,
}

/// Comprehensive environmental impact information
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct EnvironmentalImpact {
    /// Timestamp when this data was generated
    pub timestamp: u64,
    /// Time period in seconds that this data covers
    pub period: u64,
    /// Total energy consumption in kilowatt-hours
    pub total_energy_kwh: f64,
    /// Total carbon emissions in grams of CO2e
    pub total_carbon_g: f64,
    /// Percentage of energy from renewable sources
    pub renewable_percentage: f64,
    /// Number of transactions processed during this period
    pub transaction_count: u64,
    /// Energy per transaction in kWh (if available)
    pub energy_per_transaction_kwh: Option<f64>,
    /// Carbon per transaction in grams (if available)
    pub carbon_per_transaction_g: Option<f64>,
    /// List of energy sources used
    pub energy_sources: Vec<EnergySource>,
    /// List of emissions sources
    pub emissions_sources: Vec<EmissionsSource>,
    /// List of carbon offsets applied (if any)
    pub offsets: Option<Vec<CarbonOffset>>,
    /// System resource utilization data
    pub resource_utilization: Option<ResourceUtilization>,
    /// Efficiency improvement percentage compared to previous period (if available)
    pub efficiency_improvement: Option<f64>,
    /// Energy efficiency metric (kWh per resource utilization unit)
    pub energy_efficiency: f64,
    /// Carbon intensity (g CO2e per kWh)
    pub carbon_intensity: f64,
    /// Level of detail for this report
    pub detail_level: String,
}

/// Energy usage information
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct EnergyUsage {
    /// Timestamp when this data was generated
    pub timestamp: u64,
    /// Time period in seconds that this data covers
    pub period: u64,
    /// Current power consumption in watts
    pub current_power_watts: f64,
    /// Total energy consumption in kilowatt-hours
    pub total_energy_kwh: f64,
    /// List of energy sources used
    pub energy_sources: Vec<EnergySource>,
    /// Energy efficiency metric (kWh per resource utilization unit)
    pub efficiency: f64,
    /// Historical energy usage data (if requested)
    pub history: Option<Vec<EnergyUsageHistory>>,
}

/// Carbon footprint information
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CarbonFootprint {
    /// Timestamp when this data was generated
    pub timestamp: u64,
    /// Time period in seconds that this data covers
    pub period: u64,
    /// Total carbon emissions in grams of CO2e
    pub total_emissions_g: f64,
    /// Net carbon emissions after offsets in grams of CO2e
    pub net_emissions_g: f64,
    /// List of carbon offsets applied (if any)
    pub offsets: Option<Vec<CarbonOffset>>,
    /// Carbon intensity (g CO2e per kWh)
    pub intensity: f64,
    /// List of emissions sources
    pub emissions_sources: Vec<EmissionsSource>,
    /// Percentage of energy from renewable sources
    pub renewable_percentage: f64,
}

/// System resource utilization information
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ResourceUtilization {
    /// Timestamp when this data was generated
    pub timestamp: u64,
    /// Time period in seconds that this data covers
    pub period: u64,
    /// CPU utilization percentage
    pub cpu_usage: f64,
    /// Memory utilization percentage
    pub memory_usage: f64,
    /// Disk utilization percentage
    pub disk_usage: f64,
    /// Network utilization percentage
    pub network_usage: f64,
    /// System uptime in seconds
    pub uptime_seconds: u64,
}

/// Environmental monitoring settings
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct EnvironmentalSettings {
    /// Whether environmental monitoring is enabled
    pub monitoring_enabled: bool,
    /// Whether emissions tracking is enabled
    pub emission_tracking_enabled: bool,
    /// Whether power saving mode is enabled
    pub power_saving_mode: bool,
    /// Percentage of energy from renewable sources (if known)
    pub renewable_energy_percentage: Option<f64>,
    /// Type of energy source (e.g., "grid", "solar", "mixed")
    pub energy_source_type: Option<String>,
    /// Whether carbon offset mechanism is enabled
    pub carbon_offset_enabled: bool,
    /// Number of days to retain environmental data
    pub data_retention_days: u64,
    /// Target energy efficiency (kWh per resource unit)
    pub energy_efficiency_target: Option<f64>,
    /// Geographic location code for emissions calculation
    pub location_code: Option<String>,
} 