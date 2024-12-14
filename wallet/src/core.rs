use btclib::types::{Transaction, TransactionInput, TransactionOutput};
use secp256k1::{Secp256k1, SecretKey, PublicKey, Message};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use thiserror::Error;
use sha2::{Sha256, Digest};
use crate::network::NetworkClient;

#[derive(Error, Debug)]
pub enum WalletError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("Insufficient funds: required {required}, available {available}")]
    InsufficientFunds { required: u64, available: u64 },
    #[error("Invalid private key")]
    InvalidPrivateKey,
    #[error("Invalid address")]
    InvalidAddress,
    #[error("UTXO not found")]
    UTXONotFound,
    #[error("Signing error: {0}")]
    SigningError(#[from] secp256k1::Error),
    #[error("Bincode error: {0}")]
    BincodeError(#[from] bincode::Error),
    #[error("Network error: {0}")]
    NetworkError(String),
}

#[derive(Serialize, Deserialize)]
pub struct Wallet {
    private_key: SecretKey,
    public_key: PublicKey,
    utxos: HashMap<[u8; 32], Vec<UTXO>>,
    wallet_path: PathBuf,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct UTXO {
    pub tx_hash: [u8; 32],
    pub output_index: u32,
    pub amount: u64,
    pub script_pubkey: Vec<u8>,
}

impl Wallet {
    /// Create a new wallet with a random key pair
    pub fn new(wallet_path: PathBuf) -> Result<Self, WalletError> {
        let secp = Secp256k1::new();
        let (secret_key, public_key) = secp.generate_keypair(&mut rand::thread_rng());

        let wallet = Self {
            private_key: secret_key,
            public_key: public_key,
            utxos: HashMap::new(),
            wallet_path,
        };

        wallet.save()?;
        Ok(wallet)
    }

    /// Load an existing wallet from file
    pub fn load(wallet_path: PathBuf) -> Result<Self, WalletError> {
        let wallet_data = std::fs::read_to_string(&wallet_path)?;
        let wallet: Self = serde_json::from_str(&wallet_data)?;
        Ok(wallet)
    }

    /// Save wallet to file
    pub fn save(&self) -> Result<(), WalletError> {
        let wallet_data = serde_json::to_string_pretty(self)?;
        std::fs::write(&self.wallet_path, wallet_data)?;
        Ok(())
    }

    /// Get wallet address (public key hash)
    pub fn get_address(&self) -> String {
        let mut hasher = Sha256::new();
        hasher.update(&self.public_key.serialize());
        let hash = hasher.finalize();
        hex::encode(hash)
    }

    /// Get wallet balance
    pub fn get_balance(&self) -> u64 {
        self.utxos.values()
            .flatten()
            .map(|utxo| utxo.amount)
            .sum()
    }

    /// Create a new transaction
    pub fn create_transaction(
        &self,
        recipient: &str,
        amount: u64,
        fee: u64,
    ) -> Result<Transaction, WalletError> {
        // Check if we have enough funds
        let total_required = amount + fee;
        let balance = self.get_balance();
        if balance < total_required {
            return Err(WalletError::InsufficientFunds {
                required: total_required,
                available: balance,
            });
        }

        // Select UTXOs
        let selected_utxos = self.select_utxos(total_required)?;
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
                hex::decode(recipient).map_err(|_| WalletError::InvalidAddress)?,
            ),
        ];

        // Add change output if necessary
        let change = total_input - total_required;
        if change > 0 {
            outputs.push(TransactionOutput::new(
                change,
                hex::decode(self.get_address()).unwrap(),
            ));
        }

        let transaction = Transaction::new(1, inputs, outputs, 0);

        // Sign the transaction
        self.sign_transaction(transaction)
    }

    /// Send a transaction to the network
    pub async fn send_transaction(
        &self,
        recipient: &str,
        amount: u64,
        fee: u64,
        network: &NetworkClient,
    ) -> Result<[u8; 32], WalletError> {
        // Create and sign transaction
        let transaction = self.create_transaction(recipient, amount, fee)?;
        let tx_hash = transaction.hash();
        
        // Broadcast to network
        network.broadcast_transaction(transaction)
            .await
            .map_err(|e| WalletError::NetworkError(e.to_string()))?;
        
        Ok(tx_hash)
    }

    /// Sign a transaction
    fn sign_transaction(&self, mut transaction: Transaction) -> Result<Transaction, WalletError> {
        let secp = Secp256k1::new();

        // Sign each input
        for (i, _) in transaction.inputs().iter().enumerate() {
            // Create signature hash for this input
            let sighash = self.create_signature_hash(&transaction, i)?;
            
            // Create signature
            let message = Message::from_slice(&sighash)?;
            let signature = secp.sign_ecdsa(&message, &self.private_key);
            
            // Create signature script
            let mut signature_script = Vec::new();
            signature_script.extend_from_slice(&signature.serialize_der());
            signature_script.push(0x01); // SIGHASH_ALL
            signature_script.extend_from_slice(&self.public_key.serialize());
            
            // Update input with signature
            transaction.set_signature_script(i, signature_script)?;
        }

        Ok(transaction)
    }

    /// Create signature hash for an input
    fn create_signature_hash(&self, transaction: &Transaction, input_index: usize) -> Result<[u8; 32], WalletError> {
        // Create a copy of the transaction for signing
        let mut tx_copy = transaction.clone();

        // Clear all input signature scripts
        for input in tx_copy.inputs_mut() {
            input.clear_signature_script();
        }

        // Set the current input's script
        if let Some(utxo) = self.find_utxo(tx_copy.inputs()[input_index].prev_tx_hash(), 
                                          tx_copy.inputs()[input_index].prev_output_index()) {
            tx_copy.set_signature_script(input_index, utxo.script_pubkey.clone())?;
        } else {
            return Err(WalletError::UTXONotFound);
        }

        // Serialize and hash
        let mut hasher = Sha256::new();
        hasher.update(&bincode::serialize(&tx_copy)?);
        let mut hash = [0u8; 32];
        hash.copy_from_slice(&hasher.finalize());
        Ok(hash)
    }

    /// Find a UTXO by transaction hash and output index
    fn find_utxo(&self, tx_hash: [u8; 32], output_index: u32) -> Option<&UTXO> {
        self.utxos.get(&tx_hash)
            .and_then(|utxos| utxos.iter()
                .find(|utxo| utxo.output_index == output_index))
    }

    /// Select UTXOs for spending
    fn select_utxos(&self, amount: u64) -> Result<Vec<UTXO>, WalletError> {
        let mut selected = Vec::new();
        let mut selected_amount = 0;

        for utxos in self.utxos.values() {
            for utxo in utxos {
                selected.push(utxo.clone());
                selected_amount += utxo.amount;
                if selected_amount >= amount {
                    return Ok(selected);
                }
            }
        }

        Err(WalletError::InsufficientFunds {
            required: amount,
            available: selected_amount,
        })
    }

    /// Add a new UTXO to the wallet
    pub fn add_utxo(&mut self, utxo: UTXO) {
        self.utxos.entry(utxo.tx_hash)
            .or_insert_with(Vec::new)
            .push(utxo);
    }

    /// Remove a spent UTXO
    pub fn remove_utxo(&mut self, tx_hash: [u8; 32], output_index: u32) {
        if let Some(utxos) = self.utxos.get_mut(&tx_hash) {
            utxos.retain(|utxo| utxo.output_index != output_index);
            if utxos.is_empty() {
                self.utxos.remove(&tx_hash);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_wallet_creation() {
        let dir = tempdir().unwrap();
        let wallet_path = dir.path().join("wallet.json");
        let wallet = Wallet::new(wallet_path).unwrap();
        
        assert_eq!(wallet.get_balance(), 0);
        assert!(!wallet.get_address().is_empty());
    }

    #[test]
    fn test_utxo_management() {
        let dir = tempdir().unwrap();
        let wallet_path = dir.path().join("wallet.json");
        let mut wallet = Wallet::new(wallet_path).unwrap();

        let utxo = UTXO {
            tx_hash: [0u8; 32],
            output_index: 0,
            amount: 100_000,
            script_pubkey: vec![],
        };

        wallet.add_utxo(utxo.clone());
        assert_eq!(wallet.get_balance(), 100_000);

        wallet.remove_utxo(utxo.tx_hash, utxo.output_index);
        assert_eq!(wallet.get_balance(), 0);
    }

    #[test]
    fn test_transaction_signing() {
        let dir = tempdir().unwrap();
        let wallet_path = dir.path().join("wallet.json");
        let mut wallet = Wallet::new(wallet_path).unwrap();

        // Add some UTXOs
        let utxo = UTXO {
            tx_hash: [1u8; 32],
            output_index: 0,
            amount: 100_000,
            script_pubkey: wallet.public_key.serialize().to_vec(),
        };
        wallet.add_utxo(utxo);

        // Create and sign a transaction
        let recipient_address = hex::encode([2u8; 32]);
        let transaction = wallet.create_transaction(&recipient_address, 50_000, 1_000).unwrap();

        assert!(transaction.inputs().len() > 0);
        assert!(transaction.outputs().len() > 0);
        
        // Verify the transaction has signatures
        for input in transaction.inputs() {
            assert!(!input.signature_script().is_empty());
        }
    }

    #[test]
    fn test_insufficient_funds() {
        let dir = tempdir().unwrap();
        let wallet_path = dir.path().join("wallet.json");
        let wallet = Wallet::new(wallet_path).unwrap();

        let recipient_address = hex::encode([2u8; 32]);
        let result = wallet.create_transaction(&recipient_address, 100_000, 1_000);

        assert!(matches!(
            result,
            Err(WalletError::InsufficientFunds { required: 101_000, available: 0 })
        ));
    }

    #[test]
    fn test_wallet_persistence() {
        let dir = tempdir().unwrap();
        let wallet_path = dir.path().join("wallet.json");
        
        // Create and save wallet
        let mut wallet = Wallet::new(wallet_path.clone()).unwrap();
        let address = wallet.get_address();
        
        let utxo = UTXO {
            tx_hash: [1u8; 32],
            output_index: 0,
            amount: 100_000,
            script_pubkey: vec![],
        };
        wallet.add_utxo(utxo);
        wallet.save().unwrap();

        // Load wallet and verify state
        let loaded_wallet = Wallet::load(wallet_path).unwrap();
        assert_eq!(loaded_wallet.get_address(), address);
        assert_eq!(loaded_wallet.get_balance(), 100_000);
    }
}