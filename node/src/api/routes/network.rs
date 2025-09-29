//! Network API routes
//!
//! This module implements API endpoints for network operations.

use crate::api::error::ApiError;
use crate::api::types::PeerAddRequest;
use crate::api_facade::ApiFacade;
use actix_web::{web, HttpResponse};
use serde::Deserialize;
use utoipa::{IntoParams, ToSchema};
use std::sync::Arc;
use serde_json;

/// Configure network API routes
pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/network")
            .route("/info", web::get().to(get_network_info))
            .route("/connection_count", web::get().to(get_connection_count))
            .route("/peers", web::get().to(get_peers))
            .route("/peers/{peer_id}", web::get().to(get_peer))
            .route("/peers", web::post().to(add_peer))
            .route("/peers/{peer_id}", web::delete().to(remove_peer))
            .route("/bandwidth", web::get().to(get_bandwidth_usage)),
    );
}

/// Get network information
///
/// Returns information about the node's network status.
#[utoipa::path(
    get,
    path = "/api/v1/network/info",
    responses(
        (status = 200, description = "Network information retrieved successfully", body = NetworkInfo),
        (status = 500, description = "Internal server error", body = ApiError)
    )
)]
pub async fn get_network_info(
    node: web::Data<Arc<ApiFacade>>,
) -> Result<HttpResponse, actix_web::Error> {
    let network = node.network();
    match network.get_network_info().await {
        Ok(info) => Ok(HttpResponse::Ok().json(info)),
        Err(e) => Ok(HttpResponse::InternalServerError().json(
            ApiError::internal_error(format!("Failed to retrieve network info: {}", e))
        )),
    }
}

/// Get connection count
///
/// Returns the number of connections by type.
#[utoipa::path(
    get,
    path = "/api/v1/network/connection_count",
    responses(
        (status = 200, description = "Connection count retrieved successfully", body = ConnectionCount),
        (status = 500, description = "Internal server error", body = ApiError)
    )
)]
pub async fn get_connection_count(
    node: web::Data<Arc<ApiFacade>>,
) -> Result<HttpResponse, actix_web::Error> {
    let network = node.network();
    match network.get_connection_count().await {
        Ok(count) => Ok(HttpResponse::Ok().json(count)),
        Err(e) => Ok(HttpResponse::InternalServerError().json(
            ApiError::internal_error(format!("Failed to retrieve connection count: {}", e))
        )),
    }
}

/// Get a list of connected peers
///
/// Returns information about all peers currently connected to the node.
#[derive(Debug, Deserialize, IntoParams)]
struct GetPeersParams {
    /// Optional connection state filter
    connection_state: Option<String>,
    
    /// Include detailed information (default: false)
    #[param(default = "false")]
    verbose: Option<bool>,
}

#[utoipa::path(
    get,
    path = "/api/v1/network/peers",
    params(
        GetPeersParams
    ),
    responses(
        (status = 200, description = "Peer list retrieved successfully", body = Vec<PeerInfo>),
        (status = 400, description = "Invalid request parameters", body = ApiError),
        (status = 500, description = "Internal server error", body = ApiError)
    )
)]
pub async fn get_peers(
    params: web::Query<GetPeersParams>,
    node: web::Data<Arc<ApiFacade>>,
) -> Result<HttpResponse, actix_web::Error> {
    let network = node.network();
    let connection_state = params.connection_state.clone();
    let verbose = params.verbose.unwrap_or(false);
    
    match network.get_peers().await {
        Ok(mut peers) => {
            if let Some(state) = connection_state {
                peers.retain(|p| p.direction == state);
            }
            Ok(HttpResponse::Ok().json(peers))
        },
        Err(e) => Ok(HttpResponse::InternalServerError().json(
            ApiError::internal_error(format!("Failed to retrieve peers: {}", e))
        )),
    }
}

