// integrity/verifiers.rs - Verification components for different subsystems
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use bincode;
use sha2::{Digest, Sha256};
use tracing::{debug, warn};

use crate::storage::BlockchainDB;
use crate::storage::StorageError;
use crate::storage::integrity::models::{
    IntegrityConfig, IntegrityIssue, IssueLocation, IssueSeverity, IssueType,
};
use btclib::types::block::Block;
use btclib::types::transaction::Transaction;

/// Database structure verifier
pub struct DatabaseVerifier {
    db: Arc<BlockchainDB>,
}

/// Blockchain consistency verifier
pub struct BlockchainVerifier<'a> {
    db: Arc<BlockchainDB>,
    config: &'a IntegrityConfig,
}

/// UTXO set verifier
pub struct UtxoVerifier<'a> {
    db: Arc<BlockchainDB>,
    config: &'a IntegrityConfig,
}

/// Cryptographic verification component
pub struct CryptoVerifier<'a> {
    db: Arc<BlockchainDB>,
    config: &'a IntegrityConfig,
}

impl DatabaseVerifier {
    /// Create a new database verifier
    pub fn new(db: Arc<BlockchainDB>) -> Self {
        Self { db }
    }

    /// Verify database structure
    pub async fn verify(&self, issues: &mut Vec<IntegrityIssue>) -> Result<(), StorageError> {
        debug!("Verifying database structure");

        // Essential database components to verify
        let essential_components = ["blocks", "transactions", "utxos", "metadata", "headers"];

        for component in &essential_components {
            match self.db.open_tree(component) {
                Ok(tree) => {
                    // For most trees, being empty is suspicious
                    if *component != "utxos" && tree.is_empty() {
                        issues.push(IntegrityIssue::new(
                            IssueType::Structure,
                            IssueSeverity::Warning,
                            format!("Database component '{}' is empty", component),
                            IssueLocation::Database {
                                component: component.to_string(),
                            },
                            false,
                        ));
                    }
                }
                Err(_) => {
                    // Missing essential component is critical
                    issues.push(IntegrityIssue::new(
                        IssueType::Structure,
                        IssueSeverity::Critical,
                        format!("Missing essential database component: {}", component),
                        IssueLocation::Database {
                            component: component.to_string(),
                        },
                        true,
                    ));
                }
            }
        }

        // Check critical metadata
        let essential_metadata = ["height", "best_hash", "genesis_hash"];

        for key in &essential_metadata {
            match self.db.get_metadata(key.as_bytes()) {
                Ok(Some(_)) => {
                    // Metadata exists
                }
                Ok(None) => {
                    issues.push(IntegrityIssue::new(
                        IssueType::Structure,
                        IssueSeverity::Critical,
                        format!("Missing essential metadata: {}", key),
                        IssueLocation::Database {
                            component: format!("metadata:{}", key),
                        },
                        true,
                    ));
                }
                Err(e) => {
                    issues.push(IntegrityIssue::new(
                        IssueType::Structure,
                        IssueSeverity::Critical,
                        format!("Error accessing metadata '{}': {}", key, e),
                        IssueLocation::Database {
                            component: format!("metadata:{}", key),
                        },
                        true,
                    ));
                }
            }
        }

        Ok(())
    }
}

impl<'a> BlockchainVerifier<'a> {
    /// Create a new blockchain verifier
    pub fn new(db: Arc<BlockchainDB>, config: &'a IntegrityConfig) -> Self {
        Self { db, config }
    }

