// Carbon Tracking and Validation System for Supernova
// Implements real-time carbon footprint measurement with multi-oracle consensus
// Leveraging Nova Energy expertise for environmental leadership

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use crate::environmental::{
    emissions::EmissionsCalculator,
    oracle::{EnvironmentalData, EnvironmentalOracle, OracleError},
    types::{EnergySource as EnergySourceType, Region},
    verification::{
        CarbonOffset as BaseCarbonOffset, RenewableCertificate as BaseRenewableCertificate,
    },
};

/// Carbon tracking validation results
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CarbonTrackingResult {
    /// Total carbon footprint in tonnes CO2e
    pub total_emissions: f64,

    /// Carbon offsets applied in tonnes CO2e
    pub total_offsets: f64,

    /// Net carbon footprint (can be negative for carbon-negative operations)
    pub net_carbon_footprint: f64,

    /// Renewable energy percentage
    pub renewable_percentage: f64,

    /// Validation timestamp
    pub timestamp: DateTime<Utc>,

    /// Oracle consensus details
    pub oracle_consensus: OracleConsensusResult,

    /// Environmental metrics
    pub metrics: EnvironmentalMetrics,

    /// Verification proofs
    pub verification_proofs: Vec<VerificationProof>,
}

/// Oracle consensus result for carbon tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OracleConsensusResult {
    /// Number of oracles that participated
    pub participating_oracles: usize,

    /// Consensus percentage achieved
    pub consensus_percentage: f64,

    /// Individual oracle submissions
    pub oracle_submissions: Vec<OracleDataPoint>,

    /// Final consensus value
    pub consensus_value: f64,

    /// Consensus achieved
    pub consensus_achieved: bool,
}

/// Individual oracle data submission
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OracleDataPoint {
    pub oracle_id: String,
    pub submitted_value: f64,
    pub timestamp: DateTime<Utc>,
    pub confidence_score: f64,
    pub data_sources: Vec<String>,
}

/// Environmental metrics for carbon tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvironmentalMetrics {
    /// Energy efficiency (hashrate per MW).
    ///
    /// `None` when no hashrate input is available to derive a real value; the
    /// carbon-footprint path does not receive hashrate, so it must not fabricate
    /// a measured efficiency here.
    pub energy_efficiency: Option<f64>,

    /// Carbon intensity (kg CO2e per kWh)
    pub carbon_intensity: f64,

    /// Green mining percentage
    pub green_mining_percentage: f64,

    /// Carbon offset efficiency
    pub offset_efficiency: f64,

    /// Environmental score (0-100)
    pub environmental_score: f64,
}

/// Verification proof for environmental claims
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationProof {
    pub proof_type: ProofType,
    pub proof_data: Vec<u8>,
    pub issuer: String,
    pub timestamp: DateTime<Utc>,
    pub expiry: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ProofType {
    RenewableEnergyCertificate,
    CarbonOffsetCertificate,
    SmartMeterReading,
    GridMixData,
    EnvironmentalAudit,
}

// Wrapper structs with additional tracking fields
#[derive(Debug, Clone)]
struct TrackedRenewableCertificate {
    pub certificate: BaseRenewableCertificate,
    pub owner_id: String,
    pub certificate_hash: Vec<u8>,
    pub issue_date: DateTime<Utc>,
    pub expiry_date: DateTime<Utc>,
}

#[derive(Debug, Clone)]
struct TrackedCarbonOffset {
    pub offset: BaseCarbonOffset,
    pub owner_id: String,
    pub certificate_hash: Vec<u8>,
    pub issue_date: DateTime<Utc>,
    pub expiry_date: DateTime<Utc>,
}

impl TrackedCarbonOffset {
    pub fn is_valid(&self) -> bool {
        self.expiry_date > Utc::now()
    }

    pub fn amount_tonnes(&self) -> f64 {
        self.offset.amount_tonnes
    }
}

/// Real-time carbon tracking system
pub struct CarbonTracker {
    /// Environmental oracle system
    oracle: Arc<EnvironmentalOracle>,

    /// Emissions calculator
    calculator: Arc<EmissionsCalculator>,

    /// Current tracking data
    tracking_data: Arc<RwLock<HashMap<String, TrackingData>>>,

    /// Historical carbon footprints
    historical_data: Arc<RwLock<Vec<CarbonTrackingResult>>>,

    /// Renewable energy certificates
    renewable_certificates: Arc<RwLock<HashMap<String, TrackedRenewableCertificate>>>,

    /// Carbon offset certificates
    carbon_offsets: Arc<RwLock<HashMap<String, TrackedCarbonOffset>>>,

    /// Real-time monitoring data
    monitoring_data: Arc<RwLock<MonitoringData>>,
}

/// Tracking data for individual miners/operations
#[derive(Debug, Clone)]
struct TrackingData {
    pub entity_id: String,
    pub current_emissions: f64,
    pub current_offsets: f64,
    pub energy_sources: HashMap<EnergySourceType, f64>,
    pub last_updated: DateTime<Utc>,
    pub verification_status: VerificationStatus,
}

/// Real-time monitoring data
#[derive(Debug, Clone, Default)]
struct MonitoringData {
    pub total_network_emissions: f64,
    pub total_network_offsets: f64,
    pub average_carbon_intensity: f64,
    pub renewable_energy_percentage: f64,
    pub last_calculation: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq)]
