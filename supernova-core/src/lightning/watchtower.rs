//! Lightning Network Watchtower
//!
//! This module implements watchtower functionality for monitoring Lightning Network
//! channels and protecting against malicious channel closures.

use crate::crypto::quantum::QuantumScheme;
use crate::lightning::channel::ChannelId;
use crate::types::transaction::Transaction;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};
use thiserror::Error;
use tracing::{debug, error, info, warn};

/// Encrypted channel state for watchtower storage
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptedChannelState {
    /// Encrypted channel state data
    pub encrypted_data: Vec<u8>,
    /// Initialization vector for encryption
    pub iv: Vec<u8>,
    /// Authentication tag
    pub tag: Vec<u8>,
}

/// Breach remedy transaction
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BreachRemedy {
    /// Transaction to broadcast in case of breach
    pub justice_transaction: Transaction,
    /// Channel ID this remedy is for
    pub channel_id: ChannelId,
    /// Commitment number this remedy applies to
    pub commitment_number: u64,
    /// Encrypted with client's key
    pub encrypted_data: EncryptedChannelState,
}

/// Watchtower client information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WatchtowerClient {
    /// Client identifier
    pub client_id: String,
    /// Client's public key for encryption
    pub public_key: Vec<u8>,
    /// Number of channels being monitored
    pub channel_count: usize,
    /// Last update timestamp
    pub last_update: u64,
    /// Whether quantum security is enabled
    pub quantum_enabled: bool,
}

/// Channel monitoring information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelMonitorInfo {
    /// Channel ID
    pub channel_id: ChannelId,
    /// Client ID
    pub client_id: String,
    /// Latest commitment number seen
    pub latest_commitment_number: u64,
    /// Breach remedies for different commitment states
    pub breach_remedies: HashMap<u64, BreachRemedy>,
    /// Last update timestamp
    pub last_update: u64,
    /// Whether this channel uses quantum security
    pub quantum_secured: bool,
}

/// Watchtower service for monitoring Lightning Network channels
pub struct Watchtower {
    /// Registered clients
    clients: HashMap<String, WatchtowerClient>,
    /// Monitored channels
    channels: HashMap<ChannelId, ChannelMonitorInfo>,
    /// Quantum security configuration
    quantum_scheme: Option<QuantumScheme>,
    /// Watchtower configuration
    config: WatchtowerConfig,
}

/// Watchtower configuration
#[derive(Debug, Clone)]
pub struct WatchtowerConfig {
    /// Maximum number of clients to serve
    pub max_clients: usize,
    /// Maximum number of channels per client
    pub max_channels_per_client: usize,
    /// How long to keep old breach remedies (in seconds)
    pub remedy_retention_period: u64,
    /// Enable quantum-resistant monitoring
    pub quantum_monitoring: bool,
}

impl Default for WatchtowerConfig {
    fn default() -> Self {
        Self {
            max_clients: 1000,
            max_channels_per_client: 100,
            remedy_retention_period: 30 * 24 * 3600, // 30 days
            quantum_monitoring: true,
        }
    }
}

impl Watchtower {
    /// Create a new watchtower service
    pub fn new(config: WatchtowerConfig, quantum_scheme: Option<QuantumScheme>) -> Self {
        Self {
            clients: HashMap::new(),
            channels: HashMap::new(),
            quantum_scheme,
            config,
        }
    }

    /// Register a new client with the watchtower
    pub fn register_client(
        &mut self,
        client_id: String,
        public_key: Vec<u8>,
        quantum_enabled: bool,
    ) -> Result<(), WatchError> {
        if self.clients.len() >= self.config.max_clients {
            return Err(WatchError::TooManyClients);
        }

        if self.clients.contains_key(&client_id) {
            return Err(WatchError::ClientAlreadyExists(client_id));
        }

        let client = WatchtowerClient {
            client_id: client_id.clone(),
            public_key,
            channel_count: 0,
            last_update: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            quantum_enabled,
        };

        self.clients.insert(client_id.clone(), client);

        info!("Registered new watchtower client: {}", client_id);

        Ok(())
    }

