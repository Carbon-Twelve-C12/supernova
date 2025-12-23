//! Multi-Path Payments (MPP) for Lightning Network
//!
//! This module implements Base AMP (Atomic Multi-Path Payments) as specified in
//! BOLT #4 (Onion Routing) and enables splitting payments across multiple paths.
//!
//! # Benefits
//! - Larger payments than any single channel capacity
//! - Better liquidity utilization
//! - Improved privacy through payment splitting
//! - Higher success rates for large payments
//!
//! # Architecture
//! - `MultiPathPaymentCoordinator` - Orchestrates payment splitting
//! - `PaymentShard` - Individual payment part
//! - `MultiPathConfig` - Configuration for splitting strategy

use super::router::{NodeId, PaymentPath};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use thiserror::Error;
use tracing::{debug, error, info, warn};

/// Default minimum shard size (in millinova)
pub const DEFAULT_MIN_SHARD_SIZE_MNOVA: u64 = 10_000; // 10 sat minimum

/// Default maximum number of shards
pub const DEFAULT_MAX_SHARDS: usize = 8;

/// Maximum time to wait for all shards to complete
pub const DEFAULT_MPP_TIMEOUT_SECS: u64 = 60;

/// Multi-path payment errors
#[derive(Debug, Error, Clone)]
pub enum MultiPathError {
    #[error("No paths found for payment")]
    NoPathsFound,

    #[error("Insufficient total capacity: need {needed}, have {available}")]
    InsufficientCapacity { needed: u64, available: u64 },

    #[error("Shard {shard_id} failed: {reason}")]
    ShardFailed { shard_id: String, reason: String },

    #[error("Payment timeout after {elapsed_secs} seconds")]
    Timeout { elapsed_secs: u64 },

    #[error("Partial payment: {completed} of {total} shards completed")]
    PartialPayment { completed: usize, total: usize },

    #[error("Payment already in progress: {payment_hash}")]
    PaymentInProgress { payment_hash: String },

    #[error("Invalid payment hash")]
    InvalidPaymentHash,

    #[error("Configuration error: {0}")]
    ConfigError(String),

    #[error("Routing error: {0}")]
    RoutingError(String),

    #[error("Lock poisoned")]
    LockPoisoned,
}

/// Result type for multi-path operations
pub type MultiPathResult<T> = Result<T, MultiPathError>;

/// Payment splitting strategy
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SplitStrategy {
    /// Split equally among all available paths
    Equal,
    /// Split proportionally to path capacity
    ProportionalToCapacity,
    /// Split to minimize total fees
    MinimizeFees,
    /// Prefer fewer, larger shards
    MinimizeShards,
    /// Use as many paths as possible for privacy
    MaximizePrivacy,
}

impl Default for SplitStrategy {
    fn default() -> Self {
        Self::ProportionalToCapacity
    }
}

/// Configuration for multi-path payments
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultiPathConfig {
    /// Minimum shard size in millinova
    pub min_shard_size_mnova: u64,
    /// Maximum number of shards
    pub max_shards: usize,
    /// Timeout for complete payment in seconds
    pub timeout_secs: u64,
    /// Splitting strategy
    pub split_strategy: SplitStrategy,
    /// Allow overpayment to round shards
    pub allow_overpayment: bool,
    /// Maximum overpayment percentage (basis points, e.g., 100 = 1%)
    pub max_overpayment_bps: u16,
    /// Retry failed shards
    pub retry_failed_shards: bool,
    /// Maximum retries per shard
    pub max_retries: u8,
}

impl Default for MultiPathConfig {
    fn default() -> Self {
        Self {
            min_shard_size_mnova: DEFAULT_MIN_SHARD_SIZE_MNOVA,
            max_shards: DEFAULT_MAX_SHARDS,
            timeout_secs: DEFAULT_MPP_TIMEOUT_SECS,
            split_strategy: SplitStrategy::ProportionalToCapacity,
            allow_overpayment: true,
            max_overpayment_bps: 100, // 1% max overpayment
            retry_failed_shards: true,
            max_retries: 3,
        }
    }
}

