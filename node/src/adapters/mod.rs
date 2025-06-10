//! Architectural Bridge Adapters
//! 
//! This module provides adapter functions and traits to bridge the API gap
//! between btclib (core library) and node (application layer).
//! These adapters ensure smooth integration without breaking either layer's API.

pub mod method_adapters;
pub mod trait_implementations;

// Re-export all adapter traits for convenient access
pub use method_adapters::{
    ChainStateNodeMethods, BlockNodeMethods, TransactionPoolNodeMethods,
    ResultNodeMethods, CloneableReadGuard, DashMapRefExt, HashMapNodeExt
};

pub use trait_implementations::{
    SerializablePeerId, serialize_peer_id, deserialize_peer_id,
    SafeNumericConversion, IVecConversion, BorrowHelper, WalletConversion,
    SerializableUtxoTransaction, SerializableFuture, safe_numeric_cast,
    ResultConversionExt
};

use btclib::storage::chain_state::{ChainState, ChainStateError};
use crate::storage::database::BlockchainDB;
use std::sync::Arc;
use tracing::{error, warn};
use std::error::Error as StdError;

/// Extension trait for ChainState to provide node-compatible methods
pub trait ChainStateNodeAdapter {
    /// Get the best block hash without Result wrapper
    fn get_best_block_hash_unwrapped(&self) -> [u8; 32];
    
    /// Get the current height as u64 without Result wrapper
    fn get_height_u64(&self) -> u64;
    
    /// Check if we have a specific block
    fn has_block(&self, hash: &[u8; 32]) -> bool;
}

impl ChainStateNodeAdapter for ChainState {
    fn get_best_block_hash_unwrapped(&self) -> [u8; 32] {
        self.get_best_block_hash()
    }
    
    fn get_height_u64(&self) -> u64 {
        self.get_best_height()
    }
    
    fn has_block(&self, hash: &[u8; 32]) -> bool {
        self.contains_block(hash)
    }
}

/// Helper functions for Result type conversions
pub mod result_adapters {
    use btclib::storage::chain_state::ChainStateError;
    
    /// Convert Result<u32, E> to u64, using default on error
    pub fn result_u32_to_u64<E>(result: Result<u32, E>, default: u64) -> u64 
    where E: std::fmt::Display 
    {
        match result {
            Ok(val) => val as u64,
            Err(e) => {
                warn!("Error converting u32 to u64: {}, using default {}", e, default);
                default
            }
        }
    }
    
    /// Convert Result<T, E> to Option<T>, logging errors
    pub fn result_to_option<T, E>(result: Result<T, E>, context: &str) -> Option<T>
    where E: std::fmt::Display
    {
        match result {
            Ok(val) => Some(val),
            Err(e) => {
                warn!("{}: {}", context, e);
                None
            }
        }
    }
    
    /// Unwrap Result<T, E> with default value, logging errors
    pub fn unwrap_or_default<T, E>(result: Result<T, E>, default: T, context: &str) -> T
    where E: std::fmt::Display
    {
        match result {
            Ok(val) => val,
            Err(e) => {
                warn!("{}: {}, using default", context, e);
                default
            }
        }
    }
}

/// Adapter for converting between different hash representations
pub mod hash_adapters {
    /// Convert a hex string to [u8; 32] hash
    pub fn hex_to_hash(hex: &str) -> Result<[u8; 32], String> {
        if hex.len() != 64 {
            return Err(format!("Invalid hex length: expected 64, got {}", hex.len()));
        }
        
        let mut hash = [0u8; 32];
        hex::decode_to_slice(hex, &mut hash)
            .map_err(|e| format!("Invalid hex: {}", e))?;
        Ok(hash)
    }
    
    /// Convert [u8; 32] hash to hex string
    pub fn hash_to_hex(hash: &[u8; 32]) -> String {
        hex::encode(hash)
    }
}

/// Adapter for block height conversions
pub mod height_adapters {
    /// Safely convert u64 to u32, clamping to u32::MAX if needed
    pub fn u64_to_u32_clamped(height: u64) -> u32 {
        if height > u32::MAX as u64 {
            warn!("Height {} exceeds u32::MAX, clamping", height);
            u32::MAX
        } else {
            height as u32
        }
    }
    
    /// Convert u32 to u64 (always safe)
    pub fn u32_to_u64(height: u32) -> u64 {
        height as u64
    }
}

/// Result type conversion utilities
pub mod result_converters {
    use super::*;
    use tracing::error;
    
    /// Convert Result<u32> to Result<u64> with error handling
    pub fn convert_result_to_u64(result: Result<u32, ChainStateError>) -> Result<u64, ChainStateError> {
        result.map(|value| value as u64)
    }
    
    /// Convert Result<u32> to Option<u64> with error logging
    pub fn convert_result_to_option(result: Result<u32, ChainStateError>) -> Option<u64> {
        match result {
            Ok(value) => Some(value as u64),
            Err(e) => {
                error!("Error converting Result to Option: {}", e);
                None
            }
        }
    }
    
    /// Safely convert u32 to u64
    pub fn safe_u32_to_u64(value: u32) -> u64 {
        value as u64
    }
    
    /// Convert u64 to u32 with overflow check
    pub fn safe_u64_to_u32(value: u64, default: u32) -> u32 {
        if value > u32::MAX as u64 {
            error!("Error converting u32 to u64: {}, using default {}", value, default);
            default
        } else {
            value as u32
        }
    }
    