    /// Register a channel for monitoring
    pub fn register_channel(
        &mut self,
        channel_id: [u8; 32],
        client_id: &str,
        _encrypted_state: EncryptedChannelState,
    ) -> Result<(), WatchError> {
        // Convert [u8; 32] to ChannelId using the from_bytes method
        let channel_id = ChannelId::from_bytes(channel_id);

        // Verify client exists
        let client = self
            .clients
            .get_mut(client_id)
            .ok_or_else(|| WatchError::ClientNotFound(client_id.to_string()))?;

        // Check channel limits
        if client.channel_count >= self.config.max_channels_per_client {
            return Err(WatchError::TooManyChannels);
        }

        // Check if channel already exists
        if self.channels.contains_key(&channel_id) {
            return Err(WatchError::ChannelAlreadyExists(channel_id.to_string()));
        }

        let monitor_info = ChannelMonitorInfo {
            channel_id: channel_id.clone(),
            client_id: client_id.to_string(),
            latest_commitment_number: 0,
            breach_remedies: HashMap::new(),
            last_update: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            quantum_secured: client.quantum_enabled && self.config.quantum_monitoring,
        };

        self.channels.insert(channel_id.clone(), monitor_info);
        client.channel_count += 1;
        client.last_update = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        info!("Registered channel {} for client {}", channel_id, client_id);

        Ok(())
    }

    /// Update channel state with new breach remedy
    pub fn update_channel_state(
        &mut self,
        channel_id: &ChannelId,
        commitment_number: u64,
        breach_remedy: BreachRemedy,
    ) -> Result<(), WatchError> {
        let channel_info = self
            .channels
            .get_mut(channel_id)
            .ok_or_else(|| WatchError::ChannelNotFound(channel_id.to_string()))?;

        // Verify commitment number is newer
        if commitment_number <= channel_info.latest_commitment_number {
            return Err(WatchError::InvalidCommitmentNumber(commitment_number));
        }

        // Store the breach remedy
        channel_info
            .breach_remedies
            .insert(commitment_number, breach_remedy);
        channel_info.latest_commitment_number = commitment_number;
        channel_info.last_update = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        // Update client last update time
        if let Some(client) = self.clients.get_mut(&channel_info.client_id) {
            client.last_update = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs();
        }

        debug!(
            "Updated channel {} to commitment number {}",
            channel_id, commitment_number
        );

        // Clean up old breach remedies
        self.cleanup_old_remedies(channel_id);

        Ok(())
    }

    /// Check for channel breaches
    pub fn check_for_breaches(
        &self,
        channel_id: &ChannelId,
        observed_transaction: &Transaction,
    ) -> Result<Option<Transaction>, WatchError> {
        let channel_info = self
            .channels
            .get(channel_id)
            .ok_or_else(|| WatchError::ChannelNotFound(channel_id.to_string()))?;

        // Extract commitment number from the observed transaction
        let observed_commitment_number = self.extract_commitment_number(observed_transaction)?;

        // Check if this is an old commitment (breach attempt)
        if (observed_commitment_number as u64) < channel_info.latest_commitment_number {
            warn!(
                "Detected breach attempt on channel {}: observed commitment {} < latest {}",
                channel_id, observed_commitment_number, channel_info.latest_commitment_number
            );

            // Find the appropriate breach remedy
            if let Some(remedy) = channel_info
                .breach_remedies
                .get(&(observed_commitment_number as u64))
            {
                info!(
                    "Broadcasting justice transaction for channel {}",
                    channel_id
                );
                return Ok(Some(remedy.justice_transaction.clone()));
            } else {
                warn!(
                    "No breach remedy found for commitment number {}",
                    observed_commitment_number
                );
                return Err(WatchError::NoBreachRemedy(
                    observed_commitment_number as u64,
                ));
            }
        }

        // No breach detected
        Ok(None)
    }

    /// Unregister a channel from monitoring
    pub fn unregister_channel(&mut self, channel_id: &[u8; 32]) -> Result<(), WatchError> {
        // Convert [u8; 32] to ChannelId
        let channel_id = ChannelId::from_bytes(*channel_id);

        let channel_info = self
            .channels
            .remove(&channel_id)
            .ok_or_else(|| WatchError::ChannelNotFound(channel_id.to_string()))?;

        // Update client channel count
        if let Some(client) = self.clients.get_mut(&channel_info.client_id) {
            client.channel_count = client.channel_count.saturating_sub(1);
            client.last_update = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs();
        }

        info!("Unregistered channel {} from monitoring", channel_id);

        Ok(())
    }

    /// Get watchtower statistics
    pub fn get_statistics(&self) -> WatchtowerStats {
        let total_clients = self.clients.len();
        let total_channels = self.channels.len();
        let quantum_channels = self.channels.values().filter(|c| c.quantum_secured).count();

        let total_remedies = self
            .channels
            .values()
            .map(|c| c.breach_remedies.len())
            .sum();

        WatchtowerStats {
            total_clients,
            total_channels,
            quantum_channels,
            total_remedies,
        }
    }

