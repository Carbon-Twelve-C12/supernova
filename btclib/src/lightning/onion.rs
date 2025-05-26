//! Lightning Network Onion Routing
//!
//! This module implements onion routing for Lightning Network payments,
//! providing privacy and security for multi-hop payments.

use crate::crypto::quantum::QuantumScheme;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;
use tracing::{debug, info, warn, error};
use rand::{Rng, RngCore};
use sha2::{Sha256, Digest};
use serde_bytes;

/// Maximum number of hops in an onion route
pub const MAX_ONION_HOPS: usize = 20;

/// Size of onion packet in bytes
pub const ONION_PACKET_SIZE: usize = 1366;

/// Size of shared secret in bytes
pub const SHARED_SECRET_SIZE: usize = 32;

/// Size of per-hop payload
pub const PER_HOP_PAYLOAD_SIZE: usize = 65;

/// Onion packet for Lightning Network payments
#[derive(Debug, Clone)]
pub struct OnionPacket {
    /// Version byte
    pub version: u8,
    
    /// Public key for ECDH
    pub public_key: [u8; 33],
    
    /// Encrypted routing information
    pub routing_info: Vec<u8>,
    
    /// HMAC for integrity
    pub hmac: [u8; 32],
}

impl Serialize for OnionPacket {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut state = serializer.serialize_struct("OnionPacket", 4)?;
        state.serialize_field("version", &self.version)?;
        state.serialize_field("public_key", &self.public_key.as_slice())?;
        state.serialize_field("routing_info", &self.routing_info)?;
        state.serialize_field("hmac", &self.hmac.as_slice())?;
        state.end()
    }
}

impl<'de> Deserialize<'de> for OnionPacket {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de::{self, Deserializer, MapAccess, SeqAccess, Visitor};
        use std::fmt;

        #[derive(Deserialize)]
        #[serde(field_identifier, rename_all = "lowercase")]
        enum Field { Version, PublicKey, RoutingInfo, Hmac }

        struct OnionPacketVisitor;

        impl<'de> Visitor<'de> for OnionPacketVisitor {
            type Value = OnionPacket;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("struct OnionPacket")
            }

            fn visit_map<V>(self, mut map: V) -> Result<OnionPacket, V::Error>
            where
                V: MapAccess<'de>,
            {
                let mut version = None;
                let mut public_key = None;
                let mut routing_info = None;
                let mut hmac = None;
                
                while let Some(key) = map.next_key()? {
                    match key {
                        Field::Version => {
                            if version.is_some() {
                                return Err(de::Error::duplicate_field("version"));
                            }
                            version = Some(map.next_value()?);
                        }
                        Field::PublicKey => {
                            if public_key.is_some() {
                                return Err(de::Error::duplicate_field("public_key"));
                            }
                            let pk_vec: Vec<u8> = map.next_value()?;
                            if pk_vec.len() != 33 {
                                return Err(de::Error::invalid_length(pk_vec.len(), &"33"));
                            }
                            let mut pk_array = [0u8; 33];
                            pk_array.copy_from_slice(&pk_vec);
                            public_key = Some(pk_array);
                        }
                        Field::RoutingInfo => {
                            if routing_info.is_some() {
                                return Err(de::Error::duplicate_field("routing_info"));
                            }
                            routing_info = Some(map.next_value()?);
                        }
                        Field::Hmac => {
                            if hmac.is_some() {
                                return Err(de::Error::duplicate_field("hmac"));
                            }
                            let hmac_vec: Vec<u8> = map.next_value()?;
                            if hmac_vec.len() != 32 {
                                return Err(de::Error::invalid_length(hmac_vec.len(), &"32"));
                            }
                            let mut hmac_array = [0u8; 32];
                            hmac_array.copy_from_slice(&hmac_vec);
                            hmac = Some(hmac_array);
                        }
                    }
                }
                
                let version = version.ok_or_else(|| de::Error::missing_field("version"))?;
                let public_key = public_key.ok_or_else(|| de::Error::missing_field("public_key"))?;
                let routing_info = routing_info.ok_or_else(|| de::Error::missing_field("routing_info"))?;
                let hmac = hmac.ok_or_else(|| de::Error::missing_field("hmac"))?;
                
                Ok(OnionPacket {
                    version,
                    public_key,
                    routing_info,
                    hmac,
                })
            }
        }

        const FIELDS: &'static [&'static str] = &["version", "public_key", "routing_info", "hmac"];
        deserializer.deserialize_struct("OnionPacket", FIELDS, OnionPacketVisitor)
    }
}

