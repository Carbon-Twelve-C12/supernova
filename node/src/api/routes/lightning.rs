use crate::api::error::{ApiError, ApiResult};
use crate::api::types::{
    LightningInfo, LightningChannel, LightningPayment, LightningInvoice, 
    OpenChannelRequest, OpenChannelResponse, CloseChannelRequest, 
    PaymentRequest, PaymentResponse, InvoiceRequest, InvoiceResponse,
    NodeInfo, Route,
};
use btclib::lightning::manager::LightningManager;
use actix_web::{web, HttpResponse};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use std::sync::Arc;
use hex::FromHex;

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
async fn get_lightning_info(
    lightning: web::Data<Arc<LightningManager>>,
) -> ApiResult<LightningInfo> {
    match lightning.get_info() {
        Ok(info) => Ok(HttpResponse::Ok().json(info)),
        Err(e) => Err(ApiError::internal_error(format!("Failed to get Lightning info: {}", e))),
    }
}

/// Get a list of Lightning Network channels
///
/// Returns information about the node's active Lightning Network channels.
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
#[derive(Debug, Deserialize, IntoParams)]
struct GetChannelsParams {
    /// Whether to include inactive channels (default: false)
    #[param(default = "false")]
    include_inactive: Option<bool>,
    
    /// Whether to include pending channels (default: true)
    #[param(default = "true")]
    include_pending: Option<bool>,
}

