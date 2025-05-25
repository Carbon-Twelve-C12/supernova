// SuperNova Lightning Network - Wire Protocol Implementation
//
// This file contains the implementation of the Lightning Network wire protocol,
// which handles message serialization, encryption, and exchange between nodes.

use crate::lightning::channel::{ChannelId, ChannelState};
use crate::lightning::invoice::{PaymentHash, PaymentPreimage};
use thiserror::Error;
use serde::{Serialize, Deserialize};
use std::time::{SystemTime, UNIX_EPOCH};
use rand::{thread_rng, Rng, RngCore};
use sha2::{Sha256, Digest};

/// Error types for wire protocol operations
#[derive(Debug, Error)]
pub enum LightningError {
    #[error("Serialization error: {0}")]
    SerializationError(String),
    
    #[error("Deserialization error: {0}")]
    DeserializationError(String),
    
    #[error("Encryption error: {0}")]
    EncryptionError(String),
    
    #[error("Decryption error: {0}")]
    DecryptionError(String),
    
    #[error("Authentication error: {0}")]
    AuthenticationError(String),
    
    #[error("Protocol error: {0}")]
    ProtocolError(String),
    
    #[error("Connection error: {0}")]
    ConnectionError(String),
    
    #[error("Invalid message: {0}")]
    InvalidMessage(String),
}

/// Message type for Lightning Network communication
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MessageType {
    /// Initialize connection
    Init,
    
    /// Error message
    Error,
    
    /// Ping message (keep-alive)
    Ping,
    
    /// Pong response to ping
    Pong,
    
    /// Open a new channel
    OpenChannel,
    
    /// Accept a channel open request
    AcceptChannel,
    
    /// Fund a channel
    FundingCreated,
    
    /// Sign funding transaction
    FundingSigned,
    
    /// Funding transaction is ready
    FundingLocked,
    
    /// Channel announcement
    ChannelAnnouncement,
    
    /// Channel update
    ChannelUpdate,
    
    /// Add HTLCs to a channel
    UpdateAddHtlc,
    
    /// Fulfill an HTLC
    UpdateFulfillHtlc,
    
    /// Fail an HTLC
    UpdateFailHtlc,
    
    /// Sign a commitment transaction
    CommitmentSigned,
    
    /// Revoke a previous commitment transaction
    RevokeAndAck,
    
    /// Close a channel
    Shutdown,
    
    /// Signature for a closing transaction
    ClosingSigned,
    
    /// Announce a new node
    NodeAnnouncement,
    
    /// Request gossip messages
    GossipTimestampFilter,
}

/// Main message structure for Lightning Network
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    /// Message type
    pub msg_type: MessageType,
    
    /// Channel ID if applicable
    pub channel_id: Option<ChannelId>,
    
    /// Data payload
    pub payload: Vec<u8>,
    
    /// Timestamp
    pub timestamp: u64,
    
    /// Signature
    pub signature: Option<Vec<u8>>,
}

impl Message {
    /// Create a new message
    pub fn new(msg_type: MessageType, channel_id: Option<ChannelId>, payload: Vec<u8>) -> Self {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_else(|_| std::time::Duration::from_secs(0))
            .as_secs();
            
        Self {
            msg_type,
            channel_id,
            payload,
            timestamp,
            signature: None,
        }
    }
    
    /// Sign the message
    pub fn sign(&mut self, private_key: &[u8]) -> Result<(), LightningError> {
        // In a real implementation, this would sign the message with the private key
        // For simplicity, we'll just set a dummy signature
        let mut rng = thread_rng();
        let mut signature = vec![0u8; 64];
        rng.fill_bytes(&mut signature[..]);
        
        self.signature = Some(signature);
        
        Ok(())
    }
    
    /// Verify the message signature
    pub fn verify(&self, public_key: &[u8]) -> Result<bool, LightningError> {
        // In a real implementation, this would verify the signature
        // For simplicity, we'll just check if a signature exists
        if self.signature.is_some() {
            Ok(true)
        } else {
            Err(LightningError::AuthenticationError(
                "Message has no signature".to_string()
            ))
        }
    }
    
    /// Serialize the message to bytes
    pub fn serialize(&self) -> Result<Vec<u8>, LightningError> {
        // Use bincode for serialization
        bincode::serialize(self)
            .map_err(|e| LightningError::SerializationError(e.to_string()))
    }
    
    /// Deserialize from bytes
    pub fn deserialize(bytes: &[u8]) -> Result<Self, LightningError> {
        // Use bincode for deserialization
        bincode::deserialize(bytes)
            .map_err(|e| LightningError::DeserializationError(e.to_string()))
    }
    
