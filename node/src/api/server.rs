//! API server implementation
//!
//! This module implements the HTTP server for the supernova API,
//! handling requests, routing, and middleware.

use actix_cors::Cors;
use actix_web::{dev::Server, middleware, web, App, HttpServer};
use serde::{Deserialize, Serialize};
use std::net::{IpAddr, SocketAddr};
use std::str::FromStr;
use std::sync::Arc;
use tracing::{error, info, warn};
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

use super::docs::ApiDoc;
use super::middleware::auth::ApiAuth;
use super::middleware::auth_rate_limiter::{AuthRateLimiter, AuthRateLimiterConfig};
use super::middleware::rate_limiting;
use super::routes;
use crate::api_facade::ApiFacade;
use crate::metrics::ApiMetrics;
use crate::node::Node;

/// Minimum acceptable API key length. Short keys are both guessable and
/// indicative of placeholder/test configuration that must not reach prod.
const MIN_API_KEY_LEN: usize = 32;
/// Lowercase substrings that mark a key as an obvious placeholder and must
/// be rejected. The input is lowercased before comparison, so case variants
/// like `Change-Me-…` / `REPLACE-ME-…` / `Example-…` are caught too — a
/// byte-exact check was trivially bypassed by case-swapping README strings.
const PLACEHOLDER_KEY_MARKERS: &[&str] = &[
    "change-me",
    "changeme",
    "replace-me",
    "replaceme",
    "example",
    "default",
    "test",
    "secret",
    "demo",
];

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
            // SECURITY: loopback-only by default; operators must explicitly
            // opt into public exposure via configuration. Binding on 0.0.0.0
            // by default previously combined with absent authentication to
            // expose privileged RPC to the LAN.
            bind_address: "127.0.0.1".to_string(),
            port: 8080,
            enable_docs: true,
            // SECURITY: no cross-origin access by default. Previously
            // defaulted to `["*"]` and the middleware also hard-coded
            // `allow_any_origin()`, which let any website in a browser
            // talk to the RPC. Operators must explicitly list trusted
            // origins (e.g. their dashboard / explorer domain).
            cors_allowed_origins: Vec::new(),
            rate_limit: Some(100),
            enable_auth: true,
            // SECURITY: no default API key. Earlier revisions shipped a
            // hard-coded `CHANGE-ME-IN-PRODUCTION-*` placeholder key that
            // only warned on startup — operators who skipped the warning
            // ran production with a known credential. The server now
            // refuses to start unless the operator supplies a real key.
            api_keys: None,
            detailed_logging: true,
            max_json_payload_size: 5, // 5 MB
            request_timeout: 30,      // 30 seconds
        }
    }
}

/// Validate an API-key list. Returns an `io::Error` describing the first
/// offending key so misconfigurations surface at startup rather than serve
/// requests with weak or placeholder credentials.
fn validate_api_keys(keys: &[String]) -> std::io::Result<()> {
    if keys.is_empty() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "API authentication is enabled but no API keys are configured. \
             Set api_keys to one or more operator-generated secrets of at \
             least 32 characters before starting.",
        ));
    }
    for key in keys {
        let trimmed = key.trim();
        if trimmed.is_empty() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "API key is empty or whitespace-only",
            ));
        }
        // Reject keys with surrounding whitespace. Validation operates on
        // the trimmed form but the stored key — the one `ApiAuth` compares
        // against the Bearer-stripped header — is the raw form. A padded
        // key would pass every check here and then silently 401 every
        // client request (clients don't send padded Authorization values).
        // Failing loud at startup is strictly better than the silent
        // auth-always-fails mode.
        if key.len() != trimmed.len() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "API key has surrounding whitespace. Remove leading/trailing \
                 spaces or newlines — the raw TOML value is compared byte-\
                 for-byte against client Authorization headers.",
            ));
        }
        if trimmed.len() < MIN_API_KEY_LEN {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!(
                    "API key must be at least {} characters long (found {})",
                    MIN_API_KEY_LEN,
                    trimmed.len()
                ),
            ));
        }
        let lowered = trimmed.to_ascii_lowercase();
        if PLACEHOLDER_KEY_MARKERS
            .iter()
            .any(|marker| lowered.contains(marker))
        {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!(
                    "API key appears to be a placeholder (contains one of {:?}). \
                     Generate a real operator secret before starting.",
                    PLACEHOLDER_KEY_MARKERS
                ),
            ));
        }
    }
    Ok(())
}

