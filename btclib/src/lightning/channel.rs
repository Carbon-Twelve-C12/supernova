// SuperNova Lightning Network - Channel Implementation
//
// This file contains the implementation of Lightning Network payment channels.
// It handles channel state management, commitment transactions, and HTLC operations.

use crate::types::transaction::{Transaction, TransactionInput, TransactionOutput};
use crate::crypto::quantum::{QuantumKeyPair, QuantumScheme};
use std::sync::{Arc, RwLock, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use thiserror::Error;
use tracing::{debug, info, warn, error};
use rand::{thread_rng, Rng};
use sha2::{Sha256, Digest};

/// Error types for channel operations
#[derive(Debug, Error)]
pub enum ChannelError {
    #[error("Invalid state transition: {0}")]
    InvalidState(String),
    
    #[error("Insufficient funds: {0}")]
    InsufficientFunds(String),
    
    #[error("Cryptographic error: {0}")]
    CryptoError(String),
    
    #[error("HTLC error: {0}")]
    HtlcError(String),
    
    #[error("Transaction error: {0}")]
    TransactionError(String),
    
    #[error("Channel configuration error: {0}")]
    ConfigError(String),
    
    #[error("Channel timeout: {0}")]
    Timeout(String),
    
    #[error("Protocol violation: {0}")]
    ProtocolViolation(String),
}

/// Unique identifier for a channel
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ChannelId([u8; 32]);

impl ChannelId {
    /// Generate a new random channel ID
    pub fn new_random() -> Self {
        let mut rng = thread_rng();
        let mut id = [0u8; 32];
        rng.fill(&mut id);
        Self(id)
    }
    
    /// Create a channel ID from a transaction ID and output index
    pub fn from_funding_outpoint(txid: &[u8; 32], output_index: u32) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(txid);
        hasher.update(&output_index.to_le_bytes());
        
        let result = hasher.finalize();
        let mut id = [0u8; 32];
        id.copy_from_slice(&result);
        
        Self(id)
    }
    
    /// Get the raw ID bytes
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl std::fmt::Display for ChannelId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", hex::encode(&self.0[..]))
    }
}

/// Channel state
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChannelState {
    /// Initial state after channel creation
    Created,
    
    /// Funding transaction has been created
    FundingCreated,
    
    /// Funding transaction has been signed
    FundingSigned,
    
    /// Funding transaction has been broadcast
    FundingBroadcast,
    
    /// Funding transaction is in the mempool
    FundingMempool,
    
    /// Funding transaction has been confirmed
    FundingConfirmed,
    
    /// Channel is operational and can process payments
    Operational,
    
    /// Channel is in the process of being closed
    Closing,
    
    /// Channel has been closed
    Closed,
    
    /// Channel has been force closed
    ForceClosing,
    
    /// Channel has been force closed
    ForceClosed,
    
    /// Channel error state
    Error,
}

/// Channel configuration
#[derive(Debug, Clone)]
pub struct ChannelConfig {
    /// Whether to announce this channel on the network
    pub announce_channel: bool,
    
    /// Maximum value in flight
    pub max_htlc_value_in_flight_msat: u64,
    
    /// Minimum value for an HTLC
    pub min_htlc_value_msat: u64,
    
    /// Maximum number of HTLCs
    pub max_accepted_htlcs: u16,
    
    /// CLTV expiry delta for HTLCs
    pub cltv_expiry_delta: u16,
    
    /// Channel reserve value
    pub channel_reserve_satoshis: u64,
    
    /// Dust limit for outputs
    pub dust_limit_satoshis: u64,
    
    /// Maximum number of commitment transactions to keep
    pub max_commitment_transactions: u32,
    
    /// Whether to use quantum-resistant signatures
    pub use_quantum_signatures: bool,
    
    /// Force close timeout in seconds
    pub force_close_timeout_seconds: u64,
}

impl Default for ChannelConfig {
    fn default() -> Self {
        Self {
            announce_channel: true,
            max_htlc_value_in_flight_msat: 100_000_000, // 0.001 BTC in millisatoshis
            min_htlc_value_msat: 1_000,                // 1 satoshi
            max_accepted_htlcs: 30,
            cltv_expiry_delta: 40,
            channel_reserve_satoshis: 10_000,          // 0.0001 BTC
            dust_limit_satoshis: 546,
            max_commitment_transactions: 10,
            use_quantum_signatures: false,
            force_close_timeout_seconds: 86400,        // 24 hours
        }
    }
}

