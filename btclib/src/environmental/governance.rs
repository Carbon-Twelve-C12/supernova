use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;
use uuid;

use crate::crypto::signature::Signature;
use crate::environmental::treasury::{EnvironmentalTreasury, TreasuryAccountType, TreasuryError};

/// Error types for environmental governance operations
#[derive(Error, Debug)]
pub enum GovernanceError {
    #[error("Invalid proposal: {0}")]
    InvalidProposal(String),

    #[error("Proposal not found: {0}")]
    ProposalNotFound(String),

    #[error("Unauthorized vote: {0}")]
    UnauthorizedVote(String),

    #[error("Voting period ended: {0}")]
    VotingPeriodEnded(String),

    #[error("Treasury error: {0}")]
    TreasuryError(#[from] TreasuryError),

    #[error("Invalid signature: {0}")]
    InvalidSignature(String),

    #[error("Invalid allocation: {0}")]
    InvalidAllocation(String),

    #[error("Quorum not reached: {0}")]
    QuorumNotReached(String),
}

/// Status of an environmental proposal
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ProposalStatus {
    /// Proposal is active and accepting votes
    Active,
    /// Proposal has been approved
    Approved,
    /// Proposal has been rejected
    Rejected,
    /// Proposal has been executed
    Executed,
    /// Proposal has been cancelled
    Cancelled,
}

/// Types of environmental proposals
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ProposalType {
    /// Allocation of treasury funds
    TreasuryAllocation {
        /// Account to allocate funds from
        from_account: TreasuryAccountType,
        /// Account to allocate funds to
        to_account: TreasuryAccountType,
        /// Amount to allocate
        amount: u64,
    },
    /// Change fee allocation percentage
    ChangeFeeAllocation {
        /// New fee allocation percentage
        new_percentage: f64,
    },
    /// Purchase renewable energy certificates
    PurchaseRECs {
        /// Amount to spend on RECs
        amount: u64,
        /// Target provider
        provider: String,
    },
    /// Purchase carbon offsets
    PurchaseOffsets {
        /// Amount to spend on carbon offsets
        amount: u64,
        /// Target provider
        provider: String,
    },
    /// Fund environmental project
    FundProject {
        /// Project name
        project_name: String,
        /// Project description
        description: String,
        /// Amount to fund
        amount: u64,
        /// Recipient address
        recipient: String,
    },
    /// Other proposal with custom description
    Other {
        /// Description of the proposal
        description: String,
        /// Amount involved (if applicable)
        amount: Option<u64>,
    },
}

/// Environmental governance proposal
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvironmentalProposal {
    /// Unique ID of the proposal
    pub id: String,
    /// Title of the proposal
    pub title: String,
    /// Detailed description of the proposal
    pub description: String,
    /// Type of proposal
    pub proposal_type: ProposalType,
    /// Proposer's address or identifier
    pub proposer: String,
    /// Creation timestamp
    pub created_at: DateTime<Utc>,
    /// End of voting period
    pub voting_ends_at: DateTime<Utc>,
    /// Current status of the proposal
    pub status: ProposalStatus,
    /// Votes for the proposal
    pub votes_for: HashMap<String, Box<Vote>>,
    /// Votes against the proposal
    pub votes_against: HashMap<String, Box<Vote>>,
    /// Execution timestamp (if executed)
    pub executed_at: Option<DateTime<Utc>>,
    /// Transaction hash (if executed)
    pub execution_tx_hash: Option<String>,
    /// URL for additional information
    pub url: Option<String>,
}

/// Vote on an environmental proposal
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Vote {
    /// Voter address or identifier
    pub voter: String,
    /// Timestamp of the vote
    pub timestamp: DateTime<Utc>,
    /// Vote weight (based on stake, reputation, etc.)
    pub weight: u64,
    /// Optional comment with the vote
    pub comment: Option<String>,
    /// Signature of the vote
    pub signature: Signature,
}

/// Environmental governance configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GovernanceConfig {
    /// Required quorum percentage for proposal approval
    pub quorum_percentage: f64,
    /// Minimum approval percentage required
    pub approval_threshold: f64,
    /// Voting period duration in days
    pub voting_period_days: u32,
    /// Whether to allow emergency proposals with shorter voting periods
    pub allow_emergency_proposals: bool,
    /// Minimum time before allocation takes effect (days)
    pub time_lock_days: u32,
    /// Maximum allocation percentage change per proposal
    pub max_allocation_change: f64,
    /// Authorized governance participants
    pub authorized_voters: Vec<String>,
    /// Participant weights (if not specified, default to 1)
    pub voter_weights: HashMap<String, u64>,
}