/// State of a payment shard
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ShardState {
    /// Shard is pending
    Pending,
    /// Shard has been sent
    InFlight,
    /// Shard completed successfully
    Completed,
    /// Shard failed
    Failed,
    /// Shard is being retried
    Retrying,
}

/// Individual payment shard
#[derive(Debug, Clone)]
pub struct PaymentShard {
    /// Unique shard identifier
    pub shard_id: [u8; 32],
    /// Payment hash (same for all shards)
    pub payment_hash: [u8; 32],
    /// Amount in millinova
    pub amount_mnova: u64,
    /// Path for this shard
    pub path: PaymentPath,
    /// Current state
    pub state: ShardState,
    /// Creation time
    pub created_at: u64,
    /// Completion time (if completed)
    pub completed_at: Option<u64>,
    /// Preimage (if completed)
    pub preimage: Option<[u8; 32]>,
    /// Error message (if failed)
    pub error: Option<String>,
    /// Retry count
    pub retry_count: u8,
}

impl PaymentShard {
    /// Create a new payment shard
    pub fn new(payment_hash: [u8; 32], amount_mnova: u64, path: PaymentPath) -> Self {
        use sha2::{Digest, Sha256};

        // Generate unique shard ID
        let mut hasher = Sha256::new();
        hasher.update(&payment_hash);
        hasher.update(&amount_mnova.to_le_bytes());
        hasher.update(&SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or(Duration::ZERO)
            .as_nanos()
            .to_le_bytes());
        let hash = hasher.finalize();
        let mut shard_id = [0u8; 32];
        shard_id.copy_from_slice(&hash);

        Self {
            shard_id,
            payment_hash,
            amount_mnova,
            path,
            state: ShardState::Pending,
            created_at: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or(Duration::ZERO)
                .as_secs(),
            completed_at: None,
            preimage: None,
            error: None,
            retry_count: 0,
        }
    }

    /// Mark shard as in-flight
    pub fn mark_in_flight(&mut self) {
        self.state = ShardState::InFlight;
    }

    /// Mark shard as completed
    pub fn mark_completed(&mut self, preimage: [u8; 32]) {
        self.state = ShardState::Completed;
        self.preimage = Some(preimage);
        self.completed_at = Some(
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or(Duration::ZERO)
                .as_secs(),
        );
    }

    /// Mark shard as failed
    pub fn mark_failed(&mut self, error: String) {
        self.state = ShardState::Failed;
        self.error = Some(error);
    }

    /// Prepare for retry
    pub fn prepare_retry(&mut self) {
        self.state = ShardState::Retrying;
        self.retry_count += 1;
        self.error = None;
    }
}

/// Multi-path payment tracking
#[derive(Debug, Clone)]
pub struct MultiPathPayment {
    /// Payment hash
    pub payment_hash: [u8; 32],
    /// Destination node
    pub destination: NodeId,
    /// Total amount in millinova
    pub total_amount_mnova: u64,
    /// Payment shards
    pub shards: Vec<PaymentShard>,
    /// Start time
    pub started_at: u64,
    /// Configuration used
    pub config: MultiPathConfig,
    /// Is payment complete
    pub is_complete: bool,
    /// Total fees paid
    pub total_fees_mnova: u64,
}

impl MultiPathPayment {
    /// Create a new multi-path payment
    pub fn new(
        payment_hash: [u8; 32],
        destination: NodeId,
        total_amount_mnova: u64,
        config: MultiPathConfig,
    ) -> Self {
        Self {
            payment_hash,
            destination,
            total_amount_mnova,
            shards: Vec::new(),
            started_at: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or(Duration::ZERO)
                .as_secs(),
            config,
            is_complete: false,
            total_fees_mnova: 0,
        }
    }

    /// Add a shard to the payment
    pub fn add_shard(&mut self, shard: PaymentShard) {
        self.shards.push(shard);
    }

    /// Get completed shards
    pub fn completed_shards(&self) -> Vec<&PaymentShard> {
        self.shards.iter().filter(|s| s.state == ShardState::Completed).collect()
    }

