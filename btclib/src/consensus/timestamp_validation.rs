//! Timestamp validation to prevent time-warp attacks
//! 
//! This module implements comprehensive timestamp validation rules to prevent
//! attackers from manipulating block timestamps to reduce difficulty.

use std::time::{SystemTime, UNIX_EPOCH};
use thiserror::Error;

/// Maximum time a block timestamp can be in the future (2 hours)
pub const MAX_FUTURE_TIME: u64 = 2 * 60 * 60; // 2 hours in seconds

/// Number of blocks to use for median time past calculation
pub const MEDIAN_TIME_BLOCKS: usize = 11;

/// Minimum time between blocks to prevent timestamp manipulation
pub const MIN_BLOCK_TIME: u64 = 1; // 1 second minimum

/// Maximum backwards time drift allowed between consecutive blocks
pub const MAX_BACKWARD_TIME_DRIFT: u64 = 60 * 60; // 1 hour

/// Errors related to timestamp validation
#[derive(Debug, Error)]
pub enum TimestampValidationError {
    #[error("Block timestamp too far in future: {timestamp} > {max_allowed}")]
    TimestampTooFarInFuture { timestamp: u64, max_allowed: u64 },
    
    #[error("Block timestamp before median past time: {timestamp} < {median_time}")]
    TimestampBeforeMedianTime { timestamp: u64, median_time: u64 },
    
    #[error("Block timestamp too close to previous: {timestamp} - {previous} = {diff} < {min}")]
    TimestampTooClose { 
        timestamp: u64, 
        previous: u64, 
        diff: u64, 
        min: u64 
    },
    
    #[error("Block timestamp too far before previous: {previous} - {timestamp} = {diff} > {max}")]
    TimestampTooFarBackward { 
        timestamp: u64, 
        previous: u64, 
        diff: u64, 
        max: u64 
    },
    
    #[error("Insufficient block history for validation (need {required}, have {available})")]
    InsufficientHistory { required: usize, available: usize },
    
    #[error("Invalid timestamp order in block history")]
    InvalidTimestampOrder,
    
    #[error("System time error: {0}")]
    SystemTimeError(String),
}

/// Configuration for timestamp validation
#[derive(Debug, Clone)]
pub struct TimestampValidationConfig {
    /// Maximum seconds a timestamp can be in the future
    pub max_future_time: u64,
    
    /// Number of blocks to use for median calculation
    pub median_time_blocks: usize,
    
    /// Minimum time between blocks
    pub min_block_time: u64,
    
    /// Maximum backwards time drift
    pub max_backward_drift: u64,
    
    /// Whether to enforce strict monotonic timestamps
    pub enforce_monotonic: bool,
    
    /// Whether to validate against network time
    pub validate_network_time: bool,
}

impl Default for TimestampValidationConfig {
    fn default() -> Self {
        Self {
            max_future_time: MAX_FUTURE_TIME,
            median_time_blocks: MEDIAN_TIME_BLOCKS,
            min_block_time: MIN_BLOCK_TIME,
            max_backward_drift: MAX_BACKWARD_TIME_DRIFT,
            enforce_monotonic: false, // Allow some backward drift
            validate_network_time: true,
        }
    }
}

/// Timestamp validator to prevent time-warp attacks
pub struct TimestampValidator {
    config: TimestampValidationConfig,
}

impl Default for TimestampValidator {
    fn default() -> Self {
        Self::new()
    }
}

impl TimestampValidator {
    /// Create a new timestamp validator with default config
    pub fn new() -> Self {
        Self {
            config: TimestampValidationConfig::default(),
        }
    }
    
    /// Create a new timestamp validator with custom config
    pub fn with_config(config: TimestampValidationConfig) -> Self {
        Self { config }
    }
    
    /// Get the current system time as Unix timestamp
    pub fn current_time(&self) -> Result<u64, TimestampValidationError> {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .map_err(|e| TimestampValidationError::SystemTimeError(e.to_string()))
    }
    
    /// Calculate median time from a list of timestamps
    pub fn calculate_median_time(&self, timestamps: &[u64]) -> Result<u64, TimestampValidationError> {
        if timestamps.is_empty() {
            return Err(TimestampValidationError::InsufficientHistory {
                required: 1,
                available: 0,
            });
        }
        
        let mut sorted_times = timestamps.to_vec();
        sorted_times.sort_unstable();
        
        let median_index = sorted_times.len() / 2;
        Ok(sorted_times[median_index])
    }
    
