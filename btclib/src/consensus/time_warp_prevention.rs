//! Time Warp Attack Prevention Module
//! 
//! Implements comprehensive timestamp validation to prevent time manipulation attacks
//! that could be used to artificially lower difficulty.

use std::collections::VecDeque;
use std::time::{SystemTime, UNIX_EPOCH};
use thiserror::Error;
use crate::types::block::BlockHeader;

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
    
    /// Enable statistical anomaly detection
    pub enable_anomaly_detection: bool,
    
    /// Threshold for anomaly detection (standard deviations)
    pub anomaly_threshold: f64,
}

impl Default for TimeWarpConfig {
    fn default() -> Self {
        Self {
            max_future_time: 7200, // 2 hours (same as Bitcoin)
            median_time_blocks: 11, // Same as Bitcoin
            max_time_gap: 86400,   // 24 hours
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
        current_time: Option<u64>,    // For testing
    ) -> TimeValidationResult<()> {
        let timestamp = block_header.timestamp();
        let current_time = current_time.unwrap_or_else(Self::current_time);
        
        // 1. Check if timestamp is too far in the future
        if timestamp > current_time + self.config.max_future_time {
            let ahead = (timestamp - current_time) as i64;
            return Err(TimeValidationError::TooFarInFuture(ahead));
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
                return Err(TimeValidationError::ManipulationDetected(
                    format!("Time gap too large: {} seconds", timestamp - prev_timestamp)
                ));
            }
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
            let time_diff = previous_timestamps[i-1].saturating_sub(previous_timestamps[i]);
            inter_block_times.push(time_diff as f64);
        }
        
        if inter_block_times.is_empty() {
            return Ok(());
        }
        
        // Calculate mean and standard deviation
        let mean = inter_block_times.iter().sum::<f64>() / inter_block_times.len() as f64;
        let variance = inter_block_times.iter()
            .map(|&x| (x - mean).powi(2))
            .sum::<f64>() / inter_block_times.len() as f64;
        let std_dev = variance.sqrt();
        
        // Check if new inter-block time is anomalous
        if let Some(&prev) = previous_timestamps.first() {
            let new_inter_time = new_timestamp.saturating_sub(prev) as f64;
            let z_score = (new_inter_time - mean).abs() / std_dev;
            
            if z_score > self.config.anomaly_threshold {
                return Err(TimeValidationError::ManipulationDetected(
                    format!("Statistical anomaly detected: z-score {:.2} exceeds threshold", z_score)
                ));
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
            let mut check_timestamps = previous_timestamps[0..previous_timestamps.len().min(10)].to_vec();
            check_timestamps.insert(0, new_timestamp);
            
            for i in 1..check_timestamps.len() {
                let current_direction = if check_timestamps[i-1] > check_timestamps[i] {
                    Some(true) // Going up (newer timestamp is higher)
                } else if check_timestamps[i-1] < check_timestamps[i] {
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
                println!("Alternating count: {}, ups: {}, downs: {}", alternating_count, ups, downs);
                println!("Check timestamps: {:?}", check_timestamps);
            }
            
            // If we see a clear alternating pattern (at least 3 alternations)
            // and roughly equal ups and downs
            if alternating_count >= 3 && (ups - downs).abs() <= 1 {
                return Err(TimeValidationError::ManipulationDetected(
                    "Alternating timestamp pattern detected".to_string()
                ));
            }
        }
        
        // Pattern 2: Sudden timestamp jumps near difficulty adjustment
        if self.is_near_difficulty_adjustment(previous_timestamps.len()) {
            if let Some(&prev) = previous_timestamps.first() {
                let time_diff = new_timestamp.saturating_sub(prev);
                
                // If near adjustment and time difference is suspiciously large
                if time_diff > 3600 { // 1 hour
                    return Err(TimeValidationError::ManipulationDetected(
                        "Suspicious timestamp jump near difficulty adjustment".to_string()
                    ));
                }
            }
        }
        
        Ok(())
    }
    
    /// Check if we're near a difficulty adjustment boundary
    fn is_near_difficulty_adjustment(&self, block_count: usize) -> bool {
        // Assuming 2016 block adjustment period like Bitcoin
        const ADJUSTMENT_PERIOD: usize = 2016;
        let blocks_until_adjustment = ADJUSTMENT_PERIOD - (block_count % ADJUSTMENT_PERIOD);
        blocks_until_adjustment <= 10 // Within 10 blocks of adjustment
    }
    
    /// Update timestamp history
    fn update_timestamp_history(&mut self, timestamp: u64) {
        self.recent_timestamps.push_front(timestamp);
        if self.recent_timestamps.len() > self.max_history {
            self.recent_timestamps.pop_back();
        }
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
            0
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
}
