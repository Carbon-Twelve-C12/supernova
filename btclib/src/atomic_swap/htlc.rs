//! Hash Time-Locked Contract (HTLC) implementation for Supernova
//! 
//! This module implements HTLCs with quantum-resistant signatures for use in
//! atomic swaps between Bitcoin and Supernova blockchains.

use crate::crypto::{MLDSAPublicKey, MLDSASignature, Hash256};
use crate::atomic_swap::error::{HTLCError, SecurityError};
use crate::atomic_swap::crypto::{HashLock, compute_hash};
use serde::{Serialize, Deserialize};
use std::time::{SystemTime, UNIX_EPOCH};

/// Supernova HTLC contract with quantum-resistant signatures
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SupernovaHTLC {
    /// Unique identifier for the HTLC
    pub htlc_id: [u8; 32],
    
    /// Participants in the swap
    pub initiator: ParticipantInfo,
    pub participant: ParticipantInfo,
    
    /// Lock conditions
    pub hash_lock: HashLock,
    pub time_lock: TimeLock,
    
    /// Swap details
    pub amount: u64,
    pub fee_structure: FeeStructure,
    
    /// State tracking
    pub state: HTLCState,
    pub created_at: u64,
    pub bitcoin_tx_ref: Option<BitcoinTxReference>,
    
    /// Additional metadata
    pub memo: Option<String>,
}

/// Information about a swap participant
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ParticipantInfo {
    /// Quantum-resistant public key
    pub pubkey: MLDSAPublicKey,
    /// Supernova address
    pub address: String,
    /// Optional refund address (if different from main address)
    pub refund_address: Option<String>,
}

/// Time-based lock conditions
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TimeLock {
    /// Absolute timeout (Unix timestamp)
    pub absolute_timeout: u64,
    /// Relative timeout (blocks from creation)
    pub relative_timeout: u32,
    /// Grace period for network delays
    pub grace_period: u32,
}

/// Fee structure for the HTLC
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FeeStructure {
    /// Network fee for claim transaction
    pub claim_fee: u64,
    /// Network fee for refund transaction
    pub refund_fee: u64,
    /// Optional service fee for atomic swap facilitator
    pub service_fee: Option<u64>,
}

/// Current state of the HTLC
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum HTLCState {
    /// HTLC created but not yet funded
    Created,
    /// HTLC funded and active
    Funded,
    /// HTLC claimed by participant
    Claimed,
    /// HTLC refunded to initiator
    Refunded,
    /// HTLC expired (past timeout)
    Expired,
}

/// Reference to corresponding Bitcoin transaction
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BitcoinTxReference {
    pub txid: String,
    pub vout: u32,
    pub amount_sats: u64,
}

impl SupernovaHTLC {
    /// Create a new HTLC
    pub fn new(
        initiator: ParticipantInfo,
        participant: ParticipantInfo,
        hash_lock: HashLock,
        time_lock: TimeLock,
        amount: u64,
        fee_structure: FeeStructure,
    ) -> Result<Self, HTLCError> {
        // Validate amount
        if amount == 0 {
            return Err(HTLCError::InvalidAmount("Amount must be greater than 0".to_string()));
        }
        
        // Validate timeout
        let current_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
            
        if time_lock.absolute_timeout <= current_time {
            return Err(HTLCError::InvalidTimeout);
        }
        
        // Generate unique ID
        let mut htlc_id = [0u8; 32];
        let id_data = format!(
            "{:?}{:?}{:?}{}{}",
            initiator.pubkey,
            participant.pubkey,
            hash_lock.hash_value,
            amount,
            current_time
        );
        let hash = compute_hash(id_data.as_bytes())?;
        htlc_id.copy_from_slice(&hash);
        
        Ok(Self {
            htlc_id,
            initiator,
            participant,
            hash_lock,
            time_lock,
            amount,
            fee_structure,
            state: HTLCState::Created,
            created_at: current_time,
            bitcoin_tx_ref: None,
            memo: None,
        })
    }
    
    /// Verify a claim attempt with preimage and signature
    pub fn verify_claim(&self, preimage: &[u8; 32], signature: &MLDSASignature) -> Result<bool, HTLCError> {
        // Check state
        if self.state != HTLCState::Funded {
            return Ok(false);
        }
        
        // Verify hash preimage
        let computed_hash = compute_hash(preimage)?;
        if computed_hash != self.hash_lock.hash_value {
            return Ok(false);
        }
        
        // Create claim message
        let message = self.create_claim_message(preimage)?;
        
        // Verify quantum-resistant signature
        self.participant.pubkey.verify(&message, signature)
            .map_err(|_| HTLCError::InvalidSignature)
    }
    
    /// Verify a refund attempt with signature
    pub fn verify_refund(&self, signature: &MLDSASignature, current_height: u64) -> Result<bool, HTLCError> {
        // Check state
        if self.state != HTLCState::Funded {
            return Ok(false);
        }
        
        // Check timeout has passed
        let current_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
            
        if current_time < self.time_lock.absolute_timeout {
            return Err(HTLCError::TimeoutNotReached);
        }
        
        // Create refund message
        let message = self.create_refund_message()?;
        
        // Verify signature from initiator
        self.initiator.pubkey.verify(&message, signature)
            .map_err(|_| HTLCError::InvalidSignature)
    }
    
    /// Create the message to be signed for a claim
    fn create_claim_message(&self, preimage: &[u8; 32]) -> Result<Vec<u8>, HTLCError> {
        let message = format!(
            "CLAIM:{}:{}:{}",
            hex::encode(&self.htlc_id),
            hex::encode(preimage),
            self.amount
        );
        Ok(message.into_bytes())
    }
    
