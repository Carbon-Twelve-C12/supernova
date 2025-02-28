use bip39::{Mnemonic, Language, MnemonicType, Seed};
use tiny_bip32::{DerivationPath, KeyChain, ExtendedPrivKey};
use secp256k1::{Secp256k1, SecretKey, PublicKey};
use serde::{Serialize, Deserialize};
use hmac::Hmac;
use sha2::{Sha256, Sha512, Digest};
use pbkdf2::pbkdf2;
use std::collections::HashMap;
use thiserror::Error;
use std::str::FromStr;
use rand::RngCore;
use rand::rngs::OsRng;

use crate::core::{UTXO, WalletError};
use crate::network::NetworkClient;
use btclib::types::{Transaction, TransactionInput, TransactionOutput};

/// BIP-44 path constants
const PURPOSE: u32 = 44;
const COIN_TYPE: u32 = slip0044::BITCOIN; // We'll use Bitcoin's coin type for now
const ACCOUNT_INDEX: u32 = 0;
const CHANGE_EXTERNAL: u32 = 0;
const CHANGE_INTERNAL: u32 = 1;

/// Error type for HD wallet operations
#[derive(Error, Debug)]
pub enum HDWalletError {
    #[error("Mnemonic error: {0}")]
    MnemonicError(String),
    
    #[error("Key derivation error: {0}")]
    DerivationError(String),
    
    #[error("Invalid seed")]
    InvalidSeed,
    
    #[error("Invalid path: {0}")]
    InvalidPath(String),
    
    #[error("Crypto error: {0}")]
    CryptoError(#[from] secp256k1::Error),
    
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
    
    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),
    
    #[error("Wallet locked")]
    WalletLocked,
    
    #[error("Insufficient funds: required {required}, available {available}")]
    InsufficientFunds { required: u64, available: u64 },
    
    #[error("Account not found: {0}")]
    AccountNotFound(u32),
    
    #[error("Network error: {0}")]
    NetworkError(String),
    
    #[error("Bincode error: {0}")]
    BincodeError(#[from] bincode::Error),
}

/// Represents an HD wallet address
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HDAddress {
    pub path: String,
    pub public_key: PublicKey,
    pub address: String, // Hex encoded address
    pub is_change: bool,
    pub index: u32,
}

/// Represents an HD wallet account
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HDAccount {
    pub index: u32,
    pub name: String,
    pub addresses: HashMap<String, HDAddress>, // Map of address string to address info
    pub next_external_index: u32,
    pub next_internal_index: u32,
}

impl HDAccount {
    /// Create a new HD account
    pub fn new(index: u32, name: String) -> Self {
        Self {
            index,
            name,
            addresses: HashMap::new(),
            next_external_index: 0,
            next_internal_index: 0,
        }
    }
    
    /// Add an address to the account
    pub fn add_address(&mut self, address: HDAddress) {
        // Update next index based on address type
        if address.is_change {
            self.next_internal_index = self.next_internal_index.max(address.index + 1);
        } else {
            self.next_external_index = self.next_external_index.max(address.index + 1);
        }
        
        self.addresses.insert(address.address.clone(), address);
    }
    
    /// Get all receiving (external) addresses
    pub fn get_receiving_addresses(&self) -> Vec<&HDAddress> {
        self.addresses.values()
            .filter(|addr| !addr.is_change)
            .collect()
    }
    
    /// Get all change (internal) addresses
    pub fn get_change_addresses(&self) -> Vec<&HDAddress> {
        self.addresses.values()
            .filter(|addr| addr.is_change)
            .collect()
    }
}

/// Main HD wallet structure
#[derive(Serialize, Deserialize)]
pub struct HDWallet {
    #[serde(skip_serializing, skip_deserializing)]
    mnemonic: Option<Mnemonic>, // Not serialized for security
    
    #[serde(skip_serializing, skip_deserializing)]
    seed: Option<[u8; 64]>, // Not serialized for security
    
