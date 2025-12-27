//! Lightning Network Watchtower
//!
//! This module implements watchtower functionality for monitoring Lightning Network
//! channels and protecting against malicious channel closures.

use crate::crypto::quantum::QuantumScheme;
use crate::lightning::channel::ChannelId;
use crate::types::transaction::{Transaction, TransactionInput, TransactionOutput};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
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
                .unwrap_or(Duration::ZERO)
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
                .unwrap_or(Duration::ZERO)
                .as_secs(),
            quantum_secured: client.quantum_enabled && self.config.quantum_monitoring,
        };

        self.channels.insert(channel_id.clone(), monitor_info);
        client.channel_count += 1;
        client.last_update = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or(Duration::ZERO)
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
            .unwrap_or(Duration::ZERO)
            .as_secs();

        // Update client last update time
        if let Some(client) = self.clients.get_mut(&channel_info.client_id) {
            client.last_update = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or(Duration::ZERO)
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
                .unwrap_or(Duration::ZERO)
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
                .unwrap_or(Duration::ZERO)
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
            .unwrap_or(Duration::ZERO)
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
                .unwrap_or(Duration::ZERO)
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

    #[error("Monitoring error: {0}")]
    MonitoringError(String),

    #[error("Penalty transaction error: {0}")]
    PenaltyTransactionError(String),

    #[error("Lock poisoned")]
    LockPoisoned,
}

/// Blockchain transaction for monitoring
#[derive(Debug, Clone)]
pub struct MonitoredTransaction {
    /// Transaction hash
    pub txid: [u8; 32],
    /// The transaction
    pub transaction: Transaction,
    /// Block height where this was seen
    pub block_height: u64,
    /// Timestamp of observation
    pub timestamp: u64,
}

/// Breach detection result
#[derive(Debug, Clone)]
pub struct BreachDetection {
    /// Channel that was breached
    pub channel_id: ChannelId,
    /// Client ID
    pub client_id: String,
    /// The breaching transaction
    pub breach_tx: Transaction,
    /// Commitment number of the breach
    pub commitment_number: u64,
    /// Justice transaction to broadcast
    pub justice_tx: Transaction,
    /// Detection timestamp
    pub detected_at: u64,
    /// Block height of breach
    pub breach_height: u64,
}

/// Penalty transaction builder
pub struct PenaltyTransactionBuilder {
    /// Revocation private key
    revocation_privkey: Option<Vec<u8>>,
    /// Delayed payment base point
    delayed_payment_basepoint: Option<Vec<u8>>,
    /// Per commitment point
    per_commitment_point: Option<Vec<u8>>,
    /// To-self delay (in blocks)
    to_self_delay: u16,
    /// Fee rate (satoshis per vbyte)
    fee_rate: u64,
}

impl PenaltyTransactionBuilder {
    /// Create a new builder
    pub fn new() -> Self {
        Self {
            revocation_privkey: None,
            delayed_payment_basepoint: None,
            per_commitment_point: None,
            to_self_delay: 144, // Default: ~1 day
            fee_rate: 10,       // Default fee rate
        }
    }

    /// Set revocation private key
    pub fn with_revocation_key(mut self, key: Vec<u8>) -> Self {
        self.revocation_privkey = Some(key);
        self
    }

    /// Set delayed payment basepoint
    pub fn with_delayed_payment_basepoint(mut self, point: Vec<u8>) -> Self {
        self.delayed_payment_basepoint = Some(point);
        self
    }

    /// Set per-commitment point
    pub fn with_per_commitment_point(mut self, point: Vec<u8>) -> Self {
        self.per_commitment_point = Some(point);
        self
    }

    /// Set to-self delay
    pub fn with_to_self_delay(mut self, delay: u16) -> Self {
        self.to_self_delay = delay;
        self
    }

    /// Set fee rate
    pub fn with_fee_rate(mut self, rate: u64) -> Self {
        self.fee_rate = rate;
        self
    }

