// supernova Lightning Network Implementation
//
// This module implements Lightning Network functionality for the Supernova blockchain.
// It provides payment channel creation, management, and routing capabilities.

pub mod atomic_operations;
pub mod channel;
pub mod green_routing;
pub mod invoice;
pub mod manager;
pub mod onion;
pub mod payment;
pub mod quantum_channel;
pub mod quantum_lightning;
pub mod quantum_security;
pub mod router;
pub mod wallet;
pub mod watchtower;

#[cfg(test)]
pub mod race_condition_tests;

pub use atomic_operations::{AtomicChannel, AtomicChannelState, AtomicOperationError};
pub use channel::{Channel, ChannelConfig, ChannelError, ChannelId, ChannelManager, ChannelState};
pub use invoice::{EnhancedInvoice, Invoice, InvoiceDatabase, InvoiceError, RouteHint};
pub use manager::{
    LightningChannel, LightningInfo, LightningInvoice, LightningManager, LightningPayment,
    ManagerError,
};
pub use onion::{OnionPacket, OnionRouter, PerHopPayload, SharedSecret};
pub use payment::{
    Htlc, HtlcState, Payment, PaymentError, PaymentHash, PaymentPreimage, PaymentProcessor,
    PaymentStatus, RouteHop,
};
pub use quantum_lightning::{
    calculate_lightning_carbon_footprint, create_quantum_lightning_channel,
    test_quantum_htlc_operations, track_environmental_lightning_metrics,
    validate_quantum_channel_security, ChannelEnvironmentalData, EnvironmentalLightningMetrics,
    GreenLightningRoute, GreenRouteHop, LightningError, QuantumHTLC, QuantumLightningChannel,
    QuantumLightningManager,
};
pub use quantum_security::{QuantumChannelConfig, QuantumChannelSecurity, QuantumSecurityError};
pub use router::{
    ChannelInfo as RouterChannelInfo, NodeId, PathHop, PaymentPath, Router, RoutingError,
};
pub use wallet::{LightningWallet, WalletError};
pub use watchtower::{
    BreachRemedy, ChannelMonitor, EncryptedChannelState, WatchError, Watchtower, WatchtowerClient,
    WatchtowerConfig,
};

pub use green_routing::{
    apply_environmental_routing_preferences, calculate_route_carbon_footprint,
    incentivize_green_lightning_nodes, optimize_for_renewable_energy_nodes, EnvironmentalChannel,
    EnvironmentalLightningStats, EnvironmentalNetworkGraph, EnvironmentalNode,
    EnvironmentalRoutingPreferences, EnvironmentalSavingsReport, GreenIncentiveProgram,
    GreenLightningRouter, GreenPaymentCertificate, PaymentEnvironmentalImpact, RoutingPriority,
    TimePeriod,
};

// Re-export main types from this module

use crate::crypto::quantum::QuantumScheme;
use crate::types::transaction::Transaction;
use rand;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, RwLock};
use thiserror::Error;
use tracing::{debug, error, info};

/// Direction of an HTLC
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HtlcDirection {
    /// HTLC is offered by us (outgoing payment)
    Offered,
    /// HTLC is received from peer (incoming payment)
    Received,
}

