//! Eclipse Attack Prevention Metrics
//!
//! SECURITY MODULE (P1-008): Prometheus metrics for monitoring eclipse attack prevention.
//!
//! This module provides metrics to track:
//! - Peer diversity (subnet, ASN, geographic distribution)
//! - Connection ratios (inbound/outbound)
//! - Eclipse attack indicators
//! - Risk level assessment
//!
//! These metrics enable alerting when peer diversity drops below safe thresholds.

use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, RwLock};
use std::time::Instant;

/// Eclipse prevention metrics registry
pub struct EclipseMetrics {
    // Connection diversity metrics
    /// Number of unique /24 subnets (IPv4) or /48 subnets (IPv6)
    unique_subnets: AtomicUsize,
    /// Number of unique ASNs (Autonomous System Numbers)
    unique_asns: AtomicUsize,
    /// Number of unique geographic regions
    unique_regions: AtomicUsize,
    
    // Connection balance metrics
    /// Total outbound connections
    outbound_connections: AtomicUsize,
    /// Total inbound connections
    inbound_connections: AtomicUsize,
    /// Anchor (trusted) peer connections
    anchor_connections: AtomicUsize,
    
    // Attack indicator metrics
    /// Connection flooding events detected
    flooding_events: AtomicU64,
    /// Address convergence events (many peers advertising same addresses)
    convergence_events: AtomicU64,
    /// Diversity collapse events (sudden loss of peer diversity)
    diversity_collapse_events: AtomicU64,
    /// Coordinated behavior detections
    coordinated_behavior_events: AtomicU64,
    
    // Risk metrics
    /// Current eclipse risk level (0=Minimal, 1=Low, 2=Medium, 3=High, 4=Critical)
    risk_level: AtomicUsize,
    /// Diversity score (0.0-1.0 scaled to 0-100)
    diversity_score: AtomicUsize,
    
    // Peer rotation metrics
    /// Number of peer rotations performed
    rotations_performed: AtomicU64,
    /// Peers disconnected due to low behavior score
    behavior_disconnects: AtomicU64,
    /// Peers banned for malicious behavior
    peers_banned: AtomicU64,
    
    // PoW challenge metrics
    /// PoW challenges issued
    pow_challenges_issued: AtomicU64,
    /// PoW challenges completed successfully
    pow_challenges_completed: AtomicU64,
    /// PoW challenges failed/expired
    pow_challenges_failed: AtomicU64,
    
    // Subnet concentration metrics
    /// Maximum peers from any single subnet
    max_peers_per_subnet: AtomicUsize,
    /// Maximum peers from any single ASN
    max_peers_per_asn: AtomicUsize,
    
    /// Last metrics update timestamp
    last_update: RwLock<Instant>,
}

impl Default for EclipseMetrics {
    fn default() -> Self {
        Self::new()
    }
}

impl EclipseMetrics {
    /// Create a new eclipse metrics instance
    pub fn new() -> Self {
        Self {
            unique_subnets: AtomicUsize::new(0),
            unique_asns: AtomicUsize::new(0),
            unique_regions: AtomicUsize::new(0),
            outbound_connections: AtomicUsize::new(0),
            inbound_connections: AtomicUsize::new(0),
            anchor_connections: AtomicUsize::new(0),
            flooding_events: AtomicU64::new(0),
            convergence_events: AtomicU64::new(0),
            diversity_collapse_events: AtomicU64::new(0),
            coordinated_behavior_events: AtomicU64::new(0),
            risk_level: AtomicUsize::new(0),
            diversity_score: AtomicUsize::new(100),
            rotations_performed: AtomicU64::new(0),
            behavior_disconnects: AtomicU64::new(0),
            peers_banned: AtomicU64::new(0),
            pow_challenges_issued: AtomicU64::new(0),
            pow_challenges_completed: AtomicU64::new(0),
            pow_challenges_failed: AtomicU64::new(0),
            max_peers_per_subnet: AtomicUsize::new(0),
            max_peers_per_asn: AtomicUsize::new(0),
            last_update: RwLock::new(Instant::now()),
        }
    }

