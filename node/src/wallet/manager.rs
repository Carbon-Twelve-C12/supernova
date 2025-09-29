//! Wallet Manager
//!
//! This module provides a high-level interface for managing wallets within the node.

use std::sync::{Arc, RwLock};
use std::path::PathBuf;
use anyhow::Result;
use rpassword::read_password;

use crate::config::WalletConfig;
use btclib::wallet::quantum_wallet::{QuantumWallet, QuantumAddress, WalletError};
use btclib::types::{Transaction, TransactionInput, TransactionOutput};
use btclib::types::address::Address;
use btclib::types::utxo::{UtxoSet, UtxoEntry};
use btclib::types::chain_state::ChainState;

pub struct WalletManager {
    wallet: Arc<RwLock<QuantumWallet>>,
}

impl WalletManager {
    pub fn new(config: &WalletConfig) -> Result<Self> {
        let wallet_path = PathBuf::from(&config.wallet_file);
        let wallet = if wallet_path.exists() {
            println!("Wallet found. Please enter password to unlock:");
            let password = read_password()?;
            let encrypted_wallet = std::fs::read_to_string(&wallet_path)?;
            QuantumWallet::import_encrypted(&encrypted_wallet, &password)?
        } else {
            println!("No wallet found. Creating a new one.");
            println!("Please enter a password for your new wallet:");
            let password = read_password()?;
            let mnemonic = QuantumWallet::generate_mnemonic()?;
            println!("*** IMPORTANT *** Please write down this mnemonic phrase and keep it safe:");
            println!("\n{}\n", mnemonic);

            let wallet = QuantumWallet::from_mnemonic(&mnemonic, &password, "testnet", btclib::crypto::quantum::QuantumScheme::Dilithium, 3)?;
            let encrypted_wallet = wallet.export_encrypted(&password)?;
            std::fs::write(&wallet_path, encrypted_wallet)?;
            println!("Wallet created and saved to: {:?}", wallet_path);
            wallet
        };

        Ok(Self {
            wallet: Arc::new(RwLock::new(wallet)),
        })
    }

    pub async fn get_new_address(&self) -> Result<QuantumAddress> {
        let mut wallet = self.wallet.write().await;
        let address = wallet.new_address()?;
        Ok(address)
    }

    pub async fn create_transaction(
        &self,
        recipient_str: &str,
        amount: u64,
        fee_rate: u64,
        utxo_set: &UtxoSet,
        chain_state: &ChainState,
    ) -> Result<Transaction> {
        let mut wallet = self.wallet.write().await;

        // 1. Select UTXOs
        let all_addresses: Vec<_> = wallet.addresses.values().cloned().collect();
        let available_utxos = utxo_set.get_utxos_for_addresses(&all_addresses);

        let (selected_utxos, change) = self.select_utxos(available_utxos, amount, fee_rate)?;

        // 2. Create inputs
        let inputs: Vec<_> = selected_utxos.iter()
            .map(|utxo| TransactionInput::new(utxo.outpoint.txid, utxo.outpoint.vout, vec![], 0xffffffff))
            .collect();

        // 3. Create outputs
        let recipient_address = Address::from_str(recipient_str).map_err(|e| anyhow::anyhow!(e))?;
        let mut outputs = vec![TransactionOutput::new(amount, recipient_address.script_pubkey().as_bytes().to_vec())];

        if change > 0 {
            let change_address = wallet.new_address()?;
            let change_script_pubkey = Address::from_str(&change_address.address)
                .map_err(|e| anyhow::anyhow!(e))?
                .script_pubkey();
            outputs.push(TransactionOutput::new(change, change_script_pubkey.as_bytes().to_vec()));
        }

        // 4. Create unsigned transaction
        let mut tx = Transaction::new(2, inputs, outputs, chain_state.get_height() as u32);

        // 5. Sign transaction
        for (i, utxo) in selected_utxos.iter().enumerate() {
            let address_info = wallet.addresses.values().find(|a| a.address == utxo.address).ok_or_else(|| anyhow::anyhow!("Address not found for UTXO"))?;
            wallet.sign_transaction(&mut tx, i, address_info.index)?;
        }

        Ok(tx)
    }

    fn select_utxos(
        &self,
        utxos: Vec<UtxoEntry>,
        target_amount: u64,
        fee_rate: u64,
    ) -> Result<(Vec<UtxoEntry>, u64)> {
        let mut selected_utxos = Vec::new();
        let mut total_amount: u64 = 0;
        let estimated_tx_size = 250; // Simple estimation for now
        let fee = fee_rate * estimated_tx_size;

        for utxo in utxos {
            if total_amount >= target_amount + fee {
                break;
            }
            total_amount += utxo.amount();
            selected_utxos.push(utxo);
        }

        if total_amount < target_amount + fee {
            return Err(anyhow::anyhow!("Insufficient funds"));
        }

        let change = total_amount - target_amount - fee;
        Ok((selected_utxos, change))
    }
}