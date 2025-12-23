//! Quantum Canary - Early Warning System for Quantum Attacks
//!
//! This module implements a "canary in the coal mine" approach to detecting
//! quantum computer attacks before they can compromise the main system.
//!
//! The canary uses intentionally weakened quantum-resistant signatures that
//! would be broken first by an emerging quantum computer, giving us time
//! to activate emergency protocols.

use crate::crypto::quantum::{
    sign_quantum, verify_quantum_signature, QuantumKeyPair, QuantumParameters, QuantumScheme,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::{Duration, SystemTime};

/// Activity metrics for suspicious detection
#[derive(Debug, Clone)]
pub struct CanaryActivityMetrics {
    /// Verification attempts per canary in current window
    pub verification_attempts: HashMap<CanaryId, Vec<SystemTime>>,
    /// Failed verification attempts
    pub failed_attempts: HashMap<CanaryId, u32>,
    /// Unusual timing patterns detected
    pub timing_anomalies: HashMap<CanaryId, u32>,
    /// Network scan attempts detected
    pub scan_attempts: u32,
    /// Last window reset time
    pub last_reset: SystemTime,
}

impl Default for CanaryActivityMetrics {
    fn default() -> Self {
        Self {
            verification_attempts: HashMap::new(),
            failed_attempts: HashMap::new(),
            timing_anomalies: HashMap::new(),
            scan_attempts: 0,
            last_reset: SystemTime::now(),
        }
    }
}

/// On-chain canary state tracker
#[derive(Debug, Clone)]
pub struct OnChainCanaryState {
    /// UTXO status (spent = compromised)
    pub spent: bool,
    /// Block height when last checked
    pub last_checked_height: u64,
    /// Transaction that spent the UTXO (if compromised)
    pub spending_tx: Option<[u8; 32]>,
}

/// Quantum Canary System
#[derive(Debug, Clone)]
pub struct QuantumCanarySystem {
    /// Active canaries
    canaries: Arc<RwLock<HashMap<CanaryId, QuantumCanary>>>,

    /// Canary monitoring results
    monitoring_results: Arc<RwLock<Vec<MonitoringResult>>>,

    /// Emergency contacts for alerts
    alert_endpoints: Vec<String>,

    /// System configuration
    config: CanaryConfig,

    /// Activity metrics for detection
    activity_metrics: Arc<RwLock<CanaryActivityMetrics>>,

    /// On-chain state for deployed canaries
    on_chain_states: Arc<RwLock<HashMap<[u8; 32], OnChainCanaryState>>>,

    /// Flag indicating emergency migration has been triggered
    emergency_triggered: Arc<RwLock<bool>>,
}

/// Individual Quantum Canary
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuantumCanary {
    /// Unique canary identifier
    pub id: CanaryId,

    /// Intentionally weak quantum keys
    pub weak_keys: QuantumKeyPair,

    /// Canary value (bounty for breaking it)
    pub bounty_value: u64,

    /// Deployment timestamp
    pub deployed_at: SystemTime,

    /// Last verification timestamp
    pub last_verified: SystemTime,

    /// Compromise detection
    pub compromise_detected: bool,

    /// Canary transaction on blockchain
    pub canary_tx_id: Option<[u8; 32]>,

    /// Security level (intentionally low)
    pub security_level: u8,
}

/// Canary Configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanaryConfig {
    /// Check interval
    pub check_interval: Duration,

    /// Canary deployment strategy
    pub deployment_strategy: DeploymentStrategy,

    /// Alert threshold
    pub alert_threshold: u32,

    /// Auto-migration on detection
    pub auto_migrate: bool,

    /// Bounty amounts for different levels
    pub bounty_tiers: Vec<u64>,

    /// Webhook URL for alerts
    pub webhook_url: Option<String>,

    /// Email addresses for alerts
    pub alert_emails: Vec<String>,

    /// Suspicious activity threshold (attempts before flagging)
    pub suspicious_threshold: u32,

    /// Time window for rate limiting (seconds)
    pub rate_limit_window_secs: u64,

    /// Maximum verification attempts in window
    pub max_attempts_per_window: u32,
}

