# Environmental Features in SuperNova

## Overview

SuperNova implements a comprehensive framework for measuring, reporting, and mitigating the environmental impact of blockchain operations. Inspired by best practices from industry leaders, the system provides unprecedented transparency and tools for promoting sustainable blockchain operations.

Key features include:

- **Geographic Emissions Tracking**: Fine-grained tracking of energy consumption and carbon emissions based on the geographic distribution of miners
- **REC Prioritization Framework**: A verification system that prioritizes Renewable Energy Certificates (RECs) over carbon offsets
- **Market-Based & Location-Based Emissions**: Dual reporting approaches aligned with the GHG Protocol
- **Marginal Emissions Analysis**: Advanced impact assessment measuring the actual grid impact of electricity consumption
- **Hardware-Specific Efficiency**: Granular tracking of mining hardware and their energy efficiency
- **Environmental Dashboard**: Comprehensive visualization of network environmental metrics

## Emissions Tracking Methodology

SuperNova's emissions tracking is built on a hierarchical approach to emissions factors, similar to the methodology used by Industry Leaders:

```rust
// Create an emissions tracker with configuration
let emissions_tracker = EmissionsTracker::new(EmissionsConfig {
    enabled: true,
    default_emission_factor: 450.0,
    emissions_api_endpoint: Some("https://api.example.com/v1".to_string()),
    preferred_data_source: Some(EmissionsDataSource::WattTime),
    use_marginal_emissions: true,
    known_hashrate_percentage: 60.0,
    default_network_efficiency: 40.0,
    // Additional configuration omitted for brevity
});

// Register mining pools with energy information
emissions_tracker.register_pool_energy_info(pool_id, pool_energy_info);

// Update regional hashrate distribution
emissions_tracker.update_region_hashrate(
    Region::with_sub_region("US", "WA"),
    HashRate(120.0)
);

// Calculate daily network emissions with location-based and market-based methods
let daily_emissions = emissions_tracker.calculate_network_emissions(day_ago, now)?;
```

### Hierarchical Data Sources

SuperNova uses a tiered approach to emissions data:

1. **Real-time Grid Data**: Where available, we use real-time emissions data from services like WattTime or Electricity Maps
2. **Regional Grid Averages**: For areas without real-time data, we use regional grid averages from authorities like the IEA
3. **Global Averages**: As a fallback, global average emissions factors are used

### Emissions Calculation Approaches

The system provides multiple emission calculation methodologies:

- **Location-Based Emissions**: Based on grid average emissions factors for the regions where mining occurs
- **Market-Based Emissions**: Accounting for renewable energy purchases (primarily RECs) and their impact
- **Marginal Emissions Analysis**: Measuring the actual grid impact using marginal operating emissions rates

## REC Prioritization Framework

SuperNova prioritizes Renewable Energy Certificates (RECs) over carbon offsets because RECs directly address the source of emissions from electricity consumption:

```rust
// Register a miner with REC certificates
let mut miner = MinerEnvironmentalInfo::new(
    "miner1".to_string(),
    "Green Mining Operation".to_string(),
    region
);

// Add REC certificate
let rec_certificate = RECCertificate {
    certificate_id: "IREC-12345",
    issuer: "Green-e Energy",
    amount_mwh: 8760.0, // 1MW for a year
    generation_start: start_date,
    generation_end: end_date,
    generation_location: Some(region),
    energy_type: EnergySource::Wind,
    verification_status: VerificationStatus::Verified,
    certificate_url: Some("https://example.com/cert/12345"),
    last_verified: Some(verification_date),
    blockchain_tx_id: Some("0x1234567890abcdef"),
};
miner.add_rec_certificate(rec_certificate);

// Calculate fee discount with REC prioritization
let discount = miner_reporting.calculate_fee_discount_with_rec_priority(miner_id);
```

### Benefits of REC Prioritization

1. **Direct Emissions Reduction**: RECs directly connect to the source of mining emissions (electricity)
2. **Grid Impact**: REC purchases contribute to increasing renewable energy capacity on the grid
3. **Verification Standards**: RECs have well-established verification mechanisms with regulatory oversight
4. **Alignment with Industry Standards**: Follows best practices for scope 2 emissions accounting

### Implementation in SuperNova

The REC prioritization framework is implemented through:

