//! JSON-RPC method handlers
//!
//! This module implements handlers for JSON-RPC methods.

use std::sync::Arc;
use actix_web::web;
use serde_json::{Value, json};
use crate::node::Node;
use super::types::{JsonRpcError, ErrorCode};

/// Dispatch method to appropriate handler
pub async fn dispatch(
    method: &str,
    params: Value,
    node: web::Data<Arc<Node>>,
) -> Result<Value, JsonRpcError> {
    match method {
        // General info method
        "getinfo" => get_info(params, node).await,

        // Blockchain methods
        "getblockchaininfo" => get_blockchain_info(params, node).await,
        "getblock" => get_block(params, node).await,
        "getblockhash" => get_block_hash(params, node).await,
        "getbestblockhash" => get_best_block_hash(params, node).await,
        "getblockcount" => get_block_count(params, node).await,
        "getdifficulty" => get_difficulty(params, node).await,

        // Transaction methods
        "gettransaction" => get_transaction(params, node).await,
        "getrawtransaction" => get_raw_transaction(params, node).await,
        "sendrawtransaction" => send_raw_transaction(params, node).await,

        // Mempool methods
        "getmempoolinfo" => get_mempool_info(params, node).await,
        "getrawmempool" => get_raw_mempool(params, node).await,

        // Network methods
        "getnetworkinfo" => get_network_info(params, node).await,
        "getpeerinfo" => get_peer_info(params, node).await,

        // Mining methods
        "getmininginfo" => get_mining_info(params, node).await,
        "getblocktemplate" => get_block_template(params, node).await,
        "submitblock" => submit_block(params, node).await,

        // Method not found
        _ => Err(JsonRpcError {
            code: ErrorCode::MethodNotFound as i32,
            message: format!("Method '{}' not found", method),
            data: None,
        }),
    }
}

/// Get general node information
async fn get_info(
    _params: Value,
    node: web::Data<Arc<Node>>,
) -> Result<Value, JsonRpcError> {
    // Get various info from different subsystems
    let blockchain_info = node.get_blockchain_info().await.map_err(|e| JsonRpcError {
        code: ErrorCode::BlockchainError as i32,
        message: format!("Failed to get blockchain info: {}", e),
        data: None,
    })?;

    let network_info = node.get_network_info().await.map_err(|e| JsonRpcError {
        code: ErrorCode::NetworkError as i32,
        message: format!("Failed to get network info: {}", e),
        data: None,
    })?;

    let mempool_info = node.get_mempool_info().await.map_err(|e| JsonRpcError {
        code: ErrorCode::ServerError as i32,
        message: format!("Failed to get mempool info: {}", e),
        data: None,
    })?;

    // Combine into a single response
    Ok(json!({
        "version": env!("CARGO_PKG_VERSION"),
        "protocolversion": 70015,
        "blocks": blockchain_info.height,
        "headers": blockchain_info.height,
        "bestblockhash": blockchain_info.best_block_hash,
        "difficulty": blockchain_info.difficulty,
        "chainwork": blockchain_info.chain_work,
        "verificationprogress": blockchain_info.verification_progress,
        "chain": node.config().read()
            .map_err(|e| JsonRpcError {
                code: ErrorCode::InternalError as i32,
                message: format!("Failed to read config: {}", e),
                data: None,
            })?
            .node.network_name.clone(),
        "warnings": "",
        "networkhashps": calculate_network_hashrate(blockchain_info.difficulty),
        "connections": network_info.connections,
        "mempool": {
            "size": mempool_info.tx_count,
            "bytes": mempool_info.size,
            "usage": mempool_info.memory_usage,
            "minfee": mempool_info.min_fee_rate / 100000000.0, // Convert to NOVA/kB
        },
        "time": std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
    }))
}

// Extract parameters from value with proper error handling
fn extract_params<T: serde::de::DeserializeOwned>(params: Value) -> Result<T, JsonRpcError> {
    serde_json::from_value(params).map_err(|e| JsonRpcError {
        code: ErrorCode::InvalidParams as i32,
        message: format!("Invalid parameters: {}", e),
        data: None,
    })
}

