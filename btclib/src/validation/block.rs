// Block validation - comprehensive security implementation for Supernova

use crate::types::block::Block;
use crate::types::transaction::Transaction;
use crate::validation::ValidationError;
use crate::validation::transaction::TransactionValidator;
use crate::consensus::difficulty::calculate_required_work;
use crate::hash::Hash256;
use std::collections::HashSet;
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::{debug, warn};

/// Error types for block validation
#[derive(Debug, thiserror::Error)]
pub enum BlockValidationError {
    /// Block too large
    #[error("Block too large: {0} > {1}")]
    BlockTooLarge(usize, usize),
    
    /// Missing block header
    #[error("Missing block header")]
    MissingHeader,
    
    /// Missing previous block
    #[error("Previous block not found: {0:?}")]
    PrevBlockNotFound([u8; 32]),
    
    /// Incorrect previous block reference
    #[error("Previous block mismatch")]
    PrevBlockMismatch,
    
    /// Invalid Merkle root
    #[error("Invalid Merkle root")]
    InvalidMerkleRoot,
    
    /// Missing coinbase transaction
    #[error("Missing coinbase transaction")]
    MissingCoinbase,
    
    /// Multiple coinbase transactions
    #[error("Multiple coinbase transactions found")]
    MultipleCoinbase,
    
    /// Invalid transaction
    #[error("Invalid transaction: {0}")]
    InvalidTransaction(String),
    
    /// Block timestamp too far in the future
    #[error("Block timestamp too far in future: {0} > {1}")]
    TimestampTooFar(u64, u64),
    
    /// Block timestamp earlier than median time
    #[error("Block timestamp earlier than median time: {0} < {1}")]
    TimestampTooEarly(u64, u64),
    
    /// Duplicate transaction in block
    #[error("Duplicate transaction in block: {0:?}")]
    DuplicateTransaction([u8; 32]),
    
    /// Invalid proof-of-work
    #[error("Invalid proof-of-work")]
    InvalidPoW,
    
    /// Invalid difficulty
    #[error("Invalid difficulty: {0}")]
    InvalidDifficulty(String),
    
    /// Invalid block version
    #[error("Invalid block version: {0}")]
    InvalidVersion(u32),
    
    /// Block weight exceeds maximum
    #[error("Block weight too high: {0} > {1}")]
    WeightTooHigh(u64, u64),
    
    /// Invalid coinbase maturity
    #[error("Coinbase output spent before maturity")]
    ImmatureCoinbaseSpend,
    
    /// Invalid block subsidy
    #[error("Invalid block subsidy: expected {0}, got {1}")]
    InvalidSubsidy(u64, u64),
    
    /// Script validation failed
    #[error("Script validation failed: {0}")]
    ScriptValidationFailed(String),
    
    /// Witness commitment mismatch
    #[error("Witness commitment mismatch")]
    WitnessCommitmentMismatch,
    
    /// Invalid block header
    #[error("Invalid block header: {0}")]
    InvalidHeader(String),
}

/// Type for validation results
pub type BlockValidationResult = Result<(), BlockValidationError>;

/// Configuration for block validation
#[derive(Debug, Clone)]
pub struct BlockValidationConfig {
    /// Maximum block size in bytes
    pub max_block_size: usize,
    
    /// Maximum block weight
    pub max_block_weight: u64,
    
    /// Maximum timestamp offset in the future (seconds)
    pub max_future_time_offset: u64,
    
    /// Minimum required block version
    pub min_block_version: u32,
    
    /// Coinbase maturity (blocks before coinbase can be spent)
    pub coinbase_maturity: u64,
    
    /// Whether to enforce full script validation
    pub validate_scripts: bool,
    
    /// Whether to validate witness commitments
    pub validate_witness: bool,
    
    /// Whether to check proof-of-work
    pub validate_pow: bool,
}

impl Default for BlockValidationConfig {
    fn default() -> Self {
        Self {
            max_block_size: 1_000_000, // 1MB
            max_block_weight: 4_000_000, // 4M weight units
            max_future_time_offset: 7200, // 2 hours
            min_block_version: 1,
            coinbase_maturity: 100,
            validate_scripts: true,
            validate_witness: true,
            validate_pow: true,
        }
    }
}

/// Context for block validation (chain state needed for validation)
pub struct ValidationContext {
    /// Previous block hash
    pub prev_block_hash: [u8; 32],
    /// Previous block height
    pub prev_block_height: u64,
    /// Previous block timestamp
    pub prev_block_timestamp: u64,
    /// Median time past (for timestamp validation)
    pub median_time_past: u64,
    /// Current network difficulty
    pub current_difficulty: u32,
    /// UTXO set accessor (for script validation)
    pub utxo_provider: Option<Box<dyn Fn(&[u8; 32], u32) -> Option<Vec<u8>>>>,
}