    encrypted_seed: Vec<u8>, // Encrypted seed stored for recovery
    salt: [u8; 32],
    
    #[serde(skip_serializing, skip_deserializing)]
    keychain: Option<KeyChain>, // Not serialized, recreated from seed
    
    accounts: HashMap<u32, HDAccount>,
    utxos: HashMap<String, Vec<UTXO>>, // Map from address to UTXOs
    labels: HashMap<String, String>, // Transaction labels
}

impl HDWallet {
    /// Create a new HD wallet with a random mnemonic
    pub fn new(password: &str) -> Result<Self, HDWalletError> {
        // Generate a new random mnemonic
        let mnemonic = Mnemonic::new(MnemonicType::Words12, Language::English);
        Self::from_mnemonic(&mnemonic.to_string(), password)
    }
    
    /// Create an HD wallet from an existing mnemonic
    pub fn from_mnemonic(mnemonic_str: &str, password: &str) -> Result<Self, HDWalletError> {
        // Parse mnemonic
        let mnemonic = Mnemonic::from_phrase(mnemonic_str, Language::English)
            .map_err(|e| HDWalletError::MnemonicError(e.to_string()))?;
        
        // Generate seed from mnemonic
        let seed = Seed::new(&mnemonic, "");
        let seed_bytes = seed.as_bytes();
        
        if seed_bytes.len() != 64 {
            return Err(HDWalletError::InvalidSeed);
        }
        
        let mut seed_array = [0u8; 64];
        seed_array.copy_from_slice(seed_bytes);
        
        // Create keychain from seed
        let keychain = KeyChain::new(seed_bytes)
            .map_err(|e| HDWalletError::DerivationError(e.to_string()))?;
        
        // Encrypt seed with password
        let mut salt = [0u8; 32];
        OsRng.fill_bytes(&mut salt);
        
        let mut encrypted_seed = vec![0u8; 64];
        encrypt_seed(&seed_array, password, &salt, &mut encrypted_seed)?;
        
        let wallet = Self {
            mnemonic: Some(mnemonic),
            seed: Some(seed_array),
            encrypted_seed,
            salt,
            keychain: Some(keychain),
            accounts: HashMap::new(),
            utxos: HashMap::new(),
            labels: HashMap::new(),
        };
        
        // Create default account
        let mut default_account = HDAccount::new(0, "Default".to_string());
        
        // Generate initial addresses
        let receive_address = wallet.derive_new_address(0, false)?;
        let change_address = wallet.derive_new_address(0, true)?;
        
        default_account.add_address(receive_address);
        default_account.add_address(change_address);
        
        let mut accounts = HashMap::new();
        accounts.insert(0, default_account);
        
        Ok(Self {
            mnemonic: wallet.mnemonic,
            seed: wallet.seed,
            encrypted_seed,
            salt,
            keychain: wallet.keychain,
            accounts,
            utxos: HashMap::new(),
            labels: HashMap::new(),
        })
    }
    
    /// Restore wallet from encrypted seed using password
    pub fn unlock(&mut self, password: &str) -> Result<(), HDWalletError> {
        let mut seed = [0u8; 64];
        decrypt_seed(&self.encrypted_seed, password, &self.salt, &mut seed)?;
        
        // Create keychain from seed
        let keychain = KeyChain::new(&seed)
            .map_err(|e| HDWalletError::DerivationError(e.to_string()))?;
        
        self.seed = Some(seed);
        self.keychain = Some(keychain);
        
        Ok(())
    }
    
    /// Lock the wallet (clear unencrypted secrets from memory)
    pub fn lock(&mut self) {
        self.mnemonic = None;
        self.seed = None;
        self.keychain = None;
    }
    
    /// Get mnemonic for backup (be careful with this!)
    pub fn get_mnemonic(&self) -> Result<&str, HDWalletError> {
        self.mnemonic.as_ref()
            .ok_or(HDWalletError::WalletLocked)
            .map(|m| m.phrase())
    }
    