/// Represents a Hash Time Locked Contract (HTLC)
#[derive(Debug, Clone)]
pub struct Htlc {
    /// ID of the HTLC
    id: u64,
    
    /// Amount in millisatoshis
    amount_msat: u64,
    
    /// Payment hash
    payment_hash: [u8; 32],
    
    /// Expiry
    cltv_expiry: u32,
    
    /// Direction (offered or received)
    direction: HtlcDirection,
    
    /// State of the HTLC
    state: HtlcState,
}

/// Direction of an HTLC
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HtlcDirection {
    /// Offered HTLC (outgoing payment)
    Offered,
    
    /// Received HTLC (incoming payment)
    Received,
}

/// State of an HTLC
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HtlcState {
    /// HTLC has been proposed
    Proposed,
    
    /// HTLC has been accepted
    Accepted,
    
    /// HTLC is pending settlement
    Pending,
    
    /// HTLC has been fulfilled
    Fulfilled,
    
    /// HTLC has failed
    Failed,
}

/// Public information about a channel
#[derive(Debug, Clone)]
pub struct ChannelInfo {
    /// Channel ID
    pub id: ChannelId,
    
    /// Channel state
    pub state: ChannelState,
    
    /// Channel capacity in satoshis
    pub capacity: u64,
    
    /// Local balance in millisatoshis
    pub local_balance_msat: u64,
    
    /// Remote balance in millisatoshis
    pub remote_balance_msat: u64,
    
    /// Whether channel is public
    pub is_public: bool,
    
    /// Number of pending HTLCs
    pub pending_htlcs: u16,
    
    /// Channel config
    pub config: ChannelConfig,
    
    /// Channel uptime in seconds
    pub uptime_seconds: u64,
    
    /// Number of updates
    pub update_count: u64,
}

/// Commitment transaction structure
struct CommitmentTx {
    /// Transaction
    tx: Transaction,
    
    /// Remote signature
    remote_signature: Vec<u8>,
    
    /// State number
    state_num: u64,
    
    /// Pending HTLCs
    htlcs: Vec<Htlc>,
}

/// Main channel implementation
pub struct Channel {
    /// Channel ID
    id: ChannelId,
    
    /// Remote node ID
    remote_node_id: String,
    
    /// Channel state
    state: ChannelState,
    
    /// Channel capacity in satoshis
    capacity: u64,
    
    /// Local balance in millisatoshis
    local_balance_msat: u64,
    
    /// Remote balance in millisatoshis
    remote_balance_msat: u64,
    
    /// Channel configuration
    config: ChannelConfig,
    
    /// Funding transaction
    funding_tx: Option<Transaction>,
    
    /// Funding transaction output index
    funding_output_index: Option<u32>,
    
    /// Current commitment transaction
    current_commitment: Option<CommitmentTx>,
    
    /// Previous commitment transactions
    previous_commitments: Vec<CommitmentTx>,
    
    /// Pending HTLCs
    pending_htlcs: Vec<Htlc>,
    
    /// Next HTLC ID
    next_htlc_id: u64,
    
    /// Quantum key pair if quantum signatures are enabled
    quantum_keypair: Option<QuantumKeyPair>,
    
    /// Creation time
    creation_time: SystemTime,
    
    /// Last update time
    last_update_time: SystemTime,
    
    /// Update count
    update_count: u64,
}

impl Channel {
    /// Open a new channel
    pub fn open(
        remote_node_id: String,
        capacity: u64,
        push_amount: u64,
        config: ChannelConfig,
        quantum_scheme: Option<QuantumScheme>,
    ) -> Result<Self, ChannelError> {
        // Validate parameters
        if capacity < config.channel_reserve_satoshis * 2 {
            return Err(ChannelError::ConfigError(
                format!("Channel capacity must be at least twice the reserve: {} < {}",
                    capacity, config.channel_reserve_satoshis * 2)
            ));
        }
        
        if push_amount >= capacity {
            return Err(ChannelError::InsufficientFunds(
                format!("Push amount cannot exceed capacity: {} >= {}", push_amount, capacity)
            ));
        }
        
        // Convert push amount to millisatoshis
        let push_amount_msat = push_amount * 1000;
        
        // Calculate initial balances
        let local_balance_msat = (capacity * 1000) - push_amount_msat;
        let remote_balance_msat = push_amount_msat;
        
        // Create quantum keypair if needed
        let quantum_keypair = if config.use_quantum_signatures {
            if let Some(scheme) = quantum_scheme {
                Some(QuantumKeyPair::generate(scheme, 1)?)
            } else {
                return Err(ChannelError::ConfigError(
                    "Quantum signatures enabled but no scheme provided".to_string()
                ));
            }
        } else {
            None
        };
        
        // Create channel ID
        let id = ChannelId::new_random();
        
        Ok(Self {
            id,
            remote_node_id,
            state: ChannelState::Created,
            capacity,
            local_balance_msat,
            remote_balance_msat,
            config,
            funding_tx: None,
            funding_output_index: None,
            current_commitment: None,
            previous_commitments: Vec::new(),
            pending_htlcs: Vec::new(),
            next_htlc_id: 0,
            quantum_keypair,
            creation_time: SystemTime::now(),
            last_update_time: SystemTime::now(),
            update_count: 0,
        })
    }
    