    /// Verify blockchain consistency
    pub async fn verify(&self, issues: &mut Vec<IntegrityIssue>) -> Result<(), StorageError> {
        debug!("Verifying blockchain consistency");

        // Get current chain height and best hash
        let height_bytes = match self.db.get_metadata(b"height")? {
            Some(h) => h,
            None => return Ok(()), // Already reported in structure check
        };

        let best_hash_bytes = match self.db.get_metadata(b"best_hash")? {
            Some(h) => h,
            None => return Ok(()), // Already reported in structure check
        };

        let height: u64 = bincode::deserialize(&height_bytes)?;
        let mut best_hash = [0u8; 32];
        best_hash.copy_from_slice(&best_hash_bytes);

        // Verify chain by walking backwards from tip
        let mut current_hash = best_hash;
        let mut current_height = height;
        let mut prev_timestamp = u64::MAX;

        // Limit the number of blocks to check
        let blocks_to_check = std::cmp::min(self.config.max_batch_size, current_height as usize);

        for _ in 0..blocks_to_check {
            // Skip to genesis if we're at height 0
            if current_height == 0 {
                break;
            }

            // Get block at current position
            let block = match self.db.get_block(&current_hash)? {
                Some(b) => b,
                None => {
                    issues.push(IntegrityIssue::new(
                        IssueType::MissingReference,
                        IssueSeverity::Critical,
                        format!("Missing block at height {}", current_height),
                        IssueLocation::Block {
                            hash: current_hash,
                            height: Some(current_height),
                        },
                        true,
                    ));
                    break;
                }
            };

            // Verify block height matches expected height
            if block.height() != current_height {
                issues.push(IntegrityIssue::new(
                    IssueType::ChainInconsistency,
                    IssueSeverity::Error,
                    format!(
                        "Block height mismatch: expected {}, found {}",
                        current_height,
                        block.height()
                    ),
                    IssueLocation::Block {
                        hash: current_hash,
                        height: Some(current_height),
                    },
                    true,
                ));
            }

            // Verify block timestamp is not in the future
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();

            if block.timestamp() > now + 7200 {
                // Allow 2 hour clock skew
                issues.push(IntegrityIssue::new(
                    IssueType::ConsensusViolation,
                    IssueSeverity::Warning,
                    format!(
                        "Block timestamp is in the future: {} (current time: {})",
                        block.timestamp(),
                        now
                    ),
                    IssueLocation::Block {
                        hash: current_hash,
                        height: Some(current_height),
                    },
                    false,
                ));
            }

            // Verify block doesn't exceed maximum size
            let block_data = bincode::serialize(&block)?;
            if block_data.len() > self.config.requirements.max_block_size {
                issues.push(IntegrityIssue::new(
                    IssueType::ConsensusViolation,
                    IssueSeverity::Warning,
                    format!(
                        "Block size {} exceeds maximum {}",
                        block_data.len(),
                        self.config.requirements.max_block_size
                    ),
                    IssueLocation::Block {
                        hash: current_hash,
                        height: Some(current_height),
                    },
                    false,
                ));
            }

            // Verify previous timestamp
            if prev_timestamp != u64::MAX {
                let time_diff = if prev_timestamp > block.timestamp() {
                    prev_timestamp - block.timestamp()
                } else {
                    0 // Out of order timestamps
                };

                if time_diff > self.config.requirements.max_time_between_blocks {
                    issues.push(IntegrityIssue::new(
                        IssueType::ChainInconsistency,
                        IssueSeverity::Warning,
                        format!(
                            "Excessive time gap between blocks: {} seconds",
                            time_diff
                        ),
                        IssueLocation::Block {
                            hash: current_hash,
                            height: Some(current_height),
                        },
                        false,
                    ));
                }
            }
            prev_timestamp = block.timestamp();

            // Verify transactions in the block exist in the database
            for tx in block.transactions() {
                let tx_hash = tx.hash();
                match self.db.get_transaction(&tx_hash) {
                    Ok(Some(_)) => {
                        // Transaction exists, all good
                    }
                    Ok(None) => {
                        issues.push(IntegrityIssue::new(
                            IssueType::MissingReference,
                            IssueSeverity::Error,
                            format!(
                                "Transaction {} referenced in block {} is missing",
                                hex::encode(&tx_hash[..4]),
                                hex::encode(&current_hash[..4])
                            ),
                            IssueLocation::Transaction {
                                hash: tx_hash,
                                block_hash: Some(current_hash),
                            },
                            true,
                        ));
                    }
                    Err(e) => {
                        issues.push(IntegrityIssue::new(
                            IssueType::Structure,
                            IssueSeverity::Error,
                            format!(
                                "Error accessing transaction {}: {}",
                                hex::encode(&tx_hash[..4]),
                                e
                            ),
                            IssueLocation::Transaction {
                                hash: tx_hash,
                                block_hash: Some(current_hash),
                            },
                            true,
                        ));
                    }
                }
            }

            // Verify block is in the height index
            match self.db.get_block_by_height(current_height)? {
                Some(indexed_block) => {
                    if indexed_block.hash() != current_hash {
                        issues.push(IntegrityIssue::new(
                            IssueType::IndexInconsistency,
                            IssueSeverity::Error,
                            format!("Block height index inconsistency at height {}", current_height),
                            IssueLocation::Chain {
                                start_height: Some(current_height),
                                end_height: Some(current_height),
                            },
                            true,
                        ));
                    }
                }
                None => {
                    issues.push(IntegrityIssue::new(
                        IssueType::IndexInconsistency,
                        IssueSeverity::Error,
                        format!("Missing block height index at height {}", current_height),
                        IssueLocation::Chain {
                            start_height: Some(current_height),
                            end_height: Some(current_height),
                        },
                        true,
                    ));
                }
            }

            // Check if we've hit a trusted checkpoint
            if let Some(trusted_hash) = self.config.trusted_checkpoints.get(&current_height) {
                if current_hash != *trusted_hash {
                    issues.push(IntegrityIssue::new(
                        IssueType::ChainInconsistency,
                        IssueSeverity::Critical,
                        format!(
                            "Chain diverges from trusted checkpoint at height {}",
                            current_height
                        ),
                        IssueLocation::Chain {
                            start_height: Some(current_height),
                            end_height: None,
                        },
                        true,
                    ));
                }
                break; // Stop at trusted checkpoint
            }

            // Move to previous block
            current_hash = *block.prev_block_hash();
            current_height -= 1;
        }

        Ok(())
    }
}

