//! Secure Difficulty Adjustment Module for Supernova
//! 
//! This module provides a hardened difficulty adjustment algorithm that prevents
//! manipulation attacks including time-warp, difficulty lowering, and mining bypass.

use std::collections::VecDeque;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use sha2::{Sha256, Digest};
use thiserror::Error;

/// Difficulty adjustment errors
#[derive(Debug, Error)]
pub enum SecureDifficultyError {
    #[error("Insufficient block history: need {0} blocks, have {1}")]
    InsufficientHistory(usize, usize),
    
    #[error("Invalid timestamp: {0}")]
    InvalidTimestamp(String),
    
    #[error("Target exceeds maximum allowed: {0:08x} > {1:08x}")]
    TargetTooHigh(u32, u32),
    
    #[error("Target below minimum allowed: {0:08x} < {1:08x}")]
    TargetTooLow(u32, u32),
    
    #[error("Difficulty manipulation detected: {0}")]
    ManipulationDetected(String),
    
    #[error("Invalid block: {0}")]
    InvalidBlock(String),
    
    #[error("Chain validation failed: {0}")]
    ChainValidationFailed(String),
}

/// Security configuration for difficulty adjustment
#[derive(Debug, Clone)]
pub struct DifficultySecurityConfig {
    /// Minimum difficulty (maximum target value)
    pub min_difficulty_target: u32,
    
    /// Maximum difficulty (minimum target value)
    pub max_difficulty_target: u32,
    
    /// Adjustment interval in blocks
    pub adjustment_interval: u64,
    
    /// Target block time in seconds
    pub target_block_time: u64,
    
    /// Maximum allowed adjustment per interval
    pub max_adjustment_factor: f64,
    
    /// Minimum blocks for difficulty calculation
    pub min_blocks_for_calculation: usize,
    
    /// Enable anti-manipulation features
    pub enable_anti_manipulation: bool,
    
    /// Require minimum chainwork increase
    pub require_chainwork_progress: bool,
    
    /// Maximum timestamp variance allowed
    pub max_timestamp_variance: u64,
    
    /// Minimum effective difficulty (prevents bypass)
    pub absolute_minimum_difficulty: u64,
}

impl Default for DifficultySecurityConfig {
    fn default() -> Self {
        Self {
            min_difficulty_target: 0x1e0fffff,  // Genesis difficulty
            max_difficulty_target: 0x1a00ffff,  // Very high difficulty
            adjustment_interval: 2016,          // ~3.5 days at 2.5 minutes per block
            target_block_time: 150,             // 2.5 minutes
            max_adjustment_factor: 4.0,         // Maximum 4x adjustment
            min_blocks_for_calculation: 576,    // 1 day of blocks at 2.5 minutes each
            enable_anti_manipulation: true,
            require_chainwork_progress: true,
            max_timestamp_variance: 7200,       // 2 hours
            absolute_minimum_difficulty: 1000,  // Never go below this
        }
    }
}

/// Secure difficulty adjuster with manipulation prevention
pub struct SecureDifficultyAdjuster {
    config: DifficultySecurityConfig,
    /// History of recent blocks for validation
    block_history: VecDeque<BlockInfo>,
    /// Cached chainwork calculations
    chainwork_cache: VecDeque<u128>,
    /// Last validated adjustment
    last_adjustment: AdjustmentInfo,
}

/// Information about a block for difficulty calculation
#[derive(Debug, Clone)]
pub struct BlockInfo {
    pub height: u64,
    pub timestamp: u64,
    pub target: u32,
    pub hash: [u8; 32],
    pub nonce: u64,
}

/// Information about a difficulty adjustment
#[derive(Debug, Clone)]
struct AdjustmentInfo {
    height: u64,
    timestamp: u64,
    old_target: u32,
    new_target: u32,
    adjustment_ratio: f64,
}

