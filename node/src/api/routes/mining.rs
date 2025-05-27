use crate::api::error::{ApiError, ApiResult};
use crate::api::types::{
    MiningInfo, MiningTemplate, MiningStats, SubmitBlockRequest, 
    SubmitBlockResponse, MiningStatus, MiningConfiguration,
};
use btclib::mining::manager::MiningManager;
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
pub async fn get_mining_info(
    mining: web::Data<Arc<MiningManager>>,
) -> ApiResult<MiningInfo> {
    match mining.get_mining_info() {
        Ok(btclib_info) => {
            // Convert btclib::mining::MiningInfo to api::types::MiningInfo
            let api_info = MiningInfo {
                is_mining: btclib_info.is_mining,
                mining_threads: btclib_info.mining_threads,
                hashrate: btclib_info.hashrate,
                difficulty: btclib_info.difficulty,
                network_hashrate: btclib_info.network_hashrate,
                current_height: btclib_info.current_height,
                seconds_since_last_block: btclib_info.seconds_since_last_block,
                fee_rates: crate::api::types::FeeTiers {
                    high_priority: btclib_info.fee_rates.high_priority,
                    medium_priority: btclib_info.fee_rates.medium_priority,
                    low_priority: btclib_info.fee_rates.low_priority,
                    minimum: btclib_info.fee_rates.minimum,
                },
                environmental_impact: btclib_info.environmental_impact.map(|ei| {
                    crate::api::types::EnvironmentalImpact {
                        power_consumption_watts: ei.power_consumption_watts,
                        carbon_emissions_per_hour: ei.carbon_emissions_per_hour,
                        renewable_percentage: ei.renewable_percentage,
                        energy_efficiency: ei.energy_efficiency,
                    }
                }),
            };
            Ok(api_info)
        },
        Err(e) => Err(ApiError::internal_error(format!("Failed to get mining info: {}", e))),
    }
}

/// Get mining template for block creation
///
/// Returns data needed to construct a block for mining.
#[derive(Debug, Deserialize, IntoParams)]
struct GetMiningTemplateParams {
    /// Comma-separated list of capabilities (default: "standard")
    #[param(default = "standard")]
    capabilities: Option<String>,
    
    /// Maximum number of transactions to include (default: all available)
    max_transactions: Option<u32>,
}

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
pub async fn get_mining_template(
    mining: web::Data<Arc<MiningManager>>,
) -> ApiResult<MiningTemplate> {
    match mining.get_block_template() {
        Ok(template) => Ok(template),
        Err(e) => Err(ApiError::internal_error(format!("Failed to get mining template: {}", e))),
    }
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
pub async fn submit_block(
    request: web::Json<SubmitBlockRequest>,
    mining: web::Data<Arc<MiningManager>>,
) -> ApiResult<SubmitBlockResponse> {
    match mining.submit_block(&request.block_data) {
        Ok(response) => Ok(response),
        Err(e) => Err(ApiError::internal_error(format!("Failed to submit block: {}", e))),
    }
}

/// Get mining statistics
///
/// Returns statistics about mining operations.
#[derive(Debug, Deserialize, IntoParams)]
struct GetMiningStatsParams {
    /// Time period in seconds (default: 3600)
    #[param(default = "3600")]
    period: Option<u64>,
}

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
pub async fn get_mining_stats(
    mining: web::Data<Arc<MiningManager>>,
) -> ApiResult<MiningStats> {
    match mining.get_mining_stats() {
        Ok(stats) => Ok(stats),
        Err(e) => Err(ApiError::internal_error(format!("Failed to get mining stats: {}", e))),
    }
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
pub async fn get_mining_status(
    mining: web::Data<Arc<MiningManager>>,
) -> ApiResult<MiningStatus> {
    match mining.get_mining_status() {
        Ok(status) => Ok(status),
        Err(e) => Err(ApiError::internal_error(format!("Failed to get mining status: {}", e))),
    }
}

/// Start mining
///
/// Starts the mining operation.
#[derive(Debug, Deserialize, Serialize, ToSchema)]
struct StartMiningRequest {
    /// Number of threads to use for mining (default: use system-determined optimal value)
    threads: Option<u32>,
}

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
pub async fn start_mining(
    request: web::Json<StartMiningRequest>,
    mining: web::Data<Arc<MiningManager>>,
) -> ApiResult<HttpResponse> {
    let threads = request.threads;
    
    match mining.start_mining(threads) {
        Ok(_) => Ok(HttpResponse::Ok().finish()),
        Err(e) => Err(ApiError::internal_error(format!("Failed to start mining: {}", e))),
    }
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
pub async fn stop_mining(
    mining: web::Data<Arc<MiningManager>>,
) -> ApiResult<HttpResponse> {
    match mining.stop_mining() {
        Ok(_) => Ok(HttpResponse::Ok().finish()),
        Err(e) => Err(ApiError::internal_error(format!("Failed to stop mining: {}", e))),
    }
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
pub async fn get_mining_config(
    mining: web::Data<Arc<MiningManager>>,
) -> ApiResult<MiningConfiguration> {
    match mining.get_mining_config() {
        Ok(config) => Ok(config),
        Err(e) => Err(ApiError::internal_error(format!("Failed to get mining config: {}", e))),
    }
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
pub async fn update_mining_config(
    request: web::Json<MiningConfiguration>,
    mining: web::Data<Arc<MiningManager>>,
) -> ApiResult<MiningConfiguration> {
    // Convert API MiningConfiguration to btclib MiningConfiguration
    let btclib_config = btclib::mining::manager::MiningConfiguration {
        threads: request.threads,
        intensity: request.intensity,
        target_temperature: request.target_temperature,
        green_mining_enabled: request.green_mining_enabled,
        quantum_resistant: request.quantum_resistant,
        algorithm_params: request.algorithm_params.clone(),
    };
    
    match mining.update_mining_config(btclib_config) {
        Ok(updated_btclib_config) => {
            // Convert back to API MiningConfiguration
            let api_config = MiningConfiguration {
                threads: updated_btclib_config.threads,
                intensity: updated_btclib_config.intensity,
                target_temperature: updated_btclib_config.target_temperature,
                green_mining_enabled: updated_btclib_config.green_mining_enabled,
                quantum_resistant: updated_btclib_config.quantum_resistant,
                algorithm_params: updated_btclib_config.algorithm_params,
            };
            Ok(api_config)
        },
        Err(e) => Err(ApiError::internal_error(format!("Failed to update mining config: {}", e))),
    }
} 