//! Time Warp Attack Prevention Module
//!
//! Implements comprehensive timestamp validation to prevent time manipulation attacks
//! that could be used to artificially lower difficulty.

use crate::types::block::BlockHeader;
use std::collections::VecDeque;
use std::time::{SystemTime, UNIX_EPOCH};
use thiserror::Error;

/// Time validation errors
#[derive(Debug, Error)]
pub enum TimeValidationError {
    #[error("Block timestamp too far in future: {0} seconds ahead")]
    TooFarInFuture(i64),

    #[error("Block timestamp violates median time past: {0} <= {1}")]
    MedianTimePastViolation(u64, u64),

    #[error("Timestamp manipulation detected: {0}")]
    ManipulationDetected(String),

    #[error("Invalid timestamp: {0}")]
    InvalidTimestamp(u64),
}

/// Result type for time validation
pub type TimeValidationResult<T> = Result<T, TimeValidationError>;

/// Configuration for time warp prevention
#[derive(Debug, Clone)]
pub struct TimeWarpConfig {
    /// Maximum allowed time ahead of current time (seconds)
    pub max_future_time: u64,

    /// Number of blocks to consider for median time past
    pub median_time_blocks: usize,

    /// Maximum allowed timestamp gap between consecutive blocks
    pub max_time_gap: u64,

    /// Minimum time increment between consecutive blocks (seconds)
    /// SECURITY FIX (P1-002): Prevents blocks with zero or negative time differences
    pub min_time_increment: u64,

    /// Maximum allowed clock drift from network time (seconds)
    /// SECURITY FIX (P1-002): Tolerance for honest nodes with slightly incorrect clocks
    pub max_clock_drift: u64,

    /// Stricter timestamp bounds during difficulty adjustment periods
    /// SECURITY FIX (P1-002): Prevent manipulation near adjustment boundaries
    pub strict_adjustment_period: bool,

    /// Enable statistical anomaly detection
    pub enable_anomaly_detection: bool,

    /// Threshold for anomaly detection (standard deviations)
    pub anomaly_threshold: f64,
}

impl Default for TimeWarpConfig {
    fn default() -> Self {
        Self {
            max_future_time: 7200,  // 2 hours (same as Bitcoin)
            median_time_blocks: 11, // Same as Bitcoin
            max_time_gap: 86400,    // 24 hours
            min_time_increment: 1,  // SECURITY FIX (P1-002): Minimum 1 second between blocks
            max_clock_drift: 300,   // SECURITY FIX (P1-002): 5 minutes drift tolerance
            strict_adjustment_period: true, // SECURITY FIX (P1-002): Stricter validation near adjustments
            enable_anomaly_detection: true,
            anomaly_threshold: 3.0, // 3 standard deviations
        }
    }
}

/// Time warp attack prevention system
pub struct TimeWarpPrevention {
    config: TimeWarpConfig,

    /// Recent block timestamps for anomaly detection
    recent_timestamps: VecDeque<u64>,

    /// Maximum history size
    max_history: usize,
}

impl TimeWarpPrevention {
    pub fn new(config: TimeWarpConfig) -> Self {
        Self {
            config,
            recent_timestamps: VecDeque::with_capacity(100),
            max_history: 100,
        }
    }