impl SecureDifficultyAdjuster {
    /// Create a new secure difficulty adjuster
    pub fn new(config: DifficultySecurityConfig) -> Self {
        Self {
            config,
            block_history: VecDeque::with_capacity(2016),
            chainwork_cache: VecDeque::with_capacity(2016),
            last_adjustment: AdjustmentInfo {
                height: 0,
                timestamp: 0,
                old_target: 0x1e0fffff,
                new_target: 0x1e0fffff,
                adjustment_ratio: 1.0,
            },
        }
    }
    
    /// Add a block to history and validate it
    pub fn add_block(&mut self, block: BlockInfo) -> Result<(), SecureDifficultyError> {
        // Validate the block
        self.validate_block(&block)?;
        
        // Check if this block meets the current difficulty requirement
        if !self.verify_proof_of_work(&block)? {
            return Err(SecureDifficultyError::InvalidBlock(
                "Block does not meet difficulty requirement".to_string()
            ));
        }
        
        // Add to history
        self.block_history.push_back(block.clone());
        
        // Calculate and cache chainwork
        let work = self.calculate_block_work(block.target);
        self.chainwork_cache.push_back(work);
        
        // Maintain history size
        if self.block_history.len() > self.config.adjustment_interval as usize * 2 {
            self.block_history.pop_front();
            self.chainwork_cache.pop_front();
        }
        
        Ok(())
    }
    
    /// Calculate the next difficulty target
    pub fn calculate_next_target(
        &mut self,
        current_height: u64,
    ) -> Result<u32, SecureDifficultyError> {
        // Check if we're at an adjustment boundary
        if current_height % self.config.adjustment_interval != 0 && current_height > 0 {
            // Not time to adjust, return current target
            if let Some(last_block) = self.block_history.back() {
                return Ok(last_block.target);
            }
            return Ok(self.last_adjustment.new_target);
        }
        
        // Ensure we have enough history
        let required_blocks = self.config.min_blocks_for_calculation;
        if self.block_history.len() < required_blocks {
            return Err(SecureDifficultyError::InsufficientHistory(
                required_blocks,
                self.block_history.len()
            ));
        }
        
        // Get the blocks for this adjustment period
        let period_blocks = self.get_adjustment_period_blocks()?;
        
        // Validate the block sequence
        if self.config.enable_anti_manipulation {
            self.validate_block_sequence(&period_blocks)?;
        }
        
        // Calculate actual timespan with security checks
        let actual_timespan = self.calculate_secure_timespan(&period_blocks)?;
        
        // Calculate target timespan
        let target_timespan = self.config.target_block_time * (period_blocks.len() as u64 - 1);
        
        // Calculate adjustment ratio with bounds
        let mut adjustment_ratio = actual_timespan as f64 / target_timespan as f64;
        
        // Apply security limits
        adjustment_ratio = self.apply_security_limits(adjustment_ratio)?;
        
        // Get current target
        let current_target = period_blocks.last().unwrap().target;
        
        // Calculate new target
        let new_target = self.calculate_new_target(current_target, adjustment_ratio)?;
        
        // Validate the new target
        self.validate_new_target(new_target, current_target)?;
        
        // Update adjustment info
        self.last_adjustment = AdjustmentInfo {
            height: current_height,
            timestamp: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
            old_target: current_target,
            new_target,
            adjustment_ratio,
        };
        
        Ok(new_target)
    }
    
    /// Verify that a block meets the proof-of-work requirement
    pub fn verify_proof_of_work(&self, block: &BlockInfo) -> Result<bool, SecureDifficultyError> {
        // Convert target to 256-bit threshold
        let target_threshold = self.target_to_threshold(block.target);
        
        // Check if block hash is below target
        for i in 0..32 {
            if block.hash[i] < target_threshold[i] {
                return Ok(true);
            } else if block.hash[i] > target_threshold[i] {
                return Ok(false);
            }
        }
        
        // Exact match (extremely unlikely)
        Ok(true)
    }
    
