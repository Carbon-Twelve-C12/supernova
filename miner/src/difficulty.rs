use std::time::Duration;
use std::collections::VecDeque;

pub const NOVA_TOTAL_SUPPLY: u64 = 42_000_000;
pub const NOVA_BLOCK_REWARD: u64 = 50; // Initial block reward in NOVA
pub const BLOCK_TIME_TARGET: Duration = Duration::from_secs(60); // Target 1 block per minute
pub const DIFFICULTY_ADJUSTMENT_INTERVAL: u64 = 2016; // Number of blocks between adjustments
pub const DIFFICULTY_ADJUSTMENT_FACTOR: u32 = 4; // Maximum adjustment factor
pub const MOVING_AVERAGE_WINDOW: usize = 144; // 144 blocks (24 hours with 10 min blocks)
pub const TIMESTAMP_MEDIAN_TIMESPAN: usize = 11; // Median timespan for timestamp validation

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
    pub fn add_block_timestamp(&mut self, timestamp: u64) {
        if self.recent_timestamps.len() >= MOVING_AVERAGE_WINDOW {
            self.recent_timestamps.pop_front();
        }
        self.recent_timestamps.push_back(timestamp);
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
    
    // Advanced difficulty adjustment that uses a moving window for smoother adjustments
    pub fn adjust_difficulty(
        &mut self,
        current_height: u64,
        current_time: u64,
        blocks_since_adjustment: u64,
    ) -> u32 {
        // Add current timestamp to the window
        self.add_block_timestamp(current_time);
        
        // Full interval adjustment (similar to Bitcoin's 2-week adjustment)
        if blocks_since_adjustment >= DIFFICULTY_ADJUSTMENT_INTERVAL {
            return self.full_interval_adjustment(current_height, current_time, blocks_since_adjustment);
        }
        
        // Gradual adjustment based on recent blocks (more responsive to hashrate changes)
        if self.recent_timestamps.len() >= MOVING_AVERAGE_WINDOW / 2 {
            return self.moving_average_adjustment();
        }
        
        // Default: return current target if we don't have enough data
        self.current_target
    }
    
    // Full difficulty adjustment performed at the end of an interval
    fn full_interval_adjustment(
        &mut self, 
        current_height: u64, 
        current_time: u64, 
        blocks_since_adjustment: u64
    ) -> u32 {
        let time_taken = current_time - self.last_adjustment_time;
        let target_time = BLOCK_TIME_TARGET.as_secs() * blocks_since_adjustment as u64;
        
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
        
        self.current_target
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
        let time_span = newest - oldest_relevant;
        
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
        let new_target = adjuster.adjust_difficulty(
            2016, // height
            60 * 1008, // half the expected time
            2016, // full interval
        );
        assert!(new_target < 0x1d00ffff);

        // Test difficulty decrease (blocks too slow)
        let new_target = adjuster.adjust_difficulty(
            4032, // height
            60 * 4032, // double the expected time
            2016, // full interval
        );
        assert!(new_target > adjuster.current_target);
    }
    
    #[test]
    fn test_moving_average_adjustment() {
        let mut adjuster = DifficultyAdjuster::new(0x1d00ffff);
        
        // Add timestamps simulating blocks being found too quickly
        let base_time = 1000000;
        for i in 0..30 {
            adjuster.add_block_timestamp(base_time + i * 30); // 30-second blocks
        }
        
        // Adjust difficulty based on moving average
        let new_target = adjuster.moving_average_adjustment();
        
        // Should decrease target (increase difficulty) since blocks are too fast
        assert!(new_target < 0x1d00ffff);
        
        // Now simulate blocks being found too slowly
        let mut slow_adjuster = DifficultyAdjuster::new(0x1d00ffff);
        for i in 0..30 {
            slow_adjuster.add_block_timestamp(base_time + i * 120); // 120-second blocks
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
        adjuster.add_block_timestamp(1000);
        adjuster.add_block_timestamp(1200);
        adjuster.add_block_timestamp(900);
        adjuster.add_block_timestamp(1100);
        adjuster.add_block_timestamp(1050);
        
        // Get median timestamp (should be 1050)
        let median = adjuster.get_median_timestamp();
        assert_eq!(median, 1050);
    }
}