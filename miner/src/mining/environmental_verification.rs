use super::fraud_detection::{FraudDetectionConfig, FraudDetector};
use super::reward::EnvironmentalProfile;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::RwLock;

/// Environmental verification service for validating miner claims
#[derive(Clone)]
pub struct EnvironmentalVerifier {
    verified_miners: Arc<RwLock<HashMap<String, VerifiedMinerProfile>>>,
    rec_registry: Arc<RwLock<RECRegistry>>,
    consumed_certificates: Arc<RwLock<ConsumedCertificates>>,
    fraud_detector: Arc<FraudDetector>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerifiedMinerProfile {
    pub miner_id: String,
    pub environmental_profile: EnvironmentalProfile,
    pub verification_timestamp: u64,
    pub verification_expiry: u64,
    pub rec_certificates: Vec<RECCertificate>,
    pub efficiency_audit: Option<EfficiencyAudit>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RECCertificate {
    pub certificate_id: String,
    pub issuer: String,
    pub coverage_mwh: f64,
    pub valid_from: u64,
    pub valid_until: u64,
    pub verified: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EfficiencyAudit {
    pub auditor: String,
    pub hash_rate_per_watt: f64,
    pub cooling_efficiency: f64,
    pub overall_pue: f64, // Power Usage Effectiveness
    pub audit_timestamp: u64,
}

#[derive(Default)]
pub struct RECRegistry {
    certificates: HashMap<String, RECCertificate>,
    trusted_issuers: Vec<String>,
}

/// Tracks consumed certificates to prevent double-claiming
#[derive(Default)]
pub struct ConsumedCertificates {
    /// Maps certificate_id to (miner_id, consumption_timestamp)
    consumed: HashMap<String, (String, u64)>,
    /// Maps miner_id to set of consumed certificate_ids
    by_miner: HashMap<String, HashSet<String>>,
}

impl ConsumedCertificates {
    /// Check if a certificate has been consumed
    pub fn is_consumed(&self, certificate_id: &str) -> bool {
        self.consumed.contains_key(certificate_id)
    }

    /// Mark a certificate as consumed by a miner
    pub fn consume(
        &mut self,
        certificate_id: String,
        miner_id: String,
        timestamp: u64,
    ) -> Result<(), VerificationError> {
        if self.is_consumed(&certificate_id) {
            return Err(VerificationError::CertificateAlreadyConsumed(
                certificate_id,
            ));
        }

        self.consumed
            .insert(certificate_id.clone(), (miner_id.clone(), timestamp));
        self.by_miner
            .entry(miner_id)
            .or_default()
            .insert(certificate_id);

        Ok(())
    }

    /// Get all certificates consumed by a miner
    pub fn get_miner_certificates(&self, miner_id: &str) -> Option<&HashSet<String>> {
        self.by_miner.get(miner_id)
    }
}

impl Default for EnvironmentalVerifier {
    fn default() -> Self {
        Self::new()
    }
}

impl EnvironmentalVerifier {
    fn normalize_issuer(issuer: &str) -> String {
        issuer.trim().to_ascii_uppercase()
    }

    pub fn new() -> Self {
        let fraud_config = FraudDetectionConfig::default();

        Self {
            verified_miners: Arc::new(RwLock::new(HashMap::new())),
            rec_registry: Arc::new(RwLock::new(RECRegistry {
                trusted_issuers: vec![
                    "US-EPA".to_string(),
                    "EU-ETS".to_string(),
                    "GREEN-E".to_string(),
                ],
                ..Default::default()
            })),
            consumed_certificates: Arc::new(RwLock::new(ConsumedCertificates::default())),
            fraud_detector: Arc::new(FraudDetector::new(fraud_config)),
        }
    }

    /// Verify a miner's environmental claims
    pub async fn verify_miner_profile(
        &self,
        miner_id: String,
        claimed_profile: EnvironmentalProfile,
        rec_certificates: Vec<RECCertificate>,
        efficiency_audit: Option<EfficiencyAudit>,
    ) -> Result<VerifiedMinerProfile, VerificationError> {
        // First check if any certificates are already consumed
        let consumed = self.consumed_certificates.read().await;
        for cert in &rec_certificates {
            if consumed.is_consumed(&cert.certificate_id) {
                return Err(VerificationError::CertificateAlreadyConsumed(
                    cert.certificate_id.clone(),
                ));
            }
        }
        drop(consumed);

        // Verify REC certificates
        let verified_recs = self.verify_rec_certificates(&rec_certificates).await?;

        // Calculate actual renewable percentage based on verified RECs
        let renewable_percentage = self.calculate_renewable_percentage(&verified_recs)?;

        // Verify efficiency audit if provided
        let verified_efficiency = if let Some(audit) = &efficiency_audit {
            self.verify_efficiency_audit(audit)?
        } else {
            0.0
        };

        // Create verified profile
        let verified_profile = EnvironmentalProfile {
            renewable_percentage,
            efficiency_score: verified_efficiency,
            verified: true,
            rec_coverage: renewable_percentage,
        };

        // Run fraud detection analysis
        let suspicious_activities = self
            .fraud_detector
            .analyze_claim(
                &miner_id,
                &verified_profile,
                &verified_recs,
                efficiency_audit.as_ref(),
            )
            .await;

        // If high fraud risk, reject the claim
        let risk_score = self.fraud_detector.get_risk_score(&miner_id).await;
        if risk_score > 0.8 {
            return Err(VerificationError::FraudDetected(format!(
                "High fraud risk detected: {:.2}. Activities: {:?}",
                risk_score, suspicious_activities
            )));
        }

        // Mark certificates as consumed
        let mut consumed = self.consumed_certificates.write().await;
        let timestamp = current_timestamp();
        for cert in &verified_recs {
            consumed.consume(cert.certificate_id.clone(), miner_id.clone(), timestamp)?;
        }
        drop(consumed);

        let miner_profile = VerifiedMinerProfile {
            miner_id: miner_id.clone(),
            environmental_profile: verified_profile,
            verification_timestamp: timestamp,
            verification_expiry: timestamp + 30 * 24 * 3600, // 30 days
            rec_certificates: verified_recs,
            efficiency_audit,
        };

        // Store verified profile
        let mut miners = self.verified_miners.write().await;
        miners.insert(miner_id, miner_profile.clone());

        Ok(miner_profile)
    }

    /// Get a verified miner's environmental profile
    pub async fn get_verified_profile(&self, miner_id: &str) -> Option<EnvironmentalProfile> {
        self.get_verified_profile_at(miner_id, current_timestamp()).await
    }

    /// Get a verified miner's environmental profile at a specific timestamp.
    ///
    /// This is primarily useful for tests/simulations that need deterministic time travel.
    pub async fn get_verified_profile_at(
        &self,
        miner_id: &str,
        now: u64,
    ) -> Option<EnvironmentalProfile> {
        let miners = self.verified_miners.read().await;
        miners
            .get(miner_id)
            .filter(|profile| profile.verification_expiry > now)
            .map(|profile| profile.environmental_profile.clone())
    }

    /// Verify REC certificates
    async fn verify_rec_certificates(
        &self,
        certificates: &[RECCertificate],
    ) -> Result<Vec<RECCertificate>, VerificationError> {
        let registry = self.rec_registry.read().await;
        let mut verified = Vec::new();

        for cert in certificates {
            // Check if issuer is trusted
            let issuer_norm = Self::normalize_issuer(&cert.issuer);
            if !registry.trusted_issuers.contains(&issuer_norm) {
                continue;
            }

            // Check validity period
            let now = current_timestamp();
            if cert.valid_from > now || cert.valid_until < now {
                continue;
            }

            // Verify certificate exists in registry
            if let Some(registered_cert) = registry.certificates.get(&cert.certificate_id) {
                if registered_cert.coverage_mwh == cert.coverage_mwh {
                    let mut verified_cert = cert.clone();
                    verified_cert.verified = true;
                    verified.push(verified_cert);
                }
            }
        }

        Ok(verified)
    }

    /// Calculate renewable percentage based on verified RECs
    fn calculate_renewable_percentage(
        &self,
        verified_recs: &[RECCertificate],
    ) -> Result<f64, VerificationError> {
        if verified_recs.is_empty() {
            return Ok(0.0);
        }

        // Sum total MWh coverage
        let total_mwh: f64 = verified_recs.iter().map(|cert| cert.coverage_mwh).sum();

        // For simplicity, assume 100% coverage if > 100 MWh/month
        // In production, this would be compared against actual consumption
        Ok((total_mwh / 100.0).min(1.0_f64))
    }

    /// Verify efficiency audit
    fn verify_efficiency_audit(&self, audit: &EfficiencyAudit) -> Result<f64, VerificationError> {
        // Check if audit is recent (within 90 days)
        if current_timestamp() - audit.audit_timestamp > 90 * 24 * 3600 {
            return Err(VerificationError::AuditExpired);
        }

        // Calculate efficiency score based on metrics
        let mut score: f64 = 0.0;

        // Hash rate per watt scoring (higher is better)
        if audit.hash_rate_per_watt > 100.0 {
            score += 0.4; // Excellent
        } else if audit.hash_rate_per_watt > 50.0 {
            score += 0.2; // Good
        }

        // PUE scoring (lower is better, 1.0 is perfect)
        if audit.overall_pue < 1.2 {
            score += 0.4; // Excellent
        } else if audit.overall_pue < 1.5 {
            score += 0.2; // Good
        }

        // Cooling efficiency bonus
        if audit.cooling_efficiency > 0.9 {
            score += 0.2;
        }

        Ok(score.min(1.0))
    }

    /// Register a trusted REC issuer
    pub async fn register_trusted_issuer(&self, issuer: String) {
        let mut registry = self.rec_registry.write().await;
        let issuer_norm = Self::normalize_issuer(&issuer);
        if !registry.trusted_issuers.contains(&issuer_norm) {
            registry.trusted_issuers.push(issuer_norm);
        }
    }

    /// Register a REC certificate in the registry
    pub async fn register_rec_certificate(&self, certificate: RECCertificate) {
        let mut registry = self.rec_registry.write().await;
        registry
            .certificates
            .insert(certificate.certificate_id.clone(), certificate);
    }

    /// Get fraud report for a miner
    pub async fn get_fraud_report(
        &self,
        miner_id: &str,
    ) -> Option<super::fraud_detection::FraudReport> {
        self.fraud_detector.get_fraud_report(miner_id).await
    }

    /// Get fraud risk score for a miner
    pub async fn get_fraud_risk_score(&self, miner_id: &str) -> f64 {
        self.fraud_detector.get_risk_score(miner_id).await
    }
}

#[derive(Debug, thiserror::Error)]
pub enum VerificationError {
    #[error("No valid REC certificates provided")]
    NoValidCertificates,

    #[error("Efficiency audit has expired")]
    AuditExpired,

    #[error("Invalid certificate")]
    InvalidCertificate,

    #[error("Certificate with ID {0} has already been consumed")]
    CertificateAlreadyConsumed(String),

    #[error("Fraud detected: {0}")]
    FraudDetected(String),
}

fn current_timestamp() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_environmental_verification() {
        let verifier = EnvironmentalVerifier::new();

        // Register trusted issuer
        verifier
            .register_trusted_issuer("Green-e".to_string())
            .await;

        // Create and register a REC certificate
        let rec = RECCertificate {
            certificate_id: "REC-001".to_string(),
            issuer: "Green-e".to_string(),
            coverage_mwh: 150.0,
            valid_from: current_timestamp() - 3600,
            valid_until: current_timestamp() + 30 * 24 * 3600,
            verified: false,
        };

        verifier.register_rec_certificate(rec.clone()).await;

        // Create efficiency audit
        let audit = EfficiencyAudit {
            auditor: "EnergyAuditor Inc.".to_string(),
            hash_rate_per_watt: 120.0,
            cooling_efficiency: 0.95,
            overall_pue: 1.15,
            audit_timestamp: current_timestamp() - 24 * 3600,
        };

        // Verify miner profile
        let claimed_profile = EnvironmentalProfile {
            renewable_percentage: 1.0,
            efficiency_score: 1.0,
            verified: false,
            rec_coverage: 1.0,
        };

        let result = verifier
            .verify_miner_profile(
                "miner-001".to_string(),
                claimed_profile,
                vec![rec],
                Some(audit),
            )
            .await;

        assert!(result.is_ok());
        let verified = result.unwrap();
        assert!(verified.environmental_profile.verified);
        assert_eq!(verified.environmental_profile.renewable_percentage, 1.0);
        assert!(verified.environmental_profile.efficiency_score > 0.8);
    }

    #[tokio::test]
    async fn test_expired_audit_rejection() {
        let verifier = EnvironmentalVerifier::new();

        let old_audit = EfficiencyAudit {
            auditor: "EnergyAuditor Inc.".to_string(),
            hash_rate_per_watt: 120.0,
            cooling_efficiency: 0.95,
            overall_pue: 1.15,
            audit_timestamp: current_timestamp() - 100 * 24 * 3600, // 100 days old
        };

        let result = verifier.verify_efficiency_audit(&old_audit);
        assert!(matches!(result, Err(VerificationError::AuditExpired)));
    }
}
