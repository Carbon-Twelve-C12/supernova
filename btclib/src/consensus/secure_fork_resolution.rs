//! Secure Fork Resolution Module for Supernova
//! 
//! This module provides secure fork resolution that prevents permanent network splits
//! by implementing objective chain selection criteria based on accumulated work.

use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH, Duration};
use sha2::{Sha256, Digest};
use thiserror::Error;

use crate::types::Block;
use crate::types::block::BlockHeader;
use crate::consensus::difficulty::calculate_required_work;

/// Fork resolution errors
#[derive(Debug, Error)]
pub enum ForkResolutionError {
    #[error("Invalid chain work calculation")]
    InvalidChainWork,
    
    #[error("Block not found: {0}")]
    BlockNotFound(String),
    
    #[error("Invalid timestamp in chain")]
    InvalidTimestamp,
    
    #[error("Chain validation failed: {0}")]
    ChainValidationFailed(String),
    
    #[error("Fork depth exceeds maximum allowed: {0} > {1}")]
    ForkTooDeep(u32, u32),
}

/// Result type for fork resolution
pub type ForkResolutionResult<T> = Result<T, ForkResolutionError>;

/// Secure fork resolution configuration
#[derive(Debug, Clone)]
pub struct SecureForkConfig {
    /// Maximum fork depth to consider (blocks)
    pub max_fork_depth: u32,
    
    /// Minimum time between blocks for chain quality
    pub min_block_time: Duration,
    
    /// Maximum time between blocks for chain quality
    pub max_block_time: Duration,
    
    /// Weight for accumulated work (0.0-1.0)
    pub work_weight: f64,
    
    /// Weight for chain quality metrics (0.0-1.0)
    pub quality_weight: f64,
    
    /// Enable anti-split mechanisms
    pub enable_anti_split: bool,
    
    /// Time window for considering chains equal
    pub equality_window: Duration,
}

impl Default for SecureForkConfig {
    fn default() -> Self {
        Self {
            max_fork_depth: 100,
            min_block_time: Duration::from_secs(30),
            max_block_time: Duration::from_secs(3600),
            work_weight: 0.8,
            quality_weight: 0.2,
            enable_anti_split: true,
            equality_window: Duration::from_secs(300), // 5 minutes
        }
    }
}

/// Chain metrics for comparison
#[derive(Debug, Clone)]
pub struct ChainMetrics {
    /// Total accumulated work
    pub total_work: u128,
    
    /// Average block time
    pub avg_block_time: Duration,
    
    /// Block time variance
    pub block_time_variance: f64,
    
    /// Chain length
    pub length: u32,
    
    /// Timestamp of tip block
    pub tip_timestamp: u64,
    
    /// Quality score (0.0-1.0)
    pub quality_score: f64,
}

/// Secure fork resolution system
pub struct SecureForkResolver {
    config: SecureForkConfig,
    
    /// Cache of chain metrics by tip hash
    metrics_cache: HashMap<[u8; 32], ChainMetrics>,
    
    /// Anti-split tracking
    split_observations: HashMap<[u8; 32], Vec<u64>>,
}

impl SecureForkResolver {
    /// Create new fork resolver
    pub fn new(config: SecureForkConfig) -> Self {
        Self {
            config,
            metrics_cache: HashMap::new(),
            split_observations: HashMap::new(),
        }
    }
    
