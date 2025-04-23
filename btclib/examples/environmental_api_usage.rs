use std::sync::{Arc, Mutex};
use chrono::Utc;
use btclib::environmental::{
    EnvironmentalApi, 
    StandardEnvironmentalApi, 
    ThreadSafeEnvironmentalApi,
    EmissionsApiClient,
    MinerEmissionsData,
    NetworkEmissionsData,
    ReportingOptions,
    miner_reporting::{MinerEnvironmentalInfo, VerificationStatus},
    hardware_types::HardwareType,
    types::Region,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Environmental API Usage Example");
    println!("===============================\n");

    // Create a standard API instance
    let mut api = StandardEnvironmentalApi::new();
    
    // Register miners with different environmental profiles
    register_sample_miners(&mut api)?;
    
    // Calculate emissions for individual miners
    calculate_miner_emissions(&api)?;
    
    // Calculate network-wide emissions
    calculate_network_emissions(&api)?;
    
    // Demonstrate thread-safe API
    demonstrate_thread_safe_api()?;
    
    // Demonstrate API client
    demonstrate_api_client()?;
    
    println!("\nExample completed successfully!");
    Ok(())
}

fn register_sample_miners(api: &mut impl EnvironmentalApi) -> Result<(), Box<dyn std::error::Error>> {
    println!("Registering sample miners...");
    
    // Create a green miner (100% renewable energy)
    let green_miner = MinerEnvironmentalInfo {
        region: Region::with_details("US", Some("CA"), None, Some(37.7749), Some(-122.4194)),
        hardware_type: HardwareType::AntminerS19XP,
        units: 100,
        renewable_energy_percentage: 100.0,
        rec_percentage: 0.0,
        offset_percentage: 0.0,
        verification_status: VerificationStatus::Verified,
    };
    
    // Create a REC-backed miner (30% renewable + 70% RECs)
    let rec_miner = MinerEnvironmentalInfo {
        region: Region::with_details("DE", None, None, Some(52.5200), Some(13.4050)),
        hardware_type: HardwareType::AntminerS19,
        units: 200,
        renewable_energy_percentage: 30.0,
        rec_percentage: 70.0,
        offset_percentage: 0.0,
        verification_status: VerificationStatus::Verified,
    };
    
    // Create a carbon offset miner (60% offset)
    let offset_miner = MinerEnvironmentalInfo {
        region: Region::with_details("AU", None, None, Some(-33.8688), Some(151.2093)),
        hardware_type: HardwareType::WhatsminerM30SPlus,
        units: 150,
        renewable_energy_percentage: 0.0,
        rec_percentage: 0.0,
        offset_percentage: 60.0,
        verification_status: VerificationStatus::Verified,
    };
    
    // Create a standard miner with no environmental commitments
    let standard_miner = MinerEnvironmentalInfo {
        region: Region::with_details("CN", None, None, Some(39.9042), Some(116.4074)),
        hardware_type: HardwareType::AntminerS19jPro,
        units: 300,
        renewable_energy_percentage: 0.0,
        rec_percentage: 0.0,
        offset_percentage: 0.0,
        verification_status: VerificationStatus::Verified,
    };
    
    // Register miners
    api.register_miner("green_miner", green_miner)?;
    api.register_miner("rec_miner", rec_miner)?;
    api.register_miner("offset_miner", offset_miner)?;
    api.register_miner("standard_miner", standard_miner)?;
    
    println!("Registered 4 miners with different environmental profiles");
    Ok(())
}

fn calculate_miner_emissions(api: &impl EnvironmentalApi) -> Result<(), Box<dyn std::error::Error>> {
    println!("\nCalculating miner emissions...");
    
    let miner_ids = vec!["green_miner", "rec_miner", "offset_miner", "standard_miner"];
    
    for id in miner_ids {
        let emissions = api.calculate_miner_emissions(id)?;
        print_miner_emissions(&emissions);
    }
    
    Ok(())
}

