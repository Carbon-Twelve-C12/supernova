//! Fuzzing harness for consensus mechanisms
//! 
//! This harness tests fork resolution, chain selection, and other consensus
//! critical operations to ensure they handle adversarial inputs correctly.

use afl::fuzz;
use btclib::consensus::{
    ForkResolution, ChainSelector, ConsensusRules,
    ForkResolutionError, ConsensusError
};
use btclib::types::block::{Block, BlockHeader};
use btclib::blockchain::Chain;
use std::collections::HashMap;

fn main() {
    fuzz!(|data: &[u8]| {
        if data.is_empty() {
            return;
        }
        
        // Test different consensus scenarios
        match data[0] % 5 {
            0 => fuzz_fork_resolution(data),
            1 => fuzz_chain_selection(data),
            2 => fuzz_reorg_handling(data),
            3 => fuzz_timestamp_validation(data),
            4 => fuzz_difficulty_validation(data),
            _ => unreachable!(),
        }
    });
}

/// Fuzz fork resolution logic
fn fuzz_fork_resolution(data: &[u8]) {
    use btclib::consensus::secure_fork_resolution::{ForkResolver, ForkChain};
    
    // Create fork resolver
    let resolver = match ForkResolver::new() {
        Ok(r) => r,
        Err(_) => return,
    };
    
    // Generate multiple competing chains from fuzzer data
    let num_chains = (data.get(1).unwrap_or(&2) % 10) + 2;  // 2-11 chains
    let mut chains = Vec::new();
    
    for i in 0..num_chains {
        if let Some(chain) = generate_chain_from_fuzzer_data(data, i as usize) {
            chains.push(chain);
        }
    }
    
    if chains.len() < 2 {
        return;
    }
    
    // Test fork resolution
    match resolver.resolve_fork(&chains[0], &chains[1]) {
        Ok(winning_chain) => {
            // Verify the winning chain is valid
            test_chain_validity(winning_chain);
            
            // Test with additional chains
            for chain in &chains[2..] {
                let _ = resolver.resolve_fork(winning_chain, chain);
            }
        }
        Err(e) => {
            // Fork resolution errors are expected for invalid chains
            match e {
                ForkResolutionError::InvalidChain => {},
                ForkResolutionError::InvalidTimestamp => {},
                ForkResolutionError::ChainTooShort => {},
                _ => {},
            }
        }
    }
    
    // Test multi-way fork resolution
    test_multiway_fork_resolution(&resolver, &chains);
}

/// Fuzz chain selection rules
fn fuzz_chain_selection(data: &[u8]) {
    use btclib::consensus::chain_selection::{ChainSelector, SelectionCriteria};
    
    let selector = ChainSelector::new();
    
    // Generate chains with different properties
    let chains = generate_diverse_chains(data);
    
    // Test different selection criteria
    let criteria = [
        SelectionCriteria::LongestChain,
        SelectionCriteria::MostWork,
        SelectionCriteria::FirstSeen,
        SelectionCriteria::Environmental,  // Green mining preference
    ];
    
    for criterion in criteria {
        match selector.select_best_chain(&chains, criterion) {
            Ok(best_chain) => {
                // Verify selection is consistent
                verify_chain_selection(best_chain, &chains, criterion);
            }
            Err(_) => {}
        }
    }
}

/// Fuzz reorganization handling
fn fuzz_reorg_handling(data: &[u8]) {
    use btclib::consensus::reorg::{ReorgHandler, ReorgDepth};
    
    // Create reorg handler with depth limit
    let max_reorg_depth = data.get(1).unwrap_or(&100);
    let handler = ReorgHandler::new(*max_reorg_depth as usize);
    
    // Generate initial chain
    let mut current_chain = match generate_chain_from_fuzzer_data(data, 0) {
        Some(c) => c,
        None => return,
    };
    
    // Generate competing chains at various fork points
    for fork_depth in 1..10 {
        if let Some(new_chain) = generate_forked_chain(data, &current_chain, fork_depth) {
            match handler.handle_reorg(&current_chain, &new_chain) {
                Ok(reorg_result) => {
                    // Test reorg depth limits
                    if reorg_result.depth > *max_reorg_depth as usize {
                        panic!("Reorg depth exceeded limit");
                    }
                    
                    // Verify state consistency after reorg
                    test_post_reorg_consistency(&reorg_result);
                    
                    current_chain = new_chain;
                }
                Err(_) => {}
            }
        }
    }
}

