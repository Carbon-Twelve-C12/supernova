//! RPC API for atomic swap operations

use crate::atomic_swap::cache::{AtomicSwapCache, CacheConfig};
use crate::atomic_swap::error::{AtomicSwapError, HTLCError};
use crate::atomic_swap::metrics::{
    record_error, record_swap_state_transition, RpcTimer, SWAPS_INITIATED,
};
use crate::atomic_swap::monitor::{CrossChainMonitor, MonitorConfig, SwapEvent, SwapSummary};
use crate::atomic_swap::{
    AtomicSwapSetup, BitcoinHTLCReference, HTLCState, SupernovaHTLC, SwapCompletion, SwapSession,
    SwapState,
};

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// RPC result type
pub type RpcResult<T> = Result<T, RpcError>;

/// RPC error type
#[derive(Debug, Serialize, Deserialize)]
pub struct RpcError {
    pub code: i32,
    pub message: String,
    pub data: Option<serde_json::Value>,
}

impl From<AtomicSwapError> for RpcError {
    fn from(err: AtomicSwapError) -> Self {
        RpcError {
            code: -32000, // Generic server error
            message: err.to_string(),
            data: None,
        }
    }
}

/// Parameters for initiating a swap
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct InitiateSwapParams {
    /// Amount of Bitcoin to swap (in satoshis)
    pub bitcoin_amount: u64,

    /// Amount of Supernova to swap (in base units)
    pub nova_amount: u64,

    /// Bitcoin address of the counterparty
    pub bitcoin_counterparty: String,

    /// Supernova address of the counterparty
    pub nova_counterparty: String,

    /// Timeout for the swap (in minutes)
    pub timeout_minutes: u32,

    /// Optional memo
    pub memo: Option<String>,
}

/// Filter for listing swaps
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct SwapFilter {
    /// Filter by state
    pub state: Option<SwapState>,

    /// Filter by minimum amount (BTC)
    pub min_amount_btc: Option<u64>,

    /// Filter by maximum amount (BTC)
    pub max_amount_btc: Option<u64>,

    /// Filter by counterparty address
    pub counterparty: Option<String>,

    /// Maximum number of results
    pub limit: Option<usize>,
}

/// Transaction ID returned by claim/refund operations
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TransactionId {
    pub txid: String,
    pub chain: String,
}

/// Swap status information
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SwapStatus {
    pub swap_id: String,
    pub state: SwapState,
    pub bitcoin_amount: u64,
    pub nova_amount: u64,
    pub created_at: u64,
    pub updated_at: u64,
    pub bitcoin_confirmations: u32,
    pub nova_confirmations: u32,
    pub can_claim: bool,
    pub can_refund: bool,
    pub timeout_at: u64,
    pub bitcoin_htlc_address: String,
    pub nova_htlc_id: String,
    pub events: Vec<SwapEvent>,
}

/// Atomic swap RPC trait
#[async_trait]
pub trait AtomicSwapRPC: Send + Sync {
    /// Initiate a new atomic swap
    async fn initiate_swap(&self, params: InitiateSwapParams) -> RpcResult<SwapSession>;

    /// Get swap status
    async fn get_swap_status(&self, swap_id: [u8; 32]) -> RpcResult<SwapStatus>;

    /// Claim funds from HTLC
    async fn claim_swap(&self, swap_id: [u8; 32], secret: [u8; 32]) -> RpcResult<TransactionId>;

    /// Refund expired HTLC
    async fn refund_swap(&self, swap_id: [u8; 32]) -> RpcResult<TransactionId>;

    /// List active swaps
    async fn list_swaps(&self, filter: SwapFilter) -> RpcResult<Vec<SwapSummary>>;

    /// Cancel a swap (if possible)
    async fn cancel_swap(&self, swap_id: [u8; 32]) -> RpcResult<bool>;

    /// Get swap events
    async fn get_swap_events(&self, swap_id: [u8; 32], limit: usize) -> RpcResult<Vec<SwapEvent>>;

    /// Estimate fees for a swap
    async fn estimate_swap_fees(&self, params: InitiateSwapParams) -> RpcResult<FeeEstimate>;

