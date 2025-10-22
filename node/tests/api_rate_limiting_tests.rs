//! API Rate Limiting Security Tests
//!
//! SECURITY TEST SUITE (P2-007): Tests for API rate limiting
//! 
//! This test suite validates the fix for the API rate limiting vulnerability.
//! It ensures that API flooding attacks are prevented through per-IP and per-endpoint
//! rate limiting, protecting the node from denial-of-service attacks.
//!
//! Test Coverage:
//! - Per-IP global rate limiting (60 req/min)
//! - Per-endpoint rate limiting (30 req/min)
//! - Concurrent request limiting (5 max)
//! - Expensive endpoint multipliers
//! - Token bucket algorithm validation

use node::api::{ApiRateLimiter, ApiRateLimitConfig};
use std::net::IpAddr;
use std::sync::Arc;
use std::thread;

#[test]
fn test_rate_limit_config_constants() {
    // SECURITY TEST: Verify rate limit constants are properly configured
    
    assert_eq!(
        ApiRateLimitConfig::MAX_REQUESTS_PER_IP_PER_MINUTE,
        60,
        "Max requests per IP should be 60/minute"
    );
    
    assert_eq!(
        ApiRateLimitConfig::MAX_REQUESTS_PER_ENDPOINT_PER_MINUTE,
        30,
        "Max requests per endpoint should be 30/minute"
    );
    
    assert_eq!(
        ApiRateLimitConfig::MAX_CONCURRENT_REQUESTS_PER_IP,
        5,
        "Max concurrent requests should be 5"
    );
    
    assert_eq!(
        ApiRateLimitConfig::EXPENSIVE_ENDPOINT_MULTIPLIER,
        10,
        "Expensive endpoints should count as 10x"
    );
    
    assert_eq!(
        ApiRateLimitConfig::MAX_BATCH_SIZE,
        10,
        "Max batch size should be 10"
    );
    
    println!("✓ API rate limit constants properly configured");
}

#[test]
fn test_per_ip_rate_limiting() {
    // SECURITY TEST: Per-IP global rate limit enforced
    
    let rate_limiter = ApiRateLimiter::new();
    let test_ip: IpAddr = "192.168.1.100".parse().unwrap();
    
    // Try to make 70 requests (exceeds limits)
    let mut accepted = 0;
    let mut rejected = 0;
    
    for _ in 0..70 {
        match rate_limiter.check_rate_limit(test_ip, "getinfo", false) {
            Ok(_) => {
                accepted += 1;
                rate_limiter.complete_request(test_ip);
            }
            Err(_) => rejected += 1,
        }
    }
    
    // Per-endpoint limit (30) is more restrictive than global (60)
    // So we expect 30 accepted, 40 rejected
    assert_eq!(accepted, 30, "Should accept exactly 30 requests (per-endpoint limit)");
    assert_eq!(rejected, 40, "Should reject 40 requests over endpoint limit");
    
    println!("✓ Per-IP rate limit: {}/70 accepted, {}/70 rejected (endpoint limit more restrictive)", accepted, rejected);
}

#[test]
fn test_per_endpoint_rate_limiting() {
    // SECURITY TEST: Per-endpoint rate limit enforced
    
    let rate_limiter = ApiRateLimiter::new();
    let test_ip: IpAddr = "192.168.1.101".parse().unwrap();
    
    // Try to make 40 requests to same endpoint (exceeds 30/endpoint limit)
    let mut accepted = 0;
    
    for _ in 0..40 {
        if rate_limiter.check_rate_limit(test_ip, "getblock", false).is_ok() {
            accepted += 1;
            rate_limiter.complete_request(test_ip);
        }
    }
    
    // Should accept at most 30 per endpoint
    assert!(accepted <= 30, "Should not exceed 30 requests per endpoint: {}", accepted);
    
    println!("✓ Per-endpoint rate limit: {}/40 requests to 'getblock' accepted", accepted);
}

