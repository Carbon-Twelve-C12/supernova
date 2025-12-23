//! Oracle Registry System for Supernova Blockchain
//!
//! This module implements a decentralized oracle registry with:
//! - Staking verification for oracle registration
//! - Governance voting for oracle approval
//! - Slashing conditions for misbehavior
//! - Reputation-based oracle selection
//!
//! # Architecture
//! - `OracleRegistry` - Main registry managing oracle lifecycle
//! - `OracleApplication` - Application to become an oracle
//! - `GovernanceVote` - Voting mechanism for oracle approval
//! - `SlashingCondition` - Conditions that trigger oracle penalties

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, RwLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use thiserror::Error;
use tracing::{debug, error, info, warn};

use super::oracle::{OracleInfo, SlashingEvent};

/// Minimum stake required to apply as oracle (in NOVA)
pub const MIN_ORACLE_STAKE: u64 = 10_000;

/// Minimum stake required to vote on oracle applications
pub const MIN_VOTER_STAKE: u64 = 100;

/// Voting period in seconds (7 days)
pub const VOTING_PERIOD_SECS: u64 = 7 * 24 * 60 * 60;

/// Minimum approval percentage required (66%)
pub const MIN_APPROVAL_PERCENT: u8 = 66;

/// Minimum number of votes required for quorum
pub const MIN_VOTES_FOR_QUORUM: usize = 10;

/// Grace period before slashing (in seconds)
pub const SLASHING_GRACE_PERIOD_SECS: u64 = 24 * 60 * 60;

/// Registry errors
#[derive(Debug, Error, Clone)]
pub enum RegistryError {
    #[error("Oracle not found: {oracle_id}")]
    OracleNotFound { oracle_id: String },

    #[error("Application not found: {application_id}")]
    ApplicationNotFound { application_id: String },

    #[error("Insufficient stake: required {required}, have {actual}")]
    InsufficientStake { required: u64, actual: u64 },

    #[error("Already registered: {oracle_id}")]
    AlreadyRegistered { oracle_id: String },

    #[error("Application pending: {application_id}")]
    ApplicationPending { application_id: String },

    #[error("Voting period ended")]
    VotingPeriodEnded,

    #[error("Voting period active")]
    VotingPeriodActive,

    #[error("Already voted: {voter_id}")]
    AlreadyVoted { voter_id: String },

    #[error("Invalid vote: {reason}")]
    InvalidVote { reason: String },

    #[error("Quorum not reached: {votes} < {required}")]
    QuorumNotReached { votes: usize, required: usize },

    #[error("Oracle slashed: {oracle_id}")]
    OracleSlashed { oracle_id: String },

    #[error("Lock poisoned")]
    LockPoisoned,
}

/// Result type for registry operations
pub type RegistryResult<T> = Result<T, RegistryError>;

/// Oracle application status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ApplicationStatus {
    /// Application submitted, awaiting voting
    Pending,
    /// Voting in progress
    Voting,
    /// Application approved
    Approved,
    /// Application rejected
    Rejected,
    /// Application expired (no quorum)
    Expired,
}

/// Oracle application to join the registry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OracleApplication {
    /// Unique application ID
    pub application_id: String,
    /// Applicant's public key / ID
    pub applicant_id: String,
    /// Stake amount pledged
    pub stake_amount: u64,
    /// Specializations claimed
    pub specializations: HashSet<String>,
    /// Description of oracle capabilities
    pub description: String,
    /// External verification URLs (website, audit reports)
    pub verification_urls: Vec<String>,
    /// Submission timestamp
    pub submitted_at: u64,
    /// Voting deadline
    pub voting_deadline: u64,
    /// Current status
    pub status: ApplicationStatus,
    /// Votes received
    pub votes: Vec<GovernanceVote>,
}

impl OracleApplication {
    /// Calculate approval percentage
    pub fn approval_percentage(&self) -> f64 {
        if self.votes.is_empty() {
            return 0.0;
        }

        let approve_weight: u64 = self.votes.iter()
            .filter(|v| v.approve)
            .map(|v| v.stake_weight)
            .sum();

        let total_weight: u64 = self.votes.iter()
            .map(|v| v.stake_weight)
            .sum();

        if total_weight == 0 {
            return 0.0;
        }

        (approve_weight as f64 / total_weight as f64) * 100.0
    }

