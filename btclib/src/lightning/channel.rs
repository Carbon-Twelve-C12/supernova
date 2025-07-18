// supernova Lightning Network - Channel Implementation
//
// This file contains the implementation of Lightning Network payment channels.
// It handles channel state management, commitment transactions, and HTLC operations.

use thiserror::Error;
use tracing::{debug, info, warn, error};
use rand::{thread_rng, Rng, RngCore};
use sha2::{Sha256, Digest};
use std::collections::HashMap;
use serde::{Serialize, Deserialize, Serializer, Deserializer};
use crate::types::transaction::{Transaction, TransactionInput as TxIn, TransactionOutput as TxOut, OutPoint};
use crate::crypto::signature::SignatureScheme;
use crate::crypto::quantum::{QuantumKeyPair, QuantumScheme};
use std::sync::{Arc, RwLock, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

// Import proper types from existing modules
use crate::types::script::Script;
use secp256k1::{PublicKey, SecretKey as PrivateKey};

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
    
    #[error("Channel doesn't exist: {0}")]
    ChannelNotFound(String),
    
    #[error("Funding error: {0}")]
    FundingError(String),
    
    #[error("Commitment transaction error: {0}")]
    CommitmentError(String),
    
    #[error("Signature error: {0}")]
    SignatureError(String),
    
    #[error("Revocation error: {0}")]
    RevocationError(String),
    
    #[error("Closure error: {0}")]
    ClosureError(String),
}

/// Result type for channel operations
pub type ChannelResult<T> = Result<T, ChannelError>;

/// Unique identifier for a channel
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ChannelId([u8; 32]);

impl ChannelId {
    /// Generate a new random channel ID
    pub fn new_random() -> Self {
        let mut rng = thread_rng();
        let mut id = [0u8; 32];
        rng.fill_bytes(&mut id);
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
    
    /// Create a ChannelId from raw bytes
    pub fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }
    
    /// Get the raw ID bytes
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
    
    /// Convert to hex string
    pub fn to_hex(&self) -> String {
        hex::encode(&self.0)
    }
    
    /// Create from hex string
    pub fn from_hex(hex_str: &str) -> Result<Self, ChannelError> {
        if hex_str.len() != 64 {
            return Err(ChannelError::InvalidState("Invalid hex length for channel ID".to_string()));
        }
        
        let bytes = hex::decode(hex_str)
            .map_err(|_| ChannelError::InvalidState("Invalid hex string".to_string()))?;
        
        if bytes.len() != 32 {
            return Err(ChannelError::InvalidState("Invalid byte length for channel ID".to_string()));
        }
        
        let mut id = [0u8; 32];
        id.copy_from_slice(&bytes);
        Ok(Self(id))
    }
}

impl std::fmt::Display for ChannelId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", hex::encode(&self.0[..]))
    }
}

/// State of a Lightning Network channel
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChannelState {
    /// Initial negotiation
    Initializing,
    /// Funding transaction created but not confirmed
    FundingCreated,
    /// Funding transaction confirmed
    FundingSigned,
    /// Channel is active and can route payments
    Active,
    /// Channel is being cooperatively closed
    ClosingNegotiation,
    /// Channel is closed
    Closed,
    /// Channel has been force-closed
    ForceClosed,
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
    pub channel_reserve_novas: u64,
    
    /// Dust limit for outputs
    pub dust_limit_novas: u64,
    
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
            max_htlc_value_in_flight_msat: 100_000_000, // 0.001 NOVA in millinovas
            min_htlc_value_msat: 1_000,                // 1 millinova
            max_accepted_htlcs: 30,
            cltv_expiry_delta: 40,
            channel_reserve_novas: 10_000,          // 0.0001 NOVA
            dust_limit_novas: 546,                  // Dust limit in novas
            max_commitment_transactions: 10,
            use_quantum_signatures: false,
            force_close_timeout_seconds: 86400,        // 24 hours
            min_cltv_expiry_delta: 144, // Minimum 1 day (assuming 10min blocks)
            max_cltv_expiry_delta: 2016, // Maximum 2 weeks (assuming 10min blocks)
        }
    }
}

/// Information about a hash-time-locked contract (HTLC)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Htlc {
    /// Payment hash
    pub payment_hash: [u8; 32],
    /// Amount in novas
    pub amount_novas: u64,
    /// Expiry block height
    pub expiry_height: u32,
    /// Direction (true if offering, false if receiving)
    pub is_outgoing: bool,
    /// HTLC ID
    pub id: u64,
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

