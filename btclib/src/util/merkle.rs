use sha2::{Sha256, Digest};

/// Represents a node in the Merkle tree
#[derive(Debug, Clone)]
pub struct MerkleNode {
    hash: [u8; 32],
    left: Option<Box<MerkleNode>>,
    right: Option<Box<MerkleNode>>,
}

impl MerkleNode {
    /// Create a new leaf node from transaction data
    pub fn new_leaf(data: &[u8]) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(data);
        let result = hasher.finalize();
        let mut hash = [0u8; 32];
        hash.copy_from_slice(&result);
        
        Self {
            hash,
            left: None,
            right: None,
        }
    }

    /// Create a new internal node from two child nodes
    pub fn new_internal(left: MerkleNode, right: MerkleNode) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(&left.hash);
        hasher.update(&right.hash);
        let result = hasher.finalize();
        let mut hash = [0u8; 32];
        hash.copy_from_slice(&result);

        Self {
            hash,
            left: Some(Box::new(left)),
            right: Some(Box::new(right)),
        }
    }

    /// Get the hash of this node
    pub fn hash(&self) -> [u8; 32] {
        self.hash
    }
}

pub struct MerkleTree {
    root: Option<MerkleNode>,
}

impl MerkleTree {
    /// Build a new Merkle tree from a list of transactions
    pub fn new<T: AsRef<[u8]>>(transactions: &[T]) -> Self {
        if transactions.is_empty() {
            return Self { root: None };
        }

        // Create leaf nodes
        let mut nodes: Vec<MerkleNode> = transactions
            .iter()
            .map(|t| MerkleNode::new_leaf(t.as_ref()))
            .collect();

        // Build tree from bottom up
        while nodes.len() > 1 {
            let mut next_level = Vec::new();

            // Process pairs of nodes
            for chunk in nodes.chunks(2) {
                match chunk {
                    [left, right] => {
                        next_level.push(MerkleNode::new_internal(left.clone(), right.clone()));
                    }
                    [left] => {
                        // If we have an odd number of nodes, duplicate the last one
                        next_level.push(MerkleNode::new_internal(left.clone(), left.clone()));
                    }
                    _ => unreachable!(),
                }
            }

            nodes = next_level;
        }

        Self {
            root: Some(nodes.pop().unwrap()),
        }
    }

    /// Get the root hash of the tree
    pub fn root_hash(&self) -> Option<[u8; 32]> {
        self.root.as_ref().map(|node| node.hash())
    }

    /// Verify that a transaction is included in the tree
    pub fn verify(&self, transaction: &[u8]) -> bool {
        let target_hash = {
            let mut hasher = Sha256::new();
            hasher.update(transaction);
            let result = hasher.finalize();
            let mut hash = [0u8; 32];
            hash.copy_from_slice(&result);
            hash
        };

        // Traverse the tree to find the target hash
        if let Some(root) = &self.root {
            Self::verify_node(root, &target_hash)
        } else {
            false
        }
    }

    fn verify_node(node: &MerkleNode, target_hash: &[u8; 32]) -> bool {
        if node.hash == *target_hash {
            return true;
        }

        if let Some(left) = &node.left {
            if Self::verify_node(left, target_hash) {
                return true;
            }
        }

        if let Some(right) = &node.right {
            if Self::verify_node(right, target_hash) {
                return true;
            }
        }

        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_merkle_tree_single_transaction() {
        let transactions = vec![b"transaction1"];
        let tree = MerkleTree::new(&transactions);
        assert!(tree.verify(b"transaction1"));
        assert!(!tree.verify(b"invalid_transaction"));
    }

    #[test]
    fn test_merkle_tree_multiple_transactions() {
        let transactions = vec![
            b"transaction1",
            b"transaction2",
            b"transaction3",
            b"transaction4",
        ];
        let tree = MerkleTree::new(&transactions);
        
        for tx in &transactions {
            assert!(tree.verify(tx));
        }
        assert!(!tree.verify(b"invalid_transaction"));
    }

    #[test]
    fn test_merkle_tree_odd_transactions() {
        let transactions = vec![
            b"transaction1",
            b"transaction2",
            b"transaction3",
        ];
        let tree = MerkleTree::new(&transactions);
        
        for tx in &transactions {
            assert!(tree.verify(tx));
        }
    }
}