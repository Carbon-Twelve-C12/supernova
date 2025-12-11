//! Health Check Endpoints for Kubernetes Probes
//!
//! This module provides Kubernetes-compatible health check endpoints:
//! - `/health/live` - Liveness probe (process is running)
//! - `/health/ready` - Readiness probe (service is ready to accept traffic)
//!
//! PRODUCTION: These endpoints are critical for container orchestration.

use actix_web::{web, HttpResponse, Responder};
use serde::Serialize;
use tracing::{debug, warn};
use utoipa::ToSchema;

use super::NodeData;

/// Liveness response - indicates the process is running
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct LivenessResponse {
    /// Status (always "ok" if process is running)
    pub status: &'static str,
    /// Application version
    pub version: &'static str,
    /// Current timestamp (Unix seconds)
    pub timestamp: u64,
}

/// Readiness response - indicates the service is ready to accept traffic
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ReadinessResponse {
    /// Overall status ("ready" or "not_ready")
    pub status: &'static str,
    /// Individual health checks
    pub checks: ReadinessChecks,
}

/// Individual readiness check results
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ReadinessChecks {
    /// Chain synchronization status
    pub synced: CheckResult,
    /// Peer connectivity status
    pub peers: CheckResult,
    /// Database health status
    pub database: CheckResult,
}

/// Result of a single health check
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct CheckResult {
    /// Whether this check passed
    pub healthy: bool,
    /// Human-readable status message
    pub message: String,
}

/// Configuration for health checks
pub struct HealthCheckConfig;

impl HealthCheckConfig {
    /// Minimum number of peers required for readiness
    pub const MIN_PEER_COUNT: usize = 3;
    
    /// Sync progress threshold (0.0 to 1.0) - consider synced if above this
    /// We use 0.99 to allow for minor lag in reporting
    pub const SYNC_THRESHOLD: f64 = 0.99;
}

/// Configure health check routes
pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.route("/health/live", web::get().to(liveness))
        .route("/health/ready", web::get().to(readiness));
}

/// Liveness probe endpoint
///
/// Returns 200 OK if the process is running. This endpoint should always succeed
/// as long as the HTTP server is responsive.
///
/// Kubernetes uses this to determine if the container needs to be restarted.
#[utoipa::path(
    get,
    path = "/health/live",
    responses(
        (status = 200, description = "Process is alive", body = LivenessResponse)
    ),
    tag = "health"
)]
pub async fn liveness() -> impl Responder {
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    debug!("Liveness probe: OK");
    
    HttpResponse::Ok().json(LivenessResponse {
        status: "ok",
        version: env!("CARGO_PKG_VERSION"),
        timestamp,
    })
}

/// Readiness probe endpoint
///
/// Returns 200 OK only if the node is ready to accept traffic:
/// - Chain is synced (or nearly synced)
/// - Minimum peer count is met
/// - Database is healthy
///
/// Kubernetes uses this to determine if traffic should be routed to this pod.
#[utoipa::path(
    get,
    path = "/health/ready",
    responses(
        (status = 200, description = "Service is ready", body = ReadinessResponse),
        (status = 503, description = "Service is not ready", body = ReadinessResponse)
    ),
    tag = "health"
)]
pub async fn readiness(node: NodeData) -> impl Responder {
    // Check 1: Sync status
    let sync_check = check_sync_status(&node).await;
    
    // Check 2: Peer connectivity
    let peers_check = check_peer_count(&node).await;
    
    // Check 3: Database health
    let database_check = check_database_health(&node);

    // All checks must pass for readiness
    let all_healthy = sync_check.healthy && peers_check.healthy && database_check.healthy;

    let response = ReadinessResponse {
        status: if all_healthy { "ready" } else { "not_ready" },
        checks: ReadinessChecks {
            synced: sync_check,
            peers: peers_check,
            database: database_check,
        },
    };

    if all_healthy {
        debug!("Readiness probe: Ready");
        HttpResponse::Ok().json(response)
    } else {
        warn!(
            "Readiness probe: Not ready - synced={}, peers={}, db={}",
            response.checks.synced.healthy,
            response.checks.peers.healthy,
            response.checks.database.healthy
        );
        HttpResponse::ServiceUnavailable().json(response)
    }
}

