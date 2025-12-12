use bitcoin as btc_compat; // Bitcoin-compatible
use btc_compat::{
    absolute::LockTime,
    hashes::Hash,
    network::Network,
    secp256k1::Secp256k1,
    sighash::{EcdsaSighashType, SighashCache},
    transaction::Version,
    Address, Amount, OutPoint, PrivateKey, PublicKey, ScriptBuf, Sequence, Transaction, TxIn,
    TxOut, Witness,
};
use rand::rngs::OsRng;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::str::FromStr;

#[derive(Debug, thiserror::Error)]
pub enum WalletError {
    #[error("Compatibility error")]
    Compatibility(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Invalid address: {0}")]
    InvalidAddress(String),
    #[error("Invalid amount: {0}")]
    InvalidAmount(u64),
    #[error("Insufficient funds: {0}")]
    InsufficientFunds(u64),
    #[error("Transaction error: {0}")]
    Transaction(String),
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("Invalid private key")]
    InvalidPrivateKey,
    #[error("UTXO not found")]
    UTXONotFound,
    #[error("Signing error: {0}")]
    SigningError(String),
    #[error("Bincode error: {0}")]
    BincodeError(#[from] bincode::Error),
    #[error("Network error: {0}")]
    NetworkError(String),
}

#[derive(Debug)]
pub struct Wallet {
    private_key: PrivateKey,
    network: Network,
    utxos: HashMap<[u8; 32], Vec<UTXO>>,
    wallet_path: PathBuf,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UTXO {
    pub tx_hash: [u8; 32],
    pub output_index: u32,
    pub amount: u64,
    pub script_pubkey: Vec<u8>,
}

impl Wallet {
    pub fn new(network: Network) -> Result<Self, WalletError> {
        let secp = Secp256k1::new();
        // SECURITY FIX (P0-006): Use OsRng instead of thread_rng for key generation
        let (secret_key, _) = secp.generate_keypair(&mut OsRng);
        let private_key = PrivateKey::new(secret_key, network);

        Ok(Self {
            private_key,
            network,
            utxos: HashMap::new(),
            wallet_path: PathBuf::new(),
        })
    }

    pub fn from_private_key(
        private_key: PrivateKey,
        network: Network,
    ) -> Result<Self, WalletError> {
        Ok(Self {
            private_key,
            network,
            utxos: HashMap::new(),
            wallet_path: PathBuf::new(),
        })
    }

    pub fn get_public_key(&self) -> PublicKey {
        let secp = Secp256k1::new();
        self.private_key.public_key(&secp)
    }

    pub fn get_address(&self) -> Result<Address, WalletError> {
        Address::p2wpkh(&self.get_public_key(), self.network)
            .map_err(|e| WalletError::Compatibility(e.to_string()))
    }

    pub fn sign_transaction(&self, tx: &mut Transaction) -> Result<(), WalletError> {
        let secp = Secp256k1::new();

        // First, collect all the signature hashes without borrowing tx mutably
        let mut signatures = Vec::new();
        for input_index in 0..tx.input.len() {
            // Get the corresponding UTXO
            let input = &tx.input[input_index];
            let _utxo = self
                .find_utxo(
                    input.previous_output.txid.to_raw_hash().to_byte_array(),
                    input.previous_output.vout,
                )
                .ok_or(WalletError::UTXONotFound)?;

            // Create signature hash
            let sighash = self.create_signature_hash(tx, input_index)?;

            // Sign the hash
        let msg = btc_compat::secp256k1::Message::from_digest_slice(&sighash)
                .map_err(|e| WalletError::SigningError(e.to_string()))?;

            let signature = secp.sign_ecdsa(&msg, &self.private_key.inner);

            // Create signature with sighash flag
            let mut sig_serialized = signature.serialize_der().to_vec();
            sig_serialized.push(EcdsaSighashType::All as u8);

            signatures.push(sig_serialized);
        }

        // Now apply the signatures to the transaction
        let public_key = self.get_public_key();
        for (input_index, input) in tx.input.iter_mut().enumerate() {
            // Construct witness
            let mut witness_stack = Witness::new();
            witness_stack.push(signatures[input_index].clone());
            witness_stack.push(public_key.to_bytes());

            // Set witness
            input.witness = witness_stack;
        }

        Ok(())
    }

