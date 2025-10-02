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
use crate::mempool::TransactionPool;
use btclib::types::transaction::Transaction;

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
    
    /// Transaction mempool
    mempool: Arc<TransactionPool>,
}

impl WalletManager {
    /// Create new wallet manager (unlocked for testnet)
    pub fn new(
        wallet_path: PathBuf,
        db: Arc<BlockchainDB>,
        chain_state: Arc<RwLock<ChainState>>,
        mempool: Arc<TransactionPool>,
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
            mempool,
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
    
    /// Scan a block for transactions relevant to wallet
    pub fn scan_block(&self, block: &btclib::types::block::Block) -> Result<(), WalletManagerError> {
        // Get all wallet addresses
        let addresses = self.keystore.list_addresses()
            .map_err(|e| WalletManagerError::KeystoreError(e.to_string()))?;
        
        if addresses.is_empty() {
            return Ok(()); // No addresses to scan for
        }
        
        let block_height = block.height();
        let block_hash = block.hash();
        
        tracing::debug!("Scanning block {} at height {} for wallet transactions", 
            hex::encode(&block_hash[..8]), block_height);
        
        // Scan all transactions in the block
        for tx in block.transactions() {
            self.scan_transaction(tx, block_height, &addresses)?;
        }
        
        // Update UTXO index height for confirmation calculation
        self.utxo_index.update_height(block_height)
            .map_err(|e| WalletManagerError::UtxoError(e.to_string()))?;
        
        Ok(())
    }
    
    /// Scan a single transaction for wallet-relevant outputs
    fn scan_transaction(
        &self,
        tx: &btclib::types::transaction::Transaction,
        block_height: u64,
        wallet_addresses: &[String],
    ) -> Result<(), WalletManagerError> {
        let tx_hash = tx.hash();
        
        // Check outputs for any to our addresses
        for (vout, output) in tx.outputs().iter().enumerate() {
            // Try to match output script to wallet addresses
            // For now, simplified matching by script_pubkey
            let script_pubkey = output.script_pubkey();
            
            // Check if this output belongs to any of our addresses
            for wallet_addr in wallet_addresses {
                // Parse wallet address to get pubkey hash
                if let Ok(addr) = wallet::quantum_wallet::Address::from_str(wallet_addr) {
                    // Check if output script matches address
                    if script_pubkey == addr.pubkey_hash() {
                        // This output is ours!
                        let utxo = Utxo {
                            txid: tx_hash,
                            vout: vout as u32,
                            address: wallet_addr.clone(),
                            value: output.value(),
                            script_pubkey: script_pubkey.to_vec(),
                            block_height,
                            confirmations: 1, // Will be updated
                            spendable: true,
                            solvable: true,
                            label: None,
                        };
                        
                        // Add UTXO to index
                        self.utxo_index.add_utxo(utxo.clone())
                            .map_err(|e| WalletManagerError::UtxoError(e.to_string()))?;
                        
                        // Save UTXO to storage
                        self.storage.read()
                            .map_err(|_| WalletManagerError::StorageError("Lock poisoned".to_string()))?
                            .store_utxo(&utxo)
                            .map_err(|e| WalletManagerError::StorageError(e.to_string()))?;
                        
                        tracing::info!("Found wallet UTXO: {} NOVA at {}:{}",
                            utxo.value as f64 / 100_000_000.0,
                            hex::encode(&tx_hash[..8]),
                            vout
                        );
                    }
                }
            }
        }
        
        // Check inputs to mark spent UTXOs
        for input in tx.inputs() {
            let prev_txid = input.prev_tx_hash();
            let prev_vout = input.prev_output_index();
            
            // Check if this spends one of our UTXOs
            if !self.utxo_index.is_spent(&prev_txid, prev_vout) {
                if let Ok(_utxo) = self.utxo_index.get_utxo(&prev_txid, prev_vout) {
                    // Mark as spent
                    self.utxo_index.mark_spent(&prev_txid, prev_vout)
                        .map_err(|e| WalletManagerError::UtxoError(e.to_string()))?;
                    
                    // Delete from storage
                    self.storage.read()
                        .map_err(|_| WalletManagerError::StorageError("Lock poisoned".to_string()))?
                        .delete_utxo(&prev_txid, prev_vout)
                        .map_err(|e| WalletManagerError::StorageError(e.to_string()))?;
                    
                    tracing::info!("UTXO spent: {}:{}", hex::encode(&prev_txid[..8]), prev_vout);
                }
            }
        }
        
        Ok(())
    }
    
    /// Sync wallet with blockchain (full rescan)
    pub fn sync_with_blockchain(&self) -> Result<(), WalletManagerError> {
        // Get current blockchain height
        let chain_state = self.chain_state.read()
            .map_err(|_| WalletManagerError::BlockchainError("Chain state lock poisoned".to_string()))?;
        
        let current_height = chain_state.get_height();
        
        // Update UTXO index height
        self.utxo_index.update_height(current_height)
            .map_err(|e| WalletManagerError::UtxoError(e.to_string()))?;
        
        tracing::info!("Wallet synced to height {}", current_height);
        
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
    
    /// Submit transaction to mempool
    pub fn submit_transaction_to_mempool(
        &self,
        transaction: Transaction,
    ) -> Result<[u8; 32], WalletManagerError> {
        let txid = transaction.hash();
        
        // Calculate transaction size and fee rate
        let tx_size = bincode::serialize(&transaction)
            .map_err(|e| WalletManagerError::TransactionError(format!("Serialization error: {}", e)))?
            .len();
        
        // Estimate fee rate (simplified - in production would be more sophisticated)
        let fee_rate = 1000; // 1000 attonovas per byte (matches builder config)
        
        tracing::debug!("Submitting transaction {} ({} bytes) to mempool", 
            hex::encode(&txid[..8]), tx_size);
        
        // Submit to mempool
        self.mempool.add_transaction(transaction.clone(), fee_rate)
            .map_err(|e| WalletManagerError::TransactionError(format!("Mempool rejected: {}", e)))?;
        
        tracing::info!("Transaction {} accepted to mempool", hex::encode(&txid[..8]));
        
        // Mark input UTXOs as pending spent
        for input in transaction.inputs() {
            let prev_txid = input.prev_tx_hash();
            let prev_vout = input.prev_output_index();
            
            // Mark as spent in UTXO index (will be removed when block confirms)
            if let Err(e) = self.utxo_index.mark_spent(&prev_txid, prev_vout) {
                tracing::warn!("Failed to mark UTXO as spent: {}", e);
                // Don't fail the whole transaction for this
            }
        }
        
        Ok(txid)
    }
    
    /// Add test UTXO for testing (testnet only - DO NOT USE IN PRODUCTION)
    #[cfg(feature = "testnet")]
    pub fn add_test_utxo(
        &self,
        address: &str,
        amount: u64,
        txid: [u8; 32],
        vout: u32,
    ) -> Result<(), WalletManagerError> {
        // Verify address is in wallet
        if !self.keystore.has_address(address) {
            return Err(WalletManagerError::KeystoreError(
                format!("Address {} not in wallet", address)
            ));
        }
        
        let utxo = Utxo {
            txid,
            vout,
            address: address.to_string(),
            value: amount,
            script_pubkey: vec![], // Will be filled by transaction builder
            block_height: 100, // Fake block height for testing
            confirmations: 10, // Enough confirmations for spending
            spendable: true,
            solvable: true,
            label: Some("test_utxo".to_string()),
        };
        
        // Add to UTXO index
        self.utxo_index.add_utxo(utxo.clone())
            .map_err(|e| WalletManagerError::UtxoError(e.to_string()))?;
        
        // Save to storage
        self.storage.read()
            .map_err(|_| WalletManagerError::StorageError("Lock poisoned".to_string()))?
            .store_utxo(&utxo)
            .map_err(|e| WalletManagerError::StorageError(e.to_string()))?;
        
        tracing::info!("Added test UTXO: {} NOVA to address {}", 
            amount as f64 / 100_000_000.0, address);
        
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    
    // Tests would go here but require full blockchain context
}

