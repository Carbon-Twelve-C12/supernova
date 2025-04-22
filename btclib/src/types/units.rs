// SuperNova currency units and conversion utilities
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

/// SuperNova currency units
/// 
/// The base unit is the Nova (NOVA), with various subdivisions and multiples:
/// 
/// * 1 MegaNova = 1,000,000 NOVA
/// * 1 KiloNova = 1,000 NOVA
/// * 1 NOVA = the base unit
/// * 1 MilliNova = 0.001 NOVA
/// * 1 MicroNova = 0.000001 NOVA
/// * 1 NanoNova = 0.000000001 NOVA
/// * 1 PicoNova = 0.000000000001 NOVA
/// * 1 FemtoNova = 0.000000000000001 NOVA
/// * 1 AttoNova = 0.000000000000000001 NOVA (smallest unit)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NovaUnit {
    /// 1,000,000 NOVA
    MegaNova,
    /// 1,000 NOVA
    KiloNova,
    /// Base unit (1 NOVA)
    Nova,
    /// 0.001 NOVA
    MilliNova,
    /// 0.000001 NOVA
    MicroNova,
    /// 0.000000001 NOVA
    NanoNova,
    /// 0.000000000001 NOVA
    PicoNova,
    /// 0.000000000000001 NOVA
    FemtoNova,
    /// 0.000000000000000001 NOVA (smallest practical unit)
    AttoNova,
}

impl NovaUnit {
    /// Get the canonical name of this unit
    pub fn name(&self) -> &'static str {
        match self {
            Self::MegaNova => "MegaNova",
            Self::KiloNova => "KiloNova",
            Self::Nova => "NOVA",
            Self::MilliNova => "MilliNova",
            Self::MicroNova => "MicroNova",
            Self::NanoNova => "NanoNova",
            Self::PicoNova => "PicoNova",
            Self::FemtoNova => "FemtoNova",
            Self::AttoNova => "AttoNova",
        }
    }
    
    /// Get the common abbreviation for this unit
    pub fn abbreviation(&self) -> &'static str {
        match self {
            Self::MegaNova => "MNOVA",
            Self::KiloNova => "kNOVA",
            Self::Nova => "NOVA",
            Self::MilliNova => "mNOVA",
            Self::MicroNova => "μNOVA",
            Self::NanoNova => "nNOVA",
            Self::PicoNova => "pNOVA",
            Self::FemtoNova => "fNOVA",
            Self::AttoNova => "aNOVA",
        }
    }
    
    /// Get the conversion factor to convert from this unit to AttoNova (smallest unit)
    pub fn to_attonova_factor(&self) -> u128 {
        match self {
            Self::MegaNova => 1_000_000_000_000_000_000_000_000,
            Self::KiloNova => 1_000_000_000_000_000_000_000,
            Self::Nova => 1_000_000_000_000_000_000,
            Self::MilliNova => 1_000_000_000_000_000,
            Self::MicroNova => 1_000_000_000_000,
            Self::NanoNova => 1_000_000_000,
            Self::PicoNova => 1_000_000,
            Self::FemtoNova => 1_000,
            Self::AttoNova => 1,
        }
    }
    
    /// Convert a value in this unit to AttoNova (smallest unit)
    pub fn to_attonova(&self, value: u64) -> Result<u128, UnitError> {
        let factor = self.to_attonova_factor();
        value.checked_mul(factor as u64)
            .map(|v| v as u128)
            .ok_or(UnitError::ConversionOverflow)
    }
    
    /// Convert a value in AttoNova to this unit
    pub fn from_attonova(&self, attonovas: u128) -> Result<u64, UnitError> {
        let factor = self.to_attonova_factor();
        if attonovas % factor != 0 {
            return Err(UnitError::ConversionUnderflow);
        }
        
        let result = attonovas / factor;
        if result > u64::MAX as u128 {
            return Err(UnitError::ConversionOverflow);
        }
        
        Ok(result as u64)
    }
    
    /// Convert a value from this unit to another unit
    pub fn convert(&self, value: u64, target_unit: &Self) -> Result<u64, UnitError> {
        // Convert to attonovas first, then to the target unit
        let attonovas = self.to_attonova(value)?;
        target_unit.from_attonova(attonovas)
    }
    
    /// Format a value in this unit with the appropriate symbol
    pub fn format(&self, value: u64) -> String {
        format!("{} {}", value, self.abbreviation())
    }
    
    /// Format a value in this unit with up to 8 decimal places
    pub fn format_decimal(&self, value: f64) -> String {
        format!("{:.8} {}", value, self.abbreviation())
    }
}

