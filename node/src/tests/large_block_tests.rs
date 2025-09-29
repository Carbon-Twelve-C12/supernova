// Large block handling tests
//
// This file contains tests for handling blocks with many transactions or large transactions,
// stressing memory management, validation performance, and database interaction.

use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::time::sleep;
use tracing::{debug, info, warn};

use crate::chain::{Block, ChainState, Transaction, TransactionInput, TransactionOutput};
use crate::storage::BlockchainDB;
use crate::mining::BlockTemplate;
use crate::mempool::Mempool;
use crate::validation::TransactionValidator;
use crate::types::{Hash, Amount};

// Helper function to create a test transaction with random inputs and outputs
fn create_test_transaction(input_count: usize, output_count: usize, input_value: Amount) -> Transaction {
    let mut inputs = Vec::with_capacity(input_count);
    let mut outputs = Vec::with_capacity(output_count);

    // Create inputs
    for i in 0..input_count {
        let outpoint = rand_outpoint();
        inputs.push(TransactionInput::new(outpoint, vec![i as u8], 0));
    }

    // Calculate output value (input_value minus fee)
    let fee = input_value / 100; // 1% fee
    let output_value = (input_value - fee) / output_count as u64;

    // Create outputs
    for _ in 0..output_count {
        outputs.push(TransactionOutput::new(output_value, rand_pubkey_script()));
    }

    Transaction::new(1, inputs, outputs, 0)
}

// Helper to create random outpoint
fn rand_outpoint() -> (Hash, u32) {
    let mut hash = [0u8; 32];
    for i in 0..32 {
        hash[i] = rand::random();
    }
    (hash, rand::random())
}

// Helper to create random pubkey script
fn rand_pubkey_script() -> Vec<u8> {
    let mut script = Vec::with_capacity(25);
    for _ in 0..25 {
        script.push(rand::random());
    }
    script
}

// Helper to create a block with many transactions
async fn create_large_block(transaction_count: usize) -> Block {
    let mut transactions = Vec::with_capacity(transaction_count);

    // Create coinbase transaction
    let coinbase = Transaction::new(
        1,
        vec![],
        vec![TransactionOutput::new(5000000000, vec![0xAA])],
        0
    );
    transactions.push(coinbase);

    // Create regular transactions
    for _ in 0..(transaction_count - 1) {
        let tx = create_test_transaction(2, 2, 100000000);
        transactions.push(tx);
    }

    // Create block template
    let mut template = BlockTemplate::new(
        1,
        [0u8; 32],
        transactions
    );

    // Generate a block
    template.mine_block(1000).await
}

// Test validation of blocks with many transactions
#[tokio::test]
async fn test_large_block_validation() {
    // Create test chain state
    let db = Arc::new(BlockchainDB::create_in_memory().unwrap());
    let chain_state = ChainState::new(Arc::clone(&db)).unwrap();

    // Create validator
    let validator = TransactionValidator::new();

    // Transaction count sizes to test
    let tx_counts = [100, 500, 1000, 2000, 4000];

    for &count in &tx_counts {
        info!("Testing block with {} transactions", count);

        // Create large block
        let start_time = Instant::now();
        let block = create_large_block(count).await;
        let creation_time = start_time.elapsed();

        info!("Block creation took {:?}", creation_time);

        // Validate block
        let start_time = Instant::now();
        let is_valid = validator.validate_block(&block, &chain_state).await;
        let validation_time = start_time.elapsed();

        info!("Block validation took {:?}", validation_time);
        assert!(is_valid, "Block with {} transactions should be valid", count);

        // Check validation time is reasonable
        let max_expected_ms = count as u64 * 2; // ~2ms per transaction is reasonable
        assert!(
            validation_time.as_millis() < max_expected_ms as u128,
            "Validation time for {} transactions was {:?}, exceeding threshold of {}ms",
            count, validation_time, max_expected_ms
        );
    }
}

