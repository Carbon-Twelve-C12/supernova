//! Hardcoded Genesis Block Constants for Supernova Testnet
//!
//! This module contains the pre-mined testnet genesis block parameters.
//! All testnet nodes MUST use these exact values to ensure network consensus.
//!
//! These values were generated using fixed parameters:
//! - Timestamp: 1730044800 (October 27, 2025 16:00:00 UTC)
//! - Difficulty: 0x207fffff (very easy for testnet)
//! - Coinbase: "Genesis block for Supernova supernova-testnet"
//! - Reward: 50 NOVA

use supernova_core::types::{Block, BlockHeader, Transaction, TransactionInput, TransactionOutput};

/// Testnet Genesis Block Constants
/// These values are IMMUTABLE and define the testnet's origin

pub const TESTNET_GENESIS_TIMESTAMP: u64 = 1730044800; // October 27, 2025 16:00:00 UTC
pub const TESTNET_GENESIS_DIFFICULTY_BITS: u32 = 0x207fffff;
pub const TESTNET_GENESIS_VERSION: u32 = 1;
pub const TESTNET_GENESIS_REWARD: u64 = 50_000_000_00; // 50 NOVA in attaNova

/// Pre-mined nonce
/// This was generated on October 28, 2025 and is IMMUTABLE for all testnet nodes
pub const TESTNET_GENESIS_NONCE: u32 = 1;

/// Pre-calculated merkle root
/// This was generated on October 28, 2025 and is IMMUTABLE for all testnet nodes
pub const TESTNET_GENESIS_MERKLE_ROOT: [u8; 32] = [
    0x7f, 0x6a, 0xfa, 0x67, 0x18, 0xce, 0x43, 0x70,
    0x6c, 0x7b, 0x3a, 0xeb, 0x48, 0xd0, 0xca, 0xb9,
    0xcb, 0x90, 0xd5, 0xfa, 0xc0, 0x0d, 0x88, 0xa4,
    0x96, 0xae, 0xe0, 0x15, 0x54, 0x16, 0x98, 0xc3
];

/// Expected genesis block hash (for validation)
/// This was generated on October 28, 2025 and is IMMUTABLE for all testnet nodes
/// Hash: 88771cecb50a860f8d82e3a2723f07ad254e9685cdd7b0b78fadcc4c158b5f32
pub const TESTNET_GENESIS_HASH: [u8; 32] = [
    0x88, 0x77, 0x1c, 0xec, 0xb5, 0x0a, 0x86, 0x0f,
    0x8d, 0x82, 0xe3, 0xa2, 0x72, 0x3f, 0x07, 0xad,
    0x25, 0x4e, 0x96, 0x85, 0xcd, 0xd7, 0xb0, 0xb7,
    0x8f, 0xad, 0xcc, 0x4c, 0x15, 0x8b, 0x5f, 0x32
];

