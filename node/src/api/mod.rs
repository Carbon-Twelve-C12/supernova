//! API module for the SuperNova blockchain node
//!
//! This module provides the HTTP API for interacting with the SuperNova blockchain.

pub mod server;
pub mod routes;
pub mod middleware;
pub mod types;
pub mod error;
pub mod docs;
pub mod v1;
pub mod jsonrpc;

pub use server::ApiServer;
pub use server::ApiConfig;
pub use error::{ApiError, Result};

use crate::node::Node;
use std::sync::Arc;

/// API version
pub const API_VERSION: &str = "v1";

/// Creates a new API server instance
pub fn create_api_server(node: Arc<Node>, bind_address: &str, port: u16) -> ApiServer {
    ApiServer::new(node, bind_address, port)
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_api_version() {
        assert_eq!(API_VERSION, "v1");
    }
} 