/// Canary Deployment Strategy
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DeploymentStrategy {
    /// Deploy canaries with progressively weaker security
    Progressive,
    /// Deploy multiple canaries at each security level
    Redundant,
    /// Deploy canaries across different quantum schemes
    Diverse,
    /// Combine all strategies
    Comprehensive,
}

/// Canary Identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CanaryId(pub [u8; 16]);

/// Monitoring Result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitoringResult {
    /// Canary ID
    pub canary_id: CanaryId,

    /// Check timestamp
    pub checked_at: SystemTime,

    /// Result of check
    pub status: CanaryStatus,

    /// Additional details
    pub details: Option<String>,
}

/// Canary Status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CanaryStatus {
    /// Canary is intact
    Healthy,
    /// Canary shows signs of attack
    Suspicious,
    /// Canary has been compromised
    Compromised,
    /// Canary check failed
    CheckFailed,
}

/// Emergency migration record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmergencyMigrationRecord {
    /// ID of the canary that triggered the migration
    pub trigger_canary_id: CanaryId,
    /// When the migration was triggered
    pub triggered_at: SystemTime,
    /// Security level that was compromised
    pub compromised_security_level: u8,
    /// Recommended action
    pub recommended_action: MigrationAction,
    /// Urgency level
    pub urgency: MigrationUrgency,
}

/// Recommended migration action
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MigrationAction {
    /// Upgrade all quantum keys to higher security
    UpgradeAllKeys,
    /// Rotate specific keys that are at risk
    RotateVulnerableKeys,
    /// Force close Lightning channels with weak keys
    ForceCloseLightning,
    /// Full network migration to new signature scheme
    FullSchemeMigration,
    /// Monitor only (for suspicious but not confirmed threats)
    MonitorOnly,
}

/// Migration urgency level
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MigrationUrgency {
    /// Critical - immediate action required
    Critical,
    /// High - action within hours
    High,
    /// Medium - action within days
    Medium,
    /// Low - planned migration
    Low,
}

impl QuantumCanarySystem {
    /// Create new canary system
    pub fn new(config: CanaryConfig) -> Self {
        Self {
            canaries: Arc::new(RwLock::new(HashMap::new())),
            monitoring_results: Arc::new(RwLock::new(Vec::new())),
            alert_endpoints: config.alert_emails.clone(),
            config,
            activity_metrics: Arc::new(RwLock::new(CanaryActivityMetrics {
                verification_attempts: HashMap::new(),
                failed_attempts: HashMap::new(),
                timing_anomalies: HashMap::new(),
                scan_attempts: 0,
                last_reset: SystemTime::now(),
            })),
            on_chain_states: Arc::new(RwLock::new(HashMap::new())),
            emergency_triggered: Arc::new(RwLock::new(false)),
        }
    }

    /// Deploy canaries according to strategy
    pub fn deploy_canaries(&self) -> Result<Vec<CanaryId>, CanaryError> {
        match self.config.deployment_strategy {
            DeploymentStrategy::Progressive => self.deploy_progressive_canaries(),
            DeploymentStrategy::Redundant => self.deploy_redundant_canaries(),
            DeploymentStrategy::Diverse => self.deploy_diverse_canaries(),
            DeploymentStrategy::Comprehensive => self.deploy_comprehensive_canaries(),
        }
    }

    /// Deploy progressively weaker canaries
    fn deploy_progressive_canaries(&self) -> Result<Vec<CanaryId>, CanaryError> {
        let mut deployed = Vec::new();

        // Deploy canaries with security levels 1-3 (production uses 3-5)
        for security_level in 1..=3 {
            let canary = self.create_canary(
                QuantumScheme::Dilithium,
                security_level,
                self.config
                    .bounty_tiers
                    .get(security_level as usize - 1)
                    .copied()
                    .unwrap_or(1000 * security_level as u64),
            )?;

            let id = canary.id;
            self.canaries.write().unwrap().insert(id, canary);
            deployed.push(id);
        }

        Ok(deployed)
    }