    // Phase 4: Privacy features
    #[cfg(feature = "atomic-swap")]
    /// Create a confidential atomic swap
    async fn initiate_confidential_swap(
        &self,
        params: ConfidentialSwapParams,
    ) -> RpcResult<ConfidentialSwapInfo>;

    #[cfg(feature = "atomic-swap")]
    /// Create a zero-knowledge swap
    async fn initiate_zk_swap(&self, params: ZKSwapParams) -> RpcResult<ZKSwapInfo>;

    #[cfg(feature = "atomic-swap")]
    /// Verify a confidential swap proof
    async fn verify_confidential_swap(&self, swap_id: [u8; 32]) -> RpcResult<bool>;

    #[cfg(feature = "atomic-swap")]
    /// Get privacy metrics for swaps
    async fn get_privacy_metrics(&self) -> RpcResult<PrivacyMetrics>;
}

/// Fee estimate for a swap
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FeeEstimate {
    pub bitcoin_network_fee: u64,
    pub nova_network_fee: u64,
    pub service_fee: Option<u64>,
    pub total_fee_btc: u64,
    pub total_fee_nova: u64,
}

/// Parameters for confidential swaps
#[cfg(feature = "atomic-swap")]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ConfidentialSwapParams {
    /// Base swap parameters
    pub base_params: InitiateSwapParams,

    /// Minimum amount (public)
    pub min_amount: u64,

    /// Maximum amount (public)
    pub max_amount: u64,

    /// Enable amount hiding
    pub hide_amounts: bool,

    /// Enable participant hiding
    pub hide_participants: bool,
}

/// Confidential swap information
#[cfg(feature = "atomic-swap")]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ConfidentialSwapInfo {
    /// Swap ID
    pub swap_id: [u8; 32],

    /// Amount commitment (if amounts hidden)
    pub amount_commitment: Option<String>,

    /// Range proof
    pub range_proof: Option<String>,

    /// Base swap info
    pub base_info: SwapStatus,
}

/// Parameters for zero-knowledge swaps
#[cfg(feature = "atomic-swap")]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ZKSwapParams {
    /// Base swap parameters
    pub base_params: InitiateSwapParams,

    /// Enable validity proof
    pub prove_validity: bool,

    /// Enable amount range proof
    pub prove_amount_range: bool,

    /// Enable preimage knowledge proof
    pub prove_preimage_knowledge: bool,
}

/// Zero-knowledge swap information
#[cfg(feature = "atomic-swap")]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ZKSwapInfo {
    /// Swap ID
    pub swap_id: [u8; 32],

    /// Validity proof
    pub validity_proof: Option<String>,

    /// Range proof
    pub range_proof: Option<String>,

    /// Preimage proof
    pub preimage_proof: Option<String>,

    /// Base swap info
    pub base_info: SwapStatus,
}

/// Privacy metrics
#[cfg(feature = "atomic-swap")]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PrivacyMetrics {
    /// Total confidential swaps
    pub total_confidential_swaps: u64,

    /// Total ZK swaps
    pub total_zk_swaps: u64,

    /// Average proof generation time
    pub avg_proof_generation_ms: f64,

    /// Average proof verification time
    pub avg_proof_verification_ms: f64,

    /// Privacy adoption rate
    pub privacy_adoption_rate: f64,
}

/// RPC implementation for atomic swaps
pub struct AtomicSwapRpcImpl {
    config: crate::atomic_swap::AtomicSwapConfig,
    swaps: Arc<RwLock<HashMap<[u8; 32], SwapSession>>>,
    event_history: Arc<RwLock<HashMap<[u8; 32], Vec<SwapEvent>>>>,
    monitor: Arc<CrossChainMonitor>,
    #[cfg(feature = "atomic-swap")]
    bitcoin_client: Option<Arc<crate::atomic_swap::bitcoin_adapter::BitcoinRpcClient>>,
    cache: Arc<AtomicSwapCache>,
}

