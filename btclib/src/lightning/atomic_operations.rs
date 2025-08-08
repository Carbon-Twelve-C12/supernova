//! Atomic operations for Lightning Network channels
//! 
//! This module provides atomic, thread-safe operations for Lightning Network
//! channel state management to prevent race conditions and fund creation exploits.

use std::sync::{Arc, Mutex, RwLock};
use std::sync::atomic::{AtomicU64, AtomicBool, Ordering};
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};
use thiserror::Error;
use sha2::{Sha256, Digest};

use crate::lightning::channel::{Channel, ChannelState, Htlc, ChannelError};
use crate::types::transaction::Transaction;

/// Errors related to atomic operations
#[derive(Debug, Error)]
pub enum AtomicOperationError {
    #[error("Lock acquisition failed: {0}")]
    LockError(String),
    
    #[error("Operation timeout: {0}")]
    Timeout(String),
    
    #[error("State inconsistency detected: {0}")]
    InconsistentState(String),
    
    #[error("Operation aborted: {0}")]
    Aborted(String),
    
    #[error("Channel error: {0}")]
    ChannelError(#[from] ChannelError),
}

/// Result type for atomic operations
pub type AtomicResult<T> = Result<T, AtomicOperationError>;

/// Atomic channel state container
pub struct AtomicChannelState {
    /// Channel state lock
    state: Arc<Mutex<ChannelState>>,
    
    /// Balance locks (separate for fine-grained locking)
    local_balance: Arc<AtomicU64>,
    remote_balance: Arc<AtomicU64>,
    
    /// HTLC operations lock
    htlc_lock: Arc<Mutex<()>>,
    
    /// Commitment number (atomic counter)
    commitment_number: Arc<AtomicU64>,
    
    /// Operation in progress flag
    operation_in_progress: Arc<AtomicBool>,
    
    /// Pending HTLCs with individual locks
    pending_htlcs: Arc<RwLock<HashMap<u64, Arc<Mutex<Htlc>>>>>,
    
    /// Maximum concurrent operations
    max_concurrent_ops: usize,
    
    /// Operation counter for rate limiting
    operation_counter: Arc<AtomicU64>,
    
    /// Last operation timestamp
    last_operation_time: Arc<AtomicU64>,
}

impl AtomicChannelState {
    /// Create new atomic channel state
    pub fn new(initial_state: ChannelState, local_balance: u64, remote_balance: u64) -> Self {
        Self {
            state: Arc::new(Mutex::new(initial_state)),
            local_balance: Arc::new(AtomicU64::new(local_balance)),
            remote_balance: Arc::new(AtomicU64::new(remote_balance)),
            htlc_lock: Arc::new(Mutex::new(())),
            commitment_number: Arc::new(AtomicU64::new(0)),
            operation_in_progress: Arc::new(AtomicBool::new(false)),
            pending_htlcs: Arc::new(RwLock::new(HashMap::new())),
            max_concurrent_ops: 10,
            operation_counter: Arc::new(AtomicU64::new(0)),
            last_operation_time: Arc::new(AtomicU64::new(0)),
        }
    }
    
    /// Get current channel state
    pub fn get_state(&self) -> AtomicResult<ChannelState> {
        self.state.lock()
            .map(|guard| *guard)
            .map_err(|e| AtomicOperationError::LockError(format!("Failed to acquire state lock: {}", e)))
    }
    
    /// Set channel state atomically
    pub fn set_state(&self, new_state: ChannelState) -> AtomicResult<()> {
        let mut state = self.state.lock()
            .map_err(|e| AtomicOperationError::LockError(format!("Failed to acquire state lock: {}", e)))?;
        
        // Validate state transition
        if !self.is_valid_transition(&*state, &new_state) {
            return Err(AtomicOperationError::InconsistentState(
                format!("Invalid state transition from {:?} to {:?}", *state, new_state)
            ));
        }
        
        *state = new_state;
        Ok(())
    }
    
    /// Get balances atomically
    pub fn get_balances(&self) -> (u64, u64) {
        (
            self.local_balance.load(Ordering::SeqCst),
            self.remote_balance.load(Ordering::SeqCst)
        )
    }
    
