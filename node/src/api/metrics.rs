use actix_web::{web, HttpResponse, Responder};
use serde::{Serialize, Deserialize};
use std::sync::Arc;
use crate::node::Node;

/// Performance metrics response
#[derive(Debug, Serialize, Deserialize)]
pub struct PerformanceMetricsResponse {
    /// Request was successful
    pub success: bool,
    /// Metrics data
    pub data: serde_json::Value,
}

/// Get performance metrics
pub async fn get_performance_metrics(node: web::Data<Arc<Node>>) -> impl Responder {
    let metrics = node.get_performance_metrics();
    
    let response = PerformanceMetricsResponse {
        success: true,
        data: metrics,
    };
    
    HttpResponse::Ok().json(response)
}

/// Configure metrics routes
pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/metrics")
            .route("/performance", web::get().to(get_performance_metrics))
    );
} 