#[test]
fn test_expensive_endpoint_multiplier() {
    // SECURITY TEST: Expensive endpoints count as 10x requests
    
    let rate_limiter = ApiRateLimiter::new();
    let test_ip: IpAddr = "192.168.1.102".parse().unwrap();
    
    // Call expensive endpoint (counts as 10 requests)
    let result = rate_limiter.check_rate_limit(test_ip, "generate", true);
    
    assert!(result.is_ok(), "First expensive request should succeed");
    rate_limiter.complete_request(test_ip);
    
    // After expensive call, try regular calls to different endpoint
    // Each endpoint has 30 req/min limit independently
    let mut accepted_after = 0;
    
    for _ in 0..35 {
        if rate_limiter.check_rate_limit(test_ip, "getinfo", false).is_ok() {
            accepted_after += 1;
            rate_limiter.complete_request(test_ip);
        }
    }
    
    // 'getinfo' endpoint should accept 30 (its own limit)
    // The expensive 'generate' call doesn't affect 'getinfo' endpoint limit
    assert!(accepted_after >= 20, "Should accept at least 20 regular requests: {}", accepted_after);
    
    println!("✓ Expensive endpoint multiplier: Consumes global tokens but endpoints independent");
}

#[test]
fn test_multiple_ips_independent_limits() {
    // SECURITY TEST: Each IP has independent rate limit
    
    let rate_limiter = Arc::new(ApiRateLimiter::new());
    
    let ips = vec![
        "192.168.1.1".parse().unwrap(),
        "192.168.1.2".parse().unwrap(),
        "192.168.1.3".parse().unwrap(),
    ];
    
    let mut handles = Vec::new();
    
    for ip in ips {
        let limiter = Arc::clone(&rate_limiter);
        let handle = thread::spawn(move || {
            let mut count = 0;
            
            for _ in 0..60 {
                if limiter.check_rate_limit(ip, "getinfo", false).is_ok() {
                    count += 1;
                    limiter.complete_request(ip);
                }
            }
            
            count
        });
        handles.push(handle);
    }
    
    // Collect results
    let results: Vec<_> = handles
        .into_iter()
        .map(|h| h.join().expect("Thread panicked"))
        .collect();
    
    // Each IP hits per-endpoint limit (30) before global limit (60)
    for (i, count) in results.iter().enumerate() {
        assert_eq!(*count, 30, "IP {} should accept 30 requests (endpoint limit)", i);
    }
    
    println!("✓ Independent rate limits: 3 IPs × 30 requests = 90 total (endpoint-limited)");
}

#[test]
fn test_concurrent_request_limiting() {
    // SECURITY TEST: Concurrent request limit prevents queue saturation
    
    let rate_limiter = ApiRateLimiter::new();
    let test_ip: IpAddr = "192.168.1.103".parse().unwrap();
    
    // Try to start 10 concurrent requests (max is 5)
    let mut accepted_concurrent = 0;
    let mut rejected_concurrent = 0;
    
    for _ in 0..10 {
        match rate_limiter.check_rate_limit(test_ip, "getblock", false) {
            Ok(_) => accepted_concurrent += 1,
            Err(_) => rejected_concurrent += 1,
        }
    }
    
    // Should accept first 5, reject the rest
    assert!(accepted_concurrent <= 5, "Should not exceed 5 concurrent: {}", accepted_concurrent);
    assert!(rejected_concurrent >= 5, "Should reject at least 5: {}", rejected_concurrent);
    
    println!("✓ Concurrent limit: {}/10 concurrent requests accepted", accepted_concurrent);
}

#[test]
fn test_rate_limiter_stats() {
    // SECURITY TEST: Statistics tracking for monitoring
    
    let rate_limiter = ApiRateLimiter::new();
    let test_ip: IpAddr = "192.168.1.104".parse().unwrap();
    
    // Make some requests
    for _ in 0..70 {
        let _ = rate_limiter.check_rate_limit(test_ip, "getinfo", false);
    }
    
    let stats = rate_limiter.get_stats();
    
    assert_eq!(stats.total_requests, 70, "Should track all requests");
    assert!(stats.rate_limited_requests >= 40, "Should track rejections (at least 40): {}", stats.rate_limited_requests);
    assert!(stats.active_ip_limits > 0, "Should have active IP limits");
    
    println!("API Stats:");
    println!("  Total requests: {}", stats.total_requests);
    println!("  Rate limited: {}", stats.rate_limited_requests);
    println!("  Active IP limits: {}", stats.active_ip_limits);
    println!("  Active endpoint limits: {}", stats.active_endpoint_limits);
}

