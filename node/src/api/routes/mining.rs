use crate::api::error::{ApiError, ApiResult};
use crate::api::types::{
    MiningInfo, MiningTemplate, MiningStats, SubmitBlockRequest, 
    SubmitBlockResponse, MiningStatus, MiningConfiguration,
};
use crate::mining::MiningManager;
use actix_web::{web, HttpResponse};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use std::sync::Arc;
use hex::FromHex;

/// Configure mining API routes
pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/mining")
            .route("/info", web::get().to(get_mining_info))
            .route("/template", web::get().to(get_mining_template))
            .route("/submit", web::post().to(submit_block))
            .route("/stats", web::get().to(get_mining_stats))
            .route("/status", web::get().to(get_mining_status))
            .route("/start", web::post().to(start_mining))
            .route("/stop", web::post().to(stop_mining))
            .route("/config", web::get().to(get_mining_config))
            .route("/config", web::put().to(update_mining_config)),
    );
}

/// Get mining information
///
/// Returns current mining-related information.
#[utoipa::path(
    get,
    path = "/api/v1/mining/info",
    responses(
        (status = 200, description = "Mining information retrieved successfully", body = MiningInfo),
        (status = 500, description = "Internal server error", body = ApiError)
    )
)]
async fn get_mining_info(
    mining: web::Data<Arc<MiningManager>>,
) -> ApiResult<MiningInfo> {
    // TODO: Implement real mining info retrieval
    let info = mining.get_mining_info()?;
    
    Ok(HttpResponse::Ok().json(info))
}

/// Get mining template for block creation
///
/// Returns data needed to construct a block for mining.
#[utoipa::path(
    get,
    path = "/api/v1/mining/template",
    params(
        GetMiningTemplateParams
    ),
    responses(
        (status = 200, description = "Mining template retrieved successfully", body = MiningTemplate),
        (status = 400, description = "Invalid request parameters", body = ApiError),
        (status = 500, description = "Internal server error", body = ApiError)
    )
)]
#[derive(Debug, Deserialize, IntoParams)]
struct GetMiningTemplateParams {
    /// Comma-separated list of capabilities (default: "standard")
    #[param(default = "standard")]
    capabilities: Option<String>,
    
    /// Maximum number of transactions to include (default: all available)
    max_transactions: Option<u32>,
}

async fn get_mining_template(
    params: web::Query<GetMiningTemplateParams>,
    mining: web::Data<Arc<MiningManager>>,
) -> ApiResult<MiningTemplate> {
    let capabilities = params.capabilities.clone().unwrap_or_else(|| "standard".to_string());
    let max_transactions = params.max_transactions;
    
    // TODO: Implement real mining template generation
    let template = mining.get_mining_template(&capabilities, max_transactions)?;
    
    Ok(HttpResponse::Ok().json(template))
}

/// Submit a mined block
///
/// Submits a solved block to the network.
#[utoipa::path(
    post,
    path = "/api/v1/mining/submit",
    request_body = SubmitBlockRequest,
    responses(
        (status = 200, description = "Block submitted successfully", body = SubmitBlockResponse),
        (status = 400, description = "Invalid block data", body = ApiError),
        (status = 500, description = "Internal server error", body = ApiError)
    )
)]
async fn submit_block(
    request: web::Json<SubmitBlockRequest>,
    mining: web::Data<Arc<MiningManager>>,
) -> ApiResult<SubmitBlockResponse> {
    let block_data = &request.block_data;
    
    // Validate block data format
    let block_bytes = Vec::from_hex(block_data).map_err(|_| {
        ApiError::bad_request("Invalid block data format")
    })?;
    
    // TODO: Implement real block submission
    let result = mining.submit_block(&block_bytes)?;
    
    Ok(HttpResponse::Ok().json(result))
}

/// Get mining statistics
///
/// Returns statistics about mining operations.
#[utoipa::path(
    get,
    path = "/api/v1/mining/stats",
    params(
        GetMiningStatsParams
    ),
    responses(
        (status = 200, description = "Mining statistics retrieved successfully", body = MiningStats),
        (status = 400, description = "Invalid request parameters", body = ApiError),
        (status = 500, description = "Internal server error", body = ApiError)
    )
)]
#[derive(Debug, Deserialize, IntoParams)]
struct GetMiningStatsParams {
    /// Time period in seconds (default: 3600)
    #[param(default = "3600")]
    period: Option<u64>,
}

