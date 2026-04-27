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

/// Pre-mined nonce.
///
/// Becomes IMMUTABLE for the deployed testnet at launch. The current
/// values were re-mined on 2026-04-27 because the previous constants
/// (October 2025) were anchored against an older `Transaction::hash`
/// shape and the hash check failed during build. Regenerate via the
/// `diagnose_genesis_values` `#[ignore]`'d test in this module's
/// `mod tests` block whenever the coinbase transaction shape or the
/// `Transaction::hash` / `BlockHeader::hash` codec changes.
pub const TESTNET_GENESIS_NONCE: u32 = 3;

/// Pre-calculated merkle root.
///
/// `Block::calculate_merkle_root()` of a block containing the single
/// genesis coinbase tx — note that `MerkleTree::new` SHA-256-hashes
/// its inputs, so this is `SHA256(tx.hash())` for a single-tx block,
/// not `tx.hash()` itself. The diagnostic helper uses the canonical
/// path so this constant always matches what consensus computes.
/// Hex: 3671b404d424fcbf2a7639a2c46811299c8c79b41600418eb3ed5c3bafe1306d
pub const TESTNET_GENESIS_MERKLE_ROOT: [u8; 32] = [
    0x36, 0x71, 0xb4, 0x04, 0xd4, 0x24, 0xfc, 0xbf,
    0x2a, 0x76, 0x39, 0xa2, 0xc4, 0x68, 0x11, 0x29,
    0x9c, 0x8c, 0x79, 0xb4, 0x16, 0x00, 0x41, 0x8e,
    0xb3, 0xed, 0x5c, 0x3b, 0xaf, 0xe1, 0x30, 0x6d
];

/// Expected genesis block hash.
///
/// `BlockHeader::hash()` of the genesis header constructed with the
/// constants above. Meets the testnet difficulty target
/// (`TESTNET_GENESIS_DIFFICULTY_BITS = 0x207fffff`) at nonce 3.
/// Hex: 2e716d6ba655a62d1c3deea98f965ae29f00b90793b71a3eb18b11832be8ad54
pub const TESTNET_GENESIS_HASH: [u8; 32] = [
    0x2e, 0x71, 0x6d, 0x6b, 0xa6, 0x55, 0xa6, 0x2d,
    0x1c, 0x3d, 0xee, 0xa9, 0x8f, 0x96, 0x5a, 0xe2,
    0x9f, 0x00, 0xb9, 0x07, 0x93, 0xb7, 0x1a, 0x3e,
    0xb1, 0x8b, 0x11, 0x83, 0x2b, 0xe8, 0xad, 0x54
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
        return Err(format!(
            "Genesis block validation failed (pow={}, merkle={}, txs={}, computed_merkle={}, header_merkle={})",
            block.verify_proof_of_work(),
            block.verify_merkle_root(),
            block.validate_transactions(),
            hex::encode(block.calculate_merkle_root()),
            hex::encode(block.header.merkle_root)
        ));
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

    /// Diagnostic helper: re-mines the genesis block, finding a nonce
    /// that satisfies the testnet difficulty target. Prints the values
    /// that `TESTNET_GENESIS_NONCE`, `TESTNET_GENESIS_MERKLE_ROOT`, and
    /// `TESTNET_GENESIS_HASH` should be set to. Run with
    /// `cargo test diagnose_genesis_values -- --nocapture` after any
    /// change to `Transaction::hash` or the coinbase shape; copy the
    /// output back into the constants above.
    ///
    /// `#[ignore]` so it doesn't run on every test invocation — it's a
    /// developer tool, not a regression check.
    #[test]
    #[ignore = "developer tool: re-mines genesis. Run explicitly when constants need regen."]
    fn diagnose_genesis_values() {
        let coinbase_script = b"Genesis block for Supernova supernova-testnet".to_vec();
        let coinbase_input = TransactionInput::new_coinbase(coinbase_script);
        let genesis_output = TransactionOutput::new(TESTNET_GENESIS_REWARD, vec![]);
        let coinbase_tx = Transaction::new(2, vec![coinbase_input], vec![genesis_output], 0);

        // Use the canonical merkle-root path so the constant we print
        // matches what `Block::calculate_merkle_root()` will compute
        // at validation time. `MerkleTree::new` SHA-256-hashes its
        // inputs (so the single-tx root is `SHA256(tx.hash())`, not
        // `tx.hash()` directly) — easy to get wrong without going
        // through the canonical helper.
        let placeholder_header = BlockHeader::new(
            TESTNET_GENESIS_VERSION,
            [0u8; 32],
            [0u8; 32],
            TESTNET_GENESIS_TIMESTAMP,
            TESTNET_GENESIS_DIFFICULTY_BITS,
            0,
        );
        let placeholder_block = Block::new(placeholder_header, vec![coinbase_tx.clone()]);
        let actual_merkle = placeholder_block.calculate_merkle_root();

        // Mine the genesis: iterate nonce until the header hash meets
        // the difficulty target. Testnet difficulty 0x207fffff is
        // trivially easy and a valid nonce appears within the first
        // few thousand attempts.
        let mut mined: Option<(u32, [u8; 32])> = None;
        for nonce in 0u32..u32::MAX {
            let header = BlockHeader::new(
                TESTNET_GENESIS_VERSION,
                [0u8; 32],
                actual_merkle,
                TESTNET_GENESIS_TIMESTAMP,
                TESTNET_GENESIS_DIFFICULTY_BITS,
                nonce,
            );
            if header.meets_target() {
                mined = Some((nonce, header.hash()));
                break;
            }
        }

        let (nonce, hash) = mined.expect("testnet difficulty is easy; a nonce must exist");

        println!("=== Genesis diagnostic values ===");
        println!("TESTNET_GENESIS_NONCE       = {};", nonce);
        println!("TESTNET_GENESIS_MERKLE_ROOT = {:?};", actual_merkle);
        println!("TESTNET_GENESIS_HASH        = {:?};", hash);
        println!("merkle (hex): {}", hex::encode(actual_merkle));
        println!("hash   (hex): {}", hex::encode(hash));
        println!("nonce: {}", nonce);
    }
    
    #[test]
    fn test_create_genesis() {
        // Verify hardcoded genesis is set
        assert_ne!(TESTNET_GENESIS_NONCE, 0, "Genesis nonce must be hardcoded (not 0)");
        
        // Create genesis and verify it succeeds
        let genesis = create_testnet_genesis_block();
        assert!(
            genesis.is_ok(),
            "Genesis creation should succeed: {:?}",
            genesis.as_ref().err()
        );
        
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

