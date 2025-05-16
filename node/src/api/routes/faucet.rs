use std::sync::Arc;
use axum::{
    extract::{Extension, Path, Query, State},
    http::StatusCode,
    Json,
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use tracing::{debug, error, info, warn};
use actix_web::web;

use crate::node::Node;
use crate::api::error::ApiError;
use crate::testnet::faucet::FaucetError;

/// Faucet status response
#[derive(Debug, Serialize)]
pub struct FaucetStatusResponse {
    /// Whether the faucet is online
    pub is_online: bool,
    /// Current faucet balance
    pub balance: u64,
    /// Number of transactions today
    pub transactions_today: u32,
    /// Timestamp of last distribution
    pub last_distribution: Option<DateTime<Utc>>,
    /// Cooldown period in seconds
    pub cooldown_secs: u64,
    /// Maximum distribution amount
    pub distribution_amount: u64,
}

/// Request structure for requesting test tokens
#[derive(Debug, Deserialize)]
pub struct FaucetRequest {
    /// Recipient address
    pub address: String,
}

/// Response structure for successful faucet requests
#[derive(Debug, Serialize)]
pub struct FaucetResponse {
    /// Transaction ID of the distribution
    pub txid: String,
    /// Amount sent
    pub amount: u64,
    /// Recipient address
    pub recipient: String,
    /// Timestamp of the distribution
    pub timestamp: DateTime<Utc>,
}

/// Structure for a recent transaction
#[derive(Debug, Serialize)]
pub struct FaucetTransaction {
    /// Transaction ID
    pub txid: String,
    /// Recipient address
    pub recipient: String,
    /// Amount sent
    pub amount: u64,
    /// Timestamp of the distribution
    pub timestamp: DateTime<Utc>,
}

/// Response structure for recent transactions
#[derive(Debug, Serialize)]
pub struct RecentTransactionsResponse {
    /// List of recent transactions
    pub transactions: Vec<FaucetTransaction>,
}

/// Configure faucet API routes
pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/faucet")
            .route("/status", web::get().to(get_faucet_status))
            .route("/send", web::post().to(request_tokens))
            .route("/transactions", web::get().to(get_recent_transactions))
    );
}

/// Create router for faucet API endpoints (for axum-based setup)
pub fn create_faucet_router() -> Router {
    Router::new()
        .route("/status", get(get_faucet_status_axum))
        .route("/send", post(request_tokens_axum))
        .route("/transactions", get(get_recent_transactions_axum))
}

/// Get faucet status (actix-web handler)
async fn get_faucet_status(
    node: web::Data<Arc<Node>>,
) -> Result<web::Json<FaucetStatusResponse>, actix_web::Error> {
    // Get faucet from node
    let faucet = node.get_faucet()
        .ok_or_else(|| actix_web::error::ErrorServiceUnavailable("Faucet is not enabled on this node"))?;
    
    // Get faucet status
    let status = faucet.status().await
        .map_err(|e| actix_web::error::ErrorInternalServerError(format!("Failed to get faucet status: {}", e)))?;
    
    // Return response
    Ok(web::Json(FaucetStatusResponse {
        is_online: status.is_active,
        balance: status.balance,
        transactions_today: status.transactions_today,
        last_distribution: status.last_distribution,
        cooldown_secs: status.cooldown_secs,
        distribution_amount: status.distribution_amount,
    }))
}

/// Request tokens from the faucet (actix-web handler)
async fn request_tokens(
    node: web::Data<Arc<Node>>,
    request: web::Json<FaucetRequest>,
) -> Result<web::Json<FaucetResponse>, actix_web::Error> {
    // Validate request
    if request.address.is_empty() {
        return Err(actix_web::error::ErrorBadRequest("Recipient address cannot be empty"));
    }
    
    // Get faucet from node
    let faucet = node.get_faucet()
        .ok_or_else(|| actix_web::error::ErrorServiceUnavailable("Faucet is not enabled on this node"))?;
    
    // Request tokens
    let result = faucet.distribute_coins(&request.address).await
        .map_err(|e| match e {
            FaucetError::CooldownPeriod { remaining_time } => {
                actix_web::error::ErrorTooManyRequests(
                    format!("Please wait {} seconds before requesting again", remaining_time)
                )
            },
            FaucetError::DailyLimitExceeded => {
                actix_web::error::ErrorTooManyRequests(
                    "Daily distribution limit reached for this address/IP"
                )
            },
            FaucetError::InsufficientFunds => {
                actix_web::error::ErrorServiceUnavailable(
                    "Faucet has insufficient funds"
                )
            },
            FaucetError::InvalidAddress => {
                actix_web::error::ErrorBadRequest(
                    "Invalid recipient address"
                )
            },
            _ => actix_web::error::ErrorInternalServerError(
                format!("Failed to distribute coins: {}", e)
            ),
        })?;
    
    // Return response
    Ok(web::Json(FaucetResponse {
        txid: result.txid,
        amount: result.amount,
        recipient: request.address.clone(),
        timestamp: result.timestamp,
    }))
}

