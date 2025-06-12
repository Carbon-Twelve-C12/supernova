use crate::api::error::{ApiError, ApiResult};
use crate::api::types::{
    MempoolInfo, MempoolStatistics, MempoolTransaction, MempoolTransactionSubmissionResponse, 
    TransactionFees, TransactionValidationResult,
};
use crate::node::Node;
use crate::mempool::TransactionPool;
use actix_web::{web, HttpResponse};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use std::sync::Arc;
use hex::FromHex;
use bincode;

/// Configure mempool API routes
pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/mempool")
            .route("/info", web::get().to(get_mempool_info))
            .route("/transactions", web::get().to(get_mempool_transactions))
            .route("/transaction/{txid}", web::get().to(get_mempool_transaction))
            .route("/submit", web::post().to(submit_transaction))
            .route("/validate", web::post().to(validate_transaction))
            .route("/fees", web::get().to(get_fee_estimates)),
    );
}

/// Request for submitting a transaction
#[derive(Debug, Deserialize, ToSchema)]
pub struct SubmitTxRequest {
    /// Raw transaction data in hex format
    pub raw_tx: String,
}

/// Alias for OpenAPI compatibility
pub type SubmitTransactionRequest = SubmitTxRequest;

/// Request for validating a transaction
#[derive(Debug, Deserialize, ToSchema)]
pub struct ValidateTransactionRequest {
    /// Raw transaction data in hex format
    pub raw_tx: String,
}

/// Get mempool information
///
/// Returns general information about the mempool.
#[utoipa::path(
    get,
    path = "/api/v1/mempool/info",
    responses(
        (status = 200, description = "Mempool information retrieved successfully", body = MempoolInfo),
        (status = 500, description = "Internal server error", body = ApiError)
    )
)]
pub async fn get_mempool_info(
    node: web::Data<Arc<Node>>,
) -> Result<HttpResponse, actix_web::Error> {
    let info = node.mempool().get_info();
    Ok(HttpResponse::Ok().json(info))
}

/// Get mempool transactions
///
/// Returns a list of transactions currently in the mempool.
#[derive(Debug, Deserialize, IntoParams)]
struct GetMempoolTransactionsParams {
    /// Maximum number of transactions to retrieve (default: 100)
    #[param(default = "100")]
    limit: Option<u32>,
    
    /// Offset for pagination (default: 0)
    #[param(default = "0")]
    offset: Option<u32>,
    
    /// Whether to include verbose transaction details (default: false)
    #[param(default = "false")]
    verbose: Option<bool>,
}

#[utoipa::path(
    get,
    path = "/api/v1/mempool/transactions",
    params(
        GetMempoolTransactionsParams
    ),
    responses(
        (status = 200, description = "Mempool transactions retrieved successfully", body = Vec<MempoolTransaction>),
        (status = 400, description = "Invalid request parameters", body = ApiError),
        (status = 500, description = "Internal server error", body = ApiError)
    )
)]
pub async fn get_mempool_transactions(
    params: web::Query<GetMempoolTransactionsParams>,
    node: web::Data<Arc<Node>>,
) -> Result<HttpResponse, actix_web::Error> {
    let limit = params.limit.unwrap_or(100) as usize;
    let offset = params.offset.unwrap_or(0) as usize;
    let _verbose = params.verbose.unwrap_or(false);
    
    match node.mempool().get_transactions(limit, offset, "fee_desc") {
        Ok(transactions) => Ok(HttpResponse::Ok().json(transactions)),
        Err(e) => Ok(HttpResponse::InternalServerError().json(
            ApiError::internal_error(format!("Failed to get mempool transactions: {}", e))
        )),
    }
}

/// Get a specific transaction from the mempool
///
/// Returns detailed information about a specific transaction in the mempool.
#[utoipa::path(
    get,
    path = "/api/v1/mempool/transaction/{txid}",
    params(
        ("txid" = String, Path, description = "Transaction ID")
    ),
    responses(
        (status = 200, description = "Transaction retrieved successfully", body = MempoolTransaction),
        (status = 404, description = "Transaction not found", body = ApiError),
        (status = 500, description = "Internal server error", body = ApiError)
    )
)]
pub async fn get_mempool_transaction(
    path: web::Path<String>,
    node: web::Data<Arc<Node>>,
) -> Result<HttpResponse, actix_web::Error> {
    let txid = path.into_inner();
    
    match node.mempool().get_transaction_by_id(&txid) {
        Ok(Some(tx)) => Ok(HttpResponse::Ok().json(tx)),
        Ok(None) => Ok(HttpResponse::NotFound().json(
            ApiError::not_found("Transaction not found in mempool")
        )),
        Err(e) => Ok(HttpResponse::InternalServerError().json(
            ApiError::internal_error(format!("Failed to get transaction: {}", e))
        )),
    }
}