    /// Calculate median past time from the last N blocks
    pub fn calculate_median_past_time(
        &self,
        block_timestamps: &[u64],
    ) -> Result<u64, TimestampValidationError> {
        if block_timestamps.is_empty() {
            return Ok(0); // Genesis block case
        }
        
        // Take the last N blocks for median calculation
        let blocks_to_use = self.config.median_time_blocks.min(block_timestamps.len());
        let start_index = block_timestamps.len().saturating_sub(blocks_to_use);
        let recent_timestamps = &block_timestamps[start_index..];
        
        self.calculate_median_time(recent_timestamps)
    }
    
    /// Validate a block timestamp against consensus rules
    pub fn validate_timestamp(
        &self,
        block_timestamp: u64,
        previous_timestamps: &[u64],
        network_adjusted_time: Option<u64>,
    ) -> Result<(), TimestampValidationError> {
        // 1. Check against current time (with network adjustment if available)
        let current_time = if let Some(network_time) = network_adjusted_time {
            network_time
        } else if self.config.validate_network_time {
            self.current_time()?
        } else {
            // If not validating against network time, use a far future time
            u64::MAX
        };
        
        // Validate not too far in future
        let max_allowed_time = current_time.saturating_add(self.config.max_future_time);
        if block_timestamp > max_allowed_time {
            return Err(TimestampValidationError::TimestampTooFarInFuture {
                timestamp: block_timestamp,
                max_allowed: max_allowed_time,
            });
        }
        
        // 2. Check against median past time
        if !previous_timestamps.is_empty() {
            let median_time = self.calculate_median_past_time(previous_timestamps)?;
            
            // Block timestamp must be greater than median time of past blocks
            if block_timestamp <= median_time {
                return Err(TimestampValidationError::TimestampBeforeMedianTime {
                    timestamp: block_timestamp,
                    median_time,
                });
            }
        }
        
        // 3. Check against previous block timestamp
        if let Some(&previous_timestamp) = previous_timestamps.last() {
            // Check minimum time between blocks
            if block_timestamp < previous_timestamp.saturating_add(self.config.min_block_time) {
                let diff = block_timestamp.saturating_sub(previous_timestamp);
                return Err(TimestampValidationError::TimestampTooClose {
                    timestamp: block_timestamp,
                    previous: previous_timestamp,
                    diff,
                    min: self.config.min_block_time,
                });
            }
            
            // Check maximum backward drift
            if previous_timestamp > block_timestamp {
                let backward_drift = previous_timestamp - block_timestamp;
                if backward_drift > self.config.max_backward_drift {
                    return Err(TimestampValidationError::TimestampTooFarBackward {
                        timestamp: block_timestamp,
                        previous: previous_timestamp,
                        diff: backward_drift,
                        max: self.config.max_backward_drift,
                    });
                }
            }
            
            // Enforce strict monotonic timestamps if configured
            if self.config.enforce_monotonic && block_timestamp <= previous_timestamp {
                return Err(TimestampValidationError::TimestampBeforeMedianTime {
                    timestamp: block_timestamp,
                    median_time: previous_timestamp,
                });
            }
        }
        
        Ok(())
    }
    