/// A Lightning Network payment channel
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Channel {
    /// Unique channel ID
    pub channel_id: [u8; 32],
    
    /// Current state of the channel
    pub state: ChannelState,
    
    /// Funding transaction outpoint
    pub funding_outpoint: Option<OutPoint>,
    
    /// Capacity of the channel in novas
    pub capacity_novas: u64,
    
    /// Our balance in the channel in novas
    pub local_balance_novas: u64,
    
    /// Their balance in the channel in novas
    pub remote_balance_novas: u64,
    
    /// Our node ID (public key)
    pub local_node_id: PublicKey,
    
    /// Their node ID (public key)
    pub remote_node_id: PublicKey,
    
    /// Whether we initiated the channel
    pub is_initiator: bool,
    
    /// Current commitment transaction
    pub commitment_tx: Option<Transaction>,
    
    /// Current commitment number
    pub commitment_number: u64,
    
    /// Pending HTLCs
    pub pending_htlcs: Vec<Htlc>,
    
    /// CSV delay for the time-locked output
    pub to_self_delay: u16,
    
    /// Channel reserve amount (minimum balance)
    pub channel_reserve_novas: u64,
    
    /// Minimum HTLC value accepted
    pub min_htlc_value_novas: u64,
    
    /// Maximum number of pending HTLCs
    pub max_accepted_htlcs: u16,
    
    /// Whether the channel is public
    pub is_public: bool,
    
    /// Channel feature bits
    pub features: Vec<u8>,
    
    /// Last update timestamp
    pub last_update: u64,
}

impl Channel {
    /// Create a new channel
    pub fn new(
        local_node_id: PublicKey,
        remote_node_id: PublicKey,
        capacity_novas: u64,
        is_initiator: bool,
        is_public: bool,
    ) -> Self {
        let mut channel_id = [0u8; 32];
        // In a real implementation, the channel ID would be derived from the
        // funding transaction and output index
        // For now, we'll create a simple randomized ID
        for i in 0..32 {
            channel_id[i] = rand::random::<u8>();
        }
        
        Self {
            channel_id,
            state: ChannelState::Initializing,
            funding_outpoint: None,
            capacity_novas,
            local_balance_novas: if is_initiator { capacity_novas } else { 0 },
            remote_balance_novas: if is_initiator { 0 } else { capacity_novas },
            local_node_id,
            remote_node_id,
            is_initiator,
            commitment_tx: None,
            commitment_number: 0,
            pending_htlcs: Vec::new(),
            to_self_delay: 144, // 1 day default
            channel_reserve_novas: capacity_novas / 100, // 1% default
            min_htlc_value_novas: 1000, // 1000 sats minimum
            max_accepted_htlcs: 30,
            is_public,
            features: Vec::new(),
            last_update: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        }
    }
    
    /// Create funding transaction
    pub fn create_funding_transaction(
        &mut self,
        funding_inputs: Vec<TxIn>,
        change_address: Option<Script>,
        fee_rate: u64,
    ) -> ChannelResult<Transaction> {
        if self.state != ChannelState::Initializing {
            return Err(ChannelError::InvalidState(
                "Channel must be in initializing state to create funding transaction".to_string()
            ));
        }
        
        // In a real implementation, this would create a 2-of-2 multisig output
        // and properly calculate the change and fees
        
        // Create a simple funding transaction structure
        let funding_tx = Transaction::new(
            2, // version
            funding_inputs,
            vec![
                TxOut::new(
                    self.capacity_novas,
                    Script::new_p2wsh(&vec![
                        // In a real implementation, this would be:
                        // OP_2 <local_pubkey> <remote_pubkey> OP_2 OP_CHECKMULTISIG
                        0x52, // OP_2
                        0x21, // Push 33 bytes (compressed pubkey length)
                        // Local pubkey would go here
                        0x21, // Push 33 bytes
                        // Remote pubkey would go here
                        0x52, // OP_2
                        0xae, // OP_CHECKMULTISIG
                    ]).as_bytes().to_vec(),
                )
            ],
            0, // lock_time
        );
        
        // Add change output if specified
        if let Some(change_script) = change_address {
            // In a real implementation, calculate change amount based on inputs and fees
            let change_amount = 0; // Placeholder
            
            // Only add change output if there's a positive amount
            if change_amount > 0 {
                let change_output = TxOut::new(
                    change_amount,
                    change_script.as_bytes().to_vec(),
                );
                
                // Add change output to transaction
                // Note: In the real implementation, this would actually modify the funding_tx
            }
        }
        
        // Update channel state
        self.state = ChannelState::FundingCreated;
        
        // Set funding outpoint (normally would use the txid of the funding transaction)
        self.funding_outpoint = Some(OutPoint {
            txid: [0u8; 32], // Would be funding_tx.txid() in a real implementation
            vout: 0,
        });
        
        Ok(funding_tx)
    }
    