    /// Update balances atomically
    pub fn update_balances<F>(&self, updater: &F) -> AtomicResult<()>
    where
        F: Fn(u64, u64) -> Result<(u64, u64), String>
    {
        // Use a spin lock pattern with atomic operations
        let max_retries = 100;
        let mut retries = 0;
        
        loop {
            let local = self.local_balance.load(Ordering::SeqCst);
            let remote = self.remote_balance.load(Ordering::SeqCst);
            
            // Apply the update function
            let (new_local, new_remote) = updater(local, remote)
                .map_err(|e| AtomicOperationError::InconsistentState(e))?;
            
            // Try to update both atomically using compare-and-swap
            let local_updated = self.local_balance.compare_exchange(
                local,
                new_local,
                Ordering::SeqCst,
                Ordering::SeqCst
            ).is_ok();
            
            if local_updated {
                let remote_updated = self.remote_balance.compare_exchange(
                    remote,
                    new_remote,
                    Ordering::SeqCst,
                    Ordering::SeqCst
                ).is_ok();
                
                if remote_updated {
                    return Ok(());
                } else {
                    // Rollback local change
                    self.local_balance.store(local, Ordering::SeqCst);
                }
            }
            
            retries += 1;
            if retries >= max_retries {
                return Err(AtomicOperationError::Timeout(
                    "Failed to update balances atomically after maximum retries".to_string()
                ));
            }
            
            // Brief pause before retry
            std::thread::yield_now();
        }
    }
    
    /// Check if state transition is valid
    fn is_valid_transition(&self, from: &ChannelState, to: &ChannelState) -> bool {
        use ChannelState::*;
        
        match (from, to) {
            // Valid transitions
            (Initializing, FundingCreated) => true,
            (FundingCreated, FundingSigned) => true,
            (FundingSigned, Active) => true,
            (Active, ClosingNegotiation) => true,
            (Active, ForceClosed) => true,
            (ClosingNegotiation, Closed) => true,
            // Allow same state (no-op)
            (a, b) if a == b => true,
            // All other transitions are invalid
            _ => false,
        }
    }
}

/// Atomic channel wrapper providing thread-safe operations
pub struct AtomicChannel {
    /// The underlying channel
    pub channel: Arc<Mutex<Channel>>,
    
    /// Atomic state management
    state: Arc<AtomicChannelState>,
    
    /// Operation sequence number for ordering
    sequence: Arc<AtomicU64>,
    
    /// Channel ID for logging
    channel_id: [u8; 32],
}

impl AtomicChannel {
    /// Create a new atomic channel wrapper
    pub fn new(channel: Channel) -> Self {
        let channel_id = channel.channel_id;
        let state = AtomicChannelState::new(
            channel.state,
            channel.local_balance_novas,
            channel.remote_balance_novas,
        );
        
        Self {
            channel: Arc::new(Mutex::new(channel)),
            state: Arc::new(state),
            sequence: Arc::new(AtomicU64::new(0)),
            channel_id,
        }
    }
    
    /// Add an HTLC atomically
    pub fn add_htlc(
        &self,
        payment_hash: [u8; 32],
        amount_novas: u64,
        expiry_height: u32,
        is_outgoing: bool,
    ) -> AtomicResult<u64> {
        // Acquire HTLC lock first to prevent concurrent modifications
        let _htlc_guard = self.state.htlc_lock.lock()
            .map_err(|e| AtomicOperationError::LockError(format!("Failed to acquire HTLC lock: {}", e)))?;
        
        // Mark operation in progress
        if self.state.operation_in_progress.compare_exchange(
            false,
            true,
            Ordering::SeqCst,
            Ordering::SeqCst
        ).is_err() {
            return Err(AtomicOperationError::Aborted("Another operation is in progress".to_string()));
        }
        
        // Ensure we clear the flag on exit
        let _guard = OperationGuard::new(&self.state.operation_in_progress);
        
        // Check channel state
        let current_state = self.state.get_state()?;
        if current_state != ChannelState::Active {
            return Err(AtomicOperationError::ChannelError(
                ChannelError::InvalidState("Channel must be active to add HTLC".to_string())
            ));
        }
        
        // Generate HTLC ID atomically
        let htlc_id = self.sequence.fetch_add(1, Ordering::SeqCst);
        
        // Create HTLC
        let htlc = Htlc {
            payment_hash,
            amount_novas,
            expiry_height,
            is_outgoing,
            id: htlc_id,
        };
        
        // Update balances atomically
        self.state.update_balances(&|local, remote| {
            if is_outgoing {
                if local < amount_novas {
                    return Err(format!("Insufficient local balance: {} < {}", local, amount_novas));
                }
                Ok((local - amount_novas, remote))
            } else {
                if remote < amount_novas {
                    return Err(format!("Insufficient remote balance: {} < {}", remote, amount_novas));
                }
                Ok((local, remote - amount_novas))
            }
        })?;
        
        // Add HTLC to pending list
        {
            let mut htlcs = self.state.pending_htlcs.write()
                .map_err(|e| AtomicOperationError::LockError(format!("Failed to acquire HTLCs write lock: {}", e)))?;
            htlcs.insert(htlc_id, Arc::new(Mutex::new(htlc)));
        }
        
        // Update channel's internal state
        {
            let mut channel = self.channel.lock()
                .map_err(|e| AtomicOperationError::LockError(format!("Failed to acquire channel lock: {}", e)))?;
            
            // Sync balances
            channel.local_balance_novas = self.state.local_balance.load(Ordering::SeqCst);
            channel.remote_balance_novas = self.state.remote_balance.load(Ordering::SeqCst);
            
            // Add HTLC to channel's list
            channel.pending_htlcs.push(Htlc {
                payment_hash,
                amount_novas,
                expiry_height,
                is_outgoing,
                id: htlc_id,
            });
            
            // Increment commitment number
            channel.commitment_number = self.state.commitment_number.fetch_add(1, Ordering::SeqCst) + 1;
        }
        
        // Record operation time
        self.state.last_operation_time.store(
            SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
            Ordering::SeqCst
        );
        
        Ok(htlc_id)
    }
    