    /// Check if quorum is reached
    pub fn has_quorum(&self) -> bool {
        self.votes.len() >= MIN_VOTES_FOR_QUORUM
    }

    /// Check if voting period is over
    pub fn is_voting_ended(&self) -> bool {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or(Duration::ZERO)
            .as_secs();
        now > self.voting_deadline
    }
}

/// Governance vote on oracle application
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GovernanceVote {
    /// Voter's ID
    pub voter_id: String,
    /// Vote decision (true = approve)
    pub approve: bool,
    /// Voter's stake weight
    pub stake_weight: u64,
    /// Vote timestamp
    pub timestamp: u64,
    /// Optional comment
    pub comment: Option<String>,
    /// Vote signature
    pub signature: Vec<u8>,
}

/// Slashing condition types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SlashingConditionType {
    /// Provided false data
    FalseData,
    /// Collusion with other oracles
    Collusion,
    /// Non-participation in required verifications
    NonParticipation,
    /// Signature mismatch
    SignatureMismatch,
    /// Data manipulation
    DataManipulation,
    /// Front-running
    FrontRunning,
    /// Excessive downtime
    ExcessiveDowntime,
}

/// Slashing condition with severity
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlashingCondition {
    /// Condition type
    pub condition_type: SlashingConditionType,
    /// Severity (1-10)
    pub severity: u8,
    /// Base slash percentage (basis points)
    pub slash_bps: u16,
    /// Description
    pub description: String,
    /// Minimum stake to trigger
    pub min_stake_trigger: u64,
    /// Cooldown period before next slash (seconds)
    pub cooldown_secs: u64,
}

impl Default for SlashingCondition {
    fn default() -> Self {
        Self {
            condition_type: SlashingConditionType::NonParticipation,
            severity: 3,
            slash_bps: 500, // 5%
            description: "Non-participation in verification".to_string(),
            min_stake_trigger: 0,
            cooldown_secs: 86400, // 24 hours
        }
    }
}

/// Slashing proposal (for contested slashes)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlashingProposal {
    /// Proposal ID
    pub proposal_id: String,
    /// Oracle being slashed
    pub oracle_id: String,
    /// Condition that triggered slash
    pub condition: SlashingCondition,
    /// Evidence provided
    pub evidence: Vec<SlashingEvidence>,
    /// Proposer ID
    pub proposer_id: String,
    /// Created at
    pub created_at: u64,
    /// Grace period ends at
    pub grace_period_ends: u64,
    /// Is contested
    pub contested: bool,
    /// Contest votes
    pub contest_votes: Vec<GovernanceVote>,
    /// Executed
    pub executed: bool,
}

/// Evidence for slashing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlashingEvidence {
    /// Evidence type
    pub evidence_type: String,
    /// Data hash
    pub data_hash: [u8; 32],
    /// Description
    pub description: String,
    /// Timestamp
    pub timestamp: u64,
}

/// Registered oracle with extended information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisteredOracle {
    /// Oracle info from base system
    pub info: OracleInfo,
    /// Registration timestamp
    pub registered_at: u64,
    /// Application that approved this oracle
    pub application_id: String,
    /// Total verifications performed
    pub total_verifications: u64,
    /// Successful verifications
    pub successful_verifications: u64,
    /// Failed verifications
    pub failed_verifications: u64,
    /// Total rewards earned
    pub total_rewards: u64,
    /// Total penalties paid
    pub total_penalties: u64,
    /// Last activity timestamp
    pub last_activity: u64,
    /// Is suspended
    pub suspended: bool,
    /// Suspension reason
    pub suspension_reason: Option<String>,
    /// Pending slashing proposals
    pub pending_slashes: Vec<String>,
}

/// Registry configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistryConfig {
    /// Minimum stake for oracle registration
    pub min_oracle_stake: u64,
    /// Minimum stake for voting
    pub min_voter_stake: u64,
    /// Voting period in seconds
    pub voting_period_secs: u64,
    /// Minimum approval percentage
    pub min_approval_percent: u8,
    /// Minimum votes for quorum
    pub min_votes_for_quorum: usize,
    /// Slashing grace period
    pub slashing_grace_period_secs: u64,
    /// Enable governance voting
    pub governance_enabled: bool,
}