    // ========================================================================
    // Diversity Metric Updates
    // ========================================================================

    /// Update subnet diversity count
    pub fn set_unique_subnets(&self, count: usize) {
        self.unique_subnets.store(count, Ordering::SeqCst);
        self.update_timestamp();
    }

    /// Update ASN diversity count
    pub fn set_unique_asns(&self, count: usize) {
        self.unique_asns.store(count, Ordering::SeqCst);
        self.update_timestamp();
    }

    /// Update geographic region diversity
    pub fn set_unique_regions(&self, count: usize) {
        self.unique_regions.store(count, Ordering::SeqCst);
        self.update_timestamp();
    }

    /// Update connection counts
    pub fn set_connection_counts(&self, outbound: usize, inbound: usize, anchors: usize) {
        self.outbound_connections.store(outbound, Ordering::SeqCst);
        self.inbound_connections.store(inbound, Ordering::SeqCst);
        self.anchor_connections.store(anchors, Ordering::SeqCst);
        self.update_timestamp();
    }

    /// Update concentration metrics
    pub fn set_concentration(&self, max_per_subnet: usize, max_per_asn: usize) {
        self.max_peers_per_subnet.store(max_per_subnet, Ordering::SeqCst);
        self.max_peers_per_asn.store(max_per_asn, Ordering::SeqCst);
        self.update_timestamp();
    }

    // ========================================================================
    // Attack Indicator Recording
    // ========================================================================

    /// Record a connection flooding event
    pub fn record_flooding_event(&self) {
        self.flooding_events.fetch_add(1, Ordering::SeqCst);
    }

    /// Record an address convergence event
    pub fn record_convergence_event(&self) {
        self.convergence_events.fetch_add(1, Ordering::SeqCst);
    }

    /// Record a diversity collapse event
    pub fn record_diversity_collapse(&self) {
        self.diversity_collapse_events.fetch_add(1, Ordering::SeqCst);
    }

    /// Record coordinated behavior detection
    pub fn record_coordinated_behavior(&self) {
        self.coordinated_behavior_events.fetch_add(1, Ordering::SeqCst);
    }

    // ========================================================================
    // Risk Level Updates
    // ========================================================================

    /// Update eclipse risk level
    /// 0=Minimal, 1=Low, 2=Medium, 3=High, 4=Critical
    pub fn set_risk_level(&self, level: EclipseRiskLevel) {
        self.risk_level.store(level as usize, Ordering::SeqCst);
        self.update_timestamp();
    }

    /// Update diversity score (0.0 - 1.0)
    pub fn set_diversity_score(&self, score: f64) {
        // Scale to 0-100 for atomic storage
        let scaled = (score * 100.0).clamp(0.0, 100.0) as usize;
        self.diversity_score.store(scaled, Ordering::SeqCst);
        self.update_timestamp();
    }

    // ========================================================================
    // Peer Management Recording
    // ========================================================================

    /// Record a peer rotation
    pub fn record_rotation(&self) {
        self.rotations_performed.fetch_add(1, Ordering::SeqCst);
    }

    /// Record a behavior-based disconnect
    pub fn record_behavior_disconnect(&self) {
        self.behavior_disconnects.fetch_add(1, Ordering::SeqCst);
    }

    /// Record a peer ban
    pub fn record_peer_ban(&self) {
        self.peers_banned.fetch_add(1, Ordering::SeqCst);
    }

    // ========================================================================
    // PoW Challenge Recording
    // ========================================================================

    /// Record PoW challenge issued
    pub fn record_pow_challenge_issued(&self) {
        self.pow_challenges_issued.fetch_add(1, Ordering::SeqCst);
    }

    /// Record PoW challenge completed
    pub fn record_pow_challenge_completed(&self) {
        self.pow_challenges_completed.fetch_add(1, Ordering::SeqCst);
    }