impl AtomicSwapRpcImpl {
    /// Create a new RPC instance
    pub fn new(
        config: crate::atomic_swap::AtomicSwapConfig,
        monitor: Arc<CrossChainMonitor>,
        #[cfg(feature = "atomic-swap")] bitcoin_client: Option<
            Arc<crate::atomic_swap::bitcoin_adapter::BitcoinRpcClient>,
        >,
    ) -> Self {
        let cache_config = CacheConfig::default();
        let cache = Arc::new(AtomicSwapCache::new(cache_config));

        Self {
            config,
            swaps: Arc::new(RwLock::new(HashMap::new())),
            event_history: Arc::new(RwLock::new(HashMap::new())),
            monitor,
            #[cfg(feature = "atomic-swap")]
            bitcoin_client,
            cache,
        }
    }

    /// Add an event to the history
    async fn add_event(&self, swap_id: [u8; 32], event: SwapEvent) {
        let mut events = self.event_history.write().await;
        events.entry(swap_id).or_insert_with(Vec::new).push(event);
    }

    /// Generate a unique swap ID
    fn generate_swap_id() -> [u8; 32] {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        let mut id = [0u8; 32];
        rng.fill(&mut id);
        id
    }
}

#[async_trait]
impl AtomicSwapRPC for AtomicSwapRpcImpl {
    async fn initiate_swap(&self, params: InitiateSwapParams) -> RpcResult<SwapSession> {
        let _timer = RpcTimer::start("initiate_swap");

        // Validate parameters
        if params.bitcoin_amount < self.config.min_swap_amount_btc {
            return Err(RpcError {
                code: -32602,
                message: format!(
                    "Bitcoin amount too low. Minimum: {}",
                    self.config.min_swap_amount_btc
                ),
                data: None,
            });
        }

        if params.bitcoin_amount > self.config.max_swap_amount_btc {
            return Err(RpcError {
                code: -32602,
                message: format!(
                    "Bitcoin amount too high. Maximum: {}",
                    self.config.max_swap_amount_btc
                ),
                data: None,
            });
        }

        // Generate swap ID
        let swap_id = crate::atomic_swap::crypto::generate_secure_random_32();

        // Create timeout configuration
        let timeout_blocks = crate::atomic_swap::TimeoutConfig {
            bitcoin_claim_timeout: (params.timeout_minutes as u32 * 60) / 600, // ~10 min blocks
            supernova_claim_timeout: (params.timeout_minutes as u32 * 60) / 500, // Shorter for safety
            refund_safety_margin: 6,
        };

        // Create swap setup
        let setup = AtomicSwapSetup {
            swap_id,
            bitcoin_amount: params.bitcoin_amount,
            nova_amount: params.nova_amount,
            fee_distribution: crate::atomic_swap::FeeDistribution {
                bitcoin_fee_payer: crate::atomic_swap::FeePayer::Split(50),
                nova_fee_payer: crate::atomic_swap::FeePayer::Split(50),
            },
            timeout_blocks,
        };

        // Create hash lock
        let hash_lock = crate::atomic_swap::crypto::HashLock::new(
            crate::atomic_swap::crypto::HashFunction::SHA256,
        )
        .map_err(|e| RpcError {
            code: -32603,
            message: format!("Failed to create hash lock: {}", e),
            data: None,
        })?;

        // Create participant info (placeholder - would use actual keys)
        let initiator = crate::atomic_swap::htlc::ParticipantInfo {
            pubkey: crate::crypto::MLDSAPublicKey::default(),
            address: "nova1initiator".to_string(),
            refund_address: None,
        };

        let participant = crate::atomic_swap::htlc::ParticipantInfo {
            pubkey: crate::crypto::MLDSAPublicKey::default(),
            address: params.nova_counterparty.clone(),
            refund_address: None,
        };

        // Create time lock
        let time_lock = crate::atomic_swap::htlc::TimeLock {
            absolute_timeout: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs()
                + (params.timeout_minutes as u64 * 60),
            relative_timeout: timeout_blocks.supernova_claim_timeout,
            grace_period: 6,
        };

        // Create fee structure
        let fee_structure = crate::atomic_swap::htlc::FeeStructure {
            claim_fee: 1000,
            refund_fee: 1000,
            service_fee: None,
        };

        // Create HTLC
        let nova_htlc = crate::atomic_swap::htlc::SupernovaHTLC::new(
            initiator,
            participant,
            hash_lock.clone(),
            time_lock,
            params.nova_amount,
            fee_structure,
        )
        .map_err(|e| RpcError {
            code: -32603,
            message: format!("Failed to create HTLC: {}", e),
            data: None,
        })?;

        // Create swap session
        let session = SwapSession {
            setup,
            secret: hash_lock.preimage,
            nova_htlc,
            btc_htlc: crate::atomic_swap::BitcoinHTLCReference {
                txid: "pending".to_string(),
                vout: 0,
                script_pubkey: vec![],
                amount: params.bitcoin_amount,
                timeout_height: 0,
                address: "pending".to_string(),
            },
            state: SwapState::Initializing,
            created_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            updated_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        };

        // Store swap
        let mut swaps = self.swaps.write().await;
        swaps.insert(swap_id, session.clone());

        // Add swap to monitor for cross-chain monitoring
        self.monitor.add_swap(session.clone()).await.map_err(|e| {
            record_error("monitor_add_swap");
            RpcError {
                code: -32603,
                message: format!("Failed to add swap to monitor: {}", e),
                data: None,
            }
        })?;

        // Record metrics
        SWAPS_INITIATED.inc();
        record_swap_state_transition("", "Active");

        // Cache the session
        self.cache
            .cache_swap_session(swap_id, session.clone())
            .await;

        // Add initiation event
        self.add_event(
            swap_id,
            SwapEvent::SwapInitiated {
                swap_id,
                initiator: "nova1initiator".to_string(),
                participant: params.nova_counterparty,
                amounts: crate::atomic_swap::monitor::SwapAmounts {
                    bitcoin_sats: params.bitcoin_amount,
                    nova_units: params.nova_amount,
                },
            },
        )
        .await;

        Ok(session)
    }

