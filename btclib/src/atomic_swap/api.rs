//! RPC API interface for atomic swaps
//! 
//! This module provides the RPC methods for managing atomic swaps
//! between Bitcoin and Supernova blockchains.

use crate::atomic_swap::{
    AtomicSwapSetup, SwapSession, SwapState, SwapResult, 
    AtomicSwapError, SwapError
};
use crate::atomic_swap::monitor::SwapSummary;
use crate::atomic_swap::monitor::SwapEvent;
use async_trait::async_trait;
use serde::{Serialize, Deserialize};
use std::sync::Arc;
use tokio::sync::RwLock;
use std::collections::HashMap;

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

/// Implementation of the atomic swap RPC service
pub struct AtomicSwapRpcImpl {
    /// Active swap sessions
    swaps: Arc<RwLock<HashMap<[u8; 32], SwapSession>>>,
    
    /// Swap event history
    events: Arc<RwLock<HashMap<[u8; 32], Vec<SwapEvent>>>>,
    
    /// Configuration
    config: crate::atomic_swap::AtomicSwapConfig,
}

impl AtomicSwapRpcImpl {
    /// Create a new RPC implementation
    pub fn new(config: crate::atomic_swap::AtomicSwapConfig) -> Self {
        Self {
            swaps: Arc::new(RwLock::new(HashMap::new())),
            events: Arc::new(RwLock::new(HashMap::new())),
            config,
        }
    }
    
    /// Add an event to the history
    async fn add_event(&self, swap_id: [u8; 32], event: SwapEvent) {
        let mut events = self.events.write().await;
        events.entry(swap_id)
            .or_insert_with(Vec::new)
            .push(event);
    }
}

#[async_trait]
impl AtomicSwapRPC for AtomicSwapRpcImpl {
    async fn initiate_swap(&self, params: InitiateSwapParams) -> RpcResult<SwapSession> {
        // Validate parameters
        if params.bitcoin_amount < self.config.min_swap_amount_btc {
            return Err(RpcError {
                code: -32602,
                message: format!("Bitcoin amount too low. Minimum: {}", self.config.min_swap_amount_btc),
                data: None,
            });
        }
        
        if params.bitcoin_amount > self.config.max_swap_amount_btc {
            return Err(RpcError {
                code: -32602,
                message: format!("Bitcoin amount too high. Maximum: {}", self.config.max_swap_amount_btc),
                data: None,
            });
        }
        
        // Generate swap ID
        let swap_id = crate::atomic_swap::crypto::generate_secure_random_32();
        
        // Create timeout configuration
        let timeout_blocks = crate::atomic_swap::TimeoutConfig {
            bitcoin_claim_timeout: (params.timeout_minutes as u32 * 60) / 600, // ~10 min blocks
            supernova_claim_timeout: (params.timeout_minutes as u32 * 60) / 500 - 20, // Shorter for safety
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
            crate::atomic_swap::crypto::HashFunction::SHA256
        ).map_err(|e| RpcError {
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
                .as_secs() + (params.timeout_minutes as u64 * 60),
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
        ).map_err(|e| RpcError {
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
        
        // Add initiation event
        self.add_event(swap_id, SwapEvent::SwapInitiated {
            swap_id,
            initiator: "nova1initiator".to_string(),
            participant: params.nova_counterparty,
            amounts: crate::atomic_swap::monitor::SwapAmounts {
                bitcoin_sats: params.bitcoin_amount,
                nova_units: params.nova_amount,
            },
        }).await;
        
        Ok(session)
    }
    
    async fn get_swap_status(&self, swap_id: [u8; 32]) -> RpcResult<SwapStatus> {
        let swaps = self.swaps.read().await;
        let swap = swaps.get(&swap_id).ok_or_else(|| RpcError {
            code: -32602,
            message: "Swap not found".to_string(),
            data: None,
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
        if !swap.nova_htlc.hash_lock.verify_preimage(&secret).unwrap_or(false) {
            return Err(RpcError {
                code: -32602,
                message: "Invalid secret".to_string(),
                data: None,
            });
        }
        
        // Update state
        swap.state = SwapState::Claimed;
        swap.updated_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        
        // In a real implementation, we would broadcast the claim transaction
        Ok(TransactionId {
            txid: "dummy_claim_tx".to_string(),
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
        
        // Check if refund is allowed
        if !swap.nova_htlc.is_expired() {
            return Err(RpcError {
                code: -32602,
                message: "Swap has not expired yet".to_string(),
                data: None,
            });
        }
        
        // Update state
        swap.state = SwapState::Refunded;
        swap.updated_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        
        // In a real implementation, we would broadcast the refund transaction
        Ok(TransactionId {
            txid: "dummy_refund_tx".to_string(),
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
        let events = self.events.read().await;
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
        let nova_network_fee = 1000;    // 1000 base units
        let service_fee = None;         // No service fee for now
        
        Ok(FeeEstimate {
            bitcoin_network_fee,
            nova_network_fee,
            service_fee,
            total_fee_btc: bitcoin_network_fee,
            total_fee_nova: nova_network_fee,
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