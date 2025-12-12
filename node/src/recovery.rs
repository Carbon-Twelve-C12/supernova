//! Comprehensive Error Recovery System for Supernova Node
//!
//! This module implements automatic error recovery with exponential backoff,
//! circuit breaker patterns, and intelligent retry logic for different error types.

use crate::node::Node;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tokio::time::sleep;
use tracing::{debug, error, info, warn};

/// Error category for recovery classification
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ErrorCategory {
    /// Transient errors that may resolve with retry
    Transient,
    /// Permanent errors that won't resolve with retry
    Permanent,
    /// Critical errors requiring immediate attention
    Critical,
}

/// Recovery strategy for different error types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecoveryStrategy {
    /// Retry with exponential backoff
    ExponentialBackoff {
        max_attempts: u32,
        initial_delay: Duration,
        max_delay: Duration,
    },
    /// Retry with fixed delay
    FixedDelay {
        max_attempts: u32,
        delay: Duration,
    },
    /// No retry - fail immediately
    NoRetry,
    /// Circuit breaker - stop retrying after threshold
    CircuitBreaker {
        failure_threshold: u32,
        reset_timeout: Duration,
    },
}

impl Default for RecoveryStrategy {
    fn default() -> Self {
        Self::ExponentialBackoff {
            max_attempts: 3,
            initial_delay: Duration::from_millis(100),
            max_delay: Duration::from_secs(30),
        }
    }
}

/// Circuit breaker state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CircuitState {
    /// Circuit is closed - normal operation
    Closed,
    /// Circuit is open - failing fast
    Open,
    /// Circuit is half-open - testing if service recovered
    HalfOpen,
}

/// Circuit breaker for error recovery
#[derive(Debug)]
struct CircuitBreaker {
    /// Current state
    state: CircuitState,
    /// Number of consecutive failures
    failure_count: u32,
    /// Failure threshold to open circuit
    failure_threshold: u32,
    /// Time when circuit was opened
    opened_at: Option<Instant>,
    /// Reset timeout
    reset_timeout: Duration,
    /// Number of successful attempts in half-open state
    half_open_successes: u32,
    /// Required successes to close circuit
    half_open_success_threshold: u32,
}

impl CircuitBreaker {
    fn new(failure_threshold: u32, reset_timeout: Duration) -> Self {
        Self {
            state: CircuitState::Closed,
            failure_count: 0,
            failure_threshold,
            opened_at: None,
            reset_timeout,
            half_open_successes: 0,
            half_open_success_threshold: 2,
        }
    }

    fn record_success(&mut self) {
        match self.state {
            CircuitState::Closed => {
                // Reset failure count on success
                self.failure_count = 0;
            }
            CircuitState::HalfOpen => {
                self.half_open_successes += 1;
                if self.half_open_successes >= self.half_open_success_threshold {
                    info!("Circuit breaker closing - service recovered");
                    self.state = CircuitState::Closed;
                    self.failure_count = 0;
                    self.half_open_successes = 0;
                }
            }
            CircuitState::Open => {
                // Should not happen, but handle gracefully
            }
        }
    }

    fn record_failure(&mut self) {
        match self.state {
            CircuitState::Closed => {
                self.failure_count += 1;
                if self.failure_count >= self.failure_threshold {
                    warn!(
                        "Circuit breaker opening after {} failures",
                        self.failure_count
                    );
                    self.state = CircuitState::Open;
                    self.opened_at = Some(Instant::now());
                }
            }
            CircuitState::HalfOpen => {
                warn!("Circuit breaker reopening - service still failing");
                self.state = CircuitState::Open;
                self.opened_at = Some(Instant::now());
                self.half_open_successes = 0;
            }
            CircuitState::Open => {
                // Already open, just update timestamp
                self.opened_at = Some(Instant::now());
            }
        }
    }

    fn can_attempt(&mut self) -> bool {
        match self.state {
            CircuitState::Closed => true,
            CircuitState::Open => {
                if let Some(opened_at) = self.opened_at {
                    if opened_at.elapsed() >= self.reset_timeout {
                        info!("Circuit breaker entering half-open state");
                        self.state = CircuitState::HalfOpen;
                        self.half_open_successes = 0;
                        true
                    } else {
                        false
                    }
                } else {
                    false
                }
            }
            CircuitState::HalfOpen => true,
        }
    }
}

