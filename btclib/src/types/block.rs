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
    pub fn new(version: u32, prev_block_hash: [u8; 32], merkle_root: [u8; 32], target: u32) -> Self {
        Self {
            version,
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            prev_block_hash,
            merkle_root,
            target,
            nonce: 0,
        }
    }

    pub fn increment_nonce(&mut self) {
        self.nonce = self.nonce.wrapping_add(1);
    }

    pub fn hash(&self) -> [u8; 32] {
        let mut hasher = Sha256::new();
        hasher.update(&bincode::serialize(&self).unwrap());
        hasher.update(&self.nonce.to_le_bytes());
        let result = hasher.finalize();
        let mut hash = [0u8; 32];
        hash.copy_from_slice(&result);
        hash
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
            header: BlockHeader::new(version, prev_block_hash, merkle_root, target),
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
        let mut header = BlockHeader::new(1, [0u8; 32], [0u8; 32], u32::MAX);
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