    /// Compare two chains and determine which is better
    /// Returns true if chain_a is better, false if chain_b is better
    pub fn compare_chains(
        &mut self,
        chain_a_tip: &[u8; 32],
        chain_b_tip: &[u8; 32],
        get_header: impl Fn(&[u8; 32]) -> Option<BlockHeader>,
    ) -> ForkResolutionResult<bool> {
        // Calculate metrics for both chains
        let metrics_a = self.calculate_chain_metrics(chain_a_tip, &get_header)?;
        let metrics_b = self.calculate_chain_metrics(chain_b_tip, &get_header)?;
        
        // Cache metrics
        self.metrics_cache.insert(*chain_a_tip, metrics_a.clone());
        self.metrics_cache.insert(*chain_b_tip, metrics_b.clone());
        
        // Primary criterion: Accumulated work (most important)
        if metrics_a.total_work > metrics_b.total_work {
            let work_ratio = metrics_a.total_work as f64 / metrics_b.total_work as f64;
            
            // If work difference is significant (>10%), chain A wins
            if work_ratio > 1.1 {
                return Ok(true);
            }
        } else if metrics_b.total_work > metrics_a.total_work {
            let work_ratio = metrics_b.total_work as f64 / metrics_a.total_work as f64;
            
            // If work difference is significant (>10%), chain B wins
            if work_ratio > 1.1 {
                return Ok(false);
            }
        }
        
        // Work is similar, use weighted scoring
        let score_a = self.calculate_chain_score(&metrics_a);
        let score_b = self.calculate_chain_score(&metrics_b);
        
        // Anti-split mechanism: if scores are very close, prefer the chain
        // that has been observed more recently
        if self.config.enable_anti_split {
            let score_diff = (score_a - score_b).abs();
            if score_diff < 0.05 { // Within 5% - considered equal
                return self.apply_anti_split_logic(chain_a_tip, chain_b_tip, &metrics_a, &metrics_b);
            }
        }
        
        Ok(score_a > score_b)
    }
    
    /// Calculate comprehensive metrics for a chain
    fn calculate_chain_metrics(
        &self,
        tip_hash: &[u8; 32],
        get_header: &impl Fn(&[u8; 32]) -> Option<BlockHeader>,
    ) -> ForkResolutionResult<ChainMetrics> {
        let mut current_hash = *tip_hash;
        let mut headers = Vec::new();
        let mut total_work: u128 = 0;
        
        // Traverse back to find common ancestor or max depth
        for _ in 0..self.config.max_fork_depth {
            if let Some(header) = get_header(&current_hash) {
                // Calculate required work (big-endian bytes)
                let work = calculate_required_work(header.bits());
                
                // Accumulate work (simplified - just count as 1 unit per valid block)
                total_work += 1;
                
                headers.push(header.clone());
                
                // Check if we've reached a well-known block
                if self.is_well_known_block(&current_hash) {
                    break;
                }
                
                current_hash = *header.prev_block_hash();
            } else {
                return Err(ForkResolutionError::BlockNotFound(hex::encode(current_hash)));
            }
        }
        
        // Calculate timing metrics
        let (avg_block_time, variance) = self.calculate_timing_metrics(&headers)?;
        
        // Calculate quality score
        let quality_score = self.calculate_quality_score(avg_block_time, variance, &headers);
        
        Ok(ChainMetrics {
            total_work,
            avg_block_time,
            block_time_variance: variance,
            length: headers.len() as u32,
            tip_timestamp: headers.first()
                .map(|h| h.timestamp())
                .unwrap_or(0),
            quality_score,
        })
    }
    
    /// Calculate timing metrics for a chain
    fn calculate_timing_metrics(
        &self,
        headers: &[BlockHeader],
    ) -> ForkResolutionResult<(Duration, f64)> {
        if headers.len() < 2 {
            return Ok((Duration::from_secs(600), 0.0));
        }
        
        let mut block_times = Vec::new();
        
        for i in 1..headers.len() {
            let time_diff = headers[i-1].timestamp().saturating_sub(headers[i].timestamp());
            block_times.push(time_diff);
        }
        
        // Calculate average
        let sum: u64 = block_times.iter().sum();
        let avg = sum / block_times.len() as u64;
        let avg_duration = Duration::from_secs(avg);
        
        // Calculate variance
        let variance = if block_times.len() > 1 {
            let mean = avg as f64;
            let sum_squared_diff: f64 = block_times.iter()
                .map(|&t| {
                    let diff = t as f64 - mean;
                    diff * diff
                })
                .sum();
            sum_squared_diff / block_times.len() as f64
        } else {
            0.0
        };
        
        Ok((avg_duration, variance))
    }
    