    /// Record PoW challenge failed/expired
    pub fn record_pow_challenge_failed(&self) {
        self.pow_challenges_failed.fetch_add(1, Ordering::SeqCst);
    }

    // ========================================================================
    // Metric Getters
    // ========================================================================

    /// Get current eclipse metrics snapshot
    pub fn snapshot(&self) -> EclipseMetricsSnapshot {
        EclipseMetricsSnapshot {
            unique_subnets: self.unique_subnets.load(Ordering::SeqCst),
            unique_asns: self.unique_asns.load(Ordering::SeqCst),
            unique_regions: self.unique_regions.load(Ordering::SeqCst),
            outbound_connections: self.outbound_connections.load(Ordering::SeqCst),
            inbound_connections: self.inbound_connections.load(Ordering::SeqCst),
            anchor_connections: self.anchor_connections.load(Ordering::SeqCst),
            flooding_events: self.flooding_events.load(Ordering::SeqCst),
            convergence_events: self.convergence_events.load(Ordering::SeqCst),
            diversity_collapse_events: self.diversity_collapse_events.load(Ordering::SeqCst),
            coordinated_behavior_events: self.coordinated_behavior_events.load(Ordering::SeqCst),
            risk_level: self.risk_level.load(Ordering::SeqCst),
            diversity_score: self.diversity_score.load(Ordering::SeqCst) as f64 / 100.0,
            rotations_performed: self.rotations_performed.load(Ordering::SeqCst),
            behavior_disconnects: self.behavior_disconnects.load(Ordering::SeqCst),
            peers_banned: self.peers_banned.load(Ordering::SeqCst),
            pow_challenges_issued: self.pow_challenges_issued.load(Ordering::SeqCst),
            pow_challenges_completed: self.pow_challenges_completed.load(Ordering::SeqCst),
            pow_challenges_failed: self.pow_challenges_failed.load(Ordering::SeqCst),
            max_peers_per_subnet: self.max_peers_per_subnet.load(Ordering::SeqCst),
            max_peers_per_asn: self.max_peers_per_asn.load(Ordering::SeqCst),
        }
    }

    /// Update timestamp
    fn update_timestamp(&self) {
        if let Ok(mut ts) = self.last_update.write() {
            *ts = Instant::now();
        }
    }
}

/// Snapshot of eclipse metrics for export
#[derive(Debug, Clone)]
pub struct EclipseMetricsSnapshot {
    pub unique_subnets: usize,
    pub unique_asns: usize,
    pub unique_regions: usize,
    pub outbound_connections: usize,
    pub inbound_connections: usize,
    pub anchor_connections: usize,
    pub flooding_events: u64,
    pub convergence_events: u64,
    pub diversity_collapse_events: u64,
    pub coordinated_behavior_events: u64,
    pub risk_level: usize,
    pub diversity_score: f64,
    pub rotations_performed: u64,
    pub behavior_disconnects: u64,
    pub peers_banned: u64,
    pub pow_challenges_issued: u64,
    pub pow_challenges_completed: u64,
    pub pow_challenges_failed: u64,
    pub max_peers_per_subnet: usize,
    pub max_peers_per_asn: usize,
}

/// Eclipse risk levels
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(usize)]
pub enum EclipseRiskLevel {
    Minimal = 0,
    Low = 1,
    Medium = 2,
    High = 3,
    Critical = 4,
}

impl EclipseRiskLevel {
    pub fn as_str(&self) -> &'static str {
        match self {
            EclipseRiskLevel::Minimal => "minimal",
            EclipseRiskLevel::Low => "low",
            EclipseRiskLevel::Medium => "medium",
            EclipseRiskLevel::High => "high",
            EclipseRiskLevel::Critical => "critical",
        }
    }
}

// ============================================================================
// Prometheus Export
// ============================================================================

