//! API middleware components
//!
//! This module contains middleware components used by the supernova API,
//! including authentication, rate limiting, and request logging.

pub mod auth;
pub mod auth_rate_limiter;
pub mod distributed_rate_limit;
pub mod logging;
pub mod rate_limiting;

// Re-export middleware components
pub use auth::ApiAuth;
pub use auth_rate_limiter::{AuthRateLimiter, AuthRateLimiterConfig};
pub use distributed_rate_limit::{DistributedRateLimiter, DistributedRateLimitConfig, RateLimitResult};
pub use logging::ApiLogger;
pub use rate_limiting::RateLimiter;
