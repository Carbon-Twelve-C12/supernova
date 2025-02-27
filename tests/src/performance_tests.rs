use std::time::{Duration, Instant};
use btclib::types::{Block, Transaction, TransactionInput, TransactionOutput};
use node::mempool::{TransactionPool, MempoolConfig};
use node::storage::{BlockchainDB, ChainState};
use miner::mining::{Miner, MempoolInterface};
use std::sync::Arc;
use tempfile::tempdir;
use async_trait::async_trait;
use tokio::sync::mpsc;
use tracing::{info, error, debug};

// Implement benchmark for transaction processing
#[tokio::test]
async fn benchmark_transaction_processing() -> Result<(), Box<dyn std::error::Error>> {
    // Set up logging with a more detailed format for performance analysis
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .init();
    
    // Create temporary directory for storage
    let temp_dir = tempdir()?;
    
    // Initialize database and mempool with production-like settings
    let db = Arc::new(BlockchainDB::new(temp_dir.path())?);
    let mut mempool_config = MempoolConfig::default();
    mempool_config.max_size = 10000; // Increase size for more realistic benchmarking
    let mempool = Arc::new(TransactionPool::new(mempool_config));
    
    // Create test transactions with various input and output counts
    let transaction_counts = [100, 500, 1000, 5000];
    
    for count in transaction_counts {
        debug!("Testing with {} transactions", count);
        let transactions = create_test_transactions(count);
        
        // Measure time to process transactions
        let start = Instant::now();
        
        for tx in &transactions {
            mempool.add_transaction(tx.clone(), 1 + (rand::random::<u64>() % 10))?; // Vary fee rates
        }
        
        let duration = start.elapsed();
        let transactions_per_second = count as f64 / duration.as_secs_f64();
        
        info!("Transaction processing ({} tx): {:.2} tx/sec, avg: {:.6} sec/tx", 
             count, transactions_per_second, duration.as_secs_f64() / count as f64);
        
        // Clear mempool between tests
        mempool.clear_all()?;
    }
    
    // Final benchmark with more complex transactions
    let complex_transactions = create_complex_transactions(1000);
    let start = Instant::now();
    
    for tx in &complex_transactions {
        mempool.add_transaction(tx.clone(), 1 + (rand::random::<u64>() % 10))?;
    }
    
    let duration = start.elapsed();
    let transactions_per_second = complex_transactions.len() as f64 / duration.as_secs_f64();
    
    info!("Complex transaction processing: {:.2} tx/sec, avg: {:.6} sec/tx", 
         transactions_per_second, duration.as_secs_f64() / complex_transactions.len() as f64);
    
    // Assert reasonable performance for different workloads
    assert!(transactions_per_second > 50.0, "Transaction processing should be reasonably fast");
    
    Ok(())
}

// Implement benchmark for block validation
#[tokio::test]
async fn benchmark_block_validation() -> Result<(), Box<dyn std::error::Error>> {
    // Set up logging
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .init();
    
    // Create temporary directory
    let temp_dir = tempdir()?;
    
    // Initialize database and chain state
    let db = Arc::new(BlockchainDB::new(temp_dir.path())?);
    let chain_state = ChainState::new(Arc::clone(&db))?;
    
    // Test with different block sizes
    let block_counts = [10, 50, 100];
    let tx_per_block = [10, 50, 100];
    
    for &count in &block_counts {
        for &tx_count in &tx_per_block {
            debug!("Testing {} blocks with {} transactions each", count, tx_count);
            
            // Create test blocks with specified transaction count
            let blocks = create_test_blocks(count, tx_count);
            
            // Measure time to validate blocks
            let start = Instant::now();
            
            for block in &blocks {
                chain_state.validate_block(block).await?;
            }
            
            let duration = start.elapsed();
            let blocks_per_second = count as f64 / duration.as_secs_f64();
            
            info!("Block validation ({} blocks, {} tx each): {:.2} blocks/sec, avg: {:.6} sec/block", 
                 count, tx_count, blocks_per_second, duration.as_secs_f64() / count as f64);
                 
            // Reset chain state between tests
            // Note: In a real implementation, you would need a method to reset the chain state
            // For now, we'll recreate it
            let chain_state = ChainState::new(Arc::clone(&db))?;
        }
    }
    
    // Final benchmark with a realistic blockchain sequence (increasing difficulty)
    let realistic_blocks = create_realistic_blockchain(50);
    let start = Instant::now();
    
    for block in &realistic_blocks {
        chain_state.validate_block(block).await?;
    }
    
    let duration = start.elapsed();
    let blocks_per_second = realistic_blocks.len() as f64 / duration.as_secs_f64();
    
    info!("Realistic blockchain validation: {:.2} blocks/sec, avg: {:.6} sec/block", 
         blocks_per_second, duration.as_secs_f64() / realistic_blocks.len() as f64);
    
    assert!(blocks_per_second > 5.0, "Block validation should be reasonably fast");
    
    Ok(())
}

