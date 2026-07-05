//! Thread-safe API facade for the Node
//!
//! This module provides a thread-safe wrapper around the Node that can be safely
//! shared across threads in the API server.

use crate::api::types::*;
use crate::environmental::EnvironmentalMonitor;
use crate::mempool::TransactionPool;
use crate::network::NetworkProxy;
use crate::node::{Node, NodeError};
use crate::storage::{BlockchainDB, ChainState};
use crate::wallet_manager::WalletManager;
use supernova_core::types::transaction::Transaction;
use std::sync::Arc;
use std::sync::RwLock as StdRwLock;
use sysinfo::System;

/// Thread-safe API facade that wraps the Node
pub struct ApiFacade {
    /// Configuration
    config: Arc<StdRwLock<crate::config::NodeConfig>>,
    /// Blockchain database
    db: Arc<BlockchainDB>,
    /// Chain state
    chain_state: Arc<StdRwLock<ChainState>>,
    /// Transaction mempool
    mempool: Arc<TransactionPool>,
    /// Network proxy (thread-safe)
    network: Arc<NetworkProxy>,
    /// Peer ID
    peer_id: libp2p::PeerId,
    /// Start time
    start_time: std::time::Instant,
    /// Lightning manager (if enabled)
    lightning_manager: Option<Arc<StdRwLock<supernova_core::lightning::LightningManager>>>,
    /// Wallet manager (quantum-resistant wallet)
    wallet_manager: Arc<StdRwLock<WalletManager>>,
    /// Environmental monitor providing real energy/carbon telemetry
    environmental: Arc<EnvironmentalMonitor>,
}

// Ensure ApiFacade is Send + Sync. If this fails to compile, a newly added
// field is not Send+Sync — wrap it in `Arc<Mutex<_>>` or `Arc<RwLock<_>>`.
static_assertions::assert_impl_all!(ApiFacade: Send, Sync);

impl ApiFacade {
    /// Create a new API facade from a Node.
    ///
    /// Returns an error instead of panicking if the node is missing a wallet
    /// manager and a fallback cannot be created (for example, when the
    /// filesystem for the fallback wallet path is unavailable). Callers —
    /// typically `ApiServer::new` → the node bootstrap path in `main.rs` —
    /// are responsible for translating this into a non-zero exit code.
    pub fn new(node: &Node) -> Result<Self, NodeError> {
        let wallet_manager = match node.get_wallet_manager() {
            Some(wm) => wm,
            None => {
                tracing::warn!(
                    "Node has no wallet manager — initializing fallback at ./wallet_fallback"
                );
                let wallet_path = std::path::PathBuf::from("./wallet_fallback");
                // Use the same passphrase-resolution policy as the primary
                // wallet bootstrap in `Node::new`. On Production this errors
                // out unless `SUPERNOVA_WALLET_PASSPHRASE` is set — refusing
                // the fallback is correct: silently initialising it with a
                // published default would defeat the keystore encryption.
                let cfg = node.config();
                let cfg_guard = cfg.read().map_err(|_| {
                    NodeError::General("config lock poisoned".to_string())
                })?;
                let passphrase = crate::wallet_manager::resolve_wallet_passphrase(&cfg_guard)
                    .map_err(|e| {
                        NodeError::General(format!("fallback wallet passphrase: {e}"))
                    })?;
                drop(cfg_guard);
                let wm = WalletManager::new(
                    wallet_path,
                    &passphrase,
                    node.db(),
                    node.chain_state(),
                    node.mempool(),
                    node.network_proxy(),
                )
                .map_err(|e| {
                    NodeError::General(format!("fallback wallet manager init failed: {e}"))
                })?;
                Arc::new(StdRwLock::new(wm))
            }
        };

        Ok(Self {
            config: node.config(),
            db: node.db(),
            chain_state: node.chain_state(),
            mempool: node.mempool(),
            network: node.network_proxy(),
            peer_id: node.peer_id,
            start_time: node.start_time,
            lightning_manager: node.lightning(),
            wallet_manager,
            environmental: Arc::new(EnvironmentalMonitor::new()),
        })
    }

    /// Get storage (blockchain database)
    pub fn storage(&self) -> Arc<BlockchainDB> {
        Arc::clone(&self.db)
    }

    /// Get chain state
    pub fn chain_state(&self) -> Arc<StdRwLock<ChainState>> {
        Arc::clone(&self.chain_state)
    }

