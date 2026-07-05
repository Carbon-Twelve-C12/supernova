use crate::api::error::{ApiError, ApiResult};
use crate::api::types::{ApiResponse, BackupResponse, BalanceInfo, WalletInfo};
use crate::node::Node;
use actix_web::{web, HttpResponse, Responder};
use hex;
use serde::Deserialize;
use sha2::Digest;
use std::sync::Arc;
use tracing::{debug, error};
use utoipa::IntoParams;

/// Configure wallet API routes
pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.route("/info", web::get().to(get_wallet_info))
        .route("/balance", web::get().to(get_wallet_balance))
        .route("/backup", web::post().to(create_backup));
}

/// Decompose a total balance into its unconfirmed remainder given the confirmed
/// portion.
///
/// Uses saturating subtraction so that a transiently inconsistent UTXO index
/// (where the 1-confirmation balance momentarily exceeds the 0-confirmation
/// total) can never underflow-panic in debug builds.
fn unconfirmed_remainder(total: u64, confirmed: u64) -> u64 {
    total.saturating_sub(confirmed)
}

/// Compute a SHA-256 checksum, hex-encoded, over the actual bytes of a backup
/// file on disk.
///
/// This binds the reported checksum to the real backup payload (so callers can
/// verify integrity) rather than to an opaque identifier.
fn backup_file_checksum(path: &str) -> std::io::Result<String> {
    let bytes = std::fs::read(path)?;
    Ok(hex::encode(sha2::Sha256::digest(&bytes)))
}

/// Parameters for wallet info request
#[derive(Debug, Deserialize, IntoParams)]
pub struct WalletInfoParams {
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

    // Query the real wallet manager rather than returning fabricated data.
    let wallet_manager = node
        .get_wallet_manager()
        .ok_or_else(|| ApiError::service_unavailable("Wallet is not configured on this node"))?;
    let wallet = wallet_manager
        .read()
        .map_err(|_| ApiError::internal_error("Wallet lock poisoned"))?;

    let keystore = wallet.keystore();
    let locked = keystore.is_locked();
    let address_count = keystore
        .list_addresses()
        .map(|addrs| addrs.len() as u32)
        .unwrap_or(0);

    // Confirmed balance requires at least 1 confirmation; the 0-confirmation
    // total additionally includes mempool (unconfirmed) UTXOs.
    let confirmed_balance = wallet
        .get_balance(1)
        .map_err(|e| ApiError::internal_error(format!("Failed to get balance: {}", e)))?;
    let total_balance = wallet
        .get_balance(0)
        .map_err(|e| ApiError::internal_error(format!("Failed to get balance: {}", e)))?;
    let unconfirmed_balance = unconfirmed_remainder(total_balance, confirmed_balance);

    let wallet_info = WalletInfo {
        name: "default".to_string(),
        balance: total_balance,
        confirmed_balance,
        unconfirmed_balance,
        address_count,
        // No wallet-level transaction-history index is tracked yet; report 0
        // rather than a fabricated count.
        tx_count: 0,
        // Keystore is Argon2id-encrypted on disk (see WalletManager::new).
        encrypted: true,
        locked,
        // No BIP32-style master fingerprint is derived for quantum keystores.
        master_fingerprint: None,
        version: 1,
    };

    Ok(HttpResponse::Ok().json(ApiResponse::success(wallet_info)))
}

/// Parameters for balance query
#[derive(Debug, Deserialize, IntoParams)]
pub struct BalanceParams {
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

    let min_conf = u64::from(query.min_conf.unwrap_or(1));
    let _include_watchonly = query.include_watchonly.unwrap_or(false);

    // Calculate actual balances from the wallet's UTXO index.
    let wallet_manager = node
        .get_wallet_manager()
        .ok_or_else(|| ApiError::service_unavailable("Wallet is not configured on this node"))?;
    let wallet = wallet_manager
        .read()
        .map_err(|_| ApiError::internal_error("Wallet lock poisoned"))?;

    let confirmed = wallet
        .get_balance(min_conf.max(1))
        .map_err(|e| ApiError::internal_error(format!("Failed to get balance: {}", e)))?;
    let total = wallet
        .get_balance(0)
        .map_err(|e| ApiError::internal_error(format!("Failed to get balance: {}", e)))?;
    let unconfirmed = unconfirmed_remainder(total, confirmed);

    let balance_info = BalanceInfo {
        total,
        confirmed,
        unconfirmed,
        // Coinbase maturity is not separately tracked at the wallet layer yet.
        immature: 0,
        spendable: confirmed,
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
pub async fn create_backup(node: web::Data<Arc<Node>>) -> ApiResult<impl Responder> {
    debug!("Create wallet backup");

    // Use the node's backup functionality
    match node.create_backup(None, true, false) {
        Ok(backup_info) => {
            // Compute a checksum over the actual backup file bytes so callers can
            // verify the integrity of the real payload on disk, rather than a
            // checksum of an opaque identifier.
            let checksum = match backup_file_checksum(&backup_info.file_path) {
                Ok(checksum) => checksum,
                Err(e) => {
                    error!(
                        "Backup created at {} but could not be read for verification: {}",
                        backup_info.file_path, e
                    );
                    return Err(ApiError::internal_error(format!(
                        "Backup created but could not be read for verification: {}",
                        e
                    )));
                }
            };

            let backup_response = BackupResponse {
                // Real on-disk location (handle) of the backup file produced by
                // the node, not a fabricated placeholder.
                backup_data: backup_info.file_path,
                timestamp: backup_info.timestamp,
                version: 1, // API response schema version
                checksum,
            };

            Ok(HttpResponse::Ok().json(ApiResponse::success(backup_response)))
        }
        Err(e) => {
            error!("Failed to create backup: {}", e);
            Err(ApiError::internal_error(format!(
                "Backup creation failed: {}",
                e
            )))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{backup_file_checksum, unconfirmed_remainder};
    use sha2::Digest;

    #[test]
    fn backup_checksum_is_over_actual_file_bytes() {
        // The reported checksum must be bound to the real backup payload on
        // disk, not to an opaque identifier.
        let mut path = std::env::temp_dir();
        path.push(format!("supernova_backup_checksum_test_{}", std::process::id()));
        let contents = b"supernova-real-backup-payload";
        std::fs::write(&path, contents).expect("write temp backup file");

        let got = backup_file_checksum(path.to_str().unwrap()).expect("checksum ok");
        let expected = hex::encode(sha2::Sha256::digest(contents));
        assert_eq!(got, expected, "checksum must match sha256 of file bytes");

        // Guard against the previous fabricated behavior: a checksum over a
        // random id string must NOT match the checksum over the file contents.
        let id_checksum = hex::encode(sha2::Sha256::digest(b"some-uuid-id"));
        assert_ne!(got, id_checksum);

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn backup_checksum_errors_on_missing_file() {
        let missing = "/nonexistent/supernova/backup/path/that/should/not/exist";
        assert!(backup_file_checksum(missing).is_err());
    }

    #[test]
    fn unconfirmed_is_total_minus_confirmed() {
        // Normal case: total includes the confirmed portion plus mempool funds.
        assert_eq!(unconfirmed_remainder(1_000, 400), 600);
    }

    #[test]
    fn unconfirmed_is_zero_when_all_confirmed() {
        assert_eq!(unconfirmed_remainder(500, 500), 0);
    }

    #[test]
    fn unconfirmed_saturates_when_confirmed_exceeds_total() {
        // A transiently inconsistent index must never underflow-panic.
        assert_eq!(unconfirmed_remainder(400, 1_000), 0);
    }
}