/// Get blockchain information
async fn get_blockchain_info(
    _params: Value,
    node: web::Data<Arc<Node>>,
) -> Result<Value, JsonRpcError> {
    let info = node.get_blockchain_info().await.map_err(|e| JsonRpcError {
        code: ErrorCode::BlockchainError as i32,
        message: format!("Failed to get blockchain info: {}", e),
        data: None,
    })?;

    Ok(json!({
        "chain": node.config().read()
            .map_err(|e| JsonRpcError {
                code: ErrorCode::InternalError as i32,
                message: format!("Failed to read config: {}", e),
                data: None,
            })?
            .node.network_name.clone(),
        "blocks": info.height,
        "headers": info.height,
        "bestblockhash": info.best_block_hash,
        "difficulty": info.difficulty,
        "mediantime": info.median_time,
        "verificationprogress": info.verification_progress,
        "pruned": false,
        "chainwork": info.chain_work,
        "size_on_disk": info.size_on_disk,
    }))
}

/// Get block by hash
async fn get_block(
    params: Value,
    node: web::Data<Arc<Node>>,
) -> Result<Value, JsonRpcError> {
    // Extract parameters
    #[derive(serde::Deserialize)]
    struct Params {
        blockhash: String,
        #[serde(default = "default_verbosity")]
        verbosity: u8,
    }

    fn default_verbosity() -> u8 { 1 }

    let params: Params = extract_params(params)?;

    // Parse hash from hex
    let hash_bytes = hex::decode(&params.blockhash).map_err(|_| JsonRpcError {
        code: ErrorCode::InvalidParams as i32,
        message: "Invalid block hash format".to_string(),
        data: None,
    })?;

    let mut hash = [0u8; 32];
    if hash_bytes.len() != 32 {
        return Err(JsonRpcError {
            code: ErrorCode::InvalidParams as i32,
            message: "Invalid block hash length".to_string(),
            data: None,
        });
    }
    hash.copy_from_slice(&hash_bytes);

    // Get block
    let block = node.get_block_by_hash(&hash).await.map_err(|e| JsonRpcError {
        code: ErrorCode::BlockchainError as i32,
        message: format!("Failed to get block: {}", e),
        data: None,
    })?;

    let block = match block {
        Some(block) => block,
        None => return Err(JsonRpcError {
            code: ErrorCode::BlockchainError as i32,
            message: format!("Block not found: {}", params.blockhash),
            data: None,
        }),
    };

    // Format response based on verbosity
    match params.verbosity {
        0 => {
            // Return hex-encoded serialized block
            let serialized = bincode::serialize(&block).map_err(|e| JsonRpcError {
                code: ErrorCode::InternalError as i32,
                message: format!("Failed to serialize block: {}", e),
                data: None,
            })?;

            Ok(json!(hex::encode(serialized)))
        },
        1 | 2 => {
            // Format block as JSON
            let mut txids = Vec::with_capacity(block.transactions.len());
            let mut txs = Vec::with_capacity(if params.verbosity == 2 { block.transactions.len() } else { 0 });

            for tx in &block.transactions {
                let txid = tx.hash();
                txids.push(hex::encode(txid));

                if params.verbosity == 2 {
                    // Full transaction details for verbosity 2
                    txs.push(format_transaction(tx));
                }
            }

            let current_height = node.get_blockchain_info().await
                .map(|info| info.height)
                .unwrap_or(0);
            let confirmations = current_height.saturating_sub(block.height) + 1;

            let mut result = json!({
                "hash": hex::encode(block.hash()),
                "confirmations": confirmations,
                "size": block.size(),
                "height": block.height,
                "version": block.version,
                "merkleroot": hex::encode(block.merkle_root),
                "tx": if params.verbosity == 2 { Value::Array(txs) } else { Value::Array(txids.into_iter().map(Value::String).collect()) },
                "time": block.timestamp,
                "nonce": block.nonce,
                "bits": format!("{:x}", block.target),
                "difficulty": calculate_difficulty(block.target),
                "previousblockhash": hex::encode(block.prev_hash),
            });

            if block.height > 0 {
                if let Some(next_block_hash) = get_next_block_hash(&block.hash(), node.clone()).await? {
                    result["nextblockhash"] = Value::String(hex::encode(next_block_hash));
                }
            }

            Ok(result)
        },
        _ => Err(JsonRpcError {
            code: ErrorCode::InvalidParams as i32,
            message: "Invalid verbosity parameter (must be 0, 1, or 2)".to_string(),
            data: None,
        }),
    }
}

