// Lightning Network API endpoints

use crate::node::Node;
use std::sync::{Arc, Mutex};
use warp::{Filter, Rejection, Reply};
use serde::{Deserialize, Serialize};
use tracing::{info, error, warn};

/// Request to open a new payment channel
#[derive(Debug, Deserialize)]
pub struct OpenChannelRequest {
    /// Remote peer ID
    pub peer_id: String,
    
    /// Channel capacity in satoshis
    pub capacity: u64,
    
    /// Amount to push to the remote side (in satoshis)
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
pub struct SuccessResponse {
    /// Success status
    pub success: bool,
    
    /// Result of the operation
    pub result: String,
}

/// Response for an error
#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    /// Error message
    pub error: String,
}

/// Create a router for Lightning Network API endpoints
pub fn lightning_routes(
    node: Arc<Mutex<Node>>,
) -> impl Filter<Extract = impl Reply, Error = Rejection> + Clone {
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
) -> Result<impl Reply, Rejection> {
    info!("Received request to open payment channel: {:?}", request);
    
    let node = node.lock().unwrap();
    
    match node.open_payment_channel(&request.peer_id, request.capacity, request.push_amount).await {
        Ok(channel_id) => {
            let response = SuccessResponse {
                success: true,
                result: channel_id,
            };
            
            Ok(warp::reply::json(&response))
        },
        Err(e) => {
            let response = ErrorResponse {
                error: e,
            };
            
            Ok(warp::reply::json(&response))
        }
    }
}

/// Handle closing a payment channel
async fn handle_close_channel(
    request: CloseChannelRequest,
    node: Arc<Mutex<Node>>,
) -> Result<impl Reply, Rejection> {
    info!("Received request to close payment channel: {:?}", request);
    
    let node = node.lock().unwrap();
    
    match node.close_payment_channel(&request.channel_id, request.force_close).await {
        Ok(tx_id) => {
            let response = SuccessResponse {
                success: true,
                result: tx_id,
            };
            
            Ok(warp::reply::json(&response))
        },
        Err(e) => {
            let response = ErrorResponse {
                error: e,
            };
            
            Ok(warp::reply::json(&response))
        }
    }
}

/// Handle creating a new invoice
async fn handle_create_invoice(
    request: CreateInvoiceRequest,
    node: Arc<Mutex<Node>>,
) -> Result<impl Reply, Rejection> {
    info!("Received request to create invoice: {:?}", request);
    
    let node = node.lock().unwrap();
    
    match node.create_invoice(request.amount_msat, &request.description, request.expiry_seconds) {
        Ok(invoice) => {
            let response = SuccessResponse {
                success: true,
                result: invoice,
            };
            
            Ok(warp::reply::json(&response))
        },
        Err(e) => {
            let response = ErrorResponse {
                error: e,
            };
            
            Ok(warp::reply::json(&response))
        }
    }
}

/// Handle paying an invoice
async fn handle_pay_invoice(
    request: PayInvoiceRequest,
    node: Arc<Mutex<Node>>,
) -> Result<impl Reply, Rejection> {
    info!("Received request to pay invoice");
    
    let node = node.lock().unwrap();
    
    match node.pay_invoice(&request.invoice).await {
        Ok(preimage) => {
            let response = SuccessResponse {
                success: true,
                result: preimage,
            };
            
            Ok(warp::reply::json(&response))
        },
        Err(e) => {
            let response = ErrorResponse {
                error: e,
            };
            
            Ok(warp::reply::json(&response))
        }
    }
}

/// Handle listing all channels
async fn handle_list_channels(
    node: Arc<Mutex<Node>>,
) -> Result<impl Reply, Rejection> {
    info!("Received request to list channels");
    
    let node = node.lock().unwrap();
    
    match node.list_channels() {
        Ok(channels) => {
            Ok(warp::reply::json(&channels))
        },
        Err(e) => {
            let response = ErrorResponse {
                error: e,
            };
            
            Ok(warp::reply::json(&response))
        }
    }
}

/// Handle getting information about a specific channel
async fn handle_get_channel_info(
    channel_id: String,
    node: Arc<Mutex<Node>>,
) -> Result<impl Reply, Rejection> {
    info!("Received request to get channel info: {}", channel_id);
    
    let node = node.lock().unwrap();
    
    match node.get_channel_info(&channel_id) {
        Ok(info) => {
            Ok(warp::reply::json(&info))
        },
        Err(e) => {
            let response = ErrorResponse {
                error: e,
            };
            
            Ok(warp::reply::json(&response))
        }
    }
} 