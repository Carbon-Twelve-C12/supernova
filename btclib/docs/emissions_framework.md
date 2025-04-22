# SuperNova Emissions Framework Documentation

## Overview

The SuperNova Emissions Framework provides a comprehensive system for measuring, tracking, and reducing the environmental impact of mining operations. This framework prioritizes the following environmental strategies in order of preference:

1. **Direct Renewable Energy Usage** - The most preferred strategy is for miners to directly use renewable energy sources.
2. **Renewable Energy Certificates (RECs)** - When direct renewable energy usage is not possible, RECs are strongly prioritized.
3. **Carbon Offsets** - While still valuable, carbon offsets are considered a secondary approach after RECs.

## REC Prioritization

Renewable Energy Certificates (RECs) are given higher priority over carbon offsets in the SuperNova emissions framework for several key reasons:

1. **Direct Impact on Electricity Generation**: RECs directly support the production of renewable electricity, which is the primary source of emissions from mining operations.

2. **Grid Transformation**: By purchasing RECs, miners contribute to increasing renewable energy capacity on the grid, helping transform the energy landscape.

3. **Regulatory Recognition**: RECs often have stronger regulatory recognition and are part of established energy markets in many jurisdictions.

4. **Verification Robustness**: REC verification systems are typically more mature with standardized processes for ensuring authenticity.

5. **Higher Impact Score**: In our framework, RECs have an environmental impact score of 9.0 compared to 5.5 for carbon offsets, reflecting their greater effectiveness in addressing mining-related emissions.

## Implementation Details

The framework implements REC prioritization through:

1. **Treasury Allocation**: The environmental treasury defaults to allocating a minimum of 60% of funds to RECs, with the remainder going to carbon offsets.

2. **Purchase Sequencing**: When making environmental asset purchases, RECs are always purchased first, with carbon offsets only purchased if funds remain.

3. **Tiered Incentives**: Miners using RECs receive higher fee discounts compared to those using only carbon offsets.

4. **Transparent Reporting**: The environmental dashboard clearly distinguishes between miners using RECs vs. carbon offsets.

## Miner Classification

Miners in the SuperNova network are classified based on their environmental commitment:

1. **Green Miners**: Directly using verifiable renewable energy (80%+ renewable)
2. **REC-Backed Miners**: Using Renewable Energy Certificates to offset grid electricity
3. **Offset Miners**: Using carbon offsets to compensate for emissions
4. **Standard Miners**: Not participating in any environmental program

Each classification receives different benefits and incentives, with preference given to those higher in the hierarchy.

## Code Example

```rust
// Example of emissions calculation prioritizing RECs
fn calculate_net_emissions(gross_emissions: f64, renewable_pct: f64, rec_pct: f64, offset_pct: f64) -> f64 {
    // First reduce by direct renewable usage
    let after_renewable = gross_emissions * (100.0 - renewable_pct) / 100.0;
    
    // Then apply RECs (prioritized over offsets)
    let remaining_pct_after_recs = (100.0 - renewable_pct - rec_pct).max(0.0);
    let after_recs = gross_emissions * remaining_pct_after_recs / 100.0;
    
    // Finally apply offsets to any remaining emissions
    let remaining_pct_after_offsets = (remaining_pct_after_recs - offset_pct).max(0.0);
    let net_emissions = gross_emissions * remaining_pct_after_offsets / 100.0;
    
    net_emissions
}
```

## Environmental Treasury Allocation

The environmental treasury is configured to prioritize RECs:

```rust
fn purchase_prioritized_assets(&mut self, total_amount: u64, rec_allocation_percentage: f64) -> Result<(), TreasuryError> {
    // Ensure REC allocation is at least 60% by default unless explicitly set lower
    let rec_allocation = if rec_allocation_percentage < 60.0 {
        warn!("REC allocation set below recommended 60% minimum: {}", rec_allocation_percentage);
        rec_allocation_percentage
    } else {
        rec_allocation_percentage
    };
    
    // Calculate amounts for each asset type
    let rec_amount = (total_amount as f64 * rec_allocation / 100.0) as u64;
    
    // Purchase RECs first (prioritized)
    self.purchase_recs(rec_amount)?;
    
    // Only purchase carbon offsets with any remaining funds
    let remaining = total_amount.saturating_sub(rec_amount);
    if remaining > 0 {
        self.purchase_carbon_offsets(remaining)?;
    }
    
    Ok(())
}
```

## Impact Scoring

Environmental impact is scored differently based on the strategy:

- Direct Renewable Energy: 10.0 impact score
- Renewable Energy Certificates: 9.0 impact score
- Carbon Offsets: 5.5 impact score

This scoring system is used to calculate weighted environmental impact scores for miners and for the network as a whole.

## Future Enhancements

The SuperNova team is continuously working to enhance the emissions framework with:

1. **Advanced REC Verification**: Implementing more sophisticated verification mechanisms for REC claims
2. **Regional REC Markets**: Supporting region-specific REC markets and pricing
3. **Energy Source Matching**: Matching REC purchases to specific renewable energy types
4. **Time-Matched RECs**: Supporting time-based matching of energy consumption with REC generation
5. **Direct Power Purchase Agreements**: Facilitating direct PPAs between miners and renewable energy producers

For more information on implementing the emissions framework in your mining operation, see the [Miner Guide](/docs/miner-guide.md) documentation. 