    /// Clean up old breach remedies to save storage
    fn cleanup_old_remedies(&mut self, channel_id: &ChannelId) {
        if let Some(channel_info) = self.channels.get_mut(channel_id) {
            let current_time = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs();
            let cutoff_time = current_time.saturating_sub(self.config.remedy_retention_period);

            // Keep only recent remedies and the latest few
            let latest_commitment = channel_info.latest_commitment_number;
            channel_info.breach_remedies.retain(|&commitment_num, _| {
                // Keep if it's one of the latest 10 commitments or recent
                commitment_num > latest_commitment.saturating_sub(10)
                    || channel_info.last_update > cutoff_time
            });
        }
    }

    /// Extract commitment number from a transaction
    fn extract_commitment_number(&self, transaction: &Transaction) -> Result<u32, WatchError> {
        // Parse the transaction to extract the commitment number from witness data
        // In Lightning Network, commitment transactions have a specific structure
        // where the commitment number is encoded in the locktime or witness data

        // For now, we'll extract it from the transaction's locktime field
        // In a real implementation, this would parse the witness stack
        let locktime = transaction.lock_time();

        // The commitment number is typically encoded in the lower 32 bits of locktime
        let commitment_number = locktime;

        // Validate that this looks like a commitment transaction
        if transaction.inputs().is_empty() || transaction.outputs().len() < 2 {
            return Err(WatchError::InvalidCommitmentNumber(
                commitment_number as u64,
            ));
        }

        // Additional validation: check if the transaction has the expected structure
        // Commitment transactions should have specific output patterns
        let has_to_local = transaction.outputs().iter().any(|output| {
            // Check for to_local output pattern (simplified)
            output.pub_key_script.len() > 20 // Basic size check
        });

        let has_to_remote = transaction.outputs().iter().any(|output| {
            // Check for to_remote output pattern (simplified)
            output.pub_key_script.len() >= 20 && output.pub_key_script.len() <= 25
        });

        if !has_to_local && !has_to_remote {
            return Err(WatchError::InvalidCommitmentNumber(
                commitment_number as u64,
            ));
        }

        Ok(commitment_number)
    }

    /// Perform periodic maintenance
    pub fn perform_maintenance(&mut self) {
        let current_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        // Clean up old breach remedies for all channels
        let channel_ids: Vec<ChannelId> = self.channels.keys().cloned().collect();
        for channel_id in channel_ids {
            self.cleanup_old_remedies(&channel_id);
        }

        // Remove inactive clients (no updates for a long time)
        let inactive_cutoff = current_time.saturating_sub(30 * 24 * 3600); // 30 days
        let inactive_clients: Vec<String> = self
            .clients
            .iter()
            .filter(|(_, client)| client.last_update < inactive_cutoff)
            .map(|(id, _)| id.clone())
            .collect();

        for client_id in inactive_clients {
            // Remove all channels for this client first
            let client_channels: Vec<ChannelId> = self
                .channels
                .iter()
                .filter(|(_, info)| info.client_id == client_id)
                .map(|(id, _)| id.clone())
                .collect();

            for channel_id in client_channels {
                self.channels.remove(&channel_id);
            }

            self.clients.remove(&client_id);
            info!("Removed inactive client: {}", client_id);
        }

        debug!(
            "Performed watchtower maintenance: {} clients, {} channels",
            self.clients.len(),
            self.channels.len()
        );
    }

    /// Start the watchtower service
    pub async fn start(&self) -> Result<(), WatchError> {
        info!(
            "Starting watchtower service with {} max clients",
            self.config.max_clients
        );

        // In a real implementation, this would:
        // 1. Start listening for client connections
        // 2. Begin monitoring the blockchain for commitment transactions
        // 3. Set up periodic maintenance tasks
        // 4. Initialize quantum security if enabled

        if self.quantum_scheme.is_some() {
            info!("Quantum-resistant monitoring enabled");
        }

        Ok(())
    }

    /// Stop the watchtower service
    pub async fn stop(&self) -> Result<(), WatchError> {
        info!("Stopping watchtower service");

        // In a real implementation, this would:
        // 1. Stop accepting new client connections
        // 2. Gracefully disconnect existing clients
        // 3. Stop blockchain monitoring
        // 4. Clean up resources

        Ok(())
    }
}

/// Watchtower statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WatchtowerStats {
    pub total_clients: usize,
    pub total_channels: usize,
    pub quantum_channels: usize,
    pub total_remedies: usize,
}

/// Channel monitor for individual channel security
pub struct ChannelMonitor {
    /// Monitored channels
    channels: HashMap<ChannelId, ChannelMonitorInfo>,
    /// Quantum security configuration
    quantum_scheme: Option<QuantumScheme>,
}

impl Default for ChannelMonitor {
    fn default() -> Self {
        Self::new()
    }
}

impl ChannelMonitor {
    /// Create a new channel monitor
    pub fn new() -> Self {
        Self {
            channels: HashMap::new(),
            quantum_scheme: None,
        }
    }