    /// Get the channel ID
    pub fn id(&self) -> &ChannelId {
        &self.id
    }
    
    /// Get current channel state
    pub fn state(&self) -> &ChannelState {
        &self.state
    }
    
    /// Get channel information
    pub fn get_info(&self) -> ChannelInfo {
        let now = SystemTime::now();
        let uptime_seconds = now.duration_since(self.creation_time)
            .unwrap_or(Duration::from_secs(0))
            .as_secs();
        
        ChannelInfo {
            id: self.id.clone(),
            state: self.state.clone(),
            capacity: self.capacity,
            local_balance_msat: self.local_balance_msat,
            remote_balance_msat: self.remote_balance_msat,
            is_public: self.config.announce_channel,
            pending_htlcs: self.pending_htlcs.len() as u16,
            config: self.config.clone(),
            uptime_seconds,
            update_count: self.update_count,
        }
    }
    
    /// Update channel state
    fn update_state(&mut self, new_state: ChannelState) -> Result<(), ChannelError> {
        // Check if the state transition is valid
        match (&self.state, &new_state) {
            (ChannelState::Created, ChannelState::FundingCreated) => {},
            (ChannelState::FundingCreated, ChannelState::FundingSigned) => {},
            (ChannelState::FundingSigned, ChannelState::FundingBroadcast) => {},
            (ChannelState::FundingBroadcast, ChannelState::FundingMempool) => {},
            (ChannelState::FundingMempool, ChannelState::FundingConfirmed) => {},
            (ChannelState::FundingConfirmed, ChannelState::Operational) => {},
            (ChannelState::Operational, ChannelState::Closing) => {},
            (ChannelState::Closing, ChannelState::Closed) => {},
            (ChannelState::Operational, ChannelState::ForceClosing) => {},
            (ChannelState::ForceClosing, ChannelState::ForceClosed) => {},
            (_, ChannelState::Error) => {}, // Any state can transition to error
            _ => {
                return Err(ChannelError::InvalidState(
                    format!("Invalid state transition from {:?} to {:?}", self.state, new_state)
                ));
            }
        }
        
        // Update state
        self.state = new_state;
        self.last_update_time = SystemTime::now();
        self.update_count += 1;
        
        Ok(())
    }
    
    /// Process funding transaction
    pub fn process_funding_transaction(&mut self, funding_tx: Transaction, output_index: u32) -> Result<(), ChannelError> {
        // Verify we're in the correct state
        if self.state != ChannelState::Created {
            return Err(ChannelError::InvalidState(
                format!("Cannot process funding transaction in state {:?}", self.state)
            ));
        }
        
        // Verify funding transaction output
        if output_index as usize >= funding_tx.outputs().len() {
            return Err(ChannelError::TransactionError(
                format!("Invalid output index: {}", output_index)
            ));
        }
        
        let output = &funding_tx.outputs()[output_index as usize];
        if output.amount() != self.capacity {
            return Err(ChannelError::TransactionError(
                format!("Funding output amount {} doesn't match channel capacity {}", 
                    output.amount(), self.capacity)
            ));
        }
        
        // Store funding transaction and output index
        self.funding_tx = Some(funding_tx);
        self.funding_output_index = Some(output_index);
        
        // Update channel ID based on funding outpoint
        if let Some(tx) = &self.funding_tx {
            let txid = tx.hash();
            self.id = ChannelId::from_funding_outpoint(&txid, output_index);
        }
        
        // Update state
        self.update_state(ChannelState::FundingCreated)?;
        
        Ok(())
    }
    
