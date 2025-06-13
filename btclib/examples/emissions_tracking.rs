use std::collections::HashMap;
use std::error::Error;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use btclib::environmental::emissions::{
    EmissionFactor, EmissionsError, EmissionsTracker, HardwareConfig, HardwareType,
    NetworkEmissionsConfig, Region, RenewableEnergySource, RenewablePercentage, VerificationStatus,
};

// Main function to run all examples
fn main() -> Result<(), Box<dyn Error>> {
    // Initialize logger for debugging
    env_logger::init();
    println!("supernova Emissions Tracking Examples");
    println!("====================================");

    // Run different examples showcasing various features
    simple_tracker_example()?;
    println!("\n");

    hardware_portfolio_example()?;
    println!("\n");

    network_emissions_example()?;
    println!("\n");

    renewable_energy_example()?;
    println!("\n");

    verification_example()?;

    Ok(())
}

/// Simple example demonstrating basic emissions tracking for a single hardware type
fn simple_tracker_example() -> Result<(), EmissionsError> {
    println!("EXAMPLE 1: Basic Emissions Tracking");
    println!("----------------------------------");

    // Create a simple emissions tracker for US East region
    let mut tracker = EmissionsTracker::new(Region::UsEast, None)?;

    // Calculate emissions for a known hardware type running for 24 hours
    let hardware = HardwareConfig::new(
        HardwareType::AntminerS19,
        110.0, // TH/s
        3250.0, // Watts
    );

    // Calculate emissions for 24 hours of operation
    let hours = 24.0;
    let energy_kwh = hardware.power_consumption * hours / 1000.0;
    let emissions = tracker.calculate_emissions(energy_kwh)?;

    println!("Hardware: {:?}", hardware.hardware_type);
    println!("Region: {:?}", tracker.region());
    println!("Emission Factor: {:.4} kgCO2e/kWh", tracker.emission_factor().value);
    println!("Power Consumption: {:.2} W", hardware.power_consumption);
    println!("Energy Used (24h): {:.2} kWh", energy_kwh);
    println!("Total Emissions: {:.2} kgCO2e", emissions);
    println!("Hash Rate: {:.2} TH/s", hardware.hash_rate);
    println!("Emissions Intensity: {:.6} kgCO2e/TH", emissions / (hardware.hash_rate * hours));

    Ok(())
}

/// Example showing how to track emissions for multiple hardware types in a mining operation
fn hardware_portfolio_example() -> Result<(), EmissionsError> {
    println!("EXAMPLE 2: Hardware Portfolio Emissions");
    println!("-------------------------------------");

    // Create a tracker for the Europe region
    let mut tracker = EmissionsTracker::new(Region::Europe, None)?;
    
    // Define multiple hardware types in our mining operation
    let hardware_portfolio = vec![
        // Older generation miners
        (
            HardwareConfig::new(
                HardwareType::AntminerS9,
                14.0,  // TH/s
                1323.0, // Watts
            ),
            10, // Quantity
        ),
        // Current generation miners
        (
            HardwareConfig::new(
                HardwareType::AntminerS19,
                110.0, // TH/s
                3250.0, // Watts
            ),
            5, // Quantity
        ),
        // Next generation miners
        (
            HardwareConfig::new(
                HardwareType::Custom("AntminerS21".to_string()),
                200.0, // TH/s
                3500.0, // Watts
            ),
            2, // Quantity
        ),
    ];

    // Calculate total emissions for 30 days of operation
    let days = 30.0;
    let hours = days * 24.0;
    
    let mut total_energy = 0.0;
    let mut total_hashrate = 0.0;
    
    println!("Mining Farm Configuration:");
    for (hardware, quantity) in &hardware_portfolio {
        let energy_kwh = hardware.power_consumption * hours * (*quantity as f64) / 1000.0;
        total_energy += energy_kwh;
        
        let hashrate_contribution = hardware.hash_rate * (*quantity as f64);
        total_hashrate += hashrate_contribution;
        
        println!(
            "  - {:?} x{}: {:.2} TH/s, {:.2} W, {:.2} kWh over {} days", 
            hardware.hardware_type, quantity, hardware.hash_rate, 
            hardware.power_consumption, energy_kwh, days
        );
    }
    
    let total_emissions = tracker.calculate_emissions(total_energy)?;
    
    println!("\nPortfolio Summary:");
    println!("Region: {:?}", tracker.region());
    println!("Total Devices: {}", hardware_portfolio.iter().map(|(_, qty)| qty).sum::<u32>());
    println!("Total Hash Rate: {:.2} TH/s", total_hashrate);
    println!("Total Energy Used: {:.2} kWh", total_energy);
    println!("Total Emissions: {:.2} kgCO2e", total_emissions);
    println!("Emissions per TH: {:.6} kgCO2e/TH", total_emissions / (total_hashrate * hours));

    Ok(())
}

