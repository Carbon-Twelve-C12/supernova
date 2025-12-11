//! Mempool Chain Limits - Ancestor/Descendant Tracking
//!
//! SECURITY MODULE (P1-002): Prevents DoS attacks via long transaction chains
//!
//! This module enforces Bitcoin-style ancestor/descendant limits to prevent:
//! - Memory exhaustion from long unconfirmed chains
//! - CPU exhaustion from chain traversal
//! - Pinning attacks that block legitimate transactions
//!
//! Default limits (matching Bitcoin Core):
//! - Max 25 ancestors per transaction
//! - Max 25 descendants per transaction
//! - Max 101KB total ancestor size
//! - Max 101KB total descendant size

use dashmap::DashMap;
use std::collections::{HashSet, VecDeque};
use std::sync::Arc;
use tracing::warn;

use crate::mempool::error::MempoolError;

/// Configuration for chain limits
#[derive(Debug, Clone)]
pub struct ChainLimitsConfig {
    /// Maximum number of ancestors per transaction
    pub max_ancestors: usize,
    /// Maximum number of descendants per transaction
    pub max_descendants: usize,
    /// Maximum total size of ancestor chain in bytes
    pub max_ancestor_size_bytes: usize,
    /// Maximum total size of descendant chain in bytes
    pub max_descendant_size_bytes: usize,
    /// Maximum transactions that can be evicted by RBF
    pub max_rbf_evictions: usize,
}

impl Default for ChainLimitsConfig {
    fn default() -> Self {
        Self {
            max_ancestors: 25,
            max_descendants: 25,
            max_ancestor_size_bytes: 101 * 1024,      // 101KB
            max_descendant_size_bytes: 101 * 1024,    // 101KB
            max_rbf_evictions: 100,                   // Max 100 txs evicted by single RBF
        }
    }
}

/// Information about a transaction's chain relationships
#[derive(Debug, Clone)]
pub struct TxChainInfo {
    /// Transaction hash
    pub tx_hash: [u8; 32],
    /// Transaction size in bytes
    pub size: usize,
    /// Parent transaction hashes (transactions this tx spends from)
    pub parents: HashSet<[u8; 32]>,
    /// Child transaction hashes (transactions that spend from this tx)
    pub children: HashSet<[u8; 32]>,
}

impl TxChainInfo {
    pub fn new(tx_hash: [u8; 32], size: usize) -> Self {
        Self {
            tx_hash,
            size,
            parents: HashSet::new(),
            children: HashSet::new(),
        }
    }
}

/// Chain limits tracker for mempool transactions
///
/// SECURITY: Tracks parent-child relationships between unconfirmed transactions
/// and enforces limits to prevent DoS attacks via long chains.
pub struct ChainLimitsTracker {
    /// Transaction chain information
    tx_info: Arc<DashMap<[u8; 32], TxChainInfo>>,
    /// Configuration
    config: ChainLimitsConfig,
}

impl ChainLimitsTracker {
    /// Create a new chain limits tracker
    pub fn new(config: ChainLimitsConfig) -> Self {
        Self {
            tx_info: Arc::new(DashMap::new()),
            config,
        }
    }

    /// Create with default configuration
    pub fn with_defaults() -> Self {
        Self::new(ChainLimitsConfig::default())
    }

