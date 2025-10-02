// Merkle Tree Implementation for Supernova Blockchain
// Uses SHA3-512 for quantum resistance (NOT SHA-256 like Bitcoin)

use sha3::{Digest, Sha3_512};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum MerkleError {
    #[error("Empty transaction list")]
    EmptyTransactions,
    
    #[error("Invalid transaction ID length: expected 32 bytes, got {0}")]
    InvalidTxidLength(usize),
}

/// Calculate merkle root from transaction IDs
/// 
/// Uses SHA3-512 (then truncated to 32 bytes) for quantum resistance.
/// Follows Bitcoin's merkle tree algorithm:
/// - If 1 tx: return that txid
/// - If even number: pair up and hash
/// - If odd number: duplicate last tx and proceed
///
/// # Arguments
/// * `txids` - Slice of 32-byte transaction IDs
///
/// # Returns
/// * 32-byte merkle root
///
/// # Examples
/// ```
/// let txids = vec![[1u8; 32], [2u8; 32]];
/// let root = calculate_merkle_root(&txids).unwrap();
/// assert_eq!(root.len(), 32);
/// ```
pub fn calculate_merkle_root(txids: &[[u8; 32]]) -> Result<[u8; 32], MerkleError> {
    if txids.is_empty() {
        return Err(MerkleError::EmptyTransactions);
    }
    
    // Single transaction case
    if txids.len() == 1 {
        return Ok(txids[0]);
    }
    
    // Build merkle tree level by level
    let mut current_level: Vec<[u8; 32]> = txids.to_vec();
    
    while current_level.len() > 1 {
        let mut next_level = Vec::new();
        
        // Process pairs
        let mut i = 0;
        while i < current_level.len() {
            let left = current_level[i];
            
            // If odd number of elements, duplicate the last one
            let right = if i + 1 < current_level.len() {
                current_level[i + 1]
            } else {
                current_level[i]
            };
            
            // Hash the pair using SHA3-512
            let combined_hash = hash_pair(&left, &right);
            next_level.push(combined_hash);
            
            i += 2;
        }
        
        current_level = next_level;
    }
    
    Ok(current_level[0])
}

/// Hash a pair of hashes to create parent node
fn hash_pair(left: &[u8; 32], right: &[u8; 32]) -> [u8; 32] {
    let mut hasher = Sha3_512::new();
    hasher.update(left);
    hasher.update(right);
    let result = hasher.finalize();
    
    // Take first 32 bytes of 64-byte SHA3-512 output
    let mut output = [0u8; 32];
    output.copy_from_slice(&result[..32]);
    output
}

/// Build full merkle tree and return all levels (for debugging/proof generation)
pub fn build_merkle_tree(txids: &[[u8; 32]]) -> Result<Vec<Vec<[u8; 32]>>, MerkleError> {
    if txids.is_empty() {
        return Err(MerkleError::EmptyTransactions);
    }
    
    let mut tree = vec![txids.to_vec()];
    let mut current_level = txids.to_vec();
    
    while current_level.len() > 1 {
        let mut next_level = Vec::new();
        
        let mut i = 0;
        while i < current_level.len() {
            let left = current_level[i];
            let right = if i + 1 < current_level.len() {
                current_level[i + 1]
            } else {
                current_level[i]
            };
            
            next_level.push(hash_pair(&left, &right));
            i += 2;
        }
        
        tree.push(next_level.clone());
        current_level = next_level;
    }
    
    Ok(tree)
}

/// Generate merkle proof for a transaction at given index
pub fn generate_merkle_proof(
    txids: &[[u8; 32]],
    tx_index: usize,
) -> Result<Vec<[u8; 32]>, MerkleError> {
    if txids.is_empty() {
        return Err(MerkleError::EmptyTransactions);
    }
    
    if tx_index >= txids.len() {
        return Err(MerkleError::InvalidTxidLength(tx_index));
    }
    
    let tree = build_merkle_tree(txids)?;
    let mut proof = Vec::new();
    let mut index = tx_index;
    
    // For each level (except root), add the sibling hash
    for level in &tree[..tree.len() - 1] {
        let sibling_index = if index % 2 == 0 {
            index + 1
        } else {
            index - 1
        };
        
        if sibling_index < level.len() {
            proof.push(level[sibling_index]);
        } else {
            // Odd number case - duplicate last element
            proof.push(level[index]);
        }
        
        index /= 2;
    }
    
    Ok(proof)
}