    /// Register a channel for monitoring
    pub fn register_channel(
        &mut self,
        channel_id: [u8; 32],
        client_id: &str,
        _encrypted_state: EncryptedChannelState,
    ) -> Result<(), WatchError> {
        // Convert [u8; 32] to ChannelId
        let channel_id = ChannelId::from_bytes(channel_id);

        let monitor_info = ChannelMonitorInfo {
            channel_id: channel_id.clone(),
            client_id: client_id.to_string(),
            latest_commitment_number: 0,
            breach_remedies: HashMap::new(),
            last_update: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            quantum_secured: self.quantum_scheme.is_some(),
        };

        self.channels.insert(channel_id, monitor_info);

        Ok(())
    }

    /// Unregister a channel
    pub fn unregister_channel(&mut self, channel_id: &[u8; 32]) -> Result<(), WatchError> {
        // Convert [u8; 32] to ChannelId
        let channel_id = ChannelId::from_bytes(*channel_id);

        self.channels
            .remove(&channel_id)
            .ok_or_else(|| WatchError::ChannelNotFound(channel_id.to_string()))?;

        Ok(())
    }

    /// Get channel information
    pub fn get_channel_info(&self, channel_id: &ChannelId) -> Option<&ChannelMonitorInfo> {
        self.channels.get(channel_id)
    }
}

/// Watchtower errors
#[derive(Debug, Error)]
pub enum WatchError {
    #[error("Too many clients")]
    TooManyClients,

    #[error("Too many channels")]
    TooManyChannels,

    #[error("Client already exists: {0}")]
    ClientAlreadyExists(String),

    #[error("Client not found: {0}")]
    ClientNotFound(String),

    #[error("Channel already exists: {0}")]
    ChannelAlreadyExists(String),

    #[error("Channel not found: {0}")]
    ChannelNotFound(String),

    #[error("Invalid commitment number: {0}")]
    InvalidCommitmentNumber(u64),

    #[error("No breach remedy for commitment: {0}")]
    NoBreachRemedy(u64),

    #[error("Encryption error: {0}")]
    EncryptionError(String),

    #[error("Quantum signature error: {0}")]
    QuantumSignatureError(String),

    #[error("Network error: {0}")]
    NetworkError(String),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::transaction::{Transaction, TransactionInput, TransactionOutput};

    #[test]
    fn test_watchtower_client_registration() {
        let mut watchtower = Watchtower::new(WatchtowerConfig::default(), None);

        let client_id = "test_client".to_string();
        let public_key = vec![1, 2, 3, 4];

        let result = watchtower.register_client(client_id.clone(), public_key, false);
        assert!(result.is_ok());

        // Try to register the same client again
        let result = watchtower.register_client(client_id, vec![5, 6, 7, 8], false);
        assert!(matches!(result, Err(WatchError::ClientAlreadyExists(_))));
    }

    #[test]
    fn test_channel_registration() {
        let mut watchtower = Watchtower::new(WatchtowerConfig::default(), None);

        let client_id = "test_client".to_string();
        let public_key = vec![1, 2, 3, 4];

        // Register client first
        watchtower
            .register_client(client_id.clone(), public_key, false)
            .unwrap();

        // Register channel
        let channel_id = [1u8; 32];
        let encrypted_state = EncryptedChannelState {
            encrypted_data: vec![1, 2, 3],
            iv: vec![4, 5, 6],
            tag: vec![7, 8, 9],
        };

        let result = watchtower.register_channel(channel_id, &client_id, encrypted_state);
        assert!(result.is_ok());

        // Verify channel was registered
        let channel_id_obj = ChannelId::from_bytes(channel_id);
        assert!(watchtower.channels.contains_key(&channel_id_obj));
    }

    #[test]
    fn test_watchtower_statistics() {
        let mut watchtower = Watchtower::new(WatchtowerConfig::default(), None);

        // Register a client and channel
        let client_id = "test_client".to_string();
        watchtower
            .register_client(client_id.clone(), vec![1, 2, 3, 4], true)
            .unwrap();

        let channel_id = [1u8; 32];
        let encrypted_state = EncryptedChannelState {
            encrypted_data: vec![1, 2, 3],
            iv: vec![4, 5, 6],
            tag: vec![7, 8, 9],
        };
        watchtower
            .register_channel(channel_id, &client_id, encrypted_state)
            .unwrap();

        let stats = watchtower.get_statistics();
        assert_eq!(stats.total_clients, 1);
        assert_eq!(stats.total_channels, 1);
        assert_eq!(stats.quantum_channels, 1); // Client has quantum enabled
    }
}
