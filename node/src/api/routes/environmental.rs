use crate::api::error::{ApiError, ApiResult};
use crate::api::types::EnvironmentalSettings;
use crate::environmental::EnvironmentalMonitor;
use actix_web::{web, HttpResponse};
use serde::Deserialize;
use utoipa::{IntoParams, ToSchema};
use std::sync::Arc;

/// Configure environmental API routes
pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/environmental")
            .route("/impact", web::get().to(get_environmental_impact))
            .route("/energy", web::get().to(get_energy_usage))
            .route("/carbon", web::get().to(get_carbon_footprint))
            .route("/resources", web::get().to(get_resource_utilization))
            .route("/settings", web::get().to(get_environmental_settings))
            .route("/settings", web::put().to(update_environmental_settings)),
    );
}

/// Get environmental impact data
///
/// Returns comprehensive data about the node's environmental impact.
#[derive(Debug, Deserialize, IntoParams)]
struct GetEnvironmentalImpactParams {
    /// Time period in seconds for which to retrieve data (default: 86400 - 1 day)
    #[param(default = "86400")]
    period: Option<u64>,
    
    /// Level of detail for the report (default: "standard")
    #[param(default = "standard")]
    detail: Option<String>,
}

#[utoipa::path(
    get,
    path = "/api/v1/environmental/impact",
    params(
        GetEnvironmentalImpactParams
    ),
    responses(
        (status = 200, description = "Environmental impact data retrieved successfully", body = EnvironmentalImpact),
        (status = 400, description = "Invalid request parameters", body = ApiError),
        (status = 500, description = "Internal server error", body = ApiError)
    )
)]
pub async fn get_environmental_impact(
    params: web::Query<GetEnvironmentalImpactParams>,
    environmental: web::Data<Arc<EnvironmentalMonitor>>,
) -> ApiResult<HttpResponse> {
    let period = params.period.unwrap_or(3600);
    let detail = params.detail.as_deref().unwrap_or("standard");
    
    match environmental.get_environmental_impact(period, detail) {
        Ok(impact) => Ok(HttpResponse::Ok().json(impact)),
        Err(e) => Err(ApiError::internal_error(format!("Failed to get environmental impact: {}", e))),
    }
}

/// Get energy usage data
///
/// Returns detailed information about the node's energy consumption.
#[derive(Debug, Deserialize, IntoParams)]
struct GetEnergyUsageParams {
    /// Time period in seconds for which to retrieve data (default: 3600 - 1 hour)
    #[param(default = "3600")]
    period: Option<u64>,
    
    /// Whether to include historical data (default: false)
    #[param(default = "false")]
    include_history: Option<bool>,
}

#[utoipa::path(
    get,
    path = "/api/v1/environmental/energy",
    params(
        GetEnergyUsageParams
    ),
    responses(
        (status = 200, description = "Energy usage data retrieved successfully", body = EnergyUsage),
        (status = 400, description = "Invalid request parameters", body = ApiError),
        (status = 500, description = "Internal server error", body = ApiError)
    )
)]
pub async fn get_energy_usage(
    params: web::Query<GetEnergyUsageParams>,
    environmental: web::Data<Arc<EnvironmentalMonitor>>,
) -> ApiResult<HttpResponse> {
    let period = params.period.unwrap_or(3600);
    let include_history = params.include_history.unwrap_or(false);
    
    match environmental.get_energy_usage(period, include_history) {
        Ok(energy_data) => Ok(HttpResponse::Ok().json(energy_data)),
        Err(e) => Err(ApiError::internal_error(format!("Failed to get energy usage: {}", e))),
    }
}

/// Get carbon footprint data
///
/// Returns information about the node's carbon emissions.
#[derive(Debug, Deserialize, IntoParams)]
struct GetCarbonFootprintParams {
    /// Time period in seconds for which to retrieve data (default: 86400 - 1 day)
    #[param(default = "86400")]
    period: Option<u64>,
    
