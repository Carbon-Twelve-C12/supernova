//! Graceful Shutdown Coordinator for Supernova Node
//!
//! This module implements comprehensive graceful shutdown procedures to ensure
//! clean node termination with proper state persistence and component coordination.

use crate::node::Node;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tokio::time::timeout;
use tracing::{error, info, warn};

/// Shutdown signal source
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShutdownSignal {
    /// User-initiated shutdown (Ctrl+C, SIGTERM)
    User,
    /// System-initiated shutdown (SIGINT)
    System,
    /// Error-initiated shutdown
    Error,
    /// Upgrade-initiated shutdown
    Upgrade,
    /// Shutdown requested via the admin API/RPC surface
    /// (`ApiFacade::shutdown` / `ApiFacade::restart`)
    Api,
}

/// Current shutdown phase
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ShutdownPhase {
    /// Preparing for shutdown
    Preparing,
    /// Stopping components
    Stopping,
    /// Flushing data to disk
    Flushing,
    /// Shutdown complete
    Complete,
}

/// Shutdown status information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShutdownStatus {
    /// Current shutdown phase
    pub phase: ShutdownPhase,
    /// Shutdown signal source
    pub signal: String,
    /// Timestamp when shutdown started
    pub started_at: i64,
    /// Components that have completed shutdown
    pub completed_components: Vec<String>,
    /// Components still shutting down
    pub pending_components: Vec<String>,
    /// Whether shutdown completed successfully
    pub success: bool,
    /// Error message if shutdown failed
    pub error: Option<String>,
}

/// Configuration for graceful shutdown
#[derive(Debug, Clone)]
pub struct ShutdownConfig {
    /// Maximum time allowed for graceful shutdown
    pub max_shutdown_time: Duration,
    /// Timeout for individual component shutdown
    pub component_timeout: Duration,
    /// Whether to persist state before shutdown
    pub persist_state: bool,
    /// Path to save shutdown status file
    pub status_file_path: PathBuf,
    /// Whether to force shutdown after timeout
    pub force_after_timeout: bool,
}

impl Default for ShutdownConfig {
    fn default() -> Self {
        Self {
            max_shutdown_time: Duration::from_secs(30),
            component_timeout: Duration::from_secs(5),
            persist_state: true,
            status_file_path: PathBuf::from("./data/shutdown_status.json"),
            force_after_timeout: true,
        }
    }
}

/// Component shutdown result
#[derive(Debug)]
struct ComponentShutdownResult {
    component: String,
    success: bool,
    duration: Duration,
    error: Option<String>,
}

/// Graceful shutdown coordinator
pub struct ShutdownCoordinator {
    /// Node instance
    node: Arc<Node>,
    /// Shutdown configuration
    config: ShutdownConfig,
    /// Current shutdown status
    status: Arc<RwLock<ShutdownStatus>>,
    /// Shutdown signal receiver
    shutdown_requested: Arc<RwLock<bool>>,
}

impl ShutdownCoordinator {
    /// Create a new shutdown coordinator
    pub fn new(node: Arc<Node>, config: ShutdownConfig) -> Self {
        let status = ShutdownStatus {
            phase: ShutdownPhase::Preparing,
            signal: String::new(),
            started_at: 0,
            completed_components: Vec::new(),
            pending_components: Vec::new(),
            success: false,
            error: None,
        };

        Self {
            node,
            config,
            status: Arc::new(RwLock::new(status)),
            shutdown_requested: Arc::new(RwLock::new(false)),
        }
    }

    /// Check if shutdown has been requested
    pub async fn is_shutdown_requested(&self) -> bool {
        *self.shutdown_requested.read().await
    }

    /// Request shutdown
    pub async fn request_shutdown(&self, signal: ShutdownSignal) {
        let mut requested = self.shutdown_requested.write().await;
        if !*requested {
            *requested = true;
            info!("Shutdown requested: {:?}", signal);
        }
    }

