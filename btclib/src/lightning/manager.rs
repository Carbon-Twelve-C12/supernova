use std::collections::HashMap;
use std::sync::{Arc, RwLock, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH, Instant};
use tokio::sync::mpsc;
use tracing::{info, warn, error, debug};
use serde::{Serialize, Deserialize};
use thiserror::Error;

use crate::lightning::{
    Channel, ChannelId, ChannelState, ChannelConfig, 
    Payment, 
    Router, OnionRouter, Watchtower, LightningWallet, LightningConfig,
    QuantumChannelSecurity, LightningNetworkError
};
use crate::lightning::payment::{PaymentHash, PaymentPreimage, PaymentStatus, RouteHop, Htlc, HtlcState};
use crate::lightning::invoice::Invoice;
use crate::types::transaction::Transaction;
use crate::crypto::quantum::QuantumScheme;

/// Lightning Network Manager - Central coordinator for Lightning Network operations
pub struct LightningManager {
    /// Lightning Network configuration
    config: LightningConfig,
    
    /// Active payment channels
    channels: Arc<RwLock<HashMap<ChannelId, Channel>>>,
    
    /// Pending channels (opening/closing)
    pending_channels: Arc<RwLock<HashMap<ChannelId, Channel>>>,
    
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
            config.quantum_scheme.clone()
        } else {
            None
        };
        let onion_router = Arc::new(OnionRouter::new(private_key, quantum_scheme.clone()));
        
        // Initialize watchtower if enabled
        let watchtower = if config.use_quantum_signatures { // Using quantum flag as watchtower flag
            let watchtower_config = crate::lightning::watchtower::WatchtowerConfig::default();
            Some(Arc::new(Watchtower::new(watchtower_config, quantum_scheme.clone())))
        } else {
            None
        };
        
        // Initialize quantum security if enabled
        let quantum_security = if config.use_quantum_signatures {
            let quantum_config = crate::lightning::quantum_security::QuantumChannelConfig::default();
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
        };
        
        Ok((manager, event_receiver))
    }
    
    /// Start the Lightning Network manager
    pub async fn start(&self) -> Result<(), ManagerError> {
        info!("Starting Lightning Network manager");
        
        self.is_running.store(true, std::sync::atomic::Ordering::Relaxed);
        
        // Start watchtower if enabled
        if let Some(watchtower) = &self.watchtower {
            watchtower.start().await
                .map_err(|e| ManagerError::WatchtowerError(e.to_string()))?;
        }
        
        // Start router
        self.router.start().await
            .map_err(|e| ManagerError::RouterError(e.to_string()))?;
        
        info!("Lightning Network manager started successfully");
        Ok(())
    }
    
    /// Stop the Lightning Network manager
    pub async fn stop(&self) -> Result<(), ManagerError> {
        info!("Stopping Lightning Network manager");
        
        self.is_running.store(false, std::sync::atomic::Ordering::Relaxed);
        
        // Stop watchtower
        if let Some(watchtower) = &self.watchtower {
            watchtower.stop().await
                .map_err(|e| ManagerError::WatchtowerError(e.to_string()))?;
        }
        
        // Stop router
        self.router.stop().await
            .map_err(|e| ManagerError::RouterError(e.to_string()))?;
        
        info!("Lightning Network manager stopped");
        Ok(())
    }
    
    /// Get Lightning Network information
    pub fn get_info(&self) -> Result<LightningInfo, ManagerError> {
        let channels = self.channels.read().unwrap();
        let pending_channels = self.pending_channels.read().unwrap();
        let peers = self.peers.read().unwrap();
        
        let active_channels: Vec<_> = channels.values()
            .filter(|c| c.state == ChannelState::Active)
            .collect();
        
        let total_balance_msat = active_channels.iter()
            .map(|c| c.local_balance_sat * 1000) // Convert to msat
            .sum();
        
        let total_outbound_capacity_msat = active_channels.iter()
            .map(|c| c.local_balance_sat * 1000)
            .sum();
        
        let total_inbound_capacity_msat = active_channels.iter()
            .map(|c| c.remote_balance_sat * 1000)
            .sum();
        
        Ok(LightningInfo {
            node_id: self.get_node_id(),
            num_channels: active_channels.len(),
            num_pending_channels: pending_channels.len(),
            num_inactive_channels: channels.values()
                .filter(|c| c.state != ChannelState::Active)
                .count(),
            total_balance_msat,
            total_outbound_capacity_msat,
            total_inbound_capacity_msat,
            num_peers: peers.len(),
            synced_to_chain: true, // TODO: Implement chain sync status
            synced_to_graph: true, // TODO: Implement graph sync status
            block_height: 0, // TODO: Get from chain state
        })
    }
    
    /// Get all channels
    pub fn get_channels(&self, include_inactive: bool, include_pending: bool) -> Result<Vec<LightningChannel>, ManagerError> {
        let mut result = Vec::new();
        
        // Add active/inactive channels
        let channels = self.channels.read().unwrap();
        for channel in channels.values() {
            if include_inactive || channel.state == ChannelState::Active {
                result.push(self.channel_to_lightning_channel(channel));
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
        info!("Opening channel to {} with funding {}", node_id, local_funding_amount);
        
        // Validate parameters
        if local_funding_amount < 20000 {
            return Err(ManagerError::ConfigError("Minimum channel size is 20,000 satoshis".to_string()));
        }
        
        if push_amount > local_funding_amount {
            return Err(ManagerError::ConfigError("Push amount cannot exceed funding amount".to_string()));
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
        let funding_tx = self.create_funding_transaction(local_funding_amount, &channel_id).await?;
        
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
            self.config.quantum_scheme.clone(),
        ).map_err(|e| ManagerError::ChannelError(e.to_string()))?;
        
        // Add to pending channels
        {
            let mut pending_channels = self.pending_channels.write().unwrap();
            pending_channels.insert(channel_id.clone(), channel);
        }
        
        // Send event
        let _ = self.event_sender.send(LightningEvent::ChannelOpened(channel_id.clone()));
        
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
        let channel = {
            let mut channels = self.channels.write().unwrap();
            channels.remove(&channel_id)
        };
        
        if let Some(mut channel) = channel {
            // Create closing transaction
            let closing_tx = if force {
                channel.force_close()
                    .map_err(|e| ManagerError::ChannelError(e.to_string()))?
            } else {
                channel.cooperative_close()
                    .map_err(|e| ManagerError::ChannelError(e.to_string()))?
            };
            
            // Broadcast closing transaction
            self.broadcast_transaction(&closing_tx).await?;
            
            // Send event
            let _ = self.event_sender.send(LightningEvent::ChannelClosed(channel_id));
            
            Ok(true)
        } else {
            Ok(false)
        }
    }
    
    /// Get payment history
    pub fn get_payments(&self, index_offset: u64, max_payments: u64, include_pending: bool) -> Result<Vec<LightningPayment>, ManagerError> {
        let payments = self.payments.read().unwrap();
        
        let mut result: Vec<_> = payments.values()
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
        timeout_seconds: u32,
        fee_limit_msat: Option<u64>,
    ) -> Result<PaymentResponse, ManagerError> {
        info!("Sending payment: {}", payment_request);
        
        // Parse payment request (simplified - in production would parse BOLT11)
        let invoice = self.parse_payment_request(payment_request)?;
        
        // Use provided amount or invoice amount
        let amount = amount_msat.unwrap_or(invoice.amount_msat);
        
        // Find route
        let route = self.router.find_route(
            &invoice.destination,
            amount,
            &[], // Route hints
        ).map_err(|e| ManagerError::RouterError(e.to_string()))?;
        
        if route.is_empty() {
            return Err(ManagerError::PaymentFailed("No route found".to_string()));
        }
        
        // Check fee limit
        if let Some(max_fee) = fee_limit_msat {
            if route.total_fee_msat > max_fee {
                return Err(ManagerError::PaymentFailed(
                    format!("Route fee {} exceeds limit {}", route.total_fee_msat, max_fee)
                ));
            }
        }
        
        // Create payment
        let payment_hash = invoice.payment_hash.clone();
        let payment = Payment {
            payment_hash: payment_hash.clone(),
            payment_preimage: None,
            amount_msat: amount,
            status: PaymentStatus::Pending,
            created_at: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
            completed_at: None,
            fee_msat: route.total_fee_msat,
            route: Some(route.hops.iter().map(|h| RouteHop {
                channel_id: h.channel_id.to_hex().parse().unwrap_or(0),
                node_id: h.node_id.to_string(),
                amount_msat: h.amount_msat,
                fee_msat: h.channel_fee(h.amount_msat),
                cltv_expiry_delta: h.cltv_expiry_delta,
            }).collect()),
            failure_reason: None,
            carbon_footprint_grams: None,
        };
        
        // Store payment
        {
            let mut payments = self.payments.write().unwrap();
            payments.insert(payment_hash.clone(), payment);
        }
        
        // Send payment through route
        let preimage = self.send_payment_through_route(&route, &invoice).await?;
        
        // Update payment status
        {
            let mut payments = self.payments.write().unwrap();
            if let Some(payment) = payments.get_mut(&payment_hash) {
                payment.status = PaymentStatus::Succeeded;
                payment.payment_preimage = Some(preimage.clone());
                payment.completed_at = Some(SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs());
            }
        }
        
        // Send event
        let _ = self.event_sender.send(LightningEvent::PaymentSent(payment_hash.clone(), amount));
        
        Ok(PaymentResponse {
            payment_hash: payment_hash.to_hex(),
            payment_preimage: Some(preimage.to_hex()),
            payment_route: route.hops.iter().map(|h| h.node_id.to_string()).collect(),
            payment_error: None,
            payment_index: 0, // TODO: Implement payment indexing
            status: "SUCCEEDED".to_string(),
            fee_msat: route.total_fee_msat,
            value_msat: amount,
            creation_time_ns: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos() as u64,
        })
    }
    
    /// Get invoices
    pub fn get_invoices(&self, pending_only: bool, index_offset: u64, num_max_invoices: u64) -> Result<Vec<LightningInvoice>, ManagerError> {
        let invoices = self.invoices.read().unwrap();
        
        let result: Vec<_> = invoices.values()
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
        private: bool,
    ) -> Result<InvoiceResponse, ManagerError> {
        info!("Creating invoice for {} msat", value_msat);
        
        // Generate payment hash and preimage
        let preimage = crate::lightning::payment::PaymentPreimage::new_random();
        let payment_hash = preimage.payment_hash();
        
        // Convert to invoice types for compatibility
        let invoice_preimage = crate::lightning::invoice::PaymentPreimage::new(preimage.into_inner());
        let invoice_hash = crate::lightning::invoice::PaymentHash::new(payment_hash.clone().into_inner());
        
        // Create invoice
        let invoice = Invoice::new(
            invoice_hash,
            value_msat,
            memo.to_string(),
            expiry,
            private,
            self.get_node_id(),
            invoice_preimage,
        );
        
        // Store invoice using payment module types
        {
            let mut invoices = self.invoices.write().unwrap();
            invoices.insert(payment_hash.clone(), invoice.clone());
        }
        
        // Generate payment request (simplified BOLT11)
        let payment_request = self.encode_payment_request(&invoice)?;
        
        // Send event
        let _ = self.event_sender.send(LightningEvent::InvoiceCreated(payment_hash.clone()));
        
        Ok(InvoiceResponse {
            payment_request,
            payment_hash: payment_hash.to_hex(),
            add_index: 0, // TODO: Implement invoice indexing
        })
    }
    
    // Helper methods
    fn get_node_id(&self) -> String {
        // In a real implementation, this would return the node's public key
        "02abcdef1234567890abcdef1234567890abcdef1234567890abcdef1234567890ab".to_string()
    }
    
    fn channel_to_lightning_channel(&self, channel: &Channel) -> LightningChannel {
        LightningChannel {
            channel_id: hex::encode(channel.channel_id),
            remote_pubkey: hex::encode(channel.remote_node_id.serialize()),
            capacity: channel.capacity_sat,
            local_balance: channel.local_balance_sat,
            remote_balance: channel.remote_balance_sat,
            commit_fee: 1000, // Placeholder
            commit_weight: 724, // Placeholder
            fee_per_kw: 2500, // Placeholder
            unsettled_balance: 0, // Placeholder
            total_satoshis_sent: 0, // TODO: Track this
            total_satoshis_received: 0, // TODO: Track this
            num_updates: 0, // TODO: Track this
            pending_htlcs: vec![], // TODO: Convert HTLCs
            csv_delay: channel.to_self_delay,
            private: !channel.is_public,
            initiator: channel.is_initiator,
            chan_status_flags: "ChanStatusDefault".to_string(),
            local_chan_reserve_sat: channel.channel_reserve_sat,
            remote_chan_reserve_sat: channel.channel_reserve_sat,
            static_remote_key: false,
            commitment_type: "ANCHORS".to_string(),
            lifetime: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() - channel.last_update,
            uptime: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() - channel.last_update,
            close_address: "".to_string(),
            push_amount_sat: 0, // TODO: Track this
            thaw_height: 0,
            local_constraints: ChannelConstraints {
                csv_delay: channel.to_self_delay,
                chan_reserve_sat: channel.channel_reserve_sat,
                dust_limit_sat: 546,
                max_pending_amt_msat: 990000000,
                min_htlc_msat: channel.min_htlc_value_sat * 1000,
                max_accepted_htlcs: channel.max_accepted_htlcs as u32,
            },
            remote_constraints: ChannelConstraints {
                csv_delay: channel.to_self_delay,
                chan_reserve_sat: channel.channel_reserve_sat,
                dust_limit_sat: 546,
                max_pending_amt_msat: 990000000,
                min_htlc_msat: channel.min_htlc_value_sat * 1000,
                max_accepted_htlcs: channel.max_accepted_htlcs as u32,
            },
        }
    }
    
    fn payment_to_lightning_payment(&self, payment: &Payment) -> LightningPayment {
        LightningPayment {
            payment_hash: payment.payment_hash.to_hex(),
            value: payment.amount_msat / 1000,
            creation_date: payment.created_at,
            fee: payment.fee_msat / 1000,
            payment_preimage: payment.payment_preimage.as_ref()
                .map(|p| p.to_hex())
                .unwrap_or_default(),
            value_sat: payment.amount_msat / 1000,
            value_msat: payment.amount_msat,
            payment_request: "".to_string(), // TODO: Store original request
            status: match payment.status {
                PaymentStatus::Pending => "IN_FLIGHT".to_string(),
                PaymentStatus::Succeeded => "SUCCEEDED".to_string(),
                PaymentStatus::Failed(_) => "FAILED".to_string(),
                PaymentStatus::Cancelled => "FAILED".to_string(),
            },
            fee_sat: payment.fee_msat / 1000,
            fee_msat: payment.fee_msat,
            creation_time_ns: payment.created_at * 1_000_000_000,
            htlcs: vec![], // TODO: Convert HTLCs
            payment_index: 0, // TODO: Implement indexing
            failure_reason: payment.failure_reason.clone().unwrap_or_default(),
        }
    }
    
    fn invoice_to_lightning_invoice(&self, invoice: &Invoice) -> LightningInvoice {
        LightningInvoice {
            memo: invoice.description().to_string(),
            r_preimage: invoice.payment_preimage().as_bytes().to_vec(),
            r_hash: invoice.payment_hash().as_bytes().to_vec(),
            value: invoice.amount_msat() / 1000,
            value_msat: invoice.amount_msat(),
            settled: invoice.is_settled(),
            creation_date: invoice.created_at(),
            settle_date: invoice.settled_at().unwrap_or(0),
            payment_request: "".to_string(), // TODO: Generate BOLT11
            description_hash: vec![],
            expiry: invoice.expiry_seconds() as u64,
            fallback_addr: "".to_string(),
            cltv_expiry: invoice.min_final_cltv_expiry() as u64,
            route_hints: vec![], // TODO: Add route hints
            private: invoice.is_private(),
            add_index: 0, // TODO: Implement indexing
            settle_index: 0,
            amt_paid: if invoice.is_settled() { invoice.amount_msat() / 1000 } else { 0 },
            amt_paid_sat: if invoice.is_settled() { invoice.amount_msat() / 1000 } else { 0 },
            amt_paid_msat: if invoice.is_settled() { invoice.amount_msat() } else { 0 },
            state: if invoice.is_settled() { "SETTLED".to_string() } else { "OPEN".to_string() },
            htlcs: vec![], // TODO: Add HTLCs
            features: HashMap::new(),
            is_keysend: false,
            payment_addr: vec![],
            is_amp: false,
            amp_invoice_state: HashMap::new(),
        }
    }
    
    async fn create_funding_transaction(&self, amount: u64, channel_id: &ChannelId) -> Result<Transaction, ManagerError> {
        let wallet = self.wallet.lock().unwrap();
        wallet.create_funding_transaction(amount, channel_id)
            .map_err(|e| ManagerError::WalletError(e.to_string()))
    }
    
    async fn broadcast_transaction(&self, tx: &Transaction) -> Result<(), ManagerError> {
        // In a real implementation, this would broadcast to the network
        info!("Broadcasting transaction: {}", hex::encode(tx.hash()));
        Ok(())
    }
    
    fn parse_payment_request(&self, payment_request: &str) -> Result<ParsedInvoice, ManagerError> {
        // Simplified BOLT11 parsing - in production would use proper parser
        Ok(ParsedInvoice {
            payment_hash: crate::lightning::payment::PaymentHash::new([0u8; 32]), // Placeholder
            amount_msat: 1000000, // Placeholder
            destination: "destination_node".to_string(),
            expiry: 3600,
            description: "Test payment".to_string(),
        })
    }
    
    fn encode_payment_request(&self, invoice: &Invoice) -> Result<String, ManagerError> {
        // Simplified BOLT11 encoding - in production would use proper encoder
        Ok(format!("lnbc{}m1...", invoice.amount_msat() / 100_000))
    }
    
    async fn send_payment_through_route(&self, route: &crate::lightning::router::PaymentPath, invoice: &ParsedInvoice) -> Result<crate::lightning::payment::PaymentPreimage, ManagerError> {
        // Simplified payment sending - in production would handle onion routing
        info!("Sending payment through route with {} hops", route.hops.len());
        
        // For now, just return a random preimage
        Ok(crate::lightning::payment::PaymentPreimage::new_random())
    }
    
    /// Get network nodes
    pub fn get_network_nodes(&self, limit: u32) -> Result<Vec<NodeInfo>, ManagerError> {
        // In a real implementation, this would query the network graph
        Ok(vec![])
    }
    
    /// Get node information
    pub fn get_node_info(&self, node_id: &str) -> Result<Option<NodeInfo>, ManagerError> {
        // In a real implementation, this would query the network graph
        Ok(None)
    }
    
    /// Find a route
    pub async fn find_route(&self, pub_key: &str, amt_msat: u64, fee_limit_msat: u64) -> Result<Option<Route>, ManagerError> {
        match self.router.find_route(pub_key, amt_msat, &[]) {
            Ok(route) => {
                if route.total_fee_msat <= fee_limit_msat {
                    Ok(Some(Route {
                        total_time_lock: 0, // TODO: Calculate
                        total_fees: route.total_fee_msat / 1000,
                        total_amt: amt_msat / 1000,
                        hops: route.hops.iter().map(|hop| Hop {
                            chan_id: hop.channel_id.to_hex(),
                            chan_capacity: 1000000, // TODO: Get from channel
                            amt_to_forward: hop.amount_msat / 1000,
                            fee: hop.channel_fee(hop.amount_msat) / 1000,
                            expiry: hop.cltv_expiry as u32,
                            amt_to_forward_msat: hop.amount_msat,
                            fee_msat: hop.channel_fee(hop.amount_msat),
                            pub_key: hop.node_id.to_string(),
                            tlv_payload: true,
                            mpp_record: None,
                            amp_record: None,
                            custom_records: HashMap::new(),
                        }).collect(),
                        total_fees_msat: route.total_fee_msat,
                        total_amt_msat: amt_msat,
                    }))
                } else {
                    Ok(None)
                }
            },
            Err(_) => Ok(None),
        }
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