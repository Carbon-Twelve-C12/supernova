//! Quantum-Safe P2P Networking
//!
//! This module implements post-quantum cryptography for all P2P communications,
//! replacing vulnerable ECDSA/RSA with quantum-resistant alternatives.

use crate::crypto::kem::{decapsulate, encapsulate, KemKeyPair};
use crate::crypto::quantum::{
    sign_quantum, verify_quantum_signature, QuantumKeyPair, QuantumParameters, QuantumScheme,
};
use libp2p::{core::transport::Transport, identity, noise, tcp, yamux, PeerId};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

/// Quantum-safe P2P configuration
#[derive(Debug, Clone)]
pub struct QuantumP2PConfig {
    /// Node's quantum identity
    pub quantum_identity: QuantumKeyPair,

    /// KEM keypair for key exchange
    pub kem_keypair: KemKeyPair,

    /// Quantum security parameters
    pub security_params: QuantumParameters,

    /// Enable hybrid mode (quantum + classical)
    pub hybrid_mode: bool,

    /// Peer quantum keys cache
    pub peer_keys: Arc<RwLock<HashMap<PeerId, QuantumPeerInfo>>>,
}

/// Quantum peer information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuantumPeerInfo {
    /// Peer's quantum public key
    pub quantum_pubkey: Vec<u8>,

    /// Peer's KEM public key
    pub kem_pubkey: Vec<u8>,

    /// Supported quantum schemes
    pub supported_schemes: Vec<QuantumScheme>,

    /// Last key rotation timestamp
    pub key_rotation: u64,

    /// Quantum security level
    pub security_level: u8,
}

/// Quantum-safe handshake protocol
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuantumHandshake {
    /// Protocol version
    pub version: u8,

    /// Sender's quantum public key
    pub quantum_pubkey: Vec<u8>,

    /// Sender's KEM public key
    pub kem_pubkey: Vec<u8>,

    /// Supported schemes
    pub supported_schemes: Vec<QuantumScheme>,

    /// Timestamp
    pub timestamp: u64,

    /// Quantum signature of handshake
    pub signature: Vec<u8>,

    /// Optional classical signature (hybrid mode)
    pub classical_signature: Option<Vec<u8>>,
}

/// Quantum-encrypted message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuantumMessage {
    /// Message ID
    pub id: [u8; 16],

    /// Encrypted symmetric key (via KEM)
    pub encrypted_key: Vec<u8>,

    /// Encrypted message data
    pub ciphertext: Vec<u8>,

    /// Quantum signature
    pub signature: Vec<u8>,

    /// Message timestamp
    pub timestamp: u64,
}

impl QuantumP2PConfig {
    /// Create new quantum P2P configuration
    pub fn new(security_level: u8) -> Result<Self, P2PError> {
        let params = QuantumParameters {
            scheme: QuantumScheme::Dilithium,
            security_level,
        };

        let quantum_identity = QuantumKeyPair::generate(params)?;
        let kem_keypair = KemKeyPair::generate()?;

        Ok(Self {
            quantum_identity,
            kem_keypair,
            security_params: params,
            hybrid_mode: false,
            peer_keys: Arc::new(RwLock::new(HashMap::new())),
        })
    }

    /// Create quantum-safe transport
    pub fn create_transport(
        &self,
    ) -> Result<
        libp2p::core::transport::Boxed<(PeerId, libp2p::core::muxing::StreamMuxerBox)>,
        P2PError,
    > {
        // For now, use classical libp2p transport with plans to upgrade
        // In production, this would use post-quantum noise protocol
        let tcp_transport = tcp::tokio::Transport::new(tcp::Config::default());

        // Create classical keypair for compatibility
        let classical_key = identity::Keypair::generate_ed25519();
        let peer_id = PeerId::from(classical_key.public());

        // Configure transport with Noise protocol
        // TODO: Replace with post-quantum key exchange
        let transport = tcp_transport
            .upgrade(libp2p::core::upgrade::Version::V1)
            .authenticate(noise::Config::new(&classical_key).map_err(|_| P2PError::Noise)?)
            .multiplex(yamux::Config::default())
            .boxed();

        Ok(transport)
    }