    /// Create a commitment transaction
    fn create_commitment_transaction(&self) -> Result<Transaction, ChannelError> {
        // This is a placeholder implementation
        // A real implementation would create a valid commitment transaction
        // with proper outputs for both parties and HTLCs
        
        if self.funding_tx.is_none() || self.funding_output_index.is_none() {
            return Err(ChannelError::InvalidState(
                "Cannot create commitment transaction without funding information".to_string()
            ));
        }
        
        // Create a basic transaction structure
        let tx = Transaction::new(
            1, // version
            vec![], // inputs would include funding outpoint
            vec![], // outputs would include to_local and to_remote
            0, // locktime
        );
        
        Ok(tx)
    }
    
    /// Cooperatively close the channel
    pub fn cooperative_close(&self) -> Result<Transaction, ChannelError> {
        // Check if we can close the channel
        if self.state != ChannelState::Operational {
            return Err(ChannelError::InvalidState(
                format!("Cannot cooperatively close channel in state {:?}", self.state)
            ));
        }
        
        // Create a basic closing transaction
        // This is a placeholder - real implementation would create a proper closing transaction
        // with correct outputs for both parties
        
        let tx = Transaction::new(
            1, // version
            vec![], // inputs would include funding outpoint
            vec![], // outputs would include to_local and to_remote
            0, // locktime
        );
        
        Ok(tx)
    }
    
    /// Force close the channel
    pub fn force_close(&self) -> Result<Transaction, ChannelError> {
        // Check if we can force close the channel
        if self.state != ChannelState::Operational {
            return Err(ChannelError::InvalidState(
                format!("Cannot force close channel in state {:?}", self.state)
            ));
        }
        
        // Use latest commitment transaction as force close
        if let Some(commitment) = &self.current_commitment {
            return Ok(commitment.tx.clone());
        }
        
        // Create a commitment transaction if none exists
        self.create_commitment_transaction()
    }
    
    /// Add an HTLC to the channel
    pub fn add_htlc(
        &mut self,
        amount_msat: u64,
        payment_hash: [u8; 32],
        cltv_expiry: u32,
        direction: HtlcDirection,
    ) -> Result<u64, ChannelError> {
        // Check if channel is operational
        if self.state != ChannelState::Operational {
            return Err(ChannelError::InvalidState(
                format!("Cannot add HTLC in state {:?}", self.state)
            ));
        }
        
        // Check if we've reached the maximum number of HTLCs
        if self.pending_htlcs.len() >= self.config.max_accepted_htlcs as usize {
            return Err(ChannelError::HtlcError(
                format!("Maximum number of HTLCs reached: {}", self.config.max_accepted_htlcs)
            ));
        }
        
        // Check minimum HTLC value
        if amount_msat < self.config.min_htlc_value_msat {
            return Err(ChannelError::HtlcError(
                format!("HTLC amount {} is below minimum {}", 
                    amount_msat, self.config.min_htlc_value_msat)
            ));
        }
        
        // Check if sender has sufficient funds
        match direction {
            HtlcDirection::Offered => {
                if self.local_balance_msat < amount_msat {
                    return Err(ChannelError::InsufficientFunds(
                        format!("Insufficient local balance: {} < {}", 
                            self.local_balance_msat, amount_msat)
                    ));
                }
            },
            HtlcDirection::Received => {
                if self.remote_balance_msat < amount_msat {
                    return Err(ChannelError::InsufficientFunds(
                        format!("Insufficient remote balance: {} < {}", 
                            self.remote_balance_msat, amount_msat)
                    ));
                }
            }
        }
        
        // Create the HTLC
        let htlc_id = self.next_htlc_id;
        self.next_htlc_id += 1;
        
        let htlc = Htlc {
            id: htlc_id,
            amount_msat,
            payment_hash,
            cltv_expiry,
            direction,
            state: HtlcState::Proposed,
        };
        
        // Add to pending HTLCs
        self.pending_htlcs.push(htlc);
        
        // Update balances temporarily (will be finalized when HTLC settles)
        match direction {
            HtlcDirection::Offered => {
                self.local_balance_msat -= amount_msat;
            },
            HtlcDirection::Received => {
                self.remote_balance_msat -= amount_msat;
            }
        }
        
        // Update channel state
        self.last_update_time = SystemTime::now();
        self.update_count += 1;
        
        Ok(htlc_id)
    }
    