enum VerificationStatus {
    Pending,
    Verified,
    Failed,
    Expired,
}

impl CarbonTracker {
    /// Create a new carbon tracking system
    pub fn new(oracle: Arc<EnvironmentalOracle>, calculator: Arc<EmissionsCalculator>) -> Self {
        Self {
            oracle,
            calculator,
            tracking_data: Arc::new(RwLock::new(HashMap::new())),
            historical_data: Arc::new(RwLock::new(Vec::new())),
            renewable_certificates: Arc::new(RwLock::new(HashMap::new())),
            carbon_offsets: Arc::new(RwLock::new(HashMap::new())),
            monitoring_data: Arc::new(RwLock::new(MonitoringData::default())),
        }
    }

    /// Validate carbon footprint calculation with multi-oracle consensus
    pub async fn validate_carbon_footprint_calculation(
        &self,
        entity_id: &str,
        energy_consumption_mwh: f64,
        energy_sources: HashMap<EnergySourceType, f64>,
        region: Region,
    ) -> Result<CarbonTrackingResult, OracleError> {

        // Step 1: Calculate emissions using local calculator
        let local_emissions =
            self.calculate_local_emissions(energy_consumption_mwh, &energy_sources, &region)?;

        // Step 2: Request oracle verification
        let oracle_submissions = self
            .request_oracle_verification(
                entity_id,
                energy_consumption_mwh,
                &energy_sources,
                &region,
            )
            .await?;

        // Step 3: Process oracle consensus
        let consensus_result =
            self.process_oracle_consensus(&oracle_submissions, local_emissions)?;

        // Step 4: Verify renewable certificates
        let renewable_percentage = self.verify_renewable_percentage(entity_id, &energy_sources)?;

        // Step 5: Apply carbon offsets
        let total_offsets = self.calculate_applied_offsets(entity_id)?;

        // Step 6: Calculate net carbon footprint
        let net_carbon_footprint = consensus_result.consensus_value - total_offsets;

        // Step 7: Generate environmental metrics
        let metrics = self.calculate_environmental_metrics(
            consensus_result.consensus_value,
            total_offsets,
            renewable_percentage,
            energy_consumption_mwh,
        );

        // Step 8: Collect verification proofs
        let verification_proofs = self.collect_verification_proofs(entity_id)?;

        let result = CarbonTrackingResult {
            total_emissions: consensus_result.consensus_value,
            total_offsets,
            net_carbon_footprint,
            renewable_percentage,
            timestamp: Utc::now(),
            oracle_consensus: consensus_result,
            metrics,
            verification_proofs,
        };

        // Update tracking data (thread the validated energy-source mix through
        // so the tracked record reflects the same data the calculation used).
        self.update_tracking_data(entity_id, &result, energy_sources)?;

        // Store historical data
        self.historical_data
            .write()
            .map_err(|_| OracleError::LockPoisoned)?
            .push(result.clone());

        // Update network-wide monitoring
        self.update_monitoring_data(&result)?;


        Ok(result)
    }