    /// Build the penalty (justice) transaction
    pub fn build(
        &self,
        breach_tx: &Transaction,
        to_local_output_index: usize,
        sweep_address: Vec<u8>,
    ) -> Result<Transaction, WatchError> {
        // Validate inputs
        if breach_tx.outputs().len() <= to_local_output_index {
            return Err(WatchError::PenaltyTransactionError(
                "Invalid to_local output index".to_string(),
            ));
        }

        let to_local_output = &breach_tx.outputs()[to_local_output_index];
        let breach_txid = breach_tx.hash();

        // Calculate transaction size for fee estimation
        // P2WPKH input: ~68 vbytes, P2WPKH output: ~31 vbytes
        let estimated_size = 68 + 31;
        let fee = self.fee_rate * estimated_size;

        let output_value = to_local_output.amount().saturating_sub(fee);
        if output_value == 0 {
            return Err(WatchError::PenaltyTransactionError(
                "Output value too low after fees".to_string(),
            ));
        }

        // Create the spending input
        let input = TransactionInput::new(
            breach_txid,
            to_local_output_index as u32,
            Vec::new(), // Script sig (will be empty for witness)
            0xFFFFFFFF, // Sequence
        );

        // Create the sweep output
        let output = TransactionOutput::new(output_value, sweep_address);

        // Build the transaction
        let penalty_tx = Transaction::new(
            2,            // Version 2 for RBF
            vec![input],
            vec![output],
            0, // Locktime
        );

        info!(
            "Built penalty transaction: input={}, output_value={}",
            hex::encode(&breach_txid[..8]),
            output_value
        );

        Ok(penalty_tx)
    }
}

impl Default for PenaltyTransactionBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Blockchain monitor for watchtower
pub struct BlockchainMonitor {
    /// Channels being monitored (channel funding outpoint -> channel ID)
    watched_outpoints: Arc<RwLock<HashMap<([u8; 32], u32), ChannelId>>>,
    /// Recent transactions seen
    recent_transactions: Arc<RwLock<Vec<MonitoredTransaction>>>,
    /// Detected breaches
    detected_breaches: Arc<RwLock<Vec<BreachDetection>>>,
    /// Current block height
    current_height: Arc<RwLock<u64>>,
    /// Is monitoring active
    is_running: Arc<RwLock<bool>>,
}

impl BlockchainMonitor {
    /// Create a new blockchain monitor
    pub fn new() -> Self {
        Self {
            watched_outpoints: Arc::new(RwLock::new(HashMap::new())),
            recent_transactions: Arc::new(RwLock::new(Vec::new())),
            detected_breaches: Arc::new(RwLock::new(Vec::new())),
            current_height: Arc::new(RwLock::new(0)),
            is_running: Arc::new(RwLock::new(false)),
        }
    }

    /// Add an outpoint to watch for breaches
    pub fn watch_outpoint(
        &self,
        txid: [u8; 32],
        vout: u32,
        channel_id: ChannelId,
    ) -> Result<(), WatchError> {
        let mut outpoints = self.watched_outpoints.write().map_err(|_| WatchError::LockPoisoned)?;
        let channel_id_str = channel_id.to_string();
        outpoints.insert((txid, vout), channel_id);
        debug!(
            "Now watching outpoint {}:{} for channel {}",
            hex::encode(&txid[..8]),
            vout,
            channel_id_str
        );
        Ok(())
    }

    /// Remove an outpoint from watch list
    pub fn unwatch_outpoint(&self, txid: [u8; 32], vout: u32) -> Result<(), WatchError> {
        let mut outpoints = self.watched_outpoints.write().map_err(|_| WatchError::LockPoisoned)?;
        outpoints.remove(&(txid, vout));
        Ok(())
    }