impl<'a> UtxoVerifier<'a> {
    /// Create a new UTXO set verifier
    pub fn new(db: Arc<BlockchainDB>, config: &'a IntegrityConfig) -> Self {
        Self { db, config }
    }

    /// Verify UTXO set consistency
    pub async fn verify(&self, issues: &mut Vec<IntegrityIssue>) -> Result<(), StorageError> {
        debug!("Verifying UTXO set consistency");

        // Get current chain height and best hash
        let height_bytes = match self.db.get_metadata(b"height")? {
            Some(h) => h,
            None => return Ok(()), // Already reported in structure check
        };

        let best_hash_bytes = match self.db.get_metadata(b"best_hash")? {
            Some(h) => h,
            None => return Ok(()), // Already reported in structure check
        };

        let height: u64 = bincode::deserialize(&height_bytes)?;
        let mut best_hash = [0u8; 32];
        best_hash.copy_from_slice(&best_hash_bytes);

        // Build a partial UTXO set by replaying recent blocks
        let mut utxos = HashMap::new();
        let mut spent_outputs = HashSet::new();

        // Limit how far back we go
        let blocks_to_check = std::cmp::min(self.config.max_batch_size, height as usize);
        let mut current_hash = best_hash;
        let mut current_height = height;

        // Process blocks to build UTXO set
        for _ in 0..blocks_to_check {
            if current_height == 0 {
                break;
            }

            let block = match self.db.get_block(&current_hash)? {
                Some(b) => b,
                None => break, // Already reported in blockchain verification
            };

            // Process transactions
            for tx in block.transactions() {
                let tx_hash = tx.hash();

                // Process inputs (mark as spent)
                for input in tx.inputs() {
                    let prev_tx = input.prev_tx_hash();
                    let prev_idx = input.prev_output_index();
                    spent_outputs.insert((prev_tx, prev_idx));
                    utxos.remove(&(prev_tx, prev_idx));
                }

                // Process outputs (add to UTXOs if not spent)
                for (idx, output) in tx.outputs().iter().enumerate() {
                    let outpoint = (tx_hash, idx as u32);
                    if !spent_outputs.contains(&outpoint) {
                        utxos.insert(outpoint, output.clone());
                    }
                }
            }

            // Move to previous block
            current_hash = *block.prev_block_hash();
            current_height -= 1;
        }

        // Verify a sample of UTXOs against the database
        // This is a partial verification to avoid excessive database access
        let sample_size = (utxos.len() * self.config.utxo_sample_percentage) / 100;
        let sample = utxos
            .iter()
            .take(sample_size)
            .collect::<Vec<_>>();

        for ((tx_hash, output_idx), expected_output) in sample {
            // Check UTXO exists in database
            match self.db.get_utxo(tx_hash, *output_idx) {
                Ok(Some(stored_output)) => {
                    // Verify output matches expected value
                    let stored: btclib::types::TransactionOutput =
                        bincode::deserialize(&stored_output)?;
                    
                    if stored.value() != expected_output.value() {
                        issues.push(IntegrityIssue::new(
                            IssueType::UtxoInconsistency,
                            IssueSeverity::Error,
                            format!(
                                "UTXO {}:{} value mismatch: expected {}, found {}",
                                hex::encode(&tx_hash[..4]),
                                output_idx,
                                expected_output.value(),
                                stored.value()
                            ),
                            IssueLocation::Utxo {
                                tx_hash: *tx_hash,
                                output_index: *output_idx,
                            },
                            true,
                        ));
                    }
                }
                Ok(None) => {
                    issues.push(IntegrityIssue::new(
                        IssueType::UtxoInconsistency,
                        IssueSeverity::Error,
                        format!(
                            "Expected UTXO {}:{} is missing from database",
                            hex::encode(&tx_hash[..4]),
                            output_idx
                        ),
                        IssueLocation::Utxo {
                            tx_hash: *tx_hash,
                            output_index: *output_idx,
                        },
                        true,
                    ));
                }
                Err(e) => {
                    issues.push(IntegrityIssue::new(
                        IssueType::Structure,
                        IssueSeverity::Error,
                        format!(
                            "Error accessing UTXO {}:{}: {}",
                            hex::encode(&tx_hash[..4]),
                            output_idx,
                            e
                        ),
                        IssueLocation::Utxo {
                            tx_hash: *tx_hash,
                            output_index: *output_idx,
                        },
                        true,
                    ));
                }
            }
        }

        // Also check for double-spends in the UTXO set
        // This is an expensive operation, so we limit it to a reasonable number
        let double_spend_check_count = std::cmp::min(100, blocks_to_check);
        let mut txs_checked = 0;
        
        // Reset to chain tip
        current_hash = best_hash;
        current_height = height;
        
        // Create a set to track spent outputs
        let mut all_spent = HashSet::new();
        let mut double_spends = HashSet::new();
        
        for _ in 0..double_spend_check_count {
            if current_height == 0 {
                break;
            }

            let block = match self.db.get_block(&current_hash)? {
                Some(b) => b,
                None => break,
            };

            for tx in block.transactions() {
                txs_checked += 1;
                
                // Check each input for double spends
                for input in tx.inputs() {
                    let outpoint = (input.prev_tx_hash(), input.prev_output_index());
                    
                    if !all_spent.insert(outpoint) {
                        double_spends.insert(outpoint);
                    }
                }
            }

            current_hash = *block.prev_block_hash();
            current_height -= 1;
        }
        
        // Report any double spends found
        for (tx_hash, output_idx) in double_spends {
            issues.push(IntegrityIssue::new(
                IssueType::UtxoInconsistency,
                IssueSeverity::Critical,
                format!(
                    "Double spend detected for UTXO {}:{}",
                    hex::encode(&tx_hash[..4]),
                    output_idx
                ),
                IssueLocation::Utxo {
                    tx_hash,
                    output_index: output_idx,
                },
                true,
            ));
        }

        Ok(())
    }
}

