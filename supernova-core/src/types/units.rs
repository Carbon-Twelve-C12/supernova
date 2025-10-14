//! NOVA currency units and conversions
//!
//! This module defines the standard units for NOVA currency and provides
//! conversion utilities. The base unit is NOVA, with support for denominations
//! down to attaNOVA (10^-18 NOVA) using high-precision internal representation.

use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;
use thiserror::Error;

/// Error type for currency unit operations
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum UnitError {
    /// Invalid unit format or name
    #[error("Invalid unit format: {0}")]
    InvalidFormat(String),

    /// Conversion overflow
    #[error("Conversion overflow: value exceeds the target unit range")]
    ConversionOverflow,

    /// Conversion underflow (lost precision)
    #[error("Conversion underflow: value would lose precision in target unit")]
    ConversionUnderflow,
}

/// The number of attonovas in one NOVA (for internal precision)
pub const ATTONOVAS_PER_NOVA: u128 = 1_000_000_000_000_000_000; // 10^18

/// For backwards compatibility - represents the smallest user-facing unit per NOVA
/// 1 NOVA = 10^8 novas (following Bitcoin's model: 1 BTC = 10^8 satoshis)
pub const NOVAS_PER_NOVA: u64 = 100_000_000; // 10^8

/// Currency units for Supernova
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NovaUnit {
    /// The base unit (1 NOVA)
    Nova,
    /// milli-nova (0.001 NOVA)
    MilliNova,
    /// micro-nova (0.000001 NOVA)
    MicroNova,
    /// nano-nova (0.000000001 NOVA)
    NanoNova,
    /// pico-nova (0.000000000001 NOVA)
    PicoNova,
    /// femto-nova (0.000000000000001 NOVA)
    FemtoNova,
    /// atto-nova (0.000000000000000001 NOVA) - smallest representable unit
    AttaNova,
    /// kilo-nova (1,000 NOVA)
    KiloNova,
    /// mega-nova (1,000,000 NOVA)
    MegaNova,
}

impl NovaUnit {
    /// Get the scaling factor relative to NOVA (as f64 for precision)
    pub fn scaling_factor(&self) -> f64 {
        match self {
            NovaUnit::Nova => 1.0,
            NovaUnit::MilliNova => 0.001,
            NovaUnit::MicroNova => 0.000001,
            NovaUnit::NanoNova => 0.000000001,
            NovaUnit::PicoNova => 0.000000000001,
            NovaUnit::FemtoNova => 0.000000000000001,
            NovaUnit::AttaNova => 0.000000000000000001,
            NovaUnit::KiloNova => 1000.0,
            NovaUnit::MegaNova => 1000000.0,
        }
    }

    /// Get the number of attonovas per unit (for internal calculations)
    pub fn attonovas_per_unit(&self) -> u128 {
        match self {
            NovaUnit::Nova => ATTONOVAS_PER_NOVA,
            NovaUnit::MilliNova => ATTONOVAS_PER_NOVA / 1_000,
            NovaUnit::MicroNova => ATTONOVAS_PER_NOVA / 1_000_000,
            NovaUnit::NanoNova => ATTONOVAS_PER_NOVA / 1_000_000_000,
            NovaUnit::PicoNova => ATTONOVAS_PER_NOVA / 1_000_000_000_000,
            NovaUnit::FemtoNova => ATTONOVAS_PER_NOVA / 1_000_000_000_000_000,
            NovaUnit::AttaNova => 1,
            NovaUnit::KiloNova => ATTONOVAS_PER_NOVA.saturating_mul(1_000),
            NovaUnit::MegaNova => ATTONOVAS_PER_NOVA.saturating_mul(1_000_000),
        }
    }

    /// Get the decimal places for this unit relative to NOVA
    pub fn decimal_places(&self) -> i32 {
        match self {
            NovaUnit::Nova => 0,
            NovaUnit::MilliNova => 3,
            NovaUnit::MicroNova => 6,
            NovaUnit::NanoNova => 9,
            NovaUnit::PicoNova => 12,
            NovaUnit::FemtoNova => 15,
            NovaUnit::AttaNova => 18,
            NovaUnit::KiloNova => -3,
            NovaUnit::MegaNova => -6,
        }
    }

