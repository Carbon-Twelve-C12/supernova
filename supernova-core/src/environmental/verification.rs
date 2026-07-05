use crate::environmental::emissions::VerificationStatus;
use crate::environmental::types::Region;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::sync::{Arc, RwLock};
use thiserror::Error;

/// Verification service errors
#[derive(Error, Debug)]
pub enum VerificationError {
    #[error("API error: {0}")]
    ApiError(String),

    #[error("Invalid certificate: {0}")]
    InvalidCertificate(String),

    #[error("Certificate expired on {0}")]
    CertificateExpired(DateTime<Utc>),

    #[error("Invalid offset: {0}")]
    InvalidOffset(String),

    #[error("Network error: {0}")]
    NetworkError(String),

    #[error("Verification service unavailable")]
    ServiceUnavailable,

    #[error("Internal lock poisoned — another thread panicked while holding the lock")]
    LockPoisoned,
}

/// Renewable Energy Certificate
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenewableCertificate {
    /// Certificate ID
    pub certificate_id: String,
    /// Issuing organization
    pub issuer: String,
    /// Certificate type (e.g., "Solar", "Wind", "Hydro")
    pub certificate_type: String,
    /// Amount of energy in kWh
    pub amount_kwh: f64,
    /// Generation period start time
    pub generation_start: DateTime<Utc>,
    /// Generation period end time
    pub generation_end: DateTime<Utc>,
    /// Generation location
    pub location: Region,
    /// Verification status
    pub verification_status: VerificationStatus,
    /// Verification URL or reference
    pub verification_url: Option<String>,
    /// Additional metadata
    pub metadata: std::collections::HashMap<String, String>,
}

/// Carbon Offset
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CarbonOffset {
    /// Offset ID
    pub offset_id: String,
    /// Issuing organization
    pub issuer: String,
    /// Offset type (e.g., "Reforestation", "Direct Air Capture")
    pub offset_type: String,
    /// Amount of carbon offset in tonnes CO2e
    pub amount_tonnes: f64,
    /// Offset period start time
    pub period_start: DateTime<Utc>,
    /// Offset period end time (if applicable)
    pub period_end: Option<DateTime<Utc>>,
    /// Offset project location
    pub location: Region,
    /// Verification status
    pub verification_status: VerificationStatus,
    /// Verification URL or reference
    pub verification_url: Option<String>,
    /// Additional metadata
    pub metadata: std::collections::HashMap<String, String>,
}

/// Verification service configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationConfig {
    /// API endpoint for verification
    pub api_endpoint: Option<String>,
    /// API key for authentication
    pub api_key: Option<String>,
    /// Timeout for verification requests (seconds)
    pub timeout_seconds: u64,
    /// Maximum certificate age in days
    pub max_certificate_age_days: u32,
    /// Whether to cache verification results
    pub cache_results: bool,
    /// Cache expiration time in hours
    pub cache_expiration_hours: u32,
    /// Whether to accept self-reported certificates
    pub accept_self_reported: bool,
}

impl Default for VerificationConfig {
    fn default() -> Self {
        Self {
            api_endpoint: None,
            api_key: None,
            timeout_seconds: 30,
            max_certificate_age_days: 365 * 2, // 2 years
            cache_results: true,
            cache_expiration_hours: 24,
            accept_self_reported: false,
        }
    }
}

/// Trait for verification services
#[async_trait]
pub trait VerificationProvider {
    /// Verify a renewable energy certificate
    async fn verify_certificate(
        &self,
        certificate: &RenewableCertificate,
    ) -> Result<VerificationStatus, VerificationError>;

    /// Verify a carbon offset
    async fn verify_offset(
        &self,
        offset: &CarbonOffset,
    ) -> Result<VerificationStatus, VerificationError>;
}

/// Verification service for environmental assets
pub struct VerificationService {
    /// Configuration for the verification service
    config: Arc<RwLock<VerificationConfig>>,
    /// HTTP client for API requests
    client: Client,
    /// Cache of verification results
    verification_cache:
        Arc<RwLock<std::collections::HashMap<String, (VerificationStatus, DateTime<Utc>)>>>,
}

