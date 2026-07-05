//! JSON-RPC server implementation
//!
//! This module implements the JSON-RPC 2.0 API for supernova blockchain.

mod handlers;
mod types;

use actix_web::{web, HttpRequest, HttpResponse, Responder, http::header};
use serde_json::Value;
use std::sync::Arc;
use crate::api_facade::ApiFacade;
use crate::api::rate_limiter::{ApiRateLimiter, ApiRateLimitConfig, is_expensive_endpoint};
use types::{JsonRpcRequest, JsonRpcResponse, ErrorCode};

/// JSON-RPC request handler
/// 
/// Enhanced with rate limiting to prevent API DoS attacks.
pub async fn handle_jsonrpc(
    http_req: HttpRequest,
    request: web::Json<JsonRpcRequest>,
    node: web::Data<Arc<ApiFacade>>,
    rate_limiter: web::Data<Arc<ApiRateLimiter>>,
) -> impl Responder {
    let req = request.into_inner();
    let id = req.id.clone();

    // SECURITY: Extract client IP address for rate limiting
    let client_ip = http_req
        .peer_addr()
        .map(|addr| addr.ip())
        .unwrap_or_else(|| std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST));

    // SECURITY: Check if endpoint is expensive
    let is_expensive = is_expensive_endpoint(&req.method);

    // CRITICAL SECURITY CHECK: Rate limiting
    if let Err(reason) = rate_limiter.check_rate_limit(client_ip, &req.method, is_expensive) {
        return HttpResponse::TooManyRequests().json(JsonRpcResponse::error(
            id,
            ErrorCode::RateLimitExceeded,
            reason,
            Some(serde_json::json!({
                "retry_after": 60,
                "limit": ApiRateLimitConfig::MAX_REQUESTS_PER_IP_PER_MINUTE,
            })),
        ));
    }

    // Validate JSON-RPC version
    if req.jsonrpc != "2.0" {
        rate_limiter.complete_request(client_ip);
        return HttpResponse::Ok().json(JsonRpcResponse::error(
            id,
            ErrorCode::InvalidRequest,
            "Invalid JSON-RPC version".to_string(),
            None,
        ));
    }

    // Dispatch to appropriate method handler
    let result = match handlers::dispatch(&req.method, req.params, node).await {
        Ok(result) => JsonRpcResponse::result(id, result),
        Err(e) => JsonRpcResponse::error(
            id,
            ErrorCode::from(e.code),
            e.message,
            e.data,
        ),
    };

    // Mark request as complete (decrements concurrent counter)
    rate_limiter.complete_request(client_ip);

    HttpResponse::Ok().json(result)
}

/// Build a rejection response for an over-sized batch, or `None` if the batch
/// length is within the configured limit.
///
/// SECURITY: Enforces `ApiRateLimitConfig::MAX_BATCH_SIZE` so an attacker cannot
/// pack an unbounded number of (potentially expensive) sub-requests into a
/// single POST to amplify load and bypass per-request accounting.
fn oversized_batch_response(len: usize) -> Option<JsonRpcResponse> {
    if len > ApiRateLimitConfig::MAX_BATCH_SIZE {
        Some(JsonRpcResponse::error(
            Value::Null,
            ErrorCode::InvalidRequest,
            format!(
                "Batch size {} exceeds maximum of {}",
                len,
                ApiRateLimitConfig::MAX_BATCH_SIZE
            ),
            Some(serde_json::json!({
                "max_batch_size": ApiRateLimitConfig::MAX_BATCH_SIZE,
            })),
        ))
    } else {
        None
    }
}

/// Batch JSON-RPC request handler
pub async fn handle_jsonrpc_batch(
    http_req: HttpRequest,
    requests: web::Json<Vec<JsonRpcRequest>>,
    node: web::Data<Arc<ApiFacade>>,
    rate_limiter: web::Data<Arc<ApiRateLimiter>>,
) -> impl Responder {
    if requests.is_empty() {
        return HttpResponse::Ok().json(JsonRpcResponse::error(
            Value::Null,
            ErrorCode::InvalidRequest,
            "Empty batch".to_string(),
            None,
        ));
    }

    // SECURITY: Cap the number of sub-requests per batch. Without this bound a
    // client can pack many expensive methods into one POST to amplify load,
    // since each sub-request is dispatched serially. Reject the entire batch
    // rather than truncating it.
    if let Some(err) = oversized_batch_response(requests.len()) {
        return HttpResponse::Ok().json(err);
    }

    // SECURITY: Extract client IP for per-sub-request rate limiting. Batching
    // must NOT be a bypass for the DoS protection enforced on the single-call
    // path — each sub-request is dispatched serially and counts against the
    // same per-IP / per-endpoint / concurrency limits.
    let client_ip = http_req
        .peer_addr()
        .map(|addr| addr.ip())
        .unwrap_or_else(|| std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST));

    let mut responses = Vec::with_capacity(requests.len());

    for req in requests.iter() {
        let id = req.id.clone();

        // CRITICAL SECURITY CHECK: Rate limit every sub-request individually so
        // a batch cannot amplify load past the per-request ceilings.
        let is_expensive = is_expensive_endpoint(&req.method);
        if let Err(reason) = rate_limiter.check_rate_limit(client_ip, &req.method, is_expensive) {
            responses.push(JsonRpcResponse::error(
                id,
                ErrorCode::RateLimitExceeded,
                reason,
                Some(serde_json::json!({
                    "retry_after": 60,
                    "limit": ApiRateLimitConfig::MAX_REQUESTS_PER_IP_PER_MINUTE,
                })),
            ));
            continue;
        }

        // Validate JSON-RPC version
        if req.jsonrpc != "2.0" {
            rate_limiter.complete_request(client_ip);
            responses.push(JsonRpcResponse::error(
                id,
                ErrorCode::InvalidRequest,
                "Invalid JSON-RPC version".to_string(),
                None,
            ));
            continue;
        }

        // Dispatch to appropriate method handler
        let result = match handlers::dispatch(&req.method, req.params.clone(), node.clone()).await {
            Ok(result) => JsonRpcResponse::result(id, result),
            Err(e) => JsonRpcResponse::error(
                id,
                ErrorCode::from(e.code),
                e.message,
                e.data,
            ),
        };

        // Mark sub-request complete (decrements concurrent counter).
        rate_limiter.complete_request(client_ip);

        responses.push(result);
    }

    HttpResponse::Ok().json(responses)
}