    /// Get the unit symbol
    pub fn symbol(&self) -> &'static str {
        match self {
            NovaUnit::Nova => "NOVA",
            NovaUnit::MilliNova => "mNOVA",
            NovaUnit::MicroNova => "μNOVA",
            NovaUnit::NanoNova => "nNOVA",
            NovaUnit::PicoNova => "pNOVA",
            NovaUnit::FemtoNova => "fNOVA",
            NovaUnit::AttaNova => "aNOVA",
            NovaUnit::KiloNova => "kNOVA",
            NovaUnit::MegaNova => "MNOVA",
        }
    }

    /// Get the unit name
    pub fn name(&self) -> &'static str {
        match self {
            NovaUnit::Nova => "nova",
            NovaUnit::MilliNova => "millinova",
            NovaUnit::MicroNova => "micronova",
            NovaUnit::NanoNova => "nanonova",
            NovaUnit::PicoNova => "piconova",
            NovaUnit::FemtoNova => "femtonova",
            NovaUnit::AttaNova => "attonova",
            NovaUnit::KiloNova => "kilonova",
            NovaUnit::MegaNova => "meganova",
        }
    }
}

impl fmt::Display for NovaUnit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.symbol())
    }
}

impl FromStr for NovaUnit {
    type Err = UnitError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "nova" => Ok(NovaUnit::Nova),
            "millinova" | "mnova" => Ok(NovaUnit::MilliNova),
            "micronova" | "μnova" | "unova" => Ok(NovaUnit::MicroNova),
            "nanonova" | "nnova" => Ok(NovaUnit::NanoNova),
            "piconova" | "pnova" => Ok(NovaUnit::PicoNova),
            "femtonova" | "fnova" => Ok(NovaUnit::FemtoNova),
            "attonova" | "anova" => Ok(NovaUnit::AttaNova),
            "kilonova" | "knova" => Ok(NovaUnit::KiloNova),
            "meganova" => Ok(NovaUnit::MegaNova), // Use full name for mega to avoid confusion with milli
            _ => Err(UnitError::InvalidFormat(s.to_string())),
        }
    }
}

/// The smallest unit of NOVA currency (1/10^8 NOVA) - for backwards compatibility
pub type Novas = u64;

/// Internal representation using attanovas for maximum precision
pub type Attonovas = u128;

/// Represents an amount in NOVA currency
/// Internally stores the value in attanovas (10^-18 NOVA) for maximum precision
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Amount {
    /// Amount in attonovas (10^-18 NOVA) for internal precision
    attonovas: u128,
}

impl Amount {
    /// Create a new Amount from attonovas (smallest unit)
    pub const fn from_attonovas(attonovas: u128) -> Self {
        Self { attonovas }
    }

    /// Create a new Amount from novas (smallest user-facing unit) for backwards compatibility
    /// 1 NOVA = 10^8 novas (following Bitcoin's model: 1 BTC = 10^8 satoshis)
    pub const fn from_novas(novas: u64) -> Self {
        // 1 nova = 10^10 attonovas (since 1 NOVA = 10^18 attonovas and 1 NOVA = 10^8 novas)
        Self {
            attonovas: (novas as u128) * 10_000_000_000,
        }
    }

    /// Create a new Amount from NOVA (the base unit)
    pub fn from_nova(nova: f64) -> Self {
        Self {
            attonovas: (nova * ATTONOVAS_PER_NOVA as f64) as u128,
        }
    }

    /// Create a new Amount from a value in the specified unit
    pub fn from_unit(value: f64, unit: NovaUnit) -> Result<Self, UnitError> {
        let attonovas = (value * unit.attonovas_per_unit() as f64) as u128;
        Ok(Self { attonovas })
    }

    /// Get the amount in attonovas (smallest representable unit)
    pub const fn as_attonovas(&self) -> u128 {
        self.attonovas
    }

    /// Get the amount in novas (smallest user-facing unit) for backwards compatibility
    /// Note: This may lose precision for amounts smaller than 1 nova
    /// 1 NOVA = 10^8 novas (following Bitcoin's model)
    pub const fn as_novas(&self) -> u64 {
        // 1 nova = 10^10 attonovas
        (self.attonovas / 10_000_000_000) as u64
    }

    /// Get the amount in NOVA (the base unit)
    pub fn as_nova(&self) -> f64 {
        self.attonovas as f64 / ATTONOVAS_PER_NOVA as f64
    }

    /// Get the amount in the specified unit
    pub fn as_unit(&self, unit: NovaUnit) -> f64 {
        self.attonovas as f64 / unit.attonovas_per_unit() as f64
    }

