use std::collections::HashMap;
use serde::{Serialize, Deserialize};

/// Grid emission factor data sources
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EmissionFactorSource {
    /// International Energy Agency 2020 data
    IEA2020,
    /// International Energy Agency 2021 estimated data
    IEA2021,
    /// International Financial Institution Operating Marginal Grid Emission Factors
    IFI2020,
    /// WattTime Marginal Operating Emissions Rate
    WattTimeMOER,
    /// WattTime Operating Emissions Rate V3.2
    WattTimeOER,
}

/// Structure to represent emission factors for a specific country
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CountryEmissionFactor {
    /// Country name
    pub name: String,
    /// ISO-3 country code
    pub iso_code: String,
    /// Emission factors in gCO₂/kWh from different sources
    pub factors: HashMap<EmissionFactorSource, f64>,
    /// Last updated timestamp
    pub last_updated: i64,
}

/// Emission factors database
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmissionsFactorDatabase {
    /// Map of ISO country codes to emission factors
    pub countries: HashMap<String, CountryEmissionFactor>,
    /// Global average emission factor (gCO₂/kWh)
    pub global_average: f64,
    /// Database version
    pub version: String,
    /// Last update timestamp
    pub last_updated: i64,
}

impl EmissionsFactorDatabase {
    /// Create a new emission factors database with default values
    pub fn new() -> Self {
        let mut db = Self {
            countries: HashMap::new(),
            global_average: 475.0, // Global average based on IEA data
            version: "1.0.0".to_string(),
            last_updated: chrono::Utc::now().timestamp(),
        };

        // Populate with default data
        db.initialize_default_data();
        db
    }

    /// Get emission factor for a country with fallback to global average
    pub fn get_factor(&self, country_code: &str, source: EmissionFactorSource) -> f64 {
        self.countries
            .get(country_code)
            .and_then(|country| country.factors.get(&source))
            .copied()
            .unwrap_or(self.global_average)
    }

    /// Get best available emission factor for a country,
    /// prioritizing WattTime > IFI > IEA data
    pub fn get_best_factor(&self, country_code: &str) -> f64 {
        if let Some(country) = self.countries.get(country_code) {
            // Priority order: WattTime MOER > WattTime OER > IFI > IEA 2021 > IEA 2020
            if let Some(wt_moer) = country.factors.get(&EmissionFactorSource::WattTimeMOER) {
                return *wt_moer;
            }
            if let Some(wt_oer) = country.factors.get(&EmissionFactorSource::WattTimeOER) {
                return *wt_oer;
            }
            if let Some(ifi) = country.factors.get(&EmissionFactorSource::IFI2020) {
                return *ifi;
            }
            if let Some(iea2021) = country.factors.get(&EmissionFactorSource::IEA2021) {
                return *iea2021;
            }
            if let Some(iea2020) = country.factors.get(&EmissionFactorSource::IEA2020) {
                return *iea2020;
            }
        }

        // Fallback to global average
        self.global_average
    }

    /// Update emission factor for a country and source
    pub fn update_factor(&mut self, country_code: &str, source: EmissionFactorSource, value: f64) -> Result<(), String> {
        if let Some(country) = self.countries.get_mut(country_code) {
            country.factors.insert(source, value);
            country.last_updated = chrono::Utc::now().timestamp();
            Ok(())
        } else {
            Err(format!("Country code {} not found in database", country_code))
        }
    }

    /// Add a new country to the database
    pub fn add_country(&mut self, name: &str, iso_code: &str) -> Result<(), String> {
        if self.countries.contains_key(iso_code) {
            return Err(format!("Country with code {} already exists", iso_code));
        }

        let country = CountryEmissionFactor {
            name: name.to_string(),
            iso_code: iso_code.to_string(),
            factors: HashMap::new(),
            last_updated: chrono::Utc::now().timestamp(),
        };

        self.countries.insert(iso_code.to_string(), country);
        Ok(())
    }

