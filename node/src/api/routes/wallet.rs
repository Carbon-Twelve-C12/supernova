use crate::api::error::{ApiError, ApiResult};
use crate::api::types::{
    ApiResponse, Address, AddressInfo, WalletInfo, BalanceInfo, 
    Transaction, TransactionList, SignRequest, SignResponse, 
    VerifyRequest, VerifyResponse, SendRequest, SendResponse,
    LabelRequest, LabelResponse, AddressRequest, AddressResponse,
    BackupResponse, UTXOList
};
use crate::node::Node;
use crate::wallet::Wallet;
use actix_web::{web, HttpResponse, Responder};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{debug, info, warn, error};
use utoipa::{IntoParams, ToSchema};

/// Configure wallet API routes
pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/wallet")
            .route("/info", web::get().to(get_wallet_info))
            .route("/balance", web::get().to(get_wallet_balance))
            .route("/addresses", web::get().to(list_addresses))
            .route("/addresses/{address}", web::get().to(get_address_info))
            .route("/addresses/new", web::post().to(create_new_address))
            .route("/transactions", web::get().to(list_transactions))
            .route("/transactions/{txid}", web::get().to(get_transaction_info))
            .route("/utxos", web::get().to(list_utxos))
            .route("/send", web::post().to(send_transaction))
            .route("/sign", web::post().to(sign_message_or_tx))
            .route("/verify", web::post().to(verify_message))
            .route("/label", web::post().to(set_address_label))
            .route("/lock", web::post().to(lock_wallet))
            .route("/unlock", web::post().to(unlock_wallet))
            .route("/backup", web::post().to(create_backup)),
    );
}

/// Parameters for wallet info request
#[derive(Debug, Deserialize, IntoParams)]
struct WalletInfoParams {
    /// Wallet ID to query (if multiple wallets supported)
    wallet_id: Option<String>,
}

/// Get wallet information
#[utoipa::path(
    get,
    path = "/api/v1/wallet/info",
    params(
        WalletInfoParams
    ),
    responses(
        (status = 200, description = "Wallet information retrieved successfully", body = ApiResponse<WalletInfo>),
        (status = 401, description = "Unauthorized", body = ApiError),
        (status = 500, description = "Internal server error", body = ApiError)
    ),
    security(
        ("api_key" = [])
    )
)]
pub async fn get_wallet_info(
    node: web::Data<Arc<Node>>,
    query: web::Query<WalletInfoParams>,
) -> ApiResult<impl Responder> {
    debug!("Get wallet info: {:?}", query);
    
    // Get wallet from node
    let wallet = node.get_wallet()
        .ok_or_else(|| ApiError::internal("Wallet not available"))?;
    
    // Get wallet info
    let wallet_info = wallet.get_info()
        .map_err(|e| ApiError::internal(format!("Error getting wallet info: {}", e)))?;
    
    Ok(HttpResponse::Ok().json(ApiResponse::success(wallet_info)))
}

/// Parameters for balance query
#[derive(Debug, Deserialize, IntoParams)]
struct BalanceParams {
    /// Minimum confirmations to include in balance
    min_conf: Option<u32>,
    
    /// Whether to include watch-only addresses
    include_watchonly: Option<bool>,
}

/// Get wallet balance information
#[utoipa::path(
    get,
    path = "/api/v1/wallet/balance",
    params(
        BalanceParams
    ),
    responses(
        (status = 200, description = "Balance information retrieved successfully", body = ApiResponse<BalanceInfo>),
        (status = 401, description = "Unauthorized", body = ApiError),
        (status = 500, description = "Internal server error", body = ApiError)
    ),
    security(
        ("api_key" = [])
    )
)]
pub async fn get_wallet_balance(
    node: web::Data<Arc<Node>>,
    query: web::Query<BalanceParams>,
) -> ApiResult<impl Responder> {
    debug!("Get wallet balance: {:?}", query);
    
    // Get wallet from node
    let wallet = node.get_wallet()
        .ok_or_else(|| ApiError::internal("Wallet not available"))?;
    
    // Apply query parameters
    let min_conf = query.min_conf.unwrap_or(1);
    let include_watchonly = query.include_watchonly.unwrap_or(false);
    
    // Get balance
    let balance = wallet.get_balance(min_conf, include_watchonly)
        .map_err(|e| ApiError::internal(format!("Error getting balance: {}", e)))?;
    
    Ok(HttpResponse::Ok().json(ApiResponse::success(balance)))
}

