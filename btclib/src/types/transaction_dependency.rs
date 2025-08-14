use std::collections::{HashMap, HashSet, VecDeque};
use crate::types::transaction::Transaction;

/// Represents the dependency graph of transactions in the mempool
#[derive(Debug, Default)]
pub struct TransactionDependencyGraph {
    /// Map from transaction hash to its dependencies (transactions it depends on)
    dependencies: HashMap<[u8; 32], HashSet<[u8; 32]>>,
    /// Map from transaction hash to its dependents (transactions that depend on it)
    dependents: HashMap<[u8; 32], HashSet<[u8; 32]>>,
}

impl TransactionDependencyGraph {
    /// Create a new, empty dependency graph
    pub fn new() -> Self {
        Self {
            dependencies: HashMap::new(),
            dependents: HashMap::new(),
        }
    }
    
    /// Add a transaction to the dependency graph
    pub fn add_transaction(&mut self, tx: &Transaction, mempool_txs: &HashMap<[u8; 32], Transaction>) {
        let tx_hash = tx.hash();
        
        // Initialize empty sets if needed
        self.dependencies.entry(tx_hash).or_insert_with(HashSet::new);
        self.dependents.entry(tx_hash).or_insert_with(HashSet::new);
        
        // Check each input for dependencies on other mempool transactions
        for input in tx.inputs() {
            let prev_tx_hash = input.prev_tx_hash();
            
            // If the input references another transaction in the mempool, it's a dependency
            if mempool_txs.contains_key(&prev_tx_hash) {
                // Add dependency relationship
                self.dependencies.get_mut(&tx_hash).unwrap().insert(prev_tx_hash);
                
                // Add dependent relationship (inverse of dependency)
                self.dependents.entry(prev_tx_hash).or_insert_with(HashSet::new).insert(tx_hash);
            }
        }
    }
    
    /// Remove a transaction from the dependency graph
    pub fn remove_transaction(&mut self, tx_hash: &[u8; 32]) {
        // Remove as a dependency from other transactions
        if let Some(deps) = self.dependents.remove(tx_hash) {
            for dep_tx in deps {
                if let Some(dependencies) = self.dependencies.get_mut(&dep_tx) {
                    dependencies.remove(tx_hash);
                }
            }
        }
        
        // Remove its dependencies
        self.dependencies.remove(tx_hash);
    }
    
    /// Get transactions that can be processed immediately (have no dependencies)
    pub fn get_ready_transactions(&self) -> Vec<[u8; 32]> {
        self.dependencies
            .iter()
            .filter(|(_, deps)| deps.is_empty())
            .map(|(tx_hash, _)| *tx_hash)
            .collect()
    }
    
    /// Get transactions in topological order (dependencies first)
    pub fn get_topological_order(&self) -> Vec<[u8; 32]> {
        let mut result = Vec::new();
        let mut temp_graph = self.clone();
        
        // Process until no transactions left
        while !temp_graph.dependencies.is_empty() {
            // Get transactions with no dependencies
            let ready = temp_graph.get_ready_transactions();
            
            // If no ready transactions but graph not empty, we have a cycle
            if ready.is_empty() {
                // In a real implementation, we'd need to handle cycles
                // For now, just break to avoid infinite loop
                break;
            }
            
            // Add ready transactions to result
            for tx_hash in &ready {
                result.push(*tx_hash);
                temp_graph.remove_transaction(tx_hash);
            }
        }
        
        result
    }
    
    /// Get all transactions that depend on the given transaction
    pub fn get_dependents(&self, tx_hash: &[u8; 32]) -> Option<&HashSet<[u8; 32]>> {
        self.dependents.get(tx_hash)
    }
    
    /// Check if a transaction has any dependencies
    pub fn has_dependencies(&self, tx_hash: &[u8; 32]) -> bool {
        match self.dependencies.get(tx_hash) {
            Some(deps) => !deps.is_empty(),
            None => false,
        }
    }
    
    /// Get direct dependencies of a transaction
    pub fn get_dependencies(&self, tx_hash: &[u8; 32]) -> Option<&HashSet<[u8; 32]>> {
        self.dependencies.get(tx_hash)
    }
    
    /// Find all ancestors (recursive dependencies) of a transaction
    pub fn get_all_ancestors(&self, tx_hash: &[u8; 32]) -> HashSet<[u8; 32]> {
        let mut ancestors = HashSet::new();
        let mut queue = VecDeque::new();
        
        // Start with direct dependencies
        if let Some(deps) = self.get_dependencies(tx_hash) {
            for dep in deps {
                queue.push_back(*dep);
            }
        }
        
        // BFS to find all ancestors
        while let Some(curr_tx) = queue.pop_front() {
            if ancestors.insert(curr_tx) {
                // If this is a new ancestor, add its dependencies to the queue
                if let Some(deps) = self.get_dependencies(&curr_tx) {
                    for dep in deps {
                        queue.push_back(*dep);
                    }
                }
            }
        }
        
        ancestors
    }
    
