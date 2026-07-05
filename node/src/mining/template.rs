// Block Template Generation for Mining
// Assembles transactions from mempool into minable block template

use supernova_core::types::block::Block;
use supernova_core::types::transaction::Transaction;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use thiserror::Error;

use crate::mempool::TransactionPool;
use crate::storage::ChainState;
use super::coinbase::build_coinbase_transaction;
use supernova_core::util::merkle::MerkleTree;
use wallet::quantum_wallet::Address;

#[derive(Error, Debug)]
pub enum TemplateError {
    #[error("Chain state error: {0}")]
    ChainStateError(String),
    
    #[error("No transactions in mempool")]
    NoTransactions,
    
    #[error("Coinbase error: {0}")]
    CoinbaseError(String),
    
    #[error("Merkle root calculation failed: {0}")]
    MerkleError(String),
    
    #[error("Address error: {0}")]
    AddressError(String),
}

/// Block template for mining
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockTemplate {
    /// Block version
    pub version: u32,
    
    /// Previous block hash
    pub previous_block_hash: [u8; 32],
    
    /// Merkle root of transactions
    pub merkle_root: [u8; 32],
    
    /// Block timestamp
    pub timestamp: u64,
    
    /// Difficulty target (bits format)
    pub bits: u32,
    
    /// Block height
    pub height: u64,
    
    /// Transactions to include (including coinbase)
    pub transactions: Vec<Transaction>,
    
    /// Total fees from transactions
    pub total_fees: u64,
    
    /// Coinbase value (reward + fees)
    pub coinbase_value: u64,
}

impl BlockTemplate {
    /// Generate new block template
    pub fn generate(
        chain_state: Arc<std::sync::RwLock<ChainState>>,
        mempool: Arc<TransactionPool>,
        reward_address: &Address,
        treasury_address: &Address,
    ) -> Result<Self, TemplateError> {
        // Get current chain state
        let chain = chain_state.read()
            .map_err(|_| TemplateError::ChainStateError("Lock poisoned".to_string()))?;
        
        let height = chain.get_height() + 1;
        let prev_hash = chain.get_best_block_hash();

        // The difficulty the chain REQUIRES for this next block (#2.2) — the same
        // `required_bits` rule `validate_block` enforces, so a block mined from
        // this template is accepted rather than rejected for wrong difficulty.
        // This replaces a hardcoded easy `0x207fffff`, which the testnet floor
        // (0x1e0fffff) would reject.
        let difficulty_bits = chain.get_difficulty_target();

        drop(chain); // Release lock
        
        // Get transactions from mempool
        let mempool_txs = mempool.get_all_transactions();
        
        // Take first 100 transactions (simple selection for now)
        let selected_txs: Vec<Transaction> = mempool_txs.into_iter().take(100).collect();
        
        // Calculate total fees (simplified - in production would track fees properly)
        let total_fees: u64 = 0; // Placeholder - proper fee calculation needed
        
        // Build coinbase transaction
        let coinbase = build_coinbase_transaction(
            height,
            reward_address,
            total_fees,
            treasury_address,
        ).map_err(|e| TemplateError::CoinbaseError(e.to_string()))?;
        
        // Assemble all transactions (coinbase first)
        let mut all_transactions = vec![coinbase.clone()];
        all_transactions.extend(selected_txs);
        
        // Calculate merkle root using the EXACT algorithm consensus validation
        // uses (supernova_core MerkleTree: SHA-256, re-hashed leaves, promote-odd),
        // which is also what `to_block` -> `Block::new_with_params` recomputes into
        // the block header. The mining-local SHA3-512 merkle (super::merkle) produced
        // a different root for every block, so any block an external miner built from
        // this template's advertised `merkle_root` was rejected by
        // `Block::verify_merkle_root` in submit_block. Matching the consensus tree
        // here keeps getblocktemplate honest and unblocks external mining.
        let tx_hashes: Vec<[u8; 32]> = all_transactions.iter()
            .map(|tx| tx.hash())
            .collect();

        let merkle_root = MerkleTree::new(&tx_hashes).root_hash();
        
        // Get current timestamp
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|e| TemplateError::ChainStateError(e.to_string()))?
            .as_secs();
        
        // Calculate coinbase value
        let coinbase_value = coinbase.outputs().iter().map(|o| o.value()).sum();
        
        Ok(Self {
            version: 1,
            previous_block_hash: prev_hash,
            merkle_root,
            timestamp,
            bits: difficulty_bits,
            height,
            transactions: all_transactions,
            total_fees,
            coinbase_value,
        })
    }
    
    /// Build actual Block from template (after nonce is found)
    pub fn to_block(&self, nonce: u32) -> Block {
        let mut block = Block::new_with_params(
            1, // version
            self.previous_block_hash,
            self.transactions.clone(),
            self.bits,
        );
        
        // Set height and nonce via header
        block.header.set_height(self.height);
        block.header.set_nonce(nonce);
        block
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use supernova_core::types::transaction::{Transaction, TransactionInput, TransactionOutput};

    fn sample_txs(n: usize) -> Vec<Transaction> {
        (0..n)
            .map(|i| {
                Transaction::new(
                    1,
                    vec![TransactionInput::new([i as u8; 32], i as u32, vec![], 0xffffffff)],
                    vec![TransactionOutput::new(50_000_000 + i as u64, vec![i as u8])],
                    0,
                )
            })
            .collect()
    }

    /// Regression test for R5-30: the merkle root advertised by the mining
    /// template MUST equal the consensus merkle root that `to_block` /
    /// `Block::validate` recompute, otherwise every externally mined block is
    /// rejected by `Block::verify_merkle_root`. The template uses the
    /// supernova_core `MerkleTree`; the block header is filled by
    /// `Block::new_with_params`, which uses the same tree.
    #[test]
    fn template_merkle_root_matches_consensus() {
        for count in [1usize, 2, 3, 5, 8] {
            let txs = sample_txs(count);

            // Root computed the way BlockTemplate::generate now computes it.
            let tx_hashes: Vec<[u8; 32]> = txs.iter().map(|tx| tx.hash()).collect();
            let template_root = MerkleTree::new(&tx_hashes).root_hash();

            // Root the actual block will carry / consensus will enforce.
            let block = Block::new_with_params(1, [0u8; 32], txs.clone(), 0x1e0fffff);
            let consensus_root = block.calculate_merkle_root();

            assert_eq!(
                template_root, consensus_root,
                "template merkle root diverged from consensus for {count} txs"
            );
            assert_eq!(
                template_root, block.header.merkle_root,
                "template merkle root diverged from block header for {count} txs"
            );
            assert!(
                block.verify_merkle_root(),
                "block built from template failed verify_merkle_root for {count} txs"
            );
        }
    }
}