/// Example demonstrating network emissions analysis
fn network_emissions_example() -> Result<(), EmissionsError> {
    println!("EXAMPLE 3: Network Emissions Analysis");
    println!("-----------------------------------");

    // Create a network emissions config with global distribution of mining power
    let network_config = NetworkEmissionsConfig {
        // Distribution of hashrate by region
        regional_hashrate_distribution: {
            let mut distribution = HashMap::new();
            distribution.insert(Region::China, 20.0);       // 20% in China
            distribution.insert(Region::UsWest, 15.0);      // 15% in US West
            distribution.insert(Region::UsEast, 10.0);      // 10% in US East
            distribution.insert(Region::Europe, 15.0);      // 15% in Europe
            distribution.insert(Region::NorthernEurope, 5.0); // 5% in Northern Europe
            distribution.insert(Region::Canada, 10.0);      // 10% in Canada
            distribution.insert(Region::Russia, 10.0);      // 10% in Russia
            distribution.insert(Region::Kazakhstan, 5.0);   // 5% in Kazakhstan
            distribution.insert(Region::Other, 10.0);       // 10% in other regions
            distribution
        },
        // Average hardware efficiency across network (J/TH)
        average_network_efficiency: 30.0,
        // Network hashrate in TH/s
        total_network_hashrate: 400_000_000.0, // 400 EH/s
        // Average renewable percentage across regions (if specific data not available)
        default_renewable_percentage: Some(30.0),
    };

    // Create a shared tracker for network analysis
    let tracker = Arc::new(Mutex::new(
        EmissionsTracker::new_with_network_config(network_config)?
    ));
    
    // Calculate daily network emissions
    let hours = 24.0;
    let tracker_clone = Arc::clone(&tracker);
    let daily_emissions = tracker_clone.lock().unwrap().calculate_network_emissions(hours)?;
    
    // Get regional breakdown
    let regional_breakdown = tracker.lock().unwrap().calculate_regional_emissions(hours)?;
    
    println!("Network Configuration:");
    println!("Total Network Hashrate: {:.2} EH/s", network_config.total_network_hashrate / 1_000_000.0);
    println!("Average Efficiency: {:.2} J/TH", network_config.average_network_efficiency);
    
    println!("\nDaily Network Emissions:");
    println!("Total Daily Emissions: {:.2} tonnes CO2e", daily_emissions / 1000.0);
    println!("Annual Network Emissions: {:.2} million tonnes CO2e", (daily_emissions * 365.0) / 1_000_000.0);
    
    println!("\nRegional Emissions Breakdown:");
    for (region, emissions) in regional_breakdown {
        println!(
            "  - {:?}: {:.2}% of hashrate, {:.2} tonnes CO2e/day", 
            region, 
            network_config.regional_hashrate_distribution.get(&region).unwrap_or(&0.0),
            emissions / 1000.0
        );
    }

    Ok(())
}

/// Example demonstrating renewable energy integration
fn renewable_energy_example() -> Result<(), EmissionsError> {
    println!("EXAMPLE 4: Renewable Energy Integration");
    println!("-------------------------------------");

    // Create a tracker with renewable energy sources
    let mut tracker = EmissionsTracker::new(Region::UsWest, None)?;
    
    // Add renewable energy sources to our operation
    let renewable_sources = vec![
        (
            RenewableEnergySource::Solar,
            RenewablePercentage::new(30.0, VerificationStatus::Verified),
        ),
        (
            RenewableEnergySource::Hydro,
            RenewablePercentage::new(20.0, VerificationStatus::Verified),
        ),
        (
            RenewableEnergySource::Other("Stranded Natural Gas".to_string()),
            RenewablePercentage::new(15.0, VerificationStatus::Pending),
        ),
    ];
    
    // Add renewable sources to tracker
    for (source, percentage) in &renewable_sources {
        tracker.add_renewable_source(*source, percentage.clone())?;
    }

    // Calculate emissions for a specified hardware
    let hardware = HardwareConfig::new(
        HardwareType::AntminerS19XP,
        140.0, // TH/s
        3010.0, // Watts
    );
    
    // Calculate for a month of operation
    let days = 30.0;
    let hours = days * 24.0;
    let quantity = 100; // 100 miners
    
    let energy_kwh = hardware.power_consumption * hours * (quantity as f64) / 1000.0;
    
    // Calculate both gross and net emissions
    let gross_emissions = tracker.calculate_emissions_without_renewables(energy_kwh)?;
    let net_emissions = tracker.calculate_emissions(energy_kwh)?;
    
    println!("Mining Operation with Renewables:");
    println!("Hardware: {:?} x{}", hardware.hardware_type, quantity);
    println!("Region: {:?}", tracker.region());
    println!("Base Emission Factor: {:.4} kgCO2e/kWh", tracker.emission_factor().value);
    
    println!("\nRenewable Energy Sources:");
    let total_renewable = tracker.total_renewable_percentage();
    for (source, percentage) in &renewable_sources {
        println!(
            "  - {:?}: {:.1}% ({:?})", 
            source, percentage.value, percentage.verification_status
        );
    }
    
    println!("\nEmissions Impact:");
    println!("Total Energy Used: {:.2} MWh", energy_kwh / 1000.0);
    println!("Gross Emissions (without renewables): {:.2} tonnes CO2e", gross_emissions / 1000.0);
    println!("Net Emissions (with renewables): {:.2} tonnes CO2e", net_emissions / 1000.0);
    println!("Emissions Reduction: {:.2} tonnes CO2e ({:.1}%)", 
        (gross_emissions - net_emissions) / 1000.0,
        ((gross_emissions - net_emissions) / gross_emissions) * 100.0
    );
    println!("Effective Emission Factor: {:.4} kgCO2e/kWh", 
        tracker.effective_emission_factor().value
    );

    Ok(())
}

