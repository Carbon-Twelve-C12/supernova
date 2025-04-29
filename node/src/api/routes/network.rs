use crate::api::error::{ApiError, ApiResult};
use crate::api::types::{
    NetworkInfo, PeerInfo, PeerConnectionStatus, BandwidthUsage, 
    PeerAddRequest, PeerAddResponse, NodeAddress, ConnectionCount,
};
use crate::network::NetworkManager;
use actix_web::{web, HttpResponse};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use std::sync::Arc;

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
async fn get_network_info(
    network: web::Data<Arc<NetworkManager>>,
) -> ApiResult<NetworkInfo> {
    // Retrieve network information from the NetworkManager
    let info = match network.get_network_info() {
        Ok(info) => info,
        Err(e) => return Err(ApiError::internal_error(format!("Failed to retrieve network info: {}", e))),
    };
    
    Ok(HttpResponse::Ok().json(info))
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
async fn get_connection_count(
    network: web::Data<Arc<NetworkManager>>,
) -> ApiResult<ConnectionCount> {
    // Retrieve connection count from the NetworkManager
    let count = match network.get_connection_count() {
        Ok(count) => count,
        Err(e) => return Err(ApiError::internal_error(format!("Failed to retrieve connection count: {}", e))),
    };
    
    Ok(HttpResponse::Ok().json(count))
}

/// Get a list of connected peers
///
/// Returns information about all peers currently connected to the node.
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
#[derive(Debug, Deserialize, IntoParams)]
struct GetPeersParams {
    /// Optional connection state filter
    connection_state: Option<String>,
    
    /// Include detailed information (default: false)
    #[param(default = "false")]
    verbose: Option<bool>,
}

async fn get_peers(
    params: web::Query<GetPeersParams>,
    network: web::Data<Arc<NetworkManager>>,
) -> ApiResult<Vec<PeerInfo>> {
    let connection_state = params.connection_state.clone();
    let verbose = params.verbose.unwrap_or(false);
    
    // Retrieve peers from the NetworkManager with the specified filters
    let peers = match network.get_peers(connection_state, verbose) {
        Ok(peers) => peers,
        Err(e) => return Err(ApiError::internal_error(format!("Failed to retrieve peers: {}", e))),
    };
    
    Ok(HttpResponse::Ok().json(peers))
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
async fn get_peer(
    path: web::Path<String>,
    network: web::Data<Arc<NetworkManager>>,
) -> ApiResult<PeerInfo> {
    let peer_id = path.into_inner();
    
    // TODO: Implement real peer information retrieval
    match network.get_peer(&peer_id)? {
        Some(peer) => Ok(HttpResponse::Ok().json(peer)),
        None => Err(ApiError::not_found("Peer not found")),
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
async fn add_peer(
    request: web::Json<PeerAddRequest>,
    network: web::Data<Arc<NetworkManager>>,
) -> ApiResult<PeerAddResponse> {
    let address = request.address.clone();
    let permanent = request.permanent.unwrap_or(false);
    
    // TODO: Implement real peer addition
    let result = network.add_peer(&address, permanent)?;
    
    Ok(HttpResponse::Ok().json(result))
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
async fn remove_peer(
    path: web::Path<String>,
    network: web::Data<Arc<NetworkManager>>,
) -> ApiResult<HttpResponse> {
    let peer_id = path.into_inner();
    
    // TODO: Implement real peer removal
    if network.remove_peer(&peer_id)? {
        Ok(HttpResponse::Ok().finish())
    } else {
        Err(ApiError::not_found("Peer not found"))
    }
}

/// Get bandwidth usage statistics
///
/// Returns information about the node's bandwidth usage.
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
#[derive(Debug, Deserialize, IntoParams)]
struct GetBandwidthParams {
    /// Time period in seconds (default: 3600)
    #[param(default = "3600")]
    period: Option<u64>,
}

async fn get_bandwidth_usage(
    params: web::Query<GetBandwidthParams>,
    network: web::Data<Arc<NetworkManager>>,
) -> ApiResult<BandwidthUsage> {
    let period = params.period.unwrap_or(3600);
    
    // TODO: Implement real bandwidth usage retrieval
    let usage = network.get_bandwidth_usage(period)?;
    
    Ok(HttpResponse::Ok().json(usage))
} 