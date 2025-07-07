//! Legacy Difficulty Adjustment Module
//! 
//! DEPRECATED: This module contains the legacy difficulty adjustment implementation
//! which is vulnerable to manipulation attacks. New code should use
//! `node::mining::SecureDifficultyAdjuster` instead.
//! 
//! This module is retained for backward compatibility only and will be removed
//! in a future version.

#![deprecated(
    since = "0.14.0",
    note = "Use node::mining::SecureDifficultyAdjuster for secure difficulty adjustment"
)]

use std::time::{Duration, SystemTime, UNIX_EPOCH};
use std::collections::VecDeque;

pub const NOVA_TOTAL_SUPPLY: u64 = 42_000_000;
pub const NOVA_BLOCK_REWARD: u64 = 50; // Initial block reward in NOVA
pub const BLOCK_TIME_TARGET: Duration = Duration::from_secs(150); // Target 2.5 minutes per block
pub const DIFFICULTY_ADJUSTMENT_INTERVAL: u64 = 2016; // Number of blocks between adjustments
pub const DIFFICULTY_ADJUSTMENT_FACTOR: u32 = 4; // Maximum adjustment factor
pub const MOVING_AVERAGE_WINDOW: usize = 576; // 576 blocks (24 hours with 2.5 min blocks)
pub const TIMESTAMP_MEDIAN_TIMESPAN: usize = 11; // Median timespan for timestamp validation

// Maximum time a block timestamp can be in the future
pub const MAX_FUTURE_TIME: u64 = 2 * 60 * 60; // 2 hours

#[derive(Clone)]
pub struct DifficultyAdjuster {
    last_adjustment_time: u64,
    last_adjustment_height: u64,
    current_target: u32,
    // Track recent block timestamps for moving average
    recent_timestamps: VecDeque<u64>,
    // Track recent targets for smoother transitions
    recent_targets: VecDeque<u32>,
}

impl DifficultyAdjuster {
    pub fn new(initial_target: u32) -> Self {
        let mut recent_timestamps = VecDeque::with_capacity(MOVING_AVERAGE_WINDOW);
        let mut recent_targets = VecDeque::with_capacity(MOVING_AVERAGE_WINDOW);
        
        // Initialize with default values
        recent_timestamps.push_back(0);
        recent_targets.push_back(initial_target);
        
        Self {
            last_adjustment_time: 0,
            last_adjustment_height: 0,
            current_target: initial_target,
            recent_timestamps,
            recent_targets,
        }
    }
    
    // Add new block timestamp and update the window
    pub fn add_block_timestamp(&mut self, timestamp: u64) -> Result<(), String> {
        // Validate timestamp before adding
        let current_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| format!("System time error: {}", e))?
            .as_secs();
        
        // Check if timestamp is too far in the future
        if timestamp > current_time + MAX_FUTURE_TIME {
            return Err(format!(
                "Block timestamp too far in future: {} > {}",
                timestamp,
                current_time + MAX_FUTURE_TIME
            ));
        }
        
        // Check if timestamp is before median past time
        let median_time = self.get_median_timestamp();
        if timestamp <= median_time && !self.recent_timestamps.is_empty() {
            return Err(format!(
                "Block timestamp {} is not greater than median past time {}",
                timestamp,
                median_time
            ));
        }
        
        if self.recent_timestamps.len() >= MOVING_AVERAGE_WINDOW {
            self.recent_timestamps.pop_front();
        }
        self.recent_timestamps.push_back(timestamp);
        
