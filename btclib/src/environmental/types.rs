use serde::{Deserialize, Serialize};
use std::fmt;
use chrono::{DateTime, Utc};

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
        }
    }
}

/// Geographic regions for emissions calculations
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Region {
    /// ISO country code
    pub country_code: String,
    /// Optional sub-region code (e.g., state, province)
    pub sub_region: Option<String>,
    /// Optional grid zone identifier
    pub grid_zone: Option<String>,
    /// Latitude coordinate
    pub latitude: Option<f64>,
    /// Longitude coordinate
    pub longitude: Option<f64>,
}

impl Region {
    /// Create a new region with country code
    pub fn new(country_code: &str) -> Self {
        Self {
            country_code: country_code.to_string(),
            sub_region: None,
            grid_zone: None,
            latitude: None,
            longitude: None,
        }
    }
    
    /// Create a new region with country code and sub-region
    pub fn with_sub_region(country_code: &str, sub_region: &str) -> Self {
        Self {
            country_code: country_code.to_string(),
            sub_region: Some(sub_region.to_string()),
            grid_zone: None,
            latitude: None,
            longitude: None,
        }
    }
    
    /// Create a new region with full details
    pub fn with_details(
        country_code: &str, 
        sub_region: Option<&str>, 
        grid_zone: Option<&str>,
        latitude: Option<f64>,
        longitude: Option<f64>
    ) -> Self {
        Self {
            country_code: country_code.to_string(),
            sub_region: sub_region.map(|s| s.to_string()),
            grid_zone: grid_zone.map(|g| g.to_string()),
            latitude,
            longitude,
        }
    }
    
    /// Get a string representation of the region
    pub fn to_string(&self) -> String {
        if let Some(sub_region) = &self.sub_region {
            if let Some(grid_zone) = &self.grid_zone {
                format!("{}-{}-{}", self.country_code, sub_region, grid_zone)
            } else {
                format!("{}-{}", self.country_code, sub_region)
            }
        } else {
            self.country_code.clone()
        }
    }
}

impl fmt::Display for Region {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_string())
    }
}

/// Emissions data source provider
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
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
            Self::new(&Region::new("US"), 0.38, EmissionsDataSource::EPA),
            Self::new(&Region::new("CA"), 0.12, EmissionsDataSource::IEA),
            Self::new(&Region::new("EU"), 0.28, EmissionsDataSource::EEA),
            Self::new(&Region::new("CN"), 0.63, EmissionsDataSource::IEA),
            Self::new(&Region::new("IN"), 0.72, EmissionsDataSource::IEA),
            Self::new(&Region::new("RU"), 0.50, EmissionsDataSource::IEA),
            Self::new(&Region::new("AU"), 0.52, EmissionsDataSource::IEA),
            Self::new(&Region::new("BR"), 0.09, EmissionsDataSource::IEA),
            Self::new(&Region::new("ZA"), 0.85, EmissionsDataSource::IEA),
            Self::new(&Region::with_sub_region("US", "CA"), 0.21, EmissionsDataSource::EPA),
            Self::new(&Region::with_sub_region("US", "WA"), 0.09, EmissionsDataSource::EPA),
            Self::new(&Region::with_sub_region("US", "TX"), 0.41, EmissionsDataSource::EPA),
            Self::new(&Region::with_sub_region("US", "WY"), 0.79, EmissionsDataSource::EPA),
        ]
    }
    
    /// Create a global average factor
    pub fn global_average() -> Self {
        Self::new(&Region::new("GLOBAL"), 0.475, EmissionsDataSource::IEA)
    }
}

/// Mining hardware types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum HardwareType {
    /// Antminer S9
    AntminerS9,
    /// Antminer S19
    AntminerS19,
    /// Antminer S19 Pro
    AntminerS19Pro,
    /// Antminer S19j Pro
    AntminerS19jPro,
    /// Antminer S19 XP
    AntminerS19XP,
    /// Whatsminer M30S
    WhatsminerM30S,
    /// Whatsminer M30S+
    WhatsminerM30SPlus,
    /// Whatsminer M30S++
    WhatsminerM30SPlusPlus,
    /// AvalonMiner 1246
    AvalonMiner1246,
    /// AvalonMiner 1066
    AvalonMiner1066,
    /// Custom ASIC
    CustomASIC,
    /// FPGA
    FPGA,
    /// GPU
    GPU,
    /// Other
    Other,
}

