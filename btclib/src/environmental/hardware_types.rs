use serde::{Serialize, Deserialize};
use std::collections::HashMap;
use std::fmt;

/// Mining hardware specifications
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HardwareSpec {
    /// Hardware name
    pub name: String,
    /// Manufacturer
    pub manufacturer: String,
    /// Energy efficiency in Joules per TeraHash (J/TH)
    pub efficiency: f64,
    /// Nominal hashrate in TH/s
    pub hashrate: f64,
    /// Power consumption in watts
    pub power: f64,
    /// Release year
    pub year: Option<u16>,
    /// Chip size in nanometers
    pub chip_size: Option<u16>,
}

impl HardwareSpec {
    /// Calculate daily energy consumption in kWh
    pub fn daily_energy_consumption(&self) -> f64 {
        (self.power * 24.0) / 1000.0
    }
    
    /// Calculate carbon footprint based on energy source and emission factor
    pub fn carbon_footprint(&self, renewable_percentage: f64, emission_factor: f64) -> f64 {
        let daily_energy = self.daily_energy_consumption();
        let grid_energy = daily_energy * (1.0 - renewable_percentage / 100.0);
        
        // Calculate emissions (kgCO2e)
        // Energy (kWh) * Emission Factor (gCO2/kWh) / 1000 = kgCO2e
        grid_energy * emission_factor / 1000.0
    }
}

/// Standard hardware types supported by the system
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
    /// Whatsminer M50
    WhatsminerM50,
    /// Avalon A1246
    AvalonA1246,
    /// Avalon A1366
    AvalonA1366,
    /// Custom ASIC
    CustomASIC,
    /// Other hardware
    Other,
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
            HardwareType::WhatsminerM50 => write!(f, "Whatsminer M50"),
            HardwareType::AvalonA1246 => write!(f, "Avalon A1246"),
            HardwareType::AvalonA1366 => write!(f, "Avalon A1366"),
            HardwareType::CustomASIC => write!(f, "Custom ASIC"),
            HardwareType::Other => write!(f, "Other Hardware"),
        }
    }
}

/// Hardware database containing specifications for different mining hardware
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HardwareDatabase {
    /// Hardware specifications by type
    pub specs: HashMap<HardwareType, HardwareSpec>,
    /// Database version
    pub version: String,
    /// Last update timestamp
    pub last_updated: i64,
}

impl HardwareDatabase {
    /// Create a new hardware database with default values
    pub fn new() -> Self {
        let mut db = Self {
            specs: HashMap::new(),
            version: "1.0.0".to_string(),
            last_updated: chrono::Utc::now().timestamp(),
        };
        
        // Populate with default data
        db.initialize_default_specs();
        db
    }
    
    /// Get hardware specification for a given hardware type
    pub fn get_spec(&self, hardware_type: HardwareType) -> Option<&HardwareSpec> {
        self.specs.get(&hardware_type)
    }
    
    /// Get a mutable reference to a hardware specification
    pub fn get_spec_mut(&mut self, hardware_type: HardwareType) -> Option<&mut HardwareSpec> {
        self.specs.get_mut(&hardware_type)
    }
    
    /// Add or update a hardware specification
    pub fn update_spec(&mut self, hardware_type: HardwareType, spec: HardwareSpec) {
        self.specs.insert(hardware_type, spec);
        self.last_updated = chrono::Utc::now().timestamp();
    }
    
