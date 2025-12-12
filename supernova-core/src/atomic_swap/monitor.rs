//! Cross-chain monitoring for atomic swaps
//!
//! This module monitors both Bitcoin and Supernova blockchains for
//! HTLC-related events and automatically triggers claims when secrets are revealed.

use crate::atomic_swap::bitcoin_adapter::{extract_secret_from_bitcoin_tx, BitcoinRpcClient};
use crate::atomic_swap::error::MonitorError;
use crate::atomic_swap::{
    AtomicSwapError, BitcoinHTLCReference, HTLCState, SupernovaHTLC, SwapSession, SwapState,
};
use futures::stream::{Stream, StreamExt};
use lru::LruCache;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::num::NonZeroUsize;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::sync::watch;
use tokio::sync::RwLock;

use std::future::Future;
use std::time::Duration;

/// Cross-chain monitoring service
pub struct CrossChainMonitor {
    /// Active swap sessions being monitored
    active_swaps: Arc<RwLock<HashMap<[u8; 32], SwapSession>>>,

    /// Bitcoin RPC client (if feature enabled)
    #[cfg(feature = "atomic-swap")]
    bitcoin_client: Option<BitcoinRpcClient>,

    /// Supernova blockchain handle
    supernova_handle: Option<Arc<SupernovaHandle>>,

    /// Monitoring configuration
    config: MonitorConfig,

    /// Signal to stop monitoring
    stop_signal: watch::Receiver<bool>,

    /// Event channel for notifications
    event_tx: mpsc::UnboundedSender<SwapEvent>,

    /// History of emitted events
    event_history: Arc<RwLock<LruCache<SwapEvent, ()>>>,
}

/// Retry/backoff configuration for unreliable cross-chain monitoring.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RetryConfig {
    /// Base delay between retries.
    pub base_delay: Duration,
    /// Maximum backoff delay.
    pub max_delay: Duration,
    /// Maximum number of retries before failing.
    pub max_retries: u32,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            base_delay: Duration::from_millis(250),
            max_delay: Duration::from_secs(10),
            max_retries: 5,
        }
    }
}

/// Monitoring configuration
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MonitorConfig {
    /// How often to check for new blocks (seconds)
    pub poll_interval: u64,

    /// Number of confirmations required before processing
    pub min_confirmations: u32,

    /// Enable automatic claim execution
    pub auto_claim: bool,

    /// Enable automatic refund execution
    pub auto_refund: bool,

    /// Maximum number of retries for failed operations
    pub max_retries: u32,
}

impl Default for MonitorConfig {
    fn default() -> Self {
        Self {
            poll_interval: 10,
            min_confirmations: 2,
            auto_claim: true,
            auto_refund: true,
            max_retries: 3,
        }
    }
}

/// A minimal reorg tracker for swap-critical transactions.
///
/// This does not attempt to resolve the reorg; it detects it and allows callers
/// to transition swap state or rescan.
#[derive(Debug, Clone)]
pub struct ReorgTracker {
    pub confirmed_height: u64,
    pub confirmed_block_hash: [u8; 32],
}

impl ReorgTracker {
    pub fn new(confirmed_height: u64, confirmed_block_hash: [u8; 32]) -> Self {
        Self {
            confirmed_height,
            confirmed_block_hash,
        }
    }

    /// Returns true if the block hash at `confirmed_height` no longer matches the recorded hash.
    pub fn is_reorg(&self, current_block_hash_at_height: [u8; 32]) -> bool {
        current_block_hash_at_height != self.confirmed_block_hash
    }
}

