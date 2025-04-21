# SuperNova Environmental Features

This document provides an overview of the environmental impact measurement and mitigation features in the SuperNova blockchain.

## Overview

SuperNova includes comprehensive tools to measure, track, and mitigate the environmental impact of blockchain operations. These features allow for:

1. Measuring and reporting network energy consumption and emissions
2. Allocating transaction fees to environmental projects
3. Providing incentives for miners using renewable energy sources
4. Tracking and visualizing environmental metrics over time

## Emissions Tracking

The emissions tracking system uses the Cambridge Bitcoin Electricity Consumption Index (CBECI) methodology to estimate network energy usage based on:

- Network hashrate and geographical distribution
- Regional electricity grid emissions factors
- Mining hardware energy efficiency
- Reported renewable energy usage

### Basic Usage Example

```rust
use btclib::api::create_environmental_api;
use chrono::{Utc, Duration};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create an API with environmental features enabled
    let mut api = create_environmental_api();
    
    // Load default emission factors
    let tracker = api.get_emissions_tracker_mut()?;
    tracker.load_default_emission_factors();
    
    // Register mining pools with their energy sources
    api.register_pool_energy_info(
        "green_pool", 
        95.0,  // 95% renewable
        vec!["IS".to_string()], // Iceland
        true,  // Verified
    )?;
    
    // Update regional hashrate distribution
    api.update_region_hashrate("IS", 10.0)?;  // 10 TH/s in Iceland
    api.update_region_hashrate("US", 25.0)?;  // 25 TH/s in USA
    
    // Calculate emissions for the last 24 hours
    let now = Utc::now();
    let day_ago = now - Duration::days(1);
    let emissions = api.calculate_network_emissions(day_ago, now)?;
    
    println!("Daily emissions: {:.2} tonnes CO2e", emissions.tonnes_co2e);
    println!("Energy consumption: {:.2} kWh", emissions.energy_kwh);
    
    if let Some(renewable) = emissions.renewable_percentage {
        println!("Renewable energy percentage: {:.1}%", renewable);
    }
    
    Ok(())
}
```

## Emissions Factor Database

The current implementation includes a baseline emissions factor database covering major regions. Future enhancements will include:

1. **Enhanced Geographic Coverage**:
   - Country-level factors for all countries
   - State/province level for major economies
   - Local grid-level data for mining hotspots

2. **Temporal Resolution**:
   - Seasonal variations in grid emissions
   - Time-of-day variations for certain regions
   - Historical trend data

3. **Source Verification**:
   - Multiple authoritative data sources
   - Timestamp for data freshness
   - Confidence ratings for each factor

4. **API Integration**:
   - Automated updating from international data sources
   - Custom APIs for project-specific emissions factors
   - Regional policy tracking for carbon pricing

### Adding Custom Emissions Factors

```rust
use btclib::api::create_environmental_api;
use btclib::environmental::emissions::{Region, EmissionFactor, EmissionsFactorSource};

fn add_custom_factors() -> Result<(), Box<dyn std::error::Error>> {
    let mut api = create_environmental_api();
    let tracker = api.get_emissions_tracker_mut()?;
    
    // Add custom emission factor for a specific region
    let region = Region {
        country_code: "CA".to_string(),
        sub_region: Some("QC".to_string()), // Quebec province
    };
    
    let factor = EmissionFactor {
        g_co2e_per_kwh: 35.0, // Quebec has very low carbon electricity (mostly hydro)
        year: 2024,
        source: EmissionsFactorSource::Other,
    };
    
    tracker.add_emission_factor(region, factor);
    
    Ok(())
}
```

## Mining Hardware Specification

SuperNova allows for detailed hardware modeling to improve emissions estimates:

1. **Hardware Registry**:
   - Predefined database of common mining hardware
   - Energy efficiency specifications for each model
   - Performance characteristics at different settings

2. **Hardware Verification**:
   - Power consumption patterns
   - Hashrate verification
   - Certified hardware program

3. **Custom Hardware Registration**:

```rust
use btclib::api::create_environmental_api;
use btclib::environmental::emissions::{HardwareType, Efficiency};

fn register_mining_hardware() -> Result<(), Box<dyn std::error::Error>> {
    let mut api = create_environmental_api();
    let tracker = api.get_emissions_tracker_mut()?;
    
    // Register a specific ASIC model
    let hardware = HardwareType::ASIC("SuperMiner X1".to_string());
    let efficiency = Efficiency {
        joules_per_terahash: 38.0, // J/TH
        typical_power_watts: Some(3200.0), // Watts
    };
    
    tracker.register_hardware_efficiency(hardware, efficiency);
    
    Ok(())
}
```

## Environmental Treasury

The environmental treasury system automatically allocates a configurable percentage of transaction fees to fund environmental initiatives like:

- Carbon offset purchases
- Renewable energy certificates
- Energy efficiency projects
- Climate research funding

### Fee Allocation Example

