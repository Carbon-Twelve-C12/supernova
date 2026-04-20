//! Governance primitives for consensus-critical parameters.
//!
//! This module holds the canonical sources for values that must be agreed
//! across the network (treasury allocation address, governance-controlled
//! multisig scripts, etc.). Anything in here is consensus — divergence
//! between miners and validators splits the chain.

pub mod treasury;

pub use treasury::{
    treasury_script_pubkey, validate_treasury_script, TreasuryError,
    MAINNET_TREASURY_PENDING_GENESIS, TESTNET_TREASURY_TAG, TREASURY_ALLOCATION_PERCENT,
    TREASURY_SCRIPT_LEN,
};
