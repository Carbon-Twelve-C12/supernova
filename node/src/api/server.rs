//! API server implementation
//!
//! This module implements the HTTP server for the SuperNova API, 
//! handling requests, routing, and middleware.

use actix_web::{web, App, HttpServer, middleware, dev::Server};
use actix_cors::Cors;
use std::sync::Arc;
use std::net::{IpAddr, SocketAddr};
use std::str::FromStr;
use serde::{Deserialize, Serialize};
use utoipa_swagger_ui::SwaggerUi;
use tracing::{info, error};

use crate::node::Node;
use crate::metrics::ApiMetrics;
use super::routes;
use super::docs::ApiDoc;
use super::middleware::{auth, rate_limiting, logging};

/// Configuration options for the API server
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiConfig {
    /// Bind address for the API server
    pub bind_address: String,
    /// Port for the API server
    pub port: u16,
    /// Enable OpenAPI documentation
    pub enable_docs: bool,
    /// CORS allowed origins
    pub cors_allowed_origins: Vec<String>,
    /// Rate limiting settings (requests per minute)
    pub rate_limit: Option<u32>,
    /// Enable authentication
    pub enable_auth: bool,
    /// API keys (only used if enable_auth is true)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_keys: Option<Vec<String>>,
    /// Detailed logging
    pub detailed_logging: bool,
    /// Maximum JSON payload size in megabytes
    pub max_json_payload_size: usize,
    /// Request timeout in seconds
    pub request_timeout: u64,
}

impl Default for ApiConfig {
    fn default() -> Self {
        Self {
            bind_address: "127.0.0.1".to_string(),
            port: 8080,
            enable_docs: true,
            cors_allowed_origins: vec!["*".to_string()],
            rate_limit: Some(100),
            enable_auth: false,
            api_keys: None,
            detailed_logging: true,
            max_json_payload_size: 5, // 5 MB
            request_timeout: 30, // 30 seconds
        }
    }
}

/// API server
pub struct ApiServer {
    /// Node instance
    node: Arc<Node>,
    /// Server configuration
    config: ApiConfig,
    /// Bind address 
    bind_address: String,
    /// Port
    port: u16,
    /// API metrics
    metrics: Arc<ApiMetrics>,
}

impl ApiServer {
    /// Create a new API server instance
    pub fn new(node: Arc<Node>, bind_address: &str, port: u16) -> Self {
        Self {
            node,
            config: ApiConfig::default(),
            bind_address: bind_address.to_string(),
            port,
            metrics: Arc::new(ApiMetrics::new()),
        }
    }
    
    /// Set API server configuration
    pub fn with_config(mut self, config: ApiConfig) -> Self {
        self.config = config;
        self.bind_address = config.bind_address.clone();
        self.port = config.port;
        self
    }
    
    /// Start the API server
    pub async fn start(self) -> std::io::Result<Server> {
        let node_data = web::Data::new(self.node);
        let metrics_data = web::Data::new(self.metrics.clone());
        let config = self.config.clone();
        
        // Create OpenAPI documentation
        let openapi = ApiDoc::openapi();
        
        // Calculate socket address
        let socket_addr = SocketAddr::new(
            IpAddr::from_str(&self.bind_address).unwrap_or_else(|_| IpAddr::from_str("127.0.0.1").unwrap()),
            self.port
        );
        
        info!("Starting API server on {}", socket_addr);
        
        // Set up the HTTP server
        let server = HttpServer::new(move || {
            let mut app = App::new()
                .app_data(node_data.clone())
                .app_data(metrics_data.clone())
                // Configure JSON extractor limits
                .app_data(web::JsonConfig::default()
                    .limit(config.max_json_payload_size * 1024 * 1024))
                // Configure standard middleware
                .wrap(middleware::Compress::default())
                .wrap(middleware::NormalizePath::new(
                    middleware::TrailingSlash::Trim
                ));
                
            // Add CORS middleware if configured
            if !config.cors_allowed_origins.is_empty() {
                let cors = Cors::default()
                    .allowed_methods(vec!["GET", "POST", "PUT", "DELETE"])
                    .allowed_headers(vec![
                        "Authorization",
                        "Accept",
                        "Content-Type",
                        "X-Requested-With"
                    ])
                    .max_age(3600);
                    
                // Add allowed origins
                let cors = config.cors_allowed_origins.iter().fold(cors, |cors, origin| {
                    if origin == "*" {
                        cors.allow_any_origin()
                    } else {
                        cors.allowed_origin(origin)
                    }
                });
                
                app = app.wrap(cors);
            }
            
            // Add custom logging middleware if detailed logging is enabled
            if config.detailed_logging {
                app = app.wrap(logging::ApiLogger::new());
            } else {
                app = app.wrap(middleware::Logger::default());
            }
            
            // Add authentication middleware if enabled
            if config.enable_auth {
                app = app.wrap(auth::ApiAuth::new(config.api_keys.clone().unwrap_or_default()));
            }
            
            // Add rate limiting middleware if configured
            if let Some(rate) = config.rate_limit {
                app = app.wrap(rate_limiting::RateLimiter::new(rate));
            }
            
            // Configure API routes
            app = app.configure(routes::configure);
            
            // Add OpenAPI documentation if enabled
            if config.enable_docs {
                app = app.service(
                    SwaggerUi::new("/swagger-ui/{_:.*}")
                        .url("/api-docs/openapi.json", openapi.clone())
                );
            }
            
            app
        })
        .client_request_timeout(std::time::Duration::from_secs(config.request_timeout))
        .bind(socket_addr)?
        .run();
        
        info!("API server started on {}", socket_addr);
        
        Ok(server)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_api_config_default() {
        let config = ApiConfig::default();
        assert_eq!(config.bind_address, "127.0.0.1");
        assert_eq!(config.port, 8080);
        assert!(config.enable_docs);
        assert_eq!(config.cors_allowed_origins, vec!["*".to_string()]);
        assert_eq!(config.rate_limit, Some(100));
    }
} 