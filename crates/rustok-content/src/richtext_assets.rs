use axum::{Router, http::HeaderMap, response::Response, routing::get};
use rustok_web::embedded_asset_response;

mod generated {
    include!(concat!(env!("OUT_DIR"), "/richtext_assets.rs"));
}

const FRAME_PATH: &str = "/richtext/frame";
const ASSET_PREFIX: &str = "/richtext/frame/";
const DOCUMENT_CACHE_CONTROL: &str = "no-store";
const REVALIDATE_CACHE_CONTROL: &str = "public, max-age=0, must-revalidate";
const IMMUTABLE_CACHE_CONTROL: &str = "public, max-age=31536000, immutable";

pub fn router() -> Router {
    Router::new()
        .route(FRAME_PATH, get(frame))
        .route(
            path(generated::ADAPTER_NAME).as_str(),
            get(|headers| {
                asset(
                    headers,
                    generated::ADAPTER_BYTES,
                    "text/javascript; charset=utf-8",
                    REVALIDATE_CACHE_CONTROL,
                )
            }),
        )
        .route(
            path(generated::SCRIPT_NAME).as_str(),
            get(|headers| {
                asset(
                    headers,
                    generated::SCRIPT_BYTES,
                    "text/javascript; charset=utf-8",
                    IMMUTABLE_CACHE_CONTROL,
                )
            }),
        )
        .route(
            path(generated::STYLE_NAME).as_str(),
            get(|headers| {
                asset(
                    headers,
                    generated::STYLE_BYTES,
                    "text/css; charset=utf-8",
                    IMMUTABLE_CACHE_CONTROL,
                )
            }),
        )
}

async fn frame(headers: HeaderMap) -> Response {
    asset(
        headers,
        generated::FRAME_BYTES,
        "text/html; charset=utf-8",
        DOCUMENT_CACHE_CONTROL,
    )
    .await
}

async fn asset(
    headers: HeaderMap,
    bytes: &'static [u8],
    content_type: &'static str,
    cache_control: &'static str,
) -> Response {
    embedded_asset_response(
        &headers,
        bytes,
        content_type,
        cache_control,
        "embedded richtext frame response",
    )
}

fn path(name: &str) -> String {
    format!("{ASSET_PREFIX}{name}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_asset_names_are_unversioned_content_addresses() {
        assert_eq!(FRAME_PATH, "/richtext/frame");
        assert_eq!(generated::ADAPTER_NAME, "leptos-adapter.mjs");
        assert!(generated::SCRIPT_NAME.starts_with("richtext-frame."));
        assert!(generated::SCRIPT_NAME.ends_with(".js"));
        assert!(generated::STYLE_NAME.starts_with("richtext-frame."));
        assert!(generated::STYLE_NAME.ends_with(".css"));
    }
}