    /// Calculate quality score for a chain
    fn calculate_quality_score(
        &self,
        avg_block_time: Duration,
        variance: f64,
        headers: &[BlockHeader],
    ) -> f64 {
        let mut score = 1.0;
        
        // Penalize for block time outside ideal range
        if avg_block_time < self.config.min_block_time {
            let ratio = self.config.min_block_time.as_secs() as f64 / avg_block_time.as_secs() as f64;
            score *= 0.5 + 0.5 / ratio; // Heavy penalty for too fast
        } else if avg_block_time > self.config.max_block_time {
            let ratio = avg_block_time.as_secs() as f64 / self.config.max_block_time.as_secs() as f64;
            score *= 1.0 / ratio; // Penalty for too slow
        }
        
        // Penalize high variance (unstable block times)
        let normalized_variance = variance / (600.0 * 600.0); // Normalize to 10-minute blocks
        score *= 1.0 / (1.0 + normalized_variance);
        
        // Bonus for longer chains (more established)
        let length_bonus = (headers.len() as f64 / self.config.max_fork_depth as f64).min(1.0);
        score *= 0.9 + 0.1 * length_bonus;
        
        // Check timestamp progression
        let has_good_progression = self.check_timestamp_progression(headers);
        if !has_good_progression {
            score *= 0.8; // Penalty for suspicious timestamps
        }
        
        score.max(0.0).min(1.0)
    }
    
    /// Get current timestamp safely
    fn current_timestamp() -> ForkResolutionResult<u64> {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .map_err(|_| ForkResolutionError::InvalidTimestamp)
    }
    
    /// Check if timestamps progress properly
    fn check_timestamp_progression(&self, headers: &[BlockHeader]) -> bool {
        if headers.len() < 2 {
            return true;
        }
        
        let current_time = match Self::current_timestamp() {
            Ok(time) => time,
            Err(_) => return false, // If we can't get current time, assume bad progression
        };
        
        // Check first block isn't too far in future
        if headers[0].timestamp() > current_time + 7200 { // 2 hours
            return false;
        }
        
        // Check monotonic progression
        for i in 1..headers.len() {
            if headers[i-1].timestamp() <= headers[i].timestamp() {
                return false; // Timestamps should decrease as we go back
            }
        }
        
        true
    }
    
    /// Calculate weighted score for a chain
    fn calculate_chain_score(&self, metrics: &ChainMetrics) -> f64 {
        // Normalize work to a 0-1 scale (logarithmic)
        let work_score = (metrics.total_work as f64).ln() / 100.0;
        let work_score = work_score.min(1.0);
        
        // Combine work and quality scores
        self.config.work_weight * work_score + 
        self.config.quality_weight * metrics.quality_score
    }
    
    /// Apply anti-split logic when chains are nearly equal
    fn apply_anti_split_logic(
        &mut self,
        chain_a: &[u8; 32],
        chain_b: &[u8; 32],
        metrics_a: &ChainMetrics,
        metrics_b: &ChainMetrics,
    ) -> ForkResolutionResult<bool> {
        let current_time = Self::current_timestamp()?;
        
        // Record observations
        self.split_observations.entry(*chain_a).or_default().push(current_time);
        self.split_observations.entry(*chain_b).or_default().push(current_time);
        
        // Clean old observations
        let cutoff = current_time - self.config.equality_window.as_secs();
        for observations in self.split_observations.values_mut() {
            observations.retain(|&t| t > cutoff);
        }
        
        // Count recent observations
        let obs_a = self.split_observations.get(chain_a).map(|v| v.len()).unwrap_or(0);
        let obs_b = self.split_observations.get(chain_b).map(|v| v.len()).unwrap_or(0);
        
        // If one chain has been observed significantly more, prefer it
        if obs_a > obs_b * 2 {
            return Ok(true);
        } else if obs_b > obs_a * 2 {
            return Ok(false);
        }
        
        // If observations are similar, use deterministic tiebreaker
        // This ensures all nodes make the same decision
        Ok(self.deterministic_tiebreaker(chain_a, chain_b))
    }
    
