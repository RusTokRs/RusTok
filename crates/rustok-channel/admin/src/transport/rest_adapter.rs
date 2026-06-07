#[cfg(target_arch = "wasm32")]
use leptos::web_sys;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use super::ApiError;

#[derive(Debug, Deserialize)]
struct ApiErrorPayload {
    error: Option<String>,
    message: Option<String>,
}

fn api_url(path: &str) -> String {
    #[cfg(target_arch = "wasm32")]
    {
        let origin = web_sys::window()
            .and_then(|window| window.location().origin().ok())
            .unwrap_or_else(|| "http://localhost:5150".to_string());
        format!("{origin}{path}")
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let base =
            std::env::var("RUSTOK_API_URL").unwrap_or_else(|_| "http://localhost:5150".to_string());
        format!("{base}{path}")
    }
}

pub(super) async fn get_json<T>(
    path: &str,
    token: Option<String>,
    tenant_slug: Option<String>,
) -> Result<T, ApiError>
where
    T: DeserializeOwned,
{
    let client = reqwest::Client::new();
    let mut request = client.get(api_url(path));
    if let Some(token) = token {
        request = request.header(reqwest::header::AUTHORIZATION, format!("Bearer {token}"));
    }
    if let Some(tenant_slug) = tenant_slug {
        request = request.header("X-Tenant-ID", tenant_slug);
    }

    let response = request
        .send()
        .await
        .map_err(|err| ApiError::Rest(format!("request failed: {err}")))?;
    if !response.status().is_success() {
        return Err(ApiError::Rest(extract_api_error(response).await));
    }

    response
        .json::<T>()
        .await
        .map_err(|err| ApiError::Rest(format!("invalid response payload: {err}")))
}

pub(super) async fn post_json<B, T>(
    path: &str,
    body: &B,
    token: Option<String>,
    tenant_slug: Option<String>,
) -> Result<T, ApiError>
where
    B: Serialize + ?Sized,
    T: DeserializeOwned,
{
    let client = reqwest::Client::new();
    let mut request = client
        .post(api_url(path))
        .header(reqwest::header::CONTENT_TYPE, "application/json")
        .json(body);
    if let Some(token) = token {
        request = request.header(reqwest::header::AUTHORIZATION, format!("Bearer {token}"));
    }
    if let Some(tenant_slug) = tenant_slug {
        request = request.header("X-Tenant-ID", tenant_slug);
    }

    let response = request
        .send()
        .await
        .map_err(|err| ApiError::Rest(format!("request failed: {err}")))?;
    if !response.status().is_success() {
        return Err(ApiError::Rest(extract_api_error(response).await));
    }

    response
        .json::<T>()
        .await
        .map_err(|err| ApiError::Rest(format!("invalid response payload: {err}")))
}

pub(super) async fn patch_json<B, T>(
    path: &str,
    body: &B,
    token: Option<String>,
    tenant_slug: Option<String>,
) -> Result<T, ApiError>
where
    B: Serialize + ?Sized,
    T: DeserializeOwned,
{
    let client = reqwest::Client::new();
    let mut request = client
        .patch(api_url(path))
        .header(reqwest::header::CONTENT_TYPE, "application/json")
        .json(body);
    if let Some(token) = token {
        request = request.header(reqwest::header::AUTHORIZATION, format!("Bearer {token}"));
    }
    if let Some(tenant_slug) = tenant_slug {
        request = request.header("X-Tenant-ID", tenant_slug);
    }

    let response = request
        .send()
        .await
        .map_err(|err| ApiError::Rest(format!("request failed: {err}")))?;
    if !response.status().is_success() {
        return Err(ApiError::Rest(extract_api_error(response).await));
    }

    response
        .json::<T>()
        .await
        .map_err(|err| ApiError::Rest(format!("invalid response payload: {err}")))
}

pub(super) async fn delete_json<T>(
    path: &str,
    token: Option<String>,
    tenant_slug: Option<String>,
) -> Result<T, ApiError>
where
    T: DeserializeOwned,
{
    let client = reqwest::Client::new();
    let mut request = client.delete(api_url(path));
    if let Some(token) = token {
        request = request.header(reqwest::header::AUTHORIZATION, format!("Bearer {token}"));
    }
    if let Some(tenant_slug) = tenant_slug {
        request = request.header("X-Tenant-ID", tenant_slug);
    }

    let response = request
        .send()
        .await
        .map_err(|err| ApiError::Rest(format!("request failed: {err}")))?;
    if !response.status().is_success() {
        return Err(ApiError::Rest(extract_api_error(response).await));
    }

    response
        .json::<T>()
        .await
        .map_err(|err| ApiError::Rest(format!("invalid response payload: {err}")))
}

async fn extract_api_error(response: reqwest::Response) -> String {
    let status = response.status();
    let text = response.text().await.unwrap_or_default();
    let trimmed = text.trim();

    if trimmed.is_empty() {
        return format!("request failed with status {status}");
    }

    if let Ok(payload) = serde_json::from_str::<ApiErrorPayload>(trimmed) {
        if let Some(message) = payload
            .message
            .as_deref()
            .or(payload.error.as_deref())
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            return message.to_string();
        }
    }

    trimmed.to_string()
}