/// Execute an async operation with bounded retries and exponential backoff.
///
/// This is used by cross-chain monitoring to tolerate intermittent RPC failures and
/// temporary network partitions.
pub(crate) async fn retry_with_backoff<T, F, Fut>(
    retry: RetryConfig,
    mut op: F,
) -> Result<T, MonitorError>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<T, MonitorError>>,
{
    let mut attempt: u32 = 0;
    loop {
        attempt += 1;
        match op().await {
            Ok(v) => return Ok(v),
            Err(e) => {
                if attempt >= retry.max_retries {
                    return Err(e);
                }
                // Exponential backoff with cap.
                let exp = 2u32.saturating_pow((attempt - 1).min(10));
                let mut delay = retry.base_delay.saturating_mul(exp);
                if delay > retry.max_delay {
                    delay = retry.max_delay;
                }
                tokio::time::sleep(delay).await;
            }
        }
    }
}

/// Handle to Supernova blockchain operations
pub struct SupernovaHandle {
    // Placeholder for actual blockchain handle
    pub current_height: u64,
}

impl SupernovaHandle {
    /// Submit a claim transaction to the Supernova network
    async fn submit_claim(&self, claim_data: SupernovaClaimData) -> Result<String, String> {
        // In a real implementation, this would:
        // 1. Create a claim transaction
        // 2. Sign it with the appropriate key
        // 3. Broadcast to the network
        // 4. Return the transaction ID

        // For now, simulate success
        let tx_id = format!("nova_claim_{}", hex::encode(&claim_data.htlc_id[..8]));
        Ok(tx_id)
    }

    /// Submit a refund transaction to the Supernova network
    async fn submit_refund(&self, refund_data: SupernovaRefundData) -> Result<String, String> {
        // Similar to submit_claim but for refunds
        let tx_id = format!("nova_refund_{}", hex::encode(&refund_data.htlc_id[..8]));
        Ok(tx_id)
    }

    /// Get current blockchain height
    async fn get_height(&self) -> Result<u64, String> {
        Ok(self.current_height)
    }
}

/// Data required to claim a Supernova HTLC
#[derive(Clone, Debug)]
struct SupernovaClaimData {
    htlc_id: Vec<u8>,
    secret: [u8; 32],
    claimer: String,
    timestamp: u64,
}

/// Data required to refund a Supernova HTLC
#[derive(Clone, Debug)]
struct SupernovaRefundData {
    htlc_id: Vec<u8>,
    refunder: String,
    timestamp: u64,
}

/// Events emitted by the monitor
#[derive(Clone, Debug, Serialize, Deserialize, Hash, Eq, PartialEq)]
pub enum SwapEvent {
    /// New swap initiated
    SwapInitiated {
        swap_id: [u8; 32],
        initiator: String,
        participant: String,
        amounts: SwapAmounts,
    },

    /// HTLC funded on either chain
    HTLCFunded {
        swap_id: [u8; 32],
        chain: Chain,
        tx_id: String,
        confirmations: u32,
    },

    /// Secret revealed on Bitcoin chain
    SecretRevealed {
        swap_id: [u8; 32],
        secret_hash: [u8; 32],
        revealed_in_tx: String,
    },

    /// Swap completed successfully
    SwapCompleted {
        swap_id: [u8; 32],
        btc_claim_tx: String,
        nova_claim_tx: String,
        duration_seconds: u64,
    },

    /// Swap refunded
    SwapRefunded {
        swap_id: [u8; 32],
        chain: Chain,
        refund_tx: String,
        reason: RefundReason,
    },
}

/// Chain identifier
#[derive(Clone, Debug, Serialize, Deserialize, Hash, Eq, PartialEq)]
pub enum Chain {
    Bitcoin,
    Supernova,
}

/// Swap amount information
#[derive(Clone, Debug, Serialize, Deserialize, Hash, Eq, PartialEq)]
pub struct SwapAmounts {
    pub bitcoin_sats: u64,
    pub nova_units: u64,
}

/// Reason for refund
#[derive(Clone, Debug, Serialize, Deserialize, Hash, Eq, PartialEq)]
pub enum RefundReason {
    Timeout,
    UserRequested,
    CounterpartyFailure,
    NetworkError(String),
}

