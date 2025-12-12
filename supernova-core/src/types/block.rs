use crate::hash::{hash256, Hash256};
use crate::types::transaction::Transaction;
use crate::util::merkle::MerkleTree;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::time::{SystemTime, UNIX_EPOCH};

// Placeholder network protocol types for compilation compatibility
pub mod network_protocol {

    #[derive(Debug, Clone)]
    pub struct BlockHeader {
        pub version: u32,
        pub prev_block_hash: [u8; 32],
        pub merkle_root: [u8; 32],
        pub timestamp: u64,
        pub bits: u32,
        pub nonce: u32,
    }

    #[derive(Debug, Clone)]
    pub struct Block {
        // Placeholder fields
    }
}

/// BlockHeader structure representing the header of a block in the blockchain
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BlockHeader {
    /// Block version
    pub version: u32,

    /// Hash of the previous block header
    pub prev_block_hash: Hash256,

    /// Merkle root of the transactions in this block
    pub merkle_root: Hash256,

    /// Timestamp of the block (seconds since Unix epoch)
    pub timestamp: u64,

    /// Target difficulty bits
    pub bits: u32,

    /// Nonce used for proof of work
    pub nonce: u32,

    /// Height of this block in the blockchain
    pub height: u64,
}

impl BlockHeader {
    /// Create a new block header
    pub fn new(
        version: u32,
        prev_block_hash: Hash256,
        merkle_root: Hash256,
        timestamp: u64,
        bits: u32,
        nonce: u32,
    ) -> Self {
        Self {
            version,
            prev_block_hash,
            merkle_root,
            timestamp,
            bits,
            nonce,
            height: 0, // Default height, should be set by chain state
        }
    }

    /// Create a new block header with height
    pub fn new_with_height(
        version: u32,
        prev_block_hash: Hash256,
        merkle_root: Hash256,
        timestamp: u64,
        bits: u32,
        nonce: u32,
        height: u64,
    ) -> Self {
        Self {
            version,
            prev_block_hash,
            merkle_root,
            timestamp,
            bits,
            nonce,
            height,
        }
    }

    /// Calculate the hash of this block header
    pub fn hash(&self) -> Hash256 {
        hash256(&self.serialize_for_hash())
    }

    /// Serialize this header for hashing
    fn serialize_for_hash(&self) -> Vec<u8> {
        let mut buffer = Vec::new();

        // Version (Little Endian)
        buffer.extend_from_slice(&self.version.to_le_bytes());
        // Previous Block Hash (Little Endian)
        buffer.extend_from_slice(&self.prev_block_hash);
        // Merkle Root (Little Endian)
        buffer.extend_from_slice(&self.merkle_root);
        // Timestamp (Little Endian)
        buffer.extend_from_slice(&self.timestamp.to_le_bytes());
        // Target difficulty bits (Little Endian)
        buffer.extend_from_slice(&self.bits.to_le_bytes());
        // Nonce (Little Endian)
        buffer.extend_from_slice(&self.nonce.to_le_bytes());

        buffer
    }

    /// Check if the block header hash meets the target difficulty
    pub fn meets_target(&self) -> bool {
        let target = bits_to_target(self.bits);
        let hash_val = self.hash();

        // Compare as 256-bit little-endian integers
        // Start from the most significant byte (last element in little-endian)
        for i in (0..32).rev() {
            if hash_val[i] < target[i] {
                return true;
            } else if hash_val[i] > target[i] {
                return false;
            }
            // If equal, continue to next byte
        }
        // If all bytes are equal, hash meets target
        true
    }

    /// Increment the nonce for mining
    pub fn increment_nonce(&mut self) {
        self.nonce = self.nonce.wrapping_add(1);
    }

    /// Convert to the network protocol format
    pub fn to_protocol_header(&self) -> network_protocol::BlockHeader {
        network_protocol::BlockHeader {
            version: self.version,
            prev_block_hash: self.prev_block_hash,
            merkle_root: self.merkle_root,
            timestamp: self.timestamp,
            bits: self.bits,
            nonce: self.nonce,
        }
    }

    /// Create from network protocol format
    pub fn from_protocol_header(header: &network_protocol::BlockHeader) -> Self {
        Self {
            version: header.version,
            prev_block_hash: header.prev_block_hash,
            merkle_root: header.merkle_root,
            timestamp: header.timestamp,
            bits: header.bits,
            nonce: header.nonce,
            height: 0, // Height must be set separately by chain state
        }
    }

    /// Get the height (returns the stored height)
    pub fn height(&self) -> u64 {
        self.height
    }

    /// Get version
    pub fn version(&self) -> u32 {
        self.version
    }

    /// Get bits value
    pub fn bits(&self) -> u32 {
        self.bits
    }