```rust
use btclib::api::create_environmental_api;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut api = create_environmental_api();
    
    // Process transaction fees and allocate to treasury
    let block_fees = 1_000_000; // 0.01 BTC in satoshis
    let allocation = api.process_block_environmental_allocation(block_fees)?;
    
    println!("Block fees: {} satoshis", block_fees);
    println!("Environmental allocation: {} satoshis", allocation);
    
    // Get current treasury balance
    let treasury = api.get_treasury()?;
    println!("Treasury balance: {} satoshis", treasury.balance());
    
    Ok(())
}
```

## Green Miner Incentives

SuperNova provides fee discounts to miners using renewable energy sources:

| Renewable Percentage | Fee Discount |
|----------------------|--------------|
| 95-100%              | 10%          |
| 75-94%               | 7%           |
| 50-74%               | 5%           |
| 25-49%               | 2%           |
| 0-24%                | 0%           |

### Advanced Incentive Mechanisms

Beyond simple fee discounts, future implementations will offer:

1. **Block Reward Enhancement**:
   - Additional 1-3% block subsidy for verified green miners
   - Funded from environmental treasury rather than inflation
   - Scales with verified renewable percentage
   - Requires third-party verification for top tiers

2. **Reputation System**:
   - Public leaderboard of environmentally responsible miners
   - On-chain "green certificates" as NFTs
   - Visible "green mining" label in block explorers
   - Community governance of reputation criteria

3. **REC and Carbon Credit Integration**:
   - Purchase RECs and carbon credits to offset emissions
   - Integration with verified REC and carbon credit registries/markets
   - Higher ranking to "REC miners" and "offset miners" depending on renewable energy and carbon credit usage
   - On-chain verification of enhance validity

### Miner Registration Example

```rust
use btclib::api::create_environmental_api;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut api = create_environmental_api();
    
    // Register a green miner
    api.register_green_miner(
        "miner1", 
        100.0, // 100% renewable
        Some("GreenEnergy Certifier"), // Verification provider
    )?;
    
    // Calculate their fee discount
    let discount = api.get_green_miner_fee_discount("miner1")?;
    println!("Miner1 fee discount: {:.1}%", discount);
    
    Ok(())
}
```

## Environmental Dashboard

The environmental dashboard provides visualization and reporting tools for network emissions and energy usage:

- Real-time and historical emissions data
- Regional hashrate distribution maps
- Renewable energy percentage tracking
- Transaction-level emissions information
- Environmental treasury activity reports

### Dashboard Example

```rust
use btclib::api::create_environmental_api;
use btclib::environmental::dashboard::EmissionsTimePeriod;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut api = create_environmental_api();
    
    // Setup the dashboard with some data
    // ... (register pools, update hashrates, etc.)
    
    // Generate metrics for different time periods
    let daily_metrics = api.generate_environmental_metrics(
        EmissionsTimePeriod::Daily, 
        100_000, // transaction count
    )?;
    
    // Generate a text report
    let report = api.generate_environmental_report(EmissionsTimePeriod::Daily)?;
    println!("{}", report);
    
    // Export metrics as JSON for web dashboard
    let json = api.export_environmental_metrics_json(EmissionsTimePeriod::Daily)?;
    
    Ok(())
}
```

## Configuration

Environmental features can be enabled and configured in the main blockchain configuration:

```rust
use btclib::config::{Config, EnvironmentalConfig};

// Create a config with environmental features enabled
let mut config = Config::default();
config.environmental.enabled = true;
config.environmental.emissions.enabled = true;
config.environmental.treasury_allocation_percentage = 2.0; // 2% allocation
config.environmental.enable_green_miner_discounts = true;

// Or use the helper method
let config = Config::with_environmental_features();
```

## Integration with Block Explorer

When enabled, the environmental dashboard can be integrated with the block explorer to provide:

- Network-level emissions data on the main page
- Transaction-level emissions data for each transaction
- Miner environmental performance statistics
- Treasury allocation and spending information

## Implementation Status

The current implementation represents Phase 1 of the environmental framework:

- ✅ Basic emissions tracking using CBECI methodology
- ✅ Regional hashrate distribution tracking
- ✅ Mining pool energy source registration
- ✅ Environmental treasury with fee allocation
- ✅ Green miner incentive system (fee discounts)
- ✅ Transaction-level emissions calculation
- ✅ Environmental dashboard for reporting

## Future Development

Future enhancements will include:

1. **Phase 2 (3-6 months)**:
   - Enhanced emissions factor database with more regions
   - Hardware specification system with efficiency tracking
   - Carbon credit marketplace integration
   - Advanced reporting dashboard with visualization
   - Regional mining efficiency comparisons

2. **Phase 3 (6+ months)**:
   - Smart contract integration for carbon credits and RECs
   - Advanced green mining incentive mechanisms
   - Real-time grid emissions data integration
   - Integration with external environmental monitoring systems
   - Community governance of environmental treasury

## For More Information

- [Cambridge Bitcoin Electricity Consumption Index](https://ccaf.io/cbeci/index)
- [Carbon Offsetting and Blockchain](https://www.carbon-offsetting.org)
- [Renewable Energy for Blockchain](https://www.renewable-blockchain.org) 