    /// Validate a block's timestamp against time warp attacks
    pub fn validate_timestamp(
        &mut self,
        block_header: &BlockHeader,
        previous_timestamps: &[u64], // Last N block timestamps in reverse order
        current_time: Option<u64>,   // For testing
    ) -> TimeValidationResult<()> {
        let timestamp = block_header.timestamp();
        let current_time = current_time.unwrap_or_else(Self::current_time);

        // SECURITY FIX (P1-002): Clock drift tolerance - allow slight drift for honest nodes
        let adjusted_current_time = current_time.saturating_add(self.config.max_clock_drift);

        // 1. Check if timestamp is too far in the future (with drift tolerance)
        let max_allowed_future = adjusted_current_time.saturating_add(self.config.max_future_time);
        if timestamp > max_allowed_future {
            let ahead = (timestamp.saturating_sub(adjusted_current_time)) as i64;
            return Err(TimeValidationError::TooFarInFuture(ahead));
        }

        // SECURITY FIX (P1-002): Check minimum timestamp increment between blocks
        if let Some(&prev_timestamp) = previous_timestamps.first() {
            if timestamp <= prev_timestamp {
                return Err(TimeValidationError::ManipulationDetected(format!(
                    "Timestamp rollback detected: {} <= {}",
                    timestamp, prev_timestamp
                )));
            }

            // Ensure minimum time increment
            let time_diff = timestamp - prev_timestamp;
            if time_diff < self.config.min_time_increment {
                return Err(TimeValidationError::ManipulationDetected(format!(
                    "Timestamp too close to previous: {} seconds < minimum {} seconds",
                    time_diff, self.config.min_time_increment
                )));
            }
        }

        // 2. Check median time past (MTP) rule
        if !previous_timestamps.is_empty() {
            let mtp = self.calculate_median_time_past(previous_timestamps)?;
            if timestamp <= mtp {
                return Err(TimeValidationError::MedianTimePastViolation(timestamp, mtp));
            }
        }

        // 3. Check for suspiciously large time gaps
        if let Some(&prev_timestamp) = previous_timestamps.first() {
            if timestamp > prev_timestamp + self.config.max_time_gap {
                return Err(TimeValidationError::ManipulationDetected(format!(
                    "Time gap too large: {} seconds",
                    timestamp - prev_timestamp
                )));
            }
        }

        // SECURITY FIX (P1-002): Stricter validation near difficulty adjustment boundaries
        if self.config.strict_adjustment_period {
            self.validate_adjustment_period_timestamp(timestamp, previous_timestamps)?;
        }

        // 4. Check for manipulation patterns (always check, regardless of config)
        self.detect_manipulation_patterns(timestamp, previous_timestamps)?;

        // 5. Statistical anomaly detection (requires more history)
        if self.config.enable_anomaly_detection && previous_timestamps.len() >= 20 {
            self.detect_time_anomalies(timestamp, previous_timestamps)?;
        }

        // 6. Update history for future validations
        self.update_timestamp_history(timestamp);

        Ok(())
    }

    /// Calculate median time past from previous timestamps
    fn calculate_median_time_past(&self, timestamps: &[u64]) -> TimeValidationResult<u64> {
        if timestamps.is_empty() {
            return Ok(0);
        }

        // Take up to median_time_blocks timestamps
        let count = timestamps.len().min(self.config.median_time_blocks);
        let mut recent: Vec<u64> = timestamps[..count].to_vec();

        // Sort to find median
        recent.sort_unstable();

        // Return median value
        Ok(recent[count / 2])
    }

    /// Detect statistical anomalies in timestamp patterns
    fn detect_time_anomalies(
        &self,
        new_timestamp: u64,
        previous_timestamps: &[u64],
    ) -> TimeValidationResult<()> {
        // Calculate inter-block times
        let mut inter_block_times = Vec::new();
        for i in 1..previous_timestamps.len().min(20) {
            let time_diff = previous_timestamps[i - 1].saturating_sub(previous_timestamps[i]);
            inter_block_times.push(time_diff as f64);
        }

        if inter_block_times.is_empty() {
            return Ok(());
        }

        // Calculate mean and standard deviation
        let mean = inter_block_times.iter().sum::<f64>() / inter_block_times.len() as f64;
        let variance = inter_block_times
            .iter()
            .map(|&x| (x - mean).powi(2))
            .sum::<f64>()
            / inter_block_times.len() as f64;
        let std_dev = variance.sqrt();

        // Check if new inter-block time is anomalous
        if let Some(&prev) = previous_timestamps.first() {
            let new_inter_time = new_timestamp.saturating_sub(prev) as f64;
            let z_score = (new_inter_time - mean).abs() / std_dev;

            if z_score > self.config.anomaly_threshold {
                return Err(TimeValidationError::ManipulationDetected(format!(
                    "Statistical anomaly detected: z-score {:.2} exceeds threshold",
                    z_score
                )));
            }
        }

        // Pattern detection is now done separately in validate_timestamp

        Ok(())
    }