    /// Get the hash of the previous block
    pub fn prev_block_hash(&self) -> &[u8; 32] {
        &self.prev_block_hash
    }

    /// Get timestamp
    pub fn timestamp(&self) -> u64 {
        self.timestamp
    }

    /// Get merkle root
    pub fn merkle_root(&self) -> &[u8; 32] {
        &self.merkle_root
    }

    /// Set the height of this block header
    pub fn set_height(&mut self, height: u64) {
        self.height = height;
    }

    /// Set the timestamp of this block header
    pub fn set_timestamp(&mut self, timestamp: u64) {
        self.timestamp = timestamp;
    }

    /// Set the nonce of this block header
    pub fn set_nonce(&mut self, nonce: u32) {
        self.nonce = nonce;
    }

    /// Get the target as a 256-bit value
    pub fn target(&self) -> [u8; 32] {
        bits_to_target(self.bits)
    }
}

impl fmt::Display for BlockHeader {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "BlockHeader {{ version: {}, prev_block: {}, merkle_root: {}, timestamp: {}, bits: {:#x}, nonce: {} }}",
            self.version,
            hex::encode(self.prev_block_hash),
            hex::encode(self.merkle_root),
            self.timestamp,
            self.bits,
            self.nonce
        )
    }
}

/// Convert difficulty bits to a target hash
/// Compact representation: target = coefficient * 256^(exponent - 3)
fn bits_to_target(bits: u32) -> [u8; 32] {
    let mut target = [0u8; 32];

    // Extract the exponent (size in bytes) and coefficient
    let exponent = ((bits >> 24) & 0xFF) as usize;
    let coefficient = bits & 0x00FFFFFF;

    // Calculate the target based on the formula: target = coefficient * 256^(exponent-3)
    // The target is stored in little-endian (least significant byte first)
    
    if exponent <= 3 {
        // Handle special case where exponent <= 3
        // The coefficient itself needs to be shifted right
        let shift = 8 * (3 - exponent);
        let value = coefficient >> shift;
        target[0] = (value & 0xFF) as u8;
        if value > 0xFF {
            target[1] = ((value >> 8) & 0xFF) as u8;
        }
        if value > 0xFFFF {
            target[2] = ((value >> 16) & 0xFF) as u8;
        }
    } else if exponent <= 32 {
        // Normal case: place coefficient bytes starting at position (exponent - 3)
        // in little-endian order (lowest byte first)
        let start_pos = exponent - 3;
        
        // Place the 3 coefficient bytes in little-endian order
        if start_pos < 32 {
            target[start_pos] = (coefficient & 0xFF) as u8;
        }
        if start_pos + 1 < 32 {
            target[start_pos + 1] = ((coefficient >> 8) & 0xFF) as u8;
        }
        if start_pos + 2 < 32 {
            target[start_pos + 2] = ((coefficient >> 16) & 0xFF) as u8;
        }
    }

    target
}

/// Block structure representing a full block in the blockchain
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Block {
    /// Block header
    pub header: BlockHeader,

    /// Transactions in this block
    pub transactions: Vec<Transaction>,
}

impl Block {
    /// Create a new block
    pub fn new(header: BlockHeader, transactions: Vec<Transaction>) -> Self {
        Self {
            header,
            transactions,
        }
    }

    /// Create a new block with specific parameters
    pub fn new_with_params(
        version: u32,
        prev_block_hash: Hash256,
        transactions: Vec<Transaction>,
        bits: u32,
    ) -> Self {
        // Calculate Merkle root
        let merkle_root = if transactions.is_empty() {
            [0; 32]
        } else {
            let tx_hashes: Vec<Hash256> = transactions.iter().map(|tx| tx.hash()).collect();
            let merkle_tree = MerkleTree::new(&tx_hashes);
            merkle_tree.root_hash()
        };

        // Create timestamp
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("Time went backwards")
            .as_secs();

        // Create header
        let header = BlockHeader::new(
            version,
            prev_block_hash,
            merkle_root,
            timestamp,
            bits,
            0, // Initial nonce
        );

        Self {
            header,
            transactions,
        }
    }

    /// Get the block hash
    pub fn hash(&self) -> Hash256 {
        self.header.hash()
    }

    /// Calculate the Merkle root of the transactions
    pub fn calculate_merkle_root(&self) -> Hash256 {
        if self.transactions.is_empty() {
            return [0; 32];
        }

        // Get transaction hashes
        let tx_hashes: Vec<Hash256> = self.transactions.iter().map(|tx| tx.hash()).collect();

        // Create Merkle tree
        let merkle_tree = MerkleTree::new(&tx_hashes);
        merkle_tree.root_hash()
    }

    /// Verify that the block meets the target difficulty
    pub fn verify_proof_of_work(&self) -> bool {
        self.header.meets_target()
    }