    /// Create the message to be signed for a refund
    fn create_refund_message(&self) -> Result<Vec<u8>, HTLCError> {
        let message = format!(
            "REFUND:{}:{}:{}",
            hex::encode(&self.htlc_id),
            self.time_lock.absolute_timeout,
            self.amount
        );
        Ok(message.into_bytes())
    }
    
    /// Update the state of the HTLC
    pub fn update_state(&mut self, new_state: HTLCState) -> Result<(), HTLCError> {
        // Validate state transition
        match (&self.state, &new_state) {
            (HTLCState::Created, HTLCState::Funded) => {},
            (HTLCState::Funded, HTLCState::Claimed) => {},
            (HTLCState::Funded, HTLCState::Refunded) => {},
            (HTLCState::Funded, HTLCState::Expired) => {},
            _ => {
                return Err(HTLCError::InvalidStateTransition {
                    from: format!("{:?}", self.state),
                    to: format!("{:?}", new_state),
                });
            }
        }
        
        self.state = new_state;
        Ok(())
    }
    
    /// Check if the HTLC has expired
    pub fn is_expired(&self) -> bool {
        let current_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
            
        current_time >= self.time_lock.absolute_timeout + self.time_lock.grace_period as u64
    }
    
    /// Create a funding transaction output for this HTLC
    pub fn create_funding_output(&self) -> crate::types::TransactionOutput {
        crate::types::TransactionOutput::new(self.amount, self.create_script_pubkey())
    }
    
    /// Create the script pubkey for this HTLC
    /// This is a simplified version - in production, this would be more complex
    fn create_script_pubkey(&self) -> Vec<u8> {
        // In a real implementation, this would create a proper script
        // For now, we'll create a placeholder that encodes the HTLC ID
        let mut script = Vec::new();
        script.extend_from_slice(b"HTLC:");
        script.extend_from_slice(&self.htlc_id);
        script
    }
    
    /// Calculate the total amount needed including fees
    pub fn total_amount_with_fees(&self) -> u64 {
        self.amount + self.fee_structure.claim_fee + 
        self.fee_structure.service_fee.unwrap_or(0)
    }
}

/// Validate security parameters for the HTLC
pub fn validate_htlc_security(
    htlc: &SupernovaHTLC,
    config: &crate::atomic_swap::AtomicSwapConfig,
) -> Result<(), SecurityError> {
    // Validate amount limits
    if htlc.amount < config.min_swap_amount_btc {
        return Err(SecurityError::AmountTooLow { 
            min: config.min_swap_amount_btc 
        });
    }
    
    if htlc.amount > config.max_swap_amount_btc {
        return Err(SecurityError::AmountTooHigh { 
            max: config.max_swap_amount_btc 
        });
    }
    
    // Validate timeout is reasonable (not too far in the future)
    let max_timeout = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() + 30 * 24 * 60 * 60; // 30 days
        
    if htlc.time_lock.absolute_timeout > max_timeout {
        return Err(SecurityError::InvalidTimeoutOrdering);
    }
    
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypto::MLDSAPrivateKey;
    use rand::rngs::OsRng;
    
    fn create_test_participant() -> (ParticipantInfo, MLDSAPrivateKey) {
        let private_key = MLDSAPrivateKey::generate(&mut OsRng);
        let public_key = private_key.public_key();
        let participant = ParticipantInfo {
            pubkey: public_key,
            address: "nova1qtest...".to_string(),
            refund_address: None,
        };
        (participant, private_key)
    }
    
    fn create_test_htlc() -> SupernovaHTLC {
        let (initiator, _) = create_test_participant();
        let (participant, _) = create_test_participant();
        
        let hash_lock = HashLock {
            hash_type: crate::atomic_swap::crypto::HashFunction::SHA256,
            hash_value: [0x42; 32],
            preimage: None,
        };
        
        let time_lock = TimeLock {
            absolute_timeout: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs() + 3600, // 1 hour from now
            relative_timeout: 144,
            grace_period: 6,
        };
        
        let fee_structure = FeeStructure {
            claim_fee: 1000,
            refund_fee: 1000,
            service_fee: Some(100),
        };
        
        SupernovaHTLC::new(
            initiator,
            participant,
            hash_lock,
            time_lock,
            100_000_000, // 1 NOVA
            fee_structure,
        ).unwrap()
    }
    
    #[test]
    fn test_htlc_creation() {
        let htlc = create_test_htlc();
        assert_eq!(htlc.state, HTLCState::Created);
        assert_eq!(htlc.amount, 100_000_000);
        assert!(htlc.htlc_id != [0u8; 32]);
    }
    
    #[test]
    fn test_htlc_state_transitions() {
        let mut htlc = create_test_htlc();
        
        // Valid transition: Created -> Funded
        assert!(htlc.update_state(HTLCState::Funded).is_ok());
        
        // Invalid transition: Funded -> Created
        assert!(htlc.update_state(HTLCState::Created).is_err());
        
        // Valid transition: Funded -> Claimed
        assert!(htlc.update_state(HTLCState::Claimed).is_ok());
    }
    
    #[test]
    fn test_htlc_expiry() {
        let mut htlc = create_test_htlc();
        
        // Not expired initially
        assert!(!htlc.is_expired());
        
        // Manually set timeout to past
        htlc.time_lock.absolute_timeout = 1000;
        assert!(htlc.is_expired());
    }
    
    #[test]
    fn test_total_amount_calculation() {
        let htlc = create_test_htlc();
        let total = htlc.total_amount_with_fees();
        assert_eq!(total, 100_000_000 + 1000 + 100); // amount + claim_fee + service_fee
    }
} 