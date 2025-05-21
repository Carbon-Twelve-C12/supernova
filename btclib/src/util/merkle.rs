use sha2::{Sha256, Digest};
use std::rc::Rc;
use std::cell::RefCell;
use thiserror::Error;

/// Error types related to Merkle tree operations
#[derive(Debug, Error)]
pub enum MerkleError {
    #[error("Empty tree")]
    EmptyTree,
    
    #[error("Invalid proof format")]
    InvalidProofFormat,
    
    #[error("Transaction not found")]
    TransactionNotFound,
    
    #[error("Merkle path verification failed")]
    VerificationFailed,
    
    #[error("Serialization error: {0}")]
    SerializationError(String),
}

/// Direction for a hash in a Merkle path
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MerkleDirection {
    Left,
    Right,
}

/// A single step in a Merkle proof
#[derive(Debug, Clone)]
pub struct MerkleProofStep {
    /// The hash to combine with
    pub hash: [u8; 32],
    /// Whether this hash goes on the left or right
    pub direction: MerkleDirection,
}

/// A complete Merkle proof
#[derive(Debug, Clone)]
pub struct MerkleProof {
    /// The transaction hash being proven
    pub tx_hash: [u8; 32],
    /// Steps to reconstruct the root
    pub steps: Vec<MerkleProofStep>,
    /// Expected root hash
    pub root_hash: [u8; 32],
}

/// Represents a node in the Merkle tree
#[derive(Debug, Clone)]
pub struct MerkleNode {
    /// The hash stored at this node
    hash: [u8; 32],
    /// Reference to left child
    left: Option<Rc<RefCell<MerkleNode>>>,
    /// Reference to right child
    right: Option<Rc<RefCell<MerkleNode>>>,
    /// Original transaction data (only for leaf nodes)
    data: Option<Vec<u8>>,
}

impl MerkleNode {
    /// Create a new leaf node from transaction data
    pub fn new_leaf(data: &[u8]) -> Self {
        // Double hash for security (like in Bitcoin)
        let hash = Self::double_hash(data);
        
        Self {
            hash,
            left: None,
            right: None,
            data: Some(data.to_vec()),
        }
    }

    /// Create a new internal node from two child nodes
    pub fn new_internal(left: Rc<RefCell<MerkleNode>>, right: Rc<RefCell<MerkleNode>>) -> Self {
        // Hash = H(left_hash || right_hash)
        let left_hash = left.borrow().hash;
        let right_hash = right.borrow().hash;
        
        let mut combined = Vec::with_capacity(64);
        combined.extend_from_slice(&left_hash);
        combined.extend_from_slice(&right_hash);
        
        let hash = Self::double_hash(&combined);

        Self {
            hash,
            left: Some(left),
            right: Some(right),
            data: None,
        }
    }

    /// Double-hash a piece of data (SHA256(SHA256(data)))
    fn double_hash(data: &[u8]) -> [u8; 32] {
        let first_hash = Sha256::digest(data);
        let second_hash = Sha256::digest(&first_hash);
        
        let mut hash = [0u8; 32];
        hash.copy_from_slice(&second_hash);
        hash
    }

    /// Get the hash of this node
    pub fn hash(&self) -> [u8; 32] {
        self.hash
    }
    
    /// Check if this is a leaf node
    pub fn is_leaf(&self) -> bool {
        self.left.is_none() && self.right.is_none()
    }
    
    /// Get the original transaction data if this is a leaf node
    pub fn data(&self) -> Option<&Vec<u8>> {
        self.data.as_ref()
    }
}

/// Represents a Merkle tree for transaction verification
pub struct MerkleTree {
    /// Root node of the tree
    root: Option<Rc<RefCell<MerkleNode>>>,
    /// Number of transactions in the tree
    num_transactions: usize,
}

impl MerkleTree {
    /// Build a new Merkle tree from a list of transactions
    pub fn new<T: AsRef<[u8]>>(transactions: &[T]) -> Self {
        if transactions.is_empty() {
            return Self { 
                root: None,
                num_transactions: 0,
            };
        }

        // Create leaf nodes
        let mut nodes: Vec<Rc<RefCell<MerkleNode>>> = transactions
            .iter()
            .map(|t| Rc::new(RefCell::new(MerkleNode::new_leaf(t.as_ref()))))
            .collect();
            
        let num_transactions = nodes.len();

        // If odd number of nodes, duplicate the last one
        if nodes.len() % 2 == 1 {
            let last = Rc::clone(&nodes[nodes.len() - 1]);
            nodes.push(last);
        }

        // Build tree from bottom up
        while nodes.len() > 1 {
            let mut next_level = Vec::new();

            // Process pairs of nodes
            for chunk in nodes.chunks(2) {
                if chunk.len() == 2 {
                    let left = Rc::clone(&chunk[0]);
                    let right = Rc::clone(&chunk[1]);
                    
                    let parent = Rc::new(RefCell::new(
                        MerkleNode::new_internal(left, right)
                    ));
                    
                    next_level.push(parent);
                } else {
                    // This should never happen if we duplicate the last node for odd counts
                    let node = Rc::clone(&chunk[0]);
                    next_level.push(node);
                }
            }

            nodes = next_level;
        }

        Self {
            root: Some(Rc::clone(&nodes[0])),
            num_transactions,
        }
    }