    /// Get hash of the message
    pub fn hash(&self) -> [u8; 32] {
        let serialized = self.serialize().unwrap_or_default();
        let mut hasher = Sha256::new();
        hasher.update(&serialized);
        let result = hasher.finalize();
        
        let mut hash = [0u8; 32];
        hash.copy_from_slice(&result);
        
        hash
    }
}

/// Initialize connection message payload
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InitPayload {
    /// Protocol version
    pub version: u32,
    
    /// Local features
    pub local_features: Vec<u8>,
    
    /// Global features
    pub global_features: Vec<u8>,
    
    /// Node ID
    pub node_id: String,
}

/// Error message payload
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorPayload {
    /// Error code
    pub code: u16,
    
    /// Error message
    pub message: String,
    
    /// Data for debugging
    pub data: Option<Vec<u8>>,
}

/// Open channel message payload
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenChannelPayload {
    /// Chain hash
    pub chain_hash: [u8; 32],
    
    /// Temporary channel ID
    pub temporary_channel_id: [u8; 32],
    
    /// Funding amount in satoshis
    pub funding_satoshis: u64,
    
    /// Push amount in millisatoshis
    pub push_msat: u64,
    
    /// Dust limit in satoshis
    pub dust_limit_satoshis: u64,
    
    /// Maximum HTLC value in flight
    pub max_htlc_value_in_flight_msat: u64,
    
    /// Channel reserve in satoshis
    pub channel_reserve_satoshis: u64,
    
    /// Minimum HTLC value in millisatoshis
    pub htlc_minimum_msat: u64,
    
    /// Fee rate per kiloweight
    pub feerate_per_kw: u32,
    
    /// To-self delay in blocks
    pub to_self_delay: u16,
    
    /// Maximum accepted HTLCs
    pub max_accepted_htlcs: u16,
    
    /// Funding public key
    pub funding_pubkey: Vec<u8>,
    
    /// Remote public key
    pub revocation_basepoint: Vec<u8>,
    
    /// Payment basepoint
    pub payment_basepoint: Vec<u8>,
    
    /// Delayed payment basepoint
    pub delayed_payment_basepoint: Vec<u8>,
    
    /// HTLC basepoint
    pub htlc_basepoint: Vec<u8>,
    
    /// First per-commitment point
    pub first_per_commitment_point: Vec<u8>,
    
    /// Channel flags
    pub channel_flags: u8,
}

/// HTLC message payload
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HtlcPayload {
    /// Channel ID
    pub channel_id: ChannelId,
    
    /// HTLC ID
    pub htlc_id: u64,
    
    /// Amount in millisatoshis
    pub amount_msat: u64,
    
    /// Payment hash
    pub payment_hash: PaymentHash,
    
    /// CLTV expiry
    pub cltv_expiry: u32,
    
    /// Onion routing packet
    pub onion_routing_packet: Vec<u8>,
}

/// HTLC fulfill message payload
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HtlcFulfillPayload {
    /// Channel ID
    pub channel_id: ChannelId,
    
    /// HTLC ID
    pub htlc_id: u64,
    
    /// Payment preimage
    pub payment_preimage: PaymentPreimage,
}

/// HTLC fail message payload
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HtlcFailPayload {
    /// Channel ID
    pub channel_id: ChannelId,
    
    /// HTLC ID
    pub htlc_id: u64,
    
    /// Reason for failure
    pub reason: Vec<u8>,
}

/// Message factory for creating Lightning Network messages
pub struct MessageFactory {
    /// Local node ID
    local_node_id: String,
    
    /// Private key for signing
    private_key: Vec<u8>,
}

impl MessageFactory {
    /// Create a new message factory
    pub fn new(local_node_id: String, private_key: Vec<u8>) -> Self {
        Self {
            local_node_id,
            private_key,
        }
    }
    
    /// Create an init message
    pub fn create_init(&self, version: u32) -> Result<Message, LightningError> {
        let payload = InitPayload {
            version,
            local_features: vec![0, 0], // No special features
            global_features: vec![0, 0], // No special features
            node_id: self.local_node_id.clone(),
        };
        
        let serialized = bincode::serialize(&payload)
            .map_err(|e| LightningError::SerializationError(e.to_string()))?;
            
        let mut message = Message::new(MessageType::Init, None, serialized);
        message.sign(&self.private_key)?;
        
        Ok(message)
    }
    
    /// Create an error message
    pub fn create_error(&self, channel_id: Option<ChannelId>, code: u16, message: &str) -> Result<Message, LightningError> {
        let payload = ErrorPayload {
            code,
            message: message.to_string(),
            data: None,
        };
        
        let serialized = bincode::serialize(&payload)
            .map_err(|e| LightningError::SerializationError(e.to_string()))?;
            
        let mut message = Message::new(MessageType::Error, channel_id, serialized);
        message.sign(&self.private_key)?;
        
        Ok(message)
    }
    