    /// Detect specific patterns that indicate time manipulation
    fn detect_manipulation_patterns(
        &self,
        new_timestamp: u64,
        previous_timestamps: &[u64],
    ) -> TimeValidationResult<()> {
        // Pattern 1: Alternating timestamps (classic time warp)
        if previous_timestamps.len() >= 3 {
            // Look for alternating pattern in recent timestamps
            // Check if we have a clear alternating pattern
            let mut ups: i32 = 0;
            let mut downs: i32 = 0;
            let mut last_direction = None;
            let mut alternating_count = 0;

            // Check the pattern including the new timestamp
            let mut check_timestamps =
                previous_timestamps[0..previous_timestamps.len().min(10)].to_vec();
            check_timestamps.insert(0, new_timestamp);

            for i in 1..check_timestamps.len() {
                let current_direction = if check_timestamps[i - 1] > check_timestamps[i] {
                    Some(true) // Going up (newer timestamp is higher)
                } else if check_timestamps[i - 1] < check_timestamps[i] {
                    Some(false) // Going down
                } else {
                    continue; // Skip equal timestamps
                };

                if let Some(dir) = current_direction {
                    if dir {
                        ups += 1;
                    } else {
                        downs += 1;
                    }

                    if let Some(last) = last_direction {
                        if last != dir {
                            alternating_count += 1;
                        }
                    }
                    last_direction = Some(dir);
                }
            }

            // Debug output
            #[cfg(test)]
            {
            }

            // If we see a clear alternating pattern (at least 3 alternations)
            // and roughly equal ups and downs
            if alternating_count >= 3 && (ups - downs).abs() <= 1 {
                return Err(TimeValidationError::ManipulationDetected(
                    "Alternating timestamp pattern detected".to_string(),
                ));
            }
        }

        // Pattern 2: Sudden timestamp jumps near difficulty adjustment
        if self.is_near_difficulty_adjustment(previous_timestamps.len()) {
            if let Some(&prev) = previous_timestamps.first() {
                let time_diff = new_timestamp.saturating_sub(prev);

                // If near adjustment and time difference is suspiciously large
                if time_diff > 3600 {
                    // 1 hour
                    return Err(TimeValidationError::ManipulationDetected(
                        "Suspicious timestamp jump near difficulty adjustment".to_string(),
                    ));
                }
            }
        }

        Ok(())
    }

    /// SECURITY FIX (P1-002): Validate timestamp during difficulty adjustment periods
    /// Apply stricter rules near adjustment boundaries to prevent manipulation
    fn validate_adjustment_period_timestamp(
        &self,
        timestamp: u64,
        previous_timestamps: &[u64],
    ) -> TimeValidationResult<()> {
        if previous_timestamps.is_empty() {
            return Ok(());
        }

        // Check if we're near a difficulty adjustment boundary
        if self.is_near_difficulty_adjustment(previous_timestamps.len()) {
            // Stricter rules apply:
            // 1. Timestamp must be within reasonable bounds relative to previous blocks
            if let Some(&prev) = previous_timestamps.first() {
                let time_diff = timestamp.saturating_sub(prev);
                
                // During adjustment period, allow smaller time differences
                // This prevents attackers from manipulating timestamps to affect difficulty
                let max_adjustment_period_gap = self.config.max_time_gap / 4; // 25% of normal max
                
                if time_diff > max_adjustment_period_gap {
                    return Err(TimeValidationError::ManipulationDetected(format!(
                        "Suspicious timestamp jump during adjustment period: {} seconds (max: {})",
                        time_diff, max_adjustment_period_gap
                    )));
                }

                // Ensure timestamp progression is consistent
                // Check that the timestamp doesn't violate expected block time patterns
                if previous_timestamps.len() >= 2 {
                    let avg_inter_block_time = self.calculate_average_inter_block_time(previous_timestamps);
                    let expected_next_time = prev + avg_inter_block_time;
                    
                    // Allow some variance but not extreme deviations
                    let variance_tolerance = avg_inter_block_time * 2;
                    if timestamp > expected_next_time + variance_tolerance {
                        return Err(TimeValidationError::ManipulationDetected(format!(
                            "Timestamp inconsistent with block time pattern during adjustment period"
                        )));
                    }
                }
            }
        }

        Ok(())
    }