    /// Create a new account
    pub fn create_account(&mut self, name: String) -> Result<&HDAccount, HDWalletError> {
        let index = self.accounts.len() as u32;
        let mut account = HDAccount::new(index, name);
        
        // Generate initial addresses for the account
        let receive_address = self.derive_new_address(index, false)?;
        let change_address = self.derive_new_address(index, true)?;
        
        account.add_address(receive_address);
        account.add_address(change_address);
        
        self.accounts.insert(index, account);
        Ok(self.accounts.get(&index).unwrap())
    }
    
    /// Get a specific account
    pub fn get_account(&self, index: u32) -> Option<&HDAccount> {
        self.accounts.get(&index)
    }
    
    /// Get a mutable reference to an account
    pub fn get_account_mut(&mut self, index: u32) -> Option<&mut HDAccount> {
        self.accounts.get_mut(&index)
    }
    
    /// List all accounts
    pub fn list_accounts(&self) -> Vec<&HDAccount> {
        let mut accounts: Vec<_> = self.accounts.values().collect();
        accounts.sort_by_key(|a| a.index);
        accounts
    }
    
    /// Derive a new address for an account
    pub fn derive_new_address(&self, account_index: u32, is_change: bool) -> Result<HDAddress, HDWalletError> {
        let keychain = self.keychain.as_ref()
            .ok_or(HDWalletError::WalletLocked)?;
        
        let change_type = if is_change { CHANGE_INTERNAL } else { CHANGE_EXTERNAL };
        
        // Get next index from account or use 0 for new accounts
        let address_index = match self.accounts.get(&account_index) {
            Some(account) => {
                if is_change {
                    account.next_internal_index
                } else {
                    account.next_external_index
                }
            },
            None => 0,
        };
        
        // Derive BIP-44 path: m/44'/coin_type'/account'/change/address_index
        let path = format!("m/{}'/{}'/{}'/{}/{}", 
                          PURPOSE, COIN_TYPE, account_index, change_type, address_index);
        
        let derivation_path = DerivationPath::from_str(&path)
            .map_err(|_| HDWalletError::InvalidPath(path.clone()))?;
        
        // Derive private key
        let private_key = keychain.derive_private_key(derivation_path)
            .map_err(|e| HDWalletError::DerivationError(e.to_string()))?;
        
        // Convert to our key format
        let secp = Secp256k1::new();
        let secret_key = SecretKey::from_slice(&private_key.key)
            .map_err(|e| HDWalletError::CryptoError(e))?;
        
        let public_key = PublicKey::from_secret_key(&secp, &secret_key);
        
        // Create address (simple hash of public key for now)
        let mut hasher = Sha256::new();
        hasher.update(public_key.serialize());
        let address = hex::encode(hasher.finalize());
        
        Ok(HDAddress {
            path,
            public_key,
            address,
            is_change,
            index: address_index,
        })
    }
    
    /// Generate a new receiving address for an account
    pub fn new_receiving_address(&mut self, account_index: u32) -> Result<HDAddress, HDWalletError> {
        // Check if account exists
        if !self.accounts.contains_key(&account_index) {
            return Err(HDWalletError::AccountNotFound(account_index));
        }
        
        // Derive new address
        let address = self.derive_new_address(account_index, false)?;
        
        // Add to account
        if let Some(account) = self.accounts.get_mut(&account_index) {
            account.add_address(address.clone());
        }
        
        Ok(address)
    }
    
    /// Generate a new change address for an account
    pub fn new_change_address(&mut self, account_index: u32) -> Result<HDAddress, HDWalletError> {
        // Check if account exists
        if !self.accounts.contains_key(&account_index) {
            return Err(HDWalletError::AccountNotFound(account_index));
        }
        
        // Derive new address
        let address = self.derive_new_address(account_index, true)?;
        
        // Add to account
        if let Some(account) = self.accounts.get_mut(&account_index) {
            account.add_address(address.clone());
        }
        
        Ok(address)
    }
    
