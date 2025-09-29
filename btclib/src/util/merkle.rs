//! Merkle Tree implementation for the Supernova blockchain
//!
//! This module provides a Merkle Tree implementation with proof generation and verification.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fmt;
use thiserror::Error;

/// Errors that can occur when working with Merkle trees
#[derive(Error, Debug)]
pub enum MerkleError {
    /// Index is out of bounds
    #[error("Index out of bounds: {0}")]
    IndexOutOfBounds(usize),

    /// Invalid proof
    #[error("Invalid Merkle proof")]
    InvalidProof,

    /// Empty tree
    #[error("Empty Merkle tree")]
    EmptyTree,

    /// Serialization error
    #[error("Serialization error: {0}")]
    SerializationError(#[from] bincode::Error),
}

/// Result type for Merkle tree operations
pub type MerkleResult<T> = Result<T, MerkleError>;

/// A Merkle Tree implementation
#[derive(Debug, Clone)]
pub struct MerkleTree {
    /// The leaves of the tree (original data hashes)
    pub leaves: Vec<[u8; 32]>,
    /// All nodes in the tree, level by level
    nodes: Vec<Vec<[u8; 32]>>,
    /// The root hash of the tree
    pub root: [u8; 32],
}

/// A proof of inclusion in a Merkle tree
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MerkleProof {
    /// The hash of the leaf being proven
    pub leaf_hash: [u8; 32],
    /// The lemma (sibling hashes along the path to the root)
    pub lemma: Vec<[u8; 32]>,
    /// The indices for traversing the tree (0 = left, 1 = right)
    pub path_indices: Vec<usize>,
    /// The root hash of the Merkle tree
    pub root_hash: [u8; 32],
}

impl MerkleTree {
    /// Create a new Merkle tree from the given data
    pub fn new<T: AsRef<[u8]>>(data: &[T]) -> Self {
        if data.is_empty() {
            // Empty tree has a special zero root
            return Self {
                leaves: Vec::new(),
                nodes: Vec::new(),
                root: [0u8; 32],
            };
        }

        // Hash all leaves
        let mut leaves = Vec::with_capacity(data.len());
        for item in data {
            let mut hasher = Sha256::new();
            hasher.update(item.as_ref());
            let mut hash = [0u8; 32];
            hash.copy_from_slice(&hasher.finalize());
            leaves.push(hash);
        }

        // Build the tree
        let mut nodes = Vec::new();
        nodes.push(leaves.clone());

        let mut level = 0;
        while nodes[level].len() > 1 {
            let current_level = &nodes[level];
            let mut next_level = Vec::new();

            // Combine pairs of nodes to create the next level
            for i in (0..current_level.len()).step_by(2) {
                if i + 1 < current_level.len() {
                    // Combine two nodes
                    let combined = Self::hash_pair(&current_level[i], &current_level[i + 1]);
                    next_level.push(combined);
                } else {
                    // Odd number of nodes, promote the last one
                    next_level.push(current_level[i]);
                }
            }

            nodes.push(next_level);
            level += 1;
        }

        // The last level should contain only the root
        let root = nodes.last().unwrap()[0];

        Self {
            leaves,
            nodes,
            root,
        }
    }

    /// Create a proof for the data at the given index
    pub fn create_proof(&self, index: usize) -> MerkleResult<MerkleProof> {
        if self.leaves.is_empty() {
            return Err(MerkleError::EmptyTree);
        }

        if index >= self.leaves.len() {
            return Err(MerkleError::IndexOutOfBounds(index));
        }

        let mut lemma = Vec::new();
        let mut path_indices = Vec::new();

        let mut current_index = index;

        // Start at the bottom level (leaves) and work up
        for level in 0..self.nodes.len() - 1 {
            let level_len = self.nodes[level].len();

            // Determine sibling index
            let sibling_index = if current_index % 2 == 0 {
                // Current node is left child, get right sibling
                current_index + 1
            } else {
                // Current node is right child, get left sibling
                current_index - 1
            };

            // Add path index (0 for left, 1 for right)
            path_indices.push(current_index % 2);

            // Add sibling to lemma if it exists
            if sibling_index < level_len {
                lemma.push(self.nodes[level][sibling_index]);
            } else {
                // If there's no sibling, duplicate the current node
                lemma.push(self.nodes[level][current_index]);
            }

            // Move to parent index for the next level
            current_index /= 2;
        }

        Ok(MerkleProof {
            leaf_hash: self.leaves[index],
            lemma,
            path_indices,
            root_hash: self.root,
        })
    }