/// Example demonstrating verification status impacts
fn verification_example() -> Result<(), EmissionsError> {
    println!("EXAMPLE 5: Verification Status Impact");
    println!("-----------------------------------");

    // Create a tracker for comparison
    let mut unverified_tracker = EmissionsTracker::new(Region::UsWest, None)?;
    let mut verified_tracker = EmissionsTracker::new(Region::UsWest, None)?;
    
    // Add same renewable percentages but with different verification statuses
    unverified_tracker.add_renewable_source(
        RenewableEnergySource::Solar,
        RenewablePercentage::new(40.0, VerificationStatus::Claimed),
    )?;
    unverified_tracker.add_renewable_source(
        RenewableEnergySource::Wind,
        RenewablePercentage::new(20.0, VerificationStatus::Pending),
    )?;
    
    verified_tracker.add_renewable_source(
        RenewableEnergySource::Solar,
        RenewablePercentage::new(40.0, VerificationStatus::Verified),
    )?;
    verified_tracker.add_renewable_source(
        RenewableEnergySource::Wind,
        RenewablePercentage::new(20.0, VerificationStatus::Verified),
    )?;
    
    // Hardware and energy for comparison
    let hardware = HardwareConfig::new(
        HardwareType::AntminerS19Pro,
        110.0, // TH/s
        3250.0, // Watts
    );
    
    // Calculate for a month of operation
    let days = 30.0;
    let hours = days * 24.0;
    let quantity = 50; // 50 miners
    
    let energy_kwh = hardware.power_consumption * hours * (quantity as f64) / 1000.0;
    
    // Calculate emissions under both trackers
    let unverified_emissions = unverified_tracker.calculate_emissions(energy_kwh)?;
    let verified_emissions = verified_tracker.calculate_emissions(energy_kwh)?;
    
    println!("Impact of Verification Status on Emissions Accounting");
    println!("Renewable Configuration: 40% Solar, 20% Wind");
    println!("Energy Consumption: {:.2} MWh", energy_kwh / 1000.0);
    
    println!("\nEmissions with Unverified Renewables:");
    println!("Effective Renewable Percentage: {:.1}%", unverified_tracker.effective_renewable_percentage());
    println!("Effective Emission Factor: {:.4} kgCO2e/kWh", unverified_tracker.effective_emission_factor().value);
    println!("Total Emissions: {:.2} tonnes CO2e", unverified_emissions / 1000.0);
    
    println!("\nEmissions with Verified Renewables:");
    println!("Effective Renewable Percentage: {:.1}%", verified_tracker.effective_renewable_percentage());
    println!("Effective Emission Factor: {:.4} kgCO2e/kWh", verified_tracker.effective_emission_factor().value);
    println!("Total Emissions: {:.2} tonnes CO2e", verified_emissions / 1000.0);
    
    println!("\nVerification Impact:");
    println!("Emissions Difference: {:.2} tonnes CO2e", (unverified_emissions - verified_emissions) / 1000.0);
    println!("Percentage Benefit from Verification: {:.1}%", 
        ((unverified_emissions - verified_emissions) / unverified_emissions) * 100.0
    );
    
    // Treasury benefit simulation
    let daily_reward = 6.25; // BTC
    let btc_price = 50000.0; // USD
    let environmental_incentive_rate = 0.05; // 5% incentive for verified renewable use
    
    let daily_value = daily_reward * btc_price;
    let monthly_reward = daily_reward * 30.0;
    let monthly_value = daily_value * 30.0;
    
    let unverified_incentive = 0.0; // No incentive for unverified
    let verified_incentive = monthly_value * environmental_incentive_rate;
    
    println!("\nTreasury Incentive Impact (simulated):");
    println!("Monthly Mining Reward: {:.2} BTC (${:.2})", monthly_reward, monthly_value);
    println!("Environmental Incentive Rate: {:.1}%", environmental_incentive_rate * 100.0);
    println!("Unverified Renewables Incentive: ${:.2}", unverified_incentive);
    println!("Verified Renewables Incentive: ${:.2}", verified_incentive);
    println!("Financial Benefit of Verification: ${:.2}", verified_incentive - unverified_incentive);

    Ok(())
} 