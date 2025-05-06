use actix_web::web;
use super::{
    blockchain,
    wallet,
    node,
    metrics
};

/// Configure all API routes
pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/api/v1")
            .configure(blockchain::configure)
            .configure(wallet::configure)
            .configure(node::configure)
            .configure(metrics::configure)
    );
    
    // Add health check endpoint
    cfg.route("/health", web::get().to(health_check));
}

/// Health check handler
async fn health_check() -> actix_web::HttpResponse {
    actix_web::HttpResponse::Ok().json(serde_json::json!({
        "status": "ok",
        "version": env!("CARGO_PKG_VERSION"),
    }))
} 