    /// Deploy redundant canaries at each level
    fn deploy_redundant_canaries(&self) -> Result<Vec<CanaryId>, CanaryError> {
        let mut deployed = Vec::new();

        // Deploy 3 canaries at each security level
        for security_level in 1..=2 {
            for _ in 0..3 {
                let canary = self.create_canary(
                    QuantumScheme::Dilithium,
                    security_level,
                    self.config
                        .bounty_tiers
                        .get(security_level as usize - 1)
                        .copied()
                        .unwrap_or(1000),
                )?;

                let id = canary.id;
                self.canaries.write().unwrap().insert(id, canary);
                deployed.push(id);
            }
        }

        Ok(deployed)
    }

    /// Deploy diverse canaries across schemes
    fn deploy_diverse_canaries(&self) -> Result<Vec<CanaryId>, CanaryError> {
        let mut deployed = Vec::new();

        let schemes = [
            QuantumScheme::Dilithium,
            QuantumScheme::Falcon,
            QuantumScheme::SphincsPlus,
        ];

        for scheme in &schemes {
            let canary = self.create_canary(*scheme, 1, 5000)?;
            let id = canary.id;
            self.canaries.write().unwrap().insert(id, canary);
            deployed.push(id);
        }

        Ok(deployed)
    }

    /// Deploy comprehensive canary coverage
    fn deploy_comprehensive_canaries(&self) -> Result<Vec<CanaryId>, CanaryError> {
        let mut deployed = Vec::new();

        // Combine all strategies
        deployed.extend(self.deploy_progressive_canaries()?);
        deployed.extend(self.deploy_redundant_canaries()?);
        deployed.extend(self.deploy_diverse_canaries()?);

        Ok(deployed)
    }

    /// Create individual canary
    fn create_canary(
        &self,
        scheme: QuantumScheme,
        security_level: u8,
        bounty_value: u64,
    ) -> Result<QuantumCanary, CanaryError> {
        // Generate intentionally weak keys
        let params = QuantumParameters {
            scheme,
            security_level,
        };

        let weak_keys = QuantumKeyPair::generate(params)?;

        // Generate unique ID
        let mut id_bytes = [0u8; 16];
        use rand::{rngs::OsRng, RngCore};
        OsRng.fill_bytes(&mut id_bytes);

        Ok(QuantumCanary {
            id: CanaryId(id_bytes),
            weak_keys,
            bounty_value,
            deployed_at: SystemTime::now(),
            last_verified: SystemTime::now(),
            compromise_detected: false,
            canary_tx_id: None,
            security_level,
        })
    }

    /// Check all canaries for compromise
    pub async fn check_all_canaries(&self) -> Result<Vec<MonitoringResult>, CanaryError> {
        let mut results = Vec::new();

        let canaries = self.canaries.read().unwrap().clone();

        for (id, mut canary) in canaries {
            let result = self.check_canary(&mut canary).await?;

            // Update canary if compromised
            if result.status == CanaryStatus::Compromised {
                canary.compromise_detected = true;
                self.canaries.write().unwrap().insert(id, canary.clone());

                // Trigger emergency response
                self.handle_compromise(&canary).await?;
            }

            results.push(result);
        }

        // Store results
        self.monitoring_results
            .write()
            .unwrap()
            .extend(results.clone());

        Ok(results)
    }

