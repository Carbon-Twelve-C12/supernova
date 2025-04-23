use actix_web::{test, web, App};
use serde_json::Value;
use std::sync::Arc;

// Import our API modules
use node::api::server::create_app;
use node::api::types::{BlockchainInfo, MempoolInfo, NetworkInfo};

/// Mock dependency injection for testing
struct MockDependencies {
    // Add mock dependencies as needed
}

impl MockDependencies {
    fn new() -> Self {
        Self {}
    }
    
    fn inject_into_app_data() -> web::Data<Arc<MockDependencies>> {
        web::Data::new(Arc::new(Self::new()))
    }
}

#[actix_rt::test]
async fn test_health_endpoint() {
    // Initialize mock dependencies
    let dependencies = MockDependencies::inject_into_app_data();
    
    // Create test app
    let app = test::init_service(
        App::new()
            .app_data(dependencies.clone())
            .configure(|cfg| create_app(cfg, dependencies.clone()))
    ).await;
    
    // Send request
    let req = test::TestRequest::get().uri("/health").to_request();
    let resp = test::call_service(&app, req).await;
    
    // Assert response
    assert!(resp.status().is_success());
    
    // Parse response body
    let body = test::read_body(resp).await;
    let json: Value = serde_json::from_slice(&body).unwrap();
    
    // Verify response structure
    assert!(json.get("status").is_some());
    assert!(json.get("version").is_some());
    assert!(json.get("name").is_some());
    
    // Verify status value
    assert_eq!(json["status"], "ok");
}

#[actix_rt::test]
async fn test_blockchain_info_endpoint() {
    // Initialize mock dependencies
    let dependencies = MockDependencies::inject_into_app_data();
    
    // Create test app
    let app = test::init_service(
        App::new()
            .app_data(dependencies.clone())
            .configure(|cfg| create_app(cfg, dependencies.clone()))
    ).await;
    
    // Send request
    let req = test::TestRequest::get().uri("/api/v1/blockchain/info").to_request();
    let resp = test::call_service(&app, req).await;
    
    // Assert response
    assert!(resp.status().is_success());
    
    // Parse response body and verify it's a valid BlockchainInfo
    let body = test::read_body(resp).await;
    let _blockchain_info: BlockchainInfo = serde_json::from_slice(&body).unwrap();
}

#[actix_rt::test]
async fn test_mempool_info_endpoint() {
    // Initialize mock dependencies
    let dependencies = MockDependencies::inject_into_app_data();
    
    // Create test app
    let app = test::init_service(
        App::new()
            .app_data(dependencies.clone())
            .configure(|cfg| create_app(cfg, dependencies.clone()))
    ).await;
    
    // Send request
    let req = test::TestRequest::get().uri("/api/v1/mempool/info").to_request();
    let resp = test::call_service(&app, req).await;
    
    // Assert response
    assert!(resp.status().is_success());
    
    // Parse response body and verify it's a valid MempoolInfo
    let body = test::read_body(resp).await;
    let _mempool_info: MempoolInfo = serde_json::from_slice(&body).unwrap();
}

#[actix_rt::test]
async fn test_network_info_endpoint() {
    // Initialize mock dependencies
    let dependencies = MockDependencies::inject_into_app_data();
    
    // Create test app
    let app = test::init_service(
        App::new()
            .app_data(dependencies.clone())
            .configure(|cfg| create_app(cfg, dependencies.clone()))
    ).await;
    
    // Send request
    let req = test::TestRequest::get().uri("/api/v1/network/info").to_request();
    let resp = test::call_service(&app, req).await;
    
    // Assert response
    assert!(resp.status().is_success());
    
    // Parse response body and verify it's a valid NetworkInfo
    let body = test::read_body(resp).await;
    let _network_info: NetworkInfo = serde_json::from_slice(&body).unwrap();
}

#[actix_rt::test]
async fn test_block_by_height_endpoint() {
    // Initialize mock dependencies
    let dependencies = MockDependencies::inject_into_app_data();
    
    // Create test app
    let app = test::init_service(
        App::new()
            .app_data(dependencies.clone())
            .configure(|cfg| create_app(cfg, dependencies.clone()))
    ).await;
    
    // Send request (get genesis block)
    let req = test::TestRequest::get().uri("/api/v1/blockchain/block/height/0").to_request();
    let resp = test::call_service(&app, req).await;
    
    // Assert response
    assert!(resp.status().is_success());
}

#[actix_rt::test]
async fn test_invalid_block_height_endpoint() {
    // Initialize mock dependencies
    let dependencies = MockDependencies::inject_into_app_data();
    
    // Create test app
    let app = test::init_service(
        App::new()
            .app_data(dependencies.clone())
            .configure(|cfg| create_app(cfg, dependencies.clone()))
    ).await;
    
    // Send request for an invalid block height (extremely high)
    let req = test::TestRequest::get().uri("/api/v1/blockchain/block/height/999999999").to_request();
    let resp = test::call_service(&app, req).await;
    
    // Assert 404 Not Found response for non-existent block height
    assert_eq!(resp.status().as_u16(), 404);
}

#[actix_rt::test]
async fn test_submit_invalid_transaction() {
    // Initialize mock dependencies
    let dependencies = MockDependencies::inject_into_app_data();
    
    // Create test app
    let app = test::init_service(
        App::new()
            .app_data(dependencies.clone())
            .configure(|cfg| create_app(cfg, dependencies.clone()))
    ).await;
    
    // Invalid transaction data
    let invalid_tx_data = r#"{"raw_tx": "invalid_hex_data"}"#;
    
    // Send request
    let req = test::TestRequest::post()
        .uri("/api/v1/blockchain/transaction")
        .set_payload(invalid_tx_data)
        .insert_header(("content-type", "application/json"))
        .to_request();
    
    let resp = test::call_service(&app, req).await;
    
    // Assert 400 Bad Request for invalid transaction data
    assert_eq!(resp.status().as_u16(), 400);
}

#[actix_rt::test]
async fn test_openapi_spec_endpoint() {
    // Initialize mock dependencies
    let dependencies = MockDependencies::inject_into_app_data();
    
    // Create test app
    let app = test::init_service(
        App::new()
            .app_data(dependencies.clone())
            .configure(|cfg| create_app(cfg, dependencies.clone()))
    ).await;
    
    // Send request
    let req = test::TestRequest::get().uri("/api-docs/openapi.json").to_request();
    let resp = test::call_service(&app, req).await;
    
    // Assert response
    assert!(resp.status().is_success());
    
    // Verify we get a valid OpenAPI JSON document
    let body = test::read_body(resp).await;
    let json: Value = serde_json::from_slice(&body).unwrap();
    
    // Check for basic OpenAPI structure
    assert!(json.get("openapi").is_some());
    assert!(json.get("info").is_some());
    assert!(json.get("paths").is_some());
    assert!(json.get("components").is_some());
} 