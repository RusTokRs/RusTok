use reqwest::header::{AUTHORIZATION, CONTENT_TYPE};
use serde::de::DeserializeOwned;
use serde::Serialize;

use crate::model::{
    BindChannelModulePayload, BindChannelOauthAppPayload, ChannelAdminBootstrap, ChannelRecord,
    ChannelTargetRecord, CreateChannelPayload, CreateChannelTargetPayload,
};

pub type ApiError = String;

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
        return Err(format!("request failed with status {}", response.status()));
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
        return Err(format!("request failed with status {}", response.status()));
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