    /// Check if adding a transaction would violate chain limits
    ///
    /// # Arguments
    /// * `tx_hash` - Hash of the new transaction
    /// * `tx_size` - Size of the new transaction in bytes
    /// * `parent_hashes` - Hashes of transactions this tx spends from (if in mempool)
    ///
    /// # Returns
    /// * `Ok(())` - Transaction passes chain limit checks
    /// * `Err(MempoolError)` - Transaction violates chain limits
    pub fn check_chain_limits(
        &self,
        tx_hash: [u8; 32],
        tx_size: usize,
        parent_hashes: &[[u8; 32]],
    ) -> Result<(), MempoolError> {
        // Calculate ancestor count and size
        let (ancestor_count, ancestor_size) = self.calculate_ancestors(parent_hashes);

        // Check ancestor count limit
        if ancestor_count > self.config.max_ancestors {
            warn!(
                "Transaction {:02x}... has {} ancestors (limit: {})",
                tx_hash[0], ancestor_count, self.config.max_ancestors
            );
            return Err(MempoolError::AncestorChainTooLong {
                count: ancestor_count,
                limit: self.config.max_ancestors,
            });
        }

        // Check ancestor size limit (including this transaction)
        let total_ancestor_size = ancestor_size + tx_size;
        if total_ancestor_size > self.config.max_ancestor_size_bytes {
            warn!(
                "Transaction {:02x}... ancestor chain size {} bytes (limit: {} bytes)",
                tx_hash[0], total_ancestor_size, self.config.max_ancestor_size_bytes
            );
            return Err(MempoolError::AncestorSizeTooLarge {
                size: total_ancestor_size,
                limit: self.config.max_ancestor_size_bytes,
            });
        }

        // Check if adding this transaction would cause any ancestor to exceed descendant limits
        for parent_hash in parent_hashes {
            if let Some(parent_info) = self.tx_info.get(parent_hash) {
                let (desc_count, desc_size) = self.calculate_descendants(&parent_info.tx_hash);
                
                // Adding this tx would increase descendant count by 1
                if desc_count + 1 > self.config.max_descendants {
                    warn!(
                        "Adding tx {:02x}... would give parent {:02x}... {} descendants (limit: {})",
                        tx_hash[0], parent_hash[0], desc_count + 1, self.config.max_descendants
                    );
                    return Err(MempoolError::DescendantChainTooLong {
                        count: desc_count + 1,
                        limit: self.config.max_descendants,
                    });
                }

                // Adding this tx would increase descendant size
                if desc_size + tx_size > self.config.max_descendant_size_bytes {
                    warn!(
                        "Adding tx {:02x}... would give parent {:02x}... {} bytes descendants (limit: {})",
                        tx_hash[0], parent_hash[0], desc_size + tx_size, self.config.max_descendant_size_bytes
                    );
                    return Err(MempoolError::DescendantSizeTooLarge {
                        size: desc_size + tx_size,
                        limit: self.config.max_descendant_size_bytes,
                    });
                }
            }
        }

        Ok(())
    }

    /// Register a transaction with the chain tracker
    ///
    /// Call this after successfully adding a transaction to the mempool.
    pub fn register_transaction(
        &self,
        tx_hash: [u8; 32],
        tx_size: usize,
        parent_hashes: &[[u8; 32]],
    ) {
        let mut info = TxChainInfo::new(tx_hash, tx_size);

        // Add parent relationships
        for parent_hash in parent_hashes {
            if self.tx_info.contains_key(parent_hash) {
                info.parents.insert(*parent_hash);
                
                // Update parent's children
                if let Some(mut parent_info) = self.tx_info.get_mut(parent_hash) {
                    parent_info.children.insert(tx_hash);
                }
            }
        }

        self.tx_info.insert(tx_hash, info);
    }

    /// Unregister a transaction from the chain tracker
    ///
    /// Call this when removing a transaction from the mempool.
    pub fn unregister_transaction(&self, tx_hash: &[u8; 32]) {
        if let Some((_, info)) = self.tx_info.remove(tx_hash) {
            // Remove from parents' children lists
            for parent_hash in &info.parents {
                if let Some(mut parent_info) = self.tx_info.get_mut(parent_hash) {
                    parent_info.children.remove(tx_hash);
                }
            }

            // Remove from children's parents lists
            for child_hash in &info.children {
                if let Some(mut child_info) = self.tx_info.get_mut(child_hash) {
                    child_info.parents.remove(tx_hash);
                }
            }
        }
    }

    /// Calculate ancestor count and total size for a set of parent transactions
    fn calculate_ancestors(&self, parent_hashes: &[[u8; 32]]) -> (usize, usize) {
        let mut visited = HashSet::new();
        let mut total_size = 0;
        let mut queue = VecDeque::new();

        // Start with direct parents
        for parent_hash in parent_hashes {
            if self.tx_info.contains_key(parent_hash) && !visited.contains(parent_hash) {
                queue.push_back(*parent_hash);
                visited.insert(*parent_hash);
            }
        }

        // BFS to find all ancestors
        while let Some(current_hash) = queue.pop_front() {
            if let Some(info) = self.tx_info.get(&current_hash) {
                total_size += info.size;
                
                for grandparent_hash in &info.parents {
                    if !visited.contains(grandparent_hash) {
                        visited.insert(*grandparent_hash);
                        queue.push_back(*grandparent_hash);
                    }
                }
            }
        }

        (visited.len(), total_size)
    }

    /// Calculate descendant count and total size for a transaction
    fn calculate_descendants(&self, tx_hash: &[u8; 32]) -> (usize, usize) {
        let mut visited = HashSet::new();
        let mut total_size = 0;
        let mut queue = VecDeque::new();

        // Start with direct children
        if let Some(info) = self.tx_info.get(tx_hash) {
            for child_hash in &info.children {
                if !visited.contains(child_hash) {
                    queue.push_back(*child_hash);
                    visited.insert(*child_hash);
                }
            }
        }

        // BFS to find all descendants
        while let Some(current_hash) = queue.pop_front() {
            if let Some(info) = self.tx_info.get(&current_hash) {
                total_size += info.size;
                
                for grandchild_hash in &info.children {
                    if !visited.contains(grandchild_hash) {
                        visited.insert(*grandchild_hash);
                        queue.push_back(*grandchild_hash);
                    }
                }
            }
        }

        (visited.len(), total_size)
    }

