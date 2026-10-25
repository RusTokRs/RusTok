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

// ─── BFF refresh-cookie plumbing (SSR host side) ────────────────────────────
//
// The refresh token never crosses into browser-readable state: native
// server functions receive it from the API, stash it in an HttpOnly
// cookie on the SSR host's own origin, and hand the browser only the
// short-lived access token. Refresh and sign-out recover the token
// from that cookie on the server side of the server-function call.

#[cfg(feature = "ssr")]
async fn request_headers() -> Result<axum::http::HeaderMap, ServerFnError> {
    leptos_axum::extract::<axum::http::HeaderMap>()
        .await
        .map_err(ServerFnError::new)
}

#[cfg(feature = "ssr")]
fn cookie_request_is_secure(headers: &axum::http::HeaderMap) -> bool {
    let forwarded_https = headers
        .get("x-forwarded-proto")
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value.eq_ignore_ascii_case("https"));
    let origin_https = headers
        .get(axum::http::header::ORIGIN)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value.starts_with("https://"));
    forwarded_https || origin_https
}

#[cfg(feature = "ssr")]
fn set_response_cookie(cookie: String) -> Result<(), ServerFnError> {
    let options = use_context::<leptos_axum::ResponseOptions>()
        .ok_or_else(|| ServerFnError::new("response options are unavailable"))?;
    let value = axum::http::HeaderValue::from_str(&cookie).map_err(ServerFnError::new)?;
    options.insert_header(axum::http::header::SET_COOKIE, value);
    Ok(())
}

#[cfg(feature = "ssr")]
fn persist_refresh_cookie(
    headers: &axum::http::HeaderMap,
    tenant: &str,
    refresh_token: &str,
) -> Result<(), ServerFnError> {
    set_response_cookie(crate::refresh_cookie_header(
        tenant,
        refresh_token,
        cookie_request_is_secure(headers),
    ))
}

#[cfg(feature = "ssr")]
fn clear_refresh_cookie(headers: &axum::http::HeaderMap) -> Result<(), ServerFnError> {
    set_response_cookie(crate::clear_refresh_cookie_header(
        cookie_request_is_secure(headers),
    ))
}

#[cfg(feature = "ssr")]
fn refresh_cookie_from(headers: &axum::http::HeaderMap) -> Option<(String, String)> {
    headers
        .get(axum::http::header::COOKIE)?
        .to_str()
        .ok()
        .and_then(crate::extract_refresh_cookie)
}

/// Exchange a refresh token directly against the API from trusted
/// server-side code (SSR snapshot middleware). Returns the browser
/// payload plus the rotated refresh token so the host can re-issue
/// its BFF cookie; the token itself is never rendered or sent to the
/// browser in the payload.
#[cfg(feature = "ssr")]
pub async fn exchange_refresh_token_ssr(
    refresh_token: String,
    tenant: String,
) -> Result<(NativeAuthPayload, String), ServerFnError> {
    let response: RestAuthResponse = auth_rest_post(
        "/api/auth/refresh",
        &json!({
            "refresh_token": refresh_token,
        }),
        None,
        Some(tenant.clone()),
    )
    .await?;

    let rotated = response.refresh_token.clone();
    Ok((rest_payload_to_native(response, tenant), rotated))
}

#[server(prefix = "/api/fn", endpoint = "auth/sign-in")]
pub(super) async fn sign_in_native(
    email: String,
    password: String,
    tenant: String,
) -> Result<NativeAuthPayload, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        let headers = request_headers().await?;
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

        persist_refresh_cookie(&headers, &tenant, &response.refresh_token)?;
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
        let headers = request_headers().await?;
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

        persist_refresh_cookie(&headers, &tenant, &response.refresh_token)?;
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
pub(super) async fn sign_out_native(tenant: String) -> Result<(), ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        let headers = request_headers().await?;
        // Best-effort server-side revocation; the cookie is cleared
        // either way so the browser always ends up logged out.
        if let Some((cookie_tenant, refresh_token)) = refresh_cookie_from(&headers) {
            if cookie_tenant == tenant {
                let _result: Result<RestStatusResponse, ServerFnError> = auth_rest_post(
                    "/api/auth/logout",
                    &json!({
                        "refresh_token": refresh_token,
                    }),
                    None,
                    Some(tenant),
                )
                .await;
            }
        }
        clear_refresh_cookie(&headers)?;
        Ok(())
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = tenant;
        Err(ServerFnError::new(
            "auth/sign-out requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "auth/refresh-token")]
pub(super) async fn refresh_token_native(
    tenant: String,
) -> Result<NativeAuthPayload, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        let headers = request_headers().await?;
        let (cookie_tenant, refresh_token) =
            refresh_cookie_from(&headers).ok_or_else(|| ServerFnError::new("Unauthorized"))?;
        if cookie_tenant != tenant {
            return Err(ServerFnError::new("Unauthorized"));
        }

        let (payload, rotated) = exchange_refresh_token_ssr(refresh_token, tenant.clone()).await?;
        persist_refresh_cookie(&headers, &tenant, &rotated)?;
        Ok(payload)
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = tenant;
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

    if let Ok(payload) = serde_json::from_str::<RestApiErrorPayload>(trimmed)
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
