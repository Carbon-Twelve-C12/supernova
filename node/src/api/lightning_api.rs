// Lightning Network API endpoints

// Lightning API temporarily disabled due to thread safety issues with libp2p Swarm
// TODO: Refactor to use proper async/await patterns and thread-safe types

use warp::Filter;

/*
use crate::node::Node;
use std::sync::{Arc, Mutex};
use warp::{Filter, Reply};
use serde::{Deserialize, Serialize};

/// Request to open a new payment channel
#[derive(Debug, Deserialize)]
pub struct OpenChannelRequest {
    /// Remote peer ID
    pub peer_id: String,

    /// Channel capacity in millanova
    pub capacity: u64,

    /// Amount to push to the remote side (in millanova)
    pub push_amount: u64,
}

/// Request to close a payment channel
#[derive(Debug, Deserialize)]
pub struct CloseChannelRequest {
    /// Channel ID
    pub channel_id: String,

    /// Whether to force close the channel
    pub force_close: bool,
}

/// Request to create an invoice
#[derive(Debug, Deserialize)]
pub struct CreateInvoiceRequest {
    /// Amount in millisatoshis
    pub amount_msat: u64,

    /// Description
    pub description: String,

    /// Expiry time in seconds
    pub expiry_seconds: u32,
}

/// Request to pay an invoice
#[derive(Debug, Deserialize)]
pub struct PayInvoiceRequest {
    /// BOLT11 invoice string
    pub invoice: String,
}

/// Response for a successful operation
#[derive(Debug, Serialize)]
pub struct LightningResponse<T> {
    /// Success status
    pub success: bool,

    /// Result of the operation
    pub data: Option<T>,

    /// Error message
    pub error: Option<String>,
}

impl<T> LightningResponse<T> {
    pub fn success(data: T) -> Self {
        Self {
            success: true,
            data: Some(data),
            error: None,
        }
    }

    pub fn error(message: String) -> Self {
        Self {
            success: false,
            data: None,
            error: Some(message),
        }
    }
}

/// Create a router for Lightning Network API endpoints
pub fn lightning_routes(
    node: Arc<Mutex<Node>>,
) -> impl Filter<Extract = impl Reply, Error = warp::Rejection> + Clone {
    let node_clone = node.clone();
    let open_channel = warp::path!("lightning" / "channel" / "open")
        .and(warp::post())
        .and(warp::body::json())
        .and(with_node(node_clone))
        .and_then(handle_open_channel);

    let node_clone = node.clone();
    let close_channel = warp::path!("lightning" / "channel" / "close")
        .and(warp::post())
        .and(warp::body::json())
        .and(with_node(node_clone))
        .and_then(handle_close_channel);

    let node_clone = node.clone();
    let create_invoice = warp::path!("lightning" / "invoice" / "create")
        .and(warp::post())
        .and(warp::body::json())
        .and(with_node(node_clone))
        .and_then(handle_create_invoice);

    let node_clone = node.clone();
    let pay_invoice = warp::path!("lightning" / "invoice" / "pay")
        .and(warp::post())
        .and(warp::body::json())
        .and(with_node(node_clone))
        .and_then(handle_pay_invoice);

    let node_clone = node.clone();
    let list_channels = warp::path!("lightning" / "channels")
        .and(warp::get())
        .and(with_node(node_clone))
        .and_then(handle_list_channels);

    let node_clone = node.clone();
    let get_channel_info = warp::path!("lightning" / "channel" / String)
        .and(warp::get())
        .and(with_node(node_clone))
        .and_then(handle_get_channel_info);

    open_channel
        .or(close_channel)
        .or(create_invoice)
        .or(pay_invoice)
        .or(list_channels)
        .or(get_channel_info)
}

/// Inject the node into the route handlers
fn with_node(
    node: Arc<Mutex<Node>>,
) -> impl Filter<Extract = (Arc<Mutex<Node>>,), Error = std::convert::Infallible> + Clone {
    warp::any().map(move || node.clone())
}

/// Handle opening a new payment channel
async fn handle_open_channel(
    request: OpenChannelRequest,
    node: Arc<Mutex<Node>>,
) -> Result<impl Reply, warp::Rejection> {
    let node = match node.lock() {
        Ok(n) => n,
        Err(e) => {
            let response: LightningResponse<String> = LightningResponse::error(format!("Internal error: {}", e));
            return Ok(warp::reply::json(&response));
        }
    };

    match node.open_payment_channel(&request.peer_id, request.capacity, request.push_amount).await {
        Ok(channel_id) => {
            let response = LightningResponse::success(channel_id);
            Ok(warp::reply::json(&response))
        }
        Err(e) => {
            let response: LightningResponse<String> = LightningResponse::error(e);
            Ok(warp::reply::json(&response))
        }
    }
}

/// Handle closing a payment channel
async fn handle_close_channel(
    request: CloseChannelRequest,
    node: Arc<Mutex<Node>>,
) -> Result<impl Reply, warp::Rejection> {
    let node = match node.lock() {
        Ok(n) => n,
        Err(e) => {
            let response: LightningResponse<String> = LightningResponse::error(format!("Internal error: {}", e));
            return Ok(warp::reply::json(&response));
        }
    };

    match node.close_payment_channel(&request.channel_id, request.force_close).await {
        Ok(tx_id) => {
            let response = LightningResponse::success(tx_id);
            Ok(warp::reply::json(&response))
        }
        Err(e) => {
            let response: LightningResponse<String> = LightningResponse::error(e);
            Ok(warp::reply::json(&response))
        }
    }
}

/// Handle creating a new invoice
async fn handle_create_invoice(
    request: CreateInvoiceRequest,
    node: Arc<Mutex<Node>>,
) -> Result<impl Reply, warp::Rejection> {
    let node = match node.lock() {
        Ok(n) => n,
        Err(e) => {
            let response: LightningResponse<String> = LightningResponse::error(format!("Internal error: {}", e));
            return Ok(warp::reply::json(&response));
        }
    };

    match node.create_invoice(request.amount_msat, &request.description, request.expiry_seconds) {
        Ok(invoice) => {
            let response = LightningResponse::success(invoice);
            Ok(warp::reply::json(&response))
        }
        Err(e) => {
            let response: LightningResponse<String> = LightningResponse::error(e);
            Ok(warp::reply::json(&response))
        }
    }
}

/// Handle paying an invoice
async fn handle_pay_invoice(
    request: PayInvoiceRequest,
    node: Arc<Mutex<Node>>,
) -> Result<impl Reply, warp::Rejection> {
    let node = match node.lock() {
        Ok(n) => n,
        Err(e) => {
            let response: LightningResponse<String> = LightningResponse::error(format!("Internal error: {}", e));
            return Ok(warp::reply::json(&response));
        }
    };

    match node.pay_invoice(&request.invoice).await {
        Ok(preimage) => {
            let response = LightningResponse::success(preimage);
            Ok(warp::reply::json(&response))
        }
        Err(e) => {
            let response: LightningResponse<String> = LightningResponse::error(e);
            Ok(warp::reply::json(&response))
        }
    }
}

/// Handle listing all channels
async fn handle_list_channels(
    node: Arc<Mutex<Node>>,
) -> Result<impl Reply, warp::Rejection> {
    let node = match node.lock() {
        Ok(n) => n,
        Err(e) => {
            let response: LightningResponse<String> = LightningResponse::error(format!("Internal error: {}", e));
            return Ok(warp::reply::json(&response));
        }
    };

    match node.list_channels() {
        Ok(channels) => {
            let response = LightningResponse::success(channels);
            Ok(warp::reply::json(&response))
        }
        Err(e) => {
            let response: LightningResponse<Vec<String>> = LightningResponse::error(e);
            Ok(warp::reply::json(&response))
        }
    }
}

/// Handle getting information about a specific channel
async fn handle_get_channel_info(
    channel_id: String,
    node: Arc<Mutex<Node>>,
) -> Result<impl Reply, warp::Rejection> {
    let node = match node.lock() {
        Ok(n) => n,
        Err(e) => {
            let response: LightningResponse<String> = LightningResponse::error(format!("Internal error: {}", e));
            return Ok(warp::reply::json(&response));
        }
    };

    match node.get_channel_info(&channel_id) {
        Ok(info) => {
            let response = LightningResponse::success(info);
            Ok(warp::reply::json(&response))
        }
        Err(e) => {
            let response: LightningResponse<serde_json::Value> = LightningResponse::error(e);
            Ok(warp::reply::json(&response))
        }
    }
}
*/

// Placeholder for Lightning API - will be re-enabled after thread safety fixes
pub fn lightning_routes(
) -> impl warp::Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::path("lightning").and(warp::any()).map(|| {
        warp::reply::with_status(
            "Lightning API temporarily disabled",
            warp::http::StatusCode::SERVICE_UNAVAILABLE,
        )
    })
}
