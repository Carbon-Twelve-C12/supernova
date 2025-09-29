//! Mining module for Supernova node
//!
//! This module provides mining-related functionality including
//! secure difficulty adjustment to prevent manipulation attacks.

pub mod difficulty_security;

#[cfg(test)]
mod difficulty_attack_tests;

pub use difficulty_security::{
    SecureDifficultyAdjuster,
    SecureDifficultyError,
    DifficultySecurityConfig,
    DifficultyStatistics,
    BlockInfo,
};