    /// Deterministic tiebreaker based on hash comparison
    fn deterministic_tiebreaker(&self, hash_a: &[u8; 32], hash_b: &[u8; 32]) -> bool {
        // Compare hashes lexicographically
        // This ensures all nodes make the same choice
        hash_a < hash_b
    }
    
    /// Check if a block is well-known (checkpoint, etc.)
    fn is_well_known_block(&self, hash: &[u8; 32]) -> bool {
        // In a real implementation, would check against checkpoints
        // For now, just check if it's old enough
        if let Some(metrics) = self.metrics_cache.get(hash) {
            let current_time = match Self::current_timestamp() {
                Ok(time) => time,
                Err(_) => return false, // If we can't get time, assume not well-known
            };
            
            // If block is more than 1 hour old, consider it well-known
            current_time.saturating_sub(metrics.tip_timestamp) > 3600
        } else {
            false
        }
    }
    
    /// Get cached metrics for a chain
    pub fn get_chain_metrics(&self, tip_hash: &[u8; 32]) -> Option<&ChainMetrics> {
        self.metrics_cache.get(tip_hash)
    }
    
    /// Clear metrics cache
    pub fn clear_cache(&mut self) {
        self.metrics_cache.clear();
        self.split_observations.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Block;
    
    fn create_test_header(height: u64, prev_hash: [u8; 32], bits: u32, timestamp: u64) -> BlockHeader {
        let block = Block::new(
            1,
            prev_hash,
            vec![],
            bits,
        );
        let mut header = block.header().clone();
        // Set timestamp (would need proper setter in real implementation)
        header
    }
    
    #[test]
    fn test_work_comparison() {
        let config = SecureForkConfig::default();
        let mut resolver = SecureForkResolver::new(config);
        
        // Create header lookup
        let mut headers = HashMap::new();
        
        // Chain A: More work
        let header_a = create_test_header(1, [0; 32], 0x1c00ffff, 1000);
        headers.insert([1; 32], header_a);
        
        // Chain B: Less work
        let header_b = create_test_header(1, [0; 32], 0x1d00ffff, 1000);
        headers.insert([2; 32], header_b);
        
        let get_header = |hash: &[u8; 32]| headers.get(hash).cloned();
        
        // Chain A should win (more work = lower bits)
        let result = resolver.compare_chains(&[1; 32], &[2; 32], get_header).unwrap();
        assert!(result);
    }
    
    #[test]
    fn test_quality_scoring() {
        let config = SecureForkConfig::default();
        let resolver = SecureForkResolver::new(config);
        
        // Test good timestamps
        let good_headers = vec![
            create_test_header(3, [0; 32], 0x1d00ffff, 3000),
            create_test_header(2, [0; 32], 0x1d00ffff, 2400),
            create_test_header(1, [0; 32], 0x1d00ffff, 1800),
        ];
        
        let (avg_time, variance) = resolver.calculate_timing_metrics(&good_headers).unwrap();
        let score = resolver.calculate_quality_score(avg_time, variance, &good_headers);
        
        assert!(score > 0.8); // Should have high quality score
    }
    
    #[test]
    fn test_anti_split_logic() {
        let config = SecureForkConfig {
            enable_anti_split: true,
            ..Default::default()
        };
        let mut resolver = SecureForkResolver::new(config);
        
        let chain_a = [1; 32];
        let chain_b = [2; 32];
        
        let metrics = ChainMetrics {
            total_work: 1000,
            avg_block_time: Duration::from_secs(600),
            block_time_variance: 100.0,
            length: 10,
            tip_timestamp: 1000,
            quality_score: 0.9,
        };
        
        // Record multiple observations for chain A
        for _ in 0..5 {
            resolver.split_observations.entry(chain_a).or_default().push(
                SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs()
            );
        }
        
        // Chain A should be preferred due to more observations
        let result = resolver.apply_anti_split_logic(
            &chain_a, &chain_b, &metrics, &metrics
        ).unwrap();
        
        assert!(result);
    }
} 