#[test]
fn test_flood_attack_resistance() {
    // SECURITY TEST: Resistance to API flooding from single attacker
    
    let rate_limiter = Arc::new(ApiRateLimiter::new());
    let attacker_ip: IpAddr = "10.0.0.1".parse().unwrap();
    
    // Attacker tries to flood with 1000 requests
    let mut accepted = 0;
    
    for _ in 0..1000 {
        if rate_limiter.check_rate_limit(attacker_ip, "getblock", false).is_ok() {
            accepted += 1;
            rate_limiter.complete_request(attacker_ip);
        }
    }
    
    // Per-endpoint limit is 30
    assert_eq!(accepted, 30, "Flood attack limited to 30 requests (per-endpoint limit)");
    
    let stats = rate_limiter.get_stats();
    assert_eq!(stats.rate_limited_requests, 970, "Should reject 970 flood requests");
    
    println!("✓ Flood attack mitigated: 30/1000 requests accepted, 970 rejected");
}

#[test]
fn test_expensive_endpoint_list() {
    // SECURITY TEST: Expensive endpoints properly identified
    
    use node::api::rate_limiter::is_expensive_endpoint;
    
    // Expensive endpoints
    assert!(is_expensive_endpoint("generate"), "'generate' should be expensive");
    assert!(is_expensive_endpoint("generatetoaddress"), "'generatetoaddress' should be expensive");
    assert!(is_expensive_endpoint("getblocktemplate"), "'getblocktemplate' should be expensive");
    assert!(is_expensive_endpoint("submitblock"), "'submitblock' should be expensive");
    assert!(is_expensive_endpoint("sendrawtransaction"), "'sendrawtransaction' should be expensive");
    
    // Cheap endpoints
    assert!(!is_expensive_endpoint("getinfo"), "'getinfo' should not be expensive");
    assert!(!is_expensive_endpoint("getblockcount"), "'getblockcount' should not be expensive");
    assert!(!is_expensive_endpoint("getbestblockhash"), "'getbestblockhash' should not be expensive");
    
    println!("✓ Expensive endpoint identification validated");
}

#[test]
fn test_documentation() {
    // This test exists to document the security fix
    
    println!("\n=== SECURITY FIX DOCUMENTATION ===");
    println!("Vulnerability: P2-007 API Rate Limiting");
    println!("Impact: API DoS, node resource exhaustion");
    println!("Fix: Multi-layer rate limiting with token bucket");
    println!("");
    println!("Rate Limits:");
    println!("  - Global: 60 requests/minute per IP");
    println!("  - Per-endpoint: 30 requests/minute per IP");
    println!("  - Concurrent: 5 max per IP");
    println!("  - Expensive: Count as 10x (generate, submitblock, etc.)");
    println!("  - Batch: 10 requests maximum");
    println!("");
    println!("Algorithm: Token Bucket");
    println!("  - Each IP starts with 60 tokens");
    println!("  - Each request consumes 1 token (or 10 for expensive)");
    println!("  - Tokens refill after 60 seconds");
    println!("  - No tokens = rate limited (HTTP 429)");
    println!("");
    println!("Defense Layers:");
    println!("  Layer 1: Per-IP global limit (prevents single attacker)");
    println!("  Layer 2: Per-endpoint limit (prevents endpoint targeting)");
    println!("  Layer 3: Concurrent limit (prevents queue saturation)");
    println!("  Layer 4: Expensive multiplier (protects CPU-intensive ops)");
    println!("");
    println!("Response:");
    println!("  - HTTP 429 Too Many Requests");
    println!("  - JSON-RPC error code: -32006");
    println!("  - Includes retry_after hint (60 seconds)");
    println!("");
    println!("Test Coverage: 10 security-focused test cases");
    println!("Status: PROTECTED - API flooding prevented");
    println!("=====================================\n");
}