/// Get block hash by height
async fn get_block_hash(
    params: Value,
    node: web::Data<Arc<Node>>,
) -> Result<Value, JsonRpcError> {
    // Extract height parameter
    let height = match params {
        Value::Array(arr) if !arr.is_empty() => {
            match &arr[0] {
                Value::Number(n) => {
                    n.as_u64().ok_or_else(|| JsonRpcError {
                        code: ErrorCode::InvalidParams as i32,
                        message: "Invalid height parameter (must be a non-negative integer)".to_string(),
                        data: None,
                    })?
                },
                _ => return Err(JsonRpcError {
                    code: ErrorCode::InvalidParams as i32,
                    message: "Invalid height parameter (must be a number)".to_string(),
                    data: None,
                }),
            }
        },
        _ => return Err(JsonRpcError {
            code: ErrorCode::InvalidParams as i32,
            message: "Missing height parameter".to_string(),
            data: None,
        }),
    };

    // Get block hash
    let hash = node.get_block_hash_by_height(height).await.map_err(|e| JsonRpcError {
        code: ErrorCode::BlockchainError as i32,
        message: format!("Failed to get block hash: {}", e),
        data: None,
    })?;

    match hash {
        Some(hash) => Ok(Value::String(hex::encode(hash))),
        None => Err(JsonRpcError {
            code: ErrorCode::BlockchainError as i32,
            message: format!("Block at height {} not found", height),
            data: None,
        }),
    }
}

/// Get the hash of the best (tip) block
async fn get_best_block_hash(
    _params: Value,
    node: web::Data<Arc<Node>>,
) -> Result<Value, JsonRpcError> {
    let info = node.get_blockchain_info().await.map_err(|e| JsonRpcError {
        code: ErrorCode::BlockchainError as i32,
        message: format!("Failed to get blockchain info: {}", e),
        data: None,
    })?;

    Ok(Value::String(info.best_block_hash))
}

/// Get the current block count
async fn get_block_count(
    _params: Value,
    node: web::Data<Arc<Node>>,
) -> Result<Value, JsonRpcError> {
    let info = node.get_blockchain_info().await.map_err(|e| JsonRpcError {
        code: ErrorCode::BlockchainError as i32,
        message: format!("Failed to get blockchain info: {}", e),
        data: None,
    })?;

    Ok(Value::Number(serde_json::Number::from(info.height)))
}

/// Get the proof-of-work difficulty
async fn get_difficulty(
    _params: Value,
    node: web::Data<Arc<Node>>,
) -> Result<Value, JsonRpcError> {
    let info = node.get_blockchain_info().await.map_err(|e| JsonRpcError {
        code: ErrorCode::BlockchainError as i32,
        message: format!("Failed to get blockchain info: {}", e),
        data: None,
    })?;

    Ok(Value::Number(serde_json::Number::from_f64(info.difficulty).unwrap_or_default()))
}

/// Get transaction information
async fn get_transaction(
    params: Value,
    node: web::Data<Arc<Node>>,
) -> Result<Value, JsonRpcError> {
    // Implement transaction retrieval here
    // This is a placeholder implementation
    Err(JsonRpcError {
        code: ErrorCode::MethodNotFound as i32,
        message: "Method not yet implemented".to_string(),
        data: None,
    })
}

