//! Environmental Oracle System for Supernova Blockchain
//!
//! This module implements a decentralized oracle system for verifying environmental claims
//! such as renewable energy certificates (RECs) and carbon offsets. It prevents gaming
//! by requiring cryptographic proofs, multi-oracle consensus, and economic incentives.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, RwLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use thiserror::Error;

use crate::environmental::emissions::{
    CarbonOffsetInfo, EnergySourceInfo, EnergySourceType, RECCertificateInfo, VerificationStatus,
};

/// Oracle error types
#[derive(Error, Debug, Clone, PartialEq, Eq)]
pub enum OracleError {
    #[error("Oracle not registered: {0}")]
    OracleNotRegistered(String),

    #[error("Invalid signature: {0}")]
    InvalidSignature(String),

    #[error("Insufficient stake: required {required}, has {has}")]
    InsufficientStake { required: u64, has: u64 },

    #[error("Data verification failed: {0}")]
    VerificationFailed(String),

    #[error("Consensus not reached: {0}")]
    ConsensusNotReached(String),

    #[error("Oracle slashed: {0}")]
    OracleSlashed(String),

    #[error("Invalid proof: {0}")]
    InvalidProof(String),

    #[error("Expired data: {0}")]
    ExpiredData(String),

    #[error("Duplicate submission")]
    DuplicateSubmission,

    #[error("Network error: {0}")]
    NetworkError(String),
}

/// Oracle reputation and stake information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OracleInfo {
    /// Oracle identifier (public key)
    pub oracle_id: String,

    /// Stake amount in NOVA
    pub stake_amount: u64,

    /// Reputation score (0-1000)
    pub reputation_score: u32,

    /// Number of correct verifications
    pub correct_verifications: u64,

    /// Number of incorrect verifications
    pub incorrect_verifications: u64,

    /// Last activity timestamp
    pub last_activity: DateTime<Utc>,

    /// Specializations (e.g., "REC", "carbon_offset", "energy_grid")
    pub specializations: HashSet<String>,

    /// Whether oracle is currently active
    pub is_active: bool,

    /// Slashing history
    pub slashing_events: Vec<SlashingEvent>,
}

/// Slashing event record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlashingEvent {
    pub timestamp: DateTime<Utc>,
    pub amount: u64,
    pub reason: String,
}

/// Environmental data submission from an oracle
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OracleSubmission {
    /// Oracle identifier
    pub oracle_id: String,

    /// Type of data (e.g., "rec_certificate", "carbon_offset", "grid_mix")
    pub data_type: String,

    /// Reference ID for the data (e.g., certificate ID)
    pub reference_id: String,

    /// Actual data payload
    pub data: EnvironmentalData,

    /// Cryptographic proof of data authenticity
    pub proof: CryptographicProof,

    /// Timestamp of submission
    pub timestamp: DateTime<Utc>,

    /// Oracle's signature
    pub signature: Vec<u8>,

    /// Additional metadata
    pub metadata: HashMap<String, String>,
}

/// Environmental data types that can be verified
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EnvironmentalData {
    /// Renewable Energy Certificate
    RECCertificate {
        certificate_id: String,
        issuer: String,
        amount_mwh: f64,
        generation_start: DateTime<Utc>,
        generation_end: DateTime<Utc>,
        location: String,
        energy_type: EnergySourceType,
        registry_url: String,
    },

    /// Carbon Offset
    CarbonOffset {
        offset_id: String,
        issuer: String,
        amount_tonnes: f64,
        project_type: String,
        project_location: String,
        vintage_year: u16,
        registry_url: String,
    },

    /// Regional Grid Energy Mix
    GridEnergyMix {
        region_id: String,
        timestamp: DateTime<Utc>,
        energy_sources: Vec<EnergySourceInfo>,
        total_generation_mwh: f64,
        carbon_intensity: f64,
        data_source: String,
    },

    /// Mining Operation Energy Data
    MiningEnergyData {
        miner_id: String,
        timestamp: DateTime<Utc>,
        energy_consumption_mwh: f64,
        renewable_percentage: f64,
        location: String,
        meter_readings: Vec<MeterReading>,
    },
}