    /// Get pending shards
    pub fn pending_shards(&self) -> Vec<&PaymentShard> {
        self.shards.iter().filter(|s| s.state == ShardState::Pending).collect()
    }

    /// Get failed shards
    pub fn failed_shards(&self) -> Vec<&PaymentShard> {
        self.shards.iter().filter(|s| s.state == ShardState::Failed).collect()
    }

    /// Calculate progress percentage
    pub fn progress_percentage(&self) -> f64 {
        if self.shards.is_empty() {
            return 0.0;
        }
        let completed: u64 = self.completed_shards().iter().map(|s| s.amount_mnova).sum();
        (completed as f64 / self.total_amount_mnova as f64) * 100.0
    }

    /// Check if all shards are complete
    pub fn all_shards_complete(&self) -> bool {
        self.shards.iter().all(|s| s.state == ShardState::Completed)
    }

    /// Check if payment timed out
    pub fn is_timed_out(&self) -> bool {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or(Duration::ZERO)
            .as_secs();
        now.saturating_sub(self.started_at) > self.config.timeout_secs
    }

    /// Get payment preimage (if all shards complete)
    pub fn get_preimage(&self) -> Option<[u8; 32]> {
        if self.all_shards_complete() {
            // All shards should have the same preimage
            self.shards.first().and_then(|s| s.preimage)
        } else {
            None
        }
    }
}

/// Path with capacity information for splitting
#[derive(Debug, Clone)]
pub struct CapacityPath {
    /// The payment path
    pub path: PaymentPath,
    /// Estimated available capacity
    pub available_capacity: u64,
    /// Minimum capacity along the path
    pub min_hop_capacity: u64,
    /// Total fees for maximum capacity
    pub total_fees: u64,
}

/// Multi-path payment coordinator
pub struct MultiPathPaymentCoordinator {
    /// Configuration
    config: MultiPathConfig,
    /// Active payments
    active_payments: Arc<RwLock<HashMap<[u8; 32], MultiPathPayment>>>,
    /// Completed payments (for history)
    completed_payments: Arc<RwLock<Vec<MultiPathPayment>>>,
    /// Our node ID
    our_node_id: NodeId,
}

impl MultiPathPaymentCoordinator {
    /// Create a new coordinator
    pub fn new(our_node_id: NodeId, config: MultiPathConfig) -> Self {
        Self {
            config,
            active_payments: Arc::new(RwLock::new(HashMap::new())),
            completed_payments: Arc::new(RwLock::new(Vec::new())),
            our_node_id,
        }
    }

    /// Create with default configuration
    pub fn with_defaults(our_node_id: NodeId) -> Self {
        Self::new(our_node_id, MultiPathConfig::default())
    }

    /// Plan a multi-path payment
    pub fn plan_payment(
        &self,
        payment_hash: [u8; 32],
        destination: NodeId,
        amount_mnova: u64,
        available_paths: Vec<CapacityPath>,
    ) -> MultiPathResult<MultiPathPayment> {
        // Check if payment already in progress
        {
            let active = self.active_payments.read().map_err(|_| MultiPathError::LockPoisoned)?;
            if active.contains_key(&payment_hash) {
                return Err(MultiPathError::PaymentInProgress {
                    payment_hash: hex::encode(payment_hash),
                });
            }
        }

        // Filter paths with sufficient capacity
        let viable_paths: Vec<&CapacityPath> = available_paths
            .iter()
            .filter(|p| p.available_capacity >= self.config.min_shard_size_mnova)
            .collect();

        if viable_paths.is_empty() {
            return Err(MultiPathError::NoPathsFound);
        }

        // Calculate total available capacity
        let total_capacity: u64 = viable_paths.iter().map(|p| p.available_capacity).sum();
        if total_capacity < amount_mnova {
            return Err(MultiPathError::InsufficientCapacity {
                needed: amount_mnova,
                available: total_capacity,
            });
        }

        // Create payment
        let mut payment = MultiPathPayment::new(
            payment_hash,
            destination,
            amount_mnova,
            self.config.clone(),
        );

        // Split the payment according to strategy
        let shards = self.split_payment(amount_mnova, &viable_paths, payment_hash)?;

        for shard in shards {
            payment.add_shard(shard);
        }

        // Calculate total fees
        payment.total_fees_mnova = payment.shards.iter()
            .map(|s| s.path.total_fee_mnova)
            .sum();

        info!(
            "Planned multi-path payment: {} shards, {} mnova total, {} mnova fees",
            payment.shards.len(),
            payment.total_amount_mnova,
            payment.total_fees_mnova
        );

        Ok(payment)
    }

