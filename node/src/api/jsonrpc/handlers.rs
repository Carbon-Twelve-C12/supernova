//! JSON-RPC method handlers
//!
//! This module implements handlers for JSON-RPC methods.

use std::sync::Arc;
use actix_web::web;
use serde_json::{Value, json};
use crate::api_facade::ApiFacade;
use super::types::{JsonRpcError, ErrorCode};
use btclib::blockchain::{calculate_difficulty_from_bits, calculate_hashrate};

/// Dispatch method to appropriate handler
pub async fn dispatch(
    method: &str,
    params: Value,
    node: web::Data<Arc<ApiFacade>>,
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

        // Environmental methods
        "getenvironmentalmetrics" => get_environmental_metrics(params, node).await,
        "getenvironmentalinfo" => get_environmental_info(params, node).await,
        "getnetworkstats" => get_network_stats(params, node).await,

        // Wallet methods
        "getnewaddress" => get_new_address(params, node).await,
        "getbalance" => get_balance(params, node).await,
        "listunspent" => list_unspent(params, node).await,

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
    node: web::Data<Arc<ApiFacade>>,
) -> Result<Value, JsonRpcError> {
    // Get blockchain info
    let storage = node.storage();
    let height = storage.get_height().map_err(|e| JsonRpcError {
        code: ErrorCode::BlockchainError as i32,
        message: format!("Failed to get blockchain height: {}", e),
        data: None,
    })?;

    let best_block_hash = if height > 0 {
        storage.get_block_hash_by_height(height).map_err(|e| JsonRpcError {
            code: ErrorCode::BlockchainError as i32,
            message: format!("Failed to get best block hash: {}", e),
            data: None,
        })?.unwrap_or([0u8; 32])
    } else {
        [0u8; 32]
    };

    let difficulty = if height > 0 {
        if let Ok(Some(block)) = storage.get_block(&best_block_hash) {
            calculate_difficulty_from_bits(block.header().bits())
        } else {
            1.0
        }
    } else {
        1.0
    };

    let chain_work = format!("0x{:x}", (difficulty * height as f64) as u128);
    let verification_progress = 1.0;

    // Get network info
    let connections = node.network().peer_count_sync() as u32;

    // Get mempool info
    let mempool_size = node.mempool().size();
    let mempool_bytes = node.mempool().size_in_bytes();
    let mempool_usage = node.mempool().get_memory_usage() as u64;
    let min_fee_rate = 1000u64; // Default min fee rate

    // Combine into a single response
    Ok(json!({
        "version": env!("CARGO_PKG_VERSION"),
        "protocolversion": 70015,
        "blocks": height,
        "headers": height,
        "bestblockhash": hex::encode(best_block_hash),
        "difficulty": difficulty,
        "chainwork": chain_work,
        "verificationprogress": verification_progress,
        "chain": node.config().read()
            .map_err(|e| JsonRpcError {
                code: ErrorCode::InternalError as i32,
                message: format!("Failed to read config: {}", e),
                data: None,
            })?
            .network.network_id.clone(),
        "warnings": "",
        "networkhashps": calculate_network_hashrate(difficulty),
        "connections": connections,
        "mempool": {
            "size": mempool_size,
            "bytes": mempool_bytes,
            "usage": mempool_usage,
            "minfee": min_fee_rate as f64 / 100000000.0, // Convert to NOVA/kB
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
    node: web::Data<Arc<ApiFacade>>,
) -> Result<Value, JsonRpcError> {
    let storage = node.storage();
    let height = storage.get_height().map_err(|e| JsonRpcError {
        code: ErrorCode::BlockchainError as i32,
        message: format!("Failed to get height: {}", e),
        data: None,
    })?;

    let best_block_hash = if height > 0 {
        storage.get_block_hash_by_height(height).map_err(|e| JsonRpcError {
            code: ErrorCode::BlockchainError as i32,
            message: format!("Failed to get best block hash: {}", e),
            data: None,
        })?.unwrap_or([0u8; 32])
    } else {
        [0u8; 32]
    };

    let (difficulty, median_time) = if height > 0 {
        if let Ok(Some(block)) = storage.get_block(&best_block_hash) {
            (calculate_difficulty_from_bits(block.header().bits()), block.timestamp())
        } else {
            (1.0, 0)
        }
    } else {
        (1.0, 0)
    };

    let chain_work = format!("0x{:x}", (difficulty * height as f64) as u128);
    let verification_progress = 1.0;
    let size_on_disk = 0u64; // Placeholder

    Ok(json!({
        "chain": node.config().read()
            .map_err(|e| JsonRpcError {
                code: ErrorCode::InternalError as i32,
                message: format!("Failed to read config: {}", e),
                data: None,
            })?
            .network.network_id.clone(),
        "blocks": height,
        "headers": height,
        "bestblockhash": hex::encode(best_block_hash),
        "difficulty": difficulty,
        "mediantime": median_time,
        "verificationprogress": verification_progress,
        "pruned": false,
        "chainwork": chain_work,
        "size_on_disk": size_on_disk,
    }))
}

/// Get block by hash
async fn get_block(
    params: Value,
    node: web::Data<Arc<ApiFacade>>,
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
    let storage = node.storage();
    let block = storage.get_block(&hash).map_err(|e| JsonRpcError {
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
            let mut txids = Vec::with_capacity(block.transactions().len());
            let mut txs = Vec::with_capacity(if params.verbosity == 2 { block.transactions().len() } else { 0 });

            for tx in block.transactions() {
                let txid = tx.hash();
                txids.push(hex::encode(txid));

                if params.verbosity == 2 {
                    // Full transaction details for verbosity 2
                    txs.push(format_transaction(tx));
                }
            }

            let current_height = storage.get_height().unwrap_or(0);
            let confirmations = current_height.saturating_sub(block.height()) + 1;

            let block_size = bincode::serialize(&block).unwrap_or_default().len();
            let difficulty = calculate_difficulty_from_bits(block.header().bits());

            let mut result = json!({
                "hash": hex::encode(block.hash()),
                "confirmations": confirmations,
                "size": block_size,
                "height": block.height(),
                "version": block.version(),
                "merkleroot": hex::encode(block.merkle_root()),
                "tx": if params.verbosity == 2 { Value::Array(txs) } else { Value::Array(txids.into_iter().map(Value::String).collect()) },
                "time": block.timestamp(),
                "nonce": block.nonce(),
                "bits": format!("{:08x}", block.header().bits()),
                "difficulty": difficulty,
                "previousblockhash": hex::encode(block.prev_block_hash()),
            });

            if block.height() > 0 {
                if let Some(next_block_hash) = get_next_block_hash(&block.hash(), storage.clone()).await? {
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
    node: web::Data<Arc<ApiFacade>>,
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
    let storage = node.storage();
    let hash = storage.get_block_hash_by_height(height).map_err(|e| JsonRpcError {
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
    node: web::Data<Arc<ApiFacade>>,
) -> Result<Value, JsonRpcError> {
    let storage = node.storage();
    let height = storage.get_height().map_err(|e| JsonRpcError {
        code: ErrorCode::BlockchainError as i32,
        message: format!("Failed to get height: {}", e),
        data: None,
    })?;

    let best_block_hash = if height > 0 {
        storage.get_block_hash_by_height(height).map_err(|e| JsonRpcError {
            code: ErrorCode::BlockchainError as i32,
            message: format!("Failed to get best block hash: {}", e),
            data: None,
        })?.unwrap_or([0u8; 32])
    } else {
        [0u8; 32]
    };

    Ok(Value::String(hex::encode(best_block_hash)))
}

/// Get the current block count
async fn get_block_count(
    _params: Value,
    node: web::Data<Arc<ApiFacade>>,
) -> Result<Value, JsonRpcError> {
    let storage = node.storage();
    let height = storage.get_height().map_err(|e| JsonRpcError {
        code: ErrorCode::BlockchainError as i32,
        message: format!("Failed to get height: {}", e),
        data: None,
    })?;

    Ok(Value::Number(serde_json::Number::from(height)))
}

/// Get the proof-of-work difficulty
async fn get_difficulty(
    _params: Value,
    node: web::Data<Arc<ApiFacade>>,
) -> Result<Value, JsonRpcError> {
    let storage = node.storage();
    let height = storage.get_height().map_err(|e| JsonRpcError {
        code: ErrorCode::BlockchainError as i32,
        message: format!("Failed to get height: {}", e),
        data: None,
    })?;

    let difficulty = if height > 0 {
        if let Ok(Some(hash)) = storage.get_block_hash_by_height(height) {
            if let Ok(Some(block)) = storage.get_block(&hash) {
                calculate_difficulty_from_bits(block.header().bits())
            } else {
                1.0
            }
        } else {
            1.0
        }
    } else {
        1.0
    };

    Ok(Value::Number(serde_json::Number::from_f64(difficulty).unwrap_or(serde_json::Number::from(0))))
}

/// Get transaction information
async fn get_transaction(
    params: Value,
    node: web::Data<Arc<ApiFacade>>,
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
    node: web::Data<Arc<ApiFacade>>,
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
    node: web::Data<Arc<ApiFacade>>,
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
    node: web::Data<Arc<ApiFacade>>,
) -> Result<Value, JsonRpcError> {
    let mempool = node.mempool();
    let tx_count = mempool.size();
    let size = mempool.size_in_bytes();
    let memory_usage = mempool.get_memory_usage() as u64;
    let min_fee_rate = 1000u64; // Default min fee rate

    Ok(json!({
        "size": tx_count,
        "bytes": size,
        "usage": memory_usage,
        "maxmempool": node.config().read()
            .map_err(|e| JsonRpcError {
                code: ErrorCode::InternalError as i32,
                message: format!("Failed to read config: {}", e),
                data: None,
            })?
            .mempool.max_size, // Already in bytes
        "mempoolminfee": min_fee_rate as f64 / 100000000.0, // Convert to NOVA/kB
        "minrelaytxfee": min_fee_rate as f64 / 100000000.0, // Convert to NOVA/kB
    }))
}

/// Get raw mempool transactions
async fn get_raw_mempool(
    params: Value,
    node: web::Data<Arc<ApiFacade>>,
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

    let mempool = node.mempool();
    let txs = mempool.get_all_transactions();

    if verbose {
        let mut result = serde_json::Map::new();

        for tx in txs {
            let txid = hex::encode(tx.hash());
            let tx_size = bincode::serialize(&tx).unwrap_or_default().len();
            let tx_info = json!({
                "size": tx_size,
                "fee": 1000, // Placeholder fee
                "time": 0, // Placeholder timestamp
                "height": 0, // Not yet in a block
                "descendantcount": 1, // Placeholder
                "descendantsize": tx_size,
                "descendantfees": 1000,
                "ancestorcount": 1, // Placeholder
                "ancestorsize": tx_size,
                "ancestorfees": 1000,
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
    node: web::Data<Arc<ApiFacade>>,
) -> Result<Value, JsonRpcError> {
    let network = node.network();
    let connections = network.peer_count_sync() as u32;

    Ok(json!({
        "version": env!("CARGO_PKG_VERSION").replace(".", ""),
        "subversion": format!("/supernova:{}/", env!("CARGO_PKG_VERSION")),
        "protocolversion": 70015, // Protocol version
        "localservices": "000000000000000d",
        "localrelay": true,
        "timeoffset": 0,
        "connections": connections,
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
        "localaddresses": [],
        "warnings": ""
    }))
}

/// Get peer information
async fn get_peer_info(
    _params: Value,
    node: web::Data<Arc<ApiFacade>>,
) -> Result<Value, JsonRpcError> {
    // Placeholder - peer info not yet available from ApiFacade
    Ok(Value::Array(vec![]))
}

/// Get mining information
async fn get_mining_info(
    _params: Value,
    node: web::Data<Arc<ApiFacade>>,
) -> Result<Value, JsonRpcError> {
    let storage = node.storage();
    let height = storage.get_height().map_err(|e| JsonRpcError {
        code: ErrorCode::BlockchainError as i32,
        message: format!("Failed to get height: {}", e),
        data: None,
    })?;

    let difficulty = if height > 0 {
        if let Ok(Some(hash)) = storage.get_block_hash_by_height(height) {
            if let Ok(Some(block)) = storage.get_block(&hash) {
                calculate_difficulty_from_bits(block.header().bits())
            } else {
                1.0
            }
        } else {
            1.0
        }
    } else {
        1.0
    };

    let network_hashrate = calculate_hashrate(difficulty, 150); // 2.5 minute block time

    Ok(json!({
        "blocks": height,
        "currentblockweight": 4000, // Placeholder
        "currentblocktx": 10, // Placeholder
        "difficulty": difficulty,
        "networkhashps": network_hashrate,
        "pooledtx": node.mempool().size(),
        "chain": node.config().read()
            .map_err(|e| JsonRpcError {
                code: ErrorCode::InternalError as i32,
                message: format!("Failed to read config: {}", e),
                data: None,
            })?
            .network.network_id.clone(),
        "warnings": ""
    }))
}

/// Get block template for mining
async fn get_block_template(
    _params: Value,
    node: web::Data<Arc<ApiFacade>>,
) -> Result<Value, JsonRpcError> {
    // Placeholder - block template generation not yet available
    Err(JsonRpcError {
        code: ErrorCode::MethodNotFound as i32,
        message: "Method not yet implemented".to_string(),
        data: None,
    })
}

/// Submit a mined block
async fn submit_block(
    params: Value,
    node: web::Data<Arc<ApiFacade>>,
) -> Result<Value, JsonRpcError> {
    // Placeholder - block submission not yet available
    Err(JsonRpcError {
        code: ErrorCode::MethodNotFound as i32,
        message: "Method not yet implemented".to_string(),
        data: None,
    })
}

// Helper functions

/// Format transaction as JSON
fn format_transaction(tx: &btclib::types::transaction::Transaction) -> Value {
    let tx_size = bincode::serialize(tx).unwrap_or_default().len();
    let weight = tx_size * 4; // Simplified weight calculation

    json!({
        "txid": hex::encode(tx.hash()),
        "hash": hex::encode(tx.hash()),
        "version": tx.version(),
        "size": tx_size,
        "weight": weight,
        "locktime": tx.lock_time(),
    })
}

/// Get next block hash
async fn get_next_block_hash(
    block_hash: &[u8; 32],
    storage: Arc<crate::storage::BlockchainDB>
) -> Result<Option<[u8; 32]>, JsonRpcError> {
    // Get current block height
    let current_height = storage.get_block_height(block_hash)
        .map_err(|e| JsonRpcError {
            code: ErrorCode::BlockchainError as i32,
            message: format!("Failed to get block height: {}", e),
            data: None,
        })?;

    if let Some(height) = current_height {
        // Try to get the next block
        storage.get_block_hash_by_height(height + 1)
            .map_err(|e| JsonRpcError {
                code: ErrorCode::BlockchainError as i32,
                message: format!("Failed to get next block hash: {}", e),
                data: None,
            })
    } else {
        Ok(None)
    }
}

/// Helper function to calculate network hashrate from difficulty
fn calculate_network_hashrate(difficulty: f64) -> f64 {
    // Network hashrate = difficulty * 2^32 / block_time_seconds
    // For 10 minute blocks (600 seconds) - Supernova target
    difficulty * 4_294_967_296.0 / 600.0
}

/// Get environmental metrics
async fn get_environmental_metrics(
    _params: Value,
    node: web::Data<Arc<ApiFacade>>,
) -> Result<Value, JsonRpcError> {
    // Placeholder - environmental data not yet available from ApiFacade
    Ok(json!({
        "totalEmissions": 0.0,
        "carbonOffsets": 0.0,
        "netCarbon": 0.0,
        "renewablePercentage": 0.0,
        "treasuryBalance": 0,
        "isCarbonNegative": false,
        "greenMiners": 0,
        "lastUpdated": 0,
    }))
}

/// Get environmental information
async fn get_environmental_info(
    _params: Value,
    node: web::Data<Arc<ApiFacade>>,
) -> Result<Value, JsonRpcError> {
    // Placeholder - environmental data not yet available from ApiFacade
    Ok(json!({
        "carbonIntensity": 0.0,
        "greenMining": 0.0,
        "carbonNegative": false,
        "totalEmissions": 0.0,
        "totalOffsets": 0.0,
        "netEmissions": 0.0,
    }))
}

/// Get comprehensive network statistics
async fn get_network_stats(
    _params: Value,
    node: web::Data<Arc<ApiFacade>>,
) -> Result<Value, JsonRpcError> {
    let storage = node.storage();
    let height = storage.get_height().map_err(|e| JsonRpcError {
        code: ErrorCode::BlockchainError as i32,
        message: format!("Failed to get height: {}", e),
        data: None,
    })?;

    let difficulty = if height > 0 {
        if let Ok(Some(hash)) = storage.get_block_hash_by_height(height) {
            if let Ok(Some(block)) = storage.get_block(&hash) {
                calculate_difficulty_from_bits(block.header().bits())
            } else {
                1.0
            }
        } else {
            1.0
        }
    } else {
        1.0
    };

    let network_hashrate = calculate_hashrate(difficulty, 150); // 2.5 minute block time
    let connections = node.network().peer_count_sync() as u32;
    let mempool_size = node.mempool().size();

    Ok(json!({
        "blockHeight": height,
        "hashrate": network_hashrate.to_string(),
        "difficulty": difficulty.to_string(),
        "nodes": connections,
        "transactions24h": mempool_size, // Placeholder - needs actual 24h count
        "carbonIntensity": 0.0,
        "greenMiningPercentage": 0.0,
        "quantumSecurityLevel": "HIGH", // Hardcoded for now
        "networkId": node.config().read()
            .map_err(|e| JsonRpcError {
                code: ErrorCode::InternalError as i32,
                message: format!("Failed to read config: {}", e),
                data: None,
            })?
            .network.network_id.clone(),
    }))
}

// ============================================================================
// WALLET RPC METHODS
// ============================================================================

/// Get new quantum-resistant address
async fn get_new_address(
    params: Value,
    node: web::Data<Arc<ApiFacade>>,
) -> Result<Value, JsonRpcError> {
    // Parse optional label parameter
    let label = match params {
        Value::Array(arr) if !arr.is_empty() => {
            match &arr[0] {
                Value::String(s) => Some(s.clone()),
                _ => None,
            }
        }
        _ => None,
    };
    
    // Get wallet manager
    let wallet_manager = node.wallet_manager();
    let wallet = wallet_manager.read()
        .map_err(|_| JsonRpcError {
            code: -13,
            message: "Wallet lock poisoned".to_string(),
            data: None,
        })?;
    
    // Generate new address using actual wallet manager
    let address = wallet.generate_new_address(label)
        .map_err(|e| JsonRpcError {
            code: match e {
                crate::wallet_manager::WalletManagerError::WalletLocked => -13,
                _ => -1,
            },
            message: format!("Failed to generate address: {}", e),
            data: None,
        })?;
    
    Ok(Value::String(address))
}

/// Get wallet balance
async fn get_balance(
    params: Value,
    node: web::Data<Arc<ApiFacade>>,
) -> Result<Value, JsonRpcError> {
    // Parse parameters
    let (minconf, _include_watchonly) = match params {
        Value::Array(arr) => {
            let minconf = arr.get(0).and_then(|v| v.as_u64()).unwrap_or(1);
            let watchonly = arr.get(1).and_then(|v| v.as_bool()).unwrap_or(false);
            (minconf, watchonly)
        }
        _ => (1, false),
    };
    
    // Get wallet manager
    let wallet_manager = node.wallet_manager();
    let wallet = wallet_manager.read()
        .map_err(|_| JsonRpcError {
            code: -13,
            message: "Wallet lock poisoned".to_string(),
            data: None,
        })?;
    
    // Get actual balance from UTXO index
    let balance_attonovas = wallet.get_balance(minconf)
        .map_err(|e| JsonRpcError {
            code: -1,
            message: format!("Failed to get balance: {}", e),
            data: None,
        })?;
    
    let balance_nova = balance_attonovas as f64 / 100_000_000.0;
    
    Ok(json!(balance_nova))
}

/// List unspent transaction outputs
async fn list_unspent(
    params: Value,
    node: web::Data<Arc<ApiFacade>>,
) -> Result<Value, JsonRpcError> {
    // Parse parameters
    let (minconf, maxconf, addresses) = match params {
        Value::Array(arr) => {
            let minconf = arr.get(0).and_then(|v| v.as_u64()).unwrap_or(1);
            let maxconf = arr.get(1).and_then(|v| v.as_u64()).unwrap_or(9999999);
            let addresses: Vec<String> = arr.get(2)
                .and_then(|v| v.as_array())
                .map(|a| a.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect())
                .unwrap_or_default();
            (minconf, maxconf, addresses)
        }
        _ => (1, 9999999, vec![]),
    };
    
    // Get wallet manager
    let wallet_manager = node.wallet_manager();
    let wallet = wallet_manager.read()
        .map_err(|_| JsonRpcError {
            code: -13,
            message: "Wallet lock poisoned".to_string(),
            data: None,
        })?;
    
    // Get UTXOs from wallet
    let filter_addresses = if addresses.is_empty() { None } else { Some(addresses) };
    let utxos = wallet.list_unspent(minconf, maxconf, filter_addresses)
        .map_err(|e| JsonRpcError {
            code: -1,
            message: format!("Failed to list unspent: {}", e),
            data: None,
        })?;
    
    // Format UTXOs as JSON
    let utxos_json: Vec<Value> = utxos.iter().map(|utxo| {
        json!({
            "txid": hex::encode(&utxo.txid),
            "vout": utxo.vout,
            "address": &utxo.address,
            "scriptPubKey": hex::encode(&utxo.script_pubkey),
            "amount": utxo.value as f64 / 100_000_000.0,
            "confirmations": utxo.confirmations,
            "spendable": utxo.spendable,
            "solvable": utxo.solvable,
            "label": utxo.label.as_ref().unwrap_or(&String::new()),
        })
    }).collect();
    
    Ok(Value::Array(utxos_json))
}