/// Create the hardcoded testnet genesis block
/// This returns the exact same genesis block on every call
pub fn create_testnet_genesis_block() -> Result<Block, String> {
    // CRITICAL: Genesis constants MUST be hardcoded for production testnet
    // If nonce is 0, genesis wasn't properly initialized - this is a deployment error
    if TESTNET_GENESIS_NONCE == 0 {
        tracing::error!("CRITICAL: Genesis constants not set! This indicates incorrect deployment.");
        tracing::error!("For production testnet, genesis MUST be hardcoded in genesis.rs");
        tracing::error!("Mining genesis at runtime will cause network fragmentation!");
        return Err(
            "Genesis not hardcoded! Production testnet requires hardcoded genesis constants. \
             See docs/GENESIS_COORDINATION.md for proper deployment.".to_string()
        );
    }
    
    // Create the coinbase transaction with fixed parameters
    let coinbase_script = b"Genesis block for Supernova supernova-testnet".to_vec();
    let coinbase_input = TransactionInput::new_coinbase(coinbase_script);
    let genesis_output = TransactionOutput::new(TESTNET_GENESIS_REWARD, vec![]);
    let coinbase_tx = Transaction::new(2, vec![coinbase_input], vec![genesis_output], 0);
    
    // Create genesis header with hardcoded parameters
    let genesis_header = BlockHeader::new(
        TESTNET_GENESIS_VERSION,
        [0u8; 32], // prev_block_hash (all zeros for genesis)
        TESTNET_GENESIS_MERKLE_ROOT,
        TESTNET_GENESIS_TIMESTAMP,
        TESTNET_GENESIS_DIFFICULTY_BITS,
        TESTNET_GENESIS_NONCE,
    );
    
    // Create the block
    let block = Block::new(genesis_header, vec![coinbase_tx]);
    
    // Validate the hardcoded genesis block
    let actual_hash = block.header.hash();
    if actual_hash != TESTNET_GENESIS_HASH {
        return Err(format!(
            "Genesis block hash mismatch! Expected: {}, Got: {}. This indicates corrupted genesis constants.",
            hex::encode(&TESTNET_GENESIS_HASH),
            hex::encode(&actual_hash)
        ));
    }
    
    if !block.header.meets_target() {
        return Err("Genesis block does not meet target difficulty! Genesis constants are invalid.".to_string());
    }
    
    if !block.validate() {
        return Err("Genesis block validation failed! Genesis constants are invalid.".to_string());
    }
    
    tracing::info!(
        "Loaded hardcoded testnet genesis block: {}",
        hex::encode(&actual_hash[..8])
    );
    
    Ok(block)
}

/// Mine the testnet genesis block (used only for initial setup)
/// This function is called if no hardcoded genesis exists yet
fn mine_testnet_genesis() -> Result<Block, String> {
    tracing::info!("=== Mining Testnet Genesis Block ===");
    tracing::info!("Timestamp: {} (October 27, 2025 16:00:00 UTC)", TESTNET_GENESIS_TIMESTAMP);
    tracing::info!("Difficulty: 0x{:08x}", TESTNET_GENESIS_DIFFICULTY_BITS);
    
    // Create coinbase transaction
    let coinbase_script = b"Genesis block for Supernova supernova-testnet".to_vec();
    let coinbase_input = TransactionInput::new_coinbase(coinbase_script);
    let genesis_output = TransactionOutput::new(TESTNET_GENESIS_REWARD, vec![]);
    let coinbase_tx = Transaction::new(2, vec![coinbase_input], vec![genesis_output], 0);
    
    tracing::info!("Coinbase TX Hash: {}", hex::encode(&coinbase_tx.hash()));
    
    // Create genesis block header
    let genesis_header = BlockHeader::new(
        TESTNET_GENESIS_VERSION,
        [0u8; 32],
        [0u8; 32], // merkle_root will be calculated
        TESTNET_GENESIS_TIMESTAMP,
        TESTNET_GENESIS_DIFFICULTY_BITS,
        0, // nonce will be found
    );
    
    // Create the block
    let mut block = Block::new(genesis_header, vec![coinbase_tx]);
    
    // Calculate and set merkle root
    let merkle_root = block.calculate_merkle_root();
    block.header.merkle_root = merkle_root;
    
    tracing::info!("Merkle Root: {}", hex::encode(&merkle_root));
    tracing::info!("Target: {}", hex::encode(&block.header.target()));
    tracing::info!("Mining...");
    
    let start_time = std::time::Instant::now();
    let mut nonce = 0u32;
    let mut attempts = 0u64;
    
    loop {
        block.header.nonce = nonce;
        
        if block.header.meets_target() {
            let elapsed = start_time.elapsed();
            let hash = block.header.hash();
            
            tracing::info!("=== GENESIS BLOCK FOUND ===");
            tracing::info!("Nonce: {}", nonce);
            tracing::info!("Hash: {}", hex::encode(&hash));
            tracing::info!("Attempts: {}", attempts);
            tracing::info!("Time: {:.2}s", elapsed.as_secs_f64());
            
            // Print constants to hardcode
            tracing::warn!("=== COPY THESE VALUES TO node/src/blockchain/genesis.rs ===");
            tracing::warn!("pub const TESTNET_GENESIS_NONCE: u32 = {};", nonce);
            tracing::warn!("pub const TESTNET_GENESIS_MERKLE_ROOT: [u8; 32] = {};", format_array(&merkle_root));
            tracing::warn!("pub const TESTNET_GENESIS_HASH: [u8; 32] = {};", format_array(&hash));
            
            return Ok(block);
        }
        
        nonce = nonce.wrapping_add(1);
        attempts += 1;
        
        if attempts % 100_000 == 0 && attempts > 0 {
            tracing::debug!("Attempts: {} (nonce: {})", attempts, nonce);
        }
        
        if attempts > 10_000_000 {
            return Err("Genesis block mining exceeded 10M attempts. Check difficulty settings.".to_string());
        }
    }
}

