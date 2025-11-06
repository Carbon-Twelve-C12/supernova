//! Supernova Dynamic Fee Estimation
//!
//! Dual-factor pricing model combining:
//! - Network congestion (traditional)
//! - Environmental impact discount (unique to Supernova)
//! - Green mining bonus multiplier
//!
//! Tracks last 144 blocks (24 hours) of fee data and provides
//! percentile-based fee recommendations with environmental incentives.

use supernova_core::types::transaction::Transaction;
use std::collections::VecDeque;
use std::sync::{Arc, RwLock};
use std::time::SystemTime;

/// Minimum relay fee (novas per byte)
pub const MIN_RELAY_FEE: u64 = 1;

/// Number of blocks to track for fee estimation (144 blocks = 24 hours at 10 min/block)
pub const FEE_HISTORY_BLOCKS: usize = 144;

/// Fee priority levels
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FeePriority {
    /// Economy - lowest fee, slower confirmation
    Economy,
    /// Standard - balanced fee and confirmation time
    Standard,
    /// Priority - highest fee, fastest confirmation
    Priority,
}

/// Fee distribution statistics
#[derive(Debug, Clone)]
pub struct FeeDistribution {
    /// 25th percentile fee rate
    pub p25: u64,
    /// 50th percentile (median) fee rate
    pub p50: u64,
    /// 75th percentile fee rate
    pub p75: u64,
    /// 90th percentile fee rate
    pub p90: u64,
    /// Minimum fee rate observed
    pub min: u64,
    /// Maximum fee rate observed
    pub max: u64,
    /// Average fee rate
    pub avg: u64,
}

/// Historical fee data for a block
#[derive(Debug, Clone)]
struct BlockFeeData {
    /// Block height
    height: u64,
    /// Median fee rate in this block
    median_fee_rate: u64,
    /// Average fee rate in this block
    avg_fee_rate: u64,
    /// Environmental score (0.0 to 1.0)
    environmental_score: f64,
    /// Timestamp
    timestamp: SystemTime,
}

/// Fee estimator configuration
#[derive(Debug, Clone)]
pub struct FeeEstimatorConfig {
    /// Minimum relay fee (novas per byte)
    pub min_relay_fee: u64,
    /// Number of blocks to track
    pub history_blocks: usize,
    /// Maximum environmental discount (as percentage, e.g., 0.2 = 20%)
    pub max_environmental_discount: f64,
    /// Lightning channel priority boost multiplier
    pub lightning_boost_multiplier: f64,
    /// Congestion multiplier thresholds
    pub congestion_low_threshold: usize,
    pub congestion_high_threshold: usize,
}

impl Default for FeeEstimatorConfig {
    fn default() -> Self {
        Self {
            min_relay_fee: MIN_RELAY_FEE,
            history_blocks: FEE_HISTORY_BLOCKS,
            max_environmental_discount: 0.2, // 20% max discount
            lightning_boost_multiplier: 1.1, // 10% boost for Lightning
            congestion_low_threshold: 1000,   // Low congestion
            congestion_high_threshold: 5000, // High congestion
        }
    }
}

/// Supernova Dynamic Fee Estimator
pub struct FeeEstimator {
    /// Configuration
    config: FeeEstimatorConfig,
    /// Historical fee data (sliding window)
    fee_history: Arc<RwLock<VecDeque<BlockFeeData>>>,
    /// Current mempool size (for congestion calculation)
    mempool_size: Arc<RwLock<usize>>,
}

impl FeeEstimator {
    /// Create a new fee estimator
    pub fn new(config: FeeEstimatorConfig) -> Self {
        Self {
            config,
            fee_history: Arc::new(RwLock::new(VecDeque::with_capacity(FEE_HISTORY_BLOCKS))),
            mempool_size: Arc::new(RwLock::new(0)),
        }
    }

    /// Create with default configuration
    pub fn default() -> Self {
        Self::new(FeeEstimatorConfig::default())
    }

