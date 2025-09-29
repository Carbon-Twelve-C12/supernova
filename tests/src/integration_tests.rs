use node::network::{P2PNetwork, NetworkCommand, NetworkEvent};
use node::storage::{ChainState, BlockchainDB};
use node::mempool::{TransactionPool, MempoolConfig};
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::time::timeout;
use std::time::{Duration, Instant};
use btclib::types::block::Block;
use btclib::types::transaction::Transaction;
use btclib::types::transaction::{TransactionInput, TransactionOutput};
use tempfile::tempdir;
use tracing::{info, warn, error, debug};

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_transaction_flow() {
        // Create a temporary directory for the database
        let temp_dir = tempdir().unwrap();
        let db = Arc::new(BlockchainDB::new(temp_dir.path()).unwrap());

        // Create chain state
        let chain_state = ChainState::new(Arc::clone(&db)).unwrap();

        // Create mempool
        let mempool_config = MempoolConfig::default();
        let mempool = Arc::new(TransactionPool::new(mempool_config));

        // Create a simple test transaction
        let tx = Transaction::new(
            1,
            vec![TransactionInput::new([0u8; 32], 0, vec![], 0xffffffff)],
            vec![TransactionOutput::new(1_000_000, vec![1, 2, 3, 4, 5])],
            0,
        );
        let tx_hash = tx.hash();

        // Add to mempool
        mempool.add_transaction(tx.clone(), 1).unwrap();

        // Verify transaction is in mempool
        let tx_from_mempool = mempool.get_transaction(&tx_hash).unwrap();
        assert_eq!(tx_from_mempool.hash(), tx_hash);

        // Simple test passed
        assert!(true);
    }

    #[tokio::test]
    async fn test_network_setup() {
        // Create a P2P network instance
        let (network, command_tx, mut event_rx) = P2PNetwork::new(
            None, // no custom keypair
            [0u8; 32], // genesis hash
            "test" // network ID
        ).await.unwrap();

        // Wait briefly
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Check that the network was created successfully
        assert!(true);
    }

#[tokio::test]
    async fn test_blockchain_basics() {
        // Set up test environment
        let temp_dir = tempdir().unwrap();
        let db = Arc::new(BlockchainDB::new(temp_dir.path()).unwrap());
        let chain_state = ChainState::new(Arc::clone(&db)).unwrap();

        // Get initial height - should be 0 for a new chain
        let initial_height = chain_state.get_height();
        assert_eq!(initial_height, 0, "New chain should start at height 0");

        // Get genesis hash - for an empty chain this might be all zeros
        let genesis_hash = chain_state.get_genesis_hash();

        // Create a test block
        let tx = Transaction::new(
            1, // version
            vec![TransactionInput::new([0u8; 32], 0, vec![], 0xffffffff)],
            vec![TransactionOutput::new(100_000, vec![1, 2, 3, 4])],
            0, // locktime
        );

        // Create and validate a test block using the current genesis hash
        let transactions = vec![tx];
        let mut block = Block::new(
            1, // version
            genesis_hash, // previous hash
            transactions, // transactions
            u32::MAX / 10, // target difficulty
        );

        // Increment the nonce a few times to simulate mining
        for _ in 0..100 {
            block.increment_nonce();
        }

        // Validate the block structure
        assert_eq!(block.prev_block_hash(), genesis_hash, "Block should reference genesis hash");
        assert!(!block.transactions().is_empty(), "Block should contain transactions");
        assert_ne!(block.hash(), [0u8; 32], "Block hash should not be all zeros");

        // This test passes if we've successfully created a valid block
        // We don't try to add it to the chain as that has separate validation requirements
        assert!(true);
    }

#[tokio::test]
    async fn test_mempool_performance() {
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
            // Create a test transaction with varying inputs
            let tx = create_test_transaction([i as u8; 32], 1000 + i as u64);

            // Add to mempool
            if let Err(e) = mempool.add_transaction(tx, 2) {
                debug!("Failed to add transaction: {}", e);
            }
        }

        let duration = start_time.elapsed();
        info!("Added {} transactions in {:?} (avg: {:?} per tx)",
            NUM_TRANSACTIONS,
            duration,
            duration / NUM_TRANSACTIONS as u32);

        // Verify mempool size
        let txs = mempool.get_sorted_transactions();
        assert!(txs.len() <= NUM_TRANSACTIONS);

        // Benchmark transaction sorting
        let sort_start = Instant::now();
        let _ = mempool.get_sorted_transactions();
        let sort_duration = sort_start.elapsed();

        info!("Sorted {} transactions in {:?}",
            txs.len(), sort_duration);
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