- Higher fee discounts for miners with verified RECs compared to those with only carbon offsets
- Separate tracking of "green miners" (with RECs) versus "offset miners" (with only carbon offsets)
- Market-based emissions calculations that give precedence to RECs over offsets
- Treasury asset purchases that allocate more funds to RECs than to offsets

## Miner Verification System

SuperNova employs a sophisticated verification system for miner environmental claims:

```rust
// Add location verification
miner.set_location_verification(LocationVerification {
    method: LocationVerificationMethod::Audit,
    timestamp: verification_date,
    confidence: 0.95,
    verifier: Some("Verification Authority".to_string()),
    evidence_reference: Some("AUDIT-12345"),
    status: VerificationStatus::Verified,
});

// Verify REC certificate
miner_reporting.verify_rec_certificate(miner_id, certificate_id)?;
```

### Verification Methods

- **Location Verification**: Uses multiple methods, from self-declaration to multi-factor cryptographic proofs
- **REC Verification**: Validates the authenticity of REC claims through issuing authorities
- **Carbon Offset Verification**: Validates carbon offset claims through offset registries
- **Hardware Verification**: Assesses the efficiency claims of mining hardware

### Environmental Score

Miners receive an environmental score (0-100) based on:

- Renewable energy percentage (0-50 points)
- REC verification status (0-20 points)
- Carbon offset verification (0-10 points)
- Location verification strength (0-10 points)
- Energy efficiency of hardware (0-10 points)

## Environmental Dashboard

The environmental dashboard provides comprehensive reporting of network environmental performance:

```rust
// Create dashboard with all components
let dashboard = EnvironmentalDashboard::with_miner_reporting(
    emissions_tracker,
    treasury,
    miner_reporting
);

// Generate metrics for a time period
let metrics = dashboard.generate_metrics(EmissionsTimePeriod::Month, transaction_count)?;

// Generate a text report
let report = dashboard.generate_text_report(EmissionsTimePeriod::Month)?;

// Export metrics as JSON
let json = dashboard.export_metrics_json(EmissionsTimePeriod::Month)?;
```

### Dashboard Features

- **Multi-timeframe Analysis**: View metrics for day, week, month, year, or custom periods
- **Geographic Breakdown**: Visualize emissions and energy by country and region
- **Asset Summary**: Details of REC and carbon offset purchases and their impact
- **Report Types**: Toggle between location-based, market-based, and marginal emissions views
- **JSON Export**: Export metrics for external analysis or integration

## API Reference

For full API documentation, refer to the following modules:

- `btclib::environmental::emissions`: Core emissions tracking functionality
- `btclib::environmental::miner_reporting`: Miner environmental reporting system
- `btclib::environmental::treasury`: Environmental treasury and asset purchases
- `btclib::environmental::dashboard`: Environmental metrics dashboard
- `btclib::environmental::types`: Common environmental data types

## Example Applications

For examples of the environmental system in action, see:

- `btclib/examples/environmental_demo.rs`: Basic demonstration of environmental features
- `btclib/examples/filecoin_inspired_env_demo.rs`: Advanced demonstration with Filecoin Green-inspired features

## Configuration

Environmental features can be configured through the blockchain configuration file:

```toml
[environment]
enabled = true
default_emission_factor = 450.0
emissions_api_endpoint = "https://api.example.com/v1"
emissions_api_key = "your_api_key"
preferred_data_source = "WattTime" 
use_marginal_emissions = true
known_hashrate_percentage = 60.0
default_network_efficiency = 40.0
data_update_frequency_hours = 6
cache_emissions_factors = true
verify_miner_locations = true
prioritize_rec_verification = true
```

## Future Development

Future enhancements to the environmental framework include:

1. **Real-time Data Integration**: Direct integration with real-time grid emissions data providers
2. **Enhanced REC Verification**: Blockchain-based verification of REC claims
3. **Geographic Verification**: Cryptographic proof of miner location using synthetic location techniques
4. **Energy Optimization**: Tools to help miners optimize operations for minimum environmental impact
5. **REC Marketplace**: Direct integration with REC marketplaces for seamless renewable energy procurement
6. **Granular Temporal Data**: Time-of-day emissions tracking to optimize for grid conditions
7. **Enhanced Reporting**: Alignment with emerging standards for blockchain environmental reporting 