    /// Check individual canary
    async fn check_canary(
        &self,
        canary: &mut QuantumCanary,
    ) -> Result<MonitoringResult, CanaryError> {
        // Create test message
        let test_message = format!(
            "canary-check-{}-{}",
            hex::encode(canary.id.0),
            canary
                .last_verified
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_secs()
        );

        // Sign with canary's weak keys
        let signature = sign_quantum(&canary.weak_keys, test_message.as_bytes())?;

        // Verify signature still works
        let params = QuantumParameters {
            scheme: QuantumScheme::Dilithium,
            security_level: canary.security_level,
        };

        let verified = verify_quantum_signature(
            &canary.weak_keys.public_key,
            test_message.as_bytes(),
            &signature,
            params,
        )?;

        // Check if canary transaction has been spent (if deployed on-chain)
        let on_chain_status = if let Some(tx_id) = canary.canary_tx_id {
            self.check_on_chain_canary(tx_id).await?
        } else {
            CanaryStatus::Healthy
        };

        // Determine overall status
        let status = if !verified {
            CanaryStatus::Compromised
        } else if on_chain_status == CanaryStatus::Compromised {
            CanaryStatus::Compromised
        } else if self.detect_suspicious_activity(canary).await? {
            CanaryStatus::Suspicious
        } else {
            CanaryStatus::Healthy
        };

        // Update last verified time
        canary.last_verified = SystemTime::now();

        Ok(MonitoringResult {
            canary_id: canary.id,
            checked_at: SystemTime::now(),
            status,
            details: match status {
                CanaryStatus::Compromised => Some("Quantum attack detected!".to_string()),
                CanaryStatus::Suspicious => Some("Unusual activity detected".to_string()),
                _ => None,
            },
        })
    }

    /// Check on-chain canary transaction
    async fn check_on_chain_canary(&self, tx_id: [u8; 32]) -> Result<CanaryStatus, CanaryError> {
        // Check our cached on-chain state
        let states = self.on_chain_states.read().map_err(|_| CanaryError::MonitoringFailed)?;

        if let Some(state) = states.get(&tx_id) {
            if state.spent {
                tracing::error!(
                    "On-chain canary {} has been spent! Potential quantum attack.",
                    hex::encode(&tx_id[..8])
                );
                return Ok(CanaryStatus::Compromised);
            }
        }

        // If we don't have state yet, the canary hasn't been deployed on-chain
        // or needs to be synced - return healthy for now
        Ok(CanaryStatus::Healthy)
    }

    /// Update on-chain canary state (called by blockchain sync)
    pub fn update_on_chain_state(
        &self,
        tx_id: [u8; 32],
        spent: bool,
        height: u64,
        spending_tx: Option<[u8; 32]>,
    ) -> Result<(), CanaryError> {
        let mut states = self.on_chain_states.write().map_err(|_| CanaryError::MonitoringFailed)?;

        let state = states.entry(tx_id).or_insert(OnChainCanaryState {
            spent: false,
            last_checked_height: 0,
            spending_tx: None,
        });

        state.spent = spent;
        state.last_checked_height = height;
        state.spending_tx = spending_tx;

        if spent {
            tracing::warn!(
                "Canary UTXO {} marked as spent at height {} - possible compromise!",
                hex::encode(&tx_id[..8]),
                height
            );
        }

        Ok(())
    }

    /// Register a canary transaction on-chain
    pub fn register_on_chain_canary(&self, canary_id: CanaryId, tx_id: [u8; 32]) -> Result<(), CanaryError> {
        // Update the canary with the transaction ID
        let mut canaries = self.canaries.write().map_err(|_| CanaryError::MonitoringFailed)?;

        if let Some(canary) = canaries.get_mut(&canary_id) {
            canary.canary_tx_id = Some(tx_id);
            tracing::info!(
                "Registered on-chain canary {} with tx {}",
                hex::encode(canary_id.0),
                hex::encode(&tx_id[..8])
            );
        }

        // Initialize on-chain state tracking
        let mut states = self.on_chain_states.write().map_err(|_| CanaryError::MonitoringFailed)?;
        states.insert(tx_id, OnChainCanaryState {
            spent: false,
            last_checked_height: 0,
            spending_tx: None,
        });

        Ok(())
    }

