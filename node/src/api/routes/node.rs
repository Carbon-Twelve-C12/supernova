//! Node API routes
//!
//! This module implements the HTTP routes for node management, monitoring,
//! and configuration operations.

use actix_web::{web, HttpResponse, Responder};
use serde::{Deserialize, Serialize};
use tracing::{error, info, warn};
use utoipa::{IntoParams, ToSchema};

use super::NodeData;
use crate::api::types::*;

/// Configure node routes
pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.route("/info", web::get().to(get_node_info))
        .route("/status", web::get().to(get_node_status))
        .route("/config", web::get().to(get_config))
        .route("/config", web::put().to(update_config))
        .route("/restart", web::post().to(restart_node))
        .route("/shutdown", web::post().to(shutdown_node))
        .route("/metrics", web::get().to(get_metrics))
        .route("/system", web::get().to(get_system_info))
        .route("/logs", web::get().to(get_logs))
        .route("/version", web::get().to(get_version))
        .route("/backup", web::post().to(create_backup))
        .route("/backup", web::get().to(get_backup_info))
        .route("/debug", web::get().to(get_debug_info));
}

/// Get node information
///
/// Returns basic information about the node including version, network, and status
#[utoipa::path(
    get,
    path = "/api/v1/node/info",
    responses(
        (status = 200, description = "Node information retrieved successfully", body = NodeInfo),
        (status = 500, description = "Internal server error")
    ),
    tag = "node"
)]
pub async fn get_node_info(node: NodeData) -> impl Responder {
    match node.get_node_info() {
        Ok(info) => HttpResponse::Ok().json(info),
        Err(e) => {
            error!("Failed to get node info: {}", e);
            HttpResponse::InternalServerError().json(ErrorResponse {
                error: format!("Failed to get node info: {}", e),
            })
        }
    }
}

/// Get node status
#[utoipa::path(
    get,
    path = "/api/v1/node/status",
    responses(
        (status = 200, description = "Node status retrieved successfully", body = NodeStatus),
        (status = 500, description = "Internal server error")
    ),
    tag = "node"
)]
pub async fn get_node_status(node: NodeData) -> impl Responder {
    let status = node.get_status().await;
    HttpResponse::Ok().json(status)
}

/// Get system information
#[utoipa::path(
    get,
    path = "/api/v1/node/system",
    responses(
        (status = 200, description = "System information retrieved successfully", body = SystemInfo),
        (status = 500, description = "Internal server error")
    ),
    tag = "node"
)]
pub async fn get_system_info(node: NodeData) -> impl Responder {
    match node.get_system_info() {
        Ok(info) => HttpResponse::Ok().json(info),
        Err(e) => {
            error!("Failed to get system info: {}", e);
            HttpResponse::InternalServerError().json(ErrorResponse {
                error: format!("Failed to get system info: {}", e),
            })
        }
    }
}

/// Query parameters for logs endpoint
#[derive(Debug, Clone, Serialize, Deserialize, IntoParams, ToSchema)]
pub struct LogsQuery {
    /// Log level filter
    #[param(example = "info")]
    pub level: Option<String>,
    /// Component filter
    pub component: Option<String>,
    /// Maximum number of logs to return
    #[param(example = 100)]
    pub limit: Option<usize>,
    /// Offset for pagination
    #[param(example = 0)]
    pub offset: Option<usize>,
}

/// Get logs
#[utoipa::path(
    get,
    path = "/api/v1/node/logs",
    params(LogsQuery),
    responses(
        (status = 200, description = "Logs retrieved successfully", body = Vec<LogEntry>),
        (status = 500, description = "Internal server error")
    ),
    tag = "node"
)]
pub async fn get_logs(node: NodeData, query: web::Query<LogsQuery>) -> impl Responder {
    let level = query.level.as_deref().unwrap_or("info");
    let limit = query.limit.unwrap_or(100);
    let offset = query.offset.unwrap_or(0);

    match node.get_logs(level, query.component.as_deref(), limit, offset) {
        Ok(logs) => HttpResponse::Ok().json(logs),
        Err(e) => {
            error!("Failed to get logs: {}", e);
            HttpResponse::InternalServerError().json(ErrorResponse {
                error: format!("Failed to get logs: {}", e),
            })
        }
    }
}

/// Get version info
#[utoipa::path(
    get,
    path = "/api/v1/node/version",
    responses(
        (status = 200, description = "Version information retrieved successfully", body = VersionInfo),
        (status = 500, description = "Internal server error")
    ),
    tag = "node"
)]
pub async fn get_version(node: NodeData) -> impl Responder {
    match node.get_version() {
        Ok(version) => HttpResponse::Ok().json(version),
        Err(e) => {
            error!("Failed to get version info: {}", e);
            HttpResponse::InternalServerError().json(ErrorResponse {
                error: format!("Failed to get version info: {}", e),
            })
        }
    }
}

/// Query parameters for metrics endpoint
#[derive(Debug, Clone, Serialize, Deserialize, IntoParams, ToSchema)]
pub struct MetricsQuery {
    /// Time period in seconds
    #[param(example = 3600)]
    pub period: Option<u64>,
}

/// Get metrics
#[utoipa::path(
    get,
    path = "/api/v1/node/metrics",
    params(MetricsQuery),
    responses(
        (status = 200, description = "Metrics retrieved successfully", body = NodeMetrics),
        (status = 500, description = "Internal server error")
    ),
    tag = "node"
)]
pub async fn get_metrics(node: NodeData, query: web::Query<MetricsQuery>) -> impl Responder {
    let period = query.period.unwrap_or(60);

    match node.get_metrics(period) {
        Ok(metrics) => HttpResponse::Ok().json(metrics),
        Err(e) => {
            error!("Failed to get metrics: {}", e);
            HttpResponse::InternalServerError().json(ErrorResponse {
                error: format!("Failed to get metrics: {}", e),
            })
        }
    }
}

