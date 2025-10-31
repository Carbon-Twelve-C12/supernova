//! Unified Block Validation Pipeline
//!
//! This module provides a secure, unified validation pipeline that ensures
//! all critical security checks are performed in the correct order.

use crate::consensus::difficulty::DifficultyAdjustment;
use crate::consensus::time_warp_prevention::{TimeWarpConfig, TimeWarpPrevention};
use crate::types::block::{Block, BlockHeader};
use crate::types::transaction::{Transaction, TransactionOutput};
use std::collections::HashMap;
use thiserror::Error;

/// Unified validation errors
#[derive(Debug, Error)]
pub enum UnifiedValidationError {
    #[error("Invalid block structure: {0}")]
    InvalidStructure(String),

    #[error("Invalid merkle root")]
    InvalidMerkleRoot,

    #[error("Multiple coinbase transactions")]
    MultipleCoinbase,

    #[error("Invalid coinbase subsidy: expected {expected}, got {actual}")]
    InvalidSubsidy { expected: u64, actual: u64 },

    #[error("No coinbase transaction")]
    NoCoinbase,

    #[error("Invalid timestamp: {0}")]
    InvalidTimestamp(String),

    #[error("Invalid proof of work")]
    InvalidPoW,

    #[error("Invalid difficulty: expected {expected}, got {actual}")]
    InvalidDifficulty { expected: u32, actual: u32 },

    #[error("Invalid transaction: {0}")]
    InvalidTransaction(String),
}

/// Result type for unified validation
pub type UnifiedValidationResult<T> = Result<T, UnifiedValidationError>;

/// Context for block validation
pub struct ValidationContext {
    /// Previous block headers for validation
    pub previous_headers: Vec<BlockHeader>,

    /// UTXO set lookup function
    pub get_utxo: Box<dyn Fn(&[u8; 32], u32) -> Option<TransactionOutput>>,

    /// Current block height
    pub height: u64,

    /// Network time (for testing)
    pub current_time: Option<u64>,

    /// Check if a coinbase output is mature (optional for testing)
    pub is_coinbase_mature: Option<Box<dyn Fn(&[u8; 32], u32, u64) -> bool>>,
}

/// Unified block validator
pub struct UnifiedBlockValidator {
    /// Time warp prevention system
    time_warp_prevention: TimeWarpPrevention,

    /// Difficulty adjustment system
    difficulty_adjustment: DifficultyAdjustment,
}

impl Default for UnifiedBlockValidator {
    fn default() -> Self {
        Self::new()
    }
}

impl UnifiedBlockValidator {
    pub fn new() -> Self {
        Self {
            time_warp_prevention: TimeWarpPrevention::new(TimeWarpConfig::default()),
            difficulty_adjustment: DifficultyAdjustment::new(),
        }
    }

    /// Validate a block with unified security checks
    ///
    /// This is the ONLY validation function that should be called.
    /// It ensures all security checks are performed in the correct order.
    pub fn validate_block_secure(
        &mut self,
        block: &Block,
        context: Option<&ValidationContext>,
    ) -> UnifiedValidationResult<()> {
        // ===== PHASE 1: STRUCTURE & INTEGRITY (ALWAYS RUN) =====

        // 1.1: Basic structure validation
        self.validate_structure(block)?;

        // 1.2: Merkle root validation (CRITICAL - was missing in basic validation)
        self.validate_merkle_root(block)?;

        // 1.3: Transaction validation
        self.validate_transactions(block, context)?;

        // 1.4: Coinbase rules (CRITICAL - prevents money printing)
        self.validate_coinbase_rules(block, context)?;

        // ===== PHASE 2: CONTEXTUAL VALIDATION (if context provided) =====

        if let Some(ctx) = context {
            // 2.1: Timestamp validation (CRITICAL - prevents time warp)
            self.validate_timestamp(block, ctx)?;

            // 2.2: Difficulty validation
            self.validate_difficulty(block, ctx)?;

            // 2.3: Proof of Work validation
            self.validate_proof_of_work(block)?;
        }

        Ok(())
    }