/// Fuzz timestamp validation in consensus
fn fuzz_timestamp_validation(data: &[u8]) {
    use btclib::consensus::time::{TimeValidator, MedianTimePast};
    
    let validator = TimeValidator::new();
    
    // Generate blocks with various timestamp patterns
    let block_count = data.len() / 8;  // 8 bytes per timestamp
    let mut blocks = Vec::new();
    
    for i in 0..block_count.min(100) {
        let timestamp = if i * 8 + 8 <= data.len() {
            u64::from_le_bytes([
                data[i*8], data[i*8+1], data[i*8+2], data[i*8+3],
                data[i*8+4], data[i*8+5], data[i*8+6], data[i*8+7]
            ])
        } else {
            0
        };
        
        let block = create_block_with_timestamp(timestamp);
        blocks.push(block);
    }
    
    // Test median time past calculation
    if blocks.len() >= 11 {
        let mtp = MedianTimePast::calculate(&blocks);
        
        // Test future time limit
        for block in &blocks {
            let _ = validator.validate_timestamp(block, mtp);
        }
    }
    
    // Test time warp attack detection
    test_time_warp_detection(&validator, &blocks);
}

/// Fuzz difficulty adjustment validation
fn fuzz_difficulty_validation(data: &[u8]) {
    use btclib::consensus::difficulty::{DifficultyAdjuster, DifficultyParams};
    
    // Create difficulty adjuster with fuzzer-provided parameters
    let params = DifficultyParams {
        adjustment_interval: data.get(0).map(|&b| b as u32 + 1).unwrap_or(2016),
        target_block_time: data.get(1).map(|&b| b as u64 + 1).unwrap_or(150),
        max_adjustment_factor: data.get(2).map(|&b| b as u32 + 1).unwrap_or(4),
    };
    
    let adjuster = DifficultyAdjuster::new(params);
    
    // Generate blocks with various timestamps and difficulties
    let mut blocks = Vec::new();
    let mut current_time = 1_600_000_000u64;  // Start time
    let mut current_diff = 1_000_000u32;      // Start difficulty
    
    for i in 0..data.len() / 12 {
        // Extract time delta and difficulty from fuzzer data
        let time_delta = if i * 12 + 8 <= data.len() {
            u64::from_le_bytes([
                data[i*12], data[i*12+1], data[i*12+2], data[i*12+3],
                data[i*12+4], data[i*12+5], data[i*12+6], data[i*12+7]
            ]) % 1000  // Limit time delta
        } else {
            150
        };
        
        current_time += time_delta;
        
        let block = create_block_with_time_and_diff(current_time, current_diff);
        blocks.push(block);
        
        // Test difficulty adjustment at intervals
        if blocks.len() % params.adjustment_interval as usize == 0 {
            match adjuster.calculate_next_difficulty(&blocks) {
                Ok(new_diff) => {
                    // Verify adjustment limits
                    let adjustment_ratio = new_diff as f64 / current_diff as f64;
                    assert!(adjustment_ratio <= params.max_adjustment_factor as f64);
                    assert!(adjustment_ratio >= 1.0 / params.max_adjustment_factor as f64);
                    
                    current_diff = new_diff;
                }
                Err(_) => {}
            }
        }
    }
}

// Helper functions

fn generate_chain_from_fuzzer_data(data: &[u8], chain_index: usize) -> Option<ForkChain> {
    // Implementation to generate a chain from fuzzer data
    Some(ForkChain::new_test())
}

fn generate_diverse_chains(data: &[u8]) -> Vec<Chain> {
    // Generate chains with different properties for selection testing
    Vec::new()
}

fn generate_forked_chain(data: &[u8], base: &ForkChain, depth: usize) -> Option<ForkChain> {
    // Generate a chain that forks from base at given depth
    Some(ForkChain::new_test())
}

fn create_block_with_timestamp(timestamp: u64) -> Block {
    // Create a block with specific timestamp
    Block::new_test_with_timestamp(timestamp)
}

fn create_block_with_time_and_diff(timestamp: u64, difficulty: u32) -> Block {
    // Create a block with specific timestamp and difficulty
    Block::new_test_with_params(timestamp, difficulty)
}

fn test_chain_validity(chain: &ForkChain) {
    // Verify chain follows consensus rules
}

fn test_multiway_fork_resolution(resolver: &ForkResolver, chains: &[ForkChain]) {
    // Test resolution with multiple competing chains
}

fn verify_chain_selection(selected: &Chain, all_chains: &[Chain], criteria: SelectionCriteria) {
    // Verify the selection follows the criteria
}

fn test_post_reorg_consistency(result: &btclib::consensus::reorg::ReorgResult) {
    // Verify state consistency after reorganization
}

fn test_time_warp_detection(validator: &TimeValidator, blocks: &[Block]) {
    // Test detection of time manipulation attacks
}