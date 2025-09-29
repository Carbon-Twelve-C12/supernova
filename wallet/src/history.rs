use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum HistoryError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("Transaction not found")]
    TransactionNotFound,
    #[error("Invalid transaction data")]
    InvalidTransactionData,
}

/// Transaction direction
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TransactionDirection {
    Sent,
    Received,
}

/// Transaction status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TransactionStatus {
    Pending,
    Confirmed(u32), // Number of confirmations
    Failed,
}

/// Transaction record with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionRecord {
    pub hash: String,
    pub timestamp: DateTime<Utc>,
    pub direction: TransactionDirection,
    pub amount: u64,
    pub fee: u64,
    pub status: TransactionStatus,
    pub label: Option<String>,
    pub category: Option<String>,
    pub tags: Vec<String>,
}

/// Transaction history manager
#[derive(Clone)]
pub struct TransactionHistory {
    transactions: HashMap<String, TransactionRecord>,
    history_path: PathBuf,
}

impl TransactionHistory {
    /// Create a new transaction history tracker
    pub fn new(history_path: PathBuf) -> Result<Self, HistoryError> {
        let mut history = Self {
            transactions: HashMap::new(),
            history_path,
        };

        history.load()?;
        Ok(history)
    }

    /// Add a transaction to history
    pub fn add_transaction(&mut self, record: TransactionRecord) -> Result<(), HistoryError> {
        self.transactions.insert(record.hash.clone(), record);
        self.save()?;
        Ok(())
    }

    /// Update transaction status
    pub fn update_transaction_status(
        &mut self,
        hash: &str,
        status: TransactionStatus,
    ) -> Result<(), HistoryError> {
        if let Some(record) = self.transactions.get_mut(hash) {
            record.status = status;
            self.save()?;
            Ok(())
        } else {
            Err(HistoryError::TransactionNotFound)
        }
    }

    /// Update transaction label
    pub fn add_transaction_label(&mut self, hash: &str, label: String) -> Result<(), HistoryError> {
        if let Some(record) = self.transactions.get_mut(hash) {
            record.label = Some(label);
            self.save()?;
            Ok(())
        } else {
            Err(HistoryError::TransactionNotFound)
        }
    }

    /// Update transaction category
    pub fn add_transaction_category(
        &mut self,
        hash: &str,
        category: String,
    ) -> Result<(), HistoryError> {
        if let Some(record) = self.transactions.get_mut(hash) {
            record.category = Some(category);
            self.save()?;
            Ok(())
        } else {
            Err(HistoryError::TransactionNotFound)
        }
    }

    /// Update transaction tag
    pub fn add_transaction_tag(&mut self, hash: &str, tag: String) -> Result<(), HistoryError> {
        if let Some(record) = self.transactions.get_mut(hash) {
            record.tags.push(tag);
            self.save()?;
            Ok(())
        } else {
            Err(HistoryError::TransactionNotFound)
        }
    }

    /// Get transaction by hash
    pub fn get_transaction(&self, hash: &str) -> Option<&TransactionRecord> {
        self.transactions.get(hash)
    }

    /// Get all transactions
    pub fn get_all_transactions(&self) -> Vec<&TransactionRecord> {
        let mut transactions: Vec<_> = self.transactions.values().collect();
        transactions.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
        transactions
    }

    /// Get recent transactions
    pub fn get_recent_transactions(&self, count: usize) -> Vec<&TransactionRecord> {
        let mut transactions = self.get_all_transactions();
        transactions.truncate(count);
        transactions
    }

    /// Get transactions by category
    pub fn get_transactions_by_category(&self, category: &str) -> Vec<&TransactionRecord> {
        self.transactions
            .values()
            .filter(|tx| tx.category.as_deref() == Some(category))
            .collect()
    }

    /// Get transactions by tag
    pub fn get_transactions_by_tag(&self, tag: &str) -> Vec<&TransactionRecord> {
        self.transactions
            .values()
            .filter(|tx| tx.tags.contains(&tag.to_string()))
            .collect()
    }

    /// Get total sent amount
    pub fn get_total_sent(&self) -> u64 {
        self.transactions
            .values()
            .filter(|tx| matches!(tx.direction, TransactionDirection::Sent))
            .map(|tx| tx.amount)
            .sum()
    }

    /// Get total received amount
    pub fn get_total_received(&self) -> u64 {
        self.transactions
            .values()
            .filter(|tx| matches!(tx.direction, TransactionDirection::Received))
            .map(|tx| tx.amount)
            .sum()
    }

    /// Get total fees paid
    pub fn get_total_fees(&self) -> u64 {
        self.transactions.values().map(|tx| tx.fee).sum()
    }

    /// Calculate net flow (received - sent - fees)
    pub fn get_net_flow(&self) -> i64 {
        let sent = self.get_total_sent() as i64;
        let received = self.get_total_received() as i64;
        received - sent
    }

    fn load(&mut self) -> Result<(), HistoryError> {
        if self.history_path.exists() {
            let data = std::fs::read_to_string(&self.history_path)?;
            self.transactions = serde_json::from_str(&data)?;
        }
        Ok(())
    }

    fn save(&self) -> Result<(), HistoryError> {
        let data = serde_json::to_string_pretty(&self.transactions)?;
        std::fs::write(&self.history_path, data)?;
        Ok(())
    }
}

impl std::fmt::Display for TransactionStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TransactionStatus::Pending => write!(f, "Pending"),
            TransactionStatus::Confirmed(confirmations) => {
                write!(f, "Confirmed ({})", confirmations)
            }
            TransactionStatus::Failed => write!(f, "Failed"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_transaction_history() {
        let dir = tempdir().unwrap();
        let history_path = dir.path().join("history.json");
        let mut history = TransactionHistory::new(history_path).unwrap();

        // Create a test transaction
        let tx = TransactionRecord {
            hash: "test_hash".to_string(),
            timestamp: Utc::now(),
            direction: TransactionDirection::Sent,
            amount: 1000,
            fee: 10,
            status: TransactionStatus::Pending,
            label: None,
            category: None,
            tags: vec![],
        };

        // Add transaction
        history.add_transaction(tx.clone()).unwrap();

        // Verify transaction was added
        assert_eq!(history.get_transaction("test_hash").unwrap().amount, 1000);

        // Update status
        history
            .update_transaction_status("test_hash", TransactionStatus::Confirmed(1))
            .unwrap();
        assert!(matches!(
            history.get_transaction("test_hash").unwrap().status,
            TransactionStatus::Confirmed(1)
        ));

        // Add label
        history
            .add_transaction_label("test_hash", "Test Transaction".to_string())
            .unwrap();
        assert_eq!(
            history
                .get_transaction("test_hash")
                .unwrap()
                .label
                .as_deref(),
            Some("Test Transaction")
        );

        // Add category
        history
            .add_transaction_category("test_hash", "Test Category".to_string())
            .unwrap();
        assert_eq!(
            history
                .get_transaction("test_hash")
                .unwrap()
                .category
                .as_deref(),
            Some("Test Category")
        );

        // Add tag
        history
            .add_transaction_tag("test_hash", "test".to_string())
            .unwrap();
        assert!(history
            .get_transaction("test_hash")
            .unwrap()
            .tags
            .contains(&"test".to_string()));

        // Test totals
        assert_eq!(history.get_total_sent(), 1000);
        assert_eq!(history.get_total_received(), 0);
        assert_eq!(history.get_total_fees(), 10);
        assert_eq!(history.get_net_flow(), -1000);
    }
}