// Helper to create test transactions with simple structure
fn create_test_transactions(count: usize) -> Vec<Transaction> {
    let mut transactions = Vec::with_capacity(count);
    
    for i in 0..count {
        let mut prev_hash = [0u8; 32];
        prev_hash[0] = (i >> 24) as u8;
        prev_hash[1] = (i >> 16) as u8;
        prev_hash[2] = (i >> 8) as u8;
        prev_hash[3] = i as u8;
        
        let input = TransactionInput::new(
            prev_hash,
            0,
            vec![],
            0xffffffff,
        );
        
        let output = TransactionOutput::new(
            1000,
            vec![1, 2, 3, 4],
        );
        
        let transaction = Transaction::new(
            1,
            vec![input],
            vec![output],
            0,
        );
        
        transactions.push(transaction);
    }
    
    transactions
}

// Helper to create more complex transactions with multiple inputs and outputs
fn create_complex_transactions(count: usize) -> Vec<Transaction> {
    let mut transactions = Vec::with_capacity(count);
    
    for i in 0..count {
        let mut prev_hash = [0u8; 32];
        prev_hash[0] = (i >> 24) as u8;
        prev_hash[1] = (i >> 16) as u8;
        prev_hash[2] = (i >> 8) as u8;
        prev_hash[3] = i as u8;
        
        // Create 1-3 inputs
        let input_count = 1 + (i % 3);
        let mut inputs = Vec::with_capacity(input_count);
        
        for j in 0..input_count {
            let mut input_hash = prev_hash;
            input_hash[4] = j as u8;
            
            // Create more realistic signature script (20-50 bytes)
            let sig_size = 20 + (i % 31);
            let mut sig_script = Vec::with_capacity(sig_size);
            for k in 0..sig_size {
                sig_script.push(((i + j + k) % 256) as u8);
            }
            
            inputs.push(TransactionInput::new(
                input_hash,
                j as u32,
                sig_script,
                0xffffffff,
            ));
        }
        
        // Create 1-5 outputs
        let output_count = 1 + (i % 5);
        let mut outputs = Vec::with_capacity(output_count);
        
        for j in 0..output_count {
            // Create varied pubkey scripts (20-100 bytes)
            let script_size = 20 + (i % 81);
            let mut pub_key_script = Vec::with_capacity(script_size);
            for k in 0..script_size {
                pub_key_script.push(((i + j + k) % 256) as u8);
            }
            
            outputs.push(TransactionOutput::new(
                500 + ((i * j) % 1000) as u64,
                pub_key_script,
            ));
        }
        
        let transaction = Transaction::new(
            1,
            inputs,
            outputs,
            (i % 1000) as u32, // Add some locktime variety
        );
        
        transactions.push(transaction);
    }
    
    transactions
}

// Helper to create test blocks with specified transaction count
fn create_test_blocks(count: usize, tx_per_block: usize) -> Vec<Block> {
    let mut blocks = Vec::with_capacity(count);
    let mut prev_hash = [0u8; 32];
    
    for i in 0..count {
        let block = Block::new(
            1,
            prev_hash,
            create_test_transactions(tx_per_block),
            u32::MAX / 2,
        );
        
        prev_hash = block.hash();
        blocks.push(block);
    }
    
    blocks
}

// Helper to create a more realistic blockchain with difficulty adjustments
fn create_realistic_blockchain(count: usize) -> Vec<Block> {
    let mut blocks = Vec::with_capacity(count);
    let mut prev_hash = [0u8; 32];
    let mut target = u32::MAX / 2;
    
    for i in 0..count {
        // Create blocks with varied transaction counts
        let tx_count = 5 + (i % 20);
        
        // Adjust difficulty every 10 blocks
        if i % 10 == 0 && i > 0 {
            target = (target as f64 * 0.9) as u32; // Make it 10% harder
        }
        
        let block = Block::new(
            1,
            prev_hash,
            create_test_transactions(tx_count),
            target,
        );
        
        prev_hash = block.hash();
        blocks.push(block);
    }
    
    blocks
}