    /// Add a UTXO to an address
    pub fn add_utxo(&mut self, address: &str, utxo: UTXO) {
        self.utxos.entry(address.to_string())
            .or_insert_with(Vec::new)
            .push(utxo);
    }
    
    /// Get all UTXOs for an account
    pub fn get_account_utxos(&self, account_index: u32) -> Vec<(&String, &UTXO)> {
        let account = match self.accounts.get(&account_index) {
            Some(acc) => acc,
            None => return Vec::new(),
        };
        
        let mut result = Vec::new();
        
        for addr in account.addresses.keys() {
            if let Some(utxos) = self.utxos.get(addr) {
                for utxo in utxos {
                    result.push((addr, utxo));
                }
            }
        }
        
        result
    }
    
    /// Get the total balance for an account
    pub fn get_account_balance(&self, account_index: u32) -> u64 {
        self.get_account_utxos(account_index)
            .iter()
            .map(|(_, utxo)| utxo.amount)
            .sum()
    }
    
    /// Get all addresses for an account
    pub fn get_account_addresses(&self, account_index: u32) -> Vec<HDAddress> {
        match self.accounts.get(&account_index) {
            Some(account) => account.addresses.values().cloned().collect(),
            None => Vec::new(),
        }
    }
    
    /// Add a transaction label
    pub fn add_transaction_label(&mut self, tx_hash: &str, label: String) {
        self.labels.insert(tx_hash.to_string(), label);
    }
    
    /// Get a transaction label
    pub fn get_transaction_label(&self, tx_hash: &str) -> Option<&String> {
        self.labels.get(tx_hash)
    }
    
    /// Remove a transaction label
    pub fn remove_transaction_label(&mut self, tx_hash: &str) -> Option<String> {
        self.labels.remove(tx_hash)
    }
    
    /// Get all transaction labels
    pub fn get_all_labels(&self) -> &HashMap<String, String> {
        &self.labels
    }
    
    /// Create a transaction from a specific account
    pub fn create_transaction_from_account(
        &self,
        account_index: u32,
        recipient: &str,
        amount: u64,
        fee: u64,
    ) -> Result<Transaction, HDWalletError> {
        // Get account
        let account = self.accounts.get(&account_index)
            .ok_or(HDWalletError::AccountNotFound(account_index))?;
        
        // Check if we have enough funds
        let total_required = amount + fee;
        let balance = self.get_account_balance(account_index);
        if balance < total_required {
            return Err(HDWalletError::InsufficientFunds {
                required: total_required,
                available: balance,
            });
        }
        
        // Select UTXOs
        let selected_utxos = self.select_utxos_from_account(account_index, total_required)?;
        let total_input = selected_utxos.iter().map(|u| u.amount).sum::<u64>();
        
        // Create transaction inputs
        let inputs: Vec<TransactionInput> = selected_utxos
            .iter()
            .map(|utxo| TransactionInput::new(
                utxo.tx_hash,
                utxo.output_index,
                vec![], // Will be signed later
                0xffffffff,
            ))
            .collect();
        
        // Create transaction outputs
        let mut outputs = vec![
            TransactionOutput::new(
                amount,
                hex::decode(recipient).map_err(|_| HDWalletError::InvalidPath("Invalid recipient address".to_string()))?,
            ),
        ];
        
        // Add change output if necessary
        let change = total_input - total_required;
        if change > 0 {
            // Generate a new change address
            let change_address = self.derive_new_address(account_index, true)?;
            
            // Add to account
            if let Some(account) = self.accounts.get(&account_index) {
                outputs.push(TransactionOutput::new(
                    change,
                    hex::decode(&change_address.address).unwrap(),
                ));
            }
        }
        
        let transaction = Transaction::new(1, inputs, outputs, 0);
        
        // Sign the transaction
        self.sign_transaction(transaction, &selected_utxos)
    }
    