impl HardwareType {
    /// Get the typical energy efficiency for this hardware type in J/TH
    pub fn typical_efficiency(&self) -> f64 {
        match self {
            HardwareType::AntminerS9 => 98.0,
            HardwareType::AntminerS19 => 34.5,
            HardwareType::AntminerS19Pro => 29.5,
            HardwareType::AntminerS19jPro => 29.5,
            HardwareType::AntminerS19XP => 21.5,
            HardwareType::WhatsminerM30S => 38.0,
            HardwareType::WhatsminerM30SPlus => 34.0,
            HardwareType::WhatsminerM30SPlusPlus => 31.0,
            HardwareType::AvalonMiner1246 => 38.0,
            HardwareType::AvalonMiner1066 => 65.0,
            HardwareType::CustomASIC => 30.0, // Conservative estimate
            HardwareType::FPGA => 120.0,
            HardwareType::GPU => 200.0,
            HardwareType::Other => 60.0, // Conservative average
        }
    }
    
    /// Get the typical hashrate for this hardware type in TH/s
    pub fn typical_hashrate(&self) -> f64 {
        match self {
            HardwareType::AntminerS9 => 14.0,
            HardwareType::AntminerS19 => 95.0,
            HardwareType::AntminerS19Pro => 110.0,
            HardwareType::AntminerS19jPro => 104.0,
            HardwareType::AntminerS19XP => 140.0,
            HardwareType::WhatsminerM30S => 88.0,
            HardwareType::WhatsminerM30SPlus => 100.0,
            HardwareType::WhatsminerM30SPlusPlus => 112.0,
            HardwareType::AvalonMiner1246 => 90.0,
            HardwareType::AvalonMiner1066 => 55.0,
            HardwareType::CustomASIC => 100.0, // Conservative estimate
            HardwareType::FPGA => 10.0,
            HardwareType::GPU => 0.1,
            HardwareType::Other => 50.0, // Conservative average
        }
    }
    
    /// Calculate daily energy consumption for this hardware type in kWh/day
    pub fn daily_energy_consumption(&self) -> f64 {
        let efficiency = self.typical_efficiency(); // J/TH
        let hashrate = self.typical_hashrate(); // TH/s
        
        // Convert J/TH to kWh/day
        // (J/TH) * (TH/s) * (seconds per day) / (Joules per kWh)
        efficiency * hashrate * 86400.0 / 3_600_000.0
    }
}

impl fmt::Display for HardwareType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            HardwareType::AntminerS9 => write!(f, "Antminer S9"),
            HardwareType::AntminerS19 => write!(f, "Antminer S19"),
            HardwareType::AntminerS19Pro => write!(f, "Antminer S19 Pro"),
            HardwareType::AntminerS19jPro => write!(f, "Antminer S19j Pro"),
            HardwareType::AntminerS19XP => write!(f, "Antminer S19 XP"),
            HardwareType::WhatsminerM30S => write!(f, "Whatsminer M30S"),
            HardwareType::WhatsminerM30SPlus => write!(f, "Whatsminer M30S+"),
            HardwareType::WhatsminerM30SPlusPlus => write!(f, "Whatsminer M30S++"),
            HardwareType::AvalonMiner1246 => write!(f, "AvalonMiner 1246"),
            HardwareType::AvalonMiner1066 => write!(f, "AvalonMiner 1066"),
            HardwareType::CustomASIC => write!(f, "Custom ASIC"),
            HardwareType::FPGA => write!(f, "FPGA"),
            HardwareType::GPU => write!(f, "GPU"),
            HardwareType::Other => write!(f, "Other"),
        }
    }
} 