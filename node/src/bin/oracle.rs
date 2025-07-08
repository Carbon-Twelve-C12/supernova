use actix_web::{web, App, HttpResponse, HttpServer, middleware};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;
use chrono::{DateTime, Utc};
use tracing::{info, error};
use std::time::Duration;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct EnvironmentalData {
    timestamp: DateTime<Utc>,
    carbon_intensity: f64,
    renewable_percentage: f64,
    grid_efficiency: f64,
    region: String,
    data_source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct OracleState {
    current_data: EnvironmentalData,
    last_update: DateTime<Utc>,
    update_count: u64,
}

struct OracleService {
    state: Arc<RwLock<OracleState>>,
    update_interval: Duration,
}

impl OracleService {
    fn new(update_interval: Duration) -> Self {
        let initial_data = EnvironmentalData {
            timestamp: Utc::now(),
            carbon_intensity: 150.0, // gCO2/kWh
            renewable_percentage: 0.45,
            grid_efficiency: 0.92,
            region: "global".to_string(),
            data_source: "mock".to_string(),
        };

        let state = Arc::new(RwLock::new(OracleState {
            current_data: initial_data,
            last_update: Utc::now(),
            update_count: 0,
        }));

        Self {
            state,
            update_interval,
        }
    }

    async fn start_update_loop(&self) {
        let state = self.state.clone();
        let interval = self.update_interval;

        tokio::spawn(async move {
            let mut interval_timer = tokio::time::interval(interval);
            loop {
                interval_timer.tick().await;
                
                // Mock data update - in production, this would fetch from real APIs
                let mut state_guard = state.write().await;
                state_guard.current_data.timestamp = Utc::now();
                state_guard.current_data.carbon_intensity = 100.0 + (rand::random::<f64>() * 100.0);
                state_guard.current_data.renewable_percentage = 0.3 + (rand::random::<f64>() * 0.5);
                state_guard.current_data.grid_efficiency = 0.85 + (rand::random::<f64>() * 0.1);
                state_guard.last_update = Utc::now();
                state_guard.update_count += 1;
                
                info!("Updated environmental data: carbon_intensity={:.2} gCO2/kWh, renewable={:.1}%", 
                    state_guard.current_data.carbon_intensity,
                    state_guard.current_data.renewable_percentage * 100.0
                );
            }
        });
    }
}

async fn health_check() -> HttpResponse {
    HttpResponse::Ok().json(serde_json::json!({
        "status": "healthy",
        "service": "environmental-oracle",
        "timestamp": Utc::now()
    }))
}

async fn get_current_data(oracle: web::Data<Arc<OracleService>>) -> HttpResponse {
    let state = oracle.state.read().await;
    HttpResponse::Ok().json(&state.current_data)
}

async fn get_oracle_status(oracle: web::Data<Arc<OracleService>>) -> HttpResponse {
    let state = oracle.state.read().await;
    HttpResponse::Ok().json(&*state)
}

async fn get_carbon_intensity(oracle: web::Data<Arc<OracleService>>) -> HttpResponse {
    let state = oracle.state.read().await;
    HttpResponse::Ok().json(serde_json::json!({
        "carbon_intensity": state.current_data.carbon_intensity,
        "unit": "gCO2/kWh",
        "timestamp": state.current_data.timestamp
    }))
}

async fn get_renewable_mix(oracle: web::Data<Arc<OracleService>>) -> HttpResponse {
    let state = oracle.state.read().await;
    HttpResponse::Ok().json(serde_json::json!({
        "renewable_percentage": state.current_data.renewable_percentage,
        "fossil_percentage": 1.0 - state.current_data.renewable_percentage,
        "timestamp": state.current_data.timestamp
    }))
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter("info")
        .init();

    // Get configuration from environment
    let port = std::env::var("PORT")
        .unwrap_or_else(|_| "8390".to_string())
        .parse::<u16>()
        .expect("Invalid PORT");

    let update_interval_secs = std::env::var("UPDATE_INTERVAL")
        .unwrap_or_else(|_| "300".to_string())
        .parse::<u64>()
        .expect("Invalid UPDATE_INTERVAL");

    info!("Starting Environmental Oracle Service on port {}", port);
    info!("Update interval: {} seconds", update_interval_secs);

    // Create oracle service
    let oracle = Arc::new(OracleService::new(Duration::from_secs(update_interval_secs)));
    
    // Start update loop
    oracle.start_update_loop().await;

    // Start HTTP server
    HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(oracle.clone()))
            .wrap(middleware::Logger::default())
            .wrap(
                actix_cors::Cors::default()
                    .allow_any_origin()
                    .allow_any_method()
                    .allow_any_header()
            )
            .route("/health", web::get().to(health_check))
            .route("/api/current", web::get().to(get_current_data))
            .route("/api/status", web::get().to(get_oracle_status))
            .route("/api/carbon-intensity", web::get().to(get_carbon_intensity))
            .route("/api/renewable-mix", web::get().to(get_renewable_mix))
    })
    .bind(("0.0.0.0", port))?
    .run()
    .await
} 