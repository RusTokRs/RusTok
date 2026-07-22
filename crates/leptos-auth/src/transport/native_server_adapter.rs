use leptos::prelude::*;
#[cfg(feature = "ssr")]
use serde_json::json;

#[cfg(feature = "ssr")]
use super::RESET_REQUEST_MESSAGE;
use super::{NativeAuthPayload, NativeCurrentUserPayload};
#[cfg(feature = "ssr")]
use crate::AuthUser;

#[cfg(feature = "ssr")]
use super::{
    RestApiErrorPayload, RestAuthResponse, RestStatusResponse, RestUserResponse, get_api_url,
    rest_payload_to_native,
};
#[cfg(feature = "ssr")]
use serde::Deserialize;

#[server(prefix = "/api/fn", endpoint = "auth/sign-in")]
pub(super) async fn sign_in_native(
    email: String,
    password: String,
    tenant: String,
) -> Result<NativeAuthPayload, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        let response: RestAuthResponse = auth_rest_post(
            "/api/auth/login",
            &json!({
                "email": email,
                "password": password,
            }),
            None,
            Some(tenant.clone()),
        )
        .await?;

        Ok(rest_payload_to_native(response, tenant))
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (email, password, tenant);
        Err(ServerFnError::new(
            "auth/sign-in requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "auth/sign-up")]
pub(super) async fn sign_up_native(
    email: String,
    password: String,
    name: Option<String>,
    tenant: String,
) -> Result<NativeAuthPayload, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        let response: RestAuthResponse = auth_rest_post(
            "/api/auth/register",
            &json!({
                "email": email,
                "password": password,
                "name": name,
            }),
            None,
            Some(tenant.clone()),
        )
        .await?;

        Ok(rest_payload_to_native(response, tenant))
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (email, password, name, tenant);
        Err(ServerFnError::new(
            "auth/sign-up requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "auth/sign-out")]
pub(super) async fn sign_out_native(
    refresh_token: String,
    tenant: String,
) -> Result<(), ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        let _response: RestStatusResponse = auth_rest_post(
            "/api/auth/logout",
            &json!({
                "refresh_token": refresh_token,
            }),
            None,
            Some(tenant),
        )
        .await?;

        Ok(())
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (refresh_token, tenant);
        Err(ServerFnError::new(
            "auth/sign-out requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "auth/refresh-token")]
pub(super) async fn refresh_token_native(
    refresh_tok: String,
    tenant: String,
) -> Result<NativeAuthPayload, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        let response: RestAuthResponse = auth_rest_post(
            "/api/auth/refresh",
            &json!({
                "refresh_token": refresh_tok,
            }),
            None,
            Some(tenant.clone()),
        )
        .await?;

        Ok(rest_payload_to_native(response, tenant))
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (refresh_tok, tenant);
        Err(ServerFnError::new(
            "auth/refresh-token requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "auth/forgot-password")]
pub(super) async fn forgot_password_native(
    email: String,
    tenant: String,
) -> Result<String, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        let _response: RestStatusResponse = auth_rest_post(
            "/api/auth/reset/request",
            &json!({
                "email": email,
            }),
            None,
            Some(tenant),
        )
        .await?;

        Ok(RESET_REQUEST_MESSAGE.to_string())
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (email, tenant);
        Err(ServerFnError::new(
            "auth/forgot-password requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "auth/current-user")]
pub(super) async fn current_user_native(
    token: String,
    tenant: String,
) -> Result<NativeCurrentUserPayload, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        let response: RestUserResponse =
            auth_rest_get("/api/auth/me", Some(token), Some(tenant)).await?;

        Ok(NativeCurrentUserPayload {
            user: Some(AuthUser {
                id: response.id,
                email: response.email,
                name: response.name,
                role: response.role,
            }),
        })
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (token, tenant);
        Err(ServerFnError::new(
            "auth/current-user requires the `ssr` feature",
        ))
    }
}

#[cfg(feature = "ssr")]
async fn auth_rest_post<T>(
    path: &str,
    body: &serde_json::Value,
    token: Option<String>,
    tenant: Option<String>,
) -> Result<T, ServerFnError>
where
    T: for<'de> Deserialize<'de>,
{
    use reqwest::header::{AUTHORIZATION, CONTENT_TYPE};

    let client = reqwest::Client::new();
    let mut request = client
        .post(format!("{}{}", get_api_url(), path))
        .header(CONTENT_TYPE, "application/json")
        .json(body);

    if let Some(token) = token {
        request = request.header(AUTHORIZATION, format!("Bearer {token}"));
    }
    if let Some(tenant) = tenant {
        request = request.header("X-Tenant-ID", tenant);
    }

    let response = request.send().await.map_err(ServerFnError::new)?;
    if !response.status().is_success() {
        return Err(ServerFnError::new(rest_error_message(response).await));
    }

    response.json::<T>().await.map_err(ServerFnError::new)
}

#[cfg(feature = "ssr")]
async fn auth_rest_get<T>(
    path: &str,
    token: Option<String>,
    tenant: Option<String>,
) -> Result<T, ServerFnError>
where
    T: for<'de> Deserialize<'de>,
{
    use reqwest::header::AUTHORIZATION;

    let client = reqwest::Client::new();
    let mut request = client.get(format!("{}{}", get_api_url(), path));

    if let Some(token) = token {
        request = request.header(AUTHORIZATION, format!("Bearer {token}"));
    }
    if let Some(tenant) = tenant {
        request = request.header("X-Tenant-ID", tenant);
    }

    let response = request.send().await.map_err(ServerFnError::new)?;
    if !response.status().is_success() {
        return Err(ServerFnError::new(rest_error_message(response).await));
    }

    response.json::<T>().await.map_err(ServerFnError::new)
}

#[cfg(feature = "ssr")]
async fn rest_error_message(response: reqwest::Response) -> String {
    let status = response.status();
    let text = response.text().await.unwrap_or_default();
    let trimmed = text.trim();

    if trimmed.is_empty() {
        return format!("request failed with status {status}");
    }

    if let Ok(payload) = serde_json::from_str::<RestApiErrorPayload>(trimmed) {
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