    /// Split payment into shards
    fn split_payment(
        &self,
        amount_mnova: u64,
        paths: &[&CapacityPath],
        payment_hash: [u8; 32],
    ) -> MultiPathResult<Vec<PaymentShard>> {
        let mut shards = Vec::new();
        let mut remaining = amount_mnova;

        match self.config.split_strategy {
            SplitStrategy::Equal => {
                self.split_equal(amount_mnova, paths, payment_hash, &mut shards)?;
            }
            SplitStrategy::ProportionalToCapacity => {
                self.split_proportional(amount_mnova, paths, payment_hash, &mut shards)?;
            }
            SplitStrategy::MinimizeFees => {
                self.split_minimize_fees(amount_mnova, paths, payment_hash, &mut shards)?;
            }
            SplitStrategy::MinimizeShards => {
                self.split_minimize_shards(amount_mnova, paths, payment_hash, &mut shards)?;
            }
            SplitStrategy::MaximizePrivacy => {
                self.split_maximize_privacy(amount_mnova, paths, payment_hash, &mut shards)?;
            }
        }

        // Verify total
        let total_shard_amount: u64 = shards.iter().map(|s| s.amount_mnova).sum();
        if total_shard_amount < amount_mnova {
            // Need to add remaining to last shard or create new one
            if let Some(last) = shards.last_mut() {
                last.amount_mnova += amount_mnova - total_shard_amount;
            }
        }

        Ok(shards)
    }

    /// Split payment equally
    fn split_equal(
        &self,
        amount_mnova: u64,
        paths: &[&CapacityPath],
        payment_hash: [u8; 32],
        shards: &mut Vec<PaymentShard>,
    ) -> MultiPathResult<()> {
        let num_paths = std::cmp::min(paths.len(), self.config.max_shards);
        let shard_amount = amount_mnova / num_paths as u64;

        if shard_amount < self.config.min_shard_size_mnova {
            return Err(MultiPathError::ConfigError(
                "Equal split would create shards below minimum size".to_string(),
            ));
        }

        for (i, path) in paths.iter().take(num_paths).enumerate() {
            let amount = if i == num_paths - 1 {
                // Last shard gets remainder
                amount_mnova - shard_amount * (num_paths - 1) as u64
            } else {
                shard_amount
            };

            shards.push(PaymentShard::new(payment_hash, amount, path.path.clone()));
        }

        Ok(())
    }

    /// Split proportionally to capacity
    fn split_proportional(
        &self,
        amount_mnova: u64,
        paths: &[&CapacityPath],
        payment_hash: [u8; 32],
        shards: &mut Vec<PaymentShard>,
    ) -> MultiPathResult<()> {
        let total_capacity: u64 = paths.iter().map(|p| p.available_capacity).sum();
        let mut remaining = amount_mnova;

        for (i, path) in paths.iter().enumerate() {
            if i >= self.config.max_shards || remaining == 0 {
                break;
            }

            let proportion = path.available_capacity as f64 / total_capacity as f64;
            let mut shard_amount = (amount_mnova as f64 * proportion) as u64;

            // Ensure minimum size
            if shard_amount < self.config.min_shard_size_mnova && remaining >= self.config.min_shard_size_mnova {
                shard_amount = self.config.min_shard_size_mnova;
            }

            // Don't exceed remaining
            shard_amount = std::cmp::min(shard_amount, remaining);

            // Don't exceed path capacity
            shard_amount = std::cmp::min(shard_amount, path.available_capacity);

            if shard_amount > 0 {
                shards.push(PaymentShard::new(payment_hash, shard_amount, path.path.clone()));
                remaining = remaining.saturating_sub(shard_amount);
            }
        }

        Ok(())
    }

