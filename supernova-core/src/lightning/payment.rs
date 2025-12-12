//! Lightning Network Payment Processing
//!
//! This module handles Lightning Network payments, including HTLC management,
//! payment routing, and settlement.

use crate::crypto::quantum::QuantumScheme;
use hex;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use std::time::{SystemTime, UNIX_EPOCH};
use thiserror::Error;
use tracing::{debug, error, info, warn};

/// Payment hash - SHA256 hash of payment preimage
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PaymentHash([u8; 32]);

impl fmt::Display for PaymentHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_hex())
    }
}

impl PaymentHash {
    pub fn new(hash: [u8; 32]) -> Self {
        Self(hash)
    }

    pub fn into_inner(self) -> [u8; 32] {
        self.0
    }

    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    /// Convert to hex string
    pub fn to_hex(&self) -> String {
        hex::encode(self.0)
    }

    /// Create from hex string
    pub fn from_hex(hex_str: &str) -> Result<Self, PaymentError> {
        if hex_str.len() != 64 {
            return Err(PaymentError::InvalidPreimage);
        }

        let bytes = hex::decode(hex_str).map_err(|_| PaymentError::InvalidPreimage)?;

        if bytes.len() != 32 {
            return Err(PaymentError::InvalidPreimage);
        }

        let mut hash = [0u8; 32];
        hash.copy_from_slice(&bytes);
        Ok(Self(hash))
    }
}

/// Payment preimage - 32 bytes of random data
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PaymentPreimage([u8; 32]);

impl PaymentPreimage {
    pub fn new(preimage: [u8; 32]) -> Self {
        Self(preimage)
    }

    pub fn new_random() -> Self {
        let mut preimage = [0u8; 32];
        rand::thread_rng().fill_bytes(&mut preimage);
        Self(preimage)
    }

    pub fn into_inner(self) -> [u8; 32] {
        self.0
    }

    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    /// Generate payment hash from this preimage
    pub fn payment_hash(&self) -> PaymentHash {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(self.0);
        let hash = hasher.finalize();
        PaymentHash(hash.into())
    }

    /// Convert to hex string
    pub fn to_hex(&self) -> String {
        hex::encode(self.0)
    }

    /// Create from hex string
    pub fn from_hex(hex_str: &str) -> Result<Self, PaymentError> {
        if hex_str.len() != 64 {
            return Err(PaymentError::InvalidPreimage);
        }

        let bytes = hex::decode(hex_str).map_err(|_| PaymentError::InvalidPreimage)?;

        if bytes.len() != 32 {
            return Err(PaymentError::InvalidPreimage);
        }

        let mut preimage = [0u8; 32];
        preimage.copy_from_slice(&bytes);
        Ok(Self(preimage))
    }
}

/// Payment status
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PaymentStatus {
    /// Payment is pending
    Pending,
    /// Payment completed successfully
    Succeeded,
    /// Payment failed
    Failed(String),
    /// Payment was cancelled
    Cancelled,
}

/// HTLC (Hash Time Locked Contract) information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Htlc {
    /// Unique HTLC identifier
    pub id: u64,
    /// Payment hash
    pub payment_hash: [u8; 32],
    /// Amount in nova units (smallest denomination)
    pub amount_nova: u64,
    /// CLTV expiry height
    pub cltv_expiry: u32,
    /// Whether this HTLC is offered by us (outgoing) or received (incoming)
    pub offered: bool,
    /// Current state of the HTLC
    pub state: HtlcState,
    /// Quantum signature if quantum security is enabled
    pub quantum_signature: Option<Vec<u8>>,
}

impl Htlc {
    /// Check if this HTLC uses quantum signatures
    pub fn is_quantum_secured(&self) -> bool {
        self.quantum_signature.is_some()
    }
    
    /// Get effective expiry height accounting for quantum signature overhead
    /// 
    /// SECURITY FIX (P1-001): If HTLC uses quantum signatures, we add additional
    /// buffer time to account for slower verification.
    ///
    /// # Arguments
    /// * `current_height` - Current blockchain height
    ///
    /// # Returns
    /// Effective expiry height with quantum buffer if applicable
    pub fn get_effective_expiry(&self) -> u32 {
        if self.is_quantum_secured() {
            // Import from quantum_lightning module
            use crate::lightning::quantum_lightning::QuantumHTLCConfig;
            
            // Add quantum buffer to expiry
            self.cltv_expiry.saturating_add(QuantumHTLCConfig::TOTAL_SAFETY_MARGIN)
        } else {
            // Classical HTLC - use original expiry
            self.cltv_expiry
        }
    }
    