// Implement benchmark for mining performance
#[tokio::test]
async fn benchmark_mining_performance() -> Result<(), Box<dyn std::error::Error>> {
    // Set up logging
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();
    
    struct MockMempool;
    
    #[async_trait]
    impl MempoolInterface for MockMempool {
        async fn get_transactions(&self, max_size: usize) -> Vec<Transaction> {
            // Return different numbers of transactions based on requested size
            let tx_count = std::cmp::min(max_size / 1000, 200); // Approximate transaction size
            create_test_transactions(tx_count)
        }
    }
    
    // Test mining performance with different thread counts
    let thread_counts = [1, 2, 4, 8];
    
    for &threads in &thread_counts {
        info!("Testing mining performance with {} threads", threads);
        
        // Initialize miner with specified thread count
        let mempool = Arc::new(MockMempool);
        let reward_address = vec![1, 2, 3, 4];
        let (miner, mut block_rx) = miner::mining::Miner::new(
            threads,
            u32::MAX / 1000, // Set moderate difficulty
            Arc::clone(&mempool),
            reward_address.clone(),
        );
        
        // Start mining
        let mining_handle = tokio::spawn(async move {
            if let Err(e) = miner.start_mining(1, [0u8; 32], 0).await {
                error!("Mining error: {}", e);
            }
        });
        
        // Count blocks mined in a fixed time period
        let start = Instant::now();
        let duration = Duration::from_secs(15); // Reduced duration for testing
        let mut blocks_mined = 0;
        
        while start.elapsed() < duration {
            if let Some(_) = tokio::time::timeout(Duration::from_secs(1), block_rx.recv()).await? {
                blocks_mined += 1;
            }
        }
        
        // Calculate mining rate
        let blocks_per_second = blocks_mined as f64 / duration.as_secs_f64();
        let hashes_per_second = blocks_per_second * (u32::MAX / 1000) as f64;
        
        info!("Mining performance ({} threads): {:.2} blocks/sec, approx {:.2} hashes/sec", 
             threads, blocks_per_second, hashes_per_second);
        
        // Clean up
        mining_handle.abort();
        tokio::time::sleep(Duration::from_millis(100)).await; // Allow cleanup
    }
    
    // Final test with realistic difficulty
    let mempool = Arc::new(MockMempool);
    let reward_address = vec![1, 2, 3, 4];
    let (miner, mut block_rx) = miner::mining::Miner::new(
        4,
        u32::MAX / 100000, // Higher difficulty
        Arc::clone(&mempool),
        reward_address.clone(),
    );
    
    let mining_handle = tokio::spawn(async move {
        if let Err(e) = miner.start_mining(1, [0u8; 32], 0).await {
            error!("Mining error: {}", e);
        }
    });
    
    // Measure time to find a block
    let start = Instant::now();
    let timeout_duration = Duration::from_secs(60);
    
    match tokio::time::timeout(timeout_duration, block_rx.recv()).await {
        Ok(Some(_)) => {
            let time_to_block = start.elapsed();
            info!("Time to mine block at higher difficulty: {:.2} seconds", time_to_block.as_secs_f64());
        },
        Ok(None) => {
            error!("Mining channel closed unexpectedly");
        },
        Err(_) => {
            info!("No block mined within timeout period, mining rate < 1/{} blocks/sec", 
                 timeout_duration.as_secs());
        }
    }
    
    // Clean up
    mining_handle.abort();
    
    Ok(())
}

// Add benchmark for mempool management
#[tokio::test]
async fn benchmark_mempool_management() -> Result<(), Box<dyn std::error::Error>> {
    // Set up logging
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();
    
    // Create mempool
    let mut mempool_config = MempoolConfig::default();
    mempool_config.max_size = 10000;
    let mempool = Arc::new(TransactionPool::new(mempool_config));
    
    // Benchmark adding transactions
    let transactions = create_test_transactions(5000);
    
    let start = Instant::now();
    for tx in &transactions {
        mempool.add_transaction(tx.clone(), 1)?;
    }
    let add_duration = start.elapsed();
    
    info!("Mempool add performance: {:.2} tx/sec", 
         transactions.len() as f64 / add_duration.as_secs_f64());
    
    // Benchmark retrieving transactions by fee
    let start = Instant::now();
    let sorted_txs = mempool.get_sorted_transactions();
    let retrieve_duration = start.elapsed();
    
    info!("Mempool retrieval performance: {:.6} seconds for {} transactions", 
         retrieve_duration.as_secs_f64(), sorted_txs.len());
    
    // Benchmark removing transactions
    let txs_to_remove = &transactions[0..1000];
    let start = Instant::now();
    for tx in txs_to_remove {
        mempool.remove_transaction(&tx.hash());
    }
    let remove_duration = start.elapsed();
    
    info!("Mempool remove performance: {:.2} tx/sec", 
         txs_to_remove.len() as f64 / remove_duration.as_secs_f64());
    
    Ok(())
}