    /// Test multi-oracle consensus for environmental data
    pub async fn test_multi_oracle_consensus(
        &self,
        test_data: Vec<OracleDataPoint>,
    ) -> Result<OracleConsensusResult, OracleError> {

        // Simulate oracle consensus mechanism
        let total_oracles = test_data.len();
        if total_oracles < 3 {
            return Err(OracleError::ConsensusNotReached(
                "Insufficient oracles for consensus".to_string(),
            ));
        }

        // Calculate weighted average based on confidence scores
        let total_weight: f64 = test_data.iter().map(|d| d.confidence_score).sum();
        let weighted_sum: f64 = test_data
            .iter()
            .map(|d| d.submitted_value * d.confidence_score)
            .sum();

        let consensus_value = weighted_sum / total_weight;

        // Calculate consensus percentage (oracles within 5% of consensus value)
        let agreeing_oracles = test_data
            .iter()
            .filter(|d| (d.submitted_value - consensus_value).abs() / consensus_value < 0.05)
            .count();

        let consensus_percentage = (agreeing_oracles as f64 / total_oracles as f64) * 100.0;
        let consensus_achieved = consensus_percentage >= 67.0; // 2/3 majority

        Ok(OracleConsensusResult {
            participating_oracles: total_oracles,
            consensus_percentage,
            oracle_submissions: test_data,
            consensus_value,
            consensus_achieved,
        })
    }

    /// Verify environmental data integrity
    pub fn verify_environmental_data_integrity(
        &self,
        _data: &EnvironmentalData,
        proofs: &[VerificationProof],
    ) -> Result<bool, OracleError> {

        // Verify each proof
        for proof in proofs {
            match proof.proof_type {
                ProofType::RenewableEnergyCertificate => {
                    if !self.verify_rec_proof(proof)? {
                        return Ok(false);
                    }
                }
                ProofType::CarbonOffsetCertificate => {
                    if !self.verify_offset_proof(proof)? {
                        return Ok(false);
                    }
                }
                ProofType::SmartMeterReading => {
                    if !self.verify_meter_proof(proof)? {
                        return Ok(false);
                    }
                }
                _ => {
                    // Additional proof types can be verified here
                }
            }
        }

        Ok(true)
    }

    /// Activate real-time carbon tracking.
    ///
    /// Honest behavior: real-time tracking requires live, external data streams
    /// (e.g. smart-meter feeds, grid-carbon-intensity APIs, oracle push
    /// subscriptions) to be wired into this tracker. No such stream is currently
    /// connected. The previous implementation merely stamped
    /// `monitoring.last_calculation = Utc::now()` and returned `Ok(())`,
    /// reporting real-time tracking as ACTIVE when nothing was actually
    /// streaming — a no-op presented as success.
    ///
    /// Until a live feed is wired, this fails closed (mirroring
    /// [`Self::request_oracle_verification`]) so callers cannot mistake an idle
    /// tracker for an active one. Refreshing the monitoring timestamp is
    /// deliberately NOT done here, because doing so would falsely imply a fresh
    /// real-time calculation had occurred.
    pub fn implement_real_time_carbon_tracking(&self) -> Result<(), OracleError> {
        Err(OracleError::NetworkError(
            "real-time carbon tracking is not active: no live data stream \
             (smart-meter feed / grid-carbon-intensity API / oracle push \
             subscription) is wired into this tracker. Refusing to report \
             real-time tracking as active."
                .to_string(),
        ))
    }

    // Helper methods

    fn calculate_local_emissions(
        &self,
        energy_consumption_mwh: f64,
        energy_sources: &HashMap<EnergySourceType, f64>,
        region: &Region,
    ) -> Result<f64, OracleError> {
        let mut total_emissions = 0.0;

        for (source_type, percentage) in energy_sources {
            let emission_factor = self.get_emission_factor(source_type, region)?;
            total_emissions += energy_consumption_mwh * (percentage / 100.0) * emission_factor;
        }

        Ok(total_emissions)
    }