/// Parameters for listing addresses
#[derive(Debug, Deserialize, IntoParams)]
struct ListAddressesParams {
    /// Only return active addresses
    active_only: Option<bool>,
    
    /// Number of addresses to return
    limit: Option<u32>,
    
    /// Pagination offset
    offset: Option<u32>,
}

/// List wallet addresses
#[utoipa::path(
    get,
    path = "/api/v1/wallet/addresses",
    params(
        ListAddressesParams
    ),
    responses(
        (status = 200, description = "Addresses retrieved successfully", body = ApiResponse<Vec<Address>>),
        (status = 401, description = "Unauthorized", body = ApiError),
        (status = 500, description = "Internal server error", body = ApiError)
    ),
    security(
        ("api_key" = [])
    )
)]
pub async fn list_addresses(
    node: web::Data<Arc<Node>>,
    query: web::Query<ListAddressesParams>,
) -> ApiResult<impl Responder> {
    debug!("List addresses: {:?}", query);
    
    // Get wallet from node
    let wallet = node.get_wallet()
        .ok_or_else(|| ApiError::internal("Wallet not available"))?;
    
    // Apply query parameters
    let active_only = query.active_only.unwrap_or(false);
    let limit = query.limit.unwrap_or(100);
    let offset = query.offset.unwrap_or(0);
    
    // Get addresses
    let addresses = wallet.list_addresses(active_only, limit, offset)
        .map_err(|e| ApiError::internal(format!("Error listing addresses: {}", e)))?;
    
    Ok(HttpResponse::Ok().json(ApiResponse::success(addresses)))
}

/// Parameters for address info
#[derive(Debug, Deserialize, IntoParams)]
struct AddressInfoParams {
    /// The address to query
    address: String,
}

/// Get information about a specific address
#[utoipa::path(
    get,
    path = "/api/v1/wallet/addresses/{address}",
    params(
        AddressInfoParams
    ),
    responses(
        (status = 200, description = "Address information retrieved successfully", body = ApiResponse<AddressInfo>),
        (status = 401, description = "Unauthorized", body = ApiError),
        (status = 404, description = "Address not found", body = ApiError),
        (status = 500, description = "Internal server error", body = ApiError)
    ),
    security(
        ("api_key" = [])
    )
)]
pub async fn get_address_info(
    node: web::Data<Arc<Node>>,
    path: web::Path<AddressInfoParams>,
) -> ApiResult<impl Responder> {
    let address = &path.address;
    debug!("Get address info: {}", address);
    
    // Get wallet from node
    let wallet = node.get_wallet()
        .ok_or_else(|| ApiError::internal("Wallet not available"))?;
    
    // Check if address belongs to wallet
    if !wallet.is_mine(address) {
        return Err(ApiError::not_found("Address not found in wallet"));
    }
    
    // Get address info
    let address_info = wallet.get_address_info(address)
        .map_err(|e| ApiError::internal(format!("Error getting address info: {}", e)))?;
    
    Ok(HttpResponse::Ok().json(ApiResponse::success(address_info)))
}

/// Create a new address
#[utoipa::path(
    post,
    path = "/api/v1/wallet/addresses/new",
    request_body = AddressRequest,
    responses(
        (status = 200, description = "Address created successfully", body = ApiResponse<AddressResponse>),
        (status = 400, description = "Invalid request", body = ApiError),
        (status = 401, description = "Unauthorized", body = ApiError),
        (status = 500, description = "Internal server error", body = ApiError)
    ),
    security(
        ("api_key" = [])
    )
)]
pub async fn create_new_address(
    node: web::Data<Arc<Node>>,
    req: web::Json<AddressRequest>,
) -> ApiResult<impl Responder> {
    debug!("Create new address: {:?}", req);
    
    // Get wallet from node
    let wallet = node.get_wallet()
        .ok_or_else(|| ApiError::internal("Wallet not available"))?;
    
    // Apply request parameters
    let label = req.label.clone();
    let address_type = req.type_.clone().unwrap_or_else(|| "receive".to_string());
    let quantum_resistant = req.quantum_resistant.unwrap_or(false);
    
    // Create new address
    let address_response = wallet.create_new_address(&address_type, &label, quantum_resistant)
        .map_err(|e| ApiError::internal(format!("Error creating address: {}", e)))?;
    
    Ok(HttpResponse::Ok().json(ApiResponse::success(address_response)))
}

