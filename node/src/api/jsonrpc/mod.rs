//! JSON-RPC server implementation
//! 
//! This module implements the JSON-RPC 2.0 API for supernova blockchain.

mod handlers;
mod types;

use actix_web::{web, HttpResponse, Responder, http::header};
use serde_json::Value;
use std::sync::Arc;
use crate::node::Node;
use types::{JsonRpcRequest, JsonRpcResponse, JsonRpcError, ErrorCode};
use crate::api::docs::jsonrpc::JsonRpcDoc;
use serde::{Deserialize, Serialize};
use crate::api::error::{ApiError, ApiResult};

/// JSON-RPC request handler
pub async fn handle_jsonrpc(
    request: web::Json<JsonRpcRequest>,
    node: web::Data<Arc<Node>>,
) -> impl Responder {
    let req = request.into_inner();
    let id = req.id.clone();
    
    // Validate JSON-RPC version
    if req.jsonrpc != "2.0" {
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
            e.code,
            e.message,
            e.data,
        ),
    };
    
    HttpResponse::Ok().json(result)
}

/// Batch JSON-RPC request handler
pub async fn handle_jsonrpc_batch(
    requests: web::Json<Vec<JsonRpcRequest>>,
    node: web::Data<Arc<Node>>,
) -> impl Responder {
    if requests.is_empty() {
        return HttpResponse::Ok().json(JsonRpcResponse::error(
            Value::Null,
            ErrorCode::InvalidRequest,
            "Empty batch".to_string(),
            None,
        ));
    }
    
    let mut responses = Vec::with_capacity(requests.len());
    
    for req in requests.iter() {
        let id = req.id.clone();
        
        // Validate JSON-RPC version
        if req.jsonrpc != "2.0" {
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
                e.code,
                e.message,
                e.data,
            ),
        };
        
        responses.push(result);
    }
    
    HttpResponse::Ok().json(responses)
}

/// Serve JSON-RPC documentation
pub async fn get_docs() -> impl Responder {
    let docs = JsonRpcDoc::markdown();
    
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
    cfg.service(
        web::resource("")
            .route(web::post().to(handle_jsonrpc))
    )
    .service(
        web::resource("/batch")
            .route(web::post().to(handle_jsonrpc_batch))
    )
    .service(
        web::resource("/docs")
            .route(web::get().to(get_docs))
    );
} 