    /// Detect suspicious activity around canary
    async fn detect_suspicious_activity(
        &self,
        canary: &QuantumCanary,
    ) -> Result<bool, CanaryError> {
        let mut metrics = self.activity_metrics.write().map_err(|_| CanaryError::MonitoringFailed)?;

        let now = SystemTime::now();

        // Reset metrics window if expired
        let window_duration = Duration::from_secs(self.config.rate_limit_window_secs);
        if let Ok(elapsed) = now.duration_since(metrics.last_reset) {
            if elapsed > window_duration {
                metrics.verification_attempts.clear();
                metrics.failed_attempts.clear();
                metrics.timing_anomalies.clear();
                metrics.scan_attempts = 0;
                metrics.last_reset = now;
            }
        }

        // Record this verification attempt
        let attempts = metrics.verification_attempts
            .entry(canary.id)
            .or_insert_with(Vec::new);
        attempts.push(now);

        // Check 1: Unusual number of verification attempts (potential brute force)
        let recent_attempts = attempts.iter()
            .filter(|t| {
                now.duration_since(**t)
                    .map(|d| d < window_duration)
                    .unwrap_or(false)
            })
            .count() as u32;

        if recent_attempts > self.config.max_attempts_per_window {
            tracing::warn!(
                "Canary {} has {} verification attempts in window (threshold: {})",
                hex::encode(canary.id.0),
                recent_attempts,
                self.config.max_attempts_per_window
            );
            return Ok(true);
        }

        // Clone attempts for later analysis to avoid borrow conflicts
        let attempts_clone = attempts.clone();

        // Check 2: Too many failed attempts
        let failed = metrics.failed_attempts.get(&canary.id).copied().unwrap_or(0);
        if failed > self.config.suspicious_threshold {
            tracing::warn!(
                "Canary {} has {} failed attempts (threshold: {})",
                hex::encode(canary.id.0),
                failed,
                self.config.suspicious_threshold
            );
            return Ok(true);
        }

        // Check 3: Timing anomalies (attempts coming too regularly = automated attack)
        if attempts_clone.len() >= 3 {
            let mut intervals: Vec<Duration> = Vec::new();
            for window in attempts_clone.windows(2) {
                if let Ok(interval) = window[1].duration_since(window[0]) {
                    intervals.push(interval);
                }
            }

            // Check for suspiciously regular intervals (within 10ms variance)
            if intervals.len() >= 2 {
                let avg_interval_ms: u64 = intervals.iter()
                    .map(|d| d.as_millis() as u64)
                    .sum::<u64>() / intervals.len() as u64;

                let variance: u64 = intervals.iter()
                    .map(|d| {
                        let diff = (d.as_millis() as i64 - avg_interval_ms as i64).unsigned_abs();
                        diff * diff
                    })
                    .sum::<u64>() / intervals.len() as u64;

                // Very low variance indicates automated attack
                if variance < 100 && avg_interval_ms < 1000 {
                    let anomalies = metrics.timing_anomalies.entry(canary.id).or_insert(0);
                    *anomalies += 1;

                    if *anomalies > 3 {
                        tracing::warn!(
                            "Canary {} showing timing anomaly pattern (variance: {}, avg_interval: {}ms)",
                            hex::encode(canary.id.0),
                            variance,
                            avg_interval_ms
                        );
                        return Ok(true);
                    }
                }
            }
        }

        // Check 4: High scan attempts (network-wide)
        if metrics.scan_attempts > 100 {
            tracing::warn!(
                "High network scan activity detected: {} attempts",
                metrics.scan_attempts
            );
            return Ok(true);
        }

        Ok(false)
    }

    /// Record a failed verification attempt (for activity detection)
    pub fn record_failed_attempt(&self, canary_id: CanaryId) -> Result<(), CanaryError> {
        let mut metrics = self.activity_metrics.write().map_err(|_| CanaryError::MonitoringFailed)?;
        let failed = metrics.failed_attempts.entry(canary_id).or_insert(0);
        *failed += 1;
        Ok(())
    }

