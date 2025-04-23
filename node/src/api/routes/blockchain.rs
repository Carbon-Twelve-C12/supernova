//! Blockchain API routes
//!
//! This module provides API endpoints for accessing blockchain data,
//! including blocks and transactions.

use actix_web::{web, HttpResponse, Responder};
use hex::FromHex;
use std::sync::Arc;
use utoipa::OpenApi;
use tracing::{info, warn, error, debug};

use crate::node::Node;
use crate::api::error::ApiError;
use crate::api::types::{ApiResponse, BlockInfo, TransactionInfo, BlockchainInfo, BlockHeightParams, BlockHashParams, TxHashParams, SubmitTxRequest};
use crate::storage::StorageError;
use btclib::types::transaction::{Transaction, TransactionError};

/// Configure blockchain routes
pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/blockchain")
            .route("/info", web::get().to(get_blockchain_info))
            .route("/block/height/{height}", web::get().to(get_block_by_height))
            .route("/block/hash/{hash}", web::get().to(get_block_by_hash))
            .route("/tx/{txid}", web::get().to(get_transaction))
            .route("/tx", web::post().to(submit_transaction)),
    );
}

/// Get blockchain information
///
/// Returns general information about the current state of the blockchain.
#[utoipa::path(
    get,
    path = "/api/v1/blockchain/info",
    tag = "blockchain",
    responses(
        (status = 200, description = "Blockchain information retrieved successfully", body = BlockchainInfo),
        (status = 503, description = "Node is syncing"),
        (status = 500, description = "Internal server error"),
    )
)]
pub async fn get_blockchain_info(node: web::Data<Arc<Node>>) -> Result<impl Responder, ApiError> {
    let chain_state = node.chain_state();
    let storage = node.storage();
    
    // Get basic blockchain information
    let height = chain_state.get_height();
    let best_block_hash = chain_state.get_best_block_hash();
    let is_syncing = node.is_syncing();
    let sync_progress = node.sync_progress();
    
    // Get more details from storage
    let total_difficulty = chain_state.get_total_difficulty();
    let chainwork = format!("{:x}", total_difficulty);
    
    // Calculate network hashrate (simplified, would be more complex in production)
    let network_hashrate = node.get_network_hashrate();
    
    // Get median time from recent blocks
    let median_time = node.get_median_time();
    
    // Get estimated chain size on disk
    let size_on_disk = storage.get_chain_size()?;
    
    let info = BlockchainInfo {
        height,
        best_block_hash: hex::encode(best_block_hash),
        difficulty: node.get_current_difficulty(),
        median_time,
        chain_work: chainwork,
        verification_progress: sync_progress,
        size_on_disk,
        network_hashrate,
        is_synced: !is_syncing,
        sync_progress,
    };
    
    Ok(HttpResponse::Ok().json(ApiResponse::success(info)))
}

/// Get block by height
///
/// Returns detailed information about a block at the specified height.
#[utoipa::path(
    get,
    path = "/api/v1/blockchain/block/height/{height}",
    tag = "blockchain",
    params(
        ("height" = u64, Path, description = "Block height"),
    ),
    responses(
        (status = 200, description = "Block information retrieved successfully", body = BlockInfo),
        (status = 404, description = "Block not found"),
        (status = 500, description = "Internal server error"),
    )
)]
pub async fn get_block_by_height(
    node: web::Data<Arc<Node>>,
    path: web::Path<BlockHeightParams>,
) -> Result<impl Responder, ApiError> {
    let height = path.height;
    let storage = node.storage();
    
    // Get block hash by height
    let block_hash = storage.get_block_hash_by_height(height)?
        .ok_or_else(|| ApiError::NotFound(format!("Block at height {} not found", height)))?;
    
    // Get block by hash
    let block = storage.get_block(&block_hash)?
        .ok_or_else(|| ApiError::NotFound(format!("Block with hash {} not found", hex::encode(block_hash))))?;
    
    // Convert to BlockInfo
    let confirmations = node.chain_state().get_height().saturating_sub(height) + 1;
    let total_fees = block.calculate_total_fees()?;
    
    let block_info = BlockInfo {
        hash: hex::encode(block.hash()),
        height,
        prev_hash: hex::encode(block.prev_block_hash()),
        merkle_root: hex::encode(block.merkle_root()),
        timestamp: block.timestamp(),
        version: block.version(),
        target: block.target(),
        nonce: block.nonce(),
        tx_count: block.transactions().len(),
        size: block.size(),
        weight: block.weight(),
        fees: total_fees,
        confirmed: true,
        confirmations,
    };
    
    Ok(HttpResponse::Ok().json(ApiResponse::success(block_info)))
}