    /// Get the root hash of the tree
    pub fn root_hash(&self) -> Option<[u8; 32]> {
        self.root.as_ref().map(|node| node.borrow().hash())
    }
    
    /// Get the root hash of the tree as [u8; 32]
    /// Returns zeroed array if the tree is empty
    pub fn get_root(&self) -> [u8; 32] {
        self.root_hash().unwrap_or([0; 32])
    }
    
    /// Get the number of transactions in the tree
    pub fn num_transactions(&self) -> usize {
        self.num_transactions
    }

    /// Create a Merkle proof for a transaction
    pub fn create_proof(&self, transaction: &[u8]) -> Result<MerkleProof, MerkleError> {
        if self.root.is_none() {
            return Err(MerkleError::EmptyTree);
        }
        
        // Hash the transaction to get the leaf hash
        let tx_hash = MerkleNode::double_hash(transaction);
        
        // Find the path to the transaction and create the proof
        let mut path = Vec::new();
        let mut found = false;
        
        self.find_path_to_transaction(&self.root.as_ref().unwrap(), &tx_hash, &mut path, &mut found)?;
        
        if !found {
            return Err(MerkleError::TransactionNotFound);
        }
        
        let root_hash = self.root_hash().ok_or(MerkleError::EmptyTree)?;
        
        Ok(MerkleProof {
            tx_hash,
            steps: path,
            root_hash,
        })
    }
    
    /// Find the path to a transaction with the given hash
    fn find_path_to_transaction(
        &self,
        node: &Rc<RefCell<MerkleNode>>,
        tx_hash: &[u8; 32],
        path: &mut Vec<MerkleProofStep>,
        found: &mut bool
    ) -> Result<(), MerkleError> {
        let node_ref = node.borrow();
        
        // If this is a leaf node, check if it matches
        if node_ref.is_leaf() {
            if node_ref.hash == *tx_hash {
                *found = true;
            }
            return Ok(());
        }
        
        // Process left subtree
        if let Some(left) = &node_ref.left {
            let mut left_found = false;
            self.find_path_to_transaction(left, tx_hash, path, &mut left_found)?;
            
            if left_found {
                // Add right sibling to the path
                if let Some(right) = &node_ref.right {
                    path.push(MerkleProofStep {
                        hash: right.borrow().hash,
                        direction: MerkleDirection::Right,
                    });
                }
                
                *found = true;
                return Ok(());
            }
        }
        
        // Process right subtree
        if let Some(right) = &node_ref.right {
            let mut right_found = false;
            self.find_path_to_transaction(right, tx_hash, path, &mut right_found)?;
            
            if right_found {
                // Add left sibling to the path
                if let Some(left) = &node_ref.left {
                    path.push(MerkleProofStep {
                        hash: left.borrow().hash,
                        direction: MerkleDirection::Left,
                    });
                }
                
                *found = true;
                return Ok(());
            }
        }
        
        Ok(())
    }

    /// Verify that a transaction is included in the tree (shorthand method)
    pub fn verify(&self, transaction: &[u8]) -> bool {
        match self.create_proof(transaction) {
            Ok(proof) => proof.verify(),
            Err(_) => false,
        }
    }
    
    /// Verify a Merkle proof
    pub fn verify_proof(proof: &MerkleProof) -> bool {
        proof.verify()
    }
}

impl MerkleProof {
    /// Verify this Merkle proof
    pub fn verify(&self) -> bool {
        let mut current_hash = self.tx_hash;
        
        // Apply each step in the proof
        for step in &self.steps {
            let mut combined = Vec::with_capacity(64);
            
            match step.direction {
                MerkleDirection::Left => {
                    combined.extend_from_slice(&step.hash);
                    combined.extend_from_slice(&current_hash);
                },
                MerkleDirection::Right => {
                    combined.extend_from_slice(&current_hash);
                    combined.extend_from_slice(&step.hash);
                },
            }
            
            current_hash = MerkleNode::double_hash(&combined);
        }
        
        // Check if we arrived at the expected root
        current_hash == self.root_hash
    }
    