    /// Record a network scan attempt
    pub fn record_scan_attempt(&self) -> Result<(), CanaryError> {
        let mut metrics = self.activity_metrics.write().map_err(|_| CanaryError::MonitoringFailed)?;
        metrics.scan_attempts += 1;
        Ok(())
    }

    /// Handle compromised canary
    async fn handle_compromise(&self, canary: &QuantumCanary) -> Result<(), CanaryError> {
        // 1. Send alerts
        self.send_alerts(canary).await?;

        // 2. Log the event
        self.log_compromise(canary)?;

        // 3. Trigger emergency migration if configured
        if self.config.auto_migrate {
            self.trigger_emergency_migration(canary).await?;
        }

        Ok(())
    }

    /// Send alerts about compromised canary
    async fn send_alerts(&self, canary: &QuantumCanary) -> Result<(), CanaryError> {
        let _alert_message = format!(
            "QUANTUM CANARY COMPROMISED!\n\
            Canary ID: {}\n\
            Security Level: {}\n\
            Deployed: {:?}\n\
            Compromised: {:?}\n\
            ACTION REQUIRED: Initiate quantum migration immediately!",
            hex::encode(canary.id.0),
            canary.security_level,
            canary.deployed_at,
            SystemTime::now()
        );

        // Send to all configured endpoints
        for _endpoint in &self.alert_endpoints {
            // In production, send actual alerts (email, SMS, webhook, etc.)
        }

        Ok(())
    }

    /// Log compromise event
    fn log_compromise(&self, canary: &QuantumCanary) -> Result<(), CanaryError> {
        use std::fs::OpenOptions;
        use std::io::Write;

        let log_entry = format!(
            "{:?} - COMPROMISE - Canary {} (Level {}) compromised\n",
            SystemTime::now(),
            hex::encode(canary.id.0),
            canary.security_level
        );

        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open("quantum_canary.log")?;

        file.write_all(log_entry.as_bytes())?;

        Ok(())
    }

    /// Trigger emergency quantum migration
    async fn trigger_emergency_migration(&self, canary: &QuantumCanary) -> Result<(), CanaryError> {
        // Check if already triggered to prevent duplicate migrations
        {
            let triggered = self.emergency_triggered.read().map_err(|_| CanaryError::MonitoringFailed)?;
            if *triggered {
                tracing::info!("Emergency migration already triggered, skipping duplicate");
                return Ok(());
            }
        }

        // Mark as triggered
        {
            let mut triggered = self.emergency_triggered.write().map_err(|_| CanaryError::MonitoringFailed)?;
            *triggered = true;
        }

        tracing::error!(
            "CRITICAL: Triggering emergency quantum migration due to canary {} compromise!",
            hex::encode(canary.id.0)
        );

        // 1. Log the emergency event with full details
        self.log_emergency_event(canary)?;

        // 2. Send high-priority alerts to all configured endpoints
        self.send_emergency_alerts(canary).await?;

        // 3. Mark all canaries as needing immediate attention
        {
            let mut canaries = self.canaries.write().map_err(|_| CanaryError::MonitoringFailed)?;
            for (_, c) in canaries.iter_mut() {
                // Flag for migration
                c.compromise_detected = true;
            }
        }

        // 4. Create migration record for blockchain
        let migration_record = EmergencyMigrationRecord {
            trigger_canary_id: canary.id,
            triggered_at: SystemTime::now(),
            compromised_security_level: canary.security_level,
            recommended_action: MigrationAction::UpgradeAllKeys,
            urgency: MigrationUrgency::Critical,
        };

        tracing::error!(
            "EMERGENCY MIGRATION RECORD: {:?}",
            migration_record
        );

        // 5. Broadcast migration recommendation
        // In production, this would:
        // - Notify connected peers of the quantum threat
        // - Publish an emergency message to the network
        // - Begin automatic key rotation for affected addresses

        tracing::warn!(
            "Emergency migration initiated - all nodes should upgrade to security level {} or higher",
            canary.security_level + 2
        );

        Ok(())
    }

