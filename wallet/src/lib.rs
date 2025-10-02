// Supernova Wallet Library

// Enforce panic-free code in production
#![cfg_attr(not(test), warn(clippy::unwrap_used))]
#![cfg_attr(not(test), warn(clippy::expect_used))]
#![cfg_attr(not(test), warn(clippy::panic))]
// Allow certain warnings for pragmatic reasons
#![allow(dead_code)]
#![allow(clippy::too_many_arguments)]
#![allow(clippy::large_enum_variant)]
#![allow(clippy::type_complexity)]
// Test-specific allows
#![cfg_attr(test, allow(clippy::unwrap_used))]
#![cfg_attr(test, allow(clippy::expect_used))]
#![cfg_attr(test, allow(clippy::panic))]

pub mod cli;
mod core; // Legacy Bitcoin-based wallet (deprecated)
mod hdwallet;
mod history;
mod ui;

// NEW: Quantum-resistant wallet infrastructure
pub mod quantum_wallet;

use bitcoin::network::Network;
use btclib::storage::utxo_set::UtxoSet;
use std::path::PathBuf;
use thiserror::Error;

pub use core::Wallet;
pub use hdwallet::{AccountType, HDAddress, HDWallet};
pub use history::{TransactionDirection, TransactionHistory, TransactionRecord, TransactionStatus};
pub use ui::tui::WalletTui;