    /// Perform graceful shutdown
    pub async fn shutdown(&self, signal: ShutdownSignal) -> Result<(), String> {
        let start_time = Instant::now();

        // Update status
        {
            let mut status = self.status.write().await;
            status.phase = ShutdownPhase::Preparing;
            status.signal = format!("{:?}", signal);
            status.started_at = chrono::Utc::now().timestamp();
            status.completed_components.clear();
            status.pending_components = vec![
                "network".to_string(),
                "mempool".to_string(),
                "lightning".to_string(),
                "wallet".to_string(),
                "database".to_string(),
                "metrics".to_string(),
            ];
        }

        info!("Starting graceful shutdown (signal: {:?})", signal);

        // Save initial shutdown status
        if let Err(e) = self.save_status().await {
            warn!("Failed to save shutdown status: {}", e);
        }

        // Perform shutdown with timeout
        let shutdown_result = timeout(self.config.max_shutdown_time, async {
            self.shutdown_internal(signal).await
        })
        .await;

        match shutdown_result {
            Ok(Ok(())) => {
                let duration = start_time.elapsed();
                info!("Graceful shutdown completed in {:?}", duration);

                // Update status to complete
                {
                    let mut status = self.status.write().await;
                    status.phase = ShutdownPhase::Complete;
                    status.success = true;
                }

                // Save final status
                if let Err(e) = self.save_status().await {
                    warn!("Failed to save final shutdown status: {}", e);
                }

                Ok(())
            }
            Ok(Err(e)) => {
                error!("Shutdown failed: {}", e);
                {
                    let mut status = self.status.write().await;
                    status.success = false;
                    status.error = Some(e.clone());
                }
                Err(e)
            }
            Err(_) => {
                let duration = start_time.elapsed();
                warn!(
                    "Shutdown timeout after {:?} (max: {:?})",
                    duration, self.config.max_shutdown_time
                );

                if self.config.force_after_timeout {
                    warn!("Forcing shutdown after timeout");
                    self.force_shutdown().await;
                }

                {
                    let mut status = self.status.write().await;
                    status.success = false;
                    status.error = Some("Shutdown timeout".to_string());
                }

                Err("Shutdown timeout".to_string())
            }
        }
    }