impl Default for RegistryConfig {
    fn default() -> Self {
        Self {
            min_oracle_stake: MIN_ORACLE_STAKE,
            min_voter_stake: MIN_VOTER_STAKE,
            voting_period_secs: VOTING_PERIOD_SECS,
            min_approval_percent: MIN_APPROVAL_PERCENT,
            min_votes_for_quorum: MIN_VOTES_FOR_QUORUM,
            slashing_grace_period_secs: SLASHING_GRACE_PERIOD_SECS,
            governance_enabled: true,
        }
    }
}

/// Oracle Registry - Main registry for managing oracles
pub struct OracleRegistry {
    /// Configuration
    config: RegistryConfig,
    /// Registered oracles
    oracles: Arc<RwLock<HashMap<String, RegisteredOracle>>>,
    /// Pending applications
    applications: Arc<RwLock<HashMap<String, OracleApplication>>>,
    /// Slashing conditions
    slashing_conditions: Arc<RwLock<HashMap<SlashingConditionType, SlashingCondition>>>,
    /// Pending slashing proposals
    slashing_proposals: Arc<RwLock<HashMap<String, SlashingProposal>>>,
    /// Voter stake balances (would come from staking module in production)
    voter_stakes: Arc<RwLock<HashMap<String, u64>>>,
}

impl OracleRegistry {
    /// Create a new oracle registry
    pub fn new(config: RegistryConfig) -> Self {
        let registry = Self {
            config,
            oracles: Arc::new(RwLock::new(HashMap::new())),
            applications: Arc::new(RwLock::new(HashMap::new())),
            slashing_conditions: Arc::new(RwLock::new(HashMap::new())),
            slashing_proposals: Arc::new(RwLock::new(HashMap::new())),
            voter_stakes: Arc::new(RwLock::new(HashMap::new())),
        };

        // Initialize default slashing conditions
        registry.init_slashing_conditions();

        registry
    }

    /// Initialize with default configuration
    pub fn with_defaults() -> Self {
        Self::new(RegistryConfig::default())
    }

    /// Initialize default slashing conditions
    fn init_slashing_conditions(&self) {
        let mut conditions = self.slashing_conditions.write()
            .expect("Lock poisoned during initialization");

        conditions.insert(
            SlashingConditionType::FalseData,
            SlashingCondition {
                condition_type: SlashingConditionType::FalseData,
                severity: 10,
                slash_bps: 5000, // 50%
                description: "Provided verifiably false data".to_string(),
                min_stake_trigger: 0,
                cooldown_secs: 0, // No cooldown for false data
            },
        );

        conditions.insert(
            SlashingConditionType::Collusion,
            SlashingCondition {
                condition_type: SlashingConditionType::Collusion,
                severity: 10,
                slash_bps: 10000, // 100%
                description: "Provable collusion with other oracles".to_string(),
                min_stake_trigger: 0,
                cooldown_secs: 0,
            },
        );

        conditions.insert(
            SlashingConditionType::NonParticipation,
            SlashingCondition {
                condition_type: SlashingConditionType::NonParticipation,
                severity: 3,
                slash_bps: 100, // 1%
                description: "Failed to participate in required verification".to_string(),
                min_stake_trigger: 1000,
                cooldown_secs: 86400,
            },
        );

        conditions.insert(
            SlashingConditionType::SignatureMismatch,
            SlashingCondition {
                condition_type: SlashingConditionType::SignatureMismatch,
                severity: 5,
                slash_bps: 1000, // 10%
                description: "Signature verification failed".to_string(),
                min_stake_trigger: 0,
                cooldown_secs: 3600,
            },
        );

        conditions.insert(
            SlashingConditionType::DataManipulation,
            SlashingCondition {
                condition_type: SlashingConditionType::DataManipulation,
                severity: 8,
                slash_bps: 3000, // 30%
                description: "Attempted data manipulation".to_string(),
                min_stake_trigger: 0,
                cooldown_secs: 0,
            },
        );

        conditions.insert(
            SlashingConditionType::FrontRunning,
            SlashingCondition {
                condition_type: SlashingConditionType::FrontRunning,
                severity: 7,
                slash_bps: 2000, // 20%
                description: "Front-running verification submissions".to_string(),
                min_stake_trigger: 0,
                cooldown_secs: 0,
            },
        );

        conditions.insert(
            SlashingConditionType::ExcessiveDowntime,
            SlashingCondition {
                condition_type: SlashingConditionType::ExcessiveDowntime,
                severity: 2,
                slash_bps: 50, // 0.5%
                description: "Excessive downtime (>24h without participation)".to_string(),
                min_stake_trigger: 1000,
                cooldown_secs: 604800, // 7 days
            },
        );
    }

