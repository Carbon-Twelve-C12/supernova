//! Lightning Network Quantum Security
//!
//! This module implements quantum-resistant security features for Lightning Network
//! channels, including quantum-safe signatures and key exchange.

use crate::crypto::quantum::{QuantumScheme, QuantumKeyPair, QuantumSignature};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;
use tracing::{debug, info, warn, error};
use rand::{Rng, RngCore};
use rand::{SeedableRng};
use rand::distributions::{Distribution, Uniform};
use rand::rngs::StdRng;
use sha2::{Sha256, Digest};
use std::sync::{Arc, Mutex};

/// Quantum security level
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum QuantumSecurityLevel {
    /// NIST Level 1 (128-bit classical security)
    Level1 = 1,
    /// NIST Level 3 (192-bit classical security)
    Level3 = 3,
    /// NIST Level 5 (256-bit classical security)
    Level5 = 5,
}

impl Default for QuantumSecurityLevel {
    fn default() -> Self {
        Self::Level3
    }
}

/// Quantum-resistant channel security configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuantumChannelConfig {
    /// Primary quantum signature scheme
    pub primary_scheme: QuantumScheme,
    /// Backup quantum signature scheme (for hybrid security)
    pub backup_scheme: Option<QuantumScheme>,
    /// Security level
    pub security_level: QuantumSecurityLevel,
    /// Enable hybrid classical-quantum signatures
    pub hybrid_mode: bool,
    /// Key rotation interval in blocks
    pub key_rotation_interval: u32,
    /// Enable quantum key distribution (QKD) if available
    pub enable_qkd: bool,
}

impl Default for QuantumChannelConfig {
    fn default() -> Self {
        Self {
            primary_scheme: QuantumScheme::Dilithium,
            backup_scheme: Some(QuantumScheme::Falcon),
            security_level: QuantumSecurityLevel::Level3,
            hybrid_mode: true,
            key_rotation_interval: 1000, // Rotate keys every 1000 blocks
            enable_qkd: false,
        }
    }
}

/// Quantum-resistant channel state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuantumChannelState {
    /// Channel ID
    pub channel_id: [u8; 32],
    /// Current quantum key pair
    pub current_keypair: QuantumKeyPair,
    /// Next quantum key pair (for rotation)
    pub next_keypair: Option<QuantumKeyPair>,
    /// Quantum signature scheme in use
    pub quantum_scheme: QuantumScheme,
    /// Security level
    pub security_level: QuantumSecurityLevel,
    /// Last key rotation block height
    pub last_rotation_height: u32,
    /// Quantum random beacon (for entropy)
    pub quantum_beacon: Option<Vec<u8>>,
    /// Hybrid classical key (if hybrid mode enabled)
    pub classical_keypair: Option<[u8; 32]>,
}

/// Quantum-secured commitment transaction
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuantumCommitment {
    /// Commitment transaction data
    pub commitment_data: Vec<u8>,
    /// Quantum signature
    pub quantum_signature: QuantumSignature,
    /// Classical signature (if hybrid mode)
    pub classical_signature: Option<Vec<u8>>,
    /// Commitment number
    pub commitment_number: u64,
    /// Quantum security metadata
    pub security_metadata: QuantumSecurityMetadata,
}

/// Quantum security metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuantumSecurityMetadata {
    /// Signature scheme used
    pub scheme: QuantumScheme,
    /// Security level
    pub level: QuantumSecurityLevel,
    /// Timestamp of signature creation
    pub timestamp: u64,
    /// Quantum entropy source
    pub entropy_source: String,
    /// Post-quantum proof (if available)
    pub pq_proof: Option<Vec<u8>>,
}

/// Quantum channel security manager
pub struct QuantumChannelSecurity {
    /// Channel configurations
    channels: HashMap<[u8; 32], QuantumChannelState>,
    /// Global quantum configuration
    config: QuantumChannelConfig,
    /// Quantum random number generator
    quantum_rng: QuantumRng,
    /// Key derivation function
    kdf: QuantumKdf,
}

impl QuantumChannelSecurity {
    /// Create a new quantum channel security manager
    pub fn new(config: QuantumChannelConfig) -> Self {
        Self {
            channels: HashMap::new(),
            config,
            quantum_rng: QuantumRng::new(),
            kdf: QuantumKdf::new(),
        }
    }
    
    /// Initialize quantum security for a new channel
    pub fn initialize_channel(
        &mut self,
        channel_id: [u8; 32],
        initial_height: u32,
    ) -> Result<QuantumChannelState, QuantumSecurityError> {
        // Generate initial quantum key pair
        let current_keypair = self.generate_quantum_keypair()?;
        
        // Generate classical key pair if hybrid mode is enabled
        let classical_keypair = if self.config.hybrid_mode {
            let mut key = [0u8; 32];
            self.quantum_rng.fill_bytes(&mut key)?;
            Some(key)
        } else {
            None
        };
        
        // Initialize quantum beacon
        let quantum_beacon = if self.config.enable_qkd {
            Some(self.generate_quantum_beacon()?)
        } else {
            None
        };
        
        let channel_state = QuantumChannelState {
            channel_id,
            current_keypair,
            next_keypair: None,
            quantum_scheme: self.config.primary_scheme.clone(),
            security_level: self.config.security_level,
            last_rotation_height: initial_height,
            quantum_beacon,
            classical_keypair,
        };
        
        self.channels.insert(channel_id, channel_state.clone());
        
        info!("Initialized quantum security for channel {}: scheme={:?}, level={:?}", 
              hex::encode(channel_id), self.config.primary_scheme, self.config.security_level);
        
        Ok(channel_state)
    }
    
