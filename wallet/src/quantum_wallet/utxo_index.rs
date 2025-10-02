// UTXO Index for Quantum Wallet
// Tracks unspent transaction outputs for efficient balance calculation

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, RwLock};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum UtxoError {
    #[error("UTXO not found: {txid}:{vout}")]
    NotFound { txid: String, vout: u32 },
    
    #[error("UTXO already spent: {txid}:{vout}")]
    AlreadySpent { txid: String, vout: u32 },
    
    #[error("Invalid UTXO data: {0}")]
    InvalidData(String),
    
    #[error("Lock poisoned: {0}")]
    LockPoisoned(String),
    
    #[error("Database error: {0}")]
    DatabaseError(String),
}

/// Unspent Transaction Output
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Utxo {
    /// Transaction ID (SHA-256 hash, 32 bytes)
    pub txid: [u8; 32],
    
    /// Output index in transaction
    pub vout: u32,
    
    /// Address that owns this UTXO
    pub address: String,
    
    /// Amount in attonovas (smallest unit)
    pub value: u64,
    
    /// Script pubkey
    pub script_pubkey: Vec<u8>,
    
    /// Block height where this UTXO was created
    pub block_height: u64,
    
    /// Number of confirmations
    pub confirmations: u64,
    
    /// Is this spendable? (not locked, not watch-only)
    pub spendable: bool,
    
    /// Is this solvable? (we have the keys)
    pub solvable: bool,
    
    /// Optional label
    pub label: Option<String>,
}

impl Utxo {
    /// Get outpoint as string
    pub fn outpoint(&self) -> String {
        format!("{}:{}", hex::encode(&self.txid), self.vout)
    }
    
    /// Calculate value in NOVA (with 8 decimal places)
    pub fn value_nova(&self) -> f64 {
        self.value as f64 / 100_000_000.0
    }
}

/// UTXO Index for wallet
pub struct UtxoIndex {
    /// UTXOs indexed by address
    utxos_by_address: Arc<RwLock<HashMap<String, Vec<Utxo>>>>,
    
    /// All UTXOs indexed by outpoint for quick lookup
    utxos_by_outpoint: Arc<RwLock<HashMap<String, Utxo>>>,
    
    /// Spent UTXOs (for reorg handling)
    spent: Arc<RwLock<HashSet<String>>>,
    
    /// Pending (unconfirmed) UTXOs
    pending: Arc<RwLock<HashMap<String, Utxo>>>,
    
    /// Current blockchain height
    current_height: Arc<RwLock<u64>>,
}

impl UtxoIndex {
    /// Create new UTXO index
    pub fn new() -> Self {
        Self {
            utxos_by_address: Arc::new(RwLock::new(HashMap::new())),
            utxos_by_outpoint: Arc::new(RwLock::new(HashMap::new())),
            spent: Arc::new(RwLock::new(HashSet::new())),
            pending: Arc::new(RwLock::new(HashMap::new())),
            current_height: Arc::new(RwLock::new(0)),
        }
    }
    
    /// Add a UTXO to the index
    pub fn add_utxo(&self, mut utxo: Utxo) -> Result<(), UtxoError> {
        let outpoint = utxo.outpoint();
        
        // Update confirmations based on current height
        let current_height = *self.current_height.read()
            .map_err(|e| UtxoError::LockPoisoned(e.to_string()))?;
        
        if utxo.block_height > 0 {
            utxo.confirmations = current_height.saturating_sub(utxo.block_height) + 1;
        }
        
        // Add to address index
        self.utxos_by_address.write()
            .map_err(|e| UtxoError::LockPoisoned(e.to_string()))?
            .entry(utxo.address.clone())
            .or_insert_with(Vec::new)
            .push(utxo.clone());
        
        // Add to outpoint index
        self.utxos_by_outpoint.write()
            .map_err(|e| UtxoError::LockPoisoned(e.to_string()))?
            .insert(outpoint, utxo);
        
        Ok(())
    }
    
