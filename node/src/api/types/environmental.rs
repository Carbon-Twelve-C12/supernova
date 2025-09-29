use actix_web::{HttpResponse, Responder};
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

/// Environmental impact of mining operations
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct EnvironmentalImpact {
    /// Current carbon emissions in grams CO2e per hour
    pub carbon_emissions_g_per_hour: f64,
    /// Renewable energy percentage
    pub renewable_percentage: f64,
    /// Carbon intensity (gCO2e/kWh)
    pub carbon_intensity: f64,
    /// Carbon offsets purchased in tons CO2e
    pub carbon_offsets_tons: f64,
    /// Net carbon emissions after offsets (gCO2e/hour)
    pub net_emissions_g_per_hour: f64,
    /// Whether the node is carbon negative
    pub is_carbon_negative: bool,
    /// Environmental score (0-100)
    pub environmental_score: f64,
    /// Green mining bonus percentage
    pub green_mining_bonus: f64,
    /// Data sources used for calculations
    pub data_sources: Vec<String>,
    /// Timestamp of calculation
    pub calculated_at: u64,
}

impl Responder for EnvironmentalImpact {
    type Body = actix_web::body::BoxBody;

    fn respond_to(self, _req: &actix_web::HttpRequest) -> HttpResponse<Self::Body> {
        HttpResponse::Ok().json(self)
    }
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
