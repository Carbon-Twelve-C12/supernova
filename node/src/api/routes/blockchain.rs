//! Blockchain API routes
//!
//! This module provides API endpoints for accessing blockchain data,
//! including blocks and transactions.

use actix_web::{web, HttpResponse, Responder};
use hex::FromHex;
use std::sync::Arc;
use utoipa::OpenApi;
use tracing::{info, warn, error, debug};
use std::time::{SystemTime, UNIX_EPOCH};
use serde::{Deserialize, Serialize};
use bincode;

use crate::node::Node;
use crate::api::error::{ApiError, ApiResult};
use crate::api::types::{
    ApiResponse, BlockInfo, TransactionInfo, BlockchainInfo, BlockHeightParams, BlockHashParams, TxHashParams, SubmitTxRequest,
    TransactionSubmissionResponse, TransactionInput, TransactionOutput, BlockchainStats,
};
use crate::storage::StorageError;
use btclib::types::transaction::{Transaction, TransactionError};

/// Configure blockchain routes
pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/blockchain")
            .route("/info", web::get().to(get_blockchain_info))
            .route("/block/{height}", web::get().to(get_block_by_height))
            .route("/block/hash/{hash}", web::get().to(get_block_by_hash))
            .route("/transaction/{txid}", web::get().to(get_transaction))
            .route("/submit", web::post().to(submit_transaction))
            .route("/stats", web::get().to(get_blockchain_stats)),
    );
}

/// Get blockchain information
///
/// Returns general information about the blockchain state.
#[utoipa::path(
    get,
    path = "/api/v1/blockchain/info",
    responses(
        (status = 200, description = "Blockchain information retrieved successfully", body = BlockchainInfo),
        (status = 500, description = "Internal server error", body = ApiError)
    )
)]
pub async fn get_blockchain_info(
    node: web::Data<Arc<Node>>,
) -> ApiResult<BlockchainInfo> {
    let storage = node.storage();
    let height = storage.get_height()
        .map_err(|e| ApiError::internal_error(format!("Failed to get height: {}", e)))?;
    
    let best_block_hash = if height > 0 {
        storage.get_block_hash_by_height(height)
            .map_err(|e| ApiError::internal_error(format!("Failed to get best block hash: {}", e)))?
            .unwrap_or([0u8; 32])
    } else {
        [0u8; 32]
    };
    
    let difficulty = 1.0; // TODO: Get actual difficulty
    let total_work = "0x1".to_string(); // TODO: Calculate total work
    
    let config = node.config();
    let network_id = config.read().unwrap().network.network_id.clone();
    
    let info = BlockchainInfo {
        height,
        best_block_hash: hex::encode(best_block_hash),
        difficulty,
        total_work,
        network: network_id,
        version: env!("CARGO_PKG_VERSION").to_string(),
    };
    
    Ok(info)
}

/// Get a block by height
///
/// Returns detailed information about a block at the specified height.
#[utoipa::path(
    get,
    path = "/api/v1/blockchain/block/{height}",
    params(
        ("height" = u64, Path, description = "Block height")
    ),
    responses(
        (status = 200, description = "Block retrieved successfully", body = BlockInfo),
        (status = 404, description = "Block not found", body = ApiError),
        (status = 500, description = "Internal server error", body = ApiError)
    )
)]
pub async fn get_block_by_height(
    path: web::Path<u64>,
    node: web::Data<Arc<Node>>,
) -> ApiResult<BlockInfo> {
    let height = path.into_inner();
    let storage = node.storage();
    
    let block_hash = storage.get_block_hash_by_height(height)
        .map_err(|e| ApiError::internal_error(format!("Failed to get block hash: {}", e)))?
        .ok_or_else(|| ApiError::not_found("Block not found"))?;
    
    let block = storage.get_block(&block_hash)
        .map_err(|e| ApiError::internal_error(format!("Failed to get block: {}", e)))?
        .ok_or_else(|| ApiError::not_found("Block not found"))?;
    
    let confirmations = node.chain_state().get_height().saturating_sub(height) + 1;
    
    let block_info = BlockInfo {
        hash: hex::encode(block_hash),
        height,
        confirmations,
        size: bincode::serialize(&block).unwrap_or_default().len() as u64,
        weight: 0, // TODO: Calculate weight
        version: block.version(),
        merkle_root: hex::encode(block.merkle_root()),
        time: block.timestamp(),
        nonce: block.nonce(),
        difficulty: 1.0, // TODO: Get actual difficulty
        previous_block_hash: hex::encode(block.prev_block_hash()),
        next_block_hash: None, // TODO: Get next block hash if exists
        transaction_count: block.transactions().len() as u32,
        transactions: block.transactions().iter().map(|tx| hex::encode(tx.hash())).collect(),
    };
    
    Ok(block_info)
}