impl Default for GovernanceConfig {
    fn default() -> Self {
        Self {
            quorum_percentage: 33.0,         // 33% quorum required
            approval_threshold: 66.0,        // 66% approval required
            voting_period_days: 7,           // 7 day voting period
            allow_emergency_proposals: true, // Allow emergency proposals
            time_lock_days: 2,               // 2 day time lock
            max_allocation_change: 10.0,     // Maximum 10% change per proposal
            authorized_voters: Vec::new(),
            voter_weights: HashMap::new(),
        }
    }
}

/// Environmental governance system for managing treasury allocations
pub struct EnvironmentalGovernance {
    /// Active proposals
    proposals: HashMap<String, EnvironmentalProposal>,
    /// Historical proposals (archived)
    historical_proposals: HashMap<String, EnvironmentalProposal>,
    /// Governance configuration
    config: GovernanceConfig,
    /// Treasury reference
    treasury: EnvironmentalTreasury,
}

impl EnvironmentalGovernance {
    /// Create a new environmental governance system
    pub fn new(config: GovernanceConfig, treasury: EnvironmentalTreasury) -> Self {
        Self {
            proposals: HashMap::new(),
            historical_proposals: HashMap::new(),
            config,
            treasury,
        }
    }

    /// Create a new proposal
    pub fn create_proposal(
        &mut self,
        title: String,
        description: String,
        proposal_type: ProposalType,
        proposer: String,
        url: Option<String>,
        emergency: bool,
    ) -> Result<String, GovernanceError> {
        // Validate proposer is authorized
        if !self.config.authorized_voters.contains(&proposer) {
            return Err(GovernanceError::UnauthorizedVote(format!(
                "Proposer {} is not authorized",
                proposer
            )));
        }

        // Validate proposal type
        match &proposal_type {
            ProposalType::TreasuryAllocation { amount, .. } => {
                if *amount == 0 {
                    return Err(GovernanceError::InvalidProposal(
                        "Allocation amount cannot be zero".to_string(),
                    ));
                }

                // Check treasury balance
                if *amount > self.treasury.get_balance(Some(TreasuryAccountType::Main)) {
                    return Err(GovernanceError::InvalidProposal(format!(
                        "Allocation amount {} exceeds available balance {}",
                        amount,
                        self.treasury.get_balance(Some(TreasuryAccountType::Main))
                    )));
                }
            }
            ProposalType::ChangeFeeAllocation { new_percentage } => {
                if *new_percentage < 0.0 || *new_percentage > 100.0 {
                    return Err(GovernanceError::InvalidProposal(format!(
                        "Fee allocation percentage must be between 0 and 100, got {}",
                        new_percentage
                    )));
                }

                // Check maximum change
                let current_percentage = self.treasury.get_current_fee_percentage();
                let change = (*new_percentage - current_percentage).abs();
                if change > self.config.max_allocation_change {
                    return Err(GovernanceError::InvalidProposal(format!(
                        "Allocation change {} exceeds maximum allowed change {}",
                        change, self.config.max_allocation_change
                    )));
                }
            }
            ProposalType::PurchaseRECs { amount, .. }
            | ProposalType::PurchaseOffsets { amount, .. }
            | ProposalType::FundProject { amount, .. } => {
                if *amount == 0 {
                    return Err(GovernanceError::InvalidProposal(
                        "Amount cannot be zero".to_string(),
                    ));
                }

                // Check treasury balance
                if *amount > self.treasury.get_balance(Some(TreasuryAccountType::Main)) {
                    return Err(GovernanceError::InvalidProposal(format!(
                        "Amount {} exceeds available balance {}",
                        amount,
                        self.treasury.get_balance(Some(TreasuryAccountType::Main))
                    )));
                }
            }
            ProposalType::Other { amount, .. } => {
                if let Some(amount) = amount {
                    if *amount > 0
                        && *amount > self.treasury.get_balance(Some(TreasuryAccountType::Main))
                    {
                        return Err(GovernanceError::InvalidProposal(format!(
                            "Amount {} exceeds available balance {}",
                            amount,
                            self.treasury.get_balance(Some(TreasuryAccountType::Main))
                        )));
                    }
                }
            }
        }

        // Create proposal ID using UUID
        let id = format!(
            "prop_{}",
            uuid::Uuid::new_v4()
                .as_simple()
                .to_string()
                .chars()
                .take(8)
                .collect::<String>()
        );

        // Determine voting period
        let voting_period_days = if emergency && self.config.allow_emergency_proposals {
            2 // Emergency proposals have shorter voting period
        } else {
            self.config.voting_period_days
        };

        let now = Utc::now();
        let voting_ends_at = now + chrono::Duration::days(voting_period_days as i64);

        // Create proposal
        let proposal = EnvironmentalProposal {
            id: id.clone(),
            title,
            description,
            proposal_type,
            proposer,
            created_at: now,
            voting_ends_at,
            status: ProposalStatus::Active,
            votes_for: HashMap::new(),
            votes_against: HashMap::new(),
            executed_at: None,
            execution_tx_hash: None,
            url,
        };

        // Store proposal
        self.proposals.insert(id.clone(), proposal);

        Ok(id)
    }

