use actix_web::{web, HttpResponse, Responder, Error};
use serde::{Serialize, Deserialize};
use std::sync::Arc;
use crate::node::NodeHandle;
use btclib::environmental::EmissionsTracker;
use btclib::types::block::Block;
use btclib::types::transaction::Transaction;
use chrono::{DateTime, Utc};

// API response wrapper for consistent formatting
#[derive(Serialize)]
struct ApiResponse<T>
where
    T: Serialize,
{
    success: bool,
    data: Option<T>,
    error: Option<String>,
    timestamp: DateTime<Utc>,
}

impl<T> ApiResponse<T>
where
    T: Serialize,
{
    fn success(data: T) -> Self {
        Self {
            success: true,
            data: Some(data),
            error: None,
            timestamp: Utc::now(),
        }
    }

    fn error(message: String) -> Self {
        Self {
            success: false,
            data: None,
            error: Some(message),
            timestamp: Utc::now(),
        }
    }
}

// Blockchain API route handlers

/// Get node status information
pub async fn get_node_status(node: web::Data<Arc<NodeHandle>>) -> Result<HttpResponse, Error> {
    let status = node.get_status().await.map_err(|e| {
        actix_web::error::ErrorInternalServerError(format!("Failed to get node status: {}", e))
    })?;

    Ok(HttpResponse::Ok().json(ApiResponse::success(status)))
}

/// Get blockchain information
pub async fn get_blockchain_info(node: web::Data<Arc<NodeHandle>>) -> Result<HttpResponse, Error> {
    let chain_info = node.get_blockchain_info().await.map_err(|e| {
        actix_web::error::ErrorInternalServerError(format!("Failed to get blockchain info: {}", e))
    })?;

    Ok(HttpResponse::Ok().json(ApiResponse::success(chain_info)))
}

/// Get block by height
#[derive(Deserialize)]
pub struct BlockHeightParams {
    height: u64,
}

pub async fn get_block_by_height(
    params: web::Path<BlockHeightParams>,
    node: web::Data<Arc<NodeHandle>>,
) -> Result<HttpResponse, Error> {
    let block = node.get_block_by_height(params.height).await.map_err(|e| {
        actix_web::error::ErrorInternalServerError(format!("Failed to get block: {}", e))
    })?;

    match block {
        Some(block) => Ok(HttpResponse::Ok().json(ApiResponse::success(block))),
        None => Ok(HttpResponse::NotFound().json(ApiResponse::<Block>::error(format!(
            "Block at height {} not found",
            params.height
        )))),
    }
}

/// Get block by hash
#[derive(Deserialize)]
pub struct BlockHashParams {
    hash: String,
}

pub async fn get_block_by_hash(
    params: web::Path<BlockHashParams>,
    node: web::Data<Arc<NodeHandle>>,
) -> Result<HttpResponse, Error> {
    // Parse hash from hex
    let hash_bytes = hex::decode(&params.hash).map_err(|_| {
        actix_web::error::ErrorBadRequest("Invalid block hash format")
    })?;

    let mut hash = [0u8; 32];
    if hash_bytes.len() != 32 {
        return Err(actix_web::error::ErrorBadRequest("Invalid block hash length"));
    }
    hash.copy_from_slice(&hash_bytes);

    let block = node.get_block_by_hash(&hash).await.map_err(|e| {
        actix_web::error::ErrorInternalServerError(format!("Failed to get block: {}", e))
    })?;

    match block {
        Some(block) => Ok(HttpResponse::Ok().json(ApiResponse::success(block))),
        None => Ok(HttpResponse::NotFound().json(ApiResponse::<Block>::error(format!(
            "Block with hash {} not found",
            params.hash
        )))),
    }
}

/// Get transaction by hash
#[derive(Deserialize)]
pub struct TxHashParams {
    hash: String,
}

pub async fn get_transaction(
    params: web::Path<TxHashParams>,
    node: web::Data<Arc<NodeHandle>>,
) -> Result<HttpResponse, Error> {
    // Parse hash from hex
    let hash_bytes = hex::decode(&params.hash).map_err(|_| {
        actix_web::error::ErrorBadRequest("Invalid transaction hash format")
    })?;

    let mut hash = [0u8; 32];
    if hash_bytes.len() != 32 {
        return Err(actix_web::error::ErrorBadRequest("Invalid transaction hash length"));
    }
    hash.copy_from_slice(&hash_bytes);

    let tx = node.get_transaction(&hash).await.map_err(|e| {
        actix_web::error::ErrorInternalServerError(format!("Failed to get transaction: {}", e))
    })?;

    match tx {
        Some(tx) => Ok(HttpResponse::Ok().json(ApiResponse::success(tx))),
        None => Ok(HttpResponse::NotFound().json(ApiResponse::<Transaction>::error(format!(
            "Transaction with hash {} not found",
            params.hash
        )))),
    }
}

/// Submit a transaction to the mempool
#[derive(Deserialize)]
pub struct SubmitTxRequest {
    raw_tx: String,
}