    /// Verify the Merkle root matches the transactions
    pub fn verify_merkle_root(&self) -> bool {
        let calculated = self.calculate_merkle_root();
        calculated == self.header.merkle_root
    }

    /// Validate the block structure and proof of work
    pub fn validate(&self) -> bool {
        // Verify proof of work
        if !self.verify_proof_of_work() {
            return false;
        }

        // Verify Merkle root
        if !self.verify_merkle_root() {
            return false;
        }

        // Validate transactions
        if !self.validate_transactions() {
            return false;
        }

        true
    }

    /// Validate all transactions in the block
    pub fn validate_transactions(&self) -> bool {
        // Check that there is at least one transaction (coinbase)
        if self.transactions.is_empty() {
            return false;
        }

        // Check that the first transaction is a coinbase
        if !self.transactions[0].is_coinbase() {
            return false;
        }

        // Check that no other transaction is a coinbase
        for tx in &self.transactions[1..] {
            if tx.is_coinbase() {
                return false;
            }
        }

        // Validate each transaction
        for tx in &self.transactions {
            if !tx.validate_basic() {
                return false;
            }
        }

        true
    }

    /// Verify a specific transaction is in this block
    pub fn verify_transaction(&self, tx: &Transaction) -> bool {
        // Check if transaction exists in block
        self.transactions.iter().any(|t| t.hash() == tx.hash())
    }

    /// Get the total size of the block in bytes
    pub fn size(&self) -> usize {
        // Size of header
        let header_size = 80; // Fixed size: 4 + 32 + 32 + 4 + 4 + 4

        // Size of transaction count varint
        let tx_count_size = 1; // Simplified for this example

        // Sum of transaction sizes
        let tx_sizes: usize = self.transactions.iter().map(|tx| tx.calculate_size()).sum();

        header_size + tx_count_size + tx_sizes
    }

    /// Create a new genesis block
    pub fn genesis() -> Self {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("Time went backwards")
            .as_secs();

        let header = BlockHeader {
            version: 1,
            prev_block_hash: [0; 32],
            merkle_root: [0; 32], // Will be updated after adding coinbase
            timestamp,
            bits: 0x1f00ffff, // Easy difficulty for testing
            nonce: 0,
            height: 0, // Genesis block is at height 0
        };

        // Create a coinbase transaction
        let coinbase = Transaction::new_coinbase();

        let mut block = Block {
            header,
            transactions: vec![coinbase],
        };

        // Update the Merkle root
        let merkle_root = block.calculate_merkle_root();
        block.header.merkle_root = merkle_root;

        block
    }

    /// Serialize to binary format
    pub fn serialize(&self) -> Vec<u8> {
        bincode::serialize(self).expect("Failed to serialize block")
    }

    /// Deserialize from binary format
    pub fn deserialize(data: &[u8]) -> Result<Self, Box<dyn std::error::Error>> {
        let block: Block = bincode::deserialize(data)?;
        Ok(block)
    }

    /// Convert to the network protocol format block
    pub fn to_network_block(&self) -> Vec<u8> {
        // Serialize the entire block for network transmission
        self.serialize()
    }

    /// Get a reference to the block header
    pub fn header(&self) -> &BlockHeader {
        &self.header
    }

    /// Get the hash of the previous block
    pub fn prev_block_hash(&self) -> &[u8; 32] {
        &self.header.prev_block_hash
    }

    /// Get the transactions in this block
    pub fn transactions(&self) -> &Vec<Transaction> {
        &self.transactions
    }

    /// Calculate the total fees in this block
    pub fn calculate_total_fees(&self) -> u64 {
        // In a full implementation, we would:
        // 1. Sum up all input values
        // 2. Sum up all output values
        // 3. Subtract outputs from inputs
        // 4. Subtract block reward

        // For now, we'll return a simple estimate based on transaction count
        let base_fee = 1000; // 1000 nova units
        if self.transactions.is_empty() {
            0
        } else {
            // Skip coinbase transaction
            let transaction_count = self.transactions.len() - 1;
            base_fee * transaction_count as u64
        }
    }

    /// Increment the nonce for mining (delegates to header)
    pub fn increment_nonce(&mut self) {
        self.header.increment_nonce();
    }

    /// Get the height of this block (delegates to header)
    /// Note: In practice, this should be set by the chain state manager
    pub fn height(&self) -> u64 {
        self.header.height()
    }

    /// Get the timestamp of this block (delegates to header)
    pub fn timestamp(&self) -> u64 {
        self.header.timestamp
    }

    /// Get the merkle root of this block (delegates to header)
    pub fn merkle_root(&self) -> &[u8; 32] {
        &self.header.merkle_root
    }