    /// Log emergency event
    fn log_emergency_event(&self, canary: &QuantumCanary) -> Result<(), CanaryError> {
        use std::fs::OpenOptions;
        use std::io::Write;

        let emergency_entry = format!(
            "EMERGENCY MIGRATION TRIGGERED\n\
            Time: {:?}\n\
            Canary ID: {}\n\
            Security Level: {}\n\
            Deployed At: {:?}\n\
            Last Verified: {:?}\n\
            Bounty Value: {} satoshis\n\
            Action Required: Upgrade all quantum keys immediately!\n\
            ---\n",
            SystemTime::now(),
            hex::encode(canary.id.0),
            canary.security_level,
            canary.deployed_at,
            canary.last_verified,
            canary.bounty_value
        );

        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open("quantum_emergency.log")?;

        file.write_all(emergency_entry.as_bytes())?;

        Ok(())
    }

    /// Send high-priority emergency alerts
    async fn send_emergency_alerts(&self, canary: &QuantumCanary) -> Result<(), CanaryError> {
        let alert_message = format!(
            "🚨 QUANTUM EMERGENCY ALERT 🚨\n\n\
            A quantum canary has been compromised!\n\n\
            Canary ID: {}\n\
            Security Level: {} (NIST Level {})\n\
            Bounty: {} satoshis\n\n\
            IMMEDIATE ACTIONS REQUIRED:\n\
            1. Stop accepting new quantum signatures at level {}\n\
            2. Initiate key rotation for all affected addresses\n\
            3. Force-close any Lightning channels using weak keys\n\
            4. Upgrade to security level {} or higher\n\n\
            This is an automated alert from the Supernova Quantum Canary System.",
            hex::encode(canary.id.0),
            canary.security_level,
            canary.security_level,
            canary.bounty_value,
            canary.security_level,
            canary.security_level + 2
        );

        // Send to webhook if configured
        if let Some(webhook_url) = &self.config.webhook_url {
            tracing::info!("Sending emergency alert to webhook: {}", webhook_url);
            // In production, make HTTP POST to webhook
            // For now, log the intent
            tracing::error!("WEBHOOK ALERT to {}: {}", webhook_url, alert_message);
        }

        // Send to all alert endpoints
        for endpoint in &self.alert_endpoints {
            tracing::error!("EMERGENCY ALERT to {}: Quantum canary compromised!", endpoint);
        }

        Ok(())
    }

    /// Check if emergency migration has been triggered
    pub fn is_emergency_triggered(&self) -> Result<bool, CanaryError> {
        let triggered = self.emergency_triggered.read().map_err(|_| CanaryError::MonitoringFailed)?;
        Ok(*triggered)
    }

    /// Reset emergency state (for testing or after full migration)
    pub fn reset_emergency_state(&self) -> Result<(), CanaryError> {
        let mut triggered = self.emergency_triggered.write().map_err(|_| CanaryError::MonitoringFailed)?;
        *triggered = false;
        tracing::info!("Emergency state reset");
        Ok(())
    }

    /// Add alert endpoint
    pub fn add_alert_endpoint(&mut self, endpoint: String) {
        self.alert_endpoints.push(endpoint);
    }

    /// Get canary statistics
    pub fn get_statistics(&self) -> CanaryStatistics {
        let canaries = self.canaries.read().unwrap();
        let results = self.monitoring_results.read().unwrap();

        let total_canaries = canaries.len();
        let compromised = canaries.values().filter(|c| c.compromise_detected).count();
        let suspicious = results
            .iter()
            .filter(|r| r.status == CanaryStatus::Suspicious)
            .count();

        CanaryStatistics {
            total_canaries,
            healthy: total_canaries - compromised - suspicious,
            suspicious,
            compromised,
            last_check: results.last().map(|r| r.checked_at),
            total_bounty: canaries.values().map(|c| c.bounty_value).sum(),
        }
    }
}