    /// Check if HTLC has expired accounting for quantum signature buffer
    /// 
    /// SECURITY: Uses get_effective_expiry() to prevent premature timeouts
    ///
    /// # Arguments
    /// * `current_height` - Current blockchain height
    ///
    /// # Returns
    /// `true` if HTLC has truly expired, `false` otherwise
    pub fn is_expired(&self, current_height: u32) -> bool {
        current_height >= self.get_effective_expiry()
    }
}

/// HTLC state
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum HtlcState {
    /// HTLC is pending
    Pending,
    /// HTLC has been fulfilled with preimage
    Fulfilled([u8; 32]),
    /// HTLC has failed
    Failed(String),
    /// HTLC has timed out
    TimedOut,
}

/// Payment information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Payment {
    /// Payment hash
    pub payment_hash: PaymentHash,
    /// Payment preimage (if known)
    pub payment_preimage: Option<PaymentPreimage>,
    /// Amount in millinova (mnova)
    pub amount_mnova: u64,
    /// Payment status
    pub status: PaymentStatus,
    /// Creation timestamp
    pub created_at: u64,
    /// Completion timestamp
    pub completed_at: Option<u64>,
    /// Fee paid in millinova (mnova)
    pub fee_mnova: u64,
    /// Payment route taken
    pub route: Option<Vec<RouteHop>>,
    /// Failure reason if payment failed
    pub failure_reason: Option<String>,
    /// Environmental impact data
    pub carbon_footprint_grams: Option<f64>,
}

/// Route hop information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteHop {
    /// Channel ID for this hop
    pub channel_id: u64,
    /// Node public key
    pub node_id: String,
    /// Amount to forward in millinova (mnova)
    pub amount_mnova: u64,
    /// Fee for this hop in millinova (mnova)
    pub fee_mnova: u64,
    /// CLTV expiry delta
    pub cltv_expiry_delta: u16,
}

impl RouteHop {
    /// Calculate fee for forwarding an amount through this hop
    pub fn channel_fee(&self, amount_mnova: u64) -> u64 {
        // Base fee + proportional fee
        let base_fee: u64 = 1000; // 1 nova-unit base fee (expressed in mnova context)
        let proportional_fee = amount_mnova
            .saturating_mul(100)
            .saturating_div(1_000_000); // 0.01% proportional fee
        base_fee.saturating_add(proportional_fee)
    }
}

/// Payment processor for Lightning Network
pub struct PaymentProcessor {
    /// Active payments
    payments: HashMap<PaymentHash, Payment>,
    /// Active HTLCs
    htlcs: HashMap<u64, Htlc>,
    /// Next HTLC ID
    next_htlc_id: u64,
    /// Quantum security configuration
    quantum_scheme: Option<QuantumScheme>,
}

impl PaymentProcessor {
    /// Create a new payment processor
    pub fn new(quantum_scheme: Option<QuantumScheme>) -> Self {
        Self {
            payments: HashMap::new(),
            htlcs: HashMap::new(),
            next_htlc_id: 1,
            quantum_scheme,
        }
    }

    /// Create a new payment
    pub fn create_payment(
        &mut self,
        amount_mnova: u64,
        destination: &str,
    ) -> Result<PaymentHash, PaymentError> {
        let preimage = PaymentPreimage::new_random();
        let payment_hash = preimage.payment_hash();

        let payment = Payment {
            payment_hash,
            payment_preimage: Some(preimage),
            amount_mnova,
            status: PaymentStatus::Pending,
            created_at: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            completed_at: None,
            fee_mnova: 0,
            route: None,
            failure_reason: None,
            carbon_footprint_grams: None,
        };

        self.payments.insert(payment_hash, payment);

        info!(
            "Created payment: hash={:x?}, amount={} mnova, destination={}",
            &payment_hash.as_bytes()[0..4],
            amount_mnova,
            destination
        );

        Ok(payment_hash)
    }

