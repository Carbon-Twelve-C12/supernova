// Wallet Manager for Node
// Integrates quantum wallet with blockchain state

use std::path::PathBuf;
use std::sync::{Arc, RwLock};
use thiserror::Error;
use wallet::quantum_wallet::{
    Keystore, UtxoIndex, WalletStorage,
    Utxo,
};

use crate::config::{NetworkEnvironment, NodeConfig};
use crate::storage::BlockchainDB;
use crate::storage::ChainState;
use crate::mempool::TransactionPool;
use crate::network::NetworkProxy;
use supernova_core::types::transaction::Transaction;

/// Hardcoded fallback passphrase used when the operator hasn't supplied
/// `SUPERNOVA_WALLET_PASSPHRASE` AND the node is running in a non-production
/// environment. Anyone with read access to the keystore on disk can decrypt
/// it with this string — by design, since dev/testnet workflows must boot
/// without prompting for a passphrase. `resolve_wallet_passphrase` refuses
/// to use it on Production environments.
const TESTNET_DEFAULT_PASSPHRASE: &str = "testnet_default_passphrase";

/// Environment variable used to supply the wallet keystore passphrase.
/// Required when `[node].environment = "Production"`; optional (and
/// recommended) on testnet / development. Set this in the node operator's
/// startup environment, NOT in any committed config file.
pub const WALLET_PASSPHRASE_ENV: &str = "SUPERNOVA_WALLET_PASSPHRASE";

/// Resolve the wallet keystore passphrase for the configured environment.
///
/// Order of precedence:
/// 1. `SUPERNOVA_WALLET_PASSPHRASE` env var (always wins when non-empty).
/// 2. Hardcoded testnet/development default — only when the configured
///    `[node].environment` is `Testnet` or `Development`. A loud warning
///    is logged whenever this branch is taken.
/// 3. Refuse with `WalletManagerError::KeystoreError` when running in
///    `Production` without the env var set. Earlier revisions of this file
///    auto-unlocked the keystore with a published default passphrase on
///    every startup regardless of environment, which made the on-disk
///    Argon2id encryption decorative — anyone with shell access to the
///    wallet directory could decrypt it. Production now refuses to start
///    rather than silently degrade keystore protection.
pub fn resolve_wallet_passphrase(config: &NodeConfig) -> Result<String, WalletManagerError> {
    if let Ok(p) = std::env::var(WALLET_PASSPHRASE_ENV) {
        if !p.trim().is_empty() {
            tracing::info!(
                "Wallet keystore passphrase loaded from {} environment variable",
                WALLET_PASSPHRASE_ENV
            );
            return Ok(p);
        }
        tracing::warn!(
            "{} is set but empty/whitespace — falling through to environment-based resolution",
            WALLET_PASSPHRASE_ENV
        );
    }

    match config.node.environment {
        NetworkEnvironment::Production => Err(WalletManagerError::KeystoreError(format!(
            "{} environment variable is required when [node].environment = \"Production\". \
             Refusing to auto-unlock the wallet keystore with a published default \
             passphrase on a production node. Set the env var (>= 16 chars) and restart.",
            WALLET_PASSPHRASE_ENV
        ))),
        NetworkEnvironment::Testnet | NetworkEnvironment::Development => {
            tracing::warn!(
                "Wallet keystore auto-unlocked with the hardcoded development passphrase \
                 because {} is unset and [node].environment = {:?}. NEVER ship this to a \
                 production deployment — anyone with disk access can decrypt the keystore.",
                WALLET_PASSPHRASE_ENV,
                config.node.environment
            );
            Ok(TESTNET_DEFAULT_PASSPHRASE.to_string())
        }
    }
}

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
    
    /// Network proxy for broadcasting
    network: Arc<NetworkProxy>,
}

impl WalletManager {
    /// Create a new wallet manager.
    ///
    /// `passphrase` is used both to (re)initialise the in-memory keystore and
    /// to unlock the on-disk `WalletStorage` (Argon2id-encrypted). Callers
    /// MUST source the passphrase via `resolve_wallet_passphrase(config)`
    /// rather than hard-coding it — that helper enforces the production
    /// guardrail that refuses to fall back to the published testnet default.
    pub fn new(
        wallet_path: PathBuf,
        passphrase: &str,
        db: Arc<BlockchainDB>,
        chain_state: Arc<RwLock<ChainState>>,
        mempool: Arc<TransactionPool>,
        network: Arc<NetworkProxy>,
    ) -> Result<Self, WalletManagerError> {
        // Open wallet storage
        let mut storage = WalletStorage::open(wallet_path)
            .map_err(|e| WalletManagerError::StorageError(e.to_string()))?;

        // Create and initialise keystore with the operator-supplied passphrase.
        let mut keystore = Keystore::new();
        keystore.initialize(passphrase)
            .map_err(|e| WalletManagerError::KeystoreError(e.to_string()))?;

        // Unlock storage with the same passphrase.
        storage.unlock(passphrase)
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
            network,
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
    pub fn scan_block(&self, block: &supernova_core::types::block::Block) -> Result<(), WalletManagerError> {
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
        tx: &supernova_core::types::transaction::Transaction,
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
        
        // Broadcast transaction to P2P network
        tracing::debug!("Broadcasting transaction {} to network", hex::encode(&txid[..8]));
        self.network.broadcast_transaction(&transaction);
        tracing::info!("Transaction {} broadcast to network", hex::encode(&txid[..8]));
        
        // Store transaction in wallet history
        self.storage.read()
            .map_err(|_| WalletManagerError::StorageError("Lock poisoned".to_string()))?
            .store_transaction(&txid, &transaction)
            .ok(); // Don't fail if history storage fails
        
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
    
    /// Get transaction by txid (from wallet history or blockchain)
    pub fn get_transaction(&self, txid: &[u8; 32]) -> Result<Option<Transaction>, WalletManagerError> {
        // First check wallet storage
        if let Ok(tx) = self.storage.read()
            .map_err(|_| WalletManagerError::StorageError("Lock poisoned".to_string()))?
            .load_transaction(txid)
        {
            return Ok(Some(tx));
        }

        // Then check blockchain database
        match self.db.get_transaction(txid) {
            Ok(Some(tx)) => return Ok(Some(tx)),
            Ok(None) => { /* Transaction not in blockchain, continue checking */ }
            Err(e) => {
                // Log but don't fail - might still be in mempool
                tracing::debug!("Error looking up transaction in blockchain: {}", e);
            }
        }

        // Check mempool as last resort
        if let Some(tx) = self.mempool.get_transaction(txid) {
            return Ok(Some(tx));
        }

        Ok(None)
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

