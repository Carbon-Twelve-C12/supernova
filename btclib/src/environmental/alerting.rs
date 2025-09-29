use std::collections::HashMap;
use serde::{Serialize, Deserialize};
use chrono::{DateTime, Utc};
use thiserror::Error;
use std::sync::{Arc, RwLock};

use crate::environmental::emissions::{EmissionsTracker, Emissions};
use crate::environmental::dashboard::{EnvironmentalDashboard, EnvironmentalMetrics, EmissionsTimePeriod};
use crate::environmental::treasury::{EnvironmentalTreasury, TreasuryAccountType};

/// Error types for environmental alerting
#[derive(Error, Debug)]
pub enum AlertingError {
    #[error("Invalid alert configuration: {0}")]
    InvalidConfiguration(String),

    #[error("Metric calculation error: {0}")]
    MetricCalculationError(String),

    #[error("Notification error: {0}")]
    NotificationError(String),

    #[error("Database error: {0}")]
    DatabaseError(String),
}

/// Alert severity levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AlertSeverity {
    /// Informational alert
    Info,
    /// Low-priority warning
    Low,
    /// Medium-priority warning
    Medium,
    /// High-priority warning
    High,
    /// Critical alert requiring immediate attention
    Critical,
}

/// Alert status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AlertStatus {
    /// Alert is active
    Active,
    /// Alert has been acknowledged
    Acknowledged,
    /// Alert has been resolved
    Resolved,
    /// Alert has been dismissed
    Dismissed,
}

/// Types of environmental metrics that can be monitored
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MetricType {
    /// Total energy consumption
    EnergyConsumption,
    /// Total carbon emissions
    CarbonEmissions,
    /// Renewable energy percentage
    RenewablePercentage,
    /// Carbon offset percentage
    OffsetPercentage,
    /// Net carbon impact
    NetCarbonImpact,
    /// Carbon intensity per transaction
    CarbonIntensity,
    /// Treasury allocation percentage
    TreasuryAllocation,
    /// Treasury balance
    TreasuryBalance,
    /// REC coverage
    RECCoverage,
    /// Verified miner percentage
    VerifiedMinerPercentage,
}

/// Comparison operators for alerts
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ComparisonOperator {
    /// Greater than
    GreaterThan,
    /// Greater than or equal
    GreaterThanOrEqual,
    /// Less than
    LessThan,
    /// Less than or equal
    LessThanOrEqual,
    /// Equal
    Equal,
    /// Not equal
    NotEqual,
}

/// Alert notification method
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum NotificationMethod {
    /// Email notification
    Email(String),
    /// Webhook notification
    Webhook(String),
    /// Log entry
    Log,
    /// Console output
    Console,
}

/// Alert rule configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertRule {
    /// Unique ID for the rule
    pub id: String,
    /// Rule name
    pub name: String,
    /// Description of the rule
    pub description: String,
    /// Type of metric to monitor
    pub metric_type: MetricType,
    /// Comparison operator
    pub operator: ComparisonOperator,
    /// Threshold value to compare against
    pub threshold: f64,
    /// Alert severity
    pub severity: AlertSeverity,
    /// Notification methods
    pub notification_methods: Vec<NotificationMethod>,
    /// Cooldown period between alerts in seconds
    pub cooldown_seconds: u64,
    /// Whether the rule is enabled
    pub enabled: bool,
    /// Whether to auto-resolve when condition is no longer met
    pub auto_resolve: bool,
}

/// Alert instance
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Alert {
    /// Unique ID for the alert
    pub id: String,
    /// Rule that triggered the alert
    pub rule_id: String,
    /// Alert message
    pub message: String,
    /// Current metric value
    pub current_value: f64,
    /// Threshold value from the rule
    pub threshold_value: f64,
    /// Alert timestamp
    pub timestamp: DateTime<Utc>,
    /// Alert status
    pub status: AlertStatus,
    /// User who acknowledged or resolved the alert
    pub resolved_by: Option<String>,
    /// Resolution timestamp
    pub resolved_at: Option<DateTime<Utc>>,
    /// Additional context about the alert
    pub context: HashMap<String, String>,
}

