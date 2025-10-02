// Quantum-Resistant Wallet Implementation for Supernova Blockchain
// Implements post-quantum cryptography using ML-DSA (Dilithium)

pub mod keystore;
pub mod storage;
pub mod utxo_index;
pub mod transaction_builder;
pub mod address;

// Re-exports
pub use keystore::{Keystore, KeyPair, KeystoreError};
pub use storage::{WalletStorage, StorageError};
pub use utxo_index::{UtxoIndex, Utxo, UtxoError};
pub use transaction_builder::{TransactionBuilder, TransactionError, BuilderConfig, CoinSelectionStrategy};
pub use address::{Address, AddressType, AddressError};