    /// Submit an oracle application
    pub fn submit_application(
        &self,
        applicant_id: String,
        stake_amount: u64,
        specializations: HashSet<String>,
        description: String,
        verification_urls: Vec<String>,
    ) -> RegistryResult<String> {
        // Verify minimum stake
        if stake_amount < self.config.min_oracle_stake {
            return Err(RegistryError::InsufficientStake {
                required: self.config.min_oracle_stake,
                actual: stake_amount,
            });
        }

        // Check if already registered
        {
            let oracles = self.oracles.read()
                .map_err(|_| RegistryError::LockPoisoned)?;
            if oracles.contains_key(&applicant_id) {
                return Err(RegistryError::AlreadyRegistered {
                    oracle_id: applicant_id,
                });
            }
        }

        // Check if application already pending
        {
            let applications = self.applications.read()
                .map_err(|_| RegistryError::LockPoisoned)?;
            for app in applications.values() {
                if app.applicant_id == applicant_id && app.status == ApplicationStatus::Pending {
                    return Err(RegistryError::ApplicationPending {
                        application_id: app.application_id.clone(),
                    });
                }
            }
        }

        // Generate application ID
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or(Duration::ZERO)
            .as_secs();

        let mut hasher = Sha256::new();
        hasher.update(applicant_id.as_bytes());
        hasher.update(&now.to_le_bytes());
        let hash = hasher.finalize();
        let application_id = format!("app_{}", hex::encode(&hash[..8]));

        // Create application
        let application = OracleApplication {
            application_id: application_id.clone(),
            applicant_id,
            stake_amount,
            specializations,
            description,
            verification_urls,
            submitted_at: now,
            voting_deadline: now + self.config.voting_period_secs,
            status: if self.config.governance_enabled {
                ApplicationStatus::Voting
            } else {
                ApplicationStatus::Approved // Auto-approve if governance disabled
            },
            votes: Vec::new(),
        };

        // Store application
        {
            let mut applications = self.applications.write()
                .map_err(|_| RegistryError::LockPoisoned)?;
            applications.insert(application_id.clone(), application);
        }

        // If governance disabled, auto-register
        if !self.config.governance_enabled {
            self.finalize_application(&application_id)?;
        }

        info!("Oracle application submitted: {}", application_id);

        Ok(application_id)
    }

    /// Vote on an oracle application
    pub fn vote_on_application(
        &self,
        application_id: &str,
        voter_id: String,
        approve: bool,
        comment: Option<String>,
        signature: Vec<u8>,
    ) -> RegistryResult<()> {
        // Get voter stake
        let stake_weight = {
            let stakes = self.voter_stakes.read()
                .map_err(|_| RegistryError::LockPoisoned)?;
            stakes.get(&voter_id).copied().unwrap_or(0)
        };

        if stake_weight < self.config.min_voter_stake {
            return Err(RegistryError::InsufficientStake {
                required: self.config.min_voter_stake,
                actual: stake_weight,
            });
        }

        let mut applications = self.applications.write()
            .map_err(|_| RegistryError::LockPoisoned)?;

        let application = applications.get_mut(application_id)
            .ok_or_else(|| RegistryError::ApplicationNotFound {
                application_id: application_id.to_string(),
            })?;

        // Check if voting period is active
        if application.is_voting_ended() {
            return Err(RegistryError::VotingPeriodEnded);
        }

        // Check if already voted
        if application.votes.iter().any(|v| v.voter_id == voter_id) {
            return Err(RegistryError::AlreadyVoted { voter_id });
        }

        // Add vote
        let vote = GovernanceVote {
            voter_id,
            approve,
            stake_weight,
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or(Duration::ZERO)
                .as_secs(),
            comment,
            signature,
        };

        application.votes.push(vote);

        debug!(
            "Vote recorded for application {}: {} ({} stake)",
            application_id,
            if approve { "approve" } else { "reject" },
            stake_weight
        );

        Ok(())
    }