/// Get block by hash
///
/// Returns detailed information about a block with the specified hash.
#[utoipa::path(
    get,
    path = "/api/v1/blockchain/block/hash/{hash}",
    tag = "blockchain",
    params(
        ("hash" = String, Path, description = "Block hash (hex encoded)"),
    ),
    responses(
        (status = 200, description = "Block information retrieved successfully", body = BlockInfo),
        (status = 400, description = "Invalid block hash format"),
        (status = 404, description = "Block not found"),
        (status = 500, description = "Internal server error"),
    )
)]
pub async fn get_block_by_hash(
    node: web::Data<Arc<Node>>,
    path: web::Path<BlockHashParams>,
) -> Result<impl Responder, ApiError> {
    let hash_hex = &path.hash;
    
    // Parse hex hash
    let hash: [u8; 32] = Vec::from_hex(hash_hex)
        .map_err(|_| ApiError::BadRequest(format!("Invalid block hash format: {}", hash_hex)))?
        .try_into()
        .map_err(|_| ApiError::BadRequest(format!("Invalid block hash length: {}", hash_hex)))?;
    
    let storage = node.storage();
    
    // Get block by hash
    let block = storage.get_block(&hash)?
        .ok_or_else(|| ApiError::NotFound(format!("Block with hash {} not found", hash_hex)))?;
    
    // Get height for this block
    let height = storage.get_block_height(&hash)?
        .ok_or_else(|| ApiError::NotFound(format!("Block height for hash {} not found", hash_hex)))?;
    
    // Convert to BlockInfo
    let confirmations = node.chain_state().get_height().saturating_sub(height) + 1;
    let total_fees = block.calculate_total_fees()?;
    
    let block_info = BlockInfo {
        hash: hex::encode(block.hash()),
        height,
        prev_hash: hex::encode(block.prev_block_hash()),
        merkle_root: hex::encode(block.merkle_root()),
        timestamp: block.timestamp(),
        version: block.version(),
        target: block.target(),
        nonce: block.nonce(),
        tx_count: block.transactions().len(),
        size: block.size(),
        weight: block.weight(),
        fees: total_fees,
        confirmed: true,
        confirmations,
    };
    
    Ok(HttpResponse::Ok().json(ApiResponse::success(block_info)))
}