/// Submit a transaction to the mempool
///
/// Submits a new transaction to the mempool for validation and broadcasting.
#[utoipa::path(
    post,
    path = "/api/v1/mempool/submit",
    request_body = SubmitTxRequest,
    responses(
        (status = 200, description = "Transaction submitted successfully", body = MempoolTransactionSubmissionResponse),
        (status = 400, description = "Invalid transaction", body = ApiError),
        (status = 500, description = "Internal server error", body = ApiError)
    )
)]
pub async fn submit_transaction(
    request: web::Json<SubmitTxRequest>,
    node: web::Data<Arc<Node>>,
) -> Result<HttpResponse, actix_web::Error> {
    // Parse the raw transaction
    let tx_data = match hex::decode(&request.raw_tx) {
        Ok(data) => data,
        Err(_) => return Ok(HttpResponse::BadRequest().json(
            ApiError::bad_request("Invalid transaction format")
        )),
    };
    
    // Deserialize the transaction
    let tx = match bincode::deserialize::<btclib::types::transaction::Transaction>(&tx_data) {
        Ok(tx) => tx,
        Err(_) => return Ok(HttpResponse::BadRequest().json(
            ApiError::bad_request("Invalid transaction format")
        )),
    };
    
    let txid = hex::encode(tx.hash());
    
    // Add to mempool with default fee rate
    match node.mempool().add_transaction(tx.clone(), 1000) {
        Ok(()) => {
            // Broadcast to network
            node.broadcast_transaction(&tx);
            
            Ok(HttpResponse::Ok().json(MempoolTransactionSubmissionResponse {
                txid,
                accepted: true,
            }))
        },
        Err(e) => {
            match e {
                crate::mempool::MempoolError::TransactionExists(_) => {
                    Ok(HttpResponse::BadRequest().json(
                        ApiError::bad_request("Transaction already exists in mempool")
                    ))
                },
                crate::mempool::MempoolError::InvalidTransaction(msg) => {
                    Ok(HttpResponse::BadRequest().json(
                        ApiError::bad_request(format!("Invalid transaction: {}", msg))
                    ))
                },
                crate::mempool::MempoolError::FeeTooLow { .. } => {
                    Ok(HttpResponse::BadRequest().json(
                        ApiError::bad_request("Insufficient transaction fee")
                    ))
                },
                _ => {
                    Ok(HttpResponse::InternalServerError().json(
                        ApiError::internal_error(format!("Failed to add transaction to mempool: {}", e))
                    ))
                }
            }
        }
    }
}

/// Validate a transaction
///
/// Validates a transaction without adding it to the mempool.
#[utoipa::path(
    post,
    path = "/api/v1/mempool/validate",
    request_body = SubmitTxRequest,
    responses(
        (status = 200, description = "Transaction validated successfully", body = TransactionValidationResult),
        (status = 400, description = "Invalid transaction", body = ApiError),
        (status = 500, description = "Internal server error", body = ApiError)
    )
)]
pub async fn validate_transaction(
    request: web::Json<SubmitTxRequest>,
    node: web::Data<Arc<Node>>,
) -> Result<HttpResponse, actix_web::Error> {
    // Parse the raw transaction
    let tx_data = match hex::decode(&request.raw_tx) {
        Ok(data) => data,
        Err(_) => return Ok(HttpResponse::BadRequest().json(
            ApiError::bad_request("Invalid transaction format")
        )),
    };
    
    match node.mempool().validate_transaction(&tx_data) {
        Ok(result) => Ok(HttpResponse::Ok().json(result)),
        Err(e) => Ok(HttpResponse::InternalServerError().json(
            ApiError::internal_error(format!("Failed to validate transaction: {}", e))
        )),
    }
}

/// Estimate transaction fee based on current mempool state
///
/// Estimates the fee required for a transaction to be confirmed within a certain number of blocks.
#[derive(Debug, Deserialize, IntoParams)]
struct EstimateFeeParams {
    /// Target confirmation in number of blocks (default: 6)
    #[param(default = "6")]
    target_conf: Option<u32>,
}

/// Get transaction fee estimates
///
/// Returns current fee estimates for different confirmation targets.
#[derive(Debug, Deserialize, IntoParams)]
struct GetFeeEstimatesParams {
    /// Target number of blocks for confirmation (default: 6)
    #[param(default = "6")]
    target_blocks: Option<u32>,
}

#[utoipa::path(
    get,
    path = "/api/v1/mempool/fees",
    params(
        GetFeeEstimatesParams
    ),
    responses(
        (status = 200, description = "Fee estimates retrieved successfully", body = TransactionFees),
        (status = 400, description = "Invalid request parameters", body = ApiError),
        (status = 500, description = "Internal server error", body = ApiError)
    )
)]
pub async fn get_fee_estimates(
    params: web::Query<GetFeeEstimatesParams>,
    node: web::Data<Arc<Node>>,
) -> Result<HttpResponse, actix_web::Error> {
    let target_blocks = params.target_blocks.unwrap_or(6);
    
    match node.mempool().estimate_fee(target_blocks) {
        Ok(fees) => Ok(HttpResponse::Ok().json(fees)),
        Err(e) => Ok(HttpResponse::InternalServerError().json(
            ApiError::internal_error(format!("Failed to get fee estimates: {}", e))
        )),
    }
} 