/// Per-hop payload containing routing instructions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerHopPayload {
    /// Amount to forward in millisatoshis
    pub amount_msat: u64,
    /// Outgoing CLTV value
    pub outgoing_cltv_value: u32,
    /// Short channel ID for next hop (0 for final hop)
    pub short_channel_id: u64,
    /// Additional TLV fields
    pub tlv_payload: HashMap<u64, Vec<u8>>,
}

/// Route hop information for onion construction
#[derive(Debug, Clone)]
pub struct RouteHop {
    /// Node public key
    pub node_id: [u8; 33],
    /// Short channel ID
    pub short_channel_id: u64,
    /// Amount to forward
    pub amount_msat: u64,
    /// CLTV expiry delta
    pub cltv_expiry_delta: u16,
    /// Fee for this hop
    pub fee_msat: u64,
}

/// Shared secret for onion encryption
#[derive(Debug, Clone)]
pub struct SharedSecret([u8; SHARED_SECRET_SIZE]);

impl SharedSecret {
    pub fn new(secret: [u8; SHARED_SECRET_SIZE]) -> Self {
        Self(secret)
    }
    
    pub fn as_bytes(&self) -> &[u8; SHARED_SECRET_SIZE] {
        &self.0
    }
    
    pub fn into_bytes(self) -> [u8; SHARED_SECRET_SIZE] {
        self.0
    }
}

/// Onion router for constructing and processing onion packets
pub struct OnionRouter {
    /// Quantum security configuration
    quantum_scheme: Option<QuantumScheme>,
    /// Node's private key for ECDH
    private_key: [u8; 32],
    /// Node's public key
    public_key: [u8; 33],
}

impl OnionRouter {
    /// Create a new onion router
    pub fn new(private_key: [u8; 32], quantum_scheme: Option<QuantumScheme>) -> Self {
        // Derive public key from private key (simplified)
        let public_key = Self::derive_public_key(&private_key);
        
        Self {
            quantum_scheme,
            private_key,
            public_key,
        }
    }
    
    /// Construct an onion packet for a payment route
    pub fn construct_onion(
        &self,
        route: &[RouteHop],
        payment_hash: &[u8; 32],
        associated_data: &[u8],
    ) -> Result<OnionPacket, OnionError> {
        if route.is_empty() {
            return Err(OnionError::EmptyRoute);
        }
        
        if route.len() > MAX_ONION_HOPS {
            return Err(OnionError::TooManyHops(route.len()));
        }
        
        // Generate ephemeral key pair for this onion
        let mut ephemeral_private_key = [0u8; 32];
        rand::thread_rng().fill_bytes(&mut ephemeral_private_key);
        let ephemeral_public_key = Self::derive_public_key(&ephemeral_private_key);
        
        // Generate shared secrets for each hop
        let mut shared_secrets = Vec::new();
        let mut current_private_key = ephemeral_private_key;
        
        for hop in route {
            let shared_secret = self.generate_shared_secret(&current_private_key, &hop.node_id)?;
            shared_secrets.push(shared_secret.clone());
            
            // Blind the private key for the next hop
            current_private_key = self.blind_private_key(&current_private_key, &shared_secret)?;
        }
        
        // Construct per-hop payloads
        let mut payloads = Vec::new();
        for (i, hop) in route.iter().enumerate() {
            let is_final_hop = i == route.len() - 1;
            
            let payload = PerHopPayload {
                amount_msat: hop.amount_msat,
                outgoing_cltv_value: if is_final_hop { 0 } else { hop.cltv_expiry_delta as u32 },
                short_channel_id: if is_final_hop { 0 } else { hop.short_channel_id },
                tlv_payload: HashMap::new(),
            };
            
            payloads.push(payload);
        }
        
        // Encrypt the onion layers (working backwards from destination)
        let mut routing_info = vec![0u8; ONION_PACKET_SIZE - 66]; // Packet size minus version, pubkey, and HMAC
        
        for i in (0..route.len()).rev() {
            let payload_bytes = self.serialize_payload(&payloads[i])?;
            let shared_secret = &shared_secrets[i];
            
            // Encrypt this layer
            self.encrypt_layer(&mut routing_info, &payload_bytes, shared_secret)?;
        }
        
        // Calculate HMAC for the first hop
        let hmac = self.calculate_hmac(&routing_info, &shared_secrets[0], associated_data)?;
        
        let onion_packet = OnionPacket {
            version: 0,
            public_key: ephemeral_public_key,
            routing_info,
            hmac,
        };
        
        debug!("Constructed onion packet for {} hops", route.len());
        
        Ok(onion_packet)
    }
    