    /// Get mempool
    pub fn mempool(&self) -> Arc<TransactionPool> {
        Arc::clone(&self.mempool)
    }

    /// Get config
    pub fn config(&self) -> Arc<StdRwLock<crate::config::NodeConfig>> {
        Arc::clone(&self.config)
    }

    /// Get network proxy
    pub fn network(&self) -> Arc<NetworkProxy> {
        Arc::clone(&self.network)
    }

    /// Get wallet manager
    pub fn wallet_manager(&self) -> Arc<StdRwLock<WalletManager>> {
        Arc::clone(&self.wallet_manager)
    }

    /// Get environmental monitor (real energy/carbon telemetry)
    pub fn environmental(&self) -> Arc<EnvironmentalMonitor> {
        Arc::clone(&self.environmental)
    }

    /// Get node info
    pub fn get_node_info(&self) -> Result<NodeInfo, NodeError> {
        let chain_state = self
            .chain_state
            .read()
            .map_err(|e| NodeError::General(format!("Chain state lock poisoned: {}", e)))?;
        let chain_height = chain_state.get_height();
        let best_block_hash = chain_state.get_best_block_hash();
        let connections = self.network.peer_count_sync() as u32;
        let synced = !self.network.is_syncing();
        let network_id = self
            .config
            .read()
            .map(|config| config.network.network_id.clone())
            .map_err(|e| NodeError::General(format!("Config lock poisoned: {}", e)))?;

        Ok(NodeInfo {
            node_id: self.peer_id.to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            protocol_version: 1,
            network: network_id,
            height: chain_height,
            best_block_hash: hex::encode(best_block_hash),
            connections,
            synced,
            uptime: System::uptime(),
        })
    }

    /// Get node status
    pub async fn get_status(&self) -> NodeStatus {
        let (chain_height, best_block_hash) = match self.chain_state.read() {
            Ok(state) => (state.get_height(), state.get_best_block_hash()),
            Err(_) => (0, [0u8; 32]), // Safe default if lock is poisoned
        };
        let peer_count = self.network.peer_count().await;
        let synced = !self.network.is_syncing();
        let is_mining = self
            .config
            .read()
            .map(|config| config.node.enable_mining)
            .unwrap_or(false);

        // Calculate network hashrate from difficulty
        let difficulty = if let Ok(Some(hash)) = self.db.get_block_hash_by_height(chain_height) {
            if let Ok(Some(block)) = self.db.get_block(&hash) {
                supernova_core::blockchain::difficulty::calculate_difficulty_from_bits(
                    block.header().bits(),
                )
            } else {
                1.0
            }
        } else {
            1.0
        };

        // Hashrate = difficulty * 2^32 / block_time
        let hashrate = (difficulty * 4_294_967_296.0 / 150.0) as u64;
        let network_hashrate = hashrate * peer_count.max(1) as u64;

        NodeStatus {
            state: if synced {
                "synced".to_string()
            } else {
                "syncing".to_string()
            },
            height: chain_height,
            best_block_hash: hex::encode(best_block_hash),
            peer_count,
            mempool_size: self.mempool.size(),
            is_mining,
            hashrate: if is_mining { hashrate / 1_000_000 } else { 0 }, // Convert to MH/s
            difficulty,
            network_hashrate: network_hashrate / 1_000_000, // Convert to MH/s
        }
    }

    /// Get system info
    pub fn get_system_info(&self) -> Result<SystemInfo, NodeError> {
        let sys = System::new_all();

        let load_avg = System::load_average();

        Ok(SystemInfo {
            os: System::long_os_version().unwrap_or_else(|| "Unknown".to_string()),
            arch: std::env::consts::ARCH.to_string(),
            cpu_count: sys.cpus().len() as u32,
            total_memory: sys.total_memory(),
            used_memory: sys.used_memory(),
            total_swap: sys.total_swap(),
            used_swap: sys.used_swap(),
            uptime: System::uptime(),
            load_average: LoadAverage {
                one: load_avg.one,
                five: load_avg.five,
                fifteen: load_avg.fifteen,
            },
        })
    }