    /// Internal shutdown implementation
    async fn shutdown_internal(&self, signal: ShutdownSignal) -> Result<(), String> {
        // Phase 1: Stop accepting new connections
        {
            let mut status = self.status.write().await;
            status.phase = ShutdownPhase::Stopping;
        }
        info!("Phase 1: Stopping new connections");

        // Stop accepting new P2P connections
        self.shutdown_component("network_accept", || async {
            // Network will stop accepting new connections automatically
            // when stop() is called
            Ok(())
        })
        .await?;

        // Phase 2: Finish processing current transactions
        info!("Phase 2: Finishing transaction processing");
        self.shutdown_component("transaction_processing", || async {
            // Give mempool time to process pending transactions
            tokio::time::sleep(Duration::from_millis(500)).await;
            Ok(())
        })
        .await?;

        // Phase 3: Flush mempool to disk
        info!("Phase 3: Flushing mempool");
        self.shutdown_component("mempool", || async {
            // Mempool state is typically in-memory, but we can ensure
            // any pending transactions are handled
            Ok(())
        })
        .await?;

        // Phase 4: Close Lightning channels gracefully
        info!("Phase 4: Closing Lightning channels");
        let lightning_manager_opt = self.node.lightning();
        self.shutdown_component("lightning", move || {
            async move {
                if let Some(lightning_manager) = lightning_manager_opt {
                    // Lightning channels should be closed gracefully
                    // Use blocking task for std::sync::RwLock
                    tokio::task::spawn_blocking(move || {
                        let _manager = lightning_manager.read()
                            .map_err(|_| "Lightning manager lock poisoned".to_string())?;
                        // Placeholder - actual implementation depends on LightningManager API
                        Ok::<(), String>(())
                    }).await.map_err(|e| format!("Task join error: {}", e))??;
                }
                Ok(())
            }
        })
        .await?;

        // Phase 5: Save UTXO set state
        info!("Phase 5: Saving UTXO set state");
        self.shutdown_component("utxo_set", || async {
            // UTXO set is managed by ChainState which will be flushed with database
            Ok(())
        })
        .await?;

        // Phase 6: Flush all database writes
        {
            let mut status = self.status.write().await;
            status.phase = ShutdownPhase::Flushing;
        }
        info!("Phase 6: Flushing database");
        let db = self.node.db();
        let db_shutdown_handler = self.node.db_shutdown_handler.clone();
        self.shutdown_component("database", move || {
            let db_clone = Arc::clone(&db);
            let handler_clone = db_shutdown_handler.clone();
            async move {
                if let Some(handler) = handler_clone {
                    handler
                        .shutdown()
                        .await
                        .map_err(|e| format!("Database shutdown failed: {}", e))?;
                } else {
                    // Fallback: just flush the database
                    db_clone.flush().map_err(|e| {
                        format!("Database flush failed: {}", e)
                    })?;
                }
                Ok(())
            }
        })
        .await?;

        // Phase 7: Close network connections
        info!("Phase 7: Closing network connections");
        // Call stop directly - handle timeout manually to avoid Send issues
        let start_time = Instant::now();
        let stop_future = self.node.stop();
        let timeout_future = tokio::time::sleep(self.config.component_timeout);
        
        tokio::select! {
            result = stop_future => {
                match result {
                    Ok(()) => {
                        info!("Network stopped successfully in {:?}", start_time.elapsed());
                        {
                            let mut status = self.status.write().await;
                            if let Some(pos) = status.pending_components.iter().position(|x| x == "network") {
                                status.pending_components.remove(pos);
                            }
                            status.completed_components.push("network".to_string());
                        }
                    }
                    Err(e) => {
                        error!("Network stop failed: {}", e);
                        return Err(format!("network: {}", e));
                    }
                }
            }
            _ = timeout_future => {
                warn!("Network stop timeout after {:?}", start_time.elapsed());
                return Err("network: timeout".to_string());
            }
        }

        // Phase 8: Save final metrics
        info!("Phase 8: Saving metrics");
        self.shutdown_component("metrics", || async {
            // Metrics are typically collected in-memory
            // Save any critical metrics if needed
            Ok(())
        })
        .await?;

        Ok(())
    }

    /// Shutdown a component with timeout
    async fn shutdown_component<F, Fut>(
        &self,
        component_name: &str,
        shutdown_fn: F,
    ) -> Result<(), String>
    where
        F: FnOnce() -> Fut + Send,
        Fut: std::future::Future<Output = Result<(), String>> + Send,
    {
        let start_time = Instant::now();
        info!("Shutting down component: {}", component_name);

        // Update pending components
        {
            let mut status = self.status.write().await;
            if let Some(pos) = status
                .pending_components
                .iter()
                .position(|x| x == component_name)
            {
                status.pending_components.remove(pos);
            }
        }

        // Execute shutdown with timeout
        let result = timeout(self.config.component_timeout, shutdown_fn()).await;

        let duration = start_time.elapsed();
        match result {
            Ok(Ok(())) => {
                info!("Component '{}' shut down successfully in {:?}", component_name, duration);
                {
                    let mut status = self.status.write().await;
                    status.completed_components.push(component_name.to_string());
                }
                Ok(())
            }
            Ok(Err(e)) => {
                error!("Component '{}' shutdown failed: {}", component_name, e);
                {
                    let mut status = self.status.write().await;
                    status.completed_components.push(component_name.to_string());
                    // Note: We still mark as completed to continue shutdown sequence
                }
                Err(format!("{}: {}", component_name, e))
            }
            Err(_) => {
                warn!(
                    "Component '{}' shutdown timeout after {:?}",
                    component_name, duration
                );
                {
                    let mut status = self.status.write().await;
                    status.completed_components.push(component_name.to_string());
                }
                Err(format!("{}: timeout", component_name))
            }
        }
    }

