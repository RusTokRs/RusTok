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

/// Resolves the REST path against the origin that served the admin bundle.
///
/// Module admin packages never invent an API base URL: the host that served the
/// bundle proxies `/api/*` to the server. There is therefore no configurable base
/// (and no built-in loopback default that silently pointed every
/// server-rendered admin at the developer's machine), and outside the browser the
/// REST transport does not exist at all — `selected_transport_path` picks the
/// in-process server function instead.
fn api_url(path: &str) -> Result<String, ApiError> {
    #[cfg(target_arch = "wasm32")]
    {
        let origin = web_sys::window()
            .and_then(|window| window.location().origin().ok())
            .filter(|origin| !origin.is_empty())
            .ok_or_else(|| {
                ApiError::Rest(
                    "cannot determine the admin origin for the channel REST transport".to_string(),
                )
            })?;
        Ok(format!("{origin}{path}"))
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        Err(ApiError::Rest(format!(
            "the channel REST transport is browser-only; `{path}` must go through the channel admin server functions"
        )))
    }
}

fn http_client() -> &'static reqwest::Client {
    static CLIENT: std::sync::OnceLock<reqwest::Client> = std::sync::OnceLock::new();
    CLIENT.get_or_init(reqwest::Client::new)
}

pub(super) async fn get_json<T>(
    path: &str,
    token: Option<String>,
    tenant_slug: Option<String>,
) -> Result<T, ApiError>
where
    T: DeserializeOwned,
{
    let mut request = http_client().get(api_url(path)?);
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
    let mut request = http_client()
        .post(api_url(path)?)
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
    let mut request = http_client()
        .patch(api_url(path)?)
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
    let mut request = http_client().delete(api_url(path)?);
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

    if let Ok(payload) = serde_json::from_str::<ApiErrorPayload>(trimmed)
        && let Some(message) = payload
            .message
            .as_deref()
            .or(payload.error.as_deref())
            .map(str::trim)
            .filter(|value| !value.is_empty())
    {
        return message.to_string();
    }

    trimmed.to_string()
}
