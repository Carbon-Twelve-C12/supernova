//! Native atomic swap implementation between Bitcoin and Supernova blockchains
//!
//! This module provides trustless peer-to-peer atomic swaps using Hash Time-Locked
//! Contracts (HTLCs) with quantum-resistant cryptography on the Supernova side
//! while maintaining compatibility with Bitcoin's Script language.

pub mod api;
pub mod bitcoin_adapter;
pub mod cache;
pub mod crypto;
pub mod error;
pub mod htlc;
pub mod metrics;
pub mod monitor;
pub mod websocket;

// Privacy features - Phase 4
#[cfg(feature = "atomic-swap")]
pub mod confidential;
#[cfg(feature = "atomic-swap")]
pub mod zk_swap;

// Test modules
// #[cfg(test)]
// mod tests;

pub use api::AtomicSwapRPC;
pub use cache::{AtomicSwapCache, CacheConfig};
pub use error::{AtomicSwapError, HTLCError, SwapError};
pub use htlc::{HTLCState, ParticipantInfo, SupernovaHTLC, TimeLock};
pub use metrics::init_metrics;
pub use monitor::{CrossChainMonitor, SwapSummary};

use crate::crypto::{MLDSAPublicKey, MLDSASignature};
use serde::{Deserialize, Serialize};

/// Configuration for atomic swap operations
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AtomicSwapConfig {
    /// Bitcoin network settings
    pub bitcoin_network: String,
    pub bitcoin_rpc_url: String,
    pub bitcoin_rpc_user: Option<String>,
    pub bitcoin_rpc_pass: Option<String>,

    /// Security settings
    pub min_btc_confirmations: u32,
    pub min_nova_confirmations: u32,
    pub timeout_delta: u32,
    pub refund_grace_period: u32,

    /// Amount limits
    pub min_swap_amount_btc: u64,
    pub max_swap_amount_btc: u64,

    /// Rate limiting
    pub max_swaps_per_hour: u32,
    pub max_swaps_per_address: u32,
}

impl Default for AtomicSwapConfig {
    fn default() -> Self {
        Self {
            bitcoin_network: "testnet".to_string(),
            bitcoin_rpc_url: "http://localhost:8332".to_string(),
            bitcoin_rpc_user: None,
            bitcoin_rpc_pass: None,
            min_btc_confirmations: 6,
            min_nova_confirmations: 60,
            timeout_delta: 144, // ~24 hours in Bitcoin blocks
            refund_grace_period: 6,
            min_swap_amount_btc: 10_000,        // 0.0001 BTC
            max_swap_amount_btc: 1_000_000_000, // 10 BTC
            max_swaps_per_hour: 100,
            max_swaps_per_address: 10,
        }
    }
}

/// Setup parameters for initiating an atomic swap
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AtomicSwapSetup {
    pub swap_id: [u8; 32],
    pub bitcoin_amount: u64,
    pub nova_amount: u64,
    pub fee_distribution: FeeDistribution,
    pub timeout_blocks: TimeoutConfig,
}

/// Fee distribution between participants
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FeeDistribution {
    pub bitcoin_fee_payer: FeePayer,
    pub nova_fee_payer: FeePayer,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum FeePayer {
    Sender,
    Recipient,
    Split(u8), // Percentage paid by sender (0-100)
}

/// Timeout configuration for the swap
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct TimeoutConfig {
    pub bitcoin_claim_timeout: u32,   // In Bitcoin blocks
    pub supernova_claim_timeout: u32, // In Supernova blocks
    pub refund_safety_margin: u32,    // Additional blocks for safety
}

/// Active swap session tracking
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SwapSession {
    pub setup: AtomicSwapSetup,
    pub secret: Option<[u8; 32]>,
    pub nova_htlc: SupernovaHTLC,
    pub btc_htlc: BitcoinHTLCReference,
    pub state: SwapState,
    pub created_at: u64,
    pub updated_at: u64,
}

/// Reference to a Bitcoin HTLC
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BitcoinHTLCReference {
    pub txid: String,
    pub vout: u32,
    pub script_pubkey: Vec<u8>,
    pub amount: u64,
    pub timeout_height: u32,
    pub address: String,
}

/// Current state of the swap
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum SwapState {
    /// Initial state, setting up parameters
    Initializing,
    /// Supernova HTLC created and funded
    NovaFunded,
    /// Bitcoin HTLC created and funded
    BothFunded,
    /// Swap is active and can be claimed
    Active,
    /// Bitcoin has been claimed, secret revealed
    BitcoinClaimed,
    /// Both sides have been claimed successfully
    Completed,
    /// Bitcoin side has been claimed
    Claimed,
    /// One or both sides have been refunded
    Refunded,
    /// Swap failed or was cancelled
    Failed(String),
}

/// Result of a completed swap
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SwapCompletion {
    pub btc_claim_tx: String,
    pub nova_claim_tx: String,
    pub execution_time: std::time::Duration,
}

/// Result of swap operations
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum SwapResult {
    Success(SwapCompletion),
    Refunded(RefundResult),
    Failed(String),
}

/// Information about refunded swaps
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct RefundResult {
    pub bitcoin_refunded: bool,
    pub nova_refunded: bool,
    pub reason: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = AtomicSwapConfig::default();
        assert_eq!(config.bitcoin_network, "testnet");
        assert_eq!(config.min_btc_confirmations, 6);
        assert_eq!(config.min_nova_confirmations, 60);
    }

    #[test]
    fn test_swap_state_transitions() {
        let state = SwapState::Initializing;
        assert_eq!(state, SwapState::Initializing);

        let state = SwapState::Failed("test error".to_string());
        match state {
            SwapState::Failed(msg) => assert_eq!(msg, "test error"),
            _ => panic!("Expected Failed state"),
        }
    }
}
