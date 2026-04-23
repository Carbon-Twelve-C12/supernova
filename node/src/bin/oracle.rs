// Supernova environmental oracle — **mock** HTTP service.
//
// This binary exists only to exercise the environmental-data API shape during
// local development and CI. It emits randomised carbon-intensity, renewable-
// mix, and grid-efficiency values and MUST NOT be deployed to consensus-
// relevant infrastructure: if enough operators ran this by default, majority-
// noise could contaminate the environmental oracle quorum that downstream
// consensus trusts.
//
// Hardening summary:
//   1. The binary only compiles with `--features mock-oracle`. A no-op `main`
//      stub refuses to start otherwise, so a production `cargo build --release
//      --all-features` (which does **not** include `mock-oracle`) produces a
//      harmless placeholder.
//   2. Even with the feature enabled, the service binds to 127.0.0.1 by
//      default, restricts CORS to an explicit allow-list, propagates
//      configuration errors instead of panicking, and requires the operator
//      to acknowledge the mock nature via `ORACLE_I_UNDERSTAND_THIS_IS_MOCK=
//      yes`.

#[cfg(not(feature = "mock-oracle"))]
fn main() -> std::io::Result<()> {
    eprintln!(
        "supernova-oracle: refusing to start.\n\
         \n\
         This binary is a mock/demo environmental data source and is \
         disabled by default. It has no real carbon-intensity or renewable-\n\
         mix feed and must never be deployed into a consensus-relevant \
         environmental oracle quorum.\n\
         \n\
         If you are running local development or CI fixtures that need the \
         mock feed, rebuild with:\n\
         \n\
         \tcargo build --release -p supernova-node --bin supernova-oracle \
         --features mock-oracle\n\
         \n\
         and set ORACLE_I_UNDERSTAND_THIS_IS_MOCK=yes in the environment."
    );
    std::process::exit(2);
}

#[cfg(feature = "mock-oracle")]
mod mock {
    use actix_web::{middleware, web, App, HttpResponse, HttpServer};
    use chrono::{DateTime, Utc};
    use serde::{Deserialize, Serialize};
    use std::net::IpAddr;
    use std::sync::Arc;
    use std::time::Duration;
    use tokio::sync::RwLock;
    use tracing::{info, warn};

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub(super) struct EnvironmentalData {
        timestamp: DateTime<Utc>,
        carbon_intensity: f64,
        renewable_percentage: f64,
        grid_efficiency: f64,
        region: String,
        data_source: String,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub(super) struct OracleState {
        current_data: EnvironmentalData,
        last_update: DateTime<Utc>,
        update_count: u64,
    }

    pub(super) struct OracleService {
        state: Arc<RwLock<OracleState>>,
        update_interval: Duration,
    }

    impl OracleService {
        fn new(update_interval: Duration) -> Self {
            let initial_data = EnvironmentalData {
                timestamp: Utc::now(),
                carbon_intensity: 150.0,
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

                    let mut state_guard = state.write().await;
                    state_guard.current_data.timestamp = Utc::now();
                    state_guard.current_data.carbon_intensity =
                        100.0 + (rand::random::<f64>() * 100.0);
                    state_guard.current_data.renewable_percentage =
                        0.3 + (rand::random::<f64>() * 0.5);
                    state_guard.current_data.grid_efficiency =
                        0.85 + (rand::random::<f64>() * 0.1);
                    state_guard.last_update = Utc::now();
                    state_guard.update_count += 1;

                    info!(
                        "Updated MOCK environmental data: carbon_intensity={:.2} gCO2/kWh, renewable={:.1}%",
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
            "service": "environmental-oracle-mock",
            "data_source": "mock",
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
            "data_source": "mock",
            "timestamp": state.current_data.timestamp
        }))
    }

    async fn get_renewable_mix(oracle: web::Data<Arc<OracleService>>) -> HttpResponse {
        let state = oracle.state.read().await;
        HttpResponse::Ok().json(serde_json::json!({
            "renewable_percentage": state.current_data.renewable_percentage,
            "fossil_percentage": 1.0 - state.current_data.renewable_percentage,
            "data_source": "mock",
            "timestamp": state.current_data.timestamp
        }))
    }

    fn parse_env<T: std::str::FromStr>(name: &str, default: &str) -> std::io::Result<T>
    where
        T::Err: std::fmt::Display,
    {
        let raw = std::env::var(name).unwrap_or_else(|_| default.to_string());
        raw.parse::<T>().map_err(|err| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!("invalid value for {name}={raw}: {err}"),
            )
        })
    }

    fn build_cors() -> actix_cors::Cors {
        let mut cors = actix_cors::Cors::default()
            .allowed_methods(vec!["GET", "OPTIONS"])
            .allow_any_header()
            .max_age(3600);

        let raw = std::env::var("ORACLE_CORS_ALLOWED_ORIGINS")
            .unwrap_or_else(|_| "http://127.0.0.1,http://localhost".to_string());
        for origin in raw.split(',').map(str::trim).filter(|s| !s.is_empty()) {
            cors = cors.allowed_origin(origin);
        }
        cors
    }

    pub(super) async fn run() -> std::io::Result<()> {
        tracing_subscriber::fmt().with_env_filter("info").init();

        if std::env::var("ORACLE_I_UNDERSTAND_THIS_IS_MOCK").ok().as_deref()
            != Some("yes")
        {
            return Err(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                "supernova-oracle emits MOCK environmental data and must not be \
                 deployed in consensus-relevant infrastructure. Set \
                 ORACLE_I_UNDERSTAND_THIS_IS_MOCK=yes to run it for local dev/CI.",
            ));
        }

        let port: u16 = parse_env("PORT", "8390")?;
        let update_interval_secs: u64 = parse_env("UPDATE_INTERVAL", "300")?;
        let bind_host: IpAddr = parse_env("ORACLE_BIND_HOST", "127.0.0.1")?;

        if !bind_host.is_loopback() {
            warn!(
                "supernova-oracle is binding to non-loopback address {}; \
                 this exposes MOCK data to the network.",
                bind_host
            );
        }

        info!(
            "Starting MOCK Environmental Oracle on {}:{} (update interval {}s)",
            bind_host, port, update_interval_secs
        );

        let oracle = Arc::new(OracleService::new(Duration::from_secs(
            update_interval_secs,
        )));
        oracle.start_update_loop().await;

        HttpServer::new(move || {
            App::new()
                .app_data(web::Data::new(oracle.clone()))
                .wrap(middleware::Logger::default())
                .wrap(build_cors())
                .route("/health", web::get().to(health_check))
                .route("/api/current", web::get().to(get_current_data))
                .route("/api/status", web::get().to(get_oracle_status))
                .route("/api/carbon-intensity", web::get().to(get_carbon_intensity))
                .route("/api/renewable-mix", web::get().to(get_renewable_mix))
        })
        .bind((bind_host, port))?
        .run()
        .await
    }
}

#[cfg(feature = "mock-oracle")]
#[actix_web::main]
async fn main() -> std::io::Result<()> {
    mock::run().await
}