/// Alert history record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertHistoryRecord {
    /// Alert ID
    pub alert_id: String,
    /// Status change
    pub new_status: AlertStatus,
    /// Timestamp
    pub timestamp: DateTime<Utc>,
    /// User who made the change
    pub user: Option<String>,
    /// Note about the status change
    pub note: Option<String>,
}

/// Configuration for the alerting system
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertingConfig {
    /// Whether alerting is enabled
    pub enabled: bool,
    /// Check interval in seconds
    pub check_interval_seconds: u64,
    /// Max alerts per hour to prevent alert storms
    pub max_alerts_per_hour: u32,
    /// Default notification methods
    pub default_notification_methods: Vec<NotificationMethod>,
    /// Maximum history records to keep
    pub max_history_records: usize,
    /// Whether to log all alert activity
    pub log_all_activity: bool,
}

impl Default for AlertingConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            check_interval_seconds: 300,
            max_alerts_per_hour: 20,
            default_notification_methods: vec![NotificationMethod::Log],
            max_history_records: 1000,
            log_all_activity: true,
        }
    }
}

/// Type alias for compatibility
pub type AlertingSystem = EnvironmentalAlertingSystem;

/// Environmental alerting system
#[derive(Debug, Clone)]
pub struct EnvironmentalAlertingSystem {
    /// Active alerts
    pub alerts: Arc<RwLock<HashMap<String, Alert>>>,
    /// Alert rules
    pub rules: Arc<RwLock<HashMap<String, AlertRule>>>,
    /// Configuration
    pub config: AlertingConfig,
}

impl EnvironmentalAlertingSystem {
    /// Create a new environmental alerting system
    pub fn new(
        config: AlertingConfig,
        dashboard: EnvironmentalDashboard,
        emissions_tracker: EmissionsTracker,
        treasury: EnvironmentalTreasury,
    ) -> Self {
        Self {
            alerts: Arc::new(RwLock::new(HashMap::new())),
            rules: Arc::new(RwLock::new(HashMap::new())),
            config,
        }
    }

    /// Add an alert rule
    pub fn add_rule(&self, rule: AlertRule) -> Result<(), AlertingError> {
        let mut rules = self.rules.write().unwrap();
        rules.insert(rule.id.clone(), rule);
        Ok(())
    }

    /// Trigger an alert
    pub fn trigger_alert(&self, alert: Alert) -> Result<(), AlertingError> {
        if !self.config.enabled {
            return Ok(());
        }

        let mut alerts = self.alerts.write().unwrap();
        alerts.insert(alert.id.clone(), alert);

        // Clean up old alerts if necessary
        if alerts.len() > self.config.max_alerts_per_hour as usize {
            let oldest_id = alerts.keys().next().cloned();
            if let Some(id) = oldest_id {
                alerts.remove(&id);
            }
        }

        Ok(())
    }
}

/// Alert rule for environmental monitoring
impl EnvironmentalAlertingSystem {
    /// Create a new environmental alerting system
    pub fn new(
        config: AlertingConfig,
        dashboard: EnvironmentalDashboard,
        emissions_tracker: EmissionsTracker,
        treasury: EnvironmentalTreasury,
    ) -> Self {
        Self {
            config,
            rules: Vec::new(),
            active_alerts: HashMap::new(),
            alert_history: Vec::new(),
            last_check: HashMap::new(),
            alerts_this_hour: 0,
            current_hour_start: Utc::now(),
            dashboard,
            emissions_tracker,
            treasury,
        }
    }

    /// Add a new alert rule
    pub fn add_rule(&mut self, rule: AlertRule) -> Result<(), AlertingError> {
        // Validate rule
        if rule.threshold.is_nan() || rule.threshold.is_infinite() {
            return Err(AlertingError::InvalidConfiguration(
                format!("Invalid threshold value: {}", rule.threshold)
            ));
        }

        if rule.cooldown_seconds == 0 {
            return Err(AlertingError::InvalidConfiguration(
                "Cooldown seconds must be greater than zero".to_string()
            ));
        }

        // Add rule
        self.rules.push(rule);
        Ok(())
    }