/// Get a block by hash
///
/// Returns detailed information about a block with the specified hash.
#[utoipa::path(
    get,
    path = "/api/v1/blockchain/block/hash/{hash}",
    params(
        ("hash" = String, Path, description = "Block hash")
    ),
    responses(
        (status = 200, description = "Block retrieved successfully", body = BlockInfo),
        (status = 404, description = "Block not found", body = ApiError),
        (status = 500, description = "Internal server error", body = ApiError)
    )
)]
pub async fn get_block_by_hash(
    path: web::Path<String>,
    node: web::Data<Arc<Node>>,
) -> ApiResult<BlockInfo> {
    let hash_str = path.into_inner();
    let hash = hex::decode(&hash_str)
        .map_err(|_| ApiError::bad_request("Invalid block hash format"))?;
    
    if hash.len() != 32 {
        return Err(ApiError::bad_request("Invalid block hash length"));
    }
    
    let mut block_hash = [0u8; 32];
    block_hash.copy_from_slice(&hash);
    
    let storage = node.storage();
    let block = storage.get_block(&block_hash)
        .map_err(|e| ApiError::internal_error(format!("Failed to get block: {}", e)))?
        .ok_or_else(|| ApiError::not_found("Block not found"))?;
    
    let height = storage.get_block_height(&block_hash)
        .map_err(|e| ApiError::internal_error(format!("Failed to get block height: {}", e)))?
        .unwrap_or(0);
    
    let confirmations = node.chain_state().get_height().saturating_sub(height) + 1;
    
    let block_info = BlockInfo {
        hash: hash_str,
        height,
        confirmations,
        size: bincode::serialize(&block).unwrap_or_default().len() as u64,
        weight: 0, // TODO: Calculate weight
        version: block.version(),
        merkle_root: hex::encode(block.merkle_root()),
        time: block.timestamp(),
        nonce: block.nonce(),
        difficulty: 1.0, // TODO: Get actual difficulty
        previous_block_hash: hex::encode(block.prev_block_hash()),
        next_block_hash: None, // TODO: Get next block hash if exists
        transaction_count: block.transactions().len() as u32,
        transactions: block.transactions().iter().map(|tx| hex::encode(tx.hash())).collect(),
    };
    
    Ok(block_info)
}