    /// Send a transaction from a specific account
    pub async fn send_from_account(
        &mut self,
        account_index: u32,
        recipient: &str,
        amount: u64,
        fee: u64,
        network: &NetworkClient,
    ) -> Result<(Transaction, [u8; 32]), HDWalletError> {
        // Create transaction
        let transaction = self.create_transaction_from_account(account_index, recipient, amount, fee)?;
        let tx_hash = transaction.hash();
        
        // Broadcast to network
        network.broadcast_transaction(transaction.clone())
            .await
            .map_err(|e| HDWalletError::NetworkError(e.to_string()))?;
        
        // Return transaction and hash
        Ok((transaction, tx_hash))
    }
    
    /// Sign a transaction
    fn sign_transaction(&self, mut transaction: Transaction, utxos: &[&UTXO]) -> Result<Transaction, HDWalletError> {
        let keychain = self.keychain.as_ref()
            .ok_or(HDWalletError::WalletLocked)?;
            
        let secp = Secp256k1::new();
        
        // Sign each input
        for (i, input) in transaction.inputs().iter().enumerate() {
            // Find the UTXO for this input
            let utxo = utxos[i];
            
            // Find the address for this UTXO
            let mut address_path = None;
            for account in self.accounts.values() {
                for (addr_str, addr) in &account.addresses {
                    if self.utxos.get(addr_str).map_or(false, |addr_utxos| {
                        addr_utxos.iter().any(|u| u.tx_hash == input.prev_tx_hash() && u.output_index == input.prev_output_index())
                    }) {
                        address_path = Some(addr.path.clone());
                        break;
                    }
                }
                if address_path.is_some() {
                    break;
                }
            }
            
            let path = match address_path {
                Some(p) => p,
                None => return Err(HDWalletError::InvalidPath("UTXO address not found in wallet".to_string())),
            };
            
            // Derive private key for this address
            let derivation_path = DerivationPath::from_str(&path)
                .map_err(|_| HDWalletError::InvalidPath(path.clone()))?;
                
            let private_key = keychain.derive_private_key(derivation_path)
                .map_err(|e| HDWalletError::DerivationError(e.to_string()))?;
                
            let secret_key = SecretKey::from_slice(&private_key.key)
                .map_err(|e| HDWalletError::CryptoError(e))?;
            
            // Create signature hash for this input
            let sighash = self.create_signature_hash(&transaction, i, &utxo.script_pubkey)?;
            
            // Create signature
            let message = secp256k1::Message::from_slice(&sighash)?;
            let signature = secp.sign_ecdsa(&message, &secret_key);
            
            // Create signature script
            let mut signature_script = Vec::new();
            signature_script.extend_from_slice(&signature.serialize_der());
            signature_script.push(0x01); // SIGHASH_ALL
            
            let public_key = PublicKey::from_secret_key(&secp, &secret_key);
            signature_script.extend_from_slice(&public_key.serialize());
            
            // Update input with signature
            transaction.set_signature_script(i, signature_script)?;
        }
        
        Ok(transaction)
    }
    
    /// Create signature hash for an input
    fn create_signature_hash(&self, transaction: &Transaction, input_index: usize, script_pubkey: &[u8]) -> Result<[u8; 32], HDWalletError> {
        // Create a copy of the transaction for signing
        let mut tx_copy = transaction.clone();
        
        // Clear all input signature scripts
        for input in tx_copy.inputs_mut() {
            input.clear_signature_script();
        }
        
        // Set the current input's script
        tx_copy.set_signature_script(input_index, script_pubkey.to_vec())?;
        
        // Serialize and hash
        let mut hasher = Sha256::new();
        hasher.update(&bincode::serialize(&tx_copy)?);
        let mut hash = [0u8; 32];
        hash.copy_from_slice(&hasher.finalize());
        Ok(hash)
    }
    