    /// Update fee history with a new block
    pub fn update_block(
        &self,
        height: u64,
        median_fee_rate: u64,
        avg_fee_rate: u64,
        environmental_score: f64,
    ) {
        let mut history = self.fee_history.write().unwrap();
        
        let block_data = BlockFeeData {
            height,
            median_fee_rate,
            avg_fee_rate,
            environmental_score: environmental_score.clamp(0.0, 1.0),
            timestamp: SystemTime::now(),
        };

        history.push_back(block_data);

        // Maintain sliding window
        while history.len() > self.config.history_blocks {
            history.pop_front();
        }
    }

    /// Update current mempool size for congestion calculation
    pub fn update_mempool_size(&self, size: usize) {
        *self.mempool_size.write().unwrap() = size;
    }

    /// Estimate fee for a given priority level
    pub fn estimate_fee(&self, priority: FeePriority) -> u64 {
        let history = self.fee_history.read().unwrap();
        
        if history.is_empty() {
            // No history, return minimum fee scaled by priority
            return match priority {
                FeePriority::Economy => self.config.min_relay_fee,
                FeePriority::Standard => self.config.min_relay_fee * 2,
                FeePriority::Priority => self.config.min_relay_fee * 5,
            };
        }

        // Calculate percentile-based fee
        let mut fee_rates: Vec<u64> = history.iter().map(|d| d.median_fee_rate).collect();
        fee_rates.sort();

        let len = fee_rates.len();
        let base_fee = match priority {
            FeePriority::Economy => fee_rates[len / 4].max(self.config.min_relay_fee),
            FeePriority::Standard => fee_rates[len / 2].max(self.config.min_relay_fee * 2),
            FeePriority::Priority => fee_rates[(len * 3) / 4].max(self.config.min_relay_fee * 5),
        };

        // Apply congestion multiplier
        let congestion_multiplier = self.calculate_congestion_multiplier();
        let base_fee = (base_fee as f64 * congestion_multiplier) as u64;

        // Apply environmental discount (use average environmental score)
        let avg_environmental_score: f64 = history
            .iter()
            .map(|d| d.environmental_score)
            .sum::<f64>()
            / history.len() as f64;

        let environmental_discount = base_fee as f64
            * (1.0 - avg_environmental_score * self.config.max_environmental_discount);

        let final_fee = environmental_discount.max(self.config.min_relay_fee as f64) as u64;

        final_fee
    }

    /// Estimate fee with environmental score for a specific transaction
    pub fn estimate_fee_with_environmental(
        &self,
        priority: FeePriority,
        environmental_score: f64,
    ) -> u64 {
        let base_fee = self.estimate_fee(priority);

        // Apply transaction-specific environmental discount
        let environmental_discount = base_fee as f64
            * (1.0 - environmental_score.clamp(0.0, 1.0) * self.config.max_environmental_discount);

        let final_fee = environmental_discount.max(self.config.min_relay_fee as f64) as u64;

        final_fee
    }

    /// Estimate fee for Lightning Network channel updates (with priority boost)
    pub fn estimate_fee_lightning(&self, priority: FeePriority) -> u64 {
        let base_fee = self.estimate_fee(priority);
        
        // Apply Lightning priority boost
        let boosted_fee = (base_fee as f64 * self.config.lightning_boost_multiplier) as u64;
        
        boosted_fee
    }

