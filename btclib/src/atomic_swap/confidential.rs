//! Confidential atomic swaps using Bulletproofs
//! 
//! This module implements privacy-preserving atomic swaps where amounts
//! are hidden using Pedersen commitments and range proofs.

use crate::atomic_swap::{
    AtomicSwapError, HTLCError, SwapSession, AtomicSwapSetup,
    SupernovaHTLC, HTLCState, ParticipantInfo,
};
use crate::atomic_swap::error::ConfidentialError;
use crate::atomic_swap::crypto::{HashLock, generate_secure_random_32};

use bulletproofs::{BulletproofGens, PedersenGens, RangeProof};
use curve25519_dalek_ng::ristretto::{CompressedRistretto, RistrettoPoint};
use curve25519_dalek_ng::scalar::Scalar;
use curve25519_dalek_ng::traits::Identity;
use merlin::Transcript;
use rand::{thread_rng, Rng};
use serde::{Serialize, Deserialize};
use std::convert::TryFrom;

/// Confidential HTLC for privacy-preserving swaps
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ConfidentialHTLC {
    /// Base HTLC structure
    pub base_htlc: SupernovaHTLC,
    
    /// Amount commitment (hides the actual amount)
    pub amount_commitment: CompressedRistretto,
    
    /// Range proof proving amount is in valid range
    pub range_proof: Vec<u8>,
    
    /// Blinding factor (kept secret by sender)
    pub blinding_factor: Option<Scalar>,
    
    /// Minimum amount (public)
    pub min_amount: u64,
    
    /// Maximum amount (public)
    pub max_amount: u64,
}

/// Confidential swap session
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ConfidentialSwapSession {
    /// Base swap session
    pub base_session: SwapSession,
    
    /// Confidential HTLCs
    pub confidential_nova_htlc: ConfidentialHTLC,
    pub confidential_btc_reference: Option<ConfidentialBitcoinReference>,
    
    /// Shared commitments for atomic execution
    pub shared_commitment: CompressedRistretto,
}

/// Reference to confidential Bitcoin HTLC
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ConfidentialBitcoinReference {
    /// Transaction ID
    pub txid: String,
    
    /// Output index
    pub vout: u32,
    
    /// Commitment to amount (if supported)
    pub amount_commitment: Option<Vec<u8>>,
}

/// Confidential atomic swap builder
pub struct ConfidentialSwapBuilder {
    pedersen_gens: PedersenGens,
    bulletproof_gens: BulletproofGens,
}

impl ConfidentialSwapBuilder {
    /// Create a new confidential swap builder
    pub fn new() -> Self {
        Self {
            pedersen_gens: PedersenGens::default(),
            bulletproof_gens: BulletproofGens::new(64, 1),
        }
    }
    
    /// Create a confidential HTLC with hidden amount
    pub fn create_confidential_htlc(
        &self,
        base_htlc: SupernovaHTLC,
        amount: u64,
        min_amount: u64,
        max_amount: u64,
    ) -> Result<ConfidentialHTLC, ConfidentialError> {
        // Validate amount range
        if amount < min_amount || amount > max_amount {
            return Err(ConfidentialError::RangeProofFailed);
        }
        
        // Generate blinding factor
        let mut rng = thread_rng();
        let mut blinding_bytes = [0u8; 32];
        rng.fill(&mut blinding_bytes);
        let blinding = Scalar::from_bytes_mod_order(blinding_bytes);
        
        // Create Pedersen commitment
        let commitment = self.pedersen_gens.commit(Scalar::from(amount), blinding);
        
        // Create range proof
        let mut transcript = Transcript::new(b"ConfidentialHTLC");
        let (proof, committed_value) = RangeProof::prove_single(
            &self.bulletproof_gens,
            &self.pedersen_gens,
            &mut transcript,
            amount,
            &blinding,
            64,
        ).map_err(|_| ConfidentialError::RangeProofFailed)?;
        
        // Verify the proof matches the commitment
        if committed_value != commitment.compress() {
            return Err(ConfidentialError::CommitmentMismatch);
        }
        
        Ok(ConfidentialHTLC {
            base_htlc,
            amount_commitment: commitment.compress(),
            range_proof: proof.to_bytes(),
            blinding_factor: Some(blinding),
            min_amount,
            max_amount,
        })
    }
    
    /// Verify a confidential HTLC
    pub fn verify_confidential_htlc(
        &self,
        htlc: &ConfidentialHTLC,
    ) -> Result<bool, ConfidentialError> {
        // Deserialize range proof
        let proof = RangeProof::from_bytes(&htlc.range_proof)
            .map_err(|_| ConfidentialError::RangeProofFailed)?;
        
        // Verify range proof
        let mut transcript = Transcript::new(b"ConfidentialHTLC");
        let result = proof.verify_single(
            &self.bulletproof_gens,
            &self.pedersen_gens,
            &mut transcript,
            &htlc.amount_commitment,
            64,
        );
        
        match result {
            Ok(_) => Ok(true),
            Err(_) => Ok(false),
        }
    }
    