/// Get a transaction by ID
///
/// Returns detailed information about a transaction.
#[utoipa::path(
    get,
    path = "/api/v1/blockchain/transaction/{txid}",
    params(
        ("txid" = String, Path, description = "Transaction ID")
    ),
    responses(
        (status = 200, description = "Transaction retrieved successfully", body = TransactionInfo),
        (status = 404, description = "Transaction not found", body = ApiError),
        (status = 500, description = "Internal server error", body = ApiError)
    )
)]
pub async fn get_transaction(
    path: web::Path<String>,
    node: web::Data<Arc<Node>>,
) -> ApiResult<TransactionInfo> {
    let txid_str = path.into_inner();
    let txid = hex::decode(&txid_str)
        .map_err(|_| ApiError::bad_request("Invalid transaction ID format"))?;
    
    if txid.len() != 32 {
        return Err(ApiError::bad_request("Invalid transaction ID length"));
    }
    
    let mut tx_hash = [0u8; 32];
    tx_hash.copy_from_slice(&txid);
    
    let storage = node.storage();
    
    // First check mempool
    if let Some(mempool_tx) = node.mempool().get_transaction(&tx_hash) {
        let tx_info = TransactionInfo {
            txid: txid_str.clone(),
            hash: txid_str,
            version: mempool_tx.version(),
            size: bincode::serialize(&mempool_tx).unwrap_or_default().len() as u64,
            vsize: 0, // TODO: Calculate virtual size
            weight: 0, // TODO: Calculate weight
            locktime: mempool_tx.lock_time(),
            inputs: mempool_tx.inputs().iter().map(|input| {
                serde_json::json!({
                    "txid": hex::encode(input.prev_tx_hash()),
                    "vout": input.prev_output_index(),
                    "script_sig": hex::encode(input.script_sig()),
                    "sequence": input.sequence()
                })
            }).collect(),
            outputs: mempool_tx.outputs().iter().enumerate().map(|(i, output)| {
                serde_json::json!({
                    "value": output.value(),
                    "n": i,
                    "script_pubkey": hex::encode(output.script_pubkey())
                })
            }).collect(),
            block_hash: None,
            block_height: None,
            confirmations: 0,
            time: None,
            block_time: None,
        };
        return Ok(tx_info);
    }
    
    // Check blockchain storage
    let tx = storage.get_transaction(&tx_hash)
        .map_err(|e| ApiError::internal_error(format!("Failed to get transaction: {}", e)))?
        .ok_or_else(|| ApiError::not_found("Transaction not found"))?;
    
    // Get block information if transaction is in a block
    let (block_hash, block_height, confirmations) = if let Some(block_hash) = storage.get_transaction_block(&tx_hash)
        .map_err(|e| ApiError::internal_error(format!("Failed to get transaction block: {}", e)))? {
        let block_height = storage.get_block_height(&block_hash)
            .map_err(|e| ApiError::internal_error(format!("Failed to get block height: {}", e)))?
            .unwrap_or(0);
        
        let block = storage.get_block(&block_hash)
            .map_err(|e| ApiError::internal_error(format!("Failed to get block: {}", e)))?
            .ok_or_else(|| ApiError::not_found("Block not found"))?;
        
        let confirmations = node.chain_state().get_height().saturating_sub(block_height) + 1;
        
        (Some(hex::encode(block_hash)), Some(block_height), confirmations)
    } else {
        (None, None, 0)
    };
    
    // Get input and output information
    let inputs: Vec<serde_json::Value> = tx.inputs().iter().map(|input| {
        let prev_output = storage.get_transaction_output(&input.prev_tx_hash(), input.prev_output_index())
            .ok().flatten();
        
        serde_json::json!({
            "txid": hex::encode(input.prev_tx_hash()),
            "vout": input.prev_output_index(),
            "script_sig": hex::encode(input.script_sig()),
            "sequence": input.sequence(),
            "prev_output": prev_output.map(|data| hex::encode(data))
        })
    }).collect();
    
    let outputs: Vec<serde_json::Value> = tx.outputs().iter().enumerate().map(|(i, output)| {
        let spent_info = storage.get_transaction_output(&tx_hash, i as u32)
            .ok().flatten();
        let is_spent = spent_info.is_none();
        let spent_by_tx = if is_spent {
            storage.is_output_spent(&tx_hash, i as u32)
                .ok().flatten().map(|hash| hex::encode(hash))
        } else {
            None
        };
        
        serde_json::json!({
            "value": output.value(),
            "n": i,
            "script_pubkey": hex::encode(output.script_pubkey()),
            "spent": is_spent,
            "spent_by": spent_by_tx
        })
    }).collect();
    
    let tx_info = TransactionInfo {
        txid: txid_str.clone(),
        hash: hex::encode(tx.hash()),
        version: tx.version(),
        size: bincode::serialize(&tx).unwrap_or_default().len() as u64,
        vsize: 0, // TODO: Calculate virtual size
        weight: 0, // TODO: Calculate weight
        locktime: tx.lock_time(),
        inputs,
        outputs,
        block_hash,
        block_height,
        confirmations,
        time: block_height.map(|_| 0), // TODO: Get actual block time
        block_time: block_height.map(|_| 0), // TODO: Get actual block time
    };
    
    Ok(tx_info)
}

