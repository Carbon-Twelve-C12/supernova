//! Block Producer
//!
//! This module is responsible for creating block templates that miners can use
//! to perform proof-of-work computations.

use std::sync::Arc;
use tokio::sync::RwLock;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::mempool::TransactionPool;
use crate::storage::ChainState;
use btclib::types::{Block, BlockHeader, Transaction};
use btclib::crypto::hash256;

pub struct BlockProducer {
    mempool: Arc<TransactionPool>,
    chain_state: Arc<RwLock<ChainState>>,
}

impl BlockProducer {
    pub fn new(mempool: Arc<TransactionPool>, chain_state: Arc<RwLock<ChainState>>) -> Self {
        Self { mempool, chain_state }
    }

    pub async fn create_block_template(&self) -> Result<Block, String> {
        let chain_state = self.chain_state.read().await;
        let best_hash = chain_state.get_best_block_hash();
        let height = chain_state.get_height() + 1;
        let difficulty_target = chain_state.get_difficulty_target();

        let transactions = self.mempool.get_transactions_for_block();

        // TODO: Create coinbase transaction

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
                let mut hasher = hash256::new();
                hasher.update(&chunk[0]);
                if let Some(second) = chunk.get(1) {
                    hasher.update(second);
                } else {
                    hasher.update(&chunk[0]); // Duplicate last hash if odd number
                }
                next_level.push(hasher.finalize().into());
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