/// Check chain synchronization status
async fn check_sync_status(node: &NodeData) -> CheckResult {
    let is_syncing = node.network().is_syncing();
    
    // Get sync progress from metrics
    let sync_progress = match node.get_metrics(60) {
        Ok(metrics) => metrics.sync_progress,
        Err(_) => 0.0,
    };
    
    // Get current height
    let current_height = match node.chain_state().read() {
        Ok(state) => state.get_height(),
        Err(_) => 0,
    };

    // Consider synced if:
    // 1. Not actively syncing, OR
    // 2. Sync progress is above threshold
    let is_synced = !is_syncing || sync_progress >= HealthCheckConfig::SYNC_THRESHOLD;

    CheckResult {
        healthy: is_synced,
        message: if is_synced {
            format!("Synced at height {}", current_height)
        } else {
            format!(
                "Syncing: {:.1}% complete (height {})",
                sync_progress * 100.0,
                current_height
            )
        },
    }
}

/// Check peer connectivity
async fn check_peer_count(node: &NodeData) -> CheckResult {
    let peer_count = node.network().peer_count().await;
    let min_required = HealthCheckConfig::MIN_PEER_COUNT;
    let healthy = peer_count >= min_required;

    CheckResult {
        healthy,
        message: if healthy {
            format!("{} peers connected", peer_count)
        } else {
            format!(
                "Insufficient peers: {} < {} required",
                peer_count, min_required
            )
        },
    }
}

/// Check database health
fn check_database_health(node: &NodeData) -> CheckResult {
    // Try to read from chain state to verify database is accessible
    let db_accessible = node.chain_state().read().is_ok();
    
    // Try to access the storage layer
    let storage_healthy = match node.storage().health_check() {
        Ok(()) => true,
        Err(e) => {
            warn!("Database health check failed: {}", e);
            false
        }
    };

    let healthy = db_accessible && storage_healthy;

    CheckResult {
        healthy,
        message: if healthy {
            "Database healthy".to_string()
        } else if !db_accessible {
            "Chain state lock error".to_string()
        } else {
            "Database health check failed".to_string()
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_health_check_config() {
        assert_eq!(HealthCheckConfig::MIN_PEER_COUNT, 3);
        assert!(HealthCheckConfig::SYNC_THRESHOLD > 0.9);
        assert!(HealthCheckConfig::SYNC_THRESHOLD <= 1.0);
    }

    #[test]
    fn test_check_result_serialization() {
        let check = CheckResult {
            healthy: true,
            message: "Test passed".to_string(),
        };
        
        let json = serde_json::to_string(&check).expect("Should serialize");
        assert!(json.contains("\"healthy\":true"));
        assert!(json.contains("\"message\":\"Test passed\""));
    }

    #[test]
    fn test_liveness_response_serialization() {
        let response = LivenessResponse {
            status: "ok",
            version: "1.0.0",
            timestamp: 1234567890,
        };
        
        let json = serde_json::to_string(&response).expect("Should serialize");
        assert!(json.contains("\"status\":\"ok\""));
        assert!(json.contains("\"version\":\"1.0.0\""));
    }

    #[test]
    fn test_readiness_response_serialization() {
        let response = ReadinessResponse {
            status: "ready",
            checks: ReadinessChecks {
                synced: CheckResult {
                    healthy: true,
                    message: "Synced".to_string(),
                },
                peers: CheckResult {
                    healthy: true,
                    message: "5 peers".to_string(),
                },
                database: CheckResult {
                    healthy: true,
                    message: "OK".to_string(),
                },
            },
        };
        
        let json = serde_json::to_string(&response).expect("Should serialize");
        assert!(json.contains("\"status\":\"ready\""));
        assert!(json.contains("\"synced\""));
        assert!(json.contains("\"peers\""));
        assert!(json.contains("\"database\""));
    }
}