    /// Finalize an application after voting period
    pub fn finalize_application(&self, application_id: &str) -> RegistryResult<bool> {
        let mut applications = self.applications.write()
            .map_err(|_| RegistryError::LockPoisoned)?;

        let application = applications.get_mut(application_id)
            .ok_or_else(|| RegistryError::ApplicationNotFound {
                application_id: application_id.to_string(),
            })?;

        // Check if voting period ended (skip for auto-approved)
        if self.config.governance_enabled && !application.is_voting_ended() {
            return Err(RegistryError::VotingPeriodActive);
        }

        // Check quorum (skip for auto-approved)
        if self.config.governance_enabled && !application.has_quorum() {
            application.status = ApplicationStatus::Expired;
            warn!("Application {} expired: quorum not reached", application_id);
            return Ok(false);
        }

        // Check approval percentage
        let approval = application.approval_percentage();
        if self.config.governance_enabled && approval < self.config.min_approval_percent as f64 {
            application.status = ApplicationStatus::Rejected;
            warn!(
                "Application {} rejected: {:.1}% < {}%",
                application_id, approval, self.config.min_approval_percent
            );
            return Ok(false);
        }

        // Approve and register
        application.status = ApplicationStatus::Approved;

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or(Duration::ZERO)
            .as_secs();

        let oracle_info = OracleInfo {
            oracle_id: application.applicant_id.clone(),
            stake_amount: application.stake_amount,
            reputation_score: 500, // Start with neutral
            correct_verifications: 0,
            incorrect_verifications: 0,
            last_activity: Utc::now(),
            specializations: application.specializations.clone(),
            is_active: true,
            slashing_events: vec![],
        };

        let registered_oracle = RegisteredOracle {
            info: oracle_info,
            registered_at: now,
            application_id: application_id.to_string(),
            total_verifications: 0,
            successful_verifications: 0,
            failed_verifications: 0,
            total_rewards: 0,
            total_penalties: 0,
            last_activity: now,
            suspended: false,
            suspension_reason: None,
            pending_slashes: vec![],
        };

        // Store in oracles
        {
            let mut oracles = self.oracles.write()
                .map_err(|_| RegistryError::LockPoisoned)?;
            oracles.insert(application.applicant_id.clone(), registered_oracle);
        }

        info!(
            "Oracle {} registered (approval: {:.1}%)",
            application.applicant_id, approval
        );

        Ok(true)
    }

    /// Propose slashing an oracle
    pub fn propose_slash(
        &self,
        oracle_id: &str,
        condition_type: SlashingConditionType,
        evidence: Vec<SlashingEvidence>,
        proposer_id: String,
    ) -> RegistryResult<String> {
        // Verify oracle exists
        {
            let oracles = self.oracles.read()
                .map_err(|_| RegistryError::LockPoisoned)?;
            if !oracles.contains_key(oracle_id) {
                return Err(RegistryError::OracleNotFound {
                    oracle_id: oracle_id.to_string(),
                });
            }
        }

        // Get slashing condition
        let condition = {
            let conditions = self.slashing_conditions.read()
                .map_err(|_| RegistryError::LockPoisoned)?;
            conditions.get(&condition_type).cloned()
                .unwrap_or_default()
        };

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or(Duration::ZERO)
            .as_secs();

        // Generate proposal ID
        let mut hasher = Sha256::new();
        hasher.update(oracle_id.as_bytes());
        hasher.update(&now.to_le_bytes());
        let hash = hasher.finalize();
        let proposal_id = format!("slash_{}", hex::encode(&hash[..8]));

        let proposal = SlashingProposal {
            proposal_id: proposal_id.clone(),
            oracle_id: oracle_id.to_string(),
            condition,
            evidence,
            proposer_id,
            created_at: now,
            grace_period_ends: now + self.config.slashing_grace_period_secs,
            contested: false,
            contest_votes: vec![],
            executed: false,
        };

        // Store proposal
        {
            let mut proposals = self.slashing_proposals.write()
                .map_err(|_| RegistryError::LockPoisoned)?;
            proposals.insert(proposal_id.clone(), proposal);
        }

        // Add to oracle's pending slashes
        {
            let mut oracles = self.oracles.write()
                .map_err(|_| RegistryError::LockPoisoned)?;
            if let Some(oracle) = oracles.get_mut(oracle_id) {
                oracle.pending_slashes.push(proposal_id.clone());
            }
        }

        warn!("Slashing proposal {} created for oracle {}", proposal_id, oracle_id);

        Ok(proposal_id)
    }