fn print_miner_emissions(emissions: &MinerEmissionsData) {
    println!("\nMiner: {}", emissions.miner_id);
    println!("  Classification: {}", emissions.classification);
    println!("  Daily Energy: {:.2} kWh", emissions.daily_energy_kwh);
    println!("  Gross Emissions: {:.2} kg CO2e", emissions.gross_emissions_kg);
    println!("  Net Emissions: {:.2} kg CO2e", emissions.net_emissions_kg);
    println!("  Reduction: {:.1}%", emissions.reduction_percentage);
    println!("  Impact Score: {:.1}", emissions.impact_score);
}

fn calculate_network_emissions(api: &impl EnvironmentalApi) -> Result<(), Box<dyn std::error::Error>> {
    println!("\nCalculating network-wide emissions...");
    
    let options = ReportingOptions {
        include_unverified_miners: false,
        detailed_breakdown: true,
        regional_analysis: true,
        timeframe_days: 30,
    };
    
    let network_data = api.calculate_network_emissions(&options)?;
    print_network_emissions(&network_data);
    
    // Get regional breakdown
    let regional_data = api.get_regional_emissions()?;
    
    println!("\nRegional Breakdown:");
    for (region, data) in regional_data {
        println!("  {}: {:.2} kg CO2e ({} miners)", 
            region.country_code, 
            data.emissions_kg,
            data.miner_count
        );
    }
    
    Ok(())
}

fn print_network_emissions(data: &NetworkEmissionsData) {
    println!("\nNetwork Emissions Summary (as of {})", data.timestamp);
    println!("  Total Energy: {:.2} kWh", data.total_energy_kwh);
    println!("  Gross Emissions: {:.2} kg CO2e", data.total_gross_emissions_kg);
    println!("  Net Emissions: {:.2} kg CO2e", data.total_net_emissions_kg);
    println!("  Reduction: {:.1}%", data.reduction_percentage);
    println!("  Miners: {}", data.miner_count);
    println!("  Green Miners: {:.1}%", data.green_miner_percentage);
    println!("  Average Impact Score: {:.1}", data.average_impact_score);
}

fn demonstrate_thread_safe_api() -> Result<(), Box<dyn std::error::Error>> {
    println!("\nDemonstrating thread-safe API usage...");
    
    let thread_safe_api = ThreadSafeEnvironmentalApi::new();
    let api_clone = thread_safe_api.clone_api();
    
    // This would typically be used in a multi-threaded context
    std::thread::spawn(move || {
        let mut api = api_clone.lock().unwrap();
        
        // Simulate API usage in a separate thread
        let green_miner = MinerEnvironmentalInfo {
            region: Region::with_details("CA", None, None, Some(45.4215), Some(-75.6972)),
            hardware_type: HardwareType::AntminerS19XP,
            units: 50,
            renewable_energy_percentage: 100.0,
            rec_percentage: 0.0,
            offset_percentage: 0.0,
            verification_status: VerificationStatus::Verified,
        };
        
        api.register_miner("thread_miner", green_miner).unwrap();
        println!("  Registered miner from separate thread");
    }).join().unwrap();
    
    println!("  Thread-safe API example completed");
    Ok(())
}

fn demonstrate_api_client() -> Result<(), Box<dyn std::error::Error>> {
    println!("\nDemonstrating API client usage...");
    
    // Create a thread-safe API for the client to use
    let thread_safe_api = ThreadSafeEnvironmentalApi::new();
    
    // Clone the API for preparation
    {
        let mut api = thread_safe_api.clone_api().lock().unwrap();
        register_sample_miners(&mut *api)?;
    }
    
    // Create a client that wraps the thread-safe API
    let client = EmissionsApiClient::new(thread_safe_api.clone_api() as Arc<Mutex<dyn EnvironmentalApi + Send>>);
    
    // Get network emissions summary
    let summary = client.get_network_emissions_summary()?;
    println!("  Client retrieved network summary: {:.2} kg CO2e", summary.total_net_emissions_kg);
    
    // Get regional breakdown
    let regions = client.get_regional_breakdown()?;
    println!("  Client retrieved {} regions", regions.len());
    
    // Get treasury balance
    let balance = client.get_treasury_balance()?;
    println!("  Current treasury balance: {} units", balance);
    
    Ok(())
} 