pub async fn submit_transaction(
    tx_req: web::Json<SubmitTxRequest>,
    node: web::Data<Arc<NodeHandle>>,
) -> Result<HttpResponse, Error> {
    let raw_tx = hex::decode(&tx_req.raw_tx).map_err(|_| {
        actix_web::error::ErrorBadRequest("Invalid transaction format")
    })?;

    let tx: Transaction = bincode::deserialize(&raw_tx).map_err(|_| {
        actix_web::error::ErrorBadRequest("Invalid transaction data")
    })?;

    // Submit to mempool
    let tx_hash = node.submit_transaction(tx).await.map_err(|e| {
        actix_web::error::ErrorInternalServerError(format!("Failed to submit transaction: {}", e))
    })?;

    // Return the transaction hash
    #[derive(Serialize)]
    struct TxResponse {
        txid: String,
    }

    Ok(HttpResponse::Ok().json(ApiResponse::success(TxResponse {
        txid: hex::encode(tx_hash),
    })))
}

// Mempool API route handlers

/// Get mempool information
pub async fn get_mempool_info(node: web::Data<Arc<NodeHandle>>) -> Result<HttpResponse, Error> {
    let mempool_info = node.get_mempool_info().await.map_err(|e| {
        actix_web::error::ErrorInternalServerError(format!("Failed to get mempool info: {}", e))
    })?;

    Ok(HttpResponse::Ok().json(ApiResponse::success(mempool_info)))
}

/// Get mempool transactions
pub async fn get_mempool_transactions(node: web::Data<Arc<NodeHandle>>) -> Result<HttpResponse, Error> {
    let txs = node.get_mempool_transactions().await.map_err(|e| {
        actix_web::error::ErrorInternalServerError(format!("Failed to get mempool transactions: {}", e))
    })?;

    Ok(HttpResponse::Ok().json(ApiResponse::success(txs)))
}

// Network API route handlers

/// Get peer information
pub async fn get_peer_info(node: web::Data<Arc<NodeHandle>>) -> Result<HttpResponse, Error> {
    let peers = node.get_peer_info().await.map_err(|e| {
        actix_web::error::ErrorInternalServerError(format!("Failed to get peer info: {}", e))
    })?;

    Ok(HttpResponse::Ok().json(ApiResponse::success(peers)))
}

/// Get network information
pub async fn get_network_info(node: web::Data<Arc<NodeHandle>>) -> Result<HttpResponse, Error> {
    let network_info = node.get_network_info().await.map_err(|e| {
        actix_web::error::ErrorInternalServerError(format!("Failed to get network info: {}", e))
    })?;

    Ok(HttpResponse::Ok().json(ApiResponse::success(network_info)))
}

// Mining API route handlers

/// Get mining info
pub async fn get_mining_info(node: web::Data<Arc<NodeHandle>>) -> Result<HttpResponse, Error> {
    let mining_info = node.get_mining_info().await.map_err(|e| {
        actix_web::error::ErrorInternalServerError(format!("Failed to get mining info: {}", e))
    })?;

    Ok(HttpResponse::Ok().json(ApiResponse::success(mining_info)))
}

/// Get block template for mining
pub async fn get_block_template(node: web::Data<Arc<NodeHandle>>) -> Result<HttpResponse, Error> {
    let template = node.get_block_template().await.map_err(|e| {
        actix_web::error::ErrorInternalServerError(format!("Failed to get block template: {}", e))
    })?;

    Ok(HttpResponse::Ok().json(ApiResponse::success(template)))
}

/// Submit a mined block
#[derive(Deserialize)]
pub struct SubmitBlockRequest {
    raw_block: String,
}

pub async fn submit_block(
    block_req: web::Json<SubmitBlockRequest>,
    node: web::Data<Arc<NodeHandle>>,
) -> Result<HttpResponse, Error> {
    let raw_block = hex::decode(&block_req.raw_block).map_err(|_| {
        actix_web::error::ErrorBadRequest("Invalid block format")
    })?;

    let block: Block = bincode::deserialize(&raw_block).map_err(|_| {
        actix_web::error::ErrorBadRequest("Invalid block data")
    })?;

    // Submit the block
    let result = node.submit_block(block).await.map_err(|e| {
        actix_web::error::ErrorInternalServerError(format!("Failed to submit block: {}", e))
    })?;

    // Return the result
    #[derive(Serialize)]
    struct SubmitBlockResponse {
        accepted: bool,
        reason: Option<String>,
    }

    Ok(HttpResponse::Ok().json(ApiResponse::success(SubmitBlockResponse {
        accepted: result.accepted,
        reason: result.reason,
    })))
}

// Environmental API route handlers

/// Get environmental metrics
pub async fn get_environmental_metrics(
    node: web::Data<Arc<NodeHandle>>,
) -> Result<HttpResponse, Error> {
    let metrics = node.get_environmental_metrics().await.map_err(|e| {
        actix_web::error::ErrorInternalServerError(format!("Failed to get environmental metrics: {}", e))
    })?;

    Ok(HttpResponse::Ok().json(ApiResponse::success(metrics)))
}

