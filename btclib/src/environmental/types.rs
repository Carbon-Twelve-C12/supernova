use serde::{Deserialize, Serialize};
use std::fmt;
use chrono::{DateTime, Utc};
use std::collections::HashMap;

/// Energy source types for miners
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
    /// Coal power
    Coal,
    /// Natural gas
    NaturalGas,
    /// Oil/petroleum
    Oil,
    /// Biomass
    Biomass,
    /// Grid mix (varies by region)
    Grid,
    /// Other or unspecified source
    Other,
    /// Unknown source
    Unknown,
}

impl EnergySource {
    /// Check if an energy source is renewable
    pub fn is_renewable(&self) -> bool {
        match self {
            EnergySource::Solar => true,
            EnergySource::Wind => true,
            EnergySource::Hydro => true,
            EnergySource::Geothermal => true,
            EnergySource::Biomass => true,
            EnergySource::Nuclear => false, // Zero-carbon but not renewable
            EnergySource::Coal => false,
            EnergySource::NaturalGas => false,
            EnergySource::Oil => false,
            EnergySource::Grid => false, // Depends on regional mix, defaulting to false
            EnergySource::Other => false, // Conservative default
            EnergySource::Unknown => false,
        }
    }
    
    /// Check if an energy source is zero carbon
    pub fn is_zero_carbon(&self) -> bool {
        match self {
            EnergySource::Solar => true,
            EnergySource::Wind => true,
            EnergySource::Hydro => true,
            EnergySource::Geothermal => true,
            EnergySource::Nuclear => true,
            EnergySource::Biomass => false, // Can have lifecycle emissions
            EnergySource::Coal => false,
            EnergySource::NaturalGas => false,
            EnergySource::Oil => false,
            EnergySource::Grid => false,
            EnergySource::Other => false,
            EnergySource::Unknown => false,
        }
    }
    
    /// Get the default emissions factor (tonnes CO2e per MWh)
    pub fn default_emissions_factor(&self) -> f64 {
        match self {
            EnergySource::Solar => 0.048, // Lifecycle emissions
            EnergySource::Wind => 0.011,
            EnergySource::Hydro => 0.024,
            EnergySource::Geothermal => 0.038,
            EnergySource::Nuclear => 0.012,
            EnergySource::Biomass => 0.23,
            EnergySource::Coal => 1.0,
            EnergySource::NaturalGas => 0.43,
            EnergySource::Oil => 0.65,
            EnergySource::Grid => 0.475, // Global average
            EnergySource::Other => 0.5, // Conservative estimate
            EnergySource::Unknown => 0.0, // Default to zero for unknown sources
        }
    }
}

impl fmt::Display for EnergySource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EnergySource::Solar => write!(f, "Solar"),
            EnergySource::Wind => write!(f, "Wind"),
            EnergySource::Hydro => write!(f, "Hydroelectric"),
            EnergySource::Geothermal => write!(f, "Geothermal"),
            EnergySource::Nuclear => write!(f, "Nuclear"),
            EnergySource::Coal => write!(f, "Coal"),
            EnergySource::NaturalGas => write!(f, "Natural Gas"),
            EnergySource::Oil => write!(f, "Oil"),
            EnergySource::Biomass => write!(f, "Biomass"),
            EnergySource::Grid => write!(f, "Grid Mix"),
            EnergySource::Other => write!(f, "Other"),
            EnergySource::Unknown => write!(f, "Unknown"),
        }
    }
}

impl std::str::FromStr for EnergySource {
    type Err = String;
    
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "solar" => Ok(EnergySource::Solar),
            "wind" => Ok(EnergySource::Wind),
            "hydro" | "hydroelectric" => Ok(EnergySource::Hydro),
            "geothermal" => Ok(EnergySource::Geothermal),
            "nuclear" => Ok(EnergySource::Nuclear),
            "coal" => Ok(EnergySource::Coal),
            "natural gas" | "naturalgas" | "gas" => Ok(EnergySource::NaturalGas),
            "oil" | "petroleum" => Ok(EnergySource::Oil),
            "biomass" => Ok(EnergySource::Biomass),
            "grid" | "grid mix" => Ok(EnergySource::Grid),
            "unknown" => Ok(EnergySource::Unknown),
            _ => Ok(EnergySource::Other),
        }
    }
}

/// Geographic regions for environmental tracking
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Region {
    /// North America
    NorthAmerica,
    /// Europe
    Europe,
    /// Asia-Pacific
    AsiaPacific,
    /// South America
    SouthAmerica,
    /// Africa
    Africa,
    /// Middle East
    MiddleEast,
    /// Global (not region-specific)
    Global,
}

