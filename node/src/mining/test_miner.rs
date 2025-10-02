// Simple CPU Miner for Testing
// Finds valid proof-of-work for block templates

use sha2::{Digest, Sha256};
use btclib::types::block::Block;

/// Mine a block using simple CPU mining
/// Returns block with valid nonce when proof-of-work is found
pub fn mine_block_simple(mut block: Block) -> Result<Block, String> {
    let mut nonce: u32 = 0;
    let target = block.header().target();
    
    println!("Mining block at height {}...", block.height());
    println!("Target: {}", hex::encode(target));
    
    loop {
        // Set nonce
        block.header.set_nonce(nonce);
        
        // Calculate hash
        let hash = block.hash();
        
        // Check if hash meets target
        if hash_meets_target(&hash, &target) {
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

/// Check if hash meets difficulty target
fn hash_meets_target(hash: &[u8; 32], target: &[u8; 32]) -> bool {
    // Compare as big-endian numbers
    for i in (0..32).rev() {
        if hash[i] < target[i] {
            return true;
        }
        if hash[i] > target[i] {
            return false;
        }
    }
    false // Equal is also valid
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_hash_comparison() {
        let hash_low = [0u8; 32];
        let hash_high = [0xffu8; 32];
        
        assert!(hash_meets_target(&hash_low, &hash_high));
        assert!(!hash_meets_target(&hash_high, &hash_low));
    }
}

