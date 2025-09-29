use std::time::{Duration, Instant};
use node::mempool::{TransactionPool, MempoolConfig};
use node::storage::{BlockchainDB, ChainState};
use btclib::types::transaction::{Transaction, TransactionInput, TransactionOutput};
use std::sync::Arc;
use tempfile::tempdir;
use tracing::info;

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_transaction_performance() {
        // Create test environment
        let temp_dir = tempdir().unwrap();
        let db = Arc::new(BlockchainDB::new(temp_dir.path()).unwrap());
        let chain_state = ChainState::new(Arc::clone(&db)).unwrap();

        // Create mempool
        let mempool_config = MempoolConfig::default();
        let mempool = Arc::new(TransactionPool::new(mempool_config));

        // Benchmark transaction addition
        let start_time = Instant::now();
        const NUM_TRANSACTIONS: usize = 100;

        for i in 0..NUM_TRANSACTIONS {
            // Create a test transaction
            let tx = create_test_transaction([i as u8; 32], 1000);

            // Add to mempool
            if let Err(e) = mempool.add_transaction(tx, 2) {
                println!("Failed to add transaction: {}", e);
            }
        }

        let duration = start_time.elapsed();
        println!("Added {} transactions in {:?} (avg: {:?} per tx)",
            NUM_TRANSACTIONS,
            duration,
            duration / NUM_TRANSACTIONS as u32);

        // Verify mempool size
        let txs = mempool.get_sorted_transactions();
        assert!(txs.len() <= NUM_TRANSACTIONS);
    }

    fn create_test_transaction(prev_hash: [u8; 32], amount: u64) -> Transaction {
        Transaction::new(
            1, // version
            vec![TransactionInput::new(prev_hash, 0, vec![], 0xffffffff)],
            vec![TransactionOutput::new(amount, vec![1, 2, 3, 4])],
            0, // locktime
        )
    }
}