impl Region {
    /// Create a new region from country code
    pub fn new(country_code: &str) -> Self {
        match country_code.to_uppercase().as_str() {
            "US" | "CA" => Self::NorthAmerica,
            "GB" | "DE" | "FR" | "IT" | "ES" => Self::Europe,
            "CN" | "JP" | "KR" | "IN" | "AU" => Self::AsiaPacific,
            "BR" | "AR" | "CL" | "CO" | "PE" => Self::SouthAmerica,
            "ZA" | "NG" | "KE" | "EG" | "MA" => Self::Africa,
            "SA" | "AE" | "QA" | "IL" | "TR" => Self::MiddleEast,
            _ => Self::Global,
        }
    }
    
    /// Get ISO country code for region
    pub fn to_string(&self) -> String {
        match self {
            Self::NorthAmerica => "NA".to_string(),
            Self::Europe => "EU".to_string(),
            Self::AsiaPacific => "APAC".to_string(),
            Self::SouthAmerica => "SA".to_string(),
            Self::Africa => "AF".to_string(),
            Self::MiddleEast => "ME".to_string(),
            Self::Global => "GLOBAL".to_string(),
        }
    }
}

impl fmt::Display for Region {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_string())
    }
}

/// Emissions data source provider
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EmissionsDataSource {
    /// International Energy Agency
    IEA,
    /// US Environmental Protection Agency
    EPA,
    /// European Environment Agency
    EEA,
    /// WattTime real-time grid data
    WattTime,
    /// Tomorrow.io grid data
    Tomorrow,
    /// UNFCCC harmonized grid factors
    UNFCCC,
    /// Electricity Maps API
    ElectricityMaps,
    /// Custom or user-provided data
    Custom,
    /// Data from a reliable third-party provider
    ThirdPartyProvider(String),
    /// Government-provided data
    Government(String),
    /// Self-reported data
    SelfReported,
    /// Default values from established sources
    Default,
    /// Real-time grid data
    RealTimeGrid(String),
}

/// Emissions factor type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EmissionsFactorType {
    /// Average grid emissions
    GridAverage,
    /// Marginal operating emissions rate
    Marginal,
    /// Residual mix emissions
    ResidualMix,
    /// Grid electricity emissions
    GridElectricity,
    /// Natural gas combustion
    NaturalGas,
    /// Diesel combustion
    Diesel,
    /// Coal combustion
    Coal,
    /// Renewable energy
    Renewable,
}

/// Emissions factor for a specific region
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmissionFactor {
    /// Grid emissions factor in tonnes CO2e per MWh
    pub grid_emissions_factor: f64,
    /// Human-readable name of the region
    pub region_name: String,
    /// Source of the emissions data
    pub data_source: EmissionsDataSource,
    /// Type of emissions factor
    pub factor_type: EmissionsFactorType,
    /// Year the data was collected
    pub year: Option<u16>,
    /// Timestamp for real-time data
    pub timestamp: Option<DateTime<Utc>>,
    /// Confidence level (0-1) for this factor
    pub confidence: Option<f64>,
}

impl EmissionFactor {
    /// Create a new emission factor
    pub fn new(region: &Region, factor: f64, source: EmissionsDataSource) -> Self {
        Self {
            grid_emissions_factor: factor,
            region_name: region.to_string(),
            data_source: source,
            factor_type: EmissionsFactorType::GridAverage,
            year: Some(2023),
            timestamp: None,
            confidence: None,
        }
    }
    
    /// Create a new emission factor with detailed information
    pub fn with_details(
        region: &Region, 
        factor: f64, 
        source: EmissionsDataSource,
        factor_type: EmissionsFactorType,
        year: Option<u16>,
        timestamp: Option<DateTime<Utc>>,
        confidence: Option<f64>
    ) -> Self {
        Self {
            grid_emissions_factor: factor,
            region_name: region.to_string(),
            data_source: source,
            factor_type,
            year,
            timestamp,
            confidence,
        }
    }
    
    /// Create default emission factors for all regions
    pub fn default_factors() -> Vec<Self> {
        vec![
            Self::new(&Region::NorthAmerica, 0.38, EmissionsDataSource::EPA),
            Self::new(&Region::NorthAmerica, 0.12, EmissionsDataSource::IEA), // Canada
            Self::new(&Region::Europe, 0.28, EmissionsDataSource::EEA),
            Self::new(&Region::AsiaPacific, 0.63, EmissionsDataSource::IEA), // China
            Self::new(&Region::AsiaPacific, 0.72, EmissionsDataSource::IEA), // India
            Self::new(&Region::Global, 0.50, EmissionsDataSource::IEA), // Russia
            Self::new(&Region::AsiaPacific, 0.52, EmissionsDataSource::IEA), // Australia
            Self::new(&Region::SouthAmerica, 0.09, EmissionsDataSource::IEA), // Brazil
            Self::new(&Region::Africa, 0.85, EmissionsDataSource::IEA), // South Africa
        ]
    }
    