    /// Get fee distribution histogram
    pub fn get_fee_distribution(&self) -> FeeDistribution {
        let history = self.fee_history.read().unwrap();

        if history.is_empty() {
            return FeeDistribution {
                p25: self.config.min_relay_fee,
                p50: self.config.min_relay_fee * 2,
                p75: self.config.min_relay_fee * 3,
                p90: self.config.min_relay_fee * 5,
                min: self.config.min_relay_fee,
                max: self.config.min_relay_fee * 10,
                avg: self.config.min_relay_fee * 2,
            };
        }

        let mut fee_rates: Vec<u64> = history.iter().map(|d| d.median_fee_rate).collect();
        fee_rates.sort();

        let len = fee_rates.len();
        let p25 = fee_rates[len / 4];
        let p50 = fee_rates[len / 2];
        let p75 = fee_rates[(len * 3) / 4];
        let p90 = fee_rates[(len * 9) / 10];
        let min = *fee_rates.first().unwrap();
        let max = *fee_rates.last().unwrap();
        let avg = fee_rates.iter().sum::<u64>() / len as u64;

        FeeDistribution {
            p25,
            p50,
            p75,
            p90,
            min,
            max,
            avg,
        }
    }

    /// Estimate confirmation time (in blocks) for a given fee rate
    pub fn estimate_confirmation_time(&self, fee_rate: u64) -> u32 {
        let distribution = self.get_fee_distribution();

        // Compare fee rate to percentiles
        if fee_rate >= distribution.p90 {
            1 // Next block (high priority)
        } else if fee_rate >= distribution.p75 {
            2 // 2 blocks
        } else if fee_rate >= distribution.p50 {
            4 // 4 blocks
        } else if fee_rate >= distribution.p25 {
            8 // 8 blocks
        } else {
            16 // 16+ blocks (low priority)
        }
    }

    /// Get environmental discount for a transaction
    pub fn get_environmental_discount(&self, environmental_score: f64) -> f64 {
        environmental_score.clamp(0.0, 1.0) * self.config.max_environmental_discount
    }

    /// Calculate congestion multiplier based on mempool size
    fn calculate_congestion_multiplier(&self) -> f64 {
        let mempool_size = *self.mempool_size.read().unwrap();

        if mempool_size < self.config.congestion_low_threshold {
            // Low congestion - discount
            0.9
        } else if mempool_size < self.config.congestion_high_threshold {
            // Normal congestion - no multiplier
            1.0
        } else {
            // High congestion - premium
            1.0 + ((mempool_size - self.config.congestion_high_threshold) as f64 / 1000.0).min(0.5)
        }
    }

