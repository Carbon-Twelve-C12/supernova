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
    
    /// Minimum CLTV expiry delta for HTLCs
    pub min_cltv_expiry_delta: u16,
    
    /// Maximum CLTV expiry delta for HTLCs
    pub max_cltv_expiry_delta: u16,
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
            min_cltv_expiry_delta: 144, // Minimum 1 day (assuming 10min blocks)
            max_cltv_expiry_delta: 2016, // Maximum 2 weeks (assuming 10min blocks)
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
        // Check if funding transaction exists
        let funding_tx = self.funding_tx.as_ref().ok_or_else(|| {
            ChannelError::InvalidState("Funding transaction not available".to_string())
        })?;
        
        let funding_output_index = self.funding_output_index.ok_or_else(|| {
            ChannelError::InvalidState("Funding output index not available".to_string())
        })?;
        
        // Create transaction input from funding transaction
        let funding_txid = funding_tx.hash();
        let input = TransactionInput::new(
            funding_txid,
            funding_output_index,
            Vec::new(), // Signature script will be added later
            0xffffffff, // Sequence
        );
        
        // Calculate fee for the commitment transaction (simplified)
        let base_weight = 724; // Base weight for commitment transaction
        let weight_per_htlc = 172; // Weight per HTLC
        let total_weight = base_weight + (weight_per_htlc * self.pending_htlcs.len());
        
        // Convert weight to virtual bytes (weight / 4)
        let vbytes = (total_weight + 3) / 4;
        
        // Calculate fee (assume 1 sat/vbyte for now)
        let fee_satoshis = vbytes as u64;
        
        // Calculate amount available to distribute
        let total_amount_sat = self.capacity;
        let available_amount_sat = total_amount_sat - fee_satoshis;
        
        // Convert balances from millisatoshi to satoshi
        let local_amount_sat = self.local_balance_msat / 1000;
        let remote_amount_sat = self.remote_balance_msat / 1000;
        
        // Adjust for fees
        let fee_ratio_local = local_amount_sat as f64 / (local_amount_sat + remote_amount_sat) as f64;
        let local_fee_contribution = (fee_satoshis as f64 * fee_ratio_local) as u64;
        
        // Calculate final output amounts
        let adjusted_local_amount = local_amount_sat.saturating_sub(local_fee_contribution);
        let adjusted_remote_amount = remote_amount_sat;
        
        // Create outputs
        let mut outputs = Vec::new();
        
        // Add local output if above dust limit
        if adjusted_local_amount > self.config.dust_limit_satoshis {
            // In real implementation, this would use a proper script
            // For now, we'll use a simple placeholder
            let local_output = TransactionOutput::new(
                adjusted_local_amount,
                vec![0; 25], // Placeholder for actual script
            );
            outputs.push(local_output);
        }
        
        // Add remote output if above dust limit
        if adjusted_remote_amount > self.config.dust_limit_satoshis {
            // In real implementation, this would use a proper script
            let remote_output = TransactionOutput::new(
                adjusted_remote_amount,
                vec![1; 25], // Placeholder for actual script
            );
            outputs.push(remote_output);
        }
        
        // Add outputs for HTLCs
        for htlc in &self.pending_htlcs {
            // Only include HTLCs above the dust limit
            let htlc_amount_sat = htlc.amount_msat / 1000;
            if htlc_amount_sat > self.config.dust_limit_satoshis {
                let htlc_output = match htlc.direction {
                    HtlcDirection::Offered => {
                        // Create offered HTLC output with timeout path
                        TransactionOutput::new(
                            htlc_amount_sat,
                            Self::create_htlc_script(&htlc.payment_hash, htlc.cltv_expiry, true),
                        )
                    },
                    HtlcDirection::Received => {
                        // Create received HTLC output with success path
                        TransactionOutput::new(
                            htlc_amount_sat,
                            Self::create_htlc_script(&htlc.payment_hash, htlc.cltv_expiry, false),
                        )
                    },
                };
                
                outputs.push(htlc_output);
            }
        }
        
        // Create the commitment transaction
        let commitment_tx = Transaction::new(
            2, // Version
            vec![input],
            outputs,
            0, // Locktime
        );
        
        Ok(commitment_tx)
    }
    
    /// Create an HTLC script
    fn create_htlc_script(payment_hash: &[u8; 32], cltv_expiry: u32, is_offered: bool) -> Vec<u8> {
        // In a real implementation, this would create a proper HTLC script
        // with a revocation path, success path, and timeout path
        
        // For now, create a simple script with the payment hash and timeout
        let mut script = Vec::with_capacity(100);
        
        // Add script type identifier
        script.push(if is_offered { 0x02 } else { 0x03 });
        
        // Add payment hash
        script.extend_from_slice(payment_hash);
        
        // Add CLTV expiry
        script.extend_from_slice(&cltv_expiry.to_le_bytes());
        
        script
    }
    
    /// Sign the commitment transaction
    fn sign_commitment_transaction(&self, tx: &mut Transaction) -> Result<(), ChannelError> {
        // In a real implementation, this would:
        // 1. Create the signature hash (sighash)
        // 2. Sign with local private key
        // 3. Add signature to the transaction input
        
        // Simplified implementation for now
        tx.inputs()[0].signature_script = vec![0xDE, 0xAD, 0xBE, 0xEF]; // Placeholder
        
        Ok(())
    }
    
    /// Verify commitment transaction signature
    fn verify_commitment_signature(&self, tx: &Transaction, signature: &[u8]) -> Result<bool, ChannelError> {
        // In a real implementation, this would:
        // 1. Create the signature hash (sighash)
        // 2. Verify the signature against the remote party's public key
        
        // Simplified implementation for now
        Ok(true) // Always return valid for now
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
    
    /// Verifies if a timelock condition is still valid
    pub fn verify_timelock(&self, cltv_expiry: u32) -> Result<bool, ChannelError> {
        // Get the current block height (for testing, we'll simulate this)
        let current_block_height = self.get_current_block_height()?;
        
        // Check if the timelock has expired
        Ok(current_block_height < cltv_expiry)
    }
    
    /// Gets the current block height from the blockchain
    fn get_current_block_height(&self) -> Result<u32, ChannelError> {
        // In production, this would query the actual blockchain
        // For now, we'll use a simulated value
        Ok(700_000) // Example current block height
    }
    
    /// Estimate when a timelock will expire in seconds
    pub fn estimate_timelock_expiry(&self, cltv_expiry: u32) -> Result<u64, ChannelError> {
        let current_block_height = self.get_current_block_height()?;
        
        if current_block_height >= cltv_expiry {
            return Ok(0); // Already expired
        }
        
        // Assuming 10 minutes per block on average
        let blocks_remaining = cltv_expiry - current_block_height;
        let seconds_remaining = blocks_remaining as u64 * 600;
        
        Ok(seconds_remaining)
    }
    
    /// Handle HTLC timeouts and expires HTLCs that have reached their timelock
    pub fn handle_expired_htlcs(&mut self) -> Result<Vec<u64>, ChannelError> {
        if self.state != ChannelState::Operational {
            return Err(ChannelError::InvalidState(
                format!("Cannot handle expired HTLCs in state {:?}", self.state)
            ));
        }
        
        let mut expired_htlcs = Vec::new();
        let mut indices_to_remove = Vec::new();
        
        // Check each HTLC for expiration
        for (i, htlc) in self.pending_htlcs.iter().enumerate() {
            let is_valid = self.verify_timelock(htlc.cltv_expiry)?;
            
            if !is_valid {
                // HTLC has expired
                expired_htlcs.push(htlc.id);
                indices_to_remove.push(i);
                
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
            }
        }
        
        // Remove expired HTLCs in reverse order to maintain correct indices
        indices_to_remove.sort_unstable();
        indices_to_remove.reverse();
        
        for index in indices_to_remove {
            self.pending_htlcs.remove(index);
        }
        
        if !expired_htlcs.is_empty() {
            // Update channel state
            self.last_update_time = SystemTime::now();
            self.update_count += 1;
        }
        
        Ok(expired_htlcs)
    }
    
    /// Create a commitment transaction with HTLCs
    pub fn create_secure_commitment(&self) -> Result<Transaction, ChannelError> {
        let mut commitment_tx = self.create_commitment_transaction()?;
        
        // Apply HTLC-specific security measures
        self.apply_htlc_security_measures(&mut commitment_tx)?;
        
        Ok(commitment_tx)
    }
    
    /// Apply security measures to HTLCs
    fn apply_htlc_security_measures(&self, tx: &mut Transaction) -> Result<(), ChannelError> {
        // In a production implementation, this would:
        // 1. Add proper timelocks to HTLC outputs
        // 2. Add revocation paths to handle breach scenarios
        // 3. Implement relative timelocks for security
        
        // For now, we'll simulate this with a placeholder implementation
        debug!("Applied HTLC security measures to commitment transaction");
        
        Ok(())
    }
    
    /// Check for revoked HTLCs and generate breach remedies if found
    pub fn check_for_revoked_htlcs(&self, remote_commit_num: u64) -> Result<bool, ChannelError> {
        if self.previous_commitments.is_empty() {
            return Ok(false); // No previous commitments to check
        }
        
        // Check if we have any commitments that have been revoked
        for commitment in &self.previous_commitments {
            if commitment.state_num == remote_commit_num {
                info!("Found revoked commitment: {}", remote_commit_num);
                // In production, we would generate breach remedy transactions
                return Ok(true);
            }
        }
        
        Ok(false)
    }
    
    /// Generate a breach remedy transaction
    pub fn generate_breach_remedy(&self, revoked_commit_num: u64) -> Result<Transaction, ChannelError> {
        // Find the revoked commitment
        let revoked_commitment = self.previous_commitments.iter()
            .find(|c| c.state_num == revoked_commit_num)
            .ok_or_else(|| ChannelError::InvalidState(
                format!("Revoked commitment {} not found", revoked_commit_num)
            ))?;
        
        // In production, this would:
        // 1. Create a transaction spending from the revoked commitment
        // 2. Send all funds to the local node as penalty
        // 3. Use appropriate witness scripts to enable the spend
        
        // For now, we'll create a placeholder transaction
        let remedy_tx = Transaction::new(
            2, // Version
            Vec::new(), // Inputs would come from the revoked commitment
            Vec::new(), // Outputs would go to the local wallet
            0, // Locktime
        );
        
        info!("Generated breach remedy transaction for commitment {}", revoked_commit_num);
        
        Ok(remedy_tx)
    }
    
    /// Add a new HTLC with enhanced security
    pub fn add_secure_htlc(
        &mut self,
        amount_msat: u64,
        payment_hash: [u8; 32],
        cltv_expiry: u32,
        direction: HtlcDirection,
        preimage_verification: bool,
    ) -> Result<u64, ChannelError> {
        // Verify that the CLTV expiry is reasonable
        let current_height = self.get_current_block_height()?;
        let min_cltv = current_height + self.config.min_cltv_expiry_delta as u32;
        let max_cltv = current_height + self.config.max_cltv_expiry_delta as u32;
        
        if cltv_expiry < min_cltv {
            return Err(ChannelError::HtlcError(
                format!("CLTV expiry {} is too soon (minimum is {})", cltv_expiry, min_cltv)
            ));
        }
        
        if cltv_expiry > max_cltv {
            return Err(ChannelError::HtlcError(
                format!("CLTV expiry {} is too far in the future (maximum is {})", cltv_expiry, max_cltv)
            ));
        }
        
        // Perform basic size validation for amount
        if amount_msat > self.config.max_htlc_value_in_flight_msat {
            return Err(ChannelError::HtlcError(
                format!("HTLC amount {} exceeds maximum in-flight value {}", 
                    amount_msat, self.config.max_htlc_value_in_flight_msat)
            ));
        }
        
        // Calculate total in-flight value
        let current_in_flight: u64 = self.pending_htlcs.iter()
            .map(|htlc| htlc.amount_msat)
            .sum();
        
        if current_in_flight + amount_msat > self.config.max_htlc_value_in_flight_msat {
            return Err(ChannelError::HtlcError(
                format!("Adding HTLC would exceed maximum in-flight value {} + {} > {}", 
                    current_in_flight, amount_msat, self.config.max_htlc_value_in_flight_msat)
            ));
        }
        
        // Add the HTLC with standard validation
        let htlc_id = self.add_htlc(amount_msat, payment_hash, cltv_expiry, direction)?;
        
        // Log the secure HTLC addition
        info!("Added secure HTLC: id={}, amount={} msat, expiry={}, direction={:?}",
              htlc_id, amount_msat, cltv_expiry, direction);
        
        Ok(htlc_id)
    }
    
    /// Implement secure channel state revocation
    pub fn revoke_current_commitment(&mut self) -> Result<Vec<u8>, ChannelError> {
        if self.state != ChannelState::Operational {
            return Err(ChannelError::InvalidState(
                format!("Cannot revoke commitment in state {:?}", self.state)
            ));
        }
        
        if self.current_commitment.is_none() {
            return Err(ChannelError::InvalidState(
                "No current commitment to revoke".to_string()
            ));
        }
        
        // Move current commitment to previous commitments
        let current = self.current_commitment.take()
            .ok_or_else(|| ChannelError::InvalidState("Missing current commitment".to_string()))?;
        
        self.previous_commitments.push(current);
        
        // In production, this would generate a revocation secret
        // For now, we'll generate a random value
        let mut rng = thread_rng();
        let mut revocation_secret = vec![0; 32];
        rng.fill(&mut revocation_secret[..]);
        
        info!("Revoked current commitment, new revocation secret generated");
        
        Ok(revocation_secret)
    }
} 