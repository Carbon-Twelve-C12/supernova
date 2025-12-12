//! Comprehensive test suite for atomic swap functionality
//!
//! This module contains unit tests, integration tests, security tests,
//! and performance benchmarks for the atomic swap implementation.

#[cfg(test)]
pub mod unit_tests;

#[cfg(test)]
pub mod integration_tests;

#[cfg(test)]
pub mod security_tests;

#[cfg(test)]
pub mod performance_tests;

#[cfg(test)]
pub mod edge_case_tests;

#[cfg(test)]
pub mod hardening_tests;

// Re-export common test utilities
#[cfg(test)]
pub mod test_utils;