    /// Cast a vote on a proposal
    pub fn vote(
        &mut self,
        proposal_id: &str,
        voter: String,
        vote_for: bool,
        comment: Option<String>,
        signature: Signature,
    ) -> Result<(), GovernanceError> {
        // Check if voter is authorized
        if !self.config.authorized_voters.contains(&voter) {
            return Err(GovernanceError::UnauthorizedVote(format!(
                "Voter {} is not authorized",
                voter
            )));
        }

        // Get the proposal
        let proposal = self
            .proposals
            .get_mut(proposal_id)
            .ok_or_else(|| GovernanceError::ProposalNotFound(proposal_id.to_string()))?;

        // Check if voting period has ended
        if Utc::now() > proposal.voting_ends_at {
            return Err(GovernanceError::VotingPeriodEnded(format!(
                "Voting period for proposal {} has ended",
                proposal_id
            )));
        }

        // Check if proposal is still active
        if proposal.status != ProposalStatus::Active {
            return Err(GovernanceError::VotingPeriodEnded(format!(
                "Proposal {} is no longer active (status: {:?})",
                proposal_id, proposal.status
            )));
        }

        // Create vote object
        let weight = self.config.voter_weights.get(&voter).copied().unwrap_or(1);
        let vote = Vote {
            voter: voter.clone(),
            timestamp: Utc::now(),
            weight,
            comment,
            signature,
        };

        // Add vote to the appropriate side
        if vote_for {
            proposal.votes_for.insert(voter.clone(), Box::new(vote));
            // Remove any previous opposing vote
            proposal.votes_against.remove(&voter);
        } else {
            proposal.votes_against.insert(voter.clone(), Box::new(vote));
            // Remove any previous supporting vote
            proposal.votes_for.remove(&voter);
        }

        Ok(())
    }

