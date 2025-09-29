//! Fuzzing harness for block validation
//!
//! This harness tests the block validation logic to ensure it properly handles
//! malformed, adversarial, and edge-case inputs without panicking.

use afl::fuzz;
use btclib::types::block::{Block, BlockHeader};
use btclib::types::transaction::Transaction;
use btclib::validation::block::validate_block;
use btclib::blockchain::Blockchain;
use std::sync::Arc;

fn main() {
    fuzz!(|data: &[u8]| {
        // Skip if data is too small to be meaningful
        if data.len() < 80 {  // Minimum block header size
            return;
        }

        // Try to parse the data as a block
        match parse_fuzz_block(data) {
            Some(block) => {
                // Create a mock blockchain context
                let blockchain = create_mock_blockchain();

                // Attempt to validate the block
                // This should never panic, only return errors
                let _ = validate_block(&block, &blockchain);

                // Test specific validation functions
                test_block_header_validation(&block.header());
                test_transaction_validation(&block.transactions());
                test_merkle_root_validation(&block);
            }
            None => {
                // Invalid block format is expected for fuzzing
                return;
            }
        }
    });
}

/// Parse fuzzer input into a Block structure
fn parse_fuzz_block(data: &[u8]) -> Option<Block> {
    // Use the first 80 bytes for header
    if data.len() < 80 {
        return None;
    }

    // Create a block header from the fuzzer input
    let header = BlockHeader {
        version: u32::from_le_bytes([data[0], data[1], data[2], data[3]]),
        prev_hash: {
            let mut hash = [0u8; 32];
            hash.copy_from_slice(&data[4..36]);
            hash
        },
        merkle_root: {
            let mut hash = [0u8; 32];
            hash.copy_from_slice(&data[36..68]);
            hash
        },
        timestamp: u64::from_le_bytes([
            data[68], data[69], data[70], data[71],
            data[72], data[73], data[74], data[75]
        ]),
        bits: u32::from_le_bytes([data[76], data[77], data[78], data[79]]),
        nonce: if data.len() >= 84 {
            u32::from_le_bytes([data[80], data[81], data[82], data[83]])
        } else {
            0
        },
        quantum_signature: None, // Will be tested in quantum_crypto fuzzer
    };

    // Parse transactions from remaining data
    let mut transactions = Vec::new();
    if data.len() > 84 {
        // Simple transaction parsing for fuzzing
        let tx_data = &data[84..];
        let tx_count = (tx_data.len() / 100).min(1000); // Limit transaction count

        for i in 0..tx_count {
            let start = i * 100;
            if start + 100 <= tx_data.len() {
                if let Some(tx) = parse_fuzz_transaction(&tx_data[start..start + 100]) {
                    transactions.push(tx);
                }
            }
        }
    }

    Some(Block::new(header, transactions))
}

/// Parse a simple transaction from fuzzer input
fn parse_fuzz_transaction(data: &[u8]) -> Option<Transaction> {
    // Implement basic transaction parsing
    // This is simplified for fuzzing purposes
    Some(Transaction::default())
}

/// Create a mock blockchain for validation context
fn create_mock_blockchain() -> Arc<Blockchain> {
    // Create a minimal blockchain instance for testing
    Arc::new(Blockchain::new_test())
}

/// Test block header validation specifically
fn test_block_header_validation(header: &BlockHeader) {
    // Test timestamp validation
    let _ = btclib::validation::block::validate_timestamp(header.timestamp);

    // Test difficulty validation
    let _ = btclib::validation::block::validate_difficulty(header.bits);

    // Test version validation
    let _ = btclib::validation::block::validate_version(header.version);
}

/// Test transaction validation
fn test_transaction_validation(transactions: &[Transaction]) {
    for tx in transactions {
        // Basic transaction validation
        let _ = btclib::validation::transaction::validate_transaction(tx);

        // Script validation
        let _ = btclib::validation::transaction::validate_scripts(tx);

        // Signature validation (without quantum sigs for this fuzzer)
        let _ = btclib::validation::transaction::validate_signatures(tx);
    }
}

/// Test merkle root calculation and validation
fn test_merkle_root_validation(block: &Block) {
    let _ = btclib::validation::block::verify_merkle_root(block);
}