    /// Find a UTXO by transaction hash and output index
    fn find_utxo(&self, tx_hash: [u8; 32], output_index: u32) -> Option<&UTXO> {
        for utxos in self.utxos.values() {
            for utxo in utxos {
                if utxo.tx_hash == tx_hash && utxo.output_index == output_index {
                    return Some(utxo);
                }
            }
        }
        None
    }
    
    /// Select UTXOs for spending from a specific account
    fn select_utxos_from_account(&self, account_index: u32, amount: u64) -> Result<Vec<&UTXO>, HDWalletError> {
        let utxos = self.get_account_utxos(account_index);
        let mut selected = Vec::new();
        let mut selected_amount = 0;
        
        for (_, utxo) in utxos {
            selected.push(utxo);
            selected_amount += utxo.amount;
            if selected_amount >= amount {
                return Ok(selected);
            }
        }
        
        Err(HDWalletError::InsufficientFunds {
            required: amount,
            available: selected_amount,
        })
    }
    
    /// Find address by public key script
    pub fn find_address_by_pubkey(&self, pubkey: &[u8]) -> Option<String> {
        // Search through all addresses in all accounts
        for account in self.accounts.values() {
            for address in account.addresses.values() {
                // This is a simplified check - in a real implementation,
                // you would compare against the actual output script types
                if pubkey == address.public_key.serialize().as_slice() {
                    return Some(address.address.clone());
                }
            }
        }
        None
    }
    
    /// Get total balance across all accounts
    pub fn get_total_balance(&self) -> u64 {
        self.accounts.keys()
            .map(|account_idx| self.get_account_balance(*account_idx))
            .sum()
    }
    