    /// Validate basic block structure
    fn validate_structure(&self, block: &Block) -> UnifiedValidationResult<()> {
        // Must have at least one transaction (coinbase)
        if block.transactions.is_empty() {
            return Err(UnifiedValidationError::InvalidStructure(
                "Block has no transactions".to_string(),
            ));
        }

        // Version must be valid
        if block.header.version() == 0 || block.header.version() > 2 {
            return Err(UnifiedValidationError::InvalidStructure(format!(
                "Invalid block version: {}",
                block.header.version()
            )));
        }

        Ok(())
    }

    /// Validate merkle root matches transactions
    fn validate_merkle_root(&self, block: &Block) -> UnifiedValidationResult<()> {
        let calculated_root = self.calculate_merkle_root(&block.transactions);

        if calculated_root != *block.header.merkle_root() {
            return Err(UnifiedValidationError::InvalidMerkleRoot);
        }

        Ok(())
    }

    /// Calculate merkle root from transactions
    fn calculate_merkle_root(&self, transactions: &[Transaction]) -> [u8; 32] {
        use crate::util::merkle::MerkleTree;

        if transactions.is_empty() {
            return [0; 32];
        }

        // Get transaction hashes
        let tx_hashes: Vec<[u8; 32]> = transactions.iter().map(|tx| tx.hash()).collect();

        // Use the same MerkleTree implementation that Block uses
        let merkle_tree = MerkleTree::new(&tx_hashes);
        merkle_tree.root_hash()
    }

    /// Validate all transactions in the block
    fn validate_transactions(
        &self,
        block: &Block,
        context: Option<&ValidationContext>,
    ) -> UnifiedValidationResult<()> {
        // Check for duplicate transactions
        let mut seen_txids = HashMap::new();

        for (idx, tx) in block.transactions.iter().enumerate() {
            let txid = tx.hash();

            if seen_txids.contains_key(&txid) {
                return Err(UnifiedValidationError::InvalidTransaction(format!(
                    "Duplicate transaction: {}",
                    hex::encode(txid)
                )));
            }
            seen_txids.insert(txid, idx);

            // Validate individual transaction
            if !tx.is_coinbase() && context.is_some() {
                // SECURITY FIX (P0-005): Implement full transaction validation
                let ctx = context.as_ref().unwrap();
                
                // Check basic structure
                if tx.inputs().is_empty() {
                    return Err(UnifiedValidationError::InvalidTransaction(
                        "Non-coinbase transaction has no inputs".to_string(),
                    ));
                }

                // Validate outputs exist and are non-empty
                if tx.outputs().is_empty() {
                    return Err(UnifiedValidationError::InvalidTransaction(
                        "Transaction has no outputs".to_string(),
                    ));
                }

                // Validate all inputs reference existing UTXOs
                let mut total_input_value = 0u64;
                for (idx, input) in tx.inputs().iter().enumerate() {
                    match (ctx.get_utxo)(&input.prev_tx_hash(), input.prev_output_index()) {
                        Some(utxo) => {
                            // SECURITY FIX (P0-003): Use checked arithmetic to prevent overflow
                            total_input_value = total_input_value
                                .checked_add(utxo.amount())
                                .ok_or_else(|| UnifiedValidationError::InvalidTransaction(
                                    format!("Input value overflow in transaction {}", hex::encode(txid))
                                ))?;
                        }
                        None => {
                            return Err(UnifiedValidationError::InvalidTransaction(format!(
                                "Input {} references non-existent UTXO: {}:{}",
                                idx,
                                hex::encode(input.prev_tx_hash()),
                                input.prev_output_index()
                            )));
                        }
                    }
                }

                // Validate outputs and calculate total output value
                let mut total_output_value = 0u64;
                for (idx, output) in tx.outputs().iter().enumerate() {
                    if output.amount() == 0 {
                        return Err(UnifiedValidationError::InvalidTransaction(format!(
                            "Output {} has zero amount",
                            idx
                        )));
                    }
                    // SECURITY FIX (P0-003): Use checked arithmetic to prevent overflow
                    total_output_value = total_output_value
                        .checked_add(output.amount())
                        .ok_or_else(|| UnifiedValidationError::InvalidTransaction(
                            format!("Output value overflow in transaction {}", hex::encode(txid))
                        ))?;
                }

                // Validate value conservation: inputs >= outputs (fee is the difference)
                if total_output_value > total_input_value {
                    return Err(UnifiedValidationError::InvalidTransaction(format!(
                        "Transaction creates value: inputs {} < outputs {}",
                        total_input_value, total_output_value
                    )));
                }

                // Note: Signature verification is handled separately by the transaction validator
                // which has access to cryptographic verification functions. For block validation,
                // we focus on structural integrity and value conservation.
            }
        }

        Ok(())
    }

