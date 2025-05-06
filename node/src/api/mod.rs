//! SuperNova Blockchain API
//!
//! This module implements a comprehensive REST API for interacting with the SuperNova blockchain,
//! providing endpoints for blocks, transactions, wallet operations, network information,
//! environmental data, and Lightning Network functionality.

mod error;
mod types;
mod server;
pub mod routes;
pub mod middleware;
pub mod docs;
pub mod blockchain_api;
pub mod wallet_api;
pub mod mempool_api;
pub mod network_api;
pub mod environmental_api;
pub mod lightning_api;
pub mod blockchain;
pub mod wallet;
pub mod node;
pub mod metrics;

pub use error::{ApiError, Result};
pub use types::*;
pub use server::ApiServer;

use crate::node::Node;
use std::sync::Arc;

/// API version
pub const API_VERSION: &str = "v1";

/// Creates a new API server instance
pub fn create_api_server(node: Arc<Node>, bind_address: &str, port: u16) -> ApiServer {
    ApiServer::new(node, bind_address, port)
}

impl ApiServer {
    /// Configure API routes
    pub fn configure_routes(cfg: &mut web::ServiceConfig) {
        cfg.service(
            web::scope("/api/v1")
                .configure(blockchain::configure)
                .configure(wallet::configure)
                .configure(node::configure)
                .configure(metrics::configure)
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_api_version() {
        assert_eq!(API_VERSION, "v1");
    }
} 