    async fn request_oracle_verification(
        &self,
        _entity_id: &str,
        _energy_consumption_mwh: f64,
        _energy_sources: &HashMap<EnergySourceType, f64>,
        _region: &Region,
    ) -> Result<Vec<OracleDataPoint>, OracleError> {
        // Honest behavior: this tracker is NOT wired to an external, independent
        // environmental-oracle feed. The previous implementation manufactured
        // three "oracle" submissions by taking the SAME local emissions estimate
        // and perturbing it by +2% / -2% / +1%, with hardcoded confidence scores
        // and fabricated data_sources, then presented those three copies of one
        // local number as an agreeing multi-oracle consensus. That is fabricated
        // data presented as measured.
        //
        // Real multi-oracle consensus requires independently registered, bonded
        // oracles to submit signed data through `EnvironmentalOracle`
        // (see oracle.rs: request_verification / submit_verification /
        // process_consensus). Until such a live feed exists, we must not invent
        // agreement. Report the true state via the real oracle registry: if no
        // active oracle is available, the carbon footprint is unverified.
        let active_oracles = self.oracle.active_oracle_count();
        Err(OracleError::ConsensusNotReached(format!(
            "carbon footprint unverified: no independent environmental-oracle \
             submissions available ({active_oracles} active registered oracle(s), \
             none wired to a live signed-data feed). Refusing to fabricate \
             multi-oracle consensus from the local estimate."
        )))
    }

    fn process_oracle_consensus(
        &self,
        submissions: &[OracleDataPoint],
        _local_value: f64,
    ) -> Result<OracleConsensusResult, OracleError> {
        let total_oracles = submissions.len();

        // Calculate weighted consensus
        let total_weight: f64 = submissions.iter().map(|s| s.confidence_score).sum();
        let weighted_sum: f64 = submissions
            .iter()
            .map(|s| s.submitted_value * s.confidence_score)
            .sum();

        let consensus_value = weighted_sum / total_weight;

        // Check agreement threshold
        let agreeing_oracles = submissions
            .iter()
            .filter(|s| (s.submitted_value - consensus_value).abs() / consensus_value < 0.05)
            .count();

        let consensus_percentage = (agreeing_oracles as f64 / total_oracles as f64) * 100.0;

        Ok(OracleConsensusResult {
            participating_oracles: total_oracles,
            consensus_percentage,
            oracle_submissions: submissions.to_vec(),
            consensus_value,
            consensus_achieved: consensus_percentage >= 67.0,
        })
    }

    fn verify_renewable_percentage(
        &self,
        _entity_id: &str,
        energy_sources: &HashMap<EnergySourceType, f64>,
    ) -> Result<f64, OracleError> {
        let renewable_types = [
            EnergySourceType::Solar,
            EnergySourceType::Wind,
            EnergySourceType::Hydro,
            EnergySourceType::Geothermal,
        ];

        let renewable_percentage: f64 = energy_sources
            .iter()
            .filter(|(source, _)| renewable_types.contains(source))
            .map(|(_, percentage)| percentage)
            .sum();

        Ok(renewable_percentage)
    }

    fn calculate_applied_offsets(&self, entity_id: &str) -> Result<f64, OracleError> {
        let offsets = self
            .carbon_offsets
            .read()
            .map_err(|_| OracleError::LockPoisoned)?;
        let total_offsets = offsets
            .values()
            .filter(|offset| offset.owner_id == entity_id && offset.is_valid())
            .map(|offset| offset.amount_tonnes())
            .sum();

        Ok(total_offsets)
    }

    fn calculate_environmental_metrics(
        &self,
        total_emissions: f64,
        total_offsets: f64,
        renewable_percentage: f64,
        energy_consumption_mwh: f64,
    ) -> EnvironmentalMetrics {
        let carbon_intensity = if energy_consumption_mwh > 0.0 {
            (total_emissions * 1000.0) / energy_consumption_mwh // kg CO2e per MWh
        } else {
            0.0
        };

        let offset_efficiency = if total_emissions > 0.0 {
            (total_offsets / total_emissions) * 100.0
        } else {
            100.0
        };

        // Environmental score calculation (0-100)
        let mut score = 50.0; // Base score
        score += renewable_percentage * 0.3; // Up to 30 points for renewables
        score += offset_efficiency.min(100.0) * 0.2; // Up to 20 points for offsets

        // Bonus for being carbon negative
        if total_emissions - total_offsets < 0.0 {
            score = score.min(95.0) + 5.0; // Bonus 5 points, max 100
        }

        EnvironmentalMetrics {
            // No hashrate is supplied to the carbon-footprint path, so a real
            // hashrate/MW efficiency cannot be computed here; report it as
            // unavailable rather than emitting a fabricated constant.
            energy_efficiency: None,
            carbon_intensity,
            green_mining_percentage: renewable_percentage,
            offset_efficiency,
            environmental_score: score.min(100.0),
        }
    }