/// Get raw transaction data
async fn get_raw_transaction(
    params: Value,
    node: web::Data<Arc<Node>>,
) -> Result<Value, JsonRpcError> {
    // Implement raw transaction retrieval here
    // This is a placeholder implementation
    Err(JsonRpcError {
        code: ErrorCode::MethodNotFound as i32,
        message: "Method not yet implemented".to_string(),
        data: None,
    })
}

/// Send raw transaction
async fn send_raw_transaction(
    params: Value,
    node: web::Data<Arc<Node>>,
) -> Result<Value, JsonRpcError> {
    // Implement send raw transaction here
    // This is a placeholder implementation
    Err(JsonRpcError {
        code: ErrorCode::MethodNotFound as i32,
        message: "Method not yet implemented".to_string(),
        data: None,
    })
}

/// Get mempool information
async fn get_mempool_info(
    _params: Value,
    node: web::Data<Arc<Node>>,
) -> Result<Value, JsonRpcError> {
    let info = node.get_mempool_info().await.map_err(|e| JsonRpcError {
        code: ErrorCode::ServerError as i32,
        message: format!("Failed to get mempool info: {}", e),
        data: None,
    })?;

    Ok(json!({
        "size": info.tx_count,
        "bytes": info.size,
        "usage": info.memory_usage,
        "maxmempool": node.config().read()
            .map_err(|e| JsonRpcError {
                code: ErrorCode::InternalError as i32,
                message: format!("Failed to read config: {}", e),
                data: None,
            })?
            .mempool.max_mempool_size * 1024 * 1024, // Convert MB to bytes
        "mempoolminfee": info.min_fee_rate / 100000000.0, // Convert to NOVA/kB
        "minrelaytxfee": info.min_fee_rate / 100000000.0, // Convert to NOVA/kB
    }))
}

/// Get raw mempool transactions
async fn get_raw_mempool(
    params: Value,
    node: web::Data<Arc<Node>>,
) -> Result<Value, JsonRpcError> {
    // Extract verbose parameter
    let verbose = match params {
        Value::Array(arr) if !arr.is_empty() => {
            match &arr[0] {
                Value::Bool(b) => *b,
                _ => false,
            }
        },
        _ => false,
    };

    let txs = node.get_mempool_transactions().await.map_err(|e| JsonRpcError {
        code: ErrorCode::ServerError as i32,
        message: format!("Failed to get mempool transactions: {}", e),
        data: None,
    })?;

    if verbose {
        let mut result = serde_json::Map::new();

        for tx in txs {
            let txid = hex::encode(tx.hash());
            let tx_info = json!({
                "size": tx.size(),
                "fee": tx.fee().unwrap_or(0) as f64 / 100000000.0, // Convert satoshis to NOVA
                "time": tx.timestamp(),
                "height": 0, // Not yet in a block
                "descendantcount": 1, // Placeholder
                "descendantsize": tx.size(),
                "descendantfees": tx.fee().unwrap_or(0),
                "ancestorcount": 1, // Placeholder
                "ancestorsize": tx.size(),
                "ancestorfees": tx.fee().unwrap_or(0),
            });

            result.insert(txid, tx_info);
        }

        Ok(Value::Object(result))
    } else {
        let txids: Vec<Value> = txs.iter()
            .map(|tx| Value::String(hex::encode(tx.hash())))
            .collect();

        Ok(Value::Array(txids))
    }
}