    /// Process an incoming onion packet
    pub fn process_onion(
        &self,
        packet: &OnionPacket,
        associated_data: &[u8],
    ) -> Result<OnionProcessResult, OnionError> {
        // Generate shared secret with the sender
        let shared_secret = self.generate_shared_secret(&self.private_key, &packet.public_key)?;
        
        // Verify HMAC
        let expected_hmac = self.calculate_hmac(&packet.routing_info, &shared_secret, associated_data)?;
        if expected_hmac != packet.hmac {
            return Err(OnionError::InvalidHmac);
        }
        
        // Decrypt our layer
        let mut decrypted_info = packet.routing_info.clone();
        self.decrypt_layer(&mut decrypted_info, &shared_secret)?;
        
        // Extract our payload
        let payload = self.extract_payload(&decrypted_info)?;
        
        // Check if this is the final hop
        if payload.short_channel_id == 0 {
            // We are the final destination
            return Ok(OnionProcessResult::FinalHop {
                payload,
                payment_hash: [0u8; 32], // Would extract from TLV in real implementation
            });
        }
        
        // We need to forward to the next hop
        // Blind the public key for the next hop
        let next_public_key = self.blind_public_key(&packet.public_key, &shared_secret)?;
        
        // Shift the routing info and pad with zeros
        let mut next_routing_info = vec![0u8; packet.routing_info.len()];
        next_routing_info[..packet.routing_info.len() - PER_HOP_PAYLOAD_SIZE]
            .copy_from_slice(&decrypted_info[PER_HOP_PAYLOAD_SIZE..]);
        
        // Calculate HMAC for next hop (simplified - would use next hop's shared secret)
        let next_hmac = [0u8; 32]; // Placeholder
        
        let next_packet = OnionPacket {
            version: packet.version,
            public_key: next_public_key,
            routing_info: next_routing_info,
            hmac: next_hmac,
        };
        
        Ok(OnionProcessResult::ForwardHop {
            payload: payload.clone(),
            next_packet,
            next_channel_id: payload.short_channel_id,
        })
    }
    
    /// Generate a shared secret using ECDH
    fn generate_shared_secret(
        &self,
        private_key: &[u8; 32],
        public_key: &[u8; 33],
    ) -> Result<SharedSecret, OnionError> {
        // In a real implementation, this would use proper ECDH
        // For now, we'll use a hash-based approach
        let mut hasher = Sha256::new();
        hasher.update(private_key);
        hasher.update(public_key);
        let hash = hasher.finalize();
        
        let mut secret = [0u8; SHARED_SECRET_SIZE];
        secret.copy_from_slice(&hash[..SHARED_SECRET_SIZE]);
        
        Ok(SharedSecret::new(secret))
    }
    
    /// Derive public key from private key (simplified)
    fn derive_public_key(private_key: &[u8; 32]) -> [u8; 33] {
        let mut public_key = [0u8; 33];
        public_key[0] = 0x02; // Compressed public key prefix
        
        // Hash the private key to get a deterministic public key (simplified)
        let mut hasher = Sha256::new();
        hasher.update(private_key);
        let hash = hasher.finalize();
        public_key[1..].copy_from_slice(&hash[..32]);
        
        public_key
    }
    
    /// Blind a private key with a shared secret
    fn blind_private_key(
        &self,
        private_key: &[u8; 32],
        shared_secret: &SharedSecret,
    ) -> Result<[u8; 32], OnionError> {
        let mut hasher = Sha256::new();
        hasher.update(private_key);
        hasher.update(shared_secret.as_bytes());
        let hash = hasher.finalize();
        
        let mut blinded_key = [0u8; 32];
        blinded_key.copy_from_slice(&hash[..32]);
        
        Ok(blinded_key)
    }
    
