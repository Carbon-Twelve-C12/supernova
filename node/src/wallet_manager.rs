// Wallet Manager for Node
// Integrates quantum wallet with blockchain state

use std::path::PathBuf;
use std::sync::{Arc, RwLock};
use thiserror::Error;
use wallet::quantum_wallet::{
    Address, Keystore, UtxoIndex, WalletStorage, TransactionBuilder, BuilderConfig,
    Utxo, CoinSelectionStrategy,
};

use crate::storage::BlockchainDB;
use crate::storage::ChainState;

#[derive(Error, Debug)]
pub enum WalletManagerError {
    #[error("Wallet not initialized")]
    NotInitialized,
    
    #[error("Wallet locked")]
    WalletLocked,
    
    #[error("Storage error: {0}")]
    StorageError(String),
    
    #[error("Keystore error: {0}")]
    KeystoreError(String),
    
    #[error("Transaction error: {0}")]
    TransactionError(String),
    
    #[error("UTXO error: {0}")]
    UtxoError(String),
    
    #[error("Blockchain error: {0}")]
    BlockchainError(String),
}

/// Wallet manager integrating quantum wallet with blockchain
pub struct WalletManager {
    /// Wallet storage
    storage: Arc<RwLock<WalletStorage>>,
    
    /// Keystore
    keystore: Arc<Keystore>,
    
    /// UTXO index
    utxo_index: Arc<UtxoIndex>,
    
    /// Blockchain database access
    db: Arc<BlockchainDB>,
    
    /// Chain state access
    chain_state: Arc<RwLock<ChainState>>,
}

impl WalletManager {
    /// Create new wallet manager (unlocked for testnet)
    pub fn new(
        wallet_path: PathBuf,
        db: Arc<BlockchainDB>,
        chain_state: Arc<RwLock<ChainState>>,
    ) -> Result<Self, WalletManagerError> {
        // Open wallet storage
        let mut storage = WalletStorage::open(wallet_path)
            .map_err(|e| WalletManagerError::StorageError(e.to_string()))?;
        
        // Create and initialize keystore
        let mut keystore = Keystore::new();
        
        // Auto-initialize for testnet (in production, this would require user passphrase)
        keystore.initialize("testnet_default_passphrase")
            .map_err(|e| WalletManagerError::KeystoreError(e.to_string()))?;
        
        // Unlock storage
        storage.unlock("testnet_default_passphrase")
            .map_err(|e| WalletManagerError::StorageError(e.to_string()))?;
        
        // Create UTXO index
        let utxo_index = UtxoIndex::new();
        
        // Load existing addresses from storage
        let addresses = storage.list_addresses()
            .unwrap_or_default();
        
        tracing::info!("Wallet loaded with {} existing addresses", addresses.len());
        
        // Load existing keypairs into keystore
        for address in &addresses {
            if let Ok(keypair) = storage.load_keypair(address) {
                // Re-add to keystore's in-memory index
                if let Err(e) = keystore.load_keypair(address.clone(), keypair) {
                    tracing::warn!("Failed to load keypair for {}: {}", address, e);
                } else {
                    tracing::debug!("Loaded keypair for address: {}", address);
                }
            }
        }
        
        // Load existing UTXOs from storage
        if let Ok(utxos) = storage.list_utxos() {
            for utxo in utxos {
                if let Err(e) = utxo_index.add_utxo(utxo) {
                    tracing::warn!("Failed to load UTXO: {}", e);
                }
            }
            tracing::info!("Loaded {} UTXOs from storage", utxo_index.total_utxos());
        }
        
        Ok(Self {
            storage: Arc::new(RwLock::new(storage)),
            keystore: Arc::new(keystore),
            utxo_index: Arc::new(utxo_index),
            db,
            chain_state,
        })
    }
    
    /// Initialize wallet with passphrase (not needed for already initialized keystore)
    /// Use unlock() instead for normal operations
    pub fn unlock(&self, passphrase: &str) -> Result<(), WalletManagerError> {
        // Unlock keystore
        self.keystore.unlock(passphrase)
            .map_err(|e| WalletManagerError::KeystoreError(e.to_string()))?;
        
        // Unlock storage
        self.storage.write()
            .map_err(|_| WalletManagerError::StorageError("Lock poisoned".to_string()))?
            .unlock(passphrase)
            .map_err(|e| WalletManagerError::StorageError(e.to_string()))?;
        
        Ok(())
    }
    
    /// Generate new address
    pub fn generate_new_address(&self, label: Option<String>) -> Result<String, WalletManagerError> {
        if self.keystore.is_locked() {
            return Err(WalletManagerError::WalletLocked);
        }
        
        // Generate address
        let address = self.keystore.generate_address(label.clone())
            .map_err(|e| WalletManagerError::KeystoreError(e.to_string()))?;
        
        // Get keypair
        let keypair = self.keystore.get_keypair(&address.to_string())
            .map_err(|e| WalletManagerError::KeystoreError(e.to_string()))?;
        
        // Store keypair
        self.storage.read()
            .map_err(|_| WalletManagerError::StorageError("Lock poisoned".to_string()))?
            .store_keypair(&address.to_string(), &keypair)
            .map_err(|e| WalletManagerError::StorageError(e.to_string()))?;
        
        Ok(address.to_string())
    }
    
    /// Get balance with minimum confirmations
    pub fn get_balance(&self, min_confirmations: u64) -> Result<u64, WalletManagerError> {
        // Get all addresses
        let addresses = self.keystore.list_addresses()
            .map_err(|e| WalletManagerError::KeystoreError(e.to_string()))?;
        
        // Calculate total balance
        self.utxo_index.get_total_balance(&addresses, min_confirmations, false)
            .map_err(|e| WalletManagerError::UtxoError(e.to_string()))
    }
    
    /// List unspent outputs
    pub fn list_unspent(
        &self,
        min_conf: u64,
        max_conf: u64,
        addresses: Option<Vec<String>>,
    ) -> Result<Vec<Utxo>, WalletManagerError> {
        self.utxo_index.list_unspent(
            min_conf,
            max_conf,
            addresses.as_deref(),
        ).map_err(|e| WalletManagerError::UtxoError(e.to_string()))
    }
    
    /// Sync wallet with blockchain
    pub fn sync_with_blockchain(&self) -> Result<(), WalletManagerError> {
        // Get current blockchain height
        let chain_state = self.chain_state.read()
            .map_err(|_| WalletManagerError::BlockchainError("Chain state lock poisoned".to_string()))?;
        
        let current_height = chain_state.get_height();
        
        // Update UTXO index height
        self.utxo_index.update_height(current_height)
            .map_err(|e| WalletManagerError::UtxoError(e.to_string()))?;
        
        // TODO: Scan blockchain for wallet transactions and update UTXO index
        // This is a placeholder - full implementation would:
        // 1. Get all wallet addresses
        // 2. Scan blocks for transactions to/from wallet addresses
        // 3. Update UTXO index with found UTXOs
        // 4. Mark spent outputs
        
        Ok(())
    }
    
    /// Get keystore reference
    pub fn keystore(&self) -> Arc<Keystore> {
        Arc::clone(&self.keystore)
    }
    
    /// Get UTXO index reference
    pub fn utxo_index(&self) -> Arc<UtxoIndex> {
        Arc::clone(&self.utxo_index)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    
    // Tests would go here but require full blockchain context
}