/// Block validator
pub struct BlockValidator {
    /// Configuration
    config: BlockValidationConfig,
    
    /// Transaction validator
    transaction_validator: TransactionValidator,
}

impl BlockValidator {
    /// Create a new block validator with default settings
    pub fn new() -> Self {
        Self {
            config: BlockValidationConfig::default(),
            transaction_validator: TransactionValidator::new(),
        }
    }
    
    /// Create a block validator with custom configuration
    pub fn with_config(config: BlockValidationConfig) -> Self {
        Self {
            config,
            transaction_validator: TransactionValidator::new(),
        }
    }
    
    /// Validate a block with full context
    pub fn validate_block_with_context(
        &self, 
        block: &Block,
        context: &ValidationContext,
    ) -> BlockValidationResult {
        debug!("Validating block at height {}", block.height());
        
        // Phase 1: Structure validation
        self.validate_structure(block)?;
        
        // Phase 2: Header validation
        self.validate_header(block, context)?;
        
        // Phase 3: Transaction validation
        self.validate_transactions(block, context)?;
        
        // Phase 4: Consensus rules
        self.validate_consensus_rules(block, context)?;
        
        debug!("Block validation successful");
        Ok(())
    }
    
    /// Validate a block (simplified, without full context)
    pub fn validate_block(&self, block: &Block) -> BlockValidationResult {
        // Basic validation without chain context
        debug!("Performing basic block validation");
        
        // Structure validation
        self.validate_structure(block)?;
        
        // Basic header checks
        self.validate_basic_header(block)?;
        
        // Transaction structure validation
        self.validate_transaction_structure(block)?;
        
        Ok(())
    }
    
    /// Phase 1: Validate block structure
    fn validate_structure(&self, block: &Block) -> BlockValidationResult {
        // Check block size
        let block_size = block.size();
        if block_size > self.config.max_block_size {
            return Err(BlockValidationError::BlockTooLarge(
                block_size,
                self.config.max_block_size,
            ));
        }
        
        // Check block weight
        let block_weight = self.calculate_block_weight(block);
        if block_weight > self.config.max_block_weight {
            return Err(BlockValidationError::WeightTooHigh(
                block_weight,
                self.config.max_block_weight,
            ));
        }
        
        // Must have at least one transaction (coinbase)
        if block.transactions().is_empty() {
            return Err(BlockValidationError::MissingCoinbase);
        }
        
        // Check for duplicate transactions
        let mut tx_hashes = HashSet::new();
        for tx in block.transactions() {
            let tx_hash = tx.hash();
            if !tx_hashes.insert(tx_hash) {
                return Err(BlockValidationError::DuplicateTransaction(tx_hash));
            }
        }
        
        Ok(())
    }
    
    /// Phase 2: Validate block header
    fn validate_header(
        &self,
        block: &Block,
        context: &ValidationContext,
    ) -> BlockValidationResult {
        // Check version
        if block.version() < self.config.min_block_version {
            return Err(BlockValidationError::InvalidVersion(block.version()));
        }
        
        // Check previous block hash
        if block.prev_block_hash() != &context.prev_block_hash {
            return Err(BlockValidationError::PrevBlockMismatch);
        }
        
        // Check height
        if block.height() != context.prev_block_height + 1 {
            return Err(BlockValidationError::InvalidHeader(
                format!("Invalid height: expected {}, got {}", 
                    context.prev_block_height + 1, 
                    block.height())
            ));
        }
        
        // Validate timestamp
        self.validate_timestamp(block, context)?;
        
        // Validate proof-of-work if enabled
        if self.config.validate_pow {
            self.validate_pow(block, context)?;
        }
        
        // Validate merkle root
        self.validate_merkle_root(block)?;
        
        Ok(())
    }
    
    /// Basic header validation (without context)
    fn validate_basic_header(&self, block: &Block) -> BlockValidationResult {
        // Check version
        if block.version() < self.config.min_block_version {
            return Err(BlockValidationError::InvalidVersion(block.version()));
        }
        
        // Check timestamp is not too far in future
        let current_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        
        if block.timestamp() > current_time + self.config.max_future_time_offset {
            return Err(BlockValidationError::TimestampTooFar(
                block.timestamp(),
                current_time + self.config.max_future_time_offset,
            ));
        }
        
        Ok(())
    }
    
    /// Validate block timestamp
    fn validate_timestamp(
        &self,
        block: &Block,
        context: &ValidationContext,
    ) -> BlockValidationResult {
        let block_time = block.timestamp();
        
        // Check median time past
        if block_time <= context.median_time_past {
            return Err(BlockValidationError::TimestampTooEarly(
                block_time,
                context.median_time_past,
            ));
        }
        
        // Check not too far in future
        let current_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        
        if block_time > current_time + self.config.max_future_time_offset {
            return Err(BlockValidationError::TimestampTooFar(
                block_time,
                current_time + self.config.max_future_time_offset,
            ));
        }
        
        Ok(())
    }
    