/// Error types for Lightning Network operations
#[derive(Debug, Error)]
pub enum LightningNetworkError {
    #[error("Channel error: {0}")]
    ChannelError(#[from] channel::ChannelError),

    #[error("Invoice error: {0}")]
    InvoiceError(#[from] invoice::InvoiceError),

    #[error("Routing error: {0}")]
    RoutingError(#[from] router::RoutingError),

    #[error("Wallet error: {0}")]
    WalletError(#[from] wallet::WalletError),

    #[error("Watch tower error: {0}")]
    WatchError(#[from] watchtower::WatchError),

    #[error("Network error: {0}")]
    NetworkError(String),

    #[error("Invalid state: {0}")]
    InvalidState(String),

    #[error("Insufficient funds: {0}")]
    InsufficientFunds(String),
}

/// Main Lightning Network manager
pub struct LightningNetwork {
    /// Active payment channels
    channels: Arc<RwLock<HashMap<channel::ChannelId, Arc<RwLock<Channel>>>>>,

    /// Lightning Network specific wallet
    wallet: Arc<Mutex<LightningWallet>>,

    /// Payment router
    router: Arc<Router>,

    /// Channel monitor for security
    monitor: Arc<RwLock<ChannelMonitor>>,

    /// Network configuration
    config: LightningConfig,

    /// Quantum signature scheme if enabled
    quantum_scheme: Option<QuantumScheme>,
}

/// Lightning Network configuration
#[derive(Debug, Clone)]
pub struct LightningConfig {
    /// Default channel capacity
    pub default_channel_capacity: u64,

    /// Minimum channel capacity
    pub min_channel_capacity: u64,

    /// Maximum channel capacity
    pub max_channel_capacity: u64,

    /// CLTV delta for forwarded payments
    pub cltv_expiry_delta: u16,

    /// Fee rate for forwarded payments (millionths)
    pub fee_base_msat: u32,

    /// Fee rate proportional to payment amount (millionths)
    pub fee_proportional_millionths: u32,

    /// Enable quantum-resistant signatures
    pub use_quantum_signatures: bool,

    /// Quantum signature scheme if enabled
    pub quantum_scheme: Option<QuantumScheme>,

    /// Quantum security level
    pub quantum_security_level: u8,
}

impl LightningNetwork {
    /// Create a new Lightning Network manager
    pub fn new(config: LightningConfig, wallet: LightningWallet) -> Self {
        let wallet = Arc::new(Mutex::new(wallet));
        let router = Arc::new(Router::new());
        let monitor = Arc::new(RwLock::new(ChannelMonitor::new()));

        let quantum_scheme = config.quantum_scheme;

        Self {
            channels: Arc::new(RwLock::new(HashMap::new())),
            wallet,
            router,
            monitor,
            config,
            quantum_scheme,
        }
    }

    /// Open a new payment channel with a peer
    pub async fn open_channel(
        &self,
        peer_id: &str,
        capacity: u64,
        push_amount: u64,
        channel_config: Option<ChannelConfig>,
    ) -> Result<channel::ChannelId, LightningNetworkError> {
        // Validate parameters
        if capacity < self.config.min_channel_capacity {
            return Err(LightningNetworkError::InvalidState(format!(
                "Channel capacity {} is below minimum {}",
                capacity, self.config.min_channel_capacity
            )));
        }

        if capacity > self.config.max_channel_capacity {
            return Err(LightningNetworkError::InvalidState(format!(
                "Channel capacity {} is above maximum {}",
                capacity, self.config.max_channel_capacity
            )));
        }

        if push_amount >= capacity {
            return Err(LightningNetworkError::InvalidState(format!(
                "Push amount {} must be less than capacity {}",
                push_amount, capacity
            )));
        }

        // Check wallet balance
        let wallet = self.wallet.lock().unwrap();
        if wallet.get_balance() < capacity {
            return Err(LightningNetworkError::InsufficientFunds(format!(
                "Insufficient funds: needed {}, available {}",
                capacity,
                wallet.get_balance()
            )));
        }

        // Create channel with default or custom config
        let config = channel_config.unwrap_or_default();

        // Create actual channel (implementation in channel.rs)
        let channel = Channel::open(
            peer_id.to_string(),
            capacity,
            push_amount,
            config,
            self.quantum_scheme,
        )?;

        let channel_id = channel.id().clone();

        // Register channel
        {
            let mut channels = self.channels.write().unwrap();
            channels.insert(channel_id.clone(), Arc::new(RwLock::new(channel)));
        }

        // Register with monitor
        {
            let mut monitor = self.monitor.write().unwrap();

            // Create encrypted channel state (placeholder for production implementation)
            let encrypted_state = EncryptedChannelState {
                encrypted_data: vec![0u8; 32], // In real implementation, encrypt channel state
                iv: vec![0u8; 16],             // Random initialization vector
                tag: vec![0u8; 16],            // Authentication tag
            };

            monitor.register_channel(
                *channel_id.as_bytes(),
                peer_id, // Use peer_id as client_id
                encrypted_state,
            )?;
        }

        Ok(channel_id)
    }

    /// Close a payment channel
    pub async fn close_channel(
        &self,
        channel_id: &channel::ChannelId,
        force_close: bool,
    ) -> Result<Transaction, LightningNetworkError> {
        // Find channel
        let channel_arc = {
            let channels = self.channels.read().unwrap();
            channels.get(channel_id).cloned()
        };

        let channel_arc = channel_arc.ok_or_else(|| {
            LightningNetworkError::InvalidState(format!("Channel {} not found", channel_id))
        })?;

        // Close channel (cooperative or force)
        let closing_tx = if force_close {
            let mut channel = channel_arc.write().unwrap();
            channel.force_close()?
        } else {
            let channel = channel_arc.read().unwrap();
            channel.cooperative_close()?
        };

        // Unregister from monitor
        {
            let mut monitor = self.monitor.write().unwrap();
            monitor.unregister_channel(channel_id.as_bytes())?;
        }

        // Remove from active channels
        {
            let mut channels = self.channels.write().unwrap();
            channels.remove(channel_id);
        }

        Ok(closing_tx)
    }

    /// Create an invoice for payment
    pub fn create_invoice(
        &self,
        amount_msat: u64,
        description: &str,
        expiry_seconds: u32,
    ) -> Result<Invoice, LightningNetworkError> {
        let mut wallet = self.wallet.lock().unwrap();
        let invoice = wallet.create_invoice(amount_msat, description, expiry_seconds)?;

        Ok(invoice)
    }

    /// Pay an invoice
    pub async fn pay_invoice(
        &self,
        invoice: &Invoice,
    ) -> Result<payment::PaymentPreimage, LightningNetworkError> {
        info!(
            "Paying invoice: amount={} msat, destination={}",
            invoice.amount_msat(),
            invoice.destination()
        );

        // Find a route to the destination
        let route = {
            // Convert invoice route hints to router route hints
            let router_hints: Vec<router::RouteHint> = invoice
                .route_hints()
                .iter()
                .map(|hint| {
                    // Convert u64 channel ID to [u8; 32] by using it as a seed
                    let mut channel_id_bytes = [0u8; 32];
                    channel_id_bytes[0..8].copy_from_slice(&hint.channel_id.to_le_bytes());

                    router::RouteHint {
                        node_id: router::NodeId::new(hint.node_id.clone()),
                        channel_id: channel::ChannelId::from_bytes(channel_id_bytes),
                        base_fee_msat: hint.base_fee_msat,
                        fee_rate_millionths: hint.fee_rate_millionths,
                        cltv_expiry_delta: hint.cltv_expiry_delta,
                    }
                })
                .collect();

            self.router
                .find_route(invoice.destination(), invoice.amount_msat(), &router_hints)?
        };

        if route.is_empty() {
            return Err(LightningNetworkError::RoutingError(
                router::RoutingError::NoRouteFound,
            ));
        }

        debug!(
            "Found route with {} hops, total fee: {} msat",
            route.len(),
            route.total_fee_msat
        );

        // For single-hop payments, proceed directly
        if route.len() == 1 {
            let mut wallet = self.wallet.lock().unwrap();
            return Ok(wallet.pay_invoice(invoice)?);
        }

        // For multi-hop payments, we need to handle the routing
        // First, create a payment hash and shared secret for each hop
        let payment_hash = invoice.payment_hash();
        let payment_hash_bytes = payment_hash.into_inner(); // Store the bytes once
        let _next_hop_shared_secret = [0u8; 32]; // Would be derived in a real implementation

        // Generate random payment preimages for testing
        let _rng = rand::thread_rng();
        let payment_preimage = payment::PaymentPreimage::new_random();
        let payment_preimage_bytes = payment_preimage.into_inner(); // Store the bytes once

        // Track the current payment amount (decreases as we move through the route due to fees)
        let mut remaining_amount = invoice.amount_msat();
        let mut current_expiry = invoice.min_final_cltv_expiry();

        // Process the route in reverse (from destination to source)
        for i in (0..route.hops.len()).rev() {
            let _hop = &route.hops[i];

            // For real implementation, we would:
            // 1. Generate shared secret for this hop
            // 2. Create onion packet with encrypted payload
            // 3. Add HTLC to channel with appropriate timelock

            // Add fees for this hop
            if i < route.hops.len() - 1 {
                // Intermediate hop - add fees
                let fee = route.hops[i].channel_fee(remaining_amount);
                remaining_amount += fee;

                // Adjust timelock
                current_expiry += route.hops[i].cltv_expiry_delta as u32;
            }
        }

        // Now process the route from source to destination
        let mut preimage: Option<payment::PaymentPreimage> = None;

        for (i, hop) in route.hops.iter().enumerate() {
            // Find the channel for this hop
            let channel_arc = {
                let channels = self.channels.read().unwrap();
                channels.get(&hop.channel_id).cloned()
            };

            let channel_arc = channel_arc.ok_or_else(|| {
                LightningNetworkError::InvalidState(format!("Channel {} not found", hop.channel_id))
            })?;

            // Calculate the amount to forward and timelock
            let forward_amount = if i == route.hops.len() - 1 {
                // Last hop - use final amount
                invoice.amount_msat()
            } else {
                // Intermediate hop - forward amount minus fees for next hops
                let next_hop_fees = route.hops[i + 1..]
                    .iter()
                    .map(|h| h.channel_fee(remaining_amount))
                    .sum::<u64>();
                remaining_amount - next_hop_fees
            };

            // Calculate timelock
            let timelock = if i == route.hops.len() - 1 {
                // Last hop - use final expiry
                current_expiry
            } else {
                // Intermediate hop - add CLTV delta for this hop
                current_expiry + hop.cltv_expiry_delta as u32
            };

            debug!(
                "Hop {}: forwarding {} msat with timelock {}",
                i, forward_amount, timelock
            );

            let mut channel = channel_arc.write().unwrap();

            // Add HTLC to channel
            let htlc_id = channel.add_htlc(
                payment_hash_bytes,
                forward_amount / 1000, // Convert from msat to sat
                timelock,
                matches!(HtlcDirection::Offered, HtlcDirection::Offered), // Convert enum to bool
            )?;

            // If this is the final hop, wait for fulfillment (in a real implementation)
            if i == route.hops.len() - 1 {
                // For test purposes, we'll simulate the fulfillment
                // In a real implementation, we would listen for the fulfillment or failure

                preimage = Some(payment::PaymentPreimage::new(payment_preimage_bytes));

                // Fulfill the HTLC (simulate remote fulfillment)
                channel.fulfill_htlc(htlc_id, payment_preimage_bytes)?;
            }
        }

        // Now handle the fulfillment in reverse
        for i in (0..route.hops.len() - 1).rev() {
            // Find the channel for this hop
            let channel_arc = {
                let channels = self.channels.read().unwrap();
                channels.get(&route.hops[i].channel_id).cloned()
            };

            let channel_arc = channel_arc.ok_or_else(|| {
                LightningNetworkError::InvalidState(format!(
                    "Channel {} not found",
                    route.hops[i].channel_id
                ))
            })?;

            let mut channel = channel_arc.write().unwrap();

            // Find the HTLC and fulfill it
            // In a real implementation, we would track the HTLC IDs
            // For now, we'll simulate finding the right HTLC
            let htlcs = channel.get_pending_htlcs();

            if let Some(htlc) = htlcs.iter().find(|h| h.payment_hash == payment_hash_bytes) {
                channel.fulfill_htlc(htlc.id, payment_preimage_bytes)?;
            }
        }

        // Return the payment preimage
        preimage.ok_or_else(|| {
            LightningNetworkError::InvalidState(
                "Payment completed but no preimage available".to_string(),
            )
        })
    }

    /// Get all active channels
    pub fn list_channels(&self) -> Vec<channel::ChannelId> {
        let channels = self.channels.read().unwrap();
        channels.keys().cloned().collect()
    }

    /// Get detailed information about a specific channel
    pub fn get_channel_info(
        &self,
        channel_id: &channel::ChannelId,
    ) -> Option<channel::ChannelInfo> {
        let channels = self.channels.read().unwrap();

        channels.get(channel_id).map(|channel_arc| {
            let channel = channel_arc.read().unwrap();
            channel.get_info()
        })
    }

    /// Handle an incoming HTLC payment from another node
    pub async fn handle_incoming_htlc(
        &self,
        channel_id: &channel::ChannelId,
        htlc_id: u64,
        payment_hash: [u8; 32],
        amount_msat: u64,
        cltv_expiry: u32,
        _onion_packet: &[u8],
    ) -> Result<Option<payment::PaymentPreimage>, LightningNetworkError> {
        info!(
            "Received incoming HTLC on channel {}: amount={} msat, payment_hash={:x?}",
            channel_id,
            amount_msat,
            &payment_hash[0..4]
        );

        // Find the channel
        let channel_arc = {
            let channels = self.channels.read().unwrap();
            channels.get(channel_id).cloned()
        };

        let channel_arc = channel_arc.ok_or_else(|| {
            LightningNetworkError::InvalidState(format!("Channel {} not found", channel_id))
        })?;

        // Decode the onion packet to determine if this is final hop or forwarding
        // In a real implementation, we would:
        // 1. Decrypt the onion packet with our key
        // 2. Extract the payload for this hop
        // 3. Get the next hop information if not the final recipient

        // For now, we'll simulate this by checking if we have an invoice matching the payment hash
        let mut wallet = self.wallet.lock().unwrap();
        let is_final_recipient = wallet.has_invoice(&payment_hash);

        if is_final_recipient {
            debug!("We are the final recipient for this HTLC");

            // Get the invoice
            let payment_hash_obj = payment::PaymentHash::new(payment_hash);
            let invoice = wallet.get_invoice(&payment_hash_obj).ok_or_else(|| {
                LightningNetworkError::InvalidState(format!(
                    "Invoice for payment hash {:x?} not found",
                    &payment_hash[0..4]
                ))
            })?;

            // Verify amount
            if invoice.amount_msat() > amount_msat {
                return Err(LightningNetworkError::InsufficientFunds(format!(
                    "Received amount {} is less than invoice amount {}",
                    amount_msat,
                    invoice.amount_msat()
                )));
            }

            // Verify expiry (simplified)
            if cltv_expiry < invoice.min_final_cltv_expiry() {
                return Err(LightningNetworkError::InvalidState(format!(
                    "CLTV expiry {} is less than required {}",
                    cltv_expiry,
                    invoice.min_final_cltv_expiry()
                )));
            }

            // Accept the HTLC
            {
                let mut channel = channel_arc.write().unwrap();

                // Record the incoming HTLC
                channel.add_htlc(
                    payment_hash,
                    amount_msat / 1000, // Convert from msat to sat
                    cltv_expiry,
                    matches!(HtlcDirection::Received, HtlcDirection::Received), // Convert enum to bool
                )?;
            }

            // Get the payment preimage
            let preimage = invoice.payment_preimage();
            let preimage_bytes = preimage.into_inner();

            // Fulfill the HTLC
            {
                let mut channel = channel_arc.write().unwrap();
                channel.fulfill_htlc(htlc_id, preimage_bytes)?;
            }

            // Mark the invoice as paid
            wallet.mark_invoice_paid(&payment_hash)?;

            Ok(Some(payment::PaymentPreimage::new(preimage_bytes)))
        } else {
            debug!("We are forwarding this HTLC");

            // In a real implementation, we would:
            // 1. Extract the next hop information from the onion packet
            // 2. Find a suitable outgoing channel to the next hop
            // 3. Forward the HTLC with the remaining amount

            // For simplicity, we'll fail the HTLC for now
            {
                let mut channel = channel_arc.write().unwrap();
                channel.fail_htlc(htlc_id, "Unable to forward payment")?;
            }

            // Return no preimage since we failed
            Ok(None)
        }
    }
}

impl Default for LightningConfig {
    fn default() -> Self {
        Self {
            default_channel_capacity: 1_000_000, // 0.01 BTC in satoshis
            min_channel_capacity: 100_000,       // 0.001 BTC in satoshis
            max_channel_capacity: 167_772_160,   // 1.67772160 BTC in satoshis
            cltv_expiry_delta: 40,
            fee_base_msat: 1000,              // 1 satoshi base fee
            fee_proportional_millionths: 100, // 0.01% fee rate
            use_quantum_signatures: false,
            quantum_scheme: None,
            quantum_security_level: 1,
        }
    }
}

// Register Lightning module with the supernova node
// This function is called during node initialization
// NOTE: This function should be implemented in the node crate, not here
// Commented out to resolve cross-crate reference issues
/*
#[cfg(feature = "lightning")]
pub fn register_lightning(node: &mut crate::node::Node) -> Result<(), LightningNetworkError> {
    info!("Initializing Lightning Network functionality");

    // Create Lightning wallet from node wallet
    let wallet = LightningWallet::from_node_wallet(node.wallet())?;

    // Create Lightning configuration from node config
    let config = LightningConfig {
        use_quantum_signatures: node.config().use_quantum_signatures,
        quantum_scheme: node.config().quantum_scheme.clone(),
        quantum_security_level: node.config().quantum_security_level,
        ..LightningConfig::default()
    };

    // Create Lightning Network manager
    let lightning = LightningNetwork::new(config, wallet);

    // Register with node
    node.register_lightning(lightning);

    Ok(())
}
*/