    /// SECURITY FIX (P1-002): Calculate average inter-block time from recent history
    fn calculate_average_inter_block_time(&self, timestamps: &[u64]) -> u64 {
        if timestamps.len() < 2 {
            return 150; // Default 2.5 minutes
        }

        let mut total_diff = 0u64;
        let mut count = 0usize;

        // Calculate time differences between consecutive blocks
        for i in 1..timestamps.len().min(11) {
            let diff = timestamps[i - 1].saturating_sub(timestamps[i]);
            if diff > 0 {
                total_diff = total_diff.saturating_add(diff);
                count += 1;
            }
        }

        if count > 0 {
            total_diff / count as u64
        } else {
            150 // Default fallback
        }
    }

    /// Update timestamp history
    fn update_timestamp_history(&mut self, timestamp: u64) {
        self.recent_timestamps.push_front(timestamp);
        if self.recent_timestamps.len() > self.max_history {
            self.recent_timestamps.pop_back();
        }
    }

    /// Check if we're near a difficulty adjustment boundary
    fn is_near_difficulty_adjustment(&self, block_count: usize) -> bool {
        // Assuming 2016 block adjustment period like Bitcoin
        const ADJUSTMENT_PERIOD: usize = 2016;
        let blocks_until_adjustment = ADJUSTMENT_PERIOD - (block_count % ADJUSTMENT_PERIOD);
        blocks_until_adjustment <= 10 // Within 10 blocks of adjustment
    }

    /// SECURITY FIX (P1-002): Validate timestamp doesn't rollback past median time
    /// This prevents timestamp manipulation attacks that try to reduce difficulty
    fn validate_timestamp_rollback(
        &self,
        timestamp: u64,
        previous_timestamps: &[u64],
    ) -> TimeValidationResult<()> {
        if previous_timestamps.is_empty() {
            return Ok(());
        }

        // Check against median time past
        let mtp = self.calculate_median_time_past(previous_timestamps)?;
        
        // Timestamp must be strictly greater than median time past
        if timestamp <= mtp {
            return Err(TimeValidationError::MedianTimePastViolation(timestamp, mtp));
        }

        // Additional check: ensure timestamp progression is consistent
        // Calculate expected timestamp based on recent patterns
        if previous_timestamps.len() >= 3 {
            let avg_time = self.calculate_average_inter_block_time(previous_timestamps);
            let last_timestamp = previous_timestamps[0];
            let expected_min = last_timestamp.saturating_add(avg_time / 2);
            
            // If timestamp is significantly below expected minimum, it's suspicious
            if timestamp < expected_min {
                return Err(TimeValidationError::ManipulationDetected(format!(
                    "Timestamp rollback detected: {} < expected minimum {}",
                    timestamp, expected_min
                )));
            }
        }

        Ok(())
    }

    /// Get current system time
    fn current_time() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_median_time_past() {
        let config = TimeWarpConfig::default();
        let mut prevention = TimeWarpPrevention::new(config);

        // Test with odd number of timestamps
        let timestamps = vec![1000, 1100, 1200, 1300, 1400];
        let mtp = prevention.calculate_median_time_past(&timestamps).unwrap();
        assert_eq!(mtp, 1200); // Middle value

        // Test with even number of timestamps
        let timestamps = vec![1000, 1100, 1200, 1300];
        let mtp = prevention.calculate_median_time_past(&timestamps).unwrap();
        assert_eq!(mtp, 1200); // Upper middle value
    }

    #[test]
    fn test_future_time_validation() {
        let config = TimeWarpConfig::default();
        let mut prevention = TimeWarpPrevention::new(config);

        let current_time = 1_000_000;
        let header = BlockHeader::new(
            1,
            [0; 32],
            [0; 32],
            current_time + 10_000, // 10,000 seconds in future
            0x1d00ffff,
            0,
        );

        let result = prevention.validate_timestamp(&header, &[], Some(current_time));
        assert!(result.is_err());

        match result {
            Err(TimeValidationError::TooFarInFuture(ahead)) => {
                assert_eq!(ahead, 10_000); // Total time ahead
            }
            _ => panic!("Expected TooFarInFuture error"),
        }
    }