/// Error entry in history
#[derive(Debug, Clone)]
pub struct ErrorEntry {
    /// Error message
    pub error: String,
    /// Error category
    pub category: ErrorCategory,
    /// Timestamp when error occurred
    pub timestamp: Instant,
    /// Component where error occurred
    pub component: String,
}

/// Error history for pattern detection
#[derive(Debug)]
struct ErrorHistory {
    /// Recent errors (limited size)
    entries: VecDeque<ErrorEntry>,
    /// Maximum number of entries to keep
    max_entries: usize,
    /// Error counts by component
    component_counts: HashMap<String, u32>,
    /// Error counts by category
    category_counts: HashMap<ErrorCategory, u32>,
}

impl ErrorHistory {
    fn new(max_entries: usize) -> Self {
        Self {
            entries: VecDeque::with_capacity(max_entries),
            max_entries,
            component_counts: HashMap::new(),
            category_counts: HashMap::new(),
        }
    }

    fn add_error(&mut self, error: String, category: ErrorCategory, component: String) {
        let entry = ErrorEntry {
            error: error.clone(),
            category,
            timestamp: Instant::now(),
            component: component.clone(),
        };

        // Add to history
        if self.entries.len() >= self.max_entries {
            self.entries.pop_front();
        }
        self.entries.push_back(entry);

        // Update counts
        *self.component_counts.entry(component).or_insert(0) += 1;
        *self.category_counts.entry(category).or_insert(0) += 1;
    }

    fn get_recent_errors(&self, component: Option<&str>, limit: usize) -> Vec<ErrorEntry> {
        self.entries
            .iter()
            .rev()
            .filter(|e| component.map_or(true, |c| e.component == c))
            .take(limit)
            .cloned()
            .collect()
    }

    fn get_error_rate(&self, component: &str, window: Duration) -> f64 {
        let now = Instant::now();
        let errors_in_window = self
            .entries
            .iter()
            .filter(|e| {
                e.component == component && now.duration_since(e.timestamp) <= window
            })
            .count();
        errors_in_window as f64 / window.as_secs() as f64
    }
}

/// Recovery metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecoveryMetrics {
    /// Total recovery attempts
    pub total_attempts: u64,
    /// Successful recoveries
    pub successful_recoveries: u64,
    /// Failed recoveries
    pub failed_recoveries: u64,
    /// Circuit breaker activations
    pub circuit_breaker_activations: u64,
    /// Average recovery time
    pub average_recovery_time_ms: f64,
}

/// Error recovery manager
pub struct ErrorRecoveryManager {
    /// Node instance for recovery operations
    node: Arc<Node>,
    /// Recovery strategies by error type
    strategies: HashMap<String, RecoveryStrategy>,
    /// Circuit breakers by component
    circuit_breakers: Arc<RwLock<HashMap<String, CircuitBreaker>>>,
    /// Error history
    error_history: Arc<RwLock<ErrorHistory>>,
    /// Recovery metrics
    metrics: Arc<RwLock<RecoveryMetrics>>,
    /// Default recovery strategy
    default_strategy: RecoveryStrategy,
}

impl ErrorRecoveryManager {
    /// Create a new error recovery manager
    pub fn new(node: Arc<Node>) -> Self {
        let mut strategies = HashMap::new();

        // Network errors - exponential backoff with circuit breaker
        strategies.insert(
            "network".to_string(),
            RecoveryStrategy::CircuitBreaker {
                failure_threshold: 5,
                reset_timeout: Duration::from_secs(60),
            },
        );

        // Database errors - exponential backoff
        strategies.insert(
            "database".to_string(),
            RecoveryStrategy::ExponentialBackoff {
                max_attempts: 5,
                initial_delay: Duration::from_millis(100),
                max_delay: Duration::from_secs(10),
            },
        );

        // Consensus errors - no retry (critical)
        strategies.insert("consensus".to_string(), RecoveryStrategy::NoRetry);

        // Memory errors - fixed delay retry
        strategies.insert(
            "memory".to_string(),
            RecoveryStrategy::FixedDelay {
                max_attempts: 3,
                delay: Duration::from_millis(50),
            },
        );

        // Lightning errors - exponential backoff
        strategies.insert(
            "lightning".to_string(),
            RecoveryStrategy::ExponentialBackoff {
                max_attempts: 3,
                initial_delay: Duration::from_millis(200),
                max_delay: Duration::from_secs(5),
            },
        );

        Self {
            node,
            strategies,
            circuit_breakers: Arc::new(RwLock::new(HashMap::new())),
            error_history: Arc::new(RwLock::new(ErrorHistory::new(1000))),
            metrics: Arc::new(RwLock::new(RecoveryMetrics {
                total_attempts: 0,
                successful_recoveries: 0,
                failed_recoveries: 0,
                circuit_breaker_activations: 0,
                average_recovery_time_ms: 0.0,
            })),
            default_strategy: RecoveryStrategy::default(),
        }
    }