    /// Create an open channel message
    pub fn create_open_channel(
        &self,
        funding_satoshis: u64,
        push_msat: u64,
    ) -> Result<Message, LightningError> {
        // Create a temporary channel ID
        let mut rng = thread_rng();
        let mut temporary_channel_id = [0u8; 32];
        rng.fill_bytes(&mut temporary_channel_id);
        
        // Create chain hash (Bitcoin mainnet in this case)
        let chain_hash = [
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01,
        ];
        
        let payload = OpenChannelPayload {
            chain_hash,
            temporary_channel_id,
            funding_satoshis,
            push_msat,
            dust_limit_satoshis: 546,
            max_htlc_value_in_flight_msat: funding_satoshis * 1000,
            channel_reserve_satoshis: funding_satoshis / 100, // 1% reserve
            htlc_minimum_msat: 1000,
            feerate_per_kw: 1000,
            to_self_delay: 144, // 1 day (6 blocks/hour * 24 hours)
            max_accepted_htlcs: 30,
            funding_pubkey: vec![0; 33], // Dummy public key
            revocation_basepoint: vec![0; 33], // Dummy basepoint
            payment_basepoint: vec![0; 33], // Dummy basepoint
            delayed_payment_basepoint: vec![0; 33], // Dummy basepoint
            htlc_basepoint: vec![0; 33], // Dummy basepoint
            first_per_commitment_point: vec![0; 33], // Dummy commitment point
            channel_flags: 0, // No special flags
        };
        
        let serialized = bincode::serialize(&payload)
            .map_err(|e| LightningError::SerializationError(e.to_string()))?;
            
        let mut message = Message::new(MessageType::OpenChannel, None, serialized);
        message.sign(&self.private_key)?;
        
        Ok(message)
    }
    
    /// Create an add HTLC message
    pub fn create_add_htlc(
        &self,
        channel_id: ChannelId,
        htlc_id: u64,
        amount_msat: u64,
        payment_hash: PaymentHash,
        cltv_expiry: u32,
    ) -> Result<Message, LightningError> {
        let payload = HtlcPayload {
            channel_id: channel_id.clone(),
            htlc_id,
            amount_msat,
            payment_hash,
            cltv_expiry,
            onion_routing_packet: vec![0; 1366], // Dummy onion packet
        };
        
        let serialized = bincode::serialize(&payload)
            .map_err(|e| LightningError::SerializationError(e.to_string()))?;
            
        let mut message = Message::new(MessageType::UpdateAddHtlc, Some(channel_id), serialized);
        message.sign(&self.private_key)?;
        
        Ok(message)
    }
    
    /// Create a fulfill HTLC message
    pub fn create_fulfill_htlc(
        &self,
        channel_id: ChannelId,
        htlc_id: u64,
        payment_preimage: PaymentPreimage,
    ) -> Result<Message, LightningError> {
        let payload = HtlcFulfillPayload {
            channel_id: channel_id.clone(),
            htlc_id,
            payment_preimage,
        };
        
        let serialized = bincode::serialize(&payload)
            .map_err(|e| LightningError::SerializationError(e.to_string()))?;
            
        let mut message = Message::new(MessageType::UpdateFulfillHtlc, Some(channel_id), serialized);
        message.sign(&self.private_key)?;
        
        Ok(message)
    }
    
    /// Create a fail HTLC message
    pub fn create_fail_htlc(
        &self,
        channel_id: ChannelId,
        htlc_id: u64,
        reason: &str,
    ) -> Result<Message, LightningError> {
        let payload = HtlcFailPayload {
            channel_id: channel_id.clone(),
            htlc_id,
            reason: reason.as_bytes().to_vec(),
        };
        
        let serialized = bincode::serialize(&payload)
            .map_err(|e| LightningError::SerializationError(e.to_string()))?;
            
        let mut message = Message::new(MessageType::UpdateFailHtlc, Some(channel_id), serialized);
        message.sign(&self.private_key)?;
        
        Ok(message)
    }
    
    /// Create a ping message
    pub fn create_ping(&self) -> Result<Message, LightningError> {
        let mut rng = thread_rng();
        let payload: Vec<u8> = (0..16).map(|_| rng.gen::<u8>()).collect();
        
        let mut message = Message::new(MessageType::Ping, None, payload);
        message.sign(&self.private_key)?;
        
        Ok(message)
    }
    
    /// Create a pong message in response to a ping
    pub fn create_pong(&self, ping_message: &Message) -> Result<Message, LightningError> {
        let mut message = Message::new(MessageType::Pong, None, ping_message.payload.clone());
        message.sign(&self.private_key)?;
        
        Ok(message)
    }
} 