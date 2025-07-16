//! Fraud Detection Module for Environmental Claims
//! 
//! This module implements statistical anomaly detection to identify
//! potentially fraudulent environmental claims.

use super::{EnvironmentalProfile, RECCertificate, EfficiencyAudit};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use serde::{Serialize, Deserialize};

/// Configuration for fraud detection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FraudDetectionConfig {
    /// Enable anomaly detection
    pub enabled: bool,
    
    /// Z-score threshold for anomaly detection
    pub anomaly_threshold: f64,
    
    /// Minimum samples before detection activates
    pub min_samples: usize,
    
    /// Time window for analysis (seconds)
    pub analysis_window: u64,
    
    /// Enable pattern matching
    pub enable_pattern_matching: bool,
}

impl Default for FraudDetectionConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            anomaly_threshold: 3.0, // 3 standard deviations
            min_samples: 100,
            analysis_window: 86400, // 24 hours
            enable_pattern_matching: true,
        }
    }
}

/// Historical data point for analysis
#[derive(Debug, Clone)]
struct ClaimDataPoint {
    miner_id: String,
    timestamp: u64,
    renewable_percentage: f64,
    efficiency_score: f64,
    certificate_count: usize,
    total_mwh: f64,
}

/// Statistical metrics for anomaly detection
#[derive(Debug, Clone, Default)]
struct ClaimStatistics {
    count: usize,
    sum_renewable: f64,
    sum_renewable_sq: f64,
    sum_efficiency: f64,
    sum_efficiency_sq: f64,
    sum_mwh: f64,
    sum_mwh_sq: f64,
}

impl ClaimStatistics {
    fn update(&mut self, point: &ClaimDataPoint) {
        self.count += 1;
        self.sum_renewable += point.renewable_percentage;
        self.sum_renewable_sq += point.renewable_percentage * point.renewable_percentage;
        self.sum_efficiency += point.efficiency_score;
        self.sum_efficiency_sq += point.efficiency_score * point.efficiency_score;
        self.sum_mwh += point.total_mwh;
        self.sum_mwh_sq += point.total_mwh * point.total_mwh;
    }
    
    fn mean_renewable(&self) -> f64 {
        if self.count == 0 { 0.0 } else { self.sum_renewable / self.count as f64 }
    }
    
    fn std_dev_renewable(&self) -> f64 {
        if self.count < 2 { return 0.0; }
        let mean = self.mean_renewable();
        let variance = (self.sum_renewable_sq / self.count as f64) - (mean * mean);
        variance.max(0.0).sqrt()
    }
    
    fn mean_efficiency(&self) -> f64 {
        if self.count == 0 { 0.0 } else { self.sum_efficiency / self.count as f64 }
    }
    
    fn std_dev_efficiency(&self) -> f64 {
        if self.count < 2 { return 0.0; }
        let mean = self.mean_efficiency();
        let variance = (self.sum_efficiency_sq / self.count as f64) - (mean * mean);
        variance.max(0.0).sqrt()
    }
}

/// Fraud detection system
pub struct FraudDetector {
    config: FraudDetectionConfig,
    historical_data: Arc<RwLock<Vec<ClaimDataPoint>>>,
    statistics: Arc<RwLock<ClaimStatistics>>,
    suspicious_patterns: Arc<RwLock<HashMap<String, Vec<SuspiciousActivity>>>>,
}

/// Types of suspicious activities detected
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SuspiciousActivity {
    /// Renewable percentage anomaly
    RenewableAnomaly { z_score: f64, value: f64, mean: f64, std_dev: f64 },
    
    /// Efficiency score anomaly
    EfficiencyAnomaly { z_score: f64, value: f64, mean: f64, std_dev: f64 },
    
    /// Certificate volume anomaly
    VolumeAnomaly { z_score: f64, value: f64, mean: f64, std_dev: f64 },
    
    /// Rapid claim submissions
    RapidSubmissions { count: usize, time_window: u64 },
    
    /// Duplicate certificate patterns
    DuplicatePattern { certificate_ids: Vec<String> },
    
    /// Impossible efficiency claims
    ImpossibleEfficiency { claimed: f64, theoretical_max: f64 },
}

impl FraudDetector {
    pub fn new(config: FraudDetectionConfig) -> Self {
        Self {
            config,
            historical_data: Arc::new(RwLock::new(Vec::new())),
            statistics: Arc::new(RwLock::new(ClaimStatistics::default())),
            suspicious_patterns: Arc::new(RwLock::new(HashMap::new())),
        }
    }
    
    /// Analyze an environmental claim for fraud
    pub async fn analyze_claim(
        &self,
        miner_id: &str,
        profile: &EnvironmentalProfile,
        certificates: &[RECCertificate],
        audit: Option<&EfficiencyAudit>,
    ) -> Vec<SuspiciousActivity> {
        if !self.config.enabled {
            return Vec::new();
        }
        
        let mut suspicious_activities = Vec::new();
        
        // Create data point
        let total_mwh: f64 = certificates.iter().map(|c| c.coverage_mwh).sum();
        let data_point = ClaimDataPoint {
            miner_id: miner_id.to_string(),
            timestamp: current_timestamp(),
            renewable_percentage: profile.renewable_percentage,
            efficiency_score: profile.efficiency_score,
            certificate_count: certificates.len(),
            total_mwh,
        };
        
        // Update statistics
        let mut stats = self.statistics.write().await;
        stats.update(&data_point);
        
        // Perform anomaly detection if we have enough samples
        if stats.count >= self.config.min_samples {
            // Check renewable percentage anomaly
            if let Some(anomaly) = self.check_renewable_anomaly(&data_point, &stats) {
                suspicious_activities.push(anomaly);
            }
            
            // Check efficiency anomaly
            if let Some(anomaly) = self.check_efficiency_anomaly(&data_point, &stats) {
                suspicious_activities.push(anomaly);
            }
        }
        drop(stats);
        
        // Check for impossible efficiency claims
        if let Some(audit) = audit {
            if let Some(anomaly) = self.check_impossible_efficiency(audit) {
                suspicious_activities.push(anomaly);
            }
        }
        
        // Pattern detection
        if self.config.enable_pattern_matching {
            let patterns = self.detect_patterns(miner_id, &data_point).await;
            suspicious_activities.extend(patterns);
        }
        
        // Store historical data
        let mut history = self.historical_data.write().await;
        history.push(data_point);
        
        // Clean old data
        let cutoff = current_timestamp() - self.config.analysis_window;
        history.retain(|point| point.timestamp > cutoff);
        
        // Record suspicious activities
        if !suspicious_activities.is_empty() {
            let mut patterns = self.suspicious_patterns.write().await;
            patterns.entry(miner_id.to_string())
                .or_insert_with(Vec::new)
                .extend(suspicious_activities.clone());
        }
        
        suspicious_activities
    }
    
