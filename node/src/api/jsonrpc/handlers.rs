//! JSON-RPC method handlers
//!
//! This module implements handlers for JSON-RPC methods.

use std::sync::Arc;
use actix_web::web;
use serde_json::{Value, json};
use crate::api_facade::ApiFacade;
use super::types::{JsonRpcError, ErrorCode};
use supernova_core::blockchain::{calculate_difficulty_from_bits, calculate_hashrate};

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
        "gettransaction" => get_transaction_rpc(params, node).await,
        "getrawtransaction" => get_raw_transaction_rpc(params, node).await,
        "sendrawtransaction" => send_raw_transaction(params, node).await,

        // Mempool methods
        "getmempoolinfo" => get_mempool_info(params, node).await,
        "getrawmempool" => get_raw_mempool(params, node).await,

        // Network methods
        "getnetworkinfo" => get_network_info(params, node).await,
        "getpeerinfo" => get_peer_info(params, node).await,
        "getlocalpeerid" => get_local_peer_id(params, node).await,

        // Mining methods
        "getmininginfo" => get_mining_info(params, node).await,
        "getblocktemplate" => get_block_template(params, node).await,
        "submitblock" => submit_block(params, node).await,
        "generate" => generate_blocks(params, node).await,

        // Environmental methods
        "getenvironmentalmetrics" => get_environmental_metrics(params, node).await,
        "getenvironmentalinfo" => get_environmental_info(params, node).await,
        "getnetworkstats" => get_network_stats(params, node).await,

        // Wallet methods
        "getnewaddress" => get_new_address(params, node).await,
        "getbalance" => get_balance(params, node).await,
        "listunspent" => list_unspent(params, node).await,
        "sendtoaddress" => send_to_address(params, node).await,
        
        // Network admin methods
        "addnode" => add_node(params, node).await,
        
        // Test/admin methods
        "addtestutxo" => add_test_utxo(params, node).await,

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
    let verification_progress = node.network().get_sync_progress();

    // Get network info
    let connections = node.network().peer_count_sync() as u32;

    // Get mempool info
    let mempool_size = node.mempool().size();
    let mempool_bytes = node.mempool().size_in_bytes();
    let mempool_usage = node.mempool().get_memory_usage() as u64;
    let min_fee_rate = node.mempool().get_info().min_fee_rate;

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
        "networkhashps": calculate_hashrate(difficulty, 150), // 2.5 minute block time
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
    let verification_progress = node.network().get_sync_progress();
    let size_on_disk = directory_size_on_disk(storage.path());

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

    if height == 0 {
        return Err(JsonRpcError {
            code: ErrorCode::BlockchainError as i32,
            message: "No blocks in blockchain yet".to_string(),
            data: None,
        });
    }

    let best_block_hash = storage.get_block_hash_by_height(height)
        .map_err(|e| JsonRpcError {
            code: ErrorCode::BlockchainError as i32,
            message: format!("Failed to get best block hash: {}", e),
            data: None,
        })?
        .ok_or_else(|| JsonRpcError {
            code: ErrorCode::BlockchainError as i32,
            message: format!("Best block hash not found at height {}", height),
            data: None,
        })?;

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

/// Get transaction information (renamed to avoid conflict)
async fn get_transaction_rpc(
    params: Value,
    node: web::Data<Arc<ApiFacade>>,
) -> Result<Value, JsonRpcError> {
    // Parse txid parameter
    let txid_str = match params {
        Value::Array(ref arr) if !arr.is_empty() => {
            arr.get(0)
                .and_then(|v| v.as_str())
                .ok_or_else(|| JsonRpcError {
                    code: ErrorCode::InvalidParams as i32,
                    message: "Missing or invalid txid parameter".to_string(),
        data: None,
                })?
        }
        _ => {
            return Err(JsonRpcError {
                code: ErrorCode::InvalidParams as i32,
                message: "Missing txid parameter".to_string(),
                data: None,
            });
        }
    };
    
    // Decode txid
    let txid_bytes = hex::decode(txid_str)
        .map_err(|_| JsonRpcError {
            code: ErrorCode::InvalidParams as i32,
            message: "Invalid txid format".to_string(),
            data: None,
        })?;
    
    if txid_bytes.len() != 32 {
        return Err(JsonRpcError {
            code: ErrorCode::InvalidParams as i32,
            message: "Invalid txid length".to_string(),
            data: None,
        });
    }
    
    let mut txid = [0u8; 32];
    txid.copy_from_slice(&txid_bytes);
    
    // Get wallet manager
    let wallet_manager = node.wallet_manager();
    let wallet = wallet_manager.read()
        .map_err(|_| JsonRpcError {
            code: -1,
            message: "Wallet lock poisoned".to_string(),
            data: None,
        })?;
    
    // Get transaction
    let transaction = wallet.get_transaction(&txid)
        .map_err(|e| JsonRpcError {
            code: -1,
            message: format!("Failed to get transaction: {}", e),
            data: None,
        })?
        .ok_or_else(|| JsonRpcError {
            code: -5,
            message: format!("Transaction {} not found", txid_str),
            data: None,
        })?;
    
    // Calculate confirmations from blockchain
    let confirmations = {
        let db = node.storage();
        let chain_state = node.chain_state();

        // Get the block containing this transaction
        match db.get_transaction_block(&txid) {
            Ok(Some(block_hash)) => {
                // Get block height
                match db.get_block_height(&block_hash) {
                    Ok(Some(tx_block_height)) => {
                        // Get current chain height
                        match chain_state.read() {
                            Ok(state) => {
                                let current_height = state.get_height();
                                // Confirmations = current_height - tx_block_height + 1
                                if current_height >= tx_block_height {
                                    (current_height - tx_block_height + 1) as i64
                                } else {
                                    0i64
                                }
                            }
                            Err(_) => 0i64,
                        }
                    }
                    Ok(None) | Err(_) => 0i64,
                }
            }
            Ok(None) => {
                // Transaction not in a block yet (in mempool)
                0i64
            }
            Err(_) => 0i64,
        }
    };

    // Format transaction as JSON
    Ok(json!({
        "txid": hex::encode(txid),
        "hash": hex::encode(transaction.hash()),
        "version": transaction.version(),
        "size": bincode::serialize(&transaction).map(|b| b.len()).unwrap_or(0),
        "locktime": transaction.lock_time(),
        "vin": transaction.inputs().len(),
        "vout": transaction.outputs().len(),
        "confirmations": confirmations,
    }))
}