    /// Validate a single block
    fn validate_block(&self, block: &BlockInfo) -> Result<(), SecureDifficultyError> {
        // Check timestamp bounds
        let current_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        
        if block.timestamp > current_time + 7200 {
            return Err(SecureDifficultyError::InvalidTimestamp(
                format!("Block timestamp {} is more than 2 hours in future", block.timestamp)
            ));
        }
        
        // Check if timestamp is reasonable compared to previous blocks
        if let Some(prev_block) = self.block_history.back() {
            if block.timestamp <= prev_block.timestamp {
                return Err(SecureDifficultyError::InvalidTimestamp(
                    format!("Block timestamp {} not greater than previous {}", 
                        block.timestamp, prev_block.timestamp)
                ));
            }
            
            // Check for suspiciously fast blocks
            let time_delta = block.timestamp - prev_block.timestamp;
            if time_delta < 1 {
                return Err(SecureDifficultyError::ManipulationDetected(
                    "Blocks generated too quickly".to_string()
                ));
            }
        }
        
        // Validate target is within bounds
        if block.target > self.config.min_difficulty_target {
            return Err(SecureDifficultyError::TargetTooHigh(
                block.target,
                self.config.min_difficulty_target
            ));
        }
        
        if block.target < self.config.max_difficulty_target {
            return Err(SecureDifficultyError::TargetTooLow(
                block.target,
                self.config.max_difficulty_target
            ));
        }
        
        Ok(())
    }
    
    /// Get blocks for the current adjustment period
    fn get_adjustment_period_blocks(&self) -> Result<Vec<BlockInfo>, SecureDifficultyError> {
        let interval = self.config.adjustment_interval as usize;
        
        // Get the most recent 'interval' blocks
        let start_idx = if self.block_history.len() >= interval {
            self.block_history.len() - interval
        } else {
            0
        };
        
        let blocks: Vec<BlockInfo> = self.block_history
            .iter()
            .skip(start_idx)
            .cloned()
            .collect();
        
        if blocks.is_empty() {
            return Err(SecureDifficultyError::InsufficientHistory(1, 0));
        }
        
        Ok(blocks)
    }
    
    /// Validate a sequence of blocks for manipulation
    fn validate_block_sequence(&self, blocks: &[BlockInfo]) -> Result<(), SecureDifficultyError> {
        if blocks.len() < 2 {
            return Ok(());
        }
        
        let mut timestamp_deltas = Vec::new();
        
        for i in 1..blocks.len() {
            let delta = blocks[i].timestamp - blocks[i-1].timestamp;
            timestamp_deltas.push(delta);
        }
        
        // Calculate statistics
        let mean_delta = timestamp_deltas.iter().sum::<u64>() as f64 / timestamp_deltas.len() as f64;
        let variance = timestamp_deltas.iter()
            .map(|&d| {
                let diff = d as f64 - mean_delta;
                diff * diff
            })
            .sum::<f64>() / timestamp_deltas.len() as f64;
        
        let std_dev = variance.sqrt();
        
        // Check for timestamp manipulation patterns
        if std_dev > self.config.max_timestamp_variance as f64 {
            return Err(SecureDifficultyError::ManipulationDetected(
                format!("Excessive timestamp variance: {:.2}", std_dev)
            ));
        }
        
        // Check for alternating timestamps (common attack pattern)
        let mut alternating_count = 0;
        for i in 2..timestamp_deltas.len() {
            if (timestamp_deltas[i] > mean_delta && timestamp_deltas[i-1] < mean_delta) ||
               (timestamp_deltas[i] < mean_delta && timestamp_deltas[i-1] > mean_delta) {
                alternating_count += 1;
            }
        }
        
        if alternating_count > timestamp_deltas.len() * 3 / 4 {
            return Err(SecureDifficultyError::ManipulationDetected(
                "Suspicious alternating timestamp pattern detected".to_string()
            ));
        }
        
        // Verify chainwork is increasing
        if self.config.require_chainwork_progress {
            self.verify_chainwork_progress(blocks)?;
        }
        
        Ok(())
    }
    
