//! JSON-RPC types
//!
//! This module defines the data structures for JSON-RPC 2.0 requests and responses.

use serde::{Serialize, Deserialize};
use serde_json::Value;

/// JSON-RPC 2.0 request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcRequest {
    /// JSON-RPC version, must be "2.0"
    pub jsonrpc: String,
    /// Method name
    pub method: String,
    /// Method parameters
    #[serde(default)]
    pub params: Value,
    /// Request ID
    pub id: Value,
}

/// JSON-RPC 2.0 response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcResponse {
    /// JSON-RPC version, always "2.0"
    pub jsonrpc: String,
    /// Response result (only present if no error)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    /// Error information (only present if error)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
    /// Request ID, must match the ID from the request
    pub id: Value,
}

impl JsonRpcResponse {
    /// Create a successful response with a result
    pub fn result(id: Value, result: Value) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            result: Some(result),
            error: None,
            id,
        }
    }

    /// Create an error response
    pub fn error(id: Value, code: ErrorCode, message: String, data: Option<Value>) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            result: None,
            error: Some(JsonRpcError {
                code: code as i32,
                message,
                data,
            }),
            id,
        }
    }
}

/// JSON-RPC error object
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcError {
    /// Error code
    pub code: i32,
    /// Error message
    pub message: String,
    /// Additional error data (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

/// JSON-RPC standard error codes
#[derive(Debug, Clone, Copy)]
#[repr(i32)]
pub enum ErrorCode {
    /// Parse error (-32700)
    ///
    /// Invalid JSON was received by the server.
    /// An error occurred on the server while parsing the JSON text.
    ParseError = -32700,

    /// Invalid Request (-32600)
    ///
    /// The JSON sent is not a valid Request object.
    InvalidRequest = -32600,

    /// Method not found (-32601)
    ///
    /// The method does not exist / is not available.
    MethodNotFound = -32601,

    /// Invalid params (-32602)
    ///
    /// Invalid method parameter(s).
    InvalidParams = -32602,

    /// Internal error (-32603)
    ///
    /// Internal JSON-RPC error.
    InternalError = -32603,

    // Server errors (-32000 to -32099)

    /// Server error (-32000)
    ///
    /// Generic server error
    ServerError = -32000,

    /// Node is syncing (-32001)
    ///
    /// Node is still syncing with the network
    NodeSyncing = -32001,

    /// Blockchain error (-32002)
    ///
    /// Error in blockchain operations
    BlockchainError = -32002,

    /// Transaction error (-32003)
    ///
    /// Error in transaction processing
    TransactionError = -32003,

    /// Wallet error (-32004)
    ///
    /// Error in wallet operations
    WalletError = -32004,

    /// Network error (-32005)
    ///
    /// Error in network operations
    NetworkError = -32005,

    /// Rate limit exceeded (-32006)
    ///
    /// Too many requests from this IP address 
    RateLimitExceeded = -32006,
}

impl From<i32> for ErrorCode {
    fn from(code: i32) -> Self {
        match code {
            -32700 => ErrorCode::ParseError,
            -32600 => ErrorCode::InvalidRequest,
            -32601 => ErrorCode::MethodNotFound,
            -32602 => ErrorCode::InvalidParams,
            -32603 => ErrorCode::InternalError,
            -32000 => ErrorCode::ServerError,
            -32001 => ErrorCode::NodeSyncing,
            -32002 => ErrorCode::BlockchainError,
            -32003 => ErrorCode::TransactionError,
            -32004 => ErrorCode::WalletError,
            -32005 => ErrorCode::NetworkError,
            -32006 => ErrorCode::RateLimitExceeded,
            _ => ErrorCode::InternalError, // Default for unknown codes
        }
    }
}