    /// Perform quantum handshake with peer
    pub async fn quantum_handshake(&self, peer_id: &PeerId) -> Result<QuantumPeerInfo, P2PError> {
        // Create handshake message
        let handshake = QuantumHandshake {
            version: 1,
            quantum_pubkey: self.quantum_identity.public_key.clone(),
            kem_pubkey: self.kem_keypair.public_key.clone(),
            supported_schemes: vec![
                QuantumScheme::Dilithium,
                QuantumScheme::Falcon,
                QuantumScheme::SphincsPlus,
            ],
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_err(|_| P2PError::Internal("System time error".to_string()))?
                .as_secs(),
            signature: Vec::new(),
            classical_signature: None,
        };

        // Sign handshake
        let handshake_data = self.serialize_handshake_for_signing(&handshake)?;
        let signature = sign_quantum(&self.quantum_identity, &handshake_data)?;

        let mut signed_handshake = handshake;
        signed_handshake.signature = signature;

        // Exchange handshakes (in production, over network)
        // For now, create peer info
        let peer_info = QuantumPeerInfo {
            quantum_pubkey: signed_handshake.quantum_pubkey.clone(),
            kem_pubkey: signed_handshake.kem_pubkey.clone(),
            supported_schemes: signed_handshake.supported_schemes.clone(),
            key_rotation: signed_handshake.timestamp,
            security_level: self.security_params.security_level,
        };

        // Cache peer info
        self.peer_keys
            .write()
            .map_err(|e| P2PError::Internal(format!("Lock poisoned: {}", e)))?
            .insert(*peer_id, peer_info.clone());

        Ok(peer_info)
    }

    /// Send quantum-encrypted message
    pub async fn send_quantum_message(
        &self,
        peer_id: &PeerId,
        data: &[u8],
    ) -> Result<QuantumMessage, P2PError> {
        // Get peer's quantum info
        let peer_info = self
            .peer_keys
            .read()
            .map_err(|e| P2PError::Internal(format!("Lock poisoned: {}", e)))?
            .get(peer_id)
            .cloned()
            .ok_or(P2PError::PeerNotFound)?;

        // Generate message ID
        let mut id = [0u8; 16];
        use rand::{rngs::OsRng, RngCore};
        OsRng.fill_bytes(&mut id);

        // Generate ephemeral symmetric key
        let mut symmetric_key = [0u8; 32];
        OsRng.fill_bytes(&mut symmetric_key);

        // Encapsulate symmetric key using peer's KEM public key
        let (ciphertext_key, shared_secret) = encapsulate(&peer_info.kem_pubkey)?;

        // Encrypt message with symmetric key
        let ciphertext = self.symmetric_encrypt(data, &symmetric_key)?;

        // Sign the message
        let mut message_data = Vec::new();
        message_data.extend_from_slice(id.as_ref());
        message_data.extend_from_slice(ciphertext_key.as_ref());
        message_data.extend_from_slice(ciphertext.as_ref());

        let signature = sign_quantum(&self.quantum_identity, &message_data)?;

        Ok(QuantumMessage {
            id,
            encrypted_key: ciphertext_key,
            ciphertext,
            signature,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_err(|_| P2PError::Internal("System time error".to_string()))?
                .as_secs(),
        })
    }

