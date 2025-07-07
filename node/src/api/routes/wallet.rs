use crate::api::error::{ApiError, ApiResult};
use crate::api::types::{
    ApiResponse, Address, AddressInfo, WalletInfo, BalanceInfo, 
    Transaction, TransactionList, SignRequest, SignResponse, 
    VerifyRequest, VerifyResponse, SendRequest, SendResponse,
    LabelRequest, LabelResponse, AddressRequest, AddressResponse,
    BackupResponse, UTXOList
};
use crate::node::Node;
use actix_web::{web, HttpResponse, Responder};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::collections::HashMap;
use tracing::{debug, info, warn, error};
use utoipa::{IntoParams, ToSchema};
use hex;
use sha2::Digest;

/// Configure wallet API routes
pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/wallet")
            .route("/info", web::get().to(get_wallet_info))
            .route("/balance", web::get().to(get_wallet_balance))
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
    
    // Since wallet is not fully integrated, return mock data
    // In production, this would query the actual wallet manager
    let wallet_info = WalletInfo {
        name: "default".to_string(),
        balance: 0,
        confirmed_balance: 0,
        unconfirmed_balance: 0,
        address_count: 1,
        tx_count: 0,
        encrypted: true,
        locked: false,
        master_fingerprint: Some("d34db33f".to_string()),
        version: 1,
    };
    
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
    
    let min_conf = query.min_conf.unwrap_or(1);
    let include_watchonly = query.include_watchonly.unwrap_or(false);
    
    // Since wallet is not fully integrated, return mock data
    // In production, this would calculate actual balances from UTXOs
    let balance_info = BalanceInfo {
        total: 0,
        confirmed: 0,
        unconfirmed: 0,
        immature: 0,
        spendable: 0,
    };
    
    Ok(HttpResponse::Ok().json(ApiResponse::success(balance_info)))
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
    
    // Use the node's backup functionality
    match node.create_backup(None, true, false) {
        Ok(backup_info) => {
            // BackupResponse has different fields than BackupInfo, so we need to map them
            let backup_response = BackupResponse {
                backup_data: format!("backup-{}", backup_info.id), // Mock encrypted backup data
                timestamp: backup_info.timestamp,
                version: 1, // API version
                checksum: hex::encode(sha2::Sha256::digest(backup_info.id.as_bytes())), // Mock checksum
            };
            
            Ok(HttpResponse::Ok().json(ApiResponse::success(backup_response)))
        }
        Err(e) => {
            error!("Failed to create backup: {}", e);
            Err(ApiError::internal_error(format!("Backup creation failed: {}", e)))
        }
    }
} 