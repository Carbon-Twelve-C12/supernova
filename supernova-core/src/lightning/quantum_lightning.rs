// Quantum-Secure Lightning Network Implementation for Supernova
// Integrates CRYSTALS-Dilithium signatures with Lightning channel operations

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

// Use our internal types instead of external crates
use crate::crypto::quantum::{verify_quantum_signature, QuantumKeyPair, QuantumScheme};
use crate::environmental::{
    carbon_tracking::CarbonTracker, renewable_validation::RenewableValidationResult,
};
use crate::types::transaction::Transaction;

/// Quantum-secure Lightning channel
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuantumLightningChannel {
    /// Channel ID
    pub channel_id: [u8; 32],

    /// Quantum key pairs for channel parties
    pub local_quantum_keys: QuantumKeyPair,
    pub remote_quantum_pubkey: Vec<u8>,

    /// Channel funding transaction
    pub funding_tx: Transaction,
    pub funding_outpoint: (String, u32),

    /// Channel capacity in satoshis
    pub capacity_sats: u64,

    /// Current channel state
    pub state: ChannelState,

    /// Environmental data
    pub environmental_data: ChannelEnvironmentalData,

    /// Quantum security parameters
    pub quantum_params: QuantumChannelParams,

    /// Channel metadata
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ChannelState {
    Pending,
    Active,
    Closing,
    Closed,
    QuantumSecured,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelEnvironmentalData {
    /// Node's renewable energy percentage
    pub renewable_percentage: f64,

    /// Carbon footprint per transaction
    pub carbon_per_tx: f64,

    /// Green mining certification
    pub green_certified: bool,

    /// Environmental score (0-100)
    pub environmental_score: f64,

    /// Carbon offset applied
    pub carbon_offset_applied: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuantumChannelParams {
    /// Quantum signature scheme used
    pub quantum_scheme: QuantumScheme,

    /// Security level (1-5)
    pub security_level: u8,

    /// Hybrid mode (classical + quantum)
    pub hybrid_mode: bool,

    /// Post-quantum HTLC enabled
    pub quantum_htlc_enabled: bool,
}

// ============================================================================
// SECURITY FIX (P1-001): Quantum HTLC Timeout Constants
// ============================================================================

/// Quantum HTLC timeout configuration
/// 
/// SECURITY: Quantum signatures take approximately 10ms to verify compared to
/// 1ms for ECDSA signatures. This 10x overhead requires additional safety buffers
/// to prevent HTLCs from timing out during verification, which could lead to
/// fund loss or griefing attacks.
pub struct QuantumHTLCConfig;

impl QuantumHTLCConfig {
    /// Quantum signature verification overhead in blocks
    /// 
    /// Assumptions:
    /// - Block time: ~10 minutes (600 seconds)
    /// - Quantum signature verification: ~10ms
    /// - Network propagation time: ~5 seconds
    /// - Safety margin: 100x (to account for network delays, node processing queues)
    /// 
    /// Result: 144 blocks (~24 hours) provides ample buffer
    pub const QUANTUM_SIG_VERIFICATION_BLOCKS: u32 = 144;
    
    /// Additional network propagation buffer for quantum signatures
    /// Quantum signatures are larger (~2.5KB vs ~71 bytes for ECDSA), requiring
    /// more time to propagate through the network
    pub const NETWORK_PROPAGATION_BUFFER: u32 = 72; // ~12 hours
    
    /// Total safety margin for quantum HTLCs
    pub const TOTAL_SAFETY_MARGIN: u32 = Self::QUANTUM_SIG_VERIFICATION_BLOCKS 
                                        + Self::NETWORK_PROPAGATION_BUFFER; // 216 blocks (~36 hours)
    
    /// Minimum HTLC timeout for quantum-secured channels
    /// BOLT-11 standard uses 40 blocks minimum for ECDSA
    /// For quantum, we require 288 blocks (~48 hours) minimum
    pub const MIN_HTLC_TIMEOUT_BLOCKS: u32 = 288; // 48 hours minimum
    
    /// Maximum HTLC timeout (prevents indefinite lock)
    pub const MAX_HTLC_TIMEOUT_BLOCKS: u32 = 2016; // ~2 weeks
}

/// Quantum Hash Time-Locked Contract (Q-HTLC)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuantumHTLC {
    /// HTLC ID
    pub htlc_id: [u8; 32],

    /// Amount in satoshis
    pub amount_sats: u64,

    /// Payment hash (quantum-resistant)
    pub payment_hash: [u8; 32],

    /// Quantum preimage commitment
    pub quantum_preimage_commitment: Vec<u8>,

    /// Expiry block height
    pub expiry_height: u32,

    /// Quantum signature for HTLC
    pub quantum_signature: Vec<u8>,

    /// Environmental impact
    pub carbon_footprint: f64,
}

impl QuantumHTLC {
    /// Calculate safe timeout for quantum HTLCs
    /// 
    /// SECURITY FIX (P1-001): Adds quantum signature verification buffer to prevent
    /// HTLCs from timing out during signature verification.
    ///
    /// # Arguments
    /// * `base_timeout` - Base timeout in blocks
    ///
    /// # Returns
    /// Safe timeout that accounts for quantum signature overhead
    pub fn calculate_safe_timeout(base_timeout: u32) -> u32 {
        base_timeout.saturating_add(QuantumHTLCConfig::TOTAL_SAFETY_MARGIN)
    }
    
    /// Validate HTLC timeout meets minimum quantum requirements
    /// 
    /// # Arguments
    /// * `timeout` - Proposed timeout in blocks
    ///
    /// # Returns
    /// * `Ok(())` - Timeout is safe
    /// * `Err(String)` - Timeout is too short or too long
    pub fn validate_timeout(timeout: u32) -> Result<(), String> {
        if timeout < QuantumHTLCConfig::MIN_HTLC_TIMEOUT_BLOCKS {
            return Err(format!(
                "HTLC timeout too short for quantum signatures: {} < {} (minimum)",
                timeout,
                QuantumHTLCConfig::MIN_HTLC_TIMEOUT_BLOCKS
            ));
        }
        
        if timeout > QuantumHTLCConfig::MAX_HTLC_TIMEOUT_BLOCKS {
            return Err(format!(
                "HTLC timeout too long: {} > {} (maximum)",
                timeout,
                QuantumHTLCConfig::MAX_HTLC_TIMEOUT_BLOCKS
            ));
        }
        
        Ok(())
    }
    
    /// Create a new quantum HTLC with validated timeout
    /// 
    /// SECURITY: Ensures timeout meets quantum signature requirements
    pub fn new(
        htlc_id: [u8; 32],
        amount_sats: u64,
        payment_hash: [u8; 32],
        quantum_preimage_commitment: Vec<u8>,
        base_expiry_height: u32,
        quantum_signature: Vec<u8>,
        carbon_footprint: f64,
    ) -> Result<Self, String> {
        // Calculate safe expiry with quantum buffer
        let safe_expiry = Self::calculate_safe_timeout(base_expiry_height);
        
        // Validate timeout
        Self::validate_timeout(safe_expiry)?;
        
        Ok(Self {
            htlc_id,
            amount_sats,
            payment_hash,
            quantum_preimage_commitment,
            expiry_height: safe_expiry,
            quantum_signature,
            carbon_footprint,
        })
    }
}

/// Green Lightning route for carbon-conscious payments
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GreenLightningRoute {
    /// Route hops
    pub hops: Vec<GreenRouteHop>,

    /// Total route capacity
    pub total_capacity_sats: u64,

    /// Total fees
    pub total_fees_sats: u64,

    /// Environmental metrics
    pub total_carbon_footprint: f64,
    pub average_renewable_percentage: f64,
    pub green_nodes_count: usize,

    /// Route score (considers fees and environmental impact)
    pub route_score: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GreenRouteHop {
    /// Node public key
    pub node_pubkey: Vec<u8>,

    /// Channel to use
    pub channel_id: [u8; 32],

    /// Hop fee
    pub fee_sats: u64,

    /// Environmental data
    pub renewable_percentage: f64,
    pub carbon_footprint: f64,
    pub green_certified: bool,
}

/// Quantum Lightning Network Manager
pub struct QuantumLightningManager {
    /// Quantum channels
    channels: Arc<RwLock<HashMap<[u8; 32], QuantumLightningChannel>>>,

    /// Active HTLCs
    htlcs: Arc<RwLock<HashMap<[u8; 32], QuantumHTLC>>>,

    /// Environmental tracker
    environmental_tracker: Arc<CarbonTracker>,

    /// Node's quantum keys
    node_quantum_keys: QuantumKeyPair,

    /// Green routing preferences
    routing_preferences: Arc<RwLock<RoutingPreferences>>,

    /// Performance metrics
    metrics: Arc<RwLock<LightningMetrics>>,
}

#[derive(Debug, Clone)]
struct RoutingPreferences {
    /// Prioritize green nodes
    pub prefer_green_nodes: bool,

    /// Maximum acceptable carbon footprint per hop
    pub max_carbon_per_hop: f64,

    /// Minimum renewable percentage required
    pub min_renewable_percentage: f64,

    /// Green node incentive multiplier
    pub green_incentive_multiplier: f64,
}

#[derive(Debug, Clone, Default)]
struct LightningMetrics {
    pub total_channels: u64,
    pub quantum_secured_channels: u64,
    pub total_payments: u64,
    pub green_payments: u64,
    pub total_carbon_saved: f64,
    pub average_renewable_percentage: f64,
}

impl QuantumLightningManager {
    /// Create new quantum Lightning manager
    pub fn new(
        node_quantum_keys: QuantumKeyPair,
        environmental_tracker: Arc<CarbonTracker>,
    ) -> Self {
        let default_preferences = RoutingPreferences {
            prefer_green_nodes: true,
            max_carbon_per_hop: 0.1, // kg CO2e
            min_renewable_percentage: 50.0,
            green_incentive_multiplier: 0.9, // 10% discount for green nodes
        };

        Self {
            channels: Arc::new(RwLock::new(HashMap::new())),
            htlcs: Arc::new(RwLock::new(HashMap::new())),
            environmental_tracker,
            node_quantum_keys,
            routing_preferences: Arc::new(RwLock::new(default_preferences)),
            metrics: Arc::new(RwLock::new(LightningMetrics::default())),
        }
    }

    /// Create quantum-secure Lightning channel
    pub async fn create_quantum_lightning_channel(
        &self,
        remote_quantum_pubkey: Vec<u8>,
        funding_tx: Transaction,
        funding_outpoint: (String, u32),
        capacity_sats: u64,
        environmental_cert: Option<RenewableValidationResult>,
    ) -> Result<QuantumLightningChannel, LightningError> {

        // Generate channel ID
        let channel_id = self.generate_channel_id(&funding_tx);

        // Validate quantum keys
        self.validate_quantum_channel_security(&remote_quantum_pubkey)?;

        // Calculate environmental data
        let environmental_data =
            self.calculate_channel_environmental_data(environmental_cert.as_ref());

        // Create quantum channel parameters
        let quantum_params = QuantumChannelParams {
            quantum_scheme: self.node_quantum_keys.parameters.scheme,
            security_level: self.node_quantum_keys.parameters.security_level,
            hybrid_mode: matches!(
                self.node_quantum_keys.parameters.scheme,
                QuantumScheme::Hybrid(_)
            ),
            quantum_htlc_enabled: true,
        };

        let channel = QuantumLightningChannel {
            channel_id,
            local_quantum_keys: self.node_quantum_keys.clone(),
            remote_quantum_pubkey,
            funding_tx,
            funding_outpoint,
            capacity_sats,
            state: ChannelState::Pending,
            environmental_data,
            quantum_params,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        // Store channel
        self.channels
            .write()
            .unwrap()
            .insert(channel_id, channel.clone());

        // Update metrics
        self.update_metrics_for_new_channel(&channel);


        Ok(channel)
    }

    /// Validate quantum channel security
    pub fn validate_quantum_channel_security(
        &self,
        remote_pubkey: &[u8],
    ) -> Result<(), LightningError> {

        // Verify key size is appropriate for quantum resistance
        let min_key_size = match self.node_quantum_keys.parameters.scheme {
            QuantumScheme::Dilithium => 2420,      // Dilithium2 public key size
            QuantumScheme::SphincsPlus => 1088,    // SPHINCS+ public key size
            QuantumScheme::Falcon => 897,          // Falcon-512 public key size
            QuantumScheme::Hybrid(_) => 2420 + 32, // Dilithium + classical
        };

        if remote_pubkey.len() < min_key_size {
            return Err(LightningError::InvalidQuantumKey(
                "Remote public key too small for quantum security".to_string(),
            ));
        }

        // Additional security checks

        Ok(())
    }

    /// Test quantum HTLC operations
    pub async fn test_quantum_htlc_operations(&self) -> Result<(), LightningError> {

        // Create test HTLC
        let test_amount = 100_000; // sats
        let test_preimage = b"quantum_test_preimage_supernova";

        // Create quantum HTLC
        let htlc = self.create_quantum_htlc_contract(
            test_amount,
            test_preimage,
            144, // ~1 day expiry
        )?;

        // Validate quantum HTLC security
        self.validate_quantum_htlc_security(&htlc)?;

        // Test payment routing
        let _test_route = self.test_quantum_payment_routing(test_amount).await?;

        // Benchmark performance
        let _perf_results = self.benchmark_quantum_lightning_performance().await?;


        Ok(())
    }

    /// Create quantum HTLC contract
    pub fn create_quantum_htlc_contract(
        &self,
        amount_sats: u64,
        preimage: &[u8],
        expiry_blocks: u32,
    ) -> Result<QuantumHTLC, LightningError> {
        // Generate payment hash
        let mut hasher = Sha256::new();
        hasher.update(preimage);
        let payment_hash: [u8; 32] = hasher.finalize().into();

        // Create quantum preimage commitment
        let quantum_commitment = self.create_quantum_preimage_commitment(preimage)?;

        // Sign HTLC with quantum key
        let htlc_data = self.serialize_htlc_data(amount_sats, &payment_hash, expiry_blocks);
        let quantum_signature = self
            .node_quantum_keys
            .sign(&htlc_data)
            .map_err(|e| LightningError::QuantumSignatureError(e.to_string()))?;

        // Calculate carbon footprint
        let carbon_footprint = self.calculate_htlc_carbon_footprint(amount_sats);

        let htlc = QuantumHTLC {
            htlc_id: self.generate_htlc_id(),
            amount_sats,
            payment_hash,
            quantum_preimage_commitment: quantum_commitment,
            expiry_height: expiry_blocks,
            quantum_signature,
            carbon_footprint,
        };

        // Store HTLC
        self.htlcs
            .write()
            .unwrap()
            .insert(htlc.htlc_id, htlc.clone());

        Ok(htlc)
    }

    /// Validate quantum HTLC security
    pub fn validate_quantum_htlc_security(&self, htlc: &QuantumHTLC) -> Result<(), LightningError> {
        // Verify quantum signature
        let htlc_data =
            self.serialize_htlc_data(htlc.amount_sats, &htlc.payment_hash, htlc.expiry_height);

        let valid = verify_quantum_signature(
            &self.node_quantum_keys.public_key,
            &htlc_data,
            &htlc.quantum_signature,
            self.node_quantum_keys.parameters,
        )
        .map_err(|e| LightningError::QuantumSignatureError(e.to_string()))?;

        if !valid {
            return Err(LightningError::InvalidQuantumSignature);
        }

        // Verify quantum preimage commitment
        if htlc.quantum_preimage_commitment.len() < 32 {
            return Err(LightningError::InvalidPreimageCommitment);
        }

        Ok(())
    }

    /// Test quantum payment routing
    pub async fn test_quantum_payment_routing(
        &self,
        amount_sats: u64,
    ) -> Result<GreenLightningRoute, LightningError> {
        // Find green route
        let route = self
            .find_green_payment_route(
                amount_sats,
                &self.node_quantum_keys.public_key,
                &[0u8; 33], // Test destination
            )
            .await?;


        Ok(route)
    }

    /// Benchmark quantum Lightning performance
    pub async fn benchmark_quantum_lightning_performance(
        &self,
    ) -> Result<PerformanceBenchmark, LightningError> {
        let iterations = 100;
        let mut total_time = std::time::Duration::new(0, 0);

        for _ in 0..iterations {
            let start = std::time::Instant::now();

            // Benchmark quantum signature
            let test_data = b"benchmark_payment_data";
            let _ = self
                .node_quantum_keys
                .sign(test_data)
                .map_err(|e| LightningError::QuantumSignatureError(e.to_string()))?;

            total_time += start.elapsed();
        }

        let avg_time_ms = total_time.as_secs_f64() * 1000.0 / iterations as f64;

        Ok(PerformanceBenchmark {
            iterations,
            avg_operation_time_ms: avg_time_ms,
            operations_per_second: 1000.0 / avg_time_ms,
        })
    }

    /// Calculate Lightning carbon footprint
    pub fn calculate_lightning_carbon_footprint(&self, payment_amount_sats: u64) -> f64 {
        // Base carbon footprint per Lightning transaction (kg CO2e)
        let base_carbon = 0.0001; // Much lower than on-chain

        // Scale slightly with payment size
        let size_factor = (payment_amount_sats as f64 / 1_000_000.0).sqrt();

        base_carbon * (1.0 + size_factor * 0.1)
    }

    /// Apply green Lightning incentives
    pub fn apply_green_lightning_incentives(
        &self,
        base_fee_sats: u64,
        node_renewable_percentage: f64,
    ) -> u64 {
        let prefs = self.routing_preferences.read().unwrap();

        if node_renewable_percentage >= prefs.min_renewable_percentage {
            // Apply green discount
            let discount_factor = prefs.green_incentive_multiplier;
            let discounted_fee = (base_fee_sats as f64 * discount_factor) as u64;


            discounted_fee
        } else {
            base_fee_sats
        }
    }

    /// Track environmental Lightning metrics
    pub fn track_environmental_lightning_metrics(&self) -> EnvironmentalLightningMetrics {
        let channels = self.channels.read().unwrap();
        let metrics = self.metrics.read().unwrap();

        let total_renewable_percentage: f64 = channels
            .values()
            .map(|c| c.environmental_data.renewable_percentage)
            .sum::<f64>()
            / channels.len().max(1) as f64;

        let carbon_negative_channels = channels
            .values()
            .filter(|c| c.environmental_data.carbon_offset_applied)
            .count();

        EnvironmentalLightningMetrics {
            total_channels: channels.len(),
            green_certified_channels: channels
                .values()
                .filter(|c| c.environmental_data.green_certified)
                .count(),
            carbon_negative_channels,
            average_renewable_percentage: total_renewable_percentage,
            total_carbon_saved: metrics.total_carbon_saved,
            green_payment_percentage: if metrics.total_payments > 0 {
                (metrics.green_payments as f64 / metrics.total_payments as f64) * 100.0
            } else {
                0.0
            },
        }
    }

    /// Validate carbon negative payments
    pub fn validate_carbon_negative_payments(
        &self,
        payment_amount_sats: u64,
        route: &GreenLightningRoute,
    ) -> Result<bool, LightningError> {
        // Calculate payment carbon footprint
        let _payment_carbon = self.calculate_lightning_carbon_footprint(payment_amount_sats);

        // Check if route is carbon negative
        let carbon_negative = route.total_carbon_footprint < 0.0
            || (route.average_renewable_percentage >= 100.0
                && route.green_nodes_count == route.hops.len());

        if carbon_negative {
        }

        Ok(carbon_negative)
    }

    /// Find green payment route
    pub async fn find_green_payment_route(
        &self,
        amount_sats: u64,
        _source: &[u8],
        _destination: &[u8],
    ) -> Result<GreenLightningRoute, LightningError> {
        // Simulate route finding with environmental optimization
        // In production, this would integrate with actual Lightning routing

        let mock_hops = vec![
            GreenRouteHop {
                node_pubkey: vec![1u8; 33],
                channel_id: [1u8; 32],
                fee_sats: 10,
                renewable_percentage: 100.0,
                carbon_footprint: -0.001, // Carbon negative!
                green_certified: true,
            },
            GreenRouteHop {
                node_pubkey: vec![2u8; 33],
                channel_id: [2u8; 32],
                fee_sats: 15,
                renewable_percentage: 85.0,
                carbon_footprint: 0.0001,
                green_certified: true,
            },
        ];

        let total_carbon: f64 = mock_hops.iter().map(|h| h.carbon_footprint).sum();
        let avg_renewable = mock_hops
            .iter()
            .map(|h| h.renewable_percentage)
            .sum::<f64>()
            / mock_hops.len() as f64;
        let green_count = mock_hops.iter().filter(|h| h.green_certified).count();

        Ok(GreenLightningRoute {
            hops: mock_hops,
            total_capacity_sats: amount_sats,
            total_fees_sats: 25,
            total_carbon_footprint: total_carbon,
            average_renewable_percentage: avg_renewable,
            green_nodes_count: green_count,
            route_score: 95.0, // High score for green route
        })
    }

    // Helper methods

    fn generate_channel_id(&self, funding_tx: &Transaction) -> [u8; 32] {
        let mut hasher = Sha256::new();
        // Use transaction hash method
        hasher.update(funding_tx.hash());
        hasher.update(Utc::now().timestamp().to_le_bytes());
        hasher.finalize().into()
    }

    fn generate_htlc_id(&self) -> [u8; 32] {
        let mut hasher = Sha256::new();
        hasher.update(&self.node_quantum_keys.public_key);
        hasher.update(Utc::now().timestamp_nanos_opt().unwrap_or(0).to_le_bytes());
        hasher.finalize().into()
    }

    fn calculate_channel_environmental_data(
        &self,
        cert: Option<&RenewableValidationResult>,
    ) -> ChannelEnvironmentalData {
        if let Some(cert) = cert {
            ChannelEnvironmentalData {
                renewable_percentage: cert.renewable_percentage,
                carbon_per_tx: if cert.is_carbon_negative {
                    -0.001
                } else {
                    0.0001
                },
                green_certified: cert.green_mining_score >= 80.0,
                environmental_score: cert.green_mining_score,
                carbon_offset_applied: cert.is_carbon_negative,
            }
        } else {
            // Default environmental data
            ChannelEnvironmentalData {
                renewable_percentage: 0.0,
                carbon_per_tx: 0.001,
                green_certified: false,
                environmental_score: 50.0,
                carbon_offset_applied: false,
            }
        }
    }

    fn create_quantum_preimage_commitment(
        &self,
        preimage: &[u8],
    ) -> Result<Vec<u8>, LightningError> {
        // Create commitment by signing the preimage with quantum key
        let commitment_data = [preimage, &self.node_quantum_keys.public_key].concat();

        self.node_quantum_keys
            .sign(&commitment_data)
            .map_err(|e| LightningError::QuantumSignatureError(e.to_string()))
    }

    fn serialize_htlc_data(&self, amount: u64, hash: &[u8], expiry: u32) -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(&amount.to_le_bytes());
        data.extend_from_slice(hash);
        data.extend_from_slice(&expiry.to_le_bytes());
        data
    }

    fn calculate_htlc_carbon_footprint(&self, amount_sats: u64) -> f64 {
        self.calculate_lightning_carbon_footprint(amount_sats)
    }

    fn update_metrics_for_new_channel(&self, channel: &QuantumLightningChannel) {
        let mut metrics = self.metrics.write().unwrap();
        metrics.total_channels += 1;

        if matches!(channel.state, ChannelState::QuantumSecured) {
            metrics.quantum_secured_channels += 1;
        }

        // Update average renewable percentage
        let old_total = metrics.average_renewable_percentage * (metrics.total_channels - 1) as f64;
        metrics.average_renewable_percentage = (old_total
            + channel.environmental_data.renewable_percentage)
            / metrics.total_channels as f64;
    }
}

/// Lightning error types
#[derive(Debug, Clone)]
pub enum LightningError {
    InvalidQuantumKey(String),
    QuantumSignatureError(String),
    InvalidQuantumSignature,
    InvalidPreimageCommitment,
    ChannelNotFound,
    InsufficientCapacity,
    RoutingError(String),
}

/// Performance benchmark results
#[derive(Debug, Clone)]
pub struct PerformanceBenchmark {
    pub iterations: usize,
    pub avg_operation_time_ms: f64,
    pub operations_per_second: f64,
}

/// Environmental Lightning metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvironmentalLightningMetrics {
    pub total_channels: usize,
    pub green_certified_channels: usize,
    pub carbon_negative_channels: usize,
    pub average_renewable_percentage: f64,
    pub total_carbon_saved: f64,
    pub green_payment_percentage: f64,
}

/// Public API functions

pub async fn create_quantum_lightning_channel(
    manager: &QuantumLightningManager,
    remote_quantum_pubkey: Vec<u8>,
    funding_tx: Transaction,
    funding_outpoint: (String, u32),
    capacity_sats: u64,
    environmental_cert: Option<RenewableValidationResult>,
) -> Result<QuantumLightningChannel, LightningError> {
    manager
        .create_quantum_lightning_channel(
            remote_quantum_pubkey,
            funding_tx,
            funding_outpoint,
            capacity_sats,
            environmental_cert,
        )
        .await
}

pub fn validate_quantum_channel_security(
    manager: &QuantumLightningManager,
    remote_pubkey: &[u8],
) -> Result<(), LightningError> {
    manager.validate_quantum_channel_security(remote_pubkey)
}

pub async fn test_quantum_htlc_operations(
    manager: &QuantumLightningManager,
) -> Result<(), LightningError> {
    manager.test_quantum_htlc_operations().await
}

pub fn calculate_lightning_carbon_footprint(
    manager: &QuantumLightningManager,
    payment_amount_sats: u64,
) -> f64 {
    manager.calculate_lightning_carbon_footprint(payment_amount_sats)
}

pub fn track_environmental_lightning_metrics(
    manager: &QuantumLightningManager,
) -> EnvironmentalLightningMetrics {
    manager.track_environmental_lightning_metrics()
}