    /// Whether to include offset information (default: true)
    #[param(default = "true")]
    include_offsets: Option<bool>,
}

#[utoipa::path(
    get,
    path = "/api/v1/environmental/carbon",
    params(
        GetCarbonFootprintParams
    ),
    responses(
        (status = 200, description = "Carbon footprint data retrieved successfully", body = CarbonFootprint),
        (status = 400, description = "Invalid request parameters", body = ApiError),
        (status = 500, description = "Internal server error", body = ApiError)
    )
)]
pub async fn get_carbon_footprint(
    params: web::Query<GetCarbonFootprintParams>,
    environmental: web::Data<Arc<EnvironmentalMonitor>>,
) -> ApiResult<HttpResponse> {
    let period = params.period.unwrap_or(3600);
    let include_offsets = params.include_offsets.unwrap_or(false);
    
    match environmental.get_carbon_footprint(period, include_offsets) {
        Ok(carbon_data) => Ok(HttpResponse::Ok().json(carbon_data)),
        Err(e) => Err(ApiError::internal_error(format!("Failed to get carbon footprint: {}", e))),
    }
}

/// Get current resource utilization
///
/// Returns CPU, memory, and storage utilization data.
#[derive(Debug, Deserialize, IntoParams)]
struct ResourceUtilizationParams {
    /// Time period in seconds for which to retrieve data (default: 3600 - 1 hour)
    #[param(default = "3600")]
    period: Option<u64>,
}

#[utoipa::path(
    get,
    path = "/environmental/resources",
    params(
        ResourceUtilizationParams
    ),
    responses(
        (status = 200, description = "Resource utilization data", body = ResourceUtilization),
        (status = 500, description = "Internal server error")
    ),
    tag = "Environmental"
)]
pub async fn get_resource_utilization(
    environmental: web::Data<Arc<EnvironmentalMonitor>>,
    params: web::Query<ResourceUtilizationParams>,
) -> ApiResult<HttpResponse> {
    let period = params.period.unwrap_or(3600);
    
    match environmental.get_resource_utilization(period) {
        Ok(resource_data) => Ok(HttpResponse::Ok().json(resource_data)),
        Err(e) => Err(ApiError::internal_error(format!("Failed to get resource utilization: {}", e))),
    }
}

/// Get environmental monitoring settings
///
/// Returns the current environmental monitoring and optimization settings.
#[utoipa::path(
    get,
    path = "/api/v1/environmental/settings",
    responses(
        (status = 200, description = "Environmental settings retrieved successfully", body = EnvironmentalSettings),
        (status = 500, description = "Internal server error", body = ApiError)
    )
)]
pub async fn get_environmental_settings(
    environmental: web::Data<Arc<EnvironmentalMonitor>>,
) -> ApiResult<HttpResponse> {
    match environmental.get_settings() {
        Ok(settings) => Ok(HttpResponse::Ok().json(settings)),
        Err(e) => Err(ApiError::internal_error(format!("Failed to get environmental settings: {}", e))),
    }
}

/// Update environmental monitoring settings
///
/// Updates the environmental monitoring and optimization settings.
#[utoipa::path(
    put,
    path = "/api/v1/environmental/settings",
    request_body = EnvironmentalSettings,
    responses(
        (status = 200, description = "Environmental settings updated successfully", body = EnvironmentalSettings),
        (status = 400, description = "Invalid settings", body = ApiError),
        (status = 500, description = "Internal server error", body = ApiError)
    )
)]
pub async fn update_environmental_settings(
    request: web::Json<EnvironmentalSettings>,
    environmental: web::Data<Arc<EnvironmentalMonitor>>,
) -> ApiResult<HttpResponse> {
    match environmental.update_settings(request.into_inner()) {
        Ok(updated_settings) => Ok(HttpResponse::Ok().json(updated_settings)),
        Err(e) => Err(ApiError::internal_error(format!("Failed to update environmental settings: {}", e))),
    }
} 