    /// Mark UTXO as spent
    pub fn mark_spent(&self, txid: &[u8; 32], vout: u32) -> Result<(), UtxoError> {
        let outpoint = format!("{}:{}", hex::encode(txid), vout);
        
        // Remove from outpoint index
        let utxo = self.utxos_by_outpoint.write()
            .map_err(|e| UtxoError::LockPoisoned(e.to_string()))?
            .remove(&outpoint)
            .ok_or_else(|| UtxoError::NotFound {
                txid: hex::encode(txid),
                vout,
            })?;
        
        // Remove from address index
        if let Some(address_utxos) = self.utxos_by_address.write()
            .map_err(|e| UtxoError::LockPoisoned(e.to_string()))?
            .get_mut(&utxo.address)
        {
            address_utxos.retain(|u| u.outpoint() != outpoint);
        }
        
        // Add to spent set
        self.spent.write()
            .map_err(|e| UtxoError::LockPoisoned(e.to_string()))?
            .insert(outpoint);
        
        Ok(())
    }
    
    /// Get all UTXOs for an address
    pub fn get_utxos_for_address(&self, address: &str) -> Result<Vec<Utxo>, UtxoError> {
        let utxos = self.utxos_by_address.read()
            .map_err(|e| UtxoError::LockPoisoned(e.to_string()))?;
        
        Ok(utxos.get(address).cloned().unwrap_or_default())
    }
    
    /// Get UTXOs with confirmation filter
    pub fn get_utxos_with_confirmations(
        &self,
        address: &str,
        min_confirmations: u64,
        max_confirmations: u64,
    ) -> Result<Vec<Utxo>, UtxoError> {
        let utxos = self.get_utxos_for_address(address)?;
        
        Ok(utxos.into_iter()
            .filter(|u| u.confirmations >= min_confirmations && u.confirmations <= max_confirmations)
            .collect())
    }
    
    /// Calculate total balance for address
    pub fn get_balance(&self, address: &str, min_confirmations: u64) -> Result<u64, UtxoError> {
        let utxos = self.get_utxos_for_address(address)?;
        
        let balance = utxos.iter()
            .filter(|u| u.confirmations >= min_confirmations && u.spendable)
            .map(|u| u.value)
            .sum();
        
        Ok(balance)
    }
    
    /// Calculate balance for multiple addresses
    pub fn get_total_balance(
        &self,
        addresses: &[String],
        min_confirmations: u64,
        include_watch_only: bool,
    ) -> Result<u64, UtxoError> {
        let mut total = 0u64;
        
        for address in addresses {
            let utxos = self.get_utxos_for_address(address)?;
            
            for utxo in utxos {
                if utxo.confirmations >= min_confirmations {
                    if include_watch_only || utxo.solvable {
                        total = total.saturating_add(utxo.value);
                    }
                }
            }
        }
        
        Ok(total)
    }
    
    /// List all unspent outputs
    pub fn list_unspent(
        &self,
        min_conf: u64,
        max_conf: u64,
        addresses: Option<&[String]>,
    ) -> Result<Vec<Utxo>, UtxoError> {
        let mut result = Vec::new();
        
        let utxos_by_address = self.utxos_by_address.read()
            .map_err(|e| UtxoError::LockPoisoned(e.to_string()))?;
        
        match addresses {
            Some(addrs) => {
                // Filter by specific addresses
                for addr in addrs {
                    if let Some(utxos) = utxos_by_address.get(addr) {
                        for utxo in utxos {
                            if utxo.confirmations >= min_conf && utxo.confirmations <= max_conf {
                                result.push(utxo.clone());
                            }
                        }
                    }
                }
            }
            None => {
                // All addresses
                for utxos in utxos_by_address.values() {
                    for utxo in utxos {
                        if utxo.confirmations >= min_conf && utxo.confirmations <= max_conf {
                            result.push(utxo.clone());
                        }
                    }
                }
            }
        }
        
        // Sort by confirmations descending, then by value descending
        result.sort_by(|a, b| {
            b.confirmations.cmp(&a.confirmations)
                .then(b.value.cmp(&a.value))
        });
        
        Ok(result)
    }
    
    /// Update blockchain height (affects confirmations)
    pub fn update_height(&self, new_height: u64) -> Result<(), UtxoError> {
        *self.current_height.write()
            .map_err(|e| UtxoError::LockPoisoned(e.to_string()))? = new_height;
        
        // Update confirmations for all UTXOs
        self.recalculate_confirmations(new_height)?;
        
        Ok(())
    }
    