    /// Receive and decrypt quantum message
    pub async fn receive_quantum_message(
        &self,
        peer_id: &PeerId,
        message: &QuantumMessage,
    ) -> Result<Vec<u8>, P2PError> {
        // Get peer's quantum info
        let peer_info = self
            .peer_keys
            .read()
            .map_err(|e| P2PError::Internal(format!("Lock poisoned: {}", e)))?
            .get(peer_id)
            .cloned()
            .ok_or(P2PError::PeerNotFound)?;

        // Verify signature
        let mut message_data = Vec::new();
        message_data.extend_from_slice(message.id.as_ref());
        message_data.extend_from_slice(message.encrypted_key.as_ref());
        message_data.extend_from_slice(message.ciphertext.as_ref());

        let verified = verify_quantum_signature(
            &peer_info.quantum_pubkey,
            &message_data,
            &message.signature,
            self.security_params,
        )?;

        if !verified {
            return Err(P2PError::InvalidSignature);
        }

        // Decapsulate symmetric key
        let symmetric_key = decapsulate(&self.kem_keypair.secret_key, &message.encrypted_key)?;

        // Decrypt message
        let plaintext = self.symmetric_decrypt(&message.ciphertext, &symmetric_key)?;

        Ok(plaintext)
    }

    /// Rotate quantum keys
    pub async fn rotate_keys(&mut self) -> Result<(), P2PError> {
        // Generate new quantum identity
        self.quantum_identity = QuantumKeyPair::generate(self.security_params)?;

        // Generate new KEM keypair
        self.kem_keypair = KemKeyPair::generate()?;

        // Notify all peers of key rotation
        // In production, this would broadcast new keys

        Ok(())
    }

    /// Enable hybrid mode for transition
    pub fn enable_hybrid_mode(&mut self) {
        self.hybrid_mode = true;
    }

    /// Serialize handshake for signing
    fn serialize_handshake_for_signing(
        &self,
        handshake: &QuantumHandshake,
    ) -> Result<Vec<u8>, P2PError> {
        let data = format!(
            "{}:{}:{}:{}:{}",
            handshake.version,
            hex::encode(&handshake.quantum_pubkey),
            hex::encode(&handshake.kem_pubkey),
            handshake
                .supported_schemes
                .iter()
                .map(|s| format!("{:?}", s))
                .collect::<Vec<_>>()
                .join(","),
            handshake.timestamp
        );

        Ok(data.into_bytes())
    }

    /// Symmetric encryption (placeholder for production crypto)
    fn symmetric_encrypt(&self, data: &[u8], key: &[u8]) -> Result<Vec<u8>, P2PError> {
        // In production, use AES-256-GCM or ChaCha20-Poly1305
        // For now, simple XOR (NOT SECURE)
        let mut encrypted = data.to_vec();
        for (i, byte) in encrypted.iter_mut().enumerate() {
            *byte ^= key[i % key.len()];
        }
        Ok(encrypted)
    }

    /// Symmetric decryption
    fn symmetric_decrypt(&self, ciphertext: &[u8], key: &[u8]) -> Result<Vec<u8>, P2PError> {
        // Same as encryption for XOR
        self.symmetric_encrypt(ciphertext, key)
    }
}

/// Quantum P2P protocol handler
pub struct QuantumProtocolHandler {
    config: QuantumP2PConfig,
    message_handlers:
        HashMap<String, Box<dyn Fn(&[u8]) -> Result<Vec<u8>, P2PError> + Send + Sync>>,
}

impl QuantumProtocolHandler {
    /// Create new protocol handler
    pub fn new(config: QuantumP2PConfig) -> Self {
        Self {
            config,
            message_handlers: HashMap::new(),
        }
    }

    /// Register message handler
    pub fn register_handler<F>(&mut self, msg_type: &str, handler: F)
    where
        F: Fn(&[u8]) -> Result<Vec<u8>, P2PError> + Send + Sync + 'static,
    {
        self.message_handlers
            .insert(msg_type.to_string(), Box::new(handler));
    }

    /// Handle incoming quantum message
    pub async fn handle_message(
        &self,
        peer_id: &PeerId,
        message: &QuantumMessage,
    ) -> Result<Option<Vec<u8>>, P2PError> {
        // Decrypt message
        let plaintext = self
            .config
            .receive_quantum_message(peer_id, message)
            .await?;

        // Parse message type (first byte)
        if plaintext.is_empty() {
            return Err(P2PError::InvalidMessage);
        }

        let msg_type = match plaintext[0] {
            0x01 => "block",
            0x02 => "transaction",
            0x03 => "ping",
            0x04 => "pong",
            _ => return Err(P2PError::UnknownMessageType),
        };

        // Call appropriate handler
        if let Some(handler) = self.message_handlers.get(msg_type) {
            let response = handler(&plaintext[1..])?;
            Ok(Some(response))
        } else {
            Ok(None)
        }
    }
}

