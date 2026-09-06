use axum::{Router, http::HeaderMap, response::Response, routing::get};
use rustok_web::embedded_asset_response;

pub(super) const PAGES_INLINE_EDIT_BOOTSTRAP_PATH: &str = "/assets/pages-inline-edit-bootstrap.js";
pub(super) const PAGES_INLINE_EDIT_MODULE_PATH: &str =
    "/assets/pages-inline-edit/rustok_storefront.js";
pub(super) const PAGES_INLINE_EDIT_WASM_PATH: &str =
    "/assets/pages-inline-edit/rustok_storefront_bg.wasm";

const REVALIDATE_CACHE_CONTROL: &str = "public, max-age=0, must-revalidate";

const BOOTSTRAP_BYTES: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../target/site/assets/pages-inline-edit-bootstrap.js"
));
const MODULE_BYTES: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../target/site/assets/pages-inline-edit/rustok_storefront.js"
));
const WASM_BYTES: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../target/site/assets/pages-inline-edit/rustok_storefront_bg.wasm"
));

pub(super) fn router() -> Router {
    Router::new()
        .route(PAGES_INLINE_EDIT_BOOTSTRAP_PATH, get(bootstrap_asset))
        .route(PAGES_INLINE_EDIT_MODULE_PATH, get(module_asset))
        .route(PAGES_INLINE_EDIT_WASM_PATH, get(wasm_asset))
}

async fn bootstrap_asset(headers: HeaderMap) -> Response {
    asset_response(&headers, BOOTSTRAP_BYTES, "text/javascript; charset=utf-8")
}

async fn module_asset(headers: HeaderMap) -> Response {
    asset_response(&headers, MODULE_BYTES, "text/javascript; charset=utf-8")
}

async fn wasm_asset(headers: HeaderMap) -> Response {
    asset_response(&headers, WASM_BYTES, "application/wasm")
}

fn asset_response(
    headers: &HeaderMap,
    bytes: &'static [u8],
    content_type: &'static str,
) -> Response {
    embedded_asset_response(
        headers,
        bytes,
        content_type,
        REVALIDATE_CACHE_CONTROL,
        "static Pages inline asset response",
    )
}

#[cfg(test)]
mod tests {
    use super::{
        PAGES_INLINE_EDIT_BOOTSTRAP_PATH, PAGES_INLINE_EDIT_MODULE_PATH,
        PAGES_INLINE_EDIT_WASM_PATH,
    };

    #[test]
    fn fixed_asset_paths_match_the_authoring_bootstrap_contract() {
        assert_eq!(
            PAGES_INLINE_EDIT_BOOTSTRAP_PATH,
            "/assets/pages-inline-edit-bootstrap.js"
        );
        assert_eq!(
            PAGES_INLINE_EDIT_MODULE_PATH,
            "/assets/pages-inline-edit/rustok_storefront.js"
        );
        assert_eq!(
            PAGES_INLINE_EDIT_WASM_PATH,
            "/assets/pages-inline-edit/rustok_storefront_bg.wasm"
        );
    }
}