    fn collect_verification_proofs(
        &self,
        entity_id: &str,
    ) -> Result<Vec<VerificationProof>, OracleError> {
        let mut proofs = Vec::new();

        // Collect REC proofs
        let recs = self
            .renewable_certificates
            .read()
            .map_err(|_| OracleError::LockPoisoned)?;
        for (_, cert) in recs.iter() {
            if cert.owner_id == entity_id {
                proofs.push(VerificationProof {
                    proof_type: ProofType::RenewableEnergyCertificate,
                    proof_data: cert.certificate_hash.clone(),
                    issuer: cert.certificate.issuer.clone(),
                    timestamp: cert.issue_date,
                    expiry: Some(cert.expiry_date),
                });
            }
        }

        // Collect carbon offset proofs
        let offsets = self
            .carbon_offsets
            .read()
            .map_err(|_| OracleError::LockPoisoned)?;
        for (_, offset) in offsets.iter() {
            if offset.owner_id == entity_id {
                proofs.push(VerificationProof {
                    proof_type: ProofType::CarbonOffsetCertificate,
                    proof_data: offset.certificate_hash.clone(),
                    issuer: offset.offset.issuer.clone(),
                    timestamp: offset.issue_date,
                    expiry: Some(offset.expiry_date),
                });
            }
        }

        Ok(proofs)
    }

    fn update_tracking_data(
        &self,
        entity_id: &str,
        result: &CarbonTrackingResult,
        energy_sources: HashMap<EnergySourceType, f64>,
    ) -> Result<(), OracleError> {
        let mut tracking = self
            .tracking_data
            .write()
            .map_err(|_| OracleError::LockPoisoned)?;

        tracking.insert(
            entity_id.to_string(),
            TrackingData {
                entity_id: entity_id.to_string(),
                current_emissions: result.total_emissions,
                current_offsets: result.total_offsets,
                energy_sources,
                last_updated: Utc::now(),
                verification_status: VerificationStatus::Verified,
            },
        );

        Ok(())
    }

    fn update_monitoring_data(&self, result: &CarbonTrackingResult) -> Result<(), OracleError> {
        let mut monitoring = self
            .monitoring_data
            .write()
            .map_err(|_| OracleError::LockPoisoned)?;

        // Update network-wide statistics
        monitoring.total_network_emissions += result.total_emissions;
        monitoring.total_network_offsets += result.total_offsets;
        monitoring.renewable_energy_percentage =
            (monitoring.renewable_energy_percentage + result.renewable_percentage) / 2.0;
        monitoring.last_calculation = Utc::now();

        Ok(())
    }

    fn get_emission_factor(
        &self,
        source_type: &EnergySourceType,
        _region: &Region,
    ) -> Result<f64, OracleError> {
        // Emission factors in tonnes CO2e per MWh
        let emission_factor = match source_type {
            EnergySourceType::Coal => 0.95,
            EnergySourceType::NaturalGas => 0.45,
            EnergySourceType::Nuclear => 0.02,
            EnergySourceType::Solar => 0.04,
            EnergySourceType::Wind => 0.01,
            EnergySourceType::Hydro => 0.02,
            EnergySourceType::Geothermal => 0.03,
            EnergySourceType::Oil => 0.65,
            EnergySourceType::Biomass => 0.23,
            EnergySourceType::Grid => 0.475,   // Global average
            EnergySourceType::Unknown => 0.50, // Conservative estimate
            EnergySourceType::Other => 0.50,   // Conservative estimate
        };

        Ok(emission_factor)
    }