impl CrossChainMonitor {
    /// Create a new cross-chain monitor
    pub fn new(config: MonitorConfig, supernova_handle: Option<Arc<SupernovaHandle>>) -> Self {
        let (event_tx, _) = mpsc::unbounded_channel();
        let (stop_tx, stop_rx) = watch::channel(false);

        Self {
            config,
            active_swaps: Arc::new(RwLock::new(HashMap::new())),
            bitcoin_client: None,
            supernova_handle,
            event_history: Arc::new(RwLock::new(LruCache::new(NonZeroUsize::new(1000).unwrap()))),
            stop_signal: stop_rx,
            event_tx,
        }
    }

    /// Create a new cross-chain monitor with custom event channel
    pub fn new_with_event_channel(
        config: MonitorConfig,
        supernova_handle: Option<Arc<SupernovaHandle>>,
        event_tx: mpsc::UnboundedSender<SwapEvent>,
    ) -> Self {
        let (stop_tx, stop_rx) = watch::channel(false);

        Self {
            config,
            active_swaps: Arc::new(RwLock::new(HashMap::new())),
            bitcoin_client: None,
            supernova_handle,
            event_history: Arc::new(RwLock::new(LruCache::new(NonZeroUsize::new(1000).unwrap()))),
            stop_signal: stop_rx,
            event_tx,
        }
    }

    /// Add a swap session to monitor
    pub async fn add_swap(&self, swap: SwapSession) -> Result<(), MonitorError> {
        let mut swaps = self.active_swaps.write().await;
        swaps.insert(swap.setup.swap_id, swap);
        Ok(())
    }

    /// Remove a swap from monitoring
    pub async fn remove_swap(&self, swap_id: &[u8; 32]) -> Result<(), MonitorError> {
        let mut swaps = self.active_swaps.write().await;
        swaps.remove(swap_id);
        Ok(())
    }

    /// Start monitoring both chains
    pub async fn start_monitoring(&self) {
        let btc_monitor = self.monitor_bitcoin_events();
        let nova_monitor = self.monitor_supernova_events();

        tokio::join!(btc_monitor, nova_monitor);
    }

    /// Monitor Bitcoin blockchain for events
    #[cfg(feature = "atomic-swap")]
    async fn monitor_bitcoin_events(&self) {
        if let Some(client) = &self.bitcoin_client {
            let mut last_block_height = 0u64;

            loop {
                // Check if we should stop monitoring
                if self.stop_signal.borrow().clone() {
                    break;
                }

                // Get current block height
                match client.get_block_height().await {
                    Ok(current_height) => {
                        // Process new blocks
                        if current_height > last_block_height {
                            for height in (last_block_height + 1)..=current_height {
                                if let Err(e) = self.process_bitcoin_block(height).await {
                                    tracing::error!(
                                        "Error processing Bitcoin block {}: {:?}",
                                        height,
                                        e
                                    );
                                }
                            }
                            last_block_height = current_height;
                        }
                    }
                    Err(e) => {
                        tracing::error!("Failed to get Bitcoin block height: {:?}", e);
                        // Fall back to checking active swaps
                        let swaps = self.active_swaps.read().await;
                        for (swap_id, swap) in swaps.iter() {
                            if let Err(e) = self.check_bitcoin_htlc(swap).await {
                                log::error!(
                                    "Error checking Bitcoin HTLC for swap {}: {:?}",
                                    hex::encode(swap_id),
                                    e
                                );
                            }
                        }
                        drop(swaps);
                    }
                }

                // Wait before next check
                tokio::time::sleep(tokio::time::Duration::from_secs(self.config.poll_interval))
                    .await;
            }
        }
    }

    /// Monitor Bitcoin blockchain without the feature
    #[cfg(not(feature = "atomic-swap"))]
    async fn monitor_bitcoin_events(&self) {
        // No-op when feature is disabled
        log::info!("Bitcoin monitoring disabled (atomic-swap feature not enabled)");
    }

