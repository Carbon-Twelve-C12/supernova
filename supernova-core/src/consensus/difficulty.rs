use crate::consensus::timestamp_validation::{TimestampValidationError, TimestampValidator};
use std::cmp::{max, min};
use thiserror::Error;

/// Target time between blocks for mainnet in seconds (2.5 minutes)
pub const MAINNET_BLOCK_TIME_TARGET: u64 = 150;

/// Target time between blocks for testnet in seconds (2.5 minutes)
pub const TESTNET_BLOCK_TIME_TARGET: u64 = 150;

/// Legacy constant for backward compatibility (mainnet default)
pub const BLOCK_TIME_TARGET: u64 = MAINNET_BLOCK_TIME_TARGET;

/// Network types for block time configuration
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NetworkType {
    Mainnet,
    Testnet,
    Regtest,
}

/// Get the target block time for a specific network
pub fn get_target_block_time(network: NetworkType) -> u64 {
    match network {
        NetworkType::Mainnet => MAINNET_BLOCK_TIME_TARGET,
        NetworkType::Testnet => TESTNET_BLOCK_TIME_TARGET,
        NetworkType::Regtest => 30, // Fast blocks for testing
    }
}

/// Get the difficulty adjustment interval for a specific network
pub fn get_difficulty_adjustment_interval(network: NetworkType) -> u64 {
    match network {
        NetworkType::Mainnet => 2016, // ~3.5 days with 2.5-minute blocks
        NetworkType::Testnet => 2016, // ~3.5 days with 2.5-minute blocks
        NetworkType::Regtest => 144,  // ~1.2 hours with 30-second blocks
    }
}

/// Type alias for DifficultyAdjuster (same as DifficultyAdjustment)
pub type DifficultyAdjuster = DifficultyAdjustment;

/// Errors related to difficulty adjustment
#[derive(Debug, Error)]
pub enum DifficultyAdjustmentError {
    #[error("Insufficient block history (need at least {0} blocks)")]
    InsufficientHistory(usize),

    #[error("Invalid block timestamp: {0}")]
    InvalidTimestamp(String),

    #[error("Target would exceed minimum allowed difficulty")]
    ExceedsMaximumTarget,

    #[error("Target would be lower than maximum allowed difficulty")]
    BelowMinimumTarget,

    #[error("Invalid calculation: {0}")]
    InvalidCalculation(String),

    #[error("Timestamp validation failed: {0}")]
    TimestampValidation(#[from] TimestampValidationError),
}

/// Configuration for difficulty adjustment algorithm
#[derive(Debug, Clone)]
pub struct DifficultyAdjustmentConfig {
    /// Number of blocks between difficulty adjustments
    pub adjustment_interval: u64,

    /// Target time between blocks in seconds
    pub target_block_time: u64,

    /// Maximum allowed target (minimum difficulty)
    pub max_target: u32,

    /// Minimum allowed target (maximum difficulty)
    pub min_target: u32,

    /// Dampening factor to reduce oscillations (1.0 = no dampening)
    pub dampening_factor: f64,

    /// Maximum upward adjustment factor
    pub max_upward_adjustment: f64,

    /// Maximum downward adjustment factor
    pub max_downward_adjustment: f64,

    /// Whether to use a weighted time calculation
    pub use_weighted_timespan: bool,

    /// Use median-of-three for timestamps to prevent time-warp attacks
    pub use_median_time_past: bool,