/// Export eclipse metrics in Prometheus format
pub fn export_prometheus_metrics(metrics: &EclipseMetrics) -> String {
    let snapshot = metrics.snapshot();
    
    let mut output = String::with_capacity(4096);
    
    // Diversity metrics
    output.push_str("# HELP supernova_eclipse_unique_subnets Number of unique peer subnets\n");
    output.push_str("# TYPE supernova_eclipse_unique_subnets gauge\n");
    output.push_str(&format!("supernova_eclipse_unique_subnets {}\n", snapshot.unique_subnets));
    
    output.push_str("# HELP supernova_eclipse_unique_asns Number of unique peer ASNs\n");
    output.push_str("# TYPE supernova_eclipse_unique_asns gauge\n");
    output.push_str(&format!("supernova_eclipse_unique_asns {}\n", snapshot.unique_asns));
    
    output.push_str("# HELP supernova_eclipse_unique_regions Number of unique geographic regions\n");
    output.push_str("# TYPE supernova_eclipse_unique_regions gauge\n");
    output.push_str(&format!("supernova_eclipse_unique_regions {}\n", snapshot.unique_regions));
    
    // Connection balance
    output.push_str("# HELP supernova_eclipse_outbound_connections Outbound peer connections\n");
    output.push_str("# TYPE supernova_eclipse_outbound_connections gauge\n");
    output.push_str(&format!("supernova_eclipse_outbound_connections {}\n", snapshot.outbound_connections));
    
    output.push_str("# HELP supernova_eclipse_inbound_connections Inbound peer connections\n");
    output.push_str("# TYPE supernova_eclipse_inbound_connections gauge\n");
    output.push_str(&format!("supernova_eclipse_inbound_connections {}\n", snapshot.inbound_connections));
    
    output.push_str("# HELP supernova_eclipse_anchor_connections Anchor (trusted) peer connections\n");
    output.push_str("# TYPE supernova_eclipse_anchor_connections gauge\n");
    output.push_str(&format!("supernova_eclipse_anchor_connections {}\n", snapshot.anchor_connections));
    
    // Concentration metrics
    output.push_str("# HELP supernova_eclipse_max_peers_per_subnet Maximum peers from single subnet\n");
    output.push_str("# TYPE supernova_eclipse_max_peers_per_subnet gauge\n");
    output.push_str(&format!("supernova_eclipse_max_peers_per_subnet {}\n", snapshot.max_peers_per_subnet));
    
    output.push_str("# HELP supernova_eclipse_max_peers_per_asn Maximum peers from single ASN\n");
    output.push_str("# TYPE supernova_eclipse_max_peers_per_asn gauge\n");
    output.push_str(&format!("supernova_eclipse_max_peers_per_asn {}\n", snapshot.max_peers_per_asn));
    
    // Risk metrics
    output.push_str("# HELP supernova_eclipse_risk_level Current eclipse attack risk level (0-4)\n");
    output.push_str("# TYPE supernova_eclipse_risk_level gauge\n");
    output.push_str(&format!("supernova_eclipse_risk_level {}\n", snapshot.risk_level));
    
    output.push_str("# HELP supernova_eclipse_diversity_score Network diversity score (0.0-1.0)\n");
    output.push_str("# TYPE supernova_eclipse_diversity_score gauge\n");
    output.push_str(&format!("supernova_eclipse_diversity_score {:.4}\n", snapshot.diversity_score));
    
    // Attack indicators (counters)
    output.push_str("# HELP supernova_eclipse_flooding_events_total Connection flooding events detected\n");
    output.push_str("# TYPE supernova_eclipse_flooding_events_total counter\n");
    output.push_str(&format!("supernova_eclipse_flooding_events_total {}\n", snapshot.flooding_events));
    
    output.push_str("# HELP supernova_eclipse_convergence_events_total Address convergence events detected\n");
    output.push_str("# TYPE supernova_eclipse_convergence_events_total counter\n");
    output.push_str(&format!("supernova_eclipse_convergence_events_total {}\n", snapshot.convergence_events));
    
    output.push_str("# HELP supernova_eclipse_diversity_collapse_events_total Diversity collapse events\n");
    output.push_str("# TYPE supernova_eclipse_diversity_collapse_events_total counter\n");
    output.push_str(&format!("supernova_eclipse_diversity_collapse_events_total {}\n", snapshot.diversity_collapse_events));
    
    output.push_str("# HELP supernova_eclipse_coordinated_behavior_events_total Coordinated behavior detections\n");
    output.push_str("# TYPE supernova_eclipse_coordinated_behavior_events_total counter\n");
    output.push_str(&format!("supernova_eclipse_coordinated_behavior_events_total {}\n", snapshot.coordinated_behavior_events));
    
    // Peer management
    output.push_str("# HELP supernova_eclipse_rotations_total Peer rotations performed\n");
    output.push_str("# TYPE supernova_eclipse_rotations_total counter\n");
    output.push_str(&format!("supernova_eclipse_rotations_total {}\n", snapshot.rotations_performed));
    
    output.push_str("# HELP supernova_eclipse_behavior_disconnects_total Behavior-based peer disconnects\n");
    output.push_str("# TYPE supernova_eclipse_behavior_disconnects_total counter\n");
    output.push_str(&format!("supernova_eclipse_behavior_disconnects_total {}\n", snapshot.behavior_disconnects));
    
    output.push_str("# HELP supernova_eclipse_peers_banned_total Peers banned for malicious behavior\n");
    output.push_str("# TYPE supernova_eclipse_peers_banned_total counter\n");
    output.push_str(&format!("supernova_eclipse_peers_banned_total {}\n", snapshot.peers_banned));
    
    // PoW challenges
    output.push_str("# HELP supernova_eclipse_pow_challenges_issued_total PoW challenges issued\n");
    output.push_str("# TYPE supernova_eclipse_pow_challenges_issued_total counter\n");
    output.push_str(&format!("supernova_eclipse_pow_challenges_issued_total {}\n", snapshot.pow_challenges_issued));
    
    output.push_str("# HELP supernova_eclipse_pow_challenges_completed_total PoW challenges completed\n");
    output.push_str("# TYPE supernova_eclipse_pow_challenges_completed_total counter\n");
    output.push_str(&format!("supernova_eclipse_pow_challenges_completed_total {}\n", snapshot.pow_challenges_completed));
    
    output.push_str("# HELP supernova_eclipse_pow_challenges_failed_total PoW challenges failed/expired\n");
    output.push_str("# TYPE supernova_eclipse_pow_challenges_failed_total counter\n");
    output.push_str(&format!("supernova_eclipse_pow_challenges_failed_total {}\n", snapshot.pow_challenges_failed));
    
    output
}