    /// Get logs
    pub fn get_logs(
        &self,
        level: &str,
        component: Option<&str>,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<LogEntry>, NodeError> {
        let logs = crate::logging::get_recent_logs(level, component, limit, offset);
        Ok(logs)
    }

    /// Get version info
    pub fn get_version(&self) -> Result<VersionInfo, NodeError> {
        Ok(VersionInfo {
            version: env!("CARGO_PKG_VERSION").to_string(),
            protocol_version: 1,
            git_commit: option_env!("VERGEN_GIT_SHA")
                .unwrap_or("unknown")
                .to_string(),
            build_date: option_env!("VERGEN_BUILD_TIMESTAMP")
                .unwrap_or("unknown")
                .to_string(),
            rust_version: option_env!("VERGEN_RUSTC_SEMVER")
                .unwrap_or(env!("CARGO_PKG_RUST_VERSION"))
                .to_string(),
        })
    }

    /// Get metrics.
    ///
    /// Returns a point-in-time snapshot of node metrics. The `_period`
    /// argument is reserved for future windowed/aggregated metrics and is
    /// intentionally not yet consumed; callers receive current instantaneous
    /// values regardless of the requested period.
    pub fn get_metrics(&self, _period: u64) -> Result<NodeMetrics, NodeError> {
        use sysinfo::{Disks, System};
        let mut sys = System::new_all();
        sys.refresh_all();

        let cpu_usage = sys.global_cpu_info().cpu_usage() as f64;
        let memory_usage = sys.used_memory();

        let disks = Disks::new_with_refreshed_list();
        let disk_usage = disks
            .list()
            .first()
            .map(|disk| disk.total_space() - disk.available_space())
            .unwrap_or(0);

        let peer_count = self.network.peer_count_sync();
        let network_stats = self.network.get_stats_sync();

        Ok(NodeMetrics {
            uptime: self.start_time.elapsed().as_secs(),
            peer_count,
            block_height: self
                .chain_state
                .read()
                .map(|state| state.get_height())
                .unwrap_or(0),
            mempool_size: self.mempool.size(),
            mempool_bytes: self.mempool.get_memory_usage() as usize,
            sync_progress: self.network.get_sync_progress(),
            network_bytes_sent: network_stats.bytes_sent,
            network_bytes_received: network_stats.bytes_received,
            cpu_usage,
            memory_usage,
            disk_usage,
        })
    }

    /// Get config
    pub fn get_config(&self) -> Result<serde_json::Value, NodeError> {
        let config = self
            .config
            .read()
            .map_err(|e| NodeError::ConfigError(format!("Config lock poisoned: {}", e)))?;
        serde_json::to_value(&*config).map_err(|e| NodeError::ConfigError(e.to_string()))
    }

    /// Update config
    pub fn update_config(
        &self,
        new_config: serde_json::Value,
    ) -> Result<serde_json::Value, NodeError> {
        let updated_config: crate::config::NodeConfig = serde_json::from_value(new_config)
            .map_err(|e| NodeError::ConfigError(format!("Invalid config: {}", e)))?;

        updated_config
            .validate()
            .map_err(|e| NodeError::ConfigError(e.to_string()))?;

        let mut config = self
            .config
            .write()
            .map_err(|e| NodeError::ConfigError(format!("Config lock poisoned: {}", e)))?;
        *config = updated_config;

        serde_json::to_value(&*config).map_err(|e| NodeError::ConfigError(e.to_string()))
    }

    /// Create backup
    pub fn create_backup(
        &self,
        destination: Option<&str>,
        include_wallet: bool,
        encrypt: bool,
    ) -> Result<BackupInfo, NodeError> {
        use crate::storage::backup::BackupManager;
        use std::time::Duration;

        // BackupManager performs a plaintext copy of the on-disk database and does
        // not yet support at-rest encryption. Rather than silently honoring the
        // `encrypt` flag and shipping an UNENCRYPTED backup (potentially including
        // wallet key material when `include_wallet` is set) while reporting success,
        // fail loudly so the caller is never given false assurance.
        if encrypt {
            return Err(NodeError::General(
                "backup encryption not yet supported: refusing to create an \
                 unencrypted backup while encrypt=true was requested"
                    .to_string(),
            ));
        }

        let backup_dir = std::path::PathBuf::from(destination.unwrap_or("/tmp/supernova_backup"));
        let backup_manager = BackupManager::new(
            self.db.clone(),
            backup_dir.clone(),
            10,
            Duration::from_secs(3600),
        );

        let backup_path = tokio::runtime::Handle::current()
            .block_on(async { backup_manager.create_backup().await })
            .map_err(NodeError::StorageError)?;

        let metadata = std::fs::metadata(&backup_path).map_err(NodeError::IoError)?;

        Ok(BackupInfo {
            id: uuid::Uuid::new_v4().to_string(),
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0),
            size: metadata.len(),
            backup_type: if include_wallet { "full" } else { "blockchain" }.to_string(),
            status: "completed".to_string(),
            file_path: backup_path.to_string_lossy().to_string(),
            verified: true,
        })
    }