impl<'a> CryptoVerifier<'a> {
    /// Create a new cryptographic verifier
    pub fn new(db: Arc<BlockchainDB>, config: &'a IntegrityConfig) -> Self {
        Self { db, config }
    }

    /// Verify cryptographic integrity
    pub async fn verify(&self, issues: &mut Vec<IntegrityIssue>) -> Result<(), StorageError> {
        debug!("Verifying cryptographic integrity");

        // Get current chain height and best hash
        let height_bytes = match self.db.get_metadata(b"height")? {
            Some(h) => h,
            None => return Ok(()), // Already reported in structure check
        };

        let best_hash_bytes = match self.db.get_metadata(b"best_hash")? {
            Some(h) => h,
            None => return Ok(()), // Already reported in structure check
        };

        let height: u64 = bincode::deserialize(&height_bytes)?;
        let mut best_hash = [0u8; 32];
        best_hash.copy_from_slice(&best_hash_bytes);

        // For cryptographic verification, we'll sample blocks at regular intervals
        // This is computationally expensive, so we don't want to do every block
        let mut current_hash = best_hash;
        let mut current_height = height;
        
        // Determine how many blocks to check and the sampling interval
        let blocks_to_check = std::cmp::min(self.config.max_batch_size, height as usize);
        let interval = std::cmp::max(1, blocks_to_check / 10);  // Check about 10 blocks
        
        for i in 0..blocks_to_check {
            // Only check blocks at regular intervals to save computation
            if i % interval != 0 && i != 0 && i != blocks_to_check - 1 {
                // Get block to continue traversal, but don't verify
                if let Ok(Some(block)) = self.db.get_block(&current_hash) {
                    current_hash = *block.prev_block_hash();
                    current_height -= 1;
                    continue;
                } else {
                    break;
                }
            }
            
            let block = match self.db.get_block(&current_hash)? {
                Some(b) => b,
                None => break, // Already reported
            };
            
            // Verify block hash against stored hash
            let computed_hash = block.hash();
            if computed_hash != current_hash {
                issues.push(IntegrityIssue::new(
                    IssueType::CryptoVerification,
                    IssueSeverity::Critical,
                    format!(
                        "Block hash mismatch at height {}: stored {}, computed {}",
                        current_height,
                        hex::encode(&current_hash[..8]),
                        hex::encode(&computed_hash[..8])
                    ),
                    IssueLocation::Block {
                        hash: current_hash,
                        height: Some(current_height),
                    },
                    false, // Not fixable, would need to replace the block
                ));
            }
            
            // Verify the block's proof of work
            if !self.verify_block_pow(&block) {
                issues.push(IntegrityIssue::new(
                    IssueType::CryptoVerification,
                    IssueSeverity::Critical,
                    format!(
                        "Invalid proof of work for block at height {}",
                        current_height
                    ),
                    IssueLocation::Block {
                        hash: current_hash,
                        height: Some(current_height),
                    },
                    false, // Not fixable
                ));
            }
            
            // Verify merkle root if transactions are present
            if !block.transactions().is_empty() {
                let computed_merkle_root = Self::calculate_merkle_root(block.transactions());
                let merkle_root = block.merkle_root();
                
                if computed_merkle_root != *merkle_root {
                    issues.push(IntegrityIssue::new(
                        IssueType::CryptoVerification,
                        IssueSeverity::Critical,
                        format!(
                            "Merkle root mismatch at height {}: stored {}, computed {}",
                            current_height,
                            hex::encode(&merkle_root[..8]),
                            hex::encode(&computed_merkle_root[..8])
                        ),
                        IssueLocation::Block {
                            hash: current_hash,
                            height: Some(current_height),
                        },
                        false, // Not easily fixable
                    ));
                }
            }
            
            // Verify transaction hashes
            for tx in block.transactions() {
                let stored_hash = tx.hash();
                let computed_hash = Self::calculate_tx_hash(tx);
                
                if stored_hash != computed_hash {
                    issues.push(IntegrityIssue::new(
                        IssueType::CryptoVerification,
                        IssueSeverity::Critical,
                        format!(
                            "Transaction hash mismatch in block {}: stored {}, computed {}",
                            hex::encode(&current_hash[..4]),
                            hex::encode(&stored_hash[..8]),
                            hex::encode(&computed_hash[..8])
                        ),
                        IssueLocation::Transaction {
                            hash: stored_hash,
                            block_hash: Some(current_hash),
                        },
                        false, // Not fixable
                    ));
                }
                
                // Optionally verify transaction signatures if configured
                if self.config.verify_signatures {
                    // This would be very implementation-specific
                    // Signature verification logic would go here
                }
            }
            
            // Move to previous block
            current_hash = *block.prev_block_hash();
            current_height -= 1;
            
            if current_height == 0 {
                break; // Reached genesis
            }
        }

        Ok(())
    }
    
