use bip39::{Language, Mnemonic};
use bitcoin::{
    network::Network,
    secp256k1::{Secp256k1, SecretKey},
    Address, PrivateKey,
};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, path::PathBuf, str::FromStr};
use thiserror::Error;
use rand::RngCore;
use btclib::storage::utxo_set::UtxoSet;

#[derive(Error, Debug)]
pub enum HDWalletError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("Invalid mnemonic: {0}")]
    InvalidMnemonic(String),
    #[error("Account not found: {0}")]
    AccountNotFound(String),
    #[error("Address not found: {0}")]
    AddressNotFound(String),
    #[error("Bitcoin error: {0}")]
    Bitcoin(String),
    #[error("Address parsing error: {0}")]
    AddressParsing(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HDWallet {
    mnemonic: String,
    network: Network,
    accounts: HashMap<String, HDAccount>,
    wallet_path: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HDAccount {
    pub name: String,
    pub account_type: AccountType,
    pub addresses: Vec<HDAddress>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HDAddress {
    pub address: String,
    pub is_used: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum AccountType {
    Legacy,
    SegWit,
    NativeSegWit,
}

impl HDWallet {
    pub fn new(network: Network, wallet_path: PathBuf) -> Result<Self, HDWalletError> {
        // Generate entropy for a 12-word mnemonic (128 bits = 16 bytes)
        let mut entropy = [0u8; 16];
        rand::thread_rng().fill_bytes(&mut entropy);
        
        // Create mnemonic from entropy
        let mnemonic = Mnemonic::from_entropy(&entropy)
            .map_err(|e| HDWalletError::InvalidMnemonic(e.to_string()))?;
        
        Ok(Self {
            mnemonic: mnemonic.to_string(),
            network,
            accounts: HashMap::new(),
            wallet_path,
        })
    }

    pub fn from_mnemonic(mnemonic: &str, network: Network, wallet_path: PathBuf) -> Result<Self, HDWalletError> {
        Mnemonic::parse_in_normalized(Language::English, mnemonic)
            .map_err(|e| HDWalletError::InvalidMnemonic(e.to_string()))?;
        Ok(Self {
            mnemonic: mnemonic.to_string(),
            network,
            accounts: HashMap::new(),
            wallet_path,
        })
    }

    pub fn save(&self) -> Result<(), HDWalletError> {
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(&self.wallet_path, json)?;
        Ok(())
    }
    
    pub fn load(wallet_path: PathBuf) -> Result<Self, HDWalletError> {
        let json = std::fs::read_to_string(&wallet_path)?;
        let wallet: Self = serde_json::from_str(&json)?;
        Ok(wallet)
    }

    pub fn create_account(&mut self, name: String, account_type: AccountType) -> Result<(), HDWalletError> {
        let account = HDAccount {
            name: name.clone(),
            account_type,
            addresses: Vec::new(),
        };

        self.accounts.insert(name, account);
        self.save()?;
        Ok(())
    }

    pub fn get_new_address(&mut self, account_name: &str) -> Result<HDAddress, HDWalletError> {
        let account = self.accounts.get_mut(account_name)
            .ok_or_else(|| HDWalletError::AccountNotFound(account_name.to_string()))?;
            
        let secp = Secp256k1::new();
        let secret_key = SecretKey::new(&mut rand::thread_rng());
        let private_key = PrivateKey::new(secret_key, self.network);
        let public_key = private_key.public_key(&secp);

        let address = match account.account_type {
            AccountType::Legacy => Address::p2pkh(&public_key, self.network),
            AccountType::SegWit => Address::p2shwpkh(&public_key, self.network)
                .map_err(|e| HDWalletError::Bitcoin(e.to_string()))?,
            AccountType::NativeSegWit => Address::p2wpkh(&public_key, self.network)
                .map_err(|e| HDWalletError::Bitcoin(e.to_string()))?,
        };

        let hd_address = HDAddress {
            address: address.to_string(),
            is_used: false,
        };

        account.addresses.push(hd_address.clone());
        self.save()?;
        Ok(hd_address)
    }

    pub fn get_balance(&self, account_name: &str, utxo_set: &UtxoSet) -> Result<u64, HDWalletError> {
        let account = self.accounts.get(account_name)
            .ok_or_else(|| HDWalletError::AccountNotFound(account_name.to_string()))?;

        let mut balance = 0;
        for hd_address in &account.addresses {
            let address = Address::from_str(&hd_address.address)
                .map_err(|e| HDWalletError::AddressParsing(e.to_string()))?;
            balance += utxo_set.get_balance(&address.script_pubkey());
        }

        Ok(balance)
    }

    pub fn get_total_balance(&self, utxo_set: &UtxoSet) -> Result<u64, HDWalletError> {
        let mut total = 0;
        for account_name in self.accounts.keys() {
            total += self.get_balance(account_name, utxo_set)?;
        }
        Ok(total)
    }

    pub fn list_accounts(&self) -> Vec<(u32, &HDAccount)> {
        self.accounts.iter()
            .enumerate()
            .map(|(i, (_, account))| (i as u32, account))
            .collect()
    }

    pub fn get_address_count(&self) -> usize {
        self.accounts.values()
            .map(|account| account.addresses.len())
            .sum()
    }

    pub fn get_mnemonic(&self) -> &str {
        &self.mnemonic
    }
}

impl HDAccount {
    pub fn add_address(&mut self, address: HDAddress) {
        self.addresses.push(address);
    }
}

impl HDAddress {
    pub fn get_address(&self) -> &str {
        &self.address
    }
}

impl std::str::FromStr for AccountType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "legacy" => Ok(AccountType::Legacy),
            "segwit" => Ok(AccountType::SegWit),
            "native_segwit" => Ok(AccountType::NativeSegWit),
            _ => Err(format!("Invalid account type: {}", s)),
        }
    }
}