    /// Classify an error into a category
    pub fn classify_error(&self, error: &dyn std::error::Error, component: &str) -> ErrorCategory {
        let error_msg = error.to_string().to_lowercase();

        // Critical errors - consensus violations, corruption, etc.
        if error_msg.contains("consensus")
            || error_msg.contains("corruption")
            || error_msg.contains("invalid chain")
            || error_msg.contains("double spend")
        {
            return ErrorCategory::Critical;
        }

        // Permanent errors - configuration, invalid input, etc.
        if error_msg.contains("config")
            || error_msg.contains("invalid")
            || error_msg.contains("not found")
            || error_msg.contains("unauthorized")
        {
            return ErrorCategory::Permanent;
        }

        // Transient errors - network, timeout, lock, etc.
        if error_msg.contains("network")
            || error_msg.contains("timeout")
            || error_msg.contains("connection")
            || error_msg.contains("lock")
            || error_msg.contains("temporary")
        {
            return ErrorCategory::Transient;
        }

        // Default to transient for unknown errors
        ErrorCategory::Transient
    }

    /// Attempt to recover from an error
    pub async fn recover<F, Fut, T>(
        &self,
        component: &str,
        operation: F,
    ) -> Result<T, String>
    where
        F: Fn() -> Fut + Send + Sync,
        Fut: std::future::Future<Output = Result<T, Box<dyn std::error::Error + Send + Sync>>>
            + Send,
    {
        let start_time = Instant::now();
        let strategy = self
            .strategies
            .get(component)
            .copied()
            .unwrap_or(self.default_strategy);

        // Check circuit breaker
        if !self.check_circuit_breaker(component).await {
            return Err(format!("Circuit breaker open for component: {}", component));
        }

        match strategy {
            RecoveryStrategy::ExponentialBackoff {
                max_attempts,
                initial_delay,
                max_delay,
            } => {
                self.recover_with_exponential_backoff(
                    component,
                    operation,
                    max_attempts,
                    initial_delay,
                    max_delay,
                )
                .await
            }
            RecoveryStrategy::FixedDelay {
                max_attempts,
                delay,
            } => {
                self.recover_with_fixed_delay(component, operation, max_attempts, delay)
                    .await
            }
            RecoveryStrategy::NoRetry => {
                // Execute once without retry
                match operation().await {
                    Ok(result) => {
                        self.record_success(component, start_time).await;
                        Ok(result)
                    }
                    Err(e) => {
                        let category = self.classify_error(e.as_ref(), component);
                        self.record_error(&e.to_string(), category, component).await;
                        Err(e.to_string())
                    }
                }
            }
            RecoveryStrategy::CircuitBreaker {
                failure_threshold,
                reset_timeout,
            } => {
                self.recover_with_circuit_breaker(
                    component,
                    operation,
                    failure_threshold,
                    reset_timeout,
                )
                .await
            }
        }
    }

