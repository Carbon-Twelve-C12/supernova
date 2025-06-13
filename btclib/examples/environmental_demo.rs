use btclib::api::{create_environmental_api, ApiError};
use btclib::environmental::dashboard::EmissionsTimePeriod;
use btclib::environmental::treasury::EnvironmentalAssetType;
use chrono::{Utc, Duration};
use rand::rngs::OsRng;
use btclib::types::transaction::{Transaction, TransactionInput, TransactionOutput};

fn main() -> Result<(), ApiError> {
    println!("supernova Environmental Features Demo");
    println!("======================================");
    
    // Create an API instance with environmental features enabled
    let mut api = create_environmental_api();
    
    println!("\n1. Loading default emission factors...");
    let tracker = api.get_emissions_tracker_mut()?;
    tracker.load_default_emission_factors();
    println!("   Emission factors loaded.");
    
    println!("\n2. Registering mining pools with their energy sources...");
    // Register some mining pools with different energy profiles
    api.register_pool_energy_info(
        "pool1", 
        95.0,  // 95% renewable
        vec!["IS".to_string(), "NO".to_string()], // Iceland and Norway
        true,  // Verified
    )?;
    
    api.register_pool_energy_info(
        "pool2", 
        60.0,  // 60% renewable
        vec!["US".to_string()],
        false, // Not verified
    )?;
    
    api.register_pool_energy_info(
        "pool3", 
        30.0,  // 30% renewable
        vec!["CN".to_string()],
        false, // Not verified
    )?;
    
    println!("   Mining pools registered.");
    
    println!("\n3. Updating regional hashrate distribution...");
    // Update hashrate distribution
    api.update_region_hashrate("IS", 10.0)?;  // 10 TH/s in Iceland
    api.update_region_hashrate("NO", 15.0)?;  // 15 TH/s in Norway
    api.update_region_hashrate("US", 25.0)?;  // 25 TH/s in USA
    api.update_region_hashrate("CN", 50.0)?;  // 50 TH/s in China
    
    println!("   Hashrate distribution updated.");
    
    println!("\n4. Registering green miners with the treasury...");
    // Register some green miners
    api.register_green_miner("miner1", 100.0, Some("GreenEnergy Certifier"))?;
    api.register_green_miner("miner2", 75.0, Some("GreenEnergy Certifier"))?;
    api.register_green_miner("miner3", 20.0, None)?;
    
    println!("   Green miners registered.");
    
    println!("\n5. Calculating fee discounts for miners...");
    // Calculate and display fee discounts
    let discount1 = api.get_green_miner_fee_discount("miner1")?;
    let discount2 = api.get_green_miner_fee_discount("miner2")?;
    let discount3 = api.get_green_miner_fee_discount("miner3")?;
    
    println!("   Miner1 (100% renewable): {:.1}% discount", discount1);
    println!("   Miner2 (75% renewable): {:.1}% discount", discount2);
    println!("   Miner3 (20% renewable): {:.1}% discount", discount3);
    
    println!("\n6. Processing block rewards and transaction fees...");
    // Process some transaction fees
    let total_fees = 1_000_000; // 0.01 NOVA in millinova
    let allocation = api.process_block_environmental_allocation(total_fees)?;
    
    println!("   Block with {} millinova in fees", total_fees);
    println!("   Environmental allocation: {} millinova", allocation);
    
    println!("\n6a. Purchasing environmental assets with REC prioritization...");
    // Get current REC settings
    let (priority_factor, allocation_percentage) = api.get_rec_prioritization_settings()?;
    println!("   Current REC settings: priority factor = {:.1}, allocation percentage = {:.1}%", 
             priority_factor, allocation_percentage);
    
    // Purchase some assets
    let purchase_amount = 500_000; // 0.005 NOVA
    let purchases = api.purchase_environmental_assets(purchase_amount)?;
    
    // Display purchases
    println!("   Purchased {} environmental assets:", purchases.len());
    for purchase in &purchases {
        println!("     - {} {} of {:?} for {} millinova (impact score: {:.2})", 
                purchase.amount, 
                if purchase.asset_type == EnvironmentalAssetType::RenewableEnergyCertificate {
                    "MWh"
                } else {
                    "tonnes"
                },
                purchase.asset_type,
                purchase.cost,
                purchase.impact_score);
    }
    
    // Try different allocation
    println!("\n   Updating REC allocation to demonstrate impact...");
    api.update_rec_prioritization(3.0, 90.0)?;
    let (new_priority, new_allocation) = api.get_rec_prioritization_settings()?;
    println!("   New REC settings: priority factor = {:.1}, allocation percentage = {:.1}%", 
             new_priority, new_allocation);
    
    // Purchase again with new settings
    let purchases = api.purchase_environmental_assets(purchase_amount)?;
    
    // Display purchases
    println!("   Purchased with new settings:");
    for purchase in &purchases {
        println!("     - {} {} of {:?} for {} millinova (impact score: {:.2})", 
                purchase.amount, 
                if purchase.asset_type == EnvironmentalAssetType::RenewableEnergyCertificate {
                    "MWh"
                } else {
                    "tonnes"
                },
                purchase.asset_type,
                purchase.cost,
                purchase.impact_score);
    }
    
    println!("\n7. Creating a transaction and estimating its emissions...");
    // Create a sample transaction
    let tx = Transaction::new(
        1, // version
        vec![
            TransactionInput::new(
                [1u8; 32], // Previous transaction hash
                0,         // Output index
                vec![],    // Signature script
                0xffffffff, // Sequence
            ),
        ],
        vec![
            TransactionOutput::new(
                90_000, // Amount
                vec![], // Public key script
            ),
        ],
        0, // lock_time
    );
    
    // Estimate emissions for this transaction
    let tx_emissions = api.estimate_transaction_emissions(&tx)?;
    
    println!("   Transaction emissions: {:.6} kg CO2e", tx_emissions.tonnes_co2e * 1000.0);
    println!("   Transaction energy usage: {:.6} kWh", tx_emissions.energy_kwh);
    
    println!("\n8. Calculating network emissions for different time periods...");
    // Calculate emissions for different periods
    let now = Utc::now();
    let day_ago = now - Duration::days(1);
    let week_ago = now - Duration::weeks(1);
    
    let daily_emissions = api.calculate_network_emissions(day_ago, now)?;
    let weekly_emissions = api.calculate_network_emissions(week_ago, now)?;
    
    println!("   Daily emissions: {:.2} tonnes CO2e", daily_emissions.tonnes_co2e);
    println!("   Weekly emissions: {:.2} tonnes CO2e", weekly_emissions.tonnes_co2e);
    
    if let Some(renewable) = daily_emissions.renewable_percentage {
        println!("   Renewable energy percentage: {:.1}%", renewable);
    }
    
    println!("\n9. Generating environmental metrics for reporting...");
    // Generate metrics
    let daily_metrics = api.generate_environmental_metrics(EmissionsTimePeriod::Daily, 100_000)?;
    
    println!("   Generated metrics for daily period");
    println!("   - Total emissions: {:.2} tonnes CO2e", daily_metrics.total_emissions);
    println!("   - Energy consumption: {:.2} kWh", daily_metrics.energy_consumption);
    println!("   - Emissions per transaction: {:.4} kg CO2e", daily_metrics.emissions_per_transaction);
    
    println!("\n10. Generating environmental report...");
    // Generate a report
    let report = api.generate_environmental_report(EmissionsTimePeriod::Daily)?;
    
    println!("\n{}", report);
    
    println!("\n11. Exporting environmental metrics as JSON...");
    // Export as JSON
    let json = api.export_environmental_metrics_json(EmissionsTimePeriod::Daily)?;
    
    println!("   JSON metrics generated ({} bytes)", json.len());
    println!("   First 100 characters: {}", &json[0..json.len().min(100)]);
    
    println!("\nDemo completed successfully!");
    
    Ok(())
} 