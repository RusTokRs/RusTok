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
fn refresh_cookie_from(
    headers: &axum::http::HeaderMap,
) -> Result<Option<(String, String)>, ServerFnError> {
    let mut values = Vec::new();
    for value in headers.get_all(axum::http::header::COOKIE).iter() {
        values.push(
            value
                .to_str()
                .map_err(|_| ServerFnError::new("Invalid authentication cookie"))?,
        );
    }
    crate::cookie::inspect_refresh_cookie_headers(values)
        .map_err(|_| ServerFnError::new("Invalid authentication cookie"))
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
        // Do not report logout or clear a different tenant's cookie. If revocation fails,
        // preserve the cookie for recovery and leave the caller's scope unverified.
        if let Some((cookie_tenant, refresh_token)) = refresh_cookie_from(&headers)? {
            if cookie_tenant != tenant {
                return Err(ServerFnError::new("Authentication tenant mismatch"));
            }
            // Use the API's idempotent cookie-session logout contract: an already expired
            // or revoked refresh token is a completed logout, while persistence failures
            // remain errors. This Cookie header is server-to-server only, never rendered.
            auth_rest_cookie_logout(&tenant, &refresh_token).await?;
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
) -> Result<Option<NativeAuthPayload>, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        let headers = request_headers().await?;
        let Some((cookie_tenant, refresh_token)) = refresh_cookie_from(&headers)? else {
            // Only an explicitly absent cookie is evidence of a guest. Invalid, duplicate,
            // foreign, expired or revoked cookies all follow an error path instead.
            return Ok(None);
        };
        if cookie_tenant != tenant {
            return Err(ServerFnError::new("Authentication tenant mismatch"));
        }

        let (payload, rotated) = exchange_refresh_token_ssr(refresh_token, tenant.clone()).await?;
        persist_refresh_cookie(&headers, &tenant, &rotated)?;
        Ok(Some(payload))
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = tenant;
        Err(ServerFnError::new(
            "auth/refresh-token requires the `ssr` feature",
        ))
    }
}

#[cfg(feature = "ssr")]
#[derive(Debug, Deserialize)]
struct RestAcceptInviteResponse {
    #[allow(dead_code)]
    status: String,
    email: String,
    #[allow(dead_code)]
    role: String,
}