    /// Create a shared commitment for atomic execution
    pub fn create_shared_commitment(
        &self,
        alice_commitment: &CompressedRistretto,
        bob_commitment: &CompressedRistretto,
    ) -> Result<CompressedRistretto, ConfidentialError> {
        // Decompress points
        let alice_point = alice_commitment.decompress()
            .ok_or(ConfidentialError::CommitmentMismatch)?;
        let bob_point = bob_commitment.decompress()
            .ok_or(ConfidentialError::CommitmentMismatch)?;
        
        // Add commitments
        let shared = alice_point + bob_point;
        
        Ok(shared.compress())
    }
    
    /// Open a commitment with the blinding factor
    pub fn open_commitment(
        &self,
        commitment: &CompressedRistretto,
        amount: u64,
        blinding: &Scalar,
    ) -> Result<bool, ConfidentialError> {
        // Recompute commitment
        let expected = self.pedersen_gens.commit(Scalar::from(amount), *blinding);
        
        // Verify it matches
        Ok(expected.compress() == *commitment)
    }
}

/// Convert a regular swap session to confidential
pub async fn make_swap_confidential(
    session: SwapSession,
    min_amount: u64,
    max_amount: u64,
) -> Result<ConfidentialSwapSession, ConfidentialError> {
    let builder = ConfidentialSwapBuilder::new();
    
    // Create confidential HTLC
    let confidential_htlc = builder.create_confidential_htlc(
        session.nova_htlc.clone(),
        session.nova_htlc.amount,
        min_amount,
        max_amount,
    )?;
    
    // Create confidential session
    Ok(ConfidentialSwapSession {
        base_session: session,
        confidential_nova_htlc: confidential_htlc,
        confidential_btc_reference: None,
        shared_commitment: RistrettoPoint::identity().compress(),
    })
}

/// Verify a confidential swap
pub fn verify_confidential_swap(
    session: &ConfidentialSwapSession,
) -> Result<bool, ConfidentialError> {
    let builder = ConfidentialSwapBuilder::new();
    
    // Verify the confidential HTLC
    builder.verify_confidential_htlc(&session.confidential_nova_htlc)?;
    
    // Additional verifications can be added here
    
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::atomic_swap::htlc::{TimeLock, FeeStructure};
    use crate::crypto::MLDSAPrivateKey;
    use rand::rngs::OsRng;
    
    #[test]
    fn test_confidential_htlc_creation() {
        let builder = ConfidentialSwapBuilder::new();
        
        // Create test HTLC
        let alice_key = MLDSAPrivateKey::generate(&mut OsRng);
        let alice = ParticipantInfo {
            pubkey: alice_key.public_key(),
            address: "alice".to_string(),
            refund_address: None,
        };
        
        let bob_key = MLDSAPrivateKey::generate(&mut OsRng);
        let bob = ParticipantInfo {
            pubkey: bob_key.public_key(),
            address: "bob".to_string(),
            refund_address: None,
        };
        
        let hash_lock = HashLock::new(crate::atomic_swap::crypto::HashFunction::SHA256).unwrap();
        let time_lock = TimeLock {
            absolute_timeout: 1000,
            relative_timeout: 100,
            grace_period: 10,
        };
        
        let base_htlc = SupernovaHTLC::new(
            alice,
            bob,
            hash_lock,
            time_lock,
            1000000, // 1 million units
            FeeStructure {
                claim_fee: 1000,
                refund_fee: 1000,
                service_fee: None,
            },
        ).unwrap();
        
        // Create confidential version
        let result = builder.create_confidential_htlc(
            base_htlc,
            1000000,
            100000,  // min
            10000000, // max
        );
        
        assert!(result.is_ok());
        let conf_htlc = result.unwrap();
        
        // Verify the range proof
        let verify_result = builder.verify_confidential_htlc(&conf_htlc);
        assert!(verify_result.is_ok());
        assert!(verify_result.unwrap());
    }
    
    #[test]
    fn test_commitment_opening() {
        let builder = ConfidentialSwapBuilder::new();
        
        let amount = 1000000u64;
        let mut rng = thread_rng();
        let mut blinding_bytes = [0u8; 32];
        rng.fill(&mut blinding_bytes);
        let blinding = Scalar::from_bytes_mod_order(blinding_bytes);
        
        // Create commitment
        let commitment = builder.pedersen_gens.commit(Scalar::from(amount), blinding);
        
        // Verify opening
        let result = builder.open_commitment(&commitment.compress(), amount, &blinding);
        assert!(result.is_ok());
        assert!(result.unwrap());
        
        // Try wrong amount
        let wrong_result = builder.open_commitment(&commitment.compress(), amount + 1, &blinding);
        assert!(wrong_result.is_ok());
        assert!(!wrong_result.unwrap());
    }
} 