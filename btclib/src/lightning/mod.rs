// SuperNova Lightning Network Implementation
//
// This module implements Lightning Network functionality for the SuperNova blockchain.
// It provides payment channel creation, management, and routing capabilities.

mod channel;
mod wire;
mod invoice;
mod router;
mod wallet;
mod watch;

pub use channel::{Channel, ChannelId, ChannelState, ChannelConfig, ChannelError};
pub use wire::{Message, MessageType, LightningError};
pub use invoice::{Invoice, InvoiceError, PaymentHash, PaymentPreimage};
pub use router::{Router, RouteHint, PaymentPath, RoutingError};
pub use wallet::{LightningWallet, KeyManager, KeyDerivation, WalletError};
pub use watch::{WatchTower, ChannelMonitor, BreachRemedy, WatchError};

use crate::types::transaction::{Transaction, TransactionInput, TransactionOutput};
use crate::crypto::quantum::{QuantumKeyPair, QuantumScheme};
use std::sync::{Arc, RwLock, Mutex};
use std::collections::{HashMap, HashSet};
use thiserror::Error;
use tracing::{debug, info, warn, error};

/// Error types for Lightning Network operations
#[derive(Debug, Error)]
pub enum LightningNetworkError {
    #[error("Channel error: {0}")]
    ChannelError(#[from] channel::ChannelError),
    
    #[error("Wire protocol error: {0}")]
    WireError(#[from] wire::LightningError),
    
    #[error("Invoice error: {0}")]
    InvoiceError(#[from] invoice::InvoiceError),
    
    #[error("Routing error: {0}")]
    RoutingError(#[from] router::RoutingError),
    
    #[error("Wallet error: {0}")]
    WalletError(#[from] wallet::WalletError),
    
    #[error("Watch tower error: {0}")]
    WatchError(#[from] watch::WatchError),
    
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
    channels: Arc<RwLock<HashMap<ChannelId, Arc<RwLock<Channel>>>>>,
    
    /// Lightning Network specific wallet
    wallet: Arc<Mutex<LightningWallet>>,
    
    /// Payment router
    router: Arc<RwLock<Router>>,
    
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
        let router = Arc::new(RwLock::new(Router::new()));
        let monitor = Arc::new(RwLock::new(ChannelMonitor::new()));
        
        Self {
            channels: Arc::new(RwLock::new(HashMap::new())),
            wallet,
            router,
            monitor,
            config,
            quantum_scheme: config.quantum_scheme,
        }
    }
    
    /// Open a new payment channel with a peer
    pub async fn open_channel(
        &self,
        peer_id: &str,
        capacity: u64,
        push_amount: u64,
        channel_config: Option<ChannelConfig>,
    ) -> Result<ChannelId, LightningNetworkError> {
        // Validate parameters
        if capacity < self.config.min_channel_capacity {
            return Err(LightningNetworkError::InvalidState(
                format!("Channel capacity {} is below minimum {}", capacity, self.config.min_channel_capacity)
            ));
        }
        
        if capacity > self.config.max_channel_capacity {
            return Err(LightningNetworkError::InvalidState(
                format!("Channel capacity {} is above maximum {}", capacity, self.config.max_channel_capacity)
            ));
        }
        
        if push_amount >= capacity {
            return Err(LightningNetworkError::InvalidState(
                format!("Push amount {} must be less than capacity {}", push_amount, capacity)
            ));
        }
        
        // Check wallet balance
        let wallet = self.wallet.lock().unwrap();
        if wallet.get_balance() < capacity {
            return Err(LightningNetworkError::InsufficientFunds(
                format!("Insufficient funds: needed {}, available {}", capacity, wallet.get_balance())
            ));
        }
        
        // Create channel with default or custom config
        let config = channel_config.unwrap_or_else(|| ChannelConfig::default());
        
        // Create actual channel (implementation in channel.rs)
        let channel = Channel::open(
            peer_id.to_string(),
            capacity,
            push_amount,
            config,
            self.quantum_scheme.clone(),
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
            monitor.register_channel(channel_id.clone())?;
        }
        
        Ok(channel_id)
    }
    
    /// Close a payment channel
    pub async fn close_channel(
        &self,
        channel_id: &ChannelId,
        force_close: bool,
    ) -> Result<Transaction, LightningNetworkError> {
        // Find channel
        let channel_arc = {
            let channels = self.channels.read().unwrap();
            channels.get(channel_id).cloned()
        };
        
        let channel_arc = channel_arc.ok_or_else(|| 
            LightningNetworkError::InvalidState(format!("Channel {} not found", channel_id))
        )?;
        
        // Close channel (cooperative or force)
        let channel = channel_arc.read().unwrap();
        let closing_tx = if force_close {
            channel.force_close()?
        } else {
            channel.cooperative_close()?
        };
        
        // Unregister from monitor
        {
            let mut monitor = self.monitor.write().unwrap();
            monitor.unregister_channel(channel_id)?;
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
        let wallet = self.wallet.lock().unwrap();
        let invoice = wallet.create_invoice(amount_msat, description, expiry_seconds)?;
        
        Ok(invoice)
    }
    
    /// Pay an invoice
    pub async fn pay_invoice(
        &self,
        invoice: &Invoice,
    ) -> Result<PaymentPreimage, LightningNetworkError> {
        // Find a route to the destination
        let route = {
            let router = self.router.read().unwrap();
            router.find_route(
                invoice.destination(),
                invoice.amount_msat(),
                invoice.route_hints(),
            )?
        };
        
        // Execute payment across channels
        let preimage = match route.len() {
            0 => {
                return Err(LightningNetworkError::RoutingError(
                    router::RoutingError::NoRouteFound
                ));
            }
            _ => {
                // Implementation would handle multi-hop payment
                // For now, simulate a direct payment
                let wallet = self.wallet.lock().unwrap();
                wallet.pay_invoice(invoice)?
            }
        };
        
        Ok(preimage)
    }
    
    /// Get all active channels
    pub fn list_channels(&self) -> Vec<ChannelId> {
        let channels = self.channels.read().unwrap();
        channels.keys().cloned().collect()
    }
    
    /// Get detailed information about a specific channel
    pub fn get_channel_info(&self, channel_id: &ChannelId) -> Option<channel::ChannelInfo> {
        let channels = self.channels.read().unwrap();
        
        channels.get(channel_id).map(|channel_arc| {
            let channel = channel_arc.read().unwrap();
            channel.get_info()
        })
    }
}

impl Default for LightningConfig {
    fn default() -> Self {
        Self {
            default_channel_capacity: 1_000_000, // 0.01 BTC in satoshis
            min_channel_capacity: 100_000,       // 0.001 BTC in satoshis
            max_channel_capacity: 167_772_160,   // 1.67772160 BTC in satoshis
            cltv_expiry_delta: 40,
            fee_base_msat: 1000,                 // 1 satoshi base fee
            fee_proportional_millionths: 100,    // 0.01% fee rate
            use_quantum_signatures: false,
            quantum_scheme: None,
            quantum_security_level: 1,
        }
    }
}

// Register Lightning module with the SuperNova node
// This function is called during node initialization
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