    /// Recalculate confirmations for all UTXOs
    fn recalculate_confirmations(&self, current_height: u64) -> Result<(), UtxoError> {
        let mut by_address = self.utxos_by_address.write()
            .map_err(|e| UtxoError::LockPoisoned(e.to_string()))?;
        
        let mut by_outpoint = self.utxos_by_outpoint.write()
            .map_err(|e| UtxoError::LockPoisoned(e.to_string()))?;
        
        // Update UTXOs in address index
        for utxos in by_address.values_mut() {
            for utxo in utxos.iter_mut() {
                if utxo.block_height > 0 {
                    utxo.confirmations = current_height.saturating_sub(utxo.block_height) + 1;
                }
            }
        }
        
        // Update UTXOs in outpoint index
        for utxo in by_outpoint.values_mut() {
            if utxo.block_height > 0 {
                utxo.confirmations = current_height.saturating_sub(utxo.block_height) + 1;
            }
        }
        
        Ok(())
    }
    
    /// Add pending UTXO (unconfirmed)
    pub fn add_pending(&self, utxo: Utxo) -> Result<(), UtxoError> {
        let outpoint = utxo.outpoint();
        
        self.pending.write()
            .map_err(|e| UtxoError::LockPoisoned(e.to_string()))?
            .insert(outpoint, utxo);
        
        Ok(())
    }
    
    /// Confirm pending UTXO (move from pending to confirmed)
    pub fn confirm_pending(&self, txid: &[u8; 32], vout: u32, block_height: u64) -> Result<(), UtxoError> {
        let outpoint = format!("{}:{}", hex::encode(txid), vout);
        
        // Remove from pending
        let mut utxo = self.pending.write()
            .map_err(|e| UtxoError::LockPoisoned(e.to_string()))?
            .remove(&outpoint)
            .ok_or_else(|| UtxoError::NotFound {
                txid: hex::encode(txid),
                vout,
            })?;
        
        // Update block height
        utxo.block_height = block_height;
        utxo.confirmations = 1;
        
        // Add to confirmed UTXOs
        self.add_utxo(utxo)?;
        
        Ok(())
    }
    
    /// Check if UTXO is spent
    pub fn is_spent(&self, txid: &[u8; 32], vout: u32) -> bool {
        let outpoint = format!("{}:{}", hex::encode(txid), vout);
        
        self.spent.read()
            .map(|s| s.contains(&outpoint))
            .unwrap_or(false)
    }
    
    /// Get UTXO by outpoint
    pub fn get_utxo(&self, txid: &[u8; 32], vout: u32) -> Result<Utxo, UtxoError> {
        let outpoint = format!("{}:{}", hex::encode(txid), vout);
        
        self.utxos_by_outpoint.read()
            .map_err(|e| UtxoError::LockPoisoned(e.to_string()))?
            .get(&outpoint)
            .cloned()
            .ok_or_else(|| UtxoError::NotFound {
                txid: hex::encode(txid),
                vout,
            })
    }
    
    /// Get total number of UTXOs
    pub fn total_utxos(&self) -> usize {
        self.utxos_by_outpoint.read()
            .map(|u| u.len())
            .unwrap_or(0)
    }
    
    /// Get total number of addresses being tracked
    pub fn total_addresses(&self) -> usize {
        self.utxos_by_address.read()
            .map(|u| u.len())
            .unwrap_or(0)
    }
}

impl Default for UtxoIndex {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    fn create_test_utxo(address: &str, value: u64, height: u64) -> Utxo {
        let mut txid = [0u8; 32];
        txid[0] = 1;
        
        Utxo {
            txid,
            vout: 0,
            address: address.to_string(),
            value,
            script_pubkey: vec![],
            block_height: height,
            confirmations: 1,
            spendable: true,
            solvable: true,
            label: None,
        }
    }
    
