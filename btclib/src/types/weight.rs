/// Transaction weight calculations for fee estimation
use serde::{Serialize, Deserialize};

/// Weight units for transactions (in weight units, not bytes)
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Weight(u64);

impl Weight {
    /// Create a new weight
    pub const fn new(weight: u64) -> Self {
        Self(weight)
    }
    
    /// Get the weight value
    pub const fn value(&self) -> u64 {
        self.0
    }
    
    /// Create weight from virtual bytes (vbytes)
    pub const fn from_vb(vbytes: u64) -> Self {
        Self(vbytes * 4)
    }
    
    /// Convert to virtual bytes (vbytes)
    pub const fn to_vb(&self) -> u64 {
        (self.0 + 3) / 4 // Round up
    }
    
    /// Add two weights
    pub const fn add(&self, other: Weight) -> Self {
        Self(self.0 + other.0)
    }
    
    /// Subtract weight (saturating)
    pub const fn saturating_sub(&self, other: Weight) -> Self {
        Self(self.0.saturating_sub(other.0))
    }
}

impl Default for Weight {
    fn default() -> Self {
        Self::new(0)
    }
}

impl std::ops::Add for Weight {
    type Output = Self;
    
    fn add(self, rhs: Self) -> Self::Output {
        Self(self.0 + rhs.0)
    }
}

impl std::ops::Sub for Weight {
    type Output = Self;
    
    fn sub(self, rhs: Self) -> Self::Output {
        Self(self.0 - rhs.0)
    }
}

/// Transaction weight calculator
pub struct WeightCalculator;

impl WeightCalculator {
    /// Base transaction weight (version + locktime)
    pub const BASE_TX_WEIGHT: Weight = Weight::new(4 * (4 + 4));
    
    /// Weight per input (without witness data)
    pub const INPUT_WEIGHT: Weight = Weight::new(4 * (32 + 4 + 1 + 4)); // txid + vout + script_len + sequence
    
    /// Weight per output
    pub const OUTPUT_WEIGHT: Weight = Weight::new(4 * (8 + 1)); // value + script_len
    
    /// Calculate weight for a P2PKH input (with signature)
    pub fn p2pkh_input_weight() -> Weight {
        // Input base + script sig (approximately 107 bytes)
        Weight::new(4 * (32 + 4 + 1 + 107 + 4))
    }
    
    /// Calculate weight for a P2WPKH input
    pub fn p2wpkh_input_weight() -> Weight {
        // Input base (no script sig) + witness data
        Weight::new(4 * (32 + 4 + 1 + 4) + 1 * (1 + 73 + 34))
    }
    
    /// Calculate weight for a P2PKH output
    pub fn p2pkh_output_weight() -> Weight {
        Weight::new(4 * (8 + 1 + 25)) // value + script_len + script
    }
    
    /// Calculate weight for a P2WPKH output
    pub fn p2wpkh_output_weight() -> Weight {
        Weight::new(4 * (8 + 1 + 22)) // value + script_len + script
    }
    
    /// Estimate transaction weight
    pub fn estimate_weight(inputs: usize, outputs: usize, is_segwit: bool) -> Weight {
        let mut weight = Self::BASE_TX_WEIGHT;
        
        // Add input weights
        for _ in 0..inputs {
            weight = weight + if is_segwit {
                Self::p2wpkh_input_weight()
            } else {
                Self::p2pkh_input_weight()
            };
        }
        
        // Add output weights
        for _ in 0..outputs {
            weight = weight + if is_segwit {
                Self::p2wpkh_output_weight()
            } else {
                Self::p2pkh_output_weight()
            };
        }
        
        // Add varint for input/output counts
        weight = weight + Weight::new(4 * 2); // Assuming single byte varints
        
        weight
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_weight_conversion() {
        let weight = Weight::from_vb(250);
        assert_eq!(weight.value(), 1000);
        assert_eq!(weight.to_vb(), 250);
        
        let weight = Weight::new(1001);
        assert_eq!(weight.to_vb(), 251); // Rounds up
    }
    
    #[test]
    fn test_weight_estimation() {
        // 1 input, 2 outputs, non-segwit
        let weight = WeightCalculator::estimate_weight(1, 2, false);
        assert!(weight.value() > 0);
        
        // Segwit should be lighter
        let segwit_weight = WeightCalculator::estimate_weight(1, 2, true);
        assert!(segwit_weight.value() < weight.value());
    }
} 