    /// Create initial commitment transaction
    pub fn create_commitment_transaction(&mut self) -> ChannelResult<Transaction> {
        if self.state != ChannelState::FundingCreated && self.state != ChannelState::Active {
                return Err(ChannelError::InvalidState(
                "Channel must be funded to create commitment transaction".to_string()
            ));
        }
        
        if self.funding_outpoint.is_none() {
            return Err(ChannelError::FundingError("No funding outpoint".to_string()));
        }
        
        // In a real implementation, this would:
        // 1. Create a transaction spending from the funding transaction's 2-of-2 output
        // 2. Create outputs for each side based on the current balance
        // 3. Add outputs for any pending HTLCs
        // 4. Set proper sequence numbers for timelocks
        
        let funding_outpoint = self.funding_outpoint.as_ref().unwrap();
        
        let commitment_tx = Transaction::new(
            2, // version
            vec![
                TxIn::new(
                    funding_outpoint.txid,
                    funding_outpoint.vout,
                    Vec::new(), // Script sig (will be filled with signatures)
                    0xffffffff, // Sequence
                )
            ],
            vec![
                // Output to local with their balance
                TxOut::new(
                    self.local_balance_novas,
                    Script::new_p2wpkh(&self.local_node_id.serialize()).as_bytes().to_vec(),
                ),
                // Output to remote with their balance
                TxOut::new(
                    self.remote_balance_novas,
                    Script::new_p2wpkh(&self.remote_node_id.serialize()).as_bytes().to_vec(),
                ),
            ],
            0, // lock_time
        );
        
        // In a real implementation, additional outputs would be added for each HTLC
        
        // Store the commitment transaction
        self.commitment_tx = Some(commitment_tx.clone());
        
        // Update commitment number
        self.commitment_number += 1;
        
        Ok(commitment_tx)
    }
    
    /// Add an HTLC to the channel
    pub fn add_htlc(
        &mut self,
        payment_hash: [u8; 32],
        amount_novas: u64,
        expiry_height: u32,
        is_outgoing: bool,
    ) -> ChannelResult<u64> {
        if self.state != ChannelState::Active {
            return Err(ChannelError::InvalidState(
                "Channel must be active to add HTLC".to_string()
            ));
        }
        
        // Check if we have enough balance
        if is_outgoing && self.local_balance_novas < amount_novas {
            return Err(ChannelError::InsufficientFunds(
                format!("Insufficient local balance: {} < {}", self.local_balance_novas, amount_novas)
            ));
        }
        
        if !is_outgoing && self.remote_balance_novas < amount_novas {
            return Err(ChannelError::InsufficientFunds(
                format!("Insufficient remote balance: {} < {}", self.remote_balance_novas, amount_novas)
            ));
        }
        
        // Check HTLC limits
        if self.pending_htlcs.len() >= self.max_accepted_htlcs as usize {
            return Err(ChannelError::HtlcError(
                "Maximum number of HTLCs reached".to_string()
            ));
        }
        
        if amount_novas < self.min_htlc_value_novas {
            return Err(ChannelError::HtlcError(
                format!("HTLC amount {} below minimum {}", amount_novas, self.min_htlc_value_novas)
            ));
        }
        
        // Generate HTLC ID
        let htlc_id = self.pending_htlcs.len() as u64;
        
        // Create HTLC
        let htlc = Htlc {
            payment_hash,
            amount_novas,
            expiry_height,
            is_outgoing,
            id: htlc_id,
        };
        
        // Update balances
        if is_outgoing {
            self.local_balance_novas -= amount_novas;
        } else {
            self.remote_balance_novas -= amount_novas;
        }
        
        // Add HTLC to pending list
        self.pending_htlcs.push(htlc);
        
        // Update commitment number
        self.commitment_number += 1;
        
        info!("Added HTLC {} for {} novas", htlc_id, amount_novas);
        
        Ok(htlc_id)
    }
    