/// Get raw transaction data (renamed to avoid conflict)
async fn get_raw_transaction_rpc(
    params: Value,
    node: web::Data<Arc<ApiFacade>>,
) -> Result<Value, JsonRpcError> {
    // Parse txid parameter
    let txid_str = match params {
        Value::Array(ref arr) if !arr.is_empty() => {
            arr.get(0)
                .and_then(|v| v.as_str())
                .ok_or_else(|| JsonRpcError {
                    code: ErrorCode::InvalidParams as i32,
                    message: "Missing or invalid txid parameter".to_string(),
        data: None,
                })?
        }
        _ => {
            return Err(JsonRpcError {
                code: ErrorCode::InvalidParams as i32,
                message: "Missing txid parameter".to_string(),
                data: None,
            });
        }
    };
    
    // Decode txid
    let txid_bytes = hex::decode(txid_str)
        .map_err(|_| JsonRpcError {
            code: ErrorCode::InvalidParams as i32,
            message: "Invalid txid format".to_string(),
            data: None,
        })?;
    
    if txid_bytes.len() != 32 {
        return Err(JsonRpcError {
            code: ErrorCode::InvalidParams as i32,
            message: "Invalid txid length".to_string(),
            data: None,
        });
    }
    
    let mut txid = [0u8; 32];
    txid.copy_from_slice(&txid_bytes);
    
    // Get wallet manager
    let wallet_manager = node.wallet_manager();
    let wallet = wallet_manager.read()
        .map_err(|_| JsonRpcError {
            code: -1,
            message: "Wallet lock poisoned".to_string(),
            data: None,
        })?;
    
    // Get transaction
    let transaction = wallet.get_transaction(&txid)
        .map_err(|e| JsonRpcError {
            code: -1,
            message: format!("Failed to get transaction: {}", e),
            data: None,
        })?
        .ok_or_else(|| JsonRpcError {
            code: -5,
            message: format!("Transaction {} not found", txid_str),
            data: None,
        })?;
    
    // Serialize transaction to hex
    let tx_bytes = bincode::serialize(&transaction)
        .map_err(|e| JsonRpcError {
            code: -1,
            message: format!("Failed to serialize transaction: {}", e),
            data: None,
        })?;
    
    Ok(Value::String(hex::encode(tx_bytes)))
}

