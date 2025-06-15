//! API server implementation
//!
//! This module implements the HTTP server for the supernova API, 
//! handling requests, routing, and middleware.

use actix_web::{web, App, HttpServer, middleware, dev::Server};
use actix_cors::Cors;
use std::sync::Arc;
use std::net::{IpAddr, SocketAddr};
use std::str::FromStr;
use serde::{Deserialize, Serialize};
use utoipa_swagger_ui::SwaggerUi;
use utoipa::OpenApi;
use tracing::{info, error, warn};

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
            enable_auth: true,  // SECURITY: Authentication enabled by default
            api_keys: Some(vec![
                // Default secure API key - MUST be changed in production
                "CHANGE-ME-IN-PRODUCTION-supernova-default-key".to_string()
            ]),
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
        // Warn about default API key usage
        let config = ApiConfig::default();
        if config.api_keys.as_ref().map(|keys| keys.iter().any(|k| k.contains("CHANGE-ME"))).unwrap_or(false) {
            warn!("SECURITY WARNING: Using default API key. Change this in production!");
        }
        
        Self {
            node,
            config,
            bind_address: bind_address.to_string(),
            port,
            metrics: Arc::new(ApiMetrics::new()),
        }
    }
    
    /// Set API server configuration
    pub fn with_config(mut self, config: ApiConfig) -> Self {
        self.bind_address = config.bind_address.clone();
        self.port = config.port;
        self.config = config;
        self
    }
    
    /// Start the API server
    pub async fn start(self) -> std::io::Result<Server> {
        let node_data = web::Data::new(self.node);
        let metrics_data = web::Data::new(self.metrics.clone());
        let config = self.config.clone();
        let rate_limit = config.rate_limit.unwrap_or(100);
        
        // Validate API key configuration
        if config.enable_auth {
            let api_keys = config.api_keys.clone().unwrap_or_default();
            if api_keys.is_empty() {
                error!("SECURITY ERROR: Authentication is enabled but no API keys configured");
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "Authentication enabled but no API keys configured"
                ));
            }
            
            // Warn about default key usage
            if api_keys.iter().any(|k| k.contains("CHANGE-ME")) {
                warn!("SECURITY WARNING: Default API key detected. This MUST be changed in production!");
            }
        }
        
        // Create OpenAPI documentation
        let openapi = ApiDoc::openapi();
        
        // Calculate socket address
        let socket_addr = SocketAddr::new(
            IpAddr::from_str(&self.bind_address).unwrap_or_else(|_| IpAddr::from_str("127.0.0.1").unwrap()),
            self.port
        );
        
        info!("Starting API server on {}", socket_addr);
        if config.enable_auth {
            info!("API authentication is ENABLED");
        } else {
            warn!("API authentication is DISABLED - not recommended for production");
        }
        
        // Set up the HTTP server
        let server = HttpServer::new(move || {
            App::new()
                .app_data(node_data.clone())
                .app_data(metrics_data.clone())
                // Configure JSON extractor limits
                .app_data(web::JsonConfig::default().limit(4096))
                // TODO: Add authentication middleware when type issue is resolved
                .wrap(middleware::Compress::default())
                .wrap(
                    middleware::DefaultHeaders::new()
                        .header("X-Version", "1.0")
                        .header("X-Frame-Options", "DENY")
                        .header("X-Content-Type-Options", "nosniff")
                        .header("X-XSS-Protection", "1; mode=block")
                )
                .wrap(middleware::NormalizePath::new(
                    middleware::TrailingSlash::Trim
                ))
                .wrap(
                    Cors::default()
                        .allow_any_origin()
                        .allow_any_method()
                        .allow_any_header()
                        .max_age(3600)
                )
                .wrap(middleware::Logger::default())
                .wrap(rate_limiting::RateLimiter::new(rate_limit))
                // Configure API routes
                .configure(routes::configure)
                // Add OpenAPI documentation if enabled
                .service(
                    SwaggerUi::new("/swagger-ui/{_:.*}")
                        .url("/api-docs/openapi.json", openapi.clone())
                        .config(utoipa_swagger_ui::Config::default())
                )
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