    /// Contest a slashing proposal
    pub fn contest_slash(
        &self,
        proposal_id: &str,
        voter_id: String,
        uphold_slash: bool,
        comment: Option<String>,
        signature: Vec<u8>,
    ) -> RegistryResult<()> {
        // Get voter stake
        let stake_weight = {
            let stakes = self.voter_stakes.read()
                .map_err(|_| RegistryError::LockPoisoned)?;
            stakes.get(&voter_id).copied().unwrap_or(0)
        };

        if stake_weight < self.config.min_voter_stake {
            return Err(RegistryError::InsufficientStake {
                required: self.config.min_voter_stake,
                actual: stake_weight,
            });
        }

        let mut proposals = self.slashing_proposals.write()
            .map_err(|_| RegistryError::LockPoisoned)?;

        let proposal = proposals.get_mut(proposal_id)
            .ok_or_else(|| RegistryError::ApplicationNotFound {
                application_id: proposal_id.to_string(),
            })?;

        // Check grace period
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or(Duration::ZERO)
            .as_secs();

        if now > proposal.grace_period_ends {
            return Err(RegistryError::VotingPeriodEnded);
        }

        // Mark as contested
        proposal.contested = true;

        // Add vote
        let vote = GovernanceVote {
            voter_id,
            approve: uphold_slash,
            stake_weight,
            timestamp: now,
            comment,
            signature,
        };

        proposal.contest_votes.push(vote);

        Ok(())
    }

    /// Execute a slashing proposal
    pub fn execute_slash(&self, proposal_id: &str) -> RegistryResult<u64> {
        let mut proposals = self.slashing_proposals.write()
            .map_err(|_| RegistryError::LockPoisoned)?;

        let proposal = proposals.get_mut(proposal_id)
            .ok_or_else(|| RegistryError::ApplicationNotFound {
                application_id: proposal_id.to_string(),
            })?;

        // Check grace period
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or(Duration::ZERO)
            .as_secs();

        if now < proposal.grace_period_ends && !proposal.contest_votes.is_empty() {
            // If contested, need to wait for voting
            return Err(RegistryError::VotingPeriodActive);
        }

        // If contested, check votes
        if proposal.contested && !proposal.contest_votes.is_empty() {
            let uphold_weight: u64 = proposal.contest_votes.iter()
                .filter(|v| v.approve)
                .map(|v| v.stake_weight)
                .sum();

            let total_weight: u64 = proposal.contest_votes.iter()
                .map(|v| v.stake_weight)
                .sum();

            let uphold_percent = if total_weight > 0 {
                (uphold_weight as f64 / total_weight as f64) * 100.0
            } else {
                100.0 // Default to uphold if no votes
            };

            if uphold_percent < 50.0 {
                // Slash overturned
                proposal.executed = true;
                info!("Slashing proposal {} overturned by vote", proposal_id);
                return Ok(0);
            }
        }

        // Execute slash
        let oracle_id = proposal.oracle_id.clone();
        let slash_bps = proposal.condition.slash_bps;

        let slashed_amount = {
            let mut oracles = self.oracles.write()
                .map_err(|_| RegistryError::LockPoisoned)?;

            let oracle = oracles.get_mut(&oracle_id)
                .ok_or_else(|| RegistryError::OracleNotFound {
                    oracle_id: oracle_id.clone(),
                })?;

            let slash_amount = (oracle.info.stake_amount as u128 * slash_bps as u128 / 10000) as u64;
            oracle.info.stake_amount = oracle.info.stake_amount.saturating_sub(slash_amount);
            oracle.total_penalties += slash_amount;

            // Record slashing event
            oracle.info.slashing_events.push(SlashingEvent {
                timestamp: Utc::now(),
                amount: slash_amount,
                reason: proposal.condition.description.clone(),
            });

            // Reduce reputation
            let reputation_penalty = proposal.condition.severity as u32 * 10;
            oracle.info.reputation_score = oracle.info.reputation_score.saturating_sub(reputation_penalty);

            // Remove from pending
            oracle.pending_slashes.retain(|p| p != proposal_id);

            // Suspend if stake too low
            if oracle.info.stake_amount < self.config.min_oracle_stake {
                oracle.suspended = true;
                oracle.suspension_reason = Some("Stake below minimum".to_string());
                oracle.info.is_active = false;
            }

            slash_amount
        };

        proposal.executed = true;

        warn!(
            "Executed slash on oracle {}: {} NOVA",
            oracle_id, slashed_amount
        );

        Ok(slashed_amount)
    }

