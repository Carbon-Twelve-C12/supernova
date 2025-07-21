//! Cross-chain monitoring for atomic swaps
//! 
//! This module monitors both Bitcoin and Supernova blockchains for
//! HTLC-related events and automatically triggers claims when secrets are revealed.

use crate::atomic_swap::{
    SwapSession, SwapState, HTLCState, SupernovaHTLC, 
    BitcoinHTLCReference, AtomicSwapError
};
use crate::atomic_swap::error::MonitorError;
use crate::atomic_swap::bitcoin_adapter::{extract_secret_from_bitcoin_tx, BitcoinRpcClient};
use std::sync::Arc;
use tokio::sync::RwLock;
use std::collections::HashMap;
use futures::stream::{Stream, StreamExt};
use serde::{Serialize, Deserialize};

/// Cross-chain monitoring service
pub struct CrossChainMonitor {
    /// Active swap sessions being monitored
    active_swaps: Arc<RwLock<HashMap<[u8; 32], SwapSession>>>,
    
    /// Bitcoin RPC client (if feature enabled)
    #[cfg(feature = "atomic-swap")]
    bitcoin_client: Option<BitcoinRpcClient>,
    
    /// Supernova blockchain handle
    supernova_handle: Arc<RwLock<SupernovaHandle>>,
    
    /// Monitoring configuration
    config: MonitorConfig,
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

/// Handle to Supernova blockchain operations
pub struct SupernovaHandle {
    // Placeholder for actual blockchain handle
    current_height: u64,
}

/// Events emitted by the monitor
#[derive(Clone, Debug, Serialize, Deserialize)]
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
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Chain {
    Bitcoin,
    Supernova,
}

/// Swap amount information
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SwapAmounts {
    pub bitcoin_sats: u64,
    pub nova_units: u64,
}

/// Reason for refund
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum RefundReason {
    Timeout,
    UserRequested,
    CounterpartyFailure,
    NetworkError(String),
}

impl CrossChainMonitor {
    /// Create a new cross-chain monitor
    pub fn new(
        config: MonitorConfig,
        #[cfg(feature = "atomic-swap")] bitcoin_client: Option<BitcoinRpcClient>,
    ) -> Self {
        Self {
            active_swaps: Arc::new(RwLock::new(HashMap::new())),
            #[cfg(feature = "atomic-swap")]
            bitcoin_client,
            supernova_handle: Arc::new(RwLock::new(SupernovaHandle {
                current_height: 0,
            })),
            config,
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
            loop {
                // Check each active swap for Bitcoin events
                let swaps = self.active_swaps.read().await;
                for (swap_id, swap) in swaps.iter() {
                    if let Err(e) = self.check_bitcoin_htlc(swap).await {
                        log::error!("Error checking Bitcoin HTLC for swap {}: {:?}", 
                            hex::encode(swap_id), e);
                    }
                }
                drop(swaps);
                
                // Wait before next check
                tokio::time::sleep(tokio::time::Duration::from_secs(self.config.poll_interval)).await;
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
                    log::error!("Error checking Supernova HTLC for swap {}: {:?}", 
                        hex::encode(swap_id), e);
                }
            }
            drop(swaps);
            
            // Wait before next check
            tokio::time::sleep(tokio::time::Duration::from_secs(self.config.poll_interval)).await;
        }
    }
    