async fn get_mining_stats(
    params: web::Query<GetMiningStatsParams>,
    mining: web::Data<Arc<MiningManager>>,
) -> ApiResult<MiningStats> {
    let period = params.period.unwrap_or(3600);
    
    // TODO: Implement real mining stats retrieval
    let stats = mining.get_mining_stats(period)?;
    
    Ok(HttpResponse::Ok().json(stats))
}

/// Get mining status
///
/// Returns the current status of the mining operation.
#[utoipa::path(
    get,
    path = "/api/v1/mining/status",
    responses(
        (status = 200, description = "Mining status retrieved successfully", body = MiningStatus),
        (status = 500, description = "Internal server error", body = ApiError)
    )
)]
async fn get_mining_status(
    mining: web::Data<Arc<MiningManager>>,
) -> ApiResult<MiningStatus> {
    // TODO: Implement real mining status retrieval
    let status = mining.get_mining_status()?;
    
    Ok(HttpResponse::Ok().json(status))
}

/// Start mining
///
/// Starts the mining operation.
#[utoipa::path(
    post,
    path = "/api/v1/mining/start",
    request_body = StartMiningRequest,
    responses(
        (status = 200, description = "Mining operation started successfully"),
        (status = 400, description = "Invalid request parameters", body = ApiError),
        (status = 500, description = "Internal server error", body = ApiError)
    )
)]
#[derive(Debug, Deserialize, Serialize, ToSchema)]
struct StartMiningRequest {
    /// Number of threads to use for mining (default: use system-determined optimal value)
    threads: Option<u32>,
}

async fn start_mining(
    request: web::Json<StartMiningRequest>,
    mining: web::Data<Arc<MiningManager>>,
) -> ApiResult<HttpResponse> {
    let threads = request.threads;
    
    // TODO: Implement real mining start
    mining.start_mining(threads)?;
    
    Ok(HttpResponse::Ok().finish())
}

/// Stop mining
///
/// Stops the mining operation.
#[utoipa::path(
    post,
    path = "/api/v1/mining/stop",
    responses(
        (status = 200, description = "Mining operation stopped successfully"),
        (status = 500, description = "Internal server error", body = ApiError)
    )
)]
async fn stop_mining(
    mining: web::Data<Arc<MiningManager>>,
) -> ApiResult<HttpResponse> {
    // TODO: Implement real mining stop
    mining.stop_mining()?;
    
    Ok(HttpResponse::Ok().finish())
}

/// Get mining configuration
///
/// Returns the current mining configuration.
#[utoipa::path(
    get,
    path = "/api/v1/mining/config",
    responses(
        (status = 200, description = "Mining configuration retrieved successfully", body = MiningConfiguration),
        (status = 500, description = "Internal server error", body = ApiError)
    )
)]
async fn get_mining_config(
    mining: web::Data<Arc<MiningManager>>,
) -> ApiResult<MiningConfiguration> {
    // TODO: Implement real mining config retrieval
    let config = mining.get_mining_config()?;
    
    Ok(HttpResponse::Ok().json(config))
}

/// Update mining configuration
///
/// Updates the mining configuration.
#[utoipa::path(
    put,
    path = "/api/v1/mining/config",
    request_body = MiningConfiguration,
    responses(
        (status = 200, description = "Mining configuration updated successfully", body = MiningConfiguration),
        (status = 400, description = "Invalid configuration", body = ApiError),
        (status = 500, description = "Internal server error", body = ApiError)
    )
)]
async fn update_mining_config(
    request: web::Json<MiningConfiguration>,
    mining: web::Data<Arc<MiningManager>>,
) -> ApiResult<MiningConfiguration> {
    // TODO: Implement real mining config update
    let updated_config = mining.update_mining_config(request.0)?;
    
    Ok(HttpResponse::Ok().json(updated_config))
} 