//! API routes
//!
//! This module defines the API routes for the SuperNova blockchain node.

pub mod blockchain;
pub mod mempool;
pub mod network;
pub mod mining;
pub mod environmental;
pub mod lightning;
pub mod node;
pub mod wallet;

use actix_web::web;

/// Configure all API routes
pub fn configure(cfg: &mut web::ServiceConfig) {
    // Configure API v1 routes
    cfg.service(
        web::scope("/api/v1")
            .configure(blockchain::configure)
            .configure(mempool::configure)
            .configure(network::configure)
            .configure(mining::configure)
            .configure(environmental::configure)
            .configure(lightning::configure)
            .configure(node::configure)
            .configure(wallet::configure)
    );
    
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