// ============================================================================
// Global Instance
// ============================================================================

use std::sync::OnceLock;

static GLOBAL_ECLIPSE_METRICS: OnceLock<Arc<EclipseMetrics>> = OnceLock::new();

/// Get or initialize the global eclipse metrics instance
pub fn global_eclipse_metrics() -> &'static Arc<EclipseMetrics> {
    GLOBAL_ECLIPSE_METRICS.get_or_init(|| Arc::new(EclipseMetrics::new()))
}

// ============================================================================
// Convenience Functions
// ============================================================================

/// Update diversity metrics from peer manager
pub fn update_diversity(unique_subnets: usize, unique_asns: usize, unique_regions: usize) {
    let metrics = global_eclipse_metrics();
    metrics.set_unique_subnets(unique_subnets);
    metrics.set_unique_asns(unique_asns);
    metrics.set_unique_regions(unique_regions);
}

/// Update connection balance metrics
pub fn update_connection_balance(outbound: usize, inbound: usize, anchors: usize) {
    let metrics = global_eclipse_metrics();
    metrics.set_connection_counts(outbound, inbound, anchors);
}

/// Update risk level
pub fn update_risk_level(level: EclipseRiskLevel) {
    let metrics = global_eclipse_metrics();
    metrics.set_risk_level(level);
}

