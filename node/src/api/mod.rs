//! supernova Blockchain API
//!
//! This module implements a comprehensive REST API for interacting with the Supernova blockchain,
//! providing endpoints for blocks, transactions, wallet operations, network information,
//! environmental data, and Lightning Network functionality.

pub mod docs;
mod error;
pub mod middleware;
pub mod rate_limiter;   // API rate limiting
pub mod routes;
mod server;
pub mod types;
// pub mod blockchain_api;  // Missing file
// pub mod wallet_api;      // Missing file
// pub mod mempool_api;     // Missing file
// pub mod network_api;     // Missing file
// pub mod environmental_api; // Missing file
pub mod lightning_api;
// pub mod blockchain;      // Missing file
// pub mod wallet;          // Missing file
// pub mod node;            // Missing file
pub mod faucet_wrapper;
pub mod metrics;
pub mod jsonrpc;         // JSON-RPC 2.0 API enabled

pub use error::{ApiError, Result};
pub use server::{ApiConfig, ApiServer};
pub use types::*;
pub use rate_limiter::{ApiRateLimiter, ApiRateLimitConfig, ApiRateLimitStats};

use crate::node::Node;
use std::sync::Arc;

/// API version
pub const API_VERSION: &str = "v1";

/// Creates a new API server instance from the operator's configuration.
///
/// Returns an error when the `ApiFacade` cannot be constructed (for example
/// when the node lacks a wallet manager and the fallback initialization
/// fails). Callers surface this up the startup chain.
pub fn create_api_server(
    node: Arc<Node>,
    config: ApiConfig,
) -> std::result::Result<ApiServer, crate::node::NodeError> {
    ApiServer::new(node, config)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_api_version() {
        assert_eq!(API_VERSION, "v1");
    }
}