    /// Calculate timespan with security measures
    fn calculate_secure_timespan(&self, blocks: &[BlockInfo]) -> Result<u64, SecureDifficultyError> {
        if blocks.len() < 2 {
            return Err(SecureDifficultyError::InsufficientHistory(2, blocks.len()));
        }
        
        // Use median of first 11 blocks for start time
        let start_time = if blocks.len() >= 11 {
            let mut start_times: Vec<u64> = blocks[0..11].iter().map(|b| b.timestamp).collect();
            start_times.sort_unstable();
            start_times[5] // Median
        } else {
            blocks[0].timestamp
        };
        
        // Use median of last 11 blocks for end time
        let end_time = if blocks.len() >= 11 {
            let start_idx = blocks.len() - 11;
            let mut end_times: Vec<u64> = blocks[start_idx..].iter().map(|b| b.timestamp).collect();
            end_times.sort_unstable();
            end_times[5] // Median
        } else {
            blocks.last().unwrap().timestamp
        };
        
        if end_time <= start_time {
            return Err(SecureDifficultyError::InvalidTimestamp(
                format!("End time {} not after start time {}", end_time, start_time)
            ));
        }
        
        let timespan = end_time - start_time;
        
        // Apply bounds to prevent extreme manipulation
        let expected_timespan = self.config.target_block_time * (blocks.len() as u64 - 1);
        let min_timespan = expected_timespan / 4;
        let max_timespan = expected_timespan * 4;
        
        Ok(timespan.clamp(min_timespan, max_timespan))
    }
    
    /// Apply security limits to adjustment ratio
    fn apply_security_limits(&self, ratio: f64) -> Result<f64, SecureDifficultyError> {
        // Basic bounds
        let max_factor = self.config.max_adjustment_factor;
        let bounded_ratio = ratio.clamp(1.0 / max_factor, max_factor);
        
        // Additional dampening to prevent oscillations
        let dampened_ratio = 1.0 + (bounded_ratio - 1.0) * 0.75;
        
        // Check for suspicious adjustment patterns
        if (self.last_adjustment.adjustment_ratio > 2.0 && dampened_ratio < 0.5) ||
           (self.last_adjustment.adjustment_ratio < 0.5 && dampened_ratio > 2.0) {
            return Err(SecureDifficultyError::ManipulationDetected(
                "Suspicious difficulty oscillation detected".to_string()
            ));
        }
        
        Ok(dampened_ratio)
    }
    
    /// Calculate new target with overflow protection
    fn calculate_new_target(&self, current: u32, ratio: f64) -> Result<u32, SecureDifficultyError> {
        // Decompose compact target
        let exponent = (current >> 24) & 0xFF;
        let mantissa = current & 0x00FFFFFF;
        
        // Calculate new mantissa
        let new_mantissa_f64 = mantissa as f64 * ratio;
        
        // Check for overflow
        if new_mantissa_f64 > u32::MAX as f64 {
            return Err(SecureDifficultyError::ManipulationDetected(
                "Target calculation overflow".to_string()
            ));
        }
        
        let mut new_mantissa = new_mantissa_f64 as u32;
        let mut new_exponent = exponent;
        
        // Handle mantissa overflow/underflow
        while new_mantissa > 0x00FFFFFF && new_exponent < 0x20 {
            new_mantissa >>= 8;
            new_exponent += 1;
        }
        
        while new_mantissa < 0x008000 && new_exponent > 3 {
            new_mantissa <<= 8;
            new_exponent -= 1;
        }
        
        // Validate exponent
        if new_exponent > 0x20 {
            return Err(SecureDifficultyError::TargetTooHigh(
                0xFF000000,
                self.config.min_difficulty_target
            ));
        }
        
        Ok((new_exponent << 24) | (new_mantissa & 0x00FFFFFF))
    }
    
