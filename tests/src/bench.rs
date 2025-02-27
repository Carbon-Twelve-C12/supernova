#![feature(test)]
extern crate test;

use test::Bencher;
use btclib::types::{Block, Transaction, TransactionInput, TransactionOutput};
use btclib::util::merkle::MerkleTree;
use node::mempool::{TransactionPool, MempoolConfig};
use node::storage::BlockchainDB;
use std::path::PathBuf;
use tempfile::tempdir;

#[bench]
fn bench_transaction_validation(b: &mut Bencher) {
    let temp_dir = tempdir().unwrap();
    let db = BlockchainDB::new(temp_dir.path()).unwrap();
    
    // Create a transaction for benchmarking
    let tx = create_benchmark_transaction();
    
    b.iter(|| {
        // Validate transaction (simplified for benchmark)
        tx.validate(|_, _| Some(TransactionOutput::new(100_000, vec![])))
    });
}

#[bench]
fn bench_block_validation(b: &mut Bencher) {
    let temp_dir = tempdir().unwrap();
    let db = BlockchainDB::new(temp_dir.path()).unwrap();
    
    // Create a block for benchmarking
    let block = create_benchmark_block();
    
    b.iter(|| {
        // Validate block
        block.validate()
    });
}

#[bench]
fn bench_merkle_tree_construction(b: &mut Bencher) {
    // Create transactions for the merkle tree
    let transactions = (0..1000)
        .map(|i| {
            let mut data = Vec::with_capacity(32);
            data.extend_from_slice(&i.to_le_bytes());
            data.resize(32, 0);
            data
        })
        .collect::<Vec<_>>();
    
    b.iter(|| {
        // Construct merkle tree
        let tree = MerkleTree::new(&transactions);
        let _ = tree.root_hash();
    });
}

#[bench]
fn bench_mempool_insertion(b: &mut Bencher) {
    let config = MempoolConfig::default();
    let pool = TransactionPool::new(config);
    
    // Create transactions for benchmarking
    let transactions: Vec<_> = (0..100)
        .map(|i| create_transaction_with_hash([i as u8; 32]))
        .collect();
    
    b.iter(|| {
        for tx in &transactions {
            let _ = pool.add_transaction(tx.clone(), 1);
        }
    });
}

fn create_benchmark_transaction() -> Transaction {
    let input = TransactionInput::new(
        [0u8; 32],
        0,
        vec![],
        0xffffffff,
    );
    
    let output = TransactionOutput::new(
        100_000,
        vec![1, 2, 3, 4],
    );
    
    Transaction::new(
        1,
        vec![input],
        vec![output],
        0,
    )
}

fn create_transaction_with_hash(hash: [u8; 32]) -> Transaction {
    let input = TransactionInput::new(
        hash,
        0,
        vec![],
        0xffffffff,
    );
    
    let output = TransactionOutput::new(
        100_000,
        vec![1, 2, 3, 4],
    );
    
    Transaction::new(
        1,
        vec![input],
        vec![output],
        0,
    )
}

fn create_benchmark_block() -> Block {
    let transactions = (0..100)
        .map(|_| create_benchmark_transaction())
        .collect();
    
    Block::new(
        1,
        [0u8; 32],
        transactions,
        u32::MAX / 2,
    )
}