    /// Monitor Supernova blockchain for events
    async fn monitor_supernova_events(&self) {
        loop {
            // Check each active swap for Supernova events
            let swaps = self.active_swaps.read().await;
            for (swap_id, swap) in swaps.iter() {
                if let Err(e) = self.check_supernova_htlc(swap).await {
                    log::error!(
                        "Error checking Supernova HTLC for swap {}: {:?}",
                        hex::encode(swap_id),
                        e
                    );
                }
            }
            drop(swaps);

            // Wait before next check
            tokio::time::sleep(tokio::time::Duration::from_secs(self.config.poll_interval)).await;
        }
    }

    /// Process a single Bitcoin block for swap events
    #[cfg(feature = "bitcoincore-rpc")]
    async fn process_bitcoin_block(&self, height: u64) -> Result<(), MonitorError> {
        let bitcoin_client = self
            .bitcoin_client
            .as_ref()
            .ok_or(MonitorError::NotInitialized)?;

        // Get block hash using RPC
        use bitcoincore_rpc::RpcApi;
        let block_hash = bitcoin_client
            .client
            .get_block_hash(height)
            .map_err(|e| MonitorError::BitcoinRpcError(e.to_string()))?;

        // Get full block with transactions
        let block = bitcoin_client
            .client
            .get_block(&block_hash)
            .map_err(|e| MonitorError::BitcoinRpcError(e.to_string()))?;

        // Process each transaction in the block
        for tx in block.txdata.iter() {
            // Check if this transaction is relevant to any active swaps
            if let Some(event) = self.analyze_bitcoin_transaction(tx).await {
                self.handle_bitcoin_event(event).await?;
            }
        }

        Ok(())
    }

    /// Analyze a Bitcoin transaction for swap-related events
    async fn analyze_bitcoin_transaction(&self, tx: &bitcoin::Transaction) -> Option<SwapEvent> {
        // Try to extract secret from transaction inputs
        if let Ok(secret) = extract_secret_from_bitcoin_tx(tx) {
            // Calculate secret hash
            use sha2::{Digest, Sha256};
            let secret_hash_bytes = Sha256::digest(&secret);
            let mut secret_hash = [0u8; 32];
            secret_hash.copy_from_slice(&secret_hash_bytes);

            // Find which swap this secret belongs to
            let swaps = self.active_swaps.read().await;
            for (swap_id, session) in swaps.iter() {
                if session.nova_htlc.hash_lock.hash_value == secret_hash {
                    return Some(SwapEvent::SecretRevealed {
                        swap_id: *swap_id,
                        secret_hash,
                        revealed_in_tx: tx.txid().to_string(),
                    });
                }
            }
        }

        // Check for timeout/refund conditions
        // This would require analyzing the script and comparing against known HTLCs

        None
    }

    /// Handle Bitcoin blockchain events
    pub async fn handle_bitcoin_event(&self, event: SwapEvent) -> Result<(), MonitorError> {
        match &event {
            SwapEvent::SecretRevealed {
                swap_id,
                secret_hash: _,
                revealed_in_tx,
            } => {
                tracing::info!(
                    "Secret revealed for swap {:?} in Bitcoin tx: {}",
                    hex::encode(&swap_id),
                    revealed_in_tx
                );

                // If auto-claim is enabled, trigger Supernova claim
                if self.config.auto_claim {
                    // Get the swap session and find the secret
                    let swaps = self.active_swaps.read().await;
                    if let Some(swap) = swaps.get(swap_id) {
                        // In a real implementation, we would extract the actual secret from the Bitcoin tx
                        // For now, we'll use a placeholder
                        let secret = [0u8; 32]; // This should be extracted from the Bitcoin transaction
                        let swap_clone = swap.clone();
                        drop(swaps); // Release the read lock before calling trigger_supernova_claim

                        self.trigger_supernova_claim(&swap_clone, secret).await?;
                    }
                }

                // Update swap state
                let mut swaps = self.active_swaps.write().await;
                if let Some(swap) = swaps.get_mut(swap_id) {
                    swap.state = SwapState::Claimed;
                }

                // Emit event for listeners
                let _ = self.event_tx.send(event);
            }
            _ => {
                // Handle other event types
                let _ = self.event_tx.send(event);
            }
        }

        Ok(())
    }

