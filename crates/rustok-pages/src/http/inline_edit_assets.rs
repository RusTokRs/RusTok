use std::fmt::Write as _;

use axum::{
    Router,
    body::Body,
    http::{
        HeaderMap, StatusCode,
        header::{CACHE_CONTROL, CONTENT_TYPE, ETAG, IF_NONE_MATCH},
    },
    response::Response,
    routing::get,
};
use sha2::{Digest, Sha256};

pub(super) const PAGES_INLINE_EDIT_BOOTSTRAP_PATH: &str =
    "/assets/pages-inline-edit-bootstrap.js";
pub(super) const PAGES_INLINE_EDIT_MODULE_PATH: &str =
    "/assets/pages-inline-edit/rustok_storefront.js";
pub(super) const PAGES_INLINE_EDIT_WASM_PATH: &str =
    "/assets/pages-inline-edit/rustok_storefront_bg.wasm";

const REVALIDATE_CACHE_CONTROL: &str = "public, max-age=0, must-revalidate";
const SAME_ORIGIN_RESOURCE_POLICY: &str = "same-origin";

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

fn asset_response(headers: &HeaderMap, bytes: &'static [u8], content_type: &'static str) -> Response {
    let etag = content_etag(bytes);
    let not_modified = headers
        .get(IF_NONE_MATCH)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| if_none_match_matches(value, etag.as_str()));

    let mut builder = Response::builder()
        .header(CACHE_CONTROL, REVALIDATE_CACHE_CONTROL)
        .header(ETAG, etag)
        .header("cross-origin-resource-policy", SAME_ORIGIN_RESOURCE_POLICY);
    if not_modified {
        return builder
            .status(StatusCode::NOT_MODIFIED)
            .body(Body::empty())
            .expect("static Pages inline asset response headers are valid");
    }

    builder = builder.header(CONTENT_TYPE, content_type);
    builder
        .status(StatusCode::OK)
        .body(Body::from(bytes))
        .expect("static Pages inline asset response headers are valid")
}

fn content_etag(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut encoded = String::with_capacity(digest.len() * 2 + 2);
    encoded.push('"');
    for byte in digest {
        write!(&mut encoded, "{byte:02x}").expect("writing to String cannot fail");
    }
    encoded.push('"');
    encoded
}

fn if_none_match_matches(value: &str, etag: &str) -> bool {
    value.split(',').map(str::trim).any(|candidate| {
        candidate == "*" || candidate.strip_prefix("W/").unwrap_or(candidate) == etag
    })
}

#[cfg(test)]
mod tests {
    use super::{
        PAGES_INLINE_EDIT_BOOTSTRAP_PATH, PAGES_INLINE_EDIT_MODULE_PATH,
        PAGES_INLINE_EDIT_WASM_PATH, content_etag, if_none_match_matches,
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

    #[test]
    fn content_etags_are_stable_and_support_weak_if_none_match() {
        let etag = content_etag(b"pages-inline-edit");
        assert_eq!(etag.len(), 66);
        assert!(if_none_match_matches(etag.as_str(), etag.as_str()));
        assert!(if_none_match_matches(format!("W/{etag}").as_str(), etag.as_str()));
        assert!(if_none_match_matches("*", etag.as_str()));
        assert!(!if_none_match_matches("\"different\"", etag.as_str()));
    }
}