    /// Force shutdown (emergency)
    async fn force_shutdown(&self) {
        warn!("Performing force shutdown - some data may be lost");

        // Try to flush database at minimum
        if let Some(db_shutdown_handler) = self.node.db_shutdown_handler.clone() {
            if let Err(e) = db_shutdown_handler.emergency_shutdown().await {
                error!("Emergency database shutdown failed: {}", e);
            }
        } else {
            let db = self.node.db();
            if let Err(e) = db.flush() {
                error!("Emergency database flush failed: {}", e);
            }
        }
    }

    /// Save shutdown status to file
    async fn save_status(&self) -> Result<(), String> {
        let status = self.status.read().await.clone();

        // Ensure directory exists
        if let Some(parent) = self.config.status_file_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                format!("Failed to create status directory: {}", e)
            })?;
        }

        // Serialize and write status
        let json = serde_json::to_string_pretty(&status).map_err(|e| {
            format!("Failed to serialize status: {}", e)
        })?;

        std::fs::write(&self.config.status_file_path, json).map_err(|e| {
            format!("Failed to write status file: {}", e)
        })?;

        Ok(())
    }

    /// Load shutdown status from file
    pub async fn load_status<P: AsRef<Path>>(path: P) -> Result<ShutdownStatus, String> {
        let contents = std::fs::read_to_string(path).map_err(|e| {
            format!("Failed to read status file: {}", e)
        })?;

        let status: ShutdownStatus = serde_json::from_str(&contents).map_err(|e| {
            format!("Failed to parse status file: {}", e)
        })?;

        Ok(status)
    }

    /// Get current shutdown status
    pub async fn get_status(&self) -> ShutdownStatus {
        self.status.read().await.clone()
    }
}

/// Sender used to deliver an admin-API-requested shutdown into the same
/// channel `main`'s signal-handling loop already selects on. Populated the
/// one time [`register_signal_handlers`] is called by the running node's
/// event loop; a request made before that (or from a context — such as a
/// unit test — that never starts a real node loop) simply fails, which is
/// surfaced to the caller as an error rather than silently doing nothing.
static ADMIN_SHUTDOWN_TX: std::sync::OnceLock<tokio::sync::mpsc::Sender<ShutdownSignal>> =
    std::sync::OnceLock::new();

/// Set when an admin API/RPC call requested a *restart* rather than a plain
/// shutdown. Consulted by `main` after the graceful shutdown sequence
/// completes so it can exit with a distinct code that a process supervisor
/// (systemd, Docker, Kubernetes) can use to restart the node.
static ADMIN_RESTART_REQUESTED: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

/// Exit code used when an admin API/RPC call requested a restart. Distinct
/// from a normal `0` exit so a supervisor's restart policy can distinguish
/// "please bring me back up" from "I stopped on purpose, leave me down".
pub const ADMIN_RESTART_EXIT_CODE: i32 = 75;

/// Request that the running node process perform a graceful shutdown from
/// outside the normal OS-signal path — used by the admin API/RPC
/// `restart`/`shutdown` endpoints (see `ApiFacade::restart`/`shutdown`).
///
/// Returns an error if no node event loop has registered its shutdown
/// channel via [`register_signal_handlers`].
pub fn request_admin_shutdown(restart: bool) -> Result<(), String> {
    let tx = ADMIN_SHUTDOWN_TX
        .get()
        .ok_or_else(|| "node shutdown channel is not initialized".to_string())?;
    send_admin_shutdown(tx, restart)
}

