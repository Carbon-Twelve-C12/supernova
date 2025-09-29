use libp2p::PeerId;
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

/// Identity challenge for peer verification
#[derive(Debug, Clone)]
pub struct IdentityChallenge {
    /// The challenge nonce
    pub nonce: [u8; 32],
    /// The target difficulty
    pub difficulty: u8,
    /// When the challenge was created
    pub created_at: Instant,
    /// Challenge expiry duration
    pub expires_in: Duration,
}

impl IdentityChallenge {
    /// Create a new identity challenge
    pub fn new(difficulty: u8) -> Self {
        let mut nonce = [0u8; 32];
        getrandom::getrandom(&mut nonce).expect("Failed to generate random nonce");

        Self {
            nonce,
            difficulty,
            created_at: Instant::now(),
            expires_in: Duration::from_secs(300), // 5 minutes
        }
    }

    /// Check if the challenge has expired
    pub fn is_expired(&self) -> bool {
        self.created_at.elapsed() > self.expires_in
    }

    /// Verify a solution to the challenge
    pub fn verify_solution(&self, peer_id: &PeerId, solution: &[u8]) -> bool {
        // Combine peer ID, nonce, and solution
        let mut hasher = Sha256::new();
        hasher.update(peer_id.to_bytes());
        hasher.update(self.nonce);
        hasher.update(solution);
        let hash = hasher.finalize();

        // Check if hash meets difficulty requirement
        let leading_zeros = self.difficulty / 8;
        let remaining_bits = self.difficulty % 8;

        // Check full zero bytes
        for i in 0..leading_zeros as usize {
            if hash[i] != 0 {
                return false;
            }
        }

        // Check remaining bits if any
        if remaining_bits > 0 && leading_zeros < 32 {
            let mask = 0xFF << (8 - remaining_bits);
            if hash[leading_zeros as usize] & mask != 0 {
                return false;
            }
        }

        true
    }
}

/// Verification status for a peer
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VerificationStatus {
    /// Not yet verified
    Unverified,
    /// Currently being challenged
    Challenged,
    /// Successfully verified
    Verified,
    /// Failed verification
    Failed,
}

/// Identity verification system for P2P network
pub struct IdentityVerificationSystem {
    /// Active challenges for peers
    challenges: Arc<RwLock<HashMap<PeerId, IdentityChallenge>>>,
    /// Verification status for each peer
    verification_status: Arc<RwLock<HashMap<PeerId, VerificationStatus>>>,
    /// Verification timestamps
    verification_times: Arc<RwLock<HashMap<PeerId, Instant>>>,
    /// Challenge difficulty (number of leading zero bits required)
    challenge_difficulty: u8,
    /// Whether identity verification is required
    require_verification: bool,
    /// How long verification is valid
    verification_validity: Duration,
}

impl IdentityVerificationSystem {
    /// Create a new identity verification system
    pub fn new(challenge_difficulty: u8, require_verification: bool) -> Self {
        Self {
            challenges: Arc::new(RwLock::new(HashMap::new())),
            verification_status: Arc::new(RwLock::new(HashMap::new())),
            verification_times: Arc::new(RwLock::new(HashMap::new())),
            challenge_difficulty,
            require_verification,
            verification_validity: Duration::from_secs(86400), // 24 hours
        }
    }

    /// Create a challenge for a peer
    pub async fn create_challenge(&self, peer_id: &PeerId) -> IdentityChallenge {
        let challenge = IdentityChallenge::new(self.challenge_difficulty);

        // Store the challenge
        self.challenges
            .write()
            .await
            .insert(*peer_id, challenge.clone());
        self.verification_status
            .write()
            .await
            .insert(*peer_id, VerificationStatus::Challenged);

        info!(
            "Created identity challenge for peer {} with difficulty {}",
            peer_id, self.challenge_difficulty
        );
        challenge
    }

    /// Verify a challenge solution
    pub async fn verify_challenge(&self, peer_id: &PeerId, solution: &[u8]) -> bool {
        // Get the challenge
        let challenges = self.challenges.read().await;
        let challenge = match challenges.get(peer_id) {
            Some(c) => c,
            None => {
                warn!("No challenge found for peer {}", peer_id);
                return false;
            }
        };

        // Check if expired
        if challenge.is_expired() {
            warn!("Challenge for peer {} has expired", peer_id);
            drop(challenges);
            self.challenges.write().await.remove(peer_id);
            self.verification_status
                .write()
                .await
                .insert(*peer_id, VerificationStatus::Failed);
            return false;
        }

        // Verify the solution
        let is_valid = challenge.verify_solution(peer_id, solution);
        drop(challenges);

        if is_valid {
            // Remove challenge and mark as verified
            self.challenges.write().await.remove(peer_id);
            self.verification_status
                .write()
                .await
                .insert(*peer_id, VerificationStatus::Verified);
            self.verification_times
                .write()
                .await
                .insert(*peer_id, Instant::now());
            info!("Peer {} successfully verified identity", peer_id);
        } else {
            self.verification_status
                .write()
                .await
                .insert(*peer_id, VerificationStatus::Failed);
            warn!("Peer {} failed identity verification", peer_id);
        }

        is_valid
    }

