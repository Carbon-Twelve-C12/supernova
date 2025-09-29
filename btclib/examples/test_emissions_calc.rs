// Simple test script to calculate emissions
// This script contains only basic functions that rely on minimal dependencies

struct Region {
    country_code: String,
    sub_region: Option<String>,
}

impl Region {
    fn new(country_code: &str) -> Self {
        Self {
            country_code: country_code.to_string(),
            sub_region: None,
        }
    }

    fn with_sub_region(country_code: &str, sub_region: &str) -> Self {
        Self {
            country_code: country_code.to_string(),
            sub_region: Some(sub_region.to_string()),
        }
    }
}

struct EmissionFactor {
    grid_emissions_factor: f64, // tonnes CO2e per MWh
    region_name: String,
}

impl EmissionFactor {
    fn new(region_name: &str, factor: f64) -> Self {
        Self {
            grid_emissions_factor: factor,
            region_name: region_name.to_string(),
        }
    }
}

struct HardwareType {
    name: String,
    efficiency_j_th: f64, // Joules per Terahash
    hashrate_th: f64,     // Terahash per second
}

impl HardwareType {
    fn new(name: &str, efficiency: f64, hashrate: f64) -> Self {
        Self {
            name: name.to_string(),
            efficiency_j_th: efficiency,
            hashrate_th: hashrate,
        }
    }

    fn daily_energy_consumption_kwh(&self) -> f64 {
        // J/TH * TH/s * seconds per day / Joules per kWh
        self.efficiency_j_th * self.hashrate_th * 86400.0 / 3_600_000.0
    }
}

struct MinerInfo {
    region: Region,
    hardware: Vec<HardwareType>,
    renewable_percentage: f64,
    has_recs: bool,
    has_offsets: bool,
}

struct EmissionsCalculator {
    emission_factors: Vec<EmissionFactor>,
}

impl EmissionsCalculator {
    fn new() -> Self {
        let mut calculator = Self {
            emission_factors: Vec::new(),
        };

        // Add default emission factors
        calculator
            .emission_factors
            .push(EmissionFactor::new("US", 0.38));
        calculator
            .emission_factors
            .push(EmissionFactor::new("US-CA", 0.21));
        calculator
            .emission_factors
            .push(EmissionFactor::new("US-WA", 0.09));
        calculator
            .emission_factors
            .push(EmissionFactor::new("EU", 0.28));
        calculator
            .emission_factors
            .push(EmissionFactor::new("CN", 0.63));
        calculator
            .emission_factors
            .push(EmissionFactor::new("IN", 0.72));
        calculator
            .emission_factors
            .push(EmissionFactor::new("GLOBAL", 0.475));

        calculator
    }

    fn get_emission_factor(&self, region: &Region) -> f64 {
        // Try to find a match for region with sub_region first
        if let Some(sub_region) = &region.sub_region {
            let region_name = format!("{}-{}", region.country_code, sub_region);
            for factor in &self.emission_factors {
                if factor.region_name == region_name {
                    return factor.grid_emissions_factor;
                }
            }
        }

        // Try to find a match for just the country code
        for factor in &self.emission_factors {
            if factor.region_name == region.country_code {
                return factor.grid_emissions_factor;
            }
        }

        // Default to global average
        0.475 // Global average (tonnes CO2e per MWh)
    }

    fn calculate_miner_emissions(&self, miner: &MinerInfo) -> (f64, f64, f64) {
        // Calculate total energy consumption
        let daily_energy_kwh = miner
            .hardware
            .iter()
            .map(|hw| hw.daily_energy_consumption_kwh())
            .sum::<f64>();

        // Annual energy in MWh
        let annual_energy_mwh = daily_energy_kwh * 365.0 / 1000.0;

        // Get emission factor for the region
        let emission_factor = self.get_emission_factor(&miner.region);

        // Calculate gross emissions (without considering renewable percentage)
        let gross_emissions = annual_energy_mwh * emission_factor;

        // Apply renewable percentage to get location-based emissions
        let non_renewable_percentage = 100.0 - miner.renewable_percentage;
        let location_based = gross_emissions * (non_renewable_percentage / 100.0);

        // Market-based emissions (with RECs)
        let market_based = if miner.has_recs {
            // If the miner has RECs, we assume they cover all renewable claims
            location_based * 0.2 // Just for demonstration, assume RECs reduce by 80%
        } else {
            location_based
        };

        // Carbon offset impact
        let net_emissions = if miner.has_offsets {
            market_based * 0.7 // Just for demonstration, assume offsets reduce by 30%
        } else {
            market_based
        };

        (location_based, market_based, net_emissions)
    }
}