    /// Validate the new target
    fn validate_new_target(&self, new_target: u32, old_target: u32) -> Result<(), SecureDifficultyError> {
        // Check absolute bounds
        if new_target > self.config.min_difficulty_target {
            return Err(SecureDifficultyError::TargetTooHigh(
                new_target,
                self.config.min_difficulty_target
            ));
        }
        
        if new_target < self.config.max_difficulty_target {
            return Err(SecureDifficultyError::TargetTooLow(
                new_target,
                self.config.max_difficulty_target
            ));
        }
        
        // Check minimum difficulty
        let difficulty = self.target_to_difficulty(new_target);
        if difficulty < self.config.absolute_minimum_difficulty {
            return Err(SecureDifficultyError::ManipulationDetected(
                format!("Difficulty {} below absolute minimum {}", 
                    difficulty, self.config.absolute_minimum_difficulty)
            ));
        }
        
        // Verify adjustment is within allowed range
        let ratio = new_target as f64 / old_target as f64;
        if ratio > self.config.max_adjustment_factor || ratio < 1.0 / self.config.max_adjustment_factor {
            return Err(SecureDifficultyError::ManipulationDetected(
                format!("Adjustment ratio {:.2} exceeds maximum allowed", ratio)
            ));
        }
        
        Ok(())
    }
    
    /// Verify chainwork is progressing
    fn verify_chainwork_progress(&self, blocks: &[BlockInfo]) -> Result<(), SecureDifficultyError> {
        if blocks.len() < 2 || self.chainwork_cache.len() < blocks.len() {
            return Ok(());
        }
        
        let start_idx = self.chainwork_cache.len() - blocks.len();
        let total_work: u128 = self.chainwork_cache
            .iter()
            .skip(start_idx)
            .sum();
        
        // Calculate expected minimum work
        let min_work_per_block = self.calculate_block_work(self.config.min_difficulty_target);
        let expected_min_work = min_work_per_block * blocks.len() as u128;
        
        if total_work < expected_min_work / 2 {
            return Err(SecureDifficultyError::ChainValidationFailed(
                format!("Insufficient chainwork: {} < {}", total_work, expected_min_work / 2)
            ));
        }
        
        Ok(())
    }
    
    /// Calculate work done by a block
    fn calculate_block_work(&self, target: u32) -> u128 {
        // Work = 2^256 / (target + 1)
        // Approximate calculation to avoid overflow
        let difficulty = self.target_to_difficulty(target);
        difficulty as u128
    }
    
    /// Convert target to difficulty
    fn target_to_difficulty(&self, target: u32) -> u64 {
        // Difficulty = max_target / current_target
        // Simplified calculation
        let max_body = 0x00000000FFFF0000u64;
        let current_body = (target & 0x00FFFFFF) as u64;
        
        if current_body == 0 {
            return u64::MAX;
        }
        
        max_body / current_body
    }
    
    /// Convert target to 256-bit threshold
    fn target_to_threshold(&self, target: u32) -> [u8; 32] {
        let exponent = ((target >> 24) & 0xFF) as usize;
        let mantissa = target & 0x00FFFFFF;
        
        let mut threshold = [0u8; 32];
        
        if exponent >= 3 && exponent <= 32 {
            let pos = 32 - exponent;
            if pos < 30 {
                threshold[pos] = ((mantissa >> 16) & 0xFF) as u8;
                if pos < 31 {
                    threshold[pos + 1] = ((mantissa >> 8) & 0xFF) as u8;
                    if pos < 32 {
                        threshold[pos + 2] = (mantissa & 0xFF) as u8;
                    }
                }
            }
        }
        
        threshold
    }
    