/// Canary Statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanaryStatistics {
    pub total_canaries: usize,
    pub healthy: usize,
    pub suspicious: usize,
    pub compromised: usize,
    pub last_check: Option<SystemTime>,
    pub total_bounty: u64,
}

/// Canary Errors
#[derive(Debug, thiserror::Error)]
pub enum CanaryError {
    #[error("Quantum key generation failed: {0}")]
    KeyGeneration(#[from] crate::crypto::quantum::QuantumError),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Canary deployment failed")]
    DeploymentFailed,

    #[error("Monitoring failed")]
    MonitoringFailed,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config(deployment_strategy: DeploymentStrategy, auto_migrate: bool) -> CanaryConfig {
        CanaryConfig {
            check_interval: Duration::from_secs(60),
            deployment_strategy,
            alert_threshold: 1,
            auto_migrate,
            bounty_tiers: vec![1000, 5000, 10000],
            webhook_url: None,
            alert_emails: vec![],
            suspicious_threshold: 10,
            rate_limit_window_secs: 300,
            max_attempts_per_window: 100,
        }
    }

    #[tokio::test]
    async fn test_canary_deployment() {
        let config = test_config(DeploymentStrategy::Progressive, false);
        let system = QuantumCanarySystem::new(config);
        let canaries = system.deploy_canaries().unwrap();

        assert_eq!(canaries.len(), 3);
        assert_eq!(system.canaries.read().unwrap().len(), 3);
    }

    #[tokio::test]
    async fn test_canary_monitoring() {
        let config = test_config(DeploymentStrategy::Progressive, false);
        let mut system = QuantumCanarySystem::new(config);
        system.add_alert_endpoint("test@example.com".to_string());

        let _canaries = system.deploy_canaries().unwrap();
        let results = system.check_all_canaries().await.unwrap();

        assert!(!results.is_empty());
        assert!(results.iter().all(|r| r.status == CanaryStatus::Healthy));
    }

    #[test]
    fn test_canary_statistics() {
        let config = test_config(DeploymentStrategy::Comprehensive, true);
        let system = QuantumCanarySystem::new(config);
        let _canaries = system.deploy_canaries().unwrap();

        let stats = system.get_statistics();
        assert!(stats.total_canaries > 0);
        assert_eq!(stats.healthy, stats.total_canaries);
        assert_eq!(stats.compromised, 0);
    }

    #[test]
    fn test_on_chain_state_tracking() {
        let config = test_config(DeploymentStrategy::Progressive, false);
        let system = QuantumCanarySystem::new(config);

        let tx_id = [1u8; 32];
        system.update_on_chain_state(tx_id, false, 100, None).unwrap();

        // Check still healthy
        let states = system.on_chain_states.read().unwrap();
        assert!(!states.get(&tx_id).unwrap().spent);
    }

    #[test]
    fn test_emergency_trigger() {
        let config = test_config(DeploymentStrategy::Progressive, true);
        let system = QuantumCanarySystem::new(config);

        assert!(!system.is_emergency_triggered().unwrap());

        // Note: actual trigger is async and requires canary,
        // so we just test the reset functionality
        system.reset_emergency_state().unwrap();
        assert!(!system.is_emergency_triggered().unwrap());
    }

    #[test]
    fn test_activity_recording() {
        let config = test_config(DeploymentStrategy::Progressive, false);
        let system = QuantumCanarySystem::new(config);

        let canary_id = CanaryId([0u8; 16]);
        system.record_failed_attempt(canary_id).unwrap();
        system.record_failed_attempt(canary_id).unwrap();
        system.record_scan_attempt().unwrap();

        let metrics = system.activity_metrics.read().unwrap();
        assert_eq!(*metrics.failed_attempts.get(&canary_id).unwrap(), 2);
        assert_eq!(metrics.scan_attempts, 1);
    }
}