/// Send raw transaction
///
/// Accepts a hex-encoded, bincode-serialized transaction, submits it to the
/// mempool via the existing mempool-acceptance validation path, and on
/// success broadcasts it to the P2P network. Mirrors the REST
/// `/mempool/submit` handler in `node/src/api/routes/mempool.rs`.
async fn send_raw_transaction(
    params: Value,
    node: web::Data<Arc<ApiFacade>>,
) -> Result<Value, JsonRpcError> {
    // Parse raw transaction hex parameter
    let raw_tx_hex = match params {
        Value::Array(ref arr) if !arr.is_empty() => arr
            .get(0)
            .and_then(|v| v.as_str())
            .ok_or_else(|| JsonRpcError {
                code: ErrorCode::InvalidParams as i32,
                message: "Missing or invalid raw transaction parameter".to_string(),
                data: None,
            })?,
        _ => {
            return Err(JsonRpcError {
                code: ErrorCode::InvalidParams as i32,
                message: "Missing raw transaction parameter".to_string(),
                data: None,
            });
        }
    };

    // Decode hex
    let tx_bytes = hex::decode(raw_tx_hex).map_err(|_| JsonRpcError {
        code: ErrorCode::InvalidParams as i32,
        message: "Invalid raw transaction hex encoding".to_string(),
        data: None,
    })?;

    // Deserialize transaction
    let transaction =
        bincode::deserialize::<supernova_core::types::transaction::Transaction>(&tx_bytes)
            .map_err(|_| JsonRpcError {
                code: ErrorCode::InvalidParams as i32,
                message: "Invalid raw transaction format".to_string(),
                data: None,
            })?;

    let txid = hex::encode(transaction.hash());

    // Submit to mempool via the existing mempool-acceptance validation path
    match node.mempool().add_transaction(transaction.clone(), 1000) {
        Ok(()) => {
            // Broadcast to the P2P network directly (avoids re-adding to the
            // mempool, which `ApiFacade::broadcast_transaction` would do and
            // which would short-circuit on `TransactionExists`).
            node.network().broadcast_transaction(&transaction);
            Ok(Value::String(txid))
        }
        Err(e) => Err(JsonRpcError {
            code: ErrorCode::InvalidParams as i32,
            message: format!("Transaction rejected: {}", e),
            data: None,
        }),
    }
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
    let min_fee_rate = mempool.get_info().min_fee_rate;

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

    if verbose {
        let mut result = serde_json::Map::new();

        // Real chain height; verbose mempool entries report the height at which
        // they are being considered for inclusion (the current best height).
        let height = node.storage().get_height().unwrap_or(0);

        for entry in mempool.get_all_transaction_entries() {
            // The pool tracks no in-mempool ancestry graph, so each entry is its
            // own (and only) ancestor/descendant. Fee, fee rate and entry time
            // are the real values recorded by the mempool.
            let tx_info = json!({
                "size": entry.size,
                "fee": entry.fee,
                "fee_rate": entry.fee_rate,
                "time": entry.time,
                "height": height,
                "descendantcount": 1,
                "descendantsize": entry.size,
                "descendantfees": entry.fee,
                "ancestorcount": 1,
                "ancestorsize": entry.size,
                "ancestorfees": entry.fee,
            });

            result.insert(entry.txid, tx_info);
        }

        Ok(Value::Object(result))
    } else {
        let txids: Vec<Value> = mempool
            .get_all_transactions()
            .iter()
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
    // Get connected peers from network
    let peers = node.network().get_peers().await
        .map_err(|e| JsonRpcError {
        code: ErrorCode::NetworkError as i32,
            message: format!("Failed to get peers: {}", e),
        data: None,
    })?;

    // Format peers as JSON
    let peers_json: Vec<Value> = peers.iter().map(|peer| {
        json!({
            "id": peer.id,
            "addr": peer.address,
            "conntime": peer.connected_time,
            "lastsend": peer.last_send,
            "lastrecv": peer.last_recv,
            "bytessent": peer.bytes_sent,
            "bytesrecv": peer.bytes_received,
            "pingtime": peer.ping_time,
            "version": peer.version,
            "direction": peer.direction,
        })
    }).collect();

    Ok(Value::Array(peers_json))
}

/// Get local peer ID for P2P networking
async fn get_local_peer_id(
    _params: Value,
    node: web::Data<Arc<ApiFacade>>,
) -> Result<Value, JsonRpcError> {
    // Get peer ID from network proxy
    let peer_id = node.network().local_peer_id();
    
    Ok(Value::String(peer_id.to_string()))
}

/// Derive the candidate-block weight and transaction count a miner would build
/// next, given the serialized sizes of the mempool transactions available for
/// selection.
///
/// Mirrors `mining::template::BlockTemplate::generate`, which assembles a
/// coinbase plus up to `TEMPLATE_MAX_TXS` mempool transactions (`take(100)`).
/// Returns `(currentblockweight, currentblocktx)` where weight is BIP141 units
/// (serialized size * 4) over the selected transactions and the count includes
/// the coinbase every block carries. This reports real state instead of the old
/// hardcoded `4000`/`10` placeholders; the weight omits the not-yet-assembled
/// coinbase but fabricates nothing.
fn current_block_stats(mempool_tx_sizes: &[usize]) -> (u64, u64) {
    const TEMPLATE_MAX_TXS: usize = 100;
    let selected_count = mempool_tx_sizes.len().min(TEMPLATE_MAX_TXS);
    let weight: u64 = mempool_tx_sizes
        .iter()
        .take(TEMPLATE_MAX_TXS)
        .map(|&size| size as u64)
        .sum::<u64>()
        .saturating_mul(4);
    // +1 for the coinbase transaction every block includes.
    (weight, selected_count as u64 + 1)
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

    // `currentblockweight`/`currentblocktx` describe the candidate block a miner
    // would build next. Derive them from real mempool state instead of hardcoded
    // placeholders (see `current_block_stats`).
    let mempool_tx_sizes: Vec<usize> = node
        .mempool()
        .get_all_transaction_entries()
        .iter()
        .map(|e| e.size)
        .collect();
    let (current_block_weight, current_block_tx) = current_block_stats(&mempool_tx_sizes);

    Ok(json!({
        "blocks": height,
        "currentblockweight": current_block_weight,
        "currentblocktx": current_block_tx,
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
    // Get wallet manager for reward address
    let wallet_manager = node.wallet_manager();
    let wallet = wallet_manager.read()
        .map_err(|_| JsonRpcError {
            code: -1,
            message: "Wallet lock poisoned".to_string(),
        data: None,
    })?;

    // Generate reward address (or use existing)
    let reward_address = wallet.generate_new_address(Some("mining_reward".to_string()))
        .map_err(|e| JsonRpcError {
            code: -1,
            message: format!("Failed to generate reward address: {}", e),
            data: None,
        })?;
    
    let reward_addr = wallet::quantum_wallet::Address::from_str(&reward_address)
        .map_err(|e| JsonRpcError {
            code: -1,
            message: format!("Invalid reward address: {}", e),
            data: None,
        })?;
    
    // Generate treasury address
    let treasury_address = wallet.generate_new_address(Some("environmental_treasury".to_string()))
        .map_err(|e| JsonRpcError {
            code: -1,
            message: format!("Failed to generate treasury address: {}", e),
            data: None,
        })?;
    
    let treasury_addr = wallet::quantum_wallet::Address::from_str(&treasury_address)
        .map_err(|e| JsonRpcError {
            code: -1,
            message: format!("Invalid treasury address: {}", e),
            data: None,
        })?;
    
    drop(wallet); // Release wallet lock
    
    // Generate block template
    use crate::mining::template::BlockTemplate;
    let template = BlockTemplate::generate(
        node.chain_state(),
        node.mempool(),
        &reward_addr,
        &treasury_addr,
    ).map_err(|e| JsonRpcError {
        code: -1,
        message: format!("Failed to generate template: {}", e),
        data: None,
    })?;
    
    // Format as JSON-RPC response
    let mempool = node.mempool();
    let transactions_json: Vec<Value> = template.transactions.iter().skip(1) // Skip coinbase
        .map(|tx| {
        let txid = tx.hash();
        // Report the real per-transaction fee the mempool tracks (fee_rate * size).
        // External miners select/order by this field (BIP22); a hardcoded placeholder
        // misleads them. Fall back to 0 when the tx is not (or no longer) in the pool.
        let fee = mempool.get_transaction_fee(&txid).unwrap_or(0);
        json!({
            "data": hex::encode(bincode::serialize(tx).unwrap_or_default()),
            "txid": hex::encode(txid),
            "hash": hex::encode(txid),
                "fee": fee,
        })
    }).collect();

    Ok(json!({
        "version": template.version,
        "previousblockhash": hex::encode(template.previous_block_hash),
        "transactions": transactions_json,
        "coinbasevalue": template.coinbase_value,
        "target": format!("{:08x}", template.bits),
        "mintime": template.timestamp,
        "curtime": template.timestamp,
        "bits": format!("{:08x}", template.bits),
        "height": template.height,
        "merkleroot": hex::encode(template.merkle_root),
    }))
}

/// Submit a mined block
async fn submit_block(
    params: Value,
    node: web::Data<Arc<ApiFacade>>,
) -> Result<Value, JsonRpcError> {
    // Parse hex-encoded block
    let block_hex = match params {
        Value::Array(ref arr) if !arr.is_empty() => {
            arr.get(0)
                .and_then(|v| v.as_str())
                .ok_or_else(|| JsonRpcError {
                    code: ErrorCode::InvalidParams as i32,
                    message: "Missing or invalid block data".to_string(),
                    data: None,
                })?
            }
        _ => {
            return Err(JsonRpcError {
            code: ErrorCode::InvalidParams as i32,
            message: "Missing block data parameter".to_string(),
            data: None,
            });
        }
    };

    // Decode hex
    let block_bytes = hex::decode(block_hex)
        .map_err(|_| JsonRpcError {
        code: ErrorCode::InvalidParams as i32,
            message: "Invalid block hex encoding".to_string(),
        data: None,
    })?;

    // Deserialize block
    let block: supernova_core::types::block::Block = bincode::deserialize(&block_bytes)
        .map_err(|e| JsonRpcError {
        code: ErrorCode::InvalidParams as i32,
            message: format!("Failed to deserialize block: {}", e),
        data: None,
    })?;

    // Validate block
    if !block.validate() {
        return Err(JsonRpcError {
            code: -25, // Block validation error
            message: "Block validation failed".to_string(),
            data: None,
        });
    }
    
    // Verify proof-of-work
    if !block.header().meets_target() {
        return Err(JsonRpcError {
            code: -25,
            message: "Block does not meet difficulty target".to_string(),
            data: Some(json!({
                "hash": hex::encode(block.hash()),
                "target": hex::encode(block.header().target()),
            })),
        });
    }
    
    // Get chain state and process block.
    // Use spawn_blocking so the std::sync::RwLock write guard is never held
    // across the .await inside add_block on the async actix worker thread
    // (mirrors the discipline in node.rs / network/mod.rs).
    let chain_state = node.chain_state();
    let block_for_add = block.clone();
    // The guard is intentionally held across the await inside a dedicated
    // spawn_blocking thread, so it cannot stall an actix worker's executor.
    #[allow(clippy::await_holding_lock)]
    let add_result = tokio::task::spawn_blocking(move || {
        tokio::runtime::Handle::current().block_on(async move {
            match chain_state.write() {
                Ok(mut chain) => chain.add_block(&block_for_add).await,
                Err(e) => Err(crate::storage::StorageError::DatabaseError(format!(
                    "Chain state lock poisoned: {}",
                    e
                ))),
            }
        })
    })
    .await;
    add_result
        .map_err(|e| JsonRpcError {
            code: -1,
            message: format!("Block processing task failed: {}", e),
            data: None,
        })?
        .map_err(|e| JsonRpcError {
            code: -25,
            message: format!("Failed to add block: {}", e),
            data: None,
        })?;

    // Scan block for wallet transactions
    let wallet_manager = node.wallet_manager();
    if let Ok(wallet) = wallet_manager.write() {
        if let Err(e) = wallet.scan_block(&block) {
            tracing::warn!("Failed to scan block for wallet: {}", e);
        }
    }
    
    // Store block in database
    node.storage().insert_block(&block)
        .map_err(|e| JsonRpcError {
            code: -1,
            message: format!("Failed to store block: {}", e),
            data: None,
        })?;
    
    // Broadcast block to P2P network
    let block_hash = block.hash();
    tracing::info!("Broadcasting block {} to network", hex::encode(&block_hash[..8]));
    node.network().broadcast_block(&block);
    
    tracing::info!("Accepted block {} at height {}", 
        hex::encode(&block_hash[..8]), block.height());
    
    // Success - return null
        Ok(Value::Null)
}

/// Generate blocks using CPU mining (testnet only)
#[cfg(feature = "testnet")]
// The `wallet` read guard is explicitly dropped before the spawn_blocking
// await, and the chain-state write guard lives entirely on the dedicated
// spawn_blocking thread; clippy's await_holding_lock ignores explicit drops
// and cannot see the thread boundary, so both are false positives here.
#[allow(clippy::await_holding_lock)]
async fn generate_blocks(
    params: Value,
    node: web::Data<Arc<ApiFacade>>,
) -> Result<Value, JsonRpcError> {
    // Parse number of blocks to generate
    let num_blocks = match params {
        Value::Array(ref arr) if !arr.is_empty() => {
            arr.get(0)
                .and_then(|v| v.as_u64())
                .ok_or_else(|| JsonRpcError {
                    code: ErrorCode::InvalidParams as i32,
                    message: "Missing or invalid number of blocks parameter".to_string(),
                    data: None,
                })?
        }
        Value::Number(n) => n.as_u64().ok_or_else(|| JsonRpcError {
            code: ErrorCode::InvalidParams as i32,
            message: "Invalid number of blocks".to_string(),
            data: None,
        })?,
        _ => {
            return Err(JsonRpcError {
                code: ErrorCode::InvalidParams as i32,
                message: "Expected number of blocks as parameter".to_string(),
                data: None,
            });
        }
    };

    if num_blocks == 0 {
        return Err(JsonRpcError {
            code: ErrorCode::InvalidParams as i32,
            message: "Number of blocks must be greater than 0".to_string(),
            data: None,
        });
    }

    if num_blocks > 1000 {
        return Err(JsonRpcError {
            code: ErrorCode::InvalidParams as i32,
            message: "Cannot generate more than 1000 blocks at once".to_string(),
            data: None,
        });
    }

    tracing::info!("Generating {} block(s) using CPU miner", num_blocks);

    let mut block_hashes = Vec::new();

    for i in 0..num_blocks {
        tracing::debug!("Mining block {} of {}", i + 1, num_blocks);

        // Get wallet manager for addresses.
        // `wallet` is explicitly dropped (see drop(wallet) below) before the
        // later spawn_blocking await (fn-level allow covers the clippy false
        // positive from await_holding_lock ignoring the explicit drop).
        let wallet_manager = node.wallet_manager();
        let wallet = wallet_manager.read()
            .map_err(|_| JsonRpcError {
                code: -1,
                message: "Wallet lock poisoned".to_string(),
                data: None,
            })?;

        // Generate reward address
        let reward_address = wallet.generate_new_address(Some("mining_reward".to_string()))
            .map_err(|e| JsonRpcError {
                code: -1,
                message: format!("Failed to generate reward address: {}", e),
                data: None,
            })?;
        
        let reward_addr = wallet::quantum_wallet::Address::from_str(&reward_address)
            .map_err(|e| JsonRpcError {
                code: -1,
                message: format!("Invalid reward address: {}", e),
                data: None,
            })?;
        
        // Generate treasury address
        let treasury_address = wallet.generate_new_address(Some("environmental_treasury".to_string()))
            .map_err(|e| JsonRpcError {
                code: -1,
                message: format!("Failed to generate treasury address: {}", e),
                data: None,
            })?;
        
        let treasury_addr = wallet::quantum_wallet::Address::from_str(&treasury_address)
            .map_err(|e| JsonRpcError {
                code: -1,
                message: format!("Invalid treasury address: {}", e),
                data: None,
            })?;
        
        drop(wallet); // Release wallet lock

        // Generate block template
        use crate::mining::template::BlockTemplate;
        let template = BlockTemplate::generate(
            node.chain_state(),
            node.mempool(),
            &reward_addr,
            &treasury_addr,
        ).map_err(|e| JsonRpcError {
            code: -1,
            message: format!("Failed to generate template: {}", e),
            data: None,
        })?;

        // Build block from template (nonce will be set during mining)
        let block = template.to_block(0);

        // Mine the block
        use crate::mining::mine_block_simple;
        let mined_block = mine_block_simple(block)
            .map_err(|e| JsonRpcError {
                code: -1,
                message: format!("Mining failed: {}", e),
                data: None,
            })?;

        // Verify block meets target
        if !mined_block.header().meets_target() {
            return Err(JsonRpcError {
                code: -25,
                message: "Mined block does not meet difficulty target".to_string(),
                data: None,
            });
        }

        let block_hash = mined_block.hash();
        tracing::info!(
            "Successfully mined block {} at height {} with hash {}",
            i + 1,
            mined_block.height(),
            hex::encode(&block_hash[..8])
        );

        // Add block to chain state.
        // Use spawn_blocking so the std::sync::RwLock write guard is never held
        // across the .await inside add_block on the async actix worker thread
        // (mirrors the discipline in node.rs / network/mod.rs).
        let chain_state = node.chain_state();
        let block_for_add = mined_block.clone();
        // The guard is intentionally held across the await inside a dedicated
        // spawn_blocking thread, so it cannot stall an actix worker's executor.
        let add_result = tokio::task::spawn_blocking(move || {
            tokio::runtime::Handle::current().block_on(async move {
                match chain_state.write() {
                    Ok(mut chain) => chain.add_block(&block_for_add).await,
                    Err(e) => Err(crate::storage::StorageError::DatabaseError(format!(
                        "Chain state lock poisoned: {}",
                        e
                    ))),
                }
            })
        })
        .await;
        add_result
            .map_err(|e| JsonRpcError {
                code: -1,
                message: format!("Block processing task failed: {}", e),
                data: None,
            })?
            .map_err(|e| JsonRpcError {
                code: -25,
                message: format!("Failed to add block to chain: {}", e),
                data: None,
            })?;

        // Scan block for wallet transactions
        let wallet_manager = node.wallet_manager();
        if let Ok(wallet) = wallet_manager.write() {
            if let Err(e) = wallet.scan_block(&mined_block) {
                tracing::warn!("Failed to scan block for wallet: {}", e);
            }
        }

        // Store block in database
        node.storage().insert_block(&mined_block)
            .map_err(|e| JsonRpcError {
                code: -1,
                message: format!("Failed to store block: {}", e),
                data: None,
            })?;

        // Broadcast block to network
        tracing::info!("Broadcasting mined block {} to network", hex::encode(&block_hash[..8]));
        node.network().broadcast_block(&mined_block);
        tracing::info!("Block {} broadcast complete", hex::encode(&block_hash[..8]));

        block_hashes.push(hex::encode(block_hash));
    }

    tracing::info!("Successfully generated {} blocks", num_blocks);
    Ok(Value::Array(block_hashes.into_iter().map(Value::String).collect()))
}

#[cfg(not(feature = "testnet"))]
async fn generate_blocks(
    _params: Value,
    _node: web::Data<Arc<ApiFacade>>,
) -> Result<Value, JsonRpcError> {
        Err(JsonRpcError {
        code: ErrorCode::MethodNotFound as i32,
        message: "generate method is only available in testnet mode".to_string(),
            data: None,
        })
}

// Helper functions

/// Format transaction as JSON
fn format_transaction(tx: &supernova_core::types::transaction::Transaction) -> Value {
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

/// Best-effort total on-disk size (in bytes) of the storage directory tree.
///
/// Walks the database directory recursively and sums regular-file lengths.
/// Any entry that cannot be read (permissions, races with compaction) is
/// skipped rather than failing the RPC, so the returned value is a lower
/// bound rather than an error.
fn directory_size_on_disk(path: &std::path::Path) -> u64 {
    let metadata = match std::fs::symlink_metadata(path) {
        Ok(m) => m,
        Err(_) => return 0,
    };

    if metadata.is_file() {
        return metadata.len();
    }

    if !metadata.is_dir() {
        // Skip symlinks and special files to avoid cycles / double counting.
        return 0;
    }

    let entries = match std::fs::read_dir(path) {
        Ok(entries) => entries,
        Err(_) => return 0,
    };

    let mut total: u64 = 0;
    for entry in entries.flatten() {
        total = total.saturating_add(directory_size_on_disk(&entry.path()));
    }
    total
}

/// Get environmental metrics
async fn get_environmental_metrics(
    _params: Value,
    node: web::Data<Arc<ApiFacade>>,
) -> Result<Value, JsonRpcError> {
    environmental_metrics_json(&node.environmental())
}

/// Build the environmental-metrics JSON payload from real monitor telemetry.
///
/// Mirrors `environmental_info_json`: sourced from the deadlock-free
/// `get_carbon_footprint` path over a 1-hour window so `totalEmissions`,
/// `carbonOffsets` (gross minus net), `netCarbon`, `renewablePercentage`, and
/// `isCarbonNegative` reflect the node's measured resource usage rather than
/// hardcoded zeros. `treasuryBalance` and `greenMiners` have no telemetry
/// source wired into `ApiFacade` yet, so they are reported as JSON `null`
/// (unknown) instead of a fabricated `0`, keeping the endpoint honest for a
/// carbon-negative-branded chain.
fn environmental_metrics_json(
    monitor: &crate::environmental::EnvironmentalMonitor,
) -> Result<Value, JsonRpcError> {
    const PERIOD_SECS: u64 = 3600;

    let carbon = monitor
        .get_carbon_footprint(PERIOD_SECS, true)
        .map_err(|e| JsonRpcError {
            code: ErrorCode::InternalError as i32,
            message: format!("Failed to get environmental metrics: {}", e),
            data: None,
        })?;

    let total_emissions = carbon.total_emissions_g;
    let net_emissions = carbon.net_emissions_g;
    let total_offsets = (total_emissions - net_emissions).max(0.0);

    Ok(json!({
        "totalEmissions": total_emissions,
        "carbonOffsets": total_offsets,
        "netCarbon": net_emissions,
        "renewablePercentage": carbon.renewable_percentage,
        "treasuryBalance": Value::Null,
        "isCarbonNegative": net_emissions < 0.0,
        "greenMiners": Value::Null,
        "lastUpdated": carbon.timestamp,
    }))
}

/// Get environmental information
async fn get_environmental_info(
    _params: Value,
    node: web::Data<Arc<ApiFacade>>,
) -> Result<Value, JsonRpcError> {
    environmental_info_json(&node.environmental())
}

/// Build the environmental-info JSON payload from real monitor telemetry.
///
/// Reports over a 1-hour window so `carbonIntensity` (g CO2e/kWh),
/// `totalEmissions`/`netEmissions` (grams), `totalOffsets` (grams, the
/// difference between gross and net), `greenMining` (renewable-mining bonus
/// %), and `carbonNegative` all reflect the node's measured resource usage
/// rather than hardcoded zeros. Split out from the async handler so the pure
/// mapping is unit-testable without standing up a full node.
///
/// Deliberately sourced from `get_carbon_footprint` (energy + emissions) rather
/// than `get_environmental_impact`: the latter also invokes
/// `get_resource_utilization`, whose `calculate_network_usage` helper re-locks
/// the monitor's `system` mutex while it is already held and self-deadlocks.
/// The carbon footprint already carries every field this endpoint returns, so
/// this path stays deadlock-free for the API worker thread.
fn environmental_info_json(
    monitor: &crate::environmental::EnvironmentalMonitor,
) -> Result<Value, JsonRpcError> {
    const PERIOD_SECS: u64 = 3600;

    let carbon = monitor
        .get_carbon_footprint(PERIOD_SECS, true)
        .map_err(|e| JsonRpcError {
            code: ErrorCode::InternalError as i32,
            message: format!("Failed to get environmental info: {}", e),
            data: None,
        })?;

    let total_emissions = carbon.total_emissions_g;
    let net_emissions = carbon.net_emissions_g;
    let total_offsets = (total_emissions - net_emissions).max(0.0);

    // Green-mining bonus mirrors EnvironmentalMonitor::calculate_green_mining_bonus:
    // renewable-share thresholds map to a percentage mining incentive.
    let green_mining = if carbon.renewable_percentage >= 75.0 {
        10.0
    } else if carbon.renewable_percentage >= 50.0 {
        5.0
    } else {
        0.0
    };

    Ok(json!({
        "carbonIntensity": carbon.intensity,
        "greenMining": green_mining,
        "carbonNegative": net_emissions < 0.0,
        "totalEmissions": total_emissions,
        "totalOffsets": total_offsets,
        "netEmissions": net_emissions,
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

    // Source carbon intensity and renewable-mining share from the environmental
    // monitor's measured 1-hour footprint. `None` (rendered as JSON null) when
    // telemetry is unavailable, never a fabricated 0.0. Uses get_carbon_footprint
    // (not get_environmental_impact, which self-deadlocks the monitor mutex).
    let carbon = node
        .environmental()
        .get_carbon_footprint(3600, true)
        .ok()
        .map(|c| (c.intensity, c.renewable_percentage));

    // Read node config once for both the network id and the real quantum-security
    // state, so quantumSecurityLevel reflects actual configuration rather than a
    // hardcoded constant.
    let config_handle = node.config();
    let config = config_handle.read().map_err(|e| JsonRpcError {
        code: ErrorCode::InternalError as i32,
        message: format!("Failed to read config: {}", e),
        data: None,
    })?;
    let network_id = config.network.network_id.clone();
    let enable_quantum_security = config.node.enable_quantum_security;
    drop(config);

    Ok(network_stats_json(
        height,
        network_hashrate,
        difficulty,
        connections,
        mempool_size,
        carbon,
        enable_quantum_security,
        &network_id,
    ))
}

/// Assemble the `getnetworkstats` JSON payload from already-resolved inputs.
///
/// Split out from the async handler so the honest field mapping is unit-testable
/// without standing up a node. Truthfulness guarantees enforced here:
/// - `mempoolSize` is the current mempool size, accurately labelled (the old
///   payload mislabelled this as `transactions24h`).
/// - `carbonIntensity` / `greenMiningPercentage` are the monitor's measured
///   values, or JSON `null` when telemetry is unavailable — never a fabricated
///   `0.0`.
/// - `quantumSecurityLevel` reflects the real `enable_quantum_security` config
///   flag instead of a hardcoded constant.
#[allow(clippy::too_many_arguments)]
fn network_stats_json(
    height: u64,
    network_hashrate: u64,
    difficulty: f64,
    connections: u32,
    mempool_size: usize,
    carbon: Option<(f64, f64)>,
    enable_quantum_security: bool,
    network_id: &str,
) -> Value {
    let (carbon_intensity, green_mining_percentage) = match carbon {
        Some((intensity, renewable)) => (json!(intensity), json!(renewable)),
        None => (Value::Null, Value::Null),
    };
    let quantum_security_level = if enable_quantum_security {
        "HIGH"
    } else {
        "NONE"
    };

    json!({
        "blockHeight": height,
        "hashrate": network_hashrate.to_string(),
        "difficulty": difficulty.to_string(),
        "nodes": connections,
        "mempoolSize": mempool_size,
        "carbonIntensity": carbon_intensity,
        "greenMiningPercentage": green_mining_percentage,
        "quantumSecurityLevel": quantum_security_level,
        "networkId": network_id,
    })
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

/// Send NOVA to an address
async fn send_to_address(
    params: Value,
    node: web::Data<Arc<ApiFacade>>,
) -> Result<Value, JsonRpcError> {
    // Parse parameters: address, amount, optional comment
    let (address_str, amount_nova, comment) = match params {
        Value::Array(ref arr) => {
            let address = arr.get(0)
                .and_then(|v| v.as_str())
                .map(String::from)
                .ok_or_else(|| JsonRpcError {
        code: ErrorCode::InvalidParams as i32,
                    message: "Missing address parameter".to_string(),
        data: None,
    })?;

            let amount = arr.get(1)
                .and_then(|v| v.as_f64())
                .ok_or_else(|| JsonRpcError {
        code: ErrorCode::InvalidParams as i32,
                    message: "Missing or invalid amount parameter".to_string(),
        data: None,
    })?;

            let comment = arr.get(2).and_then(|v| v.as_str()).map(String::from);
            
            (address, amount, comment)
        }
        _ => {
            return Err(JsonRpcError {
                code: ErrorCode::InvalidParams as i32,
                message: "Invalid parameters for sendtoaddress".to_string(),
                data: None,
            });
        }
    };
    
    // Validate address format
    use wallet::quantum_wallet::Address;
    let recipient_address = Address::from_str(&address_str)
        .map_err(|e| JsonRpcError {
            code: -5, // Invalid address
            message: format!("Invalid address: {}", e),
        data: None,
    })?;

    // Convert amount to attonovas
    if amount_nova <= 0.0 {
        return Err(JsonRpcError {
            code: -3, // Invalid amount
            message: "Amount must be positive".to_string(),
            data: None,
        });
    }
    
    let amount_attonovas = (amount_nova * 100_000_000.0) as u64;
    
    // Get wallet manager
    let wallet_manager = node.wallet_manager();
    let wallet = wallet_manager.read()
        .map_err(|_| JsonRpcError {
            code: -13,
            message: "Wallet lock poisoned".to_string(),
            data: None,
        })?;
    
    // Check balance
    let balance = wallet.get_balance(1)
        .map_err(|e| JsonRpcError {
            code: -1,
            message: format!("Failed to check balance: {}", e),
            data: None,
        })?;
    
    // Estimate fee for 1 input, 2 outputs (payment + change)
    use wallet::quantum_wallet::TransactionBuilder;
    let estimated_fee = TransactionBuilder::estimate_transaction_size(1, 2) as u64 * 1000; // 1000 attonovas/byte
    
    let total_needed = amount_attonovas.checked_add(estimated_fee)
        .ok_or_else(|| JsonRpcError {
            code: -3,
            message: "Amount overflow".to_string(),
            data: None,
        })?;
    
    if balance < total_needed {
        return Err(JsonRpcError {
            code: -6, // Insufficient funds
            message: format!(
                "Insufficient funds: need {} NOVA (amount + fee), have {} NOVA",
                total_needed as f64 / 100_000_000.0,
                balance as f64 / 100_000_000.0
            ),
            data: None,
        });
    }
    
    // Get available UTXOs
    let utxos = wallet.list_unspent(1, 9999999, None)
        .map_err(|e| JsonRpcError {
            code: -1,
            message: format!("Failed to get UTXOs: {}", e),
            data: None,
        })?;
    
    if utxos.is_empty() {
        return Err(JsonRpcError {
            code: -6,
            message: "No spendable UTXOs available".to_string(),
            data: None,
        });
    }
    
    // Build transaction
    use wallet::quantum_wallet::BuilderConfig;
    let builder_config = BuilderConfig {
        fee_rate: 1000, // 1000 attonovas per byte
        ..Default::default()
    };
    
    let mut builder = TransactionBuilder::new(wallet.keystore(), builder_config);
    
    // Add output to recipient
    builder.add_output(recipient_address, amount_attonovas)
        .map_err(|e| JsonRpcError {
            code: -3,
            message: format!("Failed to add output: {}", e),
            data: None,
        })?;
    
    // Generate change address
    let change_address = wallet.generate_new_address(Some("change".to_string()))
        .map_err(|e| JsonRpcError {
            code: -13,
            message: format!("Failed to generate change address: {}", e),
            data: None,
        })?;
    
    let change_addr = Address::from_str(&change_address)
        .map_err(|e| JsonRpcError {
            code: -1,
            message: format!("Invalid change address: {}", e),
            data: None,
        })?;
    
    builder.set_change_address(change_addr);
    
    // Select coins
    builder.select_coins(&utxos)
        .map_err(|e| JsonRpcError {
            code: -6,
            message: format!("Coin selection failed: {}", e),
            data: None,
        })?;
    
    // Build and sign transaction
    let transaction = builder.build_and_sign()
        .map_err(|e| JsonRpcError {
            code: -1,
            message: format!("Failed to build transaction: {}", e),
            data: None,
        })?;
    
    // Submit to mempool and broadcast
    let txid = wallet.submit_transaction_to_mempool(transaction)
        .map_err(|e| JsonRpcError {
            code: match e {
                crate::wallet_manager::WalletManagerError::TransactionError(_) => -25,
                _ => -1,
            },
            message: format!("Failed to submit transaction: {}", e),
            data: None,
        })?;
    
    tracing::info!(
        "Sent transaction {} ({} NOVA to {}){}",
        hex::encode(&txid[..8]),
        amount_nova,
        address_str,
        comment.as_ref().map(|c| format!(" - {}", c)).unwrap_or_default()
    );
    
    Ok(Value::String(hex::encode(txid)))
}

/// Add test UTXO (testnet only)
#[cfg(feature = "testnet")]
async fn add_test_utxo(
    params: Value,
    node: web::Data<Arc<ApiFacade>>,
) -> Result<Value, JsonRpcError> {
    // Parse parameters: address, amount
    let (address_str, amount_nova) = match params {
        Value::Array(ref arr) => {
            let address = arr.get(0)
                .and_then(|v| v.as_str())
                .map(String::from)
                .ok_or_else(|| JsonRpcError {
                    code: ErrorCode::InvalidParams as i32,
                    message: "Missing address parameter".to_string(),
                    data: None,
                })?;
            
            let amount = arr.get(1)
                .and_then(|v| v.as_f64())
                .ok_or_else(|| JsonRpcError {
                    code: ErrorCode::InvalidParams as i32,
                    message: "Missing or invalid amount parameter".to_string(),
                    data: None,
                })?;
            
            (address, amount)
        }
        _ => {
            return Err(JsonRpcError {
                code: ErrorCode::InvalidParams as i32,
                message: "Invalid parameters".to_string(),
                data: None,
            });
        }
    };
    
    // Convert to attonovas
    let amount_attonovas = (amount_nova * 100_000_000.0) as u64;
    
    // Generate fake txid
    let mut fake_txid = [0u8; 32];
    use rand::RngCore;
    rand::thread_rng().fill_bytes(&mut fake_txid);
    
    // Get wallet manager
    let wallet_manager = node.wallet_manager();
    let wallet = wallet_manager.read()
        .map_err(|_| JsonRpcError {
            code: -13,
            message: "Wallet lock poisoned".to_string(),
            data: None,
        })?;
    
    // Add test UTXO
    wallet.add_test_utxo(&address_str, amount_attonovas, fake_txid, 0)
        .map_err(|e| JsonRpcError {
            code: -1,
            message: format!("Failed to add test UTXO: {}", e),
            data: None,
        })?;
    
    Ok(json!({
        "address": address_str,
        "amount": amount_nova,
        "txid": hex::encode(fake_txid),
        "vout": 0,
    }))
}

#[cfg(not(feature = "testnet"))]
async fn add_test_utxo(
    _params: Value,
    _node: web::Data<Arc<ApiFacade>>,
) -> Result<Value, JsonRpcError> {
    Err(JsonRpcError {
        code: ErrorCode::MethodNotFound as i32,
        message: "Method only available in testnet mode".to_string(),
        data: None,
    })
}

/// Add a peer node manually for P2P network management
async fn add_node(
    params: Value,
    node: web::Data<Arc<ApiFacade>>,
) -> Result<Value, JsonRpcError> {
    // Parse multiaddr from params
    let multiaddr_str = match params {
        Value::Array(ref arr) if !arr.is_empty() => {
            arr.get(0)
                .and_then(|v| v.as_str())
                .ok_or_else(|| JsonRpcError {
                    code: ErrorCode::InvalidParams as i32,
                    message: "Multiaddr string required".to_string(),
                    data: None,
                })?
        }
        Value::String(ref s) => s.as_str(),
        _ => {
            return Err(JsonRpcError {
                code: ErrorCode::InvalidParams as i32,
                message: "Invalid parameters for addnode".to_string(),
                data: None,
            });
        }
    };

    tracing::info!("Adding peer node via RPC: {}", multiaddr_str);
    
    // Use network proxy to broadcast connection command
    node.network().dial_peer_str(multiaddr_str).await
        .map_err(|e| JsonRpcError {
            code: -1,
            message: format!("Failed to dial peer: {}", e),
            data: None,
        })?;
    
    Ok(json!({
        "success": true,
        "message": format!("Dialing peer: {}", multiaddr_str)
    }))
}

#[cfg(test)]
mod send_raw_transaction_tests {
    use super::*;
    use crate::config::NodeConfig;
    use crate::node::Node;

    async fn create_test_facade() -> web::Data<Arc<ApiFacade>> {
        let mut config = NodeConfig::default();
        // NodeConfig::default()'s network settings are internally
        // inconsistent (max_inbound_connections > max_peers), which trips
        // NodeConfig::validate() during Node::new(). This is pre-existing,
        // unrelated to this handler; normalize it here so this test
        // exercises send_raw_transaction rather than an unrelated default
        // config bug.
        config.network.max_peers = config
            .network
            .max_peers
            .max(config.network.max_inbound_connections)
            .max(config.network.max_outbound_connections);
        // NodeConfig::default() points storage at the fixed relative path
        // "./data", which causes sled lock contention when multiple tests
        // in this module run concurrently. Give each test its own on-disk
        // temp directory instead.
        let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
        config.storage.db_path = temp_dir.path().join("data");
        let node = Node::new(config)
            .await
            .expect("Failed to create test node");
        // Keep the temp dir alive for the lifetime of the node/facade by
        // leaking it; test processes are short-lived so this is fine.
        std::mem::forget(temp_dir);
        let facade = ApiFacade::new(&node).expect("Failed to create ApiFacade");
        web::Data::new(Arc::new(facade))
    }

    // All assertions share a single node/facade. `create_test_facade` starts a
    // real `Node`, which binds a fixed P2P TCP port; running several such tests
    // in parallel (or leaving leaked nodes holding the port) would collide on
    // that port. Exercising every case against one facade keeps full coverage
    // of `send_raw_transaction` while binding the port exactly once.
    #[tokio::test]
    async fn send_raw_transaction_behaviour() {
        let facade = create_test_facade().await;

        // Garbage hex decodes fine as bytes but fails transaction
        // deserialization; either way the handler must no longer return
        // the old placeholder MethodNotFound error.
        let params = json!(["deadbeef"]);
        let result = send_raw_transaction(params, facade.clone()).await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_ne!(
            err.code,
            ErrorCode::MethodNotFound as i32,
            "send_raw_transaction must be implemented, not a MethodNotFound stub"
        );
        assert_eq!(err.code, ErrorCode::InvalidParams as i32);

        // Non-hex input is rejected at the hex-decode stage.
        let params = json!(["not-valid-hex!!"]);
        let result = send_raw_transaction(params, facade.clone()).await;
        let err = result.unwrap_err();
        assert_eq!(err.code, ErrorCode::InvalidParams as i32);
        assert!(err.message.to_lowercase().contains("hex"));

        // Missing params are rejected before any decoding.
        let result = send_raw_transaction(Value::Null, facade).await;
        let err = result.unwrap_err();
        assert_eq!(err.code, ErrorCode::InvalidParams as i32);
    }
}

#[cfg(test)]
mod directory_size_tests {
    use super::directory_size_on_disk;
    use std::fs;
    use std::io::Write;

    #[test]
    fn sums_nested_file_sizes_and_ignores_missing_paths() {
        // A path that does not exist reports zero rather than erroring.
        let missing = std::path::Path::new("/definitely/not/a/real/path/for/supernova");
        assert_eq!(directory_size_on_disk(missing), 0);

        let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
        let root = temp_dir.path();

        // Top-level file: 10 bytes.
        let mut f1 = fs::File::create(root.join("a.dat")).unwrap();
        f1.write_all(&[0u8; 10]).unwrap();

        // Nested directory with a 25-byte file.
        let nested = root.join("sub/deeper");
        fs::create_dir_all(&nested).unwrap();
        let mut f2 = fs::File::create(nested.join("b.dat")).unwrap();
        f2.write_all(&[0u8; 25]).unwrap();

        // Empty directory contributes nothing.
        fs::create_dir_all(root.join("empty")).unwrap();

        assert_eq!(directory_size_on_disk(root), 35);

        // A single regular file reports its own length.
        assert_eq!(directory_size_on_disk(&root.join("a.dat")), 10);
    }
}

#[cfg(test)]
mod environmental_info_tests {
    use super::*;
    use crate::environmental::EnvironmentalMonitor;

    // getenvironmentalinfo must surface real monitor telemetry, not the old
    // hardcoded all-zero placeholder. A real monitor measures nonzero energy
    // use, so emissions and carbon intensity must both be positive.
    #[test]
    fn environmental_info_returns_real_data() {
        let monitor = EnvironmentalMonitor::new();
        let value = environmental_info_json(&monitor).expect("should produce info");

        let carbon_intensity = value["carbonIntensity"]
            .as_f64()
            .expect("carbonIntensity present");
        let total_emissions = value["totalEmissions"]
            .as_f64()
            .expect("totalEmissions present");
        let net_emissions = value["netEmissions"]
            .as_f64()
            .expect("netEmissions present");
        let total_offsets = value["totalOffsets"]
            .as_f64()
            .expect("totalOffsets present");

        // Real telemetry, not the stubbed zeros.
        assert!(
            carbon_intensity > 0.0,
            "carbonIntensity must reflect real emission factor, got {carbon_intensity}"
        );
        assert!(
            total_emissions > 0.0,
            "totalEmissions must be measured from real energy use, got {total_emissions}"
        );
        // Offsets are the nonnegative gap between gross and net emissions.
        assert!(total_offsets >= 0.0, "totalOffsets must be nonnegative");
        assert!(
            (total_emissions - net_emissions - total_offsets).abs() < 1e-6,
            "totalOffsets must equal gross minus net emissions"
        );
        // greenMining and carbonNegative fields must be present and typed.
        assert!(value["greenMining"].is_number(), "greenMining present");
        assert!(value["carbonNegative"].is_boolean(), "carbonNegative present");
    }

    // getenvironmentalmetrics must surface real monitor telemetry, not the old
    // hardcoded all-zero placeholder. A real monitor measures nonzero energy
    // use, so total emissions must be positive; fields with no telemetry source
    // (treasuryBalance/greenMiners) must be null rather than a fabricated zero.
    #[test]
    fn environmental_metrics_returns_real_data() {
        let monitor = EnvironmentalMonitor::new();
        let value = environmental_metrics_json(&monitor).expect("should produce metrics");

        let total_emissions = value["totalEmissions"]
            .as_f64()
            .expect("totalEmissions present");
        let net_carbon = value["netCarbon"].as_f64().expect("netCarbon present");
        let carbon_offsets = value["carbonOffsets"]
            .as_f64()
            .expect("carbonOffsets present");

        // Real telemetry, not the stubbed zeros.
        assert!(
            total_emissions > 0.0,
            "totalEmissions must be measured from real energy use, got {total_emissions}"
        );
        assert!(carbon_offsets >= 0.0, "carbonOffsets must be nonnegative");
        assert!(
            (total_emissions - net_carbon - carbon_offsets).abs() < 1e-6,
            "carbonOffsets must equal gross minus net emissions"
        );
        assert!(
            value["renewablePercentage"].is_number(),
            "renewablePercentage present"
        );
        assert!(
            value["isCarbonNegative"].is_boolean(),
            "isCarbonNegative present"
        );
        // No telemetry source is wired for these yet: honest null, not a fake 0.
        assert!(
            value["treasuryBalance"].is_null(),
            "treasuryBalance must be null when unavailable"
        );
        assert!(
            value["greenMiners"].is_null(),
            "greenMiners must be null when unavailable"
        );
        // lastUpdated must be a real timestamp, not the stubbed 0.
        assert!(
            value["lastUpdated"].as_u64().expect("lastUpdated present") > 0,
            "lastUpdated must reflect a real measurement time"
        );
    }
}

#[cfg(test)]
mod network_stats_tests {
    use super::*;

    // getnetworkstats must not fabricate values. When environmental telemetry is
    // available the carbon fields carry the measured numbers; the mempool count is
    // labelled honestly; and quantumSecurityLevel reflects the real config flag.
    #[test]
    fn network_stats_reports_real_values_when_available() {
        let value = network_stats_json(
            42,             // height
            123_456,        // hashrate
            2.5,            // difficulty
            7,              // connections
            9,              // mempool size
            Some((480.0, 65.0)), // carbon intensity, renewable %
            true,           // quantum security enabled
            "testnet",
        );

        assert_eq!(value["blockHeight"], json!(42));
        assert_eq!(value["nodes"], json!(7));
        // Accurately labelled mempool size, and no fabricated 24h field.
        assert_eq!(value["mempoolSize"], json!(9));
        assert!(
            value.get("transactions24h").is_none(),
            "must not expose a fabricated 24h transaction count"
        );
        // Carbon fields are the measured monitor values, not hardcoded zeros.
        assert_eq!(value["carbonIntensity"], json!(480.0));
        assert_eq!(value["greenMiningPercentage"], json!(65.0));
        assert_eq!(value["quantumSecurityLevel"], json!("HIGH"));
        assert_eq!(value["networkId"], json!("testnet"));
    }

    // When telemetry is unavailable the carbon fields must be null (unknown),
    // never a fabricated 0.0; quantumSecurityLevel tracks the disabled flag.
    #[test]
    fn network_stats_nulls_carbon_when_telemetry_missing() {
        let value = network_stats_json(0, 0, 1.0, 0, 0, None, false, "dev");

        assert!(
            value["carbonIntensity"].is_null(),
            "carbonIntensity must be null when unknown, not fabricated"
        );
        assert!(
            value["greenMiningPercentage"].is_null(),
            "greenMiningPercentage must be null when unknown, not fabricated"
        );
        assert_eq!(value["quantumSecurityLevel"], json!("NONE"));
    }
}

#[cfg(test)]
mod mining_info_tests {
    use super::*;

    // getmininginfo must not serve the old hardcoded `4000`/`10` placeholders.
    // With an empty mempool the next block is just the coinbase: zero tx weight
    // and a transaction count of 1.
    #[test]
    fn current_block_stats_empty_mempool_is_coinbase_only() {
        let (weight, tx_count) = current_block_stats(&[]);
        assert_eq!(weight, 0, "no mempool txs means zero selected-tx weight");
        assert_eq!(tx_count, 1, "empty mempool still yields a coinbase-only block");
    }

    // Weight is BIP141 units (serialized size * 4) summed over selected txs, and
    // the count includes the coinbase (+1). Values come from real sizes, not a
    // fixed placeholder.
    #[test]
    fn current_block_stats_derives_from_real_sizes() {
        let (weight, tx_count) = current_block_stats(&[250, 500]);
        assert_eq!(weight, (250 + 500) * 4, "weight = total serialized size * 4");
        assert_eq!(tx_count, 3, "two mempool txs plus the coinbase");
    }

    // Selection mirrors the template's `take(100)` policy: at most 100 mempool
    // transactions contribute, so both weight and count are capped accordingly.
    #[test]
    fn current_block_stats_caps_at_template_limit() {
        let sizes = vec![10usize; 150];
        let (weight, tx_count) = current_block_stats(&sizes);
        assert_eq!(weight, 100 * 10 * 4, "only the first 100 txs count toward weight");
        assert_eq!(tx_count, 101, "100 selected mempool txs plus the coinbase");
    }
}