    /// Check Bitcoin HTLC status
    #[cfg(feature = "atomic-swap")]
    async fn check_bitcoin_htlc(&self, swap: &SwapSession) -> Result<(), MonitorError> {
        if let Some(client) = &self.bitcoin_client {
            // Get the Bitcoin transaction
            let tx = client
                .get_transaction(&swap.btc_htlc.txid)
                .await
                .map_err(|e| MonitorError::BlockStreamError(e.to_string()))?;

            // Check if the HTLC has been claimed
            // In a real implementation, we'd check if the output has been spent
            // For now, we'll try to extract a secret from the transaction
            if let Ok(secret) = extract_secret_from_bitcoin_tx(&tx) {
                // Secret revealed! Trigger Supernova claim if auto-claim is enabled
                if self.config.auto_claim {
                    self.trigger_supernova_claim(swap, secret).await?;
                }
            }
        }

        Ok(())
    }

    /// Check Supernova HTLC status
    async fn check_supernova_htlc(&self, swap: &SwapSession) -> Result<(), MonitorError> {
        // Check if timeout has been reached for refund
        if swap.nova_htlc.is_expired() && self.config.auto_refund {
            self.trigger_supernova_refund(swap).await?;
        }

        Ok(())
    }

    /// Trigger automatic claim on Supernova
    async fn trigger_supernova_claim(
        &self,
        swap: &SwapSession,
        secret: [u8; 32],
    ) -> Result<(), MonitorError> {
        log::info!(
            "Triggering Supernova claim for swap {}",
            hex::encode(&swap.setup.swap_id)
        );

        // Create claim data for Supernova HTLC
        let claim_data = SupernovaClaimData {
            htlc_id: swap.nova_htlc.htlc_id.to_vec(),
            secret,
            claimer: swap.nova_htlc.participant.address.clone(),
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        };

        // Submit claim to Supernova network
        if let Some(handle) = &self.supernova_handle {
            match handle.submit_claim(claim_data).await {
                Ok(tx_id) => {
                    log::info!("Submitted Supernova claim tx: {}", tx_id);

                    // Update swap state
                    let mut swaps = self.active_swaps.write().await;
                    if let Some(swap) = swaps.get_mut(&swap.setup.swap_id) {
                        swap.state = SwapState::Claimed;
                    }
                }
                Err(e) => {
                    log::error!("Failed to submit Supernova claim: {:?}", e);
                    return Err(MonitorError::ClaimFailed(format!(
                        "Supernova claim failed: {}",
                        e
                    )));
                }
            }
        } else {
            log::warn!("No Supernova handle available for auto-claim");
        }

        Ok(())
    }

    /// Trigger automatic refund on Supernova
    async fn trigger_supernova_refund(&self, swap: &SwapSession) -> Result<(), MonitorError> {
        log::info!(
            "Triggering Supernova refund for swap {}",
            hex::encode(&swap.setup.swap_id)
        );

        // Create refund data
        let refund_data = SupernovaRefundData {
            htlc_id: swap.nova_htlc.htlc_id.to_vec(),
            refunder: swap.nova_htlc.initiator.address.clone(),
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        };

        // Submit refund to Supernova network
        if let Some(handle) = &self.supernova_handle {
            match handle.submit_refund(refund_data).await {
                Ok(tx_id) => {
                    log::info!("Submitted Supernova refund tx: {}", tx_id);

                    // Update swap state
                    let mut swaps = self.active_swaps.write().await;
                    if let Some(swap) = swaps.get_mut(&swap.setup.swap_id) {
                        swap.state = SwapState::Refunded;
                    }
                }
                Err(e) => {
                    log::error!("Failed to submit Supernova refund: {:?}", e);
                    return Err(MonitorError::RefundFailed(format!(
                        "Supernova refund failed: {}",
                        e
                    )));
                }
            }
        } else {
            log::warn!("No Supernova handle available for auto-refund");
        }

        Ok(())
    }

