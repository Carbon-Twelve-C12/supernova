use std::sync::{Arc, Mutex};
use std::collections::HashMap;
use chrono::{DateTime, Utc};
use btclib::testnet::faucet::{Faucet as BtclibFaucet, FaucetError};

/// Status information for the faucet
pub struct FaucetStatus {
    pub is_active: bool,
    pub balance: u64,
    pub transactions_today: u32,
    pub last_distribution: Option<DateTime<Utc>>,
    pub cooldown_secs: u64,
    pub distribution_amount: u64,
}

/// Result of a coin distribution
pub struct DistributionResult {
    pub txid: String,
    pub amount: u64,
    pub recipient: String,
    pub timestamp: DateTime<Utc>,
}

/// Recent transaction record
pub struct RecentTransaction {
    pub txid: String,
    pub recipient: String,
    pub amount: u64,
    pub timestamp: DateTime<Utc>,
}

/// Faucet wrapper that provides the async interface expected by API routes
pub struct FaucetWrapper {
    inner: Arc<Mutex<BtclibFaucet>>,
    balance: Arc<Mutex<u64>>,
    recent_transactions: Arc<Mutex<Vec<RecentTransaction>>>,
    distribution_amount: u64,
    cooldown_secs: u64,
}

impl FaucetWrapper {
    /// Create a new faucet wrapper
    pub fn new(distribution_amount: u64, cooldown_secs: u64, initial_balance: u64) -> Self {
        Self {
            inner: Arc::new(Mutex::new(BtclibFaucet::new(distribution_amount, cooldown_secs))),
            balance: Arc::new(Mutex::new(initial_balance)),
            recent_transactions: Arc::new(Mutex::new(Vec::new())),
            distribution_amount,
            cooldown_secs,
        }
    }
    
    /// Get faucet status
    pub async fn status(&self) -> Result<FaucetStatus, FaucetError> {
        let balance = *self.balance.lock().unwrap();
        let transactions = self.recent_transactions.lock().unwrap();
        
        let transactions_today = transactions.iter()
            .filter(|tx| {
                let today = Utc::now().date_naive();
                tx.timestamp.date_naive() == today
            })
            .count() as u32;
        
        let last_distribution = transactions.last().map(|tx| tx.timestamp);
        
        Ok(FaucetStatus {
            is_active: balance > 0,
            balance,
            transactions_today,
            last_distribution,
            cooldown_secs: self.cooldown_secs,
            distribution_amount: self.distribution_amount,
        })
    }
    
    /// Distribute coins to an address
    pub async fn distribute_coins(&self, address: &str) -> Result<DistributionResult, FaucetError> {
        // Check balance
        {
            let balance = self.balance.lock().unwrap();
            if *balance < self.distribution_amount {
                return Err(FaucetError::InsufficientFunds);
            }
        }
        
        // Use inner faucet to handle cooldown and validation
        let amount = {
            let mut faucet = self.inner.lock().unwrap();
            faucet.distribute_coins(address)?
        };
        
        // Deduct from balance
        {
            let mut balance = self.balance.lock().unwrap();
            *balance -= amount;
        }
        
        // Create transaction record
        let timestamp = Utc::now();
        let txid = format!("{:x}", timestamp.timestamp());
        
        let transaction = RecentTransaction {
            txid: txid.clone(),
            recipient: address.to_string(),
            amount,
            timestamp,
        };
        
        // Add to recent transactions
        {
            let mut transactions = self.recent_transactions.lock().unwrap();
            transactions.push(transaction);
            
            // Keep only last 100 transactions
            if transactions.len() > 100 {
                transactions.remove(0);
            }
        }
        
        Ok(DistributionResult {
            txid,
            amount,
            recipient: address.to_string(),
            timestamp,
        })
    }
    
    /// Get recent transactions
    pub async fn get_recent_transactions(&self) -> Result<Vec<RecentTransaction>, FaucetError> {
        let transactions = self.recent_transactions.lock().unwrap();
        Ok(transactions.clone().into_iter().rev().take(10).collect())
    }
}

// Implement Clone for RecentTransaction
impl Clone for RecentTransaction {
    fn clone(&self) -> Self {
        Self {
            txid: self.txid.clone(),
            recipient: self.recipient.clone(),
            amount: self.amount,
            timestamp: self.timestamp,
        }
    }
} 