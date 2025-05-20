use serde::{Deserialize, Serialize};
use sha2::{Sha256, Digest};
use std::time::{SystemTime, UNIX_EPOCH};
use crate::util::merkle::MerkleTree;
use crate::types::transaction::{Transaction, TransactionInput, TransactionOutput};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockHeader {
    version: u32,
    timestamp: u64,
    prev_block_hash: [u8; 32],
    merkle_root: [u8; 32],
    target: u32,
    nonce: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Block {
    header: BlockHeader,
    transactions: Vec<Transaction>,
}

impl BlockHeader {
    /// Create a new block header
    pub fn new(
        version: u32,
        prev_block_hash: [u8; 32],
        merkle_root: [u8; 32],
        timestamp: u64,
        bits: u32,
        nonce: u32,
    ) -> Self {
        Self {
            version,
            prev_block_hash,
            merkle_root,
            timestamp,
            target: bits,
            nonce,
        }
    }
    
    /// Access the block version
    pub fn version(&self) -> u32 {
        self.version
    }
    
    /// Access the previous block hash
    pub fn prev_block_hash(&self) -> &[u8; 32] {
        &self.prev_block_hash
    }
    
    /// Access the merkle root
    pub fn merkle_root(&self) -> &[u8; 32] {
        &self.merkle_root
    }
    
    /// Access the timestamp
    pub fn timestamp(&self) -> u64 {
        self.timestamp
    }
    
    /// Access the bits (target)
    pub fn bits(&self) -> u32 {
        self.target
    }
    
    /// Access the nonce
    pub fn nonce(&self) -> u32 {
        self.nonce
    }
    
    /// Hash this block header
    pub fn hash(&self) -> [u8; 32] {
        // Placeholder: In a real implementation, this would hash the block header
        // with double SHA-256
        let mut hasher = sha2::Sha256::new();
        let encoded = bincode::serialize(self).unwrap_or(vec![]);
        hasher.update(&encoded);
        let first_hash = hasher.finalize();
        
        let mut hasher = sha2::Sha256::new();
        hasher.update(&first_hash);
        let hash_bytes = hasher.finalize();
        
        let mut result = [0u8; 32];
        result.copy_from_slice(&hash_bytes);
        result
    }

    pub fn increment_nonce(&mut self) {
        self.nonce = self.nonce.wrapping_add(1);
    }

    pub fn height(&self) -> u32 {
        0  // Default implementation, actual height would be tracked in chain state
    }
}

impl Block {
    pub fn new(
        version: u32,
        prev_block_hash: [u8; 32],
        transactions: Vec<Transaction>,
        target: u32,
    ) -> Self {
        let merkle_root = Self::calculate_merkle_root(&transactions);
        
        Self {
            header: BlockHeader::new(version, prev_block_hash, merkle_root, SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(), target, 0),
            transactions,
        }
    }

    pub fn hash(&self) -> [u8; 32] {
        self.header.hash()
    }

    pub fn increment_nonce(&mut self) {
        self.header.increment_nonce();
    }

    fn calculate_merkle_root(transactions: &[Transaction]) -> [u8; 32] {
        let tx_bytes: Vec<Vec<u8>> = transactions
            .iter()
            .map(|tx| bincode::serialize(&tx).unwrap())
            .collect();

        let tree = MerkleTree::new(&tx_bytes);
        tree.root_hash().unwrap_or([0u8; 32])
    }

    pub fn transactions(&self) -> &[Transaction] {
        &self.transactions
    }

    pub fn height(&self) -> u64 {
        0
    }

    pub fn prev_block_hash(&self) -> [u8; 32] {
        self.header.prev_block_hash
    }
    
    pub fn header(&self) -> &BlockHeader {
        &self.header
    }

    pub fn validate(&self) -> bool {
        let hash = self.header.hash();
        let target = self.header.target;
        
        let hash_value = u32::from_be_bytes([hash[0], hash[1], hash[2], hash[3]]);
        
        if hash_value > target {
            return false;
        }

        let calculated_root = Self::calculate_merkle_root(&self.transactions);
        if calculated_root != self.header.merkle_root {
            return false;
        }

        true
    }

    pub fn verify_transaction(&self, transaction: &Transaction) -> bool {
        let tx_bytes: Vec<Vec<u8>> = self.transactions
            .iter()
            .map(|tx| bincode::serialize(&tx).unwrap())
            .collect();

        let tree = MerkleTree::new(&tx_bytes);
        let tx_bytes = bincode::serialize(&transaction).unwrap();
        tree.verify(&tx_bytes)
    }

    /// Calculate the total fees of all transactions in the block
    pub fn calculate_total_fees(&self) -> u64 {
        // For a real implementation, we would need access to the UTXO set
        // For our purposes, we'll just simulate a fees calculation by summing
        // a percentage of each transaction's outputs as "fees"
        
        let mut total_fees = 0;
        
        // Skip the coinbase transaction (first one) when calculating fees
        for tx in self.transactions.iter().skip(1) {
            // In a real implementation, fees would be:
            // sum(inputs) - sum(outputs)
            // Here we'll estimate it at ~1% of the output values
            let tx_total: u64 = tx.outputs().iter()
                .map(|output| output.amount())
                .sum();
                
            let fee = tx_total / 100; // 1% fee estimate
            total_fees += fee;
        }
        
        total_fees
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_block_creation() {
        let prev_hash = [0u8; 32];
        let transactions = Vec::new();
        let block = Block::new(1, prev_hash, transactions, u32::MAX);
        
        assert_eq!(block.header.version, 1);
        assert_eq!(block.header.prev_block_hash, prev_hash);
        assert!(block.validate());
    }

    #[test]
    fn test_nonce_increment() {
        let mut header = BlockHeader::new(1, [0u8; 32], [0u8; 32], SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs(), u32::MAX, 0);
        let initial_nonce = header.nonce;
        header.increment_nonce();
        assert_eq!(header.nonce, initial_nonce + 1);
    }

    #[test]
    fn test_transaction_verification() {
        let tx = Transaction::new(
            1,
            vec![TransactionInput::new(
                [0u8; 32],
                0,
                vec![],
                0xffffffff,
            )],
            vec![TransactionOutput::new(
                50_000_000,
                vec![],
            )],
            0,
        );

        let prev_hash = [0u8; 32];
        let transactions = vec![tx.clone()];
        let block = Block::new(1, prev_hash, transactions, u32::MAX);

        assert!(block.verify_transaction(&tx));

        let different_tx = Transaction::new(
            1,
            vec![TransactionInput::new(
                [1u8; 32],
                0,
                vec![],
                0xffffffff,
            )],
            vec![TransactionOutput::new(
                50_000_000,
                vec![],
            )],
            0,
        );

        assert!(!block.verify_transaction(&different_tx));
    }
}