    /// Initialize the database with default emission factor data
    fn initialize_default_data(&mut self) {
        // Function to add a country with emission factors
        let mut add_country_with_factors = |name: &str,
                                            iso_code: &str,
                                            iea2020: Option<f64>,
                                            iea2021: Option<f64>,
                                            ifi2020: Option<f64>,
                                            wt_moer: Option<f64>,
                                            wt_oer: Option<f64>| {
            // Add country if it doesn't exist
            if !self.countries.contains_key(iso_code) {
                let _ = self.add_country(name, iso_code);
            }

            // Add factors
            if let Some(factor) = iea2020 {
                let _ = self.update_factor(iso_code, EmissionFactorSource::IEA2020, factor);
            }
            if let Some(factor) = iea2021 {
                let _ = self.update_factor(iso_code, EmissionFactorSource::IEA2021, factor);
            }
            if let Some(factor) = ifi2020 {
                let _ = self.update_factor(iso_code, EmissionFactorSource::IFI2020, factor);
            }
            if let Some(factor) = wt_moer {
                let _ = self.update_factor(iso_code, EmissionFactorSource::WattTimeMOER, factor);
            }
            if let Some(factor) = wt_oer {
                let _ = self.update_factor(iso_code, EmissionFactorSource::WattTimeOER, factor);
            }
        };

        // Add countries with emission factors
        // Including a subset of countries from the data table
        add_country_with_factors("United States", "USA", Some(353.4), Some(366.5), Some(416.0), Some(515.0), Some(1136.0));
        add_country_with_factors("China", "CHN", Some(418.1), Some(446.3), Some(899.0), None, None);
        add_country_with_factors("Australia", "AUS", Some(678.3), Some(649.1), Some(808.0), Some(639.0), Some(1409.0));
        add_country_with_factors("Germany", "DEU", Some(311.0), Some(356.1), Some(650.0), Some(764.0), Some(1685.0));
        add_country_with_factors("United Kingdom", "GBR", Some(193.2), Some(220.0), Some(380.0), Some(428.0), Some(943.0));
        add_country_with_factors("France", "FRA", Some(51.1), Some(54.1), Some(158.0), Some(384.0), Some(846.0));
        add_country_with_factors("Canada", "CAN", Some(119.5), Some(121.5), Some(372.0), Some(485.0), Some(1069.0));
        add_country_with_factors("Brazil", "BRA", Some(93.1), Some(132.8), Some(284.0), None, None);
        add_country_with_factors("India", "IND", Some(689.3), Some(691.5), Some(951.0), None, None);
        add_country_with_factors("Russia", "RUS", Some(359.0), None, Some(476.0), None, None);
        add_country_with_factors("Japan", "JPN", Some(476.1), Some(461.3), Some(471.0), None, None);
        add_country_with_factors("Sweden", "SWE", Some(10.3), Some(14.4), Some(68.0), Some(782.0), Some(1724.0));
        add_country_with_factors("Norway", "NOR", Some(6.5), Some(4.1), Some(47.0), Some(243.0), Some(536.0));
        add_country_with_factors("South Africa", "ZAF", Some(923.8), Some(890.6), Some(1070.0), None, None);
        add_country_with_factors("Italy", "ITA", Some(264.7), Some(269.0), Some(414.0), Some(386.0), Some(851.0));
        add_country_with_factors("Iceland", "ISL", Some(0.1), Some(0.1), Some(0.0), None, None);
        add_country_with_factors("Ireland", "IRL", Some(265.7), Some(338.5), Some(380.0), Some(507.0), Some(1117.0));
        add_country_with_factors("Spain", "ESP", Some(153.3), Some(150.3), Some(402.0), Some(371.0), Some(817.0));
        add_country_with_factors("Poland", "POL", Some(622.8), Some(640.3), Some(828.0), Some(852.0), Some(1879.0));
        add_country_with_factors("Switzerland", "CHE", Some(24.3), Some(22.9), Some(48.0), None, None);

        // Add more countries as needed from the data table
        // This is a subset of the data for example purposes
    }

    /// Update the database from external sources
    pub fn update_from_api(&mut self) -> Result<(), String> {
        // In a real implementation, this would fetch data from an API
        // For now, we just update the timestamp
        self.last_updated = chrono::Utc::now().timestamp();
        self.version = format!("1.0.{}", self.last_updated % 1000);

        Ok(())
    }

    /// Get countries with lowest emission factors
    pub fn get_greenest_countries(&self, count: usize) -> Vec<(String, f64)> {
        let mut countries: Vec<(String, f64)> = self.countries
            .iter()
            .map(|(code, country)| (code.clone(), self.get_best_factor(code)))
            .collect();

        // Sort by emission factor (ascending)
        countries.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));

        // Take top N
        countries.into_iter().take(count).collect()
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

impl Default for EmissionsFactorDatabase {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_database_initialization() {
        let db = EmissionsFactorDatabase::new();
        assert!(!db.countries.is_empty());
        assert!(db.countries.contains_key("USA"));
    }

    #[test]
    fn test_get_best_factor() {
        let db = EmissionsFactorDatabase::new();

        // Test country with WattTime data (prioritized)
        let usa_factor = db.get_best_factor("USA");
        assert_eq!(usa_factor, 515.0); // Should return WattTime MOER

        // Test country with only IEA data
        let bra_factor = db.get_best_factor("BRA");
        assert_eq!(bra_factor, 284.0); // Should return IFI2020

        // Test non-existent country
        let unknown_factor = db.get_best_factor("XYZ");
        assert_eq!(unknown_factor, db.global_average); // Should return global average
    }

    #[test]
    fn test_update_factor() {
        let mut db = EmissionsFactorDatabase::new();

        // Update existing country
        let result = db.update_factor("USA", EmissionFactorSource::IEA2021, 400.0);
        assert!(result.is_ok());
        assert_eq!(db.get_factor("USA", EmissionFactorSource::IEA2021), 400.0);

        // Update non-existent country
        let result = db.update_factor("XYZ", EmissionFactorSource::IEA2021, 500.0);
        assert!(result.is_err());
    }
}