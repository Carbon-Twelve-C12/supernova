use crate::api::error::{ApiError, ApiResult};
use crate::api::types::{
    MempoolInfo, MempoolStatistics, MempoolTransaction, MempoolTransactionSubmissionResponse, 
    TransactionFees, TransactionValidationResult,
};
use crate::mempool::TransactionPool;
use actix_web::{web, HttpResponse};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use std::sync::Arc;
use hex::FromHex;

/// Configure mempool API routes
pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/mempool")
            .route("/info", web::get().to(get_mempool_info))
            .route("/transactions", web::get().to(get_mempool_transactions))
            .route("/transaction/{txid}", web::get().to(get_mempool_transaction))
            .route("/transaction", web::post().to(submit_mempool_transaction))
            .route("/validate_transaction", web::post().to(validate_transaction))
            .route("/estimate_fee", web::get().to(estimate_fee)),
    );
}

/// Get information about the mempool
///
/// Returns statistics about the current state of the mempool.
#[utoipa::path(
    get,
    path = "/api/v1/mempool/info",
    responses(
        (status = 200, description = "Mempool information retrieved successfully", body = MempoolInfo),
        (status = 500, description = "Internal server error", body = ApiError)
    )
)]
pub async fn get_mempool_info(
    mempool: web::Data<Arc<TransactionPool>>,
) -> ApiResult<MempoolInfo> {
    let info = mempool.get_info();
    Ok(HttpResponse::Ok().json(info))
}

/// Get a list of transactions in the mempool
///
/// Returns a paginated list of transaction IDs in the mempool.
#[derive(Debug, Deserialize, IntoParams)]
struct GetMempoolTransactionsParams {
    /// Optional limit for the number of transactions to return (default: 100)
    #[param(default = "100")]
    limit: Option<usize>,
    
    /// Optional offset for pagination (default: 0)
    #[param(default = "0")]
    offset: Option<usize>,
    
    /// Optional sort order (default: "fee_desc")
    #[param(default = "fee_desc")]
    sort: Option<String>,
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
    mempool: web::Data<Arc<TransactionPool>>,
) -> ApiResult<Vec<MempoolTransaction>> {
    let limit = params.limit.unwrap_or(100);
    let offset = params.offset.unwrap_or(0);
    let sort = params.sort.as_deref().unwrap_or("fee_desc");
    
    match mempool.get_transactions(limit, offset, sort) {
        Ok(transactions) => Ok(HttpResponse::Ok().json(transactions)),
        Err(e) => Err(ApiError::internal_error(format!("Failed to get mempool transactions: {}", e))),
    }
}

/// Get a specific transaction from the mempool
///
/// Returns details of a specific transaction in the mempool.
#[utoipa::path(
    get,
    path = "/api/v1/mempool/transaction/{txid}",
    params(
        ("txid" = String, Path, description = "Transaction ID")
    ),
    responses(
        (status = 200, description = "Mempool transaction retrieved successfully", body = MempoolTransaction),
        (status = 400, description = "Invalid transaction ID format", body = ApiError),
        (status = 404, description = "Transaction not found in mempool", body = ApiError),
        (status = 500, description = "Internal server error", body = ApiError)
    )
)]
pub async fn get_mempool_transaction(
    path: web::Path<String>,
    mempool: web::Data<Arc<TransactionPool>>,
) -> ApiResult<MempoolTransaction> {
    let txid = path.into_inner();
    
    // Validate transaction ID format
    let _txid_bytes = Vec::from_hex(&txid).map_err(|_| {
        ApiError::bad_request("Invalid transaction ID format")
    })?;
    
    match mempool.get_transaction(&txid) {
        Ok(Some(tx)) => Ok(HttpResponse::Ok().json(tx)),
        Ok(None) => Err(ApiError::not_found("Transaction not found in mempool")),
        Err(e) => Err(ApiError::internal_error(format!("Failed to get transaction: {}", e))),
    }
}