    /// Enable strict timestamp validation
    pub validate_timestamps: bool,
}

impl Default for DifficultyAdjustmentConfig {
    fn default() -> Self {
        Self::for_network(NetworkType::Mainnet)
    }
}

impl DifficultyAdjustmentConfig {
    /// Create a configuration for a specific network
    pub fn for_network(network: NetworkType) -> Self {
        Self {
            adjustment_interval: get_difficulty_adjustment_interval(network),
            target_block_time: get_target_block_time(network),
            max_target: 0x1e0fffff,       // Minimum difficulty
            min_target: 0x1b00ffff,       // Maximum difficulty
            dampening_factor: 4.0,        // Reduce oscillations
            max_upward_adjustment: 4.0,   // Max 4x difficulty decrease
            max_downward_adjustment: 4.0, // Max 4x difficulty increase
            use_weighted_timespan: true,
            use_median_time_past: true,
            validate_timestamps: true, // Enable timestamp validation by default
        }
    }
}

/// Manages consensus rules for difficulty adjustment
pub struct DifficultyAdjustment {
    config: DifficultyAdjustmentConfig,
    timestamp_validator: TimestampValidator,
}

impl Default for DifficultyAdjustment {
    fn default() -> Self {
        Self::new()
    }
}

impl DifficultyAdjustment {
    /// Create a new difficulty adjustment manager with default configuration (mainnet)
    pub fn new() -> Self {
        Self::for_network(NetworkType::Mainnet)
    }

    /// Create a new difficulty adjustment manager for a specific network
    pub fn for_network(network: NetworkType) -> Self {
        Self {
            config: DifficultyAdjustmentConfig::for_network(network),
            timestamp_validator: TimestampValidator::new(),
        }
    }

    /// Create a new difficulty adjustment manager with custom configuration
    pub fn with_config(config: DifficultyAdjustmentConfig) -> Self {
        Self {
            config,
            timestamp_validator: TimestampValidator::new(),
        }
    }

    /// Get the configuration
    pub fn config(&self) -> &DifficultyAdjustmentConfig {
        &self.config
    }

    /// Calculate the next target difficulty based on the history of block timestamps
    pub fn calculate_next_target(
        &self,
        current_target: u32,
        block_timestamps: &[u64],
        block_heights: &[u64],
    ) -> Result<u32, DifficultyAdjustmentError> {
        // Check if we have enough blocks
        if block_timestamps.len() < 2 {
            return Err(DifficultyAdjustmentError::InsufficientHistory(2));
        }

        // Validate timestamps if enabled
        if self.config.validate_timestamps {
            self.timestamp_validator.validate_difficulty_timestamps(
                block_timestamps,
                block_heights,
                self.config.adjustment_interval,
            )?;
        }

        // Check if we're at an adjustment interval
        let latest_height = *block_heights
            .last()
            .ok_or(DifficultyAdjustmentError::InsufficientHistory(1))?;
        if latest_height % self.config.adjustment_interval != 0 && latest_height > 0 {
            // Not at an adjustment interval, so return the current target
            return Ok(current_target);
        }

        // Calculate the actual time span between the first and last block
        let actual_timespan = self.calculate_timespan(block_timestamps)?;

        // Calculate the target timespan
        let target_timespan = self.config.target_block_time * (block_timestamps.len() as u64 - 1);

        // Calculate adjustment ratio
        let mut adjustment_ratio = actual_timespan as f64 / target_timespan as f64;

        // Apply dampening to reduce oscillations
        if self.config.dampening_factor > 1.0 {
            // Move adjustment ratio closer to 1.0
            adjustment_ratio = 1.0 + (adjustment_ratio - 1.0) / self.config.dampening_factor;
        }

        // Apply adjustment limits
        adjustment_ratio = self.apply_adjustment_limits(adjustment_ratio);

        // Calculate new target
        let new_target = self.calculate_adjusted_target(current_target, adjustment_ratio)?;

        // Ensure target is within bounds
        self.enforce_target_bounds(new_target)
    }