fn format_array(bytes: &[u8; 32]) -> String {
    let hex_values: Vec<String> = bytes.iter()
        .map(|b| format!("0x{:02x}", b))
        .collect();
    format!("[\n    {}\n]", hex_values.chunks(8)
        .map(|chunk| chunk.join(", "))
        .collect::<Vec<_>>()
        .join(",\n    "))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_genesis_constants() {
        // Ensure timestamp is reasonable
        assert!(TESTNET_GENESIS_TIMESTAMP > 1700000000); // After Nov 2023
        assert!(TESTNET_GENESIS_TIMESTAMP < 2000000000); // Before 2033
        
        // Ensure difficulty is testnet-appropriate (easy)
        assert_eq!(TESTNET_GENESIS_DIFFICULTY_BITS, 0x207fffff);
    }
    
    #[test]
    fn test_create_genesis() {
        // Verify hardcoded genesis is set
        assert_ne!(TESTNET_GENESIS_NONCE, 0, "Genesis nonce must be hardcoded (not 0)");
        
        // Create genesis and verify it succeeds
        let genesis = create_testnet_genesis_block();
        assert!(genesis.is_ok(), "Genesis creation should succeed with hardcoded values");
        
        let block = genesis.expect("Genesis block creation failed");
        
        // Verify block structure
        assert!(block.validate(), "Genesis block should be valid");
        assert_eq!(block.header.prev_block_hash, [0u8; 32], "Genesis prev_hash must be zero");
        assert_eq!(block.transactions().len(), 1, "Genesis must have exactly 1 transaction");
        assert!(block.transactions()[0].is_coinbase(), "Genesis tx must be coinbase");
        
        // Verify hash matches hardcoded value
        let actual_hash = block.header.hash();
        assert_eq!(actual_hash, TESTNET_GENESIS_HASH, "Genesis hash must match hardcoded value");
    }
    
    #[test]
    fn test_genesis_determinism() {
        // Verify creating genesis multiple times produces identical results
        let genesis1 = create_testnet_genesis_block().expect("First genesis creation failed");
        let genesis2 = create_testnet_genesis_block().expect("Second genesis creation failed");
        
        let hash1 = genesis1.header.hash();
        let hash2 = genesis2.header.hash();
        
        assert_eq!(hash1, hash2, "Genesis block must be deterministic");
        assert_eq!(hash1, TESTNET_GENESIS_HASH, "Genesis hash must match constant");
    }
    
    #[test]
    fn test_genesis_constants_valid() {
        // Ensure all genesis constants are properly set (not default values)
        assert_ne!(TESTNET_GENESIS_NONCE, 0, "Nonce must be set");
        assert_ne!(TESTNET_GENESIS_MERKLE_ROOT, [0u8; 32], "Merkle root must be set");
        assert_ne!(TESTNET_GENESIS_HASH, [0u8; 32], "Hash must be set");
        
        // Verify timestamp is in reasonable range
        assert!(TESTNET_GENESIS_TIMESTAMP > 1700000000, "Timestamp too old");
        assert!(TESTNET_GENESIS_TIMESTAMP < 2000000000, "Timestamp too far in future");
        
        // Verify difficulty is testnet-appropriate
        assert_eq!(TESTNET_GENESIS_DIFFICULTY_BITS, 0x207fffff, "Must use testnet difficulty");
    }
}

