use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use reqwest::Client;
use std::sync::{Arc, RwLock};
use crate::environmental::types::Region;
use crate::environmental::emissions::VerificationStatus;
use async_trait::async_trait;

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
    async fn verify_certificate(&self, certificate: &RenewableCertificate) -> Result<VerificationStatus, VerificationError>;
    
    /// Verify a carbon offset
    async fn verify_offset(&self, offset: &CarbonOffset) -> Result<VerificationStatus, VerificationError>;
}

/// Verification service for environmental assets
pub struct VerificationService {
    /// Configuration for the verification service
    config: Arc<RwLock<VerificationConfig>>,
    /// HTTP client for API requests
    client: Client,
    /// Cache of verification results
    verification_cache: Arc<RwLock<std::collections::HashMap<String, (VerificationStatus, DateTime<Utc>)>>>,
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
    
    /// Update the service configuration
    pub fn update_config(&self, config: VerificationConfig) {
        let mut current_config = self.config.write().unwrap();
        *current_config = config;
    }
    
    /// Check if a certificate is expired
    pub fn is_certificate_expired(&self, certificate: &RenewableCertificate) -> bool {
        let config = self.config.read().unwrap();
        let max_age = chrono::Duration::days(config.max_certificate_age_days as i64);
        let now = Utc::now();
        
        certificate.generation_end + max_age < now
    }
    
    /// Check cache for verification result
    fn check_cache(&self, id: &str) -> Option<VerificationStatus> {
        let cache = self.verification_cache.read().unwrap();
        let config = self.config.read().unwrap();
        
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
    
    /// Add result to cache
    fn add_to_cache(&self, id: String, status: VerificationStatus) {
        let mut cache = self.verification_cache.write().unwrap();
        cache.insert(id, (status, Utc::now()));
    }
    
    /// Clear expired cache entries
    pub fn clear_expired_cache(&self) {
        let config = self.config.read().unwrap();
        let cache_expiration = chrono::Duration::hours(config.cache_expiration_hours as i64);
        let now = Utc::now();
        
        let mut cache = self.verification_cache.write().unwrap();
        cache.retain(|_, (_, timestamp)| *timestamp + cache_expiration > now);
    }
}

#[async_trait]
impl VerificationProvider for VerificationService {
    async fn verify_certificate(&self, certificate: &RenewableCertificate) -> Result<VerificationStatus, VerificationError> {
        // First check if certificate is expired
        if self.is_certificate_expired(certificate) {
            return Err(VerificationError::CertificateExpired(certificate.generation_end));
        }
        
        // Check cache
        if let Some(status) = self.check_cache(&certificate.certificate_id) {
            return Ok(status);
        }
        
        // Prepare data for the API request
        let api_url;
        let api_key;
        let accept_self_reported;
        
        // Get config but drop it before the await
        {
            let config = self.config.read().unwrap();
            
            // If no API endpoint is configured, use local verification
            if config.api_endpoint.is_none() {
                let status = if config.accept_self_reported {
                    VerificationStatus::Verified
                } else {
                    VerificationStatus::Pending
                };
                
                // Add to cache
                self.add_to_cache(certificate.certificate_id.clone(), status);
                
                return Ok(status);
            }
            
            // Copy necessary data from config
            api_url = config.api_endpoint.as_ref().unwrap().clone();
            api_key = config.api_key.clone();
            accept_self_reported = config.accept_self_reported;
        }
        
        // Make API request for verification
        let url = format!("{}/verify/certificate/{}", api_url, certificate.certificate_id);
        
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
                            let status_str = result.get("status")
                                .and_then(|s| s.as_str())
                                .unwrap_or("pending");
                                
                            let status = match status_str.to_lowercase().as_str() {
                                "verified" => VerificationStatus::Verified,
                                "failed" => VerificationStatus::Failed,
                                "expired" => VerificationStatus::Expired,
                                _ => VerificationStatus::Pending,
                            };
                            
                            // Add to cache
                            self.add_to_cache(certificate.certificate_id.clone(), status);
                            
                            Ok(status)
                        },
                        Err(e) => Err(VerificationError::ApiError(format!("Failed to parse response: {}", e)))
                    }
                } else {
                    Err(VerificationError::ApiError(format!("API returned error: {}", response.status())))
                }
            },
            Err(e) => Err(VerificationError::NetworkError(e.to_string()))
        }
    }
    
    async fn verify_offset(&self, offset: &CarbonOffset) -> Result<VerificationStatus, VerificationError> {
        // Check cache
        if let Some(status) = self.check_cache(&offset.offset_id) {
            return Ok(status);
        }
        
        // Prepare data for the API request
        let api_url;
        let api_key;
        let accept_self_reported;
        
        // Get config but drop it before the await
        {
            let config = self.config.read().unwrap();
            
            // If no API endpoint is configured, use local verification
            if config.api_endpoint.is_none() {
                let status = if config.accept_self_reported {
                    VerificationStatus::Verified
                } else {
                    VerificationStatus::Pending
                };
                
                // Add to cache
                self.add_to_cache(offset.offset_id.clone(), status);
                
                return Ok(status);
            }
            
            // Copy necessary data from config
            api_url = config.api_endpoint.as_ref().unwrap().clone();
            api_key = config.api_key.clone();
            accept_self_reported = config.accept_self_reported;
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
                            let status_str = result.get("status")
                                .and_then(|s| s.as_str())
                                .unwrap_or("pending");
                                
                            let status = match status_str.to_lowercase().as_str() {
                                "verified" => VerificationStatus::Verified,
                                "failed" => VerificationStatus::Failed,
                                "expired" => VerificationStatus::Expired,
                                _ => VerificationStatus::Pending,
                            };
                            
                            // Add to cache
                            self.add_to_cache(offset.offset_id.clone(), status);
                            
                            Ok(status)
                        },
                        Err(e) => Err(VerificationError::ApiError(format!("Failed to parse response: {}", e)))
                    }
                } else {
                    Err(VerificationError::ApiError(format!("API returned error: {}", response.status())))
                }
            },
            Err(e) => Err(VerificationError::NetworkError(e.to_string()))
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
} 