// Test processing blocks with many transactions
#[tokio::test]
async fn test_large_block_processing() {
    // Create test chain state
    let db = Arc::new(BlockchainDB::create_in_memory().unwrap());
    let mut chain_state = ChainState::new(Arc::clone(&db)).unwrap();

    // Create initial blockchain
    let genesis_block = create_large_block(1).await;
    chain_state.process_block(genesis_block).await.unwrap();

    // Process a series of blocks with increasing transaction counts
    let tx_counts = [50, 100, 200, 500];

    for &count in &tx_counts {
        info!("Processing block with {} transactions", count);

        // Create large block
        let block = create_large_block(count).await;

        // Process block
        let start_time = Instant::now();
        let result = chain_state.process_block(block.clone()).await;
        let processing_time = start_time.elapsed();

        assert!(result.is_ok(), "Failed to process block with {} transactions: {:?}", count, result);

        info!("Block processing took {:?}", processing_time);

        // Check processing time is reasonable
        let max_expected_ms = count as u64 * 5; // ~5ms per transaction is reasonable for full processing
        assert!(
            processing_time.as_millis() < max_expected_ms as u128,
            "Processing time for {} transactions was {:?}, exceeding threshold of {}ms",
            count, processing_time, max_expected_ms
        );

        // Add a short delay between blocks
        sleep(Duration::from_millis(100)).await;
    }
}

// Test memory usage with large blocks
#[tokio::test]
async fn test_large_block_memory_usage() {
    // This test monitors memory usage while validating and processing large blocks
    // to ensure memory usage stays within reasonable bounds.

    // Create test chain state
    let db = Arc::new(BlockchainDB::create_in_memory().unwrap());
    let mut chain_state = ChainState::new(Arc::clone(&db)).unwrap();

    // Create mempool for tracking memory usage
    let mempool = Mempool::new(
        1024 * 1024 * 1024, // 1GB max size
        1000,               // 1000 sat/byte min fee rate
        5000                // 5000 max tx per block
    );

    // Record baseline memory usage
    let baseline_memory = current_memory_usage();
    info!("Baseline memory usage: {}MB", baseline_memory / (1024 * 1024));

    // Test with a very large block (10,000 transactions)
    info!("Creating block with 10,000 transactions");
    let large_block = create_large_block(10000).await;

    // Monitor memory during validation
    info!("Validating large block");
    let validator = TransactionValidator::new();
    let _ = validator.validate_block(&large_block, &chain_state).await;

    // Check memory after validation
    let validation_memory = current_memory_usage();
    info!("Memory usage after validation: {}MB", validation_memory / (1024 * 1024));

    // Memory should not grow excessively
    let max_expected_memory_growth = 500 * 1024 * 1024; // 500MB max growth
    assert!(
        validation_memory - baseline_memory < max_expected_memory_growth,
        "Memory usage grew by {}MB, exceeding threshold of {}MB",
        (validation_memory - baseline_memory) / (1024 * 1024),
        max_expected_memory_growth / (1024 * 1024)
    );

    // Process the block
    info!("Processing large block");
    let _ = chain_state.process_block(large_block).await;

    // Check memory after processing
    let processing_memory = current_memory_usage();
    info!("Memory usage after processing: {}MB", processing_memory / (1024 * 1024));

    // Memory should stay stable after processing
    assert!(
        processing_memory - baseline_memory < max_expected_memory_growth,
        "Memory usage grew by {}MB, exceeding threshold of {}MB",
        (processing_memory - baseline_memory) / (1024 * 1024),
        max_expected_memory_growth / (1024 * 1024)
    );

    // Force garbage collection and check memory again
    drop(large_block);
    sleep(Duration::from_millis(500)).await; // Give time for memory to be reclaimed

    let final_memory = current_memory_usage();
    info!("Final memory usage: {}MB", final_memory / (1024 * 1024));

    // Memory should return closer to baseline after GC
    assert!(
        final_memory - baseline_memory < max_expected_memory_growth / 2,
        "Memory usage remained high at {}MB above baseline",
        (final_memory - baseline_memory) / (1024 * 1024)
    );
}

// Helper function to get current memory usage
fn current_memory_usage() -> usize {
    // This is a platform-specific implementation
    // On Linux, you can read from /proc/self/statm
    // On other platforms, you might need to use platform-specific APIs

    #[cfg(target_os = "linux")]
    {
        use std::fs::File;
        use std::io::Read;

        let mut buffer = String::new();
        File::open("/proc/self/statm")
            .and_then(|mut file| file.read_to_string(&mut buffer))
            .expect("Failed to read memory usage");

        let values: Vec<&str> = buffer.split_whitespace().collect();
        let resident_set_size = values[1].parse::<usize>().unwrap() * 4096; // Page size is typically 4KB

        resident_set_size
    }

    #[cfg(not(target_os = "linux"))]
    {
        // Default fallback - not accurate but prevents compilation errors
        // In a real implementation, you would use platform-specific APIs
        // such as GetProcessMemoryInfo on Windows
        0
    }
}

