use reqwest::header::{AUTHORIZATION, CONTENT_TYPE};
use serde::de::DeserializeOwned;
use serde::Deserialize;
use serde::Serialize;

use crate::model::{
    BindChannelModulePayload, BindChannelOauthAppPayload, ChannelAdminBootstrap,
    ChannelModuleBindingRecord, ChannelOauthAppRecord, ChannelRecord, ChannelTargetRecord,
    CreateChannelPayload, CreateChannelTargetPayload,
};

pub type ApiError = String;

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

async fn get_json<T>(
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
        request = request.header(AUTHORIZATION, format!("Bearer {token}"));
    }
    if let Some(tenant_slug) = tenant_slug {
        request = request.header("X-Tenant-ID", tenant_slug);
    }

    let response = request
        .send()
        .await
        .map_err(|err| format!("request failed: {err}"))?;
    if !response.status().is_success() {
        return Err(extract_api_error(response).await);
    }

    response
        .json::<T>()
        .await
        .map_err(|err| format!("invalid response payload: {err}"))
}

async fn post_json<B, T>(
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
        .header(CONTENT_TYPE, "application/json")
        .json(body);
    if let Some(token) = token {
        request = request.header(AUTHORIZATION, format!("Bearer {token}"));
    }
    if let Some(tenant_slug) = tenant_slug {
        request = request.header("X-Tenant-ID", tenant_slug);
    }

    let response = request
        .send()
        .await
        .map_err(|err| format!("request failed: {err}"))?;
    if !response.status().is_success() {
        return Err(extract_api_error(response).await);
    }

    response
        .json::<T>()
        .await
        .map_err(|err| format!("invalid response payload: {err}"))
}

async fn patch_json<B, T>(
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
        .header(CONTENT_TYPE, "application/json")
        .json(body);
    if let Some(token) = token {
        request = request.header(AUTHORIZATION, format!("Bearer {token}"));
    }
    if let Some(tenant_slug) = tenant_slug {
        request = request.header("X-Tenant-ID", tenant_slug);
    }

    let response = request
        .send()
        .await
        .map_err(|err| format!("request failed: {err}"))?;
    if !response.status().is_success() {
        return Err(extract_api_error(response).await);
    }

    response
        .json::<T>()
        .await
        .map_err(|err| format!("invalid response payload: {err}"))
}

async fn delete_json<T>(
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
        request = request.header(AUTHORIZATION, format!("Bearer {token}"));
    }
    if let Some(tenant_slug) = tenant_slug {
        request = request.header("X-Tenant-ID", tenant_slug);
    }

    let response = request
        .send()
        .await
        .map_err(|err| format!("request failed: {err}"))?;
    if !response.status().is_success() {
        return Err(extract_api_error(response).await);
    }

    response
        .json::<T>()
        .await
        .map_err(|err| format!("invalid response payload: {err}"))
}

pub async fn fetch_bootstrap(
    token: Option<String>,
    tenant_slug: Option<String>,
) -> Result<ChannelAdminBootstrap, ApiError> {
    get_json("/api/channels/bootstrap", token, tenant_slug).await
}

pub async fn create_channel(
    token: Option<String>,
    tenant_slug: Option<String>,
    payload: &CreateChannelPayload,
) -> Result<ChannelRecord, ApiError> {
    post_json("/api/channels/", payload, token, tenant_slug).await
}

pub async fn make_default_channel(
    token: Option<String>,
    tenant_slug: Option<String>,
    channel_id: &str,
) -> Result<ChannelRecord, ApiError> {
    post_json(
        &format!("/api/channels/{channel_id}/default"),
        &serde_json::json!({}),
        token,
        tenant_slug,
    )
    .await
}

pub async fn create_target(
    token: Option<String>,
    tenant_slug: Option<String>,
    channel_id: &str,
    payload: &CreateChannelTargetPayload,
) -> Result<ChannelTargetRecord, ApiError> {
    post_json(
        &format!("/api/channels/{channel_id}/targets"),
        payload,
        token,
        tenant_slug,
    )
    .await
}

pub async fn update_target(
    token: Option<String>,
    tenant_slug: Option<String>,
    channel_id: &str,
    target_id: &str,
    payload: &CreateChannelTargetPayload,
) -> Result<ChannelTargetRecord, ApiError> {
    patch_json(
        &format!("/api/channels/{channel_id}/targets/{target_id}"),
        payload,
        token,
        tenant_slug,
    )
    .await
}

pub async fn bind_module(
    token: Option<String>,
    tenant_slug: Option<String>,
    channel_id: &str,
    payload: &BindChannelModulePayload,
) -> Result<serde_json::Value, ApiError> {
    post_json(
        &format!("/api/channels/{channel_id}/modules"),
        payload,
        token,
        tenant_slug,
    )
    .await
}

pub async fn bind_oauth_app(
    token: Option<String>,
    tenant_slug: Option<String>,
    channel_id: &str,
    payload: &BindChannelOauthAppPayload,
) -> Result<serde_json::Value, ApiError> {
    post_json(
        &format!("/api/channels/{channel_id}/oauth-apps"),
        payload,
        token,
        tenant_slug,
    )
    .await
}

pub async fn delete_target(
    token: Option<String>,
    tenant_slug: Option<String>,
    channel_id: &str,
    target_id: &str,
) -> Result<ChannelTargetRecord, ApiError> {
    delete_json(
        &format!("/api/channels/{channel_id}/targets/{target_id}"),
        token,
        tenant_slug,
    )
    .await
}

pub async fn delete_module_binding(
    token: Option<String>,
    tenant_slug: Option<String>,
    channel_id: &str,
    binding_id: &str,
) -> Result<ChannelModuleBindingRecord, ApiError> {
    delete_json(
        &format!("/api/channels/{channel_id}/modules/{binding_id}"),
        token,
        tenant_slug,
    )
    .await
}

pub async fn delete_oauth_app_binding(
    token: Option<String>,
    tenant_slug: Option<String>,
    channel_id: &str,
    binding_id: &str,
) -> Result<ChannelOauthAppRecord, ApiError> {
    delete_json(
        &format!("/api/channels/{channel_id}/oauth-apps/{binding_id}"),
        token,
        tenant_slug,
    )
    .await
}

async fn extract_api_error(response: reqwest::Response) -> ApiError {
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