    /// Finish voting on a proposal and determine the outcome
    pub fn finalize_proposal(
        &mut self,
        proposal_id: &str,
    ) -> Result<ProposalStatus, GovernanceError> {
        // Get the proposal
        let proposal = self
            .proposals
            .get_mut(proposal_id)
            .ok_or_else(|| GovernanceError::ProposalNotFound(proposal_id.to_string()))?;

        // Check if voting period has ended or force finalization
        if Utc::now() <= proposal.voting_ends_at {
            return Err(GovernanceError::VotingPeriodEnded(format!(
                "Voting period for proposal {} has not yet ended",
                proposal_id
            )));
        }

        // Count votes
        let total_for: u64 = proposal.votes_for.values().map(|v| v.weight).sum();
        let total_against: u64 = proposal.votes_against.values().map(|v| v.weight).sum();
        let total_votes = total_for + total_against;

        // Calculate total possible votes
        let total_possible_votes: u64 = self
            .config
            .authorized_voters
            .iter()
            .map(|voter| self.config.voter_weights.get(voter).copied().unwrap_or(1))
            .sum();

        // Check quorum
        let quorum_reached = (total_votes as f64 / total_possible_votes as f64) * 100.0
            >= self.config.quorum_percentage;

        if !quorum_reached {
            proposal.status = ProposalStatus::Rejected;
            return Err(GovernanceError::QuorumNotReached(format!(
                "Quorum not reached for proposal {}: {}% of votes cast (required: {}%)",
                proposal_id,
                (total_votes as f64 / total_possible_votes as f64) * 100.0,
                self.config.quorum_percentage
            )));
        }

        // Calculate approval percentage
        let approval_percentage = if total_votes > 0 {
            (total_for as f64 / total_votes as f64) * 100.0
        } else {
            0.0
        };

        // Update proposal status based on voting outcome
        if approval_percentage >= self.config.approval_threshold {
            proposal.status = ProposalStatus::Approved;
        } else {
            proposal.status = ProposalStatus::Rejected;
        }

        // Return the new status
        Ok(proposal.status)
    }

    /// Execute an approved proposal
    pub fn execute_proposal(&mut self, proposal_id: &str) -> Result<(), GovernanceError> {
        // Get the proposal
        let proposal = self
            .proposals
            .get_mut(proposal_id)
            .ok_or_else(|| GovernanceError::ProposalNotFound(proposal_id.to_string()))?;

        // Check if proposal is approved
        if proposal.status != ProposalStatus::Approved {
            return Err(GovernanceError::InvalidProposal(format!(
                "Cannot execute proposal {} with status {:?}",
                proposal_id, proposal.status
            )));
        }

        // Check time lock
        let time_lock_expired = proposal.voting_ends_at
            + chrono::Duration::days(self.config.time_lock_days as i64)
            < Utc::now();
        if !time_lock_expired {
            return Err(GovernanceError::InvalidProposal(format!(
                "Time lock for proposal {} has not expired yet",
                proposal_id
            )));
        }

        // Execute the proposal based on its type
        match &proposal.proposal_type {
            ProposalType::TreasuryAllocation {
                from_account,
                to_account,
                amount,
            } => {
                self.treasury
                    .transfer_between_accounts(*from_account, *to_account, *amount)?;
            }
            ProposalType::ChangeFeeAllocation { new_percentage } => {
                self.treasury
                    .update_fee_allocation_percentage(*new_percentage)?;
            }
            ProposalType::PurchaseRECs { amount, provider } => {
                self.treasury
                    .purchase_renewable_certificates(provider, 100.0, *amount)?;
            }
            ProposalType::PurchaseOffsets { amount, provider } => {
                self.treasury
                    .purchase_carbon_offsets(provider, 100.0, *amount)?;
            }
            ProposalType::FundProject {
                amount,
                recipient,
                project_name,
                ..
            } => {
                self.treasury
                    .fund_project(project_name, *amount, recipient)?;
            }
            ProposalType::Other { .. } => {
                // Custom proposals require manual execution
                // Just mark as executed in this case
            }
        }

        // Update proposal status
        proposal.status = ProposalStatus::Executed;
        proposal.executed_at = Some(Utc::now());
        proposal.execution_tx_hash = Some(uuid::Uuid::new_v4().as_simple().to_string());

        // Move to historical proposals
        let proposal_clone = proposal.clone();
        self.historical_proposals
            .insert(proposal_id.to_string(), proposal_clone);
        self.proposals.remove(proposal_id);

        Ok(())
    }