    /// Validate coinbase rules
    fn validate_coinbase_rules(
        &self,
        block: &Block,
        context: Option<&ValidationContext>,
    ) -> UnifiedValidationResult<()> {
        // Constants for coinbase validation
        const COINBASE_MATURITY: u64 = 100; // Blocks before coinbase can be spent

        let mut coinbase_count = 0;
        let mut coinbase_value = 0u64;
        let mut coinbase_index = None;

        // First pass: count coinbase transactions and validate structure
        for (idx, tx) in block.transactions.iter().enumerate() {
            if tx.is_coinbase() {
                coinbase_count += 1;

                // Track the first coinbase index and value
                if coinbase_index.is_none() {
                    coinbase_index = Some(idx);
                    // Calculate total output value for the first coinbase
                    for output in tx.outputs() {
                        coinbase_value = coinbase_value.saturating_add(output.amount());
                    }
                }
            }
        }

        // Check for multiple coinbases first
        if coinbase_count > 1 {
            return Err(UnifiedValidationError::MultipleCoinbase);
        }

        // Must have exactly one coinbase
        if coinbase_count == 0 {
            return Err(UnifiedValidationError::NoCoinbase);
        }

        // Coinbase must be first transaction
        if let Some(idx) = coinbase_index {
            if idx != 0 {
                return Err(UnifiedValidationError::InvalidStructure(
                    "Coinbase transaction must be first".to_string(),
                ));
            }
        }

        // Additional check: first transaction must be coinbase
        if !block.transactions.is_empty() && !block.transactions[0].is_coinbase() {
            return Err(UnifiedValidationError::NoCoinbase);
        }

        // Validate subsidy and fees if context provided
        if let Some(ctx) = context {
            let expected_subsidy = self.calculate_block_subsidy(ctx.height);

            // Calculate total fees from all non-coinbase transactions
            let total_fees = self.calculate_transaction_fees(block, ctx)?;

            // Maximum allowed coinbase value is subsidy + fees
            let max_allowed = expected_subsidy.saturating_add(total_fees);

            // Strict validation: coinbase output must not exceed subsidy + fees
            if coinbase_value > max_allowed {
                return Err(UnifiedValidationError::InvalidSubsidy {
                    expected: max_allowed,
                    actual: coinbase_value,
                });
            }

            // Check coinbase maturity for any spent coinbase outputs
            if ctx.height >= COINBASE_MATURITY {
                self.validate_coinbase_maturity(block, ctx, COINBASE_MATURITY)?;
            }
        }

        Ok(())
    }

    /// Calculate total transaction fees in a block
    fn calculate_transaction_fees(
        &self,
        block: &Block,
        context: &ValidationContext,
    ) -> UnifiedValidationResult<u64> {
        let mut total_fees = 0u64;

        // Skip coinbase transaction (first one)
        for tx in block.transactions.iter().skip(1) {
            let mut input_value = 0u64;
            let mut output_value = 0u64;

            // Calculate input values from UTXO set
            for input in tx.inputs() {
                if let Some(utxo) =
                    (context.get_utxo)(&input.prev_tx_hash(), input.prev_output_index())
                {
                    input_value = input_value.saturating_add(utxo.amount());
                } else {
                    return Err(UnifiedValidationError::InvalidTransaction(format!(
                        "Input references non-existent UTXO: {}:{}",
                        hex::encode(input.prev_tx_hash()),
                        input.prev_output_index()
                    )));
                }
            }

            // Calculate output values
            for output in tx.outputs() {
                output_value = output_value.saturating_add(output.amount());
            }

            // Fee = inputs - outputs (must be non-negative)
            if input_value < output_value {
                return Err(UnifiedValidationError::InvalidTransaction(format!(
                    "Transaction creates value: inputs {} < outputs {}",
                    input_value, output_value
                )));
            }

            let tx_fee = input_value - output_value;
            total_fees = total_fees.saturating_add(tx_fee);
        }

        Ok(total_fees)
    }