/// Get treasury status
pub async fn get_treasury_status(
    node: web::Data<Arc<NodeHandle>>,
) -> Result<HttpResponse, Error> {
    let treasury = node.get_treasury_status().await.map_err(|e| {
        actix_web::error::ErrorInternalServerError(format!("Failed to get treasury status: {}", e))
    })?;

    Ok(HttpResponse::Ok().json(ApiResponse::success(treasury)))
}

/// Get emissions data for a transaction
pub async fn get_transaction_emissions(
    params: web::Path<TxHashParams>,
    node: web::Data<Arc<NodeHandle>>,
) -> Result<HttpResponse, Error> {
    // Parse hash from hex
    let hash_bytes = hex::decode(&params.hash).map_err(|_| {
        actix_web::error::ErrorBadRequest("Invalid transaction hash format")
    })?;

    let mut hash = [0u8; 32];
    if hash_bytes.len() != 32 {
        return Err(actix_web::error::ErrorBadRequest("Invalid transaction hash length"));
    }
    hash.copy_from_slice(&hash_bytes);

    let emissions = node.get_transaction_emissions(&hash).await.map_err(|e| {
        actix_web::error::ErrorInternalServerError(format!("Failed to get transaction emissions: {}", e))
    })?;

    Ok(HttpResponse::Ok().json(ApiResponse::success(emissions)))
}

/// Register renewable energy source for mining
#[derive(Deserialize)]
pub struct RenewableEnergyRequest {
    /// Mining pool identifier
    pool_id: String,
    /// Percentage of renewable energy used (0-100)
    renewable_percentage: u8,
    /// Energy provider name
    provider: String,
    /// Optional certificate identifier
    certificate: Option<String>,
    /// Contact information for verification
    contact_email: String,
}

pub async fn register_renewable_energy(
    req: web::Json<RenewableEnergyRequest>,
    node: web::Data<Arc<NodeHandle>>,
) -> Result<HttpResponse, Error> {
    // Validate input data
    if req.renewable_percentage > 100 {
        return Err(actix_web::error::ErrorBadRequest("Renewable percentage must be between 0 and 100"));
    }

    // Process the registration
    let result = node.register_renewable_energy(
        &req.pool_id,
        req.renewable_percentage,
        &req.provider,
        req.certificate.as_deref(),
        &req.contact_email,
    ).await.map_err(|e| {
        actix_web::error::ErrorInternalServerError(format!("Failed to register renewable energy: {}", e))
    })?;

    #[derive(Serialize)]
    struct RenewableEnergyResponse {
        registration_id: String,
        status: String,
        discount_rate: f64,
        verification_pending: bool,
    }

    Ok(HttpResponse::Ok().json(ApiResponse::success(RenewableEnergyResponse {
        registration_id: result.registration_id,
        status: result.status,
        discount_rate: result.discount_rate,
        verification_pending: result.verification_pending,
    })))
}

// Foundation and tokenomics APIs

/// Get foundation treasury information
pub async fn get_foundation_info(
    node: web::Data<Arc<NodeHandle>>,
) -> Result<HttpResponse, Error> {
    let info = node.get_foundation_info().await.map_err(|e| {
        actix_web::error::ErrorInternalServerError(format!("Failed to get foundation info: {}", e))
    })?;

    Ok(HttpResponse::Ok().json(ApiResponse::success(info)))
}

/// Get token allocation information
pub async fn get_token_allocation(
    node: web::Data<Arc<NodeHandle>>,
) -> Result<HttpResponse, Error> {
    let allocation = node.get_token_allocation().await.map_err(|e| {
        actix_web::error::ErrorInternalServerError(format!("Failed to get token allocation: {}", e))
    })?;

    Ok(HttpResponse::Ok().json(ApiResponse::success(allocation)))
}

// Register API routes
pub fn configure_routes(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/api/v1")
            // Blockchain routes
            .route("/status", web::get().to(get_node_status))
            .route("/blockchain", web::get().to(get_blockchain_info))
            .route("/block/height/{height}", web::get().to(get_block_by_height))
            .route("/block/hash/{hash}", web::get().to(get_block_by_hash))
            .route("/tx/{hash}", web::get().to(get_transaction))
            .route("/tx/submit", web::post().to(submit_transaction))
            
            // Mempool routes
            .route("/mempool", web::get().to(get_mempool_info))
            .route("/mempool/transactions", web::get().to(get_mempool_transactions))
            
            // Network routes
            .route("/peers", web::get().to(get_peer_info))
            .route("/network", web::get().to(get_network_info))
            
            // Mining routes
            .route("/mining", web::get().to(get_mining_info))
            .route("/mining/template", web::get().to(get_block_template))
            .route("/mining/submit", web::post().to(submit_block))
            
            // Environmental routes
            .route("/environmental/metrics", web::get().to(get_environmental_metrics))
            .route("/environmental/treasury", web::get().to(get_treasury_status))
            .route("/environmental/tx/{hash}", web::get().to(get_transaction_emissions))
            .route("/environmental/renewable", web::post().to(register_renewable_energy))
            
            // Foundation and tokenomics routes
            .route("/foundation", web::get().to(get_foundation_info))
            .route("/tokenomics", web::get().to(get_token_allocation))
    );
} 