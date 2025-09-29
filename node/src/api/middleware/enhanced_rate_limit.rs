use actix_web::{
    dev::{forward_ready, Service, ServiceRequest, ServiceResponse, Transform},
    Error, HttpResponse,
};
use futures_util::future::LocalBoxFuture;
use std::{
    collections::HashMap,
    future::{ready, Ready},
    net::IpAddr,
    rc::Rc,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

/// Rate limit configuration
#[derive(Clone)]
pub struct RateLimitConfig {
    /// Maximum requests per IP per second
    pub per_ip_limit: u32,
    /// Maximum requests per /24 subnet per second
    pub per_subnet_limit: u32,
    /// Maximum global requests per second
    pub global_limit: u32,
    /// Time window for rate limiting
    pub window: Duration,
    /// Whether to enable rate limiting
    pub enabled: bool,
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            per_ip_limit: 10,
            per_subnet_limit: 100,
            global_limit: 1000,
            window: Duration::from_secs(1),
            enabled: true,
        }
    }
}

/// Rate limiter state
#[derive(Default)]
struct RateLimiterState {
    /// IP address -> (count, window_start)
    ip_requests: HashMap<IpAddr, (u32, Instant)>,
    /// Subnet (/24) -> (count, window_start)
    subnet_requests: HashMap<String, (u32, Instant)>,
    /// Global request count and window start
    global_requests: (u32, Instant),
}

/// Enhanced rate limiting middleware
pub struct EnhancedRateLimit {
    config: RateLimitConfig,
    state: Arc<Mutex<RateLimiterState>>,
}

impl EnhancedRateLimit {
    pub fn new(config: RateLimitConfig) -> Self {
        Self {
            config,
            state: Arc::new(Mutex::new(RateLimiterState::default())),
        }
    }
}

impl<S, B> Transform<S, ServiceRequest> for EnhancedRateLimit
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type InitError = ();
    type Transform = EnhancedRateLimitMiddleware<S>;
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(EnhancedRateLimitMiddleware {
            service: Rc::new(service),
            config: self.config.clone(),
            state: self.state.clone(),
        }))
    }
}

pub struct EnhancedRateLimitMiddleware<S> {
    service: Rc<S>,
    config: RateLimitConfig,
    state: Arc<Mutex<RateLimiterState>>,
}

impl<S, B> Service<ServiceRequest> for EnhancedRateLimitMiddleware<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type Future = LocalBoxFuture<'static, Result<Self::Response, Self::Error>>;

    forward_ready!(service);

    fn call(&self, req: ServiceRequest) -> Self::Future {
        if !self.config.enabled {
            let fut = self.service.call(req);
            return Box::pin(async move { fut.await });
        }

        let peer_addr = req.peer_addr();
        let config = self.config.clone();
        let state = self.state.clone();
        let service = self.service.clone();

        Box::pin(async move {
            // Extract IP address
            let ip = match peer_addr {
                Some(addr) => addr.ip(),
                None => {
                    return Ok(req.into_response(
                        HttpResponse::BadRequest()
                            .json(serde_json::json!({
                                "error": "Cannot determine client IP",
                                "code": "MISSING_IP"
                            }))
                    ));
                }
            };

            // Check rate limits
            let now = Instant::now();
            let mut state = match state.lock() {
                Ok(s) => s,
                Err(_) => {
                    // Lock is poisoned, continue without rate limiting
                    return Ok(());
                }
            };

            // Clean up old entries
            cleanup_old_entries(&mut state, now, config.window);

            // Check per-IP limit
            if !check_ip_limit(&mut state, ip, now, config.per_ip_limit, config.window) {
                return Ok(req.into_response(
                    HttpResponse::TooManyRequests()
                        .insert_header(("Retry-After", "1"))
                        .json(serde_json::json!({
                            "error": "Rate limit exceeded",
                            "message": "Too many requests from your IP address",
                            "code": "IP_RATE_LIMIT",
                            "retry_after": 1
                        }))
                ));
            }

            // Check subnet limit (/24 for IPv4, /48 for IPv6)
            let subnet = get_subnet_key(&ip);
            if !check_subnet_limit(&mut state, &subnet, now, config.per_subnet_limit, config.window) {
                return Ok(req.into_response(
                    HttpResponse::TooManyRequests()
                        .insert_header(("Retry-After", "1"))
                        .json(serde_json::json!({
                            "error": "Rate limit exceeded",
                            "message": "Too many requests from your network",
                            "code": "SUBNET_RATE_LIMIT",
                            "retry_after": 1
                        }))
                ));
            }

            // Check global limit
            if !check_global_limit(&mut state, now, config.global_limit, config.window) {
                return Ok(req.into_response(
                    HttpResponse::ServiceUnavailable()
                        .insert_header(("Retry-After", "5"))
                        .json(serde_json::json!({
                            "error": "Service overloaded",
                            "message": "Global rate limit exceeded, please try again later",
                            "code": "GLOBAL_RATE_LIMIT",
                            "retry_after": 5
                        }))
                ));
            }

            drop(state); // Release lock before calling service
            service.call(req).await
        })
    }
}