    /// Format the amount with the specified unit
    pub fn format_with_unit(&self, unit: NovaUnit) -> String {
        let value = self.as_unit(unit);
        // Use scientific notation for very large or very small values
        if value >= 1e15 || (value > 0.0 && value < 1e-15) {
            format!("{:e} {}", value, unit.symbol())
        } else {
            format!("{} {}", value, unit.symbol())
        }
    }

    /// Check if the amount is zero
    pub const fn is_zero(&self) -> bool {
        self.attonovas == 0
    }

    /// The zero amount
    pub const fn zero() -> Self {
        Self { attonovas: 0 }
    }

    /// Add two amounts
    pub fn checked_add(&self, other: Self) -> Option<Self> {
        self.attonovas
            .checked_add(other.attonovas)
            .map(Self::from_attonovas)
    }

    /// Subtract two amounts
    pub fn checked_sub(&self, other: Self) -> Option<Self> {
        self.attonovas
            .checked_sub(other.attonovas)
            .map(Self::from_attonovas)
    }

    /// Multiply by a scalar
    pub fn checked_mul(&self, scalar: u64) -> Option<Self> {
        self.attonovas
            .checked_mul(scalar as u128)
            .map(Self::from_attonovas)
    }

    /// Divide by a scalar
    pub fn checked_div(&self, scalar: u64) -> Option<Self> {
        if scalar == 0 {
            None
        } else {
            Some(Self::from_attonovas(self.attonovas / scalar as u128))
        }
    }
}

impl fmt::Display for Amount {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let nova_value = self.as_nova();

        if self.attonovas == 0 {
            write!(f, "0 NOVA")
        } else if self.attonovas % ATTONOVAS_PER_NOVA == 0 {
            // Whole number of NOVA
            write!(f, "{} NOVA", self.attonovas / ATTONOVAS_PER_NOVA)
        } else if nova_value >= 0.01 {
            // For amounts >= 0.01 NOVA, show 8 decimal places
            write!(f, "{:.8} NOVA", nova_value)
        } else {
            // For smaller amounts, show up to 18 decimal places
            write!(f, "{:.18} NOVA", nova_value)
        }
    }
}

/// Fee rate in attonovas per byte
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct FeeRate {
    /// Fee in attonovas per byte
    attonovas_per_byte: u128,
}

impl FeeRate {
    /// Create a new fee rate from attonovas per byte
    pub const fn from_attonovas_per_byte(attonovas_per_byte: u128) -> Self {
        Self { attonovas_per_byte }
    }

    /// Create a new fee rate from novas per byte (for backwards compatibility)
    /// 1 nova = 10^10 attonovas (following Bitcoin's fee model)
    pub const fn from_novas_per_byte(novas_per_byte: u64) -> Self {
        Self {
            attonovas_per_byte: (novas_per_byte as u128) * 10_000_000_000,
        }
    }

    /// Get the fee rate in attonovas per byte
    pub const fn as_attonovas_per_byte(&self) -> u128 {
        self.attonovas_per_byte
    }

    /// Get the fee rate in novas per byte (for backwards compatibility)
    /// 1 nova = 10^10 attonovas
    pub const fn as_novas_per_byte(&self) -> u64 {
        (self.attonovas_per_byte / 10_000_000_000) as u64
    }

    /// Calculate the fee for a given size
    pub fn calculate_fee(&self, size_bytes: usize) -> Amount {
        Amount::from_attonovas(self.attonovas_per_byte * size_bytes as u128)
    }
}

