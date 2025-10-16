use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex, RwLock};
use std::time::{SystemTime, UNIX_EPOCH};
use thiserror::Error;
use tokio::sync::mpsc;
use tracing::{error, info};

use super::{LightningConfig, LightningNetworkError};
use crate::lightning::payment::RouteHop;
use crate::lightning::{
    AtomicChannel, Channel, ChannelConfig, ChannelId, ChannelState, Invoice, LightningWallet,
    OnionRouter, Payment, PaymentHash, PaymentStatus, QuantumChannelSecurity, Router, Watchtower,
};
use crate::types::transaction::Transaction;

/// Lightning Network Manager - Central coordinator for Lightning Network operations
pub struct LightningManager {
    /// Lightning Network configuration
    config: LightningConfig,

    /// Active payment channels (using AtomicChannel for thread safety)
    channels: Arc<RwLock<HashMap<ChannelId, Arc<AtomicChannel>>>>,

    /// Pending channels (opening/closing)
    pending_channels: Arc<RwLock<HashMap<ChannelId, Arc<AtomicChannel>>>>,

    /// Lightning wallet for key management
    wallet: Arc<Mutex<LightningWallet>>,

    /// Payment router for finding paths
    router: Arc<Router>,

    /// Onion router for payment privacy
    onion_router: Arc<OnionRouter>,

    /// Watchtower for security monitoring
    watchtower: Option<Arc<Watchtower>>,

    /// Active invoices
    invoices: Arc<RwLock<HashMap<PaymentHash, Invoice>>>,

    /// Payment history
    payments: Arc<RwLock<HashMap<PaymentHash, Payment>>>,

    /// Network peers
    peers: Arc<RwLock<HashMap<String, PeerInfo>>>,

    /// Quantum security manager
    quantum_security: Option<Arc<QuantumChannelSecurity>>,

    /// Event sender for notifications
    event_sender: mpsc::UnboundedSender<LightningEvent>,

    /// Running state
    is_running: Arc<std::sync::atomic::AtomicBool>,

    /// Payment index counter
    payment_index: Arc<std::sync::atomic::AtomicU64>,

    /// Invoice index counter
    invoice_index: Arc<std::sync::atomic::AtomicU64>,
}

#[derive(Debug, Clone)]
pub struct PeerInfo {
    pub node_id: String,
    pub address: String,
    pub connected: bool,
    pub last_seen: SystemTime,
    pub features: Vec<String>,
}

#[derive(Debug, Clone)]
pub enum LightningEvent {
    ChannelOpened(ChannelId),
    ChannelClosed(ChannelId),
    PaymentReceived(PaymentHash, u64),
    PaymentSent(PaymentHash, u64),
    InvoiceCreated(PaymentHash),
    PeerConnected(String),
    PeerDisconnected(String),
}

#[derive(Error, Debug)]
pub enum ManagerError {
    #[error("Channel not found: {0}")]
    ChannelNotFound(String),
    #[error("Channel error: {0}")]
    ChannelError(String),
    #[error("Insufficient balance: required {required}, available {available}")]
    InsufficientBalance { required: u64, available: u64 },
    #[error("Invalid payment request: {0}")]
    InvalidPaymentRequest(String),
    #[error("Payment failed: {0}")]
    PaymentFailed(String),
    #[error("Payment not found: {0}")]
    PaymentNotFound(String),
    #[error("Network error: {0}")]
    NetworkError(String),
    #[error("Quantum security error: {0}")]
    QuantumSecurityError(String),
    #[error("Configuration error: {0}")]
    ConfigError(String),
    #[error("Wallet error: {0}")]
    WalletError(String),
    #[error("Router error: {0}")]
    RouterError(String),
    #[error("Watchtower error: {0}")]
    WatchtowerError(String),
}

