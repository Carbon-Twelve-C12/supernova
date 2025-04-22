// Emissions calculator example using SuperNova's environmental features
// This example demonstrates how to calculate emissions for different miners

use chrono::Utc;
use std::collections::HashMap;
use btclib::environmental::types::{
    Region, EmissionFactor, HardwareType, EnergySource, EmissionsDataSource, EmissionsFactorType
};
use btclib::environmental::emissions::EmissionsTracker;
use btclib::environmental::miner_reporting::{
    MinerEnvironmentalInfo, VerificationStatus, RECCertificateInfo, CarbonOffsetInfo
};
use std::fmt;
use rand::{Rng, thread_rng};
use chrono::{DateTime, Duration};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
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

impl fmt::Display for Region {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(sub_region) = &self.sub_region {
            write!(f, "{}-{}", self.country_code, sub_region)
        } else {
            write!(f, "{}", self.country_code)
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct EmissionFactor {
    g_co2e_per_kwh: f64,
    year: u16,
}

impl EmissionFactor {
    fn new(g_co2e_per_kwh: f64, year: u16) -> Self {
        Self {
            g_co2e_per_kwh,
            year,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum HardwareType {
    AntminerS9,
    AntminerS19,
    AntminerS19Pro,
    AntminerS19XP,
    WhatsminerM30S,
    WhatsminerM30SPlus,
    AvalonMiner1246,
    Other,
}

impl HardwareType {
    fn typical_efficiency(&self) -> f64 {
        match self {
            HardwareType::AntminerS9 => 98.0,
            HardwareType::AntminerS19 => 34.5,
            HardwareType::AntminerS19Pro => 29.5,
            HardwareType::AntminerS19XP => 21.5,
            HardwareType::WhatsminerM30S => 38.0,
            HardwareType::WhatsminerM30SPlus => 34.0,
            HardwareType::AvalonMiner1246 => 38.0,
            HardwareType::Other => 60.0,
        }
    }
    
    fn typical_hashrate(&self) -> f64 {
        match self {
            HardwareType::AntminerS9 => 14.0,
            HardwareType::AntminerS19 => 95.0,
            HardwareType::AntminerS19Pro => 110.0,
            HardwareType::AntminerS19XP => 140.0,
            HardwareType::WhatsminerM30S => 88.0,
            HardwareType::WhatsminerM30SPlus => 100.0,
            HardwareType::AvalonMiner1246 => 90.0,
            HardwareType::Other => 50.0,
        }
    }
    
    fn daily_energy_consumption(&self) -> f64 {
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
            HardwareType::AntminerS19XP => write!(f, "Antminer S19 XP"),
            HardwareType::WhatsminerM30S => write!(f, "Whatsminer M30S"),
            HardwareType::WhatsminerM30SPlus => write!(f, "Whatsminer M30S+"),
            HardwareType::AvalonMiner1246 => write!(f, "AvalonMiner 1246"),
            HardwareType::Other => write!(f, "Other"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum VerificationStatus {
    None,
    Pending,
    Verified,
    Failed,
    Expired,
}

impl fmt::Display for VerificationStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            VerificationStatus::None => write!(f, "Not Verified"),
            VerificationStatus::Pending => write!(f, "Verification Pending"),
            VerificationStatus::Verified => write!(f, "Verified"),
            VerificationStatus::Failed => write!(f, "Verification Failed"),
            VerificationStatus::Expired => write!(f, "Verification Expired"),
        }
    }
}

#[derive(Debug, Clone)]
struct RECCertificateInfo {
    certificate_id: String,
    issuer: String,
    amount_mwh: f64,
    generation_start: DateTime<Utc>,
    generation_end: DateTime<Utc>,
    verification_status: VerificationStatus,
}

#[derive(Debug, Clone)]
struct CarbonOffsetInfo {
    offset_id: String,
    issuer: String,
    amount_tonnes: f64,
    project_type: String,
    verification_status: VerificationStatus,
}

#[derive(Debug, Clone)]
struct MinerInfo {
    id: String,
    hardware_type: HardwareType,
    quantity: u32,
    region: Region,
    renewable_percentage: f64,
    rec_certificates: Option<RECCertificateInfo>,
    carbon_offsets: Option<CarbonOffsetInfo>,
}

struct EmissionsCalculator {
    region_factors: HashMap<Region, EmissionFactor>,
    treasury_contribution_rate: f64, // Percentage of emissions cost contributed to treasury
    carbon_price_per_tonne: f64,     // Price in USD per tonne CO2e
}

impl EmissionsCalculator {
    fn new() -> Self {
        let mut calculator = Self {
            region_factors: HashMap::new(),
            treasury_contribution_rate: 0.05, // 5% contribution
            carbon_price_per_tonne: 25.0,      // $25 per tonne
        };
        
        // Load default emission factors
        calculator.load_default_emission_factors();
        
        calculator
    }
    
    fn load_default_emission_factors(&mut self) {
        // Add emission factors for common regions (gCO2e/kWh)
        self.region_factors.insert(Region::new("US"), EmissionFactor::new(380.0, 2023));
        self.region_factors.insert(Region::new("CA"), EmissionFactor::new(120.0, 2023));
        self.region_factors.insert(Region::new("CN"), EmissionFactor::new(630.0, 2023));
        self.region_factors.insert(Region::new("EU"), EmissionFactor::new(280.0, 2023));
        self.region_factors.insert(Region::new("IN"), EmissionFactor::new(720.0, 2023));
        self.region_factors.insert(Region::new("RU"), EmissionFactor::new(500.0, 2023));
        self.region_factors.insert(Region::new("AU"), EmissionFactor::new(520.0, 2023));
        self.region_factors.insert(Region::new("BR"), EmissionFactor::new(90.0, 2023));
        self.region_factors.insert(Region::new("ZA"), EmissionFactor::new(850.0, 2023));
        
        // Add some sub-regions with more specific factors
        self.region_factors.insert(Region::with_sub_region("US", "CA"), EmissionFactor::new(210.0, 2023));
        self.region_factors.insert(Region::with_sub_region("US", "WA"), EmissionFactor::new(90.0, 2023));
        self.region_factors.insert(Region::with_sub_region("US", "TX"), EmissionFactor::new(410.0, 2023));
        self.region_factors.insert(Region::with_sub_region("US", "WY"), EmissionFactor::new(790.0, 2023));
    }
    
    fn get_emission_factor(&self, region: &Region) -> f64 {
        // Try to find exact region match
        if let Some(factor) = self.region_factors.get(region) {
            return factor.g_co2e_per_kwh;
        }
        
        // Try country code match if subregion was specified
        if region.sub_region.is_some() {
            let country_region = Region::new(&region.country_code);
            if let Some(factor) = self.region_factors.get(&country_region) {
                return factor.g_co2e_per_kwh;
            }
        }
        
        // Default global average
        450.0 // gCO2e/kWh global average
    }
    
    fn calculate_daily_energy_consumption(&self, miner: &MinerInfo) -> f64 {
        let hw_daily_consumption = miner.hardware_type.daily_energy_consumption();
        hw_daily_consumption * miner.quantity as f64
    }
    
    fn calculate_gross_emissions(&self, miner: &MinerInfo, days: u32) -> f64 {
        let daily_energy = self.calculate_daily_energy_consumption(miner);
        let emission_factor = self.get_emission_factor(&miner.region);
        
        // Calculate total emissions without considering renewables
        // Convert from g to kg to tonnes
        let total_energy_kwh = daily_energy * days as f64;
        (total_energy_kwh * emission_factor) / 1_000_000.0
    }
    
    fn calculate_net_emissions(&self, miner: &MinerInfo, days: u32) -> f64 {
        let gross_emissions = self.calculate_gross_emissions(miner, days);
        
        // Apply renewable energy percentage
        let emissions_after_renewables = gross_emissions * (1.0 - (miner.renewable_percentage / 100.0));
        
        // Apply RECs if verified
        let emissions_after_recs = if let Some(rec) = &miner.rec_certificates {
            if rec.verification_status == VerificationStatus::Verified {
                // Convert MWh to kWh and calculate how much this covers
                let rec_coverage_kwh = rec.amount_mwh * 1000.0;
                let total_energy_kwh = self.calculate_daily_energy_consumption(miner) * days as f64;
                
                // Calculate percentage of energy covered by RECs
                let rec_coverage_percentage = (rec_coverage_kwh / total_energy_kwh).min(1.0);
                
                // Reduce emissions based on REC coverage
                emissions_after_renewables * (1.0 - rec_coverage_percentage)
            } else {
                emissions_after_renewables
            }
        } else {
            emissions_after_renewables
        };
        
        // Apply carbon offsets if verified
        let net_emissions = if let Some(offset) = &miner.carbon_offsets {
            if offset.verification_status == VerificationStatus::Verified {
                (emissions_after_recs - offset.amount_tonnes).max(0.0)
            } else {
                emissions_after_recs
            }
        } else {
            emissions_after_recs
        };
        
        net_emissions
    }
    
    fn calculate_impact_score(&self, miner: &MinerInfo, days: u32) -> f64 {
        // Calculate ratio of net to gross emissions (lower is better)
        let gross_emissions = self.calculate_gross_emissions(miner, days);
        let net_emissions = self.calculate_net_emissions(miner, days);
        
        if gross_emissions <= 0.0 {
            return 100.0; // Perfect score for zero emissions
        }
        
        // Score from 0-100, where 100 is zero net emissions
        let reduction_percentage = ((gross_emissions - net_emissions) / gross_emissions) * 100.0;
        reduction_percentage.min(100.0)
    }
    
    fn calculate_treasury_contribution(&self, miner: &MinerInfo, days: u32) -> f64 {
        let net_emissions = self.calculate_net_emissions(miner, days);
        
        // Calculate carbon cost
        let carbon_cost = net_emissions * self.carbon_price_per_tonne;
        
        // Apply treasury contribution rate
        carbon_cost * self.treasury_contribution_rate
    }
    
    fn get_verification_recommendation(&self, miner: &MinerInfo) -> String {
        let mut recommendations = Vec::new();
        
        // Check renewable percentage
        if miner.renewable_percentage > 0.0 && 
           (miner.rec_certificates.is_none() || 
            miner.rec_certificates.as_ref().unwrap().verification_status != VerificationStatus::Verified) {
            recommendations.push("Verify renewable energy claims with RECs");
        }
        
        // Check if RECs need verification
        if let Some(rec) = &miner.rec_certificates {
            if rec.verification_status == VerificationStatus::None || 
               rec.verification_status == VerificationStatus::Pending {
                recommendations.push("Complete REC verification process");
            } else if rec.verification_status == VerificationStatus::Expired {
                recommendations.push("Renew expired REC verification");
            }
        }
        
        // Check if carbon offsets need verification
        if let Some(offset) = &miner.carbon_offsets {
            if offset.verification_status == VerificationStatus::None || 
               offset.verification_status == VerificationStatus::Pending {
                recommendations.push("Complete carbon offset verification");
            } else if offset.verification_status == VerificationStatus::Expired {
                recommendations.push("Renew expired carbon offset verification");
            }
        }
        
        // Recommend offsets if emissions are high
        let daily_energy = self.calculate_daily_energy_consumption(miner);
        let emission_factor = self.get_emission_factor(&miner.region);
        let daily_emissions_kg = (daily_energy * emission_factor) / 1000.0;
        
        if daily_emissions_kg > 100.0 && miner.carbon_offsets.is_none() {
            recommendations.push("Consider purchasing carbon offsets");
        }
        
        // Recommend more efficient hardware if using older models
        if miner.hardware_type == HardwareType::AntminerS9 {
            recommendations.push("Consider upgrading to more efficient hardware");
        }
        
        if recommendations.is_empty() {
            "No recommendations at this time".to_string()
        } else {
            recommendations.join("; ")
        }
    }
}

// Simulation of network-wide emissions
struct NetworkEmissionsSimulator {
    miners: Vec<MinerInfo>,
    calculator: EmissionsCalculator,
    network_hashrate: f64, // TH/s
}

impl NetworkEmissionsSimulator {
    fn new(calculator: EmissionsCalculator) -> Self {
        Self {
            miners: Vec::new(),
            calculator,
            network_hashrate: 350_000_000.0, // 350 EH/s converted to TH/s
        }
    }
    
    fn add_miner(&mut self, miner: MinerInfo) {
        self.miners.push(miner);
    }
    
    fn get_covered_hashrate(&self) -> f64 {
        self.miners.iter()
            .map(|miner| miner.hardware_type.typical_hashrate() * miner.quantity as f64)
            .sum()
    }
    
    fn get_covered_percentage(&self) -> f64 {
        (self.get_covered_hashrate() / self.network_hashrate) * 100.0
    }
    
    fn estimate_network_emissions(&self, days: u32) -> f64 {
        let covered_hashrate = self.get_covered_hashrate();
        let covered_emissions: f64 = self.miners.iter()
            .map(|miner| self.calculator.calculate_net_emissions(miner, days))
            .sum();
            
        if covered_hashrate <= 0.0 {
            return 0.0;
        }
        
        // Extrapolate to full network
        covered_emissions * (self.network_hashrate / covered_hashrate)
    }
    
    fn estimate_network_energy(&self, days: u32) -> f64 {
        let covered_hashrate = self.get_covered_hashrate();
        let covered_energy: f64 = self.miners.iter()
            .map(|miner| self.calculator.calculate_daily_energy_consumption(miner) * days as f64)
            .sum();
            
        if covered_hashrate <= 0.0 {
            return 0.0;
        }
        
        // Extrapolate to full network
        covered_energy * (self.network_hashrate / covered_hashrate)
    }
    
    fn generate_random_miners(&mut self, count: usize) {
        let regions = vec![
            Region::new("US"),
            Region::new("CN"),
            Region::new("RU"),
            Region::with_sub_region("US", "WA"),
            Region::with_sub_region("US", "TX"),
            Region::new("CA"),
            Region::new("EU"),
        ];
        
        let hardware_types = vec![
            HardwareType::AntminerS19,
            HardwareType::AntminerS19Pro,
            HardwareType::AntminerS19XP,
            HardwareType::WhatsminerM30S,
            HardwareType::AntminerS9,
        ];
        
        let mut rng = thread_rng();
        let now = Utc::now();
        
        for i in 0..count {
            // Select random hardware and region
            let hardware = hardware_types[rng.gen_range(0..hardware_types.len())];
            let region = regions[rng.gen_range(0..regions.len())].clone();
            
            // Generate random renewable percentage
            let renewable_percentage = rng.gen_range(0..=100) as f64;
            
            // Maybe add RECs
            let rec_certificates = if rng.gen_bool(0.4) {
                Some(RECCertificateInfo {
                    certificate_id: format!("REC-{}-{}", i, rng.gen_range(1000..9999)),
                    issuer: "Green Energy Certification Ltd.".to_string(),
                    amount_mwh: rng.gen_range(50.0..500.0),
                    generation_start: now - Duration::days(rng.gen_range(10..365)),
                    generation_end: now + Duration::days(rng.gen_range(10..365)),
                    verification_status: if rng.gen_bool(0.7) {
                        VerificationStatus::Verified
                    } else {
                        VerificationStatus::Pending
                    },
                })
            } else {
                None
            };
            
            // Maybe add carbon offsets
            let carbon_offsets = if rng.gen_bool(0.3) {
                Some(CarbonOffsetInfo {
                    offset_id: format!("OFFSET-{}-{}", i, rng.gen_range(1000..9999)),
                    issuer: "Carbon Offset Registry".to_string(),
                    amount_tonnes: rng.gen_range(10.0..200.0),
                    project_type: "Reforestation".to_string(),
                    verification_status: if rng.gen_bool(0.8) {
                        VerificationStatus::Verified
                    } else {
                        VerificationStatus::Pending
                    },
                })
            } else {
                None
            };
            
            // Create miner with random quantity
            let miner = MinerInfo {
                id: format!("MINER-{}", i),
                hardware_type: hardware,
                quantity: rng.gen_range(10..1000),
                region,
                renewable_percentage,
                rec_certificates,
                carbon_offsets,
            };
            
            self.miners.push(miner);
        }
    }
}

fn main() {
    println!("SuperNova Emissions Calculator Example");
    println!("======================================\n");
    
    // Create the emissions calculator
    let calculator = EmissionsCalculator::new();
    
    // Create some example miners
    let miner1 = MinerInfo {
        id: "MINER-001".to_string(),
        hardware_type: HardwareType::AntminerS19Pro,
        quantity: 500,
        region: Region::with_sub_region("US", "WA"),
        renewable_percentage: 80.0,
        rec_certificates: Some(RECCertificateInfo {
            certificate_id: "REC-12345".to_string(),
            issuer: "Green-e Energy".to_string(),
            amount_mwh: 300.0,
            generation_start: Utc::now() - Duration::days(30),
            generation_end: Utc::now() + Duration::days(335),
            verification_status: VerificationStatus::Verified,
        }),
        carbon_offsets: None,
    };
    
    let miner2 = MinerInfo {
        id: "MINER-002".to_string(),
        hardware_type: HardwareType::AntminerS19,
        quantity: 1000,
        region: Region::new("CN"),
        renewable_percentage: 20.0,
        rec_certificates: None,
        carbon_offsets: Some(CarbonOffsetInfo {
            offset_id: "OFFSET-54321".to_string(),
            issuer: "Gold Standard".to_string(),
            amount_tonnes: 150.0,
            project_type: "Wind farm development".to_string(),
            verification_status: VerificationStatus::Verified,
        }),
    };
    
    let miner3 = MinerInfo {
        id: "MINER-003".to_string(),
        hardware_type: HardwareType::AntminerS9,
        quantity: 200,
        region: Region::new("US"),
        renewable_percentage: 0.0,
        rec_certificates: None,
        carbon_offsets: None,
    };
    
    // Calculate and display emissions for each miner
    let time_period_days = 30; // One month
    
    println!("Individual Miner Analysis (30-day period)");
    println!("----------------------------------------");
    
    for miner in &[&miner1, &miner2, &miner3] {
        println!("\nMiner ID: {}", miner.id);
        println!("Hardware: {} (Quantity: {})", miner.hardware_type, miner.quantity);
        println!("Location: {}", miner.region);
        println!("Renewable Energy: {}%", miner.renewable_percentage);
        
        // Calculate metrics
        let daily_energy = calculator.calculate_daily_energy_consumption(miner);
        let total_energy = daily_energy * time_period_days as f64;
        let gross_emissions = calculator.calculate_gross_emissions(miner, time_period_days);
        let net_emissions = calculator.calculate_net_emissions(miner, time_period_days);
        let impact_score = calculator.calculate_impact_score(miner, time_period_days);
        let treasury_contribution = calculator.calculate_treasury_contribution(miner, time_period_days);
        
        println!("Daily Energy: {:.2} kWh", daily_energy);
        println!("30-day Energy: {:.2} kWh", total_energy);
        println!("Gross Emissions: {:.2} tonnes CO2e", gross_emissions);
        println!("Net Emissions: {:.2} tonnes CO2e", net_emissions);
        println!("Environmental Impact Score: {:.1}/100", impact_score);
        println!("Treasury Contribution: ${:.2}", treasury_contribution);
        
        // Print REC information if available
        if let Some(rec) = &miner.rec_certificates {
            println!("REC Certificate: {} ({} MWh, Status: {})", 
                     rec.certificate_id, rec.amount_mwh, rec.verification_status);
        }
        
        // Print offset information if available
        if let Some(offset) = &miner.carbon_offsets {
            println!("Carbon Offset: {} ({} tonnes, Status: {})", 
                     offset.offset_id, offset.amount_tonnes, offset.verification_status);
        }
        
        // Print recommendations
        println!("Recommendations: {}", calculator.get_verification_recommendation(miner));
    }
    
    // Network simulation
    println!("\n\nNetwork-wide Emissions Simulation");
    println!("--------------------------------");
    
    let mut simulator = NetworkEmissionsSimulator::new(calculator);
    simulator.add_miner(miner1.clone());
    simulator.add_miner(miner2.clone());
    simulator.add_miner(miner3.clone());
    
    // Add some random miners to simulate a larger network
    simulator.generate_random_miners(50);
    
    // Calculate network metrics
    let covered_hashrate = simulator.get_covered_hashrate();
    let covered_percentage = simulator.get_covered_percentage();
    let network_emissions = simulator.estimate_network_emissions(time_period_days);
    let network_energy = simulator.estimate_network_energy(time_period_days);
    
    println!("Covered Hashrate: {:.2} TH/s ({:.4}% of network)", 
             covered_hashrate, covered_percentage);
    println!("Estimated Network Energy (30 days): {:.2} GWh", 
             network_energy / 1_000_000.0);
    println!("Estimated Network Emissions (30 days): {:.2} tonnes CO2e", 
             network_emissions);
    println!("Estimated Annual Network Emissions: {:.2} million tonnes CO2e", 
             (network_emissions * 12.0) / 1_000_000.0);
    
    // Compare REC vs Offset effectiveness
    println!("\nStrategic Analysis: REC vs Offset Effectiveness");
    println!("---------------------------------------------");
    
    // Create clones with different strategies
    let mut miner_base = miner2.clone();
    miner_base.rec_certificates = None;
    miner_base.carbon_offsets = None;
    
    let mut miner_rec = miner_base.clone();
    miner_rec.rec_certificates = Some(RECCertificateInfo {
        certificate_id: "REC-COMPARE".to_string(),
        issuer: "Green-e Energy".to_string(),
        amount_mwh: 200.0,
        generation_start: Utc::now() - Duration::days(30),
        generation_end: Utc::now() + Duration::days(335),
        verification_status: VerificationStatus::Verified,
    });
    
    let mut miner_offset = miner_base.clone();
    miner_offset.carbon_offsets = Some(CarbonOffsetInfo {
        offset_id: "OFFSET-COMPARE".to_string(),
        issuer: "Gold Standard".to_string(),
        amount_tonnes: 50.0,
        project_type: "Forestry".to_string(),
        verification_status: VerificationStatus::Verified,
    });
    
    let calc = EmissionsCalculator::new();
    let base_emissions = calc.calculate_net_emissions(&miner_base, time_period_days);
    let rec_emissions = calc.calculate_net_emissions(&miner_rec, time_period_days);
    let offset_emissions = calc.calculate_net_emissions(&miner_offset, time_period_days);
    
    println!("Base Emissions (No Strategy): {:.2} tonnes CO2e", base_emissions);
    println!("With RECs: {:.2} tonnes CO2e (Reduction: {:.2}%)", 
             rec_emissions, ((base_emissions - rec_emissions) / base_emissions) * 100.0);
    println!("With Offsets: {:.2} tonnes CO2e (Reduction: {:.2}%)", 
             offset_emissions, ((base_emissions - offset_emissions) / base_emissions) * 100.0);
    
    println!("\nConclusion: SuperNova's emissions tracking system allows miners to accurately");
    println!("quantify their environmental impact and contribute to the environmental treasury");
    println!("based on their net emissions. This creates incentives for miners to adopt renewable");
    println!("energy and carbon reduction strategies to minimize their environmental footprint.");
} 