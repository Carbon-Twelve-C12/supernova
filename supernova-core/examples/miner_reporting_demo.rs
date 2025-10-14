use btclib::environmental::{
    EmissionFactor, EnergySource, HardwareType, MinerEnvironmentalInfo, MinerReportingManager,
    Region, VerificationStatus,
};
use std::collections::HashMap;

fn main() {
    println!("supernova Miner Environmental Reporting Demo");
    println!("============================================\n");

    // Create a miner reporting manager
    let mut manager = MinerReportingManager::new();

    // Set up emission factors
    let emission_factors = setup_emission_factors();
    manager.set_emission_factors(emission_factors);

    // Set up hardware baselines
    let hardware_baselines = setup_hardware_baselines();
    manager.set_hardware_baselines(hardware_baselines);

    // Register some miners with different environmental profiles
    register_demo_miners(&mut manager);

    // Display registered miners
    println!("\nRegistered Miners:");
    for miner in manager.list_miners() {
        println!(
            "- {}: {} ({}% renewable)",
            miner.miner_id, miner.name, miner.renewable_percentage
        );

        if let Some(carbon) = miner.carbon_footprint_tonnes_year {
            println!("  Carbon footprint: {:.1} tonnes CO2e/year", carbon);
        }
    }

    // Calculate carbon footprints
    println!("\nCalculating carbon footprints...");
    let results = manager.calculate_carbon_footprints();

    for (id, result) in results {
        match result {
            Ok(footprint) => {
                println!("- {}: {:.1} tonnes CO2e/year", id, footprint);
            }
            Err(e) => {
                println!("- {}: Error calculating footprint: {}", id, e);
            }
        }
    }

    // Identify green miners
    println!("\nVerified Green Miners (with RECs):");
    let green_miners = manager.get_verified_green_miners();

    if green_miners.is_empty() {
        println!("No verified green miners found.");
    } else {
        for miner in green_miners {
            println!(
                "- {}: {}% renewable energy",
                miner.miner_id, miner.renewable_percentage
            );
        }
    }

    // Identify offset miners
    println!("\nMiners with Carbon Offsets:");
    let offset_miners = manager.get_offset_miners();

    if offset_miners.is_empty() {
        println!("No miners with carbon offsets found.");
    } else {
        for miner in offset_miners {
            println!("- {}: Has verified carbon offsets", miner.miner_id);
        }
    }

    // Calculate efficiency metrics
    println!("\nComparing miners to hardware baselines:");

    for miner in manager.list_miners() {
        let efficiency = match miner.calculate_energy_efficiency() {
            Some(eff) => format!("{:.1} J/TH", eff),
            None => "Unknown".to_string(),
        };

        let baseline_ratio = match manager.compare_to_baseline(&miner.miner_id) {
            Ok(ratio) => {
                let performance = if ratio <= 1.1 {
                    "Optimal"
                } else if ratio <= 1.5 {
                    "Good"
                } else if ratio <= 2.0 {
                    "Average"
                } else {
                    "Inefficient"
                };

                format!("{:.2}x ({}) vs. baseline", ratio, performance)
            }
            Err(_) => "Unknown".to_string(),
        };

        println!(
            "- {}: Efficiency {} - {}",
            miner.miner_id, efficiency, baseline_ratio
        );
    }

    // Generate and print report
    println!("\nEnvironmental Report Summary:");
    let report = manager.generate_report();

    println!("- Total miners: {}", report.total_miners);
    println!("- Verified miners: {}", report.verified_miners);
    println!(
        "- Average renewable energy: {:.1}%",
        report.average_renewable_percentage
    );
    println!("- Total hashrate: {:.1} TH/s", report.total_hashrate);
    println!(
        "- Total energy consumption: {:.1} kWh/day",
        report.total_energy_consumption_kwh_day
    );

    if let Some(efficiency) = report.average_efficiency {
        println!("- Average energy efficiency: {:.1} J/TH", efficiency);
    }

    println!("\nDemo completed!");
}

fn setup_emission_factors() -> HashMap<Region, EmissionFactor> {
    let mut factors = HashMap::new();

    factors.insert(
        Region::NorthAmerica,
        EmissionFactor {
            grid_emissions_factor: 0.38,
            region_name: "North America".to_string(),
        },
    );

    factors.insert(
        Region::Europe,
        EmissionFactor {
            grid_emissions_factor: 0.28,
            region_name: "Europe".to_string(),
        },
    );

    factors.insert(
        Region::EastAsia,
        EmissionFactor {
            grid_emissions_factor: 0.63,
            region_name: "East Asia".to_string(),
        },
    );

    factors.insert(
        Region::Global,
        EmissionFactor {
            grid_emissions_factor: 0.475,
            region_name: "Global Average".to_string(),
        },
    );

    factors
}

