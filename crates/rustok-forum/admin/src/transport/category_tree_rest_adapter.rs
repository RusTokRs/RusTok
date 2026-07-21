#[cfg(target_arch = "wasm32")]
use leptos::web_sys;
use reqwest::Method;

use crate::model::CategoryTreeResponse;

use super::ApiError;

const AUTH_HEADER: &str = "Authorization";
const TENANT_HEADER: &str = "X-Tenant-Slug";
const ACCEPT_LANGUAGE_HEADER: &str = "Accept-Language";

pub async fn fetch_category_tree(
    token: Option<String>,
    tenant_slug: Option<String>,
    locale: String,
) -> Result<CategoryTreeResponse, ApiError> {
    let client = reqwest::Client::new();
    let mut request = client.request(
        Method::GET,
        format!("{}/categories/tree?locale={locale}", api_base_url()),
    );
    if let Some(value) = token {
        request = request.header(AUTH_HEADER, format!("Bearer {value}"));
    }
    if let Some(value) = tenant_slug {
        request = request.header(TENANT_HEADER, value);
    }
    request = request.header(ACCEPT_LANGUAGE_HEADER, locale);

    let response = request
        .send()
        .await
        .map_err(|error| format!("Network error: {error}"))?;
    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(if body.trim().is_empty() {
            format!("HTTP {status}")
        } else {
            format!("HTTP {status}: {}", body.trim())
        });
    }
    response
        .json::<CategoryTreeResponse>()
        .await
        .map_err(|error| format!("Invalid JSON response: {error}"))
}

fn api_base_url() -> String {
    if let Some(url) = option_env!("RUSTOK_API_URL") {
        return format!("{url}/api/forum");
    }

    #[cfg(target_arch = "wasm32")]
    {
        let origin = web_sys::window()
            .and_then(|window| window.location().origin().ok())
            .unwrap_or_else(|| "http://localhost:5150".to_string());
        format!("{origin}/api/forum")
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let base =
            std::env::var("RUSTOK_API_URL").unwrap_or_else(|_| "http://localhost:5150".to_string());
        format!("{base}/api/forum")
    }
}