    /// Initialize the database with default hardware specifications
    fn initialize_default_specs(&mut self) {
        // Antminer S9
        self.update_spec(
            HardwareType::AntminerS9,
            HardwareSpec {
                name: "Antminer S9".to_string(),
                manufacturer: "Bitmain".to_string(),
                efficiency: 98.0, // J/TH
                hashrate: 14.0,   // TH/s
                power: 1372.0,    // Watts
                year: Some(2016),
                chip_size: Some(16),
            }
        );
        
        // Antminer S19
        self.update_spec(
            HardwareType::AntminerS19,
            HardwareSpec {
                name: "Antminer S19".to_string(),
                manufacturer: "Bitmain".to_string(),
                efficiency: 34.5, // J/TH
                hashrate: 95.0,   // TH/s
                power: 3250.0,    // Watts
                year: Some(2020),
                chip_size: Some(7),
            }
        );
        
        // Antminer S19 Pro
        self.update_spec(
            HardwareType::AntminerS19Pro,
            HardwareSpec {
                name: "Antminer S19 Pro".to_string(),
                manufacturer: "Bitmain".to_string(),
                efficiency: 29.5, // J/TH
                hashrate: 110.0,  // TH/s
                power: 3250.0,    // Watts
                year: Some(2020),
                chip_size: Some(7),
            }
        );
        
        // Antminer S19j Pro
        self.update_spec(
            HardwareType::AntminerS19jPro,
            HardwareSpec {
                name: "Antminer S19j Pro".to_string(),
                manufacturer: "Bitmain".to_string(),
                efficiency: 30.0, // J/TH
                hashrate: 104.0,  // TH/s
                power: 3120.0,    // Watts
                year: Some(2021),
                chip_size: Some(7),
            }
        );
        
        // Antminer S19 XP
        self.update_spec(
            HardwareType::AntminerS19XP,
            HardwareSpec {
                name: "Antminer S19 XP".to_string(),
                manufacturer: "Bitmain".to_string(),
                efficiency: 21.5, // J/TH
                hashrate: 140.0,  // TH/s
                power: 3010.0,    // Watts
                year: Some(2022),
                chip_size: Some(5),
            }
        );
        
        // Whatsminer M30S
        self.update_spec(
            HardwareType::WhatsminerM30S,
            HardwareSpec {
                name: "Whatsminer M30S".to_string(),
                manufacturer: "MicroBT".to_string(),
                efficiency: 42.0, // J/TH
                hashrate: 88.0,   // TH/s
                power: 3344.0,    // Watts
                year: Some(2020),
                chip_size: Some(8),
            }
        );
        
        // Whatsminer M30S+
        self.update_spec(
            HardwareType::WhatsminerM30SPlus,
            HardwareSpec {
                name: "Whatsminer M30S+".to_string(),
                manufacturer: "MicroBT".to_string(),
                efficiency: 38.0, // J/TH
                hashrate: 100.0,  // TH/s
                power: 3400.0,    // Watts
                year: Some(2020),
                chip_size: Some(8),
            }
        );
        
        // Whatsminer M30S++
        self.update_spec(
            HardwareType::WhatsminerM30SPlusPlus,
            HardwareSpec {
                name: "Whatsminer M30S++".to_string(),
                manufacturer: "MicroBT".to_string(),
                efficiency: 31.0, // J/TH
                hashrate: 112.0,  // TH/s
                power: 3472.0,    // Watts
                year: Some(2020),
                chip_size: Some(8),
            }
        );
        
        // Whatsminer M50
        self.update_spec(
            HardwareType::WhatsminerM50,
            HardwareSpec {
                name: "Whatsminer M50".to_string(),
                manufacturer: "MicroBT".to_string(),
                efficiency: 26.0, // J/TH
                hashrate: 126.0,  // TH/s
                power: 3276.0,    // Watts
                year: Some(2022),
                chip_size: Some(5),
            }
        );
        
        // Avalon A1246
        self.update_spec(
            HardwareType::AvalonA1246,
            HardwareSpec {
                name: "Avalon A1246".to_string(),
                manufacturer: "Canaan".to_string(),
                efficiency: 38.0, // J/TH
                hashrate: 90.0,   // TH/s
                power: 3420.0,    // Watts
                year: Some(2020),
                chip_size: Some(8),
            }
        );
        
        // Avalon A1366
        self.update_spec(
            HardwareType::AvalonA1366,
            HardwareSpec {
                name: "Avalon A1366".to_string(),
                manufacturer: "Canaan".to_string(),
                efficiency: 35.0, // J/TH
                hashrate: 95.0,   // TH/s
                power: 3325.0,    // Watts
                year: Some(2021),
                chip_size: Some(7),
            }
        );
        
        // Custom ASIC (placeholder for custom hardware)
        self.update_spec(
            HardwareType::CustomASIC,
            HardwareSpec {
                name: "Custom ASIC".to_string(),
                manufacturer: "Various".to_string(),
                efficiency: 24.0, // J/TH
                hashrate: 130.0,  // TH/s
                power: 3120.0,    // Watts
                year: None,
                chip_size: None,
            }
        );
        
        // Other (placeholder for unrecognized hardware)
        self.update_spec(
            HardwareType::Other,
            HardwareSpec {
                name: "Other Hardware".to_string(),
                manufacturer: "Unknown".to_string(),
                efficiency: 40.0, // J/TH
                hashrate: 80.0,   // TH/s
                power: 3200.0,    // Watts
                year: None,
                chip_size: None,
            }
        );
    }
    