    /// Get backup info
    ///
    /// Enumerates the default backup directory used by [`create_backup`] when
    /// no destination is supplied (`/tmp/supernova_backup`). Each backup is a
    /// directory named `supernova_backup_{unix_timestamp}.db` (see
    /// [`crate::storage::backup::BackupManager::create_backup`]). Backups
    /// written to a custom per-call destination are not tracked here, so this
    /// enumeration is best-effort over the default location.
    pub fn get_backup_info(&self) -> Result<Vec<BackupInfo>, NodeError> {
        enumerate_backups(&std::path::PathBuf::from("/tmp/supernova_backup"))
    }

    /// Restart node
    ///
    /// Triggers the same graceful-shutdown sequence used for SIGINT/SIGTERM
    /// (see `crate::shutdown::ShutdownCoordinator`), then exits the process
    /// with a distinct code so a process supervisor (systemd, Docker,
    /// Kubernetes) configured to restart on that code brings the node back
    /// up. This process does not re-exec itself in place.
    pub fn restart(&self) -> Result<(), NodeError> {
        crate::shutdown::request_admin_shutdown(true).map_err(NodeError::General)
    }

    /// Shutdown node
    ///
    /// Triggers the same graceful-shutdown sequence used for SIGINT/SIGTERM
    /// (see `crate::shutdown::ShutdownCoordinator`) and lets the process
    /// exit normally once it completes.
    pub fn shutdown(&self) -> Result<(), NodeError> {
        crate::shutdown::request_admin_shutdown(false).map_err(NodeError::General)
    }

    /// Get debug info
    pub fn get_debug_info(&self) -> Result<DebugInfo, NodeError> {
        // Get node info
        let node_info = self.get_node_info()?;

        // Get system info
        let system_info = self.get_system_info()?;

        // Get performance metrics
        let memory_usage = sysinfo::System::new_all().used_memory();
        let cpu_usage = sysinfo::System::new_all().global_cpu_info().cpu_usage();
        let performance_metrics = serde_json::json!({
            "uptime": self.start_time.elapsed().as_secs(),
            "memory_usage": memory_usage,
            "cpu_usage": cpu_usage,
        });

        // Get network stats
        let network_stats_raw = self.network.get_stats_sync();
        let network_stats = serde_json::json!({
            "peer_count": self.network.peer_count_sync(),
            "bytes_sent": network_stats_raw.bytes_sent,
            "bytes_received": network_stats_raw.bytes_received,
        });

        // Get mempool stats
        let mempool_stats = serde_json::json!({
            "size": self.mempool.size(),
            "memory_usage": self.mempool.get_memory_usage(),
        });

        // Get blockchain stats
        let (chain_height, best_block_hash) = match self.chain_state.read() {
            Ok(state) => (state.get_height(), state.get_best_block_hash()),
            Err(_) => (0, [0u8; 32]),
        };
        let blockchain_stats = serde_json::json!({
            "height": chain_height,
            "best_block_hash": hex::encode(best_block_hash),
        });

        // Get lightning stats
        let lightning_enabled = self.lightning_manager.is_some();
        let lightning_stats = if let Some(ln_manager) = &self.lightning_manager {
            match ln_manager.read() {
                Ok(manager) => {
                    // Get info from the manager which includes peer count
                    let info = manager.get_info().unwrap_or_else(|_| {
                        // Return default info if error
                        supernova_core::lightning::manager::LightningInfo {
                            node_id: String::new(),
                            num_channels: 0,
                            num_pending_channels: 0,
                            num_inactive_channels: 0,
                            total_balance_mnova: 0,
                            total_outbound_capacity_mnova: 0,
                            total_inbound_capacity_mnova: 0,
                            num_peers: 0,
                            synced_to_chain: false,
                            synced_to_graph: false,
                            block_height: 0,
                        }
                    });

                    serde_json::json!({
                        "enabled": true,
                        "channels": info.num_channels,
                        "peers": info.num_peers,
                    })
                }
                Err(_) => {
                    // Lock poisoned, return safe defaults
                    serde_json::json!({
                        "enabled": true,
                        "channels": 0,
                        "peers": 0,
                    })
                }
            }
        } else {
            serde_json::json!({
                "enabled": false,
                "channels": 0,
                "peers": 0,
            })
        };

        Ok(DebugInfo {
            node_info,
            system_info,
            performance_metrics,
            network_stats,
            mempool_stats,
            blockchain_stats,
            lightning_stats,
        })
    }

