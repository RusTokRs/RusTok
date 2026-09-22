use axum::{Router, http::HeaderMap, response::Response, routing::get};
use rustok_web::embedded_asset_response;

const BOOTSTRAP_PATH: &str = "/assets/blog-comment-bootstrap.js";
const MODULE_PATH: &str = "/assets/blog-comment/rustok_storefront.js";
const WASM_PATH: &str = "/assets/blog-comment/rustok_storefront_bg.wasm";
const CACHE_POLICY: &str = "public, max-age=0, must-revalidate";

const BOOTSTRAP: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../target/site/assets/blog-comment-bootstrap.js"
));
const MODULE: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../target/site/assets/blog-comment/rustok_storefront.js"
));
const WASM: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../target/site/assets/blog-comment/rustok_storefront_bg.wasm"
));

pub(super) fn router() -> Router {
    Router::new()
        .route(
            BOOTSTRAP_PATH,
            get(|headers| asset(headers, BOOTSTRAP, "text/javascript; charset=utf-8")),
        )
        .route(
            MODULE_PATH,
            get(|headers| asset(headers, MODULE, "text/javascript; charset=utf-8")),
        )
        .route(
            WASM_PATH,
            get(|headers| asset(headers, WASM, "application/wasm")),
        )
}

async fn asset(headers: HeaderMap, bytes: &'static [u8], content_type: &'static str) -> Response {
    embedded_asset_response(
        &headers,
        bytes,
        content_type,
        CACHE_POLICY,
        "static Blog comment response",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn asset_paths_are_same_origin_and_unversioned() {
        assert_eq!(BOOTSTRAP_PATH, "/assets/blog-comment-bootstrap.js");
        assert_eq!(MODULE_PATH, "/assets/blog-comment/rustok_storefront.js");
        assert_eq!(WASM_PATH, "/assets/blog-comment/rustok_storefront_bg.wasm");
    }
}