    /// Get current fee history length
    pub fn history_length(&self) -> usize {
        self.fee_history.read().unwrap().len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fee_estimation_basic() {
        let estimator = FeeEstimator::default();

        // Test with no history
        let economy_fee = estimator.estimate_fee(FeePriority::Economy);
        let standard_fee = estimator.estimate_fee(FeePriority::Standard);
        let priority_fee = estimator.estimate_fee(FeePriority::Priority);

        assert_eq!(economy_fee, MIN_RELAY_FEE);
        assert_eq!(standard_fee, MIN_RELAY_FEE * 2);
        assert_eq!(priority_fee, MIN_RELAY_FEE * 5);

        // Add some history
        for i in 0..10 {
            estimator.update_block(
                100 + i,
                100 + i * 10, // median fee
                100 + i * 10, // avg fee
                0.5,           // environmental score
            );
        }

        // Fees should be higher with history
        let economy_fee_with_history = estimator.estimate_fee(FeePriority::Economy);
        let standard_fee_with_history = estimator.estimate_fee(FeePriority::Standard);
        let priority_fee_with_history = estimator.estimate_fee(FeePriority::Priority);

        assert!(economy_fee_with_history >= economy_fee);
        assert!(standard_fee_with_history >= standard_fee);
        assert!(priority_fee_with_history >= priority_fee);
    }

    #[test]
    fn test_environmental_discount_calculation() {
        let estimator = FeeEstimator::default();

        // Add history with varying environmental scores
        for i in 0..10 {
            estimator.update_block(
                100 + i,
                1000, // base fee
                1000,
                if i < 5 { 0.8 } else { 0.2 }, // High env score for first 5
            );
        }

        // Fee with high environmental score should be lower
        let fee_high_env = estimator.estimate_fee_with_environmental(FeePriority::Standard, 0.9);
        let fee_low_env = estimator.estimate_fee_with_environmental(FeePriority::Standard, 0.1);

        assert!(fee_high_env < fee_low_env || fee_high_env == fee_low_env);
    }

    #[test]
    fn test_congestion_multiplier() {
        let estimator = FeeEstimator::default();

        // Add some history
        for i in 0..10 {
            estimator.update_block(100 + i, 1000, 1000, 0.5);
        }

        // Low congestion
        estimator.update_mempool_size(500);
        let fee_low = estimator.estimate_fee(FeePriority::Standard);

        // High congestion
        estimator.update_mempool_size(10000);
        let fee_high = estimator.estimate_fee(FeePriority::Standard);

        assert!(fee_high >= fee_low);
    }

    #[test]
    fn test_sliding_window_update() {
        let estimator = FeeEstimator::default();

        // Add more blocks than history size
        for i in 0..200 {
            estimator.update_block(100 + i, 1000, 1000, 0.5);
        }

        // History should be capped at FEE_HISTORY_BLOCKS
        assert_eq!(estimator.history_length(), FEE_HISTORY_BLOCKS);
    }

    #[test]
    fn test_percentile_calculations() {
        let estimator = FeeEstimator::default();

        // Add history with known values
        for i in 0..100 {
            estimator.update_block(100 + i, (i + 1) * 10, (i + 1) * 10, 0.5);
        }

        let distribution = estimator.get_fee_distribution();

        // Verify percentiles are reasonable
        assert!(distribution.p25 < distribution.p50);
        assert!(distribution.p50 < distribution.p75);
        assert!(distribution.p75 < distribution.p90);
        assert!(distribution.min <= distribution.p25);
        assert!(distribution.p90 <= distribution.max);
    }

    #[test]
    fn test_minimum_fee_enforcement() {
        let estimator = FeeEstimator::default();

        // Add history with very low fees
        for i in 0..10 {
            estimator.update_block(100 + i, 0, 0, 0.5);
        }

        // Even with zero fees in history, should return minimum
        let fee = estimator.estimate_fee(FeePriority::Economy);
        assert_eq!(fee, MIN_RELAY_FEE);
    }

    #[test]
    fn test_lightning_priority_boost() {
        let estimator = FeeEstimator::default();

        // Add some history
        for i in 0..10 {
            estimator.update_block(100 + i, 1000, 1000, 0.5);
        }

        let standard_fee = estimator.estimate_fee(FeePriority::Standard);
        let lightning_fee = estimator.estimate_fee_lightning(FeePriority::Standard);

        // Lightning fee should be higher due to boost
        assert!(lightning_fee >= standard_fee);
    }

    #[test]
    fn test_estimate_confirmation_time() {
        let estimator = FeeEstimator::default();

        // Add history
        for i in 0..100 {
            estimator.update_block(100 + i, (i + 1) * 10, (i + 1) * 10, 0.5);
        }

        let distribution = estimator.get_fee_distribution();

        // High fee should confirm quickly
        let time_high = estimator.estimate_confirmation_time(distribution.p90);
        assert_eq!(time_high, 1);

        // Low fee should take longer
        let time_low = estimator.estimate_confirmation_time(distribution.p25);
        assert!(time_low >= 8);
    }

    #[test]
    fn test_environmental_discount_range() {
        let estimator = FeeEstimator::default();

        // Discount should be clamped to valid range
        let discount_high = estimator.get_environmental_discount(1.5); // > 1.0
        let discount_low = estimator.get_environmental_discount(-0.5); // < 0.0
        let discount_normal = estimator.get_environmental_discount(0.5);

        assert!(discount_high >= 0.0 && discount_high <= 1.0);
        assert!(discount_low >= 0.0 && discount_low <= 1.0);
        assert!(discount_normal >= 0.0 && discount_normal <= 1.0);
    }
}