    /// Add an HTLC
    pub fn add_htlc(
        &mut self,
        payment_hash: [u8; 32],
        amount_nova: u64,
        cltv_expiry: u32,
        offered: bool,
    ) -> Result<u64, PaymentError> {
        let htlc_id = self.next_htlc_id;
        self.next_htlc_id += 1;

        let htlc = Htlc {
            id: htlc_id,
            payment_hash,
            amount_nova,
            cltv_expiry,
            offered,
            state: HtlcState::Pending,
            quantum_signature: None,
        };

        self.htlcs.insert(htlc_id, htlc);

        debug!(
            "Added HTLC {}: payment_hash={:x?}, amount={} nova units, offered={}",
            htlc_id,
            &payment_hash[0..4],
            amount_nova,
            offered
        );

        Ok(htlc_id)
    }

    /// Fulfill an HTLC with a preimage
    pub fn fulfill_htlc(&mut self, htlc_id: u64, preimage: [u8; 32]) -> Result<(), PaymentError> {
        let htlc = self
            .htlcs
            .get_mut(&htlc_id)
            .ok_or(PaymentError::HtlcNotFound(htlc_id))?;

        // Verify preimage matches payment hash
        let preimage_obj = PaymentPreimage::new(preimage);
        let expected_hash = preimage_obj.payment_hash();

        if expected_hash.as_bytes() != &htlc.payment_hash {
            return Err(PaymentError::InvalidPreimage);
        }

        htlc.state = HtlcState::Fulfilled(preimage);

        // Update payment status if this was the final HTLC
        let payment_hash = PaymentHash::new(htlc.payment_hash);
        if let Some(payment) = self.payments.get_mut(&payment_hash) {
            payment.status = PaymentStatus::Succeeded;
            payment.completed_at = Some(
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs(),
            );
            payment.payment_preimage = Some(preimage_obj);
        }

        info!(
            "Fulfilled HTLC {}: preimage={:x?}",
            htlc_id,
            &preimage[0..4]
        );

        Ok(())
    }

    /// Fail an HTLC
    pub fn fail_htlc(&mut self, htlc_id: u64, reason: &str) -> Result<(), PaymentError> {
        let htlc = self
            .htlcs
            .get_mut(&htlc_id)
            .ok_or(PaymentError::HtlcNotFound(htlc_id))?;

        htlc.state = HtlcState::Failed(reason.to_string());

        // Update payment status
        let payment_hash = PaymentHash::new(htlc.payment_hash);
        if let Some(payment) = self.payments.get_mut(&payment_hash) {
            payment.status = PaymentStatus::Failed(reason.to_string());
            payment.completed_at = Some(
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs(),
            );
            payment.failure_reason = Some(reason.to_string());
        }

        warn!("Failed HTLC {}: reason={}", htlc_id, reason);

        Ok(())
    }

    /// Get payment information
    pub fn get_payment(&self, payment_hash: &PaymentHash) -> Option<&Payment> {
        self.payments.get(payment_hash)
    }

    /// Get HTLC information
    pub fn get_htlc(&self, htlc_id: u64) -> Option<&Htlc> {
        self.htlcs.get(&htlc_id)
    }

    /// Get all pending HTLCs
    pub fn get_pending_htlcs(&self) -> Vec<&Htlc> {
        self.htlcs
            .values()
            .filter(|htlc| htlc.state == HtlcState::Pending)
            .collect()
    }

    /// Calculate environmental impact of a payment
    pub fn calculate_payment_emissions(&self, payment: &Payment) -> f64 {
        // Base emissions for Lightning transaction (much lower than on-chain)
        let base_emissions = 0.001; // 0.001g CO2 for Lightning vs ~700g for Bitcoin on-chain

        // Add emissions based on route length
        let route_emissions = if let Some(route) = &payment.route {
            route.len() as f64 * 0.0001 // 0.0001g CO2 per hop
        } else {
            0.0
        };

        base_emissions + route_emissions
    }