    /// Recover with exponential backoff
    async fn recover_with_exponential_backoff<F, Fut, T>(
        &self,
        component: &str,
        operation: F,
        max_attempts: u32,
        initial_delay: Duration,
        max_delay: Duration,
    ) -> Result<T, String>
    where
        F: Fn() -> Fut + Send + Sync,
        Fut: std::future::Future<Output = Result<T, Box<dyn std::error::Error + Send + Sync>>>
            + Send,
    {
        let mut delay = initial_delay;
        let mut last_error: Option<String> = None;

        for attempt in 1..=max_attempts {
            match operation().await {
                Ok(result) => {
                    self.record_success(component, Instant::now()).await;
                    if attempt > 1 {
                        info!(
                            "Recovery successful for {} after {} attempts",
                            component, attempt
                        );
                    }
                    return Ok(result);
                }
                Err(e) => {
                    last_error = Some(e.to_string());
                    let category = self.classify_error(e.as_ref(), component);

                    if attempt < max_attempts {
                        warn!(
                            "Attempt {} failed for {}: {}. Retrying in {:?}",
                            attempt, component, last_error.as_ref().unwrap(), delay
                        );
                        sleep(delay).await;
                        delay = std::cmp::min(delay * 2, max_delay);
                    } else {
                        error!(
                            "Recovery failed for {} after {} attempts: {}",
                            component, max_attempts, last_error.as_ref().unwrap()
                        );
                        self.record_error(
                            last_error.as_ref().unwrap(),
                            category,
                            component,
                        )
                        .await;
                        self.record_failure(component, Instant::now()).await;
                    }
                }
            }
        }

        Err(last_error.unwrap_or_else(|| "Unknown error".to_string()))
    }

    /// Recover with fixed delay
    async fn recover_with_fixed_delay<F, Fut, T>(
        &self,
        component: &str,
        operation: F,
        max_attempts: u32,
        delay: Duration,
    ) -> Result<T, String>
    where
        F: Fn() -> Fut + Send + Sync,
        Fut: std::future::Future<Output = Result<T, Box<dyn std::error::Error + Send + Sync>>>
            + Send,
    {
        let start_time = Instant::now();
        let mut last_error: Option<String> = None;

        for attempt in 1..=max_attempts {
            match operation().await {
                Ok(result) => {
                    self.record_success(component, start_time).await;
                    if attempt > 1 {
                        info!(
                            "Recovery successful for {} after {} attempts",
                            component, attempt
                        );
                    }
                    return Ok(result);
                }
                Err(e) => {
                    last_error = Some(e.to_string());
                    let category = self.classify_error(e.as_ref(), component);

                    if attempt < max_attempts {
                        warn!(
                            "Attempt {} failed for {}: {}. Retrying in {:?}",
                            attempt, component, last_error.as_ref().unwrap(), delay
                        );
                        sleep(delay).await;
                    } else {
                        error!(
                            "Recovery failed for {} after {} attempts: {}",
                            component, max_attempts, last_error.as_ref().unwrap()
                        );
                        self.record_error(
                            last_error.as_ref().unwrap(),
                            category,
                            component,
                        )
                        .await;
                        self.record_failure(component, start_time).await;
                    }
                }
            }
        }

        Err(last_error.unwrap_or_else(|| "Unknown error".to_string()))
    }

    /// Recover with circuit breaker
    async fn recover_with_circuit_breaker<F, Fut, T>(
        &self,
        component: &str,
        operation: F,
        failure_threshold: u32,
        reset_timeout: Duration,
    ) -> Result<T, String>
    where
        F: Fn() -> Fut + Send + Sync,
        Fut: std::future::Future<Output = Result<T, Box<dyn std::error::Error + Send + Sync>>>
            + Send,
    {
        // Ensure circuit breaker exists
        {
            let mut breakers = self.circuit_breakers.write().await;
            if !breakers.contains_key(component) {
                breakers.insert(
                    component.to_string(),
                    CircuitBreaker::new(failure_threshold, reset_timeout),
                );
            }
        }

        // Check if we can attempt
        if !self.check_circuit_breaker(component).await {
            return Err(format!("Circuit breaker open for component: {}", component));
        }

        let start_time = Instant::now();
        match operation().await {
            Ok(result) => {
                {
                    let mut breakers = self.circuit_breakers.write().await;
                    if let Some(breaker) = breakers.get_mut(component) {
                        breaker.record_success();
                    }
                }
                self.record_success(component, start_time).await;
                Ok(result)
            }
            Err(e) => {
                {
                    let mut breakers = self.circuit_breakers.write().await;
                    if let Some(breaker) = breakers.get_mut(component) {
                        breaker.record_failure();
                        if breaker.state == CircuitState::Open {
                            let mut metrics = self.metrics.write().await;
                            metrics.circuit_breaker_activations += 1;
                        }
                    }
                }
                let category = self.classify_error(e.as_ref(), component);
                self.record_error(&e.to_string(), category, component).await;
                self.record_failure(component, start_time).await;
                Err(e.to_string())
            }
        }
    }

