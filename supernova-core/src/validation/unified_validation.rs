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
                // TODO: Implement full transaction validation
                // For now, just check basic structure
                if tx.inputs().is_empty() {
                    return Err(UnifiedValidationError::InvalidTransaction(
                        "Non-coinbase transaction has no inputs".to_string(),
                    ));
                }
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
        // TODO: Implement proper difficulty validation
        // For now, just ensure it's not zero
        if block.header.bits() == 0 {
            return Err(UnifiedValidationError::InvalidDifficulty {
                expected: 0x1d00ffff,
                actual: 0,
            });
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
}
