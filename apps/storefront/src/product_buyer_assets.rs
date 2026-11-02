//! Host-owned immutable Product buyer assets. A build with this feature requires real WASM.

use axum::{Router, http::HeaderMap, response::Response, routing::get};
use rustok_web::embedded_asset_response;

const BOOTSTRAP: &[u8] = include_bytes!("../public/assets/product-buyer-bootstrap.js");
const MODULE: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../target/site/assets/product-buyer/rustok_storefront.js"
));
const WASM: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../target/site/assets/product-buyer/rustok_storefront_bg.wasm"
));

pub(super) fn router() -> Router {
    Router::new()
        .route(
            "/assets/product-buyer-bootstrap.js",
            get(|headers| asset(headers, BOOTSTRAP, "text/javascript; charset=utf-8")),
        )
        .route(
            "/assets/product-buyer/rustok_storefront.js",
            get(|headers| asset(headers, MODULE, "text/javascript; charset=utf-8")),
        )
        .route(
            "/assets/product-buyer/rustok_storefront_bg.wasm",
            get(|headers| asset(headers, WASM, "application/wasm")),
        )
}

async fn asset(headers: HeaderMap, bytes: &'static [u8], content_type: &'static str) -> Response {
    embedded_asset_response(
        &headers,
        bytes,
        content_type,
        "public, max-age=0, must-revalidate",
        "static Product buyer asset",
    )
}