    /// Check Bitcoin HTLC status
    #[cfg(feature = "atomic-swap")]
    async fn check_bitcoin_htlc(&self, swap: &SwapSession) -> Result<(), MonitorError> {
        if let Some(client) = &self.bitcoin_client {
            // Get the Bitcoin transaction
            let tx = client.get_transaction(&swap.btc_htlc.txid).await
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
        let handle = self.supernova_handle.read().await;
        
        // Check if timeout has been reached for refund
        if swap.nova_htlc.is_expired() && self.config.auto_refund {
            self.trigger_supernova_refund(swap).await?;
        }
        
        Ok(())
    }
    
    /// Trigger automatic claim on Supernova
    async fn trigger_supernova_claim(&self, swap: &SwapSession, secret: [u8; 32]) -> Result<(), MonitorError> {
        log::info!("Triggering Supernova claim for swap {}", hex::encode(&swap.setup.swap_id));
        
        // Update swap state
        let mut swaps = self.active_swaps.write().await;
        if let Some(mut swap) = swaps.get_mut(&swap.setup.swap_id) {
            swap.state = SwapState::BitcoinClaimed;
        }
        
        // In a real implementation, we would:
        // 1. Create claim transaction for Supernova
        // 2. Sign with participant's key
        // 3. Broadcast to Supernova network
        
        Ok(())
    }
    
    /// Trigger automatic refund on Supernova
    async fn trigger_supernova_refund(&self, swap: &SwapSession) -> Result<(), MonitorError> {
        log::info!("Triggering Supernova refund for swap {}", hex::encode(&swap.setup.swap_id));
        
        // Update swap state
        let mut swaps = self.active_swaps.write().await;
        if let Some(mut swap) = swaps.get_mut(&swap.setup.swap_id) {
            swap.state = SwapState::Refunded;
        }
        
        // In a real implementation, we would:
        // 1. Create refund transaction for Supernova
        // 2. Sign with initiator's key
        // 3. Broadcast to Supernova network
        
        Ok(())
    }
    
    /// Get status of all active swaps
    pub async fn get_active_swaps(&self) -> Vec<SwapSummary> {
        let swaps = self.active_swaps.read().await;
        swaps.values()
            .map(|swap| SwapSummary {
                swap_id: swap.setup.swap_id,
                state: swap.state.clone(),
                bitcoin_amount: swap.setup.bitcoin_amount,
                nova_amount: swap.setup.nova_amount,
                created_at: swap.created_at,
            })
            .collect()
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
        use sha2::{Sha256, Digest};
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
    
    #[tokio::test]
    async fn test_monitor_creation() {
        let config = MonitorConfig::default();
        let monitor = CrossChainMonitor::new(
            config,
            #[cfg(feature = "atomic-swap")]
            None,
        );
        
        let swaps = monitor.get_active_swaps().await;
        assert!(swaps.is_empty());
    }
    
    #[tokio::test]
    async fn test_swap_management() {
        let config = MonitorConfig::default();
        let monitor = CrossChainMonitor::new(
            config,
            #[cfg(feature = "atomic-swap")]
            None,
        );
        
        // Create a dummy swap session
        let swap = SwapSession {
            setup: crate::atomic_swap::AtomicSwapSetup {
                swap_id: [1u8; 32],
                bitcoin_amount: 100000,
                nova_amount: 1000000,
                fee_distribution: crate::atomic_swap::FeeDistribution {
                    bitcoin_fee_payer: crate::atomic_swap::FeePayer::Sender,
                    nova_fee_payer: crate::atomic_swap::FeePayer::Recipient,
                },
                timeout_blocks: crate::atomic_swap::TimeoutConfig {
                    bitcoin_claim_timeout: 144,
                    supernova_claim_timeout: 100,
                    refund_safety_margin: 6,
                },
            },
            secret: Some([0x42; 32]),
            nova_htlc: SupernovaHTLC {
                htlc_id: [1u8; 32],
                initiator: crate::atomic_swap::htlc::ParticipantInfo {
                    pubkey: crate::crypto::MLDSAPublicKey::default(),
                    address: "nova1test".to_string(),
                    refund_address: None,
                },
                participant: crate::atomic_swap::htlc::ParticipantInfo {
                    pubkey: crate::crypto::MLDSAPublicKey::default(),
                    address: "nova1test2".to_string(),
                    refund_address: None,
                },
                hash_lock: crate::atomic_swap::crypto::HashLock::from_hash(
                    crate::atomic_swap::crypto::HashFunction::SHA256,
                    [0x42; 32],
                ),
                time_lock: crate::atomic_swap::htlc::TimeLock {
                    absolute_timeout: 1000000,
                    relative_timeout: 144,
                    grace_period: 6,
                },
                amount: 1000000,
                fee_structure: crate::atomic_swap::htlc::FeeStructure {
                    claim_fee: 1000,
                    refund_fee: 1000,
                    service_fee: None,
                },
                state: HTLCState::Funded,
                created_at: 0,
                bitcoin_tx_ref: None,
                memo: None,
            },
            btc_htlc: BitcoinHTLCReference {
                txid: "dummy".to_string(),
                vout: 0,
                script_pubkey: vec![],
                amount: 100000,
                timeout_height: 500000,
            },
            state: SwapState::Active,
            created_at: 0,
            updated_at: 0,
        };
        
        // Add swap
        monitor.add_swap(swap).await.unwrap();
        
        // Check it was added
        let swaps = monitor.get_active_swaps().await;
        assert_eq!(swaps.len(), 1);
        assert_eq!(swaps[0].swap_id, [1u8; 32]);
        
        // Remove swap
        monitor.remove_swap(&[1u8; 32]).await.unwrap();
        
        // Check it was removed
        let swaps = monitor.get_active_swaps().await;
        assert!(swaps.is_empty());
    }
} 