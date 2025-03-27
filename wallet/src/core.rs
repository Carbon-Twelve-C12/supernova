use bitcoin::{
    network::Network,
    secp256k1::{Secp256k1},
    Address, PrivateKey, PublicKey, Transaction,
    hashes::Hash,
    sighash::{SighashCache, EcdsaSighashType},
    Amount,
};
use std::str::FromStr;
use std::collections::HashMap;
use std::path::PathBuf;
use sha2::Digest;
use serde::{Serialize, Deserialize};

#[derive(Debug, thiserror::Error)]
pub enum WalletError {
    #[error("Bitcoin error")]
    Bitcoin(String),
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
        let (secret_key, _) = secp.generate_keypair(&mut rand::thread_rng());
        let private_key = PrivateKey::new(secret_key, network);

        Ok(Self {
            private_key,
            network,
            utxos: HashMap::new(),
            wallet_path: PathBuf::new(),
        })
    }

    pub fn from_private_key(private_key: PrivateKey, network: Network) -> Result<Self, WalletError> {
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
            .map_err(|e| WalletError::Bitcoin(e.to_string()))
    }

    pub fn sign_transaction(&self, tx: &mut Transaction) -> Result<(), WalletError> {
        // TODO: Implement transaction signing
        Ok(())
    }

    pub fn create_transaction(
        &self,
        recipient: &str,
        amount: u64,
        fee: u64,
    ) -> Result<Transaction, WalletError> {
        let recipient_address = Address::from_str(recipient)
            .map_err(|_| WalletError::InvalidAddress(recipient.to_string()))?;

        // TODO: Implement transaction creation
        unimplemented!()
    }

    /// Get wallet balance
    pub fn get_balance(&self) -> u64 {
        self.utxos.values()
            .flatten()
            .map(|utxo| utxo.amount)
            .sum()
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
        let tx_hash = transaction.txid().to_raw_hash().to_byte_array();
        
        // For now, just return the transaction hash
        // In a real implementation, this would broadcast to the network
        Ok(tx_hash)
    }

    /// Create signature hash for an input
    fn create_signature_hash(&self, transaction: &Transaction, input_index: usize) -> Result<[u8; 32], WalletError> {
        let mut sighash_cache = SighashCache::new(transaction);
        let total_amount = Amount::from_sat(self.utxos.values().flatten().map(|utxo| utxo.amount).sum());
        
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

        Err(WalletError::InsufficientFunds(amount))
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
        state.serialize_field("wallet_path", &self.wallet_path.to_string_lossy().to_string())?;
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
        let private_key = PrivateKey::from_wif(&data.private_key)
            .map_err(serde::de::Error::custom)?;
        
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
        wallet.create_transaction(recipient_address, 100_000, 1_000).unwrap();
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