    /// Broadcast transaction (stub - needs network access)
    pub fn broadcast_transaction(&self, tx: &Transaction) {
        // Add to mempool
        if let Err(e) = self.mempool.add_transaction(tx.clone(), 1) {
            tracing::warn!("Failed to add transaction to mempool: {}", e);
            return;
        }

        // Broadcast to network
        self.network.broadcast_transaction(tx);
    }
}

/// Enumerate the backups present in `backup_dir`.
///
/// Backups are the directories written by
/// [`crate::storage::backup::BackupManager::create_backup`], named
/// `supernova_backup_{unix_timestamp}.db`. A missing directory is treated as
/// "no backups" rather than an error. Results are sorted most-recent first.
fn enumerate_backups(backup_dir: &std::path::Path) -> Result<Vec<BackupInfo>, NodeError> {
    let entries = match std::fs::read_dir(backup_dir) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(NodeError::IoError(e)),
    };

    let mut backups = Vec::new();
    for entry in entries {
        let entry = entry.map_err(NodeError::IoError)?;
        let path = entry.path();
        let file_name = match path.file_name().and_then(|n| n.to_str()) {
            Some(name) => name.to_string(),
            None => continue,
        };

        // Only surface entries produced by create_backup.
        if !file_name.starts_with("supernova_backup_") {
            continue;
        }

        let metadata = match std::fs::metadata(&path) {
            Ok(m) => m,
            Err(_) => continue,
        };

        // Prefer the timestamp embedded in the backup name; fall back to
        // the filesystem modification time.
        let timestamp = file_name
            .strip_prefix("supernova_backup_")
            .and_then(|rest| rest.strip_suffix(".db").or(Some(rest)))
            .and_then(|ts| ts.parse::<u64>().ok())
            .or_else(|| {
                metadata
                    .modified()
                    .ok()
                    .and_then(|m| m.duration_since(std::time::UNIX_EPOCH).ok())
                    .map(|d| d.as_secs())
            })
            .unwrap_or(0);

        // Backups are directories; sum the sizes of the contained files.
        let size = if metadata.is_dir() {
            std::fs::read_dir(&path)
                .map(|inner| {
                    inner
                        .flatten()
                        .filter_map(|e| e.metadata().ok())
                        .filter(|m| m.is_file())
                        .map(|m| m.len())
                        .sum()
                })
                .unwrap_or(0)
        } else {
            metadata.len()
        };

        backups.push(BackupInfo {
            id: file_name.clone(),
            timestamp,
            size,
            backup_type: "blockchain".to_string(),
            status: "completed".to_string(),
            file_path: path.to_string_lossy().to_string(),
            verified: true,
        });
    }

    // Most recent first.
    backups.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
    Ok(backups)
}

#[cfg(test)]
mod backup_enumeration_tests {
    use super::enumerate_backups;

    #[test]
    fn missing_directory_returns_empty() {
        let dir = std::env::temp_dir().join(format!("sn_backup_missing_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let result = enumerate_backups(&dir).expect("missing dir is not an error");
        assert!(result.is_empty());
    }

    #[test]
    fn enumerates_created_backups_sorted() {
        let dir = std::env::temp_dir().join(format!("sn_backup_enum_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        // Two backup directories with embedded timestamps, plus an unrelated
        // file that must be ignored.
        let older = dir.join("supernova_backup_1000.db");
        let newer = dir.join("supernova_backup_2000.db");
        std::fs::create_dir_all(&older).unwrap();
        std::fs::create_dir_all(&newer).unwrap();
        std::fs::write(older.join("data.bin"), vec![0u8; 8]).unwrap();
        std::fs::write(newer.join("data.bin"), vec![0u8; 16]).unwrap();
        std::fs::write(dir.join("unrelated.txt"), b"ignore me").unwrap();

        let result = enumerate_backups(&dir).expect("enumeration succeeds");
        assert_eq!(result.len(), 2, "only the two backups should be listed");

        // Most recent first.
        assert_eq!(result[0].timestamp, 2000);
        assert_eq!(result[1].timestamp, 1000);
        assert_eq!(result[0].size, 16);
        assert_eq!(result[1].size, 8);
        assert_eq!(result[0].status, "completed");
        assert!(result[0].verified);

        let _ = std::fs::remove_dir_all(&dir);
    }
}