    #[test]
    fn test_utxo_addition() {
        let index = UtxoIndex::new();
        let utxo = create_test_utxo("nova1qtest123", 1000000, 100);
        
        index.add_utxo(utxo.clone()).unwrap();
        
        // Verify UTXO is in index
        assert_eq!(index.total_utxos(), 1);
        
        // Verify we can retrieve it
        let retrieved = index.get_utxo(&utxo.txid, utxo.vout).unwrap();
        assert_eq!(retrieved.value, 1000000);
    }
    
    #[test]
    fn test_balance_calculation() {
        let index = UtxoIndex::new();
        let address = "nova1qtest123";
        
        // Add multiple UTXOs
        index.add_utxo(create_test_utxo(address, 1000000, 100)).unwrap();
        index.add_utxo(create_test_utxo(address, 2000000, 101)).unwrap();
        index.add_utxo(create_test_utxo(address, 3000000, 102)).unwrap();
        
        // Calculate balance with 1 confirmation required
        let balance = index.get_balance(address, 1).unwrap();
        assert_eq!(balance, 6000000);
    }
    
    #[test]
    fn test_utxo_spending() {
        let index = UtxoIndex::new();
        let utxo = create_test_utxo("nova1qtest123", 1000000, 100);
        let txid = utxo.txid;
        let vout = utxo.vout;
        
        index.add_utxo(utxo).unwrap();
        assert_eq!(index.total_utxos(), 1);
        
        // Mark as spent
        index.mark_spent(&txid, vout).unwrap();
        
        // Verify it's removed from active UTXOs
        assert_eq!(index.total_utxos(), 0);
        
        // Verify it's in spent set
        assert!(index.is_spent(&txid, vout));
    }
    
    #[test]
    fn test_confirmation_filtering() {
        let index = UtxoIndex::new();
        let address = "nova1qtest123";
        
        // Set current height
        index.update_height(200).unwrap();
        
        // Add UTXOs at different heights
        index.add_utxo(create_test_utxo(address, 1000000, 195)).unwrap(); // 6 confirmations
        index.add_utxo(create_test_utxo(address, 2000000, 199)).unwrap(); // 2 confirmations
        index.add_utxo(create_test_utxo(address, 3000000, 200)).unwrap(); // 1 confirmation
        
        // Get UTXOs with at least 5 confirmations
        let utxos = index.get_utxos_with_confirmations(address, 5, 9999).unwrap();
        assert_eq!(utxos.len(), 1);
        assert_eq!(utxos[0].value, 1000000);
        
        // Get UTXOs with 1-3 confirmations
        let utxos = index.get_utxos_with_confirmations(address, 1, 3).unwrap();
        assert_eq!(utxos.len(), 2);
    }
    
    #[test]
    fn test_list_unspent() {
        let index = UtxoIndex::new();
        index.update_height(200).unwrap();
        
        let addr1 = "nova1qtest123";
        let addr2 = "nova1qtest456";
        
        // Add UTXOs to different addresses
        index.add_utxo(create_test_utxo(addr1, 1000000, 195)).unwrap();
        index.add_utxo(create_test_utxo(addr1, 2000000, 199)).unwrap();
        index.add_utxo(create_test_utxo(addr2, 3000000, 198)).unwrap();
        
        // List all unspent
        let all = index.list_unspent(1, 9999, None).unwrap();
        assert_eq!(all.len(), 3);
        
        // List for specific address
        let addr1_utxos = index.list_unspent(1, 9999, Some(&[addr1.to_string()])).unwrap();
        assert_eq!(addr1_utxos.len(), 2);
        
        // Verify sorting (highest confirmations first)
        assert!(addr1_utxos[0].confirmations >= addr1_utxos[1].confirmations);
    }
    
    #[test]
    fn test_pending_utxo_confirmation() {
        let index = UtxoIndex::new();
        let mut txid = [0u8; 32];
        txid[0] = 1;
        
        // Add pending UTXO (height 0)
        let pending = create_test_utxo("nova1qtest123", 1000000, 0);
        index.add_pending(pending.clone()).unwrap();
        
        // Confirm it
        index.confirm_pending(&txid, 0, 150).unwrap();
        
        // Verify it's now in confirmed UTXOs
        let utxo = index.get_utxo(&txid, 0).unwrap();
        assert_eq!(utxo.block_height, 150);
        assert!(utxo.confirmations >= 1);
    }
}