    /// Baseline structural integrity gate shared by every proof verifier.
    ///
    /// This is deliberately fail-closed: a proof carrying no payload, no
    /// issuer, a future-dated timestamp, or an elapsed expiry is rejected
    /// outright. It does NOT (yet) perform registry-API lookups or
    /// cryptographic signature verification — those require external
    /// infrastructure that is not wired in-tree. Until they are, this ensures
    /// a trivially forged / malformed proof can no longer bypass integrity
    /// checking by simply being present, which the previous unconditional
    /// `Ok(true)` allowed.
    ///
    /// Returns `false` when the proof fails a baseline check; the caller
    /// treats `false` as "integrity not verified".
    fn validate_proof_structure(&self, proof: &VerificationProof) -> bool {
        // A genuine proof always carries a payload and a named issuer;
        // a forged empty proof carries neither.
        if proof.proof_data.is_empty() || proof.issuer.trim().is_empty() {
            return false;
        }

        let now = Utc::now();

        // Reject proofs stamped in the future (allow small clock skew).
        if proof.timestamp > now + chrono::Duration::minutes(5) {
            return false;
        }

        // Reject expired proofs.
        if let Some(expiry) = proof.expiry {
            if expiry <= now {
                return false;
            }
        }

        true
    }

    fn verify_rec_proof(&self, proof: &VerificationProof) -> Result<bool, OracleError> {
        // Verify REC certificate hash and validity.
        // NOTE: registry-API cross-checking is not yet wired; until it is, this
        // fail-closed structural gate rejects malformed/expired/forged-empty
        // proofs instead of blindly accepting them.
        Ok(self.validate_proof_structure(proof))
    }

    fn verify_offset_proof(&self, proof: &VerificationProof) -> Result<bool, OracleError> {
        // Verify carbon offset certificate.
        // NOTE: carbon-registry cross-checking is not yet wired; fail-closed
        // structural validation applies until it is.
        Ok(self.validate_proof_structure(proof))
    }

    fn verify_meter_proof(&self, proof: &VerificationProof) -> Result<bool, OracleError> {
        // Verify smart meter reading attestation.
        // NOTE: cryptographic signature verification against a registered meter
        // key is not yet wired; fail-closed structural validation applies until
        // it is.
        Ok(self.validate_proof_structure(proof))
    }
}

/// Public API for carbon tracking validation
pub async fn validate_carbon_footprint_calculation(
    tracker: &CarbonTracker,
    entity_id: &str,
    energy_consumption_mwh: f64,
    energy_sources: HashMap<EnergySourceType, f64>,
    region: Region,
) -> Result<CarbonTrackingResult, OracleError> {
    tracker
        .validate_carbon_footprint_calculation(
            entity_id,
            energy_consumption_mwh,
            energy_sources,
            region,
        )
        .await
}

pub async fn test_multi_oracle_consensus(
    tracker: &CarbonTracker,
    test_data: Vec<OracleDataPoint>,
) -> Result<OracleConsensusResult, OracleError> {
    tracker.test_multi_oracle_consensus(test_data).await
}

pub fn verify_environmental_data_integrity(
    tracker: &CarbonTracker,
    data: &EnvironmentalData,
    proofs: &[VerificationProof],
) -> Result<bool, OracleError> {
    tracker.verify_environmental_data_integrity(data, proofs)
}

