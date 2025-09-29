use crate::api::error::{ApiError, ApiResult};
use crate::api::types::{CloseChannelRequest, InvoiceRequest, OpenChannelRequest, PaymentRequest};
use crate::node::Node;
use actix_web::{web, HttpResponse};
use serde::Deserialize;
use std::sync::Arc;
use utoipa::{IntoParams, ToSchema};

/// Configure lightning API routes
pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/lightning")
            .route("/info", web::get().to(get_lightning_info))
            .route("/channels", web::get().to(get_channels))
            .route("/channel/{channel_id}", web::get().to(get_channel))
            .route("/channel", web::post().to(open_channel))
            .route("/channel", web::delete().to(close_channel))
            .route("/payments", web::get().to(get_payments))
            .route("/pay", web::post().to(send_payment))
            .route("/invoices", web::get().to(get_invoices))
            .route("/invoice", web::post().to(create_invoice))
            .route("/nodes", web::get().to(get_network_nodes))
            .route("/node/{node_id}", web::get().to(get_node_info))
            .route("/routes", web::get().to(find_route)),
    );
}

/// Placeholder handler for Lightning Network endpoints
async fn lightning_unavailable() -> ApiResult<HttpResponse> {
    Ok(HttpResponse::ServiceUnavailable().json(serde_json::json!({
        "error": "Lightning Network API temporarily disabled",
        "message": "Lightning Network functionality is being refactored for improved thread safety",
        "status": "service_unavailable"
    })))
}

/// Get Lightning Network information
///
/// Returns general information about the node's Lightning Network status.
#[utoipa::path(
    get,
    path = "/api/v1/lightning/info",
    responses(
        (status = 200, description = "Lightning Network information retrieved successfully", body = LightningInfo),
        (status = 500, description = "Internal server error", body = ApiError)
    )
)]
pub async fn get_lightning_info(node: web::Data<Arc<Node>>) -> ApiResult<HttpResponse> {
    // Check if Lightning Network is enabled
    let lightning_manager = node
        .lightning()
        .ok_or_else(|| ApiError::service_unavailable("Lightning Network is not enabled"))?;

    // Get Lightning info
    let manager = lightning_manager
        .read()
        .map_err(|e| ApiError::internal_error(format!("Lightning manager lock poisoned: {}", e)))?;
    let info = manager
        .get_info()
        .map_err(|e| ApiError::internal_error(format!("Failed to get Lightning info: {}", e)))?;

    Ok(HttpResponse::Ok().json(info))
}

/// Get a list of Lightning Network channels
///
/// Returns information about the node's active Lightning Network channels.
#[derive(Debug, Deserialize, IntoParams)]
struct GetChannelsParams {
    /// Whether to include inactive channels (default: false)
    #[param(default = "false")]
    include_inactive: Option<bool>,

    /// Whether to include pending channels (default: true)
    #[param(default = "true")]
    include_pending: Option<bool>,
}

#[utoipa::path(
    get,
    path = "/api/v1/lightning/channels",
    params(
        GetChannelsParams
    ),
    responses(
        (status = 200, description = "Lightning Network channels retrieved successfully", body = Vec<LightningChannel>),
        (status = 400, description = "Invalid request parameters", body = ApiError),
        (status = 500, description = "Internal server error", body = ApiError)
    )
)]
pub async fn get_channels(
    params: web::Query<GetChannelsParams>,
    node: web::Data<Arc<Node>>,
) -> ApiResult<HttpResponse> {
    // Check if Lightning Network is enabled
    let lightning_manager = node
        .lightning()
        .ok_or_else(|| ApiError::service_unavailable("Lightning Network is not enabled"))?;

    // Get channels
    let manager = lightning_manager
        .read()
        .map_err(|e| ApiError::internal_error(format!("Lightning manager lock poisoned: {}", e)))?;
    let channels = manager
        .get_channels(
            params.include_inactive.unwrap_or(false),
            params.include_pending.unwrap_or(true),
        )
        .map_err(|e| ApiError::internal_error(format!("Failed to list channels: {}", e)))?;

    Ok(HttpResponse::Ok().json(channels))
}

