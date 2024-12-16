use btclib::types::{Block, Transaction, TransactionInput, TransactionOutput};
use async_trait::async_trait;

pub const BLOCK_MAX_SIZE: usize = 1_000_000; // 1MB
pub const BLOCK_REWARD: u64 = 50 * 100_000_000; // 50 NOVA

#[async_trait]
pub trait MempoolInterface {
    async fn get_transactions(&self, max_size: usize) -> Vec<Transaction>;
}

pub struct BlockTemplate {
    version: u32,
    prev_block_hash: [u8; 32],
    target: u32,
    coinbase: Transaction,
    transactions: Vec<Transaction>,
}

impl BlockTemplate {
    pub async fn new(
        version: u32,
        prev_block_hash: [u8; 32],
        target: u32,
        reward_address: Vec<u8>,
        mempool: &dyn MempoolInterface,
    ) -> Self {
        // Create coinbase transaction
        let coinbase = Self::create_coinbase_transaction(BLOCK_REWARD, reward_address);
        
        // Get transactions from mempool
        let coinbase_size = bincode::serialize(&coinbase).unwrap().len();
        let available_size = BLOCK_MAX_SIZE - coinbase_size;
        let transactions = mempool.get_transactions(available_size).await;

        Self {
            version,
            prev_block_hash,
            target,
            coinbase,
            transactions,
        }
    }

    pub fn create_block(&self) -> Block {
        let mut transactions = vec![self.coinbase.clone()];
        transactions.extend(self.transactions.clone());

        Block::new(
            self.version,
            self.prev_block_hash,
            transactions,
            self.target,
        )
    }

    fn create_coinbase_transaction(reward: u64, reward_address: Vec<u8>) -> Transaction {
        let coinbase_input = TransactionInput::new(
            [0u8; 32],  // Previous transaction hash is zero for coinbase
            0xffffffff, // Previous output index is max value for coinbase
            vec![],     // No signature script needed for coinbase
            0,          // Sequence
        );

        let reward_output = TransactionOutput::new(
            reward,
            reward_address,
        );

        Transaction::new(
            1,  // Version
            vec![coinbase_input],
            vec![reward_output],
            0,  // Lock time
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::SystemTime;

    struct MockMempool;
    
    #[async_trait]
    impl MempoolInterface for MockMempool {
        async fn get_transactions(&self, _max_size: usize) -> Vec<Transaction> {
            Vec::new()
        }
    }

    #[tokio::test]
    async fn test_block_template_creation() {
        let mempool = MockMempool;
        let template = BlockTemplate::new(
            1,
            [0u8; 32],
            u32::MAX,
            vec![1,2,3,4],
            &mempool,
        ).await;

        let block = template.create_block();
        assert_eq!(block.transactions().len(), 1); // Only coinbase
        assert_eq!(block.transactions()[0].outputs()[0].amount(), BLOCK_REWARD);
    }
}