    /// Cancel a proposal (only proposer or admin can cancel)
    pub fn cancel_proposal(
        &mut self,
        proposal_id: &str,
        canceller: &str,
    ) -> Result<(), GovernanceError> {
        // Get the proposal
        let proposal_opt = self.proposals.get(proposal_id);
        let proposal_ref = match proposal_opt {
            Some(p) => p,
            None => return Err(GovernanceError::ProposalNotFound(proposal_id.to_string())),
        };

        // Check if canceller is the proposer or an admin
        let is_proposer = proposal_ref.proposer == canceller;
        let is_admin = self.is_admin(canceller);

        if !is_proposer && !is_admin {
            return Err(GovernanceError::UnauthorizedVote(format!(
                "User {} is not authorized to cancel this proposal",
                canceller
            )));
        }

        // Now get a mutable reference to update the proposal
        let proposal = self
            .proposals
            .get_mut(proposal_id)
            .ok_or_else(|| GovernanceError::ProposalNotFound(proposal_id.to_string()))?;

        // Check if proposal can be cancelled
        if proposal.status == ProposalStatus::Executed {
            return Err(GovernanceError::InvalidProposal(format!(
                "Cannot cancel executed proposal {}",
                proposal_id
            )));
        }

        // Update proposal status
        proposal.status = ProposalStatus::Cancelled;

        // Move to historical proposals
        let proposal_clone = proposal.clone();
        self.historical_proposals
            .insert(proposal_id.to_string(), proposal_clone);
        self.proposals.remove(proposal_id);

        Ok(())
    }

    /// Get a proposal by ID
    pub fn get_proposal(&self, proposal_id: &str) -> Option<&EnvironmentalProposal> {
        self.proposals
            .get(proposal_id)
            .or_else(|| self.historical_proposals.get(proposal_id))
    }

    /// List all active proposals
    pub fn list_active_proposals(&self) -> Vec<&EnvironmentalProposal> {
        self.proposals
            .values()
            .filter(|p| p.status == ProposalStatus::Active)
            .collect()
    }

    /// List all proposals (active and historical)
    pub fn list_all_proposals(&self) -> Vec<&EnvironmentalProposal> {
        let mut proposals: Vec<&EnvironmentalProposal> = self.proposals.values().collect();
        let historical: Vec<&EnvironmentalProposal> = self.historical_proposals.values().collect();
        proposals.extend(historical);

        // Sort by creation date (newest first)
        proposals.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        proposals
    }

    /// Get proposals by status
    pub fn get_proposals_by_status(&self, status: ProposalStatus) -> Vec<&EnvironmentalProposal> {
        let mut proposals: Vec<&EnvironmentalProposal> = self
            .proposals
            .values()
            .filter(|p| p.status == status)
            .collect();

        let historical: Vec<&EnvironmentalProposal> = self
            .historical_proposals
            .values()
            .filter(|p| p.status == status)
            .collect();

        proposals.extend(historical);

        // Sort by creation date (newest first)
        proposals.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        proposals
    }

    /// Update governance configuration
    pub fn update_config(&mut self, config: GovernanceConfig) {
        self.config = config;
    }

    /// Add an authorized voter
    pub fn add_authorized_voter(&mut self, voter: String, weight: u64) {
        if !self.config.authorized_voters.contains(&voter) {
            self.config.authorized_voters.push(voter.clone());
        }
        self.config.voter_weights.insert(voter, weight);
    }