async fn get_channels(
    params: web::Query<GetChannelsParams>,
    lightning: web::Data<Arc<LightningManager>>,
) -> ApiResult<Vec<LightningChannel>> {
    let include_inactive = params.include_inactive.unwrap_or(false);
    let include_pending = params.include_pending.unwrap_or(true);
    
    match lightning.get_channels(include_inactive, include_pending) {
        Ok(channels) => Ok(HttpResponse::Ok().json(channels)),
        Err(e) => Err(ApiError::internal_error(format!("Failed to get channels: {}", e))),
    }
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
async fn get_channel(
    path: web::Path<String>,
    lightning: web::Data<Arc<LightningManager>>,
) -> ApiResult<LightningChannel> {
    let channel_id = path.into_inner();
    
    match lightning.get_channel(&channel_id) {
        Ok(Some(channel)) => Ok(HttpResponse::Ok().json(channel)),
        Ok(None) => Err(ApiError::not_found("Channel not found")),
        Err(e) => Err(ApiError::internal_error(format!("Failed to get channel: {}", e))),
    }
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
async fn open_channel(
    request: web::Json<OpenChannelRequest>,
    lightning: web::Data<Arc<LightningManager>>,
) -> ApiResult<OpenChannelResponse> {
    match lightning.open_channel(
        &request.node_id,
        request.local_funding_amount,
        request.push_amount_msat,
        request.private.unwrap_or(false),
        request.min_htlc_msat,
    ).await {
        Ok(response) => Ok(HttpResponse::Ok().json(response)),
        Err(e) => Err(ApiError::internal_error(format!("Failed to open channel: {}", e))),
    }
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
async fn close_channel(
    request: web::Json<CloseChannelRequest>,
    lightning: web::Data<Arc<LightningManager>>,
) -> ApiResult<HttpResponse> {
    let force = request.force.unwrap_or(false);
    
    match lightning.close_channel(&request.channel_id, force).await {
        Ok(true) => Ok(HttpResponse::Ok().finish()),
        Ok(false) => Err(ApiError::not_found("Channel not found")),
        Err(e) => Err(ApiError::internal_error(format!("Failed to close channel: {}", e))),
    }
}

/// Get a list of Lightning Network payments
///
/// Returns information about the node's Lightning Network payments.
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

async fn get_payments(
    params: web::Query<GetPaymentsParams>,
    lightning: web::Data<Arc<LightningManager>>,
) -> ApiResult<Vec<LightningPayment>> {
    let index_offset = params.index_offset.unwrap_or(0);
    let max_payments = params.max_payments.unwrap_or(100);
    let include_pending = params.include_pending.unwrap_or(true);
    
    match lightning.get_payments(index_offset, max_payments, include_pending) {
        Ok(payments) => Ok(HttpResponse::Ok().json(payments)),
        Err(e) => Err(ApiError::internal_error(format!("Failed to get payments: {}", e))),
    }
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
async fn send_payment(
    request: web::Json<PaymentRequest>,
    lightning: web::Data<Arc<LightningManager>>,
) -> ApiResult<PaymentResponse> {
    match lightning.send_payment(
        &request.payment_request,
        request.amount_msat,
        request.timeout_seconds.unwrap_or(60),
        request.fee_limit_msat,
    ).await {
        Ok(response) => Ok(HttpResponse::Ok().json(response)),
        Err(e) => Err(ApiError::internal_error(format!("Failed to send payment: {}", e))),
    }
}

/// Get a list of Lightning Network invoices
///
/// Returns information about the node's Lightning Network invoices.
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

async fn get_invoices(
    params: web::Query<GetInvoicesParams>,
    lightning: web::Data<Arc<LightningManager>>,
) -> ApiResult<Vec<LightningInvoice>> {
    let pending_only = params.pending_only.unwrap_or(true);
    let index_offset = params.index_offset.unwrap_or(0);
    let num_max_invoices = params.num_max_invoices.unwrap_or(100);
    
    match lightning.get_invoices(pending_only, index_offset, num_max_invoices) {
        Ok(invoices) => Ok(HttpResponse::Ok().json(invoices)),
        Err(e) => Err(ApiError::internal_error(format!("Failed to get invoices: {}", e))),
    }
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
async fn create_invoice(
    request: web::Json<InvoiceRequest>,
    lightning: web::Data<Arc<LightningManager>>,
) -> ApiResult<InvoiceResponse> {
    match lightning.create_invoice(
        request.value_msat,
        &request.memo.clone().unwrap_or_default(),
        request.expiry.unwrap_or(3600),
        request.private.unwrap_or(false),
    ) {
        Ok(response) => Ok(HttpResponse::Ok().json(response)),
        Err(e) => Err(ApiError::internal_error(format!("Failed to create invoice: {}", e))),
    }
}

/// Get a list of Lightning Network nodes
///
/// Returns information about nodes in the Lightning Network.
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
#[derive(Debug, Deserialize, IntoParams)]
struct GetNetworkNodesParams {
    /// Optional limit for the number of nodes to return (default: 100)
    #[param(default = "100")]
    limit: Option<u32>,
}

async fn get_network_nodes(
    params: web::Query<GetNetworkNodesParams>,
    lightning: web::Data<Arc<LightningManager>>,
) -> ApiResult<Vec<NodeInfo>> {
    let limit = params.limit.unwrap_or(100);
    
    match lightning.get_network_nodes(limit) {
        Ok(nodes) => Ok(HttpResponse::Ok().json(nodes)),
        Err(e) => Err(ApiError::internal_error(format!("Failed to get network nodes: {}", e))),
    }
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
async fn get_node_info(
    path: web::Path<String>,
    lightning: web::Data<Arc<LightningManager>>,
) -> ApiResult<NodeInfo> {
    let node_id = path.into_inner();
    
    match lightning.get_node_info(&node_id) {
        Ok(Some(node)) => Ok(HttpResponse::Ok().json(node)),
        Ok(None) => Err(ApiError::not_found("Node not found")),
        Err(e) => Err(ApiError::internal_error(format!("Failed to get node info: {}", e))),
    }
}

/// Find a route through the Lightning Network
///
/// Finds a route to send a payment to a destination node.
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

async fn find_route(
    params: web::Query<FindRouteParams>,
    lightning: web::Data<Arc<LightningManager>>,
) -> ApiResult<Route> {
    let pub_key = params.pub_key.clone();
    let amt_msat = params.amt_msat;
    let fee_limit_msat = params.fee_limit_msat.unwrap_or(10000);
    
    match lightning.find_route(&pub_key, amt_msat, fee_limit_msat).await {
        Ok(Some(route)) => Ok(HttpResponse::Ok().json(route)),
        Ok(None) => Err(ApiError::not_found("No route found")),
        Err(e) => Err(ApiError::internal_error(format!("Failed to find route: {}", e))),
    }
} 