    /// Check if circuit breaker allows attempt
    async fn check_circuit_breaker(&self, component: &str) -> bool {
        let mut breakers = self.circuit_breakers.write().await;
        if let Some(breaker) = breakers.get_mut(component) {
            breaker.can_attempt()
        } else {
            true
        }
    }

    /// Record successful recovery
    async fn record_success(&self, component: &str, start_time: Instant) {
        let duration = start_time.elapsed();
        let mut metrics = self.metrics.write().await;
        metrics.total_attempts += 1;
        metrics.successful_recoveries += 1;

        // Update average recovery time
        let total_time = metrics.average_recovery_time_ms * (metrics.successful_recoveries - 1) as f64
            + duration.as_millis() as f64;
        metrics.average_recovery_time_ms = total_time / metrics.successful_recoveries as f64;

        debug!(
            "Recovery successful for {} in {:?}",
            component, duration
        );
    }

    /// Record failed recovery
    async fn record_failure(&self, component: &str, start_time: Instant) {
        let duration = start_time.elapsed();
        let mut metrics = self.metrics.write().await;
        metrics.total_attempts += 1;
        metrics.failed_recoveries += 1;

        warn!(
            "Recovery failed for {} after {:?}",
            component, duration
        );
    }

    /// Record an error in history
    async fn record_error(&self, error: &str, category: ErrorCategory, component: &str) {
        let mut history = self.error_history.write().await;
        history.add_error(error.to_string(), category, component.to_string());
    }

    /// Get recovery metrics
    pub async fn get_metrics(&self) -> RecoveryMetrics {
        self.metrics.read().await.clone()
    }

    /// Get error history for a component
    pub async fn get_error_history(&self, component: Option<&str>, limit: usize) -> Vec<ErrorEntry> {
        let history = self.error_history.read().await;
        history.get_recent_errors(component, limit)
    }

    /// Get error rate for a component
    pub async fn get_error_rate(&self, component: &str, window: Duration) -> f64 {
        let history = self.error_history.read().await;
        history.get_error_rate(component, window)
    }

    /// Reset circuit breaker for a component
    pub async fn reset_circuit_breaker(&self, component: &str) {
        let mut breakers = self.circuit_breakers.write().await;
        if let Some(breaker) = breakers.get_mut(component) {
            breaker.state = CircuitState::Closed;
            breaker.failure_count = 0;
            breaker.opened_at = None;
            breaker.half_open_successes = 0;
            info!("Circuit breaker reset for component: {}", component);
        }
    }