pub fn implement_real_time_carbon_tracking(tracker: &CarbonTracker) -> Result<(), OracleError> {
    tracker.implement_real_time_carbon_tracking()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::environmental::emissions::EmissionsCalculator;
    use crate::environmental::oracle::EnvironmentalOracle;

    fn tracker_with_no_oracles() -> CarbonTracker {
        // Oracle registry with zero registered/active oracles — i.e. no live feed.
        let oracle = Arc::new(EnvironmentalOracle::new(1_000));
        let calculator = Arc::new(EmissionsCalculator::new());
        CarbonTracker::new(oracle, calculator)
    }

    /// With no independent oracles registered, the tracker must report the
    /// footprint as UNVERIFIED rather than fabricating an agreeing multi-oracle
    /// consensus out of a single local estimate.
    #[tokio::test]
    async fn no_oracles_yields_unverified_not_fabricated_consensus() {
        let tracker = tracker_with_no_oracles();
        let mut sources = HashMap::new();
        sources.insert(EnergySourceType::Coal, 100.0);

        let result = tracker
            .validate_carbon_footprint_calculation(
                "miner-1",
                10.0,
                sources,
                Region::NorthAmerica,
            )
            .await;

        match result {
            Err(OracleError::ConsensusNotReached(msg)) => {
                assert!(
                    msg.contains("unverified"),
                    "error must state the footprint is unverified, got: {msg}"
                );
            }
            other => panic!(
                "expected ConsensusNotReached (unverified) when no oracles are \
                 registered; fabricated consensus must never be returned, got: {other:?}"
            ),
        }
    }

    /// The internal request path must never manufacture synthetic oracle
    /// submissions from the local estimate.
    #[tokio::test]
    async fn request_oracle_verification_does_not_fabricate() {
        let tracker = tracker_with_no_oracles();
        let mut sources = HashMap::new();
        sources.insert(EnergySourceType::NaturalGas, 100.0);

        let submissions = tracker
            .request_oracle_verification("miner-1", 5.0, &sources, &Region::Europe)
            .await;

        assert!(
            submissions.is_err(),
            "request_oracle_verification must not return fabricated submissions \
             when no real oracle feed exists"
        );
    }

    /// Real-time carbon tracking must not report itself as active when no live
    /// data stream is wired. It must fail closed rather than return a no-op
    /// `Ok(())` that stamps a timestamp and implies live tracking is running.
    #[test]
    fn real_time_tracking_fails_closed_when_no_stream_wired() {
        let tracker = tracker_with_no_oracles();

        // The monitoring timestamp must be untouched by a failed activation,
        // so a caller cannot mistake an idle tracker for a freshly updated one.
        let before = tracker
            .monitoring_data
            .read()
            .expect("lock")
            .last_calculation;

        let result = tracker.implement_real_time_carbon_tracking();

        match result {
            Err(OracleError::NetworkError(msg)) => {
                assert!(
                    msg.contains("not active"),
                    "error must state real-time tracking is not active, got: {msg}"
                );
            }
            other => panic!(
                "expected NetworkError (not active) when no live data stream is \
                 wired; a no-op success must never be returned, got: {other:?}"
            ),
        }

        let after = tracker
            .monitoring_data
            .read()
            .expect("lock")
            .last_calculation;
        assert_eq!(
            before, after,
            "failed real-time activation must not mutate monitoring state"
        );
    }

    /// Energy efficiency must not be a fabricated placeholder: with no hashrate
    /// input available to this path it must be reported as `None`, while the
    /// genuinely derivable metrics are still computed from the inputs.
    #[test]
    fn energy_efficiency_is_not_fabricated() {
        let tracker = tracker_with_no_oracles();

        let metrics = tracker.calculate_environmental_metrics(
            1.0,  // total_emissions (tonnes CO2e)
            0.5,  // total_offsets
            40.0, // renewable_percentage
            2.0,  // energy_consumption_mwh
        );

        assert_eq!(
            metrics.energy_efficiency, None,
            "energy_efficiency must be None when no hashrate is available, \
             not a fabricated constant"
        );
        // Sibling metrics remain genuinely computed from the inputs.
        assert_eq!(metrics.carbon_intensity, (1.0 * 1000.0) / 2.0);
        assert_eq!(metrics.green_mining_percentage, 40.0);
        assert_eq!(metrics.offset_efficiency, (0.5 / 1.0) * 100.0);
    }

    fn sample_env_data() -> EnvironmentalData {
        EnvironmentalData::CarbonOffset {
            offset_id: "offset-1".to_string(),
            issuer: "registry".to_string(),
            amount_tonnes: 10.0,
            project_type: "reforestation".to_string(),
            project_location: "somewhere".to_string(),
            vintage_year: 2025,
            registry_url: "https://registry.example".to_string(),
        }
    }

    /// A forged proof with no payload / no issuer must NOT pass integrity
    /// verification — the previous unconditional `Ok(true)` accepted anything.
    #[test]
    fn empty_proof_is_rejected() {
        let tracker = tracker_with_no_oracles();
        let data = sample_env_data();

        let forged = VerificationProof {
            proof_type: ProofType::CarbonOffsetCertificate,
            proof_data: Vec::new(),
            issuer: String::new(),
            timestamp: Utc::now(),
            expiry: None,
        };

        let ok = tracker
            .verify_environmental_data_integrity(&data, std::slice::from_ref(&forged))
            .expect("verification must not error");
        assert!(!ok, "empty forged proof must fail integrity verification");
    }

    /// An expired proof must be rejected (fail-closed on staleness).
    #[test]
    fn expired_proof_is_rejected() {
        let tracker = tracker_with_no_oracles();
        let data = sample_env_data();

        let expired = VerificationProof {
            proof_type: ProofType::RenewableEnergyCertificate,
            proof_data: vec![1, 2, 3],
            issuer: "registry".to_string(),
            timestamp: Utc::now() - chrono::Duration::days(2),
            expiry: Some(Utc::now() - chrono::Duration::days(1)),
        };

        let ok = tracker
            .verify_environmental_data_integrity(&data, std::slice::from_ref(&expired))
            .expect("verification must not error");
        assert!(!ok, "expired proof must fail integrity verification");
    }

    /// A future-dated proof (beyond clock-skew tolerance) must be rejected.
    #[test]
    fn future_dated_proof_is_rejected() {
        let tracker = tracker_with_no_oracles();
        let data = sample_env_data();

        let future = VerificationProof {
            proof_type: ProofType::SmartMeterReading,
            proof_data: vec![9, 9, 9],
            issuer: "meter-op".to_string(),
            timestamp: Utc::now() + chrono::Duration::hours(1),
            expiry: None,
        };

        let ok = tracker
            .verify_environmental_data_integrity(&data, std::slice::from_ref(&future))
            .expect("verification must not error");
        assert!(!ok, "future-dated proof must fail integrity verification");
    }

    /// A well-formed, unexpired proof still passes the baseline gate (the gate
    /// tightens the check without breaking legitimate proofs).
    #[test]
    fn well_formed_proof_passes_baseline() {
        let tracker = tracker_with_no_oracles();
        let data = sample_env_data();

        let good = VerificationProof {
            proof_type: ProofType::CarbonOffsetCertificate,
            proof_data: vec![1, 2, 3, 4],
            issuer: "registry".to_string(),
            timestamp: Utc::now(),
            expiry: Some(Utc::now() + chrono::Duration::days(30)),
        };

        let ok = tracker
            .verify_environmental_data_integrity(&data, std::slice::from_ref(&good))
            .expect("verification must not error");
        assert!(ok, "well-formed unexpired proof must pass baseline gate");
    }

    /// The validated energy-source mix must be persisted into the tracked
    /// record — not silently dropped as an empty map while the record is
    /// stamped `Verified`. Previously `update_tracking_data` stored
    /// `HashMap::new()`, presenting an empty mix as verified data.
    #[test]
    fn update_tracking_data_persists_energy_sources() {
        let tracker = tracker_with_no_oracles();

        let mut energy_sources = HashMap::new();
        energy_sources.insert(EnergySourceType::Wind, 60.0);
        energy_sources.insert(EnergySourceType::Coal, 40.0);

        let result = CarbonTrackingResult {
            total_emissions: 3.0,
            total_offsets: 1.0,
            net_carbon_footprint: 2.0,
            renewable_percentage: 60.0,
            timestamp: Utc::now(),
            oracle_consensus: OracleConsensusResult {
                participating_oracles: 0,
                consensus_percentage: 0.0,
                oracle_submissions: Vec::new(),
                consensus_value: 3.0,
                consensus_achieved: false,
            },
            metrics: EnvironmentalMetrics {
                energy_efficiency: None,
                carbon_intensity: 0.0,
                green_mining_percentage: 60.0,
                offset_efficiency: 0.0,
                environmental_score: 0.0,
            },
            verification_proofs: Vec::new(),
        };

        tracker
            .update_tracking_data("miner-1", &result, energy_sources.clone())
            .expect("update must succeed");

        let tracking = tracker.tracking_data.read().expect("lock");
        let stored = tracking
            .get("miner-1")
            .expect("tracking record must exist");

        assert_eq!(
            stored.energy_sources, energy_sources,
            "the validated energy-source mix must be stored, not dropped"
        );
        // A record carrying the real mix is what may be stamped Verified.
        assert_eq!(stored.verification_status, VerificationStatus::Verified);
    }
}
