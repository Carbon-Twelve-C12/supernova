//! API middleware components
//!
//! This module contains middleware components used by the supernova API,
//! including authentication, rate limiting, and request logging.

pub mod auth;
pub mod auth_rate_limiter;
pub mod rate_limiting;
pub mod logging;

// Re-export middleware components
pub use auth::ApiAuth;
pub use auth_rate_limiter::{AuthRateLimiter, AuthRateLimiterConfig};
pub use rate_limiting::RateLimiter;
pub use logging::ApiLogger; 