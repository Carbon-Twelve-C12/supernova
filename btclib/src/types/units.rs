//! NOVA currency units and conversions
//!
//! This module defines the standard units for NOVA currency and provides
//! conversion utilities. The base unit is the "nova" (lowercase), with
//! 1 NOVA = 100,000,000 novas (similar to Bitcoin's satoshi structure).

use std::fmt;
use std::convert::TryFrom;
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

/// The number of novas in one NOVA
pub const NOVAS_PER_NOVA: u64 = 100_000_000;

/// The smallest unit of NOVA currency (1/100,000,000 NOVA)
pub type Novas = u64;

/// Represents an amount in NOVA currency
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Amount {
    /// Amount in novas (smallest unit)
    novas: u64,
}

impl Amount {
    /// Create a new Amount from novas
    pub const fn from_novas(novas: u64) -> Self {
        Self { novas }
    }
    
    /// Create a new Amount from NOVA
    pub fn from_nova(nova: f64) -> Self {
        Self {
            novas: (nova * NOVAS_PER_NOVA as f64) as u64,
        }
    }
    
    /// Get the amount in novas
    pub const fn as_novas(&self) -> u64 {
        self.novas
    }
    
    /// Get the amount in NOVA
    pub fn as_nova(&self) -> f64 {
        self.novas as f64 / NOVAS_PER_NOVA as f64
    }
    
    /// Check if the amount is zero
    pub const fn is_zero(&self) -> bool {
        self.novas == 0
    }
    
    /// The zero amount
    pub const fn zero() -> Self {
        Self { novas: 0 }
    }
    
    /// Add two amounts
    pub fn checked_add(&self, other: Self) -> Option<Self> {
        self.novas.checked_add(other.novas).map(Self::from_novas)
    }
    
    /// Subtract two amounts
    pub fn checked_sub(&self, other: Self) -> Option<Self> {
        self.novas.checked_sub(other.novas).map(Self::from_novas)
    }
    
    /// Multiply by a scalar
    pub fn checked_mul(&self, scalar: u64) -> Option<Self> {
        self.novas.checked_mul(scalar).map(Self::from_novas)
    }
    
    /// Divide by a scalar
    pub fn checked_div(&self, scalar: u64) -> Option<Self> {
        if scalar == 0 {
            None
        } else {
            Some(Self::from_novas(self.novas / scalar))
        }
    }
}

impl fmt::Display for Amount {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.novas == 0 {
            write!(f, "0 NOVA")
        } else if self.novas % NOVAS_PER_NOVA == 0 {
            write!(f, "{} NOVA", self.novas / NOVAS_PER_NOVA)
        } else {
            write!(f, "{:.8} NOVA", self.as_nova())
        }
    }
}

/// Fee rate in novas per byte
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct FeeRate {
    /// Fee in novas per byte
    novas_per_byte: u64,
}

impl FeeRate {
    /// Create a new fee rate
    pub const fn from_novas_per_byte(novas_per_byte: u64) -> Self {
        Self { novas_per_byte }
    }
    
    /// Get the fee rate in novas per byte
    pub const fn as_novas_per_byte(&self) -> u64 {
        self.novas_per_byte
    }
    
    /// Calculate the fee for a given size
    pub fn calculate_fee(&self, size_bytes: usize) -> Amount {
        Amount::from_novas(self.novas_per_byte * size_bytes as u64)
    }
}

impl fmt::Display for FeeRate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} novas/byte", self.novas_per_byte)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_amount_conversions() {
        let amount = Amount::from_nova(1.0);
        assert_eq!(amount.as_novas(), NOVAS_PER_NOVA);
        assert_eq!(amount.as_nova(), 1.0);
        
        let amount = Amount::from_novas(50_000_000);
        assert_eq!(amount.as_nova(), 0.5);
    }
    
    #[test]
    fn test_amount_arithmetic() {
        let a = Amount::from_nova(1.5);
        let b = Amount::from_nova(0.5);
        
        assert_eq!(a.checked_add(b), Some(Amount::from_nova(2.0)));
        assert_eq!(a.checked_sub(b), Some(Amount::from_nova(1.0)));
        assert_eq!(b.checked_mul(3), Some(Amount::from_nova(1.5)));
        assert_eq!(a.checked_div(3), Some(Amount::from_nova(0.5)));
    }
    
    #[test]
    fn test_fee_rate() {
        let fee_rate = FeeRate::from_novas_per_byte(100);
        let fee = fee_rate.calculate_fee(250); // 250 byte transaction
        assert_eq!(fee.as_novas(), 25_000);
    }
    
    #[test]
    fn test_display() {
        assert_eq!(Amount::from_nova(1.0).to_string(), "1 NOVA");
        assert_eq!(Amount::from_nova(1.5).to_string(), "1.50000000 NOVA");
        assert_eq!(Amount::zero().to_string(), "0 NOVA");
        
        assert_eq!(FeeRate::from_novas_per_byte(100).to_string(), "100 novas/byte");
    }
} 