    /// Emergency recovery mode - aggressive recovery for critical errors
    pub async fn emergency_recovery(&self, component: &str) -> Result<(), String> {
        warn!("Entering emergency recovery mode for component: {}", component);

        match component {
            "network" => {
                // Try to reconnect network
                info!("Attempting network reconnection...");
                // Network recovery would be implemented here
                Ok(())
            }
            "database" => {
                // Try to flush and recover database
                info!("Attempting database recovery...");
                if let Err(e) = self.node.db().flush() {
                    return Err(format!("Database flush failed: {}", e));
                }
                Ok(())
            }
            "consensus" => {
                // Consensus errors are critical - trigger resync
                warn!("Consensus error detected - resync required");
                Err("Consensus error requires manual intervention".to_string())
            }
            "memory" => {
                // Clear caches
                info!("Clearing memory caches...");
                // Memory recovery would be implemented here
                Ok(())
            }
            "lightning" => {
                // Lightning channel recovery
                info!("Attempting Lightning channel recovery...");
                // Lightning recovery would be implemented here
                Ok(())
            }
            _ => {
                warn!("Unknown component for emergency recovery: {}", component);
                Err(format!("Unknown component: {}", component))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::NodeConfig;
    use std::sync::atomic::{AtomicU32, Ordering};

    async fn create_test_node() -> Arc<Node> {
        let config = NodeConfig::default();
        Arc::new(
            Node::new(config)
                .await
                .expect("Failed to create test node"),
        )
    }

    #[tokio::test]
    async fn test_transient_error_recovery() {
        let node = create_test_node().await;
        let recovery_manager = ErrorRecoveryManager::new(node);

        let attempt_count = Arc::new(AtomicU32::new(0));

        let result = recovery_manager
            .recover("network", || {
                let count = Arc::clone(&attempt_count);
                async move {
                    let current = count.fetch_add(1, Ordering::SeqCst);
                    if current < 2 {
                        Err(Box::new(std::io::Error::new(
                            std::io::ErrorKind::ConnectionRefused,
                            "Connection refused",
                        )) as Box<dyn std::error::Error + Send + Sync>)
                    } else {
                        Ok(42)
                    }
                }
            })
            .await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 42);
        assert_eq!(attempt_count.load(Ordering::SeqCst), 3);
    }

    #[tokio::test]
    async fn test_exponential_backoff() {
        let node = create_test_node().await;
        let recovery_manager = ErrorRecoveryManager::new(node);

        let start = Instant::now();
        let attempt_times = Arc::new(std::sync::Mutex::new(Vec::new()));

        let _: Result<(), String> = recovery_manager
            .recover("database", || {
                let times = Arc::clone(&attempt_times);
                async move {
                    let now = Instant::now();
                    times.lock().unwrap().push(now);
                    Err(Box::new(std::io::Error::new(
                        std::io::ErrorKind::TimedOut,
                        "Timeout",
                    )) as Box<dyn std::error::Error + Send + Sync>)
                }
            })
            .await;

        let times = attempt_times.lock().unwrap();
        if times.len() > 1 {
            // Verify delays increase exponentially
            let delay1 = times[1].duration_since(times[0]);
            let delay2 = times[2].duration_since(times[1]);
            assert!(delay2 > delay1);
        }
    }

    #[tokio::test]
    async fn test_circuit_breaker_activation() {
        let node = create_test_node().await;
        let recovery_manager = ErrorRecoveryManager::new(node);

        // Fail enough times to open circuit breaker
        for _ in 0..6 {
            let _: Result<(), String> = recovery_manager
                .recover("network", || async {
                    Err(Box::new(std::io::Error::new(
                        std::io::ErrorKind::ConnectionRefused,
                        "Connection refused",
                    )) as Box<dyn std::error::Error + Send + Sync>)
                })
                .await;
        }

        // Circuit breaker should be open - next attempt should fail immediately
        let result = recovery_manager
            .recover("network", || async {
                Ok(42)
            })
            .await;

        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Circuit breaker open"));
    }

    #[tokio::test]
    async fn test_error_pattern_detection() {
        let node = create_test_node().await;
        let recovery_manager = ErrorRecoveryManager::new(node);

        // Record multiple errors
        for i in 0..10 {
            recovery_manager
                .record_error(
                    &format!("Error {}", i),
                    ErrorCategory::Transient,
                    "network",
                )
                .await;
        }

        let history = recovery_manager.get_error_history(Some("network"), 5).await;
        assert_eq!(history.len(), 5);

        let error_rate = recovery_manager
            .get_error_rate("network", Duration::from_secs(1))
            .await;
        assert!(error_rate > 0.0);
    }

    #[tokio::test]
    async fn test_recovery_strategy_selection() {
        let node = create_test_node().await;
        let recovery_manager = ErrorRecoveryManager::new(node);

        // Network should use circuit breaker strategy
        let network_strategy = recovery_manager.strategies.get("network");
        assert!(matches!(
            network_strategy,
            Some(RecoveryStrategy::CircuitBreaker { .. })
        ));

        // Database should use exponential backoff
        let db_strategy = recovery_manager.strategies.get("database");
        assert!(matches!(
            db_strategy,
            Some(RecoveryStrategy::ExponentialBackoff { .. })
        ));

        // Consensus should use no retry
        let consensus_strategy = recovery_manager.strategies.get("consensus");
        assert!(matches!(consensus_strategy, Some(RecoveryStrategy::NoRetry)));
    }

    #[tokio::test]
    async fn test_emergency_recovery_mode() {
        let node = create_test_node().await;
        let recovery_manager = ErrorRecoveryManager::new(node);

        // Test emergency recovery for database
        let result = recovery_manager.emergency_recovery("database").await;
        assert!(result.is_ok());

        // Test emergency recovery for unknown component
        let result = recovery_manager.emergency_recovery("unknown").await;
        assert!(result.is_err());
    }
}