/// Verify merkle proof
pub fn verify_merkle_proof(
    tx_hash: &[u8; 32],
    proof: &[[u8; 32]],
    merkle_root: &[u8; 32],
    tx_index: usize,
) -> bool {
    let mut computed_hash = *tx_hash;
    let mut index = tx_index;
    
    for sibling in proof {
        computed_hash = if index % 2 == 0 {
            hash_pair(&computed_hash, sibling)
        } else {
            hash_pair(sibling, &computed_hash)
        };
        
        index /= 2;
    }
    
    &computed_hash == merkle_root
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_single_transaction() {
        let txid = [42u8; 32];
        let root = calculate_merkle_root(&[txid]).unwrap();
        
        // Single transaction merkle root is the transaction itself
        assert_eq!(root, txid);
    }
    
    #[test]
    fn test_two_transactions() {
        let tx1 = [1u8; 32];
        let tx2 = [2u8; 32];
        
        let root = calculate_merkle_root(&[tx1, tx2]).unwrap();
        
        // Root should be hash of tx1 + tx2
        let expected = hash_pair(&tx1, &tx2);
        assert_eq!(root, expected);
    }
    
    #[test]
    fn test_three_transactions() {
        let tx1 = [1u8; 32];
        let tx2 = [2u8; 32];
        let tx3 = [3u8; 32];
        
        let root = calculate_merkle_root(&[tx1, tx2, tx3]).unwrap();
        
        // Should duplicate tx3 for pairing
        let left = hash_pair(&tx1, &tx2);
        let right = hash_pair(&tx3, &tx3);
        let expected = hash_pair(&left, &right);
        
        assert_eq!(root, expected);
    }
    
    #[test]
    fn test_power_of_two_transactions() {
        let txids: Vec<[u8; 32]> = (0..4).map(|i| {
            let mut txid = [0u8; 32];
            txid[0] = i as u8;
            txid
        }).collect();
        
        let root = calculate_merkle_root(&txids).unwrap();
        
        // Verify structure
        let pair1 = hash_pair(&txids[0], &txids[1]);
        let pair2 = hash_pair(&txids[2], &txids[3]);
        let expected = hash_pair(&pair1, &pair2);
        
        assert_eq!(root, expected);
    }
    
    #[test]
    fn test_empty_transactions() {
        let result = calculate_merkle_root(&[]);
        assert!(result.is_err());
        assert!(matches!(result, Err(MerkleError::EmptyTransactions)));
    }
    
    #[test]
    fn test_merkle_proof_generation() {
        let txids: Vec<[u8; 32]> = (0..4).map(|i| {
            let mut txid = [0u8; 32];
            txid[0] = i as u8;
            txid
        }).collect();
        
        let root = calculate_merkle_root(&txids).unwrap();
        
        // Generate proof for tx at index 1
        let proof = generate_merkle_proof(&txids, 1).unwrap();
        
        // Verify the proof
        assert!(verify_merkle_proof(&txids[1], &proof, &root, 1));
    }
    
    #[test]
    fn test_merkle_proof_verification_fails_wrong_tx() {
        let txids: Vec<[u8; 32]> = (0..4).map(|i| {
            let mut txid = [0u8; 32];
            txid[0] = i as u8;
            txid
        }).collect();
        
        let root = calculate_merkle_root(&txids).unwrap();
        let proof = generate_merkle_proof(&txids, 1).unwrap();
        
        // Wrong transaction should fail verification
        let wrong_tx = [99u8; 32];
        assert!(!verify_merkle_proof(&wrong_tx, &proof, &root, 1));
    }
    
    #[test]
    fn test_deterministic_root() {
        let txids: Vec<[u8; 32]> = (0..7).map(|i| {
            let mut txid = [0u8; 32];
            txid[0] = i as u8;
            txid
        }).collect();
        
        // Calculate root multiple times
        let root1 = calculate_merkle_root(&txids).unwrap();
        let root2 = calculate_merkle_root(&txids).unwrap();
        let root3 = calculate_merkle_root(&txids).unwrap();
        
        // Should be deterministic
        assert_eq!(root1, root2);
        assert_eq!(root2, root3);
    }
}

