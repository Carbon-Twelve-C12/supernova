// Simple CPU Miner for Testing
// Finds valid proof-of-work for block templates

use supernova_core::types::block::Block;

/// Mine a block using simple CPU mining
/// Returns block with valid nonce when proof-of-work is found
pub fn mine_block_simple(mut block: Block) -> Result<Block, String> {
    let mut nonce: u32 = 0;
    
    println!("Mining block at height {}...", block.height());
    
    loop {
        // Set nonce
        block.header.set_nonce(nonce);
        
        // Use BlockHeader::meets_target() which has correct big-endian comparison
        if block.header().meets_target() {
            let hash = block.hash();
            println!("✓ Found valid block!");
            println!("  Nonce: {}", nonce);
            println!("  Hash: {}", hex::encode(hash));
            return Ok(block);
        }
        
        nonce = nonce.wrapping_add(1);
        
        // Progress indicator
        if nonce % 100_000 == 0 {
            println!("  Tried {} nonces...", nonce);
        }
        
        // Safety limit for testing
        if nonce == 0 {
            return Err("Mining exhausted all nonces".to_string());
        }
    }
}
