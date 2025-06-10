// Wallet module
// Re-exports wallet functionality for backwards compatibility

pub use crate::lightning::wallet::{
    LightningWallet,
    WalletError,
    KeyDerivation,
};

// Basic wallet types for non-Lightning usage
// These types are defined in crypto module but not exposed directly
// For now, we'll create simple type aliases

pub type PublicKey = Vec<u8>;
pub type PrivateKey = Vec<u8>;

pub struct KeyPair {
    pub public: PublicKey,
    pub private: PrivateKey,
} 