/// Record an attack indicator
pub fn record_attack_indicator(indicator: AttackIndicator) {
    let metrics = global_eclipse_metrics();
    match indicator {
        AttackIndicator::ConnectionFlooding => metrics.record_flooding_event(),
        AttackIndicator::AddressConvergence => metrics.record_convergence_event(),
        AttackIndicator::DiversityCollapse => metrics.record_diversity_collapse(),
        AttackIndicator::CoordinatedBehavior => metrics.record_coordinated_behavior(),
    }
}

/// Attack indicator types for recording
#[derive(Debug, Clone, Copy)]
pub enum AttackIndicator {
    ConnectionFlooding,
    AddressConvergence,
    DiversityCollapse,
    CoordinatedBehavior,
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_eclipse_metrics_creation() {
        let metrics = EclipseMetrics::new();
        let snapshot = metrics.snapshot();
        
        assert_eq!(snapshot.unique_subnets, 0);
        assert_eq!(snapshot.risk_level, 0);
        assert_eq!(snapshot.diversity_score, 1.0);
    }

    #[test]
    fn test_diversity_updates() {
        let metrics = EclipseMetrics::new();
        
        metrics.set_unique_subnets(10);
        metrics.set_unique_asns(8);
        metrics.set_unique_regions(5);
        
        let snapshot = metrics.snapshot();
        assert_eq!(snapshot.unique_subnets, 10);
        assert_eq!(snapshot.unique_asns, 8);
        assert_eq!(snapshot.unique_regions, 5);
    }

    #[test]
    fn test_connection_counts() {
        let metrics = EclipseMetrics::new();
        
        metrics.set_connection_counts(8, 16, 4);
        
        let snapshot = metrics.snapshot();
        assert_eq!(snapshot.outbound_connections, 8);
        assert_eq!(snapshot.inbound_connections, 16);
        assert_eq!(snapshot.anchor_connections, 4);
    }

    #[test]
    fn test_attack_indicators() {
        let metrics = EclipseMetrics::new();
        
        metrics.record_flooding_event();
        metrics.record_flooding_event();
        metrics.record_convergence_event();
        
        let snapshot = metrics.snapshot();
        assert_eq!(snapshot.flooding_events, 2);
        assert_eq!(snapshot.convergence_events, 1);
    }

    #[test]
    fn test_risk_level() {
        let metrics = EclipseMetrics::new();
        
        metrics.set_risk_level(EclipseRiskLevel::High);
        
        let snapshot = metrics.snapshot();
        assert_eq!(snapshot.risk_level, 3);
    }

    #[test]
    fn test_diversity_score() {
        let metrics = EclipseMetrics::new();
        
        metrics.set_diversity_score(0.75);
        
        let snapshot = metrics.snapshot();
        assert!((snapshot.diversity_score - 0.75).abs() < 0.01);
    }

    #[test]
    fn test_prometheus_export() {
        let metrics = EclipseMetrics::new();
        
        metrics.set_unique_subnets(10);
        metrics.set_connection_counts(8, 12, 4);
        metrics.set_risk_level(EclipseRiskLevel::Low);
        metrics.record_flooding_event();
        
        let output = export_prometheus_metrics(&metrics);
        
        assert!(output.contains("supernova_eclipse_unique_subnets 10"));
        assert!(output.contains("supernova_eclipse_outbound_connections 8"));
        assert!(output.contains("supernova_eclipse_risk_level 1"));
        assert!(output.contains("supernova_eclipse_flooding_events_total 1"));
    }

    #[test]
    fn test_global_instance() {
        let metrics1 = global_eclipse_metrics();
        let metrics2 = global_eclipse_metrics();
        
        // Should be same instance
        assert!(Arc::ptr_eq(metrics1, metrics2));
    }
}