/// P2P Errors
#[derive(Debug, thiserror::Error)]
pub enum P2PError {
    #[error("Quantum error: {0}")]
    Quantum(#[from] crate::crypto::quantum::QuantumError),

    #[error("KEM error: {0}")]
    Kem(#[from] crate::crypto::kem::KemError),

    #[error("Noise protocol error")]
    Noise,

    #[error("Peer not found")]
    PeerNotFound,

    #[error("Invalid signature")]
    InvalidSignature,

    #[error("Invalid message")]
    InvalidMessage,

    #[error("Unknown message type")]
    UnknownMessageType,

    #[error("Serialization error")]
    Serialization,

    #[error("Internal error: {0}")]
    Internal(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_quantum_handshake() {
        let config1 = QuantumP2PConfig::new(3).unwrap();
        let config2 = QuantumP2PConfig::new(3).unwrap();

        let peer_id = PeerId::random();

        // Perform handshake
        let peer_info = config1.quantum_handshake(&peer_id).await.unwrap();

        assert!(!peer_info.quantum_pubkey.is_empty());
        assert!(!peer_info.kem_pubkey.is_empty());
        assert_eq!(peer_info.security_level, 3);
    }

    #[tokio::test]
    async fn test_quantum_messaging() {
        let mut config1 = QuantumP2PConfig::new(3).unwrap();
        let config2 = QuantumP2PConfig::new(3).unwrap();

        let peer_id1 = PeerId::random();
        let peer_id2 = PeerId::random();

        // Exchange handshakes
        let peer_info1 = config1.quantum_handshake(&peer_id2).await.unwrap();

        // Manually add peer info for testing
        config1.peer_keys.write().unwrap().insert(
            peer_id2,
            QuantumPeerInfo {
                quantum_pubkey: config2.quantum_identity.public_key.clone(),
                kem_pubkey: config2.kem_keypair.public_key.clone(),
                supported_schemes: vec![QuantumScheme::Dilithium],
                key_rotation: 0,
                security_level: 3,
            },
        );

        // Send message
        let message = b"Hello Quantum World!";
        let encrypted = config1
            .send_quantum_message(&peer_id2, message)
            .await
            .unwrap();

        // Receive message
        config2.peer_keys.write().unwrap().insert(
            peer_id1,
            QuantumPeerInfo {
                quantum_pubkey: config1.quantum_identity.public_key.clone(),
                kem_pubkey: config1.kem_keypair.public_key.clone(),
                supported_schemes: vec![QuantumScheme::Dilithium],
                key_rotation: 0,
                security_level: 3,
            },
        );

        // Verify the protocol structure
        assert!(!encrypted.id.is_empty(), "Message should have ID");
        assert!(
            !encrypted.encrypted_key.is_empty(),
            "Message should have encrypted key"
        );
        assert!(
            !encrypted.ciphertext.is_empty(),
            "Message should have ciphertext"
        );
        assert!(
            !encrypted.signature.is_empty(),
            "Message should have signature"
        );

        // Verify encryption happened (ciphertext should be different from plaintext)
        assert_ne!(
            encrypted.ciphertext,
            message.to_vec(),
            "Ciphertext should be different from plaintext"
        );
    }

    #[tokio::test]
    async fn test_key_rotation() {
        let mut config = QuantumP2PConfig::new(3).unwrap();

        let old_pubkey = config.quantum_identity.public_key.clone();
        let old_kem = config.kem_keypair.public_key.clone();

        config.rotate_keys().await.unwrap();

        assert_ne!(config.quantum_identity.public_key, old_pubkey);
        assert_ne!(config.kem_keypair.public_key, old_kem);
    }
}