/// Get a specific Lightning Network channel
///
/// Returns detailed information about a specific Lightning Network channel.
#[utoipa::path(
    get,
    path = "/api/v1/lightning/channel/{channel_id}",
    params(
        ("channel_id" = String, Path, description = "Channel ID")
    ),
    responses(
        (status = 200, description = "Lightning Network channel retrieved successfully", body = LightningChannel),
        (status = 404, description = "Channel not found", body = ApiError),
        (status = 500, description = "Internal server error", body = ApiError)
    )
)]
pub async fn get_channel(
    path: web::Path<String>,
    node: web::Data<Arc<Node>>,
) -> ApiResult<HttpResponse> {
    let channel_id = path.into_inner();

    // Check if Lightning Network is enabled
    let lightning_manager = node
        .lightning()
        .ok_or_else(|| ApiError::service_unavailable("Lightning Network is not enabled"))?;

    // Get channel info
    let manager = lightning_manager
        .read()
        .map_err(|e| ApiError::internal_error(format!("Lightning manager lock poisoned: {}", e)))?;
    let channel = manager
        .get_channel(&channel_id)
        .map_err(|e| ApiError::internal_error(format!("Failed to get channel: {}", e)))?
        .ok_or_else(|| ApiError::not_found(format!("Channel {} not found", channel_id)))?;

    Ok(HttpResponse::Ok().json(channel))
}

/// Open a new Lightning Network channel
///
/// Opens a new payment channel with a remote Lightning Network node.
#[utoipa::path(
    post,
    path = "/api/v1/lightning/channel",
    request_body = OpenChannelRequest,
    responses(
        (status = 200, description = "Channel opening initiated successfully", body = OpenChannelResponse),
        (status = 400, description = "Invalid request parameters", body = ApiError),
        (status = 500, description = "Internal server error", body = ApiError)
    )
)]
pub async fn open_channel(
    request: web::Json<OpenChannelRequest>,
    node: web::Data<Arc<Node>>,
) -> ApiResult<HttpResponse> {
    // Check if Lightning Network is enabled
    let lightning_manager = node
        .lightning()
        .ok_or_else(|| ApiError::service_unavailable("Lightning Network is not enabled"))?;

    // Open channel
    let manager = lightning_manager
        .write()
        .map_err(|e| ApiError::internal_error(format!("Lightning manager lock poisoned: {}", e)))?;
    let response = manager
        .open_channel(
            &request.node_id,
            request.local_funding_amount,
            request.push_amount_msat,
            request.private.unwrap_or(false),
            request.min_htlc_msat,
        )
        .await
        .map_err(|e| ApiError::internal_error(format!("Failed to open channel: {}", e)))?;

    Ok(HttpResponse::Ok().json(response))
}

/// Close a Lightning Network channel
///
/// Closes an existing payment channel with a remote Lightning Network node.
#[utoipa::path(
    delete,
    path = "/api/v1/lightning/channel",
    request_body = CloseChannelRequest,
    responses(
        (status = 200, description = "Channel closing initiated successfully"),
        (status = 400, description = "Invalid request parameters", body = ApiError),
        (status = 404, description = "Channel not found", body = ApiError),
        (status = 500, description = "Internal server error", body = ApiError)
    )
)]
pub async fn close_channel(
    request: web::Json<CloseChannelRequest>,
    node: web::Data<Arc<Node>>,
) -> ApiResult<HttpResponse> {
    // Check if Lightning Network is enabled
    let lightning_manager = node
        .lightning()
        .ok_or_else(|| ApiError::service_unavailable("Lightning Network is not enabled"))?;

    // Close channel
    let manager = lightning_manager
        .write()
        .map_err(|e| ApiError::internal_error(format!("Lightning manager lock poisoned: {}", e)))?;
    let success = manager
        .close_channel(&request.channel_id, request.force.unwrap_or(false))
        .await
        .map_err(|e| ApiError::internal_error(format!("Failed to close channel: {}", e)))?;

    if success {
        Ok(HttpResponse::Ok().json(serde_json::json!({
            "status": "closing",
            "channel_id": request.channel_id
        })))
    } else {
        Err(ApiError::internal_error(
            "Failed to initiate channel closure",
        ))
    }
}

/// Get a list of Lightning Network payments
///
/// Returns information about the node's Lightning Network payments.
#[derive(Debug, Deserialize, IntoParams)]
struct GetPaymentsParams {
    /// Optional payment index to start from
    index_offset: Option<u64>,

    /// Maximum number of payments to retrieve (default: 100)
    #[param(default = "100")]
    max_payments: Option<u64>,

    /// Whether to include pending payments (default: true)
    #[param(default = "true")]
    include_pending: Option<bool>,
}

#[utoipa::path(
    get,
    path = "/api/v1/lightning/payments",
    params(
        GetPaymentsParams
    ),
    responses(
        (status = 200, description = "Lightning Network payments retrieved successfully", body = Vec<LightningPayment>),
        (status = 400, description = "Invalid request parameters", body = ApiError),
        (status = 500, description = "Internal server error", body = ApiError)
    )
)]
pub async fn get_payments(
    params: web::Query<GetPaymentsParams>,
    node: web::Data<Arc<Node>>,
) -> ApiResult<HttpResponse> {
    // Check if Lightning Network is enabled
    let lightning_manager = node
        .lightning()
        .ok_or_else(|| ApiError::service_unavailable("Lightning Network is not enabled"))?;

    // Get payments
    let manager = lightning_manager
        .read()
        .map_err(|e| ApiError::internal_error(format!("Lightning manager lock poisoned: {}", e)))?;
    let payments = manager
        .get_payments(
            params.index_offset.unwrap_or(0),
            params.max_payments.unwrap_or(100),
            params.include_pending.unwrap_or(true),
        )
        .map_err(|e| ApiError::internal_error(format!("Failed to list payments: {}", e)))?;

    Ok(HttpResponse::Ok().json(payments))
}

