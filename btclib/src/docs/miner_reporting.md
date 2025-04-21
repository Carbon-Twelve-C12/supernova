# Miner Environmental Reporting Framework

This document describes the miner-level environmental reporting framework in SuperNova, which enables miners to report, verify, and improve their environmental impact.

## Overview

The miner reporting framework allows:

1. **Miners** to report their environmental attributes, including energy sources, hardware types, and performance metrics
2. **Blockchain Nodes** to collect and verify environmental claims from miners
3. **Users and Applications** to access environmental data about miners and the network as a whole
4. **Regulators and Auditors** to verify environmental claims with standard metrics

The framework prioritizes Renewable Energy Certificates (RECs) over carbon offsets, recognizing that addressing emissions at the source is more effective than offsetting them after the fact.

## Key Components

### Miner Environmental Information

The `MinerEnvironmentalInfo` struct represents a miner's environmental attributes:

```rust
pub struct MinerEnvironmentalInfo {
    pub miner_id: String,
    pub name: String,
    pub region: Region,
    pub hardware_types: Vec<HardwareType>,
    pub energy_sources: HashMap<EnergySource, f64>,
    pub renewable_percentage: f64,
    pub verification: Option<VerificationInfo>,
    pub total_hashrate: f64,
    pub energy_consumption_kwh_day: f64,
    pub carbon_footprint_tonnes_year: Option<f64>,
    pub last_update: chrono::DateTime<chrono::Utc>,
    pub has_rec_certificates: bool,
    pub has_carbon_offsets: bool,
    pub certificates_url: Option<String>,
}
```

This comprehensive structure tracks all aspects of a miner's environmental profile, from energy sources to verification status.

### Verification System

The verification system ensures that environmental claims are trustworthy:

```rust
pub enum VerificationStatus {
    Pending,
    Verified,
    Failed,
    Expired,
    Disputed,
}

pub struct VerificationInfo {
    pub provider: String,
    pub date: chrono::DateTime<chrono::Utc>,
    pub reference: String,
    pub status: VerificationStatus,
}
```

The framework prioritizes verified claims when calculating network-wide statistics and determining fee discounts.

### Reporting Manager

The `MinerReportingManager` provides centralized management of environmental reporting:

```rust
pub struct MinerReportingManager {
    miners: HashMap<String, MinerEnvironmentalInfo>,
    emission_factors: HashMap<Region, EmissionFactor>,
    hardware_baselines: HashMap<HardwareType, f64>,
}
```

This component handles registration, updates, verification, and reporting of miner environmental data.

## Key Features

### 1. Energy Source Tracking

The framework tracks energy sources with percentage breakdowns:

```rust
pub enum EnergySource {
    Solar,
    Wind,
    Hydro,
    Geothermal,
    Nuclear,
    Coal,
    NaturalGas,
    Oil,
    Biomass,
    Grid,
    Other,
}
```

Each source can be classified as renewable or non-renewable, with specific emissions factors.

### 2. Hardware Efficiency Benchmarking

The system includes efficiency benchmarks for different mining hardware:

```rust
pub enum HardwareType {
    AntminerS9,
    AntminerS19,
    AntminerS19Pro,
    // ... other hardware types
}
```

These benchmarks allow comparing a miner's actual efficiency against theoretical baselines.

### 3. Carbon Footprint Calculation

The framework calculates carbon footprints based on:

- Energy consumption
- Energy source mix
- Regional grid emissions factors
- REC and carbon offset status

Calculations take into account both:
- **Location-based accounting**: Using regional grid factors
- **Market-based accounting**: Adjusting for RECs and other instruments

### 4. REC and Offset Prioritization

The framework emphasizes RECs over carbon offsets:

1. **RECs** directly address emissions at the source by supporting renewable energy generation
2. **Carbon Offsets** are treated as a secondary mitigation strategy
3. REC verification receives higher priority and greater fee discounts

### 5. Verification and Transparency

The system includes:

- Third-party verification tracking
- Time-limited verification validity
- Public URLs for certification evidence
- Standardized reporting formats

## Usage Examples

### Registering a Miner

```rust
let mut manager = MinerReportingManager::new();

// Create miner info
let mut miner = MinerEnvironmentalInfo::new(
    "miner123".to_string(),
    "Green Mining Co.".to_string(),
    Region::NorthAmerica,
);

// Add hardware information
miner.add_hardware_types(vec![HardwareType::AntminerS19Pro]);

// Set energy mix
let mut sources = HashMap::new();
sources.insert(EnergySource::Solar, 60.0);
sources.insert(EnergySource::Wind, 30.0);
sources.insert(EnergySource::Grid, 10.0);
miner.update_energy_sources(sources).unwrap();

// Set performance metrics
miner.update_performance_metrics(500.0, 12000.0).unwrap();

// Register REC certificates
miner.update_rec_status(true, Some("https://example.com/rec/12345".to_string()));

// Register with the system
manager.register_miner(miner).unwrap();
```

### Calculating Carbon Footprint

```rust
// Set up emission factors
let mut emission_factors = HashMap::new();
emission_factors.insert(
    Region::NorthAmerica,
    EmissionFactor {
        grid_emissions_factor: 0.38,
        region_name: "North America".to_string(),
    },
);
manager.set_emission_factors(emission_factors);

// Calculate footprints for all miners
let results = manager.calculate_carbon_footprints();

// Process results
for (miner_id, result) in results {
    match result {
        Ok(footprint) => println!("{}: {} tonnes CO2e/yr", miner_id, footprint),
        Err(e) => println!("{}: Error: {}", miner_id, e),
    }
}
```

### Identifying Green Miners

```rust
// Get verified miners with renewable energy
let green_miners = manager.get_verified_green_miners();

// Get miners with carbon offsets
let offset_miners = manager.get_offset_miners();
```

### Generating Environmental Reports

```rust
// Generate a comprehensive environmental report
let report = manager.generate_report();

println!("Total hashrate: {} TH/s", report.total_hashrate);
println!("Average renewable %: {}", report.average_renewable_percentage);
println!("Green miners: {}", report.green_miners);
```

## Integration with Fee Structure

The miner reporting framework integrates with SuperNova's fee structure to provide incentives:

1. **Green Miner Discounts**: Verified miners with high renewable percentages receive transaction fee discounts
2. **REC Prioritization**: Miners with RECs receive higher discounts than those with only carbon offsets
3. **Verification Requirement**: Only verified claims are eligible for discounts
4. **Graduated Scale**: Discounts increase with renewable percentage

## Miner Privacy Considerations

The framework balances transparency with privacy:

- Miners opt-in to reporting
- Granular control over what information is public
- Privacy-preserving aggregated statistics
- Option for confidential verification with trusted third parties

## Future Enhancements

Planned enhancements to the miner reporting framework include:

1. **Real-time Verification**: Integration with energy monitoring systems for real-time verification
2. **Geographic Precision**: More precise location tracking for accurate grid factor assignment
3. **Hardware Certification**: Certification program for energy-efficient mining hardware
4. **Integrated REC Marketplace**: Direct purchase of RECs through the framework
5. **Time-of-Use Tracking**: Accounting for variable grid emissions based on time of day
6. **Standardized API**: API for third-party environmental monitoring tools 