    /// Process a new block
    pub fn process_block(
        &self,
        height: u64,
        transactions: Vec<Transaction>,
        watchtower: &Watchtower,
    ) -> Result<Vec<BreachDetection>, WatchError> {
        // Update current height
        {
            let mut current = self.current_height.write().map_err(|_| WatchError::LockPoisoned)?;
            *current = height;
        }

        let mut breaches = Vec::new();
        let outpoints = self.watched_outpoints.read().map_err(|_| WatchError::LockPoisoned)?;

        for tx in transactions {
            // Check if any inputs spend a watched outpoint
            for input in tx.inputs() {
                let outpoint = (input.prev_tx_hash(), input.prev_output_index());

                if let Some(channel_id) = outpoints.get(&outpoint) {
                    // This transaction spends a watched outpoint!
                    // Check if it's a breach
                    if let Ok(Some(justice_tx)) = watchtower.check_for_breaches(channel_id, &tx) {
                        let breach = BreachDetection {
                            channel_id: channel_id.clone(),
                            client_id: watchtower.channels.get(channel_id)
                                .map(|c| c.client_id.clone())
                                .unwrap_or_default(),
                            breach_tx: tx.clone(),
                            commitment_number: tx.lock_time() as u64,
                            justice_tx,
                            detected_at: SystemTime::now()
                                .duration_since(UNIX_EPOCH)
                                .unwrap_or(Duration::ZERO)
                                .as_secs(),
                            breach_height: height,
                        };

                        warn!(
                            "BREACH DETECTED on channel {} at height {}!",
                            channel_id, height
                        );

                        breaches.push(breach.clone());

                        // Store the breach
                        let mut detected = self.detected_breaches.write()
                            .map_err(|_| WatchError::LockPoisoned)?;
                        detected.push(breach);
                    }
                }
            }

            // Store transaction for recent history
            let monitored = MonitoredTransaction {
                txid: tx.hash(),
                transaction: tx,
                block_height: height,
                timestamp: SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or(Duration::ZERO)
                    .as_secs(),
            };

            let mut recent = self.recent_transactions.write().map_err(|_| WatchError::LockPoisoned)?;
            recent.push(monitored);

            // Keep only recent transactions (last 1000)
            if recent.len() > 1000 {
                recent.drain(0..100);
            }
        }

        Ok(breaches)
    }

    /// Get current block height
    pub fn get_height(&self) -> Result<u64, WatchError> {
        let height = self.current_height.read().map_err(|_| WatchError::LockPoisoned)?;
        Ok(*height)
    }

    /// Get detected breaches
    pub fn get_breaches(&self) -> Result<Vec<BreachDetection>, WatchError> {
        let breaches = self.detected_breaches.read().map_err(|_| WatchError::LockPoisoned)?;
        Ok(breaches.clone())
    }

    /// Start monitoring
    pub fn start(&self) -> Result<(), WatchError> {
        let mut running = self.is_running.write().map_err(|_| WatchError::LockPoisoned)?;
        *running = true;
        info!("Blockchain monitor started");
        Ok(())
    }

    /// Stop monitoring
    pub fn stop(&self) -> Result<(), WatchError> {
        let mut running = self.is_running.write().map_err(|_| WatchError::LockPoisoned)?;
        *running = false;
        info!("Blockchain monitor stopped");
        Ok(())
    }

    /// Check if monitor is running
    pub fn is_running(&self) -> Result<bool, WatchError> {
        let running = self.is_running.read().map_err(|_| WatchError::LockPoisoned)?;
        Ok(*running)
    }

    /// Get statistics
    pub fn get_stats(&self) -> Result<MonitorStats, WatchError> {
        let outpoints = self.watched_outpoints.read().map_err(|_| WatchError::LockPoisoned)?;
        let breaches = self.detected_breaches.read().map_err(|_| WatchError::LockPoisoned)?;
        let height = self.current_height.read().map_err(|_| WatchError::LockPoisoned)?;

        Ok(MonitorStats {
            watched_outpoints: outpoints.len(),
            detected_breaches: breaches.len(),
            current_height: *height,
        })
    }
}