    /// Get all descendants of a transaction (for RBF eviction)
    pub fn get_all_descendants(&self, tx_hash: &[u8; 32]) -> Vec<[u8; 32]> {
        let mut visited = HashSet::new();
        let mut queue = VecDeque::new();
        let mut descendants = Vec::new();

        // Start with direct children
        if let Some(info) = self.tx_info.get(tx_hash) {
            for child_hash in &info.children {
                if !visited.contains(child_hash) {
                    queue.push_back(*child_hash);
                    visited.insert(*child_hash);
                }
            }
        }

        // BFS to collect all descendants
        while let Some(current_hash) = queue.pop_front() {
            descendants.push(current_hash);
            
            if let Some(info) = self.tx_info.get(&current_hash) {
                for grandchild_hash in &info.children {
                    if !visited.contains(grandchild_hash) {
                        visited.insert(*grandchild_hash);
                        queue.push_back(*grandchild_hash);
                    }
                }
            }
        }

        descendants
    }

    /// Check if RBF replacement would evict too many transactions
    pub fn check_rbf_eviction_count(&self, conflicting_txs: &[[u8; 32]]) -> Result<(), MempoolError> {
        let mut total_evictions = conflicting_txs.len();
        
        // Count descendants of all conflicting transactions
        for tx_hash in conflicting_txs {
            let descendants = self.get_all_descendants(tx_hash);
            total_evictions += descendants.len();
        }

        if total_evictions > self.config.max_rbf_evictions {
            return Err(MempoolError::RbfTooManyEvictions {
                count: total_evictions,
                limit: self.config.max_rbf_evictions,
            });
        }

        Ok(())
    }

    /// Get chain statistics for a transaction
    pub fn get_chain_stats(&self, tx_hash: &[u8; 32]) -> Option<ChainStats> {
        if let Some(info) = self.tx_info.get(tx_hash) {
            let (ancestor_count, ancestor_size) = self.calculate_ancestors(
                &info.parents.iter().cloned().collect::<Vec<_>>()
            );
            let (descendant_count, descendant_size) = self.calculate_descendants(tx_hash);
            
            Some(ChainStats {
                ancestor_count,
                ancestor_size,
                descendant_count,
                descendant_size,
            })
        } else {
            None
        }
    }

    /// Clear all tracking data
    pub fn clear(&self) {
        self.tx_info.clear();
    }

    /// Get number of tracked transactions
    pub fn len(&self) -> usize {
        self.tx_info.len()
    }

    /// Check if tracker is empty
    pub fn is_empty(&self) -> bool {
        self.tx_info.is_empty()
    }
}

/// Statistics about a transaction's chain
#[derive(Debug, Clone)]
pub struct ChainStats {
    pub ancestor_count: usize,
    pub ancestor_size: usize,
    pub descendant_count: usize,
    pub descendant_size: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chain_limits_basic() {
        let tracker = ChainLimitsTracker::with_defaults();
        
        // Register a parent transaction
        let parent_hash = [1u8; 32];
        tracker.register_transaction(parent_hash, 250, &[]);
        
        // Child should pass limits
        let child_hash = [2u8; 32];
        assert!(tracker.check_chain_limits(child_hash, 250, &[parent_hash]).is_ok());
        tracker.register_transaction(child_hash, 250, &[parent_hash]);
        
        // Verify relationship
        assert_eq!(tracker.tx_info.get(&parent_hash).unwrap().children.len(), 1);
        assert_eq!(tracker.tx_info.get(&child_hash).unwrap().parents.len(), 1);
    }

    #[test]
    fn test_ancestor_count_limit() {
        let config = ChainLimitsConfig {
            max_ancestors: 3,
            ..Default::default()
        };
        let tracker = ChainLimitsTracker::new(config);
        
        // Create a chain of 4 transactions
        let tx1 = [1u8; 32];
        let tx2 = [2u8; 32];
        let tx3 = [3u8; 32];
        let tx4 = [4u8; 32];
        let tx5 = [5u8; 32];
        
        tracker.register_transaction(tx1, 100, &[]);
        tracker.register_transaction(tx2, 100, &[tx1]);
        tracker.register_transaction(tx3, 100, &[tx2]);
        tracker.register_transaction(tx4, 100, &[tx3]);
        
        // tx5 would have 4 ancestors, exceeding limit of 3
        let result = tracker.check_chain_limits(tx5, 100, &[tx4]);
        assert!(matches!(result, Err(MempoolError::AncestorChainTooLong { .. })));
    }

