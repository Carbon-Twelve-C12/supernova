// Manual Verification System for Renewable Energy Certificates
// Implements quarterly manual review process by Supernova Foundation staff
// Combines automated validation with human oversight for complex cases

use chrono::{DateTime, Datelike, Duration, TimeZone, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, RwLock};

use crate::environmental::{
    oracle::OracleError,
    types::{EnergySourceType, Region},
};

/// Manual verification request for Foundation review
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManualVerificationRequest {
    /// Unique request ID
    pub request_id: String,

    /// Miner/entity requesting verification
    pub requester_id: String,

    /// Type of verification needed
    pub verification_type: VerificationType,

    /// Documents submitted for review
    pub submitted_documents: Vec<SubmittedDocument>,

    /// Energy data to verify
    pub energy_data: EnergyVerificationData,

    /// Submission timestamp
    pub submitted_at: DateTime<Utc>,

    /// Current status
    pub status: ManualVerificationStatus,

    /// Priority level
    pub priority: PriorityLevel,

    /// Assigned reviewer (Foundation staff)
    pub assigned_reviewer: Option<String>,

    /// Review deadline
    pub review_deadline: DateTime<Utc>,
}

/// Type of environmental verification
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum VerificationType {
    /// Large-scale renewable installation (>10MW)
    LargeScaleRenewable,

    /// Complex multi-source energy mix
    ComplexEnergyMix,

    /// International renewable certificates
    InternationalREC,

    /// Custom power purchase agreements
    CustomPPA,

    /// Off-grid renewable systems
    OffGridSystem,

    /// Novel renewable technology
    NovelTechnology,

    /// Disputed automatic verification
    DisputedVerification,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubmittedDocument {
    pub document_id: String,
    pub document_type: DocumentType,
    pub file_hash: String,
    pub file_size: u64,
    pub uploaded_at: DateTime<Utc>,
    pub issuer: Option<String>,
    pub metadata: HashMap<String, String>,
}

/// Document types for verification
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum DocumentType {
    RenewableEnergyCertificate,
    PowerPurchaseAgreement,
    GridConnectionAgreement,
    MeteringData,
    AuditReport,
    GovernmentCertification,
    ThirdPartyAttestation,
    PhotoEvidence,
    Other(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnergyVerificationData {
    /// Total energy consumption (MWh)
    pub total_consumption_mwh: f64,

    /// Claimed renewable energy (MWh)
    pub claimed_renewable_mwh: f64,

    /// Energy sources breakdown
    pub energy_sources: HashMap<EnergySourceType, f64>,

    /// Time period covered
    pub coverage_period: (DateTime<Utc>, DateTime<Utc>),

    /// Location of operations
    pub location: LocationData,

    /// Additional claims
    pub additional_claims: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocationData {
    pub region: Region,
    pub country: String,
    pub state_province: Option<String>,
    pub city: Option<String>,
    pub coordinates: Option<(f64, f64)>, // (latitude, longitude)
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ManualVerificationStatus {
    /// Submitted, awaiting assignment
    Pending,

    /// Assigned to reviewer
    UnderReview,

    /// Additional information requested
    InfoRequested,

    /// Approved by Foundation
    Approved,

    /// Rejected with reasons
    Rejected(Vec<String>),

    /// Partially approved
    PartiallyApproved(f64), // Percentage approved

    /// Deferred to next quarter
    Deferred,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum PriorityLevel {
    Low = 0,
    Medium = 1,
    High = 2,
    Critical = 3,
}

/// Manual verification result from Foundation review
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManualVerificationResult {
    /// Request ID
    pub request_id: String,

    /// Reviewer details
    pub reviewer_id: String,
    pub reviewer_name: String,

    /// Review timestamp
    pub reviewed_at: DateTime<Utc>,

    /// Verification decision
    pub decision: VerificationDecision,

    /// Approved renewable amount (MWh)
    pub approved_renewable_mwh: f64,

    /// Detailed findings
    pub findings: Vec<ReviewFinding>,

    /// Recommendations
    pub recommendations: Vec<String>,

    /// Validity period
    pub valid_from: DateTime<Utc>,
    pub valid_until: DateTime<Utc>,

    /// Tamper-evident digest of this review's content.
    ///
    /// NOTE: this is a keyless SHA-256 digest of the review fields, NOT an
    /// asymmetric digital signature. It provides integrity (detects accidental
    /// mutation of a stored result) but NO authentication or non-repudiation:
    /// it does not cryptographically prove which reviewer authored the decision.
    /// Reviewer authorization is enforced separately via the authorized-reviewer
    /// registry in `process_manual_verification`.
    pub review_digest: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationDecision {
    pub status: ManualVerificationStatus,
    pub confidence_score: f64, // 0.0 to 1.0
    pub notes: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewFinding {
    pub finding_type: FindingType,
    pub description: String,
    pub severity: FindingSeverity,
    pub evidence_refs: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FindingType {
    DocumentAuthenticity,
    DataConsistency,
    RenewableSourceVerified,
    LocationVerified,
    TimePeriodVerified,
    ComplianceCheck,
    TechnicalAssessment,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FindingSeverity {
    Info,
    Minor,
    Major,
    Critical,
}

/// Quarterly review batch for Foundation processing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuarterlyReviewBatch {
    /// Quarter identifier (e.g., "2024-Q1")
    pub quarter_id: String,

    /// Batch creation date
    pub created_at: DateTime<Utc>,

    /// Review deadline
    pub deadline: DateTime<Utc>,

    /// Requests in this batch
    pub requests: Vec<String>, // Request IDs

    /// Batch status
    pub status: BatchStatus,

    /// Statistics
    pub stats: BatchStatistics,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BatchStatus {
    Preparing,
    ReadyForReview,
    InProgress,
    Completed,
    Archived,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BatchStatistics {
    pub total_requests: u64,
    pub reviewed: u64,
    pub approved: u64,
    pub rejected: u64,
    pub deferred: u64,
    pub total_mwh_reviewed: f64,
    pub total_mwh_approved: f64,
}

/// Manual Verification System
pub struct ManualVerificationSystem {
    /// Pending verification requests
    pending_requests: Arc<RwLock<HashMap<String, ManualVerificationRequest>>>,

    /// Completed verifications
    completed_verifications: Arc<RwLock<HashMap<String, ManualVerificationResult>>>,

    /// Quarterly batches
    quarterly_batches: Arc<RwLock<HashMap<String, QuarterlyReviewBatch>>>,

    /// Authorized reviewers (Foundation staff)
    authorized_reviewers: Arc<RwLock<HashMap<String, ReviewerProfile>>>,

    /// Review templates and guidelines
    review_guidelines: Arc<RwLock<ReviewGuidelines>>,

    /// System metrics
    metrics: Arc<RwLock<VerificationMetrics>>,
}

#[derive(Debug, Clone)]
struct ReviewerProfile {
    pub reviewer_id: String,
    pub name: String,
    pub email: String,
    pub expertise: Vec<String>,
    pub regions: Vec<Region>,
    pub max_quarterly_reviews: u32,
    pub current_assignments: u32,
}

#[derive(Debug, Clone)]
struct ReviewGuidelines {
    pub document_requirements: HashMap<VerificationType, Vec<DocumentType>>,
    pub review_checklists: HashMap<VerificationType, Vec<String>>,
    pub approval_thresholds: HashMap<VerificationType, f64>,
    pub red_flags: Vec<String>,
}

#[derive(Debug, Clone, Default)]
struct VerificationMetrics {
    pub total_requests: u64,
    pub completed_reviews: u64,
    pub average_review_time: Duration,
    pub approval_rate: f64,
    pub total_mwh_verified: f64,
}

impl Default for ManualVerificationSystem {
    fn default() -> Self {
        Self::new()
    }
}

impl ManualVerificationSystem {
    /// Create a new manual verification system
    pub fn new() -> Self {
        Self {
            pending_requests: Arc::new(RwLock::new(HashMap::new())),
            completed_verifications: Arc::new(RwLock::new(HashMap::new())),
            quarterly_batches: Arc::new(RwLock::new(HashMap::new())),
            authorized_reviewers: Arc::new(RwLock::new(HashMap::new())),
            review_guidelines: Arc::new(RwLock::new(Self::initialize_guidelines())),
            metrics: Arc::new(RwLock::new(VerificationMetrics::default())),
        }
    }

    /// Submit a request for manual verification
    pub fn submit_manual_verification_request(
        &self,
        requester_id: String,
        verification_type: VerificationType,
        documents: Vec<SubmittedDocument>,
        energy_data: EnergyVerificationData,
    ) -> Result<String, OracleError> {
        let request_id = self.generate_request_id(&requester_id);

        // Determine priority based on type and scale
        let priority = self.determine_priority(&verification_type, &energy_data);

        // Calculate review deadline (end of current quarter)
        let review_deadline = self.get_quarter_end_date(Utc::now());

        let request = ManualVerificationRequest {
            request_id: request_id.clone(),
            requester_id,
            verification_type,
            submitted_documents: documents,
            energy_data,
            submitted_at: Utc::now(),
            status: ManualVerificationStatus::Pending,
            priority,
            assigned_reviewer: None,
            review_deadline,
        };

        // Validate request has required documents
        self.validate_required_documents(&request)?;

        // Add to pending requests
        self.pending_requests
            .write()
            .map_err(|_| OracleError::LockPoisoned)?
            .insert(request_id.clone(), request);

        // Add to current quarter batch
        self.add_to_quarterly_batch(&request_id)?;


        Ok(request_id)
    }

    /// Process a manual verification (Foundation staff only)
    pub fn process_manual_verification(
        &self,
        request_id: &str,
        reviewer_id: &str,
        decision: VerificationDecision,
        approved_mwh: f64,
        findings: Vec<ReviewFinding>,
        recommendations: Vec<String>,
    ) -> Result<ManualVerificationResult, OracleError> {
        // Verify reviewer is authorized
        let reviewers = self
            .authorized_reviewers
            .read()
            .map_err(|_| OracleError::LockPoisoned)?;
        let reviewer = reviewers
            .get(reviewer_id)
            .ok_or_else(|| OracleError::OracleNotRegistered(reviewer_id.to_string()))?;

        // Reject re-processing of an already-completed request. A finalized
        // decision must not be silently overwritten — doing so would let an
        // approved-MWh figure be rewritten after the fact and would double-count
        // completed_reviews / total_mwh_verified in the metrics.
        if self
            .completed_verifications
            .read()
            .map_err(|_| OracleError::LockPoisoned)?
            .contains_key(request_id)
        {
            return Err(OracleError::VerificationFailed(format!(
                "Request {request_id} has already been verified and cannot be re-processed"
            )));
        }

        // Get the request
        let mut requests = self
            .pending_requests
            .write()
            .map_err(|_| OracleError::LockPoisoned)?;
        let request = requests
            .get_mut(request_id)
            .ok_or_else(|| OracleError::VerificationFailed("Request not found".to_string()))?;

        // Enforce reviewer assignment: once a request has an assigned reviewer,
        // only that reviewer may finalize it. Unassigned requests may still be
        // claimed by any authorized reviewer.
        if let Some(assigned) = &request.assigned_reviewer {
            if assigned != reviewer_id {
                return Err(OracleError::VerificationFailed(format!(
                    "Request {request_id} is assigned to reviewer {assigned}, not {reviewer_id}"
                )));
            }
        }

        // Bound the approved renewable amount by what was actually claimed and
        // reject non-finite / negative figures, so verified MWh cannot be
        // inflated beyond the request's claim (which would corrupt
        // total_mwh_verified).
        let claimed_renewable_mwh = request.energy_data.claimed_renewable_mwh;
        if !approved_mwh.is_finite() || approved_mwh < 0.0 {
            return Err(OracleError::VerificationFailed(format!(
                "approved_mwh must be a non-negative finite value, got {approved_mwh}"
            )));
        }
        if approved_mwh > claimed_renewable_mwh {
            return Err(OracleError::VerificationFailed(format!(
                "approved_mwh {approved_mwh} exceeds claimed_renewable_mwh {claimed_renewable_mwh}"
            )));
        }

        // Update request status
        request.status = decision.status.clone();
        request.assigned_reviewer = Some(reviewer_id.to_string());

        // Capture the review timestamp once so the exact value that feeds the
        // tamper-evident digest is also the value stored in `reviewed_at`. This
        // lets a verifier recompute the digest from the stored result's fields;
        // if the digest hashed a fresh `Utc::now()` it could never be
        // reproduced and the integrity check would be unrealizable.
        let reviewed_at = Utc::now();

        // Compute the tamper-evident content digest before `decision` is moved
        // into the result struct.
        let review_digest = Self::review_digest_at(
            reviewer_id,
            request_id,
            &decision,
            approved_mwh,
            reviewed_at.timestamp(),
        );

        // Create verification result
        let result = ManualVerificationResult {
            request_id: request_id.to_string(),
            reviewer_id: reviewer_id.to_string(),
            reviewer_name: reviewer.name.clone(),
            reviewed_at,
            decision,
            approved_renewable_mwh: approved_mwh,
            findings,
            recommendations,
            valid_from: request.energy_data.coverage_period.0,
            valid_until: request.energy_data.coverage_period.1,
            review_digest,
        };

        // Store completed verification
        self.completed_verifications
            .write()
            .map_err(|_| OracleError::LockPoisoned)?
            .insert(request_id.to_string(), result.clone());

        // Update metrics
        self.update_metrics(&result);


        Ok(result)
    }

    /// Get quarterly review batch. Read-only; recovers from lock poisoning
    /// so the query path never panics.
    pub fn get_quarterly_batch(&self, quarter_id: &str) -> Option<QuarterlyReviewBatch> {
        self.quarterly_batches
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .get(quarter_id)
            .cloned()
    }

    /// Create quarterly review batch
    pub fn create_quarterly_batch(&self) -> Result<String, OracleError> {
        let quarter_id = self.get_current_quarter_id();
        let deadline = self.get_quarter_end_date(Utc::now());

        // Collect all pending requests
        let pending = self
            .pending_requests
            .read()
            .map_err(|_| OracleError::LockPoisoned)?;
        let request_ids: Vec<String> = pending.keys().cloned().collect();

        let batch = QuarterlyReviewBatch {
            quarter_id: quarter_id.clone(),
            created_at: Utc::now(),
            deadline,
            requests: request_ids.clone(),
            status: BatchStatus::Preparing,
            stats: BatchStatistics {
                total_requests: request_ids.len() as u64,
                ..Default::default()
            },
        };

        self.quarterly_batches
            .write()
            .map_err(|_| OracleError::LockPoisoned)?
            .insert(quarter_id.clone(), batch);


        Ok(quarter_id)
    }

    /// Assign requests to reviewers
    pub fn assign_requests_to_reviewers(&self, quarter_id: &str) -> Result<u32, OracleError> {
        let mut batches = self
            .quarterly_batches
            .write()
            .map_err(|_| OracleError::LockPoisoned)?;
        let batch = batches
            .get_mut(quarter_id)
            .ok_or_else(|| OracleError::VerificationFailed("Batch not found".to_string()))?;

        let mut requests = self
            .pending_requests
            .write()
            .map_err(|_| OracleError::LockPoisoned)?;
        let reviewers = self
            .authorized_reviewers
            .read()
            .map_err(|_| OracleError::LockPoisoned)?;

        let mut assigned_count = 0;

        // Simple round-robin assignment
        let reviewer_ids: Vec<String> = reviewers.keys().cloned().collect();

        // Guard against remainder-by-zero: with no authorized reviewers there
        // is nobody to assign to, so fail closed instead of panicking below.
        if reviewer_ids.is_empty() {
            return Err(OracleError::VerificationFailed(
                "no authorized reviewers registered".to_string(),
            ));
        }

        let mut reviewer_index = 0;

        for request_id in &batch.requests {
            if let Some(request) = requests.get_mut(request_id) {
                if request.assigned_reviewer.is_none() {
                    let reviewer_id = &reviewer_ids[reviewer_index % reviewer_ids.len()];
                    request.assigned_reviewer = Some(reviewer_id.clone());
                    assigned_count += 1;
                    reviewer_index += 1;
                }
            }
        }

        batch.status = BatchStatus::ReadyForReview;


        Ok(assigned_count)
    }

    /// Generate quarterly report. Read-only; recovers from lock poisoning
    /// on every lock acquired so the reporting path never panics.
    pub fn generate_quarterly_report(&self, quarter_id: &str) -> QuarterlyReport {
        let _batch = self
            .quarterly_batches
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .get(quarter_id)
            .cloned()
            .unwrap_or_else(|| QuarterlyReviewBatch {
                quarter_id: quarter_id.to_string(),
                created_at: Utc::now(),
                deadline: Utc::now(),
                requests: vec![],
                status: BatchStatus::Completed,
                stats: BatchStatistics::default(),
            });

        let completed = self
            .completed_verifications
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let quarter_results: Vec<ManualVerificationResult> = completed
            .values()
            .filter(|r| self.get_quarter_id(r.reviewed_at) == quarter_id)
            .cloned()
            .collect();

        QuarterlyReport {
            quarter_id: quarter_id.to_string(),
            total_requests_reviewed: quarter_results.len() as u64,
            total_mwh_claimed: quarter_results
                .iter()
                .map(|r| r.request_id.clone())
                .filter_map(|id| {
                    self.pending_requests
                        .read()
                        .unwrap_or_else(|poisoned| poisoned.into_inner())
                        .get(&id)
                        .cloned()
                })
                .map(|req| req.energy_data.claimed_renewable_mwh)
                .sum(),
            total_mwh_approved: quarter_results
                .iter()
                .map(|r| r.approved_renewable_mwh)
                .sum(),
            approval_rate: if quarter_results.is_empty() {
                0.0
            } else {
                quarter_results
                    .iter()
                    .filter(|r| matches!(r.decision.status, ManualVerificationStatus::Approved))
                    .count() as f64
                    / quarter_results.len() as f64
            },
            key_findings: self.summarize_findings(&quarter_results),
            recommendations: self.compile_recommendations(&quarter_results),
            generated_at: Utc::now(),
        }
    }

    // Helper methods

    fn initialize_guidelines() -> ReviewGuidelines {
        let mut doc_requirements = HashMap::new();

        // Large scale renewable requirements
        doc_requirements.insert(
            VerificationType::LargeScaleRenewable,
            vec![
                DocumentType::RenewableEnergyCertificate,
                DocumentType::GridConnectionAgreement,
                DocumentType::MeteringData,
                DocumentType::AuditReport,
            ],
        );

        // International REC requirements
        doc_requirements.insert(
            VerificationType::InternationalREC,
            vec![
                DocumentType::RenewableEnergyCertificate,
                DocumentType::GovernmentCertification,
                DocumentType::ThirdPartyAttestation,
            ],
        );

        ReviewGuidelines {
            document_requirements: doc_requirements,
            review_checklists: HashMap::new(),
            approval_thresholds: HashMap::new(),
            red_flags: vec![
                "Inconsistent metering data".to_string(),
                "Expired certificates".to_string(),
                "Unverifiable issuer".to_string(),
                "Data anomalies".to_string(),
            ],
        }
    }

    fn generate_request_id(&self, requester_id: &str) -> String {
        let timestamp = Utc::now().timestamp();
        let mut hasher = Sha256::new();
        hasher.update(requester_id.as_bytes());
        hasher.update(timestamp.to_string().as_bytes());
        format!("MV-{}", hex::encode(&hasher.finalize()[..8]))
    }

    fn determine_priority(
        &self,
        verification_type: &VerificationType,
        energy_data: &EnergyVerificationData,
    ) -> PriorityLevel {
        match verification_type {
            VerificationType::DisputedVerification => PriorityLevel::Critical,
            VerificationType::LargeScaleRenewable
                if energy_data.claimed_renewable_mwh > 50000.0 =>
            {
                PriorityLevel::High
            }
            VerificationType::NovelTechnology => PriorityLevel::High,
            _ => PriorityLevel::Medium,
        }
    }

    fn get_current_quarter_id(&self) -> String {
        let now = Utc::now();
        let quarter = (now.month() - 1) / 3 + 1;
        format!("{}-Q{}", now.year(), quarter)
    }

    fn get_quarter_id(&self, date: DateTime<Utc>) -> String {
        let quarter = (date.month() - 1) / 3 + 1;
        format!("{}-Q{}", date.year(), quarter)
    }

    fn get_quarter_end_date(&self, date: DateTime<Utc>) -> DateTime<Utc> {
        let quarter = (date.month() - 1) / 3 + 1;
        let end_month = quarter * 3;
        let year = date.year();

        // `end_month = quarter * 3` with `quarter ∈ {1,2,3,4}` (derived from
        // a valid chrono month) can only be 3, 6, 9, or 12. The `_` branch
        // is mathematically unreachable; keep it as a safe fallback (28)
        // rather than `unreachable!()` so a future refactor can't turn a
        // type-system gap into a panic.
        let end_day = match end_month {
            3 | 12 => 31,
            6 | 9 => 30,
            _ => 28,
        };

        // `with_ymd_and_hms(...).single()` returns `None` only if the
        // combination is ambiguous or invalid (e.g. DST gaps — not
        // applicable to UTC). If it somehow fails, fall back to the input
        // date: callers treat the result as a deadline, and an already-
        // elapsed deadline is harmless (the request lands in the next
        // batch) whereas a panic would kill the verification worker.
        Utc.with_ymd_and_hms(year, end_month, end_day, 23, 59, 59)
            .single()
            .unwrap_or(date)
    }

    fn validate_required_documents(
        &self,
        request: &ManualVerificationRequest,
    ) -> Result<(), OracleError> {
        let guidelines = self
            .review_guidelines
            .read()
            .map_err(|_| OracleError::LockPoisoned)?;

        if let Some(required_docs) = guidelines
            .document_requirements
            .get(&request.verification_type)
        {
            let submitted_types: HashSet<_> = request
                .submitted_documents
                .iter()
                .map(|d| &d.document_type)
                .collect();

            for required in required_docs {
                if !submitted_types.contains(required) {
                    return Err(OracleError::VerificationFailed(format!(
                        "Missing required document type: {:?}",
                        required
                    )));
                }
            }
        }

        Ok(())
    }

    fn add_to_quarterly_batch(&self, request_id: &str) -> Result<(), OracleError> {
        let quarter_id = self.get_current_quarter_id();
        let mut batches = self
            .quarterly_batches
            .write()
            .map_err(|_| OracleError::LockPoisoned)?;

        let batch = batches
            .entry(quarter_id.clone())
            .or_insert_with(|| QuarterlyReviewBatch {
                quarter_id: quarter_id.clone(),
                created_at: Utc::now(),
                deadline: self.get_quarter_end_date(Utc::now()),
                requests: Vec::new(),
                status: BatchStatus::Preparing,
                stats: BatchStatistics::default(),
            });

        batch.requests.push(request_id.to_string());
        batch.stats.total_requests += 1;

        Ok(())
    }

    /// Compute a keyless, tamper-evident digest binding a review's identity and
    /// decision content.
    ///
    /// This is deliberately NOT a digital signature: it uses no secret key and
    /// therefore provides no authentication or non-repudiation. Its sole purpose
    /// is to detect accidental corruption/mutation of a stored review result.
    /// Reviewer authenticity is enforced by the authorized-reviewer registry, not
    /// by this value.
    /// Recompute a stored result's tamper-evident digest from its own fields and
    /// compare it against the stored `review_digest`. Returns `true` iff the
    /// content is intact.
    ///
    /// This is realizable precisely because `reviewed_at` retains the timestamp
    /// that fed the original digest: the same inputs (reviewer id, request id,
    /// decision status/confidence, approved MWh, and `reviewed_at.timestamp()`)
    /// reproduce the same hash. As with the digest itself, this detects
    /// accidental mutation only — it provides no authentication.
    pub fn verify_review_digest(result: &ManualVerificationResult) -> bool {
        let recomputed = Self::review_digest_at(
            &result.reviewer_id,
            &result.request_id,
            &result.decision,
            result.approved_renewable_mwh,
            result.reviewed_at.timestamp(),
        );
        recomputed == result.review_digest
    }

    /// Pure, timestamp-explicit digest computation (factored out so the content
    /// binding can be tested deterministically).
    fn review_digest_at(
        reviewer_id: &str,
        request_id: &str,
        decision: &VerificationDecision,
        approved_mwh: f64,
        timestamp: i64,
    ) -> String {
        let mut hasher = Sha256::new();
        hasher.update(reviewer_id.as_bytes());
        hasher.update(request_id.as_bytes());
        hasher.update(format!("{:?}", decision.status).as_bytes());
        hasher.update(decision.confidence_score.to_le_bytes());
        hasher.update(approved_mwh.to_le_bytes());
        hasher.update(timestamp.to_string().as_bytes());
        hex::encode(hasher.finalize())
    }

    fn update_metrics(&self, result: &ManualVerificationResult) {
        // Metrics are best-effort; recover from poison so a prior panic in
        // this path doesn't cascade.
        let mut metrics = self
            .metrics
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        metrics.completed_reviews += 1;
        metrics.total_mwh_verified += result.approved_renewable_mwh;

        if matches!(result.decision.status, ManualVerificationStatus::Approved) {
            let current_approvals = metrics.approval_rate * metrics.completed_reviews as f64;
            metrics.approval_rate = (current_approvals + 1.0) / metrics.completed_reviews as f64;
        }
    }

    fn summarize_findings(&self, results: &[ManualVerificationResult]) -> Vec<String> {
        let mut summary = Vec::new();

        let total_findings: usize = results.iter().map(|r| r.findings.len()).sum();

        let critical_findings: usize = results
            .iter()
            .flat_map(|r| &r.findings)
            .filter(|f| matches!(f.severity, FindingSeverity::Critical))
            .count();

        summary.push(format!("Total findings: {}", total_findings));
        summary.push(format!("Critical findings: {}", critical_findings));

        summary
    }

    fn compile_recommendations(&self, results: &[ManualVerificationResult]) -> Vec<String> {
        let mut all_recommendations = Vec::new();

        for result in results {
            all_recommendations.extend(result.recommendations.clone());
        }

        // Deduplicate and sort
        all_recommendations.sort();
        all_recommendations.dedup();

        all_recommendations
    }
}

/// Quarterly verification report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuarterlyReport {
    pub quarter_id: String,
    pub total_requests_reviewed: u64,
    pub total_mwh_claimed: f64,
    pub total_mwh_approved: f64,
    pub approval_rate: f64,
    pub key_findings: Vec<String>,
    pub recommendations: Vec<String>,
    pub generated_at: DateTime<Utc>,
}

/// Public API functions

pub fn submit_manual_verification_request(
    system: &ManualVerificationSystem,
    requester_id: String,
    verification_type: VerificationType,
    documents: Vec<SubmittedDocument>,
    energy_data: EnergyVerificationData,
) -> Result<String, OracleError> {
    system.submit_manual_verification_request(
        requester_id,
        verification_type,
        documents,
        energy_data,
    )
}

pub fn process_manual_verification(
    system: &ManualVerificationSystem,
    request_id: &str,
    reviewer_id: &str,
    decision: VerificationDecision,
    approved_mwh: f64,
    findings: Vec<ReviewFinding>,
    recommendations: Vec<String>,
) -> Result<ManualVerificationResult, OracleError> {
    system.process_manual_verification(
        request_id,
        reviewer_id,
        decision,
        approved_mwh,
        findings,
        recommendations,
    )
}

pub fn create_quarterly_batch(system: &ManualVerificationSystem) -> Result<String, OracleError> {
    system.create_quarterly_batch()
}

pub fn generate_quarterly_report(
    system: &ManualVerificationSystem,
    quarter_id: &str,
) -> QuarterlyReport {
    system.generate_quarterly_report(quarter_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn decision(status: ManualVerificationStatus, confidence: f64) -> VerificationDecision {
        VerificationDecision {
            status,
            confidence_score: confidence,
            notes: "test".to_string(),
        }
    }

    #[test]
    fn review_digest_is_deterministic_and_hex64() {
        let d = decision(ManualVerificationStatus::Approved, 0.9);
        let a = ManualVerificationSystem::review_digest_at("rev-1", "MV-abc", &d, 42.0, 1_000);
        let b = ManualVerificationSystem::review_digest_at("rev-1", "MV-abc", &d, 42.0, 1_000);
        assert_eq!(a, b, "same inputs must produce the same digest");
        assert_eq!(a.len(), 64, "SHA-256 hex digest must be 64 chars");
        assert!(a.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn review_digest_binds_decision_content() {
        let base = decision(ManualVerificationStatus::Approved, 0.9);
        let d0 = ManualVerificationSystem::review_digest_at("rev-1", "MV-abc", &base, 42.0, 1_000);

        // Changing the decision status changes the digest.
        let rejected = decision(ManualVerificationStatus::Rejected(vec!["bad".to_string()]), 0.9);
        let d_status =
            ManualVerificationSystem::review_digest_at("rev-1", "MV-abc", &rejected, 42.0, 1_000);
        assert_ne!(d0, d_status, "decision status must be bound into the digest");

        // Changing the approved MWh changes the digest.
        let d_mwh =
            ManualVerificationSystem::review_digest_at("rev-1", "MV-abc", &base, 99.0, 1_000);
        assert_ne!(d0, d_mwh, "approved_mwh must be bound into the digest");

        // Changing reviewer or request identifiers changes the digest.
        let d_rev =
            ManualVerificationSystem::review_digest_at("rev-2", "MV-abc", &base, 42.0, 1_000);
        assert_ne!(d0, d_rev, "reviewer_id must be bound into the digest");
        let d_req =
            ManualVerificationSystem::review_digest_at("rev-1", "MV-xyz", &base, 42.0, 1_000);
        assert_ne!(d0, d_req, "request_id must be bound into the digest");
    }

    fn reviewer(id: &str) -> ReviewerProfile {
        ReviewerProfile {
            reviewer_id: id.to_string(),
            name: format!("Reviewer {id}"),
            email: format!("{id}@example.org"),
            expertise: vec![],
            regions: vec![Region::NorthAmerica],
            max_quarterly_reviews: 100,
            current_assignments: 0,
        }
    }

    /// Build a system with a registered reviewer and a single pending request
    /// whose claimed renewable MWh and assigned reviewer are configurable.
    fn system_with_request(
        reviewer_id: &str,
        assigned_reviewer: Option<&str>,
        claimed_renewable_mwh: f64,
    ) -> (ManualVerificationSystem, String) {
        let system = ManualVerificationSystem::new();
        system
            .authorized_reviewers
            .write()
            .unwrap()
            .insert(reviewer_id.to_string(), reviewer(reviewer_id));

        let now = Utc::now();
        let request_id = "MV-test-1".to_string();
        let request = ManualVerificationRequest {
            request_id: request_id.clone(),
            requester_id: "miner-1".to_string(),
            verification_type: VerificationType::LargeScaleRenewable,
            submitted_documents: vec![],
            energy_data: EnergyVerificationData {
                total_consumption_mwh: claimed_renewable_mwh,
                claimed_renewable_mwh,
                energy_sources: HashMap::new(),
                coverage_period: (now, now),
                location: LocationData {
                    region: Region::NorthAmerica,
                    country: "US".to_string(),
                    state_province: None,
                    city: None,
                    coordinates: None,
                },
                additional_claims: vec![],
            },
            submitted_at: now,
            status: ManualVerificationStatus::Pending,
            priority: PriorityLevel::Low,
            assigned_reviewer: assigned_reviewer.map(|s| s.to_string()),
            review_deadline: now,
        };
        system
            .pending_requests
            .write()
            .unwrap()
            .insert(request_id.clone(), request);
        (system, request_id)
    }

    #[test]
    fn approved_mwh_exceeding_claim_is_rejected() {
        let (system, req) = system_with_request("rev-1", None, 100.0);
        let res = system.process_manual_verification(
            &req,
            "rev-1",
            decision(ManualVerificationStatus::Approved, 0.9),
            150.0, // exceeds claimed 100.0
            vec![],
            vec![],
        );
        assert!(res.is_err(), "approving more MWh than claimed must be rejected");
        // No result should have been stored, and metrics must not be inflated.
        assert!(system.completed_verifications.read().unwrap().is_empty());
        assert_eq!(system.metrics.read().unwrap().total_mwh_verified, 0.0);
    }

    #[test]
    fn negative_or_nonfinite_approved_mwh_is_rejected() {
        let (system, req) = system_with_request("rev-1", None, 100.0);
        for bad in [-1.0_f64, f64::NAN, f64::INFINITY] {
            let res = system.process_manual_verification(
                &req,
                "rev-1",
                decision(ManualVerificationStatus::Approved, 0.9),
                bad,
                vec![],
                vec![],
            );
            assert!(res.is_err(), "approved_mwh {bad} must be rejected");
        }
    }

    #[test]
    fn wrong_assigned_reviewer_is_rejected() {
        // Request is assigned to rev-1; rev-2 must not be able to finalize it.
        let (system, req) = system_with_request("rev-2", Some("rev-1"), 100.0);
        system
            .authorized_reviewers
            .write()
            .unwrap()
            .insert("rev-1".to_string(), reviewer("rev-1"));
        let res = system.process_manual_verification(
            &req,
            "rev-2",
            decision(ManualVerificationStatus::Approved, 0.9),
            50.0,
            vec![],
            vec![],
        );
        assert!(res.is_err(), "a non-assigned reviewer must be rejected");
        assert!(system.completed_verifications.read().unwrap().is_empty());
    }

    #[test]
    fn already_completed_request_cannot_be_reprocessed() {
        let (system, req) = system_with_request("rev-1", None, 100.0);
        let first = system.process_manual_verification(
            &req,
            "rev-1",
            decision(ManualVerificationStatus::Approved, 0.9),
            40.0,
            vec![],
            vec![],
        );
        assert!(first.is_ok(), "first valid review should succeed");

        // A second attempt (e.g. rewriting approved MWh) must be rejected.
        let second = system.process_manual_verification(
            &req,
            "rev-1",
            decision(ManualVerificationStatus::Approved, 0.9),
            90.0,
            vec![],
            vec![],
        );
        assert!(second.is_err(), "re-processing a completed request must be rejected");

        // Stored result and metrics reflect only the first review.
        let stored = system.completed_verifications.read().unwrap();
        assert_eq!(stored.get(&req).unwrap().approved_renewable_mwh, 40.0);
        assert_eq!(system.metrics.read().unwrap().completed_reviews, 1);
        assert_eq!(system.metrics.read().unwrap().total_mwh_verified, 40.0);
    }

    #[test]
    fn valid_review_within_claim_succeeds() {
        let (system, req) = system_with_request("rev-1", Some("rev-1"), 100.0);
        let res = system.process_manual_verification(
            &req,
            "rev-1",
            decision(ManualVerificationStatus::Approved, 0.9),
            80.0,
            vec![],
            vec![],
        );
        let result = res.expect("assigned reviewer approving within claim should succeed");
        assert_eq!(result.approved_renewable_mwh, 80.0);
    }

    #[test]
    fn assign_with_no_reviewers_errs_instead_of_panicking() {
        // A pending, unassigned request with an empty authorized-reviewers set
        // must fail closed rather than panic on remainder-by-zero.
        let (system, _req) = system_with_request("rev-1", None, 100.0);
        system.authorized_reviewers.write().unwrap().clear();

        let quarter_id = system
            .create_quarterly_batch()
            .expect("creating a quarterly batch should succeed");

        let res = system.assign_requests_to_reviewers(&quarter_id);
        assert!(
            matches!(res, Err(OracleError::VerificationFailed(_))),
            "assigning with no reviewers must return VerificationFailed, got {res:?}"
        );
    }

    #[test]
    fn stored_result_digest_recomputes_and_detects_mutation() {
        // A result produced by the real path must carry a digest that a verifier
        // can recompute from the stored fields alone (proving the timestamp that
        // fed the digest is retained in `reviewed_at`), and any mutation of a
        // bound field must break that recomputation.
        let (system, req) = system_with_request("rev-1", Some("rev-1"), 100.0);
        let stored = system
            .process_manual_verification(
                &req,
                "rev-1",
                decision(ManualVerificationStatus::Approved, 0.9),
                80.0,
                vec![],
                vec![],
            )
            .expect("valid review should succeed");

        assert!(
            ManualVerificationSystem::verify_review_digest(&stored),
            "digest must recompute from the stored result's own fields"
        );

        // Mutating the approved MWh must break the digest.
        let mut tampered_mwh = stored.clone();
        tampered_mwh.approved_renewable_mwh = 81.0;
        assert!(
            !ManualVerificationSystem::verify_review_digest(&tampered_mwh),
            "mutating approved_mwh must be detected by the digest"
        );

        // Mutating the stored timestamp must break the digest (confirms the
        // digest is genuinely bound to the retained `reviewed_at`).
        let mut tampered_time = stored.clone();
        tampered_time.reviewed_at = stored.reviewed_at + chrono::Duration::seconds(1);
        assert!(
            !ManualVerificationSystem::verify_review_digest(&tampered_time),
            "mutating reviewed_at must be detected by the digest"
        );
    }
}