impl VerificationService {
    /// Create a new verification service
    pub fn new(config: VerificationConfig) -> Self {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(config.timeout_seconds))
            .build()
            .unwrap_or_default();

        Self {
            config: Arc::new(RwLock::new(config)),
            client,
            verification_cache: Arc::new(RwLock::new(std::collections::HashMap::new())),
        }
    }

    /// Create a new verification service with default configuration
    pub fn default() -> Self {
        Self::new(VerificationConfig::default())
    }

    /// Update the service configuration. Recovers from lock poisoning so
    /// a prior panic in a config writer can't cascade into this path.
    pub fn update_config(&self, config: VerificationConfig) {
        let mut current_config = self
            .config
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        *current_config = config;
    }

    /// Check if a certificate is expired. Read-only; recovers from
    /// lock poisoning.
    pub fn is_certificate_expired(&self, certificate: &RenewableCertificate) -> bool {
        let config = self
            .config
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let max_age = chrono::Duration::days(config.max_certificate_age_days as i64);
        let now = Utc::now();

        certificate.generation_end + max_age < now
    }

    /// Check cache for verification result. Read-only; recovers from
    /// lock poisoning on both locks.
    fn check_cache(&self, id: &str) -> Option<VerificationStatus> {
        let cache = self
            .verification_cache
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let config = self
            .config
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner());

        if let Some((status, timestamp)) = cache.get(id) {
            // Check if cache entry is still valid
            let cache_expiration = chrono::Duration::hours(config.cache_expiration_hours as i64);
            let now = Utc::now();

            if *timestamp + cache_expiration > now {
                return Some(*status);
            }
        }

        None
    }

    /// Add result to cache. Best-effort; recovers from lock poisoning.
    fn add_to_cache(&self, id: String, status: VerificationStatus) {
        let mut cache = self
            .verification_cache
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        cache.insert(id, (status, Utc::now()));
    }

    /// Clear expired cache entries. Best-effort; recovers from lock
    /// poisoning on both locks.
    pub fn clear_expired_cache(&self) {
        let config = self
            .config
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let cache_expiration = chrono::Duration::hours(config.cache_expiration_hours as i64);
        let now = Utc::now();

        let mut cache = self
            .verification_cache
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        cache.retain(|_, (_, timestamp)| *timestamp + cache_expiration > now);
    }
}

/// Compare two energy/carbon amounts for equality, allowing only benign
/// floating-point rounding introduced by JSON round-tripping.
fn amounts_match(a: f64, b: f64) -> bool {
    if !a.is_finite() || !b.is_finite() {
        return false;
    }
    let diff = (a - b).abs();
    let scale = a.abs().max(b.abs()).max(1.0);
    diff / scale <= 1e-6
}

/// Compare a JSON timestamp field against an expected instant at
/// whole-second granularity. Absent, non-string, or unparseable values
/// never match (fail-closed).
fn json_time_matches(value: Option<&serde_json::Value>, expected: DateTime<Utc>) -> bool {
    match value.and_then(|v| v.as_str()) {
        Some(s) => match DateTime::parse_from_rfc3339(s) {
            Ok(dt) => dt.timestamp() == expected.timestamp(),
            Err(_) => false,
        },
        None => false,
    }
}

/// Cross-check that a registry response echoes the certificate's own claimed
/// fields. Querying only by `certificate_id` and trusting a bare `status`
/// lets a miner replay a real certificate ID with an inflated `amount_kwh`
/// (the field `renewable_validation` credits) and receive full green credit.
/// The registry's authoritative amount, issuer, and generation period must
/// all be echoed and match the miner's submission, or the certificate is not
/// treated as independently verified.
fn response_matches_certificate(
    result: &serde_json::Value,
    certificate: &RenewableCertificate,
) -> bool {
    let amount_ok = result
        .get("amount_kwh")
        .and_then(|v| v.as_f64())
        .map(|v| amounts_match(v, certificate.amount_kwh))
        .unwrap_or(false);

    let issuer_ok = result
        .get("issuer")
        .and_then(|v| v.as_str())
        .map(|v| v.trim().eq_ignore_ascii_case(certificate.issuer.trim()))
        .unwrap_or(false);

    let period_ok = json_time_matches(result.get("generation_start"), certificate.generation_start)
        && json_time_matches(result.get("generation_end"), certificate.generation_end);

    amount_ok && issuer_ok && period_ok
}

