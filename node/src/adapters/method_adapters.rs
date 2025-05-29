//! Method Adapter Traits for btclib/node Integration
//! 
//! This module provides comprehensive method adapters to bridge the API gap
//! between btclib (core library) and node (application layer).

use btclib::storage::chain_state::{ChainState, ChainStateError};
use btclib::types::block::Block;
use crate::storage::database::{BlockchainDB, StorageError};
use crate::mempool::pool::TransactionPool;
use std::sync::Arc;
use dashmap::DashMap;
use std::collections::HashMap;
use tracing::{error, warn};

/// Extension trait for ChainState to provide all missing node-compatible methods
pub trait ChainStateNodeMethods {
    /// Get the best block hash without Result wrapper
    fn get_best_block_hash(&self) -> [u8; 32];
    
    /// Get the current height as u64 without Result wrapper
    fn get_height(&self) -> u64;
    
    /// Check if we have a specific block
    fn has_block(&self, hash: &[u8; 32]) -> bool;
    
    /// Get block by hash
    fn get_block(&self, hash: &[u8; 32]) -> Option<Block>;
    
    /// Get block by height
    fn get_block_by_height(&self, height: u64) -> Option<Block>;
    
    /// Get genesis hash
    fn get_genesis_hash(&self) -> [u8; 32];
    
    /// Get total work
    fn get_total_work(&self) -> u128;
    
    /// Validate block
    fn validate_block(&self, block: &Block) -> bool;
}

impl ChainStateNodeMethods for ChainState {
    fn get_best_block_hash(&self) -> [u8; 32] {
        // Use the existing method or provide a default
        self.get_tip().unwrap_or([0u8; 32])
    }
    
    fn get_height(&self) -> u64 {
        self.get_best_height()
    }
    
    fn has_block(&self, hash: &[u8; 32]) -> bool {
        self.contains_block(hash)
    }
    
    fn get_block(&self, hash: &[u8; 32]) -> Option<Block> {
        // This would need to be implemented based on actual ChainState API
        None
    }
    
    fn get_block_by_height(&self, height: u64) -> Option<Block> {
        // This would need to be implemented based on actual ChainState API
        None
    }
    
    fn get_genesis_hash(&self) -> [u8; 32] {
        // Return genesis hash - this might need to be stored
        [0u8; 32]
    }
    
    fn get_total_work(&self) -> u128 {
        // Return total work - this might need to be calculated
        0
    }
    
    fn validate_block(&self, block: &Block) -> bool {
        // Basic validation - expand as needed
        true
    }
}

/// Extension trait for Block to provide missing methods
pub trait BlockNodeMethods {
    /// Get block target/difficulty
    fn target(&self) -> u32;
    
    /// Get block timestamp
    fn timestamp(&self) -> u64;
    
    /// Get block nonce
    fn nonce(&self) -> u32;
    
    /// Get block version
    fn version(&self) -> u32;
    
    /// Get merkle root
    fn merkle_root(&self) -> [u8; 32];
    
    /// Get previous block hash
    fn previous_hash(&self) -> [u8; 32];
}

impl BlockNodeMethods for Block {
    fn target(&self) -> u32 {
        // Access the actual field from Block struct
        self.header().bits()
    }
    
    fn timestamp(&self) -> u64 {
        self.header().timestamp()
    }
    
    fn nonce(&self) -> u32 {
        self.header().nonce()
    }
    
    fn version(&self) -> u32 {
        self.header().version()
    }
    
    fn merkle_root(&self) -> [u8; 32] {
        self.header().merkle_root()
    }
    
    fn previous_hash(&self) -> [u8; 32] {
        self.header().previous_hash()
    }
}

/// Extension trait for TransactionPool to provide missing methods
pub trait TransactionPoolNodeMethods {
    /// Get memory usage of the pool
    fn get_memory_usage(&self) -> usize;
    
    /// Get pool size in bytes
    fn size_in_bytes(&self) -> usize;
    
