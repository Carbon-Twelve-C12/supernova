use crate::api::error::{ApiError, ApiResult};
use crate::api::types::{
    NodeInfo, SystemInfo, LogEntry, NodeStatus, NodeVersion, 
    NodeConfiguration, BackupInfo, NodeMetrics, DebugInfo,
};
use crate::node::Node;
use actix_web::{web, HttpResponse};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use std::sync::Arc;
use serde_json;

/// Configure node API routes
pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/node")
            .route("/info", web::get().to(get_node_info))
            .route("/system", web::get().to(get_system_info))
            .route("/logs", web::get().to(get_logs))
            .route("/status", web::get().to(get_node_status))
            .route("/version", web::get().to(get_node_version))
            .route("/metrics", web::get().to(get_node_metrics))
            .route("/config", web::get().to(get_node_config))
            .route("/config", web::put().to(update_node_config))
            .route("/backup", web::post().to(create_backup))
            .route("/backup", web::get().to(get_backup_info))
            .route("/restart", web::post().to(restart_node))
            .route("/shutdown", web::post().to(shutdown_node))
            .route("/debug", web::get().to(get_debug_info)),
    );
}

/// Get general node information
///
/// Returns general information about the node.
#[utoipa::path(
    get,
    path = "/api/v1/node/info",
    responses(
        (status = 200, description = "Node information retrieved successfully", body = NodeInfo),
        (status = 500, description = "Internal server error", body = ApiError)
    )
)]
pub async fn get_node_info(
    node: web::Data<Arc<Node>>,
) -> ApiResult<HttpResponse> {
    // TODO: Implement real node info retrieval
    let info = node.get_info()?;
    
    Ok(HttpResponse::Ok().json(info))
}

/// Get system information
///
/// Returns information about the system the node is running on.
#[utoipa::path(
    get,
    path = "/api/v1/node/system",
    responses(
        (status = 200, description = "System information retrieved successfully", body = SystemInfo),
        (status = 500, description = "Internal server error", body = ApiError)
    )
)]
pub async fn get_system_info(
    node: web::Data<Arc<Node>>,
) -> ApiResult<HttpResponse> {
    // TODO: Implement real system info retrieval
    let info = node.get_system_info()?;
    
    Ok(HttpResponse::Ok().json(info))
}

/// Get node logs
///
/// Returns logs from the node's operation.
#[derive(Debug, Deserialize, IntoParams)]
struct GetLogsParams {
    /// Optional log level filter (default: "info")
    #[param(default = "info")]
    level: Option<String>,
    
    /// Optional component filter
    component: Option<String>,
    
    /// Maximum number of log entries to retrieve (default: 100)
    #[param(default = "100")]
    limit: Option<u32>,
    
    /// Log entry offset for pagination (default: 0)
    #[param(default = "0")]
    offset: Option<u32>,
}

#[utoipa::path(
    get,
    path = "/api/v1/node/logs",
    params(
        GetLogsParams
    ),
    responses(
        (status = 200, description = "Logs retrieved successfully", body = Vec<LogEntry>),
        (status = 400, description = "Invalid request parameters", body = ApiError),
        (status = 500, description = "Internal server error", body = ApiError)
    )
)]
pub async fn get_logs(
    params: web::Query<GetLogsParams>,
    node: web::Data<Arc<Node>>,
) -> ApiResult<HttpResponse> {
    let level = params.level.clone().unwrap_or_else(|| "info".to_string());
    let component = params.component.clone();
    let limit = params.limit.unwrap_or(100);
    let offset = params.offset.unwrap_or(0);
    
    // TODO: Implement real logs retrieval
    let logs = node.get_logs(&level, component.as_deref(), limit, offset)?;
    
    Ok(HttpResponse::Ok().json(logs))
}

/// Get node status
///
/// Returns the current status of the node.
#[utoipa::path(
    get,
    path = "/api/v1/node/status",
    responses(
        (status = 200, description = "Node status retrieved successfully", body = NodeStatus),
        (status = 500, description = "Internal server error", body = ApiError)
    )
)]
pub async fn get_node_status(
    node: web::Data<Arc<Node>>,
) -> ApiResult<HttpResponse> {
    // TODO: Implement real node status retrieval
    let status = node.get_status()?;
    
    Ok(HttpResponse::Ok().json(status))
}

/// Get node version information
///
/// Returns version information for the node and its components.
#[utoipa::path(
    get,
    path = "/api/v1/node/version",
    responses(
        (status = 200, description = "Node version information retrieved successfully", body = NodeVersion),
        (status = 500, description = "Internal server error", body = ApiError)
    )
)]
pub async fn get_node_version(
    node: web::Data<Arc<Node>>,
) -> ApiResult<HttpResponse> {
    // TODO: Implement real node version retrieval
    let version = node.get_version()?;
    
    Ok(HttpResponse::Ok().json(version))
}

/// Get node performance metrics
///
/// Returns performance metrics for the node.
#[derive(Debug, Deserialize, IntoParams)]
struct GetNodeMetricsParams {
    /// Time period in seconds for which to retrieve metrics (default: 300 - 5 minutes)
    #[param(default = "300")]
    period: Option<u64>,
}

