//! Direct browser authentication against the server's HttpOnly cookie
//! session endpoints (`/api/auth/session/*`).
//!
//! The refresh token is held by the server in an HttpOnly cookie and
//! never enters JavaScript-readable memory: responses contain only
//! the short-lived access token and the user profile. This path
//! requires the API to be same-origin with the page (the default
//! deployment); cross-origin API bases cannot persist the cookie and
//! should use the SSR (native) transport instead.

use serde::Deserialize;
use serde_json::json;

use super::{get_api_url, now_unix_secs};
use crate::{AuthError, AuthSession, AuthUser};

#[derive(Debug, Deserialize)]
struct CookieSessionResponse {
    access_token: String,
    #[allow(dead_code)]
    token_type: String,
    expires_in: u64,
    user: CookieUserInfo,
}

#[derive(Debug, Deserialize)]
struct CookieUserInfo {
    id: String,
    email: String,
    name: Option<String>,
    role: String,
    #[allow(dead_code)]
    status: String,
}

#[derive(Debug, Deserialize)]
struct StatusResponse {
    #[allow(dead_code)]
    status: String,
}

fn to_user_session(payload: CookieSessionResponse, tenant: String) -> (AuthUser, AuthSession) {
    let user = AuthUser {
        id: payload.user.id,
        email: payload.user.email,
        name: payload.user.name,
        role: payload.user.role,
    };
    let session = AuthSession {
        token: payload.access_token,
        expires_at: now_unix_secs() + payload.expires_in as i64,
        tenant,
    };
    (user, session)
}

async fn post_session(
    path: &str,
    tenant: &str,
    body: Option<serde_json::Value>,
    session_request: bool,
    is_login: bool,
) -> Result<CookieSessionResponse, AuthError> {
    let client = reqwest::Client::new();
    let mut request = client
        .post(format!("{}{path}", get_api_url()))
        .header("X-Tenant-ID", tenant);
    if session_request {
        // CSRF barrier required by the cookie-authenticated endpoints.
        request = request.header("x-rustok-session-request", "1");
    }
    if let Some(body) = body {
        request = request.json(&body);
    }

    let response = request.send().await.map_err(|_| AuthError::Network)?;
    if !response.status().is_success() {
        return Err(AuthError::from_status(
            response.status().as_u16(),
            is_login,
        ));
    }

    response
        .json::<CookieSessionResponse>()
        .await
        .map_err(|_| AuthError::Network)
}

pub(super) async fn sign_in_cookie(
    email: String,
    password: String,
    tenant: String,
) -> Result<(AuthUser, AuthSession), AuthError> {
    let payload = post_session(
        "/api/auth/session/login",
        &tenant,
        Some(json!({ "email": email, "password": password })),
        false,
        true,
    )
    .await?;
    Ok(to_user_session(payload, tenant))
}

pub(super) async fn sign_up_cookie(
    email: String,
    password: String,
    name: Option<String>,
    tenant: String,
) -> Result<(AuthUser, AuthSession), AuthError> {
    let payload = post_session(
        "/api/auth/session/register",
        &tenant,
        Some(json!({ "email": email, "password": password, "name": name })),
        false,
        false,
    )
    .await?;
    Ok(to_user_session(payload, tenant))
}

pub(super) async fn refresh_session_cookie(
    tenant: String,
) -> Result<(AuthSession, AuthUser), AuthError> {
    let payload = post_session("/api/auth/session/refresh", &tenant, None, true, false).await?;
    let (user, session) = to_user_session(payload, tenant);
    Ok((session, user))
}

pub(super) async fn sign_out_cookie(tenant: String) -> Result<(), AuthError> {
    let client = reqwest::Client::new();
    let response = client
        .post(format!("{}/api/auth/session/logout", get_api_url()))
        .header("X-Tenant-ID", tenant)
        .header("x-rustok-session-request", "1")
        .send()
        .await
        .map_err(|_| AuthError::Network)?;
    if !response.status().is_success() {
        return Err(AuthError::from_status(response.status().as_u16(), false));
    }
    let _ = response.json::<StatusResponse>().await;
    Ok(())
}