    fn check_renewable_anomaly(
        &self,
        point: &ClaimDataPoint,
        stats: &ClaimStatistics,
    ) -> Option<SuspiciousActivity> {
        let mean = stats.mean_renewable();
        let std_dev = stats.std_dev_renewable();
        
        if std_dev > 0.0 {
            let z_score = (point.renewable_percentage - mean).abs() / std_dev;
            if z_score > self.config.anomaly_threshold {
                return Some(SuspiciousActivity::RenewableAnomaly {
                    z_score,
                    value: point.renewable_percentage,
                    mean,
                    std_dev,
                });
            }
        }
        None
    }
    
    fn check_efficiency_anomaly(
        &self,
        point: &ClaimDataPoint,
        stats: &ClaimStatistics,
    ) -> Option<SuspiciousActivity> {
        let mean = stats.mean_efficiency();
        let std_dev = stats.std_dev_efficiency();
        
        if std_dev > 0.0 {
            let z_score = (point.efficiency_score - mean).abs() / std_dev;
            if z_score > self.config.anomaly_threshold {
                return Some(SuspiciousActivity::EfficiencyAnomaly {
                    z_score,
                    value: point.efficiency_score,
                    mean,
                    std_dev,
                });
            }
        }
        None
    }
    
    fn check_impossible_efficiency(&self, audit: &EfficiencyAudit) -> Option<SuspiciousActivity> {
        // Theoretical maximum efficiency for current technology
        let theoretical_max = match audit.hash_rate_per_watt {
            x if x > 200.0 => return Some(SuspiciousActivity::ImpossibleEfficiency {
                claimed: x,
                theoretical_max: 200.0,
            }),
            _ => return None,
        };
    }
    
    async fn detect_patterns(
        &self,
        miner_id: &str,
        _point: &ClaimDataPoint,
    ) -> Vec<SuspiciousActivity> {
        let mut patterns = Vec::new();
        let history = self.historical_data.read().await;
        
        // Check for rapid submissions
        let recent_claims: Vec<_> = history.iter()
            .filter(|p| p.miner_id == miner_id)
            .filter(|p| p.timestamp > current_timestamp() - 3600) // Last hour
            .collect();
        
        if recent_claims.len() > 10 {
            patterns.push(SuspiciousActivity::RapidSubmissions {
                count: recent_claims.len(),
                time_window: 3600,
            });
        }
        
        patterns
    }
    
    /// Get fraud risk score for a miner (0.0 - 1.0)
    pub async fn get_risk_score(&self, miner_id: &str) -> f64 {
        let patterns = self.suspicious_patterns.read().await;
        
        if let Some(activities) = patterns.get(miner_id) {
            // Simple scoring: more activities = higher risk
            let score = (activities.len() as f64 / 10.0).min(1.0);
            score
        } else {
            0.0
        }
    }
    
    /// Get detailed fraud report for a miner
    pub async fn get_fraud_report(&self, miner_id: &str) -> Option<FraudReport> {
        let patterns = self.suspicious_patterns.read().await;
        
        patterns.get(miner_id).map(|activities| {
            FraudReport {
                miner_id: miner_id.to_string(),
                risk_score: (activities.len() as f64 / 10.0).min(1.0),
                suspicious_activities: activities.clone(),
                timestamp: current_timestamp(),
            }
        })
    }
}

/// Fraud detection report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FraudReport {
    pub miner_id: String,
    pub risk_score: f64,
    pub suspicious_activities: Vec<SuspiciousActivity>,
    pub timestamp: u64,
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
    async fn test_anomaly_detection() {
        let config = FraudDetectionConfig::default();
        let detector = FraudDetector::new(config);
        
        // Add normal claims to build baseline
        for i in 0..100 {
            let profile = EnvironmentalProfile {
                renewable_percentage: 0.5 + (i as f64 % 10.0) * 0.01,
                efficiency_score: 0.7,
                verified: true,
                rec_coverage: 0.5,
            };
            
            detector.analyze_claim(
                &format!("miner_{}", i),
                &profile,
                &[],
                None,
            ).await;
        }
        
        // Submit anomalous claim
        let anomaly_profile = EnvironmentalProfile {
            renewable_percentage: 0.99, // Very high
            efficiency_score: 0.95,    // Very high
            verified: true,
            rec_coverage: 0.99,
        };
        
        let activities = detector.analyze_claim(
            "suspicious_miner",
            &anomaly_profile,
            &[],
            None,
        ).await;
        
        // Should detect anomalies
        assert!(!activities.is_empty());
        assert!(activities.iter().any(|a| matches!(a, SuspiciousActivity::RenewableAnomaly { .. })));
    }
} 