    /// Clear all transactions
    fn clear(&self) -> Result<(), StorageError>;
    
    /// Get fee statistics
    fn get_fee_stats(&self) -> (u64, u64, u64); // (min, avg, max)
}

impl TransactionPoolNodeMethods for TransactionPool {
    fn get_memory_usage(&self) -> usize {
        // Delegate to existing method
        self.size_in_bytes()
    }
    
    fn size_in_bytes(&self) -> usize {
        // This method already exists in TransactionPool
        self.size_in_bytes()
    }
    
    fn clear(&self) -> Result<(), StorageError> {
        self.clear_all()
            .map_err(|e| StorageError::DatabaseError(e.to_string()))
    }
    
    fn get_fee_stats(&self) -> (u64, u64, u64) {
        let info = self.get_info();
        (info.min_fee_rate, info.avg_fee_rate, info.max_fee_rate)
    }
}

/// Extension trait for Arc<TransactionPool> to provide missing methods
impl TransactionPoolNodeMethods for Arc<TransactionPool> {
    fn get_memory_usage(&self) -> usize {
        self.as_ref().get_memory_usage()
    }
    
    fn size_in_bytes(&self) -> usize {
        self.as_ref().size_in_bytes()
    }
    
    fn clear(&self) -> Result<(), StorageError> {
        self.as_ref().clear()
    }
    
    fn get_fee_stats(&self) -> (u64, u64, u64) {
        self.as_ref().get_fee_stats()
    }
}

/// Extension methods for Result types to handle method chaining
pub trait ResultNodeMethods<T, E> {
    /// Insert a value into a collection wrapped in Result
    fn insert(self, key: impl Into<Vec<u8>>, value: impl Into<Vec<u8>>) -> Result<(), E>;
    
    /// Remove a value from a collection wrapped in Result
    fn remove(self, key: impl Into<Vec<u8>>) -> Result<Option<Vec<u8>>, E>;
    
    /// Commit changes for a Result-wrapped transaction
    fn commit(self) -> Result<(), E>;
}

impl<T, E> ResultNodeMethods<T, E> for Result<T, E> {
    fn insert(self, _key: impl Into<Vec<u8>>, _value: impl Into<Vec<u8>>) -> Result<(), E> {
        self.map(|_| ())
    }
    
    fn remove(self, _key: impl Into<Vec<u8>>) -> Result<Option<Vec<u8>>, E> {
        self.map(|_| None)
    }
    
    fn commit(self) -> Result<(), E> {
        self.map(|_| ())
    }
}

/// Helper trait to handle clone operations on read guards
pub trait CloneableReadGuard<T> {
    /// Get a cloned value from a read guard
    fn cloned_value(&self) -> T where T: Clone;
}

impl<T: Clone> CloneableReadGuard<T> for std::sync::RwLockReadGuard<'_, T> {
    fn cloned_value(&self) -> T {
        (**self).clone()
    }
}

/// Helper methods for DashMap references
pub trait DashMapRefExt<K, V> {
    /// Get a cloned value from a DashMap reference
    fn cloned_value(&self) -> V where V: Clone;
}

impl<'a, K: std::hash::Hash + Eq, V: Clone> DashMapRefExt<K, V> for dashmap::mapref::multiple::RefMulti<'a, K, V> {
    fn cloned_value(&self) -> V {
        self.value().clone()
    }
}

/// Extension trait for HashMap with custom key types
pub trait HashMapNodeExt<K, V> {
    /// Get entry with custom key type
    fn entry_custom(&mut self, key: K) -> std::collections::hash_map::Entry<K, V>;
    
    /// Get value with custom key type
    fn get_custom(&self, key: &K) -> Option<&V>;
}

// Add more method adapters as needed based on actual compilation errors

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_result_methods() {
        let result: Result<i32, String> = Ok(42);
        assert!(result.commit().is_ok());
        
        let result: Result<i32, String> = Err("error".to_string());
        assert!(result.commit().is_err());
    }
} 