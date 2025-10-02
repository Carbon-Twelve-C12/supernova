// Block Template Generation for Mining
// Assembles transactions from mempool into minable block template

use btclib::types::block::Block;
use btclib::types::transaction::Transaction;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use thiserror::Error;

use crate::mempool::TransactionPool;
use crate::storage::ChainState;
use super::merkle::calculate_merkle_root;
use super::coinbase::build_coinbase_transaction;
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
        let difficulty_bits = 0x1d00ffff; // Initial difficulty (same as Bitcoin genesis)
        
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
        
        // Calculate merkle root
        let txids: Vec<[u8; 32]> = all_transactions.iter()
            .map(|tx| tx.hash())
            .collect();
        
        let merkle_root = calculate_merkle_root(&txids)
            .map_err(|e| TemplateError::MerkleError(e.to_string()))?;
        
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
    
    // Tests require full blockchain context
    // Will be tested via integration tests
}