/// Submit a raw transaction to the mempool
///
/// Submits a raw transaction to be included in the mempool.
#[derive(Debug, Deserialize, Serialize, ToSchema)]
struct SubmitTransactionRequest {
    /// Raw transaction data in hexadecimal format
    raw_tx: String,
    
    /// Whether to allow high fees (default: false)
    #[schema(default = false)]
    allow_high_fees: Option<bool>,
}

#[utoipa::path(
    post,
    path = "/api/v1/mempool/transaction",
    request_body = SubmitTransactionRequest,
    responses(
        (status = 200, description = "Transaction submitted successfully", body = MempoolTransactionSubmissionResponse),
        (status = 400, description = "Invalid transaction data", body = ApiError),
        (status = 500, description = "Internal server error", body = ApiError)
    )
)]
pub async fn submit_mempool_transaction(
    request: web::Json<SubmitTransactionRequest>,
    mempool: web::Data<Arc<TransactionPool>>,
) -> ApiResult<MempoolTransactionSubmissionResponse> {
    let raw_tx = &request.raw_tx;
    let allow_high_fees = request.allow_high_fees.unwrap_or(false);
    
    // Validate and decode the raw transaction
    let tx_bytes = Vec::from_hex(raw_tx).map_err(|_| {
        ApiError::bad_request("Invalid transaction data format")
    })?;
    
    match mempool.submit_transaction(&tx_bytes, allow_high_fees) {
        Ok(txid) => Ok(HttpResponse::Ok().json(MempoolTransactionSubmissionResponse {
            txid,
            accepted: true,
        })),
        Err(e) => Err(ApiError::bad_request(format!("Transaction submission failed: {}", e))),
    }
}

/// Validate a transaction without submitting it to the mempool
///
/// Validates a transaction without adding it to the mempool.
#[derive(Debug, Deserialize, Serialize, ToSchema)]
struct ValidateTransactionRequest {
    /// Raw transaction data in hexadecimal format
    raw_tx: String,
}

#[utoipa::path(
    post,
    path = "/api/v1/mempool/validate_transaction",
    request_body = ValidateTransactionRequest,
    responses(
        (status = 200, description = "Transaction validation result", body = TransactionValidationResult),
        (status = 400, description = "Invalid transaction data", body = ApiError),
        (status = 500, description = "Internal server error", body = ApiError)
    )
)]
pub async fn validate_transaction(
    request: web::Json<ValidateTransactionRequest>,
    mempool: web::Data<Arc<TransactionPool>>,
) -> ApiResult<TransactionValidationResult> {
    let raw_tx = &request.raw_tx;
    
    // Validate and decode the raw transaction
    let tx_bytes = Vec::from_hex(raw_tx).map_err(|_| {
        ApiError::bad_request("Invalid transaction data format")
    })?;
    
    match mempool.validate_transaction(&tx_bytes) {
        Ok(result) => Ok(HttpResponse::Ok().json(result)),
        Err(e) => Err(ApiError::internal_error(format!("Transaction validation failed: {}", e))),
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

#[utoipa::path(
    get,
    path = "/api/v1/mempool/estimate_fee",
    params(
        EstimateFeeParams
    ),
    responses(
        (status = 200, description = "Fee estimation successful", body = TransactionFees),
        (status = 400, description = "Invalid request parameters", body = ApiError),
        (status = 500, description = "Internal server error", body = ApiError)
    )
)]
pub async fn estimate_fee(
    params: web::Query<EstimateFeeParams>,
    mempool: web::Data<Arc<TransactionPool>>,
) -> ApiResult<TransactionFees> {
    let target_conf = params.target_conf.unwrap_or(6);
    
    match mempool.estimate_fee(target_conf) {
        Ok(fees) => Ok(HttpResponse::Ok().json(fees)),
        Err(e) => Err(ApiError::internal_error(format!("Fee estimation failed: {}", e))),
    }
} 