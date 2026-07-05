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

        let header = BlockHeader {
            version: 1,
            prev_block_hash: best_hash,
            // Placeholder; the authoritative Merkle root is computed below via the
            // consensus `Block::calculate_merkle_root` so producer and validator agree.
            merkle_root: [0u8; 32],
            // A wall-clock that predates UNIX_EPOCH would indicate a misconfigured
            // host; the miner's median-time-past check still rejects the block, so
            // `Duration::ZERO` is a safe fallback that keeps us from panicking.
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            bits: difficulty_target,
            nonce: 0,
            height,
        };

        let mut block = Block::new(header, transactions);

        // Use the consensus Merkle implementation so the template's merkle_root
        // matches what `Block::verify_merkle_root` (and peers) expect.
        let merkle_root = block.calculate_merkle_root();
        block.header.merkle_root = merkle_root;

        Ok(block)
    }

    // TODO: Implement this method to take a wallet reference
    // and create a proper coinbase transaction with the miner's reward address.
    fn create_coinbase_transaction(&self, height: u64) -> Transaction {
        // Placeholder implementation
        Transaction::new(1, vec![], vec![], 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A template's merkle root must be produced by the consensus
    /// `Block::calculate_merkle_root` so it survives `verify_merkle_root`
    /// (and peer validation). This mirrors how `create_block_template`
    /// assembles the block, without needing a live `ChainState`/mempool.
    #[test]
    fn template_merkle_root_matches_consensus() {
        let build = |txs: Vec<Transaction>| {
            let header = BlockHeader {
                version: 1,
                prev_block_hash: [0u8; 32],
                merkle_root: [0u8; 32],
                timestamp: 0,
                bits: 0x207fffff,
                nonce: 0,
                height: 1,
            };
            let mut block = Block::new(header, txs);
            let merkle_root = block.calculate_merkle_root();
            block.header.merkle_root = merkle_root;
            block
        };

        // Single (coinbase-only) transaction.
        let single = build(vec![Transaction::new(1, vec![], vec![], 0)]);
        assert!(
            single.verify_merkle_root(),
            "coinbase-only template must pass consensus merkle verification"
        );

        // Multiple transactions, including an odd count to exercise odd-promotion.
        let many = build(vec![
            Transaction::new(1, vec![], vec![], 0),
            Transaction::new(2, vec![], vec![], 1),
            Transaction::new(3, vec![], vec![], 2),
        ]);
        assert!(
            many.verify_merkle_root(),
            "multi-tx template must pass consensus merkle verification"
        );
    }
}
