use btclib::types::{Block, Transaction, TransactionInput, TransactionOutput};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::mpsc;
use sha2::{Sha256, Digest};
use tracing;

pub struct MiningWorker {
    pub(crate) stop_signal: Arc<AtomicBool>,
    pub(crate) block_sender: mpsc::Sender<Block>,
    pub(crate) target: u32,
    pub(crate) worker_id: usize,
}

impl MiningWorker {
    pub fn new(
        stop_signal: Arc<AtomicBool>,
        block_sender: mpsc::Sender<Block>,
        target: u32,
        worker_id: usize,
    ) -> Self {
        Self {
            stop_signal,
            block_sender,
            target,
            worker_id,
        }
    }

    pub async fn mine_block(
        &self,
        version: u32,
        prev_block_hash: [u8; 32],
        transactions: Vec<Transaction>,
    ) -> Result<(), String> {
        let mut block = Block::new(
            version,
            prev_block_hash,
            transactions,
            self.target,
        );

        let mut attempts = 0;
        while !self.stop_signal.load(Ordering::Relaxed) {
            if attempts % 1000000 == 0 {
                tracing::info!(
                    "Worker {} - Mining attempt {}", 
                    self.worker_id, 
                    attempts
                );
            }

            // Try current nonce
            if self.check_proof_of_work(&block) {
                // Found a valid block!
                tracing::info!(
                    "Worker {} - Found valid block after {} attempts!", 
                    self.worker_id, 
                    attempts
                );
                self.block_sender.send(block.clone()).await.map_err(|e| e.to_string())?;
                return Ok(());
            }

            // Increment nonce and try again
            block.increment_nonce();
            attempts += 1;
        }

        Ok(())
    }

    fn check_proof_of_work(&self, block: &Block) -> bool {
        let hash = block.hash();
        let hash_value = u32::from_be_bytes([hash[0], hash[1], hash[2], hash[3]]);
        hash_value <= self.target
    }

    fn create_coinbase_transaction(&self, reward: u64) -> Transaction {
        let coinbase_input = TransactionInput::new(
            [0u8; 32],  // Previous transaction hash is zero for coinbase
            0xffffffff, // Previous output index is max value for coinbase
            vec![],     // No signature script needed for coinbase
            0,          // Sequence
        );

        let reward_output = TransactionOutput::new(
            reward,
            vec![], // TODO: Add proper public key script
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
    use tokio::sync::mpsc;

    #[tokio::test]
    async fn test_mining_worker() {
        let (tx, mut rx) = mpsc::channel(1);
        let stop_signal = Arc::new(AtomicBool::new(false));
        let worker = MiningWorker::new(
            Arc::clone(&stop_signal),
            tx,
            u32::MAX, // Use maximum target for quick testing
            0,
        );

        // Start mining
        let mining_handle = tokio::spawn(async move {
            worker.mine_block(1, [0u8; 32], Vec::new()).await.unwrap();
        });

        // Wait for a block or timeout
        tokio::select! {
            Some(block) = rx.recv() => {
                assert!(block.validate());
            }
            _ = tokio::time::sleep(tokio::time::Duration::from_secs(1)) => {
                stop_signal.store(true, Ordering::Relaxed);
                panic!("Mining timed out");
            }
        }

        mining_handle.await.unwrap();
    }
}