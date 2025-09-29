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

    /// Digital signature
    pub reviewer_signature: String,
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
            .unwrap()
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
        let reviewers = self.authorized_reviewers.read().unwrap();
        let reviewer = reviewers
            .get(reviewer_id)
            .ok_or_else(|| OracleError::OracleNotRegistered(reviewer_id.to_string()))?;

        // Get the request
        let mut requests = self.pending_requests.write().unwrap();
        let request = requests
            .get_mut(request_id)
            .ok_or_else(|| OracleError::VerificationFailed("Request not found".to_string()))?;

        // Update request status
        request.status = decision.status.clone();
        request.assigned_reviewer = Some(reviewer_id.to_string());

        // Create verification result
        let result = ManualVerificationResult {
            request_id: request_id.to_string(),
            reviewer_id: reviewer_id.to_string(),
            reviewer_name: reviewer.name.clone(),
            reviewed_at: Utc::now(),
            decision,
            approved_renewable_mwh: approved_mwh,
            findings,
            recommendations,
            valid_from: request.energy_data.coverage_period.0,
            valid_until: request.energy_data.coverage_period.1,
            reviewer_signature: self.generate_signature(reviewer_id, request_id),
        };

        // Store completed verification
        self.completed_verifications
            .write()
            .unwrap()
            .insert(request_id.to_string(), result.clone());

        // Update metrics
        self.update_metrics(&result);


        Ok(result)
    }

    /// Get quarterly review batch
    pub fn get_quarterly_batch(&self, quarter_id: &str) -> Option<QuarterlyReviewBatch> {
        self.quarterly_batches
            .read()
            .unwrap()
            .get(quarter_id)
            .cloned()
    }

    /// Create quarterly review batch
    pub fn create_quarterly_batch(&self) -> Result<String, OracleError> {
        let quarter_id = self.get_current_quarter_id();
        let deadline = self.get_quarter_end_date(Utc::now());

        // Collect all pending requests
        let pending = self.pending_requests.read().unwrap();
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
            .unwrap()
            .insert(quarter_id.clone(), batch);


        Ok(quarter_id)
    }

    /// Assign requests to reviewers
    pub fn assign_requests_to_reviewers(&self, quarter_id: &str) -> Result<u32, OracleError> {
        let mut batches = self.quarterly_batches.write().unwrap();
        let batch = batches
            .get_mut(quarter_id)
            .ok_or_else(|| OracleError::VerificationFailed("Batch not found".to_string()))?;

        let mut requests = self.pending_requests.write().unwrap();
        let reviewers = self.authorized_reviewers.read().unwrap();

        let mut assigned_count = 0;

        // Simple round-robin assignment
        let reviewer_ids: Vec<String> = reviewers.keys().cloned().collect();
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

    /// Generate quarterly report
    pub fn generate_quarterly_report(&self, quarter_id: &str) -> QuarterlyReport {
        let batch = self
            .quarterly_batches
            .read()
            .unwrap()
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

        let completed = self.completed_verifications.read().unwrap();
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
                .filter_map(|id| self.pending_requests.read().unwrap().get(&id).cloned())
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

        let end_day = match end_month {
            3 => 31,
            6 => 30,
            9 => 30,
            12 => 31,
            _ => unreachable!(),
        };

        Utc.with_ymd_and_hms(year, end_month, end_day, 23, 59, 59)
            .single()
            .expect("Invalid date")
    }

    fn validate_required_documents(
        &self,
        request: &ManualVerificationRequest,
    ) -> Result<(), OracleError> {
        let guidelines = self.review_guidelines.read().unwrap();

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
        let mut batches = self.quarterly_batches.write().unwrap();

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

    fn generate_signature(&self, reviewer_id: &str, request_id: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(reviewer_id.as_bytes());
        hasher.update(request_id.as_bytes());
        hasher.update(Utc::now().timestamp().to_string().as_bytes());
        hex::encode(hasher.finalize())
    }

    fn update_metrics(&self, result: &ManualVerificationResult) {
        let mut metrics = self.metrics.write().unwrap();
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
