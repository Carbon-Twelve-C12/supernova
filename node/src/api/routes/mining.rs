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
                    // Calculate carbon intensity properly
                    let power_kwh = ei.power_consumption_watts / 1000.0;
                    let carbon_intensity = if power_kwh > 0.0 {
                        ei.carbon_emissions_per_hour / power_kwh
                    } else {
                        0.0
                    };
                    
                    // Calculate carbon offsets (assuming 50% offset for high renewable usage)
                    let carbon_offsets_tons = if ei.renewable_percentage > 50.0 {
                        (ei.carbon_emissions_per_hour * 0.5) / 1_000_000.0 // Convert grams to tons
                    } else {
                        0.0
                    };
                    
                    // Calculate net emissions after offsets
                    let net_emissions_g_per_hour = ei.carbon_emissions_per_hour - (carbon_offsets_tons * 1_000_000.0);
                    
                    // Check if carbon negative
                    let is_carbon_negative = net_emissions_g_per_hour < 0.0;
                    
                    // Calculate environmental score (0-100)
                    let renewable_score = ei.renewable_percentage;
                    let emission_score = 100.0 - (ei.carbon_emissions_per_hour / 1000.0).min(100.0);
                    let environmental_score = (renewable_score + emission_score) / 2.0;
                    
                    // Calculate green mining bonus based on renewable percentage
                    let green_mining_bonus = if ei.renewable_percentage >= 75.0 {
                        10.0 // 10% bonus for >75% renewable
                    } else if ei.renewable_percentage >= 50.0 {
                        5.0 // 5% bonus for >50% renewable
                    } else {
                        0.0
                    };
                    
                    crate::api::types::environmental::EnvironmentalImpact {
                        carbon_emissions_g_per_hour: ei.carbon_emissions_per_hour,
                        renewable_percentage: ei.renewable_percentage,
                        carbon_intensity,
                        carbon_offsets_tons,
                        net_emissions_g_per_hour,
                        is_carbon_negative,
                        environmental_score,
                        green_mining_bonus,
                        data_sources: vec!["btclib".to_string()],
                        calculated_at: chrono::Utc::now().timestamp() as u64,
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
    params: web::Query<GetMiningTemplateParams>,
    mining: web::Data<Arc<MiningManager>>,
) -> ApiResult<MiningTemplate> {
    let capabilities = params.capabilities.as_deref().unwrap_or("standard");
    let max_transactions = params.max_transactions;
    
    match mining.get_mining_template(capabilities, max_transactions) {
        Ok(btclib_template) => {
            // Convert btclib template to API template
            let api_template = MiningTemplate {
                version: btclib_template.version,
                prev_hash: btclib_template.prev_hash,
                timestamp: btclib_template.timestamp,
                height: btclib_template.height,
                target: btclib_template.target,
                merkle_root: btclib_template.merkle_root,
                transactions: btclib_template.transactions.into_iter().map(|tx| {
                    crate::api::types::TemplateTransaction {
                        txid: tx.txid,
                        data: tx.data,
                        fee: tx.fee,
                        weight: tx.weight,
                        ancestor_fee: tx.ancestor_fee,
                        ancestor_weight: tx.ancestor_weight,
                    }
                }).collect(),
                total_fees: btclib_template.total_fees,
                size: btclib_template.size,
                weight: btclib_template.weight,
                estimated_time_to_mine: btclib_template.estimated_time_to_mine,
                environmental_data: btclib_template.environmental_data.map(|ed| {
                    crate::api::types::TemplateEnvironmentalData {
                        estimated_energy_kwh: ed.estimated_energy_kwh,
                        estimated_carbon_grams: ed.estimated_carbon_grams,
                        green_mining_bonus: ed.green_mining_bonus,
                    }
                }),
            };
            Ok(api_template)
        },
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
    // Convert hex string to bytes
    let block_bytes = hex::decode(&request.block_data)
        .map_err(|_| ApiError::bad_request("Invalid block data format"))?;
    
    match mining.submit_block(&block_bytes) {
        Ok(btclib_response) => {
            // Convert btclib response to API response
            Ok(SubmitBlockResponse {
                accepted: btclib_response.accepted,
                block_hash: btclib_response.block_hash,
                reject_reason: btclib_response.reject_reason,
            })
        },
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
    // Default to 1 hour period
    let period = 3600u64;
    
    match mining.get_mining_stats(period) {
        Ok(btclib_stats) => {
            // Convert btclib stats to API stats
            Ok(MiningStats {
                total_hashes: btclib_stats.total_hashes,
                blocks_found: btclib_stats.blocks_found,
                uptime_seconds: btclib_stats.uptime_seconds,
                avg_hashrate_1h: btclib_stats.avg_hashrate_1h,
                current_difficulty: btclib_stats.current_difficulty,
                estimated_time_to_block: btclib_stats.estimated_time_to_block,
                power_consumption_watts: btclib_stats.power_consumption_watts,
                energy_efficiency: btclib_stats.energy_efficiency,
                carbon_emissions_per_hash: btclib_stats.carbon_emissions_per_hash,
                renewable_percentage: btclib_stats.renewable_percentage,
            })
        },
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
        Ok(btclib_status) => {
            // Convert btclib status to API status
            Ok(MiningStatus {
                state: btclib_status.state,
                active_workers: btclib_status.active_workers,
                template_age_seconds: btclib_status.template_age_seconds,
                hashrate_1m: btclib_status.hashrate_1m,
                hashrate_5m: btclib_status.hashrate_5m,
                hashrate_15m: btclib_status.hashrate_15m,
                hardware_temperature: btclib_status.hardware_temperature,
                fan_speed_percentage: btclib_status.fan_speed_percentage,
            })
        },
        Err(e) => Err(ApiError::internal_error(format!("Failed to get mining status: {}", e))),
    }
}

/// Start mining
///
/// Starts the mining operation.
#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct StartMiningRequest {
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
        Ok(btclib_config) => {
            // Convert btclib config to API config
            Ok(MiningConfiguration {
                threads: btclib_config.threads,
                intensity: btclib_config.intensity,
                target_temperature: btclib_config.target_temperature,
                green_mining_enabled: btclib_config.green_mining_enabled,
                quantum_resistant: btclib_config.quantum_resistant,
                algorithm_params: btclib_config.algorithm_params,
            })
        },
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