/// Parameters for listing transactions
#[derive(Debug, Deserialize, IntoParams)]
struct ListTransactionsParams {
    /// Number of transactions to return
    limit: Option<u32>,
    
    /// Pagination offset
    offset: Option<u32>,
    
    /// Include raw transaction data
    include_raw: Option<bool>,
    
    /// Filter by category
    category: Option<String>,
    
    /// Filter by start time (ISO format)
    start_time: Option<String>,
    
    /// Filter by end time (ISO format)
    end_time: Option<String>,
}

/// List wallet transactions
#[utoipa::path(
    get,
    path = "/api/v1/wallet/transactions",
    params(
        ListTransactionsParams
    ),
    responses(
        (status = 200, description = "Transactions retrieved successfully", body = ApiResponse<TransactionList>),
        (status = 401, description = "Unauthorized", body = ApiError),
        (status = 500, description = "Internal server error", body = ApiError)
    ),
    security(
        ("api_key" = [])
    )
)]
pub async fn list_transactions(
    node: web::Data<Arc<Node>>,
    query: web::Query<ListTransactionsParams>,
) -> ApiResult<impl Responder> {
    debug!("List transactions: {:?}", query);
    
    // Get wallet from node
    let wallet = node.get_wallet()
        .ok_or_else(|| ApiError::internal("Wallet not available"))?;
    
    // Apply query parameters
    let limit = query.limit.unwrap_or(50);
    let offset = query.offset.unwrap_or(0);
    let include_raw = query.include_raw.unwrap_or(false);
    
    // Parse filter parameters
    let mut filter_opts = HashMap::new();
    if let Some(category) = &query.category {
        filter_opts.insert("category".to_string(), category.clone());
    }
    if let Some(start_time) = &query.start_time {
        filter_opts.insert("start_time".to_string(), start_time.clone());
    }
    if let Some(end_time) = &query.end_time {
        filter_opts.insert("end_time".to_string(), end_time.clone());
    }
    
    // Get transactions
    let txs = wallet.list_transactions(limit, offset, include_raw, filter_opts)
        .map_err(|e| ApiError::internal(format!("Error listing transactions: {}", e)))?;
    
    Ok(HttpResponse::Ok().json(ApiResponse::success(txs)))
}

/// Parameters for transaction info
#[derive(Debug, Deserialize, IntoParams)]
struct TransactionInfoParams {
    /// The transaction ID
    txid: String,
    
    /// Include raw transaction data
    include_raw: Option<bool>,
}

/// Get information about a specific transaction
#[utoipa::path(
    get,
    path = "/api/v1/wallet/transactions/{txid}",
    params(
        TransactionInfoParams
    ),
    responses(
        (status = 200, description = "Transaction information retrieved successfully", body = ApiResponse<Transaction>),
        (status = 401, description = "Unauthorized", body = ApiError),
        (status = 404, description = "Transaction not found", body = ApiError),
        (status = 500, description = "Internal server error", body = ApiError)
    ),
    security(
        ("api_key" = [])
    )
)]
pub async fn get_transaction_info(
    node: web::Data<Arc<Node>>,
    path: web::Path<String>,
    query: web::Query<TransactionInfoParams>,
) -> ApiResult<impl Responder> {
    let txid = path.into_inner();
    debug!("Get transaction info: {} (include_raw={})", txid, query.include_raw.unwrap_or(false));
    
    // Get wallet from node
    let wallet = node.get_wallet()
        .ok_or_else(|| ApiError::internal("Wallet not available"))?;
    
    // Apply query parameters
    let include_raw = query.include_raw.unwrap_or(false);
    
    // Get transaction info
    let tx = wallet.get_transaction(&txid, include_raw)
        .map_err(|e| {
            if e.to_string().contains("not found") {
                ApiError::not_found("Transaction not found")
            } else {
                ApiError::internal(format!("Error getting transaction: {}", e))
            }
        })?;
    
    Ok(HttpResponse::Ok().json(ApiResponse::success(tx)))
}