/// Get information about a specific peer
///
/// Returns detailed information about a specific connected peer.
#[utoipa::path(
    get,
    path = "/api/v1/network/peers/{peer_id}",
    params(
        ("peer_id" = String, Path, description = "Peer ID")
    ),
    responses(
        (status = 200, description = "Peer information retrieved successfully", body = PeerInfo),
        (status = 404, description = "Peer not found", body = ApiError),
        (status = 500, description = "Internal server error", body = ApiError)
    )
)]
pub async fn get_peer(
    path: web::Path<String>,
    node: web::Data<Arc<ApiFacade>>,
) -> Result<HttpResponse, actix_web::Error> {
    let network = node.network();
    let peer_id = path.into_inner();
    
    match network.get_peer(&peer_id).await {
        Ok(Some(peer)) => Ok(HttpResponse::Ok().json(peer)),
        Ok(None) => Ok(HttpResponse::NotFound().json(
            ApiError::not_found("Peer not found")
        )),
        Err(e) => Ok(HttpResponse::InternalServerError().json(
            ApiError::internal_error(format!("Failed to get peer: {}", e))
        )),
    }
}

/// Add a new peer connection
///
/// Attempts to connect to a new peer by address.
#[utoipa::path(
    post,
    path = "/api/v1/network/peers",
    request_body = PeerAddRequest,
    responses(
        (status = 200, description = "Peer addition initiated successfully", body = PeerAddResponse),
        (status = 400, description = "Invalid peer address format", body = ApiError),
        (status = 500, description = "Internal server error", body = ApiError)
    )
)]
pub async fn add_peer(
    request: web::Json<PeerAddRequest>,
    node: web::Data<Arc<ApiFacade>>,
) -> Result<HttpResponse, actix_web::Error> {
    let network = node.network();
    let address = &request.address;
    let permanent = request.permanent.unwrap_or(false);
    
    match network.add_peer(address, permanent).await {
        Ok(result) => Ok(HttpResponse::Ok().json(result)),
        Err(e) => Ok(HttpResponse::InternalServerError().json(
            ApiError::internal_error(format!("Failed to add peer: {}", e))
        )),
    }
}

/// Remove a peer connection
///
/// Disconnects from a specific peer.
#[utoipa::path(
    delete,
    path = "/api/v1/network/peers/{peer_id}",
    params(
        ("peer_id" = String, Path, description = "Peer ID")
    ),
    responses(
        (status = 200, description = "Peer removed successfully"),
        (status = 404, description = "Peer not found", body = ApiError),
        (status = 500, description = "Internal server error", body = ApiError)
    )
)]
pub async fn remove_peer(
    path: web::Path<String>,
    node: web::Data<Arc<ApiFacade>>,
) -> Result<HttpResponse, actix_web::Error> {
    let network = node.network();
    let peer_id = path.into_inner();
    
    match network.remove_peer(&peer_id).await {
        Ok(success) => {
            if success {
                Ok(HttpResponse::Ok().json(serde_json::json!({
                    "success": true,
                    "message": "Peer removed successfully"
                })))
            } else {
                Ok(HttpResponse::NotFound().json(
                    ApiError::not_found("Peer not found")
                ))
            }
        },
        Err(e) => Ok(HttpResponse::InternalServerError().json(
            ApiError::internal_error(format!("Failed to remove peer: {}", e))
        )),
    }
}

/// Get bandwidth usage statistics
///
/// Returns information about the node's bandwidth usage.
#[derive(Debug, Deserialize, IntoParams)]
struct GetBandwidthParams {
    /// Time period in seconds (default: 3600)
    #[param(default = "3600")]
    period: Option<u64>,
}

#[utoipa::path(
    get,
    path = "/api/v1/network/bandwidth",
    params(
        GetBandwidthParams
    ),
    responses(
        (status = 200, description = "Bandwidth usage statistics retrieved successfully", body = BandwidthUsage),
        (status = 400, description = "Invalid request parameters", body = ApiError),
        (status = 500, description = "Internal server error", body = ApiError)
    )
)]
pub async fn get_bandwidth_usage(
    params: web::Query<GetBandwidthParams>,
    node: web::Data<Arc<ApiFacade>>,
) -> Result<HttpResponse, actix_web::Error> {
    let network = node.network();
    let period = params.period.unwrap_or(3600);
    
    match network.get_bandwidth_usage(period).await {
        Ok(usage) => Ok(HttpResponse::Ok().json(usage)),
        Err(e) => Ok(HttpResponse::InternalServerError().json(
            ApiError::internal_error(format!("Failed to get bandwidth usage: {}", e))
        )),
    }
} 