    /// Verify a proof
    pub fn verify_proof(proof: &MerkleProof) -> bool {
        if proof.lemma.is_empty() {
            // Special case: if there's only one node, the proof is valid if
            // the leaf hash equals the root hash
            return proof.leaf_hash == proof.root_hash;
        }

        let mut current_hash = proof.leaf_hash;

        // Traverse the path and compute the root
        for (i, path_index) in proof.path_indices.iter().enumerate() {
            if i >= proof.lemma.len() {
                return false;
            }

            let sibling = proof.lemma[i];

            // Compute the parent hash
            if *path_index == 0 {
                // Current node is left child
                current_hash = Self::hash_pair(&current_hash, &sibling);
            } else {
                // Current node is right child
                current_hash = Self::hash_pair(&sibling, &current_hash);
            }
        }

        // Check if the computed root matches the expected root
        current_hash == proof.root_hash
    }

    /// Hash a pair of nodes
    fn hash_pair(left: &[u8; 32], right: &[u8; 32]) -> [u8; 32] {
        let mut hasher = Sha256::new();
        hasher.update(left);
        hasher.update(right);
        let mut result = [0u8; 32];
        result.copy_from_slice(&hasher.finalize());
        result
    }

    /// Get the root hash of the tree
    pub fn root_hash(&self) -> [u8; 32] {
        self.root
    }

    /// Get the number of leaves in the tree
    pub fn len(&self) -> usize {
        self.leaves.len()
    }

    /// Check if the tree is empty
    pub fn is_empty(&self) -> bool {
        self.leaves.is_empty()
    }
}

impl fmt::Display for MerkleTree {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "MerkleTree {{")?;
        writeln!(f, "  leaves: {}", self.leaves.len())?;
        writeln!(f, "  levels: {}", self.nodes.len())?;
        writeln!(f, "  root: {}", hex::encode(self.root))?;
        writeln!(f, "}}")
    }
}

impl MerkleProof {
    /// Serialize the proof to bytes
    pub fn to_bytes(&self) -> MerkleResult<Vec<u8>> {
        bincode::serialize(self).map_err(MerkleError::SerializationError)
    }

    /// Deserialize a proof from bytes
    pub fn from_bytes(bytes: &[u8]) -> MerkleResult<Self> {
        bincode::deserialize(bytes).map_err(MerkleError::SerializationError)
    }
}

impl fmt::Display for MerkleProof {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "MerkleProof {{")?;
        writeln!(f, "  leaf_hash: {}", hex::encode(self.leaf_hash))?;
        writeln!(f, "  path_length: {}", self.lemma.len())?;
        writeln!(f, "  root_hash: {}", hex::encode(self.root_hash))?;
        writeln!(f, "}}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_tree() {
        let tree = MerkleTree::new::<&[u8]>(&[]);
        assert!(tree.is_empty());
        assert_eq!(tree.len(), 0);
        assert_eq!(tree.root, [0u8; 32]);
    }

    #[test]
    fn test_single_leaf() {
        let data = vec![b"test data".to_vec()];
        let tree = MerkleTree::new(&data);

        assert_eq!(tree.len(), 1);

        // For a single leaf, the leaf hash is the root hash
        let mut hasher = Sha256::new();
        hasher.update(&data[0]);
        let mut expected = [0u8; 32];
        expected.copy_from_slice(&hasher.finalize());

        assert_eq!(tree.root, expected);
    }

    #[test]
    fn test_multiple_leaves() {
        let data = vec![
            b"data1".to_vec(),
            b"data2".to_vec(),
            b"data3".to_vec(),
            b"data4".to_vec(),
        ];

        let tree = MerkleTree::new(&data);

        assert_eq!(tree.len(), 4);
        assert_eq!(tree.nodes.len(), 3); // Leaves + 2 levels
    }

    #[test]
    fn test_proof_creation_and_verification() {
        let data = vec![
            b"data1".to_vec(),
            b"data2".to_vec(),
            b"data3".to_vec(),
            b"data4".to_vec(),
        ];

        let tree = MerkleTree::new(&data);

        for i in 0..data.len() {
            let proof = tree.create_proof(i).unwrap();
            assert!(MerkleTree::verify_proof(&proof));

            // Test serialization
            let bytes = proof.to_bytes().unwrap();
            let deserialized = MerkleProof::from_bytes(&bytes).unwrap();
            assert!(MerkleTree::verify_proof(&deserialized));
        }
    }

    #[test]
    fn test_invalid_proof() {
        let data = vec![
            b"data1".to_vec(),
            b"data2".to_vec(),
            b"data3".to_vec(),
            b"data4".to_vec(),
        ];

        let tree = MerkleTree::new(&data);

        // Create a valid proof
        let mut proof = tree.create_proof(0).unwrap();

        // Tamper with the leaf hash
        proof.leaf_hash[0] = proof.leaf_hash[0].wrapping_add(1);

        // Verification should fail
        assert!(!MerkleTree::verify_proof(&proof));
    }

    #[test]
    fn test_proof_out_of_bounds() {
        let data = vec![b"test".to_vec()];
        let tree = MerkleTree::new(&data);

        assert!(tree.create_proof(0).is_ok());
        assert!(matches!(
            tree.create_proof(1),
            Err(MerkleError::IndexOutOfBounds(1))
        ));
    }
}
