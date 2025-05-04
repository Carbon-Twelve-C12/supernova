# Environmental Features Implementation

This document provides a comprehensive overview of Supernova's environmental features, including emissions tracking, green mining incentives, reporting systems, and governance.

## Table of Contents

- [Overview](#overview)
- [Emissions Tracking](#emissions-tracking)
- [Green Mining Incentives](#green-mining-incentives)
- [Environmental Treasury](#environmental-treasury)
- [Governance and Transparency](#governance-and-transparency)
- [Monitoring and Alerting](#monitoring-and-alerting)
- [Integration Guide](#integration-guide)
- [API Reference](#api-reference)

## Overview

Supernova's environmental features are designed to address the carbon footprint of blockchain operations by:

1. **Measuring Impact**: Tracking energy consumption and carbon emissions at the network, block, and transaction level.
2. **Providing Incentives**: Encouraging miners to use renewable energy through fee discounts and prioritization.
3. **Offsetting Emissions**: Allocating a portion of transaction fees to purchasing Renewable Energy Certificates (RECs) and carbon offsets.
4. **Ensuring Transparency**: Providing comprehensive reporting on environmental metrics and mitigation efforts.
5. **Supporting Governance**: Allowing community input on environmental treasury allocation.

These features aim to make Supernova carbon-negative by design, prioritizing renewable energy generation over carbon offsets while maintaining full transparency.

## Emissions Tracking

### Methodology

Supernova's emissions tracking system uses the Cambridge Bitcoin Electricity Consumption Index (CBECI) methodology to estimate energy consumption, combined with region-specific emissions factors to calculate carbon dioxide equivalent (CO₂e) emissions.

The system accounts for:

- **Geographic Distribution**: Mining operations in different regions have different carbon intensities based on local electricity grid compositions.
- **Hardware Efficiency**: Different mining hardware has varying energy efficiency (Joules per Terahash).
- **Renewable Energy**: Self-reported and verified renewable energy usage by miners.
- **RECs and Offsets**: Accounting for purchased renewable energy certificates and carbon offsets.

### Tracking Components

1. **Regional Emissions Factors Database**
   - Contains emissions factors (gCO₂e/kWh) for different regions
   - Updated regularly from authoritative sources
   - Prioritizes marginal emissions data where available
   - Fallback to grid average emissions when necessary

2. **Mining Hardware Database**
   - Energy efficiency data for various ASIC models
   - Updated with new hardware specifications
   - Age-based derating for older equipment

3. **Transaction-level Attribution**
   - Assigns proportional emissions to transactions based on:
     - Transaction size (bytes)
     - Transaction complexity (computational cost)
     - Block space consumed
   - Calculated in real-time for all transactions

4. **Network-level Metrics**
   - Total energy consumption (kWh)
   - Total carbon emissions (tonnes CO₂e)
   - Renewable energy percentage
   - Net carbon impact after mitigation
   - Carbon intensity per transaction

### Verification System

The emissions tracking system includes verification mechanisms for:

- **Miner Location**: GPS coordinates, IP geolocation, third-party audits
- **Renewable Energy Claims**: Energy attribute certificates, bilateral contracts, direct ownership
- **Hardware Efficiency**: Performance benchmarks, power consumption tests
- **Carbon Offsets**: Registry verification, additionality assessment, permanence evaluation

## Green Mining Incentives

### Overview

Supernova implements multiple incentives to encourage the use of renewable energy for mining:

1. **Fee Discounts**: Miners using verified renewable energy receive transaction fee discounts.
2. **Block Prioritization**: Transactions routed to green miners receive higher priority.
3. **REC Validation**: Miners can register renewable energy certificates for additional benefits.

### Fee Discount Structure

| Renewable Percentage | Verification Status | Fee Discount |
|----------------------|---------------------|--------------|
| 95-100%              | Verified            | 10%          |
| 75-94%               | Verified            | 7%           |
| 50-74%               | Verified            | 5%           |
| 25-49%               | Verified            | 2%           |
| Any                  | Unverified          | 0%           |

Miners can increase their discounts by:
- Registering verifiable renewable energy certificates
- Providing third-party audits of energy sources
- Participating in carbon offset programs

### Verification Process

1. **Registration**: Miners register with the network, providing details about their energy sources.
2. **Documentation**: Miners submit evidence of renewable energy usage (utility bills, RECs, etc.).
3. **Verification**: Independent verifiers confirm the claims.
4. **Monitoring**: Ongoing monitoring ensures continued compliance.
5. **Renewal**: Periodic re-verification to maintain status.

### Implementation Details

The incentive system is implemented through:

```rust
// Mining fee calculation with green incentives
pub fn calculate_mining_fee(&self, transaction_size: usize, miner_id: &str) -> u64 {
    let base_fee = self.calculate_base_fee(transaction_size);
    
    // Apply green miner discount if applicable
    if let Some(miner) = self.get_miner_info(miner_id) {
        if miner.verification_status == VerificationStatus::Verified {
            let discount_percentage = match miner.renewable_percentage {
                p if p >= 95.0 => 10.0,
                p if p >= 75.0 => 7.0,
                p if p >= 50.0 => 5.0,
                p if p >= 25.0 => 2.0,
                _ => 0.0,
            };
            
            let discount_multiplier = 1.0 - (discount_percentage / 100.0);
            return (base_fee as f64 * discount_multiplier) as u64;
        }
    }
    
    base_fee
}
```

## Environmental Treasury

### Overview

A portion of transaction fees is allocated to the Environmental Treasury, which funds:

1. **Renewable Energy Certificates**: Directly supporting new renewable energy projects
2. **Carbon Offset Projects**: Investing in verified carbon reduction/removal projects
3. **Environmental Grants**: Funding research and development of green mining technologies
4. **Operational Costs**: Supporting verification and reporting systems

### Fee Allocation

By default, 2% of transaction fees go to the Environmental Treasury, with a dynamic adjustment mechanism based on carbon negativity targets.

### Treasury Accounts

The treasury is divided into sub-accounts:

| Account                  | Default Allocation | Purpose                                     |
|--------------------------|-------------------|----------------------------------------------|
| Renewable Certificates   | 50%              | Purchase of RECs from verified providers     |
| Carbon Offsets           | 30%              | Purchase of verified carbon offsets          |
| Environmental Grants     | 10%              | Funding research and development             |
| Operations               | 5%               | Supporting verification and reporting        |
| Emergency Reserve        | 5%               | Insurance against unexpected needs           |

### Purchasing Prioritization

The treasury prioritizes:

1. **Renewable Energy Certificates**: Given highest priority due to direct energy transition impact
2. **Verified Carbon Offsets**: High-quality projects with proven additionality and permanence
3. **Environmental Grants**: Strategic investments in green mining technology and research

## Governance and Transparency

### Governance System

The environmental governance system allows stakeholders to propose and vote on:

1. **Treasury Allocations**: Changing the distribution percentages between accounts
2. **Fee Rate Changes**: Adjusting the overall percentage of fees allocated to the treasury
3. **Project Funding**: Selecting specific environmental projects to support
4. **Policy Updates**: Modifying verification requirements or discount structures

### Proposal and Voting Process

1. **Submission**: Authorized participants can submit proposals with detailed descriptions
2. **Discussion Period**: Community input and feedback period before voting begins
3. **Voting**: Weighted voting by stakeholders (miners, developers, users)
4. **Execution**: Successful proposals are automatically implemented after a time lock period

### Transparency Dashboard

The Transparency Dashboard provides:

1. **Real-time Metrics**: Current energy usage, emissions, and offsetting activities
2. **Historical Data**: Trends in energy mix, emissions intensity, and offsetting
3. **Treasury Activity**: All purchases of RECs and carbon offsets
4. **Verification Status**: Status of miner renewable claims and verification audits
5. **Impact Reports**: Periodic detailed reports on environmental impact and mitigation

## Monitoring and Alerting

### Metrics Collection

The environmental monitoring system collects:

1. **Network Metrics**: Hashrate, difficulty, block time, transaction volume
2. **Energy Metrics**: Total consumption, renewable percentage, efficiency
3. **Emissions Metrics**: Total emissions, offsets, net impact, intensity
4. **Treasury Metrics**: Balance, allocation, purchasing activity
5. **Miner Metrics**: Distribution, energy sources, verification status

### Alerting System

The alerting system monitors key metrics and sends notifications when:

1. **Thresholds are Crossed**: e.g., emissions intensity exceeds target levels
2. **Verification is Needed**: e.g., miner claims require renewal
3. **Treasury Actions are Required**: e.g., available funds for REC purchases
4. **System Status Changes**: e.g., significant changes in renewable percentage

### Reporting System

Regular reports are generated at different intervals:

1. **Daily Snapshots**: Brief summary of current status
2. **Weekly Reports**: Detailed metrics and trend analysis
3. **Monthly Reviews**: Comprehensive analysis with recommendations
4. **Quarterly Audits**: In-depth verification and validation of all metrics

## Integration Guide

### For Miners

To participate in green mining incentives:

1. **Register Mining Operation**: Provide location, hardware, and energy information
2. **Document Energy Sources**: Submit evidence of renewable energy usage
3. **Complete Verification**: Work with verification providers to validate claims
4. **Maintain Compliance**: Keep documentation current and submit for periodic review

Example registration:

```rust
// Register as a green miner
let miner_info = MinerEnvironmentalInfo {
    miner_id: "miner123".to_string(),
    name: "Green Mining Operation".to_string(),
    region: Region::new("US-CA"),
    hardware_types: vec![HardwareType::AntminerS19XP],
    renewable_percentage: 80.0,
    verification_status: VerificationStatus::Pending,
    // Additional fields...
};

environmental_system.register_miner(miner_info);
```

### For Wallet Providers

Wallet providers can integrate environmental features by:

1. **Displaying Emissions**: Show carbon footprint of transactions
2. **Supporting Green Routing**: Preferentially route to green miners
3. **Offering Offsetting**: Allow users to purchase additional offsets
4. **Providing Transparency**: Show network environmental metrics

Example transaction with environmental data:

```rust
// Get transaction environmental impact
let tx_impact = environmental_system.calculate_transaction_emissions(transaction_size, fee);

// Display to user
wallet.display_environmental_impact(tx_impact);

// Optional: Add additional offsets
if user_wants_additional_offsets {
    wallet.add_carbon_offset(tx_impact.emissions_kg * 2.0); // 200% offset
}
```

### For Node Operators

Node operators can contribute by:

1. **Running Energy-Efficient Hardware**: Minimizing node power consumption
2. **Using Renewable Energy**: Powering nodes with green energy
3. **Enabling Metrics Collection**: Contributing to network environmental data
4. **Participating in Governance**: Voting on environmental proposals

## API Reference

### Emissions Tracking API

```rust
// Calculate network emissions
pub fn calculate_network_emissions(
    &self,
    start_time: DateTime<Utc>,
    end_time: DateTime<Utc>
) -> Result<Emissions, EmissionsError>;

// Estimate transaction emissions
pub fn estimate_transaction_emissions(
    &self,
    transaction: &Transaction
) -> Result<Emissions, EmissionsError>;

// Calculate miner emissions
pub fn calculate_miner_emissions(
    &self,
    miner_id: &str
) -> Result<MinerEmissions, EmissionsError>;
```

### Treasury API

```rust
// Process transaction fees
pub fn process_transaction_fees(
    &self,
    total_fees: u64
) -> Result<u64, TreasuryError>;

// Purchase renewable certificates
pub fn purchase_renewable_certificates(
    &self,
    amount: u64,
    provider: &str
) -> Result<String, TreasuryError>;

// Purchase carbon offsets
pub fn purchase_carbon_offsets(
    &self,
    amount: u64,
    provider: &str
) -> Result<String, TreasuryError>;
```

### Governance API

```rust
// Create a proposal
pub fn create_proposal(
    &mut self,
    title: String,
    description: String,
    proposal_type: ProposalType,
    proposer: String,
    url: Option<String>,
    emergency: bool,
) -> Result<String, GovernanceError>;

// Vote on a proposal
pub fn vote(
    &mut self,
    proposal_id: &str,
    voter: String,
    vote_for: bool,
    comment: Option<String>,
    signature: Signature,
) -> Result<(), GovernanceError>;
```

### Dashboard API

```rust
// Generate environmental metrics
pub fn generate_metrics(
    &mut self,
    period: EmissionsTimePeriod,
    transaction_count: u64
) -> Result<EnvironmentalMetrics, String>;

// Export metrics as JSON
pub fn export_metrics_json(
    &self,
    period: EmissionsTimePeriod
) -> Result<String, String>;
```

### Alerting API

```rust
// Add alert rule
pub fn add_rule(
    &mut self,
    rule: AlertRule
) -> Result<(), AlertingError>;

// Check all alert rules
pub fn check_alerts(&mut self) -> Vec<Alert>;

// Get active alerts
pub fn get_active_alerts(&self) -> Vec<&Alert>;
``` 