/// Submit a transaction to the blockchain
///
/// Submits a new transaction to the mempool for validation and broadcasting.
#[utoipa::path(
    post,
    path = "/api/v1/blockchain/submit",
    request_body = SubmitTxRequest,
    responses(
        (status = 200, description = "Transaction submitted successfully", body = TransactionSubmissionResponse),
        (status = 400, description = "Invalid transaction", body = ApiError),
        (status = 500, description = "Internal server error", body = ApiError)
    )
)]
pub async fn submit_transaction(
    request: web::Json<SubmitTxRequest>,
    node: web::Data<Arc<Node>>,
) -> ApiResult<TransactionSubmissionResponse> {
    // Parse the raw transaction
    let tx_data = hex::decode(&request.raw_tx)
        .map_err(|_| ApiError::bad_request("Invalid transaction format"))?;
    
    // Deserialize the transaction
    let tx = bincode::deserialize::<btclib::types::transaction::Transaction>(&tx_data)
        .map_err(|_| ApiError::bad_request("Invalid transaction format"))?;
    
    let txid = hex::encode(tx.hash());
    
    // Add to mempool with default fee rate
    match node.mempool().add_transaction(tx.clone(), 1000) {
        Ok(()) => {
            // Broadcast to network
            node.broadcast_transaction(&tx);
            
            Ok(TransactionSubmissionResponse {
                txid: Some(txid),
                accepted: true,
                error: None,
            })
        },
        Err(e) => {
            match e {
                crate::mempool::MempoolError::TransactionExists(_) => {
                    Err(ApiError::bad_request("Transaction already exists in mempool"))
                },
                crate::mempool::MempoolError::InvalidTransaction(msg) => {
                    Err(ApiError::bad_request(format!("Invalid transaction: {}", msg)))
                },
                crate::mempool::MempoolError::FeeTooLow { .. } => {
                    Err(ApiError::bad_request("Insufficient transaction fee"))
                },
                _ => {
                    Err(ApiError::internal_error(format!("Failed to add transaction to mempool: {}", e)))
                }
            }
        }
    }
}

/// Get blockchain statistics
///
/// Returns statistical information about the blockchain.
#[utoipa::path(
    get,
    path = "/api/v1/blockchain/stats",
    responses(
        (status = 200, description = "Blockchain statistics retrieved successfully", body = BlockchainStats),
        (status = 500, description = "Internal server error", body = ApiError)
    )
)]
pub async fn get_blockchain_stats(
    node: web::Data<Arc<Node>>,
) -> ApiResult<BlockchainStats> {
    let storage = node.storage();
    let height = storage.get_height()
        .map_err(|e| ApiError::internal_error(format!("Failed to get height: {}", e)))?;
    
    let stats = BlockchainStats {
        height,
        total_transactions: 0, // TODO: Count total transactions
        total_blocks: height + 1,
        difficulty: 1.0, // TODO: Get actual difficulty
        hashrate: 0, // TODO: Calculate network hashrate
        mempool_size: node.mempool().size(),
        mempool_bytes: node.mempool().size_in_bytes(),
        utxo_set_size: 0, // TODO: Count UTXO set size
        chain_size_bytes: 0, // TODO: Calculate chain size
    };
    
    Ok(stats)
} 