    /// Create a quantum-secured commitment
    pub fn create_quantum_commitment(
        &self,
        channel_id: &[u8; 32],
        commitment_data: Vec<u8>,
        commitment_number: u64,
    ) -> Result<QuantumCommitment, QuantumSecurityError> {
        let channel_state = self.channels.get(channel_id)
            .ok_or(QuantumSecurityError::ChannelNotFound(hex::encode(channel_id)))?;
        
        // Create quantum signature
        let quantum_signature = self.sign_quantum(&commitment_data, &channel_state.current_keypair)?;
        
        // Create classical signature if hybrid mode
        let classical_signature = if let Some(classical_key) = &channel_state.classical_keypair {
            Some(self.sign_classical(&commitment_data, classical_key)?)
        } else {
            None
        };
        
        // Create security metadata
        let security_metadata = QuantumSecurityMetadata {
            scheme: channel_state.quantum_scheme.clone(),
            level: channel_state.security_level,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            entropy_source: "quantum_rng".to_string(),
            pq_proof: None, // Could add zero-knowledge proofs here
        };
        
        let commitment = QuantumCommitment {
            commitment_data,
            quantum_signature,
            classical_signature,
            commitment_number,
            security_metadata,
        };
        
        debug!("Created quantum commitment {} for channel {}", 
               commitment_number, hex::encode(channel_id));
        
        Ok(commitment)
    }
    
    /// Generate a quantum key pair
    fn generate_quantum_keypair(&self) -> Result<QuantumKeyPair, QuantumSecurityError> {
        // Use the quantum scheme to generate a key pair
        let mut rng = rand::rngs::OsRng;
        
        let params = crate::crypto::quantum::QuantumParameters {
            scheme: self.config.primary_scheme.clone(),
            security_level: self.config.security_level as u8,
        };
        
        QuantumKeyPair::generate(&mut rng, params)
            .map_err(|e| QuantumSecurityError::KeyGenerationFailed(e.to_string()))
    }
    
    /// Sign data with quantum signature
    fn sign_quantum(
        &self,
        data: &[u8],
        keypair: &QuantumKeyPair,
    ) -> Result<QuantumSignature, QuantumSecurityError> {
        let signature_bytes = keypair.sign(data)
            .map_err(|e| QuantumSecurityError::SignatureFailed(e.to_string()))?;
        
        // Convert Vec<u8> to QuantumSignature
        let quantum_signature = QuantumSignature {
            signature: signature_bytes,
            parameters: keypair.parameters.clone(),
        };
        
        Ok(quantum_signature)
    }
    
    /// Sign data with classical signature
    fn sign_classical(
        &self,
        data: &[u8],
        private_key: &[u8; 32],
    ) -> Result<Vec<u8>, QuantumSecurityError> {
        // Simple HMAC-based signature (in practice would use ECDSA)
        let mut hasher = Sha256::new();
        hasher.update(private_key);
        hasher.update(data);
        let signature = hasher.finalize();
        
        Ok(signature.to_vec())
    }
    
    /// Generate quantum beacon for entropy
    fn generate_quantum_beacon(&self) -> Result<Vec<u8>, QuantumSecurityError> {
        let mut beacon = vec![0u8; 64]; // 512 bits of quantum entropy
        self.quantum_rng.fill_bytes(&mut beacon)?;
        Ok(beacon)
    }
    
    /// Get channel quantum state
    pub fn get_channel_state(&self, channel_id: &[u8; 32]) -> Option<&QuantumChannelState> {
        self.channels.get(channel_id)
    }
}

/// Thread-safe quantum random number generator
#[derive(Clone)]
pub struct QuantumRng {
    rng: Arc<Mutex<StdRng>>,
    entropy_pool: Arc<Mutex<Vec<u8>>>,
}

impl QuantumRng {
    pub fn new() -> Self {
        Self {
            rng: Arc::new(Mutex::new(StdRng::from_entropy())),
            entropy_pool: Arc::new(Mutex::new(Vec::new())),
        }
    }
    
    pub fn fill_bytes(&self, dest: &mut [u8]) -> Result<(), QuantumSecurityError> {
        // In a real implementation, this would use quantum entropy sources
        // For now, use cryptographically secure PRNG
        let mut rng = self.rng.lock().unwrap();
        rng.fill_bytes(dest);
        Ok(())
    }
}

/// Quantum key derivation function
pub struct QuantumKdf {
    // Quantum-resistant key derivation
}

impl QuantumKdf {
    pub fn new() -> Self {
        Self {}
    }
}

/// Quantum security errors
#[derive(Debug, Error)]
pub enum QuantumSecurityError {
    #[error("Channel not found: {0}")]
    ChannelNotFound(String),
    
    #[error("Key generation failed: {0}")]
    KeyGenerationFailed(String),
    
    #[error("Signature failed: {0}")]
    SignatureFailed(String),
    
    #[error("Verification failed: {0}")]
    VerificationFailed(String),
    
    #[error("Quantum RNG error: {0}")]
    QuantumRngError(String),
    
    #[error("Key derivation failed: {0}")]
    KeyDerivationFailed(String),
    
    #[error("Invalid security level: {0}")]
    InvalidSecurityLevel(u8),
    
    #[error("Quantum hardware unavailable")]
    QuantumHardwareUnavailable,
    
    #[error("Unsupported scheme: {0}")]
    UnsupportedScheme(String),
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_quantum_channel_initialization() {
        let config = QuantumChannelConfig::default();
        let mut security = QuantumChannelSecurity::new(config);
        
        let channel_id = [1u8; 32];
        let result = security.initialize_channel(channel_id, 0);
        
        assert!(result.is_ok());
        let state = result.unwrap();
        assert_eq!(state.channel_id, channel_id);
        assert_eq!(state.quantum_scheme, QuantumScheme::Dilithium);
    }
} 