fn setup_hardware_baselines() -> HashMap<HardwareType, f64> {
    let mut baselines = HashMap::new();

    // Use the typical efficiencies for the baselines
    baselines.insert(HardwareType::AntminerS9, 98.0);
    baselines.insert(HardwareType::AntminerS19, 34.5);
    baselines.insert(HardwareType::AntminerS19Pro, 29.5);
    baselines.insert(HardwareType::AntminerS19XP, 21.5);
    baselines.insert(HardwareType::WhatsminerM30S, 38.0);
    baselines.insert(HardwareType::WhatsminerM30SPlus, 34.0);
    baselines.insert(HardwareType::AvalonMiner1246, 38.0);
    baselines.insert(HardwareType::CustomASIC, 30.0);
    baselines.insert(HardwareType::FPGA, 120.0);
    baselines.insert(HardwareType::GPU, 200.0);

    baselines
}

fn register_demo_miners(manager: &mut MinerReportingManager) {
    // Green Miner with 100% renewables and RECs
    let mut green_miner = MinerEnvironmentalInfo::new(
        "miner1".to_string(),
        "Green Mining Co.".to_string(),
        Region::NorthAmerica,
    );

    // Set hardware type
    green_miner.add_hardware_types(vec![HardwareType::AntminerS19Pro]);

    // Set 100% renewable energy mix
    let mut sources = HashMap::new();
    sources.insert(EnergySource::Solar, 40.0);
    sources.insert(EnergySource::Wind, 40.0);
    sources.insert(EnergySource::Hydro, 20.0);
    green_miner.update_energy_sources(sources).unwrap();

    // Set performance metrics
    green_miner
        .update_performance_metrics(1000.0, 25000.0)
        .unwrap(); // 1,000 TH/s, 25,000 kWh/day

    // Add verification
    green_miner.add_verification(
        "GreenCert Inc.".to_string(),
        "CERT-12345".to_string(),
        VerificationStatus::Verified,
    );

    // Add REC certificates
    green_miner.update_rec_status(
        true,
        Some("https://greencerts.example.com/12345".to_string()),
    );

    // Register with manager
    manager.register_miner(green_miner).unwrap();

    // Mixed energy miner with carbon offsets
    let mut mixed_miner = MinerEnvironmentalInfo::new(
        "miner2".to_string(),
        "Mixed Energy Mining".to_string(),
        Region::Europe,
    );

    // Set hardware types
    mixed_miner.add_hardware_types(vec![
        HardwareType::AntminerS19,
        HardwareType::WhatsminerM30SPlus,
    ]);

    // Set mixed energy sources (50% renewable)
    let mut sources = HashMap::new();
    sources.insert(EnergySource::Hydro, 30.0);
    sources.insert(EnergySource::Wind, 20.0);
    sources.insert(EnergySource::NaturalGas, 50.0);
    mixed_miner.update_energy_sources(sources).unwrap();

    // Set performance metrics
    mixed_miner
        .update_performance_metrics(500.0, 15000.0)
        .unwrap(); // 500 TH/s, 15,000 kWh/day

    // Add verification
    mixed_miner.add_verification(
        "EuroVerify".to_string(),
        "EV-67890".to_string(),
        VerificationStatus::Verified,
    );

    // Add carbon offsets
    mixed_miner.update_offset_status(
        true,
        Some("https://carbonoffsets.example.com/67890".to_string()),
    );

    // Register with manager
    manager.register_miner(mixed_miner).unwrap();

    // Unverified fossil fuel miner
    let mut fossil_miner = MinerEnvironmentalInfo::new(
        "miner3".to_string(),
        "Traditional Mining LLC".to_string(),
        Region::EastAsia,
    );

    // Set hardware type (older equipment)
    fossil_miner.add_hardware_types(vec![
        HardwareType::AntminerS9,
        HardwareType::AvalonMiner1066,
    ]);

    // Set predominantly fossil fuel mix
    let mut sources = HashMap::new();
    sources.insert(EnergySource::Coal, 70.0);
    sources.insert(EnergySource::NaturalGas, 20.0);
    sources.insert(EnergySource::Hydro, 10.0);
    fossil_miner.update_energy_sources(sources).unwrap();

    // Set performance metrics
    fossil_miner
        .update_performance_metrics(200.0, 18000.0)
        .unwrap(); // 200 TH/s, 18,000 kWh/day (inefficient)

    // No verification or environmental certificates

    // Register with manager
    manager.register_miner(fossil_miner).unwrap();

    // Pending verification miner
    let mut pending_miner = MinerEnvironmentalInfo::new(
        "miner4".to_string(),
        "New Green Mining".to_string(),
        Region::Global,
    );

    // Set hardware type
    pending_miner.add_hardware_types(vec![HardwareType::AntminerS19XP]);

    // Set energy mix (80% renewable)
    let mut sources = HashMap::new();
    sources.insert(EnergySource::Solar, 50.0);
    sources.insert(EnergySource::Wind, 30.0);
    sources.insert(EnergySource::Grid, 20.0);
    pending_miner.update_energy_sources(sources).unwrap();

    // Set performance metrics
    pending_miner
        .update_performance_metrics(300.0, 5500.0)
        .unwrap(); // 300 TH/s, 5,500 kWh/day (efficient)

    // Add pending verification
    pending_miner.add_verification(
        "GlobalCert".to_string(),
        "GC-45678".to_string(),
        VerificationStatus::Pending,
    );

    // Register with manager
    manager.register_miner(pending_miner).unwrap();
}