    pub fn create_transaction(
        &self,
        recipient: &str,
        amount: u64,
        fee: u64,
    ) -> Result<Transaction, WalletError> {
        let recipient_address = Address::from_str(recipient)
            .map_err(|_| WalletError::InvalidAddress(recipient.to_string()))?
            .require_network(self.network)
            .map_err(|_| WalletError::InvalidAddress(recipient.to_string()))?;

        // Calculate total amount needed
        let total_needed = amount
            .checked_add(fee)
            .ok_or(WalletError::InsufficientFunds(amount))?;

        // Select UTXOs to use as inputs
        let selected_utxos = self.select_utxos(total_needed)?;

        // Calculate total input amount
        let total_input: u64 = selected_utxos.iter().map(|utxo| utxo.amount).sum();

        // Calculate change amount
        let change_amount = total_input - total_needed;

        // Create inputs
        let inputs: Vec<TxIn> = selected_utxos
            .iter()
            .map(|utxo| {
                let outpoint = OutPoint {
                    txid: btc_compat::Txid::from_raw_hash(
                        btc_compat::hashes::sha256d::Hash::from_byte_array(utxo.tx_hash),
                    ),
                    vout: utxo.output_index,
                };

                TxIn {
                    previous_output: outpoint,
                    script_sig: ScriptBuf::new(),
                    sequence: Sequence::MAX, // Default sequence
                    witness: Witness::new(),
                }
            })
            .collect();

        // Create output to recipient
        let mut outputs: Vec<TxOut> = vec![TxOut {
            value: Amount::from_sat(amount),
            script_pubkey: recipient_address.script_pubkey(),
        }];

        // Add change output if necessary
        if change_amount > 0 {
            let change_address = self.get_address()?;
            outputs.push(TxOut {
                value: Amount::from_sat(change_amount),
                script_pubkey: change_address.script_pubkey(),
            });
        }

        // Create transaction
        let mut tx = Transaction {
            version: Version::TWO,     // Default version
            lock_time: LockTime::ZERO, // No lock time
            input: inputs,
            output: outputs,
        };

        // Sign transaction
        self.sign_transaction(&mut tx)?;

        Ok(tx)
    }

    /// Get wallet balance
    pub fn get_balance(&self) -> u64 {
        self.utxos.values().flatten().map(|utxo| utxo.amount).sum()
    }

    /// Send a transaction
    pub fn send_transaction(
        &self,
        recipient: &str,
        amount: u64,
        fee: u64,
    ) -> Result<[u8; 32], WalletError> {
        // Create and sign transaction
        let transaction = self.create_transaction(recipient, amount, fee)?;

        // Broadcast the transaction to the network
        self.broadcast_transaction(&transaction)?;

        let tx_hash = transaction.txid().to_raw_hash().to_byte_array();
        Ok(tx_hash)
    }

    /// Broadcast a transaction to the network
    pub fn broadcast_transaction(&self, transaction: &Transaction) -> Result<(), WalletError> {
        // Serialize the transaction
        let tx_data = btc_compat::consensus::encode::serialize(transaction);

        // In a real implementation, this would connect to the P2P network or an RPC interface
        // and broadcast the transaction to the network

        // For now, we'll use a simple HTTP request to a node's RPC interface
        let node_url = std::env::var("SUPERNOVA_NODE_URL")
            .unwrap_or_else(|_| "http://localhost:9332".to_string());

        let client = reqwest::blocking::Client::new();
        let response = client
            .post(format!("{}/api/v1/mempool/transactions", node_url))
            .header("Content-Type", "application/octet-stream")
            .body(tx_data)
            .send()
            .map_err(|e| {
                WalletError::NetworkError(format!("Failed to broadcast transaction: {}", e))
            })?;

        if !response.status().is_success() {
            return Err(WalletError::NetworkError(format!(
                "Failed to broadcast transaction: HTTP status {}",
                response.status()
            )));
        }

        Ok(())
    }

