use serde::{Deserialize, Serialize};
use sha2::{Sha256, Digest};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockHeader {
    /// Version number to track protocol upgrades
    version: u32,
    /// Unix timestamp of when the block was created
    timestamp: u64,
    /// Hash of the previous block in the chain
    prev_block_hash: [u8; 32],
    /// Root of the merkle tree containing all transactions
    merkle_root: [u8; 32],
    /// Mining difficulty target
    target: u32,
    /// Nonce used for mining
    nonce: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Block {
    /// Block header containing metadata
    header: BlockHeader,
    /// List of transactions included in this block
    transactions: Vec<Transaction>,  // We'll implement Transaction next
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

    /// Increment the nonce value - used during mining
    pub fn increment_nonce(&mut self) {
        self.nonce = self.nonce.wrapping_add(1);
    }

    /// Calculate the hash of this block header
    pub fn hash(&self) -> [u8; 32] {
        let serialized = bincode::serialize(&self).unwrap();
        let mut hasher = Sha256::new();
        hasher.update(&serialized);
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
        // Calculate merkle root from transactions
        let merkle_root = Self::calculate_merkle_root(&transactions);
        
        Self {
            header: BlockHeader::new(version, prev_block_hash, merkle_root, target),
            transactions,
        }
    }

    /// Calculate the merkle root of the transactions
    fn calculate_merkle_root(transactions: &[Transaction]) -> [u8; 32] {
        // TODO: Implement proper merkle tree
        // For now, just hash all transactions together
        let mut hasher = Sha256::new();
        for tx in transactions {
            let tx_bytes = bincode::serialize(&tx).unwrap();
            hasher.update(&tx_bytes);
        }
        let result = hasher.finalize();
        let mut root = [0u8; 32];
        root.copy_from_slice(&result);
        root
    }

    /// Validate basic block properties
    pub fn validate(&self) -> bool {
        // Verify proof of work
        let hash = self.header.hash();
        let target = self.header.target;
        
        // Convert first 4 bytes of hash to u32 for difficulty comparison
        let hash_value = u32::from_be_bytes([hash[0], hash[1], hash[2], hash[3]]);
        
        // Check if hash meets difficulty target
        if hash_value > target {
            return false;
        }

        // Verify merkle root matches transactions
        let calculated_root = Self::calculate_merkle_root(&self.transactions);
        if calculated_root != self.header.merkle_root {
            return false;
        }

        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_block_creation() {
        let prev_hash = [0u8; 32];
        let transactions = Vec::new(); // Empty transaction list for testing
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
}