fn main() {
    // Create our emissions calculator
    let calculator = EmissionsCalculator::new();

    // Create some example miners

    // 1. US-based green miner with RECs
    let us_miner = MinerInfo {
        region: Region::with_sub_region("US", "WA"),
        hardware: vec![
            HardwareType::new("AntminerS19XP", 21.5, 140.0),
            HardwareType::new("WhatsminerM30SPlus", 34.0, 100.0),
        ],
        renewable_percentage: 75.0,
        has_recs: true,
        has_offsets: false,
    };

    // 2. European miner with 100% renewable and RECs
    let eu_miner = MinerInfo {
        region: Region::new("EU"),
        hardware: vec![HardwareType::new("AntminerS19XP", 21.5, 140.0)],
        renewable_percentage: 100.0,
        has_recs: true,
        has_offsets: false,
    };

    // 3. Asian miner with offsets but not RECs
    let asia_miner = MinerInfo {
        region: Region::new("CN"),
        hardware: vec![
            HardwareType::new("AntminerS19", 34.5, 95.0),
            HardwareType::new("WhatsminerM30S", 38.0, 88.0),
        ],
        renewable_percentage: 15.0,
        has_recs: false,
        has_offsets: true,
    };

    // Calculate and print emissions
    println!("Miner Emissions Calculations");
    println!("============================\n");

    let (us_loc, us_market, us_net) = calculator.calculate_miner_emissions(&us_miner);
    println!("US Green Miner (75% renewable, with RECs):");
    println!("  Location-based: {:.2} tonnes CO2e/year", us_loc);
    println!("  Market-based:   {:.2} tonnes CO2e/year", us_market);
    println!("  Net emissions:  {:.2} tonnes CO2e/year", us_net);
    println!();

    let (eu_loc, eu_market, eu_net) = calculator.calculate_miner_emissions(&eu_miner);
    println!("EU Miner (100% renewable, with RECs):");
    println!("  Location-based: {:.2} tonnes CO2e/year", eu_loc);
    println!("  Market-based:   {:.2} tonnes CO2e/year", eu_market);
    println!("  Net emissions:  {:.2} tonnes CO2e/year", eu_net);
    println!();

    let (asia_loc, asia_market, asia_net) = calculator.calculate_miner_emissions(&asia_miner);
    println!("Asia Miner (15% renewable, with offsets):");
    println!("  Location-based: {:.2} tonnes CO2e/year", asia_loc);
    println!("  Market-based:   {:.2} tonnes CO2e/year", asia_market);
    println!("  Net emissions:  {:.2} tonnes CO2e/year", asia_net);
    println!();

    // Calculate the effectiveness of RECs vs offsets
    println!("Effectiveness of REC vs Offset Strategies");
    println!("=======================================");
    println!(
        "REC reduction (US miner):   {:.1}%",
        100.0 * (1.0 - us_market / us_loc)
    );
    println!(
        "REC reduction (EU miner):   {:.1}%",
        100.0 * (1.0 - eu_market / eu_loc.max(0.001))
    );
    println!(
        "Offset reduction (Asia):    {:.1}%",
        100.0 * (1.0 - asia_net / asia_market)
    );

    // Show annual energy consumption
    println!("\nAnnual Energy Consumption");
    println!("========================");

    let us_energy = us_miner
        .hardware
        .iter()
        .map(|hw| hw.daily_energy_consumption_kwh())
        .sum::<f64>()
        * 365.0
        / 1000.0;
    let eu_energy = eu_miner
        .hardware
        .iter()
        .map(|hw| hw.daily_energy_consumption_kwh())
        .sum::<f64>()
        * 365.0
        / 1000.0;
    let asia_energy = asia_miner
        .hardware
        .iter()
        .map(|hw| hw.daily_energy_consumption_kwh())
        .sum::<f64>()
        * 365.0
        / 1000.0;

    println!("US Miner:   {:.2} MWh/year", us_energy);
    println!("EU Miner:   {:.2} MWh/year", eu_energy);
    println!("Asia Miner: {:.2} MWh/year", asia_energy);
}