/// Send a Lightning Network payment
///
/// Sends a payment over the Lightning Network.
#[utoipa::path(
    post,
    path = "/api/v1/lightning/pay",
    request_body = PaymentRequest,
    responses(
        (status = 200, description = "Payment sent successfully", body = PaymentResponse),
        (status = 400, description = "Invalid request parameters", body = ApiError),
        (status = 500, description = "Internal server error", body = ApiError)
    )
)]
pub async fn send_payment(
    request: web::Json<PaymentRequest>,
    node: web::Data<Arc<Node>>,
) -> ApiResult<HttpResponse> {
    // Check if Lightning Network is enabled
    let lightning_manager = node
        .lightning()
        .ok_or_else(|| ApiError::service_unavailable("Lightning Network is not enabled"))?;

    // Send payment
    let manager = lightning_manager
        .write()
        .map_err(|e| ApiError::internal_error(format!("Lightning manager lock poisoned: {}", e)))?;
    let response = manager
        .send_payment(
            &request.payment_request,
            request.amount_msat,
            request.timeout_seconds.unwrap_or(60),
            request.fee_limit_msat,
        )
        .await;

    let response =
        response.map_err(|e| ApiError::internal_error(format!("Failed to send payment: {}", e)))?;

    Ok(HttpResponse::Ok().json(response))
}

/// Get a list of Lightning Network invoices
///
/// Returns information about the node's Lightning Network invoices.
#[derive(Debug, Deserialize, IntoParams)]
struct GetInvoicesParams {
    /// Whether to include pending invoices (default: true)
    #[param(default = "true")]
    pending_only: Option<bool>,

    /// Optional invoice index to start from
    index_offset: Option<u64>,

    /// Maximum number of invoices to retrieve (default: 100)
    #[param(default = "100")]
    num_max_invoices: Option<u64>,
}

#[utoipa::path(
    get,
    path = "/api/v1/lightning/invoices",
    params(
        GetInvoicesParams
    ),
    responses(
        (status = 200, description = "Lightning Network invoices retrieved successfully", body = Vec<LightningInvoice>),
        (status = 400, description = "Invalid request parameters", body = ApiError),
        (status = 500, description = "Internal server error", body = ApiError)
    )
)]
pub async fn get_invoices(
    params: web::Query<GetInvoicesParams>,
    node: web::Data<Arc<Node>>,
) -> ApiResult<HttpResponse> {
    // Check if Lightning Network is enabled
    let lightning_manager = node
        .lightning()
        .ok_or_else(|| ApiError::service_unavailable("Lightning Network is not enabled"))?;

    // Get invoices
    let manager = lightning_manager
        .read()
        .map_err(|e| ApiError::internal_error(format!("Lightning manager lock poisoned: {}", e)))?;
    let invoices = manager
        .get_invoices(
            params.pending_only.unwrap_or(true),
            params.index_offset.unwrap_or(0),
            params.num_max_invoices.unwrap_or(100),
        )
        .map_err(|e| ApiError::internal_error(format!("Failed to list invoices: {}", e)))?;

    Ok(HttpResponse::Ok().json(invoices))
}

/// Create a Lightning Network invoice
///
/// Creates a new invoice for receiving a payment over the Lightning Network.
#[utoipa::path(
    post,
    path = "/api/v1/lightning/invoice",
    request_body = InvoiceRequest,
    responses(
        (status = 200, description = "Invoice created successfully", body = InvoiceResponse),
        (status = 400, description = "Invalid request parameters", body = ApiError),
        (status = 500, description = "Internal server error", body = ApiError)
    )
)]
pub async fn create_invoice(
    request: web::Json<InvoiceRequest>,
    node: web::Data<Arc<Node>>,
) -> ApiResult<HttpResponse> {
    // Check if Lightning Network is enabled
    let lightning_manager = node
        .lightning()
        .ok_or_else(|| ApiError::service_unavailable("Lightning Network is not enabled"))?;

    // Create invoice
    let manager = lightning_manager
        .write()
        .map_err(|e| ApiError::internal_error(format!("Lightning manager lock poisoned: {}", e)))?;
    let response = manager
        .create_invoice(
            request.value_msat,
            request.memo.as_deref().unwrap_or(""),
            request.expiry.unwrap_or(3600),
            request.private.unwrap_or(false),
        )
        .map_err(|e| ApiError::internal_error(format!("Failed to create invoice: {}", e)))?;

    Ok(HttpResponse::Ok().json(response))
}