    /// Split to minimize fees
    fn split_minimize_fees(
        &self,
        amount_mnova: u64,
        paths: &[&CapacityPath],
        payment_hash: [u8; 32],
        shards: &mut Vec<PaymentShard>,
    ) -> MultiPathResult<()> {
        // Sort paths by fee rate (lowest first)
        let mut sorted_paths: Vec<_> = paths.to_vec();
        sorted_paths.sort_by(|a, b| {
            let fee_rate_a = if a.available_capacity > 0 {
                a.total_fees as f64 / a.available_capacity as f64
            } else {
                f64::MAX
            };
            let fee_rate_b = if b.available_capacity > 0 {
                b.total_fees as f64 / b.available_capacity as f64
            } else {
                f64::MAX
            };
            fee_rate_a.partial_cmp(&fee_rate_b).unwrap_or(std::cmp::Ordering::Equal)
        });

        let mut remaining = amount_mnova;

        for path in sorted_paths.iter().take(self.config.max_shards) {
            if remaining == 0 {
                break;
            }

            let shard_amount = std::cmp::min(remaining, path.available_capacity);
            if shard_amount >= self.config.min_shard_size_mnova {
                shards.push(PaymentShard::new(payment_hash, shard_amount, path.path.clone()));
                remaining = remaining.saturating_sub(shard_amount);
            }
        }

        Ok(())
    }

    /// Split to minimize number of shards
    fn split_minimize_shards(
        &self,
        amount_mnova: u64,
        paths: &[&CapacityPath],
        payment_hash: [u8; 32],
        shards: &mut Vec<PaymentShard>,
    ) -> MultiPathResult<()> {
        // Sort paths by capacity (highest first)
        let mut sorted_paths: Vec<_> = paths.to_vec();
        sorted_paths.sort_by(|a, b| b.available_capacity.cmp(&a.available_capacity));

        let mut remaining = amount_mnova;

        for path in sorted_paths.iter() {
            if remaining == 0 {
                break;
            }

            let shard_amount = std::cmp::min(remaining, path.available_capacity);
            if shard_amount >= self.config.min_shard_size_mnova {
                shards.push(PaymentShard::new(payment_hash, shard_amount, path.path.clone()));
                remaining = remaining.saturating_sub(shard_amount);
            }
        }

        Ok(())
    }

    /// Split to maximize privacy (use many paths)
    fn split_maximize_privacy(
        &self,
        amount_mnova: u64,
        paths: &[&CapacityPath],
        payment_hash: [u8; 32],
        shards: &mut Vec<PaymentShard>,
    ) -> MultiPathResult<()> {
        // Use as many paths as possible with equal amounts
        let num_paths = paths.len();
        let shard_amount = amount_mnova / num_paths as u64;

        if shard_amount < self.config.min_shard_size_mnova {
            // Fall back to proportional
            return self.split_proportional(amount_mnova, paths, payment_hash, shards);
        }

        let mut remaining = amount_mnova;
        for (i, path) in paths.iter().enumerate() {
            if remaining == 0 {
                break;
            }

            let amount = if i == num_paths - 1 {
                remaining
            } else {
                std::cmp::min(shard_amount, path.available_capacity)
            };

            if amount >= self.config.min_shard_size_mnova {
                shards.push(PaymentShard::new(payment_hash, amount, path.path.clone()));
                remaining = remaining.saturating_sub(amount);
            }
        }

        Ok(())
    }

    /// Start a planned payment
    pub fn start_payment(&self, payment: MultiPathPayment) -> MultiPathResult<()> {
        let payment_hash = payment.payment_hash;

        let mut active = self.active_payments.write().map_err(|_| MultiPathError::LockPoisoned)?;

        if active.contains_key(&payment_hash) {
            return Err(MultiPathError::PaymentInProgress {
                payment_hash: hex::encode(payment_hash),
            });
        }

        active.insert(payment_hash, payment);
        info!("Started multi-path payment: {}", hex::encode(payment_hash));
        Ok(())
    }