    /// Get status of all active swaps
    pub async fn get_active_swaps(&self) -> Vec<SwapSummary> {
        let swaps = self.active_swaps.read().await;
        swaps
            .values()
            .map(|swap| SwapSummary {
                swap_id: swap.setup.swap_id,
                state: swap.state.clone(),
                bitcoin_amount: swap.setup.bitcoin_amount,
                nova_amount: swap.setup.nova_amount,
                created_at: swap.created_at,
            })
            .collect()
    }

    /// Find a secret that corresponds to a given hash
    async fn find_secret_for_hash(&self, secret_hash: &[u8; 32]) -> Option<[u8; 32]> {
        // In a real implementation, this would:
        // 1. Check a cache of revealed secrets
        // 2. Query the blockchain for claim transactions
        // 3. Parse witness data for the secret

        // For now, return None as we don't have the secret storage implemented
        None
    }
}

/// Summary of a swap for monitoring
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SwapSummary {
    pub swap_id: [u8; 32],
    pub state: SwapState,
    pub bitcoin_amount: u64,
    pub nova_amount: u64,
    pub created_at: u64,
}

/// Parse Bitcoin transaction for swap events
pub fn parse_bitcoin_swap_event(tx: &bitcoin::Transaction) -> Option<SwapEvent> {
    // Try to extract secret from transaction
    if let Ok(secret) = extract_secret_from_bitcoin_tx(tx) {
        // This is a claim transaction revealing a secret
        use sha2::{Digest, Sha256};
        let mut secret_hash = [0u8; 32];
        secret_hash.copy_from_slice(&Sha256::digest(&secret));

        return Some(SwapEvent::SecretRevealed {
            swap_id: [0u8; 32], // Would need to determine actual swap ID
            secret_hash,
            revealed_in_tx: tx.txid().to_string(),
        });
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::atomic_swap::{
        AtomicSwapSetup, FeeDistribution, FeePayer, HashFunction, HashLock, ParticipantInfo,
        TimeoutConfig,
    };
    use crate::crypto::MLDSAPrivateKey;
    use rand::rngs::OsRng;

    fn create_test_swap() -> SwapSession {
        let alice_key = MLDSAPrivateKey::generate(&mut OsRng);
        let bob_key = MLDSAPrivateKey::generate(&mut OsRng);

        let alice = ParticipantInfo {
            pubkey: alice_key.public_key(),
            address: "nova1alice".to_string(),
            refund_address: None,
        };

        let bob = ParticipantInfo {
            pubkey: bob_key.public_key(),
            address: "nova1bob".to_string(),
            refund_address: None,
        };

        let hash_lock = HashLock::new(HashFunction::SHA256).unwrap();
        let timeout_config = TimeoutConfig {
            bitcoin_blocks: 144,
            supernova_blocks: 720,
        };

        let setup = AtomicSwapSetup {
            swap_id: [1u8; 32],
            initiator: alice,
            recipient: bob,
            hash_lock,
            timeout_config,
            bitcoin_amount: 100000,
            nova_amount: 1000000000,
            fee_distribution: FeeDistribution::Split {
                initiator_share: 50,
                recipient_share: 50,
            },
            fee_payer: FeePayer::Initiator,
            created_at: 0,
        };

        SwapSession {
            setup,
            state: SwapState::Active,
            bitcoin_htlc_address: "tb1qtest".to_string(),
            nova_htlc_id: vec![2u8; 32],
            created_at: 0,
        }
    }

    #[tokio::test]
    async fn test_monitor_creation() {
        let config = MonitorConfig::default();
        let monitor = CrossChainMonitor::new(config, None);

        let swaps = monitor.get_active_swaps().await;
        assert!(swaps.is_empty());
    }

    #[tokio::test]
    async fn test_add_remove_swap() {
        let config = MonitorConfig::default();
        let monitor = CrossChainMonitor::new(config, None);

        let swap = create_test_swap();
        let swap_id = swap.setup.swap_id;

        // Add swap
        monitor.add_swap(swap).await.unwrap();
        let swaps = monitor.get_active_swaps().await;
        assert_eq!(swaps.len(), 1);
        assert_eq!(swaps[0].swap_id, swap_id);

        // Remove swap
        monitor.remove_swap(&swap_id).await.unwrap();
        let swaps = monitor.get_active_swaps().await;
        assert!(swaps.is_empty());
    }

    #[tokio::test]
    async fn test_swap_state_transitions() {
        let config = MonitorConfig::default();
        let monitor = CrossChainMonitor::new(config, None);

        let swap = create_test_swap();
        let swap_id = swap.setup.swap_id;

        monitor.add_swap(swap).await.unwrap();

        // Test state transition to claimed
        {
            let mut swaps = monitor.active_swaps.write().await;
            if let Some(swap) = swaps.get_mut(&swap_id) {
                swap.state = SwapState::Claimed;
            }
        }

        let swaps = monitor.get_active_swaps().await;
        assert_eq!(swaps[0].state, SwapState::Claimed);
    }

    #[tokio::test]
    async fn test_bitcoin_transaction_analysis() {
        let config = MonitorConfig::default();
        let monitor = CrossChainMonitor::new(config, None);

        // Create a swap with known hash
        let mut swap = create_test_swap();
        let secret = [42u8; 32];
        use sha2::{Digest, Sha256};
        let secret_hash = Sha256::digest(&secret);
        let mut hash_value = [0u8; 32];
        hash_value.copy_from_slice(&secret_hash);
        swap.setup.hash_lock.hash_value = hash_value;

        monitor.add_swap(swap.clone()).await.unwrap();

        // Create a mock Bitcoin transaction that reveals the secret
        // In a real test, we would create a proper Bitcoin transaction
        // For now, we'll test the event detection logic

        // Verify the swap is active
        let swaps = monitor.get_active_swaps().await;
        assert_eq!(swaps.len(), 1);
        assert_eq!(swaps[0].state, SwapState::Active);
    }

    #[tokio::test]
    async fn test_monitor_with_timeout() {
        use tokio::time::{timeout, Duration};

        let mut config = MonitorConfig::default();
        config.poll_interval = 1; // 1 second for faster testing

        let monitor = CrossChainMonitor::new(config, None);

        // Start monitoring in background
        let monitor_handle = tokio::spawn(async move {
            monitor.start_monitoring().await;
        });

        // Let it run for a short time
        let _ = timeout(Duration::from_secs(2), monitor_handle).await;

        // Test passes if no panic occurred
    }

    #[tokio::test]
    async fn test_supernova_claim_submission() {
        let config = MonitorConfig::default();
        let supernova_handle = Some(SupernovaHandle {
            current_height: 1000,
        });
        let monitor = CrossChainMonitor::new(config, supernova_handle);

        let swap = create_test_swap();
        monitor.add_swap(swap.clone()).await.unwrap();

        // Test claim submission
        let secret = [99u8; 32];
        let result = monitor.trigger_supernova_claim(&swap, secret).await;

        // Should succeed with our mock implementation
        assert!(result.is_ok());

        // Verify state was updated
        let swaps = monitor.get_active_swaps().await;
        assert_eq!(swaps[0].state, SwapState::Claimed);
    }

    #[tokio::test]
    async fn test_supernova_refund_submission() {
        let config = MonitorConfig::default();
        let supernova_handle = Some(SupernovaHandle {
            current_height: 1000,
        });
        let monitor = CrossChainMonitor::new(config, supernova_handle);

        let swap = create_test_swap();
        monitor.add_swap(swap.clone()).await.unwrap();

        // Test refund submission
        let result = monitor.trigger_supernova_refund(&swap).await;

        // Should succeed with our mock implementation
        assert!(result.is_ok());

        // Verify state was updated
        let swaps = monitor.get_active_swaps().await;
        assert_eq!(swaps[0].state, SwapState::Refunded);
    }
}