    async fn get_swap_status(&self, swap_id: [u8; 32]) -> RpcResult<SwapStatus> {
        let _timer = RpcTimer::start("get_swap_status");

        // Check cache first
        if let Some(cached) = self.cache.get_swap_session(&swap_id).await {
            return Ok(SwapStatus {
                swap_id: hex::encode(&swap_id),
                state: cached.state.clone(),
                bitcoin_amount: cached.setup.bitcoin_amount,
                nova_amount: cached.setup.nova_amount,
                created_at: cached.created_at,
                updated_at: cached.updated_at,
                bitcoin_confirmations: 0, // Would query actual confirmations
                nova_confirmations: 0,
                can_claim: cached.state == SwapState::Active && cached.secret.is_some(),
                can_refund: cached.nova_htlc.is_expired(),
                timeout_at: cached.nova_htlc.time_lock.absolute_timeout,
                bitcoin_htlc_address: cached.btc_htlc.address.clone(),
                nova_htlc_id: hex::encode(&cached.nova_htlc.htlc_id),
                events: vec![], // Would fetch from event history
            });
        }

        // Not in cache, check storage
        let swaps = self.swaps.read().await;
        let swap = swaps.get(&swap_id).ok_or_else(|| {
            record_error("swap_not_found");
            RpcError {
                code: -32602,
                message: "Swap not found".to_string(),
                data: None,
            }
        })?;

        let current_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        Ok(SwapStatus {
            swap_id: hex::encode(&swap_id),
            state: swap.state.clone(),
            bitcoin_amount: swap.setup.bitcoin_amount,
            nova_amount: swap.setup.nova_amount,
            created_at: swap.created_at,
            updated_at: swap.updated_at,
            bitcoin_confirmations: 0, // Would query actual confirmations
            nova_confirmations: 0,    // Would query actual confirmations
            can_claim: swap.state == SwapState::Active && swap.secret.is_some(),
            can_refund: swap.nova_htlc.is_expired(),
            timeout_at: swap.nova_htlc.time_lock.absolute_timeout,
            bitcoin_htlc_address: swap.btc_htlc.address.clone(),
            nova_htlc_id: hex::encode(&swap.nova_htlc.htlc_id),
            events: vec![], // Would fetch from event history
        })
    }