    #[test]
    fn test_descendant_count_limit() {
        let config = ChainLimitsConfig {
            max_descendants: 2,
            ..Default::default()
        };
        let tracker = ChainLimitsTracker::new(config);
        
        // Create a chain: tx1 -> tx2 -> tx3
        let tx1 = [1u8; 32];
        let tx2 = [2u8; 32];
        let tx3 = [3u8; 32];
        let tx4 = [4u8; 32];
        
        tracker.register_transaction(tx1, 100, &[]);
        tracker.register_transaction(tx2, 100, &[tx1]);
        tracker.register_transaction(tx3, 100, &[tx2]);
        
        // tx4 spending from tx1 would give tx1 3 descendants (tx2, tx3, tx4)
        // but limit is 2
        let result = tracker.check_chain_limits(tx4, 100, &[tx1]);
        assert!(matches!(result, Err(MempoolError::DescendantChainTooLong { .. })));
    }

    #[test]
    fn test_ancestor_size_limit() {
        let config = ChainLimitsConfig {
            max_ancestor_size_bytes: 500,
            ..Default::default()
        };
        let tracker = ChainLimitsTracker::new(config);
        
        let tx1 = [1u8; 32];
        let tx2 = [2u8; 32];
        let tx3 = [3u8; 32];
        
        // Total ancestor size would be 200 + 200 + 200 = 600 > 500
        tracker.register_transaction(tx1, 200, &[]);
        tracker.register_transaction(tx2, 200, &[tx1]);
        
        let result = tracker.check_chain_limits(tx3, 200, &[tx2]);
        assert!(matches!(result, Err(MempoolError::AncestorSizeTooLarge { .. })));
    }

    #[test]
    fn test_unregister_transaction() {
        let tracker = ChainLimitsTracker::with_defaults();
        
        let tx1 = [1u8; 32];
        let tx2 = [2u8; 32];
        
        tracker.register_transaction(tx1, 100, &[]);
        tracker.register_transaction(tx2, 100, &[tx1]);
        
        // Remove tx2
        tracker.unregister_transaction(&tx2);
        
        // tx1 should no longer have tx2 as child
        assert!(tracker.tx_info.get(&tx1).unwrap().children.is_empty());
        assert!(!tracker.tx_info.contains_key(&tx2));
    }

    #[test]
    fn test_get_all_descendants() {
        let tracker = ChainLimitsTracker::with_defaults();
        
        // Create tree:
        //       tx1
        //      /   \
        //    tx2   tx3
        //    /
        //  tx4
        let tx1 = [1u8; 32];
        let tx2 = [2u8; 32];
        let tx3 = [3u8; 32];
        let tx4 = [4u8; 32];
        
        tracker.register_transaction(tx1, 100, &[]);
        tracker.register_transaction(tx2, 100, &[tx1]);
        tracker.register_transaction(tx3, 100, &[tx1]);
        tracker.register_transaction(tx4, 100, &[tx2]);
        
        let descendants = tracker.get_all_descendants(&tx1);
        assert_eq!(descendants.len(), 3); // tx2, tx3, tx4
    }

    #[test]
    fn test_rbf_eviction_limit() {
        let config = ChainLimitsConfig {
            max_rbf_evictions: 3,
            ..Default::default()
        };
        let tracker = ChainLimitsTracker::new(config);
        
        // Create: tx1 -> tx2 -> tx3 -> tx4
        let tx1 = [1u8; 32];
        let tx2 = [2u8; 32];
        let tx3 = [3u8; 32];
        let tx4 = [4u8; 32];
        
        tracker.register_transaction(tx1, 100, &[]);
        tracker.register_transaction(tx2, 100, &[tx1]);
        tracker.register_transaction(tx3, 100, &[tx2]);
        tracker.register_transaction(tx4, 100, &[tx3]);
        
        // Replacing tx1 would evict tx1, tx2, tx3, tx4 = 4 transactions > limit of 3
        let result = tracker.check_rbf_eviction_count(&[tx1]);
        assert!(matches!(result, Err(MempoolError::RbfTooManyEvictions { .. })));
        
        // Replacing tx3 would evict tx3, tx4 = 2 transactions <= limit of 3
        assert!(tracker.check_rbf_eviction_count(&[tx3]).is_ok());
    }
}

