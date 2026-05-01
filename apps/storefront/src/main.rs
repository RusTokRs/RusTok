#[cfg(all(feature = "ssr", not(feature = "csr")))]
use axum::{
    http::{header, StatusCode},
    response::IntoResponse,
    routing::get,
};
#[cfg(all(feature = "ssr", not(feature = "csr")))]
use std::path::PathBuf;

#[cfg(all(feature = "ssr", not(feature = "csr")))]
fn static_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("static")
}

#[cfg(all(feature = "ssr", not(feature = "csr")))]
async fn css_handler() -> impl IntoResponse {
    let css_path = static_dir().join("app.css");
    match tokio::fs::read(css_path).await {
        Ok(contents) => ([(header::CONTENT_TYPE, "text/css")], contents).into_response(),
        Err(_) => StatusCode::NOT_FOUND.into_response(),
    }
}

#[cfg(all(feature = "ssr", not(feature = "csr")))]
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let app = rustok_storefront::router().route("/assets/app.css", get(css_handler));

    let listener = tokio::net::TcpListener::bind("[::1]:3100").await?;
    println!("Storefront SSR running on http://localhost:3100");
    axum::serve(listener, app.into_make_service()).await?;
    Ok(())
}

#[cfg(feature = "csr")]
fn main() {
    use leptos::prelude::*;
    use rustok_storefront::app::{StorefrontModulePage, StorefrontShell};

    console_error_panic_hook::set_once();

    let (locale, route_segment, query_params) = browser_route_context();
    let enabled_modules = debug_enabled_modules();

    mount_to_body(move || {
        if let Some(route_segment) = route_segment.clone() {
            view! {
                <StorefrontModulePage
                    locale=locale.clone()
                    enabled_modules=enabled_modules.clone()
                    route_segment=route_segment
                    query_params=query_params.clone()
                />
            }
            .into_any()
        } else {
            view! {
                <StorefrontShell
                    locale=locale.clone()
                    enabled_modules=enabled_modules.clone()
                    query_params=query_params.clone()
                />
            }
            .into_any()
        }
    });
}

#[cfg(feature = "csr")]
fn browser_route_context() -> (
    String,
    Option<String>,
    std::collections::HashMap<String, String>,
) {
    let location = web_sys::window()
        .expect("window is available in the storefront CSR runtime")
        .location();
    let pathname = location.pathname().unwrap_or_else(|_| "/".to_string());
    let search = location.search().unwrap_or_default();
    let query_params = parse_query_params(search.as_str());
    let segments = pathname
        .trim_matches('/')
        .split('/')
        .filter(|segment| !segment.is_empty())
        .collect::<Vec<_>>();
    let locale = segments
        .first()
        .filter(|segment| looks_like_locale(segment))
        .copied()
        .unwrap_or("en")
        .to_string();
    let route_offset = usize::from(
        segments
            .first()
            .is_some_and(|segment| looks_like_locale(segment)),
    );
    let route_segment = match segments.as_slice().get(route_offset..route_offset + 2) {
        Some(["modules", segment]) => Some((*segment).to_string()),
        _ => None,
    };

    (locale, route_segment, query_params)
}

#[cfg(feature = "csr")]
fn parse_query_params(search: &str) -> std::collections::HashMap<String, String> {
    search
        .trim_start_matches('?')
        .split('&')
        .filter(|item| !item.is_empty())
        .filter_map(|item| {
            let (key, value) = item.split_once('=').unwrap_or((item, ""));
            if key.is_empty() {
                None
            } else {
                Some((key.to_string(), value.to_string()))
            }
        })
        .collect()
}

#[cfg(feature = "csr")]
fn looks_like_locale(segment: &str) -> bool {
    let len = segment.len();
    (len == 2 || len == 5)
        && segment
            .chars()
            .all(|ch| ch.is_ascii_alphabetic() || ch == '-')
}

#[cfg(feature = "csr")]
fn debug_enabled_modules() -> Vec<String> {
    [
        "content",
        "cart",
        "customer",
        "product",
        "taxonomy",
        "region",
        "pricing",
        "inventory",
        "order",
        "payment",
        "fulfillment",
        "commerce",
        "pages",
        "blog",
        "forum",
        "search",
    ]
    .into_iter()
    .map(ToOwned::to_owned)
    .collect()
}

#[cfg(not(any(feature = "ssr", feature = "csr")))]
fn main() {}
