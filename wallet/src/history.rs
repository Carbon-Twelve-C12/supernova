use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};
use serde::{Serialize, Deserialize};
use btclib::types::Transaction;
use crate::hdwallet::HDWallet;

/// Transaction direction
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TransactionDirection {
    Incoming,
    Outgoing,
    SelfTransfer,
}

/// Transaction status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TransactionStatus {
    Pending,
    Confirmed(u64), // Confirmations
    Failed,
}

/// Transaction record with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionRecord {
    pub tx_hash: String,
    pub timestamp: u64,
    pub direction: TransactionDirection,
    pub amount: u64,
    pub fee: u64,
    pub status: TransactionStatus,
    pub block_height: Option<u64>,
    pub addresses: Vec<String>, // Involved addresses
    pub label: Option<String>,
}

/// Transaction history manager
#[derive(Debug, Serialize, Deserialize)]
pub struct TransactionHistory {
    transactions: HashMap<String, TransactionRecord>,
    address_transactions: HashMap<String, Vec<String>>, // Map from address to tx hashes
}

impl TransactionHistory {
    /// Create a new transaction history tracker
    pub fn new() -> Self {
        Self {
            transactions: HashMap::new(),
            address_transactions: HashMap::new(),
        }
    }
    
    /// Add a transaction to history
    pub fn add_transaction(&mut self, 
                          tx: &Transaction, 
                          direction: TransactionDirection,
                          amount: u64,
                          fee: u64,
                          wallet: &HDWallet) -> TransactionRecord {
        let tx_hash = hex::encode(tx.hash());
        
        // Check if we already have this transaction
        if let Some(record) = self.transactions.get(&tx_hash) {
            return record.clone();
        }
        
        // Determine involved addresses
        let mut addresses = Vec::new();
        
        // For outputs, we can determine our addresses
        for (i, output) in tx.outputs().iter().enumerate() {
            let script_pubkey = output.pub_key_script();
            
            // Check if this output belongs to our wallet
            if let Some(addr) = wallet.find_address_by_pubkey(&script_pubkey) {
                addresses.push(addr.clone());
            }
        }
        
        // Create transaction record
        let record = TransactionRecord {
            tx_hash: tx_hash.clone(),
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            direction,
            amount,
            fee,
            status: TransactionStatus::Pending,
            block_height: None,
            addresses: addresses.clone(),
            label: wallet.get_transaction_label(&tx_hash).cloned(),
        };
        
        // Add to main transactions map
        self.transactions.insert(tx_hash.clone(), record.clone());
        
        // Add to address index
        for address in addresses {
            self.address_transactions.entry(address)
                .or_insert_with(Vec::new)
                .push(tx_hash.clone());
        }
        
        record
    }
    
    /// Update transaction status
    pub fn update_status(&mut self, tx_hash: &str, status: TransactionStatus, block_height: Option<u64>) {
        if let Some(record) = self.transactions.get_mut(tx_hash) {
            record.status = status;
            record.block_height = block_height;
        }
    }
    
    /// Update transaction label
    pub fn update_label(&mut self, tx_hash: &str, label: Option<String>) {
        if let Some(record) = self.transactions.get_mut(tx_hash) {
            record.label = label;
        }
    }
    
    /// Get transaction by hash
    pub fn get_transaction(&self, tx_hash: &str) -> Option<&TransactionRecord> {
        self.transactions.get(tx_hash)
    }
    
    /// Get all transactions
    pub fn get_all_transactions(&self) -> Vec<&TransactionRecord> {
        let mut records: Vec<_> = self.transactions.values().collect();
        records.sort_by(|a, b| b.timestamp.cmp(&a.timestamp)); // Sort by time, newest first
        records
    }
    
    /// Get transactions for a specific address
    pub fn get_address_transactions(&self, address: &str) -> Vec<&TransactionRecord> {
        match self.address_transactions.get(address) {
            Some(tx_hashes) => {
                let mut records = Vec::new();
                for hash in tx_hashes {
                    if let Some(record) = self.transactions.get(hash) {
                        records.push(record);
                    }
                }
                records.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
                records
            }
            None => Vec::new(),
        }
    }
    
    /// Get transactions by type (incoming/outgoing)
    pub fn get_transactions_by_direction(&self, direction: TransactionDirection) -> Vec<&TransactionRecord> {
        let mut records: Vec<_> = self.transactions.values()
            .filter(|r| r.direction == direction)
            .collect();
        records.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
        records
    }
    
    /// Get transactions by status
    pub fn get_transactions_by_status(&self, status_filter: fn(&TransactionStatus) -> bool) -> Vec<&TransactionRecord> {
        let mut records: Vec<_> = self.transactions.values()
            .filter(|r| status_filter(&r.status))
            .collect();
        records.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
        records
    }
    
    /// Search transactions by label
    pub fn search_by_label(&self, query: &str) -> Vec<&TransactionRecord> {
        let query = query.to_lowercase();
        let mut records: Vec<_> = self.transactions.values()
            .filter(|r| {
                r.label.as_ref()
                    .map(|label| label.to_lowercase().contains(&query))
                    .unwrap_or(false)
            })
            .collect();
        records.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
        records
    }
    
    /// Get total sent amount
    pub fn get_total_sent(&self) -> u64 {
        self.transactions.values()
            .filter(|r| r.direction == TransactionDirection::Outgoing)
            .map(|r| r.amount)
            .sum()
    }
    
    /// Get total received amount
    pub fn get_total_received(&self) -> u64 {
        self.transactions.values()
            .filter(|r| r.direction == TransactionDirection::Incoming)
            .map(|r| r.amount)
            .sum()
    }
    
    /// Get total fees paid
    pub fn get_total_fees(&self) -> u64 {
        self.transactions.values()
            .filter(|r| r.direction == TransactionDirection::Outgoing)
            .map(|r| r.fee)
            .sum()
    }
    
    /// Calculate net flow (received - sent - fees)
    pub fn get_net_flow(&self) -> i64 {
        let received = self.get_total_received() as i64;
        let sent = self.get_total_sent() as i64;
        let fees = self.get_total_fees() as i64;
        
        received - sent - fees
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use btclib::types::{TransactionInput, TransactionOutput};
    use crate::hdwallet::HDWallet;
    
    // Create test transaction
    fn create_test_transaction(inputs: Vec<TransactionInput>, outputs: Vec<TransactionOutput>) -> Transaction {
        Transaction::new(1, inputs, outputs, 0)
    }
    
    // This test requires setup with an HDWallet, which we'll skip for simplicity
    #[test]
    fn test_transaction_history_basic() {
        let mut history = TransactionHistory::new();
        
        // These assertions don't require an HDWallet
        assert_eq!(history.get_all_transactions().len(), 0);
        assert_eq!(history.get_total_sent(), 0);
        assert_eq!(history.get_total_received(), 0);
        assert_eq!(history.get_total_fees(), 0);
        assert_eq!(history.get_net_flow(), 0);
    }
}