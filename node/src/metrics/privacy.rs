//! Metrics Privacy Framework
//!
//! SECURITY MODULE (P2-011): Privacy filtering for Prometheus metrics
//! 
//! This module provides privacy-preserving filters for exported metrics to prevent
//! information leakage about network topology, user activity, and node capabilities.

use std::net::IpAddr;
use sha2::{Digest, Sha256};

// ============================================================================
// Metrics Privacy Configuration
// ============================================================================

/// Metrics privacy level configuration
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MetricsPrivacyLevel {
    /// Full metrics including all details (development only)
    Full,
    /// Standard metrics with sensitive data filtered
    Standard,
    /// Minimal metrics, only aggregates (maximum privacy)
    Minimal,
}

/// Metrics privacy configuration
pub struct MetricsPrivacyConfig;

impl MetricsPrivacyConfig {
    /// Default privacy level for production
    pub const DEFAULT_PRIVACY_LEVEL: MetricsPrivacyLevel = MetricsPrivacyLevel::Standard;
    
    /// Whether to include IP addresses in metrics (default: false)
    pub const INCLUDE_IP_ADDRESSES: bool = false;
    
    /// Whether to include peer IDs in metrics (default: hashed only)
    pub const INCLUDE_PEER_IDS: bool = false;
    
    /// Whether to include transaction details (default: aggregates only)
    pub const INCLUDE_TRANSACTION_DETAILS: bool = false;
    
    /// Whether to include wallet information (default: never)
    pub const INCLUDE_WALLET_INFO: bool = false;
}

/// Metrics privacy filter
/// 
/// SECURITY: Provides sanitization functions for sensitive data in metrics.
pub struct MetricsPrivacyFilter {
    privacy_level: MetricsPrivacyLevel,
}

impl MetricsPrivacyFilter {
    /// Create a new privacy filter with specified level
    pub fn new(privacy_level: MetricsPrivacyLevel) -> Self {
        Self { privacy_level }
    }
    
    /// Create filter with standard privacy level
    pub fn standard() -> Self {
        Self::new(MetricsPrivacyLevel::Standard)
    }
    
    /// Sanitize IP address for metrics
    /// 
    /// SECURITY: Replaces IP with hash or aggregate identifier to prevent
    /// network topology mapping and targeted attacks.
    ///
    /// # Arguments
    /// * `ip` - IP address to sanitize
    ///
    /// # Returns
    /// Sanitized identifier (hash prefix or "redacted")
    pub fn sanitize_ip(&self, ip: &IpAddr) -> String {
        match self.privacy_level {
            MetricsPrivacyLevel::Full => {
                // Development only - include actual IP
                ip.to_string()
            }
            MetricsPrivacyLevel::Standard => {
                // Hash IP and use first 8 characters
                let hash = self.hash_sensitive_data(&ip.to_string());
                format!("ip_{}", &hash[..8])
            }
            MetricsPrivacyLevel::Minimal => {
                // No IP information at all
                "redacted".to_string()
            }
        }
    }
    
    /// Sanitize peer ID for metrics
    /// 
    /// # Arguments
    /// * `peer_id` - Peer identifier
    ///
    /// # Returns
    /// Sanitized peer identifier
    pub fn sanitize_peer_id(&self, peer_id: &str) -> String {
        match self.privacy_level {
            MetricsPrivacyLevel::Full => peer_id.to_string(),
            MetricsPrivacyLevel::Standard => {
                let hash = self.hash_sensitive_data(peer_id);
                format!("peer_{}", &hash[..12])
            }
            MetricsPrivacyLevel::Minimal => "redacted".to_string(),
        }
    }
    
    /// Sanitize address for metrics
    /// 
    /// # Arguments
    /// * `address` - Blockchain address
    ///
    /// # Returns
    /// Sanitized address identifier
    pub fn sanitize_address(&self, address: &str) -> String {
        match self.privacy_level {
            MetricsPrivacyLevel::Full => address.to_string(),
            MetricsPrivacyLevel::Standard | MetricsPrivacyLevel::Minimal => {
                // Never expose addresses in metrics (privacy critical)
                "redacted".to_string()
            }
        }
    }
    