    async fn claim_swap(&self, swap_id: [u8; 32], secret: [u8; 32]) -> RpcResult<TransactionId> {
        let mut swaps = self.swaps.write().await;
        let swap = swaps.get_mut(&swap_id).ok_or_else(|| RpcError {
            code: -32602,
            message: "Swap not found".to_string(),
            data: None,
        })?;

        // Verify secret
        if !swap
            .nova_htlc
            .hash_lock
            .verify_preimage(&secret)
            .unwrap_or(false)
        {
            return Err(RpcError {
                code: -32602,
                message: "Invalid secret".to_string(),
                data: None,
            });
        }

        // Update state
        swap.state = SwapState::Claimed;
        swap.secret = Some(secret);
        swap.updated_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        // Add claim event
        self.add_event(
            swap_id,
            SwapEvent::SwapCompleted {
                swap_id,
                btc_claim_tx: format!("btc_claim_{}", hex::encode(&swap_id[..8])),
                nova_claim_tx: format!("nova_claim_{}", hex::encode(&swap_id[..8])),
                duration_seconds: 0, // TODO: Calculate actual duration
            },
        )
        .await;

        // In production, this would broadcast the actual claim transaction
        let tx_id = format!("nova_claim_{}", hex::encode(&swap_id[..8]));

        Ok(TransactionId {
            txid: tx_id,
            chain: "supernova".to_string(),
        })
    }

    async fn refund_swap(&self, swap_id: [u8; 32]) -> RpcResult<TransactionId> {
        let mut swaps = self.swaps.write().await;
        let swap = swaps.get_mut(&swap_id).ok_or_else(|| RpcError {
            code: -32602,
            message: "Swap not found".to_string(),
            data: None,
        })?;

        // Comprehensive rollback validation
        
        // Validation 1: Check if refund is allowed (timelock expired)
        if !swap.nova_htlc.is_expired() {
            return Err(RpcError {
                code: -32602,
                message: format!(
                    "Swap has not expired yet. Timeout at block {}, current state: {:?}",
                    swap.nova_htlc.timeout_height,
                    swap.state
                ),
                data: Some(serde_json::json!({
                    "timeout_height": swap.nova_htlc.timeout_height,
                    "current_state": format!("{:?}", swap.state),
                })),
            });
        }
        
        // Validation 2: Check swap is in refundable state
        if !matches!(swap.state, SwapState::NovaFunded | SwapState::BothFunded | SwapState::Active | SwapState::Failed(_)) {
            return Err(RpcError {
                code: -32602,
                message: format!(
                    "Swap in state {:?} cannot be refunded",
                    swap.state
                ),
                data: None,
            });
        }
        
        // Validation 3: Verify swap hasn't already been refunded
        if matches!(swap.state, SwapState::Refunded | SwapState::Completed) {
            return Err(RpcError {
                code: -32602,
                message: format!("Swap already in final state: {:?}", swap.state),
                data: None,
            });
        }

        // TODO (Production): Actual refund implementation would:
        // 1. Generate refund transaction spending HTLC back to initiator
        // 2. Sign refund transaction with initiator's key
        // 3. Broadcast refund transaction to Supernova network
        // 4. Wait for confirmation
        // 5. Update UTXO set to unlock funds
        // 6. Trigger Bitcoin refund if applicable
        
        // For now, update state with comprehensive tracking
        swap.state = SwapState::Refunded;
        swap.updated_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        // Log refund for audit trail
        log::info!(
            "Swap {} refunded after timeout. HTLC funds should be returned to {}",
            hex::encode(&swap_id),
            swap.nova_htlc.initiator.address
        );
        
        // SECURITY: Return clear indication this is a stub
        // Production deployment must implement actual refund transaction
        Ok(TransactionId {
            txid: format!("STUB_refund_{}", hex::encode(&swap_id[..8])),
            chain: "supernova".to_string(),
        })
    }