impl FromStr for NovaUnit {
    type Err = UnitError;
    
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "meganove" | "mnova" => Ok(Self::MegaNova),
            "kilonova" | "knova" => Ok(Self::KiloNova),
            "nova" => Ok(Self::Nova),
            "millinova" | "mnova" => Ok(Self::MilliNova),
            "micronova" | "μnova" => Ok(Self::MicroNova),
            "nanonova" | "nnova" => Ok(Self::NanoNova),
            "piconova" | "pnova" => Ok(Self::PicoNova),
            "femtonova" | "fnova" => Ok(Self::FemtoNova),
            "attonova" | "anova" => Ok(Self::AttoNova),
            _ => Err(UnitError::InvalidFormat(s.to_string())),
        }
    }
}

impl fmt::Display for NovaUnit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name())
    }
}

/// The total supply of NOVA
pub const TOTAL_NOVA_SUPPLY: u64 = 42_000_000;

/// Convert a value from a specific unit to millinovas (our primary internal unit)
pub fn to_millinovas(value: u64, unit: NovaUnit) -> Result<u64, UnitError> {
    unit.convert(value, &NovaUnit::MilliNova)
}

/// Convert millinovas to another unit with proper formatting
pub fn from_millinovas(millinovas: u64, target_unit: NovaUnit) -> Result<String, UnitError> {
    let converted = NovaUnit::MilliNova.convert(millinovas, &target_unit)?;
    Ok(target_unit.format(converted))
}

/// Format a value in millinovas as NOVA with decimal precision
pub fn format_as_nova(millinovas: u64) -> String {
    let novas = millinovas as f64 / 1000.0;
    format!("{:.3} NOVA", novas)
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_unit_conversion() {
        // 1 NOVA = 1000 MilliNova
        let result = NovaUnit::Nova.convert(1, &NovaUnit::MilliNova).unwrap();
        assert_eq!(result, 1000);
        
        // 5000 MilliNova = 5 NOVA
        let result = NovaUnit::MilliNova.convert(5000, &NovaUnit::Nova).unwrap();
        assert_eq!(result, 5);
        
        // 1 MegaNova = 1,000,000 NOVA
        let result = NovaUnit::MegaNova.convert(1, &NovaUnit::Nova).unwrap();
        assert_eq!(result, 1_000_000);
        
        // Test precision loss
        let result = NovaUnit::MilliNova.convert(5, &NovaUnit::Nova);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), UnitError::ConversionUnderflow));
    }
    
    #[test]
    fn test_formatting() {
        assert_eq!(NovaUnit::Nova.format(42), "42 NOVA");
        assert_eq!(NovaUnit::MilliNova.format(500), "500 mNOVA");
        assert_eq!(NovaUnit::KiloNova.format(10), "10 kNOVA");
        
        assert_eq!(format_as_nova(1500), "1.500 NOVA");
        assert_eq!(format_as_nova(1_000_000), "1000.000 NOVA");
    }
    
    #[test]
    fn test_from_str() {
        assert_eq!(NovaUnit::from_str("nova").unwrap(), NovaUnit::Nova);
        assert_eq!(NovaUnit::from_str("millinova").unwrap(), NovaUnit::MilliNova);
        assert_eq!(NovaUnit::from_str("NOVA").unwrap(), NovaUnit::Nova);
        
        let result = NovaUnit::from_str("invalid");
        assert!(result.is_err());
    }
} 