    /// Settle an HTLC atomically
    pub fn settle_htlc(&self, htlc_id: u64, preimage: [u8; 32]) -> AtomicResult<()> {
        // Acquire HTLC lock
        let _htlc_guard = self.state.htlc_lock.lock()
            .map_err(|e| AtomicOperationError::LockError(format!("Failed to acquire HTLC lock: {}", e)))?;
        
        // Mark operation in progress
        if self.state.operation_in_progress.compare_exchange(
            false,
            true,
            Ordering::SeqCst,
            Ordering::SeqCst
        ).is_err() {
            return Err(AtomicOperationError::Aborted("Another operation is in progress".to_string()));
        }
        
        let _guard = OperationGuard::new(&self.state.operation_in_progress);
        
        // Find and validate HTLC
        let htlc = {
            let htlcs = self.state.pending_htlcs.read()
                .map_err(|e| AtomicOperationError::LockError(format!("Failed to acquire HTLCs read lock: {}", e)))?;
            
            let htlc_arc = htlcs.get(&htlc_id)
                .ok_or_else(|| AtomicOperationError::ChannelError(
                    ChannelError::HtlcError(format!("HTLC {} not found", htlc_id))
                ))?;
            
            let htlc = htlc_arc.lock()
                .map_err(|e| AtomicOperationError::LockError(format!("Failed to lock HTLC: {}", e)))?;
            
            // Verify preimage
            let mut hasher = Sha256::new();
            hasher.update(&preimage);
            let hash = hasher.finalize();
            
            if hash.as_slice() != &htlc.payment_hash {
                return Err(AtomicOperationError::ChannelError(
                    ChannelError::HtlcError("Invalid preimage for HTLC".to_string())
                ));
            }
            
            htlc.clone()
        };
        
        // Update balances atomically
        self.state.update_balances(&|local, remote| {
            if htlc.is_outgoing {
                // We sent this HTLC, so the remote party gets the funds
                Ok((local, remote + htlc.amount_novas))
            } else {
                // We received this HTLC, so we get the funds
                Ok((local + htlc.amount_novas, remote))
            }
        })?;
        
        // Remove HTLC from pending list
        {
            let mut htlcs = self.state.pending_htlcs.write()
                .map_err(|e| AtomicOperationError::LockError(format!("Failed to acquire HTLCs write lock: {}", e)))?;
            htlcs.remove(&htlc_id);
        }
        
        // Update channel's internal state
        {
            let mut channel = self.channel.lock()
                .map_err(|e| AtomicOperationError::LockError(format!("Failed to acquire channel lock: {}", e)))?;
            
            // Sync balances
            channel.local_balance_novas = self.state.local_balance.load(Ordering::SeqCst);
            channel.remote_balance_novas = self.state.remote_balance.load(Ordering::SeqCst);
            
            // Remove HTLC from channel's list
            channel.pending_htlcs.retain(|h| h.id != htlc_id);
            
            // Increment commitment number
            channel.commitment_number = self.state.commitment_number.fetch_add(1, Ordering::SeqCst) + 1;
        }
        
        // Record operation time
        self.state.last_operation_time.store(
            SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
            Ordering::SeqCst
        );
        
        Ok(())
    }
    