    /// Blind a public key with a shared secret
    fn blind_public_key(
        &self,
        public_key: &[u8; 33],
        shared_secret: &SharedSecret,
    ) -> Result<[u8; 33], OnionError> {
        let mut hasher = Sha256::new();
        hasher.update(public_key);
        hasher.update(shared_secret.as_bytes());
        let hash = hasher.finalize();
        
        let mut blinded_key = [0u8; 33];
        blinded_key[0] = public_key[0]; // Keep the prefix
        blinded_key[1..].copy_from_slice(&hash[..32]);
        
        Ok(blinded_key)
    }
    
    /// Serialize a per-hop payload
    fn serialize_payload(&self, payload: &PerHopPayload) -> Result<Vec<u8>, OnionError> {
        // Simplified serialization - in practice would use proper TLV encoding
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&payload.amount_msat.to_be_bytes());
        bytes.extend_from_slice(&payload.outgoing_cltv_value.to_be_bytes());
        bytes.extend_from_slice(&payload.short_channel_id.to_be_bytes());
        
        // Pad to fixed size
        bytes.resize(PER_HOP_PAYLOAD_SIZE, 0);
        
        Ok(bytes)
    }
    
    /// Extract payload from decrypted routing info
    fn extract_payload(&self, routing_info: &[u8]) -> Result<PerHopPayload, OnionError> {
        if routing_info.len() < PER_HOP_PAYLOAD_SIZE {
            return Err(OnionError::InvalidPayload);
        }
        
        let payload_bytes = &routing_info[..PER_HOP_PAYLOAD_SIZE];
        
        // Deserialize (simplified)
        let amount_msat = u64::from_be_bytes(payload_bytes[0..8].try_into().unwrap());
        let outgoing_cltv_value = u32::from_be_bytes(payload_bytes[8..12].try_into().unwrap());
        let short_channel_id = u64::from_be_bytes(payload_bytes[12..20].try_into().unwrap());
        
        Ok(PerHopPayload {
            amount_msat,
            outgoing_cltv_value,
            short_channel_id,
            tlv_payload: HashMap::new(),
        })
    }
    
    /// Encrypt a layer of the onion
    fn encrypt_layer(
        &self,
        routing_info: &mut [u8],
        payload: &[u8],
        shared_secret: &SharedSecret,
    ) -> Result<(), OnionError> {
        // Shift existing data to make room for new payload
        let shift_amount = payload.len();
        for i in (shift_amount..routing_info.len()).rev() {
            routing_info[i] = routing_info[i - shift_amount];
        }
        
        // Insert new payload at the beginning
        routing_info[..payload.len()].copy_from_slice(payload);
        
        // Encrypt the entire routing info with the shared secret
        self.stream_cipher_encrypt(routing_info, shared_secret)?;
        
        Ok(())
    }
    
    /// Decrypt a layer of the onion
    fn decrypt_layer(
        &self,
        routing_info: &mut [u8],
        shared_secret: &SharedSecret,
    ) -> Result<(), OnionError> {
        // Decrypt with stream cipher
        self.stream_cipher_decrypt(routing_info, shared_secret)?;
        
        Ok(())
    }
    
    /// Stream cipher encryption (simplified ChaCha20-like)
    fn stream_cipher_encrypt(
        &self,
        data: &mut [u8],
        shared_secret: &SharedSecret,
    ) -> Result<(), OnionError> {
        // Generate keystream from shared secret
        let keystream = self.generate_keystream(shared_secret, data.len())?;
        
        // XOR data with keystream
        for (i, byte) in data.iter_mut().enumerate() {
            *byte ^= keystream[i];
        }
        
        Ok(())
    }
    
    /// Stream cipher decryption (same as encryption for XOR cipher)
    fn stream_cipher_decrypt(
        &self,
        data: &mut [u8],
        shared_secret: &SharedSecret,
    ) -> Result<(), OnionError> {
        self.stream_cipher_encrypt(data, shared_secret)
    }
    
    /// Generate keystream from shared secret
    fn generate_keystream(
        &self,
        shared_secret: &SharedSecret,
        length: usize,
    ) -> Result<Vec<u8>, OnionError> {
        let mut keystream = Vec::with_capacity(length);
        let mut counter = 0u64;
        
        while keystream.len() < length {
            let mut hasher = Sha256::new();
            hasher.update(shared_secret.as_bytes());
            hasher.update(&counter.to_be_bytes());
            let hash = hasher.finalize();
            
            let remaining = length - keystream.len();
            let to_take = std::cmp::min(remaining, hash.len());
            keystream.extend_from_slice(&hash[..to_take]);
            
            counter += 1;
        }
        
        Ok(keystream)
    }
    
    /// Calculate HMAC for integrity verification
    fn calculate_hmac(
        &self,
        data: &[u8],
        shared_secret: &SharedSecret,
        associated_data: &[u8],
    ) -> Result<[u8; 32], OnionError> {
        let mut hasher = Sha256::new();
        hasher.update(shared_secret.as_bytes());
        hasher.update(data);
        hasher.update(associated_data);
        let hash = hasher.finalize();
        
        let mut hmac = [0u8; 32];
        hmac.copy_from_slice(&hash[..32]);
        
        Ok(hmac)
    }
}