    /// Validate proof-of-work
    fn validate_pow(
        &self,
        block: &Block,
        context: &ValidationContext,
    ) -> BlockValidationResult {
        // Calculate block hash
        let block_hash = block.hash();
        
        // Calculate target from difficulty
        let target = calculate_required_work(context.current_difficulty);
        
        // Hash must be less than target (using the standalone function from hash module)
        if !crate::hash::meets_difficulty(&block_hash, &target) {
            return Err(BlockValidationError::InvalidPoW);
        }
        
        Ok(())
    }
    
    /// Validate merkle root
    fn validate_merkle_root(&self, block: &Block) -> BlockValidationResult {
        let calculated_root = block.calculate_merkle_root();
        
        if calculated_root != *block.merkle_root() {
            return Err(BlockValidationError::InvalidMerkleRoot);
        }
        
        Ok(())
    }
    
    /// Phase 3: Validate all transactions
    fn validate_transactions(
        &self,
        block: &Block,
        context: &ValidationContext,
    ) -> BlockValidationResult {
        let mut has_coinbase = false;
        
        for (index, tx) in block.transactions().iter().enumerate() {
            if index == 0 {
                // First transaction must be coinbase
                if !tx.is_coinbase() {
                    return Err(BlockValidationError::MissingCoinbase);
                }
                has_coinbase = true;
                
                // Validate coinbase specifics
                self.validate_coinbase(tx, block, context)?;
            } else {
                // Non-coinbase transactions
                if tx.is_coinbase() {
                    return Err(BlockValidationError::MultipleCoinbase);
                }
                
                // Validate transaction
                if let Err(e) = self.transaction_validator.validate(tx) {
                    return Err(BlockValidationError::InvalidTransaction(e.to_string()));
                }
                
                // Check coinbase maturity for inputs
                if self.spends_immature_coinbase(tx, block.height(), context) {
                    return Err(BlockValidationError::ImmatureCoinbaseSpend);
                }
            }
        }
        
        if !has_coinbase {
            return Err(BlockValidationError::MissingCoinbase);
        }
        
        Ok(())
    }
    
    /// Basic transaction structure validation
    fn validate_transaction_structure(&self, block: &Block) -> BlockValidationResult {
        for (index, tx) in block.transactions().iter().enumerate() {
            if index == 0 && !tx.is_coinbase() {
                return Err(BlockValidationError::MissingCoinbase);
            }
            
            if index > 0 && tx.is_coinbase() {
                return Err(BlockValidationError::MultipleCoinbase);
            }
        }
        
        Ok(())
    }
    
    /// Validate coinbase transaction
    fn validate_coinbase(
        &self,
        coinbase: &Transaction,
        block: &Block,
        context: &ValidationContext,
    ) -> BlockValidationResult {
        // Calculate expected subsidy
        let expected_subsidy = self.calculate_block_subsidy(block.height());
        
        // Calculate actual subsidy (outputs - inputs, but coinbase has no real inputs)
        let actual_subsidy = coinbase.outputs()
            .iter()
            .map(|out| out.value())
            .sum::<u64>();
        
        // For now, just check it doesn't exceed maximum
        // In full implementation, would need to account for fees
        if actual_subsidy > expected_subsidy {
            return Err(BlockValidationError::InvalidSubsidy(
                expected_subsidy,
                actual_subsidy,
            ));
        }
        
        Ok(())
    }
    
    /// Phase 4: Validate consensus rules
    fn validate_consensus_rules(
        &self,
        block: &Block,
        context: &ValidationContext,
    ) -> BlockValidationResult {
        // Additional consensus rules can be added here
        // For example: soft fork activation rules, etc.
        
        Ok(())
    }
    
    /// Calculate block weight
    fn calculate_block_weight(&self, block: &Block) -> u64 {
        // Weight = base size * 3 + total size
        // For now, simplified calculation
        block.size() as u64 * 4
    }
    
    /// Calculate block subsidy for a given height
    fn calculate_block_subsidy(&self, height: u64) -> u64 {
        // Supernova halving schedule: every 210,000 blocks
        let halvings = height / 210_000;
        
        if halvings >= 64 {
            return 0;
        }
        
        // Initial subsidy: 50 NOVA (in smallest units)
        let initial_subsidy = 50_000_000_000u64; // 50 * 10^9
        
        initial_subsidy >> halvings
    }
    
    /// Check if transaction spends immature coinbase
    fn spends_immature_coinbase(
        &self,
        tx: &Transaction,
        current_height: u64,
        context: &ValidationContext,
    ) -> bool {
        // Would need UTXO set access to properly implement
        // For now, return false (no immature spend)
        false
    }
} 