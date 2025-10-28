//! Binary to mine the testnet genesis block and output hardcoded constants
//!
//! Run with: cargo run --bin mine_genesis

use tracing_subscriber;

fn main() {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();
    
    println!("=== Supernova Testnet Genesis Miner ===\n");
    
    // Call the genesis mining function
    match node::blockchain::genesis::create_testnet_genesis_block() {
        Ok(genesis) => {
            let hash = genesis.header.hash();
            println!("\n✓ Genesis block created successfully!");
            println!("  Hash: {}", hex::encode(&hash));
            println!("  Nonce: {}", genesis.header.nonce);
            println!("  Merkle Root: {}", hex::encode(&genesis.header.merkle_root));
            println!("\nCopy the values printed above to node/src/blockchain/genesis.rs");
        }
        Err(e) => {
            eprintln!("✗ Failed to create genesis block: {}", e);
            std::process::exit(1);
        }
    }
}