    async fn list_swaps(&self, filter: SwapFilter) -> RpcResult<Vec<SwapSummary>> {
        let swaps = self.swaps.read().await;
        let mut results: Vec<SwapSummary> = swaps
            .values()
            .filter(|swap| {
                // Apply filters
                if let Some(state) = &filter.state {
                    if swap.state != *state {
                        return false;
                    }
                }

                if let Some(min) = filter.min_amount_btc {
                    if swap.setup.bitcoin_amount < min {
                        return false;
                    }
                }

                if let Some(max) = filter.max_amount_btc {
                    if swap.setup.bitcoin_amount > max {
                        return false;
                    }
                }

                true
            })
            .map(|swap| SwapSummary {
                swap_id: swap.setup.swap_id,
                state: swap.state.clone(),
                bitcoin_amount: swap.setup.bitcoin_amount,
                nova_amount: swap.setup.nova_amount,
                created_at: swap.created_at,
            })
            .collect();

        // Apply limit
        if let Some(limit) = filter.limit {
            results.truncate(limit);
        }

        Ok(results)
    }

    async fn cancel_swap(&self, swap_id: [u8; 32]) -> RpcResult<bool> {
        let mut swaps = self.swaps.write().await;
        let swap = swaps.get_mut(&swap_id).ok_or_else(|| RpcError {
            code: -32602,
            message: "Swap not found".to_string(),
            data: None,
        })?;

        // Can only cancel if in initializing state
        if swap.state != SwapState::Initializing {
            return Ok(false);
        }

        swap.state = SwapState::Failed("Cancelled by user".to_string());
        Ok(true)
    }

    async fn get_swap_events(&self, swap_id: [u8; 32], limit: usize) -> RpcResult<Vec<SwapEvent>> {
        let events = self.event_history.read().await;
        let swap_events = events.get(&swap_id).ok_or_else(|| RpcError {
            code: -32602,
            message: "No events found for swap".to_string(),
            data: None,
        })?;

        let mut result = swap_events.clone();
        result.reverse(); // Most recent first
        result.truncate(limit);

        Ok(result)
    }

    async fn estimate_swap_fees(&self, params: InitiateSwapParams) -> RpcResult<FeeEstimate> {
        // Simple fee estimation
        let bitcoin_network_fee = 2000; // ~2000 sats for HTLC transactions
        let nova_network_fee = 1000; // 1000 base units
        let service_fee = None; // No service fee for now

        Ok(FeeEstimate {
            bitcoin_network_fee,
            nova_network_fee,
            service_fee,
            total_fee_btc: bitcoin_network_fee,
            total_fee_nova: nova_network_fee,
        })
    }

    #[cfg(feature = "atomic-swap")]
    async fn initiate_confidential_swap(
        &self,
        params: ConfidentialSwapParams,
    ) -> RpcResult<ConfidentialSwapInfo> {
        use crate::atomic_swap::confidential::{make_swap_confidential, ConfidentialSwapBuilder};

        // First create a regular swap
        let swap_session = self.initiate_swap(params.base_params).await?;

        // Convert to confidential
        let confidential_session =
            make_swap_confidential(swap_session.clone(), params.min_amount, params.max_amount)
                .await
                .map_err(|e| RpcError {
                    code: -32000,
                    message: format!("Failed to create confidential swap: {:?}", e),
                    data: None,
                })?;

        // Get commitment info
        let amount_commitment = if params.hide_amounts {
            Some(hex::encode(
                confidential_session
                    .confidential_nova_htlc
                    .amount_commitment
                    .as_bytes(),
            ))
        } else {
            None
        };

        let range_proof = Some(hex::encode(
            &confidential_session.confidential_nova_htlc.range_proof,
        ));

        Ok(ConfidentialSwapInfo {
            swap_id: swap_session.setup.swap_id,
            amount_commitment,
            range_proof,
            base_info: SwapStatus {
                swap_id: hex::encode(swap_session.setup.swap_id),
                state: swap_session.state.clone(),
                bitcoin_amount: swap_session.setup.bitcoin_amount,
                nova_amount: swap_session.setup.nova_amount,
                created_at: swap_session.created_at,
                updated_at: swap_session.updated_at,
                bitcoin_confirmations: 0,
                nova_confirmations: 0,
                can_claim: swap_session.state == SwapState::Active && swap_session.secret.is_some(),
                can_refund: swap_session.nova_htlc.is_expired(),
                timeout_at: swap_session.nova_htlc.time_lock.absolute_timeout,
                bitcoin_htlc_address: swap_session.btc_htlc.address.clone(),
                nova_htlc_id: hex::encode(&swap_session.nova_htlc.htlc_id),
                events: vec![],
            },
        })
    }

