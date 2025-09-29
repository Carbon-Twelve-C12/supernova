//! Common test utilities and imports for the btclib crate
//! This module provides a centralized location for all commonly used test imports

#![cfg(test)]

// Re-export commonly used test types
pub use std::collections::HashMap;
pub use std::sync::Arc;
pub use chrono::{DateTime, Utc};

// Environmental types
pub use crate::environmental::types::{
    Region, HardwareType, EnergySource, EmissionsFactorType,
    EmissionsDataSource,
};

// Miner reporting types
pub use crate::environmental::miner_reporting::{
    MinerEnvironmentalInfo, MinerVerificationStatus,
    VerificationInfo, RECCertificate, CarbonOffset,
};

// API types (includes MinerEmissionsData and EnvironmentalAsset)
pub use crate::environmental::api::{
    MinerEmissionsData, EnvironmentalAsset, EnvironmentalApiError,
};

// Treasury types
pub use crate::environmental::treasury::{
    TreasuryConfig, TreasuryAllocation, EnvironmentalAssetPurchase,
    TreasuryAccountType, EnvironmentalAssetType,
};

// Emissions types  
pub use crate::environmental::emissions::{
    NetworkEmissions, EmissionsCalculator, EmissionsTracker,
};

// Signature and crypto types
pub use crate::crypto::signature::{
    SignatureType, SignatureScheme, SignatureError, SignatureVerifier,
};

// Transaction types
pub use crate::types::transaction::{
    Transaction, TransactionInput, TransactionOutput,
    TransactionSignatureData, SignatureSchemeType,
};

// Block types
pub use crate::types::block::{
    Block, BlockHeader,
};

// Additional utility types often used in tests
pub use crate::error::SupernovaError;
pub use crate::types::units::{Amount, FeeRate, NovaUnit};

// Test-specific utilities
pub mod prelude {
    pub use super::*;
    
    /// Create a test coinbase transaction
    pub fn create_test_coinbase(height: u64) -> Transaction {
        let input = TransactionInput::new_coinbase(height.to_le_bytes().to_vec());
        Transaction::new(
            1,
            vec![input],
            vec![TransactionOutput::new(50_000_000_000, vec![])], // 50 NOVA
            0,
        )
    }
    
    /// Create a test block with specified parameters
    pub fn create_test_block_with_coinbase(
        height: u64,
        prev_hash: [u8; 32],
        _timestamp: u64,
    ) -> Block {
        let coinbase = create_test_coinbase(height);
        Block::new_with_params(height as u32, prev_hash, vec![coinbase], 0x207fffff)
    }
}