// Response types for API compatibility
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LightningInfo {
    pub node_id: String,
    pub num_channels: usize,
    pub num_pending_channels: usize,
    pub num_inactive_channels: usize,
    pub total_balance_msat: u64,
    pub total_outbound_capacity_msat: u64,
    pub total_inbound_capacity_msat: u64,
    pub num_peers: usize,
    pub synced_to_chain: bool,
    pub synced_to_graph: bool,
    pub block_height: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LightningChannel {
    pub channel_id: String,
    pub remote_pubkey: String,
    pub capacity: u64,
    pub local_balance: u64,
    pub remote_balance: u64,
    pub commit_fee: u64,
    pub commit_weight: u64,
    pub fee_per_kw: u64,
    pub unsettled_balance: u64,
    pub total_satoshis_sent: u64,
    pub total_satoshis_received: u64,
    pub num_updates: u64,
    pub pending_htlcs: Vec<PendingHTLC>,
    pub csv_delay: u32,
    pub private: bool,
    pub initiator: bool,
    pub chan_status_flags: String,
    pub local_chan_reserve_sat: u64,
    pub remote_chan_reserve_sat: u64,
    pub static_remote_key: bool,
    pub commitment_type: String,
    pub lifetime: u64,
    pub uptime: u64,
    pub close_address: String,
    pub push_amount_sat: u64,
    pub thaw_height: u32,
    pub local_constraints: ChannelConstraints,
    pub remote_constraints: ChannelConstraints,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingHTLC {
    pub incoming: bool,
    pub amount: u64,
    pub outpoint: String,
    pub maturity_height: u32,
    pub blocks_til_maturity: i32,
    pub stage: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelConstraints {
    pub csv_delay: u32,
    pub chan_reserve_sat: u64,
    pub dust_limit_sat: u64,
    pub max_pending_amt_msat: u64,
    pub min_htlc_msat: u64,
    pub max_accepted_htlcs: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LightningPayment {
    pub payment_hash: String,
    pub value: u64,
    pub creation_date: u64,
    pub fee: u64,
    pub payment_preimage: String,
    pub value_sat: u64,
    pub value_msat: u64,
    pub payment_request: String,
    pub status: String,
    pub fee_sat: u64,
    pub fee_msat: u64,
    pub creation_time_ns: u64,
    pub htlcs: Vec<HTLCAttempt>,
    pub payment_index: u64,
    pub failure_reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HTLCAttempt {
    pub attempt_id: u64,
    pub status: String,
    pub route: Route,
    pub attempt_time_ns: u64,
    pub resolve_time_ns: u64,
    pub failure: Option<Failure>,
    pub preimage: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Route {
    pub total_time_lock: u32,
    pub total_fees: u64,
    pub total_amt: u64,
    pub hops: Vec<Hop>,
    pub total_fees_msat: u64,
    pub total_amt_msat: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Hop {
    pub chan_id: String,
    pub chan_capacity: u64,
    pub amt_to_forward: u64,
    pub fee: u64,
    pub expiry: u32,
    pub amt_to_forward_msat: u64,
    pub fee_msat: u64,
    pub pub_key: String,
    pub tlv_payload: bool,
    pub mpp_record: Option<MPPRecord>,
    pub amp_record: Option<AMPRecord>,
    pub custom_records: HashMap<u64, Vec<u8>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MPPRecord {
    pub payment_addr: Vec<u8>,
    pub total_amt_msat: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AMPRecord {
    pub root_share: Vec<u8>,
    pub set_id: Vec<u8>,
    pub child_index: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Failure {
    pub code: String,
    pub channel_update: Option<ChannelUpdate>,
    pub htlc_msat: u64,
    pub onion_sha_256: Vec<u8>,
    pub cltv_expiry: u32,
    pub flags: u32,
    pub failure_source_index: u32,
    pub height: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelUpdate {
    pub signature: Vec<u8>,
    pub chain_hash: Vec<u8>,
    pub chan_id: u64,
    pub timestamp: u32,
    pub message_flags: u32,
    pub channel_flags: u32,
    pub time_lock_delta: u32,
    pub htlc_minimum_msat: u64,
    pub base_fee: u32,
    pub fee_rate: u32,
    pub htlc_maximum_msat: u64,
    pub extra_opaque_data: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LightningInvoice {
    pub memo: String,
    pub r_preimage: Vec<u8>,
    pub r_hash: Vec<u8>,
    pub value: u64,
    pub value_msat: u64,
    pub settled: bool,
    pub creation_date: u64,
    pub settle_date: u64,
    pub payment_request: String,
    pub description_hash: Vec<u8>,
    pub expiry: u64,
    pub fallback_addr: String,
    pub cltv_expiry: u64,
    pub route_hints: Vec<RouteHint>,
    pub private: bool,
    pub add_index: u64,
    pub settle_index: u64,
    pub amt_paid: u64,
    pub amt_paid_sat: u64,
    pub amt_paid_msat: u64,
    pub state: String,
    pub htlcs: Vec<InvoiceHTLC>,
    pub features: HashMap<u32, Feature>,
    pub is_keysend: bool,
    pub payment_addr: Vec<u8>,
    pub is_amp: bool,
    pub amp_invoice_state: HashMap<String, AMPInvoiceState>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteHint {
    pub hop_hints: Vec<HopHint>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HopHint {
    pub node_id: String,
    pub chan_id: u64,
    pub fee_base_msat: u32,
    pub fee_proportional_millionths: u32,
    pub cltv_expiry_delta: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InvoiceHTLC {
    pub chan_id: u64,
    pub htlc_index: u64,
    pub amt_msat: u64,
    pub accept_height: i32,
    pub accept_time: u64,
    pub resolve_time: u64,
    pub expiry_height: i32,
    pub state: String,
    pub custom_records: HashMap<u64, Vec<u8>>,
    pub mpp_total_amt_msat: u64,
    pub amp: Option<AMP>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AMP {
    pub root_share: Vec<u8>,
    pub set_id: Vec<u8>,
    pub child_index: u32,
    pub hash: Vec<u8>,
    pub preimage: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Feature {
    pub name: String,
    pub is_required: bool,
    pub is_known: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AMPInvoiceState {
    pub state: String,
    pub settle_index: u64,
    pub settle_time: u64,
    pub amt_paid_msat: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeInfo {
    pub node: LightningNode,
    pub num_channels: u32,
    pub total_capacity: u64,
    pub channels: Vec<ChannelEdge>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LightningNode {
    pub last_update: u32,
    pub pub_key: String,
    pub alias: String,
    pub addresses: Vec<NodeAddress>,
    pub color: String,
    pub features: HashMap<u32, Feature>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeAddress {
    pub network: String,
    pub addr: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelEdge {
    pub channel_id: u64,
    pub chan_point: String,
    pub last_update: u32,
    pub node1_pub: String,
    pub node2_pub: String,
    pub capacity: u64,
    pub node1_policy: Option<RoutingPolicy>,
    pub node2_policy: Option<RoutingPolicy>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutingPolicy {
    pub time_lock_delta: u32,
    pub min_htlc: u64,
    pub fee_base_msat: u64,
    pub fee_rate_milli_msat: u64,
    pub disabled: bool,
    pub max_htlc_msat: u64,
    pub last_update: u32,
}

impl LightningManager {
    /// Create a new Lightning Network manager
    pub fn new(
        config: LightningConfig,
        wallet: LightningWallet,
    ) -> Result<(Self, mpsc::UnboundedReceiver<LightningEvent>), ManagerError> {
        let (event_sender, event_receiver) = mpsc::unbounded_channel();

        // Initialize router
        let router = Arc::new(Router::new());

        // Initialize onion router with a default private key
        let private_key = [1u8; 32]; // In production, this would be derived from the wallet
        let quantum_scheme = if config.use_quantum_signatures {
            config.quantum_scheme
        } else {
            None
        };
        let onion_router = Arc::new(OnionRouter::new(private_key, quantum_scheme));

        // Initialize watchtower if enabled
        let watchtower = if config.use_quantum_signatures {
            // Using quantum flag as watchtower flag
            let watchtower_config = crate::lightning::watchtower::WatchtowerConfig::default();
            Some(Arc::new(Watchtower::new(watchtower_config, quantum_scheme)))
        } else {
            None
        };

        // Initialize quantum security if enabled
        let quantum_security = if config.use_quantum_signatures {
            let quantum_config =
                crate::lightning::quantum_security::QuantumChannelConfig::default();
            Some(Arc::new(QuantumChannelSecurity::new(quantum_config)))
        } else {
            None
        };

        let manager = Self {
            config,
            channels: Arc::new(RwLock::new(HashMap::new())),
            pending_channels: Arc::new(RwLock::new(HashMap::new())),
            wallet: Arc::new(Mutex::new(wallet)),
            router,
            onion_router,
            watchtower,
            invoices: Arc::new(RwLock::new(HashMap::new())),
            payments: Arc::new(RwLock::new(HashMap::new())),
            peers: Arc::new(RwLock::new(HashMap::new())),
            quantum_security,
            event_sender,
            is_running: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            payment_index: Arc::new(std::sync::atomic::AtomicU64::new(0)),
            invoice_index: Arc::new(std::sync::atomic::AtomicU64::new(0)),
        };

        Ok((manager, event_receiver))
    }

    /// Start the Lightning Network manager
    pub async fn start(&self) -> Result<(), ManagerError> {
        info!("Starting Lightning Network manager");

        self.is_running
            .store(true, std::sync::atomic::Ordering::Relaxed);

        // Start watchtower if enabled
        if let Some(watchtower) = &self.watchtower {
            watchtower
                .start()
                .await
                .map_err(|e| ManagerError::WatchtowerError(e.to_string()))?;
        }

        // Start router
        self.router
            .start()
            .await
            .map_err(|e| ManagerError::RouterError(e.to_string()))?;

        info!("Lightning Network manager started successfully");
        Ok(())
    }

    /// Stop the Lightning Network manager
    pub async fn stop(&self) -> Result<(), ManagerError> {
        info!("Stopping Lightning Network manager");

        self.is_running
            .store(false, std::sync::atomic::Ordering::Relaxed);

        // Stop watchtower
        if let Some(watchtower) = &self.watchtower {
            watchtower
                .stop()
                .await
                .map_err(|e| ManagerError::WatchtowerError(e.to_string()))?;
        }

        // Stop router
        self.router
            .stop()
            .await
            .map_err(|e| ManagerError::RouterError(e.to_string()))?;

        info!("Lightning Network manager stopped");
        Ok(())
    }

    /// Get Lightning Network information
    pub fn get_info(&self) -> Result<LightningInfo, ManagerError> {
        let channels = self.channels.read().unwrap();
        let pending_channels = self.pending_channels.read().unwrap();
        let peers = self.peers.read().unwrap();

        let active_channels: Vec<_> = channels
            .values()
            .filter(|c| {
                // Use get_channel_info() to access the state
                match c.get_channel_info() {
                    Ok(info) => info.state == ChannelState::Active,
                    Err(_) => false, // If we can't get info, consider it inactive
                }
            })
            .collect();

        let total_balance_msat = active_channels
            .iter()
            .filter_map(|c| {
                c.get_channel_info()
                    .map(|info| info.local_balance_novas * 1000) // Convert to millinovas
                    .ok()
            })
            .sum();

        let total_outbound_capacity_msat = active_channels
            .iter()
            .filter_map(|c| {
                c.get_channel_info()
                    .map(|info| info.local_balance_novas * 1000)
                    .ok()
            })
            .sum();

        let total_inbound_capacity_msat = active_channels
            .iter()
            .filter_map(|c| {
                c.get_channel_info()
                    .map(|info| info.remote_balance_novas * 1000)
                    .ok()
            })
            .sum();

        // Check chain sync status by comparing our height with network
        let current_height = self.get_current_height();
        let synced_to_chain = current_height > 0; // Simple check

        // Check graph sync status by verifying we have network topology
        let synced_to_graph = self.router.node_count() > 0;

        Ok(LightningInfo {
            node_id: self.get_node_id(),
            num_channels: active_channels.len(),
            num_pending_channels: pending_channels.len(),
            num_inactive_channels: channels
                .values()
                .filter(|c| {
                    match c.get_channel_info() {
                        Ok(info) => info.state != ChannelState::Active,
                        Err(_) => true, // If we can't get info, consider it inactive
                    }
                })
                .count(),
            total_balance_msat,
            total_outbound_capacity_msat,
            total_inbound_capacity_msat,
            num_peers: peers.len(),
            synced_to_chain,
            synced_to_graph,
            block_height: current_height,
        })
    }

    /// Get all channels
    pub fn get_channels(
        &self,
        include_inactive: bool,
        include_pending: bool,
    ) -> Result<Vec<LightningChannel>, ManagerError> {
        let mut result = Vec::new();

        // Add active/inactive channels
        let channels = self.channels.read().unwrap();
        for channel in channels.values() {
            if let Ok(info) = channel.get_channel_info() {
                if include_inactive || info.state == ChannelState::Active {
                    result.push(self.channel_to_lightning_channel(channel));
                }
            }
        }

        // Add pending channels
        if include_pending {
            let pending_channels = self.pending_channels.read().unwrap();
            for channel in pending_channels.values() {
                result.push(self.channel_to_lightning_channel(channel));
            }
        }

        Ok(result)
    }

    /// Get specific channel
    pub fn get_channel(&self, channel_id: &str) -> Result<Option<LightningChannel>, ManagerError> {
        let channel_id = ChannelId::from_hex(channel_id)
            .map_err(|_| ManagerError::InvalidPaymentRequest("Invalid channel ID".to_string()))?;

        let channels = self.channels.read().unwrap();
        if let Some(channel) = channels.get(&channel_id) {
            return Ok(Some(self.channel_to_lightning_channel(channel)));
        }

        let pending_channels = self.pending_channels.read().unwrap();
        if let Some(channel) = pending_channels.get(&channel_id) {
            return Ok(Some(self.channel_to_lightning_channel(channel)));
        }

        Ok(None)
    }

    /// Open a new payment channel
    pub async fn open_channel(
        &self,
        node_id: &str,
        local_funding_amount: u64,
        push_amount: u64,
        private: bool,
        min_htlc_msat: Option<u64>,
    ) -> Result<OpenChannelResponse, ManagerError> {
        info!(
            "Opening channel to {} with funding {}",
            node_id, local_funding_amount
        );

        // Validate parameters
        if local_funding_amount < 20000 {
            return Err(ManagerError::ConfigError(
                "Minimum channel size is 20,000 satoshis".to_string(),
            ));
        }

        if push_amount > local_funding_amount {
            return Err(ManagerError::ConfigError(
                "Push amount cannot exceed funding amount".to_string(),
            ));
        }

        // Check wallet balance
        let wallet = self.wallet.lock().unwrap();
        let available_balance = wallet.get_balance();
        if available_balance < local_funding_amount {
            return Err(ManagerError::InsufficientBalance {
                required: local_funding_amount,
                available: available_balance,
            });
        }
        drop(wallet);

        // Generate channel ID
        let channel_id = ChannelId::new_random();

        // Create funding transaction
        let funding_tx = self
            .create_funding_transaction(local_funding_amount, &channel_id)
            .await?;

        // Create channel configuration
        let mut config = ChannelConfig::default();
        config.announce_channel = !private;
        if let Some(min_htlc) = min_htlc_msat {
            config.min_htlc_value_msat = min_htlc;
        }

        // Create channel
        let channel = Channel::open(
            node_id.to_string(),
            local_funding_amount,
            push_amount,
            config,
            self.config.quantum_scheme,
        )
        .map_err(|e| ManagerError::ChannelError(e.to_string()))?;

        // Wrap in AtomicChannel for thread safety
        let atomic_channel = Arc::new(AtomicChannel::new(channel));

        // Add to pending channels
        {
            let mut pending_channels = self.pending_channels.write().unwrap();
            pending_channels.insert(channel_id.clone(), atomic_channel);
        }

        // Send event
        let _ = self
            .event_sender
            .send(LightningEvent::ChannelOpened(channel_id.clone()));

        Ok(OpenChannelResponse {
            channel_id: channel_id.to_hex(),
            funding_txid: hex::encode(funding_tx.hash()),
            output_index: 0,
        })
    }

    /// Close a payment channel
    pub async fn close_channel(&self, channel_id: &str, force: bool) -> Result<bool, ManagerError> {
        let channel_id = ChannelId::from_hex(channel_id)
            .map_err(|_| ManagerError::InvalidPaymentRequest("Invalid channel ID".to_string()))?;

        info!("Closing channel {} (force: {})", channel_id.to_hex(), force);

        // Find channel
        let atomic_channel = {
            let mut channels = self.channels.write().unwrap();
            channels.remove(&channel_id)
        };

        if let Some(atomic_channel) = atomic_channel {
            // Get channel info for closing
            let channel_info = atomic_channel.get_channel_info().map_err(|e| {
                ManagerError::ChannelError(format!("Failed to get channel info: {}", e))
            })?;

            // Check if channel can be closed
            if channel_info.state != ChannelState::Active
                && channel_info.state != ChannelState::ClosingNegotiation
            {
                return Err(ManagerError::ChannelError(format!(
                    "Channel in invalid state for closing: {:?}",
                    channel_info.state
                )));
            }

            // Create closing transaction using the underlying channel
            // This is a simplified approach - in production, we'd handle this through atomic operations
            let closing_tx = {
                let mut channel = atomic_channel.channel.lock().map_err(|e| {
                    ManagerError::ChannelError(format!("Failed to lock channel: {}", e))
                })?;

                if force {
                    channel
                        .force_close()
                        .map_err(|e| ManagerError::ChannelError(e.to_string()))?
                } else {
                    channel
                        .cooperative_close()
                        .map_err(|e| ManagerError::ChannelError(e.to_string()))?
                }
            };

            // Broadcast closing transaction
            self.broadcast_transaction(&closing_tx).await?;

            // Send event
            let _ = self
                .event_sender
                .send(LightningEvent::ChannelClosed(channel_id));

            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Get payment history
    pub fn get_payments(
        &self,
        index_offset: u64,
        max_payments: u64,
        include_pending: bool,
    ) -> Result<Vec<LightningPayment>, ManagerError> {
        let payments = self.payments.read().unwrap();

        let mut result: Vec<_> = payments
            .values()
            .filter(|p| include_pending || p.status != PaymentStatus::Pending)
            .skip(index_offset as usize)
            .take(max_payments as usize)
            .map(|p| self.payment_to_lightning_payment(p))
            .collect();

        // Sort by creation time (newest first)
        result.sort_by(|a, b| b.creation_date.cmp(&a.creation_date));

        Ok(result)
    }

    /// Send a payment
    pub async fn send_payment(
        &self,
        payment_request: &str,
        amount_msat: Option<u64>,
        _timeout_seconds: u32,
        fee_limit_msat: Option<u64>,
    ) -> Result<PaymentResponse, ManagerError> {
        info!("Sending payment: {}", payment_request);

        // Parse payment request (simplified - in production would parse BOLT11)
        let invoice = self.parse_payment_request(payment_request)?;

        // Use provided amount or invoice amount
        let amount = amount_msat.unwrap_or(invoice.amount_msat);

        // Find route
        let route = self
            .router
            .find_route(
                &invoice.destination,
                amount,
                &[], // Route hints
            )
            .map_err(|e| ManagerError::RouterError(e.to_string()))?;

        if route.is_empty() {
            return Err(ManagerError::PaymentFailed("No route found".to_string()));
        }

        // Check fee limit
        if let Some(max_fee) = fee_limit_msat {
            if route.total_fee_msat > max_fee {
                return Err(ManagerError::PaymentFailed(format!(
                    "Route fee {} exceeds limit {}",
                    route.total_fee_msat, max_fee
                )));
            }
        }

        // Create payment
        let payment_hash = invoice.payment_hash;
        let payment_index = self
            .payment_index
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);

        let payment = Payment {
            payment_hash,
            payment_preimage: None,
            amount_msat: amount,
            status: PaymentStatus::Pending,
            created_at: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            completed_at: None,
            fee_msat: route.total_fee_msat,
            route: Some(
                route
                    .hops
                    .iter()
                    .map(|h| RouteHop {
                        channel_id: h.channel_id.to_hex().parse().unwrap_or(0),
                        node_id: h.node_id.to_string(),
                        amount_msat: h.amount_msat,
                        fee_msat: h.channel_fee(h.amount_msat),
                        cltv_expiry_delta: h.cltv_expiry_delta,
                    })
                    .collect(),
            ),
            failure_reason: None,
            carbon_footprint_grams: None,
        };

        // Store payment with original request
        {
            let mut payments = self.payments.write().unwrap();
            payments.insert(payment_hash, payment);
        }

        // Send payment through route
        let preimage = self.send_payment_through_route(&route, &invoice).await?;

        // Update payment status
        {
            let mut payments = self.payments.write().unwrap();
            if let Some(payment) = payments.get_mut(&payment_hash) {
                payment.status = PaymentStatus::Succeeded;
                payment.payment_preimage = Some(preimage);
                payment.completed_at = Some(
                    SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap()
                        .as_secs(),
                );
            }
        }

        // Send event
        let _ = self
            .event_sender
            .send(LightningEvent::PaymentSent(payment_hash, amount));

        Ok(PaymentResponse {
            payment_hash: payment_hash.to_hex(),
            payment_preimage: Some(preimage.to_hex()),
            payment_route: route.hops.iter().map(|h| h.node_id.to_string()).collect(),
            payment_error: None,
            payment_index,
            status: "SUCCEEDED".to_string(),
            fee_msat: route.total_fee_msat,
            value_msat: amount,
            creation_time_ns: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos() as u64,
        })
    }

    /// Get invoices
    pub fn get_invoices(
        &self,
        pending_only: bool,
        index_offset: u64,
        num_max_invoices: u64,
    ) -> Result<Vec<LightningInvoice>, ManagerError> {
        let invoices = self.invoices.read().unwrap();

        let result: Vec<_> = invoices
            .values()
            .filter(|i| !pending_only || !i.is_settled())
            .skip(index_offset as usize)
            .take(num_max_invoices as usize)
            .map(|i| self.invoice_to_lightning_invoice(i))
            .collect();

        Ok(result)
    }

    /// Create an invoice
    pub fn create_invoice(
        &self,
        value_msat: u64,
        memo: &str,
        expiry: u32,
        _private: bool,
    ) -> Result<InvoiceResponse, ManagerError> {
        info!("Creating invoice for {} millinovas", value_msat);

        // Generate payment hash and preimage using payment module types
        let preimage = crate::lightning::payment::PaymentPreimage::new_random();
        let payment_hash = preimage.payment_hash();
        let _invoice_index = self
            .invoice_index
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);

        // Create invoice using payment module types directly
        let invoice = Invoice::new_with_preimage(preimage, value_msat, memo.to_string(), expiry)
            .map_err(|e| ManagerError::InvalidPaymentRequest(e.to_string()))?;

        // Store invoice using payment module types
        {
            let mut invoices = self.invoices.write().unwrap();
            invoices.insert(payment_hash, invoice.clone());
        }

        // Convert HTLCs from invoice (if any pending)
        let _htlcs = {
            let channels = self.channels.read().unwrap();
            let mut invoice_htlcs = vec![];

            for atomic_channel in channels.values() {
                // Get channel info safely
                if let Ok(_channel_info) = atomic_channel.get_channel_info() {
                    // Access the underlying channel for HTLCs
                    if let Ok(channel) = atomic_channel.channel.lock() {
                        for htlc in &channel.pending_htlcs {
                            if htlc.payment_hash == *invoice.payment_hash().as_bytes() {
                                invoice_htlcs.push(crate::lightning::payment::Htlc {
                                    id: htlc.id,
                                    payment_hash: htlc.payment_hash,
                                    amount_sat: htlc.amount_novas,
                                    cltv_expiry: htlc.expiry_height,
                                    offered: htlc.is_outgoing,
                                    state: crate::lightning::payment::HtlcState::Pending,
                                    quantum_signature: None,
                                });
                            }
                        }
                    }
                }
            }

            invoice_htlcs
        };

        // Get invoice index
        let add_index = self.invoice_index.load(std::sync::atomic::Ordering::SeqCst);

        Ok(InvoiceResponse {
            payment_request: self.encode_payment_request(&invoice)?,
            payment_hash: invoice.payment_hash().to_hex(),
            add_index,
        })
    }

    // Helper methods
    fn get_node_id(&self) -> String {
        // In a real implementation, this would return the node's public key
        "02abcdef1234567890abcdef1234567890abcdef1234567890abcdef1234567890ab".to_string()
    }

    fn channel_to_lightning_channel(
        &self,
        atomic_channel: &Arc<AtomicChannel>,
    ) -> LightningChannel {
        // Get channel info atomically
        let channel_info = atomic_channel
            .get_channel_info()
            .unwrap_or_else(|_| panic!("Failed to get channel info"));

        // Get the underlying channel for detailed info
        let channel = atomic_channel
            .channel
            .lock()
            .unwrap_or_else(|_| panic!("Failed to lock channel"));

        // Calculate actual statistics from channel data
        let total_novas_sent = channel
            .pending_htlcs
            .iter()
            .filter(|htlc| htlc.is_outgoing)
            .map(|htlc| htlc.amount_novas)
            .sum::<u64>();

        let total_novas_received = channel
            .pending_htlcs
            .iter()
            .filter(|htlc| !htlc.is_outgoing)
            .map(|htlc| htlc.amount_novas)
            .sum::<u64>();

        let num_updates = channel_info.commitment_number;

        // Calculate uptime
        let current_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let uptime = current_time - channel_info.last_operation_time;

        LightningChannel {
            channel_id: hex::encode(channel_info.channel_id),
            remote_pubkey: hex::encode(channel.remote_node_id.serialize()),
            capacity: channel_info.capacity_novas,
            local_balance: channel_info.local_balance_novas,
            remote_balance: channel_info.remote_balance_novas,
            commit_fee: 1000,   // Standard commitment fee
            commit_weight: 724, // Standard commitment weight
            fee_per_kw: 2500,   // Standard fee per kiloweight
            unsettled_balance: channel
                .pending_htlcs
                .iter()
                .map(|htlc| htlc.amount_novas)
                .sum(),
            total_satoshis_sent: total_novas_sent,
            total_satoshis_received: total_novas_received,
            num_updates,
            pending_htlcs: channel
                .pending_htlcs
                .iter()
                .map(|htlc| PendingHTLC {
                    incoming: !htlc.is_outgoing,
                    amount: htlc.amount_novas,
                    outpoint: format!("{}:{}", hex::encode(htlc.payment_hash), htlc.id),
                    maturity_height: htlc.expiry_height,
                    blocks_til_maturity: htlc.expiry_height as i32
                        - self.get_current_height() as i32,
                    stage: 1, // Simplified stage
                })
                .collect(),
            csv_delay: channel.to_self_delay as u32,
            private: !channel.is_public,
            initiator: channel.is_initiator,
            chan_status_flags: "ChanStatusDefault".to_string(),
            local_chan_reserve_sat: channel.channel_reserve_novas,
            remote_chan_reserve_sat: channel.channel_reserve_novas,
            static_remote_key: false,
            commitment_type: "ANCHORS".to_string(),
            lifetime: uptime,
            uptime,
            close_address: "".to_string(),
            push_amount_sat: 0, // Would need to track this from channel opening
            thaw_height: 0,
            local_constraints: ChannelConstraints {
                csv_delay: channel.to_self_delay as u32,
                chan_reserve_sat: channel.channel_reserve_novas,
                dust_limit_sat: 546,
                max_pending_amt_msat: 990000000,
                min_htlc_msat: channel.min_htlc_value_novas * 1000,
                max_accepted_htlcs: channel.max_accepted_htlcs as u32,
            },
            remote_constraints: ChannelConstraints {
                csv_delay: channel.to_self_delay as u32,
                chan_reserve_sat: channel.channel_reserve_novas,
                dust_limit_sat: 546,
                max_pending_amt_msat: 990000000,
                min_htlc_msat: channel.min_htlc_value_novas * 1000,
                max_accepted_htlcs: channel.max_accepted_htlcs as u32,
            },
        }
    }

    fn payment_to_lightning_payment(&self, payment: &Payment) -> LightningPayment {
        // Convert HTLCs from payment
        let htlcs = if let Some(route) = &payment.route {
            vec![HTLCAttempt {
                attempt_id: 0, // Single attempt for now
                status: match payment.status {
                    PaymentStatus::Pending => "IN_FLIGHT".to_string(),
                    PaymentStatus::Succeeded => "SUCCEEDED".to_string(),
                    PaymentStatus::Failed(_) => "FAILED".to_string(),
                    PaymentStatus::Cancelled => "FAILED".to_string(),
                },
                route: Route {
                    total_time_lock: 0, // Would need to track this
                    total_fees: payment.fee_msat / 1000,
                    total_amt: payment.amount_msat / 1000,
                    hops: route
                        .iter()
                        .map(|h| Hop {
                            chan_id: format!("{:016x}", h.channel_id),
                            chan_capacity: 1000000, // Would need channel info
                            amt_to_forward: h.amount_msat / 1000,
                            fee: h.channel_fee(h.amount_msat) / 1000,
                            expiry: h.cltv_expiry_delta as u32,
                            amt_to_forward_msat: h.amount_msat,
                            fee_msat: h.channel_fee(h.amount_msat),
                            pub_key: h.node_id.clone(),
                            tlv_payload: true,
                            mpp_record: None,
                            amp_record: None,
                            custom_records: HashMap::new(),
                        })
                        .collect(),
                    total_fees_msat: payment.fee_msat,
                    total_amt_msat: payment.amount_msat,
                },
                attempt_time_ns: payment.created_at * 1_000_000_000,
                resolve_time_ns: payment.completed_at.unwrap_or(payment.created_at) * 1_000_000_000,
                failure: payment.failure_reason.as_ref().map(|reason| Failure {
                    code: reason.clone(),
                    channel_update: None,
                    htlc_msat: payment.amount_msat,
                    onion_sha_256: vec![],
                    cltv_expiry: 0,
                    flags: 0,
                    failure_source_index: 0,
                    height: 0,
                }),
                preimage: payment
                    .payment_preimage
                    .as_ref()
                    .map(|p| p.to_hex())
                    .unwrap_or_default(),
            }]
        } else {
            vec![]
        };

        // Get payment index
        let payment_index = self
            .payment_index
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);

        LightningPayment {
            payment_hash: payment.payment_hash.to_hex(),
            value: payment.amount_msat / 1000,
            creation_date: payment.created_at,
            fee: payment.fee_msat / 1000,
            payment_preimage: payment
                .payment_preimage
                .as_ref()
                .map(|p| p.to_hex())
                .unwrap_or_default(),
            value_sat: payment.amount_msat / 1000,
            value_msat: payment.amount_msat,
            payment_request: "".to_string(),
            status: match payment.status {
                PaymentStatus::Pending => "IN_FLIGHT".to_string(),
                PaymentStatus::Succeeded => "SUCCEEDED".to_string(),
                PaymentStatus::Failed(_) => "FAILED".to_string(),
                PaymentStatus::Cancelled => "FAILED".to_string(),
            },
            fee_sat: payment.fee_msat / 1000,
            fee_msat: payment.fee_msat,
            creation_time_ns: payment.created_at * 1_000_000_000,
            htlcs,
            payment_index,
            failure_reason: payment.failure_reason.clone().unwrap_or_default(),
        }
    }

    fn invoice_to_lightning_invoice(&self, invoice: &Invoice) -> LightningInvoice {
        // Get HTLCs for this invoice
        let htlcs = {
            let channels = self.channels.read().unwrap();
            let mut invoice_htlcs = vec![];

            for (channel_id, atomic_channel) in channels.iter() {
                if let Ok(channel) = atomic_channel.channel.lock() {
                    for (idx, htlc) in channel.pending_htlcs.iter().enumerate() {
                        if htlc.payment_hash == *invoice.payment_hash().as_bytes() {
                            invoice_htlcs.push(InvoiceHTLC {
                                chan_id: channel_id.to_hex().parse().unwrap_or(0),
                                htlc_index: idx as u64,
                                amt_msat: htlc.amount_novas * 1000, // Convert to msat
                                accept_height: htlc.expiry_height as i32,
                                accept_time: invoice.created_at(),
                                resolve_time: invoice.settled_at().unwrap_or(0),
                                expiry_height: htlc.expiry_height as i32,
                                state: if invoice.is_settled() {
                                    "SETTLED".to_string()
                                } else {
                                    "ACCEPTED".to_string()
                                },
                                custom_records: HashMap::new(),
                                mpp_total_amt_msat: 0, // Would need MPP info
                                amp: None,             // Would need AMP info
                            });
                        }
                    }
                }
            }

            invoice_htlcs
        };

        // Generate BOLT11 payment request
        let payment_request = self.encode_payment_request(invoice).unwrap_or_default();

        // Get invoice index
        let add_index = self
            .invoice_index
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let settle_index = if invoice.is_settled() { add_index } else { 0 };

        LightningInvoice {
            memo: invoice.description().to_string(),
            r_preimage: invoice.payment_preimage().as_bytes().to_vec(),
            r_hash: invoice.payment_hash().as_bytes().to_vec(),
            value: invoice.amount_msat() / 1000,
            value_msat: invoice.amount_msat(),
            settled: invoice.is_settled(),
            creation_date: invoice.created_at(),
            settle_date: invoice.settled_at().unwrap_or(0),
            payment_request,
            description_hash: vec![],
            expiry: invoice.expiry_seconds() as u64,
            fallback_addr: "".to_string(),
            cltv_expiry: invoice.min_final_cltv_expiry() as u64,
            route_hints: vec![], // Would need to add route hints from channels
            private: invoice.is_private(),
            add_index,
            settle_index,
            amt_paid: if invoice.is_settled() {
                invoice.amount_msat() / 1000
            } else {
                0
            },
            amt_paid_sat: if invoice.is_settled() {
                invoice.amount_msat() / 1000
            } else {
                0
            },
            amt_paid_msat: if invoice.is_settled() {
                invoice.amount_msat()
            } else {
                0
            },
            state: if invoice.is_settled() {
                "SETTLED".to_string()
            } else {
                "OPEN".to_string()
            },
            htlcs,
            features: HashMap::new(),
            is_keysend: false,
            payment_addr: vec![],
            is_amp: false,
            amp_invoice_state: HashMap::new(),
        }
    }

    async fn create_funding_transaction(
        &self,
        amount: u64,
        channel_id: &ChannelId,
    ) -> Result<Transaction, ManagerError> {
        let wallet = self.wallet.lock().unwrap();
        wallet
            .create_funding_transaction(amount, channel_id)
            .map_err(|e| ManagerError::WalletError(e.to_string()))
    }

    async fn broadcast_transaction(&self, tx: &Transaction) -> Result<(), ManagerError> {
        // In a real implementation, this would broadcast to the network
        info!("Broadcasting transaction: {}", hex::encode(tx.hash()));
        Ok(())
    }

    fn parse_payment_request(&self, _payment_request: &str) -> Result<ParsedInvoice, ManagerError> {
        // Simplified BOLT11 parsing - in production would use proper parser
        Ok(ParsedInvoice {
            payment_hash: crate::lightning::payment::PaymentHash::new([0u8; 32]), // Placeholder
            amount_msat: 1000000,                                                 // Placeholder
            destination: "destination_node".to_string(),
            expiry: 3600,
            description: "Test payment".to_string(),
        })
    }

    fn encode_payment_request(&self, invoice: &Invoice) -> Result<String, ManagerError> {
        // BOLT11 payment request encoding
        // Format: ln[prefix][amount][separator][data][checksum]

        let _network_prefix = "bc"; // mainnet
        let amount_part = if invoice.amount_msat() > 0 {
            // Convert millinovas to the appropriate unit
            let amount_novas = invoice.amount_msat() / 1000;
            if amount_novas >= 1000000 {
                format!("{}m", amount_novas / 1000000) // mega-novas
            } else if amount_novas >= 1000 {
                format!("{}k", amount_novas / 1000) // kilo-novas
            } else {
                format!("{}", amount_novas) // novas
            }
        } else {
            "".to_string()
        };

        // Simplified BOLT11 - in production would use proper bech32 encoding
        let payment_hash_hex = invoice.payment_hash().to_hex();
        let timestamp = invoice.created_at();
        let expiry = invoice.expiry_seconds();

        // Create a simplified but recognizable payment request
        let payment_request = format!(
            "lnbc{}1{}{}{}{}",
            amount_part,
            timestamp % 1000000,      // Simplified timestamp
            &payment_hash_hex[0..10], // First 10 chars of payment hash
            expiry,
            "00" // Simplified checksum
        );

        Ok(payment_request)
    }

    async fn send_payment_through_route(
        &self,
        route: &crate::lightning::router::PaymentPath,
        _invoice: &ParsedInvoice,
    ) -> Result<crate::lightning::payment::PaymentPreimage, ManagerError> {
        // Simplified payment sending - in production would handle onion routing
        info!(
            "Sending payment through route with {} hops",
            route.hops.len()
        );

        // For now, just return a random preimage
        Ok(crate::lightning::payment::PaymentPreimage::new_random())
    }

    /// Get network nodes
    pub fn get_network_nodes(&self, _limit: u32) -> Result<Vec<NodeInfo>, ManagerError> {
        // In a real implementation, this would query the network graph
        Ok(vec![])
    }

    /// Get node information
    pub fn get_node_info(&self, _node_id: &str) -> Result<Option<NodeInfo>, ManagerError> {
        // In a real implementation, this would query the network graph
        Ok(None)
    }

    /// Find a route
    pub async fn find_route(
        &self,
        pub_key: &str,
        amt_msat: u64,
        fee_limit_msat: u64,
    ) -> Result<Option<Route>, ManagerError> {
        match self.router.find_route(pub_key, amt_msat, &[]) {
            Ok(route) => {
                if route.total_fee_msat <= fee_limit_msat {
                    // Get channel capacities for each hop
                    let channels = self.channels.read().unwrap();

                    Ok(Some(Route {
                        total_time_lock: route
                            .hops
                            .iter()
                            .map(|h| h.cltv_expiry_delta as u32)
                            .sum(),
                        total_fees: route.total_fee_msat / 1000,
                        total_amt: amt_msat / 1000,
                        hops: route
                            .hops
                            .iter()
                            .map(|hop| {
                                // Try to find channel capacity from our channels
                                let chan_capacity = channels
                                    .values()
                                    .find_map(|atomic_channel| {
                                        atomic_channel
                                            .get_channel_info()
                                            .ok()
                                            .filter(|info| {
                                                // Compare channel IDs directly as byte arrays
                                                &info.channel_id == hop.channel_id.as_bytes()
                                            })
                                            .map(|info| info.capacity_novas)
                                    })
                                    .unwrap_or(1000000); // Default 1M novas if not found

                                Hop {
                                    chan_id: hop.channel_id.to_hex(),
                                    chan_capacity,
                                    amt_to_forward: hop.amount_msat / 1000,
                                    fee: hop.channel_fee(hop.amount_msat) / 1000,
                                    expiry: hop.cltv_expiry_delta as u32,
                                    amt_to_forward_msat: hop.amount_msat,
                                    fee_msat: hop.channel_fee(hop.amount_msat),
                                    pub_key: hop.node_id.to_string(),
                                    tlv_payload: true,
                                    mpp_record: None,
                                    amp_record: None,
                                    custom_records: HashMap::new(),
                                }
                            })
                            .collect(),
                        total_fees_msat: route.total_fee_msat,
                        total_amt_msat: amt_msat,
                    }))
                } else {
                    Ok(None)
                }
            }
            Err(_) => Ok(None),
        }
    }

    /// Get the current blockchain height
    fn get_current_height(&self) -> u64 {
        // In a real implementation, this would query the blockchain state
        // For now, return a placeholder value
        700000 // Approximate current height
    }

    /// Lookup a payment by hash
    pub fn lookup_payment(&self, payment_hash: &str) -> Result<PaymentResponse, ManagerError> {
        let hash_bytes = hex::decode(payment_hash)
            .map_err(|_| ManagerError::InvalidPaymentRequest("Invalid payment hash".to_string()))?;

        if hash_bytes.len() != 32 {
            return Err(ManagerError::InvalidPaymentRequest(
                "Payment hash must be 32 bytes".to_string(),
            ));
        }

        let mut hash_array = [0u8; 32];
        hash_array.copy_from_slice(&hash_bytes);

        let payment_hash_obj = crate::lightning::payment::PaymentHash::new(hash_array);

        let payments = self.payments.read().unwrap();
        let payment = payments
            .get(&payment_hash_obj)
            .ok_or_else(|| ManagerError::PaymentNotFound("Payment not found".to_string()))?;

        // Convert HTLCs from payment route
        let _htlcs = if let Some(route) = &payment.route {
            route
                .iter()
                .enumerate()
                .map(|(i, hop)| crate::lightning::payment::Htlc {
                    id: i as u64,
                    payment_hash: *payment_hash_obj.as_bytes(),
                    amount_sat: hop.amount_msat / 1000, // Convert to sats
                    cltv_expiry: 0,                     // Would need to track this
                    offered: i == 0,                    // First hop is offered by us
                    state: crate::lightning::payment::HtlcState::Pending,
                    quantum_signature: None,
                })
                .collect()
        } else {
            vec![]
        };

        // Get payment index from our atomic counter
        let payment_index = self.payment_index.load(std::sync::atomic::Ordering::SeqCst);

        Ok(PaymentResponse {
            payment_hash: payment_hash.to_string(),
            payment_preimage: payment.payment_preimage.as_ref().map(|p| p.to_hex()),
            payment_route: payment
                .route
                .as_ref()
                .map(|r| r.iter().map(|h| h.node_id.clone()).collect())
                .unwrap_or_default(),
            payment_error: payment.failure_reason.clone(),
            payment_index,
            status: match payment.status {
                PaymentStatus::Pending => "PENDING".to_string(),
                PaymentStatus::Succeeded => "SUCCEEDED".to_string(),
                PaymentStatus::Failed(_) => "FAILED".to_string(),
                PaymentStatus::Cancelled => "FAILED".to_string(),
            },
            fee_msat: payment.fee_msat,
            value_msat: payment.amount_msat,
            creation_time_ns: payment.created_at * 1_000_000_000, // Convert to nanoseconds
        })
    }
}

// Helper structs
struct ParsedInvoice {
    payment_hash: crate::lightning::payment::PaymentHash,
    amount_msat: u64,
    destination: String,
    expiry: u32,
    description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenChannelResponse {
    pub channel_id: String,
    pub funding_txid: String,
    pub output_index: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaymentResponse {
    pub payment_hash: String,
    pub payment_preimage: Option<String>,
    pub payment_route: Vec<String>,
    pub payment_error: Option<String>,
    pub payment_index: u64,
    pub status: String,
    pub fee_msat: u64,
    pub value_msat: u64,
    pub creation_time_ns: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InvoiceResponse {
    pub payment_request: String,
    pub payment_hash: String,
    pub add_index: u64,
}

// Error conversions
impl From<LightningNetworkError> for ManagerError {
    fn from(err: LightningNetworkError) -> Self {
        ManagerError::NetworkError(err.to_string())
    }
}

impl From<crate::lightning::QuantumSecurityError> for ManagerError {
    fn from(err: crate::lightning::QuantumSecurityError) -> Self {
        ManagerError::QuantumSecurityError(err.to_string())
    }
}