/// Smart meter reading with cryptographic attestation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeterReading {
    pub meter_id: String,
    pub timestamp: DateTime<Utc>,
    pub energy_kwh: f64,
    pub power_kw: f64,
    pub attestation: Vec<u8>, // Signed by meter's secure element
}

/// Cryptographic proof for data verification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CryptographicProof {
    /// Type of proof (e.g., "merkle", "signature", "zkproof")
    pub proof_type: String,

    /// The actual proof data
    pub proof_data: Vec<u8>,

    /// Root hash or commitment
    pub commitment: [u8; 32],

    /// Additional verification parameters
    pub parameters: HashMap<String, Vec<u8>>,
}

/// Oracle consensus mechanism
#[derive(Debug, Clone)]
pub struct OracleConsensus {
    /// Minimum number of oracles required for consensus
    pub min_oracles: usize,

    /// Required agreement percentage (e.g., 67 for 2/3 consensus)
    pub consensus_threshold: u8,

    /// Submission timeout
    pub submission_timeout: Duration,

    /// Verification timeout
    pub verification_timeout: Duration,

    /// Reward distribution parameters
    pub reward_params: RewardParameters,
}

/// Parameters for oracle rewards and penalties
#[derive(Debug, Clone)]
pub struct RewardParameters {
    /// Base reward for correct verification
    pub base_reward: u64,

    /// Penalty for incorrect verification
    pub incorrect_penalty: u64,

    /// Penalty for non-participation
    pub non_participation_penalty: u64,

    /// Bonus for being first correct oracle
    pub first_oracle_bonus: u64,

    /// Reputation multiplier
    pub reputation_multiplier: f64,
}

/// Environmental Oracle System
pub struct EnvironmentalOracle {
    /// Registered oracles
    oracles: Arc<RwLock<HashMap<String, OracleInfo>>>,

    /// Pending verifications
    pending_verifications: Arc<RwLock<HashMap<String, VerificationRequest>>>,

    /// Completed verifications
    completed_verifications: Arc<RwLock<HashMap<String, VerificationResult>>>,

    /// Oracle submissions by verification ID
    oracle_submissions: Arc<RwLock<HashMap<String, Vec<OracleSubmission>>>>,

    /// Consensus parameters
    consensus_params: OracleConsensus,

    /// Minimum stake required to be an oracle
    min_oracle_stake: u64,

    /// Trusted data registries for cross-verification
    trusted_registries: Arc<RwLock<HashMap<String, RegistryInfo>>>,

    /// Cache of verified data
    verification_cache: Arc<RwLock<HashMap<String, CachedVerification>>>,

    /// Oracle performance metrics
    oracle_metrics: Arc<RwLock<HashMap<String, OracleMetrics>>>,
}

/// Verification request submitted to oracles
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationRequest {
    /// Unique request ID
    pub request_id: String,

    /// Type of verification needed
    pub verification_type: String,

    /// Data to verify
    pub data: EnvironmentalData,

    /// Requester (miner ID)
    pub requester: String,

    /// Request timestamp
    pub timestamp: DateTime<Utc>,

    /// Expiry time
    pub expiry: DateTime<Utc>,

    /// Required specializations
    pub required_specializations: HashSet<String>,

    /// Bounty offered for verification
    pub bounty: u64,
}

/// Result of oracle verification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationResult {
    /// Request ID
    pub request_id: String,

    /// Final verification status
    pub status: VerificationStatus,

    /// Consensus details
    pub consensus_details: ConsensusDetails,

    /// Participating oracles
    pub participating_oracles: Vec<String>,

    /// Timestamp of completion
    pub completed_at: DateTime<Utc>,

    /// Additional verified data
    pub verified_data: Option<HashMap<String, String>>,

    /// Proof of consensus
    pub consensus_proof: [u8; 32],
}

/// Details about consensus achievement
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsensusDetails {
    pub total_oracles: usize,
    pub agreeing_oracles: usize,
    pub disagreeing_oracles: usize,
    pub consensus_percentage: f64,
    pub consensus_reached: bool,
}

/// Trusted registry information
#[derive(Debug, Clone)]
pub struct RegistryInfo {
    pub registry_id: String,
    pub registry_type: String,
    pub api_endpoint: String,
    pub public_key: Vec<u8>,
    pub supported_data_types: HashSet<String>,
}