/// Get network information
async fn get_network_info(
    _params: Value,
    node: web::Data<Arc<Node>>,
) -> Result<Value, JsonRpcError> {
    let info = node.get_network_info().await.map_err(|e| JsonRpcError {
        code: ErrorCode::NetworkError as i32,
        message: format!("Failed to get network info: {}", e),
        data: None,
    })?;

    Ok(json!({
        "version": env!("CARGO_PKG_VERSION").replace(".", ""),
        "subversion": format!("/supernova:{}/", env!("CARGO_PKG_VERSION")),
        "protocolversion": 70015, // Protocol version
        "localservices": "000000000000000d",
        "localrelay": true,
        "timeoffset": 0,
        "connections": info.connections,
        "networks": [
            {
                "name": "ipv4",
                "limited": false,
                "reachable": true,
                "proxy": "",
                "proxy_randomize_credentials": false
            },
            {
                "name": "ipv6",
                "limited": false,
                "reachable": true,
                "proxy": "",
                "proxy_randomize_credentials": false
            }
        ],
        "relayfee": 0.00001000, // Minimum relay fee in NOVA/kB
        "incrementalfee": 0.00001000, // Incremental fee in NOVA/kB
        "localaddresses": info.local_addresses.iter().map(|addr| json!({
            "address": addr.address,
            "port": addr.port,
            "score": addr.score
        })).collect::<Vec<Value>>(),
        "warnings": ""
    }))
}

/// Get peer information
async fn get_peer_info(
    _params: Value,
    node: web::Data<Arc<Node>>,
) -> Result<Value, JsonRpcError> {
    let peers = node.get_peer_info().await.map_err(|e| JsonRpcError {
        code: ErrorCode::NetworkError as i32,
        message: format!("Failed to get peer info: {}", e),
        data: None,
    })?;

    let peers_json: Vec<Value> = peers.iter().map(|peer| {
        json!({
            "id": peer.id,
            "addr": peer.address,
            "services": "000000000000000d", // Placeholder
            "lastsend": peer.last_send,
            "lastrecv": peer.last_recv,
            "bytessent": peer.bytes_sent,
            "bytesrecv": peer.bytes_received,
            "conntime": peer.connected_time,
            "pingtime": peer.ping_time.unwrap_or(0.0),
            "version": peer.version,
            "subver": peer.user_agent,
            "inbound": peer.direction == "inbound",
            "startingheight": peer.height,
            "banscore": 0, // Placeholder
            "synced_headers": peer.height,
            "synced_blocks": peer.height
        })
    }).collect();

    Ok(Value::Array(peers_json))
}

/// Get mining information
async fn get_mining_info(
    _params: Value,
    node: web::Data<Arc<Node>>,
) -> Result<Value, JsonRpcError> {
    let info = node.get_mining_info().await.map_err(|e| JsonRpcError {
        code: ErrorCode::ServerError as i32,
        message: format!("Failed to get mining info: {}", e),
        data: None,
    })?;

    let blockchain_info = node.get_blockchain_info().await.map_err(|e| JsonRpcError {
        code: ErrorCode::BlockchainError as i32,
        message: format!("Failed to get blockchain info: {}", e),
        data: None,
    })?;

    Ok(json!({
        "blocks": blockchain_info.height,
        "currentblockweight": 4000, // Placeholder
        "currentblocktx": 10, // Placeholder
        "difficulty": info.difficulty,
        "networkhashps": info.network_hashrate,
        "pooledtx": 10, // Placeholder
        "chain": node.config().read()
            .map_err(|e| JsonRpcError {
                code: ErrorCode::InternalError as i32,
                message: format!("Failed to read config: {}", e),
                data: None,
            })?
            .node.network_name.clone(),
        "warnings": ""
    }))
}