    /// Fulfill an HTLC
    pub fn fulfill_htlc(
        &mut self,
        htlc_id: u64,
        preimage: [u8; 32],
    ) -> Result<(), ChannelError> {
        // Check if channel is operational
        if self.state != ChannelState::Operational {
            return Err(ChannelError::InvalidState(
                format!("Cannot fulfill HTLC in state {:?}", self.state)
            ));
        }
        
        // Find the HTLC
        let htlc_index = self.pending_htlcs.iter().position(|htlc| htlc.id == htlc_id)
            .ok_or_else(|| ChannelError::HtlcError(format!("HTLC {} not found", htlc_id)))?;
        
        let htlc = &mut self.pending_htlcs[htlc_index];
        
        // Check HTLC state
        if htlc.state != HtlcState::Accepted && htlc.state != HtlcState::Pending {
            return Err(ChannelError::HtlcError(
                format!("Cannot fulfill HTLC in state {:?}", htlc.state)
            ));
        }
        
        // Verify preimage
        let mut hasher = Sha256::new();
        hasher.update(&preimage);
        let hash = hasher.finalize();
        
        let mut calculated_hash = [0u8; 32];
        calculated_hash.copy_from_slice(&hash);
        
        if calculated_hash != htlc.payment_hash {
            return Err(ChannelError::HtlcError(
                "Invalid payment preimage".to_string()
            ));
        }
        
        // Update HTLC state
        htlc.state = HtlcState::Fulfilled;
        
        // Update balances
        match htlc.direction {
            HtlcDirection::Offered => {
                // Remote party keeps the funds
                self.remote_balance_msat += htlc.amount_msat;
            },
            HtlcDirection::Received => {
                // We receive the funds
                self.local_balance_msat += htlc.amount_msat;
            }
        }
        
        // Remove HTLC from pending list
        self.pending_htlcs.remove(htlc_index);
        
        // Update channel state
        self.last_update_time = SystemTime::now();
        self.update_count += 1;
        
        Ok(())
    }
    
    /// Fail an HTLC
    pub fn fail_htlc(
        &mut self,
        htlc_id: u64,
        reason: &str,
    ) -> Result<(), ChannelError> {
        // Check if channel is operational
        if self.state != ChannelState::Operational {
            return Err(ChannelError::InvalidState(
                format!("Cannot fail HTLC in state {:?}", self.state)
            ));
        }
        
        // Find the HTLC
        let htlc_index = self.pending_htlcs.iter().position(|htlc| htlc.id == htlc_id)
            .ok_or_else(|| ChannelError::HtlcError(format!("HTLC {} not found", htlc_id)))?;
        
        let htlc = &mut self.pending_htlcs[htlc_index];
        
        // Check HTLC state
        if htlc.state != HtlcState::Proposed && htlc.state != HtlcState::Accepted && htlc.state != HtlcState::Pending {
            return Err(ChannelError::HtlcError(
                format!("Cannot fail HTLC in state {:?}", htlc.state)
            ));
        }
        
        // Update HTLC state
        htlc.state = HtlcState::Failed;
        
        // Update balances - return funds to sender
        match htlc.direction {
            HtlcDirection::Offered => {
                // Return to local balance
                self.local_balance_msat += htlc.amount_msat;
            },
            HtlcDirection::Received => {
                // Return to remote balance
                self.remote_balance_msat += htlc.amount_msat;
            }
        }
        
        // Remove HTLC from pending list
        self.pending_htlcs.remove(htlc_index);
        
        // Update channel state
        self.last_update_time = SystemTime::now();
        self.update_count += 1;
        
        Ok(())
    }
    
    /// Get all pending HTLCs
    pub fn get_pending_htlcs(&self) -> Vec<&Htlc> {
        self.pending_htlcs.iter().collect()
    }
    
    /// Check if the channel has sufficient capacity for a payment
    pub fn can_send(&self, amount_msat: u64) -> bool {
        if self.state != ChannelState::Operational {
            return false;
        }
        
        self.local_balance_msat >= amount_msat
    }
    
    /// Check if the channel has sufficient capacity to receive a payment
    pub fn can_receive(&self, amount_msat: u64) -> bool {
        if self.state != ChannelState::Operational {
            return false;
        }
        
        self.remote_balance_msat >= amount_msat
    }
    
    /// Get channel uptime in seconds
    pub fn uptime(&self) -> u64 {
        SystemTime::now()
            .duration_since(self.creation_time)
            .unwrap_or(Duration::from_secs(0))
            .as_secs()
    }
    
    /// Get time since last update in seconds
    pub fn time_since_last_update(&self) -> u64 {
        SystemTime::now()
            .duration_since(self.last_update_time)
            .unwrap_or(Duration::from_secs(0))
            .as_secs()
    }
} 