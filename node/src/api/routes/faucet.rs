use actix_web::{web, HttpResponse};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::debug;
use utoipa::ToSchema;

use crate::api::error::ApiError;
use crate::node::Node;
use supernova_core::testnet::faucet::FaucetError;

/// Faucet status response
#[derive(Debug, Serialize, ToSchema)]
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
#[derive(Debug, Deserialize, ToSchema)]
pub struct FaucetRequest {
    /// Recipient address
    pub address: String,
}

/// Response structure for successful faucet requests
#[derive(Debug, Serialize, ToSchema)]
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
#[derive(Debug, Serialize, ToSchema)]
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
#[derive(Debug, Serialize, ToSchema)]
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
            .route("/transactions", web::get().to(get_recent_transactions)),
    );
}

/// Get faucet status
#[utoipa::path(
    get,
    path = "/api/v1/faucet/status",
    responses(
        (status = 200, description = "Faucet status retrieved successfully", body = FaucetStatusResponse),
        (status = 503, description = "Faucet not enabled", body = ApiError)
    ),
    tag = "faucet"
)]
pub async fn get_faucet_status(
    node: web::Data<Arc<Node>>,
) -> Result<HttpResponse, actix_web::Error> {
    debug!("Getting faucet status");

    // Get faucet from node
    let faucet = match node.get_faucet() {
        Ok(Some(f)) => f,
        Ok(None) => {
            return Ok(
                HttpResponse::ServiceUnavailable().json(ApiError::service_unavailable(
                    "Faucet is not enabled on this node",
                )),
            )
        }
        Err(e) => {
            return Ok(
                HttpResponse::InternalServerError().json(ApiError::internal_error(format!(
                    "Failed to get faucet: {}",
                    e
                ))),
            )
        }
    };

    // Get faucet status
    match faucet.get_faucet_status().await {
        Ok(status) => Ok(HttpResponse::Ok().json(FaucetStatusResponse {
            is_online: status.is_active,
            balance: status.balance,
            transactions_today: status.transactions_today,
            last_distribution: status.last_distribution,
            cooldown_secs: status.cooldown_secs,
            distribution_amount: status.distribution_amount,
        })),
        Err(e) => Ok(
            HttpResponse::InternalServerError().json(ApiError::internal_error(format!(
                "Failed to get faucet status: {}",
                e
            ))),
        ),
    }
}

/// Request tokens from the faucet
#[utoipa::path(
    post,
    path = "/api/v1/faucet/send",
    request_body = FaucetRequest,
    responses(
        (status = 200, description = "Tokens sent successfully", body = FaucetResponse),
        (status = 400, description = "Invalid request", body = ApiError),
        (status = 429, description = "Rate limited", body = ApiError),
        (status = 503, description = "Faucet unavailable", body = ApiError)
    ),
    tag = "faucet"
)]
pub async fn request_tokens(
    node: web::Data<Arc<Node>>,
    request: web::Json<FaucetRequest>,
) -> Result<HttpResponse, actix_web::Error> {
    debug!("Processing faucet request for address: {}", request.address);

    // Validate request
    if request.address.is_empty() {
        return Ok(HttpResponse::BadRequest()
            .json(ApiError::bad_request("Recipient address cannot be empty")));
    }

    // Get faucet from node
    let faucet = match node.get_faucet() {
        Ok(Some(f)) => f,
        Ok(None) => {
            return Ok(
                HttpResponse::ServiceUnavailable().json(ApiError::service_unavailable(
                    "Faucet is not enabled on this node",
                )),
            )
        }
        Err(e) => {
            return Ok(
                HttpResponse::InternalServerError().json(ApiError::internal_error(format!(
                    "Failed to get faucet: {}",
                    e
                ))),
            )
        }
    };

    // Request tokens
    match faucet.request_faucet_coins(&request.address).await {
        Ok(result) => Ok(HttpResponse::Ok().json(FaucetResponse {
            txid: result.txid,
            amount: result.amount,
            recipient: request.address.clone(),
            timestamp: result.timestamp,
        })),
        Err(e) => match e {
            FaucetError::CooldownPeriod { remaining_time } => Ok(HttpResponse::TooManyRequests()
                .json(ApiError::rate_limited(format!(
                    "Please wait {} seconds before requesting again",
                    remaining_time
                )))),
            FaucetError::DailyLimitExceeded => Ok(HttpResponse::TooManyRequests().json(
                ApiError::rate_limited("Daily distribution limit reached for this address/IP"),
            )),
            FaucetError::InsufficientFunds => Ok(HttpResponse::ServiceUnavailable().json(
                ApiError::service_unavailable("Faucet has insufficient funds"),
            )),
            FaucetError::InvalidAddress(_) => {
                Ok(HttpResponse::BadRequest()
                    .json(ApiError::bad_request("Invalid recipient address")))
            }
            _ => Ok(
                HttpResponse::InternalServerError().json(ApiError::internal_error(format!(
                    "Failed to distribute coins: {}",
                    e
                ))),
            ),
        },
    }
}

/// Get recent transactions
#[utoipa::path(
    get,
    path = "/api/v1/faucet/transactions",
    responses(
        (status = 200, description = "Recent transactions retrieved successfully", body = RecentTransactionsResponse),
        (status = 503, description = "Faucet not enabled", body = ApiError)
    ),
    tag = "faucet"
)]
pub async fn get_recent_transactions(
    node: web::Data<Arc<Node>>,
) -> Result<HttpResponse, actix_web::Error> {
    debug!("Getting recent faucet transactions");

    // Get faucet from node
    let faucet = match node.get_faucet() {
        Ok(Some(f)) => f,
        Ok(None) => {
            return Ok(
                HttpResponse::ServiceUnavailable().json(ApiError::service_unavailable(
                    "Faucet is not enabled on this node",
                )),
            )
        }
        Err(e) => {
            return Ok(
                HttpResponse::InternalServerError().json(ApiError::internal_error(format!(
                    "Failed to get faucet: {}",
                    e
                ))),
            )
        }
    };

    // Get recent transactions
    match faucet.get_recent_faucet_transactions().await {
        Ok(transactions) => {
            // Convert to response format
            let tx_responses = transactions
                .into_iter()
                .map(|tx| FaucetTransaction {
                    txid: tx.txid,
                    recipient: tx.recipient,
                    amount: tx.amount,
                    timestamp: tx.timestamp,
                })
                .collect();

            Ok(HttpResponse::Ok().json(RecentTransactionsResponse {
                transactions: tx_responses,
            }))
        }
        Err(e) => Ok(
            HttpResponse::InternalServerError().json(ApiError::internal_error(format!(
                "Failed to get recent transactions: {}",
                e
            ))),
        ),
    }
}