    /// Settle an HTLC with a preimage
    pub fn settle_htlc(&mut self, htlc_id: u64, preimage: [u8; 32]) -> ChannelResult<()> {
        // Find the HTLC
        let htlc_index = self.pending_htlcs.iter()
            .position(|h| h.id == htlc_id)
            .ok_or_else(|| ChannelError::HtlcError(
                format!("HTLC {} not found", htlc_id)
            ))?;
        
        let htlc = &self.pending_htlcs[htlc_index];
        
        // Verify preimage
        let mut hasher = Sha256::new();
        hasher.update(&preimage);
        let hash = hasher.finalize();
        
        if hash.as_slice() != &htlc.payment_hash {
            return Err(ChannelError::HtlcError(
                "Invalid preimage for HTLC".to_string()
            ));
        }
        
        // Update balances based on HTLC direction
        if htlc.is_outgoing {
            // We sent this HTLC, so the remote party gets the funds
            self.remote_balance_novas += htlc.amount_novas;
        } else {
            // We received this HTLC, so we get the funds
            self.local_balance_novas += htlc.amount_novas;
        }
        
        // Remove HTLC from pending list
        self.pending_htlcs.remove(htlc_index);
        
        // Update commitment number
        self.commitment_number += 1;
        
        info!("Settled HTLC {} with preimage", htlc_id);
        
        Ok(())
    }
    
    /// Fail an HTLC
    pub fn fail_htlc(&mut self, htlc_id: u64, reason: &str) -> ChannelResult<()> {
        // Find the HTLC
        let htlc_index = self.pending_htlcs.iter()
            .position(|h| h.id == htlc_id)
            .ok_or_else(|| ChannelError::HtlcError(
                format!("HTLC {} not found", htlc_id)
            ))?;
        
        let htlc = &self.pending_htlcs[htlc_index];
        
        // Return funds to the sender
        if htlc.is_outgoing {
            // We sent this HTLC, so we get the funds back
            self.local_balance_novas += htlc.amount_novas;
        } else {
            // Remote party sent this HTLC, so they get the funds back
            self.remote_balance_novas += htlc.amount_novas;
        }
        
        // Remove HTLC from pending list
        self.pending_htlcs.remove(htlc_index);
        
        // Update commitment number
        self.commitment_number += 1;
        
        warn!("Failed HTLC {} with reason: {}", htlc_id, reason);
        
        Ok(())
    }
    
    /// Initiate cooperative channel close
    pub fn initiate_close(&mut self) -> ChannelResult<Transaction> {
        if self.state != ChannelState::Active {
            return Err(ChannelError::InvalidState(
                "Channel must be active to initiate close".to_string()
            ));
        }
        
        // Check if there are pending HTLCs
        if !self.pending_htlcs.is_empty() {
            return Err(ChannelError::InvalidState(
                "Cannot close channel with pending HTLCs".to_string()
            ));
        }
        
        // Create closing transaction
        let closing_tx = self.create_closing_transaction()?;
        
        // Update state
        self.state = ChannelState::ClosingNegotiation;
        
        info!("Initiated cooperative close for channel {}", hex::encode(&self.channel_id));
        
        Ok(closing_tx)
    }
    
    /// Create a closing transaction
    fn create_closing_transaction(&self) -> ChannelResult<Transaction> {
        let funding_outpoint = self.funding_outpoint
            .as_ref()
            .ok_or_else(|| ChannelError::InvalidState(
                "No funding outpoint available".to_string()
            ))?;
        
        // Create inputs from funding transaction
        let input = TxIn::new(
            funding_outpoint.txid,
            funding_outpoint.vout,
            Vec::new(), // Script sig (will be filled with signatures)
            0xffffffff, // Sequence
        );
        
        // Create outputs for final balances
        let mut outputs = Vec::new();
        
        // Local output (if we have balance)
        if self.local_balance_novas > 0 {
            outputs.push(TxOut::new(
                self.local_balance_novas,
                Script::new_p2wpkh(&[0u8; 20]).as_bytes().to_vec(), // Placeholder script
            ));
        }
        
        // Remote output (if they have balance)
        if self.remote_balance_novas > 0 {
            outputs.push(TxOut::new(
                self.remote_balance_novas,
                Script::new_p2wpkh(&[0u8; 20]).as_bytes().to_vec(), // Placeholder script
            ));
        }
        
        Ok(Transaction::new(
            2, // version
            vec![input],
            outputs,
            0, // lock_time
        ))
    }
    
