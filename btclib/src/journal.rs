// Journal module for transaction history and logging
// Provides backwards compatibility

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JournalEntry {
    pub timestamp: DateTime<Utc>,
    pub entry_type: String,
    pub data: serde_json::Value,
}

#[derive(Debug, Default)]
pub struct Journal {
    entries: Vec<JournalEntry>,
}

impl Journal {
    pub fn new() -> Self {
        Self::default()
    }
    
    pub fn add_entry(&mut self, entry_type: String, data: serde_json::Value) {
        self.entries.push(JournalEntry {
            timestamp: Utc::now(),
            entry_type,
            data,
        });
    }
    
    pub fn get_entries(&self) -> &[JournalEntry] {
        &self.entries
    }
} 