/// Parameters for listing UTXOs
#[derive(Debug, Deserialize, IntoParams)]
struct ListUtxosParams {
    /// Minimum confirmations
    min_conf: Option<u32>,
    
    /// Maximum confirmations
    max_conf: Option<u32>,
    
    /// Minimum amount
    min_amount: Option<f64>,
    
    /// Maximum amount
    max_amount: Option<f64>,
    
    /// Include outputs that are not safe to spend
    include_unsafe: Option<bool>,
}

/// List unspent transaction outputs (UTXOs)
#[utoipa::path(
    get,
    path = "/api/v1/wallet/utxos",
    params(
        ListUtxosParams
    ),
    responses(
        (status = 200, description = "UTXOs retrieved successfully", body = ApiResponse<UTXOList>),
        (status = 401, description = "Unauthorized", body = ApiError),
        (status = 500, description = "Internal server error", body = ApiError)
    ),
    security(
        ("api_key" = [])
    )
)]
pub async fn list_utxos(
    node: web::Data<Arc<Node>>,
    query: web::Query<ListUtxosParams>,
) -> ApiResult<impl Responder> {
    debug!("List UTXOs: {:?}", query);
    
    // Get wallet from node
    let wallet = node.get_wallet()
        .ok_or_else(|| ApiError::internal("Wallet not available"))?;
    
    // Apply query parameters
    let min_conf = query.min_conf.unwrap_or(1);
    let max_conf = query.max_conf.unwrap_or(9999999);
    let min_amount = query.min_amount.unwrap_or(0.0);
    let max_amount = query.max_amount.unwrap_or(f64::MAX);
    let include_unsafe = query.include_unsafe.unwrap_or(false);
    
    // Get UTXOs
    let utxos = wallet.list_unspent(min_conf, max_conf, min_amount, max_amount, include_unsafe)
        .map_err(|e| ApiError::internal(format!("Error listing UTXOs: {}", e)))?;
    
    Ok(HttpResponse::Ok().json(ApiResponse::success(utxos)))
}

/// Send a transaction
#[utoipa::path(
    post,
    path = "/api/v1/wallet/send",
    request_body = SendRequest,
    responses(
        (status = 200, description = "Transaction sent successfully", body = ApiResponse<SendResponse>),
        (status = 400, description = "Invalid request", body = ApiError),
        (status = 401, description = "Unauthorized", body = ApiError),
        (status = 500, description = "Internal server error", body = ApiError)
    ),
    security(
        ("api_key" = [])
    )
)]
pub async fn send_transaction(
    node: web::Data<Arc<Node>>,
    req: web::Json<SendRequest>,
) -> ApiResult<impl Responder> {
    debug!("Send transaction: {:?}", req);
    
    // Get wallet from node
    let wallet = node.get_wallet()
        .ok_or_else(|| ApiError::internal("Wallet not available"))?;
    
    // Validate request
    if req.outputs.is_empty() {
        return Err(ApiError::bad_request("No outputs specified"));
    }
    
    // Apply request parameters
    let fee_rate = req.fee_rate.unwrap_or(1.0);
    let subtract_fee = req.subtract_fee_from_amount.unwrap_or(false);
    let replaceable = req.replaceable.unwrap_or(true);
    let comment = req.comment.clone().unwrap_or_default();
    
    // Create and send transaction
    let send_response = wallet.send_transaction(&req.outputs, fee_rate, subtract_fee, replaceable, &comment)
        .map_err(|e| {
            if e.to_string().contains("insufficient funds") {
                ApiError::bad_request("Insufficient funds")
            } else {
                ApiError::internal(format!("Error sending transaction: {}", e))
            }
        })?;
    
    Ok(HttpResponse::Ok().json(ApiResponse::success(send_response)))
}