#[derive(Error, Debug)]
pub enum WalletError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("HD wallet error: {0}")]
    HDWallet(#[from] hdwallet::HDWalletError),
    #[error("History error: {0}")]
    History(#[from] history::HistoryError),
    #[error("UI error: {0}")]
    UI(String),
}

pub struct WalletManager {
    hd_wallet: HDWallet,
    transaction_history: TransactionHistory,
    utxo_set: UtxoSet,
}

impl WalletManager {
    pub fn new(wallet_dir: PathBuf, network: Network) -> Result<Self, WalletError> {
        let wallet_path = wallet_dir.join("wallet.json");
        let history_path = wallet_dir.join("history.json");

        let utxo_set = UtxoSet::new_in_memory(1000);
        let hd_wallet = HDWallet::new(network, wallet_path)?;
        let transaction_history = TransactionHistory::new(history_path)?;

        Ok(Self {
            hd_wallet,
            transaction_history,
            utxo_set,
        })
    }

    pub fn load(wallet_dir: PathBuf) -> Result<Self, WalletError> {
        let wallet_path = wallet_dir.join("wallet.json");
        let history_path = wallet_dir.join("history.json");

        let hd_wallet = HDWallet::load(wallet_path)?;
        let transaction_history = TransactionHistory::new(history_path)?;
        let utxo_set = UtxoSet::new_in_memory(1000);

        Ok(Self {
            hd_wallet,
            transaction_history,
            utxo_set,
        })
    }

    pub fn from_mnemonic(
        mnemonic: &str,
        wallet_dir: PathBuf,
        network: Network,
    ) -> Result<Self, WalletError> {
        let wallet_path = wallet_dir.join("wallet.json");
        let history_path = wallet_dir.join("history.json");

        let hd_wallet = HDWallet::from_mnemonic(mnemonic, network, wallet_path)?;
        let transaction_history = TransactionHistory::new(history_path)?;
        let utxo_set = UtxoSet::new_in_memory(1000);

        Ok(Self {
            hd_wallet,
            transaction_history,
            utxo_set,
        })
    }

    pub fn run_tui(&mut self) -> Result<(), WalletError> {
        let mut tui = WalletTui::new(self.hd_wallet.clone(), self.transaction_history.clone())
            .map_err(|e| WalletError::UI(e.to_string()))?;

        tui.run().map_err(|e| WalletError::UI(e.to_string()))?;
        Ok(())
    }

    pub fn create_account(
        &mut self,
        name: String,
        account_type: AccountType,
    ) -> Result<(), WalletError> {
        self.hd_wallet
            .create_account(name, account_type)
            .map_err(WalletError::HDWallet)
    }

    pub fn get_new_address(&mut self, account_name: &str) -> Result<HDAddress, WalletError> {
        self.hd_wallet
            .get_new_address(account_name)
            .map_err(WalletError::HDWallet)
    }

    pub fn get_balance(&self, account_name: &str) -> Result<u64, WalletError> {
        self.hd_wallet
            .get_balance(account_name, &self.utxo_set)
            .map_err(WalletError::HDWallet)
    }

    pub fn get_total_balance(&self) -> Result<u64, WalletError> {
        self.hd_wallet
            .get_total_balance(&self.utxo_set)
            .map_err(WalletError::HDWallet)
    }

    pub fn list_accounts(&self) -> Vec<(u32, &hdwallet::HDAccount)> {
        self.hd_wallet.list_accounts()
    }

    pub fn get_address_count(&self) -> usize {
        self.hd_wallet.get_address_count()
    }

    pub fn add_transaction(&mut self, record: TransactionRecord) -> Result<(), WalletError> {
        self.transaction_history
            .add_transaction(record)
            .map_err(WalletError::History)
    }

    pub fn update_transaction_status(
        &mut self,
        hash: &str,
        status: TransactionStatus,
    ) -> Result<(), WalletError> {
        self.transaction_history
            .update_transaction_status(hash, status)
            .map_err(WalletError::History)
    }

    pub fn add_transaction_label(&mut self, hash: &str, label: String) -> Result<(), WalletError> {
        self.transaction_history
            .add_transaction_label(hash, label)
            .map_err(WalletError::History)
    }

    pub fn add_transaction_category(
        &mut self,
        hash: &str,
        category: String,
    ) -> Result<(), WalletError> {
        self.transaction_history
            .add_transaction_category(hash, category)
            .map_err(WalletError::History)
    }

    pub fn add_transaction_tag(&mut self, hash: &str, tag: String) -> Result<(), WalletError> {
        self.transaction_history
            .add_transaction_tag(hash, tag)
            .map_err(WalletError::History)
    }

    pub fn get_transaction(&self, hash: &str) -> Option<&TransactionRecord> {
        self.transaction_history.get_transaction(hash)
    }

    pub fn get_all_transactions(&self) -> Vec<&TransactionRecord> {
        self.transaction_history.get_all_transactions()
    }

    pub fn get_recent_transactions(&self, count: usize) -> Vec<&TransactionRecord> {
        self.transaction_history.get_recent_transactions(count)
    }

    pub fn get_transactions_by_category(&self, category: &str) -> Vec<&TransactionRecord> {
        self.transaction_history
            .get_transactions_by_category(category)
    }

    pub fn get_transactions_by_tag(&self, tag: &str) -> Vec<&TransactionRecord> {
        self.transaction_history.get_transactions_by_tag(tag)
    }

    pub fn get_total_sent(&self) -> u64 {
        self.transaction_history.get_total_sent()
    }

    pub fn get_total_received(&self) -> u64 {
        self.transaction_history.get_total_received()
    }

    pub fn get_total_fees(&self) -> u64 {
        self.transaction_history.get_total_fees()
    }

    pub fn get_net_flow(&self) -> i64 {
        self.transaction_history.get_net_flow()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_wallet_manager() {
        let dir = tempdir().unwrap();
        let mut manager = WalletManager::new(dir.path().to_path_buf(), Network::Testnet).unwrap();

        // Create an account
        manager
            .create_account("Test Account".to_string(), AccountType::NativeSegWit)
            .unwrap();

        // Get a new address
        let address = manager.get_new_address("Test Account").unwrap();
        assert!(!address.get_address().is_empty());

        // Add a transaction
        let tx = TransactionRecord {
            hash: "test_hash".to_string(),
            timestamp: chrono::Utc::now(),
            direction: TransactionDirection::Received,
            amount: 1000,
            fee: 0,
            status: TransactionStatus::Pending,
            label: None,
            category: None,
            tags: vec![],
        };

        manager.add_transaction(tx).unwrap();

        // Verify transaction was added
        assert_eq!(manager.get_transaction("test_hash").unwrap().amount, 1000);
        assert_eq!(manager.get_total_received(), 1000);
        assert_eq!(manager.get_total_sent(), 0);
        assert_eq!(manager.get_net_flow(), 1000);
    }
}