    /// Verify proof of work for a block
    fn verify_block_pow(&self, block: &Block) -> bool {
        // The block's verify_proof_of_work method already handles this
        block.verify_proof_of_work()
    }
    
    /// Calculate merkle root from transactions
    fn calculate_merkle_root(transactions: &[Transaction]) -> [u8; 32] {
        if transactions.is_empty() {
            return [0u8; 32];
        }
        
        let mut hashes: Vec<[u8; 32]> = transactions.iter()
            .map(|tx| tx.hash())
            .collect();
            
        while hashes.len() > 1 {
            let mut new_hashes = Vec::new();
            
            for chunk in hashes.chunks(2) {
                let mut hasher = Sha256::new();
                hasher.update(&chunk[0]);
                
                // If odd number of hashes, duplicate the last one
                if chunk.len() == 2 {
                    hasher.update(&chunk[1]);
                } else {
                    hasher.update(&chunk[0]);
                }
                
                let hash_result = hasher.finalize();
                
                // Double SHA-256
                let mut hasher = Sha256::new();
                hasher.update(&hash_result);
                let result = hasher.finalize();
                
                let mut hash = [0u8; 32];
                hash.copy_from_slice(&result);
                new_hashes.push(hash);
            }
            
            hashes = new_hashes;
        }
        
        hashes[0]
    }
    
    /// Calculate transaction hash
    fn calculate_tx_hash(tx: &Transaction) -> [u8; 32] {
        let serialized = bincode::serialize(tx).unwrap_or(vec![]);
        
        let mut hasher = Sha256::new();
        hasher.update(&serialized);
        let result = hasher.finalize();
        
        // Double SHA-256 for Bitcoin-like chains
        let mut hasher = Sha256::new();
        hasher.update(&result);
        let result = hasher.finalize();
        
        let mut hash = [0u8; 32];
        hash.copy_from_slice(&result);
        hash
    }
}