/// Sign a message or transaction
#[utoipa::path(
    post,
    path = "/api/v1/wallet/sign",
    request_body = SignRequest,
    responses(
        (status = 200, description = "Signing successful", body = ApiResponse<SignResponse>),
        (status = 400, description = "Invalid request", body = ApiError),
        (status = 401, description = "Unauthorized", body = ApiError),
        (status = 500, description = "Internal server error", body = ApiError)
    ),
    security(
        ("api_key" = [])
    )
)]
pub async fn sign_message_or_tx(
    node: web::Data<Arc<Node>>,
    req: web::Json<SignRequest>,
) -> ApiResult<impl Responder> {
    debug!("Sign message or transaction: {:?}", req);
    
    // Get wallet from node
    let wallet = node.get_wallet()
        .ok_or_else(|| ApiError::internal("Wallet not available"))?;
    
    // Validate request
    if req.data.is_empty() {
        return Err(ApiError::bad_request("No data provided to sign"));
    }
    
    if req.type_ == "message" && req.address.is_none() {
        return Err(ApiError::bad_request("Address is required for message signing"));
    }
    
    // Sign message or transaction
    let sign_response = match req.type_.as_str() {
        "message" => {
            let address = req.address.as_ref().unwrap();
            wallet.sign_message(&req.data, address)
                .map_err(|e| ApiError::internal(format!("Error signing message: {}", e)))?
        },
        "transaction" => {
            wallet.sign_transaction(&req.data)
                .map_err(|e| ApiError::internal(format!("Error signing transaction: {}", e)))?
        },
        _ => return Err(ApiError::bad_request("Invalid type, must be 'message' or 'transaction'")),
    };
    
    Ok(HttpResponse::Ok().json(ApiResponse::success(sign_response)))
}

/// Verify a message signature
#[utoipa::path(
    post,
    path = "/api/v1/wallet/verify",
    request_body = VerifyRequest,
    responses(
        (status = 200, description = "Verification successful", body = ApiResponse<VerifyResponse>),
        (status = 400, description = "Invalid request", body = ApiError),
        (status = 401, description = "Unauthorized", body = ApiError),
        (status = 500, description = "Internal server error", body = ApiError)
    ),
    security(
        ("api_key" = [])
    )
)]
pub async fn verify_message(
    node: web::Data<Arc<Node>>,
    req: web::Json<VerifyRequest>,
) -> ApiResult<impl Responder> {
    debug!("Verify message signature: {:?}", req);
    
    // Get wallet from node
    let wallet = node.get_wallet()
        .ok_or_else(|| ApiError::internal("Wallet not available"))?;
    
    // Validate request
    if req.message.is_empty() {
        return Err(ApiError::bad_request("Message is required"));
    }
    if req.signature.is_empty() {
        return Err(ApiError::bad_request("Signature is required"));
    }
    if req.address.is_empty() {
        return Err(ApiError::bad_request("Address is required"));
    }
    
    // Verify message signature
    let verify_response = wallet.verify_message(&req.message, &req.signature, &req.address)
        .map_err(|e| ApiError::internal(format!("Error verifying message: {}", e)))?;
    
    Ok(HttpResponse::Ok().json(ApiResponse::success(verify_response)))
}

/// Set a label for an address
#[utoipa::path(
    post,
    path = "/api/v1/wallet/label",
    request_body = LabelRequest,
    responses(
        (status = 200, description = "Label set successfully", body = ApiResponse<LabelResponse>),
        (status = 400, description = "Invalid request", body = ApiError),
        (status = 401, description = "Unauthorized", body = ApiError),
        (status = 404, description = "Address not found", body = ApiError),
        (status = 500, description = "Internal server error", body = ApiError)
    ),
    security(
        ("api_key" = [])
    )
)]
pub async fn set_address_label(
    node: web::Data<Arc<Node>>,
    req: web::Json<LabelRequest>,
) -> ApiResult<impl Responder> {
    debug!("Set address label: {:?}", req);
    
    // Get wallet from node
    let wallet = node.get_wallet()
        .ok_or_else(|| ApiError::internal("Wallet not available"))?;
    
    // Validate request
    if req.address.is_empty() {
        return Err(ApiError::bad_request("Address is required"));
    }
    
    // Check if address belongs to wallet
    if !wallet.is_mine(&req.address) {
        return Err(ApiError::not_found("Address not found in wallet"));
    }
    
    // Set address label
    let label_response = wallet.set_address_label(&req.address, &req.label)
        .map_err(|e| ApiError::internal(format!("Error setting address label: {}", e)))?;
    
    Ok(HttpResponse::Ok().json(ApiResponse::success(label_response)))
}