    /// Register voter stake (in production, would read from staking module)
    pub fn register_voter_stake(&self, voter_id: String, stake: u64) -> RegistryResult<()> {
        let mut stakes = self.voter_stakes.write()
            .map_err(|_| RegistryError::LockPoisoned)?;
        stakes.insert(voter_id, stake);
        Ok(())
    }

    /// Get oracle by ID
    pub fn get_oracle(&self, oracle_id: &str) -> RegistryResult<Option<RegisteredOracle>> {
        let oracles = self.oracles.read()
            .map_err(|_| RegistryError::LockPoisoned)?;
        Ok(oracles.get(oracle_id).cloned())
    }

    /// Get application by ID
    pub fn get_application(&self, application_id: &str) -> RegistryResult<Option<OracleApplication>> {
        let applications = self.applications.read()
            .map_err(|_| RegistryError::LockPoisoned)?;
        Ok(applications.get(application_id).cloned())
    }

    /// List all active oracles
    pub fn list_active_oracles(&self) -> RegistryResult<Vec<RegisteredOracle>> {
        let oracles = self.oracles.read()
            .map_err(|_| RegistryError::LockPoisoned)?;

        Ok(oracles.values()
            .filter(|o| o.info.is_active && !o.suspended)
            .cloned()
            .collect())
    }

    /// List pending applications
    pub fn list_pending_applications(&self) -> RegistryResult<Vec<OracleApplication>> {
        let applications = self.applications.read()
            .map_err(|_| RegistryError::LockPoisoned)?;

        Ok(applications.values()
            .filter(|a| a.status == ApplicationStatus::Voting && !a.is_voting_ended())
            .cloned()
            .collect())
    }

    /// Get registry statistics
    pub fn get_stats(&self) -> RegistryResult<RegistryStats> {
        let oracles = self.oracles.read()
            .map_err(|_| RegistryError::LockPoisoned)?;
        let applications = self.applications.read()
            .map_err(|_| RegistryError::LockPoisoned)?;
        let proposals = self.slashing_proposals.read()
            .map_err(|_| RegistryError::LockPoisoned)?;

        let total_stake: u64 = oracles.values()
            .map(|o| o.info.stake_amount)
            .sum();

        Ok(RegistryStats {
            total_oracles: oracles.len(),
            active_oracles: oracles.values().filter(|o| o.info.is_active && !o.suspended).count(),
            pending_applications: applications.values()
                .filter(|a| a.status == ApplicationStatus::Voting)
                .count(),
            total_stake,
            pending_slashes: proposals.values().filter(|p| !p.executed).count(),
        })
    }

    /// Get configuration
    pub fn config(&self) -> &RegistryConfig {
        &self.config
    }
}