    /// Serialize the proof to binary format
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut result = Vec::new();
        
        // Add transaction hash
        result.extend_from_slice(&self.tx_hash);
        
        // Add number of steps
        let steps_len = self.steps.len() as u16;
        result.extend_from_slice(&steps_len.to_be_bytes());
        
        // Add each step
        for step in &self.steps {
            // Add direction (0 = left, 1 = right)
            let direction_byte = match step.direction {
                MerkleDirection::Left => 0u8,
                MerkleDirection::Right => 1u8,
            };
            result.push(direction_byte);
            
            // Add hash
            result.extend_from_slice(&step.hash);
        }
        
        // Add root hash
        result.extend_from_slice(&self.root_hash);
        
        result
    }
    
    /// Deserialize a proof from binary format
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, MerkleError> {
        if bytes.len() < 66 { // 32 (tx_hash) + 2 (steps_len) + 32 (root_hash)
            return Err(MerkleError::InvalidProofFormat);
        }
        
        let mut pos = 0;
        
        // Read transaction hash
        let mut tx_hash = [0u8; 32];
        tx_hash.copy_from_slice(&bytes[pos..pos+32]);
        pos += 32;
        
        // Read number of steps
        let steps_len = u16::from_be_bytes([bytes[pos], bytes[pos+1]]) as usize;
        pos += 2;
        
        // Read steps
        let mut steps = Vec::with_capacity(steps_len);
        for _ in 0..steps_len {
            if pos + 33 > bytes.len() {
                return Err(MerkleError::InvalidProofFormat);
            }
            
            // Read direction
            let direction = match bytes[pos] {
                0 => MerkleDirection::Left,
                1 => MerkleDirection::Right,
                _ => return Err(MerkleError::InvalidProofFormat),
            };
            pos += 1;
            
            // Read hash
            let mut hash = [0u8; 32];
            hash.copy_from_slice(&bytes[pos..pos+32]);
            pos += 32;
            
            steps.push(MerkleProofStep { hash, direction });
        }
        
        // Read root hash
        if pos + 32 > bytes.len() {
            return Err(MerkleError::InvalidProofFormat);
        }
        
        let mut root_hash = [0u8; 32];
        root_hash.copy_from_slice(&bytes[pos..pos+32]);
        
        Ok(MerkleProof {
            tx_hash,
            steps,
            root_hash,
        })
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
            b"transaction1".as_slice(),
            b"transaction2".as_slice(),
            b"transaction3".as_slice(),
            b"transaction4".as_slice(),
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
            b"transaction1".as_slice(),
            b"transaction2".as_slice(),
            b"transaction3".as_slice(),
        ];
        let tree = MerkleTree::new(&transactions);
        
        for tx in &transactions {
            assert!(tree.verify(tx));
        }
    }
    
    #[test]
    fn test_merkle_proof_creation_and_verification() {
        let transactions = vec![
            b"transaction1".as_slice(),
            b"transaction2".as_slice(),
            b"transaction3".as_slice(),
            b"transaction4".as_slice(),
        ];
        let tree = MerkleTree::new(&transactions);
        
        // Create a proof for transaction2
        let proof = tree.create_proof(b"transaction2").unwrap();
        
        // Verify the proof
        assert!(proof.verify());
        
        // Create an invalid proof by modifying the transaction hash
        let mut invalid_proof = proof.clone();
        invalid_proof.tx_hash[0] ^= 0xFF;
        
        // Verify that the invalid proof fails
        assert!(!invalid_proof.verify());
    }
    
    #[test]
    fn test_merkle_proof_serialization() {
        let transactions = vec![
            b"transaction1".as_slice(),
            b"transaction2".as_slice(),
            b"transaction3".as_slice(),
            b"transaction4".as_slice(),
        ];
        let tree = MerkleTree::new(&transactions);
        
        let original_proof = tree.create_proof(b"transaction3").unwrap();
        
        // Serialize and deserialize
        let bytes = original_proof.to_bytes();
        let deserialized_proof = MerkleProof::from_bytes(&bytes).unwrap();
        
        // Check that they match
        assert_eq!(original_proof.tx_hash, deserialized_proof.tx_hash);
        assert_eq!(original_proof.root_hash, deserialized_proof.root_hash);
        assert_eq!(original_proof.steps.len(), deserialized_proof.steps.len());
        
        // Verify the deserialized proof
        assert!(deserialized_proof.verify());
    }
}