    /// Remove an authorized voter
    pub fn remove_authorized_voter(&mut self, voter: &str) {
        self.config.authorized_voters.retain(|v| v != voter);
        self.config.voter_weights.remove(voter);
    }

    /// Check if a user is an admin
    fn is_admin(&self, user: &str) -> bool {
        // In a real implementation, there would be a more sophisticated admin system
        // For simplicity, we assume the first few authorized voters are admins
        self.config
            .authorized_voters
            .iter()
            .take(3)
            .any(|v| v == user)
    }

    /// Process expired proposals
    pub fn process_expired_proposals(
        &mut self,
    ) -> Vec<(String, Result<ProposalStatus, GovernanceError>)> {
        let now = Utc::now();

        // Find proposals with ended voting periods
        let expired_ids: Vec<String> = self
            .proposals
            .values()
            .filter(|p| p.status == ProposalStatus::Active && p.voting_ends_at < now)
            .map(|p| p.id.clone())
            .collect();

        // Process each expired proposal
        let mut results = Vec::new();
        for id in expired_ids {
            let result = self.finalize_proposal(&id);
            results.push((id, result));
        }

        results
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_common::*;

    fn create_test_governance() -> EnvironmentalGovernance {
        let treasury = EnvironmentalTreasury::new(TreasuryConfig::default());

        let mut config = GovernanceConfig::default();
        config.authorized_voters = vec![
            "voter1".to_string(),
            "voter2".to_string(),
            "voter3".to_string(),
            "voter4".to_string(),
            "voter5".to_string(),
        ];

        EnvironmentalGovernance::new(config, treasury)
    }

    #[test]
    fn test_proposal_creation() {
        let mut governance = create_test_governance();

        // Create a valid proposal
        let result = governance.create_proposal(
            "Test Proposal".to_string(),
            "This is a test proposal".to_string(),
            ProposalType::ChangeFeeAllocation {
                new_percentage: 3.0,
            },
            "voter1".to_string(),
            None,
            false,
        );

        assert!(result.is_ok());
        let proposal_id = result.unwrap();

        // Verify the proposal was created
        let proposal = governance.get_proposal(&proposal_id);
        assert!(proposal.is_some());

        let proposal = proposal.unwrap();
        assert_eq!(proposal.title, "Test Proposal");
        assert_eq!(proposal.status, ProposalStatus::Active);
    }

    #[test]
    fn test_voting() {
        let mut governance = create_test_governance();

        // Create a proposal
        let proposal_id = governance
            .create_proposal(
                "Test Proposal".to_string(),
                "This is a test proposal".to_string(),
                ProposalType::ChangeFeeAllocation {
                    new_percentage: 3.0,
                },
                "voter1".to_string(),
                None,
                false,
            )
            .unwrap();

        // Cast votes
        assert!(governance
            .vote(
                &proposal_id,
                "voter1".to_string(),
                true,
                None,
                Signature {
                    signature_type: SignatureType::Secp256k1,
                    signature_bytes: vec![0; 64],
                    public_key_bytes: vec![0; 33],
                },
            )
            .is_ok());

        assert!(governance
            .vote(
                &proposal_id,
                "voter2".to_string(),
                true,
                None,
                Signature {
                    signature_type: SignatureType::Secp256k1,
                    signature_bytes: vec![0; 64],
                    public_key_bytes: vec![0; 33],
                },
            )
            .is_ok());

        assert!(governance
            .vote(
                &proposal_id,
                "voter3".to_string(),
                false,
                None,
                Signature {
                    signature_type: SignatureType::Secp256k1,
                    signature_bytes: vec![0; 64],
                    public_key_bytes: vec![0; 33],
                },
            )
            .is_ok());

        // Verify votes were recorded
        let proposal = governance.get_proposal(&proposal_id).unwrap();
        assert_eq!(proposal.votes_for.len(), 2);
        assert_eq!(proposal.votes_against.len(), 1);
    }
}