    /// Find all descendants (recursive dependents) of a transaction
    pub fn get_all_descendants(&self, tx_hash: &[u8; 32]) -> HashSet<[u8; 32]> {
        let mut descendants = HashSet::new();
        let mut queue = VecDeque::new();
        
        // Start with direct dependents
        if let Some(deps) = self.get_dependents(tx_hash) {
            for dep in deps {
                queue.push_back(*dep);
            }
        }
        
        // BFS to find all descendants
        while let Some(curr_tx) = queue.pop_front() {
            if descendants.insert(curr_tx) {
                // If this is a new descendant, add its dependents to the queue
                if let Some(deps) = self.get_dependents(&curr_tx) {
                    for dep in deps {
                        queue.push_back(*dep);
                    }
                }
            }
        }
        
        descendants
    }
    
    /// Calculate the package fee rate for a transaction and all its ancestors
    pub fn calculate_package_fee_rate(
        &self, 
        tx_hash: &[u8; 32], 
        mempool_txs: &HashMap<[u8; 32], Transaction>,
        get_output: impl Fn(&[u8; 32], u32) -> Option<crate::types::transaction::TransactionOutput>
    ) -> Option<u64> {
        // Get the transaction and all its ancestors
        let mut package = HashSet::new();
        package.insert(*tx_hash);
        package.extend(self.get_all_ancestors(tx_hash));
        
        // Calculate total fee and size
        let mut total_fee = 0u64;
        let mut total_size = 0usize;
        
        for tx_hash in &package {
            if let Some(tx) = mempool_txs.get(tx_hash) {
                if let Some(fee) = tx.calculate_fee(&get_output) {
                    total_fee += fee;
                    total_size += tx.calculate_size();
                } else {
                    return None; // Cannot calculate fee for one transaction
                }
            } else {
                return None; // Transaction not found in mempool
            }
        }
        
        if total_size > 0 {
            Some(total_fee / total_size as u64)
        } else {
            None
        }
    }
}

impl Clone for TransactionDependencyGraph {
    fn clone(&self) -> Self {
        Self {
            dependencies: self.dependencies.clone(),
            dependents: self.dependents.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::transaction::{Transaction, TransactionInput, TransactionOutput};
    
    // Helper function to create a test transaction with specified inputs
    fn create_test_tx(prev_tx_hashes: Vec<[u8; 32]>) -> Transaction {
        let inputs = prev_tx_hashes.into_iter().enumerate().map(|(i, hash)| {
            TransactionInput::new(hash, i as u32, vec![], 0xffffffff)
        }).collect();
        
        let outputs = vec![TransactionOutput::new(50_000_000, vec![])];
        
        Transaction::new(1, inputs, outputs, 0)
    }
    
    #[test]
    fn test_dependency_tracking() {
        let mut graph = TransactionDependencyGraph::new();
        let mut mempool = HashMap::new();
        
        // Create transactions with dependencies
        // tx1 has no mempool dependencies (uses external UTXO)
        let external_utxo = [255u8; 32]; // Represents a UTXO from blockchain
        let tx1 = create_test_tx(vec![external_utxo]);
        let tx1_hash = tx1.hash();
        
        // tx2 depends on tx1
        let tx2 = create_test_tx(vec![tx1_hash]);
        let tx2_hash = tx2.hash();
        
        // tx3 depends on tx2
        let tx3 = create_test_tx(vec![tx2_hash]);
        let tx3_hash = tx3.hash();
        
        // Add to mempool with their actual hashes
        mempool.insert(tx1_hash, tx1.clone());
        mempool.insert(tx2_hash, tx2.clone());
        mempool.insert(tx3_hash, tx3.clone());
        
        // Add to dependency graph
        graph.add_transaction(&tx1, &mempool);
        graph.add_transaction(&tx2, &mempool);
        graph.add_transaction(&tx3, &mempool);
        
        // Check dependencies
        assert!(!graph.has_dependencies(&tx1_hash));
        assert!(graph.has_dependencies(&tx2_hash));
        assert!(graph.has_dependencies(&tx3_hash));
        
        // Check topological order
        let order = graph.get_topological_order();
        assert_eq!(order.len(), 3);
        assert_eq!(order[0], tx1_hash); // tx1 has no dependencies, should be first
        
        // Check ancestors
        let tx3_ancestors = graph.get_all_ancestors(&tx3_hash);
        assert_eq!(tx3_ancestors.len(), 2);
        assert!(tx3_ancestors.contains(&tx1_hash));
        assert!(tx3_ancestors.contains(&tx2_hash));
        
        // Check descendants
        let tx1_descendants = graph.get_all_descendants(&tx1_hash);
        assert_eq!(tx1_descendants.len(), 2);
        assert!(tx1_descendants.contains(&tx2_hash));
        assert!(tx1_descendants.contains(&tx3_hash));
    }
} 