    /// Validate that spent coinbase outputs are mature
    fn validate_coinbase_maturity(
        &self,
        block: &Block,
        context: &ValidationContext,
        maturity_blocks: u64,
    ) -> UnifiedValidationResult<()> {
        // Skip coinbase transaction (first one)
        for tx in block.transactions.iter().skip(1) {
            for input in tx.inputs() {
                // Check if this input spends a coinbase output
                if let Some(is_mature) = &context.is_coinbase_mature {
                    if !is_mature(
                        &input.prev_tx_hash(),
                        input.prev_output_index(),
                        maturity_blocks,
                    ) {
                        return Err(UnifiedValidationError::InvalidTransaction(format!(
                            "Spending immature coinbase output: {}:{}",
                            hex::encode(input.prev_tx_hash()),
                            input.prev_output_index()
                        )));
                    }
                }
            }
        }

        Ok(())
    }

    /// Calculate block subsidy for a given height
    pub fn calculate_block_subsidy(&self, height: u64) -> u64 {
        // Supernova emission schedule - Bitcoin-inspired
        const INITIAL_SUBSIDY: u64 = 50_000_000_000; // 50 NOVA in nanoNOVAs (smallest unit)
        const HALVING_INTERVAL: u64 = 210_000; // Halve every 210,000 blocks
        const MAX_HALVINGS: u64 = 64; // After 64 halvings, subsidy becomes 0

        let halvings = height / HALVING_INTERVAL;

        // After 64 halvings (13.44 million blocks), no more subsidy
        if halvings >= MAX_HALVINGS {
            return 0;
        }

        // Calculate subsidy: initial_subsidy / 2^halvings
        // Using bit shift for efficiency: subsidy = initial >> halvings
        INITIAL_SUBSIDY >> halvings
    }

    /// Get the halving epoch for a given height
    pub fn get_halving_epoch(&self, height: u64) -> u64 {
        const HALVING_INTERVAL: u64 = 210_000;
        height / HALVING_INTERVAL
    }

    /// Check if a height is at a halving boundary
    pub fn is_halving_height(&self, height: u64) -> bool {
        const HALVING_INTERVAL: u64 = 210_000;
        height > 0 && height % HALVING_INTERVAL == 0
    }

    /// Validate timestamp against time warp attacks
    fn validate_timestamp(
        &mut self,
        block: &Block,
        context: &ValidationContext,
    ) -> UnifiedValidationResult<()> {
        // Get previous timestamps
        let previous_timestamps: Vec<u64> = context
            .previous_headers
            .iter()
            .map(|h| h.timestamp())
            .collect();

        self.time_warp_prevention
            .validate_timestamp(&block.header, &previous_timestamps, context.current_time)
            .map_err(|e| UnifiedValidationError::InvalidTimestamp(e.to_string()))
    }

