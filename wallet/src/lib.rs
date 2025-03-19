use serde::{Deserialize, Serialize};
use thiserror::Error;
use btclib::types::transaction::{Transaction, TransactionInput, TransactionOutput};

#[derive(Error, Debug)]
pub enum WalletError {
    #[error("Not implemented")]
    NotImplemented,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct UTXO {
    pub tx_hash: [u8; 32],
    pub output_index: u32,
    pub amount: u64,
    pub script_pubkey: Vec<u8>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Wallet {
    version: u32,
}

impl Wallet {
    pub fn new() -> Self {
        Self {
            version: 1,
        }
    }

    pub fn create_transaction(&self, _to: &str, _amount: u64) -> Result<Transaction, WalletError> {
        Err(WalletError::NotImplemented)
    }
}

pub fn stub_function() -> &'static str {
    "This is a stub wallet implementation to satisfy dependencies"
} 