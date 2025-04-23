pub mod v1;

use actix_web::{web, App, HttpServer, middleware};
use std::net::SocketAddr;
use std::sync::Arc;
use crate::node::NodeHandle;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

#[derive(OpenApi)]
#[openapi(
    paths(
        v1::routes::get_node_status,
        v1::routes::get_blockchain_info,
        v1::routes::get_block_by_height,
        v1::routes::get_block_by_hash,
        v1::routes::get_transaction,
        v1::routes::submit_transaction,
        v1::routes::get_mempool_info,
        v1::routes::get_mempool_transactions,
        v1::routes::get_peer_info,
        v1::routes::get_network_info,
        v1::routes::get_mining_info,
        v1::routes::get_block_template,
        v1::routes::submit_block,
        v1::routes::get_environmental_metrics,
        v1::routes::get_treasury_status,
        v1::routes::get_transaction_emissions,
        v1::routes::register_renewable_energy,
        v1::routes::get_foundation_info,
        v1::routes::get_token_allocation,
    ),
    components(
        schemas(
            v1::routes::BlockHeightParams,
            v1::routes::BlockHashParams,
            v1::routes::TxHashParams,
            v1::routes::SubmitTxRequest,
            v1::routes::SubmitBlockRequest,
            v1::routes::RenewableEnergyRequest,
        )
    ),
    tags(
        (name = "blockchain", description = "Blockchain API"),
        (name = "mempool", description = "Mempool API"),
        (name = "network", description = "Network API"),
        (name = "mining", description = "Mining API"),
        (name = "environmental", description = "Environmental API"),
        (name = "foundation", description = "Foundation and Tokenomics API"),
    ),
    info(
        title = "SuperNova Blockchain API",
        version = "1.0.0",
        description = "REST API for SuperNova blockchain node",
        license(
            name = "MIT",
            url = "https://opensource.org/licenses/MIT"
        ),
        contact(
            name = "SuperNova Developer Team",
            url = "https://supernova.io",
            email = "dev@supernova.io"
        )
    )
)]
struct ApiDoc;

/// Configuration options for the API server
pub struct ApiConfig {
    /// Bind address for the API server
    pub bind_address: SocketAddr,
    /// Enable OpenAPI documentation
    pub enable_docs: bool,
    /// CORS allowed origins
    pub cors_allowed_origins: Vec<String>,
    /// Rate limiting settings (requests per minute)
    pub rate_limit: Option<u32>,
}

impl Default for ApiConfig {
    fn default() -> Self {
        Self {
            bind_address: "127.0.0.1:8080".parse().unwrap(),
            enable_docs: true,
            cors_allowed_origins: vec!["*".to_string()],
            rate_limit: Some(100),
        }
    }
}

/// Start the API server
pub async fn start_api_server(
    node: Arc<NodeHandle>,
    config: ApiConfig,
) -> std::io::Result<actix_web::dev::Server> {
    let node_data = web::Data::new(node);
    
    let openapi = ApiDoc::openapi();
    
    let server = HttpServer::new(move || {
        let mut app = App::new()
            .app_data(node_data.clone())
            .wrap(middleware::Logger::default())
            .wrap(middleware::Compress::default());
            
        // Add CORS middleware if configured
        if !config.cors_allowed_origins.is_empty() {
            let cors = actix_cors::Cors::default()
                .allowed_methods(vec!["GET", "POST"])
                .allowed_headers(vec![actix_web::http::header::AUTHORIZATION, actix_web::http::header::ACCEPT])
                .allowed_header(actix_web::http::header::CONTENT_TYPE);
                
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
        
        // Add rate limiting if configured
        if let Some(rate) = config.rate_limit {
            app = app.wrap(
                actix_web_lab::middleware::from_fn(move |req, next| {
                    use std::time::{Duration, Instant};
                    use futures::future::{ready, Ready};
                    use std::collections::HashMap;
                    use std::sync::Mutex;
                    use actix_web::Error;
                    
                    // Simple in-memory rate limiter
                    // In production, you'd use a distributed rate limiter like Redis
                    static RATE_LIMIT_STORE: Mutex<HashMap<String, (Instant, u32)>> = 
                        Mutex::new(HashMap::new());
                    
                    async move {
                        let ip = req.peer_addr()
                            .map(|addr| addr.ip().to_string())
                            .unwrap_or_else(|| "unknown".to_string());
                            
                        let now = Instant::now();
                        let window = Duration::from_secs(60); // 1 minute window
                        
                        let mut store = RATE_LIMIT_STORE.lock().unwrap();
                        let entry = store.entry(ip).or_insert_with(|| (now, 0));
                        
                        // Reset counter if window has passed
                        if now.duration_since(entry.0) > window {
                            *entry = (now, 1);
                            next.call(req).await
                        } else if entry.1 >= rate {
                            // Rate limit exceeded
                            actix_web::HttpResponse::TooManyRequests()
                                .json(serde_json::json!({
                                    "success": false,
                                    "error": "Rate limit exceeded",
                                    "retry_after": window.as_secs()
                                }))
                                .into()
                        } else {
                            // Increment counter
                            entry.1 += 1;
                            next.call(req).await
                        }
                    }
                })
            );
        }
        
        // Configure API routes
        app = app.configure(v1::routes::configure_routes);
        
        // Add OpenAPI documentation if enabled
        if config.enable_docs {
            app = app.service(
                SwaggerUi::new("/swagger-ui/{_:.*}")
                    .url("/api-docs/openapi.json", openapi.clone())
            );
        }
        
        app
    })
    .bind(config.bind_address)?
    .run();
    
    tracing::info!("API server started on {}", config.bind_address);
    
    Ok(server)
} 