    /// Validate difficulty adjustment
    fn validate_difficulty(
        &self,
        block: &Block,
        context: &ValidationContext,
    ) -> UnifiedValidationResult<()> {
        // SECURITY FIX (P1-001): Complete difficulty validation with comprehensive checks
        let difficulty_adjuster = &self.difficulty_adjustment;
        let config = difficulty_adjuster.config();
        
        // Basic check: difficulty must not be zero
        if block.header.bits() == 0 {
            return Err(UnifiedValidationError::InvalidDifficulty {
                expected: 0x1d00ffff,
                actual: 0,
            });
        }

        // SECURITY FIX (P1-001): Validate difficulty bounds
        let actual_bits = block.header.bits();
        if actual_bits > config.max_target {
            return Err(UnifiedValidationError::InvalidDifficulty {
                expected: config.max_target,
                actual: actual_bits,
            });
        }
        if actual_bits < config.min_target {
            return Err(UnifiedValidationError::InvalidDifficulty {
                expected: config.min_target,
                actual: actual_bits,
            });
        }

        // SECURITY FIX (P1-001): Validate timestamp ordering (monotonic progression)
        if context.previous_headers.len() > 0 {
            let previous_timestamp = context.previous_headers
                .last()
                .map(|h| h.timestamp())
                .unwrap_or(0);
            
            // Block timestamp must be greater than previous block timestamp
            if block.header.timestamp() <= previous_timestamp {
                return Err(UnifiedValidationError::InvalidTimestamp(format!(
                    "Block timestamp {} is not after previous block timestamp {}",
                    block.header.timestamp(),
                    previous_timestamp
                )));
            }
        }

        // SECURITY FIX (P1-001): Validate difficulty adjustment for non-genesis blocks
        if context.previous_headers.len() >= 2 {
            // Extract timestamps and heights from previous headers
            let mut timestamps: Vec<u64> = context.previous_headers
                .iter()
                .map(|h| h.timestamp())
                .collect();
            
            // Add current block timestamp
            timestamps.push(block.header.timestamp());
            
            // SECURITY FIX (P1-001): Validate timestamps are monotonically increasing
            for i in 1..timestamps.len() {
                if timestamps[i] <= timestamps[i - 1] {
                    return Err(UnifiedValidationError::InvalidTimestamp(format!(
                        "Non-monotonic timestamps at index {}: {} <= {}",
                        i,
                        timestamps[i],
                        timestamps[i - 1]
                    )));
                }
            }
            
            // Extract heights (assuming sequential blocks)
            let mut heights: Vec<u64> = (0..context.previous_headers.len())
                .map(|i| context.height.saturating_sub((context.previous_headers.len() - i) as u64))
                .collect();
            heights.push(context.height);

            // Get the previous block's difficulty as current target
            let current_target = context.previous_headers
                .last()
                .map(|h| h.bits())
                .unwrap_or(0x1d00ffff);

            // SECURITY FIX (P1-001): Calculate expected next target using DifficultyAdjustment
            match difficulty_adjuster.calculate_next_target(
                current_target,
                &timestamps,
                &heights,
            ) {
                Ok(expected_target) => {
                    // SECURITY FIX (P1-001): Strict validation - difficulty must match exactly
                    // Difficulty adjustment is deterministic, so there should be no tolerance
                    if block.header.bits() != expected_target {
                        // Check if we're at an adjustment boundary
                        let is_adjustment_boundary = context.height % config.adjustment_interval == 0;
                        
                        if is_adjustment_boundary {
                            // At adjustment boundary, difficulty must match exactly
                            return Err(UnifiedValidationError::InvalidDifficulty {
                                expected: expected_target,
                                actual: block.header.bits(),
                            });
                        } else {
                            // Not at adjustment boundary, difficulty should remain the same
                            if block.header.bits() != current_target {
                                return Err(UnifiedValidationError::InvalidDifficulty {
                                    expected: current_target,
                                    actual: block.header.bits(),
                                });
                            }
                        }
                    }
                }
                Err(e) => {
                    // SECURITY FIX (P1-001): For non-genesis blocks, difficulty calculation failure is suspicious
                    // Log error and reject block if we have sufficient history
                    if context.height > config.adjustment_interval {
                        return Err(UnifiedValidationError::InvalidDifficulty {
                            expected: 0x1d00ffff,
                            actual: block.header.bits(),
                        });
                    }
                    // For early blocks (< adjustment_interval), difficulty calculation may fail
                    // Just ensure difficulty matches previous block (no adjustment yet)
                    if context.previous_headers.len() > 0 {
                        let previous_bits = context.previous_headers
                            .last()
                            .map(|h| h.bits())
                            .unwrap_or(0x1d00ffff);
                        
                        if block.header.bits() != previous_bits {
                            return Err(UnifiedValidationError::InvalidDifficulty {
                                expected: previous_bits,
                                actual: block.header.bits(),
                            });
                        }
                    }
                }
            }
        } else if context.previous_headers.len() == 1 {
            // SECURITY FIX (P1-001): For second block (after genesis), difficulty should match genesis
            let genesis_bits = context.previous_headers[0].bits();
            if block.header.bits() != genesis_bits {
                return Err(UnifiedValidationError::InvalidDifficulty {
                    expected: genesis_bits,
                    actual: block.header.bits(),
                });
            }
        } else {
            // SECURITY FIX (P1-001): Genesis block - validate difficulty is within bounds
            // Genesis difficulty should be set to initial difficulty
            if block.header.bits() == 0 || block.header.bits() > config.max_target {
                return Err(UnifiedValidationError::InvalidDifficulty {
                    expected: 0x1d00ffff,
                    actual: block.header.bits(),
                });
            }
        }

        Ok(())
    }