    /// Fail an HTLC atomically
    pub fn fail_htlc(&self, htlc_id: u64, reason: &str) -> AtomicResult<()> {
        // Acquire HTLC lock
        let _htlc_guard = self.state.htlc_lock.lock()
            .map_err(|e| AtomicOperationError::LockError(format!("Failed to acquire HTLC lock: {}", e)))?;
        
        // Mark operation in progress
        if self.state.operation_in_progress.compare_exchange(
            false,
            true,
            Ordering::SeqCst,
            Ordering::SeqCst
        ).is_err() {
            return Err(AtomicOperationError::Aborted("Another operation is in progress".to_string()));
        }
        
        let _guard = OperationGuard::new(&self.state.operation_in_progress);
        
        // Find HTLC
        let htlc = {
            let htlcs = self.state.pending_htlcs.read()
                .map_err(|e| AtomicOperationError::LockError(format!("Failed to acquire HTLCs read lock: {}", e)))?;
            
            let htlc_arc = htlcs.get(&htlc_id)
                .ok_or_else(|| AtomicOperationError::ChannelError(
                    ChannelError::HtlcError(format!("HTLC {} not found", htlc_id))
                ))?;
            
            let htlc = htlc_arc.lock()
                .map_err(|e| AtomicOperationError::LockError(format!("Failed to lock HTLC: {}", e)))?;
            
            htlc.clone()
        };
        
        // Update balances atomically
        self.state.update_balances(&|local, remote| {
            if htlc.is_outgoing {
                // We sent this HTLC, so we get the funds back
                Ok((local + htlc.amount_novas, remote))
            } else {
                // Remote party sent this HTLC, so they get the funds back
                Ok((local, remote + htlc.amount_novas))
            }
        })?;
        
        // Remove HTLC from pending list
        {
            let mut htlcs = self.state.pending_htlcs.write()
                .map_err(|e| AtomicOperationError::LockError(format!("Failed to acquire HTLCs write lock: {}", e)))?;
            htlcs.remove(&htlc_id);
        }
        
        // Update channel's internal state
        {
            let mut channel = self.channel.lock()
                .map_err(|e| AtomicOperationError::LockError(format!("Failed to acquire channel lock: {}", e)))?;
            
            // Sync balances
            channel.local_balance_novas = self.state.local_balance.load(Ordering::SeqCst);
            channel.remote_balance_novas = self.state.remote_balance.load(Ordering::SeqCst);
            
            // Remove HTLC from channel's list
            channel.pending_htlcs.retain(|h| h.id != htlc_id);
            
            // Increment commitment number
            channel.commitment_number = self.state.commitment_number.fetch_add(1, Ordering::SeqCst) + 1;
        }
        
        log::warn!("Failed HTLC {} with reason: {}", htlc_id, reason);
        
        // Record operation time
        self.state.last_operation_time.store(
            SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
            Ordering::SeqCst
        );
        
        Ok(())
    }
    
    /// Get channel info atomically
    pub fn get_channel_info(&self) -> AtomicResult<ChannelInfo> {
        let channel = self.channel.lock()
            .map_err(|e| AtomicOperationError::LockError(format!("Failed to acquire channel lock: {}", e)))?;
        
        let (local_balance, remote_balance) = self.state.get_balances();
        let state = self.state.get_state()?;
        
        let htlcs = self.state.pending_htlcs.read()
            .map_err(|e| AtomicOperationError::LockError(format!("Failed to acquire HTLCs read lock: {}", e)))?;
        
        Ok(ChannelInfo {
            channel_id: self.channel_id,
            state,
            local_balance_novas: local_balance,
            remote_balance_novas: remote_balance,
            capacity_novas: channel.capacity_novas,
            pending_htlcs_count: htlcs.len(),
            commitment_number: self.state.commitment_number.load(Ordering::SeqCst),
            last_operation_time: self.state.last_operation_time.load(Ordering::SeqCst),
        })
    }
    