/// Get recent transactions (actix-web handler)
async fn get_recent_transactions(
    node: web::Data<Arc<Node>>,
) -> Result<web::Json<RecentTransactionsResponse>, actix_web::Error> {
    // Get faucet from node
    let faucet = node.get_faucet()
        .ok_or_else(|| actix_web::error::ErrorServiceUnavailable("Faucet is not enabled on this node"))?;
    
    // Get recent transactions
    let transactions = faucet.get_recent_transactions().await
        .map_err(|e| actix_web::error::ErrorInternalServerError(format!("Failed to get recent transactions: {}", e)))?;
    
    // Convert to response format
    let tx_responses = transactions.into_iter()
        .map(|tx| FaucetTransaction {
            txid: tx.txid,
            recipient: tx.recipient,
            amount: tx.amount,
            timestamp: tx.timestamp,
        })
        .collect();
    
    // Return response
    Ok(web::Json(RecentTransactionsResponse {
        transactions: tx_responses,
    }))
}

// Axum Handlers (for compatibility with newer API style)

/// Get faucet status (axum handler)
async fn get_faucet_status_axum(
    Extension(node): Extension<Arc<Node>>,
) -> Result<Json<FaucetStatusResponse>, ApiError> {
    // Get faucet from node
    let faucet = node.get_faucet()
        .ok_or_else(|| ApiError::new(StatusCode::SERVICE_UNAVAILABLE, "Faucet is not enabled on this node"))?;
    
    // Get faucet status
    let status = faucet.status().await
        .map_err(|e| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, &format!("Failed to get faucet status: {}", e)))?;
    
    // Return response
    Ok(Json(FaucetStatusResponse {
        is_online: status.is_active,
        balance: status.balance,
        transactions_today: status.transactions_today,
        last_distribution: status.last_distribution,
        cooldown_secs: status.cooldown_secs,
        distribution_amount: status.distribution_amount,
    }))
}

/// Request tokens from the faucet (axum handler)
async fn request_tokens_axum(
    Extension(node): Extension<Arc<Node>>,
    Json(request): Json<FaucetRequest>,
) -> Result<Json<FaucetResponse>, ApiError> {
    // Validate request
    if request.address.is_empty() {
        return Err(ApiError::new(StatusCode::BAD_REQUEST, "Recipient address cannot be empty"));
    }
    
    // Get faucet from node
    let faucet = node.get_faucet()
        .ok_or_else(|| ApiError::new(StatusCode::SERVICE_UNAVAILABLE, "Faucet is not enabled on this node"))?;
    
    // Request tokens
    let result = faucet.distribute_coins(&request.address).await
        .map_err(|e| match e {
            FaucetError::CooldownPeriod { remaining_time } => {
                ApiError::new(
                    StatusCode::TOO_MANY_REQUESTS, 
                    &format!("Please wait {} seconds before requesting again", remaining_time)
                )
            },
            FaucetError::DailyLimitExceeded => {
                ApiError::new(
                    StatusCode::TOO_MANY_REQUESTS, 
                    "Daily distribution limit reached for this address/IP"
                )
            },
            FaucetError::InsufficientFunds => {
                ApiError::new(
                    StatusCode::SERVICE_UNAVAILABLE, 
                    "Faucet has insufficient funds"
                )
            },
            FaucetError::InvalidAddress => {
                ApiError::new(
                    StatusCode::BAD_REQUEST, 
                    "Invalid recipient address"
                )
            },
            _ => ApiError::new(
                StatusCode::INTERNAL_SERVER_ERROR, 
                &format!("Failed to distribute coins: {}", e)
            ),
        })?;
    
    // Return response
    Ok(Json(FaucetResponse {
        txid: result.txid,
        amount: result.amount,
        recipient: request.address,
        timestamp: result.timestamp,
    }))
}

/// Get recent transactions (axum handler)
async fn get_recent_transactions_axum(
    Extension(node): Extension<Arc<Node>>,
) -> Result<Json<RecentTransactionsResponse>, ApiError> {
    // Get faucet from node
    let faucet = node.get_faucet()
        .ok_or_else(|| ApiError::new(StatusCode::SERVICE_UNAVAILABLE, "Faucet is not enabled on this node"))?;
    
    // Get recent transactions
    let transactions = faucet.get_recent_transactions().await
        .map_err(|e| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, &format!("Failed to get recent transactions: {}", e)))?;
    
    // Convert to response format
    let tx_responses = transactions.into_iter()
        .map(|tx| FaucetTransaction {
            txid: tx.txid,
            recipient: tx.recipient,
            amount: tx.amount,
            timestamp: tx.timestamp,
        })
        .collect();
    
    // Return response
    Ok(Json(RecentTransactionsResponse {
        transactions: tx_responses,
    }))
}