    /// Get the total number of addresses across all accounts
    pub fn get_address_count(&self) -> usize {
        self.accounts.values()
            .map(|account| account.addresses.len())
            .sum()
    }
}

/// Helper function to encrypt seed with password
fn encrypt_seed(seed: &[u8; 64], password: &str, salt: &[u8; 32], output: &mut [u8]) -> Result<(), HDWalletError> {
    // Simple encryption using password and PBKDF2
    // In a real implementation, you might want to use a more robust encryption scheme
    let mut key = [0u8; 32];
    pbkdf2::<Hmac<Sha256>>(password.as_bytes(), salt, 10000, &mut key)
        .map_err(|_| HDWalletError::MnemonicError("PBKDF2 error".to_string()))?;
    
    // XOR the seed with derived key (repeating the key)
    for i in 0..seed.len() {
        output[i] = seed[i] ^ key[i % key.len()];
    }
    
    Ok(())
}

/// Helper function to decrypt seed with password
fn decrypt_seed(encrypted: &[u8], password: &str, salt: &[u8; 32], output: &mut [u8; 64]) -> Result<(), HDWalletError> {
    // Derive same key as encryption
    let mut key = [0u8; 32];
    pbkdf2::<Hmac<Sha256>>(password.as_bytes(), salt, 10000, &mut key)
        .map_err(|_| HDWalletError::MnemonicError("PBKDF2 error".to_string()))?;
    
    // XOR to decrypt (same operation as encryption)
    for i in 0..encrypted.len() {
        output[i] = encrypted[i] ^ key[i % key.len()];
    }
    
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_wallet_creation_and_address_derivation() {
        let wallet = HDWallet::new("password123").unwrap();
        
        // Check default account was created
        let accounts = wallet.list_accounts();
        assert_eq!(accounts.len(), 1);
        
        let default_account = accounts[0];
        assert_eq!(default_account.index, 0);
        assert_eq!(default_account.name, "Default");
        
        // Check initial addresses were created
        let addresses = wallet.get_account_addresses(0);
        assert_eq!(addresses.len(), 2);
        
        // Verify we have one receiving and one change address
        let receiving = addresses.iter().filter(|a| !a.is_change).count();
        let change = addresses.iter().filter(|a| a.is_change).count();
        assert_eq!(receiving, 1);
        assert_eq!(change, 1);
    }
    
    #[test]
    fn test_multiple_accounts() {
        let mut wallet = HDWallet::new("password123").unwrap();
        
        // Create a second account
        let second_account = wallet.create_account("Savings".to_string()).unwrap();
        assert_eq!(second_account.index, 1);
        assert_eq!(second_account.name, "Savings");
        
        // Check addresses for second account
        let addresses = wallet.get_account_addresses(1);
        assert_eq!(addresses.len(), 2);
        
        // Generate additional addresses
        let new_address = wallet.new_receiving_address(1).unwrap();
        assert_eq!(new_address.index, 1); // Should be index 1 since we already have index 0
        assert!(!new_address.is_change);
        
        // Check address was added
        let updated_addresses = wallet.get_account_addresses(1);
        assert_eq!(updated_addresses.len(), 3);
    }
    
    #[test]
    fn test_seed_encryption_decryption() {
        let password = "secure_password";
        let seed = [42u8; 64];
        let mut salt = [0u8; 32];
        OsRng.fill_bytes(&mut salt);
        
        let mut encrypted = vec![0u8; 64];
        encrypt_seed(&seed, password, &salt, &mut encrypted).unwrap();
        
        // Encrypted data should differ from original
        assert_ne!(&seed[..], &encrypted[..]);
        
        // Decrypt and verify
        let mut decrypted = [0u8; 64];
        decrypt_seed(&encrypted, password, &salt, &mut decrypted).unwrap();
        
        assert_eq!(seed, decrypted);
        
        // Try with wrong password
        let mut wrong_decryption = [0u8; 64];
        decrypt_seed(&encrypted, "wrong_password", &salt, &mut wrong_decryption).unwrap();
        assert_ne!(seed, wrong_decryption);
    }
    
    #[test]
    fn test_utxo_management() {
        let mut wallet = HDWallet::new("password123").unwrap();
        
        // Get a receiving address
        let addresses = wallet.get_account_addresses(0);
        let receiving_address = addresses.iter()
            .find(|a| !a.is_change)
            .unwrap();
        
        // Add a UTXO
        let utxo = UTXO {
            tx_hash: [1u8; 32],
            output_index: 0,
            amount: 1000_000,
            script_pubkey: vec![1, 2, 3, 4],
        };
        
        wallet.add_utxo(&receiving_address.address, utxo);
        
        // Check balance
        let balance = wallet.get_account_balance(0);
        assert_eq!(balance, 1000_000);
        
        // Add another UTXO
        let utxo2 = UTXO {
            tx_hash: [2u8; 32],
            output_index: 1,
            amount: 500_000,
            script_pubkey: vec![1, 2, 3, 4],
        };
        
        wallet.add_utxo(&receiving_address.address, utxo2);
        
        // Check updated balance
        let balance = wallet.get_account_balance(0);
        assert_eq!(balance, 1500_000);
    }
    
    #[test]
    fn test_transaction_labels() {
        let mut wallet = HDWallet::new("password123").unwrap();
        
        // Add labels
        wallet.add_transaction_label("tx1", "Payment to Alice".to_string());
        wallet.add_transaction_label("tx2", "Savings".to_string());
        
        // Check labels
        assert_eq!(wallet.get_transaction_label("tx1").unwrap(), "Payment to Alice");
        assert_eq!(wallet.get_transaction_label("tx2").unwrap(), "Savings");
        
        // Update a label
        wallet.add_transaction_label("tx1", "Updated: Payment to Alice".to_string());
        assert_eq!(wallet.get_transaction_label("tx1").unwrap(), "Updated: Payment to Alice");
        
        // Remove a label
        wallet.remove_transaction_label("tx2");
        assert!(wallet.get_transaction_label("tx2").is_none());
    }
}