    /// Check if a peer is verified
    pub async fn is_verified(&self, peer_id: &PeerId) -> bool {
        if !self.require_verification {
            return true;
        }

        let status = self.verification_status.read().await;
        let times = self.verification_times.read().await;

        match status.get(peer_id) {
            Some(VerificationStatus::Verified) => {
                // Check if verification is still valid
                if let Some(verified_at) = times.get(peer_id) {
                    verified_at.elapsed() < self.verification_validity
                } else {
                    false
                }
            }
            _ => false,
        }
    }

    /// Get verification status for a peer
    pub async fn get_status(&self, peer_id: &PeerId) -> VerificationStatus {
        self.verification_status
            .read()
            .await
            .get(peer_id)
            .copied()
            .unwrap_or(VerificationStatus::Unverified)
    }

    /// Remove verification data for a peer
    pub async fn remove_peer(&self, peer_id: &PeerId) {
        self.challenges.write().await.remove(peer_id);
        self.verification_status.write().await.remove(peer_id);
        self.verification_times.write().await.remove(peer_id);
        debug!("Removed verification data for peer {}", peer_id);
    }

    /// Clean up expired challenges and verifications
    pub async fn cleanup_expired(&self) {
        let now = Instant::now();

        // Clean up expired challenges
        let mut challenges = self.challenges.write().await;
        let mut status = self.verification_status.write().await;

        let expired_challenges: Vec<PeerId> = challenges
            .iter()
            .filter(|(_, challenge)| challenge.is_expired())
            .map(|(peer_id, _)| *peer_id)
            .collect();

        for peer_id in expired_challenges {
            challenges.remove(&peer_id);
            status.insert(peer_id, VerificationStatus::Failed);
            warn!("Challenge for peer {} expired", peer_id);
        }

        // Clean up expired verifications
        let mut times = self.verification_times.write().await;
        let expired_verifications: Vec<PeerId> = times
            .iter()
            .filter(|(_, verified_at)| verified_at.elapsed() > self.verification_validity)
            .map(|(peer_id, _)| *peer_id)
            .collect();

        for peer_id in expired_verifications {
            times.remove(&peer_id);
            status.insert(peer_id, VerificationStatus::Unverified);
            info!("Verification for peer {} expired", peer_id);
        }
    }

    /// Get statistics about the verification system
    pub async fn get_stats(&self) -> VerificationStats {
        let status = self.verification_status.read().await;
        let challenges = self.challenges.read().await;

        let mut stats = VerificationStats::default();

        for (_, s) in status.iter() {
            match s {
                VerificationStatus::Unverified => stats.unverified += 1,
                VerificationStatus::Challenged => stats.challenged += 1,
                VerificationStatus::Verified => stats.verified += 1,
                VerificationStatus::Failed => stats.failed += 1,
            }
        }

        stats.active_challenges = challenges.len();
        stats
    }
}

/// Statistics for the verification system
#[derive(Debug, Default, Clone)]
pub struct VerificationStats {
    /// Number of unverified peers
    pub unverified: usize,
    /// Number of peers being challenged
    pub challenged: usize,
    /// Number of verified peers
    pub verified: usize,
    /// Number of peers that failed verification
    pub failed: usize,
    /// Number of active challenges
    pub active_challenges: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_challenge_creation() {
        let challenge = IdentityChallenge::new(16);
        assert_eq!(challenge.difficulty, 16);
        assert!(!challenge.is_expired());
    }

    #[test]
    fn test_challenge_verification() {
        let peer_id = PeerId::random();
        let challenge = IdentityChallenge::new(8); // Low difficulty for testing

        // Try to find a valid solution
        let mut solution = vec![0u8; 8];
        let mut found = false;

        for i in 0..10000 {
            solution[0] = (i >> 24) as u8;
            solution[1] = (i >> 16) as u8;
            solution[2] = (i >> 8) as u8;
            solution[3] = i as u8;

            if challenge.verify_solution(&peer_id, &solution) {
                found = true;
                break;
            }
        }

        assert!(found, "Should find a valid solution for low difficulty");
    }

    #[tokio::test]
    async fn test_verification_system() {
        let system = IdentityVerificationSystem::new(8, true);
        let peer_id = PeerId::random();

        // Create challenge
        let challenge = system.create_challenge(&peer_id).await;
        assert_eq!(
            system.get_status(&peer_id).await,
            VerificationStatus::Challenged
        );

        // Find valid solution
        let mut solution = vec![0u8; 8];
        for i in 0..100000 {
            solution[0] = (i >> 24) as u8;
            solution[1] = (i >> 16) as u8;
            solution[2] = (i >> 8) as u8;
            solution[3] = i as u8;

            if challenge.verify_solution(&peer_id, &solution) {
                break;
            }
        }

        // Verify solution
        assert!(system.verify_challenge(&peer_id, &solution).await);
        assert!(system.is_verified(&peer_id).await);
        assert_eq!(
            system.get_status(&peer_id).await,
            VerificationStatus::Verified
        );
    }
}