/// Registry statistics
#[derive(Debug, Clone)]
pub struct RegistryStats {
    pub total_oracles: usize,
    pub active_oracles: usize,
    pub pending_applications: usize,
    pub total_stake: u64,
    pub pending_slashes: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_registry() -> OracleRegistry {
        let config = RegistryConfig {
            governance_enabled: true,
            min_votes_for_quorum: 2, // Lower for testing
            ..Default::default()
        };
        OracleRegistry::new(config)
    }

    #[test]
    fn test_application_submission() {
        let registry = create_test_registry();

        let result = registry.submit_application(
            "oracle_001".to_string(),
            15_000,
            ["environmental".to_string()].into_iter().collect(),
            "Test oracle".to_string(),
            vec![],
        );

        assert!(result.is_ok());
        let app_id = result.unwrap();
        assert!(app_id.starts_with("app_"));
    }

    #[test]
    fn test_insufficient_stake() {
        let registry = create_test_registry();

        let result = registry.submit_application(
            "oracle_002".to_string(),
            5_000, // Below minimum
            HashSet::new(),
            "Test".to_string(),
            vec![],
        );

        assert!(matches!(result, Err(RegistryError::InsufficientStake { .. })));
    }

    #[test]
    fn test_voting_on_application() {
        let registry = create_test_registry();

        // Register voter stake
        registry.register_voter_stake("voter_001".to_string(), 1000).unwrap();
        registry.register_voter_stake("voter_002".to_string(), 2000).unwrap();

        // Submit application
        let app_id = registry.submit_application(
            "oracle_003".to_string(),
            15_000,
            HashSet::new(),
            "Test".to_string(),
            vec![],
        ).unwrap();

        // Vote
        registry.vote_on_application(
            &app_id,
            "voter_001".to_string(),
            true,
            None,
            vec![1, 2, 3],
        ).unwrap();

        registry.vote_on_application(
            &app_id,
            "voter_002".to_string(),
            true,
            Some("Good oracle".to_string()),
            vec![4, 5, 6],
        ).unwrap();

        // Check application
        let app = registry.get_application(&app_id).unwrap().unwrap();
        assert_eq!(app.votes.len(), 2);
        assert_eq!(app.approval_percentage(), 100.0);
        assert!(app.has_quorum());
    }

    #[test]
    fn test_slashing_proposal() {
        let registry = OracleRegistry::new(RegistryConfig {
            governance_enabled: false, // Auto-approve for testing
            slashing_grace_period_secs: 0, // Immediate execution
            ..Default::default()
        });

        // Submit and auto-approve application
        let app_id = registry.submit_application(
            "oracle_004".to_string(),
            15_000,
            HashSet::new(),
            "Test".to_string(),
            vec![],
        ).unwrap();

        // Finalize (auto-approved)
        registry.finalize_application(&app_id).unwrap();

        // Verify oracle exists
        let oracle = registry.get_oracle("oracle_004").unwrap();
        assert!(oracle.is_some());

        // Propose slash
        let proposal_id = registry.propose_slash(
            "oracle_004",
            SlashingConditionType::FalseData,
            vec![SlashingEvidence {
                evidence_type: "test".to_string(),
                data_hash: [0u8; 32],
                description: "Test evidence".to_string(),
                timestamp: 0,
            }],
            "reporter_001".to_string(),
        ).unwrap();

        // Execute slash
        let slashed = registry.execute_slash(&proposal_id).unwrap();
        assert!(slashed > 0);

        // Verify stake reduced
        let oracle = registry.get_oracle("oracle_004").unwrap().unwrap();
        assert!(oracle.info.stake_amount < 15_000);
    }

    #[test]
    fn test_registry_stats() {
        let registry = OracleRegistry::new(RegistryConfig {
            governance_enabled: false,
            ..Default::default()
        });

        // Register oracle
        let app_id = registry.submit_application(
            "oracle_005".to_string(),
            15_000,
            HashSet::new(),
            "Test".to_string(),
            vec![],
        ).unwrap();

        registry.finalize_application(&app_id).unwrap();

        let stats = registry.get_stats().unwrap();
        assert_eq!(stats.total_oracles, 1);
        assert_eq!(stats.active_oracles, 1);
        assert_eq!(stats.total_stake, 15_000);
    }
}