/// Get a list of Lightning Network nodes
///
/// Returns information about nodes in the Lightning Network.
#[derive(Debug, Deserialize, IntoParams)]
struct GetNetworkNodesParams {
    /// Optional limit for the number of nodes to return (default: 100)
    #[param(default = "100")]
    limit: Option<u32>,
}

#[utoipa::path(
    get,
    path = "/api/v1/lightning/nodes",
    params(
        GetNetworkNodesParams
    ),
    responses(
        (status = 200, description = "Lightning Network nodes retrieved successfully", body = Vec<NodeInfo>),
        (status = 400, description = "Invalid request parameters", body = ApiError),
        (status = 500, description = "Internal server error", body = ApiError)
    )
)]
pub async fn get_network_nodes(
    params: web::Query<GetNetworkNodesParams>,
    node: web::Data<Arc<Node>>,
) -> ApiResult<HttpResponse> {
    // Check if Lightning Network is enabled
    let lightning_manager = node
        .lightning()
        .ok_or_else(|| ApiError::service_unavailable("Lightning Network is not enabled"))?;

    // Get network nodes
    let manager = lightning_manager
        .read()
        .map_err(|e| ApiError::internal_error(format!("Lightning manager lock poisoned: {}", e)))?;
    let nodes = manager
        .get_network_nodes(params.limit.unwrap_or(100))
        .map_err(|e| ApiError::internal_error(format!("Failed to list network nodes: {}", e)))?;

    Ok(HttpResponse::Ok().json(nodes))
}

/// Get information about a specific Lightning Network node
///
/// Returns detailed information about a specific node in the Lightning Network.
#[utoipa::path(
    get,
    path = "/api/v1/lightning/node/{node_id}",
    params(
        ("node_id" = String, Path, description = "Node ID (public key)")
    ),
    responses(
        (status = 200, description = "Node information retrieved successfully", body = NodeInfo),
        (status = 404, description = "Node not found", body = ApiError),
        (status = 500, description = "Internal server error", body = ApiError)
    )
)]
pub async fn get_node_info(
    path: web::Path<String>,
    node: web::Data<Arc<Node>>,
) -> ApiResult<HttpResponse> {
    let node_id = path.into_inner();

    // Check if Lightning Network is enabled
    let lightning_manager = node
        .lightning()
        .ok_or_else(|| ApiError::service_unavailable("Lightning Network is not enabled"))?;

    // Get node info
    let manager = lightning_manager
        .read()
        .map_err(|e| ApiError::internal_error(format!("Lightning manager lock poisoned: {}", e)))?;
    let node_info = manager
        .get_node_info(&node_id)
        .map_err(|e| ApiError::internal_error(format!("Failed to get node info: {}", e)))?
        .ok_or_else(|| ApiError::not_found(format!("Node {} not found", node_id)))?;

    Ok(HttpResponse::Ok().json(node_info))
}

/// Find a route through the Lightning Network
///
/// Finds a route to send a payment to a destination node.
#[derive(Debug, Deserialize, IntoParams)]
struct FindRouteParams {
    /// Destination node ID (public key)
    pub_key: String,

    /// Amount to send in millisatoshis
    amt_msat: u64,

    /// Maximum fee in millisatoshis (default: 10000)
    #[param(default = "10000")]
    fee_limit_msat: Option<u64>,
}

#[utoipa::path(
    get,
    path = "/api/v1/lightning/routes",
    params(
        FindRouteParams
    ),
    responses(
        (status = 200, description = "Route found successfully", body = Route),
        (status = 400, description = "Invalid request parameters", body = ApiError),
        (status = 404, description = "No route found", body = ApiError),
        (status = 500, description = "Internal server error", body = ApiError)
    )
)]
pub async fn find_route(
    params: web::Query<FindRouteParams>,
    node: web::Data<Arc<Node>>,
) -> ApiResult<HttpResponse> {
    // Check if Lightning Network is enabled
    let lightning_manager = node
        .lightning()
        .ok_or_else(|| ApiError::service_unavailable("Lightning Network is not enabled"))?;

    // Find route
    let manager = lightning_manager
        .read()
        .map_err(|e| ApiError::internal_error(format!("Lightning manager lock poisoned: {}", e)))?;
    let route = manager
        .find_route(
            &params.pub_key,
            params.amt_msat,
            params.fee_limit_msat.unwrap_or(10000),
        )
        .await
        .map_err(|e| ApiError::internal_error(format!("Failed to find route: {}", e)))?
        .ok_or_else(|| ApiError::not_found("No route found to destination"))?;

    Ok(HttpResponse::Ok().json(route))
}