/// Get block template for mining
async fn get_block_template(
    _params: Value,
    node: web::Data<Arc<Node>>,
) -> Result<Value, JsonRpcError> {
    let template = node.get_block_template().await.map_err(|e| JsonRpcError {
        code: ErrorCode::ServerError as i32,
        message: format!("Failed to get block template: {}", e),
        data: None,
    })?;

    let txs_json: Vec<Value> = template.transactions.iter().map(|tx| {
        json!({
            "data": hex::encode(bincode::serialize(tx).unwrap_or_default()),
            "txid": hex::encode(tx.hash()),
            "hash": hex::encode(tx.hash()),
            "depends": [],
            "fee": tx.fee().unwrap_or(0),
            "sigops": 4, // Placeholder
            "weight": tx.weight()
        })
    }).collect();

    Ok(json!({
        "version": template.version,
        "previousblockhash": hex::encode(template.prev_hash),
        "transactions": txs_json,
        "coinbaseaux": {
            "flags": ""
        },
        "coinbasevalue": 5000000000, // Placeholder
        "longpollid": format!("{} {}", hex::encode(template.prev_hash), template.height),
        "target": hex::encode(template.target.to_be_bytes()),
        "mintime": template.timestamp,
        "mutable": [
            "time",
            "transactions",
            "prevblock"
        ],
        "noncerange": "00000000ffffffff",
        "sigoplimit": 80000,
        "sizelimit": 4000000,
        "curtime": template.timestamp,
        "bits": format!("{:x}", template.target),
        "height": template.height
    }))
}

/// Submit a mined block
async fn submit_block(
    params: Value,
    node: web::Data<Arc<Node>>,
) -> Result<Value, JsonRpcError> {
    // Extract block data parameter
    let block_data = match params {
        Value::Array(arr) if !arr.is_empty() => {
            match &arr[0] {
                Value::String(s) => s.clone(),
                _ => return Err(JsonRpcError {
                    code: ErrorCode::InvalidParams as i32,
                    message: "Invalid block data parameter (must be a hex string)".to_string(),
                    data: None,
                }),
            }
        },
        _ => return Err(JsonRpcError {
            code: ErrorCode::InvalidParams as i32,
            message: "Missing block data parameter".to_string(),
            data: None,
        }),
    };

    // Decode hex
    let block_bytes = hex::decode(&block_data).map_err(|_| JsonRpcError {
        code: ErrorCode::InvalidParams as i32,
        message: "Invalid block data format".to_string(),
        data: None,
    })?;

    // Deserialize block
    let block = bincode::deserialize(&block_bytes).map_err(|_| JsonRpcError {
        code: ErrorCode::InvalidParams as i32,
        message: "Invalid block data".to_string(),
        data: None,
    })?;

    // Submit block
    let result = node.submit_block(block).await.map_err(|e| JsonRpcError {
        code: ErrorCode::ServerError as i32,
        message: format!("Failed to submit block: {}", e),
        data: None,
    })?;

    if result.accepted {
        Ok(Value::Null)
    } else {
        Err(JsonRpcError {
            code: ErrorCode::ServerError as i32,
            message: result.reason.unwrap_or_else(|| "Block rejected".to_string()),
            data: None,
        })
    }
}

// Helper functions

/// Format transaction as JSON
fn format_transaction(tx: &btclib::types::transaction::Transaction) -> Value {
    // Placeholder implementation - in a real implementation, this would format the transaction
    // with all of its details according to the API specification
    json!({
        "txid": hex::encode(tx.hash()),
        "hash": hex::encode(tx.hash()),
        "version": tx.version,
        "size": tx.size(),
        "weight": tx.weight(),
        "locktime": tx.locktime,
    })
}

/// Calculate difficulty from target
fn calculate_difficulty(target: u32) -> f64 {
    // Placeholder implementation
    // In a real implementation, this would calculate the actual difficulty
    // based on the target difficulty bits
    1.0
}

/// Get next block hash
async fn get_next_block_hash(
    block_hash: &[u8; 32],
    node: web::Data<Arc<Node>>
) -> Result<Option<[u8; 32]>, JsonRpcError> {
    // This is a placeholder implementation
    // In a real implementation, this would query the blockchain database
    // to find the next block in the chain
    Ok(None)
}

/// Helper function to calculate network hashrate from difficulty
fn calculate_network_hashrate(difficulty: f64) -> f64 {
    // Network hashrate = difficulty * 2^32 / block_time_seconds
    // For 2.5 minute blocks (150 seconds)
    difficulty * 4_294_967_296.0 / 150.0
}