    /// Get current statistics for monitoring
    pub fn get_statistics(&self) -> DifficultyStatistics {
        let current_target = self.last_adjustment.new_target;
        let current_difficulty = self.target_to_difficulty(current_target);
        
        let avg_block_time = if self.block_history.len() >= 2 {
            let total_time = self.block_history.back().unwrap().timestamp 
                - self.block_history.front().unwrap().timestamp;
            total_time / (self.block_history.len() as u64 - 1)
        } else {
            self.config.target_block_time
        };
        
        DifficultyStatistics {
            current_target,
            current_difficulty,
            last_adjustment_height: self.last_adjustment.height,
            last_adjustment_ratio: self.last_adjustment.adjustment_ratio,
            average_block_time: avg_block_time,
            blocks_in_history: self.block_history.len(),
            total_chainwork: self.chainwork_cache.iter().sum(),
        }
    }
}

/// Statistics about current difficulty
#[derive(Debug, Clone)]
pub struct DifficultyStatistics {
    pub current_target: u32,
    pub current_difficulty: u64,
    pub last_adjustment_height: u64,
    pub last_adjustment_ratio: f64,
    pub average_block_time: u64,
    pub blocks_in_history: usize,
    pub total_chainwork: u128,
}

#[cfg(test)]
mod tests {
    use super::*;
    
    fn create_test_block(height: u64, timestamp: u64, target: u32) -> BlockInfo {
        BlockInfo {
            height,
            timestamp,
            target,
            hash: [0; 32], // Simplified for tests
            nonce: 0,
        }
    }
    
    #[test]
    fn test_difficulty_adjustment_prevention() {
        let config = DifficultySecurityConfig {
            adjustment_interval: 10,
            target_block_time: 60,
            ..Default::default()
        };
        
        let mut adjuster = SecureDifficultyAdjuster::new(config);
        
        // Add blocks with manipulated timestamps
        for i in 0..10 {
            let timestamp = if i % 2 == 0 {
                1000 + i * 30  // Fast blocks
            } else {
                1000 + i * 90  // Slow blocks
            };
            
            let block = create_test_block(i, timestamp, 0x1e0fffff);
            let _ = adjuster.add_block(block);
        }
        
        // Try to calculate next target - should detect manipulation
        let result = adjuster.calculate_next_target(10);
        
        // Should either fail or apply heavy dampening
        if let Ok(new_target) = result {
            // Verify the adjustment is limited
            let ratio = new_target as f64 / 0x1e0fffff as f64;
            assert!(ratio > 0.5 && ratio < 2.0, "Adjustment ratio should be limited");
        }
    }
    
    #[test]
    fn test_minimum_difficulty_enforcement() {
        let config = DifficultySecurityConfig {
            absolute_minimum_difficulty: 1000,
            min_difficulty_target: 0x1f00ffff, // Very easy
            ..Default::default()
        };
        
        let mut adjuster = SecureDifficultyAdjuster::new(config);
        
        // Try to set a target that would result in too low difficulty
        let too_easy_target = 0x1f00ffff;
        let result = adjuster.validate_new_target(too_easy_target, 0x1e0fffff);
        
        // Should be rejected
        assert!(result.is_err());
    }
    
    #[test]
    fn test_chainwork_validation() {
        let config = DifficultySecurityConfig {
            require_chainwork_progress: true,
            ..Default::default()
        };
        
        let mut adjuster = SecureDifficultyAdjuster::new(config);
        
        // Add legitimate blocks
        for i in 0..20 {
            let block = create_test_block(i, 1000 + i * 600, 0x1e0fffff);
            adjuster.add_block(block).unwrap();
        }
        
        // Get statistics
        let stats = adjuster.get_statistics();
        
        // Verify chainwork is accumulating
        assert!(stats.total_chainwork > 0);
        assert_eq!(stats.blocks_in_history, 20);
    }
    
    #[test]
    fn test_timestamp_attack_prevention() {
        let config = DifficultySecurityConfig::default();
        let mut adjuster = SecureDifficultyAdjuster::new(config);
        
        // Try to add block with future timestamp
        let future_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() + 10000;
        
        let future_block = create_test_block(1, future_time, 0x1e0fffff);
        let result = adjuster.add_block(future_block);
        
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("future"));
    }
} 