    #[test]
    fn test_time_warp_pattern_detection() {
        let mut config = TimeWarpConfig::default();
        config.enable_anomaly_detection = true;
        let mut prevention = TimeWarpPrevention::new(config);

        // Create alternating timestamp pattern (classic time warp)
        // Previous timestamps in reverse order (newest first)
        let previous = vec![2200, 900, 2100, 1000, 2000];
        let new_timestamp = 800; // Continues the pattern

        let header = BlockHeader::new(6, [0; 32], [0; 32], new_timestamp, 0x1d00ffff, 0);
        let result = prevention.validate_timestamp(&header, &previous, Some(3000));

        assert!(result.is_err());
        match result {
            Err(TimeValidationError::ManipulationDetected(msg)) => {
                assert!(msg.contains("Alternating timestamp pattern"));
            }
            _ => panic!("Expected ManipulationDetected error"),
        }
    }

    /// SECURITY FIX (P1-002): Tests for time warp attack prevention edge cases
    #[test]
    fn test_time_warp_attack_boundary_manipulation() {
        let mut config = TimeWarpConfig::default();
        config.strict_adjustment_period = true;
        let mut prevention = TimeWarpPrevention::new(config);

        // Simulate being near difficulty adjustment boundary (within 10 blocks)
        // Block count 2006 means we're 10 blocks away from adjustment (2016)
        let previous_timestamps: Vec<u64> = (0..2006)
            .map(|i| 1000000 + (i as u64) * 150)
            .rev()
            .collect();

        // Attempt to manipulate timestamp with large jump near adjustment
        let malicious_timestamp = previous_timestamps[0] + 10000; // 10,000 seconds jump
        
        let header = BlockHeader::new(
            1,
            [0; 32],
            [0; 32],
            malicious_timestamp,
            0x1d00ffff,
            0,
        );

        let result = prevention.validate_timestamp(&header, &previous_timestamps, Some(1500000));
        assert!(result.is_err(), "Large timestamp jump near adjustment should be rejected");
    }

    #[test]
    fn test_clock_drift_tolerance() {
        let config = TimeWarpConfig::default();
        let mut prevention = TimeWarpPrevention::new(config);

        let current_time = 1_000_000;
        let drift_time = current_time + config.max_clock_drift - 1; // Within drift tolerance
        
        let header = BlockHeader::new(
            1,
            [0; 32],
            [0; 32],
            drift_time,
            0x1d00ffff,
            0,
        );

        // Should pass validation with clock drift tolerance
        let result = prevention.validate_timestamp(&header, &[], Some(current_time));
        assert!(result.is_ok(), "Timestamp within clock drift tolerance should pass");

        // Test beyond drift tolerance
        let beyond_drift = current_time + config.max_clock_drift + config.max_future_time + 1;
        let header2 = BlockHeader::new(1, [0; 32], [0; 32], beyond_drift, 0x1d00ffff, 0);
        let result2 = prevention.validate_timestamp(&header2, &[], Some(current_time));
        assert!(result2.is_err(), "Timestamp beyond drift tolerance should fail");
    }

    #[test]
    fn test_median_time_past_validation() {
        let config = TimeWarpConfig::default();
        let mut prevention = TimeWarpPrevention::new(config);

        // Create timestamps with clear median
        let previous_timestamps = vec![1500, 1400, 1300, 1200, 1100, 1000, 900, 800, 700, 600, 500];
        let mtp = prevention.calculate_median_time_past(&previous_timestamps).unwrap();
        assert_eq!(mtp, 1000, "Median should be middle value");

        // Test timestamp that violates MTP rule
        let violating_timestamp = mtp - 1; // Below median
        let header = BlockHeader::new(
            1,
            [0; 32],
            [0; 32],
            violating_timestamp,
            0x1d00ffff,
            0,
        );

        let result = prevention.validate_timestamp(&header, &previous_timestamps, Some(2000));
        assert!(result.is_err(), "Timestamp below median time past should fail");
        match result {
            Err(TimeValidationError::MedianTimePastViolation(ts, mtp_val)) => {
                assert_eq!(ts, violating_timestamp);
                assert_eq!(mtp_val, mtp);
            }
            _ => panic!("Expected MedianTimePastViolation error"),
        }
    }

