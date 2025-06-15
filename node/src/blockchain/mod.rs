use btclib::types::{Block, BlockHeader, Transaction, TransactionInput, TransactionOutput};
use btclib::crypto::hash256;
use chrono::Utc;

/// Create the genesis block for a given chain ID
pub fn create_genesis_block(chain_id: &str) -> Block {
    // Create coinbase transaction
    let coinbase_script = format!("Genesis block for Supernova {}", chain_id).into_bytes();
    let coinbase_input = TransactionInput::new_coinbase(coinbase_script);
    
    // Create output with initial supply (50 NOVA)
    let initial_reward = 50_000_000_00; // 50 NOVA in attaNova
    let genesis_output = TransactionOutput::new(
        initial_reward,
        vec![], // Empty script for genesis
    );
    
    let coinbase_tx = Transaction::new(
        2, // version
        vec![coinbase_input],
        vec![genesis_output],
        0, // locktime
    );
    
    // Create genesis block header
    let timestamp = match chain_id {
        "mainnet" => 1767225600u64, // January 1, 2026 00:00:00 UTC
        "testnet" => Utc::now().timestamp() as u64,
        _ => Utc::now().timestamp() as u64,
    };
    
    // Use easier difficulty for testnet
    let difficulty_bits = match chain_id {
        "mainnet" => 0x1d00ffff, // Standard difficulty
        _ => 0x207fffff, // Very easy difficulty for testnet/devnet
    };
    
    let genesis_header = BlockHeader::new(
        1, // version
        [0u8; 32], // prev_block_hash (all zeros for genesis)
        [0u8; 32], // merkle_root (will be calculated)
        timestamp,
        difficulty_bits,
        0, // nonce
    );
    
    // Create the block
    let mut block = Block::new(genesis_header, vec![coinbase_tx]);
    
    // Calculate and set merkle root
    let merkle_root = block.calculate_merkle_root();
    block.header.merkle_root = merkle_root;
    
    // Mine the genesis block (find valid nonce)
    let target = block.header.target();
    let mut nonce = 0u32;
    loop {
        block.header.nonce = nonce;
        let hash = block.header.hash();
        if hash <= target {
            break;
        }
        nonce += 1;
    }
    
    block
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_create_genesis_block() {
        let genesis = create_genesis_block("testnet");
        assert!(genesis.validate());
        assert_eq!(genesis.header.prev_block_hash, [0u8; 32]);
        assert_eq!(genesis.transactions().len(), 1);
        assert!(genesis.transactions()[0].is_coinbase());
    }
} 