impl Default for BlockchainMonitor {
    fn default() -> Self {
        Self::new()
    }
}

/// Monitor statistics
#[derive(Debug, Clone)]
pub struct MonitorStats {
    pub watched_outpoints: usize,
    pub detected_breaches: usize,
    pub current_height: u64,
}

/// Privacy-preserving blob storage for watchtower
#[derive(Debug, Clone)]
pub struct EncryptedBlobStorage {
    /// Storage of encrypted blobs (blob_id -> encrypted data)
    blobs: HashMap<[u8; 32], EncryptedBlob>,
    /// Maximum storage per client
    max_storage_per_client: usize,
    /// Client storage usage
    client_usage: HashMap<String, usize>,
}

/// Encrypted blob for privacy-preserving storage
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptedBlob {
    /// Hint for locating the breach (hash of commitment tx)
    pub hint: [u8; 32],
    /// Encrypted remedy data (only client can decrypt)
    pub encrypted_remedy: Vec<u8>,
    /// Client ID
    pub client_id: String,
    /// Creation timestamp
    pub created_at: u64,
}

impl EncryptedBlobStorage {
    /// Create new blob storage
    pub fn new(max_storage_per_client: usize) -> Self {
        Self {
            blobs: HashMap::new(),
            max_storage_per_client,
            client_usage: HashMap::new(),
        }
    }

    /// Store an encrypted blob
    pub fn store_blob(
        &mut self,
        client_id: &str,
        hint: [u8; 32],
        encrypted_remedy: Vec<u8>,
    ) -> Result<[u8; 32], WatchError> {
        // Check storage limit
        let current_usage = self.client_usage.get(client_id).copied().unwrap_or(0);
        if current_usage + encrypted_remedy.len() > self.max_storage_per_client {
            return Err(WatchError::MonitoringError("Storage limit exceeded".to_string()));
        }

        // Generate blob ID from hint
        let blob_id = hint; // Using hint as ID for simplicity

        let blob = EncryptedBlob {
            hint,
            encrypted_remedy: encrypted_remedy.clone(),
            client_id: client_id.to_string(),
            created_at: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or(Duration::ZERO)
                .as_secs(),
        };

        self.blobs.insert(blob_id, blob);
        *self.client_usage.entry(client_id.to_string()).or_insert(0) += encrypted_remedy.len();

        debug!("Stored encrypted blob {} for client {}", hex::encode(&blob_id[..8]), client_id);

        Ok(blob_id)
    }

    /// Retrieve blob by hint
    pub fn get_blob_by_hint(&self, hint: &[u8; 32]) -> Option<&EncryptedBlob> {
        self.blobs.get(hint)
    }

    /// Delete blob
    pub fn delete_blob(&mut self, blob_id: &[u8; 32]) -> Option<EncryptedBlob> {
        if let Some(blob) = self.blobs.remove(blob_id) {
            // Update client usage
            if let Some(usage) = self.client_usage.get_mut(&blob.client_id) {
                *usage = usage.saturating_sub(blob.encrypted_remedy.len());
            }
            Some(blob)
        } else {
            None
        }
    }

    /// Get storage usage for client
    pub fn get_client_usage(&self, client_id: &str) -> usize {
        self.client_usage.get(client_id).copied().unwrap_or(0)
    }

    /// Clean up old blobs
    pub fn cleanup_old_blobs(&mut self, max_age_secs: u64) {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or(Duration::ZERO)
            .as_secs();

        let cutoff = now.saturating_sub(max_age_secs);

        let old_blobs: Vec<[u8; 32]> = self.blobs
            .iter()
            .filter(|(_, blob)| blob.created_at < cutoff)
            .map(|(id, _)| *id)
            .collect();

        for blob_id in old_blobs {
            self.delete_blob(&blob_id);
        }
    }
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