    /// Calculate the actual timespan considering special rules and timestamp validation
    fn calculate_timespan(&self, timestamps: &[u64]) -> Result<u64, DifficultyAdjustmentError> {
        if timestamps.len() < 2 {
            return Err(DifficultyAdjustmentError::InsufficientHistory(2));
        }

        // Basic calculation: time between first and last block
        let mut start_time = timestamps[0];
        let mut end_time = *timestamps.last().ok_or_else(|| {
            DifficultyAdjustmentError::InvalidCalculation("Empty timestamp array".to_string())
        })?;

        // Use median-of-three for timestamps to prevent time-warp attacks
        if self.config.use_median_time_past && timestamps.len() >= 3 {
            // Use median of earliest 3 timestamps for start
            let early_timestamps = &timestamps[0..min(3, timestamps.len())];
            start_time = self.median_timestamp(early_timestamps);

            // Use median of latest 3 timestamps for end
            let latest_index = timestamps.len() - 3;
            let late_timestamps = &timestamps[max(0, latest_index)..];
            end_time = self.median_timestamp(late_timestamps);
        }

        if end_time <= start_time {
            return Err(DifficultyAdjustmentError::InvalidTimestamp(format!(
                "End time {} is not after start time {}",
                end_time, start_time
            )));
        }

        // Calculate the basic timespan
        let timespan = end_time - start_time;

        // If using weighted timespan, apply a more sophisticated calculation
        if self.config.use_weighted_timespan && timestamps.len() > 2 {
            return self.calculate_weighted_timespan(timestamps);
        }

        // SECURITY: Apply bounds to prevent timestamp manipulation attacks
        // These bounds limit how much an attacker can influence difficulty
        // by manipulating block timestamps
        
        let expected_timespan = self.config.target_block_time * (timestamps.len() as u64 - 1);
        
        // Minimum timespan is 1/4 of target (prevents "time warp" attacks)
        // If blocks claim to be mined 4x faster than target, clamp to 4x
        let min_timespan = expected_timespan / 4;
        
        // Maximum timespan is 4x target (prevents artificial difficulty drops)
        // If blocks claim to be mined 4x slower than target, clamp to 4x
        let max_timespan = expected_timespan * 4;
        
        let clamped_timespan = timespan.clamp(min_timespan, max_timespan);
        
        // Log if clamping occurred (timestamp manipulation attempt)
        if clamped_timespan != timespan {
            tracing::warn!(
                "Timespan clamped from {} to {} (min: {}, max: {}). Possible timestamp manipulation.",
                timespan, clamped_timespan, min_timespan, max_timespan
            );
        }

        Ok(clamped_timespan)
    }

    /// Calculate a weighted timespan that reduces impact of outliers
    fn calculate_weighted_timespan(
        &self,
        timestamps: &[u64],
    ) -> Result<u64, DifficultyAdjustmentError> {
        if timestamps.len() < 2 {
            return Err(DifficultyAdjustmentError::InsufficientHistory(2));
        }

        let mut intervals = Vec::with_capacity(timestamps.len() - 1);

        // Calculate all block intervals
        for i in 1..timestamps.len() {
            if timestamps[i] <= timestamps[i - 1] {
                // Ensure monotonically increasing timestamps
                continue;
            }
            intervals.push(timestamps[i] - timestamps[i - 1]);
        }

        if intervals.is_empty() {
            return Err(DifficultyAdjustmentError::InvalidTimestamp(
                "No valid intervals between blocks".to_string(),
            ));
        }

        // Sort intervals to identify outliers
        intervals.sort_unstable();

        // Remove the top and bottom 20% to eliminate outliers
        let outlier_count = intervals.len() / 5;
        let filtered_intervals = &intervals[outlier_count..intervals.len() - outlier_count];

        if filtered_intervals.is_empty() {
            // If we have too few intervals, use the median instead
            return Ok(intervals[intervals.len() / 2] * (timestamps.len() as u64 - 1));
        }

        // Calculate sum of filtered intervals
        let sum: u64 = filtered_intervals.iter().sum();

        // Scale to match the expected number of intervals
        let filtered_count = filtered_intervals.len() as u64;
        let expected_count = timestamps.len() as u64 - 1;

        let weighted_timespan = sum * expected_count / filtered_count;

        // Apply bounds to prevent extreme manipulation
        let min_timespan = self.config.target_block_time * expected_count / 4;
        let max_timespan = self.config.target_block_time * expected_count * 4;

        Ok(weighted_timespan.clamp(min_timespan, max_timespan))
    }