    /// Update shard status
    pub fn update_shard(
        &self,
        payment_hash: [u8; 32],
        shard_id: [u8; 32],
        state: ShardState,
        preimage: Option<[u8; 32]>,
        error: Option<String>,
    ) -> MultiPathResult<()> {
        let mut active = self.active_payments.write().map_err(|_| MultiPathError::LockPoisoned)?;

        let payment = active.get_mut(&payment_hash).ok_or(MultiPathError::InvalidPaymentHash)?;

        let shard = payment.shards.iter_mut()
            .find(|s| s.shard_id == shard_id)
            .ok_or(MultiPathError::ShardFailed {
                shard_id: hex::encode(shard_id),
                reason: "Shard not found".to_string(),
            })?;

        match state {
            ShardState::Completed => {
                if let Some(preimage) = preimage {
                    shard.mark_completed(preimage);
                    debug!("Shard {} completed", hex::encode(&shard_id[..8]));
                }
            }
            ShardState::Failed => {
                shard.mark_failed(error.unwrap_or_default());
                warn!("Shard {} failed", hex::encode(&shard_id[..8]));

                // Check if we should retry
                if payment.config.retry_failed_shards
                    && shard.retry_count < payment.config.max_retries
                {
                    shard.prepare_retry();
                }
            }
            ShardState::InFlight => {
                shard.mark_in_flight();
            }
            _ => {}
        }

        // Check if payment is complete
        if payment.all_shards_complete() {
            payment.is_complete = true;
            info!("Multi-path payment {} complete!", hex::encode(payment_hash));
        }

        Ok(())
    }

    /// Get payment status
    pub fn get_payment(&self, payment_hash: [u8; 32]) -> MultiPathResult<Option<MultiPathPayment>> {
        let active = self.active_payments.read().map_err(|_| MultiPathError::LockPoisoned)?;
        Ok(active.get(&payment_hash).cloned())
    }

    /// Complete a payment (move to history)
    pub fn complete_payment(&self, payment_hash: [u8; 32]) -> MultiPathResult<MultiPathPayment> {
        let mut active = self.active_payments.write().map_err(|_| MultiPathError::LockPoisoned)?;

        let payment = active.remove(&payment_hash).ok_or(MultiPathError::InvalidPaymentHash)?;

        let mut completed = self.completed_payments.write().map_err(|_| MultiPathError::LockPoisoned)?;
        completed.push(payment.clone());

        // Keep only last 100 completed
        if completed.len() > 100 {
            completed.drain(0..10);
        }

        Ok(payment)
    }

    /// Check for timed out payments
    pub fn check_timeouts(&self) -> MultiPathResult<Vec<[u8; 32]>> {
        let active = self.active_payments.read().map_err(|_| MultiPathError::LockPoisoned)?;

        let timed_out: Vec<[u8; 32]> = active
            .iter()
            .filter(|(_, p)| p.is_timed_out())
            .map(|(h, _)| *h)
            .collect();

        for hash in &timed_out {
            warn!("Payment {} timed out", hex::encode(hash));
        }

        Ok(timed_out)
    }

    /// Get statistics
    pub fn get_stats(&self) -> MultiPathResult<MultiPathStats> {
        let active = self.active_payments.read().map_err(|_| MultiPathError::LockPoisoned)?;
        let completed = self.completed_payments.read().map_err(|_| MultiPathError::LockPoisoned)?;

        let active_shards: usize = active.values().map(|p| p.shards.len()).sum();
        let completed_payments = completed.len();
        let total_amount: u64 = completed.iter().map(|p| p.total_amount_mnova).sum();
        let total_fees: u64 = completed.iter().map(|p| p.total_fees_mnova).sum();

        Ok(MultiPathStats {
            active_payments: active.len(),
            active_shards,
            completed_payments,
            total_amount_paid_mnova: total_amount,
            total_fees_paid_mnova: total_fees,
        })
    }

    /// Get configuration
    pub fn config(&self) -> &MultiPathConfig {
        &self.config
    }
}