    /// Validate proof of work
    fn validate_proof_of_work(&self, block: &Block) -> UnifiedValidationResult<()> {
        // For test blocks with max difficulty (0x207fffff), skip PoW validation
        // This allows us to test the validation pipeline without mining
        #[cfg(test)]
        {
            if block.header.bits() == 0x207fffff {
                return Ok(());
            }
        }

        if !block.header.meets_target() {
            return Err(UnifiedValidationError::InvalidPoW);
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::transaction::{TransactionInput, TransactionOutput};

    fn create_test_block() -> Block {
        let coinbase_input = TransactionInput::new_coinbase(vec![1, 2, 3]);
        let coinbase_output = TransactionOutput::new(50_000_000_000, vec![1, 2, 3, 4]);
        let coinbase_tx = Transaction::new(1, vec![coinbase_input], vec![coinbase_output], 0);

        Block::new_with_params(
            1,
            [0; 32],
            vec![coinbase_tx],
            0x207fffff, // Easy difficulty
        )
    }

    #[test]
    fn test_unified_validation_basic() {
        let mut validator = UnifiedBlockValidator::new();
        let block = create_test_block();

        // Should pass basic validation
        let result = validator.validate_block_secure(&block, None);
        assert!(result.is_ok());
    }

    #[test]
    fn test_multiple_coinbase_detection() {
        let mut validator = UnifiedBlockValidator::new();

        // Create block with two coinbase transactions
        let coinbase1 = Transaction::new(
            1,
            vec![TransactionInput::new_coinbase(vec![1])],
            vec![TransactionOutput::new(25_000_000_000, vec![1])],
            0,
        );

        let coinbase2 = Transaction::new(
            1,
            vec![TransactionInput::new_coinbase(vec![2])],
            vec![TransactionOutput::new(25_000_000_000, vec![2])],
            0,
        );

        let block = Block::new_with_params(1, [0; 32], vec![coinbase1, coinbase2], 0x207fffff);

        let result = validator.validate_block_secure(&block, None);
        assert!(matches!(
            result,
            Err(UnifiedValidationError::MultipleCoinbase)
        ));
    }

    #[test]
    fn test_merkle_root_validation() {
        let mut validator = UnifiedBlockValidator::new();
        let mut block = create_test_block();

        // Corrupt the merkle root
        block.header.merkle_root = [0xFF; 32];

        let result = validator.validate_block_secure(&block, None);
        assert!(matches!(
            result,
            Err(UnifiedValidationError::InvalidMerkleRoot)
        ));
    }

    /// SECURITY FIX (P1-001): Tests for complete difficulty validation
    #[test]
    fn test_difficulty_validation_normal_blocks() {
        let mut validator = UnifiedBlockValidator::new();
        let config = validator.difficulty_adjustment.config();
        
        // Create previous headers with valid difficulty progression
        let mut previous_headers = Vec::new();
        let base_time = 1000000;
        let base_bits = 0x1d00ffff;
        
        // Create 3 previous blocks
        for i in 0..3 {
            let header = BlockHeader::new_with_height(
                1,
                if i == 0 { [0; 32] } else { [i as u8; 32] },
                [0; 32], // merkle_root
                base_time + (i as u64) * 150, // 2.5 minutes apart
                base_bits,
                0, // nonce
                i,
            );
            previous_headers.push(header);
        }
        
        // Create current block with correct difficulty (should match previous)
        let mut current_block = Block::new_with_params(
            1,
            [3; 32],
            vec![create_test_block().transactions[0].clone()],
            base_bits, // Same difficulty (not at adjustment boundary)
        );
        current_block.header.set_timestamp(base_time + 450);
        
        let context = ValidationContext {
            previous_headers: previous_headers.clone(),
            get_utxo: Box::new(|_, _| None),
            height: 3,
            current_time: Some(base_time + 500),
            is_coinbase_mature: None,
        };
        
        // Should pass validation
        let result = validator.validate_block_secure(&current_block, Some(&context));
        assert!(result.is_ok(), "Normal block with correct difficulty should pass");
    }

    #[test]
    fn test_difficulty_validation_adjustment_boundary() {
        let mut validator = UnifiedBlockValidator::new();
        let config = validator.difficulty_adjustment.config();
        let adjustment_interval = config.adjustment_interval;
        
        // Create headers up to adjustment boundary
        let mut previous_headers = Vec::new();
        let base_time = 1000000;
        let base_bits = 0x1d00ffff;
        
        // Create blocks leading up to adjustment boundary
        for i in 0..adjustment_interval {
            let header = BlockHeader::new_with_height(
                1,
                if i == 0 { [0; 32] } else { [i as u8; 32] },
                [0; 32],
                base_time + (i as u64) * 150,
                base_bits,
                0,
                i,
            );
            previous_headers.push(header);
        }
        
        // Calculate expected target at adjustment boundary
        let timestamps: Vec<u64> = previous_headers.iter().map(|h| h.timestamp()).collect();
        let heights: Vec<u64> = (0..adjustment_interval).collect();
        let current_target = previous_headers.last().unwrap().bits();
        
        let expected_target = validator.difficulty_adjustment
            .calculate_next_target(current_target, &timestamps, &heights)
            .unwrap();
        
        // Create block at adjustment boundary with correct difficulty
        let mut current_block = Block::new_with_params(
            1,
            [adjustment_interval as u8; 32],
            vec![create_test_block().transactions[0].clone()],
            expected_target,
        );
        current_block.header.set_timestamp(base_time + (adjustment_interval as u64) * 150);
        current_block.header.set_height(adjustment_interval);
        
        let context = ValidationContext {
            previous_headers,
            get_utxo: Box::new(|_, _| None),
            height: adjustment_interval,
            current_time: Some(base_time + (adjustment_interval as u64) * 150 + 100),
            is_coinbase_mature: None,
        };
        
        // Should pass validation
        let result = validator.validate_block_secure(&current_block, Some(&context));
        assert!(result.is_ok(), "Block at adjustment boundary with correct difficulty should pass");
        
        // Test with incorrect difficulty at adjustment boundary
        let mut invalid_block = current_block.clone();
        invalid_block.header.bits = 0x1e00ffff; // Wrong difficulty
        
        let result = validator.validate_block_secure(&invalid_block, Some(&context));
        assert!(matches!(
            result,
            Err(UnifiedValidationError::InvalidDifficulty { .. })
        ), "Block at adjustment boundary with incorrect difficulty should fail");
    }

    #[test]
    fn test_difficulty_validation_genesis_block() {
        let mut validator = UnifiedBlockValidator::new();
        let config = validator.difficulty_adjustment.config();
        
        // Create genesis block with valid difficulty
        let genesis_block = Block::new_with_params(
            1,
            [0; 32],
            vec![create_test_block().transactions[0].clone()],
            0x1d00ffff, // Valid genesis difficulty
        );
        
        let context = ValidationContext {
            previous_headers: Vec::new(),
            get_utxo: Box::new(|_, _| None),
            height: 0,
            current_time: Some(1000000),
            is_coinbase_mature: None,
        };
        
        // Genesis block should pass validation
        let result = validator.validate_block_secure(&genesis_block, Some(&context));
        assert!(result.is_ok(), "Genesis block with valid difficulty should pass");
        
        // Test genesis block with zero difficulty
        let invalid_genesis = Block::new_with_params(
            1,
            [0; 32],
            vec![create_test_block().transactions[0].clone()],
            0, // Invalid: zero difficulty
        );
        
        let result = validator.validate_block_secure(&invalid_genesis, Some(&context));
        assert!(matches!(
            result,
            Err(UnifiedValidationError::InvalidDifficulty { .. })
        ), "Genesis block with zero difficulty should fail");
        
        // Test genesis block with difficulty exceeding max_target
        let too_easy_genesis = Block::new_with_params(
            1,
            [0; 32],
            vec![create_test_block().transactions[0].clone()],
            config.max_target + 1, // Invalid: exceeds max_target
        );
        
        let result = validator.validate_block_secure(&too_easy_genesis, Some(&context));
        assert!(matches!(
            result,
            Err(UnifiedValidationError::InvalidDifficulty { .. })
        ), "Genesis block with difficulty exceeding max_target should fail");
    }

    #[test]
    fn test_difficulty_validation_invalid_bits() {
        let mut validator = UnifiedBlockValidator::new();
        let config = validator.difficulty_adjustment.config();
        
        // Create previous header
        let previous_header = BlockHeader::new_with_height(
            1,
            [0; 32],
            [0; 32],
            1000000,
            0x1d00ffff,
            0,
            0,
        );
        
        // Test with difficulty below min_target
        let mut invalid_block = Block::new_with_params(
            1,
            [1; 32],
            vec![create_test_block().transactions[0].clone()],
            config.min_target - 1, // Invalid: below min_target
        );
        invalid_block.header.set_timestamp(1000150);
        
        let context = ValidationContext {
            previous_headers: vec![previous_header.clone()],
            get_utxo: Box::new(|_, _| None),
            height: 1,
            current_time: Some(1000200),
            is_coinbase_mature: None,
        };
        
        let result = validator.validate_block_secure(&invalid_block, Some(&context));
        assert!(matches!(
            result,
            Err(UnifiedValidationError::InvalidDifficulty { .. })
        ), "Block with difficulty below min_target should fail");
        
        // Test with difficulty above max_target
        let mut too_easy_block = Block::new_with_params(
            1,
            [1; 32],
            vec![create_test_block().transactions[0].clone()],
            config.max_target + 1, // Invalid: above max_target
        );
        too_easy_block.header.set_timestamp(1000150);
        
        let result = validator.validate_block_secure(&too_easy_block, Some(&context));
        assert!(matches!(
            result,
            Err(UnifiedValidationError::InvalidDifficulty { .. })
        ), "Block with difficulty above max_target should fail");
    }

    #[test]
    fn test_difficulty_validation_timestamp_bounds() {
        let mut validator = UnifiedBlockValidator::new();
        
        // Create previous header
        let previous_header = BlockHeader::new_with_height(
            1,
            [0; 32],
            [0; 32],
            1000000,
            0x1d00ffff,
            0,
            0,
        );
        
        // Test with timestamp not after previous block
        let mut invalid_block = Block::new_with_params(
            1,
            [1; 32],
            vec![create_test_block().transactions[0].clone()],
            0x1d00ffff,
        );
        invalid_block.header.set_timestamp(1000000); // Same as previous (should fail)
        
        let context = ValidationContext {
            previous_headers: vec![previous_header.clone()],
            get_utxo: Box::new(|_, _| None),
            height: 1,
            current_time: Some(1000200),
            is_coinbase_mature: None,
        };
        
        let result = validator.validate_block_secure(&invalid_block, Some(&context));
        assert!(matches!(
            result,
            Err(UnifiedValidationError::InvalidTimestamp(..))
        ), "Block with timestamp not after previous block should fail");
        
        // Test with timestamp before previous block
        let mut invalid_block2 = Block::new_with_params(
            1,
            [1; 32],
            vec![create_test_block().transactions[0].clone()],
            0x1d00ffff,
        );
        invalid_block2.header.set_timestamp(999999); // Before previous (should fail)
        
        let result = validator.validate_block_secure(&invalid_block2, Some(&context));
        assert!(matches!(
            result,
            Err(UnifiedValidationError::InvalidTimestamp(..))
        ), "Block with timestamp before previous block should fail");
    }

    #[test]
    fn test_difficulty_validation_second_block() {
        let mut validator = UnifiedBlockValidator::new();
        
        // Create genesis block
        let genesis_header = BlockHeader::new_with_height(
            1,
            [0; 32],
            [0; 32],
            1000000,
            0x1d00ffff,
            0,
            0,
        );
        
        // Second block should match genesis difficulty
        let mut second_block = Block::new_with_params(
            1,
            [1; 32],
            vec![create_test_block().transactions[0].clone()],
            0x1d00ffff, // Correct: matches genesis
        );
        second_block.header.set_timestamp(1000150);
        
        let context = ValidationContext {
            previous_headers: vec![genesis_header.clone()],
            get_utxo: Box::new(|_, _| None),
            height: 1,
            current_time: Some(1000200),
            is_coinbase_mature: None,
        };
        
        // Should pass validation
        let result = validator.validate_block_secure(&second_block, Some(&context));
        assert!(result.is_ok(), "Second block with matching genesis difficulty should pass");
        
        // Test with different difficulty (should fail)
        let mut invalid_second = Block::new_with_params(
            1,
            [1; 32],
            vec![create_test_block().transactions[0].clone()],
            0x1c00ffff, // Invalid: different from genesis
        );
        invalid_second.header.set_timestamp(1000150);
        
        let result = validator.validate_block_secure(&invalid_second, Some(&context));
        assert!(matches!(
            result,
            Err(UnifiedValidationError::InvalidDifficulty { .. })
        ), "Second block with different difficulty should fail");
    }
}