    /// Get the median timestamp from a slice of timestamps
    fn median_timestamp(&self, timestamps: &[u64]) -> u64 {
        if timestamps.is_empty() {
            return 0;
        }

        let mut sorted = timestamps.to_vec();
        sorted.sort_unstable();

        sorted[sorted.len() / 2]
    }

    /// Apply adjustment ratio limits
    /// 
    /// SECURITY FIX (P2-006): Explicit clamping to prevent difficulty manipulation.
    /// 
    /// This method ensures that difficulty cannot change more than 4x in either direction
    /// per adjustment period. This prevents timestamp manipulation attacks where miners
    /// artificially inflate or deflate difficulty.
    ///
    /// # Clamping Range
    /// - Minimum ratio: 0.25 (difficulty can increase up to 4x)
    /// - Maximum ratio: 4.0  (difficulty can decrease up to 4x)
    ///
    /// # Security Rationale
    /// - Too-large adjustments enable oscillation attacks
    /// - Gradual adjustment prevents sudden difficulty spikes/drops
    /// - 4x limit is an industry standard
    ///
    /// # Arguments
    /// * `ratio` - Raw adjustment ratio (actual_time / target_time)
    ///
    /// # Returns
    /// Clamped ratio in range [0.25, 4.0]
    fn apply_adjustment_limits(&self, ratio: f64) -> f64 {
        // SECURITY: Explicit min/max bounds for clarity
        const MIN_ADJUSTMENT_RATIO: f64 = 0.25; // Difficulty can increase 4x
        const MAX_ADJUSTMENT_RATIO: f64 = 4.0;  // Difficulty can decrease 4x
        
        // Clamp to safe range
        let clamped = ratio.clamp(MIN_ADJUSTMENT_RATIO, MAX_ADJUSTMENT_RATIO);
        
        // Log if clamping occurred (indicates potential manipulation attempt)
        if clamped != ratio {
            if ratio > MAX_ADJUSTMENT_RATIO {
                tracing::warn!(
                    "Difficulty adjustment clamped: ratio {:.2} > {:.2} max (possible timestamp manipulation)",
                    ratio, MAX_ADJUSTMENT_RATIO
                );
            } else {
                tracing::warn!(
                    "Difficulty adjustment clamped: ratio {:.2} < {:.2} min (possible timestamp manipulation)",
                    ratio, MIN_ADJUSTMENT_RATIO
                );
            }
        }
        
        clamped
    }

    /// Calculate adjusted target
    fn calculate_adjusted_target(
        &self,
        current_target: u32,
        adjustment_ratio: f64,
    ) -> Result<u32, DifficultyAdjustmentError> {
        // Extract the exponent and mantissa from the current target (encoded in "compact" format)
        let exponent = (current_target >> 24) & 0xFF;
        let mantissa = current_target & 0x00FFFFFF;

        // Calculate new target (mantissa * adjustment_ratio)
        let new_mantissa = (mantissa as f64 * adjustment_ratio) as u32;

        // Handle overflow by adjusting exponent
        let (adjusted_mantissa, adjusted_exponent) = if new_mantissa > 0x00FFFFFF {
            // Mantissa overflow, increment exponent
            (new_mantissa >> 8, exponent + 1)
        } else if new_mantissa < 0x008000 && exponent > 3 {
            // Mantissa too small, decrement exponent
            (new_mantissa << 8, exponent - 1)
        } else {
            (new_mantissa, exponent)
        };

        // Validate the exponent
        if adjusted_exponent > 0x20 {
            return Err(DifficultyAdjustmentError::ExceedsMaximumTarget);
        }

        // Recombine into new target
        Ok((adjusted_exponent << 24) | (adjusted_mantissa & 0x00FFFFFF))
    }