/// Result of processing an onion packet
#[derive(Debug)]
pub enum OnionProcessResult {
    /// This node is the final destination
    FinalHop {
        payload: PerHopPayload,
        payment_hash: [u8; 32],
    },
    /// This node should forward to the next hop
    ForwardHop {
        payload: PerHopPayload,
        next_packet: OnionPacket,
        next_channel_id: u64,
    },
}

/// Onion routing errors
#[derive(Debug, Error)]
pub enum OnionError {
    #[error("Empty route")]
    EmptyRoute,
    
    #[error("Too many hops: {0} (max: {MAX_ONION_HOPS})")]
    TooManyHops(usize),
    
    #[error("Invalid HMAC")]
    InvalidHmac,
    
    #[error("Invalid payload")]
    InvalidPayload,
    
    #[error("Encryption error: {0}")]
    EncryptionError(String),
    
    #[error("Key derivation error: {0}")]
    KeyDerivationError(String),
    
    #[error("Quantum signature error: {0}")]
    QuantumSignatureError(String),
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_onion_construction() {
        let private_key = [1u8; 32];
        let router = OnionRouter::new(private_key, None);
        
        let route = vec![
            RouteHop {
                node_id: [2u8; 33],
                short_channel_id: 12345,
                amount_msat: 100000,
                cltv_expiry_delta: 40,
                fee_msat: 1000,
            },
            RouteHop {
                node_id: [3u8; 33],
                short_channel_id: 0, // Final hop
                amount_msat: 99000,
                cltv_expiry_delta: 0,
                fee_msat: 0,
            },
        ];
        
        let payment_hash = [4u8; 32];
        let associated_data = b"test_payment";
        
        let result = router.construct_onion(&route, &payment_hash, associated_data);
        assert!(result.is_ok());
        
        let onion_packet = result.unwrap();
        assert_eq!(onion_packet.version, 0);
        assert_eq!(onion_packet.routing_info.len(), ONION_PACKET_SIZE - 66);
    }
    
    #[test]
    fn test_shared_secret_generation() {
        let router = OnionRouter::new([1u8; 32], None);
        let private_key = [2u8; 32];
        let public_key = [3u8; 33];
        
        let secret1 = router.generate_shared_secret(&private_key, &public_key).unwrap();
        let secret2 = router.generate_shared_secret(&private_key, &public_key).unwrap();
        
        // Should be deterministic
        assert_eq!(secret1.as_bytes(), secret2.as_bytes());
    }
    
    #[test]
    fn test_payload_serialization() {
        let router = OnionRouter::new([1u8; 32], None);
        
        let payload = PerHopPayload {
            amount_msat: 100000,
            outgoing_cltv_value: 500000,
            short_channel_id: 12345,
            tlv_payload: HashMap::new(),
        };
        
        let serialized = router.serialize_payload(&payload).unwrap();
        assert_eq!(serialized.len(), PER_HOP_PAYLOAD_SIZE);
        
        let deserialized = router.extract_payload(&serialized).unwrap();
        assert_eq!(deserialized.amount_msat, payload.amount_msat);
        assert_eq!(deserialized.outgoing_cltv_value, payload.outgoing_cltv_value);
        assert_eq!(deserialized.short_channel_id, payload.short_channel_id);
    }
} 