    /// Complete cooperative channel close
    pub fn complete_close(&mut self, closing_tx: Transaction) -> ChannelResult<()> {
        if self.state != ChannelState::ClosingNegotiation {
            return Err(ChannelError::InvalidState(
                "Channel must be in closing negotiation state".to_string()
            ));
        }
        
        // In a real implementation, we would verify the closing transaction
        // and ensure it matches our expectations
        
        // Update state
        self.state = ChannelState::Closed;
        
        info!("Completed cooperative close for channel {}", hex::encode(&self.channel_id));
        
        Ok(())
    }
    
    /// Force close the channel
    pub fn force_close(&mut self) -> ChannelResult<Transaction> {
        if self.state == ChannelState::Closed || self.state == ChannelState::ForceClosed {
            return Err(ChannelError::InvalidState(
                "Channel is already closed".to_string()
            ));
        }
        
        // Create force close transaction (commitment transaction)
        let force_close_tx = self.commitment_tx.clone()
            .ok_or_else(|| ChannelError::InvalidState(
                "No commitment transaction available for force close".to_string()
            ))?;
        
        // Update state
        self.state = ChannelState::ForceClosed;
        
        warn!("Force closed channel {}", hex::encode(&self.channel_id));
        
        Ok(force_close_tx)
    }

    /// Open a new channel
    pub fn open(
        peer_id: String,
        capacity: u64,
        push_amount: u64,
        config: ChannelConfig,
        quantum_scheme: Option<QuantumScheme>,
    ) -> ChannelResult<Self> {
        // Generate temporary keys for this example
        let secp = secp256k1::Secp256k1::new();
        let local_private_key = PrivateKey::from_slice(&[1u8; 32])
            .map_err(|e| ChannelError::CryptoError(format!("Invalid private key: {}", e)))?;
        let local_node_id = PublicKey::from_secret_key(&secp, &local_private_key);
        let remote_node_id = PublicKey::from_slice(&[
            0x02, // Compressed public key prefix
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x02,
        ]).map_err(|e| ChannelError::CryptoError(format!("Invalid public key: {}", e)))?; // Placeholder
        
        let mut channel = Channel::new(
            local_node_id,
            remote_node_id,
            capacity,
            true, // We are the initiator
            config.announce_channel,
        );
        
        // Apply configuration
        channel.channel_reserve_novas = config.channel_reserve_novas;
        channel.min_htlc_value_novas = config.min_htlc_value_msat / 1000; // Convert from msat to novas
        channel.max_accepted_htlcs = config.max_accepted_htlcs;
        
        // If there's a push amount, adjust balances
        if push_amount > 0 {
            if push_amount > capacity {
            return Err(ChannelError::InvalidState(
                    "Push amount cannot exceed channel capacity".to_string()
                ));
            }
            channel.local_balance_novas = capacity - push_amount;
            channel.remote_balance_novas = push_amount;
        }
        
        Ok(channel)
    }

    /// Get channel ID
    pub fn id(&self) -> ChannelId {
        ChannelId::from_bytes(self.channel_id)
    }

    /// Create a cooperative close transaction
    pub fn cooperative_close(&self) -> ChannelResult<Transaction> {
        if self.state != ChannelState::Active {
            return Err(ChannelError::InvalidState(
                "Channel must be active for cooperative close".to_string()
            ));
        }
        
        if !self.pending_htlcs.is_empty() {
            return Err(ChannelError::InvalidState(
                "Cannot cooperatively close channel with pending HTLCs".to_string()
            ));
        }
        
        let funding_outpoint = self.funding_outpoint
            .as_ref()
            .ok_or_else(|| ChannelError::InvalidState(
                "No funding outpoint available".to_string()
            ))?;
        
        let input = TxIn::new(
            funding_outpoint.txid,
            funding_outpoint.vout,
            Vec::new(),
            0xffffffff,
        );
        
        let mut outputs = Vec::new();
        
        if self.local_balance_novas > 0 {
            outputs.push(TxOut::new(
                self.local_balance_novas,
                Script::new_p2wpkh(&[0u8; 20]).as_bytes().to_vec(),
            ));
        }
        
        if self.remote_balance_novas > 0 {
            outputs.push(TxOut::new(
                self.remote_balance_novas,
                Script::new_p2wpkh(&[0u8; 20]).as_bytes().to_vec(),
            ));
        }
        
        Ok(Transaction::new(
            2,
            vec![input],
            outputs,
            0,
        ))
    }

