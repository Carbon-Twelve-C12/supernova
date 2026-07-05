//! API routes
//!
//! This module defines the API routes for the supernova blockchain node.

use actix_web::web;
use std::sync::Arc;

pub mod blockchain;
pub mod environmental;
pub mod faucet;
pub mod health;
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
        // JSON-RPC 2.0 API at root (main endpoint)
        .configure(crate::api::jsonrpc::configure)
        // Health check routes (Kubernetes probes)
        .configure(health::configure)
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

    // Legacy health check endpoint (for backwards compatibility)
    cfg.route("/health", web::get().to(health_check_legacy));
}

/// Legacy health check endpoint (for backwards compatibility)
/// Use /health/live and /health/ready for Kubernetes probes
async fn health_check_legacy() -> actix_web::HttpResponse {
    actix_web::HttpResponse::Ok().json(serde_json::json!({
        "status": "ok",
        "version": env!("CARGO_PKG_VERSION"),
        "name": env!("CARGO_PKG_NAME"),
    }))
}

#[cfg(test)]
mod tests {
    use super::configure;
    use actix_web::{test, App};

    /// Every REST module must register its routes so that the live path equals
    /// the `/api/v1/<name>/...` path advertised by the OpenAPI (utoipa)
    /// annotations. A previous double-scope bug registered paths like
    /// `/api/v1/mempool/mempool/info`, making every documented path 404.
    ///
    /// We assert only that the documented path resolves to a route (status is
    /// NOT 404); handlers may return 500 here because no application data is
    /// wired in the test, but a matched route can never return 404.
    #[actix_web::test]
    async fn documented_rest_paths_are_registered() {
        let app = test::init_service(App::new().configure(configure)).await;

        // One representative GET endpoint per module whose configure() was
        // previously wrapped in a redundant inner scope.
        let documented_paths = [
            "/api/v1/mempool/info",
            "/api/v1/network/info",
            "/api/v1/wallet/info",
            "/api/v1/mining/info",
            "/api/v1/environmental/impact",
            "/api/v1/faucet/status",
            "/api/v1/lightning/info",
            // Modules that were always correct (regression guard).
            "/api/v1/blockchain/info",
            "/api/v1/node/info",
        ];

        for path in documented_paths {
            let req = test::TestRequest::get().uri(path).to_request();
            let resp = test::call_service(&app, req).await;
            assert_ne!(
                resp.status().as_u16(),
                404,
                "documented path {} must be registered (got 404)",
                path
            );
        }
    }

    /// The previously-registered doubled paths must NOT exist anymore.
    #[actix_web::test]
    async fn doubled_paths_are_not_registered() {
        let app = test::init_service(App::new().configure(configure)).await;

        let doubled_paths = [
            "/api/v1/mempool/mempool/info",
            "/api/v1/network/network/info",
            "/api/v1/wallet/wallet/info",
            "/api/v1/mining/mining/info",
            "/api/v1/environmental/environmental/impact",
            "/api/v1/faucet/faucet/status",
            "/api/v1/lightning/lightning/info",
        ];

        for path in doubled_paths {
            let req = test::TestRequest::get().uri(path).to_request();
            let resp = test::call_service(&app, req).await;
            assert_eq!(
                resp.status().as_u16(),
                404,
                "stale doubled path {} must not be registered",
                path
            );
        }
    }
}