/// Serve JSON-RPC documentation
pub async fn get_docs() -> impl Responder {
    let docs = "# JSON-RPC API Documentation\n\nDocumentation coming soon...";

    HttpResponse::Ok()
        .insert_header(header::ContentType::html())
        .body(format!(
            r#"<!DOCTYPE html>
<html>
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>supernova JSON-RPC API Documentation</title>
    <link rel="stylesheet" href="https://cdn.jsdelivr.net/npm/github-markdown-css@5.2.0/github-markdown-light.css">
    <style>
        .markdown-body {{
            box-sizing: border-box;
            min-width: 200px;
            max-width: 980px;
            margin: 0 auto;
            padding: 45px;
        }}

        @media (max-width: 767px) {{
            .markdown-body {{
                padding: 15px;
            }}
        }}

        code {{
            background-color: #f6f8fa;
            padding: 0.2em 0.4em;
            border-radius: 3px;
        }}

        pre {{
            background-color: #f6f8fa;
            padding: 16px;
            border-radius: 6px;
            overflow: auto;
        }}
    </style>
</head>
<body>
    <div class="markdown-body">
        {0}
    </div>
    <script src="https://cdn.jsdelivr.net/npm/marked@4.3.0/marked.min.js"></script>
    <script>
        document.addEventListener('DOMContentLoaded', function() {{
            const markdownContent = document.querySelector('.markdown-body');
            markdownContent.innerHTML = marked.parse(markdownContent.textContent);
        }});
    </script>
</body>
</html>"#,
            docs
        ))
}

/// Configure JSON-RPC routes
pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.route("/", web::post().to(handle_jsonrpc))
        .route("/batch", web::post().to(handle_jsonrpc_batch))
        .route("/docs", web::get().to(get_docs));
}

#[cfg(test)]
mod batch_limit_tests {
    use super::*;

    #[test]
    fn batch_at_or_below_limit_is_allowed() {
        // The maximum permitted batch size must not be rejected.
        assert!(oversized_batch_response(ApiRateLimitConfig::MAX_BATCH_SIZE).is_none());
        assert!(oversized_batch_response(1).is_none());
        // Empty batches are handled separately by the caller, but the size
        // check itself must not reject them.
        assert!(oversized_batch_response(0).is_none());
    }

    #[test]
    fn batch_above_limit_is_rejected_as_invalid_request() {
        let resp = oversized_batch_response(ApiRateLimitConfig::MAX_BATCH_SIZE + 1)
            .expect("over-sized batch must be rejected");
        let err = resp.error.expect("rejection must carry a JSON-RPC error");
        assert_eq!(err.code, ErrorCode::InvalidRequest as i32);
        // The advertised cap must be surfaced to the client.
        let data = err.data.expect("error data must include the cap");
        assert_eq!(
            data.get("max_batch_size").and_then(|v| v.as_u64()),
            Some(ApiRateLimitConfig::MAX_BATCH_SIZE as u64)
        );
    }

    #[test]
    fn large_batch_is_rejected() {
        assert!(oversized_batch_response(80).is_some());
        assert!(oversized_batch_response(usize::MAX).is_some());
    }

    /// SECURITY: batched sub-requests must be subject to the same per-IP rate
    /// limit as single POST-`/` calls. This mirrors the check/complete pairing
    /// `handle_jsonrpc_batch` now applies per sub-request, proving that packing
    /// expensive methods into one batch cannot exceed the per-IP token budget.
    #[test]
    fn batch_subrequests_share_the_per_ip_rate_limit() {
        use std::net::{IpAddr, Ipv4Addr};

        let limiter = ApiRateLimiter::new();
        let ip = IpAddr::V4(Ipv4Addr::new(203, 0, 113, 7));

        // 60 tokens / 10 (expensive multiplier) = 6 expensive calls permitted.
        let mut allowed = 0usize;
        for _ in 0..12 {
            match limiter.check_rate_limit(ip, "generate", true) {
                Ok(()) => {
                    allowed += 1;
                    // The batch loop completes each sub-request before the next,
                    // so the concurrency ceiling is never the limiting factor.
                    limiter.complete_request(ip);
                }
                Err(_) => break,
            }
        }

        assert_eq!(
            allowed, 6,
            "expensive batch sub-requests must exhaust the per-IP budget, not bypass it"
        );
        assert!(
            limiter.check_rate_limit(ip, "generate", true).is_err(),
            "further sub-requests from the same IP must be rate limited"
        );
    }
}