// Test malformed blocks
#[tokio::test]
async fn test_malformed_blocks() {
    // Create test chain state
    let db = Arc::new(BlockchainDB::create_in_memory().unwrap());
    let mut chain_state = ChainState::new(Arc::clone(&db)).unwrap();

    // Create validator
    let validator = TransactionValidator::new();

    // Case 1: Block with duplicate transactions
    info!("Testing block with duplicate transactions");
    let mut block = create_large_block(10).await;
    let duplicate_tx = block.transactions()[1].clone();
    block.transactions_mut().push(duplicate_tx);

    let is_valid = validator.validate_block(&block, &chain_state).await;
    assert!(!is_valid, "Block with duplicate transactions should be invalid");

    // Case 2: Block with invalid merkle root
    info!("Testing block with invalid merkle root");
    let mut block = create_large_block(10).await;
    let mut header = block.header().clone();
    let mut root = header.merkle_root();
    // Modify one byte of the merkle root
    root[0] = !root[0];
    header.set_merkle_root(root);
    block.set_header(header);

    let is_valid = validator.validate_block(&block, &chain_state).await;
    assert!(!is_valid, "Block with invalid merkle root should be invalid");

    // Case 3: Block with no coinbase transaction
    info!("Testing block with no coinbase transaction");
    let regular_tx = create_test_transaction(2, 2, 100000000);
    let mut invalid_txs = vec![regular_tx.clone(), regular_tx];

    let mut template = BlockTemplate::new(
        1,
        [0u8; 32],
        invalid_txs
    );

    let invalid_block = template.mine_block(1000).await;

    let is_valid = validator.validate_block(&invalid_block, &chain_state).await;
    assert!(!is_valid, "Block with no coinbase transaction should be invalid");

    // Case 4: Block with coinbase in wrong position
    info!("Testing block with misplaced coinbase");
    let coinbase = Transaction::new(
        1,
        vec![],
        vec![TransactionOutput::new(5000000000, vec![0xAA])],
        0
    );

    invalid_txs = vec![regular_tx.clone(), coinbase, regular_tx];

    template = BlockTemplate::new(
        1,
        [0u8; 32],
        invalid_txs
    );

    let invalid_block = template.mine_block(1000).await;

    let is_valid = validator.validate_block(&invalid_block, &chain_state).await;
    assert!(!is_valid, "Block with misplaced coinbase should be invalid");
}

// Test block with very large transactions
#[tokio::test]
async fn test_very_large_transactions() {
    // Create test chain state
    let db = Arc::new(BlockchainDB::create_in_memory().unwrap());
    let mut chain_state = ChainState::new(Arc::clone(&db)).unwrap();

    // Create validator
    let validator = TransactionValidator::new();

    // Create a transaction with many inputs and outputs
    let large_tx = create_test_transaction(1000, 1000, 100000000000);

    // Create coinbase transaction
    let coinbase = Transaction::new(
        1,
        vec![],
        vec![TransactionOutput::new(5000000000, vec![0xAA])],
        0
    );

    let transactions = vec![coinbase, large_tx];

    // Create block template
    let mut template = BlockTemplate::new(
        1,
        [0u8; 32],
        transactions
    );

    // Generate a block
    let block = template.mine_block(1000).await;

    // Validate block
    let start_time = Instant::now();
    let is_valid = validator.validate_block(&block, &chain_state).await;
    let validation_time = start_time.elapsed();

    info!("Validation of block with very large transaction took {:?}", validation_time);
    assert!(is_valid, "Block with large transaction should be valid");

    // Process block
    let start_time = Instant::now();
    let result = chain_state.process_block(block).await;
    let processing_time = start_time.elapsed();

    info!("Processing of block with very large transaction took {:?}", processing_time);
    assert!(result.is_ok(), "Failed to process block with large transaction: {:?}", result);
}