        Ok(())
    }
    
    // Get median timestamp from recent blocks to prevent timestamp manipulation
    pub fn get_median_timestamp(&self) -> u64 {
        if self.recent_timestamps.len() <= 1 {
            return self.recent_timestamps.back().copied().unwrap_or(0);
        }
        
        let count = std::cmp::min(TIMESTAMP_MEDIAN_TIMESPAN, self.recent_timestamps.len());
        let mut timestamps: Vec<u64> = self.recent_timestamps
            .iter()
            .rev()
            .take(count)
            .copied()
            .collect();
        
        timestamps.sort_unstable();
        timestamps[count / 2]
    }
    
    // Validate timestamps for time-warp attack prevention
    fn validate_timestamps_for_adjustment(&self, timestamps: &[u64]) -> Result<(), String> {
        if timestamps.len() < 2 {
            return Ok(());
        }
        
        // Check for suspicious patterns
        let mut previous = timestamps[0];
        let mut same_count = 1;
        
        for &timestamp in timestamps.iter().skip(1) {
            if timestamp == previous {
                same_count += 1;
                // Don't allow more than 5 consecutive blocks with same timestamp
                if same_count > 5 {
                    return Err("Too many consecutive blocks with identical timestamps".to_string());
                }
            } else {
                same_count = 1;
            }
            
            // Check for excessive backward drift
            if previous > timestamp && previous - timestamp > 3600 {
                return Err(format!(
                    "Excessive backward time drift: {} -> {} (diff: {})",
                    previous,
                    timestamp,
                    previous - timestamp
                ));
            }
            
            previous = timestamp;
        }
        
        // Check average block time is reasonable
        let time_span = timestamps.last().unwrap() - timestamps[0];
        let block_count = timestamps.len() as u64 - 1;
        
        if block_count > 0 {
            let avg_block_time = time_span / block_count;
            
            // Reject if average block time is suspiciously low (< 10 seconds)
            if avg_block_time < 10 {
                return Err(format!(
                    "Average block time too low: {} seconds",
                    avg_block_time
                ));
            }
            
            // Reject if average block time is suspiciously high (> 2 hours)
            if avg_block_time > 7200 {
                return Err(format!(
                    "Average block time too high: {} seconds",
                    avg_block_time
                ));
            }
        }
        
        Ok(())
    }
    
    // Advanced difficulty adjustment that uses a moving window for smoother adjustments
    pub fn adjust_difficulty(
        &mut self,
        current_height: u64,
        current_time: u64,
        blocks_since_adjustment: u64,
    ) -> Result<u32, String> {
        // Add current timestamp to the window
        self.add_block_timestamp(current_time)?;
        
        // Validate recent timestamps
        let timestamps: Vec<u64> = self.recent_timestamps.iter().copied().collect();
        self.validate_timestamps_for_adjustment(&timestamps)?;
        
        // Full interval adjustment (similar to Bitcoin's 2-week adjustment)
        if blocks_since_adjustment >= DIFFICULTY_ADJUSTMENT_INTERVAL {
            return Ok(self.full_interval_adjustment(current_height, current_time, blocks_since_adjustment));
        }
        
        // Gradual adjustment based on recent blocks (more responsive to hashrate changes)
        if self.recent_timestamps.len() >= MOVING_AVERAGE_WINDOW / 2 {
            return Ok(self.moving_average_adjustment());
        }
        
        // Default: return current target if we don't have enough data
        Ok(self.current_target)
    }
    
    // Full difficulty adjustment performed at the end of an interval
    fn full_interval_adjustment(
        &mut self, 
        current_height: u64, 
        current_time: u64, 
        blocks_since_adjustment: u64
    ) -> u32 {
        let time_taken = current_time.saturating_sub(self.last_adjustment_time);
        let target_time = BLOCK_TIME_TARGET.as_secs() * blocks_since_adjustment;
        
        // Apply dampening to avoid extreme adjustments
        let time_ratio = if time_taken < target_time / 4 {
            // Cap at 4x increase in difficulty
            0.25
        } else if time_taken > target_time * 4 {
            // Cap at 4x decrease in difficulty
            4.0
        } else {
            time_taken as f64 / target_time as f64
        };
        
        // Calculate new target with dampening
        // For time_ratio > 1, this increases the target (decreases difficulty)
        // For time_ratio < 1, this decreases the target (increases difficulty)
        let new_target = (self.current_target as f64 * time_ratio) as u32;
        
        // Update state
        self.last_adjustment_time = current_time;
        self.last_adjustment_height = current_height;
        self.current_target = new_target;
        
        // Update moving window
        if self.recent_targets.len() >= MOVING_AVERAGE_WINDOW {
            self.recent_targets.pop_front();
        }
        self.recent_targets.push_back(new_target);
        
        new_target
    }
    
    // Gradual adjustment based on recent block timestamps
    fn moving_average_adjustment(&mut self) -> u32 {
        if self.recent_timestamps.len() < 2 {
            return self.current_target;
        }
        
        // Calculate average time between the most recent blocks
        let window_size = std::cmp::min(24, self.recent_timestamps.len() - 1);
        let oldest_relevant = self.recent_timestamps[self.recent_timestamps.len() - window_size - 1];
        let newest = *self.recent_timestamps.back().unwrap();
        let time_span = newest.saturating_sub(oldest_relevant);
        
        // Avoid division by zero and ensure reasonable values
        if time_span == 0 || window_size == 0 {
            return self.current_target;
        }
        
        let average_time = time_span as f64 / window_size as f64;
        let target_time = BLOCK_TIME_TARGET.as_secs() as f64;
        
        // Calculate adjustment factor with dampening
        let mut adjustment_factor = average_time / target_time;
        
        // Limit extreme adjustments
        adjustment_factor = adjustment_factor.clamp(0.75, 1.25);
        
        // Apply a weighted adjustment (25% new factor, 75% previous target)
        let weighted_adjustment = 0.25 * adjustment_factor + 0.75;
        let new_target = (self.current_target as f64 * weighted_adjustment) as u32;
        
        // Update current target
        self.current_target = new_target;
        
        // Update moving window
        if self.recent_targets.len() >= MOVING_AVERAGE_WINDOW {
            self.recent_targets.pop_front();
        }
        self.recent_targets.push_back(new_target);
        
        self.current_target
    }

    pub fn get_current_target(&self) -> u32 {
        self.current_target
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_difficulty_adjustment() {
        let mut adjuster = DifficultyAdjuster::new(0x1d00ffff);
        
        // Test difficulty increase (blocks too fast)
        let result = adjuster.adjust_difficulty(
            2016, // height
            60 * 1008, // half the expected time
            2016, // full interval
        );
        
        assert!(result.is_ok());
        let new_target = result.unwrap();
        assert!(new_target < 0x1d00ffff, "Target should decrease when blocks are too fast");
        
        // Store the current target after first adjustment
        let first_adjusted_target = adjuster.get_current_target();

        // Test difficulty decrease (blocks too slow)
        let result = adjuster.adjust_difficulty(
            4032, // height
            60 * 4032 * 2, // double the expected time from the current position
            2016, // full interval
        );
        
        assert!(result.is_ok());
        let new_target = result.unwrap();
        assert!(new_target > first_adjusted_target, "Target should increase when blocks are too slow");
    }
    
    #[test]
    fn test_timestamp_validation() {
        let mut adjuster = DifficultyAdjuster::new(0x1d00ffff);
        
        // Get current time
        let current_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        
        // Test adding valid timestamp
        let result = adjuster.add_block_timestamp(current_time);
        assert!(result.is_ok());
        
        // Test adding timestamp too far in future
        let future_time = current_time + MAX_FUTURE_TIME + 1;
        let result = adjuster.add_block_timestamp(future_time);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("too far in future"));
        
        // Add some valid timestamps
        for i in 1..12 {
            let _ = adjuster.add_block_timestamp(current_time + i * 60);
        }
        
        // Test adding timestamp before median
        let median = adjuster.get_median_timestamp();
        let result = adjuster.add_block_timestamp(median - 1);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not greater than median"));
    }
    
    #[test]
    fn test_moving_average_adjustment() {
        let mut adjuster = DifficultyAdjuster::new(0x1d00ffff);
        
        // Add timestamps simulating blocks being found too quickly
        let base_time = 1000000;
        for i in 0..30 {
            let _ = adjuster.add_block_timestamp(base_time + i * 30); // 30-second blocks
        }
        
        // Adjust difficulty based on moving average
        let new_target = adjuster.moving_average_adjustment();
        
        // Should decrease target (increase difficulty) since blocks are too fast
        assert!(new_target < 0x1d00ffff);
        
        // Now simulate blocks being found too slowly
        let mut slow_adjuster = DifficultyAdjuster::new(0x1d00ffff);
        for i in 0..30 {
            let _ = slow_adjuster.add_block_timestamp(base_time + i * 120); // 120-second blocks
        }
        
        // Adjust difficulty based on moving average
        let new_slow_target = slow_adjuster.moving_average_adjustment();
        
        // Should increase target (decrease difficulty) since blocks are too slow
        assert!(new_slow_target > 0x1d00ffff);
    }
    
    #[test]
    fn test_median_timestamp() {
        let mut adjuster = DifficultyAdjuster::new(0x1d00ffff);
        
        // Add some timestamps
        let _ = adjuster.add_block_timestamp(1000);
        let _ = adjuster.add_block_timestamp(1200);
        let _ = adjuster.add_block_timestamp(900);
        let _ = adjuster.add_block_timestamp(1100);
        let _ = adjuster.add_block_timestamp(1050);
        
        // Get median timestamp (should be 1050)
        let median = adjuster.get_median_timestamp();
        assert_eq!(median, 1050);
    }
}