    /// Get hardware types sorted by efficiency (most efficient first)
    pub fn get_most_efficient_hardware(&self, count: usize) -> Vec<(HardwareType, &HardwareSpec)> {
        let mut hardware: Vec<(HardwareType, &HardwareSpec)> = self.specs
            .iter()
            .map(|(hw_type, spec)| (*hw_type, spec))
            .collect();
        
        // Sort by efficiency (ascending = more efficient first)
        hardware.sort_by(|a, b| a.1.efficiency.partial_cmp(&b.1.efficiency).unwrap_or(std::cmp::Ordering::Equal));
        
        // Take top N
        hardware.into_iter().take(count).collect()
    }
    
    /// Calculate average efficiency of all hardware
    pub fn calculate_average_efficiency(&self) -> f64 {
        if self.specs.is_empty() {
            return 0.0;
        }
        
        let total_efficiency: f64 = self.specs.values().map(|spec| spec.efficiency).sum();
        total_efficiency / self.specs.len() as f64
    }
    
    /// Export the database to JSON
    pub fn export_to_json(&self) -> Result<String, String> {
        serde_json::to_string_pretty(self)
            .map_err(|e| format!("Failed to export database to JSON: {}", e))
    }
    
    /// Import the database from JSON
    pub fn import_from_json(json: &str) -> Result<Self, String> {
        serde_json::from_str(json)
            .map_err(|e| format!("Failed to import database from JSON: {}", e))
    }
}

impl Default for HardwareDatabase {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_database_initialization() {
        let db = HardwareDatabase::new();
        assert!(!db.specs.is_empty());
        assert!(db.specs.contains_key(&HardwareType::AntminerS19Pro));
    }
    
    #[test]
    fn test_get_spec() {
        let db = HardwareDatabase::new();
        
        // Test getting a known hardware spec
        let spec = db.get_spec(HardwareType::AntminerS19Pro);
        assert!(spec.is_some());
        assert_eq!(spec.unwrap().efficiency, 29.5);
        
        // Test getting an unknown hardware spec
        let spec = db.get_spec(HardwareType::Other);
        assert!(spec.is_some());
    }
    
    #[test]
    fn test_daily_energy_consumption() {
        let db = HardwareDatabase::new();
        let spec = db.get_spec(HardwareType::AntminerS19Pro).unwrap();
        
        // Daily consumption = power (W) * 24 hours / 1000 = kWh
        let expected_consumption = 3250.0 * 24.0 / 1000.0;
        assert_eq!(spec.daily_energy_consumption(), expected_consumption);
    }
    
    #[test]
    fn test_most_efficient_hardware() {
        let db = HardwareDatabase::new();
        let most_efficient = db.get_most_efficient_hardware(1);
        
        assert_eq!(most_efficient.len(), 1);
        // The S19 XP should be the most efficient
        assert_eq!(most_efficient[0].0, HardwareType::AntminerS19XP);
    }
} 