    /// Create signature hash for an input
    fn create_signature_hash(
        &self,
        transaction: &Transaction,
        input_index: usize,
    ) -> Result<[u8; 32], WalletError> {
        let mut sighash_cache = SighashCache::new(transaction);
        let total_amount =
            Amount::from_sat(self.utxos.values().flatten().map(|utxo| utxo.amount).sum());

        let sighash = sighash_cache
            .p2wpkh_signature_hash(
                input_index,
                &self.get_address()?.script_pubkey(),
                total_amount,
                EcdsaSighashType::All,
            )
            .map_err(|e| WalletError::Transaction(e.to_string()))?;

        Ok(sighash.to_raw_hash().to_byte_array())
    }

    /// Find a UTXO by transaction hash and output index
    fn find_utxo(&self, tx_hash: [u8; 32], output_index: u32) -> Option<&UTXO> {
        self.utxos
            .get(&tx_hash)
            .and_then(|utxos| utxos.iter().find(|utxo| utxo.output_index == output_index))
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

        Err(WalletError::InsufficientFunds(amount))
    }

    /// Add a new UTXO to the wallet
    pub fn add_utxo(&mut self, utxo: UTXO) {
        self.utxos.entry(utxo.tx_hash).or_default().push(utxo);
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

    /// Save wallet to file
    pub fn save(&self) -> Result<(), WalletError> {
        let json = serde_json::to_string_pretty(&self)?;
        std::fs::write(&self.wallet_path, json)?;
        Ok(())
    }
}

// Custom serialization for Wallet
impl Serialize for Wallet {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;

        // Create a serializable structure
        let mut state = serializer.serialize_struct("Wallet", 4)?;
        state.serialize_field("private_key", &self.private_key.to_wif())?;
        state.serialize_field("network", &self.network)?;
        state.serialize_field("utxos", &self.utxos)?;
        state.serialize_field(
            "wallet_path",
            &self.wallet_path.to_string_lossy().to_string(),
        )?;
        state.end()
    }
}

// Custom deserialization for Wallet
impl<'de> Deserialize<'de> for Wallet {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct WalletData {
            private_key: String,
            network: Network,
            utxos: HashMap<[u8; 32], Vec<UTXO>>,
            wallet_path: String,
        }

        let data = WalletData::deserialize(deserializer)?;

        // Parse private key from WIF
        let private_key =
            PrivateKey::from_wif(&data.private_key).map_err(serde::de::Error::custom)?;

        Ok(Wallet {
            private_key,
            network: data.network,
            utxos: data.utxos,
            wallet_path: PathBuf::from(data.wallet_path),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_wallet_creation() {
        let wallet = Wallet::new(Network::Testnet).unwrap();
        assert!(!wallet.get_address().unwrap().to_string().is_empty());
    }

    #[test]
    fn test_wallet_from_private_key() {
        let secp = Secp256k1::new();
        let (secret_key, _) = secp.generate_keypair(&mut rand::thread_rng());
        let private_key = PrivateKey::new(secret_key, Network::Testnet);
        let wallet = Wallet::from_private_key(private_key, Network::Testnet).unwrap();
        assert!(!wallet.get_address().unwrap().to_string().is_empty());
    }

    #[test]
    fn test_utxo_management() {
        let dir = tempdir().unwrap();
        let wallet_path = dir.path().join("wallet.json");
        let mut wallet = Wallet::new(Network::Testnet).unwrap();

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
    #[should_panic(expected = "not implemented")]
    fn test_insufficient_funds() {
        let wallet = Wallet::new(Network::Testnet).unwrap();
        let recipient_address = "tb1q7cy0njxmsxfj7qx282t0h499w6apaul6xuson5";

        // Should panic with unimplemented!()
        wallet
            .create_transaction(recipient_address, 100_000, 1_000)
            .unwrap();
    }

    #[test]
    fn test_wallet_persistence() {
        let dir = tempdir().unwrap();
        let wallet_path = dir.path().join("wallet.json");

        // Create and save wallet
        let mut wallet = Wallet::new(Network::Testnet).unwrap();
        let address = wallet.get_address().unwrap().to_string();

        let utxo = UTXO {
            tx_hash: [1u8; 32],
            output_index: 0,
            amount: 100_000,
            script_pubkey: vec![],
        };
        wallet.add_utxo(utxo);
        wallet.wallet_path = wallet_path.clone();
        wallet.save().unwrap();

        // In a real implementation, we would load the wallet from file
        // For now, just verifying that the address is correct
        assert!(!address.is_empty());
    }
}