/// Cross-check that a registry response echoes the offset's own claimed
/// fields (see `response_matches_certificate`). The authoritative tonnage,
/// issuer, and period must be echoed and match the miner's submission.
fn response_matches_offset(result: &serde_json::Value, offset: &CarbonOffset) -> bool {
    let amount_ok = result
        .get("amount_tonnes")
        .and_then(|v| v.as_f64())
        .map(|v| amounts_match(v, offset.amount_tonnes))
        .unwrap_or(false);

    let issuer_ok = result
        .get("issuer")
        .and_then(|v| v.as_str())
        .map(|v| v.trim().eq_ignore_ascii_case(offset.issuer.trim()))
        .unwrap_or(false);

    let period_ok = json_time_matches(result.get("period_start"), offset.period_start)
        && match offset.period_end {
            Some(end) => json_time_matches(result.get("period_end"), end),
            None => true,
        };

    amount_ok && issuer_ok && period_ok
}

#[async_trait]
impl VerificationProvider for VerificationService {
    async fn verify_certificate(
        &self,
        certificate: &RenewableCertificate,
    ) -> Result<VerificationStatus, VerificationError> {
        // First check if certificate is expired
        if self.is_certificate_expired(certificate) {
            return Err(VerificationError::CertificateExpired(
                certificate.generation_end,
            ));
        }

        // Check cache
        if let Some(status) = self.check_cache(&certificate.certificate_id) {
            return Ok(status);
        }

        // Prepare data for the API request
        let api_url;
        let api_key;
        let _accept_self_reported;

        // Get config but drop it before the await
        {
            let config = self
                .config
                .read()
                .map_err(|_| VerificationError::LockPoisoned)?;

            // If no API endpoint is configured, use local verification;
            // otherwise extract the endpoint and fall through to the
            // remote path. `match` avoids the is_none/unwrap antipattern.
            api_url = match config.api_endpoint.as_ref() {
                Some(endpoint) => endpoint.clone(),
                None => {
                    // No external verification endpoint is configured, so the
                    // certificate has received zero independent verification.
                    // Never map self-reported acceptance to `Verified` — doing
                    // so would let unverified data flow into transparency
                    // `verified_mwh`/`verified_tonnes` and "verification
                    // percentage" figures as though it were independently
                    // verified. Self-reported claims stay `Pending` so the
                    // reporting layers can count them separately.
                    let status = VerificationStatus::Pending;
                    drop(config);
                    self.add_to_cache(certificate.certificate_id.clone(), status);
                    return Ok(status);
                }
            };
            api_key = config.api_key.clone();
            _accept_self_reported = config.accept_self_reported;
        }

        // Make API request for verification
        let url = format!(
            "{}/verify/certificate/{}",
            api_url, certificate.certificate_id
        );

        let request = self.client.get(&url);

        // Add API key if configured
        let request = if let Some(key) = &api_key {
            request.header("Authorization", format!("Bearer {}", key))
        } else {
            request
        };

        // Make request - note that config is no longer held
        match request.send().await {
            Ok(response) => {
                if response.status().is_success() {
                    // Parse response
                    let verification_result: Result<serde_json::Value, _> = response.json().await;

                    match verification_result {
                        Ok(result) => {
                            // Extract verification status
                            let status_str = result
                                .get("status")
                                .and_then(|s| s.as_str())
                                .unwrap_or("pending");

                            let status = match status_str.to_lowercase().as_str() {
                                "verified" => {
                                    // A bare "verified" for this ID is not enough: the
                                    // registry must also echo the certificate's own
                                    // amount/issuer/period so a real ID can't be
                                    // replayed with an inflated amount_kwh. On any
                                    // mismatch, deny green credit.
                                    if response_matches_certificate(&result, certificate) {
                                        VerificationStatus::Verified
                                    } else {
                                        VerificationStatus::Failed
                                    }
                                }
                                "failed" => VerificationStatus::Failed,
                                "expired" => VerificationStatus::Expired,
                                _ => VerificationStatus::Pending,
                            };

                            // Add to cache
                            self.add_to_cache(certificate.certificate_id.clone(), status);

                            Ok(status)
                        }
                        Err(e) => Err(VerificationError::ApiError(format!(
                            "Failed to parse response: {}",
                            e
                        ))),
                    }
                } else {
                    Err(VerificationError::ApiError(format!(
                        "API returned error: {}",
                        response.status()
                    )))
                }
            }
            Err(e) => Err(VerificationError::NetworkError(e.to_string())),
        }
    }

