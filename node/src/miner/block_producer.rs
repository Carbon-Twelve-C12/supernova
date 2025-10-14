//! Block Producer
//!
//! This module is responsible for creating block templates that miners can use
//! to perform proof-of-work computations.

use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;

use crate::mempool::TransactionPool;
use crate::storage::ChainState;
use supernova_core::types::{Block, BlockHeader, Transaction};
use sha2::{Digest, Sha256};

pub struct BlockProducer {
    mempool: Arc<TransactionPool>,
    chain_state: Arc<RwLock<ChainState>>,
}

impl BlockProducer {
    pub fn new(mempool: Arc<TransactionPool>, chain_state: Arc<RwLock<ChainState>>) -> Self {
        Self {
            mempool,
            chain_state,
        }
    }

    pub async fn create_block_template(&self) -> Result<Block, String> {
        let chain_state = self.chain_state.read().await;
        let best_hash = chain_state.get_best_block_hash();
        let height = chain_state.get_height() + 1;
        let difficulty_target = chain_state.get_difficulty_target();

        // Create coinbase transaction
        let coinbase = self.create_coinbase_transaction(height);

        // Get transactions from mempool sorted by fee
        let mempool_txs = self.mempool.get_sorted_transactions();

        // Limit to reasonable number of transactions
        let max_txs = 1000;
        let mempool_txs: Vec<_> = mempool_txs.into_iter().take(max_txs).collect();

        // Combine coinbase with mempool transactions
        let mut transactions = vec![coinbase];
        transactions.extend(mempool_txs);

        let merkle_root = self.calculate_merkle_root(&transactions);

        let header = BlockHeader {
            version: 1,
            prev_block_hash: best_hash,
            merkle_root,
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("Time went backwards")
                .as_secs(),
            bits: difficulty_target,
            nonce: 0,
            height,
        };

        Ok(Block::new(header, transactions))
    }

    fn calculate_merkle_root(&self, transactions: &[Transaction]) -> [u8; 32] {
        if transactions.is_empty() {
            return [0u8; 32];
        }

        let tx_hashes: Vec<_> = transactions.iter().map(|tx| tx.hash()).collect();
        let mut level = tx_hashes;

        while level.len() > 1 {
            let mut next_level = Vec::new();
            for chunk in level.chunks(2) {
                let mut hasher = Sha256::new();
                hasher.update(chunk[0]);
                if let Some(second) = chunk.get(1) {
                    hasher.update(second);
                } else {
                    hasher.update(chunk[0]); // Duplicate last hash if odd number
                }
                let result = hasher.finalize();

                // Double SHA-256
                let mut hasher = Sha256::new();
                hasher.update(result);
                let double_hash = hasher.finalize();

                let mut hash = [0u8; 32];
                hash.copy_from_slice(&double_hash);
                next_level.push(hash);
            }
            level = next_level;
        }
        level[0]
    }

    // TODO: Implement this method to take a wallet reference
    // and create a proper coinbase transaction with the miner's reward address.
    fn create_coinbase_transaction(&self, height: u64) -> Transaction {
        // Placeholder implementation
        Transaction::new(1, vec![], vec![], 0)
    }
}
