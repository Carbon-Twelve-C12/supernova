use btclib::environmental::{
    api::{AssetPurchaseRecord, EnvironmentalApi, MinerEmissionsData, NetworkEmissionsData},
    emissions::Region,
    miner_reporting::VerificationStatus,
    types::HardwareType,
};
use chrono::{DateTime, TimeZone, Utc};
use serde_json::json;
use std::collections::HashMap;

/// Mock implementation of the EnvironmentalApi trait for demonstration purposes
struct MockEnvironmentalApi {
    miners: Vec<MinerEmissionsData>,
    asset_purchases: Vec<AssetPurchaseRecord>,
}

impl MockEnvironmentalApi {
    fn new() -> Self {
        Self {
            miners: Vec::new(),
            asset_purchases: Vec::new(),
        }
    }

    fn add_miner(&mut self, 
        id: &str, 
        hardware: HardwareType, 
        region: &str, 
        renewable: f64, 
        status: VerificationStatus, 
        emissions: f64
    ) {
        self.miners.push(MinerEmissionsData {
            miner_id: id.to_string(),
            hardware_type: hardware,
            region: region.to_string(),
            renewable_percentage: renewable,
            verification_status: status,
            emissions,
        });
    }

    fn add_asset_purchase(&mut self, asset_type: &str, value: f64, date: &str) {
        self.asset_purchases.push(AssetPurchaseRecord {
            asset_type: asset_type.to_string(),
            value,
            purchase_date: date.to_string(),
        });
    }
}

impl EnvironmentalApi for MockEnvironmentalApi {
    fn get_network_emissions(&self) -> Result<NetworkEmissionsData, String> {
        Ok(NetworkEmissionsData {
            total_emissions: 125000.0,
            emissions_per_transaction: 0.35,
            emissions_per_block: 18.5,
            timestamp: Utc::now(),
        })
    }

    fn get_all_miners(&self) -> Result<Vec<MinerEmissionsData>, String> {
        Ok(self.miners.clone())
    }

    fn get_recent_asset_purchases(&self, limit: usize) -> Result<Vec<AssetPurchaseRecord>, String> {
        Ok(self.asset_purchases.iter().rev().take(limit).cloned().collect())
    }

    fn get_treasury_balance(&self) -> Result<f64, String> {
        Ok(250000.0)
    }

    fn get_emissions_history(&self, days: usize) -> Result<Vec<(DateTime<Utc>, f64)>, String> {
        let mut history = Vec::new();
        let now = Utc::now();
        
        // Generate some mock historical data
        for i in 0..days {
            let date = now - chrono::Duration::days(i as i64);
            let emissions = 120000.0 + (i as f64 * 500.0); // Simple mock data
            history.push((date, emissions));
        }
        
        Ok(history)
    }

    fn get_all_asset_purchases(&self) -> Result<Vec<AssetPurchaseRecord>, String> {
        Ok(self.asset_purchases.clone())
    }
}

fn populate_mock_data(api: &mut MockEnvironmentalApi) {
    // Add miners with various configurations
    api.add_miner(
        "miner1", 
        HardwareType::AntminerS19Pro, 
        "US-West", 
        0.95, 
        VerificationStatus::VerifiedRec, 
        120.5
    );
    
    api.add_miner(
        "miner2", 
        HardwareType::WhatsminerM30S, 
        "EU-Central", 
        0.75, 
        VerificationStatus::PendingVerification, 
        245.8
    );
    
    api.add_miner(
        "miner3", 
        HardwareType::AntminerS9, 
        "Asia-East", 
        0.0, 
        VerificationStatus::Unverified, 
        890.2
    );
    
    api.add_miner(
        "miner4", 
        HardwareType::AvalonA1246, 
        "US-East", 
        0.50, 
        VerificationStatus::VerifiedOffset, 
        310.7
    );
    
    api.add_miner(
        "miner5", 
        HardwareType::AntminerS19, 
        "Europe-North", 
        1.0, 
        VerificationStatus::VerifiedRec, 
        0.0
    );
    
    // Add asset purchases
    api.add_asset_purchase("REC", 75000.0, "2025-04-01");
    api.add_asset_purchase("Carbon Offset", 25000.0, "2025-04-05");
    api.add_asset_purchase("REC", 100000.0, "2025-04-10");
    api.add_asset_purchase("REC", 50000.0, "2025-04-15");
    api.add_asset_purchase("Carbon Offset", 15000.0, "2025-04-20");
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("supernova Environmental Dashboard Demo");
    println!("======================================\n");
    
    // Initialize our mock API
    let mut api = MockEnvironmentalApi::new();
    populate_mock_data(&mut api);
    
    // Get network emissions data
    let network_data = api.get_network_emissions()?;
    println!("Network Emissions Summary:");
    println!("  Total CO2e: {:.2} kg", network_data.total_emissions);
    println!("  Per Transaction: {:.2} kg CO2e", network_data.emissions_per_transaction);
    println!("  Per Block: {:.2} kg CO2e", network_data.emissions_per_block);
    println!("  Timestamp: {}\n", network_data.timestamp);
    
    // Display JSON format for potential dashboard rendering
    let json_output = json!({
        "network": {
            "total_emissions": network_data.total_emissions,
            "per_transaction": network_data.emissions_per_transaction,
            "per_block": network_data.emissions_per_block,
        },
        "treasury": {
            "balance": api.get_treasury_balance()?,
        }
    });
    
    println!("Dashboard Data (JSON):");
    println!("{}\n", serde_json::to_string_pretty(&json_output)?);
    
    // Get verification status distribution
    let miners = api.get_all_miners()?;
    let mut status_counts: HashMap<VerificationStatus, usize> = HashMap::new();
    
    for miner in &miners {
        *status_counts.entry(miner.verification_status).or_insert(0) += 1;
    }
    
    println!("Miner Verification Status Distribution:");
    for (status, count) in &status_counts {
        println!("  {:?}: {} miners", status, count);
    }
    println!();
    
    // Calculate renewable energy percentage
    let total_miners = miners.len();
    let renewable_miners = miners.iter()
        .filter(|m| m.renewable_percentage >= 0.5)
        .count();
    
    println!("Renewable Energy Adoption:");
    println!("  {:.1}% of miners use 50%+ renewable energy\n", 
        (renewable_miners as f64 / total_miners as f64) * 100.0);
    
    // Calculate environmental asset distribution
    let asset_purchases = api.get_all_asset_purchases()?;
    let mut rec_total = 0.0;
    let mut offset_total = 0.0;
    
    for purchase in &asset_purchases {
        if purchase.asset_type == "REC" {
            rec_total += purchase.value;
        } else if purchase.asset_type == "Carbon Offset" {
            offset_total += purchase.value;
        }
    }
    
    let total_purchases = rec_total + offset_total;
    
    println!("Environmental Asset Distribution:");
    println!("  Renewable Energy Certificates: ${:.2} ({:.1}%)", 
        rec_total, (rec_total / total_purchases) * 100.0);
    println!("  Carbon Offsets: ${:.2} ({:.1}%)", 
        offset_total, (offset_total / total_purchases) * 100.0);
    
    Ok(())
} 