    /// Check all alert rules
    pub fn check_alerts(&mut self) -> Vec<Alert> {
        if !self.config.enabled {
            return Vec::new();
        }

        // Reset hourly counter if needed
        let now = Utc::now();
        if (now - self.current_hour_start).num_seconds() > 3600 {
            self.alerts_this_hour = 0;
            self.current_hour_start = now;
        }

        // Don't send more alerts than configured maximum per hour
        if self.alerts_this_hour >= self.config.max_alerts_per_hour {
            return Vec::new();
        }

        let mut new_alerts = Vec::new();

        // Collect all data first to avoid borrow conflicts

        // 1. Collect the rules that need processing
        let mut rules_to_process = Vec::new();
        for rule in &self.rules {
            if !rule.enabled {
                continue;
            }

            // Check cooldown period
            let should_process = if let Some(last_check) = self.last_check.get(&rule.id) {
                let cooldown = chrono::Duration::seconds(rule.cooldown_seconds as i64);
                (now - *last_check) >= cooldown
            } else {
                true
            };

            // Check if already alerting
            let is_already_alerting = self.active_alerts.values().any(|a|
                a.rule_id == rule.id && a.status == AlertStatus::Active
            );

            if should_process && !is_already_alerting {
                rules_to_process.push(rule.clone());
            }
        }

        // 2. Collect metrics for all rules we need to process
        let mut rule_metrics: Vec<(AlertRule, Result<f64, AlertingError>)> = Vec::new();
        for rule in rules_to_process {
            // Update last check time here
            self.last_check.insert(rule.id.clone(), now);

            // Get the metric value
            let metric_value = self.get_metric_value(rule.metric_type);
            rule_metrics.push((rule, metric_value));
        }

        // 3. Collect rules that need auto-resolving
        let mut auto_resolves = Vec::new();
        for (rule, metric_result) in &rule_metrics {
            if !rule.auto_resolve {
                continue;
            }

            if let Ok(value) = metric_result {
                // If condition no longer met
                if !self.compare_value(*value, rule.threshold, rule.operator) {
                    // Find all alerts for this rule that need resolving
                    for (alert_id, alert) in &self.active_alerts {
                        if alert.rule_id == rule.id && alert.status == AlertStatus::Active {
                            auto_resolves.push((alert_id.clone(), rule.clone()));
                        }
                    }
                }
            }
        }

        // 4. Process all metrics and create new alerts
        for (rule, metric_result) in rule_metrics {
            match metric_result {
                Ok(value) => {
                    // Check if threshold is crossed
                    if self.compare_value(value, rule.threshold, rule.operator) {
                        // Generate a simple random alert ID
                        let random_id = format!("{:x}", rand::random::<u64>());
                        let alert_id = format!("alert_{}", random_id);

                        // Create alert message
                        let message = format!("{}: {} is {} {} (threshold: {})",
                            rule.name,
                            self.get_metric_name(rule.metric_type),
                            value,
                            self.get_operator_symbol(rule.operator),
                            rule.threshold
                        );

                        // Create the alert
                        let alert = Alert {
                            id: alert_id.clone(),
                            rule_id: rule.id.clone(),
                            message,
                            current_value: value,
                            threshold_value: rule.threshold,
                            timestamp: now,
                            status: AlertStatus::Active,
                            resolved_by: None,
                            resolved_at: None,
                            context: HashMap::new(),
                        };

                        // Store alert
                        self.active_alerts.insert(alert_id.clone(), alert.clone());

                        // Create history record
                        let history_record = AlertHistoryRecord {
                            alert_id: alert_id.clone(),
                            new_status: AlertStatus::Active,
                            timestamp: now,
                            user: None,
                            note: None,
                        };
                        self.alert_history.push(history_record);

                        // Send notifications
                        self.send_notifications(&rule, &alert);

                        // Increment alert counter
                        self.alerts_this_hour += 1;

                        // Add to new alerts
                        new_alerts.push(alert);

                        // Stop if we've reached the hourly limit
                        if self.alerts_this_hour >= self.config.max_alerts_per_hour {
                            break;
                        }
                    }
                },
                Err(e) => {
                    // Log error
                    eprintln!("Error checking rule {}: {}", rule.name, e);
                }
            }
        }

        // 5. Process auto-resolves
        for (alert_id, rule) in auto_resolves {
            if let Some(alert) = self.active_alerts.get_mut(&alert_id) {
                alert.status = AlertStatus::Resolved;
                alert.resolved_by = Some("System".to_string());
                alert.resolved_at = Some(now);

                // Create history record for the resolution
                let history_record = AlertHistoryRecord {
                    alert_id: alert_id.clone(),
                    new_status: AlertStatus::Resolved,
                    timestamp: now,
                    user: Some("System".to_string()),
                    note: Some("Auto-resolved: condition no longer met".to_string()),
                };
                self.alert_history.push(history_record);

                // Log the auto-resolve if configured
                if self.config.log_all_activity {
                    println!("[{}] Alert {} changed to {:?} by System: Auto-resolved: condition no longer met",
                        now.format("%Y-%m-%d %H:%M:%S"),
                        alert_id,
                        AlertStatus::Resolved
                    );
                }
            }
        }

        // 6. Clean up history if needed
        if self.alert_history.len() > self.config.max_history_records {
            self.alert_history.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));
            self.alert_history.truncate(self.config.max_history_records);
        }

        new_alerts
    }

    /// Acknowledge an alert
    pub fn acknowledge_alert(&mut self, alert_id: &str, user: &str, note: Option<String>) -> Result<(), AlertingError> {
        if let Some(alert) = self.active_alerts.get_mut(alert_id) {
            if alert.status == AlertStatus::Active {
                alert.status = AlertStatus::Acknowledged;

                // Create history record
                let history_record = AlertHistoryRecord {
                    alert_id: alert_id.to_string(),
                    new_status: AlertStatus::Acknowledged,
                    timestamp: Utc::now(),
                    user: Some(user.to_string()),
                    note: note.clone(),
                };
                self.alert_history.push(history_record);

                // Log if configured
                if self.config.log_all_activity {
                    let note_str = if let Some(note) = note {
                        format!(": {}", note)
                    } else {
                        "".to_string()
                    };

                    println!("[{}] Alert {} changed to {:?} by {}{}",
                        Utc::now().format("%Y-%m-%d %H:%M:%S"),
                        alert_id,
                        AlertStatus::Acknowledged,
                        user,
                        note_str
                    );
                }

                Ok(())
            } else {
                Err(AlertingError::InvalidConfiguration(
                    format!("Alert {} is not active (status: {:?})", alert_id, alert.status)
                ))
            }
        } else {
            Err(AlertingError::InvalidConfiguration(
                format!("Alert {} not found", alert_id)
            ))
        }
    }

    /// Resolve an alert
    pub fn resolve_alert(&mut self, alert_id: &str, user: &str, note: Option<&str>) -> Result<(), AlertingError> {
        if let Some(alert) = self.active_alerts.get_mut(alert_id) {
            if alert.status == AlertStatus::Active || alert.status == AlertStatus::Acknowledged {
                alert.status = AlertStatus::Resolved;
                alert.resolved_by = Some(user.to_string());
                alert.resolved_at = Some(Utc::now());

                // Create history record
                let history_record = AlertHistoryRecord {
                    alert_id: alert_id.to_string(),
                    new_status: AlertStatus::Resolved,
                    timestamp: Utc::now(),
                    user: Some(user.to_string()),
                    note: note.map(|s| s.to_string()),
                };
                self.alert_history.push(history_record);

                // Log if configured
                if self.config.log_all_activity {
                    let note_str = if let Some(note) = note {
                        format!(": {}", note)
                    } else {
                        "".to_string()
                    };

                    println!("[{}] Alert {} changed to {:?} by {}{}",
                        Utc::now().format("%Y-%m-%d %H:%M:%S"),
                        alert_id,
                        AlertStatus::Resolved,
                        user,
                        note_str
                    );
                }

                Ok(())
            } else {
                Err(AlertingError::InvalidConfiguration(
                    format!("Alert {} cannot be resolved (status: {:?})", alert_id, alert.status)
                ))
            }
        } else {
            Err(AlertingError::InvalidConfiguration(
                format!("Alert {} not found", alert_id)
            ))
        }
    }

    /// Dismiss an alert
    pub fn dismiss_alert(&mut self, alert_id: &str, user: &str, note: Option<String>) -> Result<(), AlertingError> {
        if let Some(alert) = self.active_alerts.get_mut(alert_id) {
            alert.status = AlertStatus::Dismissed;
            alert.resolved_by = Some(user.to_string());
            alert.resolved_at = Some(Utc::now());

            // Create history record
            let history_record = AlertHistoryRecord {
                alert_id: alert_id.to_string(),
                new_status: AlertStatus::Dismissed,
                timestamp: Utc::now(),
                user: Some(user.to_string()),
                note: note.clone(),
            };
            self.alert_history.push(history_record);

            // Log if configured
            if self.config.log_all_activity {
                let note_str = if let Some(note) = &note {
                    format!(": {}", note)
                } else {
                    "".to_string()
                };

                println!("[{}] Alert {} changed to {:?} by {}{}",
                    Utc::now().format("%Y-%m-%d %H:%M:%S"),
                    alert_id,
                    AlertStatus::Dismissed,
                    user,
                    note_str
                );
            }

            Ok(())
        } else {
            Err(AlertingError::InvalidConfiguration(
                format!("Alert {} not found", alert_id)
            ))
        }
    }

    /// Get a list of active alerts
    pub fn get_active_alerts(&self) -> Vec<&Alert> {
        self.active_alerts.values()
            .filter(|a| a.status == AlertStatus::Active || a.status == AlertStatus::Acknowledged)
            .collect()
    }

    /// Get a list of all alerts
    pub fn get_all_alerts(&self) -> Vec<&Alert> {
        self.active_alerts.values().collect()
    }

    /// Get alerts by status
    pub fn get_alerts_by_status(&self, status: AlertStatus) -> Vec<&Alert> {
        self.active_alerts.values()
            .filter(|a| a.status == status)
            .collect()
    }

    /// Get alerts by severity
    pub fn get_alerts_by_severity(&self, severity: AlertSeverity) -> Vec<&Alert> {
        self.active_alerts.values()
            .filter(|a| {
                if let Some(rule) = self.get_rule(&a.rule_id) {
                    rule.severity == severity
                } else {
                    false
                }
            })
            .collect()
    }

    /// Get alerts by metric type
    pub fn get_alerts_by_metric(&self, metric_type: MetricType) -> Vec<&Alert> {
        self.active_alerts.values()
            .filter(|a| {
                if let Some(rule) = self.get_rule(&a.rule_id) {
                    rule.metric_type == metric_type
                } else {
                    false
                }
            })
            .collect()
    }

    /// Get alert history
    pub fn get_alert_history(&self, limit: Option<usize>) -> Vec<&AlertHistoryRecord> {
        let limit = limit.unwrap_or(self.alert_history.len());

        self.alert_history.iter()
            .rev()
            .take(limit)
            .collect()
    }

    /// Get alert rules
    pub fn get_rules(&self) -> &[AlertRule] {
        &self.rules
    }

    /// Enable or disable a rule
    pub fn set_rule_enabled(&mut self, rule_id: &str, enabled: bool) -> Result<(), AlertingError> {
        for rule in &mut self.rules {
            if rule.id == rule_id {
                rule.enabled = enabled;
                return Ok(());
            }
        }

        Err(AlertingError::InvalidConfiguration(
            format!("Rule {} not found", rule_id)
        ))
    }

    /// Get the rule by ID
    fn get_rule(&self, rule_id: &str) -> Option<&AlertRule> {
        self.rules.iter().find(|r| r.id == rule_id)
    }

    /// Get the current value for a metric
    fn get_metric_value(&self, metric_type: MetricType) -> Result<f64, AlertingError> {
        match metric_type {
            MetricType::EnergyConsumption => {
                if let Some(metrics) = self.dashboard.get_metrics(EmissionsTimePeriod::Day) {
                    Ok(metrics.energy_consumption)
                } else {
                    Err(AlertingError::MetricCalculationError("No daily metrics available".to_string()))
                }
            },
            MetricType::CarbonEmissions => {
                if let Some(metrics) = self.dashboard.get_metrics(EmissionsTimePeriod::Day) {
                    Ok(metrics.total_emissions)
                } else {
                    Err(AlertingError::MetricCalculationError("No daily metrics available".to_string()))
                }
            },
            MetricType::RenewablePercentage => {
                if let Some(metrics) = self.dashboard.get_metrics(EmissionsTimePeriod::Day) {
                    Ok(metrics.renewable_percentage.unwrap_or(0.0))
                } else {
                    Err(AlertingError::MetricCalculationError("No daily metrics available".to_string()))
                }
            },
            MetricType::OffsetPercentage => {
                if let Some(metrics) = self.dashboard.get_metrics(EmissionsTimePeriod::Day) {
                    if metrics.total_emissions > 0.0 {
                        Ok((metrics.total_assets / metrics.total_emissions) * 100.0)
                    } else {
                        Ok(0.0)
                    }
                } else {
                    Err(AlertingError::MetricCalculationError("No daily metrics available".to_string()))
                }
            },
            MetricType::NetCarbonImpact => {
                if let Some(metrics) = self.dashboard.get_metrics(EmissionsTimePeriod::Day) {
                    Ok(metrics.net_emissions)
                } else {
                    Err(AlertingError::MetricCalculationError("No daily metrics available".to_string()))
                }
            },
            MetricType::CarbonIntensity => {
                if let Some(metrics) = self.dashboard.get_metrics(EmissionsTimePeriod::Day) {
                    Ok(metrics.emissions_per_transaction)
                } else {
                    Err(AlertingError::MetricCalculationError("No daily metrics available".to_string()))
                }
            },
            MetricType::TreasuryAllocation => {
                Ok(self.treasury.get_current_fee_percentage())
            },
            MetricType::TreasuryBalance => {
                Ok(self.treasury.get_balance(Some(TreasuryAccountType::Main)) as f64)
            },
            MetricType::RECCoverage => {
                if let Some(metrics) = self.dashboard.get_metrics(EmissionsTimePeriod::Month) {
                    Ok(metrics.rec_coverage_percentage.unwrap_or(0.0))
                } else {
                    Err(AlertingError::MetricCalculationError("No monthly metrics available".to_string()))
                }
            },
            MetricType::VerifiedMinerPercentage => {
                // This would require the miner reporting manager
                Err(AlertingError::MetricCalculationError("Verified miner percentage not available".to_string()))
            },
        }
    }

    /// Compare a value against a threshold using the specified operator
    fn compare_value(&self, value: f64, threshold: f64, operator: ComparisonOperator) -> bool {
        match operator {
            ComparisonOperator::GreaterThan => value > threshold,
            ComparisonOperator::GreaterThanOrEqual => value >= threshold,
            ComparisonOperator::LessThan => value < threshold,
            ComparisonOperator::LessThanOrEqual => value <= threshold,
            ComparisonOperator::Equal => (value - threshold).abs() < f64::EPSILON,
            ComparisonOperator::NotEqual => (value - threshold).abs() >= f64::EPSILON,
        }
    }

    /// Get a human-readable name for a metric
    fn get_metric_name(&self, metric_type: MetricType) -> &'static str {
        match metric_type {
            MetricType::EnergyConsumption => "Energy Consumption",
            MetricType::CarbonEmissions => "Carbon Emissions",
            MetricType::RenewablePercentage => "Renewable Energy Percentage",
            MetricType::OffsetPercentage => "Carbon Offset Percentage",
            MetricType::NetCarbonImpact => "Net Carbon Impact",
            MetricType::CarbonIntensity => "Carbon Intensity",
            MetricType::TreasuryAllocation => "Treasury Allocation Percentage",
            MetricType::TreasuryBalance => "Treasury Balance",
            MetricType::RECCoverage => "REC Coverage Percentage",
            MetricType::VerifiedMinerPercentage => "Verified Miner Percentage",
        }
    }

    /// Get a symbol for a comparison operator
    fn get_operator_symbol(&self, operator: ComparisonOperator) -> &'static str {
        match operator {
            ComparisonOperator::GreaterThan => ">",
            ComparisonOperator::GreaterThanOrEqual => ">=",
            ComparisonOperator::LessThan => "<",
            ComparisonOperator::LessThanOrEqual => "<=",
            ComparisonOperator::Equal => "=",
            ComparisonOperator::NotEqual => "!=",
        }
    }

    /// Send notifications for an alert
    fn send_notifications(&self, rule: &AlertRule, alert: &Alert) {
        for method in &rule.notification_methods {
            match method {
                NotificationMethod::Email(email) => {
                    // In a real implementation, this would send an email
                    println!("Sending email notification to {}: {}", email, alert.message);
                },
                NotificationMethod::Webhook(url) => {
                    // In a real implementation, this would call a webhook
                    println!("Calling webhook at {}: {}", url, alert.message);
                },
                NotificationMethod::Log => {
                    println!("[{}] ALERT {}: {} ({})",
                        Utc::now().format("%Y-%m-%d %H:%M:%S"),
                        rule.severity.to_string().to_uppercase(),
                        alert.message,
                        alert.id
                    );
                },
                NotificationMethod::Console => {
                    eprintln!("\x1b[31mALERT {}: {} ({})\x1b[0m",
                        rule.severity.to_string().to_uppercase(),
                        alert.message,
                        alert.id
                    );
                },
            }
        }
    }
}