    /// Sanitize transaction details for metrics
    /// 
    /// SECURITY: Prevents transaction correlation and user tracking.
    ///
    /// # Arguments
    /// * `tx_details` - Transaction information
    ///
    /// # Returns
    /// Aggregate-only information
    pub fn sanitize_transaction(&self, _tx_details: &str) -> String {
        match self.privacy_level {
            MetricsPrivacyLevel::Full => {
                // Development: Include tx hash
                _tx_details.to_string()
            }
            MetricsPrivacyLevel::Standard | MetricsPrivacyLevel::Minimal => {
                // Production: Only aggregates, no individual tx tracking
                "tx_aggregate".to_string()
            }
        }
    }
    
    /// Sanitize wallet balance for metrics
    /// 
    /// SECURITY: Never expose individual balances, only totals.
    ///
    /// # Arguments
    /// * `balance` - Wallet balance
    ///
    /// # Returns
    /// Filtered balance information
    pub fn sanitize_balance(&self, balance: u64) -> String {
        match self.privacy_level {
            MetricsPrivacyLevel::Full => {
                // Development: Show actual balance
                format!("{}", balance)
            }
            MetricsPrivacyLevel::Standard | MetricsPrivacyLevel::Minimal => {
                // Production: Never expose balances
                "redacted".to_string()
            }
        }
    }
    
    /// Check if metric should be exported based on privacy level
    /// 
    /// # Arguments
    /// * `metric_name` - Name of metric
    ///
    /// # Returns
    /// `true` if metric should be exported, `false` if too sensitive
    pub fn should_export_metric(&self, metric_name: &str) -> bool {
        // Sensitive metrics that should never be exported
        let sensitive_metrics = [
            "wallet_balance",
            "wallet_address",
            "transaction_amount",
            "peer_ip_address",
            "user_agent",
            "connection_ip",
        ];
        
        match self.privacy_level {
            MetricsPrivacyLevel::Full => true, // Export everything in dev
            MetricsPrivacyLevel::Standard => {
                // Filter out highly sensitive metrics
                !sensitive_metrics.iter().any(|s| metric_name.contains(s))
            }
            MetricsPrivacyLevel::Minimal => {
                // Only allow basic aggregate metrics
                metric_name.starts_with("total_")
                    || metric_name.starts_with("count_")
                    || metric_name.starts_with("avg_")
            }
        }
    }
    
    /// Hash sensitive data for pseudonymization
    /// 
    /// Uses SHA256 to create deterministic but unlinkable identifiers.
    fn hash_sensitive_data(&self, data: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(data.as_bytes());
        hasher.update(b"supernova_metrics_privacy_salt");
        let result = hasher.finalize();
        hex::encode(result)
    }
}

impl Default for MetricsPrivacyFilter {
    fn default() -> Self {
        Self::standard()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_ip_sanitization() {
        let filter = MetricsPrivacyFilter::standard();
        let ip: IpAddr = "192.168.1.1".parse().unwrap();
        
        let sanitized = filter.sanitize_ip(&ip);
        
        // Should not contain actual IP
        assert!(!sanitized.contains("192.168"));
        // Should be deterministic hash
        assert!(sanitized.starts_with("ip_"));
        
        println!("IP sanitized: {} → {}", ip, sanitized);
    }
    
    #[test]
    fn test_address_never_exposed() {
        let filter = MetricsPrivacyFilter::standard();
        let address = "nova1abcdef123456";
        
        let sanitized = filter.sanitize_address(address);
        
        assert_eq!(sanitized, "redacted");
        assert!(!sanitized.contains("nova1"));
        
        println!("Address sanitized: {} → {}", address, sanitized);
    }
    
    #[test]
    fn test_sensitive_metric_filtering() {
        let filter = MetricsPrivacyFilter::standard();
        
        assert!(!filter.should_export_metric("wallet_balance"));
        assert!(!filter.should_export_metric("peer_ip_address"));
        assert!(filter.should_export_metric("total_blocks"));
        assert!(filter.should_export_metric("count_transactions"));
        
        println!("Sensitive metrics properly filtered");
    }
}