/// Lock the wallet
#[utoipa::path(
    post,
    path = "/api/v1/wallet/lock",
    responses(
        (status = 200, description = "Wallet locked successfully", body = ApiResponse<bool>),
        (status = 401, description = "Unauthorized", body = ApiError),
        (status = 500, description = "Internal server error", body = ApiError)
    ),
    security(
        ("api_key" = [])
    )
)]
pub async fn lock_wallet(
    node: web::Data<Arc<Node>>,
) -> ApiResult<impl Responder> {
    debug!("Lock wallet");
    
    // Get wallet from node
    let wallet = node.get_wallet()
        .ok_or_else(|| ApiError::internal("Wallet not available"))?;
    
    // Lock wallet
    wallet.lock()
        .map_err(|e| ApiError::internal(format!("Error locking wallet: {}", e)))?;
    
    let response = serde_json::json!({
        "locked": true
    });
    
    Ok(HttpResponse::Ok().json(ApiResponse::success(response)))
}

/// Parameters for unlocking the wallet
#[derive(Debug, Deserialize, ToSchema)]
struct UnlockRequest {
    /// The wallet passphrase
    passphrase: String,
    
    /// Lock after this many seconds
    timeout: Option<u32>,
}

/// Unlock the wallet
#[utoipa::path(
    post,
    path = "/api/v1/wallet/unlock",
    request_body = UnlockRequest,
    responses(
        (status = 200, description = "Wallet unlocked successfully", body = ApiResponse<bool>),
        (status = 400, description = "Invalid passphrase", body = ApiError),
        (status = 401, description = "Unauthorized", body = ApiError),
        (status = 500, description = "Internal server error", body = ApiError)
    ),
    security(
        ("api_key" = [])
    )
)]
pub async fn unlock_wallet(
    node: web::Data<Arc<Node>>,
    req: web::Json<UnlockRequest>,
) -> ApiResult<impl Responder> {
    debug!("Unlock wallet");
    
    // Get wallet from node
    let wallet = node.get_wallet()
        .ok_or_else(|| ApiError::internal("Wallet not available"))?;
    
    // Apply request parameters
    let timeout = req.timeout.unwrap_or(0);
    
    // Unlock wallet
    let result = wallet.unlock(&req.passphrase, timeout)
        .map_err(|e| {
            if e.to_string().contains("incorrect passphrase") {
                ApiError::bad_request("Incorrect passphrase")
            } else {
                ApiError::internal(format!("Error unlocking wallet: {}", e))
            }
        })?;
    
    let response = serde_json::json!({
        "unlocked": result,
        "timeout": timeout
    });
    
    Ok(HttpResponse::Ok().json(ApiResponse::success(response)))
}

/// Create a wallet backup
#[utoipa::path(
    post,
    path = "/api/v1/wallet/backup",
    responses(
        (status = 200, description = "Backup created successfully", body = ApiResponse<BackupResponse>),
        (status = 401, description = "Unauthorized", body = ApiError),
        (status = 500, description = "Internal server error", body = ApiError)
    ),
    security(
        ("api_key" = [])
    )
)]
pub async fn create_backup(
    node: web::Data<Arc<Node>>,
) -> ApiResult<impl Responder> {
    debug!("Create wallet backup");
    
    // Get wallet from node
    let wallet = node.get_wallet()
        .ok_or_else(|| ApiError::internal("Wallet not available"))?;
    
    // Create backup
    let backup_response = wallet.create_backup()
        .map_err(|e| ApiError::internal(format!("Error creating backup: {}", e)))?;
    
    Ok(HttpResponse::Ok().json(ApiResponse::success(backup_response)))
} 