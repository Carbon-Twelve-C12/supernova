//! Safe arithmetic operations for preventing integer overflows
//!
//! This module provides checked arithmetic operations that are used throughout
//! the codebase to prevent integer overflow vulnerabilities, particularly in
//! fee calculations where overflows could lead to negative fees or economic attacks.

use std::fmt;

/// Error type for arithmetic operations
#[derive(Debug, Clone, PartialEq)]
pub enum ArithmeticError {
    /// Addition overflow
    AdditionOverflow,
    /// Subtraction overflow
    SubtractionOverflow,
    /// Multiplication overflow
    MultiplicationOverflow,
    /// Division by zero
    DivisionByZero,
}

impl fmt::Display for ArithmeticError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ArithmeticError::AdditionOverflow => write!(f, "Addition overflow"),
            ArithmeticError::SubtractionOverflow => write!(f, "Subtraction overflow"),
            ArithmeticError::MultiplicationOverflow => write!(f, "Multiplication overflow"),
            ArithmeticError::DivisionByZero => write!(f, "Division by zero"),
        }
    }
}

impl std::error::Error for ArithmeticError {}

/// Safe addition that returns an error on overflow
pub fn safe_add(a: u64, b: u64) -> Result<u64, ArithmeticError> {
    a.checked_add(b).ok_or(ArithmeticError::AdditionOverflow)
}

/// Safe subtraction that returns an error on underflow
pub fn safe_sub(a: u64, b: u64) -> Result<u64, ArithmeticError> {
    a.checked_sub(b).ok_or(ArithmeticError::SubtractionOverflow)
}

/// Safe multiplication that returns an error on overflow
pub fn safe_mul(a: u64, b: u64) -> Result<u64, ArithmeticError> {
    a.checked_mul(b)
        .ok_or(ArithmeticError::MultiplicationOverflow)
}

/// Safe division that returns an error on division by zero
pub fn safe_div(a: u64, b: u64) -> Result<u64, ArithmeticError> {
    if b == 0 {
        Err(ArithmeticError::DivisionByZero)
    } else {
        Ok(a / b)
    }
}

/// Calculate fee from fee rate and size with overflow protection
pub fn calculate_fee_safe(fee_rate: u64, size: usize) -> Result<u64, ArithmeticError> {
    let size_u64 = size as u64;
    safe_mul(fee_rate, size_u64)
}

/// Calculate total from iterator with overflow protection
pub fn sum_safe<I>(mut iter: I) -> Result<u64, ArithmeticError>
where
    I: Iterator<Item = u64>,
{
    iter.try_fold(0u64, safe_add)
}

/// Safe percentage calculation (value * percentage / 100)
pub fn percentage_safe(value: u64, percentage: u64) -> Result<u64, ArithmeticError> {
    let product = safe_mul(value, percentage)?;
    safe_div(product, 100)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_safe_add() {
        assert_eq!(safe_add(100, 200), Ok(300));
        assert_eq!(
            safe_add(u64::MAX, 1),
            Err(ArithmeticError::AdditionOverflow)
        );
        assert_eq!(safe_add(u64::MAX - 10, 10), Ok(u64::MAX));
    }

    #[test]
    fn test_safe_sub() {
        assert_eq!(safe_sub(300, 200), Ok(100));
        assert_eq!(
            safe_sub(100, 200),
            Err(ArithmeticError::SubtractionOverflow)
        );
        assert_eq!(safe_sub(100, 100), Ok(0));
    }

    #[test]
    fn test_safe_mul() {
        assert_eq!(safe_mul(100, 200), Ok(20000));
        assert_eq!(
            safe_mul(u64::MAX, 2),
            Err(ArithmeticError::MultiplicationOverflow)
        );
        assert_eq!(safe_mul(1000, 0), Ok(0));
    }

    #[test]
    fn test_safe_div() {
        assert_eq!(safe_div(1000, 10), Ok(100));
        assert_eq!(safe_div(1000, 0), Err(ArithmeticError::DivisionByZero));
        assert_eq!(safe_div(0, 10), Ok(0));
    }

    #[test]
    fn test_calculate_fee_safe() {
        // Normal fee calculation
        assert_eq!(calculate_fee_safe(100, 250), Ok(25000));

        // Overflow protection
        assert_eq!(
            calculate_fee_safe(u64::MAX, 2),
            Err(ArithmeticError::MultiplicationOverflow)
        );
    }

    #[test]
    fn test_sum_safe() {
        let values = vec![100, 200, 300];
        assert_eq!(sum_safe(values.into_iter()), Ok(600));

        let overflow_values = vec![u64::MAX - 100, 200];
        assert_eq!(
            sum_safe(overflow_values.into_iter()),
            Err(ArithmeticError::AdditionOverflow)
        );
    }

    #[test]
    fn test_percentage_safe() {
        assert_eq!(percentage_safe(1000, 10), Ok(100)); // 10% of 1000
        assert_eq!(percentage_safe(500, 50), Ok(250)); // 50% of 500

        // Overflow protection
        assert_eq!(
            percentage_safe(u64::MAX, 200),
            Err(ArithmeticError::MultiplicationOverflow)
        );
    }
}