    /// Process expired HTLCs
    /// 
    /// SECURITY FIX (P1-001): Uses Htlc::is_expired() which accounts for quantum
    /// signature verification overhead. This prevents HTLCs from timing out during
    /// verification, which could lead to fund loss or griefing attacks.
    ///
    /// # Arguments
    /// * `current_height` - Current blockchain height
    ///
    /// # Returns
    /// Vector of expired HTLC IDs
    pub fn process_expired_htlcs(&mut self, current_height: u32) -> Vec<u64> {
        let mut expired_htlcs = Vec::new();

        for (htlc_id, htlc) in self.htlcs.iter_mut() {
            // CRITICAL SECURITY FIX: Use is_expired() instead of direct comparison
            // This method accounts for quantum signature verification overhead
            // 
            // BEFORE: if current_height >= htlc.cltv_expiry
            // AFTER:  if htlc.is_expired(current_height)
            //
            // For quantum HTLCs, this adds 216 blocks (~36 hours) buffer
            if htlc.state == HtlcState::Pending && htlc.is_expired(current_height) {
                htlc.state = HtlcState::TimedOut;
                expired_htlcs.push(*htlc_id);

                // Update payment status
                let payment_hash = PaymentHash::new(htlc.payment_hash);
                if let Some(payment) = self.payments.get_mut(&payment_hash) {
                    payment.status = PaymentStatus::Failed("HTLC expired".to_string());
                    payment.completed_at = Some(
                        SystemTime::now()
                            .duration_since(UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_secs(),
                    );
                    payment.failure_reason = Some(format!(
                        "HTLC expired at height {} (quantum-adjusted timeout)",
                        current_height
                    ));
                }
                
                debug!(
                    "HTLC {} expired at height {} (quantum: {})",
                    htlc_id,
                    current_height,
                    htlc.is_quantum_secured()
                );
            }
        }

        if !expired_htlcs.is_empty() {
            info!("Processed {} expired HTLCs at height {}", expired_htlcs.len(), current_height);
        }

        expired_htlcs
    }

    /// Get payment statistics
    pub fn get_payment_stats(&self) -> PaymentStats {
        let total_payments = self.payments.len();
        let successful_payments = self
            .payments
            .values()
            .filter(|p| p.status == PaymentStatus::Succeeded)
            .count();
        let failed_payments = self
            .payments
            .values()
            .filter(|p| matches!(p.status, PaymentStatus::Failed(_)))
            .count();
        let pending_payments = self
            .payments
            .values()
            .filter(|p| p.status == PaymentStatus::Pending)
            .count();

        let total_volume_msat = self
            .payments
            .values()
            .filter(|p| p.status == PaymentStatus::Succeeded)
            .map(|p| p.amount_msat)
            .sum();

        let total_fees_msat = self
            .payments
            .values()
            .filter(|p| p.status == PaymentStatus::Succeeded)
            .map(|p| p.fee_msat)
            .sum();

        PaymentStats {
            total_payments,
            successful_payments,
            failed_payments,
            pending_payments,
            total_volume_msat,
            total_fees_msat,
        }
    }
}

/// Payment statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaymentStats {
    pub total_payments: usize,
    pub successful_payments: usize,
    pub failed_payments: usize,
    pub pending_payments: usize,
    pub total_volume_msat: u64,
    pub total_fees_msat: u64,
}

/// Payment processing errors
#[derive(Debug, Error)]
pub enum PaymentError {
    #[error("HTLC not found: {0}")]
    HtlcNotFound(u64),

    #[error("Invalid preimage")]
    InvalidPreimage,

    #[error("Payment not found")]
    PaymentNotFound,

    #[error("Insufficient funds")]
    InsufficientFunds,

    #[error("Payment expired")]
    PaymentExpired,

    #[error("Invalid route")]
    InvalidRoute,

    #[error("Network error: {0}")]
    NetworkError(String),

    #[error("Quantum signature error: {0}")]
    QuantumSignatureError(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_payment_creation() {
        let mut processor = PaymentProcessor::new(None);
        let payment_hash = processor
            .create_payment(100_000, "test_destination")
            .unwrap();

        let payment = processor.get_payment(&payment_hash).unwrap();
        assert_eq!(payment.amount_msat, 100_000);
        assert_eq!(payment.status, PaymentStatus::Pending);
    }

    #[test]
    fn test_htlc_lifecycle() {
        let mut processor = PaymentProcessor::new(None);

        let preimage = PaymentPreimage::new_random();
        let payment_hash = preimage.payment_hash();

        // Add HTLC
        let htlc_id = processor
            .add_htlc(payment_hash.into_inner(), 1000, 100, true)
            .unwrap();

        // Fulfill HTLC
        processor
            .fulfill_htlc(htlc_id, preimage.into_inner())
            .unwrap();

        let htlc = processor.get_htlc(htlc_id).unwrap();
        assert!(matches!(htlc.state, HtlcState::Fulfilled(_)));
    }

    #[test]
    fn test_preimage_hash_verification() {
        let preimage = PaymentPreimage::new_random();
        let payment_hash = preimage.payment_hash();

        // Verify that the hash of the preimage matches
        let computed_hash = preimage.payment_hash();
        assert_eq!(payment_hash, computed_hash);
    }
}