    /// Fulfill an HTLC (alias for settle_htlc)
    pub fn fulfill_htlc(&mut self, htlc_id: u64, preimage: [u8; 32]) -> ChannelResult<()> {
        self.settle_htlc(htlc_id, preimage)
    }

    /// Get pending HTLCs
    pub fn get_pending_htlcs(&self) -> Vec<Htlc> {
        self.pending_htlcs.clone()
    }

    /// Get channel information
    pub fn get_info(&self) -> ChannelInfo {
        ChannelInfo {
            id: ChannelId::from_bytes(self.channel_id),
            state: self.state,
            capacity: self.capacity_novas,
            local_balance_msat: self.local_balance_novas * 1000, // Convert to millisatoshis
            remote_balance_msat: self.remote_balance_novas * 1000,
            is_public: self.is_public,
            pending_htlcs: self.pending_htlcs.len() as u16,
            config: ChannelConfig::default(), // In real implementation, store actual config
            uptime_seconds: 0, // Would track actual uptime
            update_count: self.commitment_number,
        }
    }
}

/// Manager for Lightning Network channels
pub struct ChannelManager {
    /// Channels by channel ID
    channels: HashMap<[u8; 32], Channel>,
    
    /// Local node ID
    local_node_id: PublicKey,
    
    /// Local private key
    local_private_key: PrivateKey,
}

impl ChannelManager {
    /// Create a new channel manager
    pub fn new(local_private_key: PrivateKey) -> Self {
        let secp = secp256k1::Secp256k1::new();
        let local_node_id = PublicKey::from_secret_key(&secp, &local_private_key);
        
        Self {
            channels: HashMap::new(),
            local_node_id,
            local_private_key,
        }
    }
    
    /// Open a new channel
    pub fn open_channel(
        &mut self,
        remote_node_id: PublicKey,
        capacity_sat: u64,
        is_public: bool,
    ) -> ChannelResult<[u8; 32]> {
        // Create a new channel
        let channel = Channel::new(
            self.local_node_id.clone(),
            remote_node_id,
            capacity_sat,
            true, // We are the initiator
            is_public,
        );
        
        let channel_id = channel.channel_id;
        
        // Store the channel
        self.channels.insert(channel_id, channel);
        
        Ok(channel_id)
    }
    
    /// Get a channel by ID
    pub fn get_channel(&self, channel_id: &[u8; 32]) -> ChannelResult<&Channel> {
        self.channels.get(channel_id)
            .ok_or_else(|| ChannelError::ChannelNotFound(hex::encode(channel_id)))
    }
    
    /// Get a mutable reference to a channel by ID
    pub fn get_channel_mut(&mut self, channel_id: &[u8; 32]) -> ChannelResult<&mut Channel> {
        self.channels.get_mut(channel_id)
            .ok_or_else(|| ChannelError::ChannelNotFound(hex::encode(channel_id)))
    }
    
    /// List all channels
    pub fn list_channels(&self) -> Vec<&Channel> {
        self.channels.values().collect()
    }
    
    /// Handle channel update from peer
    pub fn handle_channel_update(
        &mut self,
        channel_id: &[u8; 32],
        update_type: &str,
        payload: &[u8],
    ) -> ChannelResult<()> {
        // In a real implementation, this would parse and process various update types
        // like funding_created, funding_signed, commitment_signed, etc.
        
        Ok(())
    }
    
    /// Close all channels
    pub fn close_all_channels(&mut self) -> Vec<ChannelResult<Transaction>> {
        let channel_ids: Vec<[u8; 32]> = self.channels.keys().cloned().collect();
        
        let mut results = Vec::new();
        
        for channel_id in channel_ids {
            let result = match self.get_channel(&channel_id) {
                Ok(channel) => {
                    if channel.state == ChannelState::Active {
                        match self.get_channel_mut(&channel_id) {
                            Ok(channel) => channel.initiate_close(),
                            Err(e) => Err(e),
                        }
                    } else {
                        Err(ChannelError::InvalidState(
                            "Channel is not active".to_string()
                        ))
                    }
                },
                Err(e) => Err(e),
            };
            
            results.push(result);
        }
        
        results
    }
}

 