fn cleanup_old_entries(state: &mut RateLimiterState, now: Instant, window: Duration) {
    // Clean up IP entries
    state.ip_requests.retain(|_, (_, start)| now.duration_since(*start) < window);

    // Clean up subnet entries
    state.subnet_requests.retain(|_, (_, start)| now.duration_since(*start) < window);
}

fn check_ip_limit(
    state: &mut RateLimiterState,
    ip: IpAddr,
    now: Instant,
    limit: u32,
    window: Duration,
) -> bool {
    let entry = state.ip_requests.entry(ip).or_insert((0, now));

    if now.duration_since(entry.1) >= window {
        // New window
        entry.0 = 1;
        entry.1 = now;
        true
    } else if entry.0 < limit {
        // Within limit
        entry.0 += 1;
        true
    } else {
        // Limit exceeded
        false
    }
}

fn check_subnet_limit(
    state: &mut RateLimiterState,
    subnet: &str,
    now: Instant,
    limit: u32,
    window: Duration,
) -> bool {
    let entry = state.subnet_requests.entry(subnet.to_string()).or_insert((0, now));

    if now.duration_since(entry.1) >= window {
        entry.0 = 1;
        entry.1 = now;
        true
    } else if entry.0 < limit {
        entry.0 += 1;
        true
    } else {
        false
    }
}

fn check_global_limit(
    state: &mut RateLimiterState,
    now: Instant,
    limit: u32,
    window: Duration,
) -> bool {
    let (count, start) = &mut state.global_requests;

    if now.duration_since(*start) >= window {
        *count = 1;
        *start = now;
        true
    } else if *count < limit {
        *count += 1;
        true
    } else {
        false
    }
}

fn get_subnet_key(ip: &IpAddr) -> String {
    match ip {
        IpAddr::V4(ipv4) => {
            // /24 subnet for IPv4
            let octets = ipv4.octets();
            format!("{}.{}.{}.0/24", octets[0], octets[1], octets[2])
        }
        IpAddr::V6(ipv6) => {
            // /48 subnet for IPv6
            let segments = ipv6.segments();
            format!("{:x}:{:x}:{:x}::/48", segments[0], segments[1], segments[2])
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::{test, web, App, HttpResponse};
    use std::net::{IpAddr, Ipv4Addr, SocketAddr};

    #[actix_web::test]
    async fn test_per_ip_rate_limit() {
        let config = RateLimitConfig {
            per_ip_limit: 2,
            ..Default::default()
        };

        let app = test::init_service(
            App::new()
                .wrap(EnhancedRateLimit::new(config))
                .route("/test", web::get().to(|| async { HttpResponse::Ok().body("OK") }))
        ).await;

        let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8080);

        // First two requests should succeed
        for _ in 0..2 {
            let req = test::TestRequest::get()
                .peer_addr(addr)
                .uri("/test")
                .to_request();
            let resp = test::call_service(&app, req).await;
            assert_eq!(resp.status(), 200);
        }

        // Third request should be rate limited
        let req = test::TestRequest::get()
            .peer_addr(addr)
            .uri("/test")
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 429);
    }
}