// String representation of alert severity
impl ToString for AlertSeverity {
    fn to_string(&self) -> String {
        match self {
            AlertSeverity::Info => "info".to_string(),
            AlertSeverity::Low => "low".to_string(),
            AlertSeverity::Medium => "medium".to_string(),
            AlertSeverity::High => "high".to_string(),
            AlertSeverity::Critical => "critical".to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_comparison_operators() {
        let system = EnvironmentalAlertingSystem::new(
            AlertingConfig::default(),
            EnvironmentalDashboard::new(EmissionsTracker::new(), EnvironmentalTreasury::new()),
            EmissionsTracker::new(),
            EnvironmentalTreasury::new(),
        );

        assert!(system.compare_value(10.0, 5.0, ComparisonOperator::GreaterThan));
        assert!(!system.compare_value(5.0, 10.0, ComparisonOperator::GreaterThan));

        assert!(system.compare_value(10.0, 10.0, ComparisonOperator::GreaterThanOrEqual));
        assert!(system.compare_value(15.0, 10.0, ComparisonOperator::GreaterThanOrEqual));
        assert!(!system.compare_value(5.0, 10.0, ComparisonOperator::GreaterThanOrEqual));

        assert!(system.compare_value(5.0, 10.0, ComparisonOperator::LessThan));
        assert!(!system.compare_value(10.0, 5.0, ComparisonOperator::LessThan));

        assert!(system.compare_value(10.0, 10.0, ComparisonOperator::LessThanOrEqual));
        assert!(system.compare_value(5.0, 10.0, ComparisonOperator::LessThanOrEqual));
        assert!(!system.compare_value(15.0, 10.0, ComparisonOperator::LessThanOrEqual));

        assert!(system.compare_value(10.0, 10.0, ComparisonOperator::Equal));
        assert!(!system.compare_value(10.1, 10.0, ComparisonOperator::Equal));

        assert!(system.compare_value(10.1, 10.0, ComparisonOperator::NotEqual));
        assert!(!system.compare_value(10.0, 10.0, ComparisonOperator::NotEqual));
    }
}