    /// Convert Result<T, ChainStateError> to Result<T, NodeError>
    pub fn convert_error<T>(result: Result<T, ChainStateError>, context: &str) -> Result<T, String> {
        result.map_err(|e| {
            error!("{}: {}", context, e);
            format!("{}: {}", context, e)
        })
    }
    
    /// Convert Option<T> to Result<T, String>
    pub fn option_to_result<T>(option: Option<T>, error_msg: &str) -> Result<T, String> {
        option.ok_or_else(|| error_msg.to_string())
    }
    
    /// Convert Result with default value on error
    pub fn result_with_default<T: Default>(result: Result<T, ChainStateError>, context: &str) -> T {
        result.unwrap_or_else(|e| {
            error!("{}: {}, using default", context, e);
            T::default()
        })
    }
}

/// Error conversion adapters for bridging btclib and node error types
pub mod error_adapters {
    use btclib::storage::chain_state::ChainStateError;
    use crate::storage::database::StorageError;
    use crate::api::ApiError;
    use std::fmt;
    use std::error::Error as StdError;
    
    /// Convert ChainStateError to StorageError
    impl From<ChainStateError> for StorageError {
        fn from(err: ChainStateError) -> Self {
            match err {
                ChainStateError::BlockNotFound(hash) => StorageError::KeyNotFound(format!("Block not found: {}", hash)),
                ChainStateError::BlockAlreadyExists(hash) => StorageError::DatabaseError(format!("Block already exists: {}", hash)),
                ChainStateError::InvalidBlock(msg) => StorageError::InvalidBlock,
                ChainStateError::StorageError(msg) => StorageError::DatabaseError(msg),
                ChainStateError::UtxoError(msg) => StorageError::DatabaseError(format!("UTXO error: {}", msg)),
                _ => StorageError::DatabaseError(format!("Chain state error: {}", err)),
            }
        }
    }
    
    /// Convert StorageError to ApiError
    impl From<StorageError> for ApiError {
        fn from(err: StorageError) -> Self {
            ApiError::internal_error(format!("Storage error: {}", err))
        }
    }
    
    /// Convert ChainStateError to ApiError
    impl From<ChainStateError> for ApiError {
        fn from(err: ChainStateError) -> Self {
            ApiError::internal_error(format!("Chain state error: {}", err))
        }
    }
    
    /// Convert Box<dyn Error> to ApiError
    impl From<Box<dyn StdError>> for ApiError {
        fn from(err: Box<dyn StdError>) -> Self {
            ApiError::internal_error(format!("Error: {}", err))
        }
    }
    
    /// Convert Box<dyn Error + Send + Sync> to ApiError
    impl From<Box<dyn StdError + Send + Sync>> for ApiError {
        fn from(err: Box<dyn StdError + Send + Sync>) -> Self {
            ApiError::internal_error(format!("Error: {}", err))
        }
    }
    
    /// Generic error conversion helper
    pub fn convert_error<E, T>(result: Result<T, E>) -> Result<T, StorageError>
    where 
        E: fmt::Display
    {
        result.map_err(|e| StorageError::DatabaseError(e.to_string()))
    }
    
    /// Convert any error to ApiError
    pub fn to_api_error<E: fmt::Display>(err: E) -> ApiError {
        ApiError::internal_error(err.to_string())
    }
    
    /// Handle error logging and conversion
    pub fn log_and_convert_error<E: StdError>(error: E, context: &str) -> String {
        error!("{}: {}", context, error);
        format!("{}: {}", context, error)
    }
}

/// Method extension adapters for missing methods
pub mod method_extension_adapters {
    use std::fmt;
    
    /// Extension trait to add ok_or_else to Result<T, E> where it's missing
    pub trait ResultExt<T, E> {
        fn ok_or_else_ext<F, D>(self, f: F) -> Result<T, D>
        where
            F: FnOnce() -> D;
    }
    
    impl<T, E> ResultExt<T, E> for Result<T, E> {
        fn ok_or_else_ext<F, D>(self, f: F) -> Result<T, D>
        where
            F: FnOnce() -> D
        {
            match self {
                Ok(val) => Ok(val),
                Err(_) => Err(f()),
            }
        }
    }
    
    /// Helper to unwrap Result<u32> to u64 with proper error handling
    pub fn unwrap_u32_to_u64<E: fmt::Display>(result: Result<u32, E>, context: &str) -> u64 {
        match result {
            Ok(val) => val as u64,
            Err(e) => {
                warn!("{}: {}", context, e);
                0u64
            }
        }
    }
}

/// Comparison adapters for reference vs value issues
pub mod comparison_adapters {
    /// Compare two byte arrays, handling reference/value mismatches
    pub fn compare_hashes(a: &[u8; 32], b: &[u8; 32]) -> bool {
        a == b
    }
    
    /// Compare with automatic dereferencing
    pub fn compare_hash_ref_value(a: &[u8; 32], b: [u8; 32]) -> bool {
        *a == b
    }
    
    /// Compare with automatic referencing
    pub fn compare_hash_value_ref(a: [u8; 32], b: &[u8; 32]) -> bool {
        a == *b
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_height_conversions() {
        assert_eq!(height_adapters::u64_to_u32_clamped(1000), 1000);
        assert_eq!(height_adapters::u64_to_u32_clamped(u64::MAX), u32::MAX);
        assert_eq!(height_adapters::u32_to_u64(1000), 1000u64);
    }
    
    #[test]
    fn test_hash_conversions() {
        let hash = [1u8; 32];
        let hex = hash_adapters::hash_to_hex(&hash);
        assert_eq!(hex.len(), 64);
        
        let converted = hash_adapters::hex_to_hash(&hex).unwrap();
        assert_eq!(converted, hash);
    }
} 