#[utoipa::path(
    get,
    path = "/api/v1/node/metrics",
    params(
        GetNodeMetricsParams
    ),
    responses(
        (status = 200, description = "Node metrics retrieved successfully", body = NodeMetrics),
        (status = 400, description = "Invalid request parameters", body = ApiError),
        (status = 500, description = "Internal server error", body = ApiError)
    )
)]
pub async fn get_node_metrics(
    params: web::Query<GetNodeMetricsParams>,
    node: web::Data<Arc<Node>>,
) -> ApiResult<HttpResponse> {
    let period = params.period.unwrap_or(300);
    
    // TODO: Implement real node metrics retrieval
    let metrics = node.get_metrics(period)?;
    
    Ok(HttpResponse::Ok().json(metrics))
}

/// Get node configuration
///
/// Returns the current configuration of the node.
#[utoipa::path(
    get,
    path = "/api/v1/node/config",
    responses(
        (status = 200, description = "Node configuration retrieved successfully", body = NodeConfiguration),
        (status = 500, description = "Internal server error", body = ApiError)
    )
)]
pub async fn get_node_config(
    node: web::Data<Arc<Node>>,
) -> ApiResult<HttpResponse> {
    // TODO: Implement real node configuration retrieval
    let config = node.get_config()?;
    
    Ok(HttpResponse::Ok().json(config))
}

/// Update node configuration
///
/// Updates the configuration of the node.
#[utoipa::path(
    put,
    path = "/api/v1/node/config",
    request_body = NodeConfiguration,
    responses(
        (status = 200, description = "Node configuration updated successfully", body = NodeConfiguration),
        (status = 400, description = "Invalid configuration", body = ApiError),
        (status = 500, description = "Internal server error", body = ApiError)
    )
)]
pub async fn update_node_config(
    request: web::Json<NodeConfiguration>,
    node: web::Data<Arc<Node>>,
) -> ApiResult<HttpResponse> {
    // TODO: Implement real node configuration update
    let config_value = serde_json::to_value(&request.0).map_err(|e| ApiError::bad_request(format!("Invalid configuration: {}", e)))?;
    let updated_config = node.update_config(config_value)?;
    
    Ok(HttpResponse::Ok().json(updated_config))
}

/// Create a node backup
///
/// Creates a backup of the node's data.
#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct CreateBackupRequest {
    /// Optional destination path for the backup (default: system-determined location)
    destination: Option<String>,
    
    /// Whether to include wallet data (default: true)
    #[schema(default = true)]
    include_wallet: Option<bool>,
    
    /// Whether to encrypt the backup (default: true)
    #[schema(default = true)]
    encrypt: Option<bool>,
}

#[utoipa::path(
    post,
    path = "/api/v1/node/backup",
    request_body = CreateBackupRequest,
    responses(
        (status = 200, description = "Backup created successfully", body = BackupInfo),
        (status = 400, description = "Invalid request parameters", body = ApiError),
        (status = 500, description = "Internal server error", body = ApiError)
    )
)]
pub async fn create_backup(
    request: web::Json<CreateBackupRequest>,
    node: web::Data<Arc<Node>>,
) -> ApiResult<HttpResponse> {
    let destination = request.destination.clone();
    let include_wallet = request.include_wallet.unwrap_or(true);
    let encrypt = request.encrypt.unwrap_or(true);
    
    // TODO: Implement real backup creation
    let backup_info = node.create_backup(destination.as_deref(), include_wallet, encrypt)?;
    
    Ok(HttpResponse::Ok().json(backup_info))
}

/// Get backup information
///
/// Returns information about available backups.
#[utoipa::path(
    get,
    path = "/api/v1/node/backup",
    responses(
        (status = 200, description = "Backup information retrieved successfully", body = Vec<BackupInfo>),
        (status = 500, description = "Internal server error", body = ApiError)
    )
)]
pub async fn get_backup_info(
    node: web::Data<Arc<Node>>,
) -> ApiResult<HttpResponse> {
    // TODO: Implement real backup info retrieval
    let backup_info = node.get_backup_info()?;
    
    Ok(HttpResponse::Ok().json(backup_info))
}

/// Restart the node
///
/// Initiates a node restart operation.
#[utoipa::path(
    post,
    path = "/api/v1/node/restart",
    responses(
        (status = 200, description = "Node restart initiated successfully"),
        (status = 500, description = "Internal server error", body = ApiError)
    )
)]
pub async fn restart_node(
    node: web::Data<Arc<Node>>,
) -> ApiResult<HttpResponse> {
    // TODO: Implement real node restart
    node.restart()?;
    
    Ok(HttpResponse::Ok().finish())
}

/// Shutdown the node
///
/// Initiates a node shutdown operation.
#[utoipa::path(
    post,
    path = "/api/v1/node/shutdown",
    responses(
        (status = 200, description = "Node shutdown initiated successfully"),
        (status = 500, description = "Internal server error", body = ApiError)
    )
)]
pub async fn shutdown_node(
    node: web::Data<Arc<Node>>,
) -> ApiResult<HttpResponse> {
    // TODO: Implement real node shutdown
    node.shutdown()?;
    
    Ok(HttpResponse::Ok().finish())
}

/// Get debug information
///
/// Returns debug information for troubleshooting.
#[utoipa::path(
    get,
    path = "/api/v1/node/debug",
    responses(
        (status = 200, description = "Debug information retrieved successfully", body = DebugInfo),
        (status = 500, description = "Internal server error", body = ApiError)
    )
)]
pub async fn get_debug_info(
    node: web::Data<Arc<Node>>,
) -> ApiResult<HttpResponse> {
    // TODO: Implement real debug info retrieval
    let debug_info = node.get_debug_info()?;
    
    Ok(HttpResponse::Ok().json(debug_info))
} 