/// Multi-path payment statistics
#[derive(Debug, Clone)]
pub struct MultiPathStats {
    pub active_payments: usize,
    pub active_shards: usize,
    pub completed_payments: usize,
    pub total_amount_paid_mnova: u64,
    pub total_fees_paid_mnova: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_path(capacity: u64, fee: u64) -> CapacityPath {
        CapacityPath {
            path: PaymentPath::new(),
            available_capacity: capacity,
            min_hop_capacity: capacity,
            total_fees: fee,
        }
    }

    #[test]
    fn test_payment_creation() {
        let coordinator = MultiPathPaymentCoordinator::with_defaults(NodeId::new("test".to_string()));

        let payment_hash = [1u8; 32];
        let destination = NodeId::new("dest".to_string());
        let amount = 100_000;

        let paths = vec![
            create_test_path(50_000, 100),
            create_test_path(50_000, 150),
            create_test_path(30_000, 80),
        ];

        let result = coordinator.plan_payment(payment_hash, destination, amount, paths);
        assert!(result.is_ok());

        let payment = result.unwrap();
        assert!(!payment.shards.is_empty());

        let total: u64 = payment.shards.iter().map(|s| s.amount_mnova).sum();
        assert!(total >= amount);
    }

    #[test]
    fn test_insufficient_capacity() {
        let coordinator = MultiPathPaymentCoordinator::with_defaults(NodeId::new("test".to_string()));

        let payment_hash = [2u8; 32];
        let destination = NodeId::new("dest".to_string());
        let amount = 200_000;

        let paths = vec![
            create_test_path(50_000, 100),
            create_test_path(50_000, 150),
        ];

        let result = coordinator.plan_payment(payment_hash, destination, amount, paths);
        assert!(matches!(result, Err(MultiPathError::InsufficientCapacity { .. })));
    }

    #[test]
    fn test_equal_split() {
        let mut config = MultiPathConfig::default();
        config.split_strategy = SplitStrategy::Equal;
        let coordinator = MultiPathPaymentCoordinator::new(NodeId::new("test".to_string()), config);

        let payment_hash = [3u8; 32];
        let destination = NodeId::new("dest".to_string());
        let amount = 90_000;

        let paths = vec![
            create_test_path(50_000, 100),
            create_test_path(50_000, 100),
            create_test_path(50_000, 100),
        ];

        let payment = coordinator.plan_payment(payment_hash, destination, amount, paths).unwrap();
        assert_eq!(payment.shards.len(), 3);

        // Each shard should be roughly equal
        for shard in &payment.shards {
            assert!(shard.amount_mnova >= 29_000 && shard.amount_mnova <= 31_000);
        }
    }

    #[test]
    fn test_shard_state_transitions() {
        let mut shard = PaymentShard::new([1u8; 32], 10_000, PaymentPath::new());
        assert_eq!(shard.state, ShardState::Pending);

        shard.mark_in_flight();
        assert_eq!(shard.state, ShardState::InFlight);

        shard.mark_completed([2u8; 32]);
        assert_eq!(shard.state, ShardState::Completed);
        assert!(shard.preimage.is_some());
        assert!(shard.completed_at.is_some());
    }

    #[test]
    fn test_payment_tracking() {
        let coordinator = MultiPathPaymentCoordinator::with_defaults(NodeId::new("test".to_string()));

        let payment_hash = [4u8; 32];
        let destination = NodeId::new("dest".to_string());
        let amount = 50_000;

        let paths = vec![
            create_test_path(50_000, 100),
        ];

        let payment = coordinator.plan_payment(payment_hash, destination, amount, paths).unwrap();
        coordinator.start_payment(payment).unwrap();

        // Verify payment is tracked
        let retrieved = coordinator.get_payment(payment_hash).unwrap();
        assert!(retrieved.is_some());
        assert!(!retrieved.unwrap().is_complete);
    }

    #[test]
    fn test_stats() {
        let coordinator = MultiPathPaymentCoordinator::with_defaults(NodeId::new("test".to_string()));
        let stats = coordinator.get_stats().unwrap();

        assert_eq!(stats.active_payments, 0);
        assert_eq!(stats.completed_payments, 0);
    }
}
