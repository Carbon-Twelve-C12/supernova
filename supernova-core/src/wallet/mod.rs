//! Wallet module for Supernova blockchain
//!
//! This module provides wallet functionality including:
//! - Classical wallet operations
//! - Quantum-resistant wallet implementation
//! - HD wallet support
//! - Multi-signature support

pub mod quantum_wallet;

// Re-export main types
pub use quantum_wallet::{
    QuantumAddress, QuantumAddressType, QuantumWallet, WalletError, WalletMetadata,
};
