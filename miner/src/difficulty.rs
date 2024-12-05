use std::time::Duration;

pub const NOVA_TOTAL_SUPPLY: u64 = 42_000_000;
pub const NOVA_BLOCK_REWARD: u64 = 50; // Initial block reward in NOVA
pub const BLOCK_TIME_TARGET: Duration = Duration::from_secs(60); // Target 1 block per minute
pub const DIFFICULTY_ADJUSTMENT_INTERVAL: u64 = 2016; // Number of blocks between adjustments
pub const DIFFICULTY_ADJUSTMENT_FACTOR: u32 = 4; // Maximum adjustment factor

pub struct DifficultyAdjuster {
    last_adjustment_time: u64,
    last_adjustment_height: u64,
    current_target: u32,
}

impl DifficultyAdjuster {
    pub fn new(initial_target: u32) -> Self {
        Self {
            last_adjustment_time: 0,
            last_adjustment_height: 0,
            current_target: initial_target,
        }
    }

    pub fn adjust_difficulty(
        &mut self,
        current_height: u64,
        current_time: u64,
        blocks_since_adjustment: u64,
    ) -> u32 {
        if blocks_since_adjustment < DIFFICULTY_ADJUSTMENT_INTERVAL {
            return self.current_target;
        }

        let time_taken = current_time - self.last_adjustment_time;
        let target_time = BLOCK_TIME_TARGET.as_secs() * blocks_since_adjustment as u64;

        // Calculate new target
        let mut new_target = self.current_target as u64;
        new_target = new_target * time_taken / target_time;

        // Apply adjustment factor limits
        let min_target = self.current_target as u64 / DIFFICULTY_ADJUSTMENT_FACTOR as u64;
        let max_target = self.current_target as u64 * DIFFICULTY_ADJUSTMENT_FACTOR as u64;
        new_target = new_target.clamp(min_target, max_target);

        // Update state
        self.last_adjustment_time = current_time;
        self.last_adjustment_height = current_height;
        self.current_target = new_target as u32;

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
}