    /// Validate timestamps for difficulty adjustment calculation
    /// This prevents attackers from manipulating timestamps to trigger easier difficulty
    pub fn validate_difficulty_timestamps(
        &self,
        block_timestamps: &[u64],
        block_heights: &[u64],
        adjustment_interval: u64,
    ) -> Result<(), TimestampValidationError> {
        if block_timestamps.len() != block_heights.len() {
            return Err(TimestampValidationError::InvalidTimestampOrder);
        }
        
        if block_timestamps.len() < 2 {
            return Err(TimestampValidationError::InsufficientHistory {
                required: 2,
                available: block_timestamps.len(),
            });
        }
        
        // Validate timestamp ordering within the adjustment window
        let mut previous_timestamp = block_timestamps[0];
        for (i, &timestamp) in block_timestamps.iter().enumerate().skip(1) {
            // Allow some backward drift but not excessive
            if previous_timestamp > timestamp {
                let drift = previous_timestamp - timestamp;
                if drift > self.config.max_backward_drift {
                    return Err(TimestampValidationError::TimestampTooFarBackward {
                        timestamp,
                        previous: previous_timestamp,
                        diff: drift,
                        max: self.config.max_backward_drift,
                    });
                }
            }
            
            // Check that timestamps are not suspiciously clustered
            // This prevents miners from creating artificial timestamp patterns
            if i > 0 && timestamp == previous_timestamp {
                // Count consecutive blocks with same timestamp
                let mut same_count = 1;
                let mut j = i;
                while j > 0 && block_timestamps[j] == block_timestamps[j - 1] {
                    same_count += 1;
                    j -= 1;
                }
                
                // Don't allow more than 5 consecutive blocks with same timestamp
                if same_count > 5 {
                    return Err(TimestampValidationError::InvalidTimestampOrder);
                }
            }
            
            previous_timestamp = timestamp;
        }
        
        // Validate the time span is reasonable
        let first_timestamp = block_timestamps[0];
        let last_timestamp = block_timestamps[block_timestamps.len() - 1];
        let time_span = last_timestamp.saturating_sub(first_timestamp);
        let block_count = block_timestamps.len() as u64 - 1;
        
        // Average time per block
        if block_count > 0 {
            let avg_block_time = time_span / block_count;
            
            // Reject if average block time is suspiciously low (< 10 seconds)
            if avg_block_time < 10 {
                return Err(TimestampValidationError::TimestampTooClose {
                    timestamp: last_timestamp,
                    previous: first_timestamp,
                    diff: time_span,
                    min: 10 * block_count,
                });
            }
            
            // Reject if average block time is suspiciously high (> 2 hours)
            if avg_block_time > 7200 {
                return Err(TimestampValidationError::InvalidTimestampOrder);
            }
        }
        
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_median_time_calculation() {
        let validator = TimestampValidator::new();
        
        // Test odd number of timestamps
        let timestamps = vec![100, 300, 200, 500, 400];
        let median = validator.calculate_median_time(&timestamps).unwrap();
        assert_eq!(median, 300);
        
        // Test even number of timestamps
        let timestamps = vec![100, 200, 300, 400];
        let median = validator.calculate_median_time(&timestamps).unwrap();
        assert_eq!(median, 300);
    }
    
    #[test]
    fn test_timestamp_too_far_in_future() {
        let validator = TimestampValidator::new();
        let current_time = 1000000;
        let future_timestamp = current_time + MAX_FUTURE_TIME + 1;
        
        let result = validator.validate_timestamp(
            future_timestamp,
            &[],
            Some(current_time),
        );
        
        assert!(matches!(
            result,
            Err(TimestampValidationError::TimestampTooFarInFuture { .. })
        ));
    }
    
    #[test]
    fn test_timestamp_before_median_time() {
        let validator = TimestampValidator::new();
        let previous_timestamps = vec![100, 200, 300, 400, 500];
        let block_timestamp = 250; // Less than median (300)
        
        let result = validator.validate_timestamp(
            block_timestamp,
            &previous_timestamps,
            None,
        );
        
        assert!(matches!(
            result,
            Err(TimestampValidationError::TimestampBeforeMedianTime { .. })
        ));
    }
    
    #[test]
    fn test_valid_timestamp() {
        let validator = TimestampValidator::new();
        let previous_timestamps = vec![100, 200, 300, 400, 500];
        let block_timestamp = 600;
        let current_time = 1000;
        
        let result = validator.validate_timestamp(
            block_timestamp,
            &previous_timestamps,
            Some(current_time),
        );
        
        assert!(result.is_ok());
    }
    
    #[test]
    fn test_difficulty_timestamp_validation() {
        let validator = TimestampValidator::new();
        
        // Valid timestamps
        let timestamps = vec![1000, 1600, 2200, 2800, 3400];
        let heights = vec![0, 1, 2, 3, 4];
        
        let result = validator.validate_difficulty_timestamps(&timestamps, &heights, 2016);
        assert!(result.is_ok());
        
        // Invalid: too many blocks with same timestamp
        let bad_timestamps = vec![1000, 2000, 2000, 2000, 2000, 2000, 2000];
        let bad_heights = vec![0, 1, 2, 3, 4, 5, 6];
        
        let result = validator.validate_difficulty_timestamps(&bad_timestamps, &bad_heights, 2016);
        assert!(result.is_err());
    }
} 