/// Get transaction
///
/// Returns detailed information about a transaction with the specified hash.
#[utoipa::path(
    get,
    path = "/api/v1/blockchain/tx/{txid}",
    tag = "blockchain",
    params(
        ("txid" = String, Path, description = "Transaction ID (hex encoded hash)"),
    ),
    responses(
        (status = 200, description = "Transaction information retrieved successfully", body = TransactionInfo),
        (status = 400, description = "Invalid transaction hash format"),
        (status = 404, description = "Transaction not found"),
        (status = 500, description = "Internal server error"),
    )
)]
pub async fn get_transaction(
    node: web::Data<Arc<Node>>,
    path: web::Path<TxHashParams>,
) -> Result<impl Responder, ApiError> {
    let txid_hex = &path.txid;
    
    // Parse hex hash
    let txid: [u8; 32] = Vec::from_hex(txid_hex)
        .map_err(|_| ApiError::BadRequest(format!("Invalid transaction hash format: {}", txid_hex)))?
        .try_into()
        .map_err(|_| ApiError::BadRequest(format!("Invalid transaction hash length: {}", txid_hex)))?;
    
    let storage = node.storage();
    
    // Check mempool first
    let tx = match node.mempool().get_transaction(&txid) {
        Some(tx) => {
            // Transaction is in mempool (unconfirmed)
            tx
        },
        None => {
            // Check in blockchain storage
            storage.get_transaction(&txid)?
                .ok_or_else(|| ApiError::NotFound(format!("Transaction with hash {} not found", txid_hex)))?
        }
    };
    
    // Get block information if transaction is confirmed
    let (block_hash, block_height, confirmed_time, confirmations) = if let Some(block_hash) = storage.get_transaction_block(&txid)? {
        let block_height = storage.get_block_height(&block_hash)?
            .ok_or_else(|| ApiError::InternalError("Block height not found for transaction block".to_string()))?;
        
        let block = storage.get_block(&block_hash)?
            .ok_or_else(|| ApiError::InternalError("Block not found for transaction".to_string()))?;
            
        let confirmations = node.chain_state().get_height().saturating_sub(block_height) + 1;
        
        (Some(hex::encode(block_hash)), Some(block_height), Some(block.timestamp()), confirmations)
    } else {
        (None, None, None, 0)
    };
    
    // Calculate fee
    let fee = tx.calculate_fee(&|outpoint| {
        storage.get_transaction_output(&outpoint.txid, outpoint.vout).ok().flatten()
    })?;
    
    // Convert inputs and outputs to API format
    let inputs = tx.inputs().iter().map(|input| {
        let prev_output = storage.get_transaction_output(&input.outpoint().txid, input.outpoint().vout)
            .ok()
            .flatten();
        
        let value = prev_output.as_ref().map(|o| o.value()).unwrap_or(0);
        let address = prev_output.as_ref().and_then(|o| o.extract_address()).map(|a| a.to_string());
        
        crate::api::types::TransactionInput {
            txid: hex::encode(input.outpoint().txid),
            vout: input.outpoint().vout,
            script_sig: hex::encode(input.script_sig()),
            script_sig_asm: input.script_sig_asm(),
            witness: if input.witness().is_empty() {
                None
            } else {
                Some(input.witness().iter().map(hex::encode).collect())
            },
            sequence: input.sequence(),
            value,
            address,
        }
    }).collect();
    
    let outputs = tx.outputs().iter().enumerate().map(|(i, output)| {
        // Check if this output has been spent
        let (spent, spent_by_tx) = match storage.is_output_spent(&txid, i as u32)? {
            Some(spending_tx) => (Some(true), Some(hex::encode(spending_tx))),
            None => (Some(false), None),
        };
        
        crate::api::types::TransactionOutput {
            value: output.value(),
            script_pub_key: hex::encode(output.script_pubkey()),
            script_pub_key_asm: output.script_pubkey_asm(),
            script_type: output.script_type().to_string(),
            address: output.extract_address().map(|a| a.to_string()),
            spent,
            spent_by_tx,
        }
    }).collect::<Result<Vec<_>, _>>()?;
    
    // Calculate fee rate
    let fee_rate = if tx.size() > 0 {
        fee as f64 / tx.size() as f64
    } else {
        0.0
    };
    
    // Fetch environmental data if available
    let estimated_emissions = node.environmental_manager().map(|em| {
        em.calculate_transaction_emissions(&tx).ok()
    }).flatten();
    
    let tx_info = TransactionInfo {
        txid: hex::encode(tx.hash()),
        version: tx.version(),
        size: tx.size(),
        weight: tx.weight(),
        locktime: tx.locktime(),
        block_hash,
        block_height,
        inputs,
        outputs,
        fee,
        fee_rate,
        confirmations,
        confirmed_time,
        estimated_emissions,
    };
    
    Ok(HttpResponse::Ok().json(ApiResponse::success(tx_info)))
}

/// Submit transaction
///
/// Submits a new transaction to the network.
#[utoipa::path(
    post,
    path = "/api/v1/blockchain/tx",
    tag = "blockchain",
    request_body = SubmitTxRequest,
    responses(
        (status = 200, description = "Transaction submitted successfully"),
        (status = 400, description = "Invalid transaction format"),
        (status = 409, description = "Transaction validation failed"),
        (status = 500, description = "Internal server error"),
    )
)]
pub async fn submit_transaction(
    node: web::Data<Arc<Node>>,
    request: web::Json<SubmitTxRequest>,
) -> Result<impl Responder, ApiError> {
    let tx_data = Vec::from_hex(&request.tx_data)
        .map_err(|_| ApiError::BadRequest("Invalid transaction hex format".to_string()))?;
    
    // Deserialize transaction
    let tx: Transaction = bincode::deserialize(&tx_data)
        .map_err(|e| ApiError::BadRequest(format!("Invalid transaction format: {}", e)))?;
    
    // Validate and add to mempool
    match node.mempool().add_transaction(tx.clone()) {
        Ok(_) => {
            // Broadcast the transaction to peers
            node.broadcast_transaction(&tx);
            
            // Return success with txid
            Ok(HttpResponse::Ok().json(ApiResponse::success(serde_json::json!({
                "txid": hex::encode(tx.hash()),
                "status": "accepted",
            }))))
        },
        Err(e) => {
            match e {
                TransactionError::InvalidTransaction(msg) => {
                    Err(ApiError::BadRequest(format!("Transaction validation failed: {}", msg)))
                },
                TransactionError::InsufficientFunds => {
                    Err(ApiError::BadRequest("Insufficient funds".to_string()))
                },
                TransactionError::DuplicateTransaction => {
                    Err(ApiError::BadRequest("Transaction already in mempool".to_string()))
                },
                err => Err(ApiError::InternalError(format!("Failed to add transaction: {}", err))),
            }
        }
    }
} 