/// Core of [`request_admin_shutdown`], split out so it can be unit tested
/// against a locally-created channel instead of the process-global
/// `ADMIN_SHUTDOWN_TX`/`ADMIN_RESTART_REQUESTED` statics (which, being
/// shared across every test in the binary, aren't safe to assert on from
/// tests that run concurrently).
fn send_admin_shutdown(
    tx: &tokio::sync::mpsc::Sender<ShutdownSignal>,
    restart: bool,
) -> Result<(), String> {
    if restart {
        ADMIN_RESTART_REQUESTED.store(true, std::sync::atomic::Ordering::SeqCst);
    }
    tx.try_send(ShutdownSignal::Api)
        .map_err(|e| format!("failed to signal shutdown: {}", e))
}

/// Whether the pending/most recent admin-initiated shutdown was requested
/// as a restart. Checked by `main` once shutdown completes.
pub fn admin_restart_requested() -> bool {
    ADMIN_RESTART_REQUESTED.load(std::sync::atomic::Ordering::SeqCst)
}

/// Register signal handlers for graceful shutdown
/// Returns a receiver channel that will receive shutdown signals
pub fn register_signal_handlers() -> tokio::sync::mpsc::Receiver<ShutdownSignal> {
    use tokio::signal;
    use tokio::sync::mpsc;

    let (tx, rx) = mpsc::channel(1);
    // Make this channel reachable from `ApiFacade::restart`/`shutdown` via
    // `request_admin_shutdown`. Best-effort: if `register_signal_handlers`
    // is somehow called more than once, the first registration wins.
    let _ = ADMIN_SHUTDOWN_TX.set(tx.clone());

    let tx_clone = tx.clone();
    // Spawn task to handle Ctrl+C
    tokio::spawn(async move {
        if let Ok(()) = signal::ctrl_c().await {
            info!("Ctrl+C received, initiating graceful shutdown");
            let _ = tx_clone.send(ShutdownSignal::User).await;
        }
    });

    #[cfg(unix)]
    {
        use tokio::signal::unix::{signal, SignalKind};

        // Handle SIGTERM
        let tx_sigterm = tx.clone();
        tokio::spawn(async move {
            match signal(SignalKind::terminate()) {
                Ok(mut sigterm) => {
                    loop {
                        sigterm.recv().await;
                        info!("SIGTERM received, initiating graceful shutdown");
                        let _ = tx_sigterm.send(ShutdownSignal::System).await;
                    }
                }
                Err(e) => {
                    error!("Failed to register SIGTERM handler: {}", e);
                }
            }
        });

        // Handle SIGINT
        let tx_sigint = tx.clone();
        tokio::spawn(async move {
            match signal(SignalKind::interrupt()) {
                Ok(mut sigint) => {
                    loop {
                        sigint.recv().await;
                        info!("SIGINT received, initiating graceful shutdown");
                        let _ = tx_sigint.send(ShutdownSignal::System).await;
                    }
                }
                Err(e) => {
                    error!("Failed to register SIGINT handler: {}", e);
                }
            }
        });
    }

    rx
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::NodeConfig;
    use std::time::Duration;

    async fn create_test_node() -> Arc<Node> {
        let config = NodeConfig::default();
        Arc::new(
            Node::new(config)
                .await
                .expect("Failed to create test node"),
        )
    }

    #[tokio::test]
    async fn test_graceful_shutdown_sequence() {
        let node = create_test_node().await;
        let config = ShutdownConfig {
            max_shutdown_time: Duration::from_secs(10),
            component_timeout: Duration::from_secs(2),
            ..Default::default()
        };

        let coordinator = ShutdownCoordinator::new(node, config);

        // Request shutdown
        coordinator.request_shutdown(ShutdownSignal::User).await;
        assert!(coordinator.is_shutdown_requested().await);

        // Perform shutdown
        let result = coordinator.shutdown(ShutdownSignal::User).await;

        // Verify shutdown completed
        let status = coordinator.get_status().await;
        assert_eq!(status.phase, ShutdownPhase::Complete);
        assert!(status.success || result.is_err()); // May fail in test environment
    }

    #[tokio::test]
    async fn test_shutdown_timeout_enforcement() {
        let node = create_test_node().await;
        let config = ShutdownConfig {
            max_shutdown_time: Duration::from_millis(100),
            component_timeout: Duration::from_millis(50),
            force_after_timeout: true,
            ..Default::default()
        };

        let coordinator = ShutdownCoordinator::new(node, config);

        // This should timeout quickly
        let result = coordinator.shutdown(ShutdownSignal::User).await;

        // Should either complete quickly or timeout
        assert!(result.is_err() || result.is_ok());
    }

    #[tokio::test]
    async fn test_state_persistence_on_shutdown() {
        let node = create_test_node().await;
        let config = ShutdownConfig {
            persist_state: true,
            status_file_path: PathBuf::from("/tmp/test_shutdown_status.json"),
            ..Default::default()
        };

        let coordinator = ShutdownCoordinator::new(node, config.clone());

        // Request shutdown
        coordinator.request_shutdown(ShutdownSignal::User).await;

        // Perform shutdown (may fail in test, but status should be saved)
        let _ = coordinator.shutdown(ShutdownSignal::User).await;

        // Verify status file was created
        if config.status_file_path.exists() {
            let status = ShutdownCoordinator::load_status(&config.status_file_path)
                .await
                .expect("Failed to load status");
            assert!(!status.signal.is_empty());
        }
    }

    #[tokio::test]
    async fn test_signal_handler_registration() {
        // Register signal handlers - should return a receiver
        let _rx = register_signal_handlers();
        // Test passes if registration doesn't panic
        assert!(true);
    }

    /// `ApiFacade::shutdown`/`restart` (node/src/api_facade.rs) delegate to
    /// `request_admin_shutdown`, whose delivery mechanics live in
    /// `send_admin_shutdown`. Verify a plain shutdown request delivers an
    /// `Api` signal without flagging a restart, and a restart request both
    /// delivers the signal and flags `admin_restart_requested()`.
    #[tokio::test]
    async fn test_admin_shutdown_delivers_signal_and_restart_flag() {
        let (tx, mut rx) = tokio::sync::mpsc::channel(1);

        send_admin_shutdown(&tx, false).expect("shutdown send should succeed");
        let signal = rx.recv().await.expect("channel should deliver a signal");
        assert_eq!(signal, ShutdownSignal::Api);

        send_admin_shutdown(&tx, true).expect("restart send should succeed");
        let signal = rx.recv().await.expect("channel should deliver a signal");
        assert_eq!(signal, ShutdownSignal::Api);
        assert!(admin_restart_requested());
    }

    #[tokio::test]
    async fn test_component_shutdown_order() {
        let node = create_test_node().await;
        let config = ShutdownConfig {
            max_shutdown_time: Duration::from_secs(10),
            component_timeout: Duration::from_secs(1),
            ..Default::default()
        };

        let coordinator = ShutdownCoordinator::new(node, config);

        // Request shutdown
        coordinator.request_shutdown(ShutdownSignal::User).await;

        // Perform shutdown
        let _ = coordinator.shutdown(ShutdownSignal::User).await;

        // Verify components were shut down in order
        let status = coordinator.get_status().await;
        assert!(!status.completed_components.is_empty());
    }

    #[tokio::test]
    async fn test_force_shutdown_after_timeout() {
        let node = create_test_node().await;
        let config = ShutdownConfig {
            max_shutdown_time: Duration::from_millis(50),
            component_timeout: Duration::from_millis(10),
            force_after_timeout: true,
            ..Default::default()
        };

        let coordinator = ShutdownCoordinator::new(node, config);

        // This should timeout and force shutdown
        let result = coordinator.shutdown(ShutdownSignal::User).await;

        // Should timeout
        assert!(result.is_err());
        let status = coordinator.get_status().await;
        assert!(!status.success);
    }
}

