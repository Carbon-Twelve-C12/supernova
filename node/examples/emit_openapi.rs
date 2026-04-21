//! Emit the Supernova HTTP API OpenAPI 3.0 specification to stdout.
//!
//! Used by maintainers and CI to regenerate `docs/api/openapi.json`:
//!
//!     cargo run -p node --example emit_openapi --release > docs/api/openapi.json
//!
//! The produced document is the same one served at `/api-docs/openapi.json`
//! by a running node, derived from the `utoipa` annotations on the handler
//! and type definitions in this crate.

use node::api::docs::ApiDoc;
use utoipa::OpenApi;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let spec = ApiDoc::openapi();
    let json = serde_json::to_string_pretty(&spec)?;
    println!("{}", json);
    Ok(())
}