/// Build a CORS middleware from the configured allow-list.
///
/// * empty list — returns the `Cors::default()` layer, which has no allowed
///   origins / methods / headers and therefore blocks all cross-origin
///   requests (browsers enforce same-origin when no permissive CORS headers
///   come back). Same-origin requests still work.
/// * contains `"*"` — any origin is allowed but credentials are *not* sent
///   along, per the CORS spec (`Access-Control-Allow-Credentials` must be
///   `false` whenever the origin is `*`). A loud warning is emitted.
/// * explicit origins — each is registered individually.
fn build_cors(allowed_origins: &[String]) -> Cors {
    if allowed_origins.is_empty() {
        return Cors::default();
    }
    let mut cors = Cors::default()
        .allowed_methods(vec!["GET", "POST", "PUT", "DELETE", "OPTIONS"])
        .allowed_headers(vec!["Content-Type", "Authorization", "Accept", "X-Requested-With"])
        .max_age(3600);

    if allowed_origins.iter().any(|origin| origin == "*") {
        warn!(
            "API CORS is configured to allow ANY origin (`*`). Credentials \
             are disabled for safety; prefer an explicit allow-list."
        );
        // actix-cors semantics: `send_wildcard()` alone is a no-op because
        // `Cors::default()` starts with `allowed_origins = Some(empty_set)`.
        // Promoting to the `All` arm requires `allow_any_origin()`; without
        // it every cross-origin request is rejected with `OriginNotAllowed`,
        // silently breaking the documented `["*"]` escape hatch.
        cors = cors.allow_any_origin().send_wildcard();
    } else {
        for origin in allowed_origins {
            cors = cors.allowed_origin(origin);
        }
    }
    cors
}

/// API server
pub struct ApiServer {
    /// Node facade (thread-safe)
    node_facade: Arc<ApiFacade>,
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
    /// Create a new API server instance from the operator's configuration.
    ///
    /// Takes the full [`ApiConfig`] so callers cannot forget to wire
    /// bind address, auth, or CORS settings — a previous signature that
    /// accepted only `&str` / `u16` combined with `ApiServer::new` storing
    /// `ApiConfig::default()` meant the operator's `[api]` TOML was
    /// silently dropped: with the secure default (`api_keys = None,
    /// enable_auth = true`), startup always failed closed with no keys
    /// configured, and every deployment lost its API on upgrade.
    ///
    /// Returns an error if the underlying `ApiFacade` cannot be constructed
    /// (e.g. a missing wallet manager whose fallback initialization fails).
    pub fn new(node: Arc<Node>, config: ApiConfig) -> Result<Self, crate::node::NodeError> {
        let node_facade = Arc::new(ApiFacade::new(&node)?);
        let bind_address = config.bind_address.clone();
        let port = config.port;

        Ok(Self {
            node_facade,
            config,
            bind_address,
            port,
            metrics: Arc::new(ApiMetrics::new()),
        })
    }

    /// Override the API configuration after construction.
    ///
    /// Rarely needed now that [`ApiServer::new`] accepts the full
    /// [`ApiConfig`]; kept for callers that construct the server and then
    /// adjust settings (e.g. test harnesses).
    pub fn with_config(mut self, config: ApiConfig) -> Self {
        self.bind_address = config.bind_address.clone();
        self.port = config.port;
        self.config = config;
        self
    }