    async fn verify_offset(
        &self,
        offset: &CarbonOffset,
    ) -> Result<VerificationStatus, VerificationError> {
        // Check cache
        if let Some(status) = self.check_cache(&offset.offset_id) {
            return Ok(status);
        }

        // Prepare data for the API request
        let api_url;
        let api_key;
        let _accept_self_reported;

        // Get config but drop it before the await
        {
            let config = self
                .config
                .read()
                .map_err(|_| VerificationError::LockPoisoned)?;

            api_url = match config.api_endpoint.as_ref() {
                Some(endpoint) => endpoint.clone(),
                None => {
                    // No external verification endpoint is configured, so the
                    // offset has received zero independent verification. Never
                    // map self-reported acceptance to `Verified` — self-reported
                    // claims stay `Pending` so downstream reporting counts them
                    // separately rather than as independently verified tonnes.
                    let status = VerificationStatus::Pending;
                    drop(config);
                    self.add_to_cache(offset.offset_id.clone(), status);
                    return Ok(status);
                }
            };
            api_key = config.api_key.clone();
            _accept_self_reported = config.accept_self_reported;
        }

        // Make API request for verification
        let url = format!("{}/verify/offset/{}", api_url, offset.offset_id);

        let request = self.client.get(&url);

        // Add API key if configured
        let request = if let Some(key) = &api_key {
            request.header("Authorization", format!("Bearer {}", key))
        } else {
            request
        };

        // Make request - note that config is no longer held
        match request.send().await {
            Ok(response) => {
                if response.status().is_success() {
                    // Parse response
                    let verification_result: Result<serde_json::Value, _> = response.json().await;

                    match verification_result {
                        Ok(result) => {
                            // Extract verification status
                            let status_str = result
                                .get("status")
                                .and_then(|s| s.as_str())
                                .unwrap_or("pending");

                            let status = match status_str.to_lowercase().as_str() {
                                "verified" => {
                                    // As with certificates, require the registry to
                                    // echo the offset's own amount/issuer/period so a
                                    // real offset ID can't be replayed with inflated
                                    // tonnage. On any mismatch, deny credit.
                                    if response_matches_offset(&result, offset) {
                                        VerificationStatus::Verified
                                    } else {
                                        VerificationStatus::Failed
                                    }
                                }
                                "failed" => VerificationStatus::Failed,
                                "expired" => VerificationStatus::Expired,
                                _ => VerificationStatus::Pending,
                            };

                            // Add to cache
                            self.add_to_cache(offset.offset_id.clone(), status);

                            Ok(status)
                        }
                        Err(e) => Err(VerificationError::ApiError(format!(
                            "Failed to parse response: {}",
                            e
                        ))),
                    }
                } else {
                    Err(VerificationError::ApiError(format!(
                        "API returned error: {}",
                        response.status()
                    )))
                }
            }
            Err(e) => Err(VerificationError::NetworkError(e.to_string())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_certificate_expired() {
        let service = VerificationService::default();

        // Create an expired certificate
        let expired_cert = RenewableCertificate {
            certificate_id: "CERT-123".to_string(),
            issuer: "Test Issuer".to_string(),
            certificate_type: "Wind".to_string(),
            amount_kwh: 1000.0,
            generation_start: Utc::now() - chrono::Duration::days(800),
            generation_end: Utc::now() - chrono::Duration::days(730), // 2 years and 5 days ago
            location: Region::new("US"),
            verification_status: VerificationStatus::Pending,
            verification_url: None,
            metadata: std::collections::HashMap::new(),
        };

        assert!(service.is_certificate_expired(&expired_cert));

        // Create a valid certificate
        let valid_cert = RenewableCertificate {
            certificate_id: "CERT-456".to_string(),
            issuer: "Test Issuer".to_string(),
            certificate_type: "Solar".to_string(),
            amount_kwh: 1000.0,
            generation_start: Utc::now() - chrono::Duration::days(300),
            generation_end: Utc::now() - chrono::Duration::days(200),
            location: Region::new("US"),
            verification_status: VerificationStatus::Pending,
            verification_url: None,
            metadata: std::collections::HashMap::new(),
        };

        assert!(!service.is_certificate_expired(&valid_cert));
    }

    /// A self-reported certificate (no verification endpoint configured, even
    /// with `accept_self_reported = true`) must NOT be reported as `Verified`,
    /// since it received zero independent verification. It stays `Pending`.
    #[tokio::test]
    async fn test_self_reported_certificate_is_not_verified() {
        let config = VerificationConfig {
            api_endpoint: None,
            accept_self_reported: true,
            ..Default::default()
        };
        let service = VerificationService::new(config);

        let cert = RenewableCertificate {
            certificate_id: "CERT-SELF-1".to_string(),
            issuer: "Self Reporter".to_string(),
            certificate_type: "Solar".to_string(),
            amount_kwh: 1000.0,
            generation_start: Utc::now() - chrono::Duration::days(30),
            generation_end: Utc::now() - chrono::Duration::days(10),
            location: Region::new("US"),
            verification_status: VerificationStatus::Pending,
            verification_url: None,
            metadata: std::collections::HashMap::new(),
        };

        let status = service.verify_certificate(&cert).await.expect("verify");
        assert_eq!(
            status,
            VerificationStatus::Pending,
            "self-reported certificate must not be marked Verified"
        );
        assert_ne!(status, VerificationStatus::Verified);
    }

    /// Same guarantee for carbon offsets on the self-reported path.
    #[tokio::test]
    async fn test_self_reported_offset_is_not_verified() {
        let config = VerificationConfig {
            api_endpoint: None,
            accept_self_reported: true,
            ..Default::default()
        };
        let service = VerificationService::new(config);

        let offset = CarbonOffset {
            offset_id: "OFFSET-SELF-1".to_string(),
            issuer: "Self Reporter".to_string(),
            offset_type: "Reforestation".to_string(),
            amount_tonnes: 50.0,
            period_start: Utc::now() - chrono::Duration::days(30),
            period_end: Some(Utc::now() - chrono::Duration::days(10)),
            location: Region::new("US"),
            verification_status: VerificationStatus::Pending,
            verification_url: None,
            metadata: std::collections::HashMap::new(),
        };

        let status = service.verify_offset(&offset).await.expect("verify");
        assert_eq!(
            status,
            VerificationStatus::Pending,
            "self-reported offset must not be marked Verified"
        );
        assert_ne!(status, VerificationStatus::Verified);
    }

    fn sample_certificate() -> RenewableCertificate {
        RenewableCertificate {
            certificate_id: "CERT-XCHK".to_string(),
            issuer: "Green Registry".to_string(),
            certificate_type: "Solar".to_string(),
            amount_kwh: 1000.0,
            generation_start: DateTime::parse_from_rfc3339("2026-01-01T00:00:00Z")
                .unwrap()
                .with_timezone(&Utc),
            generation_end: DateTime::parse_from_rfc3339("2026-02-01T00:00:00Z")
                .unwrap()
                .with_timezone(&Utc),
            location: Region::new("US"),
            verification_status: VerificationStatus::Pending,
            verification_url: None,
            metadata: std::collections::HashMap::new(),
        }
    }

    /// A registry response that echoes the certificate's own fields is a match.
    #[test]
    fn test_response_matches_certificate_ok() {
        let cert = sample_certificate();
        let result = serde_json::json!({
            "status": "verified",
            "amount_kwh": 1000.0,
            "issuer": "green registry",
            "generation_start": "2026-01-01T00:00:00Z",
            "generation_end": "2026-02-01T00:00:00Z",
        });
        assert!(response_matches_certificate(&result, &cert));
    }

    /// Replaying a real certificate ID with an inflated amount_kwh must NOT
    /// match, even though the registry reports the ID as verified.
    #[test]
    fn test_inflated_amount_does_not_match() {
        let cert = sample_certificate();
        let result = serde_json::json!({
            "status": "verified",
            "amount_kwh": 1_000_000.0, // registry's real value is 1000
            "issuer": "Green Registry",
            "generation_start": "2026-01-01T00:00:00Z",
            "generation_end": "2026-02-01T00:00:00Z",
        });
        assert!(!response_matches_certificate(&result, &cert));
    }

    /// A "verified" response that omits the amount cannot be trusted, since the
    /// credited field is never confirmed.
    #[test]
    fn test_missing_amount_does_not_match() {
        let cert = sample_certificate();
        let result = serde_json::json!({
            "status": "verified",
            "issuer": "Green Registry",
            "generation_start": "2026-01-01T00:00:00Z",
            "generation_end": "2026-02-01T00:00:00Z",
        });
        assert!(!response_matches_certificate(&result, &cert));
    }

    /// A mismatched issuer or generation period must not match.
    #[test]
    fn test_wrong_issuer_or_period_does_not_match() {
        let cert = sample_certificate();
        let wrong_issuer = serde_json::json!({
            "amount_kwh": 1000.0,
            "issuer": "Impostor Registry",
            "generation_start": "2026-01-01T00:00:00Z",
            "generation_end": "2026-02-01T00:00:00Z",
        });
        assert!(!response_matches_certificate(&wrong_issuer, &cert));

        let wrong_period = serde_json::json!({
            "amount_kwh": 1000.0,
            "issuer": "Green Registry",
            "generation_start": "2025-01-01T00:00:00Z",
            "generation_end": "2026-02-01T00:00:00Z",
        });
        assert!(!response_matches_certificate(&wrong_period, &cert));
    }

    /// Offset cross-check: echoed fields match; inflated tonnage does not.
    #[test]
    fn test_response_matches_offset() {
        let offset = CarbonOffset {
            offset_id: "OFFSET-XCHK".to_string(),
            issuer: "Carbon Registry".to_string(),
            offset_type: "Reforestation".to_string(),
            amount_tonnes: 50.0,
            period_start: DateTime::parse_from_rfc3339("2026-01-01T00:00:00Z")
                .unwrap()
                .with_timezone(&Utc),
            period_end: Some(
                DateTime::parse_from_rfc3339("2026-02-01T00:00:00Z")
                    .unwrap()
                    .with_timezone(&Utc),
            ),
            location: Region::new("US"),
            verification_status: VerificationStatus::Pending,
            verification_url: None,
            metadata: std::collections::HashMap::new(),
        };

        let good = serde_json::json!({
            "amount_tonnes": 50.0,
            "issuer": "Carbon Registry",
            "period_start": "2026-01-01T00:00:00Z",
            "period_end": "2026-02-01T00:00:00Z",
        });
        assert!(response_matches_offset(&good, &offset));

        let inflated = serde_json::json!({
            "amount_tonnes": 5000.0,
            "issuer": "Carbon Registry",
            "period_start": "2026-01-01T00:00:00Z",
            "period_end": "2026-02-01T00:00:00Z",
        });
        assert!(!response_matches_offset(&inflated, &offset));
    }
}