#[server(prefix = "/api/fn", endpoint = "auth/accept-invite")]
pub(super) async fn accept_invite_native(
    invite_token: String,
    password: String,
    name: Option<String>,
    tenant: String,
) -> Result<String, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        let response: RestAcceptInviteResponse = auth_rest_post(
            "/api/auth/invite/accept",
            &json!({
                "token": invite_token,
                "password": password,
                "name": name,
            }),
            None,
            Some(tenant),
        )
        .await?;
        Ok(response.email)
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (invite_token, password, name, tenant);
        Err(ServerFnError::new(
            "auth/accept-invite requires the `ssr` feature",
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
                permissions: response.permissions,
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
fn api_session_cookie(refresh_token: &str) -> Result<reqwest::header::HeaderValue, ServerFnError> {
    // Only RFC 6265 cookie-octets can be forwarded without changing the meaning of the
    // Cookie header. This is syntax validation, not validation of the refresh credential.
    if refresh_token.is_empty()
        || refresh_token.len() > 4096
        || !refresh_token.bytes().all(
            |byte| matches!(byte, 0x21 | 0x23..=0x2b | 0x2d..=0x3a | 0x3c..=0x5b | 0x5d..=0x7e),
        )
    {
        return Err(ServerFnError::new("Invalid authentication cookie"));
    }
    let mut cookie =
        reqwest::header::HeaderValue::from_str(&format!("rustok-session={refresh_token}"))
            .map_err(|_| ServerFnError::new("Invalid authentication cookie"))?;
    cookie.set_sensitive(true);
    Ok(cookie)
}

#[cfg(feature = "ssr")]
fn confirm_cookie_logout(response: RestStatusResponse) -> Result<(), ServerFnError> {
    if response.status == "ok" {
        Ok(())
    } else {
        Err(ServerFnError::new("Logout could not be confirmed"))
    }
}

#[cfg(feature = "ssr")]
async fn auth_rest_cookie_logout(tenant: &str, refresh_token: &str) -> Result<(), ServerFnError> {
    auth_rest_cookie_logout_at(&get_api_url(), tenant, refresh_token).await
}

#[cfg(feature = "ssr")]
async fn auth_rest_cookie_logout_at(
    api_url: &str,
    tenant: &str,
    refresh_token: &str,
) -> Result<(), ServerFnError> {
    let cookie = api_session_cookie(refresh_token)?;
    // A redirect must never forward a bearer-like cookie to a different endpoint.
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(ServerFnError::new)?;
    let response = client
        .post(format!(
            "{}/api/auth/session/logout",
            api_url.trim_end_matches('/')
        ))
        .header(reqwest::header::COOKIE, cookie)
        .header("X-Tenant-ID", tenant)
        .header("x-rustok-session-request", "1")
        .send()
        .await
        .map_err(|_| ServerFnError::new("Logout request failed"))?;
    if !response.status().is_success() {
        // Do not copy a potentially credential-bearing upstream body into diagnostics.
        return Err(ServerFnError::new(format!(
            "Logout request failed with status {}",
            response.status().as_u16()
        )));
    }
    let response: RestStatusResponse = response
        .json()
        .await
        .map_err(|_| ServerFnError::new("Logout could not be confirmed"))?;
    confirm_cookie_logout(response)
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

#[cfg(all(test, feature = "ssr"))]
mod tests {
    use super::{
        RestStatusResponse, api_session_cookie, auth_rest_cookie_logout_at, confirm_cookie_logout,
        refresh_cookie_from,
    };
    use axum::http::{HeaderMap, HeaderValue, header::COOKIE};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    async fn mock_logout_api(response: String) -> (String, tokio::task::JoinHandle<String>) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let received = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut request = Vec::new();
            let mut chunk = [0u8; 1024];
            while !request.windows(4).any(|window| window == b"\r\n\r\n") {
                let read = socket.read(&mut chunk).await.unwrap();
                assert!(read > 0 && request.len() + read <= 16_384);
                request.extend_from_slice(&chunk[..read]);
            }
            socket.write_all(response.as_bytes()).await.unwrap();
            String::from_utf8(request).unwrap()
        });
        (format!("http://{address}"), received)
    }

    fn http_reply(status: &str, body: &str, extra_headers: &str) -> String {
        format!(
            "HTTP/1.1 {status}\r\ncontent-length: {}\r\nconnection: close\r\n{extra_headers}\r\n{body}",
            body.len()
        )
    }

    #[tokio::test]
    async fn native_logout_uses_cookie_session_contract_and_requires_confirmation() {
        let (url, received) = mock_logout_api(http_reply("200 OK", r#"{"status":"ok"}"#, "")).await;
        assert!(
            auth_rest_cookie_logout_at(&url, "shop", "fake-token")
                .await
                .is_ok()
        );
        let request = received.await.unwrap().to_ascii_lowercase();
        assert!(request.starts_with("post /api/auth/session/logout http/1.1\r\n"));
        assert!(request.contains("\r\ncookie: rustok-session=fake-token\r\n"));
        assert!(request.contains("\r\nx-tenant-id: shop\r\n"));
        assert!(request.contains("\r\nx-rustok-session-request: 1\r\n"));

        let (url, received) =
            mock_logout_api(http_reply("503 Service Unavailable", "fake-token", "")).await;
        let error = auth_rest_cookie_logout_at(&url, "shop", "fake-token")
            .await
            .unwrap_err();
        assert!(!error.to_string().contains("fake-token"));
        received.await.unwrap();

        let (url, received) =
            mock_logout_api(http_reply("200 OK", r#"{"status":"unknown"}"#, "")).await;
        assert!(
            auth_rest_cookie_logout_at(&url, "shop", "fake-token")
                .await
                .is_err()
        );
        received.await.unwrap();
    }

    #[tokio::test]
    async fn native_logout_never_follows_a_redirect_with_the_cookie() {
        let (url, received) = mock_logout_api(http_reply(
            "307 Temporary Redirect",
            "",
            "location: http://127.0.0.1:1/not-the-auth-service\r\n",
        ))
        .await;
        let error = auth_rest_cookie_logout_at(&url, "shop", "fake-token")
            .await
            .unwrap_err();
        assert!(error.to_string().contains("status 307"));
        received.await.unwrap();
    }

    #[test]
    fn logout_forwarding_is_sensitive_and_never_accepts_cookie_injection() {
        let cookie = api_session_cookie("abc-123_~.token").unwrap();
        assert!(cookie.is_sensitive());
        assert_eq!(cookie.to_str().unwrap(), "rustok-session=abc-123_~.token");
        for invalid in ["", "has space", "x;other=y", "x,y", "x\"y", "x\\y", "x\ny"] {
            assert!(api_session_cookie(invalid).is_err());
        }
        assert!(api_session_cookie(&"a".repeat(4097)).is_err());
    }

    #[test]
    fn logout_requires_the_expected_api_confirmation() {
        assert!(
            confirm_cookie_logout(RestStatusResponse {
                status: "ok".into()
            })
            .is_ok()
        );
        assert!(
            confirm_cookie_logout(RestStatusResponse {
                status: "error".into()
            })
            .is_err()
        );
    }

    #[test]
    fn native_refresh_distinguishes_absence_from_invalid_and_duplicate_cookies() {
        let mut headers = HeaderMap::new();
        assert_eq!(refresh_cookie_from(&headers).unwrap(), None);
        headers.append(COOKIE, HeaderValue::from_static("other=value"));
        assert_eq!(refresh_cookie_from(&headers).unwrap(), None);
        headers.append(
            COOKIE,
            HeaderValue::from_static("rustok-admin-refresh-v1=shop.token"),
        );
        assert_eq!(
            refresh_cookie_from(&headers).unwrap(),
            Some(("shop".into(), "token".into()))
        );
        headers.append(
            COOKIE,
            HeaderValue::from_static("rustok-admin-refresh-v1=shop.token"),
        );
        assert!(refresh_cookie_from(&headers).is_err());

        let mut malformed = HeaderMap::new();
        malformed.insert(
            COOKIE,
            HeaderValue::from_static("rustok-admin-refresh-v1=not-a-token"),
        );
        assert!(refresh_cookie_from(&malformed).is_err());
        malformed.insert(COOKIE, HeaderValue::from_bytes(&[0xff]).unwrap());
        assert!(refresh_cookie_from(&malformed).is_err());
    }
}