    /// Start the API server
    pub async fn start(self) -> std::io::Result<Server> {
        let node_data = web::Data::new(self.node_facade);
        let metrics_data = web::Data::new(self.metrics.clone());
        let config = self.config.clone();
        let rate_limit = config.rate_limit.unwrap_or(100);

        // Validate API key configuration fail-closed. An enable_auth=true
        // config with missing or placeholder keys is a production foot-gun
        // and must refuse to start.
        let validated_keys: Option<Vec<String>> = if config.enable_auth {
            let keys = config.api_keys.clone().unwrap_or_default();
            if let Err(err) = validate_api_keys(&keys) {
                error!("SECURITY: refusing to start API server: {}", err);
                return Err(err);
            }
            Some(keys)
        } else {
            warn!(
                "API authentication is DISABLED. This is only acceptable on \
                 trusted, loopback-only deployments."
            );
            None
        };

        let allowed_origins = config.cors_allowed_origins.clone();
        let enable_docs = config.enable_docs;

        // Create OpenAPI documentation
        let openapi = ApiDoc::openapi();

        // Calculate socket address. A malformed bind_address falls back to
        // loopback — the safer choice — but still emits a warning so the
        // misconfig is visible in logs.
        let socket_addr = SocketAddr::new(
            IpAddr::from_str(&self.bind_address).unwrap_or_else(|e| {
                warn!(
                    "Failed to parse bind_address '{}': {}. Falling back to 127.0.0.1",
                    self.bind_address, e
                );
                IpAddr::V4(std::net::Ipv4Addr::new(127, 0, 0, 1))
            }),
            self.port,
        );

        info!("Starting API server on {}", socket_addr);
        if validated_keys.is_some() {
            info!("API authentication is ENABLED");
        }
        if allowed_origins.is_empty() {
            info!("CORS disabled — same-origin requests only");
        } else {
            info!("CORS allowed origins: {:?}", allowed_origins);
        }

        // A single shared rate limiter across all workers. Built outside the
        // factory closure because actix-web spawns the closure once per
        // worker, and each call to `AuthRateLimiter::new` allocates a fresh
        // `failed_attempts` map — per-worker isolation would multiply the
        // brute-force ceiling by the worker count.
        let auth_rate_limiter =
            Arc::new(AuthRateLimiter::new(AuthRateLimiterConfig::default()));

        // Set up the HTTP server. The factory closure is invoked per worker;
        // middleware values must be freshly constructed each call because
        // actix-cors' `Cors` and our `ApiAuth` are not `Clone`. The
        // middleware stack has a fixed shape across both `enable_auth`
        // states (we install `ApiAuth::disabled()` when auth is off) so
        // the App type stays homogeneous and avoids conditional `.boxed()`.
        let server = HttpServer::new(move || {
            let auth = match &validated_keys {
                Some(keys) => ApiAuth::from_validated_keys_with_rate_limiter(
                    keys.clone(),
                    auth_rate_limiter.clone(),
                ),
                None => ApiAuth::disabled_with_rate_limiter(auth_rate_limiter.clone()),
            };

            let app = App::new()
                .app_data(node_data.clone())
                .app_data(metrics_data.clone())
                // Configure JSON extractor limits
                .app_data(web::JsonConfig::default().limit(4096))
                .wrap(middleware::Compress::default())
                .wrap(
                    middleware::DefaultHeaders::new()
                        .add(("X-Version", "1.0"))
                        .add(("X-Frame-Options", "DENY"))
                        .add(("X-Content-Type-Options", "nosniff"))
                        .add(("X-XSS-Protection", "1; mode=block")),
                )
                .wrap(middleware::NormalizePath::new(
                    middleware::TrailingSlash::Trim,
                ))
                // Auth sits inside CORS so that 401 responses still carry
                // the CORS headers a browser needs to interpret the
                // failure. ApiAuth internally bypasses OPTIONS preflights
                // and the public-path allow-list.
                .wrap(auth)
                .wrap(build_cors(&allowed_origins))
                .wrap(middleware::Logger::default())
                .wrap(rate_limiting::RateLimiter::new(rate_limit))
                .configure(routes::configure);

            if enable_docs {
                app.service(
                    SwaggerUi::new("/swagger-ui/{_:.*}")
                        .url("/api-docs/openapi.json", openapi.clone())
                        .config(utoipa_swagger_ui::Config::default()),
                )
            } else {
                app
            }
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
    fn test_api_config_default_is_safe() {
        let config = ApiConfig::default();
        assert_eq!(config.bind_address, "127.0.0.1");
        assert_eq!(config.port, 8080);
        assert!(config.enable_docs);
        assert!(
            config.cors_allowed_origins.is_empty(),
            "Default CORS must be empty — operators must opt into origins"
        );
        assert_eq!(config.rate_limit, Some(100));
        assert!(config.enable_auth, "Authentication must default to on");
        assert!(
            config.api_keys.is_none(),
            "No default API key may be shipped — operators must configure one"
        );
    }

    #[test]
    fn test_validate_api_keys_rejects_empty() {
        assert!(validate_api_keys(&[]).is_err());
    }

    #[test]
    fn test_validate_api_keys_rejects_whitespace() {
        assert!(validate_api_keys(&["   ".to_string()]).is_err());
    }

    #[test]
    fn test_validate_api_keys_rejects_short() {
        // 31-char key — one below the minimum.
        assert!(validate_api_keys(&["a".repeat(31)]).is_err());
    }

    #[test]
    fn test_validate_api_keys_rejects_surrounding_whitespace() {
        // A key whose trimmed form passes every other check is still
        // rejected if it has padding — the stored raw form would never
        // match the Bearer-stripped header and auth would silently fail.
        let core: String = (0..40).map(|i| ((b'a' + (i as u8 % 26)) as char)).collect();
        for padded in [
            format!(" {}", core),
            format!("{} ", core),
            format!("  {}  ", core),
            format!("\t{}", core),
            format!("{}\n", core),
        ] {
            assert!(
                validate_api_keys(&[padded.clone()]).is_err(),
                "padded key {:?} must be rejected",
                padded
            );
        }
    }

    #[test]
    fn test_validate_api_keys_rejects_placeholder() {
        // Case variants of every marker must all be rejected — the prior
        // byte-exact check only caught `CHANGE-ME` and `changeme`, so a
        // simple case-swap of any other placeholder slipped through.
        let placeholder_prefixes = [
            "CHANGE-ME-",
            "change-me-",
            "Change-Me-",
            "REPLACE-ME-",
            "ReplaceMe-",
            "CHANGEME-",
            "Example-",
            "EXAMPLE-",
            "Default-",
            "TEST-",
            "Secret-",
            "DEMO-",
        ];
        for prefix in &placeholder_prefixes {
            let k = format!("{}{}", prefix, "x".repeat(40));
            assert!(
                validate_api_keys(&[k.clone()]).is_err(),
                "placeholder `{}` should be rejected",
                k
            );
        }
    }

    #[test]
    fn test_validate_api_keys_accepts_real_key() {
        // 64-char non-placeholder key.
        let k: String = (0..64).map(|i| ((b'a' + (i as u8 % 26)) as char)).collect();
        assert!(validate_api_keys(&[k]).is_ok());
    }
}