/// Get configuration
#[utoipa::path(
    get,
    path = "/api/v1/node/config",
    responses(
        (status = 200, description = "Configuration retrieved successfully", body = serde_json::Value),
        (status = 500, description = "Internal server error")
    ),
    tag = "node"
)]
pub async fn get_config(node: NodeData) -> impl Responder {
    match node.get_config() {
        Ok(config) => HttpResponse::Ok().json(config),
        Err(e) => {
            error!("Failed to get config: {}", e);
            HttpResponse::InternalServerError().json(ErrorResponse {
                error: format!("Failed to get config: {}", e),
            })
        }
    }
}

/// Update configuration
#[utoipa::path(
    put,
    path = "/api/v1/node/config",
    request_body = serde_json::Value,
    responses(
        (status = 200, description = "Configuration updated successfully", body = serde_json::Value),
        (status = 400, description = "Invalid configuration"),
        (status = 500, description = "Internal server error")
    ),
    tag = "node"
)]
pub async fn update_config(
    node: NodeData,
    new_config: web::Json<serde_json::Value>,
) -> impl Responder {
    match node.update_config(new_config.into_inner()) {
        Ok(config) => {
            info!("Configuration updated successfully");
            HttpResponse::Ok().json(config)
        }
        Err(e) => {
            warn!("Failed to update config: {}", e);
            HttpResponse::BadRequest().json(ErrorResponse {
                error: format!("Failed to update config: {}", e),
            })
        }
    }
}

/// Backup request parameters
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct BackupRequest {
    /// Destination path for the backup
    pub destination: Option<String>,
    /// Whether to include wallet data
    pub include_wallet: bool,
    /// Whether to encrypt the backup
    pub encrypt: bool,
}

/// Create backup
#[utoipa::path(
    post,
    path = "/api/v1/node/backup",
    request_body = BackupRequest,
    responses(
        (status = 200, description = "Backup created successfully", body = BackupInfo),
        (status = 500, description = "Internal server error")
    ),
    tag = "node"
)]
pub async fn create_backup(node: NodeData, request: web::Json<BackupRequest>) -> impl Responder {
    let include_wallet = request.include_wallet;
    let encrypt = request.encrypt;

    match node.create_backup(request.destination.as_deref(), include_wallet, encrypt) {
        Ok(backup) => {
            info!("Backup created: {}", backup.id);
            HttpResponse::Ok().json(backup)
        }
        Err(e) => {
            error!("Failed to create backup: {}", e);
            HttpResponse::InternalServerError().json(ErrorResponse {
                error: format!("Failed to create backup: {}", e),
            })
        }
    }
}

/// Get backup info
#[utoipa::path(
    get,
    path = "/api/v1/node/backup",
    responses(
        (status = 200, description = "Backup information retrieved successfully", body = Vec<BackupInfo>),
        (status = 500, description = "Internal server error")
    ),
    tag = "node"
)]
pub async fn get_backup_info(node: NodeData) -> impl Responder {
    match node.get_backup_info() {
        Ok(backups) => HttpResponse::Ok().json(backups),
        Err(e) => {
            error!("Failed to get backup info: {}", e);
            HttpResponse::InternalServerError().json(ErrorResponse {
                error: format!("Failed to get backup info: {}", e),
            })
        }
    }
}

/// Restart node
#[utoipa::path(
    post,
    path = "/api/v1/node/restart",
    responses(
        (status = 200, description = "Node restart initiated"),
        (status = 500, description = "Internal server error")
    ),
    tag = "node"
)]
pub async fn restart_node(node: NodeData) -> impl Responder {
    info!("Node restart requested");

    match node.restart() {
        Ok(_) => HttpResponse::Ok().json(serde_json::json!({
            "message": "Node restart initiated"
        })),
        Err(e) => {
            error!("Failed to restart node: {}", e);
            HttpResponse::InternalServerError().json(ErrorResponse {
                error: format!("Failed to restart node: {}", e),
            })
        }
    }
}

/// Shutdown node
#[utoipa::path(
    post,
    path = "/api/v1/node/shutdown",
    responses(
        (status = 200, description = "Node shutdown initiated"),
        (status = 500, description = "Internal server error")
    ),
    tag = "node"
)]
pub async fn shutdown_node(node: NodeData) -> impl Responder {
    warn!("Node shutdown requested");

    match node.shutdown() {
        Ok(_) => HttpResponse::Ok().json(serde_json::json!({
            "message": "Node shutdown initiated"
        })),
        Err(e) => {
            error!("Failed to shutdown node: {}", e);
            HttpResponse::InternalServerError().json(ErrorResponse {
                error: format!("Failed to shutdown node: {}", e),
            })
        }
    }
}

/// Get debug info
#[utoipa::path(
    get,
    path = "/api/v1/node/debug",
    responses(
        (status = 200, description = "Debug information retrieved successfully", body = DebugInfo),
        (status = 500, description = "Internal server error")
    ),
    tag = "node"
)]
pub async fn get_debug_info(node: NodeData) -> impl Responder {
    match node.get_debug_info() {
        Ok(debug) => HttpResponse::Ok().json(debug),
        Err(e) => {
            error!("Failed to get debug info: {}", e);
            HttpResponse::InternalServerError().json(ErrorResponse {
                error: format!("Failed to get debug info: {}", e),
            })
        }
    }
}
