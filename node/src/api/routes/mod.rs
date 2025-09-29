//! API routes
//!
//! This module defines the API routes for the supernova blockchain node.

use actix_web::web;
use std::sync::Arc;

pub mod blockchain;
pub mod environmental;
pub mod faucet;
pub mod lightning;
pub mod mempool;
pub mod mining;
pub mod network;
pub mod node;
pub mod wallet;

// Type alias for the node data passed to route handlers
pub type NodeData = web::Data<Arc<crate::api_facade::ApiFacade>>;

/// Configure all API routes
pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg
        // Blockchain routes
        .service(web::scope("/api/v1/blockchain").configure(blockchain::configure))
        // Node routes
        .service(web::scope("/api/v1/node").configure(node::configure))
        // Network routes
        .service(web::scope("/api/v1/network").configure(network::configure))
        // Mempool routes
        .service(web::scope("/api/v1/mempool").configure(mempool::configure))
        // Faucet routes
        .service(web::scope("/api/v1/faucet").configure(faucet::configure))
        // Wallet routes
        .service(web::scope("/api/v1/wallet").configure(wallet::configure))
        // Lightning routes
        .service(web::scope("/api/v1/lightning").configure(lightning::configure))
        // Mining routes
        .service(web::scope("/api/v1/mining").configure(mining::configure))
        // Environmental routes
        .service(web::scope("/api/v1/environmental").configure(environmental::configure));

    // Add health check endpoint at root
    cfg.route("/health", web::get().to(health_check));
}

/// Health check endpoint
async fn health_check() -> actix_web::HttpResponse {
    actix_web::HttpResponse::Ok().json(serde_json::json!({
        "status": "ok",
        "version": env!("CARGO_PKG_VERSION"),
        "name": env!("CARGO_PKG_NAME"),
    }))
}