    /// Set the height of this block (updates the header)
    /// This should be called by the chain state manager when adding blocks
    pub fn set_height(&mut self, height: u64) {
        self.header.set_height(height);
    }

    /// Get the version of this block (delegates to header)
    pub fn version(&self) -> u32 {
        self.header.version
    }

    /// Get the nonce of this block (delegates to header)
    pub fn nonce(&self) -> u32 {
        self.header.nonce
    }
}

impl fmt::Display for Block {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Block {{ hash: {}, height: N/A, tx_count: {} }}",
            hex::encode(self.hash()),
            self.transactions.len()
        )
    }
}

// Add this implementation to support conversion from network protocol block to core block
impl network_protocol::Block {
    pub fn from_core_block(block: &Block) -> Self {
        // This is a placeholder implementation that should be replaced with proper conversion
        Self {}
    }

    pub fn to_core_block(&self) -> Block {
        // This is a placeholder implementation that should be replaced with proper conversion
        let header = BlockHeader::new(
            1,       // version
            [0; 32], // prev_block_hash
            [0; 32], // merkle_root
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(), // timestamp
            0x1d00ffff, // bits
            0,       // nonce
        );

        Block::new(header, Vec::new())
    }

    pub fn hash(&self) -> [u8; 32] {
        [0; 32] // Placeholder implementation
    }
}

// Add this implementation to support conversion between protocol and core header types
impl network_protocol::BlockHeader {
    pub fn to_core_header(&self) -> BlockHeader {
        BlockHeader {
            version: self.version,
            prev_block_hash: self.prev_block_hash,
            merkle_root: self.merkle_root,
            timestamp: self.timestamp,
            bits: self.bits,
            nonce: self.nonce,
            height: 0, // Height must be set separately by chain state
        }
    }

    pub fn from_core_header(header: &BlockHeader) -> Self {
        Self {
            version: header.version,
            prev_block_hash: header.prev_block_hash,
            merkle_root: header.merkle_root,
            timestamp: header.timestamp,
            bits: header.bits,
            nonce: header.nonce,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_common::{Transaction, TransactionInput, TransactionOutput};

    #[test]
    fn test_block_creation() {
        let prev_hash = [0u8; 32];
        // Create a coinbase transaction
        let coinbase_input = TransactionInput::new_coinbase(vec![1, 2, 3]);
        let coinbase_output = TransactionOutput::new(50_000_000_000, vec![1, 2, 3, 4]);
        let coinbase_tx = Transaction::new(1, vec![coinbase_input], vec![coinbase_output], 0);
        let transactions = vec![coinbase_tx];

        let block = Block::new_with_params(1, prev_hash, transactions, 0x1d00ffff);

        assert_eq!(block.header.version, 1);
        assert_eq!(block.header.prev_block_hash, prev_hash);
        assert!(block.verify_merkle_root());
        assert!(block.validate_transactions());
    }

    #[test]
    fn test_nonce_increment() {
        let mut header = BlockHeader::new(
            1,
            [0u8; 32],
            [0u8; 32],
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            0x1d00ffff,
            0,
        );

        let initial_nonce = header.nonce;
        header.increment_nonce();
        assert_eq!(header.nonce, initial_nonce + 1);
    }

    #[test]
    fn test_transaction_verification() {
        // Create a coinbase transaction
        let coinbase = Transaction::new_coinbase();

        // Create a block with the coinbase transaction
        let prev_hash = [0u8; 32];
        let block = Block::new_with_params(1, prev_hash, vec![coinbase.clone()], 0x1d00ffff);

        // Verify the transaction is in the block
        assert!(block.verify_transaction(&coinbase));

        // Create a different transaction
        let different_tx = Transaction::new(
            1,
            vec![TransactionInput::new([1u8; 32], 0, vec![], 0xffffffff)],
            vec![TransactionOutput::new(50_000_000, vec![])],
            0,
        );

        // Verify different transaction is not in the block
        assert!(!block.verify_transaction(&different_tx));
    }

    #[test]
    fn test_merkle_root_calculation() {
        // Create a coinbase transaction
        let coinbase = Transaction::new_coinbase();

        // Create a block with the coinbase transaction
        let prev_hash = [0u8; 32];
        let mut block = Block::new_with_params(1, prev_hash, vec![coinbase], 0x1d00ffff);

        // Calculate Merkle root manually
        let merkle_root = block.calculate_merkle_root();

        // Set the calculated Merkle root
        block.header.merkle_root = merkle_root;

        // Verify the Merkle root
        assert!(block.verify_merkle_root());

        // Modify the Merkle root to be invalid
        block.header.merkle_root = [1u8; 32];

        // Verify the Merkle root is now invalid
        assert!(!block.verify_merkle_root());
    }
}