    #[cfg(feature = "atomic-swap")]
    async fn initiate_zk_swap(&self, params: ZKSwapParams) -> RpcResult<ZKSwapInfo> {
        use crate::atomic_swap::zk_swap::create_zk_swap_session;

        // First create a regular swap
        let swap_session = self.initiate_swap(params.base_params).await?;

        // Convert to ZK swap
        let zk_session = create_zk_swap_session(swap_session.clone())
            .await
            .map_err(|e| RpcError {
                code: -32000,
                message: format!("Failed to create ZK swap: {:?}", e),
                data: None,
            })?;

        // Extract proof info
        let validity_proof = zk_session.validity_proof.map(|p| hex::encode(&p.proof));
        let range_proof = zk_session.range_proof.map(|p| hex::encode(&p.proof));
        let preimage_proof = zk_session.preimage_proof.map(|p| hex::encode(&p.proof));

        Ok(ZKSwapInfo {
            swap_id: swap_session.setup.swap_id,
            validity_proof,
            range_proof,
            preimage_proof,
            base_info: SwapStatus {
                swap_id: hex::encode(swap_session.setup.swap_id),
                state: swap_session.state.clone(),
                bitcoin_amount: swap_session.setup.bitcoin_amount,
                nova_amount: swap_session.setup.nova_amount,
                created_at: swap_session.created_at,
                updated_at: swap_session.updated_at,
                bitcoin_confirmations: 0,
                nova_confirmations: 0,
                can_claim: swap_session.state == SwapState::Active && swap_session.secret.is_some(),
                can_refund: swap_session.nova_htlc.is_expired(),
                timeout_at: swap_session.nova_htlc.time_lock.absolute_timeout,
                bitcoin_htlc_address: swap_session.btc_htlc.address.clone(),
                nova_htlc_id: hex::encode(&swap_session.nova_htlc.htlc_id),
                events: vec![],
            },
        })
    }

    #[cfg(feature = "atomic-swap")]
    async fn verify_confidential_swap(&self, swap_id: [u8; 32]) -> RpcResult<bool> {
        // Placeholder - would verify the confidential proofs
        Ok(true)
    }

    #[cfg(feature = "atomic-swap")]
    async fn get_privacy_metrics(&self) -> RpcResult<PrivacyMetrics> {
        // Placeholder metrics
        Ok(PrivacyMetrics {
            total_confidential_swaps: 0,
            total_zk_swaps: 0,
            avg_proof_generation_ms: 150.0,
            avg_proof_verification_ms: 25.0,
            privacy_adoption_rate: 0.0,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_swap_lifecycle() {
        let config = crate::atomic_swap::AtomicSwapConfig::default();
        let rpc = AtomicSwapRpcImpl::new(config);

        // Initiate swap
        let params = InitiateSwapParams {
            bitcoin_amount: 100000,
            nova_amount: 1000000,
            bitcoin_counterparty: "bc1qtest".to_string(),
            nova_counterparty: "nova1test".to_string(),
            timeout_minutes: 60,
            memo: Some("Test swap".to_string()),
        };

        let session = rpc.initiate_swap(params).await.unwrap();
        assert_eq!(session.state, SwapState::Initializing);

        // Get status
        let status = rpc.get_swap_status(session.setup.swap_id).await.unwrap();
        assert_eq!(status.state, SwapState::Initializing);

        // List swaps
        let swaps = rpc.list_swaps(SwapFilter::default()).await.unwrap();
        assert_eq!(swaps.len(), 1);
    }
}