    /// Create commitment transaction atomically
    pub fn create_commitment_transaction(&self) -> AtomicResult<Transaction> {
        let _htlc_guard = self.state.htlc_lock.lock()
            .map_err(|e| AtomicOperationError::LockError(format!("Failed to acquire HTLC lock: {}", e)))?;
        
        let mut channel = self.channel.lock()
            .map_err(|e| AtomicOperationError::LockError(format!("Failed to acquire channel lock: {}", e)))?;
        
        // Ensure balances are synchronized
        channel.local_balance_novas = self.state.local_balance.load(Ordering::SeqCst);
        channel.remote_balance_novas = self.state.remote_balance.load(Ordering::SeqCst);
        
        // Create commitment transaction
        channel.create_commitment_transaction()
            .map_err(|e| AtomicOperationError::ChannelError(e))
    }
}

/// Channel info snapshot
#[derive(Debug, Clone)]
pub struct ChannelInfo {
    pub channel_id: [u8; 32],
    pub state: ChannelState,
    pub local_balance_novas: u64,
    pub remote_balance_novas: u64,
    pub capacity_novas: u64,
    pub pending_htlcs_count: usize,
    pub commitment_number: u64,
    pub last_operation_time: u64,
}

/// Guard to ensure operation flag is cleared
struct OperationGuard<'a> {
    flag: &'a AtomicBool,
}

impl<'a> OperationGuard<'a> {
    fn new(flag: &'a AtomicBool) -> Self {
        Self { flag }
    }
}

impl<'a> Drop for OperationGuard<'a> {
    fn drop(&mut self) {
        self.flag.store(false, Ordering::SeqCst);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lightning::channel::Channel;
    use secp256k1::PublicKey;
    
    #[test]
    fn test_atomic_htlc_operations() {
        // Create a test channel
        let local_node_id = PublicKey([1u8; 33]);
        let remote_node_id = PublicKey([2u8; 33]);
        let mut channel = Channel::new(local_node_id, remote_node_id, 1000000, true, false);
        channel.state = ChannelState::Active;
        channel.local_balance_novas = 600000;
        channel.remote_balance_novas = 400000;
        
        let atomic_channel = AtomicChannel::new(channel);
        
        // Test adding HTLC
        let payment_hash = [3u8; 32];
        let htlc_id = atomic_channel.add_htlc(payment_hash, 100000, 500000, true)
            .expect("Failed to add HTLC");
        
        // Check balances after adding HTLC
        let (local, remote) = atomic_channel.state.get_balances();
        assert_eq!(local, 500000); // 600000 - 100000
        assert_eq!(remote, 400000);
        
        // Test settling HTLC
        let preimage = [3u8; 32]; // Matching preimage for our test
        atomic_channel.settle_htlc(htlc_id, preimage)
            .expect("Failed to settle HTLC");
        
        // Check balances after settling
        let (local, remote) = atomic_channel.state.get_balances();
        assert_eq!(local, 500000);
        assert_eq!(remote, 500000); // 400000 + 100000
    }
    
    #[test]
    fn test_concurrent_htlc_operations() {
        use std::thread;
        use std::sync::Arc;
        
        // Create a test channel
        let local_node_id = PublicKey([1u8; 33]);
        let remote_node_id = PublicKey([2u8; 33]);
        let mut channel = Channel::new(local_node_id, remote_node_id, 1000000, true, false);
        channel.state = ChannelState::Active;
        channel.local_balance_novas = 800000;
        channel.remote_balance_novas = 200000;
        
        let atomic_channel = Arc::new(AtomicChannel::new(channel));
        
        // Spawn multiple threads trying to add HTLCs concurrently
        let mut handles = vec![];
        
        for i in 0..10 {
            let channel_clone = Arc::clone(&atomic_channel);
            let handle = thread::spawn(move || {
                let payment_hash = [i as u8; 32];
                channel_clone.add_htlc(payment_hash, 10000, 500000, true)
            });
            handles.push(handle);
        }
        
        // Wait for all threads and collect results
        let mut successful_htlcs = 0;
        for handle in handles {
            if handle.join().unwrap().is_ok() {
                successful_htlcs += 1;
            }
        }
        
        // Check final balances
        let (local, remote) = atomic_channel.state.get_balances();
        assert_eq!(local, 800000 - (successful_htlcs * 10000));
        assert_eq!(remote, 200000);
        
        // Total should still equal capacity
        assert_eq!(local + remote + (successful_htlcs * 10000), 1000000);
    }
} 