    /// Create a global average factor
    pub fn global_average() -> Self {
        Self::new(&Region::Global, 0.475, EmissionsDataSource::IEA)
    }
}

/// Mining hardware types
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

impl HardwareType {
    /// Get typical power consumption in watts
    pub fn power_consumption(&self) -> f64 {
        match self {
            Self::Asic => 3000.0,   // High power ASIC
            Self::Gpu => 1200.0,    // High-end GPU mining rig
            Self::Cpu => 200.0,     // Modern CPU
            Self::Other => 400.0,   // Conservative default
        }
    }
    
    /// Get typical hashrate in TH/s for SHA-256
    pub fn hashrate(&self) -> f64 {
        match self {
            Self::Asic => 100.0,    // Modern ASIC
            Self::Gpu => 0.1,       // GPUs are inefficient for SHA-256
            Self::Cpu => 0.001,     // CPUs are very inefficient for SHA-256
            Self::Other => 50.0,    // Conservative estimate
        }
    }
    
    /// Calculate daily energy consumption for this hardware type in kWh/day
    pub fn daily_energy_consumption(&self) -> f64 {
        let efficiency = self.energy_efficiency(); // J/TH
        let hashrate = self.hashrate(); // TH/s
        
        // Convert J/TH to kWh/day
        // (J/TH) * (TH/s) * (seconds per day) / (Joules per kWh)
        efficiency * hashrate * 86400.0 / 3_600_000.0
    }

    /// Get the energy efficiency in J/TH for this hardware type
    pub fn energy_efficiency(&self) -> f64 {
        let power_w = self.power_consumption();
        let hashrate_ths = self.hashrate();
        
        if hashrate_ths > 0.0 {
            // Convert W to J/s, then multiply by seconds per hour, divide by TH/s
            // The result is Joules per TH
            (power_w * 3600.0) / hashrate_ths
        } else {
            f64::MAX // Avoid division by zero
        }
    }
}

/// Implement Display for HardwareType
impl fmt::Display for HardwareType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            HardwareType::Asic => write!(f, "ASIC"),
            HardwareType::Gpu => write!(f, "GPU"),
            HardwareType::Cpu => write!(f, "CPU"),
            HardwareType::Other => write!(f, "Other"),
        }
    }
}

/// Default emissions factors for different regions
#[derive(Debug, Clone)]
pub struct DefaultEmissionsFactors {
    /// Map of regions to emission factors
    pub factors: HashMap<Region, EmissionFactor>,
}

impl Default for DefaultEmissionsFactors {
    fn default() -> Self {
        Self::new()
    }
}

impl DefaultEmissionsFactors {
    /// Create a new set of default emissions factors
    pub fn new() -> Self {
        let mut factors = HashMap::new();
        
        // Add some default emission factors for common regions
        factors.insert(
            Region::NorthAmerica,
            EmissionFactor {
                grid_emissions_factor: 0.38, // US average in kg CO2e/kWh
                region_name: "North America".to_string(),
                data_source: EmissionsDataSource::EPA,
                factor_type: EmissionsFactorType::GridAverage,
                year: Some(2022),
                timestamp: None,
                confidence: Some(0.9),
            }
        );
        
        factors.insert(
            Region::Europe,
            EmissionFactor {
                grid_emissions_factor: 0.23, // EU average in kg CO2e/kWh
                region_name: "Europe".to_string(),
                data_source: EmissionsDataSource::EEA,
                factor_type: EmissionsFactorType::GridAverage,
                year: Some(2022),
                timestamp: None,
                confidence: Some(0.9),
            }
        );
        
        factors.insert(
            Region::AsiaPacific,
            EmissionFactor {
                grid_emissions_factor: 0.64, // Asia average in kg CO2e/kWh
                region_name: "Asia-Pacific".to_string(),
                data_source: EmissionsDataSource::IEA,
                factor_type: EmissionsFactorType::GridAverage,
                year: Some(2022),
                timestamp: None,
                confidence: Some(0.8),
            }
        );
        
        factors.insert(
            Region::Global,
            EmissionFactor {
                grid_emissions_factor: 0.48, // Global average in kg CO2e/kWh
                region_name: "Global".to_string(),
                data_source: EmissionsDataSource::IEA,
                factor_type: EmissionsFactorType::GridAverage,
                year: Some(2022),
                timestamp: None,
                confidence: Some(0.7),
            }
        );
        
        Self {
            factors
        }
    }
}

// Type alias for backwards compatibility
pub type EnergySourceType = EnergySource; 