    #[test]
    fn test_timestamp_rollback_prevention() {
        let config = TimeWarpConfig::default();
        let mut prevention = TimeWarpPrevention::new(config);

        let previous_timestamps = vec![1500, 1400, 1300];
        
        // Test timestamp rollback (less than previous)
        let rollback_timestamp = previous_timestamps[0] - 1;
        let header = BlockHeader::new(
            1,
            [0; 32],
            [0; 32],
            rollback_timestamp,
            0x1d00ffff,
            0,
        );

        let result = prevention.validate_timestamp(&header, &previous_timestamps, Some(2000));
        assert!(result.is_err(), "Timestamp rollback should be rejected");
        match result {
            Err(TimeValidationError::ManipulationDetected(msg)) => {
                assert!(msg.contains("Timestamp rollback") || msg.contains("rollback"));
            }
            _ => panic!("Expected ManipulationDetected error for rollback"),
        }

        // Test timestamp equal to previous (should also fail)
        let equal_timestamp = previous_timestamps[0];
        let header2 = BlockHeader::new(1, [0; 32], [0; 32], equal_timestamp, 0x1d00ffff, 0);
        let result2 = prevention.validate_timestamp(&header2, &previous_timestamps, Some(2000));
        assert!(result2.is_err(), "Timestamp equal to previous should fail");
    }

    #[test]
    fn test_difficulty_adjustment_time_attack() {
        let mut config = TimeWarpConfig::default();
        config.strict_adjustment_period = true;
        let mut prevention = TimeWarpPrevention::new(config);

        // Simulate blocks leading up to difficulty adjustment
        // Create timestamps that would allow manipulation
        let mut previous_timestamps = Vec::new();
        let base_time = 1000000;
        
        // Create blocks with consistent timing up to adjustment boundary
        for i in 0..2010 {
            previous_timestamps.push(base_time + (i as u64) * 150);
        }
        previous_timestamps.reverse(); // Reverse for validation function

        // Attempt attack: timestamp that's inconsistent with block time pattern
        // This would artificially affect difficulty calculation
        let avg_inter_block = 150; // Expected 2.5 minutes
        let attack_timestamp = previous_timestamps[0] + avg_inter_block * 10; // 10x normal
        
        let header = BlockHeader::new(
            1,
            [0; 32],
            [0; 32],
            attack_timestamp,
            0x1d00ffff,
            0,
        );

        let result = prevention.validate_timestamp(&header, &previous_timestamps, Some(base_time + 400000));
        // Should be rejected due to strict adjustment period validation
        assert!(result.is_err(), "Timestamp manipulation during adjustment period should be rejected");
    }

    #[test]
    fn test_minimum_time_increment_validation() {
        let mut config = TimeWarpConfig::default();
        config.min_time_increment = 5; // Require 5 seconds minimum
        let mut prevention = TimeWarpPrevention::new(config);

        let previous_timestamps = vec![1000];
        
        // Test with timestamp too close (less than minimum increment)
        let too_close = previous_timestamps[0] + config.min_time_increment - 1;
        let header = BlockHeader::new(1, [0; 32], [0; 32], too_close, 0x1d00ffff, 0);
        
        let result = prevention.validate_timestamp(&header, &previous_timestamps, Some(2000));
        assert!(result.is_err(), "Timestamp too close should fail");
        match result {
            Err(TimeValidationError::ManipulationDetected(msg)) => {
                assert!(msg.contains("too close") || msg.contains("minimum"));
            }
            _ => panic!("Expected ManipulationDetected error"),
        }

        // Test with valid increment
        let valid_timestamp = previous_timestamps[0] + config.min_time_increment;
        let header2 = BlockHeader::new(1, [0; 32], [0; 32], valid_timestamp, 0x1d00ffff, 0);
        let result2 = prevention.validate_timestamp(&header2, &previous_timestamps, Some(2000));
        assert!(result2.is_ok(), "Timestamp with valid increment should pass");
    }

    #[test]
    fn test_timestamp_progression_consistency() {
        let config = TimeWarpConfig::default();
        let mut prevention = TimeWarpPrevention::new(config);

        // Create timestamps with consistent progression
        let previous_timestamps = vec![1500, 1400, 1300, 1200, 1100, 1000, 900, 800, 700, 600, 500];
        
        // Valid timestamp: follows expected pattern
        let valid_timestamp = previous_timestamps[0] + 150; // Normal block time
        let header = BlockHeader::new(1, [0; 32], [0; 32], valid_timestamp, 0x1d00ffff, 0);
        
        let result = prevention.validate_timestamp(&header, &previous_timestamps, Some(2000));
        assert!(result.is_ok(), "Consistent timestamp progression should pass");
    }
}