    /// Ensure the target is within allowed bounds
    fn enforce_target_bounds(&self, target: u32) -> Result<u32, DifficultyAdjustmentError> {
        if target > self.config.max_target {
            return Ok(self.config.max_target);
        }

        if target < self.config.min_target {
            return Ok(self.config.min_target);
        }

        Ok(target)
    }

    /// Convert a target to a 256-bit hash target threshold
    pub fn target_to_hash(&self, target: u32) -> [u8; 32] {
        let exponent = ((target >> 24) & 0xFF) as usize;
        let mantissa = target & 0x00FFFFFF;

        let mut hash = [0u8; 32];

        // Convert mantissa to big-endian bytes
        hash[32 - exponent] = ((mantissa >> 16) & 0xFF) as u8;
        hash[32 - exponent + 1] = ((mantissa >> 8) & 0xFF) as u8;
        hash[32 - exponent + 2] = (mantissa & 0xFF) as u8;

        hash
    }

    /// Convert a 256-bit hash to a compact target representation
    pub fn hash_to_target(&self, hash: &[u8; 32]) -> u32 {
        // Find the first non-zero byte
        let mut exponent = 1;
        for (i, &byte) in hash.iter().enumerate() {
            if byte != 0 {
                exponent = 32 - i;
                break;
            }
        }

        // Extract the mantissa (up to 3 bytes)
        let start_idx = 32 - exponent;
        let mantissa = if start_idx < 32 {
            let mut value = 0u32;
            let bytes_to_read = min(3, 32 - start_idx);

            for i in 0..bytes_to_read {
                value = (value << 8) | hash[start_idx + i] as u32;
            }

            // Shift if we read fewer than 3 bytes
            value << (8 * (3 - bytes_to_read))
        } else {
            0
        };

        // Combine exponent and mantissa
        (exponent as u32) << 24 | mantissa
    }
}

/// Calculate the required work (target hash) from a difficulty value
pub fn calculate_required_work(difficulty: u32) -> [u8; 32] {
    // Convert compact difficulty to 256-bit target hash
    let exponent = ((difficulty >> 24) & 0xFF) as usize;
    let mantissa = difficulty & 0x00FFFFFF;

    let mut target = [0u8; 32];

    if (3..=32).contains(&exponent) {
        let pos = 32 - exponent;
        // Set the mantissa bytes
        if pos < 30 {
            target[pos] = ((mantissa >> 16) & 0xFF) as u8;
            if pos < 31 {
                target[pos + 1] = ((mantissa >> 8) & 0xFF) as u8;
                if pos < 32 {
                    target[pos + 2] = (mantissa & 0xFF) as u8;
                }
            }
        }
    }

    target
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_no_adjustment_outside_interval() {
        let adjuster = DifficultyAdjustment::new();

        // Initial target
        let current_target = 0x1e00ffff;

        // Block timestamps (2.5 minutes apart)
        let timestamps = vec![1000, 1150, 1300, 1450];

        // Block at height 10 (not divisible by adjustment_interval)
        let heights = vec![7, 8, 9, 10];

        let result = adjuster
            .calculate_next_target(current_target, &timestamps, &heights)
            .unwrap();

        // Should not change the target
        assert_eq!(result, current_target);
    }

    #[test]
    fn test_adjustment_at_interval() {
        let adjuster = DifficultyAdjustment::with_config(DifficultyAdjustmentConfig {
            adjustment_interval: 4, // Adjust every 4 blocks for testing
            target_block_time: 150, // 2.5 minutes
            ..DifficultyAdjustmentConfig::default()
        });

        // Initial target
        let current_target = 0x1e00ffff;

        // Block timestamps (4 minutes apart instead of 2.5)
        let timestamps = vec![1000, 1240, 1480, 1720, 1960];

        // Block at height 4 (divisible by adjustment_interval)
        let heights = vec![0, 1, 2, 3, 4];

        let result = adjuster
            .calculate_next_target(current_target, &timestamps, &heights)
            .unwrap();

        // Should increase target (decrease difficulty) due to longer block times
        assert!(result > current_target);
    }

    #[test]
    fn test_weighted_timespan_calculation() {
        let adjuster = DifficultyAdjustment::with_config(DifficultyAdjustmentConfig {
            adjustment_interval: 5,
            target_block_time: 150,
            use_weighted_timespan: true,
            ..DifficultyAdjustmentConfig::default()
        });

        // Normal intervals with one outlier
        let timestamps = vec![1000, 1150, 1300, 1900, 2050, 2200];

        let result = adjuster.calculate_timespan(&timestamps).unwrap();

        // The weighted calculation should reduce the impact of the outlier
        // Regular timespan: 2200 - 1000 = 1200
        // Block intervals: [150, 150, 600, 150, 150]
        // After removing outliers and scaling: closer to 5*150 = 750
        assert!(result < 1200, "Result {} should be less than 1200", result);
        assert!(
            result >= 750,
            "Result {} should be greater than or equal to 750",
            result
        );
    }

    #[test]
    fn test_median_timestamp_calculation() {
        let adjuster = DifficultyAdjustment::new();

        let timestamps = vec![1000, 1300, 1200];
        let median = adjuster.median_timestamp(&timestamps);

        // Median should be 1200
        assert_eq!(median, 1200);
    }

    #[test]
    fn test_target_hash_conversions() {
        let adjuster = DifficultyAdjustment::new();

        // Test multiple known targets
        // Note: Some targets with leading zeros in mantissa may not round-trip perfectly
        // due to the compact representation format
        let test_cases = vec![
            // Target with non-zero leading mantissa byte (should round-trip perfectly)
            0x1d7fffff, 0x1e1234ff, 0x207fffff,
            // These may have precision issues but are still valid
            0x1d00ffff, 0x1e00ffff,
        ];

        for target in test_cases {
            // Convert to hash threshold
            let hash = adjuster.target_to_hash(target);

            // Convert back to target
            let recovered_target = adjuster.hash_to_target(&hash);

            // Verify the conversion preserves the difficulty intent
            // The hash representations should be equivalent for mining purposes
            let hash2 = adjuster.target_to_hash(recovered_target);

            // The two hashes should represent the same difficulty threshold
            assert_eq!(
                hash, hash2,
                "Hash mismatch for target 0x{:08x} -> 0x{:08x}",
                target, recovered_target
            );
        }
    }

    #[test]
    fn test_adjustment_caps() {
        let adjuster = DifficultyAdjustment::with_config(DifficultyAdjustmentConfig {
            adjustment_interval: 4,
            target_block_time: 150,
            max_upward_adjustment: 2.0,   // Max 2x easier
            max_downward_adjustment: 2.0, // Max 2x harder
            ..DifficultyAdjustmentConfig::default()
        });

        // Initial target
        let current_target = 0x1e00ffff;

        // Block timestamps (30 minutes = 1800 seconds apart - 12x slower than expected 150s)
        let slow_timestamps = vec![1000, 2800, 4600, 6400, 8200];

        // Block at height 4 (divisible by adjustment_interval)
        let heights = vec![0, 1, 2, 3, 4];

        let slow_result = adjuster
            .calculate_next_target(current_target, &slow_timestamps, &heights)
            .unwrap();

        // Target should increase (difficulty decrease) but by at most 2x
        assert!(slow_result > current_target);
        assert!(slow_result <= 0x1e01fffe); // Approximately 2x current_target

        // Block timestamps (50 seconds apart - 3x faster than expected 150s)
        let fast_timestamps = vec![1000, 1050, 1100, 1150, 1200];

        let fast_result = adjuster
            .calculate_next_target(current_target, &fast_timestamps, &heights)
            .unwrap();

        // Target should decrease (difficulty increase) but by at most 2x
        assert!(fast_result < current_target);
        assert!(fast_result >= 0x1d00ffff); // Approximately 1/2 of current_target
    }
}