impl fmt::Display for FeeRate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.attonovas_per_byte >= 10_000_000_000 {
            write!(f, "{} novas/byte", self.as_novas_per_byte())
        } else {
            write!(f, "{} attonovas/byte", self.attonovas_per_byte)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_amount_conversions() {
        // Test NOVA as base unit
        let amount = Amount::from_nova(1.0);
        assert_eq!(amount.as_attonovas(), ATTONOVAS_PER_NOVA);
        assert_eq!(amount.as_nova(), 1.0);

        // Test backwards compatibility with novas (nanoNOVAs)
        assert_eq!(amount.as_novas(), NOVAS_PER_NOVA);

        let amount = Amount::from_attonovas(500_000_000_000_000_000);
        assert_eq!(amount.as_nova(), 0.5);

        // Test from_novas backwards compatibility
        let amount = Amount::from_novas(50_000_000); // 0.5 NOVA in old novas
        assert_eq!(amount.as_nova(), 0.5);
    }

    #[test]
    fn test_nova_unit_conversions() {
        // Test from_unit - all units work including attaNOVA
        let amount = Amount::from_unit(1000.0, NovaUnit::MilliNova).unwrap();
        assert_eq!(amount.as_nova(), 1.0);

        let amount = Amount::from_unit(1_000_000.0, NovaUnit::MicroNova).unwrap();
        assert_eq!(amount.as_nova(), 1.0);

        let amount = Amount::from_unit(1_000_000_000.0, NovaUnit::NanoNova).unwrap();
        assert_eq!(amount.as_nova(), 1.0);

        // Test sub-nanoNOVA units (now supported!)
        let amount = Amount::from_unit(1e12, NovaUnit::PicoNova).unwrap();
        assert_eq!(amount.as_nova(), 1.0);

        let amount = Amount::from_unit(1e15, NovaUnit::FemtoNova).unwrap();
        assert_eq!(amount.as_nova(), 1.0);

        let amount = Amount::from_unit(1e18, NovaUnit::AttaNova).unwrap();
        assert_eq!(amount.as_nova(), 1.0);

        let amount = Amount::from_unit(0.001, NovaUnit::KiloNova).unwrap();
        assert_eq!(amount.as_nova(), 1.0);

        // Test as_unit
        let amount = Amount::from_nova(1.0);
        assert_eq!(amount.as_unit(NovaUnit::Nova), 1.0);
        assert_eq!(amount.as_unit(NovaUnit::MilliNova), 1000.0);
        assert_eq!(amount.as_unit(NovaUnit::MicroNova), 1_000_000.0);
        assert_eq!(amount.as_unit(NovaUnit::NanoNova), 1_000_000_000.0);
        assert_eq!(amount.as_unit(NovaUnit::PicoNova), 1e12);
        assert_eq!(amount.as_unit(NovaUnit::FemtoNova), 1e15);
        assert_eq!(amount.as_unit(NovaUnit::AttaNova), 1e18);
        assert_eq!(amount.as_unit(NovaUnit::KiloNova), 0.001);
    }

    #[test]
    fn test_nova_unit_formatting() {
        let amount = Amount::from_nova(1.5);

        assert_eq!(amount.format_with_unit(NovaUnit::Nova), "1.5 NOVA");
        assert_eq!(amount.format_with_unit(NovaUnit::MilliNova), "1500 mNOVA");
        assert_eq!(
            amount.format_with_unit(NovaUnit::MicroNova),
            "1500000 μNOVA"
        );
        assert_eq!(amount.format_with_unit(NovaUnit::AttaNova), "1.5e18 aNOVA");
    }

    #[test]
    fn test_nova_unit_from_str() {
        assert_eq!(NovaUnit::from_str("nova").unwrap(), NovaUnit::Nova);
        assert_eq!(NovaUnit::from_str("NOVA").unwrap(), NovaUnit::Nova);
        assert_eq!(
            NovaUnit::from_str("millinova").unwrap(),
            NovaUnit::MilliNova
        );
        assert_eq!(NovaUnit::from_str("mNOVA").unwrap(), NovaUnit::MilliNova);
        assert_eq!(NovaUnit::from_str("meganova").unwrap(), NovaUnit::MegaNova);
        assert_eq!(NovaUnit::from_str("MNOVA").unwrap(), NovaUnit::MilliNova); // M/m prefix means milli- in SI
        assert_eq!(NovaUnit::from_str("attonova").unwrap(), NovaUnit::AttaNova);
        assert_eq!(NovaUnit::from_str("aNOVA").unwrap(), NovaUnit::AttaNova);

        assert!(NovaUnit::from_str("invalid").is_err());
    }

    #[test]
    fn test_smallest_unit_precision() {
        // Test that we can represent 1 attaNOVA
        let one_atto = Amount::from_attonovas(1);
        assert_eq!(one_atto.as_attonovas(), 1);
        assert_eq!(one_atto.as_unit(NovaUnit::AttaNova), 1.0);
        assert_eq!(one_atto.as_nova(), 1e-18);

        // Test creating small amounts
        let small_amount = Amount::from_unit(123.456, NovaUnit::AttaNova).unwrap();
        assert_eq!(small_amount.as_attonovas(), 123);

        // Test that we maintain precision
        let precise_amount = Amount::from_nova(0.000000000000000001); // 1 attaNOVA
        assert_eq!(precise_amount.as_attonovas(), 1);

        // Test very small amounts
        let tiny = Amount::from_unit(1.0, NovaUnit::AttaNova).unwrap();
        assert_eq!(tiny.as_nova(), 1e-18);
        assert_eq!(tiny.as_unit(NovaUnit::AttaNova), 1.0);
    }

    #[test]
    fn test_backwards_compatibility() {
        // Test that from_novas still works
        let amount = Amount::from_novas(100_000_000); // 1 NOVA in old novas
        assert_eq!(amount.as_nova(), 1.0);
        assert_eq!(amount.as_novas(), 100_000_000);

        // Test fee rate backwards compatibility
        let fee_rate = FeeRate::from_novas_per_byte(100);
        assert_eq!(fee_rate.as_novas_per_byte(), 100);
        assert_eq!(fee_rate.as_attonovas_per_byte(), 1_000_000_000_000); // 100 * 10^10

        // Test precision loss warning for as_novas()
        let small = Amount::from_unit(500.0, NovaUnit::PicoNova).unwrap();
        assert_eq!(small.as_novas(), 0); // Lost precision - too small for novas
        assert_eq!(small.as_attonovas(), 500_000_000); // But preserved in attonovas
    }

    #[test]
    fn test_amount_arithmetic() {
        let a = Amount::from_nova(1.5);
        let b = Amount::from_nova(0.5);

        assert_eq!(a.checked_add(b), Some(Amount::from_nova(2.0)));
        assert_eq!(a.checked_sub(b), Some(Amount::from_nova(1.0)));
        assert_eq!(b.checked_mul(3), Some(Amount::from_nova(1.5)));
        assert_eq!(a.checked_div(3), Some(Amount::from_nova(0.5)));

        // Test with very small amounts
        let tiny_a = Amount::from_unit(100.0, NovaUnit::AttaNova).unwrap();
        let tiny_b = Amount::from_unit(50.0, NovaUnit::AttaNova).unwrap();

        assert_eq!(
            tiny_a.checked_add(tiny_b),
            Some(Amount::from_attonovas(150))
        );
        assert_eq!(tiny_a.checked_sub(tiny_b), Some(Amount::from_attonovas(50)));
    }

    #[test]
    fn test_fee_rate() {
        // Test with traditional novas per byte
        let fee_rate = FeeRate::from_novas_per_byte(100);
        let fee = fee_rate.calculate_fee(250); // 250 byte transaction
        assert_eq!(fee.as_novas(), 25_000);

        // Test with attonovas per byte for micro-fees
        let micro_fee_rate = FeeRate::from_attonovas_per_byte(1_000_000); // 0.000001 novas/byte
        let micro_fee = micro_fee_rate.calculate_fee(1000);
        assert_eq!(micro_fee.as_attonovas(), 1_000_000_000);
        assert_eq!(micro_fee.as_nova(), 0.000000001); // 1 nanoNOVA total
    }

    #[test]
    fn test_display() {
        // Whole NOVAs
        assert_eq!(Amount::from_nova(1.0).to_string(), "1 NOVA");
        assert_eq!(Amount::from_nova(100.0).to_string(), "100 NOVA");

        // Fractional NOVAs (8 decimals for amounts >= 0.01)
        assert_eq!(Amount::from_nova(1.5).to_string(), "1.50000000 NOVA");
        assert_eq!(Amount::from_nova(0.12345678).to_string(), "0.12345678 NOVA");

        // Very small amounts (18 decimals)
        assert_eq!(
            Amount::from_nova(0.000000001).to_string(),
            "0.000000001000000000 NOVA"
        );
        assert_eq!(
            Amount::from_unit(1.0, NovaUnit::AttaNova)
                .unwrap()
                .to_string(),
            "0.000000000000000001 NOVA"
        );

        // Zero
        assert_eq!(Amount::zero().to_string(), "0 NOVA");

        // Fee rates
        assert_eq!(
            FeeRate::from_novas_per_byte(100).to_string(),
            "100 novas/byte"
        );
        assert_eq!(
            FeeRate::from_attonovas_per_byte(100).to_string(),
            "100 attonovas/byte"
        );
    }
}