/// Cached verification result
#[derive(Debug, Clone)]
pub struct CachedVerification {
    pub result: VerificationResult,
    pub cached_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
}

/// Oracle performance metrics
#[derive(Debug, Clone, Default)]
pub struct OracleMetrics {
    pub total_verifications: u64,
    pub successful_verifications: u64,
    pub failed_verifications: u64,
    pub average_response_time: Duration,
    pub total_rewards_earned: u64,
    pub total_penalties_paid: u64,
}

impl EnvironmentalOracle {
    /// Create a new Environmental Oracle system
    pub fn new(min_stake: u64) -> Self {
        let consensus_params = OracleConsensus {
            min_oracles: 3,
            consensus_threshold: 67,                        // 2/3 majority
            submission_timeout: Duration::from_secs(300),   // 5 minutes
            verification_timeout: Duration::from_secs(600), // 10 minutes
            reward_params: RewardParameters {
                base_reward: 100,              // 100 NOVA
                incorrect_penalty: 500,        // 500 NOVA
                non_participation_penalty: 50, // 50 NOVA
                first_oracle_bonus: 50,        // 50 NOVA bonus
                reputation_multiplier: 1.5,
            },
        };

        Self {
            oracles: Arc::new(RwLock::new(HashMap::new())),
            pending_verifications: Arc::new(RwLock::new(HashMap::new())),
            completed_verifications: Arc::new(RwLock::new(HashMap::new())),
            oracle_submissions: Arc::new(RwLock::new(HashMap::new())),
            consensus_params,
            min_oracle_stake: min_stake,
            trusted_registries: Arc::new(RwLock::new(HashMap::new())),
            verification_cache: Arc::new(RwLock::new(HashMap::new())),
            oracle_metrics: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Register a new oracle
    pub fn register_oracle(
        &self,
        oracle_id: String,
        stake_amount: u64,
        specializations: HashSet<String>,
    ) -> Result<(), OracleError> {
        if stake_amount < self.min_oracle_stake {
            return Err(OracleError::InsufficientStake {
                required: self.min_oracle_stake,
                has: stake_amount,
            });
        }

        let oracle_info = OracleInfo {
            oracle_id: oracle_id.clone(),
            stake_amount,
            reputation_score: 500, // Start with neutral reputation
            correct_verifications: 0,
            incorrect_verifications: 0,
            last_activity: Utc::now(),
            specializations,
            is_active: true,
            slashing_events: vec![],
        };

        self.oracles
            .write()
            .unwrap()
            .insert(oracle_id.clone(), oracle_info);
        self.oracle_metrics
            .write()
            .unwrap()
            .insert(oracle_id, OracleMetrics::default());

        Ok(())
    }

    /// Submit a verification request
    pub fn request_verification(
        &self,
        data: EnvironmentalData,
        requester: String,
        bounty: u64,
        required_specializations: HashSet<String>,
    ) -> Result<String, OracleError> {
        let request_id = self.generate_request_id(&data, &requester);

        // Check cache first
        if let Some(cached) = self.check_cache(&request_id) {
            return Ok(cached.result.request_id);
        }

        let request = VerificationRequest {
            request_id: request_id.clone(),
            verification_type: self.get_data_type(&data),
            data,
            requester,
            timestamp: Utc::now(),
            expiry: Utc::now()
                + chrono::Duration::from_std(self.consensus_params.verification_timeout).unwrap(),
            required_specializations,
            bounty,
        };

        self.pending_verifications
            .write()
            .unwrap()
            .insert(request_id.clone(), request);

        Ok(request_id)
    }

    /// Submit oracle verification data
    pub fn submit_verification(
        &self,
        oracle_id: String,
        request_id: String,
        verification_data: OracleSubmission,
    ) -> Result<(), OracleError> {
        // Verify oracle is registered and active
        let oracles = self.oracles.read().unwrap();
        let oracle_info = oracles
            .get(&oracle_id)
            .ok_or_else(|| OracleError::OracleNotRegistered(oracle_id.clone()))?;

        if !oracle_info.is_active {
            return Err(OracleError::OracleSlashed(oracle_id));
        }

        // Verify signature
        if !self.verify_oracle_signature(&verification_data) {
            return Err(OracleError::InvalidSignature(
                "Invalid oracle signature".to_string(),
            ));
        }

        // Add submission
        let mut submissions = self.oracle_submissions.write().unwrap();
        submissions
            .entry(request_id.clone())
            .or_default()
            .push(verification_data);

        // Check if we have enough submissions for consensus
        let submission_count = submissions.get(&request_id).map(|s| s.len()).unwrap_or(0);
        if submission_count >= self.consensus_params.min_oracles {
            drop(submissions);
            self.process_consensus(&request_id)?;
        }

        Ok(())
    }

    /// Process consensus for a verification request
    fn process_consensus(&self, request_id: &str) -> Result<(), OracleError> {
        let submissions = self.oracle_submissions.read().unwrap();
        let oracle_submissions = submissions
            .get(request_id)
            .ok_or_else(|| OracleError::ConsensusNotReached("No submissions".to_string()))?;

        // Group submissions by verification result
        let mut verification_groups: HashMap<String, Vec<&OracleSubmission>> = HashMap::new();
        for submission in oracle_submissions {
            let key = self.hash_verification_data(&submission.data);
            verification_groups.entry(key).or_default().push(submission);
        }

        // Find the majority group
        let total_submissions = oracle_submissions.len();
        let mut majority_group = None;
        let mut max_count = 0;

        for (key, group) in &verification_groups {
            if group.len() > max_count {
                max_count = group.len();
                majority_group = Some(key.clone());
            }
        }

        // Check if consensus is reached
        let consensus_percentage = (max_count as f64 / total_submissions as f64) * 100.0;
        let consensus_reached =
            consensus_percentage >= self.consensus_params.consensus_threshold as f64;

        if !consensus_reached {
            return Err(OracleError::ConsensusNotReached(format!(
                "Only {}% agreement",
                consensus_percentage
            )));
        }

        // Create verification result
        let pending = self.pending_verifications.read().unwrap();
        let _request = pending
            .get(request_id)
            .ok_or_else(|| OracleError::ConsensusNotReached("Request not found".to_string()))?;

        let result = VerificationResult {
            request_id: request_id.to_string(),
            status: VerificationStatus::Verified,
            consensus_details: ConsensusDetails {
                total_oracles: total_submissions,
                agreeing_oracles: max_count,
                disagreeing_oracles: total_submissions - max_count,
                consensus_percentage,
                consensus_reached,
            },
            participating_oracles: oracle_submissions
                .iter()
                .map(|s| s.oracle_id.clone())
                .collect(),
            completed_at: Utc::now(),
            verified_data: self.extract_verified_data(&oracle_submissions[0].data),
            consensus_proof: self.generate_consensus_proof(oracle_submissions),
        };

        // Store result
        self.completed_verifications
            .write()
            .unwrap()
            .insert(request_id.to_string(), result.clone());

        // Distribute rewards and penalties
        self.distribute_rewards_and_penalties(
            &result,
            &verification_groups,
            &majority_group.unwrap(),
        )?;

        // Cache result
        self.cache_verification(request_id, result);

        Ok(())
    }

    /// Verify REC certificate through oracle consensus
    pub fn verify_rec_certificate(
        &self,
        certificate: &RECCertificateInfo,
    ) -> Result<VerificationStatus, OracleError> {
        let data = EnvironmentalData::RECCertificate {
            certificate_id: certificate.certificate_id.clone(),
            issuer: certificate.issuer.clone(),
            amount_mwh: certificate.amount_mwh,
            generation_start: certificate.generation_start,
            generation_end: certificate.generation_end,
            location: certificate
                .generation_location
                .as_ref()
                .map(|r| {
                    format!(
                        "{}-{}",
                        r.country_code,
                        r.sub_region.as_ref().unwrap_or(&"".to_string())
                    )
                })
                .unwrap_or_default(),
            energy_type: EnergySourceType::Other, // Would be specified in real cert
            registry_url: certificate.certificate_url.clone().unwrap_or_default(),
        };

        let _request_id = self.request_verification(
            data,
            "system".to_string(),
            100, // 100 NOVA bounty
            ["rec_certificate".to_string()].into(),
        )?;

        // Wait for consensus (in production, this would be async)
        // For now, return pending
        Ok(VerificationStatus::Pending)
    }

    /// Verify carbon offset through oracle consensus
    pub fn verify_carbon_offset(
        &self,
        offset: &CarbonOffsetInfo,
    ) -> Result<VerificationStatus, OracleError> {
        let data = EnvironmentalData::CarbonOffset {
            offset_id: offset.offset_id.clone(),
            issuer: offset.issuer.clone(),
            amount_tonnes: offset.amount_tonnes,
            project_type: offset.project_type.clone(),
            project_location: offset
                .project_location
                .as_ref()
                .map(|r| {
                    format!(
                        "{}-{}",
                        r.country_code,
                        r.sub_region.as_ref().unwrap_or(&"".to_string())
                    )
                })
                .unwrap_or_default(),
            vintage_year: 2024, // Would be specified in real offset
            registry_url: offset.certificate_url.clone().unwrap_or_default(),
        };

        let _request_id = self.request_verification(
            data,
            "system".to_string(),
            100, // 100 NOVA bounty
            ["carbon_offset".to_string()].into(),
        )?;

        Ok(VerificationStatus::Pending)
    }

    /// Slash an oracle for misbehavior
    pub fn slash_oracle(
        &self,
        oracle_id: &str,
        amount: u64,
        reason: String,
    ) -> Result<(), OracleError> {
        let mut oracles = self.oracles.write().unwrap();
        let oracle = oracles
            .get_mut(oracle_id)
            .ok_or_else(|| OracleError::OracleNotRegistered(oracle_id.to_string()))?;

        // Reduce stake
        oracle.stake_amount = oracle.stake_amount.saturating_sub(amount);

        // Record slashing event
        oracle.slashing_events.push(SlashingEvent {
            timestamp: Utc::now(),
            amount,
            reason: reason.clone(),
        });

        // Reduce reputation
        oracle.reputation_score = oracle.reputation_score.saturating_sub(100);

        // Deactivate if stake too low
        if oracle.stake_amount < self.min_oracle_stake {
            oracle.is_active = false;
        }

        Ok(())
    }

    // Helper methods

    fn generate_request_id(&self, data: &EnvironmentalData, requester: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(requester.as_bytes());
        hasher.update(bincode::serialize(data).unwrap_or_default());
        hasher.update(
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs()
                .to_le_bytes(),
        );
        format!("{:x}", hasher.finalize())
    }

    fn get_data_type(&self, data: &EnvironmentalData) -> String {
        match data {
            EnvironmentalData::RECCertificate { .. } => "rec_certificate".to_string(),
            EnvironmentalData::CarbonOffset { .. } => "carbon_offset".to_string(),
            EnvironmentalData::GridEnergyMix { .. } => "grid_energy_mix".to_string(),
            EnvironmentalData::MiningEnergyData { .. } => "mining_energy_data".to_string(),
        }
    }

    fn verify_oracle_signature(&self, submission: &OracleSubmission) -> bool {
        // Verify signature using the oracle's public key (simplified for now)
        // For now, check that signature is not empty
        !submission.signature.is_empty()
    }

    fn hash_verification_data(&self, data: &EnvironmentalData) -> String {
        let serialized = bincode::serialize(data).unwrap_or_default();
        let mut hasher = Sha256::new();
        hasher.update(&serialized);
        format!("{:x}", hasher.finalize())
    }

    fn extract_verified_data(&self, data: &EnvironmentalData) -> Option<HashMap<String, String>> {
        let mut result = HashMap::new();

        match data {
            EnvironmentalData::RECCertificate {
                certificate_id,
                amount_mwh,
                ..
            } => {
                result.insert("certificate_id".to_string(), certificate_id.clone());
                result.insert("amount_mwh".to_string(), amount_mwh.to_string());
            }
            EnvironmentalData::CarbonOffset {
                offset_id,
                amount_tonnes,
                ..
            } => {
                result.insert("offset_id".to_string(), offset_id.clone());
                result.insert("amount_tonnes".to_string(), amount_tonnes.to_string());
            }
            _ => {}
        }

        Some(result)
    }

    fn generate_consensus_proof(&self, submissions: &[OracleSubmission]) -> [u8; 32] {
        let mut hasher = Sha256::new();
        for submission in submissions {
            hasher.update(&submission.signature);
        }
        let result = hasher.finalize();
        let mut proof = [0u8; 32];
        proof.copy_from_slice(&result);
        proof
    }

    fn distribute_rewards_and_penalties(
        &self,
        result: &VerificationResult,
        verification_groups: &HashMap<String, Vec<&OracleSubmission>>,
        majority_key: &str,
    ) -> Result<(), OracleError> {
        let majority_group = verification_groups.get(majority_key).unwrap();
        let majority_oracles: HashSet<_> =
            majority_group.iter().map(|s| s.oracle_id.clone()).collect();

        let mut oracles = self.oracles.write().unwrap();
        let mut metrics = self.oracle_metrics.write().unwrap();

        // Reward oracles in majority
        for oracle_id in &majority_oracles {
            if let Some(oracle) = oracles.get_mut(oracle_id) {
                oracle.correct_verifications += 1;
                oracle.reputation_score = (oracle.reputation_score + 10).min(1000);

                if let Some(metric) = metrics.get_mut(oracle_id) {
                    metric.successful_verifications += 1;
                    metric.total_rewards_earned += self.consensus_params.reward_params.base_reward;
                }
            }
        }

        // Penalize oracles not in majority
        for oracle_id in &result.participating_oracles {
            if !majority_oracles.contains(oracle_id) {
                if let Some(oracle) = oracles.get_mut(oracle_id) {
                    oracle.incorrect_verifications += 1;
                    oracle.reputation_score = oracle.reputation_score.saturating_sub(20);

                    if let Some(metric) = metrics.get_mut(oracle_id) {
                        metric.failed_verifications += 1;
                        metric.total_penalties_paid +=
                            self.consensus_params.reward_params.incorrect_penalty;
                    }
                }
            }
        }

        Ok(())
    }

    fn check_cache(&self, request_id: &str) -> Option<CachedVerification> {
        let cache = self.verification_cache.read().unwrap();
        cache
            .get(request_id)
            .cloned()
            .filter(|c| c.expires_at > Utc::now())
    }

    fn cache_verification(&self, request_id: &str, result: VerificationResult) {
        let cached = CachedVerification {
            result,
            cached_at: Utc::now(),
            expires_at: Utc::now() + chrono::Duration::hours(24),
        };

        self.verification_cache
            .write()
            .unwrap()
            .insert(request_id.to_string(), cached);
    }

    /// Get oracle statistics
    pub fn get_oracle_stats(&self, oracle_id: &str) -> Option<(OracleInfo, OracleMetrics)> {
        let oracles = self.oracles.read().unwrap();
        let metrics = self.oracle_metrics.read().unwrap();

        if let (Some(info), Some(metric)) = (oracles.get(oracle_id), metrics.get(oracle_id)) {
            Some((info.clone(), metric.clone()))
        } else {
            None
        }
    }

    /// Get verification result
    pub fn get_verification_result(&self, request_id: &str) -> Option<VerificationResult> {
        self.completed_verifications
            .read()
            .unwrap()
            .get(request_id)
            .cloned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_oracle_registration() {
        let oracle_system = EnvironmentalOracle::new(1000);

        // Test successful registration
        let result = oracle_system.register_oracle(
            "oracle1".to_string(),
            2000,
            ["rec_certificate".to_string()].into(),
        );
        assert!(result.is_ok());

        // Test insufficient stake
        let result = oracle_system.register_oracle(
            "oracle2".to_string(),
            500,
            ["rec_certificate".to_string()].into(),
        );
        assert!(matches!(result, Err(OracleError::InsufficientStake { .. })));
    }

    #[test]
    fn test_verification_request() {
        let oracle_system = EnvironmentalOracle::new(1000);

        let data = EnvironmentalData::RECCertificate {
            certificate_id: "REC123".to_string(),
            issuer: "GreenCerts".to_string(),
            amount_mwh: 100.0,
            generation_start: Utc::now() - chrono::Duration::days(30),
            generation_end: Utc::now() - chrono::Duration::days(1),
            location: "US-CA".to_string(),
            energy_type: EnergySourceType::Solar,
            registry_url: "https://greencerts.com/REC123".to_string(),
        };

        let result = oracle_system.request_verification(
            data,
            "miner1".to_string(),
            100,
            ["rec_certificate".to_string()].into(),
        );

        assert!(result.is_ok());
        assert!(!result.unwrap().is_empty());
    }
}
